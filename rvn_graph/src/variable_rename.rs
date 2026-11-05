//! Independent RVN binding refactor. Initializers read the incoming environment;
//! pure producers shared across those environments need distinct consumers.
use crate::{GraphDocument, GraphEditError, GraphId, GraphKind, GraphStatementOrigin, NodeId,
    NodeKind, PinDirection, PinId, PropertyValue};
use rvn_parser::Statement;
use std::collections::{BTreeMap, BTreeSet};

const GLOBAL: u8 = 1;
const LOCAL: u8 = 2;

/// A source consumer's environment, rather than the type cached under its name.
/// Ordinary narrative fragments may enter with either binding until a project
/// caller analysis resolves their activation. This query does not choose one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableBindingContext {
    ProjectGlobal,
    ActivationLocal,
    GlobalOrLocal,
}

impl VariableBindingContext {
    fn from_bits(bits: u8) -> Self {
        match bits {
            GLOBAL => Self::ProjectGlobal,
            LOCAL => Self::ActivationLocal,
            _ => Self::GlobalOrLocal,
        }
    }
}

/// Read-only evidence for a scoped migration. Pin/node IDs belong to `graph`.
/// Missing entries are unresolved or unreachable, never a global default.
/// Repeated local introductions share an activation slot but keep all authored
/// declaration identities; a caller must not pick the first one silently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableBindingProjection {
    pub graph: GraphId,
    pub local_declarations: Vec<NodeId>,
    pub statement_contexts: BTreeMap<NodeId, VariableBindingContext>,
    pub input_contexts: BTreeMap<PinId, VariableBindingContext>,
    pub write_contexts: BTreeMap<NodeId, VariableBindingContext>,
    pub read_contexts: BTreeMap<PinId, VariableBindingContext>,
    pub narrative_call_contexts: BTreeMap<String, VariableBindingContext>,
    pub embedded_reference_contexts: BTreeMap<NodeId, VariableBindingContext>,
}

#[derive(Clone, Copy)]
enum Scope { Narrative, Calculation, Handler }

struct Uses<'a> {
    graph: &'a GraphDocument,
    name: &'a str,
    owner: NodeId,
    inputs: BTreeMap<PinId, u8>,
    execution: BTreeMap<NodeId, u8>,
    targets: BTreeMap<NodeId, u8>,
    narrative_calls: BTreeMap<String, u8>,
    prune_false_loops: bool,
}

