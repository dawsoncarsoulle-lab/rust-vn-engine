use crate::{
    DiagnosticSeverity, GraphDiagnostic, GraphDocument, GraphKind, GraphNode, GraphPin, NodeId,
    NodeKind, PinDirection, PinId, PropertyValue,
};
use rvn_parser::Script;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub struct TranspiledScript {
    pub source: String,
    pub ast: Script,
}

pub fn transpile(graph: &GraphDocument) -> Result<TranspiledScript, TranspileError> {
    let diagnostics = graph.validate();
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return Err(TranspileError::InvalidGraph(diagnostics));
    }

    let source = Emitter { graph }.emit_document()?;
    let ast = rvn_parser::parse(&source)
        .map_err(|error| TranspileError::GeneratedSourceInvalid(error.to_string()))?;
    Ok(TranspiledScript { source, ast })
}

struct Emitter<'a> {
    graph: &'a GraphDocument,
}

/// Validate editor expressions without accepting extra injected statements.
pub fn validate_expression(expression: &str) -> Result<(), TranspileError> {
    let parsed = rvn_parser::parse(&format!("set __editor_check = {expression}\n"))
        .map_err(|error| TranspileError::GeneratedSourceInvalid(error.to_string()))?;
    if !matches!(parsed.as_slice(), [rvn_parser::Statement::SetVar { .. }]) {
        return Err(TranspileError::GeneratedSourceInvalid("Une seule expression RVN est attendue".into()));
    }
    Ok(())
}

