//! Primitive declaration edits use the executable local, with a captured
//! expression as a compare-and-swap guard. No last-assignment cache is edited.
use crate::{GraphDocument, LocalVariableInitializer, NodeId, NodeKind, PinDirection,
    PinId, PropertyValue};
use std::collections::BTreeMap;

/// Read-only preflight for one activation-local slot. Reconstruction and the
/// project owner's transaction must still be prepared before installing it.
/// Pins are classified individually: a global assignment can feed a later
/// local read without making the assignment itself a local write.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalVariableTypeProposal {
    before: GraphDocument,
    name: String,
    previous_type: crate::ValueType,
    target_type: crate::ValueType,
    enum_default1026: Option<rvn_parser::enum_asset::EnumValue>,
    declarations: Vec<LocalVariableInitializer>,
    detached_declarations: std::collections::BTreeSet<NodeId>,
    local_pins: std::collections::BTreeSet<PinId>,
    foreign_pins: std::collections::BTreeSet<PinId>,
    shared_pins: std::collections::BTreeSet<PinId>,
    unresolved_pins: std::collections::BTreeSet<PinId>,
    references: std::collections::BTreeSet<NodeId>,
    dependent_calls: BTreeMap<String, crate::VariableBindingContext>,
}

impl LocalVariableTypeProposal {
    /// Explicit selected member; syntax/UUID alone never admits a project asset.
    pub fn with_enum_default1026(mut self, value:rvn_parser::enum_asset::EnumValue, resources:&rvn_parser::enum_asset_admission1025::EnumAssets1025) -> Result<Self,String> {
        resources.validate_value(value)?;
        if self.target_type != crate::ValueType::Enum(value.asset) { return Err("The selected member has another nominal local variable type".into()); }
        self.enum_default1026=Some(value);
        Ok(self)
    }
    pub fn name(&self) -> &str { &self.name }
    pub fn previous_type(&self) -> &crate::ValueType { &self.previous_type }
    pub fn target_type(&self) -> &crate::ValueType { &self.target_type }
    pub fn is_noop(&self) -> bool { self.previous_type == self.target_type }
    pub fn declarations(&self) -> &[LocalVariableInitializer] { &self.declarations }
    pub fn detached_declarations(&self) -> &std::collections::BTreeSet<NodeId> { &self.detached_declarations }
    pub fn local_pins(&self) -> &std::collections::BTreeSet<PinId> { &self.local_pins }
    pub fn foreign_pins(&self) -> &std::collections::BTreeSet<PinId> { &self.foreign_pins }
    pub fn shared_pins(&self) -> &std::collections::BTreeSet<PinId> { &self.shared_pins }
    pub fn unresolved_pins(&self) -> &std::collections::BTreeSet<PinId> { &self.unresolved_pins }
    pub fn references(&self) -> &std::collections::BTreeSet<NodeId> { &self.references }
    pub fn dependent_calls(&self) -> &BTreeMap<String, crate::VariableBindingContext> { &self.dependent_calls }
    pub fn requires_confirmation(&self) -> bool { !self.is_noop() && !self.references.is_empty() }
    pub fn requires_project_analysis(&self) -> bool {
        !self.is_noop() && (!self.shared_pins.is_empty()
            || !self.unresolved_pins.is_empty() || !self.dependent_calls.is_empty())
    }

    /// A graph guard, not a project-owner lease. The editor must also check
    /// the originating asset/window/workspace before opening a transaction.
    /// Compare the entire authoring model, including topology and metadata.
    pub fn validate_against(&self, graph: &GraphDocument) -> Result<(), String> {
        if graph != &self.before {
            return Err("The graph changed while the variable type decision was pending".into());
        }
        Ok(())
    }

    /// Prepare an independent authoring document, never install it on an editor
    /// owner. The caller still owes confirmation, a live owner check and one
    /// history transaction. Retained type errors are not executable success.
    pub fn reconstruct(&self, graph: &GraphDocument) -> Result<GraphDocument, String> {
        self.reconstruct_with_scene_entries(graph, &Default::default())
    }