impl Uses<'_> {
    fn error(&self, node: NodeId, message: impl Into<String>) -> GraphEditError {
        GraphEditError::InvalidReferenceExpression { node, key: "binding".into(), message: message.into() }
    }

    fn mark(&mut self, origin: &GraphStatementOrigin, state: u8, target: Option<u8>) {
        let Some(node) = origin.node.filter(|node| self.graph.nodes.contains_key(node)) else { return; };
        *self.execution.entry(node).or_default() |= state;
        if let Some(target) = target { *self.targets.entry(node).or_default() |= target; }
        for id in &self.graph.nodes[&node].pins {
            let pin = &self.graph.pins[id];
            if pin.direction == PinDirection::Input && !pin.value_type.is_execution() {
                let context = if pin.key == "name" { target.unwrap_or(state) } else { state };
                *self.inputs.entry(*id).or_default() |= context;
            }
        }
    }

    fn walk(&mut self, statements: &[Statement], origins: &[GraphStatementOrigin], mut state: u8,
        scope: Scope) -> Result<u8, GraphEditError>
    {
        if statements.len() != origins.len() {
            return Err(self.error(self.owner, "Missing statement origins for the binding refactor"));
        }
        for (statement, origin) in statements.iter().zip(origins) {
            if state == 0 { break; }
            match statement {
                Statement::LocalVar { name, .. } if name == self.name => {
                    self.mark(origin, state, Some(LOCAL));
                    state = LOCAL;
                }
                Statement::SetVar { name, .. } if name == self.name => {
                    let target = if matches!(scope, Scope::Calculation) { LOCAL } else { state };
                    self.mark(origin, state, Some(target));
                    if matches!(scope, Scope::Calculation) { state = LOCAL; }
                }
                Statement::Function { parameters, body, .. } | Statement::Screen { parameters, body, .. }
                | Statement::Handler { parameters, body, .. } | Statement::UnitFunction { parameters, body, .. } => {
                    self.mark(origin, state, None);
                    let incoming = if parameters.iter().any(|name| name == self.name) { LOCAL } else { GLOBAL };
                    let nested = if matches!(statement, Statement::Handler { .. } | Statement::UnitFunction { .. }) { Scope::Handler } else { Scope::Calculation };
                    self.walk(body, &origin.children, incoming, nested)?;
                }
                Statement::Init { body } => {
                    self.mark(origin, state, None);
                    state = self.walk(body, &origin.children, state, Scope::Narrative)?;
                }
                Statement::ScheduledConsumer { body, .. } => {
                    // Captures precede the body in this same lexical scope.
                    // Its real consumer child carries the incoming input
                    // context and introduces any Local after its initializer.
                    // The wrapper itself is not another variable write.
                    state = self.walk(body, &origin.children, state, scope)?;
                }
                Statement::If { then_branch, else_branch, .. } => {
                    self.mark(origin, state, None);
                    let split = then_branch.len();
                    if origin.children.len() != split + else_branch.len() {
                        return Err(self.error(self.owner, "Incomplete branch origins"));
                    }
                    let first = self.walk(then_branch, &origin.children[..split], state, scope)?;
                    let second = self.walk(else_branch, &origin.children[split..], state, scope)?;
                    state = first | second;
                }
                Statement::Sequence { branches } => {
                    self.mark(origin, state, None);
                    let mut offset = 0;
                    for body in branches {
                        let next = offset + body.len();
                        let children = origin.children.get(offset..next)
                            .ok_or_else(|| self.error(self.owner, "Incomplete sequence origins"))?;
                        state = self.walk(body, children, state, scope)?;
                        offset = next;
                    }
                    if offset != origin.children.len() { return Err(self.error(self.owner, "Unexpected sequence origins")); }
                }
                Statement::While { condition, body } => {
                    // A literal false guard cannot enter the body or acquire
                    // a binding from a fragment called inside it.
                    if self.prune_false_loops && matches!(condition, rvn_parser::Expr::Bool(false)) {
                        self.mark(origin, state, None);
                        continue;
                    }
                    let entry = state;
                    // Two possible bindings make this monotone fixed point finite.
                    loop {
                        self.mark(origin, state, None);
                        let next = entry | self.walk(body, &origin.children, state, scope)?;
                        if next == state { break; }
                        state = next;
                    }
                }
                Statement::ForEach { name, body, .. } | Statement::ForLoopWithBreak { name, body, .. } => {
                    let iterator = name == self.name && !matches!(scope, Scope::Narrative);
                    self.mark(origin, state, (name == self.name).then_some(if iterator { LOCAL } else { state }));
                    let entry = state;
                    let mut body_state = if iterator { LOCAL } else { state };
                    loop {
                        let next = (if iterator { LOCAL } else { entry })
                            | self.walk(body, &origin.children, body_state, scope)?;
                        if next == body_state { break; }
                        body_state = next;
                    }
                    // Ranges/collections may be empty. Their expressions are
                    // evaluated at entry, not once for each iteration.
                    state = entry | body_state;
                }
                Statement::Choice { options } => {
                    self.mark(origin, state, None);
                    let entry = state;
                    let mut joined = if options.is_empty() || options.iter().any(|option| option.condition.is_some()) { entry } else { 0 };
                    let mut offset = 0;
                    for option in options {
                        let next = offset + option.body.len();
                        let children = origin.children.get(offset..next)
                            .ok_or_else(|| self.error(self.owner, "Incomplete choice origins"))?;
                        joined |= self.walk(&option.body, children, entry, scope)?;
                        offset = next;
                    }
                    if offset != origin.children.len() { return Err(self.error(self.owner, "Unexpected choice origins")); }
                    state = joined;
                }
                Statement::Imagemap { hotspots, .. } => {
                    self.mark(origin, state, None);
                    let entry = state;
                    let mut joined = entry;
                    let mut offset = 0;
                    for hotspot in hotspots {
                        let next = offset + hotspot.body.len();
                        let children = origin.children.get(offset..next)
                            .ok_or_else(|| self.error(self.owner, "Incomplete hotspot origins"))?;
                        joined |= self.walk(&hotspot.body, children, entry, scope)?;
                        offset = next;
                    }
                    if offset != origin.children.len() { return Err(self.error(self.owner, "Unexpected hotspot origins")); }
                    state = joined;
                }
                Statement::Call { target } => {
                    self.mark(origin, state, None);
                    if matches!(scope, Scope::Narrative) {
                        *self.narrative_calls.entry(target.clone()).or_default() |= state;
                    }
                    // A fragment may introduce this slot. Project-wide incoming
                    // binding analysis is required to disambiguate that case.
                    if state & GLOBAL != 0 { state |= LOCAL; }
                }
                Statement::Jump { target } => {
                    self.mark(origin, state, None);
                    if matches!(scope, Scope::Narrative) {
                        *self.narrative_calls.entry(target.clone()).or_default() |= state;
                    }
                    state = 0;
                }
                Statement::Return | Statement::FunctionReturn { .. } | Statement::UnitReturn => {
                    self.mark(origin, state, None);
                    state = 0;
                }
                _ => self.mark(origin, state, None),
            }
        }
        Ok(state)
    }
}

