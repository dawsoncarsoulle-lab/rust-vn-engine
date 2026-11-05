//! Authored schema14 Unit parameter projections over real entry/call pins.
//! SourceWorkspace owns admission/publication. This helper only projects real
//! entry/call pins; it never authorizes a GraphKind, source refactor or service.
use crate::{GraphDocument, GraphId, GraphPin, NodeId, NodeKind, PinDirection,
    PinId, PropertyValue, SourceWorkspace, ValueType};
use crate::{ParameterId, GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION};
use std::{collections::BTreeSet, path::PathBuf};

/// Opaque authored identity in the actual owner/declaration, plus a borrowed
/// snapshot guard for editing. PinId is layout only and is not stored here.
/// No public constructor or serialization grants cross-owner authority.
#[derive(Clone, Debug)]
pub(super) struct ParameterBinding<'a> {
    owner: PathBuf,
    declaration: GraphId,
    parameter: ParameterId,
    observed: &'a GraphDocument,
}

impl PartialEq for ParameterBinding<'_> {
    fn eq(&self, other: &Self) -> bool {
        (&self.owner, self.declaration, &self.parameter) == (&other.owner, other.declaration, &other.parameter)
    }
}
impl Eq for ParameterBinding<'_> {}
impl PartialOrd for ParameterBinding<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}
impl Ord for ParameterBinding<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.owner, self.declaration, &self.parameter).cmp(&(&other.owner, other.declaration, &other.parameter))
    }
}

/// Borrows the complete admitted owner's current authored projection.
/// It stores no parameter names, types, defaults or second signature table.
pub(super) struct EntryParameters<'a> {
    workspace: &'a SourceWorkspace,
    owner: PathBuf,
    graphs: &'a [GraphDocument],
    declaration: GraphId,
    entry: NodeId,
}

impl<'a> EntryParameters<'a> {
    /// Call only inside SourceWorkspace's existing authoring admission frontier.
    /// Future Unit graph policy remains outside this structural helper. The
    /// caller supplies every authored graph, including closed graphs.
    pub(super) fn from_admitted_projection(
        workspace: &'a SourceWorkspace, graphs: &'a [GraphDocument],
        declaration: GraphId, entry: NodeId,
    ) -> Result<Self, String> {
        if workspace.source_error.is_some() || workspace.project().has_imports() {
            return Err("Parameter projection needs a current owned source; imported declarations require their own admitted owner mapping".into());
        }
        let expected = workspace.project().graphs();
        let ids: BTreeSet<_> = graphs.iter().map(|graph| graph.graph_id).collect();
        if ids.len() != graphs.len() || ids != expected.iter().map(|graph| graph.graph_id).collect() {
            return Err("Parameter projection omits, duplicates or substitutes an authored graph".into());
        }
        for graph in graphs {
            let baseline = expected.iter().find(|old| old.graph_id == graph.graph_id)
                .ok_or("Parameter graph is outside its admitted owner")?;
            if graph.kind != baseline.kind || (graph.schema_version != baseline.schema_version
                && !crate::supports_unit_schema1026(graph.schema_version)) {
                return Err("Graph family/name/schema changes need the owning source transaction first".into());
            }
        }
        let owner = workspace.source_path();
        if !owner.is_absolute() { return Err("An admitted absolute source locator is required".into()); }
        let result = Self { workspace, owner, graphs, declaration, entry };
        result.pins()?;
        result.validate_argument_bindings()?;
        Ok(result)
    }

