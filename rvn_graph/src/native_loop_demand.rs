//! Preparation of the explicit loop contract. This analysis creates no VM,
//! activation, source authority or saved cursor and never evaluates a producer.
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Absent on all historical nodes. Selection is explicit, never inferred from
/// NativeFrameV1, a variable name or the number of connected output pins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeLoopContract {
    DemandV1,
}

/// A reviewable edit on a clone; old user VariableGet(item) nodes are retained,
/// never guessed to mean a lazy ArrayElement output.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeLoopMigration {
    pub graph: GraphDocument,
    pub owner: NodeId,
    pub added_pins: Vec<PinId>,
    pub legacy_named_reads: Vec<NodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeLoopPhase {
    ArrayLength,
    FirstIndex,
    LastIndex,
    Consumer(NodeId),
}

/// All endpoints below are actual graph objects. This is not an executable
/// instruction and must be rebound to the real source/owner by the backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeDemandWire {
    pub edge: EdgeId,
    pub output: PinId,
    pub input: PinId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeDemandOperation {
    Pure { node: NodeId, outputs: BTreeSet<PinId> },
    /// Existing Legacy inspector evaluates per terminal, unlike shared Native.
    LegacyInspectorTerminal { node: NodeId, output: PinId },
    RetainedExecutionOutput { node: NodeId, output: PinId },
    LoopIndex { owner: NodeId, output: PinId },
    ArrayElement { owner: NodeId, output: PinId, collection_input: PinId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeLoopDemandPlan {
    pub graph: GraphId,
    pub owner: NodeId,
    pub phase: NativeLoopPhase,
    pub operations: Vec<NativeDemandOperation>,
    pub wires: Vec<NativeDemandWire>,
}

impl GraphDocument {
    pub fn validate_native_loop_contracts(&self) -> Result<(), String> {
        for node in self.nodes.values() {
            if node.native_loop_contract.is_none() { continue; }
            if !matches!(self.schema_version, GRAPH_NATIVE_LOOP_DEMAND_SCHEMA_VERSION | GRAPH_NOMINAL_ENUM_SCHEMA_VERSION)
                || self.inspector_evaluation.is_legacy()
                || !matches!(node.kind, NodeKind::ForEach | NodeKind::ForLoopWithBreak) {
                return Err("Explicit native loop contract requires its schema, Native owner and actual loop node".into());
            }
            let mut expected = node_definition(node.kind).pins;
            if node.kind == NodeKind::ForEach {
                expected.extend(crate::pin_catalog::native_loop_output_definitions());
            }
            if expected.len() != node.pins.len() { return Err("Native loop port count differs from its contract".into()); }
            for (expected, actual) in expected.iter().zip(&node.pins) {
                let pin = self.pins.get(actual).ok_or("Native loop port missing")?;
                if pin.node != node.id || pin.key != expected.key || pin.direction != expected.direction
                    || pin.value_type != expected.value_type || pin.cardinality != expected.cardinality {
                    return Err("Native loop port role/order/type/direction differs from its contract".into());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn require_native_loop_backend(&self) -> Result<(), String> {
        self.validate_native_loop_contracts()?;
        for owner in self.nodes.values().filter(|n|n.native_loop_contract.is_some()){
            let old_name=self.variable_node_name(owner.id);
            if self.nodes.values().any(|node|node.kind==NodeKind::VariableGet&&self.variable_node_name(node.id).is_some_and(|name|Some(name)==old_name)){
                return Err("Explicit Native migration retains named item/index reads: connect the actual typed output before executable export; no guessed rewrite".into());
            }
        }
        if self.nodes.values().any(|node| node.native_loop_contract.is_some())
            && !matches!(self.kind,GraphKind::Init|GraphKind::Function{..}|GraphKind::Handler{..}|GraphKind::Screen{..})
            && !(matches!(self.kind,GraphKind::Label{..})&&self.narrative.is_some()) {
            return Err("Native loop demand requires an actual Init, explicit Narrative or lexical Function/Handler/Screen graph root".into());
        }
        Ok(())
    }

    /// Recalculate a header gather; a new call denotes a new phase activation.
    /// The backend, not this read-only plan, decides when that activation runs.
    pub fn plan_native_loop_header(&self, owner: NodeId, phase: NativeLoopPhase) -> Result<NativeLoopDemandPlan, String> {
        self.validate_native_loop_contracts()?;
        let node = self.nodes.get(&owner).ok_or("Loop owner absent")?;
        if node.native_loop_contract != Some(NativeLoopContract::DemandV1) { return Err("Loop has no explicit demand contract".into()); }
        let key = match (node.kind, phase) {
            (NodeKind::ForEach, NativeLoopPhase::ArrayLength) => "collection",
            (NodeKind::ForLoopWithBreak, NativeLoopPhase::FirstIndex) => "first",
            (NodeKind::ForLoopWithBreak, NativeLoopPhase::LastIndex) => "last",
            _ => return Err("Header phase differs from actual loop kind".into()),
        };
        let mut gather = Gather::new(self, owner, phase);
        gather.input(self.pin_by_key(owner, key).ok_or("Loop header input absent")?.id, 0)?;
        Ok(gather.plan)
    }

    /// Static dependency placement only. The consumer must be reached by a
    /// real body execution before its dependencies may run; planning all body
    /// consumers here does not speculate evaluation of either branch.
    pub fn plan_native_loop_consumer(&self, owner: NodeId, consumer: NodeId) -> Result<NativeLoopDemandPlan, String> {
        self.validate_native_loop_contracts()?;
        let loop_node = self.nodes.get(&owner).ok_or("Loop owner absent")?;
        if loop_node.native_loop_contract != Some(NativeLoopContract::DemandV1) { return Err("Loop has no explicit demand contract".into()); }
        let node = self.nodes.get(&consumer).ok_or("Actual consumer absent")?;
        if consumer == owner || !self.execution_body_contains(owner, consumer)? {
            return Err("Consumer has no unique demonstrated body context for this loop".into());
        }
        if !node.pins.iter().any(|id| self.pins.get(id).is_some_and(|p| p.value_type.is_execution())) {
            return Err("Demand phase requires an executed consumer, not an isolated pure preview".into());
        }
        let mut gather = Gather::new(self, owner, NativeLoopPhase::Consumer(consumer));
        for id in &node.pins {
            let pin = self.pins.get(id).ok_or("Consumer pin missing")?;
            if pin.direction == PinDirection::Input && !pin.value_type.is_execution() {
                gather.input(pin.id, 0)?;
            }
        }
        Ok(gather.plan)
    }

    fn execution_body_contains(&self, owner: NodeId, consumer: NodeId) -> Result<bool, String> {
        let body = self.pin_by_key(owner, "body").ok_or("Loop body port absent")?.id;
        let completed = self.pin_by_key(owner, "completed").ok_or("Loop completed port absent")?.id;
        // A consumer also reachable from Completed has an ambiguous activation
        // here. Keep its wire untouched; this plan cannot lend an active owner.
        Ok(self.execution_reachable(body, owner)?.contains(&consumer)
            && !self.execution_reachable(completed, owner)?.contains(&consumer))
    }

    fn execution_reachable(&self, output: PinId, stop: NodeId) -> Result<BTreeSet<NodeId>, String> {
        let mut pending = vec![output];
        let mut seen = BTreeSet::new();
        while let Some(output) = pending.pop() {
            for edge in self.edges.values().filter(|edge| edge.output == output) {
                let pin = self.pins.get(&edge.input).ok_or("Execution edge target absent")?;
                if pin.direction != PinDirection::Input || !pin.value_type.is_execution() { return Err("Malformed execution edge".into()); }
                let node = self.nodes.get(&pin.node).ok_or("Execution edge node absent")?;
                if node.id == stop || node.kind == NodeKind::BranchEnd || !seen.insert(node.id) { continue; }
                for id in &node.pins {
                    let pin = self.pins.get(id).ok_or("Execution node pin absent")?;
                    if pin.direction == PinDirection::Output && pin.value_type.is_execution() { pending.push(pin.id); }
                }
            }
        }
        Ok(seen)
    }
}

struct Gather<'a> {
    graph: &'a GraphDocument,
    plan: NativeLoopDemandPlan,
    pure: BTreeMap<NodeId, usize>,
    terminals: BTreeSet<PinId>,
    active: BTreeSet<PinId>,
}
impl<'a> Gather<'a> {
    fn new(graph: &'a GraphDocument, owner: NodeId, phase: NativeLoopPhase) -> Self {
        Self { graph, plan: NativeLoopDemandPlan { graph: graph.graph_id, owner, phase, operations: vec![], wires: vec![] },
               pure: BTreeMap::new(), terminals: BTreeSet::new(), active: BTreeSet::new() }
    }
    fn input(&mut self, input: PinId, depth: usize) -> Result<(), String> {
        let graph = self.graph;
        let pin = graph.pins.get(&input).ok_or("Demand input absent")?;
        if pin.direction != PinDirection::Input || pin.value_type.is_execution() { return Err("Demand input is not a data input".into()); }
        let edges: Vec<_> = graph.edges.values().filter(|edge| edge.input == input).cloned().collect();
        if edges.len() > 1 { return Err("Ambiguous demand source".into()); }
        if let Some(edge) = edges.first() {
            if !self.plan.wires.iter().any(|wire| wire.edge == edge.id) {
                self.plan.wires.push(NativeDemandWire { edge: edge.id, output: edge.output, input });
            }
            self.output(edge.output, depth + 1)?;
        }
        Ok(())
    }
    fn output(&mut self, output: PinId, depth: usize) -> Result<(), String> {
        // Same dependency depth as existing NativeFrame preparation; no new
        // runtime iteration/node-count budget is inferred from this analysis.
        if depth > 128 || !self.active.insert(output) { return Err("Native loop demand dependency is cyclic or exceeds128 levels".into()); }
        let result = self.output_inner(output, depth);
        self.active.remove(&output);
        result
    }
    fn output_inner(&mut self, output: PinId, depth: usize) -> Result<(), String> {
        let graph = self.graph;
        let pin = graph.pins.get(&output).ok_or("Demand output absent")?;
        if pin.direction != PinDirection::Output || pin.value_type.is_execution() { return Err("Demand source is not a data output".into()); }
        let node = graph.nodes.get(&pin.node).ok_or("Demand source owner absent")?;
        if let Some(index) = self.pure.get(&node.id).copied() {
            if let NativeDemandOperation::Pure { outputs, .. } = &mut self.plan.operations[index] { outputs.insert(output); }
            return Ok(());
        }
        if self.terminals.contains(&output) { return Ok(()); }
        if node.native_loop_contract == Some(NativeLoopContract::DemandV1) {
            if node.id != self.plan.owner || !matches!(self.plan.phase, NativeLoopPhase::Consumer(_)) {
                return Err("Loop output requires its own demonstrated body phase; nested/after-Completed placement remains unqualified".into());
            }
            match (node.kind, pin.key.as_str()) {
                (NodeKind::ForEach, "array_index") | (NodeKind::ForLoopWithBreak, "index") =>
                    self.plan.operations.push(NativeDemandOperation::LoopIndex { owner: node.id, output }),
                (NodeKind::ForEach, "array_element") => {
                    let input = graph.pin_by_key(node.id, "collection").ok_or("Actual collection input absent")?.id;
                    self.input(input, depth)?;
                    self.plan.operations.push(NativeDemandOperation::ArrayElement { owner: node.id, output, collection_input: input });
                },
                _ => return Err("Unknown native loop output role".into()),
            }
            self.terminals.insert(output);
            return Ok(());
        }
        if node.pins.iter().any(|id| graph.pins.get(id).is_some_and(|pin| pin.value_type.is_execution())) {
            // SET/Local expose the assigned value. Their RHS is not gathered.
            self.plan.operations.push(NativeDemandOperation::RetainedExecutionOutput { node: node.id, output });
            self.terminals.insert(output);
            return Ok(());
        }
        let legacy = node.kind == NodeKind::GetStringTableIdAndKey
            && crate::native_frame_preparation::inspector_contract(graph, node.id).is_legacy();
        for id in &node.pins {
            let input = graph.pins.get(id).ok_or("Pure dependency pin absent")?;
            if input.direction == PinDirection::Input && !input.value_type.is_execution() { self.input(input.id, depth)?; }
        }
        if legacy {
            self.plan.operations.push(NativeDemandOperation::LegacyInspectorTerminal { node: node.id, output });
            self.terminals.insert(output);
        } else {
            let index = self.plan.operations.len();
            self.plan.operations.push(NativeDemandOperation::Pure { node: node.id, outputs: [output].into_iter().collect() });
            self.pure.insert(node.id, index);
        }
        Ok(())
    }
}