// Preserve real statement origins while supplying the existing Unit compiler
// with the actual project. This never imports bodies or allocates parameters.
fn compile_local_binding(graph: &GraphDocument, project: Option<&[GraphDocument]>)
    -> Result<crate::TranspiledScriptWithOrigins, crate::TranspileError>
{
    match project {
        Some(project) if graph.inspector_evaluation.is_legacy() || crate::uses_unit_context1026(graph) => {
            graph.require_native_loop_backend().map_err(crate::TranspileError::GeneratedSourceInvalid)?;
            crate::codegen::transpile_unit_in_project(graph, project)
        }
        _ => crate::transpile_with_origins(graph),
    }
}

impl GraphDocument {
    /// Rename using the exact current project owner and its real Unit ports.
    /// The caller passes its unmodified project snapshot; a detached/stale
    /// graph or an invalid Unit signature is refused before any edit is made.
    /// The context-free API deliberately retains its existing refusal.
    pub fn rename_variable_in_project(&mut self, old: &str, new: impl Into<String>,
        project: &[GraphDocument]) -> Result<(), GraphEditError>
    {
        let error = |message: String| GraphEditError::InvalidReferenceExpression {
            node: NodeId(0), key: "binding".into(), message,
        };
        let index = self.binding_project_owner_index(project).map_err(error)?;
        let mut next = self.clone();
        let new = new.into();
        if next.variable_scope(old) == Some(crate::VariableScope::Local)
            && next.nodes.values().any(|node| node.kind == NodeKind::LocalVariable
                && next.variable_node_name(node.id).as_deref() == Some(old))
        {
            rename_local_in_project(&mut next, old, new, Some(project))?;
        } else {
            next.rename_variable(old, new)?;
        }
        // Recheck the edited snapshot through the same real ownership and
        // strict compiler gates before replacing the caller's graph.
        let mut edited = project.to_vec();
        edited[index] = next.clone();
        crate::unit_graph_source1003::validate_project(&edited).map_err(error)?;
        compile_local_binding(&next, Some(&edited)).map_err(|e| error(e.to_string()))?;
        *self = next;
        Ok(())
    }