    /// Only SourceProject may supply labels proved to create fresh activations.
    /// This does not waive shared pins, unresolved reads or fragment effects.
    pub(crate) fn reconstruct_with_scene_entries(&self, graph: &GraphDocument,
        scene_entries: &std::collections::BTreeSet<String>) -> Result<GraphDocument, String> {
        self.reconstruct_context(graph, scene_entries, None)
    }

    pub fn reconstruct_in_project(&self, graph: &GraphDocument, project: &[GraphDocument]) -> Result<GraphDocument, String> {
        self.reconstruct_in_project_with_scene_entries(graph, &Default::default(), project)
    }

    pub(crate) fn reconstruct_in_project_with_scene_entries(&self, graph: &GraphDocument,
        scene_entries: &std::collections::BTreeSet<String>, project: &[GraphDocument]) -> Result<GraphDocument, String> {
        graph.binding_project_owner_index(project)?;
        self.reconstruct_context(graph, scene_entries, Some(project))
    }

    fn reconstruct_context(&self, graph: &GraphDocument,
        scene_entries: &std::collections::BTreeSet<String>, project: Option<&[GraphDocument]>) -> Result<GraphDocument, String> {
        use crate::{ValueType, VariableBindingContext as Context};
        self.validate_against(graph)?;
        if self.is_noop() { return Ok(graph.clone()); }
        if !self.shared_pins.is_empty() || !self.unresolved_pins.is_empty()
            || self.dependent_calls.keys().any(|target| !scene_entries.contains(target)) {
            return Err("This local type change needs project binding/caller analysis".into());
        }
        let default = if let ValueType::Enum(asset)=self.target_type {
            let value=self.enum_default1026.filter(|value|value.asset==asset).ok_or("Changing a local to Enum requires an explicitly selected admitted member")?;
            PropertyValue::Enum(value)
        } else { local_type_default(&self.target_type)? };
        let globals = graph.source_global_binding_types()?;
        let projection = match project { Some(project) => graph.variable_binding_projection_in_project(&self.name, project), None => graph.variable_binding_projection(&self.name) }.map_err(|e| e.to_string())?;
        let mut next = graph.clone();
        let definition = next.variables.get_mut(&self.name).ok_or("Missing local definition")?;
        definition.value_type = self.target_type.clone();
        definition.default_value = default.clone();
        for id in &self.local_pins {
            next.pins.get_mut(id).ok_or("Missing local reference pin")?.value_type = self.target_type.clone();
        }
        // An unconsumed Set/Local output has no read context. Its stored type
        // follows its write owner; a consumed output follows its read context.
        for (node, context) in &projection.write_contexts {
            if *context != Context::ActivationLocal { continue; }
            if let Some(pin) = graph.pin_by_key(*node, "value_out") {
                if !projection.read_contexts.contains_key(&pin.id) {
                    next.pins.get_mut(&pin.id).ok_or("Missing local output")?.value_type = self.target_type.clone();
                }
            }
        }
        for declaration in &self.declarations {
            // RVN computed initializers are actual Blueprint expressions, not
            // an Unreal function-entry default. Keep their wires/side effects.
            if declaration.literal_value.is_some() {
                let input = graph.pin_by_key(declaration.node, "value").ok_or("Missing declaration input")?.id;
                reset_local_type_default(&mut next, input, &self.target_type, &default)?;
            }
        }
        for node in &self.detached_declarations {
            let input=graph.pin_by_key(*node,"value").ok_or("Missing detached local input")?.id;
            if detached_scalar_input(graph,input,graph.nodes.len()+1) {
                reset_local_type_default(&mut next,input,&self.target_type,&default)?;
            }
        }
        for id in &self.local_pins {
            let pin = &graph.pins[id];
            if pin.direction != PinDirection::Input || graph.nodes[&pin.node].kind != NodeKind::SetVariable
                || graph.edges.values().any(|edge| edge.input == *id) { continue; }
            let old_type = graph.pin_constraint_type(*id).ok_or("Missing assignment type")?;
            let old_default = pin.default_value.as_ref();
            let automatic = local_type_default(&old_type).ok();
            if old_default.is_none() || old_default == automatic.as_ref() {
                reset_local_type_default(&mut next, *id, &self.target_type, &default)?;
                continue;
            }
            let old = old_default.unwrap();
            let source_type = crate::type_inference::property_type(old);
            let precision_cast = self.target_type == ValueType::Float && source_type == ValueType::Int;
            if self.target_type.accepts(&source_type) && !precision_cast { continue; }
            let path = self.target_type.conversion_specs_from(&source_type)
                .ok_or("The explicit assignment default needs an orphan-pin migration before changing this type")?;
            let position = graph.nodes[&pin.node].position;
            let literal = next.add_catalog_node(NodeKind::Literal, [position[0] - 360.0, position[1] + 64.0])
                .map_err(|e| e.to_string())?;
            next.set_property(literal, "value", old.clone()).map_err(|e| e.to_string())?;
            let mut output = next.pin_by_key(literal, "value").ok_or("Missing literal output")?.id;
            for (index, spec) in path.into_iter().enumerate() {
                let (_, input, result) = next.add_conversion_node(spec,
                    [position[0] - 180.0 + index as f64 * 180.0, position[1] + 64.0])
                    .map_err(|e| e.to_string())?;
                next.connect(output, input).map_err(|e| e.to_string())?;
                output = result;
            }
            next.connect(output, *id).map_err(|e| e.to_string())?;
            next.pins.get_mut(id).unwrap().default_value = Some(default.clone());
        }
        next.adopt_nominal_enum_schema1026();
        let edited = project.map(|project| graph.binding_project_replacing(&next, project)).transpose()?;
        match &edited { Some(project) => next.refresh_narrative_binding_types_in_project(&globals, project)?, None => next.refresh_narrative_binding_types(&globals)? };
        // The binding-analysis compiler tolerates only retained type errors.
        // Broken structure, cycles and missing expressions remain fatal.
        let edited = project.map(|project| graph.binding_project_replacing(&next, project)).transpose()?;
        match &edited { Some(project) => next.local_declaration_initializers_for_binding_analysis_in_project(project), None => next.local_declaration_initializers_for_binding_analysis() }.map_err(|e| e.to_string())?;
        Ok(next)
    }
}