    fn graph(&self) -> Result<&'a GraphDocument, String> {
        self.graphs.iter().find(|graph| graph.graph_id == self.declaration)
            .ok_or_else(|| "Parameter declaration is absent".into())
    }

    /// The actual entry pin vector is the sole ordered signature. Parameter
    /// labels are lexical names; source remains untyped with no defaults.
    pub(super) fn pins(&self) -> Result<Vec<&'a GraphPin>, String> {
        let graph = self.graph()?;
        if !matches!(graph.kind,crate::GraphKind::UnitFunction{..}) { return Err("An actual Unit declaration is required".into()); }
        if !crate::supports_unit_schema1026(graph.schema_version) {
            return Err("Parameter identity requires explicit schema14 authoring; legacy load/save allocates nothing".into());
        }
        let entry = graph.nodes.get(&self.entry).ok_or("Parameter entry is absent")?;
        if entry.id != self.entry || entry.kind != NodeKind::FunctionEntry {
            return Err("A real FunctionEntry is required".into());
        }
        match entry.properties.get("parameters") {
            None => (),
            Some(PropertyValue::StringList(names)) if names.is_empty() => (),
            _ => return Err("Legacy metadata-only parameters need an explicit future migration; they are never materialized implicitly".into()),
        }
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut parameter_ids = BTreeSet::new();
        let mut parameters = Vec::new();
        let mut exec = 0;
        for id in &entry.pins {
            let pin = graph.pins.get(id).ok_or("Entry references an absent pin")?;
            if id.get() == 0 || pin.id != *id || pin.node != self.entry || !ids.insert(*id) {
                return Err("Malformed parameter pin ownership/identity".into());
            }
            if pin.key == "exec_out" && pin.direction == PinDirection::Output
                && pin.value_type == ValueType::Execution { exec += 1; continue; }
            let parameter = pin.parameter_id.as_ref().ok_or("Materialized parameter has no opaque identity")?;
            if pin.key != format!("parameter_{}", parameter.as_str()) || !parameter_ids.insert(parameter)
                || pin.direction != PinDirection::Output || pin.cardinality != crate::PinCardinality::Many || pin.value_type != ValueType::Any
                || pin.default_value.is_some() || pin.text_localization.is_some()
                || pin.text_table_reference.is_some()
                || rvn_parser::variable_binding_source_name(&pin.label).is_none()
                || !names.insert(pin.label.as_str()) {
                return Err("Malformed materialized lexical parameter".into());
            }
            parameters.push(pin);
        }
        if exec != 1 || parameters.len() > 128 || graph.pins.iter().any(|(id, pin)|
            pin.node == self.entry && (pin.id != *id || !ids.contains(id))) {
            return Err("Incomplete entry pin ownership/signature".into());
        }
        Ok(parameters)
    }

    pub(super) fn bindings(&self) -> Result<Vec<ParameterBinding<'a>>, String> {
        let observed = self.graph()?;
        self.pins()?.iter().map(|pin| Ok(ParameterBinding {
            owner: self.owner.clone(), declaration: self.declaration, parameter: pin.parameter_id.clone().ok_or("Parameter identity disappeared")?,
            observed,
        })).collect()
    }

    fn parameter(&self, binding: &ParameterBinding<'_>) -> Result<&'a GraphPin, String> {
        if binding.owner != self.owner || binding.declaration != self.declaration {
            return Err("Parameter belongs to another admitted source/declaration".into());
        }
        if binding.observed != self.graph()? {
            return Err("Parameter binding snapshot is stale; re-admit the actual graph/source transaction".into());
        }
        self.pins()?.into_iter().find(|pin| pin.parameter_id.as_ref() == Some(&binding.parameter))
            .ok_or_else(|| "Parameter identity is stale; explicit source reconciliation is required".into())
    }

    fn argument_key(&self, parameter: &ParameterId) -> String {
        format!("argument_{}", parameter.as_str())
    }

    // References are scoped to this already resolved callee. A bare UUID or
    // GraphId is data, never project/source or callable-mode authority.
    fn validate_argument_bindings(&self) -> Result<(), String> {
        crate::unit_graph_source1003::validate_project(self.graphs)?;
        let parameters = self.pins()?;
        let valid: BTreeSet<_> = parameters.into_iter().filter_map(|pin| pin.parameter_id.as_ref()).collect();
        let mut calls = BTreeSet::new();
        for graph in self.graphs {
            for pin in graph.pins.values().filter(|pin| pin.direction == PinDirection::Input
                && pin.parameter_id.as_ref().is_some_and(|id| valid.contains(id))) {
                calls.insert((graph.graph_id, pin.node));
            }
        }
        for (graph, node) in calls { self.ordered_arguments(graph, node)?; }
        Ok(())
    }

    /// Resolve real inputs by declaration identity and the current entry order,
    /// never by argument position or parameter display name. Unit call kind,
    /// target and exec/no-result policy are verified against the real project.
    pub(super) fn ordered_arguments(&self, caller: GraphId, call: NodeId) -> Result<Vec<&'a GraphPin>, String> {
        let graph = self.graphs.iter().find(|graph| graph.graph_id == caller).ok_or("Caller is outside this owner")?;
        let node = graph.nodes.get(&call).ok_or("Call node is absent")?;
        let (callee,_) = crate::unit_graph_source1003::resolve_call(self.graphs,graph,call)?;
        if callee.graph_id != self.declaration { return Err("Call resolves to another declaration".into()); }
        let mut inputs = Vec::new();
        let mut seen = BTreeSet::new();
        for id in &node.pins {
            if !seen.insert(*id) { return Err("Call duplicates an actual pin identity".into()); }
            let pin = graph.pins.get(id).ok_or("Call references an absent pin")?;
            if pin.id != *id || pin.node != call { return Err("Malformed call pin owner".into()); }
            if pin.value_type.is_execution() { continue; }
            if pin.direction != PinDirection::Input { return Err("A bound Unit argument cannot be a result output".into()); }
            inputs.push(pin);
        }
        if graph.pins.iter().any(|(id, pin)| pin.node == call && (pin.id != *id || !seen.contains(id))) {
            return Err("Call contains an unlisted actual pin".into());
        }
        let mut result = Vec::new();
        for parameter in self.pins()? {
            let matches: Vec<_> = inputs.iter().copied().filter(|pin| pin.parameter_id == parameter.parameter_id
                && pin.key == self.argument_key(parameter.parameter_id.as_ref().unwrap())).collect();
            let [pin] = matches.as_slice() else { return Err("Call argument bindings must match the complete declaration exactly".into()); };
            result.push(*pin);
        }
        if inputs.len() != result.len() { return Err("Call contains a foreign or extra argument".into()); }
        Ok(result)
    }

    pub(super) fn append_projection(&self, name: &str,
        mut allocate: impl FnMut() -> Result<String, String>) -> Result<(Vec<GraphDocument>, ParameterId), String> {
        if rvn_parser::variable_binding_source_name(name).is_none()
            || self.pins()?.iter().any(|pin| pin.label == name) || self.pins()?.len() >= 128 {
            return Err("Invalid, duplicate or excessive parameter name".into());
        }
        let parameters = self.pins()?;
        let has_call = self.graphs.iter().any(|graph| graph.nodes.values().any(|node|
            node.kind==NodeKind::UnitCall && crate::unit_graph_source1003::resolve_call(self.graphs,graph,node.id)
                .is_ok_and(|(callee,_)|callee.graph_id==self.declaration)));
        if has_call || self.graphs.iter().any(|graph| graph.pins.values().any(|pin|
            pin.direction == PinDirection::Input && parameters.iter().any(|parameter|
                pin.parameter_id.is_some() && pin.parameter_id == parameter.parameter_id))) {
            return Err("Adding a parameter requires explicit arguments at existing callers; no semantic default is fabricated".into());
        }
        self.graph()?.check_allocation_capacity(0, 1, 0).map_err(|error| format!("{error:?}"))?;
        let identity = ParameterId::parse(allocate()?)?;
        if self.graphs.iter().any(|graph| graph.pins.values().any(|pin| pin.parameter_id.as_ref() == Some(&identity))) {
            return Err("Allocated parameter identity collides with the actual project".into());
        }
        let mut result = self.graphs.to_vec();
        let graph = result.iter_mut().find(|graph| graph.graph_id == self.declaration).ok_or("Declaration disappeared")?;
        let pin = graph.add_pin(self.entry, format!("parameter_{}", identity.as_str()), name,
            PinDirection::Output, ValueType::Any, crate::PinCardinality::Many).map_err(|error| format!("{error:?}"))?;
        graph.pins.get_mut(&pin).ok_or("Allocated port disappeared")?.parameter_id = Some(identity.clone());
        Ok((result, identity))
    }

    /// Pin projection only. Root must reconcile lexical source references in
    /// its existing source transaction before publishing this result.
    pub(super) fn rename_projection(&self, binding: &ParameterBinding<'_>, name: &str) -> Result<Vec<GraphDocument>, String> {
        let pin = self.parameter(binding)?;
        if rvn_parser::variable_binding_source_name(name).is_none()
            || self.pins()?.iter().any(|other| other.id != pin.id && other.label == name)
            || (name != pin.label && (self.graph()?.variables.contains_key(name)
                || self.graph()?.local_variable_names().contains(name))) {
            return Err("Invalid or duplicate lexical parameter name".into());
        }
        // Resolve the actual current owner/source body before relying on a
        // presentation cache or editing any disposable graph projection.
        crate::unit_parameter_references1003::source_uses_parameter(self.workspace,self.graphs,self.declaration,pin)?;
        let mut result = self.graphs.to_vec();
        for graph in &mut result {
            if graph.graph_id == self.declaration {
                crate::unit_parameter_references1003::rename_unit_slot_references(graph,&pin.label,name)?;
                graph.pins.get_mut(&pin.id).ok_or("Parameter disappeared")?.label = name.into();
            }
            for argument in graph.pins.values_mut().filter(|argument| argument.direction == PinDirection::Input && argument.parameter_id == pin.parameter_id) {
                argument.label = name.into();
            }
        }
        Ok(result)
    }

    pub(super) fn reorder_projection(&self, order: &[ParameterBinding<'_>]) -> Result<Vec<GraphDocument>, String> {
        for binding in order { self.parameter(binding)?; }
        let current = self.bindings()?;
        if order.len() != current.len() || order.iter().collect::<BTreeSet<_>>() != current.iter().collect() {
            return Err("Parameter reorder must be an exact owner-qualified permutation".into());
        }
        let mut result = self.graphs.to_vec();
        let graph = result.iter_mut().find(|graph| graph.graph_id == self.declaration).ok_or("Declaration disappeared")?;
        let ids: BTreeSet<_> = self.pins()?.iter().map(|pin| pin.id).collect();
        let layout = order.iter().map(|binding| self.parameter(binding).map(|pin| pin.id)).collect::<Result<Vec<_>, _>>()?;
        let mut parameters = layout.into_iter();
        for id in &mut graph.nodes.get_mut(&self.entry).ok_or("Entry disappeared")?.pins {
            if ids.contains(id) { *id = parameters.next().ok_or("Incomplete reorder")?; }
        }
        Ok(result)
    }

    /// Conservative primitive: removal requires an unreferenced parameter.
    /// A later explicit arity/source rewrite may define a broader transaction.
    pub(super) fn remove_unreferenced_projection(&self, binding: &ParameterBinding<'_>) -> Result<Vec<GraphDocument>, String> {
        let pin = self.parameter(binding)?;
        if crate::unit_parameter_references1003::source_uses_parameter(self.workspace,self.graphs,self.declaration,pin)?
            || self.graph()?.edges.values().any(|edge| edge.input == pin.id || edge.output == pin.id)
            || self.graph()?.nodes.values().any(|node| self.graph().ok().and_then(|graph|graph.variable_node_name(node.id)).as_deref()==Some(pin.label.as_str()))
            || self.graphs.iter().any(|graph| graph.pins.values().any(|input| input.direction == PinDirection::Input && input.parameter_id == pin.parameter_id)) {
            return Err("Parameter is referenced; removal requires the complete lexical/call/source transaction".into());
        }
        let mut result = self.graphs.to_vec();
        let graph = result.iter_mut().find(|graph| graph.graph_id == self.declaration).ok_or("Declaration disappeared")?;
        graph.nodes.get_mut(&self.entry).ok_or("Entry disappeared")?.pins.retain(|id| *id != pin.id);
        graph.pins.remove(&pin.id);
        graph.variables.remove(&pin.label);
        Ok(result)
    }

    /// Numeric equality alone proves no lineage. The existing source/refactor
    /// transaction must supply its explicit old/new authoring mapping. A naked
    /// text import with no such mapping rejects even if fresh IDs coincide.
    /// This check never creates or publishes that transaction/mapping.
    pub(super) fn require_preserved_identities(&self, candidate: &[GraphDocument],
        mapping: &[(ParameterBinding<'_>, ParameterBinding<'_>)]) -> Result<(), String> {
        let previous = self.bindings()?;
        if mapping.len() != previous.len()
            || mapping.iter().map(|(old, _)| old).collect::<BTreeSet<_>>() != previous.iter().collect()
            || mapping.iter().any(|(old, new)| old != new) {
            return Err("Source import requires a complete explicit identity-preserving authoring mapping; names/positions/numeric coincidence cannot supply it".into());
        }
        let ids: BTreeSet<_> = candidate.iter().map(|graph| graph.graph_id).collect();
        if ids.len() != candidate.len() || ids != self.graphs.iter().map(|graph| graph.graph_id).collect() {
            return Err("Source import does not preserve the complete owned graph set".into());
        }
        for graph in candidate {
            let previous = self.graphs.iter().find(|old| old.graph_id == graph.graph_id).ok_or("Imported graph owner mismatch")?;
            if graph.kind != previous.kind || graph.schema_version != previous.schema_version {
                return Err("Imported graph family/name/schema needs an explicit refactor mapping".into());
            }
        }
        let projected = EntryParameters { workspace: self.workspace, owner: self.owner.clone(), graphs: candidate,
            declaration: self.declaration, entry: self.entry };
        projected.pins()?;
        projected.validate_argument_bindings()?;
        for (old, new) in mapping { self.parameter(old)?; projected.parameter(new)?; }
        if projected.bindings()?.into_iter().collect::<BTreeSet<_>>() != previous.into_iter().collect() {
            return Err("Source import changed parameter membership without an explicit source edit transaction".into());
        }
        for parameter in self.pins()? {
            let identity = parameter.parameter_id.as_ref().ok_or("Parameter identity disappeared")?;
            if !projected.pins()?.iter().any(|pin| pin.parameter_id.as_ref() == Some(identity)) {
                return Err("Opaque parameter identity was lost; explicit source reconciliation is required".into());
            }
        }
        Ok(())
    }
}