    /// Project the existing RVN binding flow without changing definitions,
    /// producers, execution, inferred caches or source. An initializer's RHS
    /// uses the incoming environment; the declaration target uses the new one.
    /// Branches, loops and unknown fragment effects remain explicit unions.
    /// Retained incompatible links do not prevent analysis; they still fail
    /// strict compilation and publication. All structural errors are refused.
    pub fn variable_binding_projection(&self, name: &str)
        -> Result<VariableBindingProjection, GraphEditError>
    {
        self.variable_binding_projection_context(name, None, None)
    }

    // Only actual source/project import callers supply this context. No Unit
    // declaration or parameter identity is inferred from the reference name.
    pub(crate) fn variable_binding_projection_in_project(&self, name: &str, project: &[GraphDocument])
        -> Result<VariableBindingProjection, GraphEditError>
    {
        self.variable_binding_projection_context(name, Some(project), None)
    }

    pub(crate) fn variable_binding_projection_library1027(&self,name:&str,project:&[GraphDocument],resources:&crate::LibraryGraphResources1027<'_>)->Result<VariableBindingProjection,GraphEditError>{
        self.variable_binding_projection_context(name,Some(project),Some(resources))
    }

    fn variable_binding_projection_context(&self, name: &str, project: Option<&[GraphDocument]>,library:Option<&crate::LibraryGraphResources1027<'_>>)
        -> Result<VariableBindingProjection, GraphEditError>
    {
        let owner = self.nodes.values().find(|node|
            self.variable_node_name(node.id).as_deref() == Some(name))
            .map(|node| node.id).unwrap_or(NodeId(0));
        let compiled = if let Some(resources)=library {
            crate::codegen::transpile_library_binding_analysis1027(self,project.ok_or_else(||GraphEditError::InvalidReferenceExpression{node:owner,key:"binding".into(),message:"Original scoped binding project is absent".into()})?,resources)
        } else {match project {
            Some(project) => crate::codegen::transpile_for_binding_analysis_in_project(self, project),
            None => crate::codegen::transpile_for_binding_analysis(self),
        }}.map_err(|error|
            GraphEditError::InvalidReferenceExpression {
                node: owner, key: "binding".into(), message: error.to_string(),
            })?;
        let entry = if matches!(self.kind, GraphKind::Label { .. })
            && self.narrative != Some(crate::NarrativeMetadata::SceneEntry)
            { GLOBAL | LOCAL } else { GLOBAL };
        let mut uses = Uses { graph: self, name, owner, inputs: BTreeMap::new(),
            execution: BTreeMap::new(), targets: BTreeMap::new(),
            narrative_calls: BTreeMap::new(), prune_false_loops: false };
        uses.walk(&compiled.ast, &compiled.origins, entry, Scope::Narrative)?;
        let mut contexts = BTreeMap::new();
        for (input, state) in &uses.inputs {
            for edge in self.edges.values().filter(|edge| edge.input == *input) {
                propagate(self, edge.output, *state, &mut contexts);
            }
        }
        // Reuse the parser-backed reference rewrite as a read-only probe. A
        // Text/Choice property is a reference only if its parsed expression
        // changes, never because its displayed string contains a name.
        let mut embedded_reference_contexts = BTreeMap::new();
        if self.variables.contains_key(name) {
            let mut probe = "RvNTypeReferenceProbe".to_owned();
            while self.variables.contains_key(&probe) { probe.push('_'); }
            let mut renamed = self.clone();
            renamed.rename_variable_references(name, probe)?;
            for node in self.nodes.values().filter(|node|
                matches!(node.kind, NodeKind::TextValue | NodeKind::Choice)) {
                if node.properties != renamed.nodes[&node.id].properties {
                    let bits = contexts.get(&node.id).copied().unwrap_or(0)
                        | uses.execution.get(&node.id).copied().unwrap_or(0);
                    if bits != 0 { embedded_reference_contexts.insert(node.id,
                        VariableBindingContext::from_bits(bits)); }
                }
            }
        }
        let mut read_contexts = BTreeMap::new();
        for (node, state) in contexts {
            if self.variable_node_name(node).as_deref() != Some(name) { continue; }
            for id in &self.nodes[&node].pins {
                let pin = &self.pins[id];
                if pin.direction == PinDirection::Output && !pin.value_type.is_execution() {
                    read_contexts.insert(*id, VariableBindingContext::from_bits(state));
                }
            }
        }
        let local_declarations = self.nodes.values().filter(|node|
            crate::widget_factory_graph_1001::is_local_assignment(node)
                && self.variable_node_name(node.id).as_deref() == Some(name)
                && uses.targets.contains_key(&node.id))
            .map(|node| node.id).collect();
        let convert = VariableBindingContext::from_bits;
        Ok(VariableBindingProjection {
            graph: self.graph_id, local_declarations, read_contexts,
            statement_contexts: uses.execution.into_iter().map(|(id, bits)| (id, convert(bits))).collect(),
            input_contexts: uses.inputs.into_iter().map(|(id, bits)| (id, convert(bits))).collect(),
            write_contexts: uses.targets.into_iter().map(|(id, bits)| (id, convert(bits))).collect(),
            narrative_call_contexts: uses.narrative_calls.into_iter().map(|(name, bits)| (name, convert(bits))).collect(),
            embedded_reference_contexts,
        })
    }
}

/// `None` preserves the conservative dependency check when additional label
/// entries or fallthrough require a project-wide control-flow analysis.
pub(super) fn narrative_calls_with_local_binding(
    graph: &GraphDocument, name: &str, project: &[GraphDocument],
) -> Result<Option<BTreeMap<String, bool>>, GraphEditError> {
    let owner = graph.nodes.values().find(|node| crate::widget_factory_graph_1001::is_local_assignment(node)
        && graph.variable_node_name(node.id).as_deref() == Some(name))
        .ok_or_else(|| GraphEditError::InvalidReferenceExpression {
            node: NodeId(0), key: "binding".into(), message: "Missing local declaration owner".into(),
        })?.id;
    let compiled = compile_local_binding(graph, Some(project)).map_err(|error| GraphEditError::InvalidReferenceExpression {
        node: owner, key: "binding".into(), message: error.to_string(),
    })?;
    if compiled.ast.iter().filter(|statement| matches!(statement,
        Statement::Label { .. } | Statement::SceneLabel { .. })).count() != 1 {
        return Ok(None);
    }
    let entry = if graph.narrative == Some(crate::NarrativeMetadata::SceneEntry) { GLOBAL } else { GLOBAL | LOCAL };
    let mut uses = Uses { graph, name, owner, inputs: BTreeMap::new(), execution: BTreeMap::new(),
        targets: BTreeMap::new(), narrative_calls: BTreeMap::new(), prune_false_loops: true };
    if uses.walk(&compiled.ast, &compiled.origins, entry, Scope::Narrative)? != 0 {
        return Ok(None);
    }
    Ok(Some(uses.narrative_calls.into_iter()
        .map(|(target, state)| (target, state & LOCAL != 0)).collect()))
}

fn executable(graph: &GraphDocument, node: NodeId) -> bool {
    graph.nodes[&node].pins.iter().any(|id| graph.pins[id].value_type.is_execution())
}

fn incoming(graph: &GraphDocument, node: NodeId) -> Vec<(PinId, PinId)> {
    graph.edges.values().filter(|edge| graph.pins[&edge.input].node == node
        && !graph.pins[&edge.input].value_type.is_execution())
        .map(|edge| (edge.input, edge.output)).collect()
}

fn propagate(graph: &GraphDocument, output: PinId, state: u8, nodes: &mut BTreeMap<NodeId, u8>) {
    let node = graph.pins[&output].node;
    let previous = nodes.get(&node).copied().unwrap_or(0);
    if previous | state == previous { return; }
    nodes.insert(node, previous | state);
    // Assignment outputs read the live slot. Following their initializer would
    // falsely evaluate it in the later consumer's environment.
    if !executable(graph, node) {
        for (_, source) in incoming(graph, node) { propagate(graph, source, state, nodes); }
    }
}

struct Producers<'a> {
    before: &'a GraphDocument,
    projected: &'a GraphDocument,
    next: GraphDocument,
    name: &'a str,
    new_name: &'a str,
    contexts: BTreeMap<NodeId, u8>,
    changed: BTreeMap<PinId, bool>,
    copies: BTreeMap<NodeId, BTreeMap<PinId, PinId>>,
    visited: BTreeSet<NodeId>,
}