// Inspect only authored primitive syntax, never execute a detached RHS.
// Computed producers (including Append and interpolated text) keep their wires.
fn detached_scalar_input(graph:&GraphDocument,input:PinId,remaining:usize)->bool {
    if remaining==0{return false;}
    let Some(pin)=graph.pins.get(&input)else{return false;};
    let Some(edge)=graph.edges.values().find(|edge|edge.input==input)else{return true;};
    let Some(node)=graph.pins.get(&edge.output).and_then(|pin|graph.nodes.get(&pin.node))else{return false;};
    if node.kind==NodeKind::MathNegate {
        return graph.pin_by_key(node.id,"value").is_some_and(|pin|detached_scalar_input(graph,pin.id,remaining-1));
    }
    node.kind==NodeKind::Literal&&matches!(node.properties.get("value"),Some(PropertyValue::Bool(_)|PropertyValue::Int(_)|PropertyValue::Float(_)|PropertyValue::String(_)|PropertyValue::Name(_)|PropertyValue::Enum(_)))
}

fn local_type_default(value_type: &crate::ValueType) -> Result<PropertyValue, String> {
    use crate::ValueType as T;
    fn valid_list_item(ty: &T) -> bool {
        match ty { T::Execution => false, T::List(item) => valid_list_item(item), _ => true }
    }
    Ok(match value_type {
        T::Bool => PropertyValue::Bool(false), T::Int => PropertyValue::Int(0),
        T::Float => PropertyValue::Float(0.0),
        T::String | T::InterpolatedText => PropertyValue::String(String::new()),
        T::Name => PropertyValue::Name(crate::NameDefault::default()),
        T::List(item) if valid_list_item(item) => PropertyValue::StringList(Vec::new()),
        _ => return Err("This value type needs a dedicated RVN default constructor before reconstruction".into()),
    })
}