/// Per-graph metadata roles; the actual project validator additionally resolves
/// every Unit call by owner context, callee family, arity and opaque identity.
pub(crate) fn validate_graph_parameter_metadata(graph: &GraphDocument) -> Result<(), String> {
    let feature = crate::supports_unit_schema1026(graph.schema_version);
    if !feature && (matches!(graph.kind,crate::GraphKind::UnitFunction{..})
        || graph.nodes.values().any(|node|matches!(node.kind,NodeKind::UnitCall|NodeKind::UnitReturn))) {
        return Err("Actual Unit declarations/calls/returns require schema14".into());
    }
    for (id, pin) in &graph.pins {
        let Some(identity) = &pin.parameter_id else {
            if feature && graph.nodes.get(&pin.node).is_some_and(|node|
                (node.kind==NodeKind::FunctionEntry && matches!(graph.kind,crate::GraphKind::UnitFunction{..}) && pin.key.starts_with("parameter_"))
                || (node.kind==NodeKind::UnitCall && pin.key.starts_with("argument_"))) {
                return Err("Schema14 parameter/argument port is missing its opaque identity".into());
            }
            continue;
        };
        if !feature { return Err("Parameter identity metadata requires schema14; legacy graphs are not promoted on load".into()); }
        let node = graph.nodes.get(&pin.node).ok_or("Parameter port owner is absent")?;
        if pin.id != *id || id.get() == 0 || node.pins.iter().filter(|other| **other == *id).count() != 1
            || pin.value_type != ValueType::Any || pin.text_localization.is_some() || pin.text_table_reference.is_some() {
            return Err("Malformed owned parameter/argument port metadata".into());
        }
        match pin.direction {
            PinDirection::Output if node.kind == NodeKind::FunctionEntry
                && matches!(graph.kind,crate::GraphKind::UnitFunction{..})
                && pin.cardinality == crate::PinCardinality::Many
                && pin.key == format!("parameter_{}", identity.as_str()) && pin.default_value.is_none() => (),
            PinDirection::Input if node.kind == NodeKind::UnitCall
                && pin.key == format!("argument_{}", identity.as_str())
                && pin.cardinality == crate::PinCardinality::One => (),
            _ => return Err("Parameter identity is attached to an invalid entry/argument role".into()),
        }
    }
    Ok(())
}