impl Producers<'_> {
    fn depends(&mut self, output: PinId) -> bool {
        if let Some(result) = self.changed.get(&output) { return *result; }
        let node = self.before.pins[&output].node;
        let result = if executable(self.before, node) {
            self.before.variable_node_name(node).as_deref() == Some(self.name)
        } else {
            self.before.nodes[&node].properties != self.projected.nodes[&node].properties
                || incoming(self.before, node).into_iter().any(|(_, source)| self.depends(source))
        };
        self.changed.insert(output, result);
        result
    }

    fn local(&mut self, output: PinId) -> Result<PinId, GraphEditError> {
        if !self.depends(output) { return Ok(output); }
        let node = self.before.pins[&output].node;
        if executable(self.before, node) {
            if self.next.variable_node_name(node).as_deref() == Some(self.new_name) { return Ok(output); }
            // Keep the executable global assignment; provide a genuine local
            // read for a consumer after the local declaration instead.
            if let Some(mapping) = self.copies.get(&node) { return Ok(mapping[&output]); }
            let old = &self.before.nodes[&node];
            let get = self.next.add_catalog_node(NodeKind::VariableGet, [old.position[0]+32.0, old.position[1]+32.0])?;
            self.next.set_property(get, "name", PropertyValue::String(self.new_name.into()))?;
            let pin = self.next.pin_by_key(get, "value").unwrap().id;
            self.copies.insert(node, BTreeMap::from([(output, pin)]));
            return Ok(pin);
        }
        if let Some(mapping) = self.copies.get(&node) { return Ok(mapping[&output]); }
        let shared = self.contexts.get(&node).copied().unwrap_or(LOCAL) & GLOBAL != 0;
        let target = if shared {
            let old = &self.projected.nodes[&node];
            let target = self.next.add_node(old.kind, [old.position[0]+32.0, old.position[1]+32.0]);
            self.next.nodes.get_mut(&target).unwrap().properties = old.properties.clone();
            self.next.nodes.get_mut(&target).unwrap().title_override = old.title_override.clone();
            self.next.nodes.get_mut(&target).unwrap().annotation = old.annotation.clone();
            let mut mapping = BTreeMap::new();
            for id in &old.pins {
                let pin = &self.projected.pins[id];
                let new = self.next.add_pin(target, &pin.key, &pin.label, pin.direction, pin.value_type.clone(), pin.cardinality)?;
                let mut copy = pin.clone();copy.id = new;copy.node = target;
                self.next.pins.insert(new, copy);mapping.insert(*id, new);
            }
            self.copies.insert(node, mapping);
            target
        } else { node };
        if self.visited.insert(node) {
            for (input, source) in incoming(self.before, node) {
                let renamed = self.local(source)?;
                if shared {
                    let input = self.copies[&node][&input];
                    self.next.connect(renamed, input)?;
                } else if let Some(edge) = self.next.edges.values_mut().find(|edge| edge.input == input) {
                    edge.output = renamed;
                }
            }
        }
        Ok(if target == node { output } else { self.copies[&node][&output] })
    }
}