fn reset_local_type_default(graph: &mut GraphDocument, input: PinId, target: &crate::ValueType,
    value: &PropertyValue) -> Result<(), String>
{
    use crate::ValueType as T;
    let edge = graph.edges.values().find(|edge| edge.input == input).cloned();
    if edge.is_none() && !matches!(target, T::String | T::InterpolatedText | T::List(_)) {
        graph.pins.get_mut(&input).ok_or("Missing declaration input")?.default_value = Some(value.clone());
        return Ok(());
    }
    // Reuse the existing scalar edit's copy-on-write path only when the node
    // can really produce the new type. A Text node never becomes an Int merely
    // because its input consumer changed, nor does a signed expression become
    // a Boolean. Old expression nodes are retained for other consumers/Undo.
    if !matches!(target, T::List(_) | T::InterpolatedText) {
        let producer = edge.as_ref().and_then(|edge| graph.pins.get(&edge.output))
            .and_then(|pin| graph.nodes.get(&pin.node));
        if producer.is_some_and(|node| node.kind == NodeKind::Literal
            || (node.kind == NodeKind::MathNegate && matches!(target, T::Int | T::Float))) {
            let depth = graph.nodes.len() + 1;
            set_scalar(graph, input, value.clone(), depth)?;
            return Ok(());
        }
    }
    let owner = graph.pins.get(&input).ok_or("Missing declaration input")?.node;
    let position = graph.nodes.get(&owner).ok_or("Missing declaration")?.position;
    let kind = match target { T::InterpolatedText => NodeKind::TextValue,
        T::List(_) => NodeKind::ListLiteral, _ => NodeKind::Literal };
    let producer = graph.add_catalog_node(kind, [position[0] - 180.0, position[1] + 64.0])
        .map_err(|e| e.to_string())?;
    if kind != NodeKind::ListLiteral {
        graph.set_property(producer, "value", value.clone()).map_err(|e| e.to_string())?;
    }
    let output = graph.nodes[&producer].pins.iter().filter_map(|id| graph.pins.get(id))
        .find(|pin| pin.direction == PinDirection::Output).ok_or("Missing default output")?.id;
    if let Some(edge) = edge { graph.edges.get_mut(&edge.id).unwrap().output = output; }
    else { graph.connect(output, input).map_err(|e| e.to_string())?; }
    graph.pins.get_mut(&input).unwrap().default_value = None;
    Ok(())
}

fn same_primitive(a: &PropertyValue, b: &PropertyValue) -> bool {
    if let (PropertyValue::Enum(a), PropertyValue::Enum(b)) = (a, b) { return a.asset == b.asset; }
    matches!((a, b), (PropertyValue::Bool(_), PropertyValue::Bool(_))
        | (PropertyValue::Int(_), PropertyValue::Int(_))
        | (PropertyValue::Float(_), PropertyValue::Float(_))
        | (PropertyValue::Name(_), PropertyValue::Name(_))
        | (PropertyValue::String(_), PropertyValue::String(_)))
}

fn negate(value: PropertyValue) -> Result<PropertyValue, String> {
    match value {
        PropertyValue::Int(n) => n.checked_neg().map(PropertyValue::Int)
            .ok_or_else(|| "This signed expression cannot represent that integer without changing its operator".into()),
        PropertyValue::Float(n) => Ok(PropertyValue::Float(-n)),
        _ => Err("A signed initializer requires a numeric value".into()),
    }
}

