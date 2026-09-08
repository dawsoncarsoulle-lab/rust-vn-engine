//! Deterministic, loss-checked import of the narrative RVN subset.
//! Imports are resolved by the caller; unsupported statements fail explicitly.
use crate::*;
use rvn_parser::{BinOpKind, Expr, Script, Statement, Transition};
use std::collections::BTreeMap;

/// One init document followed by one document per source label, in source order.
pub fn import_script(script: &Script) -> Result<Vec<GraphDocument>, String> {
    let mut init = Vec::new();
    let mut labels: Vec<(String, Script)> = Vec::new();
    for statement in script {
        match statement {
            Statement::Init { body } => init.extend(body.clone()),
            Statement::Label { name } => {
                if labels.iter().any(|(n, _)| n == name) { return Err(format!("Label dupliqué : {name}")); }
                labels.push((name.clone(), Vec::new()));
            }
            Statement::Use { .. } => return Err("Résoudre les imports RVN avant la conversion".into()),
            other => labels.last_mut().ok_or("Instruction hors init et label")?.1.push(other.clone()),
        }
    }
    let mut characters = BTreeMap::new();
    let mut variables = BTreeMap::new();
    for statement in &init {
        match statement {
            Statement::CharacterCreate { id, display_name } => {
                if characters.insert(id.clone(), display_name.clone()).is_some() { return Err(format!("Personnage déclaré deux fois : {id}")); }
            }
            Statement::SetVar { name, value } => {
                let default_value = literal(value).ok_or_else(|| format!("Initialisation non littérale non prise en charge : {name}"))?;
                let value_type = match &default_value {
                    PropertyValue::Bool(_) => ValueType::Bool, PropertyValue::Int(_) => ValueType::Int,
                    PropertyValue::Float(_) => ValueType::Float, _ => ValueType::String,
                };
                variables.insert(name.clone(), VariableDefinition { name: name.clone(), value_type, default_value });
            }
            _ => {},
        }
    }
    // Characters are serialized once through the shared character registry.
    init.retain(|s| !matches!(s, Statement::CharacterCreate { .. }));
    let mut blocks = vec![(GraphKind::Init, init)];
    for (index, (name, body)) in labels.iter().enumerate() {
        let mut body = body.clone();
        if !body.last().is_some_and(|s| matches!(s, Statement::Jump { .. } | Statement::Return)) {
            if let Some((next, _)) = labels.get(index + 1) {
                body.push(Statement::Jump { target: next.clone() });
            }
        }
        blocks.push((GraphKind::Label { name: name.clone() }, body));
    }
    let mut result = Vec::new();
    for (index, (kind, body)) in blocks.into_iter().enumerate() {
        let mut graph = GraphDocument::new(GraphId::new(index as u64 + 1), kind.clone());
        graph.characters = characters.clone();
        graph.variables = variables.clone();
        let root = graph.add_catalog_node(if kind == GraphKind::Init { NodeKind::Init } else { NodeKind::Label }, [0.0, 0.0]).map_err(err)?;
        let mut builder = Builder { graph, lane: 0.0 };
        builder.sequence(&body, (root, "exec_out".into()), 960.0, 0.0, None)
            .map_err(|e| format!("{kind:?} : {e}"))?;
        builder.graph.materialize_visible_defaults().map_err(err)?;
        layout_imported_values(&mut builder.graph);
        // Catch omitted/incorrect pins immediately, before any files are written.
        transpile(&builder.graph).map_err(err)?;
        result.push(builder.graph);
    }
    transpile_project(&result)?;
    Ok(result)
}

/// Arrange pure inputs below their action using actual asset-card clearances.
/// Execution positions and all graph semantics remain untouched.
pub fn layout_imported_values(graph: &mut GraphDocument) {
    fn inputs(graph: &GraphDocument, node: NodeId) -> Vec<NodeId> {
        graph.nodes[&node].pins.iter().filter_map(|id| {
            let pin = &graph.pins[id];
            if pin.direction != PinDirection::Input || pin.value_type.is_execution() { return None; }
            graph.edges.values().find(|e| e.input == *id).map(|e| graph.pins[&e.output].node)
        }).collect()
    }
    fn place(graph: &mut GraphDocument, node: NodeId, pos: [f64;2], visited: &mut std::collections::BTreeSet<NodeId>) {
        if !visited.insert(node) { return; }
        graph.nodes.get_mut(&node).unwrap().position = pos;
        for (i, child) in inputs(graph,node).into_iter().enumerate() {
            place(graph,child,[pos[0]-300.0,pos[1]+180.0+i as f64*240.0],visited);
        }
    }
    let actions: Vec<_> = graph.nodes.values().filter(|n| n.pins.iter().any(|id|graph.pins[id].value_type.is_execution())).map(|n|n.id).collect();
    let mut visited: std::collections::BTreeSet<_> = actions.iter().copied().collect();
    for action in actions {
        let pos=graph.nodes[&action].position;
        for (i, child) in inputs(graph,action).into_iter().enumerate() {
            place(graph,child,[pos[0]-320.0,pos[1]+180.0+i as f64*240.0],&mut visited);
        }
    }
}