pub(super) fn rename_local(graph: &mut GraphDocument, name: &str, new_name: String) -> Result<(), GraphEditError> {
    rename_local_in_project(graph, name, new_name, None)
}

fn rename_local_in_project(graph: &mut GraphDocument, name: &str, new_name: String,
    project: Option<&[GraphDocument]>) -> Result<(), GraphEditError> {
    // The existing complete expression/name preflight remains the first gate.
    let mut projected = graph.clone();projected.rename_variable_references(name, &new_name)?;
    if name == new_name { return Ok(()); }
    let owner = graph.nodes.values().find(|node| crate::widget_factory_graph_1001::is_local_assignment(node)
        && graph.variable_node_name(node.id).as_deref() == Some(name)).unwrap().id;
    let compiled = compile_local_binding(graph, project).map_err(|error| GraphEditError::InvalidReferenceExpression {
        node: owner, key: "binding".into(), message: error.to_string(),
    })?;
    let entry = if matches!(graph.kind, GraphKind::Label { .. })
        && graph.narrative != Some(crate::NarrativeMetadata::SceneEntry) { GLOBAL | LOCAL } else { GLOBAL };
    let mut uses = Uses { graph, name, owner, inputs: BTreeMap::new(), execution: BTreeMap::new(),
        targets: BTreeMap::new(), narrative_calls: BTreeMap::new(), prune_false_loops: false };
    uses.walk(&compiled.ast, &compiled.origins, entry, Scope::Narrative)?;
    let mut contexts = BTreeMap::new();
    for (input, state) in &uses.inputs {
        if let Some(edge) = graph.edges.values().find(|edge| edge.input == *input) { propagate(graph, edge.output, *state, &mut contexts); }
    }
    let mut next = projected.clone();
    let mut retain_global = false;
    for (node, state) in &uses.execution {
        let target = uses.targets.get(node).copied().unwrap_or(*state);
        for (key, value) in &graph.nodes[node].properties {
            if projected.nodes[node].properties.get(key) == Some(value) { continue; }
            let context = if key == "name" { target } else { *state };
            if context == GLOBAL | LOCAL { return Err(uses.error(*node, "This reference can resolve to distinct bindings; a project binding refactor is required before renaming")); }
            if context == GLOBAL {
                next.nodes.get_mut(node).unwrap().properties.insert(key.clone(), value.clone());
                retain_global = true;
            }
        }
        for id in &graph.nodes[node].pins {
            let context = if graph.pins[id].key == "name" { target } else { *state };
            if context == GLOBAL { next.pins.insert(*id, graph.pins[id].clone()); }
        }
    }
    for (node, state) in &contexts {
        if *state & GLOBAL != 0 && !executable(graph, *node) {
            if graph.nodes[node].properties != projected.nodes[node].properties { retain_global = true; }
            next.nodes.insert(*node, graph.nodes[node].clone());
            for id in &graph.nodes[node].pins { next.pins.insert(*id, graph.pins[id].clone()); }
        }
    }
    if retain_global { next.variables.insert(name.into(), graph.variables[name].clone()); }
    let mut producers = Producers { before: graph, projected: &projected, next, name, new_name: &new_name,
        contexts, changed: BTreeMap::new(), copies: BTreeMap::new(), visited: BTreeSet::new() };
    for (input, state) in &uses.inputs {
        let Some(output) = graph.edges.values().find(|edge| edge.input == *input).map(|edge| edge.output) else { continue; };
        if *state == GLOBAL | LOCAL && producers.depends(output) {
            return Err(uses.error(graph.pins[input].node, "This data input receives distinct bindings; a project binding refactor is required before renaming"));
        }
        if *state == LOCAL {
            let output = producers.local(output)?;
            producers.next.edges.values_mut().find(|edge| edge.input == *input).unwrap().output = output;
        }
    }
    *graph = producers.next;
    Ok(())
}