/// Retain every original producer/pin/edge for foreign consumers. Existing
/// consumer edges keep their identity; only their output changes on a copy.
fn own_producer(graph: &mut GraphDocument, consumer: PinId) -> Result<Option<NodeId>, String> {
    let Some(edge_id) = graph.edges.values().find(|edge| edge.input == consumer).map(|edge| edge.id)
        else { return Ok(None); };
    let output = graph.edges[&edge_id].output;
    let node = graph.pins.get(&output).ok_or("Missing initializer producer pin")?.node;
    let model = graph.nodes.get(&node).ok_or("Missing initializer producer")?.clone();
    if !matches!(model.kind, NodeKind::Literal | NodeKind::TextValue | NodeKind::MathNegate)
        || model.pins.iter().any(|id| graph.pins[id].value_type.is_execution()) {
        return Err("This initializer is computed: edit its Blueprint expression".into());
    }
    let shared = graph.edges.values().any(|edge| edge.id != edge_id
        && graph.pins.get(&edge.output).is_some_and(|pin| pin.node == node));
    if !shared { return Ok(Some(node)); }
    let copied = graph.add_node(model.kind, [model.position[0] + 32.0, model.position[1] + 32.0]);
    graph.nodes.get_mut(&copied).unwrap().properties = model.properties.clone();
    graph.nodes.get_mut(&copied).unwrap().title_override = model.title_override.clone();
    graph.nodes.get_mut(&copied).unwrap().annotation = model.annotation.clone();
    let mut pins = BTreeMap::new();
    for id in &model.pins {
        let old = graph.pins[id].clone();
        let new = graph.add_pin(copied, &old.key, &old.label, old.direction,
            old.value_type.clone(), old.cardinality).map_err(|error| error.to_string())?;
        let mut copy = old; copy.id = new; copy.node = copied;
        graph.pins.insert(new, copy); pins.insert(*id, new);
    }
    let incoming: Vec<_> = graph.edges.values().filter(|edge| model.pins.contains(&edge.input))
        .map(|edge| (edge.output, edge.input)).collect();
    for (source, input) in incoming {
        // connect may hide an inline default; retain the authored original.
        let default = graph.pins[&pins[&input]].default_value.clone();
        graph.connect(source, pins[&input]).map_err(|error| error.to_string())?;
        graph.pins.get_mut(&pins[&input]).unwrap().default_value = default;
    }
    graph.edges.get_mut(&edge_id).unwrap().output = pins[&output];
    Ok(Some(copied))
}

fn set_scalar(graph: &mut GraphDocument, consumer: PinId, value: PropertyValue,
    remaining: usize) -> Result<(), String>
{
    if remaining == 0 { return Err("Initializer expression is cyclic or too deep".into()); }
    let pin = graph.pins.get(&consumer).ok_or("Missing declaration input")?;
    if pin.direction != PinDirection::Input || pin.value_type.is_execution() {
        return Err("Expected a declaration data input".into());
    }
    match own_producer(graph, consumer)? {
        None => graph.pins.get_mut(&consumer).unwrap().default_value = Some(value),
        Some(node) if graph.nodes[&node].kind == NodeKind::MathNegate => {
            let input = graph.pin_by_key(node, "value").ok_or("Missing signed initializer input")?.id;
            set_scalar(graph, input, negate(value)?, remaining - 1)?;
        }
        Some(node) => graph.set_property(node, "value", value).map_err(|error| error.to_string())?,
    }
    Ok(())
}

impl GraphDocument {
    /// Prepare scope evidence without mutating pins, declarations or caches.
    /// Every declaration of this activation slot is retained; the captured
    /// declaration proves identity rather than choosing the first by name.
    /// Incompatible wires are inspectable, all structural errors remain fatal.
    pub fn prepare_local_variable_type(&self, name: &str,
        expected: &LocalVariableInitializer, target_type: crate::ValueType)
        -> Result<LocalVariableTypeProposal, String>
    {
        self.prepare_local_variable_type_context(name, expected, target_type, None)
    }

    pub fn prepare_local_variable_type_in_project(&self, name: &str, expected: &LocalVariableInitializer,
        target_type: crate::ValueType, project: &[GraphDocument]) -> Result<LocalVariableTypeProposal, String> {
        self.binding_project_owner_index(project)?;
        self.prepare_local_variable_type_context(name, expected, target_type, Some(project))
    }

    fn prepare_local_variable_type_context(&self, name: &str, expected: &LocalVariableInitializer,
        target_type: crate::ValueType, project: Option<&[GraphDocument]>) -> Result<LocalVariableTypeProposal, String> {
        let declarations=self.local_initializers_context(project)
            .map_err(|error|error.to_string())?.remove(name).unwrap_or_default();
        if !declarations.iter().any(|declaration|declaration==expected){
            return Err("The captured local declaration changed or belongs to another graph".into());
        }
        self.prepare_local_variable_type_slot_context(name,target_type,project)
    }