fn err(error: impl std::fmt::Display) -> String { error.to_string() }
fn expression_source(expr: &Expr) -> String {
    match expr {
        Expr::Int(v) => v.to_string(), Expr::Float(v) => format!("{v:?}"), Expr::Bool(v) => v.to_string(),
        Expr::Str(v) => serde_json::to_string(v).unwrap(), Expr::Var(v) => v.clone(),
        Expr::BinOp { op, left, right } => format!("({} {op} {})",expression_source(left),expression_source(right)),
        Expr::And(a,b) => format!("({} and {})",expression_source(a),expression_source(b)),
        Expr::Or(a,b) => format!("({} or {})",expression_source(a),expression_source(b)),
        Expr::Not(v) => format!("(not {})",expression_source(v)), Expr::Neg(v) => format!("(-{})",expression_source(v)),
        Expr::Call { name,args } => format!("{name}({})",args.iter().map(expression_source).collect::<Vec<_>>().join(", ")),
        Expr::ListLit(items) => format!("[{}]",items.iter().map(expression_source).collect::<Vec<_>>().join(", ")),
        Expr::Index { target,index } => format!("{}[{}]",expression_source(target),expression_source(index)),
    }
}
fn text_source(text: &rvn_parser::InterpolatedText) -> String {
    text.0.iter().map(|s| match s {
        rvn_parser::TextSegment::Lit(v) => v.clone(),
        rvn_parser::TextSegment::Interp(v) => format!("[{}]",expression_source(v)),
    }).collect()
}
fn literal(value: &Expr) -> Option<PropertyValue> {
    Some(match value {
        Expr::Int(v) => PropertyValue::Int(*v), Expr::Float(v) => PropertyValue::Float(*v as f64),
        Expr::Bool(v) => PropertyValue::Bool(*v), Expr::Str(v) => PropertyValue::String(v.clone()),
        _ => return None,
    })
}
struct Builder { graph: GraphDocument, lane: f64 }
impl Builder {
    fn node(&mut self, kind: NodeKind, pos: [f64; 2]) -> Result<NodeId, String> { self.graph.add_catalog_node(kind, pos).map_err(err) }
    fn property(&mut self, n: NodeId, key: &str, value: PropertyValue) -> Result<(), String> { self.graph.set_property(n, key, value).map_err(err) }
    fn default(&mut self, n: NodeId, key: &str, value: PropertyValue) -> Result<(), String> {
        let id = self.graph.pin_by_key(n, key).ok_or_else(|| format!("Broche absente : {n}.{key}"))?.id;
        self.graph.pins.get_mut(&id).unwrap().default_value = Some(value); Ok(())
    }
    fn text(&mut self, n: NodeId, key: &str, value: &str) -> Result<(), String> { self.default(n, key, PropertyValue::String(value.into())) }
    fn connect(&mut self, a: NodeId, ak: &str, b: NodeId, bk: &str) -> Result<(), String> {
        let output = self.graph.pin_by_key(a, ak).ok_or_else(|| format!("Sortie absente : {a}.{ak}"))?.id;
        let input = self.graph.pin_by_key(b, bk).ok_or_else(|| format!("Entrée absente : {b}.{bk}"))?.id;
        self.graph.connect(output, input).map_err(err)?; Ok(())
    }
    fn variable(&mut self, node: NodeId, name: &str) -> Result<(), String> {
        let ty = self.graph.variables.get(name).ok_or_else(|| format!("Variable sans déclaration typée : {name}"))?.value_type.clone();
        self.property(node, "name", PropertyValue::String(name.into()))?;
        for key in ["value", "value_out"] {
            if let Some(id) = self.graph.pin_by_key(node, key).map(|p| p.id) { self.graph.pins.get_mut(&id).unwrap().value_type = ty.clone(); }
        }
        Ok(())
    }
    fn expression(&mut self, expr: &Expr, pos: [f64; 2]) -> Result<NodeId, String> {
        if let Some(value) = literal(expr) {
            let n = self.node(NodeKind::Literal, pos)?; self.property(n, "value", value)?; return Ok(n);
        }
        if let Expr::Var(name) = expr {
            let n = self.node(NodeKind::VariableGet, pos)?; self.variable(n, name)?; return Ok(n);
        }
        let (kind, inputs): (NodeKind, Vec<(&str, &Expr)>) = match expr {
            Expr::BinOp { op, left, right } => (match op {
                BinOpKind::Add => NodeKind::MathAdd, BinOpKind::Sub => NodeKind::MathSubtract,
                BinOpKind::Mul => NodeKind::MathMultiply, BinOpKind::Div => NodeKind::MathDivide,
                BinOpKind::Eq => NodeKind::MathEqual, BinOpKind::Ne => NodeKind::MathNotEqual,
                BinOpKind::Lt => NodeKind::MathLess, BinOpKind::Le => NodeKind::MathLessEqual,
                BinOpKind::Gt => NodeKind::MathGreater, BinOpKind::Ge => NodeKind::MathGreaterEqual,
            }, vec![("left", left), ("right", right)]),
            Expr::And(a,b) => (NodeKind::LogicAnd, vec![("left", a), ("right", b)]),
            Expr::Or(a,b) => (NodeKind::LogicOr, vec![("left", a), ("right", b)]),
            Expr::Not(v) => (NodeKind::LogicNot, vec![("value", v)]),
            Expr::Neg(v) => (NodeKind::MathNegate, vec![("value", v)]),
            _ => return Err(format!("Expression non prise en charge par l'importeur : {expr:?}")),
        };
        let n = self.node(kind, pos)?;
        for (i, (key, expr)) in inputs.iter().enumerate() {
            let source = self.expression(expr, [pos[0] - 260.0, pos[1] + 120.0 + i as f64 * 180.0])?;
            self.connect(source, "value", n, key)?;
        }
        Ok(n)
    }
    fn input_expr(&mut self, n: NodeId, key: &str, value: &Expr) -> Result<(), String> {
        if let Some(value) = literal(value) { return self.default(n, key, value); }
        let p = self.graph.nodes[&n].position;
        let expr = self.expression(value, [p[0] - 260.0, p[1] + 160.0])?;
        self.connect(expr, "value", n, key)
    }
    fn transition(&mut self, n: NodeId, value: &Transition) -> Result<(), String> { self.text(n, "transition", &value.to_string()) }
    fn sequence(&mut self, body: &[Statement], mut prev: (NodeId, String), mut x: f64, y: f64, owner: Option<NodeId>) -> Result<f64, String> {
        self.lane = self.lane.max(y);
        for (index, stmt) in body.iter().enumerate() {
            use Statement as S;
            let kind = match stmt {
                S::SetVar { .. } => NodeKind::SetVariable, S::Dialogue { .. } => NodeKind::Dialogue,
                S::Choice { .. } => NodeKind::Choice, S::If { .. } => NodeKind::If,
                S::Jump { .. } => NodeKind::Jump, S::Call { .. } => NodeKind::Call, S::Return => NodeKind::Return,
                S::Scene { .. } => NodeKind::Scene, S::ShowSprite { .. } => NodeKind::SpriteShow,
                S::HideSprite { .. } => NodeKind::SpriteHide, S::MoveSprite { .. } => NodeKind::SpriteMove,
                S::MusicPlay { .. } => NodeKind::MusicPlay, S::MusicStop { .. } => NodeKind::MusicStop,
                S::SfxPlay { .. } => NodeKind::SfxPlay, S::SfxStop { .. } => NodeKind::SfxStop,
                S::VoicePlay { .. } => NodeKind::VoicePlay, S::VoiceStop => NodeKind::VoiceStop,
                S::CinematicShow { .. } => NodeKind::CinematicShow, S::CinematicHide { .. } => NodeKind::CinematicHide,
                S::UnlockEnding { .. } => NodeKind::UnlockEnding, S::Imagemap { .. } => NodeKind::Imagemap,
                S::TypewriterSet { .. } => NodeKind::TypewriterSet, S::TypewriterSpeed { .. } => NodeKind::TypewriterSpeed,
                S::Config { .. } => NodeKind::Config,
                other => return Err(format!("Instruction non prise en charge : {other:?}")),
            };
            let n = self.node(kind, [x, y])?;
            self.connect(prev.0, &prev.1, n, "exec_in")?;
            let mut continuation = "exec_out";
            match stmt {
                S::Config { key, value } => { self.text(n,"key",key)?; self.text(n,"value",value)?; }
                S::SetVar { name, value } => { self.variable(n,name)?; self.text(n,"name",name)?; self.input_expr(n,"value",value)?; }
                S::Dialogue { character_id, text } => {
                    self.text(n,"character",character_id.as_deref().unwrap_or(""))?;
                    // Blank dialogue is intentional in credits; never trim it.
                    let value = self.node(NodeKind::TextValue,[x-260.0,y+340.0])?;
                    self.property(value,"value",PropertyValue::String(text_source(text)))?;
                    self.connect(value,"value",n,"text")?;
                }
                S::Jump { target } | S::Call { target } => self.text(n,"target",target)?,
                S::Scene { background, transition } => { self.text(n,"background",background)?; self.transition(n,transition)?; }
                S::ShowSprite { character_id, emotion, position, transition } => {
                    self.text(n,"character",character_id)?; self.text(n,"emotion",emotion.as_deref().unwrap_or(""))?;
                    self.text(n,"position",&position.as_ref().map(ToString::to_string).unwrap_or_default())?; self.transition(n,transition)?;
                }
                S::HideSprite { character_id, transition } => { self.text(n,"character",character_id)?; self.transition(n,transition)?; }
                S::MoveSprite { character_id, position, transition } => { self.text(n,"character",character_id)?; self.text(n,"position",&position.to_string())?; self.transition(n,transition)?; }
                S::MusicPlay { file, transition } => { self.text(n,"file",file)?; self.transition(n,transition)?; }
                S::MusicStop { transition } => self.transition(n,transition)?,
                S::SfxPlay { file, transition } | S::SfxStop { file, transition } => {
                    if *transition != Transition::None { return Err("Transition SFX non représentable actuellement".into()); }
                    self.text(n,"file",file)?;
                }
                S::VoicePlay { file } => self.text(n,"file",file)?,
                S::CinematicShow { id, transition } => { self.text(n,"cinematic",id)?; self.text(n,"transition",transition.as_deref().unwrap_or("none"))?; }
                S::CinematicHide { transition } => self.text(n,"transition",transition.as_deref().unwrap_or("none"))?,
                S::UnlockEnding { id } => self.text(n,"id",id)?,
                S::TypewriterSet { enabled } => self.default(n,"enabled",PropertyValue::Bool(*enabled))?,
                S::TypewriterSpeed { chars_per_sec } => self.default(n,"speed",PropertyValue::Int(*chars_per_sec as i64))?,
                S::If { condition, then_branch, else_branch } => {
                    continuation = "completed"; self.input_expr(n,"condition",condition)?;
                    for (key, branch) in [("then",then_branch),("else",else_branch)] {
                        self.lane += 960.0;
                        x = x.max(self.sequence(branch,(n,key.into()), self.graph.nodes[&n].position[0]+960.0, self.lane,Some(n))?);
                    }
                }
                S::Choice { options } => {
                    continuation = "completed";
                    for option in options {
                        let pins = self.graph.add_choice_option(n,text_source(&option.label)).map_err(err)?;
                        if let Some(condition) = &option.condition { self.property(n,&format!("option_{}_condition",pins.index),PropertyValue::String(expression_source(condition)))?; }
                        self.lane += 960.0;
                        x = x.max(self.sequence(&option.body,(n,format!("option_{}",pins.index)),self.graph.nodes[&n].position[0]+960.0,self.lane,Some(n))?);
                    }
                }
                S::Imagemap { background, hover_image, hotspots } => {
                    continuation = "completed"; self.text(n,"background",background)?; self.text(n,"hover",hover_image.as_deref().unwrap_or(""))?;
                    let mut specs = Vec::new();
                    for h in hotspots {
                        let name = h.name.as_deref().ok_or("Les zones sans nom ne sont pas prises en charge")?;
                        if name.contains(':') { return Err("Nom de zone contenant ':' non représentable".into()); }
                        let a = &h.area; let mut s = format!("{name}:{}:{}:{}:{}",a.x1,a.y1,a.x2,a.y2);
                        if let Some(a) = &h.hover_area { s.push_str(&format!(":{}:{}:{}:{}",a.x1,a.y1,a.x2,a.y2)); } specs.push(s);
                    }
                    self.graph.set_imagemap_hotspots(n,specs).map_err(err)?;
                    for (i,h) in hotspots.iter().enumerate() {
                        self.lane += 960.0;
                        x = x.max(self.sequence(&h.body,(n,format!("hotspot_{i}")),self.graph.nodes[&n].position[0]+960.0,self.lane,Some(n))?);
                    }
                }
                S::Return | S::VoiceStop => {},
                _ => unreachable!(),
            }
            x += 960.0;
            if matches!(stmt,S::Jump { .. } | S::Return) {
                if index + 1 != body.len() { return Err("Instructions inaccessibles après jump/return : conversion refusée".into()); }
                return Ok(x);
            }
            prev = (n,continuation.into());
        }
        if let Some(owner) = owner {
            let end = self.graph.add_branch_end(owner,[x,y]).map_err(err)?;
            self.connect(prev.0,&prev.1,end,"exec_in")?;
        }
        Ok(x)
    }
}