impl Emitter<'_> {
    fn emit_document(&self) -> Result<String, TranspileError> {
        let (root_kind, header) = match &self.graph.kind {
            GraphKind::Label { name } => (
                NodeKind::Label,
                format!("label {}\n", validate_identifier(name)?),
            ),
            GraphKind::Init => (NodeKind::Init, "init {\n".to_owned()),
            kind => return Err(TranspileError::UnsupportedGraphKind(kind.clone())),
        };
        let roots: Vec<_> = self
            .graph
            .nodes
            .values()
            .filter(|node| node.kind == root_kind && (root_kind != NodeKind::Label
                || !matches!(node.properties.get("label"), Some(PropertyValue::String(name)) if !name.is_empty())))
            .map(|node| node.id)
            .collect();
        let [root] = roots.as_slice() else {
            return Err(TranspileError::ExpectedSingleRoot {
                kind: root_kind,
                found: roots.len(),
            });
        };

        let mut source = String::new();
        for node in self
            .graph
            .nodes
            .values()
            .filter(|node| node.kind == NodeKind::Use)
        {
            let paths = self.pin_string_list(node.id, "paths")?;
            if paths.len() == 1 {
                source.push_str("use ");
                source.push_str(&quote(&paths[0]));
                source.push('\n');
            } else if !paths.is_empty() {
                source.push_str("use { ");
                source.push_str(
                    &paths
                        .iter()
                        .map(|path| quote(path))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                source.push_str(" }\n");
            }
        }
        if !self.graph.characters.is_empty() {
            source.push_str("init {\n");
            for (id, name) in &self.graph.characters {
                validate_identifier(id)?;
                source.push_str(&format!("    character.create({}, {})\n", quote(id), quote(name)));
            }
            source.push_str("}\n");
        }
        source.push_str(&header);
        self.emit_sequence(
            self.successor(*root, "exec_out")?,
            1,
            None,
            &mut source,
            &mut BTreeSet::new(),
        )?;
        if matches!(self.graph.kind, GraphKind::Init) {
            source.push_str("}\n");
        }
        if let GraphKind::Label { name } = &self.graph.kind {
            let mut names = BTreeSet::from([name.clone()]);
            for node in self.graph.nodes.values().filter(|n| n.kind == NodeKind::Label && n.id != *root) {
                let label = property_string(node.id, &node.properties, "label")?;
                validate_identifier(&label)?;
                if !names.insert(label.clone()) {
                    return Err(TranspileError::GeneratedSourceInvalid(format!("Label dupliqué : {label}")));
                }
                source.push_str(&format!("\nlabel {label}\n"));
                self.emit_sequence(self.successor(node.id, "exec_out")?, 1, None, &mut source, &mut BTreeSet::new())?;
            }
        }
        Ok(source)
    }

    fn emit_sequence(
        &self,
        mut current: Option<NodeId>,
        indent: usize,
        branch_owner: Option<NodeId>,
        source: &mut String,
        visited: &mut BTreeSet<NodeId>,
    ) -> Result<(), TranspileError> {
        while let Some(node_id) = current {
            if !visited.insert(node_id) {
                return Err(TranspileError::ExecutionCycle(node_id));
            }
            let node = self
                .graph
                .nodes
                .get(&node_id)
                .ok_or(TranspileError::NodeNotFound(node_id))?;
            if node.kind == NodeKind::BranchEnd {
                let owner = property_int(node_id, &node.properties, "owner")? as u64;
                if branch_owner != Some(NodeId::new(owner)) {
                    return Err(TranspileError::WrongBranchEnd {
                        node: node_id,
                        expected_owner: branch_owner,
                        found_owner: NodeId::new(owner),
                    });
                }
                return Ok(());
            }

            current = match node.kind {
                NodeKind::Reroute => self.successor(node_id, "value_out")?,
                NodeKind::Dialogue => {
                    let character = self.pin_string(node_id, "character")?;
                    let text = self.pin_interpolated_text(node_id, "text")?;
                    if !character.is_empty() {
                        self.emit_character_appearance(node_id, &character, indent, source)?;
                    }
                    push_indent(source, indent);
                    if !character.is_empty() {
                        source.push_str(validate_identifier(&character)?);
                        source.push(' ');
                    }
                    source.push_str(&quote(&text));
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SetVariable => {
                    let name = match node.properties.get("name") {
                        Some(PropertyValue::String(name)) => name.clone(),
                        _ => self.pin_string(node_id, "name")?,
                    };
                    let value = self.expression_from_input(node_id, "value")?;
                    push_indent(source, indent);
                    source.push_str("set ");
                    source.push_str(validate_variable_name(&name)?);
                    source.push_str(" = ");
                    source.push_str(&value);
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::Scene => {
                    let background = self.pin_string(node_id, "background")?;
                    let transition = self.pin_string(node_id, "transition")?;
                    push_indent(source, indent);
                    source.push_str("scene ");
                    source.push_str(&quote(&background));
                    if !transition.is_empty() && transition != "none" {
                        source.push_str(" with ");
                        source.push_str(&transition);
                    }
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::Config => {
                    let key = self.pin_string(node_id, "key")?;
                    let value = self.pin_string(node_id, "value")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&key)?);
                    source.push_str(": ");
                    source.push_str(&quote(&value));
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::CharacterCreate => {
                    let id = self.pin_string(node_id, "id")?;
                    let display_name = self.pin_string(node_id, "display_name")?;
                    push_indent(source, indent);
                    source.push_str("character.create(");
                    source.push_str(&quote(&id));
                    source.push_str(", ");
                    source.push_str(&quote(&display_name));
                    source.push_str(")\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::CinematicShow => {
                    let cinematic = self.pin_string(node_id, "cinematic")?;
                    push_indent(source, indent);
                    source.push_str("cinematic ");
                    source.push_str(&quote(&cinematic));
                    self.push_cinematic_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::CinematicHide => {
                    push_indent(source, indent);
                    source.push_str("cinematic hide");
                    self.push_cinematic_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::UnlockEnding => {
                    let id = self.pin_string(node_id, "id")?;
                    push_indent(source, indent);
                    source.push_str("unlock_ending ");
                    source.push_str(&quote(&id));
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteShow => {
                    let character = self.pin_string(node_id, "character")?;
                    let emotion = self.connected_sprite(node_id)?.ok_or(TranspileError::MissingInputValue { node: node_id, key: "Sprite : reliez une image au personnage".into() })?;
                    let position = self.pin_string(node_id, "position")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(".show(");
                    source.push_str(&quote(&emotion));
                    source.push(')');
                    if !position.is_empty() {
                        source.push_str(" at ");
                        source.push_str(&position);
                    }
                    self.push_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteHide => {
                    let character = self.pin_string(node_id, "character")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(".hide()");
                    self.push_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteMove => {
                    let character = self.pin_string(node_id, "character")?;
                    let position = self.pin_string(node_id, "position")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(".move() at ");
                    source.push_str(&position);
                    self.push_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteAnimate => {
                    let character = self.pin_string(node_id, "character")?;
                    let animation = self.pin_string(node_id, "animation")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(".animate(");
                    source.push_str(&quote(&animation));
                    if let Some(PropertyValue::StringList(params)) = node.properties.get("params") {
                        for param in params {
                            let (name, value) = param.split_once('=').ok_or_else(|| {
                                TranspileError::InvalidDynamicParameter(param.clone())
                            })?;
                            if node.properties.contains_key(&format!("animation_param_{}", name.trim())) { continue; }
                            source.push_str(", ");
                            source.push_str(validate_identifier(name.trim())?);
                            source.push_str(": ");
                            source.push_str(value.trim());
                        }
                    }
                    for (key, value) in &node.properties {
                        if let Some(name) = key.strip_prefix("animation_param_") {
                            source.push_str(", ");
                            source.push_str(validate_identifier(name)?);
                            source.push_str(": ");
                            source.push_str(&emit_property_expression(value)?);
                        }
                    }
                    source.push_str(")\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteStopAnimation => {
                    let character = self.pin_string(node_id, "character")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(".stop_animation()\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteEffect => {
                    let character = self.pin_string(node_id, "character")?;
                    let mut effects = Vec::new();
                    for key in ["flip_x", "flip_y", "scale", "rotation", "tint"] {
                        // Missing flags mean legacy behavior: all effects were applied.
                        if node.properties.get(&format!("apply_{key}")) == Some(&PropertyValue::Bool(false)) { continue; }
                        let value = match key {
                            "flip_x" | "flip_y" => self.pin_bool(node_id, key)?.to_string(),
                            "tint" => match self.pin_string(node_id, key) {
                                Ok(color) => quote(&color),
                                Err(_) => quote(&format!("[{}]", self.expression_from_input(node_id, key)?)),
                            },
                            _ => self.pin_float(node_id, key)?.to_string(),
                        };
                        effects.push(format!("{key}: {value}"));
                    }
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(&format!(".effect({})\n", effects.join(", ")));
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::Timer => {
                    let duration = self.pin_float(node_id, "duration")?;
                    let target = self.pin_string(node_id, "target")?;
                    push_indent(source, indent);
                    source.push_str(&format!(
                        "timer {duration} => jump {}\n",
                        validate_identifier(&target)?
                    ));
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::TimerCancel => {
                    push_indent(source, indent);
                    source.push_str("timer cancel\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::MethodCall => {
                    let target = self.pin_string(node_id, "target")?;
                    let method = self.pin_string(node_id, "method")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&target)?);
                    source.push('.');
                    source.push_str(validate_identifier(&method)?);
                    source.push('(');
                    let arg = self.pin_string(node_id, "arg")?;
                    if !arg.is_empty() {
                        source.push_str(&quote(&arg));
                    }
                    source.push(')');
                    self.push_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::MusicPlay => {
                    let file = self.pin_string(node_id, "file")?;
                    push_indent(source, indent);
                    source.push_str("music.play(");
                    source.push_str(&quote(&file));
                    source.push(')');
                    self.push_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::MusicStop => {
                    push_indent(source, indent);
                    source.push_str("music.stop()");
                    self.push_optional_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::MusicVolume => {
                    let level = self.pin_float(node_id, "level")?;
                    // Le parseur historique des appels de méthode reçoit ses
                    // arguments sous forme de chaînes avant de convertir le volume.
                    push_indent(source, indent);
                    source.push_str(&format!("music.volume(\"{level}\")\n"));
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SfxPlay | NodeKind::SfxStop => {
                    let file = self.pin_string(node_id, "file")?;
                    push_indent(source, indent);
                    source.push_str(if node.kind == NodeKind::SfxPlay {
                        "sfx.play("
                    } else {
                        "sfx.stop("
                    });
                    source.push_str(&quote(&file));
                    source.push_str(")\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::VoicePlay => {
                    let file = self.pin_string(node_id, "file")?;
                    push_indent(source, indent);
                    source.push_str("voice ");
                    source.push_str(&quote(&file));
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::VoiceStop => {
                    push_indent(source, indent);
                    source.push_str("voice stop\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::Imagemap => {
                    let background = self.pin_string(node_id, "background")?;
                    let hover = self.pin_string(node_id, "hover")?;
                    push_indent(source, indent);
                    source.push_str("imagemap {\n");
                    push_indent(source, indent + 1);
                    source.push_str("background: ");
                    source.push_str(&quote(&background));
                    source.push('\n');
                    if !hover.is_empty() {
                        push_indent(source, indent + 1);
                        source.push_str("hover: ");
                        source.push_str(&quote(&hover));
                        source.push('\n');
                    }
                    if let Some(PropertyValue::StringList(hotspots)) =
                        node.properties.get("hotspots")
                    {
                        for (index, hotspot) in hotspots.iter().enumerate() {
                            let parts: Vec<_> = hotspot.split(':').map(str::trim).collect();
                            if parts.len() != 5 && parts.len() != 9 {
                                return Err(TranspileError::InvalidHotspot(hotspot.clone()));
                            }
                            let coordinates: Vec<i32> = parts[1..]
                                .iter()
                                .map(|value| value.parse::<i32>())
                                .collect::<Result<_, _>>()
                                .map_err(|_| TranspileError::InvalidHotspot(hotspot.clone()))?;
                            if coordinates[2] <= coordinates[0] || coordinates[3] <= coordinates[1]
                            {
                                return Err(TranspileError::InvalidHotspot(hotspot.clone()));
                            }
                            push_indent(source, indent + 1);
                            source.push_str("hotspot { name: ");
                            source.push_str(&quote(parts[0]));
                            source.push_str(&format!(
                                " area: ({}, {}, {}, {})",
                                coordinates[0], coordinates[1], coordinates[2], coordinates[3]
                            ));
                            if coordinates.len() == 8 {
                                if coordinates[6] <= coordinates[4] || coordinates[7] <= coordinates[5] {
                                    return Err(TranspileError::InvalidHotspot(hotspot.clone()));
                                }
                                source.push_str(&format!(" hover_area: ({}, {}, {}, {})", coordinates[4], coordinates[5], coordinates[6], coordinates[7]));
                            }
                            source.push_str(" } => {\n");
                            self.emit_sequence(
                                self.successor(node_id, &format!("hotspot_{index}"))?,
                                indent + 2,
                                Some(node_id),
                                source,
                                visited,
                            )?;
                            push_indent(source, indent + 1);
                            source.push_str("}\n");
                        }
                    }
                    push_indent(source, indent);
                    source.push_str("}\n");
                    self.successor(node_id, "completed")?
                }
                NodeKind::TypewriterSet => {
                    let enabled = self.pin_bool(node_id, "enabled")?;
                    push_indent(source, indent);
                    source.push_str(&format!("typewriter = {enabled}\n"));
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::TypewriterSpeed => {
                    let speed = self.pin_int(node_id, "speed")?;
                    push_indent(source, indent);
                    source.push_str(&format!("typewriter.speed({speed})\n"));
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::If => {
                    self.emit_if(node_id, indent, source, visited)?;
                    self.successor(node_id, "completed")?
                }
                NodeKind::Choice => {
                    self.emit_choice(node_id, indent, source, visited)?;
                    self.successor(node_id, "completed")?
                }
                NodeKind::Jump | NodeKind::Call => {
                    let keyword = if node.kind == NodeKind::Jump {
                        "jump"
                    } else {
                        "call"
                    };
                    let target = self.pin_string(node_id, "target")?;
                    push_indent(source, indent);
                    source.push_str(keyword);
                    source.push(' ');
                    source.push_str(validate_identifier(&target)?);
                    source.push('\n');
                    if node.kind == NodeKind::Call { self.successor(node_id, "exec_out")? } else { None }
                }
                NodeKind::Return => {
                    push_indent(source, indent);
                    source.push_str("return\n");
                    None
                }
                kind => {
                    return Err(TranspileError::UnsupportedStatement {
                        node: node_id,
                        kind,
                    })
                }
            };
        }

        Ok(())
    }

    fn emit_if(
        &self,
        node: NodeId,
        indent: usize,
        source: &mut String,
        visited: &mut BTreeSet<NodeId>,
    ) -> Result<(), TranspileError> {
        let condition = self.expression_from_input(node, "condition")?;
        push_indent(source, indent);
        source.push_str("if ");
        source.push_str(&condition);
        source.push_str(" {\n");
        self.emit_sequence(
            self.successor(node, "then")?,
            indent + 1,
            Some(node),
            source,
            visited,
        )?;
        push_indent(source, indent);
        source.push('}');
        if let Some(else_start) = self.successor(node, "else")? {
            source.push_str(" else {\n");
            self.emit_sequence(Some(else_start), indent + 1, Some(node), source, visited)?;
            push_indent(source, indent);
            source.push('}');
        }
        source.push('\n');
        Ok(())
    }

    fn emit_choice(
        &self,
        node: NodeId,
        indent: usize,
        source: &mut String,
        visited: &mut BTreeSet<NodeId>,
    ) -> Result<(), TranspileError> {
        let graph_node = self.graph.nodes.get(&node).unwrap();
        let labels = match graph_node.properties.get("options") {
            Some(PropertyValue::StringList(labels)) if !labels.is_empty() => labels,
            _ => return Err(TranspileError::ChoiceWithoutOptions(node)),
        };
        push_indent(source, indent);
        source.push_str("choice {\n");
        for (index, label) in labels.iter().enumerate() {
            push_indent(source, indent + 1);
            source.push_str(&quote(label));
            let condition_key = format!("option_{index}_condition");
            if self.input_is_connected(node, &condition_key)? {
                source.push_str(" if ");
                source.push_str(&self.expression_from_input(node, &condition_key)?);
            } else if let Some(PropertyValue::String(condition)) = graph_node.properties.get(&condition_key) {
                if !condition.trim().is_empty() {
                    validate_expression(condition)?;
                    source.push_str(" if ");
                    source.push_str(condition);
                }
            }
            source.push_str(" => {\n");
            self.emit_sequence(
                self.successor(node, &format!("option_{index}"))?,
                indent + 2,
                Some(node),
                source,
                visited,
            )?;
            push_indent(source, indent + 1);
            source.push_str("}\n");
        }
        push_indent(source, indent);
        source.push_str("}\n");
        Ok(())
    }

    fn expression_from_input(&self, node: NodeId, key: &str) -> Result<String, TranspileError> {
        self.expression_from_input_with_stack(node, key, &mut BTreeSet::new())
    }

    fn expression_from_input_with_stack(
        &self,
        node: NodeId,
        key: &str,
        active: &mut BTreeSet<NodeId>,
    ) -> Result<String, TranspileError> {
        let input = self.pin(node, key)?;
        if input.direction != PinDirection::Input {
            return Err(TranspileError::ExpectedInput(input.id));
        }
        let connections: Vec<_> = self
            .graph
            .edges
            .values()
            .filter(|edge| edge.input == input.id)
            .collect();
        match connections.as_slice() {
            [] => match input.default_value.as_ref() {
                // Une chaîne vide provenant du catalogue est un placeholder
                // d'édition, pas une expression câblée par l'utilisateur.
                // Elle ne doit donc jamais produire `to_int("")` ou un SET
                // silencieux dans le script généré.
                Some(PropertyValue::String(value)) if value.is_empty() => {
                    Err(TranspileError::MissingInputValue {
                        node,
                        key: key.to_owned(),
                    })
                }
                Some(value) => emit_property_expression(value),
                None => Err(TranspileError::MissingInputValue {
                    node,
                    key: key.to_owned(),
                }),
            },
            [edge] => self.expression_from_output(edge.output, active),
            _ => Err(TranspileError::MultipleInputConnections(input.id)),
        }
    }

    fn expression_from_output(
        &self,
        output: PinId,
        active: &mut BTreeSet<NodeId>,
    ) -> Result<String, TranspileError> {
        let pin = self
            .graph
            .pins
            .get(&output)
            .ok_or(TranspileError::PinNotFound(output))?;
        if !active.insert(pin.node) {
            return Err(TranspileError::ExpressionCycle(pin.node));
        }
        let node = self.graph.nodes.get(&pin.node).unwrap();
        let expression = match node.kind {
            NodeKind::Literal => {
                let value = node
                    .properties
                    .get("value")
                    .or(pin.default_value.as_ref())
                    .ok_or_else(|| TranspileError::MissingProperty {
                        node: node.id,
                        key: "value".into(),
                    })?;
                emit_property_expression(value)?
            }
            NodeKind::TextValue => quote(&property_string(node.id, &node.properties, "value")?),
            NodeKind::MakeColor => {
                let channels = ["r", "g", "b", "a"].into_iter()
                    .map(|key| self.expression_from_input_with_stack(node.id, key, active))
                    .collect::<Result<Vec<_>, _>>()?;
                let function = if node.properties.get("rgb_max") == Some(&PropertyValue::Int(255)) { "make_color_rgb" } else { "make_color" };
                format!("{function}({})", channels.join(", "))
            }
            NodeKind::FormatText => quote(&self.format_text_output(node.id, active)?),
            NodeKind::Reroute => self.expression_from_input_with_stack(node.id, "value", active)?,
            NodeKind::CharacterValue => {
                quote(&property_string(node.id, &node.properties, "character")?)
            }
            NodeKind::LabelValue => quote(&property_string(node.id, &node.properties, "label")?),
            NodeKind::PositionValue => quote(&property_string(node.id, &node.properties, "position")?),
            NodeKind::SceneAsset
            | NodeKind::SpriteAsset
            | NodeKind::MusicAsset
            | NodeKind::SoundEffectAsset
            | NodeKind::VoiceAsset
            | NodeKind::CinematicAsset
            | NodeKind::HoverImageAsset
            | NodeKind::ScriptAsset => quote(&property_string(node.id, &node.properties, "path")?),
            NodeKind::TransitionNone
            | NodeKind::TransitionFade
            | NodeKind::TransitionDissolve
            | NodeKind::TransitionSlideLeft
            | NodeKind::TransitionSlideRight
            | NodeKind::TransitionSlideUp
            | NodeKind::TransitionSlideDown
            | NodeKind::TransitionZoomIn
            | NodeKind::TransitionZoomOut
            | NodeKind::TransitionWipe
            | NodeKind::TransitionBlur => quote(&transition_value(node)?),
            NodeKind::VariableGet => {
                validate_variable_name(&property_string(node.id, &node.properties, "name")?)?
                    .to_owned()
            }
            // Comme le nœud SET de Blueprint, `value_out` réexpose exactement la
            // valeur affectée. Cela permet d'enchaîner une affectation et la
            // réutilisation de sa valeur sans transformer SET en expression pure.
            NodeKind::SetVariable => {
                self.expression_from_input_with_stack(node.id, "value", active)?
            }
            NodeKind::ConvertIntToFloat => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                format!("({value} + 0.0)")
            }
            NodeKind::ConvertNumberToText => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                format!("(\"\" + {value})")
            }
            NodeKind::ConvertTextToInt => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                format!("to_int({value})")
            }
            NodeKind::VariableReference => {
                quote(&property_string(node.id, &node.properties, "name")?)
            }
            NodeKind::BinaryOperator => {
                let operator = property_string(node.id, &node.properties, "operator")?;
                validate_operator(&operator, false)?;
                let left = self.expression_from_input_with_stack(node.id, "left", active)?;
                let right = self.expression_from_input_with_stack(node.id, "right", active)?;
                format!("({left} {operator} {right})")
            }
            NodeKind::UnaryOperator => {
                let operator = property_string(node.id, &node.properties, "operator")?;
                validate_operator(&operator, true)?;
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                if operator == "not" {
                    format!("(not {value})")
                } else {
                    format!("(-{value})")
                }
            }
            NodeKind::MathAdd
            | NodeKind::MathSubtract
            | NodeKind::MathMultiply
            | NodeKind::MathDivide
            | NodeKind::MathEqual
            | NodeKind::MathNotEqual
            | NodeKind::MathLess
            | NodeKind::MathLessEqual
            | NodeKind::MathGreater
            | NodeKind::MathGreaterEqual
            | NodeKind::LogicAnd
            | NodeKind::LogicOr => {
                let operator = explicit_binary_operator(node.kind);
                let input_keys: Vec<_> = node
                    .pins
                    .iter()
                    .map(|pin| &self.graph.pins[pin])
                    .filter(|pin| pin.direction == PinDirection::Input)
                    .map(|pin| pin.key.clone())
                    .collect();
                let mut values = input_keys
                    .iter()
                    .map(|key| self.expression_from_input_with_stack(node.id, key, active));
                let first = values
                    .next()
                    .ok_or_else(|| TranspileError::MissingInputValue {
                        node: node.id,
                        key: "left".into(),
                    })??;
                values.try_fold(first, |left, right| {
                    let right = right?;
                    Ok(format!("({left} {operator} {right})"))
                })?
            }
            NodeKind::LogicNot | NodeKind::MathNegate => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                if node.kind == NodeKind::LogicNot {
                    format!("(not {value})")
                } else {
                    format!("(-{value})")
                }
            }
            NodeKind::Index => {
                let target = self.expression_from_input_with_stack(node.id, "target", active)?;
                let index = self.expression_from_input_with_stack(node.id, "index", active)?;
                format!("{target}[{index}]")
            }
            NodeKind::FunctionCall => {
                let function = property_string(node.id, &node.properties, "function")?;
                if let Some(PropertyValue::Int(count)) = node.properties.get("input_count") {
                    let args = (0..*count).map(|i| self.expression_from_input_with_stack(node.id, &format!("item_{i}"), active))
                        .collect::<Result<Vec<_>, _>>()?;
                    active.remove(&node.id);
                    return Ok(format!("{}({})", validate_identifier(&function)?, args.join(", ")));
                }
                let args = property_string_list(node.id, &node.properties, "args")?;
                format!(
                    "{}({})",
                    validate_identifier(&function)?,
                    args.iter()
                        .map(|value| quote(value))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            NodeKind::ListLiteral => {
                if let Some(PropertyValue::Int(count)) = node.properties.get("input_count") {
                    let items = (0..*count).map(|i| self.expression_from_input_with_stack(node.id, &format!("item_{i}"), active))
                        .collect::<Result<Vec<_>, _>>()?;
                    active.remove(&node.id);
                    return Ok(format!("[{}]", items.join(", ")));
                }
                let items = property_string_list(node.id, &node.properties, "items")?;
                format!(
                    "[{}]",
                    items
                        .iter()
                        .map(|value| quote(value))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            kind => {
                return Err(TranspileError::UnsupportedExpression {
                    node: node.id,
                    kind,
                })
            }
        };
        active.remove(&pin.node);
        Ok(expression)
    }

    fn connected_character(&self, node: NodeId, key: &str, visited: &mut BTreeSet<NodeId>) -> Option<NodeId> {
        if !visited.insert(node) { return None; }
        let pin = self.pin(node, key).ok()?;
        let edge = self.graph.edges.values().find(|edge| edge.input == pin.id)?;
        let source = &self.graph.nodes[&self.graph.pins[&edge.output].node];
        match source.kind {
            NodeKind::CharacterValue => Some(source.id),
            NodeKind::Reroute => self.connected_character(source.id, "value", visited),
            _ => None,
        }
    }

    fn connected_sprite(&self, consumer: NodeId) -> Result<Option<String>, TranspileError> {
        let Some(character) = self.connected_character(consumer, "character", &mut BTreeSet::new()) else { return Ok(None) };
        let Some(pin) = self.graph.pin_by_key(character,"sprite") else { return Ok(None) };
        if !self.graph.edges.values().any(|e|e.input == pin.id) { return Ok(None); }
        let path = self.pin_string(character,"sprite")?;
        if path.trim().is_empty() { return Err(TranspileError::MissingInputValue { node: character, key: "Sprite : chemin d’image vide".into() }); }
        // A file at the asset root is still a file, never an emotion name.
        Ok(Some(if path.contains('/') { path } else { format!("./{path}") }))
    }

    fn emit_character_appearance(&self, consumer: NodeId, character: &str, indent: usize, source: &mut String) -> Result<(),TranspileError> {
        let character = validate_identifier(character)?;
        if let Some(path) = self.connected_sprite(consumer)? {
            push_indent(source,indent);
            source.push_str(&format!("{character}.show({})\n",quote(&path)));
        }
        Ok(())
    }

    fn pin_string(&self, node: NodeId, key: &str) -> Result<String, TranspileError> {
        let pin = self.pin(node, key)?;
        if let Some(edge) = self.graph.edges.values().find(|edge| edge.input == pin.id) {
            let source_pin = self
                .graph
                .pins
                .get(&edge.output)
                .ok_or(TranspileError::PinNotFound(edge.output))?;
            let source = self.graph.nodes.get(&source_pin.node).unwrap();
            return match source.kind {
                NodeKind::TextValue => property_string(source.id, &source.properties, "value"),
                NodeKind::LabelValue => property_string(source.id, &source.properties, "label"),
                NodeKind::PositionValue => property_string(source.id, &source.properties, "position"),
                NodeKind::Reroute => self.pin_string(source.id, "value"),
                NodeKind::CharacterValue => {
                    property_string(source.id, &source.properties, "character")
                }
                NodeKind::SceneAsset
                | NodeKind::SpriteAsset
                | NodeKind::MusicAsset
                | NodeKind::SoundEffectAsset
                | NodeKind::VoiceAsset
                | NodeKind::CinematicAsset
                | NodeKind::HoverImageAsset
                | NodeKind::ScriptAsset => property_string(source.id, &source.properties, "path"),
                NodeKind::VariableReference => {
                    property_string(source.id, &source.properties, "name")
                }
                NodeKind::Literal => property_string(source.id, &source.properties, "value"),
                NodeKind::TransitionNone
                | NodeKind::TransitionFade
                | NodeKind::TransitionDissolve
                | NodeKind::TransitionSlideLeft
                | NodeKind::TransitionSlideRight
                | NodeKind::TransitionSlideUp
                | NodeKind::TransitionSlideDown
                | NodeKind::TransitionZoomIn
                | NodeKind::TransitionZoomOut
                | NodeKind::TransitionWipe
                | NodeKind::TransitionBlur => Ok(transition_value(source)?),
                _ => Err(TranspileError::ExpectedString {
                    node,
                    key: key.into(),
                }),
            };
        }
        match &pin.default_value {
            Some(PropertyValue::String(value)) => Ok(value.clone()),
            Some(_) => Err(TranspileError::ExpectedString {
                node,
                key: key.into(),
            }),
            None => Err(TranspileError::MissingInputValue {
                node,
                key: key.into(),
            }),
        }
    }

    fn pin_interpolated_text(&self, node: NodeId, key: &str) -> Result<String, TranspileError> {
        let pin = self.pin(node, key)?;
        let Some(edge) = self.graph.edges.values().find(|edge| edge.input == pin.id) else {
            // Le texte d'un dialogue est une donnée métier obligatoire. Une
            // ancienne valeur par défaut cachée ne doit jamais être émise si
            // aucune source visible n'est branchée dans le graphe.
            return Err(TranspileError::MissingInputValue {
                node,
                key: key.into(),
            });
        };
        let source_pin = self
            .graph
            .pins
            .get(&edge.output)
            .ok_or(TranspileError::PinNotFound(edge.output))?;
        let source = self.graph.nodes.get(&source_pin.node).unwrap();
        if source.kind == NodeKind::VariableGet {
            let name = property_string(source.id, &source.properties, "name")?;
            return Ok(format!("[{}]", validate_variable_name(&name)?));
        }
        if source.kind == NodeKind::FormatText {
            return self.format_text_output(source.id, &mut BTreeSet::new());
        }
        if source.kind == NodeKind::Reroute {
            return self.pin_interpolated_text(source.id, "value");
        }
        self.pin_string(node, key)
    }

    fn format_text_output(
        &self,
        node: NodeId,
        active: &mut BTreeSet<NodeId>,
    ) -> Result<String, TranspileError> {
        let pattern = self.pin_string(node, "format")?;
        let mut result = String::new();
        let mut chars = pattern.chars().peekable();
        while let Some(character) = chars.next() {
            if character == '`' {
                if let Some(escaped) = chars.next() {
                    result.push(escaped);
                }
                continue;
            }
            if character != '{' {
                result.push(character);
                continue;
            }

            let mut marker = String::new();
            let mut closed = false;
            while let Some(next) = chars.next() {
                if next == '}' {
                    closed = true;
                    break;
                }
                marker.push(next);
            }
            if !closed {
                result.push('{');
                result.push_str(&marker);
                break;
            }
            let argument = marker.trim();
            let key = format!("argument_{argument}");
            if self.graph.pin_by_key(node, &key).is_none() {
                result.push('{');
                result.push_str(&marker);
                result.push('}');
                continue;
            }
            let expression = self.expression_from_input_with_stack(node, &key, active)?;
            result.push('[');
            result.push_str(&expression);
            result.push(']');
        }
        Ok(result)
    }

    fn pin_string_list(&self, node: NodeId, key: &str) -> Result<Vec<String>, TranspileError> {
        let pin = self.pin(node, key)?;
        match &pin.default_value {
            Some(PropertyValue::StringList(values)) => Ok(values.clone()),
            Some(_) => Err(TranspileError::ExpectedStringList {
                node,
                key: key.into(),
            }),
            None => Ok(Vec::new()),
        }
    }

    fn constant_input(&self, node: NodeId, key: &str, active: &mut BTreeSet<NodeId>) -> Result<Option<PropertyValue>, TranspileError> {
        if !active.insert(node) { return Err(TranspileError::ExpressionCycle(node)); }
        let pin = self.pin(node, key)?;
        let result = if let Some(edge) = self.graph.edges.values().find(|e| e.input == pin.id) {
            let source = &self.graph.nodes[&self.graph.pins[&edge.output].node];
            match source.kind {
                NodeKind::Literal => source.properties.get("value").cloned(),
                NodeKind::Reroute => self.constant_input(source.id, "value", active)?,
                _ => return Err(TranspileError::GeneratedSourceInvalid(format!("La broche {node}.{key} requiert une constante RVN, pas une expression dynamique"))),
            }
        } else { pin.default_value.clone() };
        active.remove(&node);
        Ok(result)
    }

    fn pin_bool(&self, node: NodeId, key: &str) -> Result<bool, TranspileError> {
        match self.constant_input(node, key, &mut BTreeSet::new())? {
            Some(PropertyValue::Bool(value)) => Ok(value),
            _ => Err(TranspileError::ExpectedBool {
                node,
                key: key.into(),
            }),
        }
    }

    fn pin_int(&self, node: NodeId, key: &str) -> Result<i64, TranspileError> {
        match self.constant_input(node, key, &mut BTreeSet::new())? {
            Some(PropertyValue::Int(value)) => Ok(value),
            _ => Err(TranspileError::ExpectedInt {
                node,
                key: key.into(),
            }),
        }
    }

    fn pin_float(&self, node: NodeId, key: &str) -> Result<f64, TranspileError> {
        match self.constant_input(node, key, &mut BTreeSet::new())? {
            Some(PropertyValue::Float(value)) => Ok(value),
            Some(PropertyValue::Int(value)) => Ok(value as f64),
            _ => Err(TranspileError::ExpectedFloat {
                node,
                key: key.into(),
            }),
        }
    }

    fn push_cinematic_transition(&self, node: NodeId, source: &mut String) -> Result<(), TranspileError> {
        let value = self.pin_string(node, "transition")?;
        let name = match value.as_str() {
            "" | "none" => return Ok(()),
            "fade" | "fade(500)" => "fade",
            "dissolve" | "dissolve(300)" => "dissolve",
            _ => return Err(TranspileError::GeneratedSourceInvalid("Les illustrations cinématiques RVN acceptent fade/dissolve sans durée personnalisée".into())),
        };
        source.push_str(&format!(" with {name}")); Ok(())
    }

    fn push_transition(&self, node: NodeId, source: &mut String) -> Result<(), TranspileError> {
        let transition = self.pin_string(node, "transition")?;
        if !transition.is_empty() && transition != "none" {
            source.push_str(" with ");
            source.push_str(&transition);
        }
        Ok(())
    }

    fn push_optional_transition(
        &self,
        node: NodeId,
        source: &mut String,
    ) -> Result<(), TranspileError> {
        if self.graph.pin_by_key(node, "transition").is_some() {
            self.push_transition(node, source)?;
        }
        Ok(())
    }

    fn input_is_connected(&self, node: NodeId, key: &str) -> Result<bool, TranspileError> {
        let pin = self.pin(node, key)?;
        Ok(self.graph.edges.values().any(|edge| edge.input == pin.id))
    }

    fn successor(&self, node: NodeId, key: &str) -> Result<Option<NodeId>, TranspileError> {
        let output = self.pin(node, key)?;
        let edges: Vec<_> = self
            .graph
            .edges
            .values()
            .filter(|edge| edge.output == output.id)
            .collect();
        match edges.as_slice() {
            [] => Ok(None),
            [edge] => Ok(Some(
                self.graph
                    .pins
                    .get(&edge.input)
                    .ok_or(TranspileError::PinNotFound(edge.input))?
                    .node,
            )),
            _ => Err(TranspileError::AmbiguousExecutionOutput(output.id)),
        }
    }

    fn pin(&self, node: NodeId, key: &str) -> Result<&GraphPin, TranspileError> {
        self.graph
            .pin_by_key(node, key)
            .ok_or_else(|| TranspileError::MissingPin {
                node,
                key: key.into(),
            })
    }
}

fn property_string(
    node: NodeId,
    properties: &BTreeMap<String, PropertyValue>,
    key: &str,
) -> Result<String, TranspileError> {
    match properties.get(key) {
        Some(PropertyValue::String(value)) => Ok(value.clone()),
        Some(_) => Err(TranspileError::ExpectedString {
            node,
            key: key.into(),
        }),
        None => Err(TranspileError::MissingProperty {
            node,
            key: key.into(),
        }),
    }
}

fn property_int(
    node: NodeId,
    properties: &BTreeMap<String, PropertyValue>,
    key: &str,
) -> Result<i64, TranspileError> {
    match properties.get(key) {
        Some(PropertyValue::Int(value)) => Ok(*value),
        Some(_) => Err(TranspileError::ExpectedInt {
            node,
            key: key.into(),
        }),
        None => Err(TranspileError::MissingProperty {
            node,
            key: key.into(),
        }),
    }
}

fn property_string_list(
    node: NodeId,
    properties: &BTreeMap<String, PropertyValue>,
    key: &str,
) -> Result<Vec<String>, TranspileError> {
    match properties.get(key) {
        Some(PropertyValue::StringList(values)) => Ok(values.clone()),
        Some(_) => Err(TranspileError::ExpectedStringList {
            node,
            key: key.into(),
        }),
        None => Err(TranspileError::MissingProperty {
            node,
            key: key.into(),
        }),
    }
}

fn emit_property_expression(value: &PropertyValue) -> Result<String, TranspileError> {
    match value {
        PropertyValue::Bool(value) => Ok(value.to_string()),
        PropertyValue::Int(value) => Ok(value.to_string()),
        PropertyValue::Float(value) => {
            if !value.is_finite() {
                return Err(TranspileError::NonFiniteFloat(*value));
            }
            let mut text = value.to_string();
            if !text.contains('.') {
                text.push_str(".0");
            }
            Ok(text)
        }
        PropertyValue::String(value) => Ok(quote(value)),
        PropertyValue::StringList(values) => Ok(format!(
            "[{}]",
            values
                .iter()
                .map(|value| quote(value))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn validate_identifier(value: &str) -> Result<&str, TranspileError> {
    let mut chars = value.chars();
    let valid = chars
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    valid
        .then_some(value)
        .ok_or_else(|| TranspileError::InvalidIdentifier(value.into()))
}

fn validate_variable_name(value: &str) -> Result<&str, TranspileError> {
    if let Some(suffix) = value.strip_prefix("persistent.") {
        validate_identifier(suffix)?;
        Ok(value)
    } else {
        validate_identifier(value)
    }
}

fn validate_operator(operator: &str, unary: bool) -> Result<(), TranspileError> {
    let valid = if unary {
        matches!(operator, "not" | "-")
    } else {
        matches!(
            operator,
            "+" | "-" | "*" | "/" | "==" | "!=" | "<" | "<=" | ">" | ">=" | "and" | "or"
        )
    };
    valid
        .then_some(())
        .ok_or_else(|| TranspileError::InvalidOperator(operator.into()))
}

fn explicit_binary_operator(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::MathAdd => "+",
        NodeKind::MathSubtract => "-",
        NodeKind::MathMultiply => "*",
        NodeKind::MathDivide => "/",
        NodeKind::MathEqual => "==",
        NodeKind::MathNotEqual => "!=",
        NodeKind::MathLess => "<",
        NodeKind::MathLessEqual => "<=",
        NodeKind::MathGreater => ">",
        NodeKind::MathGreaterEqual => ">=",
        NodeKind::LogicAnd => "and",
        NodeKind::LogicOr => "or",
        _ => unreachable!("not an explicit binary operator"),
    }
}

fn transition_value(node: &GraphNode) -> Result<String, TranspileError> {
    let name = match node.kind {
        NodeKind::TransitionNone => return Ok("none".into()),
        NodeKind::TransitionFade => "fade",
        NodeKind::TransitionDissolve => "dissolve",
        NodeKind::TransitionSlideLeft => "slideleft",
        NodeKind::TransitionSlideRight => "slideright",
        NodeKind::TransitionSlideUp => "slideup",
        NodeKind::TransitionSlideDown => "slidedown",
        NodeKind::TransitionZoomIn => "zoomin",
        NodeKind::TransitionZoomOut => "zoomout",
        NodeKind::TransitionWipe => "wipe",
        NodeKind::TransitionBlur => "blur",
        _ => unreachable!("not a transition value node"),
    };
    let duration = match node.properties.get("duration_ms") {
        Some(PropertyValue::Int(value)) if *value >= 0 => *value,
        _ => {
            return Err(TranspileError::MissingProperty {
                node: node.id,
                key: "duration_ms".into(),
            })
        }
    };
    Ok(format!("{name}({duration})"))
}

fn quote(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

fn push_indent(source: &mut String, level: usize) {
    for _ in 0..level {
        source.push_str("    ");
    }
}

#[derive(Debug)]
pub enum TranspileError {
    InvalidGraph(Vec<GraphDiagnostic>),
    UnsupportedGraphKind(GraphKind),
    ExpectedSingleRoot {
        kind: NodeKind,
        found: usize,
    },
    NodeNotFound(NodeId),
    PinNotFound(PinId),
    MissingPin {
        node: NodeId,
        key: String,
    },
    MissingInputValue {
        node: NodeId,
        key: String,
    },
    MissingProperty {
        node: NodeId,
        key: String,
    },
    ExpectedString {
        node: NodeId,
        key: String,
    },
    ExpectedStringList {
        node: NodeId,
        key: String,
    },
    ExpectedBool {
        node: NodeId,
        key: String,
    },
    ExpectedFloat {
        node: NodeId,
        key: String,
    },
    ExpectedInt {
        node: NodeId,
        key: String,
    },
    ExpectedInput(PinId),
    MultipleInputConnections(PinId),
    AmbiguousExecutionOutput(PinId),
    ExecutionCycle(NodeId),
    ExpressionCycle(NodeId),
    MissingBranchEnd(NodeId),
    WrongBranchEnd {
        node: NodeId,
        expected_owner: Option<NodeId>,
        found_owner: NodeId,
    },
    ChoiceWithoutOptions(NodeId),
    UnsupportedStatement {
        node: NodeId,
        kind: NodeKind,
    },
    UnsupportedExpression {
        node: NodeId,
        kind: NodeKind,
    },
    InvalidIdentifier(String),
    InvalidOperator(String),
    InvalidDynamicParameter(String),
    InvalidHotspot(String),
    NonFiniteFloat(f64),
    GeneratedSourceInvalid(String),
}

impl TranspileError {
    /// Élément de graphe à sélectionner lorsqu'une erreur est affichée dans
    /// l'éditeur. Les erreurs globales n'ont volontairement pas de cible.
    pub fn node(&self) -> Option<NodeId> {
        match self {
            Self::NodeNotFound(node)
            | Self::ExecutionCycle(node)
            | Self::ExpressionCycle(node)
            | Self::MissingBranchEnd(node)
            | Self::ChoiceWithoutOptions(node) => Some(*node),
            Self::MissingPin { node, .. }
            | Self::MissingInputValue { node, .. }
            | Self::MissingProperty { node, .. }
            | Self::ExpectedString { node, .. }
            | Self::ExpectedStringList { node, .. }
            | Self::ExpectedBool { node, .. }
            | Self::ExpectedFloat { node, .. }
            | Self::ExpectedInt { node, .. }
            | Self::WrongBranchEnd { node, .. }
            | Self::UnsupportedStatement { node, .. }
            | Self::UnsupportedExpression { node, .. } => Some(*node),
            Self::PinNotFound(_)
            | Self::ExpectedInput(_)
            | Self::MultipleInputConnections(_)
            | Self::AmbiguousExecutionOutput(_)
            | Self::InvalidGraph(_)
            | Self::UnsupportedGraphKind(_)
            | Self::ExpectedSingleRoot { .. }
            | Self::InvalidIdentifier(_)
            | Self::InvalidOperator(_)
            | Self::InvalidDynamicParameter(_)
            | Self::InvalidHotspot(_)
            | Self::NonFiniteFloat(_)
            | Self::GeneratedSourceInvalid(_) => None,
        }
    }

    pub fn pin(&self) -> Option<PinId> {
        match self {
            Self::PinNotFound(pin)
            | Self::ExpectedInput(pin)
            | Self::MultipleInputConnections(pin)
            | Self::AmbiguousExecutionOutput(pin) => Some(*pin),
            _ => None,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidGraph(_) => "invalid_graph",
            Self::UnsupportedGraphKind(_) => "unsupported_graph_kind",
            Self::ExpectedSingleRoot { .. } => "expected_single_root",
            Self::NodeNotFound(_) => "node_not_found",
            Self::PinNotFound(_) => "pin_not_found",
            Self::MissingPin { .. } => "missing_pin",
            Self::MissingInputValue { .. } => "missing_input_value",
            Self::MissingProperty { .. } => "missing_property",
            Self::ExpectedString { .. } => "expected_string",
            Self::ExpectedStringList { .. } => "expected_string_list",
            Self::ExpectedBool { .. } => "expected_bool",
            Self::ExpectedFloat { .. } => "expected_float",
            Self::ExpectedInt { .. } => "expected_int",
            Self::ExpectedInput(_) => "expected_input",
            Self::MultipleInputConnections(_) => "multiple_input_connections",
            Self::AmbiguousExecutionOutput(_) => "ambiguous_execution_output",
            Self::ExecutionCycle(_) => "execution_cycle",
            Self::ExpressionCycle(_) => "expression_cycle",
            Self::MissingBranchEnd(_) => "missing_branch_end",
            Self::WrongBranchEnd { .. } => "wrong_branch_end",
            Self::ChoiceWithoutOptions(_) => "choice_without_options",
            Self::UnsupportedStatement { .. } => "unsupported_statement",
            Self::UnsupportedExpression { .. } => "unsupported_expression",
            Self::InvalidIdentifier(_) => "invalid_identifier",
            Self::InvalidOperator(_) => "invalid_operator",
            Self::InvalidDynamicParameter(_) => "invalid_dynamic_parameter",
            Self::InvalidHotspot(_) => "invalid_hotspot",
            Self::NonFiniteFloat(_) => "non_finite_float",
            Self::GeneratedSourceInvalid(_) => "generated_source_invalid",
        }
    }
}

impl std::fmt::Display for TranspileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TranspileError {}