    /// A calculation graph owns its local slot even before its promoted
    /// declaration and references are connected. Narrative fragments still
    /// require genuine consumer/caller evidence and never use this fallback.
    pub fn prepare_local_variable_type_slot(&self,name:&str,target_type:crate::ValueType)
        ->Result<LocalVariableTypeProposal,String>
    {
        self.prepare_local_variable_type_slot_context(name, target_type, None)
    }

    pub fn prepare_local_variable_type_slot_in_project(&self, name: &str, target_type: crate::ValueType,
        project: &[GraphDocument]) -> Result<LocalVariableTypeProposal, String> {
        self.binding_project_owner_index(project)?;
        self.prepare_local_variable_type_slot_context(name, target_type, Some(project))
    }

    fn local_initializers_context(&self, project: Option<&[GraphDocument]>)
        -> Result<BTreeMap<String, Vec<LocalVariableInitializer>>, crate::TranspileError> {
        match project { Some(project) => self.local_declaration_initializers_for_binding_analysis_in_project(project), None => self.local_declaration_initializers_for_binding_analysis() }
    }

    fn prepare_local_variable_type_slot_context(&self, name: &str, target_type: crate::ValueType,
        project: Option<&[GraphDocument]>) -> Result<LocalVariableTypeProposal, String> {
        use crate::{ValueType, VariableBindingContext as Context, VariableScope};
        if target_type == ValueType::Execution {
            return Err("An execution pin is not a variable value type".into());
        }
        if self.variable_scope(name) != Some(VariableScope::Local) {
            return Err("The requested definition is not an activation-local slot".into());
        }
        let declarations = self.local_initializers_context(project)
            .map_err(|error| error.to_string())?.remove(name).unwrap_or_default();
        let calculation_slot=matches!(self.kind,crate::GraphKind::UnitFunction{..}|crate::GraphKind::Function{..}|crate::GraphKind::Screen{..}|crate::GraphKind::Handler{..});
        let detached_declarations:std::collections::BTreeSet<_>=if calculation_slot {
            self.nodes.values().filter(|node|node.kind==NodeKind::LocalVariable
                &&self.variable_node_name(node.id).as_deref()==Some(name)
                &&!declarations.iter().any(|declaration|declaration.node==node.id)).map(|node|node.id).collect()
        }else{Default::default()};
        if declarations.is_empty()&&detached_declarations.is_empty(){
            return Err("This slot needs project analysis of its declarations".into());
        }
        let projection = match project { Some(project) => self.variable_binding_projection_in_project(name, project), None => self.variable_binding_projection(name) }.map_err(|error| error.to_string())?;
        let definition = self.variables.get(name).ok_or("Missing local variable definition")?;
        let declaration_ids: std::collections::BTreeSet<_> = declarations.iter().map(|d| d.node).chain(detached_declarations.iter().copied()).collect();
        let mut proposal = LocalVariableTypeProposal {
            before: self.clone(), name: name.into(), previous_type: definition.value_type.clone(),
            target_type, enum_default1026:None, declarations, detached_declarations, local_pins: Default::default(), foreign_pins: Default::default(),
            shared_pins: Default::default(), unresolved_pins: Default::default(), references: Default::default(),
            dependent_calls: projection.narrative_call_contexts.iter()
                .filter(|(_, context)| **context != Context::ProjectGlobal)
                .map(|(name, context)| (name.clone(), *context)).collect(),
        };
        for (node, context) in &projection.embedded_reference_contexts {
            if *context != Context::ProjectGlobal { proposal.references.insert(*node); }
        }
        for node in self.nodes.values().filter(|node| matches!(node.kind,
            NodeKind::VariableGet | NodeKind::SetVariable | NodeKind::LocalVariable)
            && self.variable_node_name(node.id).as_deref() == Some(name))
        {
            for id in &node.pins {
                let pin = self.pins.get(id).ok_or("Missing variable reference pin")?;
                if !matches!(pin.key.as_str(), "value" | "value_out") { continue; }
                let context = if pin.direction == PinDirection::Input {
                    projection.write_contexts.get(&node.id).copied()
                } else {
                    projection.read_contexts.get(id).copied()
                };
                // Unconnected calculation nodes still refer to their graph's
                // declared local slot. Preserve every actual reachable context,
                // including a global read before an executable Local.
                let context=context.or_else(||calculation_slot.then_some(Context::ActivationLocal));
                // A narrative output without a consumer carries no new proof.
                if context.is_none() && pin.direction == PinDirection::Output
                    && node.kind != NodeKind::VariableGet { continue; }
                match context {
                    Some(Context::ActivationLocal) => { proposal.local_pins.insert(*id); }
                    Some(Context::ProjectGlobal) => { proposal.foreign_pins.insert(*id); }
                    Some(Context::GlobalOrLocal) => { proposal.shared_pins.insert(*id); }
                    None => { proposal.unresolved_pins.insert(*id); }
                }
                if context != Some(Context::ProjectGlobal) && !declaration_ids.contains(&node.id) {
                    proposal.references.insert(node.id);
                }
            }
        }
        Ok(proposal)
    }