pub(crate) fn validate_project_parameter_metadata(graphs: &[GraphDocument]) -> Result<(), String> {
    if graphs.iter().map(|graph| graph.graph_id).collect::<BTreeSet<_>>().len() != graphs.len() {
        return Err("Parameter ownership cannot resolve duplicate authored graph identities".into());
    }
    let mut definitions = BTreeSet::new();
    for graph in graphs {
        validate_graph_parameter_metadata(graph)?;
        for pin in graph.pins.values().filter(|pin| pin.direction == PinDirection::Output) {
            if let Some(identity) = &pin.parameter_id {
                if !definitions.insert(identity) { return Err("Distinct entry parameters share the same opaque identity".into()); }
            }
        }
    }
    for graph in graphs {
        for pin in graph.pins.values().filter(|pin| pin.direction == PinDirection::Input) {
            if pin.parameter_id.as_ref().is_some_and(|identity| !definitions.contains(identity)) {
                return Err("Argument references an absent owned declaration parameter".into());
            }
        }
    }
    Ok(())
}

/// Existing source transaction calls this against its actual before/after
/// project. It does not grant source/callable permission or allocate anything.
pub(crate) fn validate_parameter_identity_edit(before: &[GraphDocument], after: &[GraphDocument]) -> Result<(), String> {
    validate_project_parameter_metadata(after)?;
    for previous in before {
        // Deletion never authorizes transferring an opaque identity elsewhere.
        for pin in previous.pins.values().filter(|pin|pin.direction==PinDirection::Output && pin.parameter_id.is_some()) {
            if after.iter().filter(|graph|graph.graph_id!=previous.graph_id).any(|graph|
                graph.pins.values().any(|other|other.direction==PinDirection::Output && other.parameter_id==pin.parameter_id)) {
                return Err("A parameter identity cannot migrate from a deleted or retained declaration".into());
            }
        }
        let Some(next) = after.iter().find(|graph| graph.graph_id == previous.graph_id) else { continue; };
        if crate::supports_unit_schema1026(previous.schema_version) && next.schema_version != previous.schema_version && next.schema_version != crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION {
            return Err("A Unit/signature edit cannot silently downgrade its feature schema".into());
        }
        for pin in previous.pins.values().filter(|pin| pin.parameter_id.is_some()) {
            if next.pins.get(&pin.id).is_some_and(|current| current.parameter_id != pin.parameter_id) {
                return Err("An existing port's opaque parameter identity is immutable".into());
            }
            if pin.direction == PinDirection::Output {
                for graph in after.iter().filter(|graph| graph.graph_id != previous.graph_id) {
                    if graph.pins.values().any(|other| other.direction == PinDirection::Output && other.parameter_id == pin.parameter_id) {
                        return Err("A parameter identity cannot migrate to a different declaration owner".into());
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "entry_parameter_identity_tests1003.rs"]
mod tests;