    /// An authoring edit may retain incompatible wires; structure and binding
    /// identity remain validated, and executable publication stays strict.
    /// Purely preflight on a clone before publishing. Return false for a noop
    /// so editor history/dirty state need not change. Repeated declarations
    /// require their explicit node identity rather than a name-only default.
    pub fn replace_local_initializer(&mut self, name: &str,
        expected: &LocalVariableInitializer, mut value: PropertyValue)
        -> Result<bool, String>
    {
        self.replace_local_initializer_context(name, expected, value, None)
    }

    pub fn replace_local_initializer_in_project(&mut self, name: &str, expected: &LocalVariableInitializer,
        value: PropertyValue, project: &[GraphDocument]) -> Result<bool, String> {
        self.binding_project_owner_index(project)?;
        self.replace_local_initializer_context(name, expected, value, Some(project))
    }

    fn replace_local_initializer_context(&mut self, name: &str, expected: &LocalVariableInitializer,
        mut value: PropertyValue, project: Option<&[GraphDocument]>) -> Result<bool, String> {
        let current = self.local_initializers_context(project).map_err(|error| error.to_string())?
            .remove(name).unwrap_or_default().into_iter().find(|entry| entry.node == expected.node)
            .ok_or("The local declaration no longer belongs to this binding")?;
        if current != *expected { return Err("The initial value changed while it was being edited".into()); }
        let old = current.literal_value.as_ref().ok_or("This initializer is computed: edit its Blueprint expression")?;
        if !same_primitive(old, &value) { return Err("The initial value must retain its declared primitive type".into()); }
        if let PropertyValue::Float(n) = value {
            let runtime = n as f32;
            if !n.is_finite() || !runtime.is_finite() { return Err("A finite RVN number is required".into()); }
            value = PropertyValue::Float(runtime as f64);
        }
        if old == &value { return Ok(false); }
        let input = self.pin_by_key(expected.node, "value").ok_or("Missing local initializer pin")?.id;
        let mut next = self.clone();
        let depth = next.nodes.len() + 1;
        set_scalar(&mut next, input, value.clone(), depth)?;
        let edited = project.map(|project| self.binding_project_replacing(&next, project)).transpose()?;
        let globals = next.source_global_binding_types()?;
        match &edited { Some(project) => next.refresh_narrative_binding_types_in_project(&globals, project)?, None => next.refresh_narrative_binding_types(&globals)? };
        let edited = project.map(|project| self.binding_project_replacing(&next, project)).transpose()?;
        let changed = next.local_initializers_context(edited.as_deref()).map_err(|error| error.to_string())?
            .remove(name).unwrap_or_default().into_iter().find(|entry| entry.node == expected.node).ok_or("Missing edited declaration")?;
        if changed.literal_value.as_ref() != Some(&value) {
            return Err("The edited expression does not produce the requested primitive value".into());
        }
        *self = next;
        Ok(true)
    }
}
