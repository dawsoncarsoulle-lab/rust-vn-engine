use crate::{
    node_definition, AssetKind, EdgeId, GraphDiagnostic, GraphId, GraphKind, NodeId, NodeKind,
    PinCardinality, PinDirection, PinId, PropertyValue, ValueType, VariableDefinition,
    GRAPH_SCHEMA_VERSION, GRAPH_MAX_SUPPORTED_SCHEMA_VERSION,
};

fn asset_node_kind(kind: AssetKind) -> NodeKind {
    match kind {
        AssetKind::StringTable => NodeKind::StringTableAsset,
        AssetKind::Background => NodeKind::SceneAsset,
        AssetKind::Sprite => NodeKind::SpriteAsset,
        AssetKind::Music => NodeKind::MusicAsset,
        AssetKind::SoundEffect => NodeKind::SoundEffectAsset,
        AssetKind::Voice => NodeKind::VoiceAsset,
        AssetKind::Cinematic => NodeKind::CinematicAsset,
        AssetKind::HoverImage => NodeKind::HoverImageAsset,
        AssetKind::Script => NodeKind::ScriptAsset,
    }
}

/// Asset object selections are authored pin state, independently of the node
/// consuming them. A cable overrides the selection; it does not erase it.
/// Keep the legacy policy for other input domains and their source migrations.
pub(crate) fn retains_inline_pin_default(kind: NodeKind, value_type: &ValueType) -> bool {
    matches!(value_type, ValueType::Asset(_)) || retains_inline_pin_defaults(kind)
        || scalar_consumer_slot(kind).is_some_and(|(_, declared)| &declared == value_type)
}

/// Declared scalar consumer slots keep authoring fallback state behind wires.
/// This exact table is shared by editing/cleanup and Native reconstruction;
/// it grants no extra type, runtime consumer, source or service capability.
pub(crate) fn scalar_consumer_slot(kind: NodeKind) -> Option<(&'static str, ValueType)> {
    Some(match kind {
        NodeKind::TypewriterSet => ("enabled", ValueType::Bool),
        NodeKind::TypewriterSpeed => ("speed", ValueType::Int),
        NodeKind::MusicVolume => ("level", ValueType::Float),
        _ => return None,
    })
}

/// Allocator reservations are editor identity history, not executable data.
/// The source already reconstructed its objects; retain only monotone numeric
/// frontiers from the same real Native scope, never any deleted object or ID.
pub(crate) fn restore_native_allocator_reservations(current:&mut GraphDocument,previous:&GraphDocument)->Result<(),String>{
    if current.inspector_evaluation.is_legacy() || previous.inspector_evaluation.is_legacy(){return Ok(());}
    if current.graph_id!=previous.graph_id
        || std::mem::discriminant(&current.kind)!=std::mem::discriminant(&previous.kind){
        return Err("Native allocator presentation requires the same source-owned scope".into());
    }
    for graph in [&*current,previous]{
        graph.validate_inspector_evaluation_contract()?;
        // Validate each actual root independently. Current source may legally
        // upgrade ExecLinks metadata; numeric reservations cannot veto that
        // source capability or substitute the previous root/ports for it.
        crate::native_frame_preparation::native_scope(graph).map_err(|error|error.to_string())?;
        if graph.next_node_id==0 || graph.next_pin_id==0 || graph.next_edge_id==0
            || graph.nodes.keys().any(|id|id.get()>=graph.next_node_id)
            || graph.pins.keys().any(|id|id.get()>=graph.next_pin_id)
            || graph.edges.keys().any(|id|id.get()>=graph.next_edge_id){
            return Err("Invalid Native allocator presentation reservation".into());
        }
    }
    current.next_node_id=current.next_node_id.max(previous.next_node_id);
    current.next_pin_id=current.next_pin_id.max(previous.next_pin_id);
    current.next_edge_id=current.next_edge_id.max(previous.next_edge_id);
    Ok(())
}

/// Native source owns topology and explicit operands. Existing presentation may
/// restore a hidden Asset fallback only to the same compatible declared slot.
/// A removed slot is retired; rewiring a surviving slot does not erase it.
pub(crate) fn restore_connected_asset_presentation(current: &mut GraphDocument, previous: &GraphDocument) {
    if current.inspector_evaluation.is_legacy() || current.graph_id != previous.graph_id
        || std::mem::discriminant(&current.kind) != std::mem::discriminant(&previous.kind) { return; }
    let mut retained = Vec::new();
    for pin in current.pins.values().filter(|pin| pin.direction == PinDirection::Input
        && matches!(&pin.value_type, ValueType::Asset(_))) {
        let Some(prior) = previous.pins.get(&pin.id) else { continue; };
        if prior.default_value.is_none() || pin.node != prior.node || pin.key != prior.key
            || pin.direction != prior.direction || pin.value_type != prior.value_type
            || pin.cardinality != prior.cardinality { continue; }
        let (Some(owner), Some(old_owner)) = (current.nodes.get(&pin.node), previous.nodes.get(&prior.node)) else { continue; };
        if owner.kind != old_owner.kind
            || !current.edges.values().any(|edge| edge.input == pin.id)
            || !previous.edges.values().any(|edge| edge.input == prior.id) { continue; }
        retained.push((pin.id, prior.default_value.clone(), prior.text_localization.clone(), prior.text_table_reference.clone()));
    }
    for (id, value, text, table) in retained {
        let pin = current.pins.get_mut(&id).unwrap();
        pin.default_value = value; pin.text_localization = text; pin.text_table_reference = table;
    }
}

pub(crate) fn retains_inline_pin_defaults(kind: NodeKind) -> bool {
    crate::standard_macros_1020::kind(kind).is_some() || kind.is_string_table_query() || matches!(
        kind,
        NodeKind::StringTableId | NodeKind::TextFromStringTable | NodeKind::SetVariable
            | NodeKind::LocalVariable
            | NodeKind::LogicAnd
            | NodeKind::LogicOr
            | NodeKind::LogicNot
            | NodeKind::If
            | NodeKind::While
            | NodeKind::Choice
            | NodeKind::ConvertBoolToInt
            | NodeKind::ConvertBoolToFloat
            | NodeKind::ConvertIntToBool
            | NodeKind::ConvertIntToFloat
            | NodeKind::ConvertNumberToText
            | NodeKind::ConvertTextToInt
            | NodeKind::ConvertStringToText
            | NodeKind::ConvertTextToString
            | NodeKind::ConvertStringToName
            | NodeKind::ConvertNameToString
            | NodeKind::MathAdd
            | NodeKind::MathSubtract
            | NodeKind::MathMultiply
            | NodeKind::MathDivide
            | NodeKind::MathNegate
            | NodeKind::MathEqual
            | NodeKind::MathNotEqual
            | NodeKind::StringAppend
            | NodeKind::MathLess
            | NodeKind::MathLessEqual
            | NodeKind::MathGreater
            | NodeKind::MathGreaterEqual
            | NodeKind::BinaryOperator
            | NodeKind::UnaryOperator
    )
}

#[path = "native_macro_presentation1021.rs"]
mod native_macro_presentation1021;
pub(crate) use native_macro_presentation1021::{native_authoring_execution_projection, restore_isolated_macro_presentation};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

fn rename_variable_expression(expr: &mut rvn_parser::Expr, old: &str, new: &str) {
    use rvn_parser::Expr;
    match expr {
        Expr::Var(name) if name == old => *name = new.into(),
        Expr::Call { args, .. } | Expr::ListLit(args) => {
            for argument in args { rename_variable_expression(argument, old, new); }
        }
        Expr::BinOp { left, right, .. } | Expr::Index { target: left, index: right }
            | Expr::And(left, right) | Expr::Or(left, right) => {
            rename_variable_expression(left, old, new);
            rename_variable_expression(right, old, new);
        }
        Expr::Neg(value) | Expr::Not(value) => rename_variable_expression(value, old, new),
        _ => {}
    }
}

fn rename_variable_text(text: &str, old: &str, new: &str) -> Result<String, String> {
    // Graph payloads have already lost the outer RVN string escaping. Restore
    // that layer before using the parser; backslashes inside quoted names must
    // not be decoded a second time.
    let quoted = serde_json::to_string(text).map_err(|error| error.to_string())?;
    let before = rvn_parser::parse_interpolated_str(&quoted[1..quoted.len() - 1])
        .map_err(|error| error.to_string())?;
    let mut after = before.clone();
    for segment in &mut after.0 {
        if let rvn_parser::TextSegment::Interp(expression) = segment {
            rename_variable_expression(expression, old, new);
        }
    }
    Ok(if before == after { text.into() } else { crate::import::text_source(&after) })
}

fn rename_variable_condition(text: &str, old: &str, new: &str) -> Result<String, String> {
    if text.trim().is_empty() { return Ok(text.into()); }
    let parsed = rvn_parser::parse(&format!("set value = {text}"))
        .map_err(|error| error.to_string())?;
    let [rvn_parser::Statement::SetVar { value, .. }] = parsed.as_slice()
        else { return Err("Une expression RVN unique est attendue.".into()); };
    let mut next = value.clone();
    rename_variable_expression(&mut next, old, new);
    Ok(if next == *value { text.into() } else { crate::import::expression_source(&next) })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub position: [f64; 2],
    pub title_override: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotation: Option<NodeAnnotation>,
    /// Hidden from property inspectors; None is admitted only for historical nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub string_table_contract: Option<crate::StringTableQueryContract>,
    /// Per-node evaluation semantics; absent historical markers remain Legacy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspector_contract: Option<crate::InspectorEvaluationContract>,
    /// Explicit opt-in only; old snapshots retain absent markers and exact JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_loop_contract: Option<crate::NativeLoopContract>,
    /// Explicit instance semantics; absent markers cannot admit new macro nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard_macro_contract: Option<crate::StandardMacroContract>,
    pub properties: BTreeMap<String, PropertyValue>,
    pub pins: Vec<PinId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphPin {
    pub id: PinId,
    pub node: NodeId,
    pub key: String,
    pub label: String,
    pub direction: PinDirection,
    pub value_type: ValueType,
    pub cardinality: PinCardinality,
    pub default_value: Option<PropertyValue>,
    /// Schema14 only: definition on an entry output, reference on a call input.
    /// Immutable on a retained port; allocation belongs to its source transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_id: Option<crate::ParameterId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_localization: Option<rvn_parser::localized_text::TextLocalization>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_table_reference: Option<rvn_parser::localized_text::StringTableReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: EdgeId,
    pub output: PinId,
    pub input: PinId,
}

/// One undoable connection edit, including any visible conversion nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedConnection {
    pub edge: EdgeId,
    pub conversions: Vec<NodeId>,
}

/// Narrative semantics are independent of the existing Label node and graph.
/// A fragment shares its caller's activation; only SceneEntry starts a visit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NarrativeMetadata {
    SceneEntry,
    SceneFragment,
}

/// Editing defaults for nodes created later, never executable source/Frame authority.
/// Missing historical metadata retains the historical creation policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringDefaults {
    #[serde(deserialize_with = "deserialize_authoring_defaults_version")]
    version: u32,
    pub loop_contract: crate::NativeLoopContract,
}

fn deserialize_authoring_defaults_version<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let version = u32::deserialize(deserializer)?;
    if version != 1 {
        return Err(serde::de::Error::custom(format!("Unsupported graph authoring defaults version {version}")));
    }
    Ok(version)
}

impl AuthoringDefaults {
    /// Factories select this only for an explicitly new document.
    pub fn native_loop_demand() -> Self {
        Self { version: 1, loop_contract: crate::NativeLoopContract::DemandV1 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphDocument {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library1027: Option<crate::LibraryGraphMetadata1027>,
    pub graph_id: GraphId,
    #[serde(default, skip_serializing_if = "crate::InspectorEvaluationContract::is_legacy")]
    pub inspector_evaluation: crate::InspectorEvaluationContract,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authoring_defaults: Option<AuthoringDefaults>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_owner: Option<crate::TextOwnerId>,
    pub kind: GraphKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narrative: Option<NarrativeMetadata>,
    pub nodes: BTreeMap<NodeId, GraphNode>,
    pub pins: BTreeMap<PinId, GraphPin>,
    pub edges: BTreeMap<EdgeId, GraphEdge>,
    #[serde(default)]
    pub variables: BTreeMap<String, VariableDefinition>,
    /// Characters declared through the editor: identifier â†’ display name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub characters: BTreeMap<String, String>,
    next_node_id: u64,
    next_pin_id: u64,
    next_edge_id: u64,
}

impl GraphDocument {
    /// Creation policy only. Existing nodes and the executable contract are
    /// unaffected; the real fresh node still needs normal preparation.
    pub fn new_loops_use_demand_contract(&self) -> bool {
        !self.inspector_evaluation.is_legacy()
            || self.authoring_defaults.is_some_and(|defaults|
                defaults.loop_contract == crate::NativeLoopContract::DemandV1)
    }
    /// PRIVATE: restore only identities explicitly supplied by authoritative
    /// versioned source. No position/order/semantic-signature guessing.
    pub(crate) fn restore_native_source_identities(&mut self, identities:&BTreeMap<NodeId,crate::native_frame_preparation::NativeNodeIdentity>)->Result<(),String> {
        if identities.len()!=self.nodes.len() {
            let undeclared=self.nodes.iter().filter(|(id,_)|!identities.contains_key(*id))
                .map(|(id,node)|format!("{id}:{:?}",node.kind)).take(16).collect::<Vec<_>>();
            let absent=identities.keys().filter(|id|!self.nodes.contains_key(*id)).take(16).collect::<Vec<_>>();
            return Err(format!("Native source does not identify every reconstructed node; provisional shape refused; actual={} declared={} undeclared={undeclared:?} absent={absent:?}",self.nodes.len(),identities.len()));
        }
        let mut nodes=BTreeMap::new();let mut pins=BTreeMap::new();let mut edges=BTreeMap::new();
        let mut claimed_nodes=BTreeSet::new();let mut claimed_pins=BTreeSet::new();let mut claimed_edges=BTreeSet::new();
        let mut exec_pending=Vec::new();
        for (old,identity) in identities {
            let node=self.nodes.get(old).ok_or("Native identity references absent reconstructed node")?;
            if node.kind!=identity.kind||node.pins.len()!=identity.ports.len()||!claimed_nodes.insert(identity.node) {return Err("Native node kind/count/collision mismatch".into());}
            nodes.insert(*old,identity.node);
            for ((key,id,incoming),actual) in identity.ports.iter().zip(&node.pins) {
                let pin=&self.pins[actual];
                if pin.key!=*key||!claimed_pins.insert(*id) {return Err("Native port role/order/PinId collision".into());}
                pins.insert(*actual,*id);
                let linked:Vec<_>=self.edges.values().filter(|edge|edge.input==*actual).collect();
                if let Some(links)=identity.exec_inputs.get(key){
                    if !identity.exec_links_v1||pin.direction!=PinDirection::Input||pin.value_type!=ValueType::Execution||incoming.is_some()||linked.len()!=links.len(){
                        return Err("ExecLinksV1 actual input direction/count differs from full declaration".into());
                    }
                    exec_pending.push((*actual,links));continue;
                }
                match (incoming,linked.as_slice()) {
                    (None,[])=>{},
                    (Some(id),[edge]) if claimed_edges.insert(*id)=>{edges.insert(edge.id,*id);},
                    _=>return Err("Native incoming EdgeId connection/collision mismatch".into()),
                }
            }
        }
        // All actual output pins have now been mapped. Resolve by both real
        // endpoints, never provisional-ID order or first incoming edge.
        for (input,links) in exec_pending {
            for link in links {
                let matching=self.edges.values().filter(|edge|edge.input==input&&pins.get(&edge.output).is_some_and(|pin|pin.get()==link.output_pin)).collect::<Vec<_>>();
                let [edge]=matching.as_slice()else{return Err("ExecLinksV1 source output does not match one actual reconstructed execution wire".into());};
                let id=EdgeId::new(link.edge);
                if !claimed_edges.insert(id)||edges.insert(edge.id,id).is_some(){return Err("ExecLinksV1 incoming EdgeId/endpoint collision".into());}
            }
        }
        if pins.len()!=self.pins.len()||edges.len()!=self.edges.len() {return Err("Native source omitted pin/edge identities".into());}
        for (old,identity) in identities {
            for (key,expected) in identity.static_inputs.iter().chain(identity.dynamic_inputs.iter()) {
                let input=self.pin_by_key(*old,key).ok_or("Static audio input role missing")?;
                let linked:Vec<_>=self.edges.values().filter(|edge|edge.input==input.id).collect();
                if !matches!(linked.as_slice(),[edge] if pins.get(&edge.output)==Some(expected)) {
                    return Err("Static audio input source identity/wiring mismatch".into());
                }
            }
        }
        self.nodes=std::mem::take(&mut self.nodes).into_values().map(|mut node|{
            if node.kind==NodeKind::BranchEnd {
                if let Some(PropertyValue::Int(owner))=node.properties.get_mut("owner") {*owner=nodes[&NodeId::new(*owner as u64)].get() as i64;}
            }
            node.id=nodes[&node.id];node.pins=node.pins.into_iter().map(|id|pins[&id]).collect();(node.id,node)
        }).collect();
        self.pins=std::mem::take(&mut self.pins).into_values().map(|mut pin|{
            pin.id=pins[&pin.id];pin.node=nodes[&pin.node];(pin.id,pin)
        }).collect();
        self.edges=std::mem::take(&mut self.edges).into_values().map(|mut edge|{
            edge.id=edges[&edge.id];edge.output=pins[&edge.output];edge.input=pins[&edge.input];(edge.id,edge)
        }).collect();
        // A source constructor has no authored hidden fallback. Connection
        // editing retains one, but a freshly allocated catalogue default may
        // not masquerade as source data. Clear only real wired scalar slots;
        // exact previous authoring is restored separately after reconstruction.
        let connected: BTreeSet<_> = self.edges.values().map(|edge| edge.input).collect();
        for pin in self.pins.values_mut() {
            if connected.contains(&pin.id) && pin.direction == PinDirection::Input
                && self.nodes.get(&pin.node).and_then(|node| scalar_consumer_slot(node.kind))
                    .is_some_and(|(key, value_type)| pin.key == key && pin.value_type == value_type) {
                pin.default_value = None;
                pin.text_localization = None;
                pin.text_table_reference = None;
            }
        }
        self.next_node_id=self.nodes.keys().map(|id|id.get()).max().unwrap_or(0).checked_add(1).ok_or("NodeId exhausted")?;
        self.next_pin_id=self.pins.keys().map(|id|id.get()).max().unwrap_or(0).checked_add(1).ok_or("PinId exhausted")?;
        self.next_edge_id=self.edges.keys().map(|id|id.get()).max().unwrap_or(0).checked_add(1).ok_or("EdgeId exhausted")?;
        Ok(())
    }
    /// Change only the explicit entry semantics, preserving all document IDs.
    pub fn set_narrative_metadata(&mut self, metadata: Option<NarrativeMetadata>) -> Result<(), String> {
        if metadata.is_some() && !matches!(self.kind, GraphKind::Label { .. }) {
            return Err("Narrative scene metadata requires a Label graph".into());
        }
        self.narrative = metadata;
        self.schema_version = if self.schema_version >= crate::GRAPH_NAME_TABLE_SCHEMA_VERSION { self.schema_version }
            else if self.schema_version == crate::GRAPH_TABLE_TEXT_SCHEMA_VERSION { crate::GRAPH_TABLE_TEXT_SCHEMA_VERSION }
            else if self.text_owner.is_some() { crate::GRAPH_TEXT_SCHEMA_VERSION }
            else if metadata.is_some() { crate::GRAPH_NARRATIVE_SCHEMA_VERSION }
            else { GRAPH_SCHEMA_VERSION };
        Ok(())
    }

    pub fn narrative_context(&self) -> rvn_parser::NarrativeContext {
        if self.narrative.is_some() { rvn_parser::NarrativeContext::ExplicitScenes }
        else { rvn_parser::NarrativeContext::Legacy }
    }

    pub fn validate_narrative_metadata(&self) -> Result<(), String> {
        self.validate_library_metadata1027()?;
        crate::entry_parameter_identity1003::validate_graph_parameter_metadata(self)?;
        self.validate_standard_macro_contracts()?;
        self.validate_inspector_evaluation_contract()?;
        self.validate_native_loop_contracts()?;
        self.validate_text_pins()?;
        self.validate_string_table_contracts()?;
        self.validate_name_text_nodes()?;
        self.validate_nominal_enum_shape1026()?;
        if self.schema_version > GRAPH_MAX_SUPPORTED_SCHEMA_VERSION {
            return Err(format!("Unsupported graph schema {}", self.schema_version));
        }
        if self.text_owner.is_some() && ![crate::GRAPH_TEXT_SCHEMA_VERSION, crate::GRAPH_TABLE_TEXT_SCHEMA_VERSION, crate::GRAPH_NAME_TABLE_SCHEMA_VERSION, crate::GRAPH_NAME_TEXT_SCHEMA_VERSION, crate::GRAPH_NATIVE_FRAME_SCHEMA_VERSION, crate::GRAPH_NATIVE_LOOP_DEMAND_SCHEMA_VERSION, crate::GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION, crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION, crate::GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION].contains(&self.schema_version) {
            return Err("Persistent text ownership requires opt-in graph schema 7 or 8".into());
        }
        if self.schema_version >= crate::GRAPH_NAME_TABLE_SCHEMA_VERSION {
            return if self.narrative.is_none() || matches!(self.kind, GraphKind::Label { .. }) { Ok(()) }
                else { Err("Narrative scene metadata requires a Label graph".into()) };
        }
        if [crate::GRAPH_TEXT_SCHEMA_VERSION, crate::GRAPH_TABLE_TEXT_SCHEMA_VERSION].contains(&self.schema_version) {
            if self.text_owner.is_none() {
                return Err("Text schema requires a persistent text asset owner".into());
            }
            return if self.narrative.is_none() || matches!(self.kind, GraphKind::Label { .. }) {
                Ok(())
            } else { Err("Narrative scene metadata requires a Label graph".into()) };
        }
        match (self.schema_version, self.narrative) {
            (6, Some(_)) if matches!(self.kind, GraphKind::Label { .. }) => Ok(()),
            (6, _) => Err("Schema 6 requires explicit narrative metadata on a Label graph".into()),
            (_, Some(_)) => Err("Narrative metadata requires opt-in graph schema 6".into()),
            (_, None) => Ok(()),
        }
    }

    /// Preserve identities and presentation when re-importing changed source.
    /// This is in-memory only; callers keep the previous document on failure.
    pub fn reconcile_import(&mut self, previous: &Self) -> Result<(), String> {
        if self.kind != previous.kind {
            return Err("Cannot reconcile different graph kinds".into());
        }
        for graph in [&*self, previous] {
            if graph.nodes.values().any(|node| {
                node.pins
                    .iter()
                    .any(|id| !graph.pins.get(id).is_some_and(|pin| pin.node == node.id))
            }) || graph.pins.values().any(|pin| {
                !graph
                    .nodes
                    .get(&pin.node)
                    .is_some_and(|node| node.pins.contains(&pin.id))
            }) || graph.edges.values().any(|edge| {
                !graph.pins.contains_key(&edge.output) || !graph.pins.contains_key(&edge.input)
            }) || graph.nodes.values().any(|node| {
                node.position
                    .iter()
                    .any(|value| !value.is_finite() || value.abs() > 1_000_000_000.0)
            }) || graph.nodes.keys().any(|id| id.get() >= graph.next_node_id)
                || graph.pins.keys().any(|id| id.get() >= graph.next_pin_id)
                || graph.edges.keys().any(|id| id.get() >= graph.next_edge_id)
            {
                return Err("Cannot reconcile a structurally invalid graph".into());
            }
        }
        fn signature(graph: &GraphDocument, node: &GraphNode) -> Result<String, String> {
            fn input_signatures(
                graph: &GraphDocument,
                node: &GraphNode,
                seen: &mut BTreeSet<NodeId>,
                depth: usize,
            ) -> Result<String, String> {
                if depth == 0 || !seen.insert(node.id) {
                    return Ok("cycle-or-depth-limit".into());
                }
                let mut inputs = Vec::new();
                for id in &node.pins {
                    let pin = &graph.pins[id];
                    if pin.direction != PinDirection::Input || pin.value_type.is_execution() {
                        continue;
                    }
                    for edge in graph.edges.values().filter(|edge| edge.input == *id) {
                        let output = &graph.pins[&edge.output];
                        let source = &graph.nodes[&output.node];
                        inputs.push((
                            pin.key.clone(),
                            output.key.clone(),
                            source.kind,
                            source.string_table_contract,
                            semantic_node_properties(source),
                            &output.text_table_reference,
                            input_signatures(graph, source, seen, depth - 1)?,
                        ));
                    }
                }
                seen.remove(&node.id);
                serde_json::to_string(&inputs).map_err(|e| e.to_string())
            }
            let pins: Vec<_> = node
                .pins
                .iter()
                .map(|id| {
                    let pin = &graph.pins[id];
                    (
                        &pin.key,
                        pin.direction,
                        &pin.value_type,
                        pin.cardinality,
                        // A connected Asset fallback is presentation, not
                        // source identity. The actual incoming producer still
                        // participates in input_signatures above.
                        (!(pin.direction == PinDirection::Input
                            && matches!(&pin.value_type, ValueType::Asset(_))
                            && graph.edges.values().any(|edge| edge.input == pin.id)))
                            .then_some(&pin.default_value),
                        &pin.text_table_reference,
                    )
                })
                .collect();
            let inputs = input_signatures(graph, node, &mut BTreeSet::new(), 32)?;
            let qualified=graph.library_call1027(node.id).map(|call|(&call.target,call.arguments.iter().map(|(id,_)|id).collect::<Vec<_>>()));
            let mut properties=semantic_node_properties(node);if qualified.is_some(){properties.remove("function");}
            serde_json::to_string(&(node.kind, node.string_table_contract, properties, qualified, pins, inputs))
                .map_err(|e| e.to_string())
        }
        fn allocate(next: &mut u64) -> Result<u64, String> {
            let id = *next;
            *next = next
                .checked_add(1)
                .ok_or("Graph identity space exhausted")?;
            Ok(id)
        }
        let mut matches: BTreeMap<String, std::collections::VecDeque<NodeId>> = BTreeMap::new();
        for node in previous.nodes.values() {
            matches
                .entry(signature(previous, node)?)
                .or_default()
                .push_back(node.id);
        }
        let retained_asset_owners: BTreeSet<_> = previous.pins.values()
            .filter(|pin| pin.direction == PinDirection::Input
                && matches!(&pin.value_type, ValueType::Asset(_)) && pin.default_value.is_some()
                && previous.edges.values().any(|edge| edge.input == pin.id))
            .map(|pin| pin.node).collect();
        let mut source_signatures = BTreeMap::<String, usize>::new();
        if !retained_asset_owners.is_empty() {
            for node in self.nodes.values() {
                *source_signatures.entry(signature(self, node)?).or_default() += 1;
            }
        }
        let mut matched_nodes = BTreeMap::new();
        let mut used = BTreeSet::new();
        let mut reference_signatures = BTreeMap::<String, usize>::new();
        for node in self.nodes.values().filter(|node| node.pins.iter().any(|id| self.pins[id].text_table_reference.is_some())) {
            *reference_signatures.entry(signature(self, node)?).or_default() += 1;
        }
        for node in self.nodes.values() {
            let key = signature(self, node)?;
            let Some(ids) = matches.get_mut(&key) else { continue; };
            // Repeated retained references have the same value but distinct
            // producers. Source order cannot choose their durable identity.
            // The neighbor pass below anchors them to their named consumers.
            if reference_signatures.get(&key).is_some_and(|count| *count > 1 || ids.len() > 1) { continue; }
            // Hidden Asset state cannot inherit an order-selected identity or
            // use such a duplicate as its anchor. Wait for real unique neighbors.
            if !retained_asset_owners.is_empty()
                && (ids.len() > 1 || source_signatures.get(&key).is_some_and(|count| *count > 1)) { continue; }
            if let Some(id) = ids.pop_front() {
                matched_nodes.insert(node.id, id);
                used.insert(id);
            }
        }
        // Changed values keep their identity when their unchanged connected
        // neighbors identify them unambiguously. No position/order guessing.
        loop {
            let mut proposals = Vec::new();
            for node in self
                .nodes
                .values()
                .filter(|node| !matched_nodes.contains_key(&node.id))
            {
                let shape = |graph: &GraphDocument, node: &GraphNode| {
                    node.pins
                        .iter()
                        .map(|id| {
                            let pin = &graph.pins[id];
                            (
                                pin.key.clone(),
                                pin.direction,
                                pin.value_type.clone(),
                                pin.cardinality,
                            )
                        })
                        .collect::<Vec<_>>()
                };
                let mut candidates = Vec::new();
                for old in previous
                    .nodes
                    .values()
                    .filter(|old| !used.contains(&old.id) && old.kind == node.kind)
                {
                    if shape(self, node) != shape(previous, old) {
                        continue;
                    }
                    let mut score = 0;
                    for edge in self.edges.values() {
                        let output = &self.pins[&edge.output];
                        let input = &self.pins[&edge.input];
                        let anchor = if output.node == node.id {
                            matched_nodes.get(&input.node).map(|id| (*id, true))
                        } else if input.node == node.id {
                            matched_nodes.get(&output.node).map(|id| (*id, false))
                        } else {
                            None
                        };
                        if let Some((neighbor, outgoing)) = anchor {
                            if previous.edges.values().any(|edge| {
                                let old_output = &previous.pins[&edge.output];
                                let old_input = &previous.pins[&edge.input];
                                old_output.key == output.key
                                    && old_input.key == input.key
                                    && if outgoing {
                                        old_output.node == old.id && old_input.node == neighbor
                                    } else {
                                        old_input.node == old.id && old_output.node == neighbor
                                    }
                            }) {
                                score += 1;
                            }
                        }
                    }
                    if score > 0 {
                        candidates.push((score, old.id));
                    }
                }
                candidates.sort_by(|a, b| b.0.cmp(&a.0));
                if let Some((score, id)) = candidates.first().copied() {
                    if candidates.get(1).is_none_or(|other| other.0 < score) {
                        proposals.push((node.id, id));
                    }
                }
            }
            let unique: Vec<_> = proposals
                .iter()
                .filter(|(_, old)| {
                    proposals
                        .iter()
                        .filter(|(_, candidate)| candidate == old)
                        .count()
                        == 1
                })
                .copied()
                .collect();
            if unique.is_empty() {
                break;
            }
            for (node, old) in unique {
                matched_nodes.insert(node, old);
                used.insert(old);
            }
        }
        // Unused old nodes with no compatible new candidate were deleted.
        // Only an unresolved surviving candidate needs explicit reconciliation;
        // the presence of an old fallback never vetoes unrelated source edits.
        for old in retained_asset_owners.iter().filter(|id| !used.contains(*id)) {
            let old = &previous.nodes[old];
            let compatible = self.nodes.values().filter(|node| !matched_nodes.contains_key(&node.id))
                .any(|node| node.kind == old.kind && node.pins.len() == old.pins.len()
                    && node.pins.iter().zip(&old.pins).all(|(new, prior)| {
                        let new = &self.pins[new]; let prior = &previous.pins[prior];
                        new.key == prior.key && new.direction == prior.direction
                            && new.value_type == prior.value_type && new.cardinality == prior.cardinality
                    }));
            if compatible {
                return Err(format!("Ambiguous retained Asset slot at node {}; explicit presentation reconciliation required; no graph was changed", old.id));
            }
        }
        let mut next_node = previous.next_node_id;
        let mut next_pin = previous.next_pin_id;
        let mut next_edge = previous.next_edge_id;
        // BranchEnd stores a node reference in its owner property, not in a
        // pin. Resolve every fresh identity before copying any node so a
        // forward/backward owner reference follows the same remapping.
        let mut node_ids = matched_nodes.clone();
        for original in self.nodes.values() {
            if !node_ids.contains_key(&original.id) {
                node_ids.insert(original.id, NodeId::new(allocate(&mut next_node)?));
            }
        }
        let mut nodes = BTreeMap::new();
        let mut pins = BTreeMap::new();
        let mut pin_ids = BTreeMap::new();
        for original in self.nodes.values() {
            let matched = matched_nodes.get(&original.id).copied();
            let id = node_ids[&original.id];
            let mut node = original.clone();
            node.id = id;
            if node.kind == NodeKind::BranchEnd {
                if let Some(PropertyValue::Int(owner)) = node.properties.get_mut("owner") {
                    if let Ok(owner_id) = u64::try_from(*owner) {
                        if let Some(mapped) = node_ids.get(&NodeId::new(owner_id)) {
                            *owner = i64::try_from(mapped.get()).map_err(|_| {
                                "Branch owner identity exceeds its property representation"
                            })?;
                        }
                    }
                }
            }
            node.pins.clear();
            if let Some(old) = matched.and_then(|id| previous.nodes.get(&id)) {
                node.position = old.position;
                node.title_override = old.title_override.clone();
                node.annotation = old.annotation.clone();
                // Keep old presentation readable; conversion to the typed
                // annotation occurs only on an explicit editor modification.
                if let Some(comment) = old.properties.get("comment") {
                    node.properties.insert("comment".into(), comment.clone());
                }
            }
            for original_pin in &original.pins {
                let mut pin = self.pins[original_pin].clone();
                let nominal_parameter=self.library_call1027(original.id).and_then(|call|call.arguments.iter().find(|(_,id)|id==original_pin).map(|(parameter,_)|parameter));
                let old_pin = if let Some(parameter)=nominal_parameter{
                    matched.and_then(|old|previous.library_call1027(old)).and_then(|call|call.arguments.iter().find(|(id,_)|id==parameter)).and_then(|(_,id)|previous.pins.get(id))
                }else{matched.and_then(|old| previous.pin_by_key(old, &pin.key))};
                if let Some(prior) = old_pin.filter(|prior| prior.direction == PinDirection::Input
                    && retains_inline_pin_default(original.kind, &pin.value_type)
                    && Some(prior.node) == matched
                    && previous.nodes.get(&prior.node).is_some_and(|owner| owner.kind == original.kind)
                    && pin.direction == prior.direction && pin.value_type == prior.value_type
                    && pin.cardinality == prior.cardinality && prior.default_value.is_some()
                    && self.edges.values().any(|edge| edge.input == *original_pin)
                    && previous.edges.values().any(|edge| edge.input == prior.id)) {
                    // matched is the actual unique reconciliation result, not a
                    // provisional numeric ID or a positional sidecar lookup.
                    pin.default_value = prior.default_value.clone();
                    pin.text_localization = prior.text_localization.clone();
                    pin.text_table_reference = prior.text_table_reference.clone();
                }
                let pin_id = match old_pin {
                    Some(old) => old.id,
                    None => PinId::new(allocate(&mut next_pin)?),
                };
                pin_ids.insert(*original_pin, pin_id);
                pin.id = pin_id;
                pin.node = id;
                node.pins.push(pin_id);
                pins.insert(pin_id, pin);
            }
            nodes.insert(id, node);
        }
        let old_edges: BTreeMap<_, _> = previous
            .edges
            .values()
            .map(|edge| ((edge.output, edge.input), edge.id))
            .collect();
        let mut edges = BTreeMap::new();
        for original in self.edges.values() {
            let output = pin_ids[&original.output];
            let input = pin_ids[&original.input];
            let id = match old_edges.get(&(output, input)) {
                Some(id) => *id,
                None => EdgeId::new(allocate(&mut next_edge)?),
            };
            edges.insert(id, GraphEdge { id, output, input });
        }
        // Keep all existing placements untouched, including intentional
        // overlaps. Only freshly imported nodes receive automatic placement.
        let mut trial = self.clone();
        trial.graph_id = previous.graph_id;
        trial.text_owner = previous.text_owner.clone();
        trial.authoring_defaults = previous.authoring_defaults;
        trial.set_narrative_metadata(self.narrative)?;
        if let Some(metadata)=&mut trial.library1027{
            metadata.calls=std::mem::take(&mut metadata.calls).into_iter().map(|(node,mut call)|{
                for(_,pin)in &mut call.arguments{*pin=pin_ids[pin];}(node_ids[&node],call)
            }).collect();
        }
        trial.nodes = nodes;
        trial.pins = pins;
        trial.edges = edges;
        trial.next_node_id = next_node;
        trial.next_pin_id = next_pin;
        trial.next_edge_id = next_edge;
        let new_nodes: Vec<_> = trial
            .nodes
            .keys()
            .copied()
            .filter(|id| !used.contains(id))
            .collect();
        for node in new_nodes {
            trial.place_without_overlap(node);
        }
        // Commit only after all allocations and placement succeed.
        *self = trial;
        Ok(())
    }

    pub fn new(graph_id: GraphId, kind: GraphKind) -> Self {
        Self {
            library1027: None,
            schema_version: if matches!(&kind,GraphKind::UnitFunction{..}) { crate::GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION } else { GRAPH_SCHEMA_VERSION },
            graph_id,
            inspector_evaluation: Default::default(),
            authoring_defaults: None,
            text_owner: None,
            kind,
            narrative: None,
            nodes: BTreeMap::new(),
            pins: BTreeMap::new(),
            edges: BTreeMap::new(),
            variables: BTreeMap::new(),
            characters: BTreeMap::new(),
            next_node_id: 1,
            next_pin_id: 1,
            next_edge_id: 1,
        }
    }

    /// Check a batch before allocating any graph identities. This query never
    /// reserves identities or changes the document; the caller must allocate
    /// on the same unchanged candidate after success.
    pub fn check_allocation_capacity(&self, nodes: usize, pins: usize, edges: usize) -> Result<(), String> {
        fn end(start: u64, count: usize) -> Result<u64, String> {
            u64::try_from(count).ok().and_then(|count| start.checked_add(count))
                .filter(|_| start != 0).ok_or_else(|| "Graph identity allocation exhausted".into())
        }
        let node_end = end(self.next_node_id, nodes)?;
        let pin_end = end(self.next_pin_id, pins)?;
        let edge_end = end(self.next_edge_id, edges)?;
        if self.nodes.range(NodeId::new(self.next_node_id)..NodeId::new(node_end)).next().is_some()
            || self.pins.range(PinId::new(self.next_pin_id)..PinId::new(pin_end)).next().is_some()
            || self.edges.range(EdgeId::new(self.next_edge_id)..EdgeId::new(edge_end)).next().is_some() {
            return Err("Graph identity allocation counter is stale".into());
        }
        Ok(())
    }

    pub fn add_node(&mut self, kind: NodeKind, position: [f64; 2]) -> NodeId {
        if kind==NodeKind::GetStringTableIdAndKey {
            let historical_native=self.schema_version==crate::GRAPH_NATIVE_FRAME_SCHEMA_VERSION && !self.inspector_evaluation.is_legacy();
            for previous in self.nodes.values_mut().filter(|node|node.kind==NodeKind::GetStringTableIdAndKey) {
                previous.inspector_contract.get_or_insert(if historical_native {crate::InspectorEvaluationContract::NativeFrameV1} else {crate::InspectorEvaluationContract::LegacyStateless});
            }
            self.inspector_evaluation=crate::InspectorEvaluationContract::NativeFrameV1;
            self.schema_version=self.schema_version.max(crate::GRAPH_MIXED_INSPECTOR_SCHEMA_VERSION);
        }
        if matches!(kind,NodeKind::UnitCall|NodeKind::UnitReturn){self.schema_version=self.schema_version.max(crate::GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION);}
        let id = NodeId::new(self.next_node_id);
        self.next_node_id += 1;
        if kind.has_string_table_contract() || matches!(kind, NodeKind::TextFromStringTable | NodeKind::GetStringTableIdAndKey) {
            // Direct serde users and old in-memory documents may still carry
            // absent markers. Promotion records their existing String behavior
            // before enabling the stricter schema; no pin/value is retaped.
            for previous in self.nodes.values_mut() {
                if previous.kind.has_string_table_contract() && previous.string_table_contract.is_none() {
                    previous.string_table_contract = Some(crate::StringTableQueryContract::LegacyStringV1);
                }
            }
            let feature = if kind.has_string_table_contract() { crate::GRAPH_NAME_TABLE_SCHEMA_VERSION } else { crate::GRAPH_NAME_TEXT_SCHEMA_VERSION };
            self.schema_version = self.schema_version.max(feature);
        }
        self.nodes.insert(
            id,
            GraphNode {
                id,
                kind,
                position,
                title_override: None,
                annotation: None,
                string_table_contract: kind.has_string_table_contract().then_some(crate::StringTableQueryContract::NameV2),
                inspector_contract: (kind==NodeKind::GetStringTableIdAndKey).then_some(crate::InspectorEvaluationContract::NativeFrameV1),
                native_loop_contract: None,
                standard_macro_contract: crate::standard_macros_1020::kind(kind).map(|_|crate::StandardMacroContract::WidgetInstanceV1),
                properties: BTreeMap::new(),
                pins: Vec::new(),
            },
        );
        id
    }

    /// Create an explicitly selected behavior boundary without retaping existing nodes.
    pub fn add_table_query_node(&mut self, kind: NodeKind, position: [f64; 2], contract: crate::StringTableQueryContract) -> Result<NodeId, GraphEditError> {
        let definitions = crate::pin_catalog::table_pin_definitions(kind, contract)
            .ok_or(GraphEditError::WrongNodeKind { node: NodeId::new(self.next_node_id), expected: NodeKind::StringTableId, found: kind })?;
        let before_schema = self.schema_version;
        let node = self.add_catalog_node(kind, position)?;
        self.nodes.get_mut(&node).unwrap().string_table_contract = Some(contract);
        for definition in definitions {
            let id = self.pin_by_key(node, definition.key).unwrap().id;
            let pin = self.pins.get_mut(&id).unwrap();
            pin.value_type = definition.value_type;
            pin.default_value = definition.default_value;
        }
        if contract == crate::StringTableQueryContract::LegacyStringV1 { self.schema_version = before_schema; }
        Ok(node)
    }

    /// Validate query behavior without requiring a complete owned Text document.
    pub fn validate_string_table_contracts(&self) -> Result<(), String> {
        for node in self.nodes.values() {
            if !node.kind.has_string_table_contract() {
                if node.string_table_contract.is_some() { return Err("Table query contract on unrelated node".into()); }
                continue;
            }
            if node.string_table_contract == Some(crate::StringTableQueryContract::NameV2) && self.schema_version < crate::GRAPH_NAME_TABLE_SCHEMA_VERSION {
                return Err("Name table queries require opt-in graph schema 9".into());
            }
            if self.schema_version >= crate::GRAPH_NAME_TABLE_SCHEMA_VERSION && node.string_table_contract.is_none() {
                return Err("Schema 9 table query requires an explicit contract".into());
            }
            for expected in crate::pin_catalog::table_pin_definitions(node.kind, node.table_query_contract()).unwrap() {
                let pin = self.pin_by_key(node.id, expected.key).ok_or("Table query pin is missing")?;
                if pin.value_type != expected.value_type || pin.direction != expected.direction || pin.cardinality != expected.cardinality {
                    return Err(format!("Table query contract mismatch at {}.{}", node.id, expected.key));
                }
                if let Some(value) = &pin.default_value {
                    let actual = crate::type_inference::property_type(value);
                    if !pin.value_type.accepts(&actual) && !(pin.value_type == ValueType::Asset(AssetKind::StringTable) && matches!(value, PropertyValue::String(_))) {
                        return Err(format!("Table query default mismatch at {}.{}", node.id, expected.key));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn add_variable(
        &mut self,
        name: impl Into<String>,
        value_type: ValueType,
        default_value: PropertyValue,
    ) -> Result<(), GraphEditError> {
        let name = name.into();
        if name.trim().is_empty() || self.variables.contains_key(&name) {
            return Err(GraphEditError::InvalidVariableName(name));
        }
        if matches!(&value_type, ValueType::Enum(_)) && property_value_type(&default_value) != value_type {
            return Err(GraphEditError::IncompatibleTypes { output: property_value_type(&default_value), input: value_type });
        }
        self.variables.insert(
            name.clone(),
            VariableDefinition {
                name,
                value_type,
                default_value,
            },
        );
        self.adopt_nominal_enum_schema1026();
        Ok(())
    }

    pub fn rename_variable(
        &mut self,
        old: &str,
        new: impl Into<String>,
    ) -> Result<(), GraphEditError> {
        let new = new.into();
        if self.variable_scope(old) == Some(crate::VariableScope::Local)
            && self.nodes.values().any(|node| node.kind == NodeKind::LocalVariable
                && self.variable_node_name(node.id).as_deref() == Some(old))
        {
            crate::variable_rename::rename_local(self, old, new)
        } else {
            self.rename_variable_references(old, new)
        }
    }

    /// Internal name projection for dependency probes. A probe must include
    /// pre-declaration global reads even when a user refactor preserves them.
    pub(crate) fn rename_variable_references(
        &mut self,
        old: &str,
        new: impl Into<String>,
    ) -> Result<(), GraphEditError> {
        let new = new.into();
        if new.trim().is_empty() || (old != new && self.variables.contains_key(&new)) {
            return Err(GraphEditError::InvalidVariableName(new));
        }
        let Some(mut definition) = self.variables.get(old).cloned() else {
            return Err(GraphEditError::InvalidVariableName(old.into()));
        };
        if old == new { return Ok(()); }
        self.rename_variable_reference_payloads(old, &new)?;
        self.variables.remove(old);
        definition.name = new.clone();
        self.variables.insert(new, definition);
        Ok(())
    }

    /// Rewrite real named ports and embedded consumers after lexical owner
    /// admission. The optional presentation registry is not binding authority.
    pub(crate) fn rename_variable_reference_payloads(
        &mut self, old: &str, new: impl Into<String>,
    ) -> Result<(), GraphEditError> {
        let new = new.into();
        if old == new { return Ok(()); }
        if rvn_parser::variable_source_name(&new).is_none() {
            return Err(GraphEditError::InvalidVariableName(new));
        }
        // Preflight every embedded expression before changing the declaration
        // or a pin. An unfinished text draft must never produce a half rename.
        let mut properties = Vec::new();
        let mut choice_labels = Vec::new();
        for node in self.nodes.values() {
            let failure = |key: &str, message: String| GraphEditError::InvalidReferenceExpression {
                node: node.id, key: key.into(), message,
            };
            if node.kind == NodeKind::TextValue && self.pin_by_key(node.id, "value")
                .is_none_or(|pin| pin.text_localization.is_none()) {
                if let Some(PropertyValue::String(value)) = node.properties.get("value") {
                    let next = rename_variable_text(value, old, &new).map_err(|error| failure("value", error))?;
                    if next != *value { properties.push((node.id, "value".into(), PropertyValue::String(next))); }
                }
            }
            if node.kind == NodeKind::Choice {
                if let Some(PropertyValue::StringList(options)) = node.properties.get("options") {
                    let next: Vec<_> = options.iter().map(|text| rename_variable_text(text, old, &new))
                        .collect::<Result<_, _>>().map_err(|error| failure("options", error))?;
                    if next != *options {
                        for (index, label) in next.iter().enumerate() {
                            if let Some(pin) = self.pin_by_key(node.id, &format!("option_{index}")) {
                                choice_labels.push((pin.id, label.clone()));
                            }
                        }
                        properties.push((node.id, "options".into(), PropertyValue::StringList(next)));
                    }
                }
                for (key, value) in &node.properties {
                    if key.starts_with("option_") && key.ends_with("_condition") {
                        if let PropertyValue::String(text) = value {
                            let next = rename_variable_condition(text, old, &new).map_err(|error| failure(key, error))?;
                            if next != *text { properties.push((node.id, key.clone(), PropertyValue::String(next))); }
                        }
                    }
                }
            }
        }
        for (node, key, value) in properties { self.nodes.get_mut(&node).unwrap().properties.insert(key, value); }
        for (pin, label) in choice_labels { self.pins.get_mut(&pin).unwrap().label = label; }
        for node in self.nodes.values_mut() {
            if matches!(node.kind, NodeKind::VariableGet | NodeKind::VariableReference
                | NodeKind::SetVariable | NodeKind::LocalVariable | NodeKind::ForEach | NodeKind::ForLoopWithBreak)
                && node.properties.get("name") == Some(&PropertyValue::String(old.into()))
            {
                node.properties
                    .insert("name".into(), PropertyValue::String(new.clone()));
                for pin in &node.pins {
                    if let Some(pin) = self.pins.get_mut(pin) {
                        if pin.key == "name" {
                            pin.default_value = Some(PropertyValue::String(new.clone()));
                        }
                    }
                }
            }
            if matches!(node.kind, NodeKind::FunctionEntry | NodeKind::ScreenEntry | NodeKind::HandlerEntry) {
                if let Some(PropertyValue::StringList(parameters)) = node.properties.get_mut("parameters") {
                    for parameter in parameters { if parameter == old { *parameter = new.clone(); } }
                }
            }
        }
        Ok(())
    }

    pub fn add_catalog_node(
        &mut self,
        kind: NodeKind,
        position: [f64; 2],
    ) -> Result<NodeId, GraphEditError> {
        if crate::widget_viewport_graph1003::is_viewport_kind(kind) {
            self.widget_viewport_node_context(kind).map_err(|message|GraphEditError::InvalidReferenceExpression{node:NodeId::new(self.next_node_id),key:"contexte".into(),message})?;
        }
        let definition = node_definition(kind);
        let node = self.add_node(kind, position);
        for pin_definition in definition.pins {
            let pin = self.add_pin(
                node,
                pin_definition.key,
                pin_definition.label,
                pin_definition.direction,
                pin_definition.value_type,
                pin_definition.cardinality,
            )?;
            self.pins.get_mut(&pin).unwrap().default_value = pin_definition.default_value;
        }
        // Les propriÃ©tÃ©s qui configurent la forme d'un nÅ“ud (et non une entrÃ©e)
        // doivent Ãªtre prÃ©sentes dÃ¨s sa crÃ©ation. L'Ã©diteur peut ainsi proposer
        // un inspecteur typÃ© sans connaÃ®tre des valeurs implicites propres au codegen.
        let defaults = match kind {
            NodeKind::WidgetClassValue => vec![("uuid", PropertyValue::String(String::new()))],
            NodeKind::WidgetCreate => vec![("name", PropertyValue::String(format!("widget_result_{}", node.get()))), ("local", PropertyValue::Bool(false))],
            NodeKind::UiOpen => vec![("story",PropertyValue::Bool(false))],
            NodeKind::Label => vec![(
                "label",
                PropertyValue::String(
                    if self
                        .nodes
                        .values()
                        .filter(|n| n.kind == NodeKind::Label)
                        .count()
                        == 1
                    {
                        String::new()
                    } else {
                        format!("label_{}", node)
                    },
                ),
            )],
            NodeKind::MakeColor => vec![("rgb_max", PropertyValue::Int(255))],
            NodeKind::Literal => vec![("value", PropertyValue::String(String::new()))],
            NodeKind::TextValue => vec![("value", PropertyValue::String(String::new()))],
            NodeKind::LabelValue => vec![("label", PropertyValue::String(String::new()))],
            NodeKind::PositionValue => vec![("position", PropertyValue::String("center".into()))],
            NodeKind::CharacterValue => vec![
                ("character", PropertyValue::String(String::new())),
                ("emotion", PropertyValue::String(String::new())),
            ],
            NodeKind::StringTableAsset => vec![("table", PropertyValue::String(String::new()))],
            NodeKind::SceneAsset
            | NodeKind::SpriteAsset
            | NodeKind::MusicAsset
            | NodeKind::SoundEffectAsset
            | NodeKind::VoiceAsset
            | NodeKind::CinematicAsset
            | NodeKind::HoverImageAsset
            | NodeKind::ScriptAsset => {
                vec![("path", PropertyValue::String(String::new()))]
            }
            NodeKind::TransitionNone => Vec::new(),
            NodeKind::TransitionFade => vec![("duration_ms", PropertyValue::Int(500))],
            NodeKind::TransitionDissolve => vec![("duration_ms", PropertyValue::Int(300))],
            NodeKind::TransitionSlideLeft
            | NodeKind::TransitionSlideRight
            | NodeKind::TransitionSlideUp
            | NodeKind::TransitionSlideDown
            | NodeKind::TransitionZoomIn
            | NodeKind::TransitionZoomOut
            | NodeKind::TransitionBlur => vec![("duration_ms", PropertyValue::Int(400))],
            NodeKind::TransitionWipe => vec![("duration_ms", PropertyValue::Int(500))],
            NodeKind::VariableGet => {
                vec![("name", PropertyValue::String("variable".into()))]
            }
            NodeKind::VariableReference => {
                vec![("name", PropertyValue::String("variable".into()))]
            }
            NodeKind::BinaryOperator => {
                vec![("operator", PropertyValue::String("+".into()))]
            }
            NodeKind::UnaryOperator => {
                vec![("operator", PropertyValue::String("not".into()))]
            }
            NodeKind::UnitCall => vec![("function",PropertyValue::String(String::new()))],
            NodeKind::FunctionCall => {
                vec![
                    ("function", PropertyValue::String("min".into())),
                    ("args", PropertyValue::StringList(Vec::new())),
                ]
            }
            NodeKind::FunctionEntry | NodeKind::ScreenEntry => {
                vec![("parameters", PropertyValue::StringList(Vec::new()))]
            }
            NodeKind::HandlerEntry => vec![(
                "parameters",
                PropertyValue::StringList(vec!["event".into()]),
            )],
            NodeKind::ForEach => vec![("name", PropertyValue::String("item".into()))],
            NodeKind::Sequence => vec![("output_count",PropertyValue::Int(2))],
            NodeKind::ForLoopWithBreak => vec![("name",PropertyValue::String("index".into()))],
            NodeKind::ListLiteral => {
                vec![("items", PropertyValue::StringList(Vec::new()))]
            }
            NodeKind::Choice => {
                vec![("options", PropertyValue::StringList(Vec::new()))]
            }
            NodeKind::SpriteAnimate => {
                vec![("params", PropertyValue::StringList(Vec::new()))]
            }
            NodeKind::SpriteEffect => ["flip_x", "flip_y", "scale", "rotation", "tint"]
                .into_iter()
                .map(|key| {
                    (
                        match key {
                            "flip_x" => "apply_flip_x",
                            "flip_y" => "apply_flip_y",
                            "scale" => "apply_scale",
                            "rotation" => "apply_rotation",
                            _ => "apply_tint",
                        },
                        PropertyValue::Bool(false),
                    )
                })
                .collect(),
            NodeKind::Imagemap => {
                vec![("hotspots", PropertyValue::StringList(Vec::new()))]
            }
            _ => Vec::new(),
        };
        for (key, value) in defaults {
            self.set_property(node, key, value)?;
        }
        Ok(node)
    }

    /// Typed, connectable arguments. Legacy string arrays are kept as strings
    /// when explicitly upgraded; numeric-looking old text is never reinterpreted.
    pub fn resize_value_inputs(
        &mut self,
        node: NodeId,
        count: usize,
    ) -> Result<(), GraphEditError> {
        if self.library_call1027(node).is_some(){return Err(GraphEditError::InvalidReferenceExpression{node,key:"function".into(),message:"Qualified Library arguments require the original source signature and explicit ParameterId-to-PinId transaction".into()});}
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if !matches!(owner.kind, NodeKind::FunctionCall | NodeKind::ListLiteral) {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::FunctionCall,
                found: owner.kind,
            });
        }
        let legacy_key = if owner.kind == NodeKind::FunctionCall {
            "args"
        } else {
            "items"
        };
        let legacy = match owner.properties.get(legacy_key) {
            Some(PropertyValue::StringList(values)) => values.clone(),
            _ => Vec::new(),
        };
        let count = count.min(128);
        for index in 0..count {
            let key = format!("item_{index}");
            if self.pin_by_key(node, &key).is_none() {
                let pin = self.add_pin(
                    node,
                    key,
                    format!("[{index}]"),
                    PinDirection::Input,
                    ValueType::Any,
                    PinCardinality::One,
                )?;
                self.pins.get_mut(&pin).unwrap().default_value = Some(
                    legacy
                        .get(index)
                        .map(|v| PropertyValue::String(v.clone()))
                        .unwrap_or(PropertyValue::Int(0)),
                );
            }
        }
        let removed: BTreeSet<_> = self.nodes[&node]
            .pins
            .iter()
            .copied()
            .filter(|id| {
                self.pins[id]
                    .key
                    .strip_prefix("item_")
                    .and_then(|s| s.parse::<usize>().ok())
                    .is_some_and(|index| index >= count)
            })
            .collect();
        self.nodes
            .get_mut(&node)
            .unwrap()
            .pins
            .retain(|id| !removed.contains(id));
        self.remove_edges_touching_pins(&removed);
        self.pins.retain(|id, _| !removed.contains(id));
        self.set_property(node, "input_count", PropertyValue::Int(count as i64))?;
        Ok(())
    }

    /// Prepare a reviewable explicit migration. This never changes the source
    /// graph or pretends that a historical named item read is ArrayElement.
    pub fn prepare_native_loop_demand(&self, owner: NodeId) -> Result<crate::NativeLoopMigration, String> {
        let node = self.nodes.get(&owner).ok_or("Loop migration owner absent")?;
        if !matches!(node.kind, NodeKind::ForEach | NodeKind::ForLoopWithBreak) {
            return Err("Native loop demand migration requires an actual ForEach/ForLoop node".into());
        }
        if node.native_loop_contract.is_some() {
            self.validate_native_loop_contracts()?;
            return Ok(crate::NativeLoopMigration { graph: self.clone(), owner, added_pins: vec![], legacy_named_reads: vec![] });
        }
        let legacy_name = self.variable_node_name(owner);
        let legacy_named_reads = self.nodes.values().filter(|node| node.kind == NodeKind::VariableGet
            && self.variable_node_name(node.id).is_some_and(|name| Some(name) == legacy_name)).map(|node| node.id).collect();
        let mut graph = self.clone();
        let added_pins=graph.install_native_loop_demand(owner)?;
        Ok(crate::NativeLoopMigration { graph, owner, added_pins, legacy_named_reads })
    }

    /// Owned source reconstruction uses the same checked pin installation as
    /// reviewable migrations, without cloning the whole growing graph per loop.
    pub(crate) fn install_native_loop_demand(&mut self,owner:NodeId)->Result<Vec<PinId>,String>{
        let node=self.nodes.get(&owner).ok_or("Native loop owner absent")?;
        let kind=node.kind;
        if !matches!(kind,NodeKind::ForEach|NodeKind::ForLoopWithBreak){return Err("Native source loop kind differs".into());}
        if node.native_loop_contract.is_some(){self.validate_native_loop_contracts()?;return Ok(vec![]);}
        self.enable_native_frame_preparation()?;
        self.schema_version = self.schema_version.max(crate::GRAPH_NATIVE_LOOP_DEMAND_SCHEMA_VERSION);
        let definitions = if kind == NodeKind::ForEach { crate::pin_catalog::native_loop_output_definitions() } else { vec![] };
        let needed = u64::try_from(definitions.len()).map_err(|_| "Loop pin allocation overflow")?;
        if self.next_pin_id == 0 || self.next_pin_id.checked_add(needed).is_none_or(|next| next >= u64::MAX)
            || (0..needed).any(|offset| self.pins.contains_key(&PinId::new(self.next_pin_id + offset))) {
            return Err("Loop pin allocation has no checked fresh IDs".into());
        }
        let mut added_pins = Vec::new();
        for definition in definitions {
            added_pins.push(self.add_pin(owner, definition.key, definition.label, definition.direction,
                definition.value_type, definition.cardinality).map_err(|error| format!("{error:?}"))?);
        }
        self.nodes.get_mut(&owner).unwrap().native_loop_contract = Some(crate::NativeLoopContract::DemandV1);
        self.validate_native_loop_contracts()?;
        Ok(added_pins)
    }

    pub fn add_pin(
        &mut self,
        node: NodeId,
        key: impl Into<String>,
        label: impl Into<String>,
        direction: PinDirection,
        value_type: ValueType,
        cardinality: PinCardinality,
    ) -> Result<PinId, GraphEditError> {
        let Some(owner) = self.nodes.get_mut(&node) else {
            return Err(GraphEditError::NodeNotFound(node));
        };
        let key = key.into();
        if owner
            .pins
            .iter()
            .filter_map(|pin| self.pins.get(pin))
            .any(|pin| pin.key == key)
        {
            return Err(GraphEditError::DuplicatePinKey { node, key });
        }
        let id = PinId::new(self.next_pin_id);
        self.next_pin_id += 1;
        owner.pins.push(id);
        self.pins.insert(
            id,
            GraphPin {
                id,
                node,
                key,
                label: label.into(),
                direction,
                value_type,
                cardinality,
                default_value: None,
                parameter_id: None,
                text_localization: None,
                text_table_reference: None,
            },
        );
        Ok(id)
    }

    /// Donne aux deux extrÃ©mitÃ©s d'un nÅ“ud de reroute le type exact du cÃ¢ble.
    /// Le type est sÃ©rialisÃ© avec le graphe afin que le nÅ“ud reste fidÃ¨le aprÃ¨s
    /// rechargement, y compris pour le flux d'exÃ©cution.
    pub fn set_reroute_type(
        &mut self,
        node: NodeId,
        value_type: ValueType,
    ) -> Result<(), GraphEditError> {
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::Reroute {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::Reroute,
                found: owner.kind,
            });
        }
        let pins = owner.pins.clone();
        for pin in pins {
            self.pins.get_mut(&pin).unwrap().value_type = value_type.clone();
        }
        Ok(())
    }

    pub fn pin_by_key(&self, node: NodeId, key: &str) -> Option<&GraphPin> {
        self.nodes.get(&node)?.pins.iter().find_map(|pin_id| {
            let pin = self.pins.get(pin_id)?;
            (pin.key == key).then_some(pin)
        })
    }

    pub fn set_pin_default(
        &mut self,
        node: NodeId,
        key: &str,
        value: PropertyValue,
    ) -> Result<(), GraphEditError> {
        let pin = self
            .pin_by_key(node, key)
            .map(|pin| pin.id)
            .ok_or_else(|| GraphEditError::PinKeyNotFound {
                node,
                key: key.to_owned(),
            })?;
        let enum_source = crate::type_inference::property_type(&value);
        let enum_target = self.pin_constraint_type(pin).unwrap_or(ValueType::Any);
        if (crate::enum_graph1026::contains_type1026(&enum_source) || crate::enum_graph1026::contains_type1026(&enum_target))
            && !self.accepts_pin_source(pin, &enum_source) {
            return Err(GraphEditError::IncompatibleTypes { output: enum_source, input: enum_target });
        }
        if self.has_blueprint_operator_policy(node) {
            let value_type = crate::type_inference::property_type(&value);
            return self.set_blueprint_pin_default(pin, value, value_type);
        }
        if (self.pins[&pin].text_localization.is_some() || self.pins[&pin].text_table_reference.is_some())
            && self.pins[&pin].default_value.as_ref() != Some(&value) {
            return Err(GraphEditError::InvalidPropertyType { node, key: key.into() });
        }
        if self.nodes[&node].kind == NodeKind::Choice && key.ends_with("_condition") {
            if !matches!(value, PropertyValue::Bool(_)) {
                return Err(GraphEditError::InvalidPropertyType { node, key: key.into() });
            }
            self.nodes.get_mut(&node).unwrap().properties.insert(
                crate::choice::default_marker(key), PropertyValue::Bool(true));
            self.nodes.get_mut(&node).unwrap().properties.insert(
                key.into(), PropertyValue::String(String::new()));
        }
        self.pins.get_mut(&pin).unwrap().default_value = Some(value);
        self.adopt_nominal_enum_schema1026();
        self.nodes
            .get_mut(&node)
            .unwrap()
            .properties
            .remove(&format!("@blueprint_default_type:{key}"));
        Ok(())
    }

    pub fn set_property(
        &mut self,
        node: NodeId,
        key: impl Into<String>,
        value: PropertyValue,
    ) -> Result<(), GraphEditError> {
        let key = key.into();
        if self.library_call1027(node).is_some()&&matches!(key.as_str(),"function"|"args"|"input_count"){
            return Err(GraphEditError::InvalidReferenceExpression{node,key,message:"Qualified Library targets/signatures require the original nominal resource transaction".into()});
        }
        let literal_type = property_value_type(&value);
        if key == "value" && self.nodes.get(&node).is_some_and(|n| n.kind == NodeKind::TextValue)
            && self.pin_by_key(node, &key).is_some_and(|p| p.text_localization.is_some() || p.text_table_reference.is_some())
            && self.nodes[&node].properties.get(&key) != Some(&value) {
            return Err(GraphEditError::InvalidPropertyType { node, key });
        }
        let owner = self
            .nodes
            .get_mut(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        let is_literal_value = owner.kind == NodeKind::Literal && key == "value";
        owner.properties.insert(key, value);
        if is_literal_value {
            let output = owner.pins.iter().copied().find(|pin_id| {
                self.pins
                    .get(pin_id)
                    .is_some_and(|pin| pin.direction == PinDirection::Output && pin.key == "value")
            });
            if let Some(output) = output {
                self.pins.get_mut(&output).unwrap().value_type = literal_type;
            }
        }
        self.adopt_nominal_enum_schema1026();
        Ok(())
    }

    pub fn add_choice_option(
        &mut self,
        node: NodeId,
        label: impl Into<String>,
    ) -> Result<ChoiceOptionPins, GraphEditError> {
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::Choice {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::Choice,
                found: owner.kind,
            });
        }
        let label = label.into();
        let index = match owner.properties.get("options") {
            Some(PropertyValue::StringList(options)) => options.len(),
            _ => 0,
        };
        let owner = self.nodes.get_mut(&node).unwrap();
        match owner.properties.entry("options".into()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(PropertyValue::StringList(vec![label.clone()]));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let PropertyValue::StringList(options) = entry.get_mut() else {
                    return Err(GraphEditError::InvalidPropertyType {
                        node,
                        key: "options".into(),
                    });
                };
                options.push(label.clone());
            }
        }
        let branch = self.add_pin(
            node,
            format!("option_{index}"),
            label,
            PinDirection::Output,
            ValueType::Execution,
            PinCardinality::Many,
        )?;
        let condition = self.add_pin(
            node,
            format!("option_{index}_condition"),
            "Condition",
            PinDirection::Input,
            ValueType::Bool,
            PinCardinality::One,
        )?;
        self.set_property(
            node,
            format!("option_{index}_condition"),
            PropertyValue::String(String::new()),
        )?;
        Ok(ChoiceOptionPins {
            index,
            branch,
            condition,
        })
    }

    pub fn add_operator_operand(&mut self, node: NodeId) -> Result<PinId, GraphEditError> {
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        let value_type = match owner.kind {
            NodeKind::LogicAnd | NodeKind::LogicOr => ValueType::Bool,
            NodeKind::MathAdd | NodeKind::MathMultiply => ValueType::Any,
            NodeKind::StringAppend => ValueType::String,
            _ => return Err(GraphEditError::UnsupportedDynamicOperand(node)),
        };
        let input_count = owner
            .pins
            .iter()
            .filter(|pin| self.pins[pin].direction == PinDirection::Input)
            .count();
        let pin = self.add_pin(
            node,
            format!("operand_{input_count}"),
            if owner.kind == NodeKind::StringAppend && input_count < 26 {
                ((b'A' + input_count as u8) as char).to_string()
            } else {
                format!("EntrÃ©e {}", input_count + 1)
            },
            PinDirection::Input,
            value_type.clone(),
            PinCardinality::One,
        )?;
        if value_type == ValueType::Bool {
            self.pins.get_mut(&pin).unwrap().default_value = Some(PropertyValue::Bool(false));
        } else if value_type == ValueType::String {
            self.pins.get_mut(&pin).unwrap().default_value =
                Some(PropertyValue::String(String::new()));
        }
        Ok(pin)
    }

    /// Met Ã  jour le patron d'un nÅ“ud Format Text et synchronise ses broches
    /// nommÃ©es comme le nÅ“ud Blueprint d'Unreal. Les broches dont le nom reste
    /// prÃ©sent conservent leur identifiant et donc leur cÃ¢ble.
    pub fn set_format_text_pattern(
        &mut self,
        node: NodeId,
        pattern: impl Into<String>,
    ) -> Result<(), GraphEditError> {
        let pattern = pattern.into();
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::FormatText {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::FormatText,
                found: owner.kind,
            });
        }

        let arguments = format_text_arguments(&pattern);
        if self.pin_by_key(node, "format").is_some_and(|pin| pin.text_table_reference.is_some()) {
            return Err(GraphEditError::InvalidPropertyType { node, key: "format".into() });
        }
        if owner.properties.get("deferred_text") == Some(&PropertyValue::Bool(true))
            && self.pin_by_key(node, "format").is_some_and(|pin| self.edges.values().any(|e| e.input == pin.id)) {
            return Err(GraphEditError::InvalidPropertyType { node, key: "format".into() });
        }
        if self.pin_by_key(node, "format").is_some_and(|pin| pin.text_localization.is_some()
            && pin.default_value != Some(PropertyValue::String(pattern.clone()))) {
            return Err(GraphEditError::InvalidPropertyType { node, key: "format".into() });
        }
        self.set_format_argument_names(node, &arguments)?;
        self.set_pin_default(node, "format", PropertyValue::String(pattern))?;
        Ok(())
    }

    /// Synchronize authored names only during an explicit default/reference edit.
    /// A table publication cannot call this or replace existing wires or IDs.
    pub(crate) fn set_format_argument_names(&mut self, node: NodeId, arguments: &[String]) -> Result<(), GraphEditError> {
        let owner = self.nodes.get(&node).ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::FormatText {
            return Err(GraphEditError::WrongNodeKind { node, expected: NodeKind::FormatText, found: owner.kind });
        }
        let work = arguments.iter().try_fold(0usize, |total, name| total.checked_add(name.len()));
        let unique: BTreeSet<_> = arguments.iter().collect();
        let new_count = arguments.iter().filter(|name| self.pin_by_key(node, &format!("argument_{name}")).is_none()).count();
        if arguments.len() > 4096 || work.is_none_or(|bytes| bytes > 1_048_576)
            || unique.len() != arguments.len() || arguments.iter().any(|name| name.is_empty() || name.chars().any(char::is_control))
            || self.next_pin_id.checked_add(new_count as u64).is_none() {
            return Err(GraphEditError::InvalidPropertyType { node, key: "format argument names".into() });
        }
        let wanted: BTreeSet<_> = arguments
            .iter()
            .map(|name| format!("argument_{name}"))
            .collect();
        let removed: BTreeSet<_> = owner
            .pins
            .iter()
            .filter_map(|pin| self.pins.get(pin))
            .filter(|pin| pin.key.starts_with("argument_") && !wanted.contains(&pin.key))
            .map(|pin| pin.id)
            .collect();

        self.nodes
            .get_mut(&node)
            .unwrap()
            .pins
            .retain(|pin| !removed.contains(pin));
        self.remove_edges_touching_pins(&removed);
        self.pins.retain(|pin, _| !removed.contains(pin));

        for argument in arguments {
            let key = format!("argument_{argument}");
            if self.pin_by_key(node, &key).is_none() {
                self.add_pin(
                    node,
                    key,
                    argument.clone(),
                    PinDirection::Input,
                    ValueType::Any,
                    PinCardinality::One,
                )?;
            }
        }

        // Unreal place Result aprÃ¨s les arguments dans la liste visuelle.
        // On rÃ©ordonne sans recrÃ©er la pin de sortie.
        let format_pin = self.pin_by_key(node, "format").map(|pin| pin.id);
        let result_pin = self.pin_by_key(node, "result").map(|pin| pin.id);
        let argument_pins: Vec<_> = arguments
            .iter()
            .filter_map(|argument| {
                self.pin_by_key(node, &format!("argument_{argument}"))
                    .map(|pin| pin.id)
            })
            .collect();
        let pins = &mut self.nodes.get_mut(&node).unwrap().pins;
        pins.clear();
        if let Some(pin) = format_pin {
            pins.push(pin);
        }
        pins.extend(argument_pins);
        if let Some(pin) = result_pin {
            pins.push(pin);
        }
        Ok(())
    }

    /// Remplace la liste d'options tout en conservant les identifiants des pins
    /// existants. Les cÃ¢bles des options qui restent en place ne sautent donc pas
    /// pendant une simple modification de libellÃ© dans l'inspecteur.
    pub fn set_choice_options(
        &mut self,
        node: NodeId,
        labels: Vec<String>,
    ) -> Result<(), GraphEditError> {
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::Choice {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::Choice,
                found: owner.kind,
            });
        }
        let previous_len = match owner.properties.get("options") {
            Some(PropertyValue::StringList(options)) => options.len(),
            _ => 0,
        };
        let shared_len = previous_len.min(labels.len());
        for (index, label) in labels.iter().take(shared_len).enumerate() {
            if let Some(pin) = self
                .pin_by_key(node, &format!("option_{index}"))
                .map(|pin| pin.id)
            {
                self.pins.get_mut(&pin).unwrap().label = label.clone();
            }
        }
        for label in labels.iter().skip(previous_len) {
            self.add_choice_option(node, label.clone())?;
        }
        if labels.len() < previous_len {
            for index in labels.len()..previous_len {
                self.nodes
                    .get_mut(&node)
                    .unwrap()
                    .properties
                    .remove(&format!("option_{index}_condition"));
                self.nodes.get_mut(&node).unwrap().properties
                    .remove(&crate::choice::default_marker(&format!("option_{index}_condition")));
            }
            let removed: BTreeSet<_> = (labels.len()..previous_len)
                .flat_map(|index| {
                    [
                        format!("option_{index}"),
                        format!("option_{index}_condition"),
                    ]
                })
                .filter_map(|key| self.pin_by_key(node, &key).map(|pin| pin.id))
                .collect();
            self.nodes
                .get_mut(&node)
                .unwrap()
                .pins
                .retain(|pin| !removed.contains(pin));
            self.remove_edges_touching_pins(&removed);
            self.pins.retain(|pin, _| !removed.contains(pin));
        }
        self.nodes
            .get_mut(&node)
            .unwrap()
            .properties
            .insert("options".into(), PropertyValue::StringList(labels));
        Ok(())
    }

    pub fn set_imagemap_hotspots(
        &mut self,
        node: NodeId,
        hotspots: Vec<String>,
    ) -> Result<(), GraphEditError> {
        let owner = self
            .nodes
            .get(&node)
            .ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::Imagemap {
            return Err(GraphEditError::WrongNodeKind {
                node,
                expected: NodeKind::Imagemap,
                found: owner.kind,
            });
        }
        let previous_len = match owner.properties.get("hotspots") {
            Some(PropertyValue::StringList(values)) => values.len(),
            _ => 0,
        };
        for (index, hotspot) in hotspots.iter().enumerate().take(previous_len) {
            if let Some(pin) = self
                .pin_by_key(node, &format!("hotspot_{index}"))
                .map(|pin| pin.id)
            {
                self.pins.get_mut(&pin).unwrap().label = hotspot_name(hotspot).to_owned();
            }
        }
        for (index, hotspot) in hotspots.iter().enumerate().skip(previous_len) {
            self.add_pin(
                node,
                format!("hotspot_{index}"),
                hotspot_name(hotspot),
                PinDirection::Output,
                ValueType::Execution,
                PinCardinality::Many,
            )?;
        }
        if hotspots.len() < previous_len {
            let removed: BTreeSet<_> = (hotspots.len()..previous_len)
                .filter_map(|index| {
                    self.pin_by_key(node, &format!("hotspot_{index}"))
                        .map(|pin| pin.id)
                })
                .collect();
            self.nodes
                .get_mut(&node)
                .unwrap()
                .pins
                .retain(|pin| !removed.contains(pin));
            self.remove_edges_touching_pins(&removed);
            self.pins.retain(|pin, _| !removed.contains(pin));
        }
        self.nodes
            .get_mut(&node)
            .unwrap()
            .properties
            .insert("hotspots".into(), PropertyValue::StringList(hotspots));
        Ok(())
    }

    /// Transforme les valeurs implicites significatives en nÅ“uds compacts
    /// connectÃ©s. Lâ€™Ã©diteur peut ainsi montrer la vÃ©ritable source de chaque
    /// donnÃ©e au lieu de cacher une valeur dans le pin consommateur.
    pub fn materialize_visible_defaults(&mut self) -> Result<usize, GraphEditError> {
        let mut created = self.normalize_legacy_operators();
        created += self.materialize_choice_conditions()?;
        let candidates: Vec<_> = self
            .pins
            .values()
            .filter(|pin| pin.direction == PinDirection::Input)
            .filter(|pin| !pin.value_type.is_execution())
            .filter(|pin| {
                let node = &self.nodes[&pin.node];
                node.kind != NodeKind::MakeColor
                    && !(matches!(node.kind, NodeKind::FunctionCall | NodeKind::ListLiteral)
                        && node.properties.contains_key("input_count"))
                    && !(node.kind == NodeKind::SpriteEffect
                        && node.properties.get(&format!("apply_{}", pin.key))
                            == Some(&PropertyValue::Bool(false)))
            })
            .filter(|pin| {
                !crate::type_inference::has_adaptive_operand_defaults(&self.nodes[&pin.node])
                    && !self.nodes[&pin.node].kind.is_string_table_query()
                    && self.nodes[&pin.node].kind != NodeKind::TextFromStringTable
                    && !matches!(
                        self.nodes[&pin.node].kind,
                        NodeKind::LogicAnd
                            | NodeKind::LogicOr
                            | NodeKind::LogicNot
                            | NodeKind::If
                            | NodeKind::While
                            | NodeKind::ConvertBoolToInt
            | NodeKind::ConvertBoolToFloat
                            | NodeKind::ConvertIntToBool
            | NodeKind::ConvertIntToFloat
                            | NodeKind::ConvertNumberToText
                            | NodeKind::ConvertTextToInt
                            | NodeKind::ConvertStringToText
                            | NodeKind::ConvertTextToString
                            | NodeKind::ConvertStringToName
                            | NodeKind::ConvertNameToString
                    )
            })
            // Le nom affichÃ© par un SET est son identitÃ©, pas une valeur de
            // graphe. UE l'intÃ¨gre au nÅ“ud SET et ne crÃ©e jamais une fausse
            // constante texte reliÃ©e Ã  une broche `name`.
            .filter(|pin| !matches!(self.nodes[&pin.node].kind, NodeKind::SetVariable | NodeKind::LocalVariable))
            // Use paths are static compiler dependencies, kept as authored
            // inline metadata. The exporter reads this exact StringList;
            // extracting it into a runtime literal loses its Script asset type.
            .filter(|pin| !(self.nodes[&pin.node].kind == NodeKind::Use && pin.key == "paths"))
            // Le patron est volontairement Ã©ditÃ© dans le nÅ“ud Format Text,
            // comme dans UE5. Il ne doit pas Ãªtre extrait dans une constante.
            .filter(|pin| {
                !(self.nodes[&pin.node].kind == NodeKind::FormatText && pin.key == "format")
            })
            .filter(|pin| !self.edges.values().any(|edge| edge.input == pin.id))
            .filter_map(|pin| {
                let value = pin.default_value.clone()?;
                let visible = match &value {
                    PropertyValue::String(value) => !value.trim().is_empty(),
                    PropertyValue::Bool(_) | PropertyValue::Int(_) | PropertyValue::Float(_) => {
                        true
                    }
                    PropertyValue::StringList(values) => !values.is_empty(),
                    PropertyValue::Name(_) | PropertyValue::Enum(_) => true,
                };
                visible.then_some((
                    pin.id,
                    pin.node,
                    pin.key.clone(),
                    pin.value_type.clone(),
                    value,
                ))
            })
            .collect();
        let mut owner_slots = BTreeMap::<NodeId, usize>::new();
        for (input, owner, key, value_type, value) in candidates {
            if self.nodes[&owner].kind == NodeKind::Choice && key.ends_with("_condition")
                && (value == PropertyValue::Bool(true)
                    || self.nodes[&owner].properties.get(&crate::choice::default_marker(&key))
                        == Some(&PropertyValue::Bool(true)))
            {
                // The inline availability checkbox is real authoring state,
                // not a constant to extract into another node on reopen.
                continue;
            }
            if self.nodes[&owner].kind == NodeKind::Choice
                && key.ends_with("_condition")
                && value == PropertyValue::Bool(false)
            {
                // Une condition absente signifie Â« option toujours visible Â».
                // Les anciens graphes sÃ©rialisaient `false`, ce qui Ã©tait Ã  la
                // fois trompeur dans lâ€™inspecteur et diffÃ©rent sÃ©mantiquement.
                self.pins.get_mut(&input).unwrap().default_value = None;
                created += 1;
                continue;
            }
            let owner_position = self.nodes[&owner].position;
            let slot = owner_slots.entry(owner).or_default();
            let offset = 112.0 + *slot as f64 * 96.0;
            *slot += 1;
            let kind = match value_type {
                ValueType::InterpolatedText => NodeKind::TextValue,
                ValueType::Character => NodeKind::CharacterValue,
                ValueType::Label => NodeKind::LabelValue,
                ValueType::Position => NodeKind::PositionValue,
                ValueType::Asset(kind) => asset_node_kind(kind),
                ValueType::Transition => transition_kind(&value),
                _ => NodeKind::Literal,
            };
            let source_position = [owner_position[0] - 235.0, owner_position[1] + offset];
            let source = self.add_catalog_node(kind, source_position)?;
            self.place_without_overlap(source);
            match kind {
                NodeKind::StringTableAsset => self.set_property(source, "table", value.clone())?,
                NodeKind::TextValue => self.set_property(source, "value", value.clone())?,
                NodeKind::LabelValue => self.set_property(source, "label", value.clone())?,
                NodeKind::PositionValue => self.set_property(source, "position", value.clone())?,
                NodeKind::CharacterValue => {
                    self.set_property(source, "character", value.clone())?
                }
                NodeKind::SceneAsset
                | NodeKind::SpriteAsset
                | NodeKind::MusicAsset
                | NodeKind::SoundEffectAsset
                | NodeKind::VoiceAsset
                | NodeKind::CinematicAsset
                | NodeKind::HoverImageAsset
                | NodeKind::ScriptAsset => self.set_property(source, "path", value.clone())?,
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
                | NodeKind::TransitionBlur => {
                    if let Some(duration) = transition_duration(&value) {
                        self.set_property(source, "duration_ms", PropertyValue::Int(duration))?;
                    }
                }
                _ => self.set_property(source, "value", value.clone())?,
            }
            let output = self.pin_by_key(source, "value").unwrap().id;
            self.connect(output, input)?;
            self.pins.get_mut(&input).unwrap().default_value = None;
            created += 1;
        }
        created += self.materialize_legacy_sprite_sources()?;
        Ok(created)
    }

    /// Consume an old explicit emotion once; never recreate a disconnected sprite.
    pub fn materialize_legacy_sprite_sources(&mut self) -> Result<usize, GraphEditError> {
        fn source(
            g: &GraphDocument,
            node: NodeId,
            key: &str,
            seen: &mut BTreeSet<NodeId>,
        ) -> Option<NodeId> {
            if !seen.insert(node) {
                return None;
            }
            let pin = g.pin_by_key(node, key)?;
            let edge = g.edges.values().find(|e| e.input == pin.id)?;
            let owner = g.pins.get(&edge.output)?.node;
            if g.nodes[&owner].kind == NodeKind::Reroute {
                source(g, owner, "value", seen)
            } else {
                Some(owner)
            }
        }
        let mut candidates = Vec::new();
        for node in self
            .nodes
            .values()
            .filter(|n| n.kind == NodeKind::SpriteShow)
        {
            let Some(character) = source(self, node.id, "character", &mut BTreeSet::new())
                .filter(|id| self.nodes[id].kind == NodeKind::CharacterValue)
            else {
                continue;
            };
            let Some(PropertyValue::String(id)) =
                self.nodes[&character].properties.get("character")
            else {
                continue;
            };
            let Some(pin) = self.pin_by_key(character, "sprite") else {
                continue;
            };
            if self.edges.values().any(|e| e.input == pin.id) {
                continue;
            }
            let emotion = source(self, node.id, "emotion", &mut BTreeSet::new())
                .and_then(|id| self.nodes[&id].properties.get("value"))
                .or_else(|| {
                    self.pin_by_key(node.id, "emotion")
                        .and_then(|p| p.default_value.as_ref())
                });
            let Some(PropertyValue::String(emotion)) =
                emotion.filter(|v| matches!(v,PropertyValue::String(s) if !s.is_empty()))
            else {
                continue;
            };
            let path = if emotion.contains('/') {
                emotion.clone()
            } else {
                format!("sprites/{id}/{emotion}.png")
            };
            candidates.push((node.id, character, pin.id, path));
        }
        for (show, character, input, path) in &candidates {
            let pos = self.nodes[character].position;
            let image =
                self.add_catalog_node(NodeKind::SpriteAsset, [pos[0] - 280.0, pos[1] + 160.0])?;
            self.set_property(image, "path", PropertyValue::String(path.clone()))?;
            self.place_without_overlap(image);
            let output = self.pin_by_key(image, "value").unwrap().id;
            self.connect(output, *input)?;
            if let Some(pin) = self.pin_by_key(*show, "emotion").map(|p| p.id) {
                // Only sources detached by this exact migration are candidates.
                // A shared Literal, or an already orphaned presentation node,
                // must keep its identity; no general dead-node sweep occurs.
                let retired: BTreeSet<_> = self.edges.values()
                    .filter(|edge| edge.input == pin)
                    .filter_map(|edge| self.pins.get(&edge.output))
                    .map(|output| output.node)
                    .filter(|node| self.nodes[node].kind == NodeKind::Literal)
                    .collect();
                self.disconnect_pin(pin);
                self.pins.get_mut(&pin).unwrap().default_value = None;
                for source in retired {
                    if !self.edges.values().any(|edge|
                        self.pins[&edge.output].node == source || self.pins[&edge.input].node == source)
                    {
                        self.remove_node(source);
                    }
                }
            }
            self.nodes
                .get_mut(character)
                .unwrap()
                .properties
                .remove("emotion");
        }
        Ok(candidates.len())
    }

    /// Donne aux anciens littÃ©raux leur type rÃ©el puis matÃ©rialise les
    /// conversions implicites connues. Les graphes crÃ©Ã©s avant les littÃ©raux
    /// typÃ©s contenaient des sorties `Any`, ce qui masquait notamment les
    /// liaisons Texte â†’ Entier.
    pub fn materialize_implicit_conversions(&mut self) -> Result<usize, GraphEditError> {
        let mut trial = self.clone();
        let mut changed = trial.normalize_literal_pin_types();
        let types = trial.effective_pin_types();
        let candidates: Vec<_> = trial
            .edges
            .values()
            .filter_map(|edge| {
                let output = trial.pins.get(&edge.output)?;
                let input = trial.pins.get(&edge.input)?;
                let source = types.get(&output.id).unwrap_or(&output.value_type);
                let target = trial
                    .pin_constraint_type(input.id)
                    .unwrap_or_else(|| input.value_type.clone());
                if target.accepts(source) || matches!((source, &target),
                    (ValueType::Bool, ValueType::Int | ValueType::Float) | (ValueType::Int, ValueType::Bool)
                    | (ValueType::Name, ValueType::InterpolatedText)) {
                    // Additive Boolean and invariant Name casts are available for
                    // new authored connections, not repairs of old invalid wires.
                    return None;
                }
                target
                    .conversion_path_from(source)
                    .map(|_| (edge.id, edge.output, edge.input))
            })
            .collect();

        for (edge, output, input) in candidates {
            let output_node = trial.pins[&output].node;
            let input_node = trial.pins[&input].node;
            let output_position = trial.nodes[&output_node].position;
            let input_position = trial.nodes[&input_node].position;
            let position = [
                (output_position[0] + input_position[0]) * 0.5,
                (output_position[1] + input_position[1]) * 0.5,
            ];
            trial.remove_edge(edge);
            let connection = trial.connect_with_conversions(output, input, position)?;
            for node in &connection.conversions {
                if matches!(
                    trial.nodes[node].kind,
                    NodeKind::ConvertStringToText | NodeKind::ConvertTextToString
                ) {
                    // Old Text/String wires were representation aliases. Keep
                    // their exact RVN/locale keys while showing the boundary.
                    trial.set_property(*node, "legacy_passthrough", PropertyValue::Bool(true))?;
                }
            }
            // The original destination edge remains identifiable after upgrade.
            let mut restored = trial.edges.remove(&connection.edge).unwrap();
            restored.id = edge;
            trial.edges.insert(edge, restored);
            changed += connection.conversions.len();
        }

        *self = trial;
        Ok(changed)
    }

    /// Supprime uniquement les anciennes rÃ©fÃ©rences de variable gÃ©nÃ©riques.
    /// Une rÃ©fÃ©rence utilisÃ©e comme nom d'un SET est absorbÃ©e par celui-ci ;
    /// une rÃ©fÃ©rence de lecture encore utile devient un vÃ©ritable GET, mais
    /// seulement si sa variable a Ã©tÃ© dÃ©clarÃ©e dans le panneau Variables.
    /// Les constantes restent des nÅ“uds visibles et explicitement branchÃ©s.
    pub fn normalize_legacy_variable_references(&mut self) -> usize {
        let mut changed = 0;
        let references: Vec<_> = self
            .nodes
            .values()
            .filter(|node| node.kind == NodeKind::VariableReference)
            .map(|node| node.id)
            .collect();
        for node in references {
            let name = match self.nodes[&node].properties.get("name") {
                Some(PropertyValue::String(name)) => name.clone(),
                _ => String::new(),
            };
            let output = self.pin_by_key(node, "value").map(|pin| pin.id);
            let set_name_edges: Vec<_> = output
                .into_iter()
                .flat_map(|output| {
                    self.edges
                        .values()
                        .filter(move |edge| edge.output == output)
                })
                .filter_map(|edge| {
                    let input = self.pins.get(&edge.input)?;
                    (input.key == "name"
                        && self.nodes.get(&input.node)?.kind == NodeKind::SetVariable)
                        .then_some((edge.id, input.id, input.node))
                })
                .collect();
            for (edge, input, setter) in &set_name_edges {
                if let Some(setter) = self.nodes.get_mut(setter) {
                    setter
                        .properties
                        .insert("name".into(), PropertyValue::String(name.clone()));
                }
                if let Some(pin) = self.pins.get_mut(input) {
                    pin.default_value = Some(PropertyValue::String(name.clone()));
                }
                self.remove_edge(*edge);
            }

            let has_remaining_edges =
                output.is_some_and(|output| self.edges.values().any(|edge| edge.output == output));
            if has_remaining_edges || set_name_edges.is_empty() {
                if let Some(definition) = self.variables.get(&name).cloned() {
                    self.nodes.get_mut(&node).unwrap().kind = NodeKind::VariableGet;
                    if let Some(output) = output {
                        self.pins.get_mut(&output).unwrap().value_type = definition.value_type;
                    }
                    changed += 1;
                    continue;
                }
            }
            self.remove_node(node);
            changed += 1;
        }
        changed
    }

    /// Clean obsolete defaults on inputs whose values are represented by source
    /// nodes. Inline editable inputs retain their authored fallback while wired,
    /// matching connect(): the wire hides the value and disconnect reveals it.
    pub fn clear_shadowed_input_defaults(&mut self) -> usize {
        let connected_inputs: BTreeSet<_> = self.edges.values().map(|edge| edge.input).collect();
        let mut changed = 0;
        for input in connected_inputs {
            if self.pins.get(&input).is_some_and(|pin| !pin.value_type.is_execution()
                && self.nodes.get(&pin.node).is_some_and(|node| retains_inline_pin_default(node.kind, &pin.value_type))) {
                continue;
            }
            if let Some(pin) = self.pins.get_mut(&input) {
                if pin.default_value.take().is_some() {
                    changed += 1;
                }
            }
        }
        changed
    }

    fn normalize_literal_pin_types(&mut self) -> usize {
        let literals: Vec<_> = self
            .nodes
            .values()
            .filter(|node| node.kind == NodeKind::Literal)
            .filter_map(|node| {
                let value_type = property_value_type(node.properties.get("value")?);
                let output = node.pins.iter().copied().find(|pin_id| {
                    self.pins.get(pin_id).is_some_and(|pin| {
                        pin.direction == PinDirection::Output && pin.key == "value"
                    })
                })?;
                Some((output, value_type))
            })
            .collect();
        let mut changed = 0;
        for (output, value_type) in literals {
            let pin = self.pins.get_mut(&output).unwrap();
            if pin.value_type != value_type {
                pin.value_type = value_type;
                changed += 1;
            }
        }
        changed
    }

    fn normalize_legacy_operators(&mut self) -> usize {
        let replacements: Vec<_> = self
            .nodes
            .values()
            .filter_map(|node| {
                let PropertyValue::String(operator) = node.properties.get("operator")? else {
                    return None;
                };
                let replacement = match (node.kind, operator.as_str()) {
                    (NodeKind::BinaryOperator, "+") => NodeKind::MathAdd,
                    (NodeKind::BinaryOperator, "-") => NodeKind::MathSubtract,
                    (NodeKind::BinaryOperator, "*") => NodeKind::MathMultiply,
                    (NodeKind::BinaryOperator, "/") => NodeKind::MathDivide,
                    (NodeKind::BinaryOperator, "==") => NodeKind::MathEqual,
                    (NodeKind::BinaryOperator, "!=") => NodeKind::MathNotEqual,
                    (NodeKind::BinaryOperator, "<") => NodeKind::MathLess,
                    (NodeKind::BinaryOperator, "<=") => NodeKind::MathLessEqual,
                    (NodeKind::BinaryOperator, ">") => NodeKind::MathGreater,
                    (NodeKind::BinaryOperator, ">=") => NodeKind::MathGreaterEqual,
                    (NodeKind::BinaryOperator, "and") => NodeKind::LogicAnd,
                    (NodeKind::BinaryOperator, "or") => NodeKind::LogicOr,
                    (NodeKind::UnaryOperator, "not") => NodeKind::LogicNot,
                    (NodeKind::UnaryOperator, "-") => NodeKind::MathNegate,
                    _ => return None,
                };
                Some((node.id, replacement))
            })
            .collect();
        for (node, replacement) in &replacements {
            self.nodes.get_mut(node).unwrap().kind = *replacement;
            let definitions = node_definition(*replacement);
            let pin_ids = self.nodes[node].pins.clone();
            for pin in pin_ids {
                let key = self.pins[&pin].key.clone();
                if let Some(definition) = definitions.pins.iter().find(|item| item.key == key) {
                    let graph_pin = self.pins.get_mut(&pin).unwrap();
                    graph_pin.value_type = definition.value_type.clone();
                    graph_pin.label = definition.label.into();
                    if graph_pin.default_value.is_none() {
                        graph_pin.default_value = definition.default_value.clone();
                    }
                }
            }
            self.nodes
                .get_mut(node)
                .unwrap()
                .properties
                .remove("operator");
        }
        replacements.len()
    }

    fn place_without_overlap(&mut self, source: NodeId) {
        const MARGIN: f64 = 18.0;
        let source_size = self.estimated_node_size(source);
        loop {
            let source_position = self.nodes[&source].position;
            let collision_bottom = self
                .nodes
                .keys()
                .copied()
                .filter(|node| *node != source)
                .filter_map(|node| {
                    let other = &self.nodes[&node];
                    let other_size = self.estimated_node_size(node);
                    rectangles_overlap(
                        source_position,
                        source_size,
                        other.position,
                        other_size,
                        MARGIN,
                    )
                    .then_some(other.position[1] + other_size[1] + MARGIN)
                })
                .max_by(f64::total_cmp);
            let Some(next_y) = collision_bottom else {
                break;
            };
            self.nodes.get_mut(&source).unwrap().position[1] = next_y;
        }
    }

    fn estimated_node_size(&self, node: NodeId) -> [f64; 2] {
        let graph_node = &self.nodes[&node];
        let compact = is_compact_node(graph_node.kind);
        let title = graph_node
            .title_override
            .as_deref()
            .unwrap_or_else(|| node_definition(graph_node.kind).title);
        let mut inputs = 0usize;
        let mut outputs = 0usize;
        for pin in graph_node.pins.iter().map(|pin| &self.pins[pin]) {
            if pin.key == "completed" && matches!(graph_node.kind, NodeKind::If | NodeKind::Choice)
            {
                continue;
            }
            match pin.direction {
                PinDirection::Input => inputs += 1,
                PinDirection::Output => outputs += 1,
            }
        }
        let rows = inputs.max(outputs).max(1) as f64;
        if compact {
            [
                (title.chars().count() as f64 * 8.2 + 70.0).clamp(132.0, 238.0),
                (22.0 + rows * 22.0).max(48.0),
            ]
        } else {
            [
                (title.chars().count() as f64 * 6.6 + 52.0).clamp(176.0, 286.0),
                (52.0 + rows * 23.0).max(76.0),
            ]
        }
    }

    /// Ordered outputs are authored pins, not multiple ambiguous edges on one pin.
    /// Resize refuses before mutation; removing a branch removes its exact edges.
    pub fn resize_sequence_outputs(&mut self,node:NodeId,count:usize)->Result<(),GraphEditError>{
        let owner=self.nodes.get(&node).ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind!=NodeKind::Sequence{return Err(GraphEditError::WrongNodeKind{node,expected:NodeKind::Sequence,found:owner.kind});}
        if !(1..=32).contains(&count){return Err(GraphEditError::InvalidSequenceOutputCount(count));}
        for index in 0..count {let key=format!("then_{index}");if self.pin_by_key(node,&key).is_none(){self.add_pin(node,key,format!("Puis {index}"),PinDirection::Output,ValueType::Execution,PinCardinality::One)?;}}
        let removed:BTreeSet<_>=self.nodes[&node].pins.iter().copied().filter(|id|self.pins[id].key.strip_prefix("then_").and_then(|s|s.parse::<usize>().ok()).is_some_and(|i|i>=count)).collect();
        self.nodes.get_mut(&node).unwrap().pins.retain(|id|!removed.contains(id));
        self.remove_edges_touching_pins(&removed);
        self.pins.retain(|id,_|!removed.contains(id));
        let mut pins=self.nodes[&node].pins.clone();
        pins.sort_by_key(|id|{let key=&self.pins[id].key;if key=="exec_in"{0}else if let Some(i)=key.strip_prefix("then_").and_then(|s|s.parse::<usize>().ok()){i+1}else{33}});
        self.nodes.get_mut(&node).unwrap().pins=pins;
        self.set_property(node,"output_count",PropertyValue::Int(count as i64))?;
        Ok(())
    }

    // Validate the complete ordered branch set before changing any endpoint.
    // Legacy one-output Sequences remain loadable/resizable; the contextual
    // removal gesture deliberately keeps the two-output creation baseline.
    fn sequence_output_removal(&self, pin: PinId) -> Result<(NodeId, Vec<PinId>), GraphEditError> {
        let target = self.pins.get(&pin).ok_or(GraphEditError::PinNotFound(pin))?;
        let node = target.node;
        let owner = self.nodes.get(&node).ok_or(GraphEditError::NodeNotFound(node))?;
        if owner.kind != NodeKind::Sequence {
            return Err(GraphEditError::WrongNodeKind { node, expected: NodeKind::Sequence, found: owner.kind });
        }
        let count = match owner.properties.get("output_count") {
            Some(PropertyValue::Int(count)) if (1..=32).contains(count) => *count as usize,
            _ => return Err(GraphEditError::MalformedSequenceOutputs(node)),
        };
        let mut seen = BTreeSet::new();
        let mut outputs = BTreeMap::new();
        for id in &owner.pins {
            if !seen.insert(*id) { return Err(GraphEditError::MalformedSequenceOutputs(node)); }
            let port = self.pins.get(id).filter(|port| port.id == *id && port.node == node)
                .ok_or(GraphEditError::MalformedSequenceOutputs(node))?;
            if let Some(suffix) = port.key.strip_prefix("then_") {
                let index = suffix.parse::<usize>().ok().filter(|index|
                    *index < count && port.key == format!("then_{index}"))
                    .ok_or(GraphEditError::MalformedSequenceOutputs(node))?;
                // Catalogue outputs retain Many; resized outputs use One.
                // Both are valid authored execution ports. Ordered identity,
                // ownership, direction and type define branch removal.
                if port.direction != PinDirection::Output || port.value_type != ValueType::Execution
                    || outputs.insert(index, *id).is_some() {
                    return Err(GraphEditError::MalformedSequenceOutputs(node));
                }
            }
        }
        if outputs.len() != count || self.pins.values().any(|port|
            port.node == node && port.key.starts_with("then_") && !seen.contains(&port.id)) {
            return Err(GraphEditError::MalformedSequenceOutputs(node));
        }
        let outputs: Vec<_> = outputs.into_values().collect();
        if !outputs.contains(&pin) { return Err(GraphEditError::InvalidSequenceOutputPin(pin)); }
        if count <= 2 { return Err(GraphEditError::SequenceOutputRemovalMinimum { node, count }); }
        Ok((node, outputs))
    }

    pub fn can_remove_sequence_output(&self, pin: PinId) -> bool {
        self.sequence_output_removal(pin).is_ok()
    }

    /// Remove exactly this branch, not the last branch. Surviving PinIds and
    /// EdgeIds keep their authored endpoints/defaults while names compact.
    /// Every fallible admission precedes mutation; no load migration is added.
    pub fn remove_sequence_output(&mut self, pin: PinId) -> Result<(), GraphEditError> {
        let (node, outputs) = self.sequence_output_removal(pin)?;
        self.remove_edges_touching_pins(&BTreeSet::from([pin]));
        self.nodes.get_mut(&node).unwrap().pins.retain(|id| *id != pin);
        self.pins.remove(&pin);
        for (index, id) in outputs.iter().copied().filter(|id| *id != pin).enumerate() {
            let port = self.pins.get_mut(&id).unwrap();
            port.key = format!("then_{index}");
            port.label = format!("Puis {index}");
        }
        // Keep input/completed and any retained metadata untouched. The same
        // authored branch order is now expressed by the compacted keys.
        let mut pins = self.nodes[&node].pins.clone();
        pins.sort_by_key(|id| { let key = &self.pins[id].key;
            if key == "exec_in" { 0 } else if let Some(index) = key.strip_prefix("then_").and_then(|value| value.parse::<usize>().ok()) { index + 1 } else { 33 }
        });
        let owner = self.nodes.get_mut(&node).unwrap();
        owner.pins = pins;
        owner.properties.insert("output_count".into(), PropertyValue::Int((outputs.len() - 1) as i64));
        Ok(())
    }

    pub fn add_branch_end(
        &mut self,
        owner: NodeId,
        position: [f64; 2],
    ) -> Result<NodeId, GraphEditError> {
        if !self.nodes.contains_key(&owner) {
            return Err(GraphEditError::NodeNotFound(owner));
        }
        let branch_end = self.add_catalog_node(NodeKind::BranchEnd, position)?;
        self.set_property(branch_end, "owner", PropertyValue::Int(owner.get() as i64))?;
        Ok(branch_end)
    }

    pub fn connect(&mut self, output: PinId, input: PinId) -> Result<EdgeId, GraphEditError> {
        let output_pin = self
            .pins
            .get(&output)
            .ok_or(GraphEditError::PinNotFound(output))?;
        let input_pin = self
            .pins
            .get(&input)
            .ok_or(GraphEditError::PinNotFound(input))?;
        if input_pin.text_table_reference.is_some() {
            // Changing a referenced Format default into a wire needs the
            // current reference projection to choose its argument transition.
            // The editor must prepare that explicit transaction first.
            return Err(GraphEditError::InvalidPropertyType { node: input_pin.node, key: input_pin.key.clone() });
        }
        if output_pin.direction != PinDirection::Output
            || input_pin.direction != PinDirection::Input
        {
            return Err(GraphEditError::InvalidDirection { output, input });
        }
        let contextual = matches!(
            crate::blueprint_policy::blueprint_operand_domain(&self.nodes[&input_pin.node]),
            Some(crate::blueprint_policy::BlueprintOperandDomain::Equality)
        ) || crate::enum_graph1026::is_equality1026(&self.nodes[&input_pin.node]);
        let types = if contextual {
            self.effective_pin_types()
        } else {
            BTreeMap::new()
        };
        let output_type = if self.nodes[&output_pin.node].kind==NodeKind::Reroute {
            self.connection_output_type(output).unwrap_or(ValueType::Any)
        } else {
            types.get(&output).cloned()
                .unwrap_or_else(|| self.effective_pin_type(output).unwrap_or(ValueType::Any))
        };
        let input_type = self.pin_constraint_type(input).unwrap_or(ValueType::Any);
        if !self.accepts_pin_source_with_types(input, &output_type, &types)
            && !self.reroute_wildcard_exec_connection(output, input) {
            return Err(GraphEditError::IncompatibleTypes {
                output: output_type,
                input: input_type,
            });
        }
        if self
            .edges
            .values()
            .any(|edge| edge.output == output && edge.input == input)
        {
            return Err(GraphEditError::DuplicateEdge { output, input });
        }
        if input_pin.cardinality == PinCardinality::One
            && self.edges.values().any(|edge| edge.input == input)
        {
            return Err(GraphEditError::InputAlreadyConnected(input));
        }
        let execution = output_type.is_execution()
            || self.reroute_wildcard_exec_connection(output,input);
        let reroute_seeds=[output_pin.node,input_pin.node];
        if execution && !self.reroute_exec_connection_is_resolvable(output,input) {
            // Report the unresolved prospective cable as wildcard; never
            // accept an Exec edge that cannot remain Exec after the edit.
            return Err(GraphEditError::IncompatibleTypes {
                output:if self.nodes[&output_pin.node].kind==NodeKind::Reroute {ValueType::Any}else{output_type},
                input:if self.nodes[&input_pin.node].kind==NodeKind::Reroute {ValueType::Any}else{input_type},
            });
        }
        if !execution && self.data_path_exists(input_pin.node, output_pin.node)
        {
            return Err(GraphEditError::DataCycle {
                from: output_pin.node,
                to: input_pin.node,
            });
        }

        let clear_deferred_pattern = self.nodes[&input_pin.node].kind == NodeKind::FormatText
            && input_pin.key == "format"
            && self.nodes[&input_pin.node].properties.get("deferred_text") == Some(&PropertyValue::Bool(true))
            && matches!(&input_pin.default_value, Some(PropertyValue::String(source)) if !source.is_empty());
        let id = EdgeId::new(self.next_edge_id);
        self.next_edge_id += 1;
        self.edges.insert(id, GraphEdge { id, output, input });
        let input_pin = &self.pins[&input];
        let inline_default = retains_inline_pin_default(self.nodes[&input_pin.node].kind, &input_pin.value_type);
        // A wire takes precedence during evaluation. Retain an inline default
        // while hidden, so disconnect restores it rather than deleting a value
        // merely because the user tried a connection.
        if !self.pins[&input].value_type.is_execution() && !inline_default {
            self.pins.get_mut(&input).unwrap().default_value = None;
            self.pins.get_mut(&input).unwrap().text_localization = None;
        }
        let input_node = self.pins[&input].node;
        if self.nodes[&input_node].kind == NodeKind::FormatText && self.pins[&input].key == "format"
            && self.nodes[&input_node].properties.get("deferred_text") == Some(&PropertyValue::Bool(true)) {
            if clear_deferred_pattern {
            let removed: BTreeSet<_> = self.nodes[&input_node].pins.iter().copied()
                .filter(|p| self.pins[p].key.starts_with("argument_")).collect();
            self.nodes.get_mut(&input_node).unwrap().pins.retain(|p| !removed.contains(p));
            self.remove_edges_touching_pins(&removed);
            self.pins.retain(|p, _| !removed.contains(p));
            }
            // The pattern pin remains a typed empty default behind its wire.
            self.pins.get_mut(&input).unwrap().default_value = Some(PropertyValue::String(String::new()));
            self.pins.get_mut(&input).unwrap().text_localization = None;
        }
        self.refresh_reroute_components(&reroute_seeds);
        Ok(id)
    }

    /// Materialize one finite conversion spec with genuine catalogue pins.
    /// Callers compose this on their transaction candidate, just like ordinary
    /// add_catalog_node. No source identity or runtime authority is supplied.
    pub fn add_conversion_node(&mut self, spec: crate::ConversionSpec, position: [f64; 2])
        -> Result<(NodeId, PinId, PinId), GraphEditError>
    {
        let node = self.add_catalog_node(spec.node_kind(), position)?;
        if let Some(function) = spec.function() {
            self.set_property(node, "function", PropertyValue::String(function.into()))?;
            self.resize_value_inputs(node, 1)?;
        }
        let input = self.pin_by_key(node, spec.input_key()).ok_or_else(|| GraphEditError::PinKeyNotFound {
            node, key: spec.input_key().into(),
        })?.id;
        let output = self.pin_by_key(node, "result").ok_or_else(|| GraphEditError::PinKeyNotFound {
            node, key: "result".into(),
        })?.id;
        Ok((node, input, output))
    }

    /// Connect distinct data types through visible conversion nodes. The whole
    /// change is atomic: invalid directions, cycles and occupied inputs leave
    /// the original graph and its identity counters untouched.
    pub fn connect_with_conversions(
        &mut self,
        output: PinId,
        input: PinId,
        position: [f64; 2],
    ) -> Result<ConvertedConnection, GraphEditError> {
        let output_pin = self
            .pins
            .get(&output)
            .ok_or(GraphEditError::PinNotFound(output))?;
        let input_pin = self
            .pins
            .get(&input)
            .ok_or(GraphEditError::PinNotFound(input))?;
        if output_pin.direction != PinDirection::Output
            || input_pin.direction != PinDirection::Input
        {
            return Err(GraphEditError::InvalidDirection { output, input });
        }
        let output_type = self.connection_output_type(output).unwrap_or(ValueType::Any);
        let input_type = self.pin_constraint_type(input).unwrap_or(ValueType::Any);
        let needs_precision_cast = input_type == ValueType::Float
            && output_type == ValueType::Int
            && self.nodes[&input_pin.node].kind != NodeKind::ConvertNumberToText;
        if (input_type.accepts(&output_type) || self.reroute_wildcard_exec_connection(output,input)) && !needs_precision_cast {
            let edge = self.connect(output, input)?;
            return Ok(ConvertedConnection {
                edge,
                conversions: Vec::new(),
            });
        }
        let path = input_type
            .conversion_specs_from(&output_type)
            .ok_or_else(|| GraphEditError::IncompatibleTypes {
                output: output_type.clone(),
                input: input_type.clone(),
            })?;
        let mut trial = self.clone();
        let mut tail = output;
        let mut conversions = Vec::new();
        for (index, spec) in path.into_iter().enumerate() {
            let (converter, converter_input, converter_output) = trial.add_conversion_node(
                spec, [position[0] + index as f64 * 180.0, position[1]])?;
            trial.connect(tail, converter_input)?;
            conversions.push(converter);
            tail = converter_output;
        }
        let edge = trial.connect(tail, input)?;
        *self = trial;
        Ok(ConvertedConnection { edge, conversions })
    }

    pub fn remove_node(&mut self, node: NodeId) -> Option<GraphNode> {
        let removed = self.nodes.remove(&node)?;
        if let Some(metadata)=&mut self.library1027{metadata.calls.remove(&node);}
        let removed_pins: BTreeSet<_> = removed.pins.iter().copied().collect();
        let edges: Vec<_> = self.edges.values().filter(|e| removed_pins.contains(&e.output)
            || removed_pins.contains(&e.input)).map(|e| e.id).collect();
        for edge in edges { self.remove_edge(edge); }
        self.pins.retain(|id, _| !removed_pins.contains(id));
        self.edges.retain(|_, edge| {
            !removed_pins.contains(&edge.output) && !removed_pins.contains(&edge.input)
        });
        Some(removed)
    }

    /// Disconnect genuine edges before deleting or replacing their pins. All
    /// affected reroute components follow the same explicit edit/Undo boundary.
    pub fn disconnect_pin(&mut self, pin: PinId) -> usize {
        self.remove_edges_touching_pins(&BTreeSet::from([pin]))
    }

    fn remove_edges_touching_pins(&mut self, pins: &BTreeSet<PinId>) -> usize {
        let ids: Vec<_>=self.edges.values().filter(|e|pins.contains(&e.input)||pins.contains(&e.output)).map(|e|e.id).collect();
        let count=ids.len();
        for id in ids { self.remove_edge(id); }
        count
    }

    pub fn remove_edge(&mut self, edge: EdgeId) -> Option<GraphEdge> {
        let removed = self.edges.remove(&edge)?;
        let seeds: Vec<_>=[removed.output,removed.input].into_iter()
            .filter_map(|id|self.pins.get(&id).map(|p|p.node)).collect();
        self.refresh_reroute_components(&seeds);
        if let Some(pin) = self.pins.get_mut(&removed.input) {
            if self.nodes.get(&pin.node).is_some_and(|node| node.kind == NodeKind::FormatText
                && node.properties.get("deferred_text") == Some(&PropertyValue::Bool(true)))
                && pin.key == "format" && !self.edges.values().any(|e| e.input == pin.id) {
                pin.default_value = Some(PropertyValue::String(String::new()));
                pin.text_localization = Some(rvn_parser::localized_text::TextLocalization::Empty);
            }
        }
        Some(removed)
    }

    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Upgrade normalized RGB inputs without changing their rendered color.
    /// Connected sources are left intact and explicitly scaled at the color input.
    pub fn upgrade_color_range(&mut self) -> Result<usize, GraphEditError> {
        let legacy: Vec<_> = self
            .nodes
            .values()
            .filter(|n| {
                n.kind == NodeKind::MakeColor
                    && n.properties.get("rgb_max") != Some(&PropertyValue::Int(255))
            })
            .map(|n| n.id)
            .collect();
        for node in &legacy {
            for (index, key) in ["r", "g", "b"].into_iter().enumerate() {
                let Some(pin) = self.pin_by_key(*node, key).map(|p| p.id) else {
                    continue;
                };
                if let Some(edge) = self.edges.values().find(|e| e.input == pin).cloned() {
                    let pos = self.nodes[node].position;
                    let scale = self.add_catalog_node(
                        NodeKind::MathMultiply,
                        [pos[0] - 220.0, pos[1] + index as f64 * 90.0],
                    )?;
                    let left = self.pin_by_key(scale, "left").unwrap().id;
                    let right = self.pin_by_key(scale, "right").unwrap().id;
                    let result = self.pin_by_key(scale, "value").unwrap().id;
                    self.pins.get_mut(&right).unwrap().default_value =
                        Some(PropertyValue::Float(255.0));
                    self.remove_edge(edge.id);
                    self.connect(edge.output, left)?;
                    self.connect(result, pin)?;
                } else if let Some(value) = &mut self.pins.get_mut(&pin).unwrap().default_value {
                    *value = match value {
                        PropertyValue::Float(n) => PropertyValue::Float(*n * 255.0),
                        PropertyValue::Int(n) => PropertyValue::Float(*n as f64 * 255.0),
                        _ => value.clone(),
                    };
                }
            }
            self.set_property(*node, "rgb_max", PropertyValue::Int(255))?;
        }
        Ok(legacy.len())
    }

    pub fn from_json(source: &str) -> Result<Self, GraphLoadError> {
        Self::from_json_with_report(source).map(|(graph, _)| graph)
    }

    /// Add the value returned by a local assignment to old presentation caches.
    /// Existing IDs, wires, positions and defaults are left intact. The new
    /// output reads the assigned variable; it never re-evaluates its input.
    /// This is additive and idempotent, including schema-current caches made
    /// before LocalVariable exposed the same value output as SetVariable.
    pub fn normalize_assignment_value_outputs(&mut self) -> Result<usize, GraphEditError> {
        let missing: Vec<_> = self.nodes.values()
            .filter(|node| node.kind == NodeKind::LocalVariable)
            .filter(|node| self.pin_by_key(node.id, "value_out").is_none())
            .map(|node| (node.id, self.pin_by_key(node.id, "value")
                .map(|pin| pin.value_type.clone()).unwrap_or(ValueType::Any)))
            .collect();
        for (node, value_type) in &missing {
            self.add_pin(*node, "value_out", "", PinDirection::Output,
                value_type.clone(), PinCardinality::Many)?;
        }
        Ok(missing.len())
    }

    pub fn from_json_with_report(
        source: &str,
    ) -> Result<(Self, crate::MigrationReport), GraphLoadError> {
        let value = serde_json::from_str(source).map_err(GraphLoadError::Json)?;
        let (value, mut report) = crate::migrate_graph_value(value).map_err(|found| {
            GraphLoadError::UnsupportedSchema {
                found,
                supported: GRAPH_MAX_SUPPORTED_SCHEMA_VERSION,
            }
        })?;
        let mut graph: Self = serde_json::from_value(value).map_err(GraphLoadError::Json)?;
        graph.validate_narrative_metadata().map_err(GraphLoadError::InvalidNarrativeMetadata)?;
        graph.next_node_id = graph.next_node_id.max(
            graph
                .nodes
                .keys()
                .map(|id| id.get().saturating_add(1))
                .max()
                .unwrap_or(1),
        );
        graph.next_pin_id = graph.next_pin_id.max(
            graph
                .pins
                .keys()
                .map(|id| id.get().saturating_add(1))
                .max()
                .unwrap_or(1),
        );
        graph.next_edge_id = graph.next_edge_id.max(
            graph
                .edges
                .keys()
                .map(|id| id.get().saturating_add(1))
                .max()
                .unwrap_or(1),
        );
        let legacy_characters: Vec<_> = graph
            .nodes
            .values()
            .filter(|node| {
                node.kind == NodeKind::CharacterValue
                    && graph.pin_by_key(node.id, "sprite").is_none()
            })
            .map(|node| node.id)
            .collect();
        for node in legacy_characters {
            let _ = graph.add_pin(
                node,
                "sprite",
                "Sprite",
                PinDirection::Input,
                ValueType::Asset(AssetKind::Sprite),
                PinCardinality::One,
            );
        }
        let legacy_calls: Vec<_> = graph
            .nodes
            .values()
            .filter(|node| {
                node.kind == NodeKind::Call && graph.pin_by_key(node.id, "exec_out").is_none()
            })
            .map(|node| node.id)
            .collect();
        for node in legacy_calls {
            graph
                .add_pin(
                    node,
                    "exec_out",
                    "AprÃ¨s retour",
                    PinDirection::Output,
                    ValueType::Execution,
                    PinCardinality::Many,
                )
                .expect("existing Call node");
        }
        let legacy_music: Vec<_> = graph
            .nodes
            .values()
            .filter(|node| {
                node.kind == NodeKind::MusicStop
                    && graph.pin_by_key(node.id, "transition").is_none()
            })
            .map(|node| node.id)
            .collect();
        for node in legacy_music {
            let id = graph
                .add_pin(
                    node,
                    "transition",
                    "Transition",
                    PinDirection::Input,
                    ValueType::Transition,
                    PinCardinality::One,
                )
                .expect("existing MusicStop node");
            graph.pins.get_mut(&id).unwrap().default_value =
                Some(PropertyValue::String("none".into()));
        }
        if graph.normalize_assignment_value_outputs().map_err(GraphLoadError::Migration)? > 0 {
            report.steps.push("assignment_value_outputs");
        }
        if report.from < 3 {
            graph
                .materialize_implicit_conversions()
                .map_err(GraphLoadError::Migration)?;
        }
        Ok((graph, report))
    }

    pub fn validate(&self) -> Vec<GraphDiagnostic> {
        crate::validate_document(self)
    }

    fn data_path_exists(&self, start: NodeId, target: NodeId) -> bool {
        let types=self.effective_pin_types();
        let mut pending = vec![start];
        let mut visited = BTreeSet::new();
        while let Some(node) = pending.pop() {
            if node == target {
                return true;
            }
            if !visited.insert(node) {
                continue;
            }
            for edge in self.edges.values() {
                let Some(output) = self.pins.get(&edge.output) else {
                    continue;
                };
                let Some(input) = self.pins.get(&edge.input) else {
                    continue;
                };
                if output.node == node && !types.get(&output.id).unwrap_or(&output.value_type).is_execution() {
                    pending.push(input.node);
                }
            }
        }
        false
    }
}

fn hotspot_name(value: &str) -> String {
    crate::imagemap_spec::label(value)
}

fn property_value_type(value: &PropertyValue) -> ValueType {
    match value {
        PropertyValue::Bool(_) => ValueType::Bool,
        PropertyValue::Int(_) => ValueType::Int,
        PropertyValue::Float(_) => ValueType::Float,
        PropertyValue::String(_) => ValueType::String,
        PropertyValue::Name(_) => ValueType::Name,
        PropertyValue::Enum(value) => ValueType::Enum(value.asset),
        PropertyValue::StringList(_) => ValueType::List(Box::new(ValueType::String)),
    }
}

pub(crate) fn transition_kind(value: &PropertyValue) -> NodeKind {
    let PropertyValue::String(value) = value else {
        return NodeKind::TransitionNone;
    };
    match value.split(['(', ':']).next().unwrap_or(value).trim() {
        "fade" => NodeKind::TransitionFade,
        "dissolve" => NodeKind::TransitionDissolve,
        "slideleft" | "slide_left" => NodeKind::TransitionSlideLeft,
        "slideright" | "slide_right" => NodeKind::TransitionSlideRight,
        "slideup" | "slide_up" => NodeKind::TransitionSlideUp,
        "slidedown" | "slide_down" => NodeKind::TransitionSlideDown,
        "zoomin" | "zoom_in" => NodeKind::TransitionZoomIn,
        "zoomout" | "zoom_out" => NodeKind::TransitionZoomOut,
        "wipe" => NodeKind::TransitionWipe,
        "blur" => NodeKind::TransitionBlur,
        _ => NodeKind::TransitionNone,
    }
}

pub(crate) fn transition_duration(value: &PropertyValue) -> Option<i64> {
    let PropertyValue::String(value) = value else {
        return None;
    };
    let start = value.find('(')? + 1;
    let digits: String = value[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

fn format_text_arguments(pattern: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    let mut chars = pattern.char_indices().peekable();
    while let Some((_, character)) = chars.next() {
        // Unreal utilise l'accent grave pour Ã©chapper les marqueurs de format.
        if character == '`' {
            chars.next();
            continue;
        }
        if character != '{' {
            continue;
        }
        let start = chars
            .peek()
            .map(|(index, _)| *index)
            .unwrap_or(pattern.len());
        let mut end = None;
        while let Some((index, next)) = chars.next() {
            if next == '}' {
                end = Some(index);
                break;
            }
            if next == '{' {
                break;
            }
        }
        let Some(end) = end else { continue };
        let name = pattern[start..end].trim();
        let valid = !name.is_empty()
            && name
                .chars()
                .next()
                .is_some_and(|first| first == '_' || first.is_alphabetic())
            && name
                .chars()
                .all(|value| value == '_' || value.is_alphanumeric());
        if valid && !arguments.iter().any(|argument| argument == name) {
            arguments.push(name.to_owned());
        }
    }
    arguments
}

fn is_compact_node(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Literal
            | NodeKind::VariableGet
            | NodeKind::Reroute
            | NodeKind::ConvertBoolToInt
            | NodeKind::ConvertBoolToFloat
            | NodeKind::ConvertIntToBool
            | NodeKind::ConvertIntToFloat
            | NodeKind::ConvertNumberToText
            | NodeKind::ConvertTextToInt
            | NodeKind::ConvertTextToString
            | NodeKind::BinaryOperator
            | NodeKind::ConvertStringToName
            | NodeKind::ConvertNameToString
            | NodeKind::UnaryOperator
            | NodeKind::TextValue
            | NodeKind::SceneAsset
            | NodeKind::TransitionNone
            | NodeKind::TransitionFade
            | NodeKind::TransitionDissolve
            | NodeKind::TransitionSlideLeft
            | NodeKind::TransitionSlideRight
            | NodeKind::TransitionSlideUp
            | NodeKind::TransitionSlideDown
            | NodeKind::TransitionZoomIn
            | NodeKind::TransitionZoomOut
            | NodeKind::TransitionWipe
            | NodeKind::TransitionBlur
            | NodeKind::VariableReference
            | NodeKind::MathAdd
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
            | NodeKind::LogicOr
            | NodeKind::LogicNot
            | NodeKind::MathNegate
    )
}

fn rectangles_overlap(
    first_position: [f64; 2],
    first_size: [f64; 2],
    second_position: [f64; 2],
    second_size: [f64; 2],
    margin: f64,
) -> bool {
    first_position[0] < second_position[0] + second_size[0] + margin
        && first_position[0] + first_size[0] + margin > second_position[0]
        && first_position[1] < second_position[1] + second_size[1] + margin
        && first_position[1] + first_size[1] + margin > second_position[1]
}

#[derive(Debug)]
pub enum GraphLoadError {
    Json(serde_json::Error),
    Migration(GraphEditError),
    UnsupportedSchema { found: u32, supported: u32 },
    InvalidNarrativeMetadata(String),
}

impl std::fmt::Display for GraphLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => error.fmt(formatter),
            Self::Migration(error) => write!(formatter, "graph migration failed: {error}"),
            Self::InvalidNarrativeMetadata(error) => write!(formatter, "invalid narrative metadata: {error}"),
            Self::UnsupportedSchema { found, supported } => write!(
                formatter,
                "unsupported graph schema {found}; this build supports schema {supported}"
            ),
        }
    }
}

impl std::error::Error for GraphLoadError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphEditError {
    InvalidVariableName(String),
    InvalidReferenceExpression {
        node: NodeId,
        key: String,
        message: String,
    },
    NodeNotFound(NodeId),
    PinNotFound(PinId),
    InvalidDirection {
        output: PinId,
        input: PinId,
    },
    IncompatibleTypes {
        output: ValueType,
        input: ValueType,
    },
    DuplicateEdge {
        output: PinId,
        input: PinId,
    },
    InputAlreadyConnected(PinId),
    DataCycle {
        from: NodeId,
        to: NodeId,
    },
    DuplicatePinKey {
        node: NodeId,
        key: String,
    },
    PinKeyNotFound {
        node: NodeId,
        key: String,
    },
    WrongNodeKind {
        node: NodeId,
        expected: NodeKind,
        found: NodeKind,
    },
    InvalidPropertyType {
        node: NodeId,
        key: String,
    },
    InvalidSequenceOutputCount(usize),
    InvalidSequenceOutputPin(PinId),
    MalformedSequenceOutputs(NodeId),
    SequenceOutputRemovalMinimum { node: NodeId, count: usize },
    UnsupportedDynamicOperand(NodeId),
    UnsupportedBlueprintOperator(NodeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoiceOptionPins {
    pub index: usize,
    pub branch: PinId,
    pub condition: PinId,
}

impl std::fmt::Display for GraphEditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for GraphEditError {}

/// Editor presentation attached to a stable node identity. It never emits RVN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeAnnotation {
    pub text: String,
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub pinned: bool,
}

impl GraphNode {
    pub fn table_query_contract(&self) -> crate::StringTableQueryContract {
        self.string_table_contract.unwrap_or(crate::StringTableQueryContract::LegacyStringV1)
    }
    pub fn string_table_query(&self) -> Option<(&'static str, &'static [&'static str], ValueType)> {
        self.kind.string_table_query_with_contract(self.table_query_contract())
    }

    /// Read old comments without rewriting an existing document on open.
    pub fn effective_annotation(&self) -> Option<NodeAnnotation> {
        self.annotation.clone().or_else(|| {
            match self.properties.get("comment") {
                Some(PropertyValue::String(text)) => Some(NodeAnnotation {
                    text: text.clone(), visible: true, pinned: false,
                }),
                _ => None,
            }
        })
    }
}

fn semantic_node_properties(node: &GraphNode) -> BTreeMap<String, PropertyValue> {
    node.properties.iter().filter(|(key, _)| key.as_str() != "comment")
        .map(|(key, value)| (key.clone(), value.clone())).collect()
}


#[cfg(test)]
mod legacy_sprite_retired_literal_tests1002 {
    use super::*;
    fn imported(source: &str) -> GraphDocument {
        let script = rvn_parser::parse(source).unwrap();
        crate::import_script(&script).unwrap().into_iter()
            .find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap()
    }
    fn bare_show() -> (GraphDocument, NodeId) {
        let graph = imported("scene label start\nhero.show() at center\n\"Ready\"\nreturn\n");
        let show = graph.nodes.values().find(|node| node.kind == NodeKind::SpriteShow).unwrap().id;
        (graph, show)
    }
    fn connect(graph: &mut GraphDocument, from: NodeId, output: &str, to: NodeId, input: &str) {
        graph.connect(graph.pin_by_key(from, output).unwrap().id,
            graph.pin_by_key(to, input).unwrap().id).unwrap();
    }
    #[test]
    fn legacy_sprite1002_actual_import_retires_only_emotion_literal_and_native_roundtrips() {
        let mut graph = imported("scene label start\nhero.show(\"sprites/hero/default.png\") at center\n\"Ready\"\nreturn\n");
        assert!(graph.nodes.values().any(|node| node.kind == NodeKind::SpriteAsset
            && node.properties.get("path") == Some(&PropertyValue::String("sprites/hero/default.png".into()))));
        assert!(!graph.nodes.values().any(|node| node.kind == NodeKind::Literal
            && node.properties.get("value") == Some(&PropertyValue::String("sprites/hero/default.png".into()))));
        graph.enable_native_frame_preparation().unwrap();
        let compiled = crate::prepare_native_frame_graph(&graph).unwrap();
        assert!(compiled.source.contains("sprites/hero/default.png"));
    }
    #[test]
    fn legacy_sprite1002_migration_keeps_shared_literal_and_existing_orphan_identities() {
        let (mut graph, show) = bare_show();
        let literal = graph.add_catalog_node(NodeKind::Literal, [0., 0.]).unwrap();
        graph.set_property(literal, "value", PropertyValue::String("sprites/hero/shared.png".into())).unwrap();
        connect(&mut graph, literal, "value", show, "emotion");
        let convert = graph.add_catalog_node(NodeKind::ConvertStringToText, [0., 100.]).unwrap();
        connect(&mut graph, literal, "value", convert, "value");
        let shared = graph.edges.values().find(|edge| graph.pins[&edge.input].node == convert).unwrap().clone();
        let orphan = graph.add_catalog_node(NodeKind::Literal, [0., 200.]).unwrap();
        graph.set_property(orphan, "value", PropertyValue::String("already-orphaned".into())).unwrap();
        let orphan_node = graph.nodes[&orphan].clone();
        let orphan_pin = graph.pin_by_key(orphan, "value").unwrap().clone();
        assert_eq!(graph.materialize_legacy_sprite_sources().unwrap(), 1);
        assert_eq!(graph.nodes[&orphan], orphan_node);
        assert_eq!(graph.pins[&orphan_pin.id], orphan_pin);
        assert!(graph.nodes.contains_key(&literal));
        assert_eq!(graph.edges[&shared.id], shared);
        assert!(!graph.edges.values().any(|edge| edge.input == graph.pin_by_key(show, "emotion").unwrap().id));
        assert_eq!(graph.materialize_legacy_sprite_sources().unwrap(), 0);
        assert!(graph.nodes.contains_key(&literal) && graph.nodes.contains_key(&orphan));
    }
    #[test]
    fn legacy_sprite1002_direct_mono_use_retirement_removes_its_pin_without_sweeping_neighbors() {
        let (mut graph, show) = bare_show();
        let literal = graph.add_catalog_node(NodeKind::Literal, [0., 0.]).unwrap();
        graph.set_property(literal, "value", PropertyValue::String("sprites/hero/mono.png".into())).unwrap();
        let pin = graph.pin_by_key(literal, "value").unwrap().id;
        connect(&mut graph, literal, "value", show, "emotion");
        assert_eq!(graph.materialize_legacy_sprite_sources().unwrap(), 1);
        assert!(!graph.nodes.contains_key(&literal) && !graph.pins.contains_key(&pin));
        assert!(graph.nodes.contains_key(&show));
        assert!(graph.edges.values().all(|edge| graph.pins.contains_key(&edge.output) && graph.pins.contains_key(&edge.input)));
    }
}


#[cfg(test)]
mod native_asset_identity_tests1021 {
    use super::*;
    #[test]
    fn native_source1021_scene_inline_asset_default_reconstructs_after_actual_disconnect() {
        fn scope_bytes(source:&str,name:&str)->String {
            let parsed=rvn_parser::parse_with_origins(source,rvn_parser::NarrativeContext::ExplicitScenes).unwrap();
            let index=parsed.script.iter().position(|s|matches!(s,rvn_parser::Statement::Function{name:actual,..}|rvn_parser::Statement::Label{name:actual}|rvn_parser::Statement::SceneLabel{name:actual}if actual==name)).unwrap();
            let end=if matches!(parsed.script[index],rvn_parser::Statement::Function{..}) {parsed.spans[index].range.end}
                else {parsed.script.iter().enumerate().skip(index+1).find(|(_,s)|matches!(s,rvn_parser::Statement::Label{..}|rvn_parser::Statement::SceneLabel{..})).map(|(i,_)|parsed.spans[i].range.start).unwrap_or(source.len())};
            source[parsed.spans[index].range.start..end].to_owned()
        }
        // The reserved nonempty sibling forces the real exporter to select its
        // collision suffix; it must remain authored content, not be discarded.
        for sibling in ["spare","__blueprint_project_end"] {
            let raw=format!("function sibling(v){{return v}}\nscene label start\nset asset_probe=random(1,9)\n\"before\"\nscene \"background_B.png\"\n\"after\"\nscene label {sibling}\n\"Sibling exact\"\nreturn\n");
            let mut graphs=crate::import_script(&rvn_parser::parse(&raw).unwrap()).unwrap();
            let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
            let command=graph.nodes.values().find(|n|n.kind==NodeKind::Scene).unwrap().id;
            let input=graph.pin_by_key(command,"background").unwrap().id;
            let asset=graph.edges.values().find(|e|e.input==input).map(|e|graph.pins[&e.output].node).unwrap();
            assert_eq!(graph.nodes[&asset].kind,NodeKind::SceneAsset);
            graph.set_pin_default(command,"background",PropertyValue::String("background_A.png".into())).unwrap();
            graph.enable_native_frame_preparation().unwrap();
            let compiled=crate::transpile_project(&graphs).unwrap();
            let graphs=crate::reimport_native_frame_script(&compiled.ast,&graphs).unwrap();
            let mut project=crate::SourceProject::open(compiled.source,&graphs).unwrap();
            let untouched_function=scope_bytes(project.source(),"sibling");
            let untouched_label=scope_bytes(project.source(),sibling);
            let mut edited=project.graphs().to_vec();
            let graph=edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
            assert_eq!(graph.pins[&input].default_value,Some(PropertyValue::String("background_A.png".into())));
            graph.remove_node(asset).unwrap();
            assert!(!graph.edges.values().any(|e|e.input==input));
            let expected=graph.clone();
            // Raw generation is diagnostic evidence only. Admission still uses
            // the strict importer/preparer and full SourceProject publisher.
            let diagnostic=crate::native_frame_preparation::prepare_native_unchecked(graph).unwrap();
            let restored=crate::import_native_frame_source(&diagnostic.source,rvn_parser::NarrativeContext::ExplicitScenes)
                .unwrap_or_else(|error|panic!("scene-inline stage=import: {error}\nSOURCE:\n{}",diagnostic.source));
            let actual=restored.iter().find(|g|g.graph_id==expected.graph_id).unwrap();
            assert_eq!(actual.edges,expected.edges);assert!(actual.nodes.keys().eq(expected.nodes.keys()));assert!(actual.pins.keys().eq(expected.pins.keys()));
            crate::prepare_native_frame_graph(graph)
                .unwrap_or_else(|error|panic!("scene-inline stage=prepare: {error}\nSOURCE:\n{}",diagnostic.source));
            let complete_expected=crate::transpile_project(&edited).unwrap().ast;
            let before=project.source().to_owned();
            project.apply_visual(&before,&edited)
                .unwrap_or_else(|error|panic!("scene-inline stage=publish: {error}\nSOURCE:\n{}",diagnostic.source));
            assert_eq!(rvn_parser::parse_program(project.source()).unwrap().script,complete_expected,"entire project AST including transport and siblings");
            assert_eq!(scope_bytes(project.source(),"sibling"),untouched_function);
            assert_eq!(scope_bytes(project.source(),sibling),untouched_label);
            let cold=crate::SourceProject::open(project.source(),project.graphs()).unwrap();
            let actual=cold.graphs().iter().find(|g|g.graph_id==expected.graph_id).unwrap();
            assert_eq!(actual.edges,expected.edges);assert!(actual.nodes.keys().eq(expected.nodes.keys()));assert!(actual.pins.keys().eq(expected.pins.keys()));
            assert_eq!(actual.pins[&input].default_value,Some(PropertyValue::String("background_A.png".into())));
            assert!(project.source().contains("background_A.png"));
            assert!(!project.source().contains("background_B.png"));
            assert!(project.source().contains("\"after\""));
            let valid=project.source().to_owned();
            let parsed=rvn_parser::parse_program(&valid).unwrap();
            let end=crate::project::project_transport_end(&parsed.script).unwrap();
            assert!(valid.contains(&format!("jump {end}")));
            for bad in [
                valid.replace(&format!("jump {end}"),&format!("jump {sibling}")),
                valid.replace(end,"__blueprint_project_end_noncanonical"),
                format!("{valid}set invalid_footer=1\n"),
            ] {
                assert_ne!(bad,valid);
                rvn_parser::parse_program(&bad).unwrap();
                assert!(crate::SourceProject::open(bad,cold.graphs()).is_err(),"an arbitrary Jump/noncanonical or executable footer must not gain transport identity");
                assert_eq!(project.source(),valid);
                assert_eq!(cold.source(),valid);
            }
            // Editing the genuine last dialogue and then connecting a real
            // Return must preserve both nodes and the exporter's bare footer.
            let mut edited=cold.graphs().to_vec();
            let graph=edited.iter_mut().find(|g|g.graph_id==expected.graph_id).unwrap();
            let text=graph.nodes.values().find(|n|n.kind==NodeKind::TextValue
                && n.properties.get("value")==Some(&PropertyValue::String("after".into()))).unwrap().id;
            let output=graph.pin_by_key(text,"value").unwrap().id;
            let dialogue=graph.edges.values().find(|e|e.output==output)
                .map(|e|graph.pins[&e.input].node).unwrap();
            assert_eq!(graph.nodes[&dialogue].kind,NodeKind::Dialogue);
            // Import made the original start -> sibling fallthrough into a
            // genuine Jump and LabelValue. Replace that actual exit; never
            // attach a second successor to the occupied Dialogue output.
            let tail=graph.pin_by_key(dialogue,"exec_out").unwrap().id;
            let exits=graph.edges.values().filter(|e|e.output==tail).cloned().collect::<Vec<_>>();
            assert_eq!(exits.len(),1,"one actual imported fallthrough exit");
            let jump=graph.pins[&exits[0].input].node;
            assert_eq!(graph.nodes[&jump].kind,NodeKind::Jump);
            assert_eq!(graph.pin_by_key(jump,"exec_in").unwrap().id,exits[0].input);
            let target=graph.pin_by_key(jump,"target").unwrap().id;
            let targets=graph.edges.values().filter(|e|e.input==target).cloned().collect::<Vec<_>>();
            assert_eq!(targets.len(),1,"the actual materialized sibling target");
            let label=graph.pins[&targets[0].output].node;
            assert_eq!(graph.nodes[&label].kind,NodeKind::LabelValue);
            assert_eq!(graph.nodes[&label].properties.get("label"),Some(&PropertyValue::String(sibling.into())));
            assert_eq!(graph.edges.values().filter(|e|e.output==targets[0].output).count(),1,"only this replaced exit owns the target value");
            assert_eq!(graph.edges.values().filter(|e|graph.pins[&e.output].node==jump||graph.pins[&e.input].node==jump).count(),2);
            let surviving_nodes=graph.nodes.keys().copied().filter(|id|*id!=jump&&*id!=label).collect::<Vec<_>>();
            let surviving_pins=graph.pins.values().filter(|p|p.node!=jump&&p.node!=label).map(|p|p.id).collect::<Vec<_>>();
            let surviving_edges=graph.edges.values().filter(|e|e.id!=exits[0].id&&e.id!=targets[0].id).cloned().collect::<Vec<_>>();
            assert_eq!(graph.disconnect_pin(tail),1);
            graph.remove_node(jump).unwrap();graph.remove_node(label).unwrap();
            assert!(surviving_nodes.iter().all(|id|graph.nodes.contains_key(id)));
            assert!(surviving_pins.iter().all(|id|graph.pins.contains_key(id)));
            assert!(surviving_edges.iter().all(|e|graph.edges.get(&e.id)==Some(e)));
            graph.set_property(text,"value",PropertyValue::String("after edited".into())).unwrap();
            let returned=graph.add_catalog_node(NodeKind::Return,[0.,400.]).unwrap();
            graph.connect(graph.pin_by_key(dialogue,"exec_out").unwrap().id,
                graph.pin_by_key(returned,"exec_in").unwrap().id).unwrap();
            let expected=graph.clone();
            let expected_project=crate::transpile_project(&edited).unwrap();
            assert!(expected_project.ast.windows(2).any(|pair|matches!(pair,
                [rvn_parser::Statement::ScheduledConsumer{version:2,body,..},rvn_parser::Statement::Jump{target}]
                if matches!(body.as_slice(),[rvn_parser::Statement::Return])&&target==end)));
            let before=project.source().to_owned();
            project.apply_visual(&before,&edited).unwrap();
            assert_eq!(rvn_parser::parse_program(project.source()).unwrap().script,expected_project.ast);
            assert_eq!(scope_bytes(project.source(),"sibling"),untouched_function);
            assert_eq!(scope_bytes(project.source(),sibling),untouched_label);
            let cold=crate::SourceProject::open(project.source(),project.graphs()).unwrap();
            let actual=cold.graphs().iter().find(|g|g.graph_id==expected.graph_id).unwrap();
            assert_eq!(actual.edges,expected.edges);assert!(actual.nodes.keys().eq(expected.nodes.keys()));assert!(actual.pins.keys().eq(expected.pins.keys()));
            assert_eq!(actual.nodes[&text].properties["value"],PropertyValue::String("after edited".into()));
            assert_eq!(actual.nodes[&returned].kind,NodeKind::Return);
        }
    }
}

#[cfg(test)]
mod native_allocator_cold1022{
    use super::*;
    fn source_graph(source:&str)->GraphDocument{
        let program=rvn_parser::parse_program(source).unwrap();
        let mut graphs=crate::import_script(&program.script).unwrap();
        let graph=graphs.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
        graph.enable_native_frame_preparation().unwrap();
        let emitted=crate::prepare_native_frame_graph(graph).unwrap();
        crate::import_native_frame_script(&emitted.ast).unwrap().into_iter().find(|actual|actual.graph_id==graph.graph_id).unwrap()
    }
    #[test]
    fn native_allocator1022_scalar_asset_and_text_keep_spent_ids_without_restoring_deleted_objects(){
        for source in ["scene label start\ntypewriter=true\n\"boundary\"\n","scene label start\nscene \"background.png\"\n\"boundary\"\n","scene label start\n\"boundary\"\n"]{
            let mut old=source_graph(source);
            // Spend all three real allocators using public graph edits, then
            // delete the complete temporary component before source emission.
            let literal=old.add_catalog_node(NodeKind::Literal,[0.,800.]).unwrap();
            old.set_property(literal,"value",PropertyValue::Bool(true)).unwrap();
            let consumer=old.add_catalog_node(NodeKind::LogicNot,[200.,800.]).unwrap();
            let input=old.nodes[&consumer].pins.iter().copied().find(|id|old.pins[id].direction==PinDirection::Input).unwrap();
            let edge=old.connect(old.pin_by_key(literal,"value").unwrap().id,input).unwrap();
            old.remove_node(consumer).unwrap();old.remove_node(literal).unwrap();
            let frontier=(old.next_node_id,old.next_pin_id,old.next_edge_id);
            let emitted=crate::prepare_native_frame_graph(&old).unwrap();
            let plain=crate::import_native_frame_script(&emitted.ast).unwrap().into_iter().find(|actual|actual.graph_id==old.graph_id).unwrap();
            assert!(plain.next_node_id<frontier.0 && plain.next_pin_id<frontier.1 && plain.next_edge_id<frontier.2,"plain source owns active IDs but not spent sidecar reservations");
            let mut restored=crate::reimport_native_frame_script(&emitted.ast,&[old.clone()]).unwrap().into_iter().find(|actual|actual.graph_id==old.graph_id).unwrap();
            assert_eq!(restored,old,"strict complete authoring equality includes all three monotone allocators");
            assert!(!restored.nodes.contains_key(&literal) && !restored.nodes.contains_key(&consumer) && !restored.edges.contains_key(&edge));
            assert_eq!(crate::prepare_native_frame_graph(&restored).unwrap().ast,emitted.ast,"reservation history cannot grant execution");
            let fresh=restored.add_catalog_node(NodeKind::Literal,[0.,900.]).unwrap();
            assert_eq!(fresh.get(),frontier.0);assert!(restored.nodes[&fresh].pins.iter().all(|pin|pin.get()>=frontier.1));
        }
    }
    #[test]
    fn native_allocator1022_real_exec_links_upgrade_keeps_source_capability_and_owner_reservations(){
        // The real public Macro ExecLinks gate requires a Handler and actual
        // shared macro control ingress. The Label-only fixture was inadmissible.
        let parsed=rvn_parser::parse("handler allowed(event) {local marker=1}").unwrap();
        let mut previous=crate::import_script(&parsed).unwrap().into_iter().find(|graph|matches!(graph.kind,GraphKind::Handler{..})).unwrap();
        let root=previous.nodes.values().find(|node|node.kind==NodeKind::HandlerEntry).unwrap().id;
        let first=previous.edges.values().find(|edge|edge.output==previous.pin_by_key(root,"exec_out").unwrap().id).unwrap().clone();
        previous.remove_edge(first.id).unwrap();
        let branch=previous.add_catalog_node(NodeKind::If,[200.,0.]).unwrap();
        let once=previous.add_catalog_node(NodeKind::DoOnce,[500.,0.]).unwrap();
        previous.set_pin_default(once,"start_closed",PropertyValue::Bool(false)).unwrap();
        let wire=|graph:&mut GraphDocument,a:NodeId,ak:&str,b:NodeId,bk:&str|{
            graph.connect(graph.pin_by_key(a,ak).unwrap().id,graph.pin_by_key(b,bk).unwrap().id).unwrap()
        };
        wire(&mut previous,root,"exec_out",branch,"exec_in");
        wire(&mut previous,branch,"then",once,"exec_in");
        let reset=wire(&mut previous,branch,"else",once,"reset");
        previous.connect(previous.pin_by_key(once,"completed").unwrap().id,first.input).unwrap();
        previous.enable_native_frame_preparation().unwrap();
        // The old source/presentation is itself strictly representable before
        // adding the real fan-in and its explicit source capability.
        let old_source=crate::prepare_native_frame_graph(&previous).unwrap();
        assert!(!old_source.source.contains("!exec_links_v1"));
        let mut changed=previous.clone();changed.remove_edge(reset).unwrap();
        wire(&mut changed,branch,"else",once,"exec_in");
        changed.enable_exec_links_source_preparation().unwrap();
        assert_ne!(crate::native_frame_preparation::native_scope(&changed).unwrap(),crate::native_frame_preparation::native_scope(&previous).unwrap(),"real source capability legitimately changes the encoded root scope");
        let emitted=crate::prepare_native_frame_graph(&changed).unwrap();
        assert!(emitted.source.contains("!exec_links_v1"));
        let restored=crate::reimport_native_frame_script(&emitted.ast,&[previous]).unwrap();
        let restored=restored.iter().find(|graph|graph.graph_id==changed.graph_id).unwrap();
        assert_eq!(restored,&changed,"source wins: numeric history does not veto a real ExecLinks upgrade, rewrite its marker or restore old topology");
        assert_eq!(crate::prepare_native_frame_graph(restored).unwrap().ast,emitted.ast);
    }
    #[test]
    fn native_allocator1022_malformed_or_foreign_reservations_refuse_without_partial_mutation(){
        let current=source_graph("scene label start\ntypewriter=true\n\"boundary\"\n");
        for which in 0..7{
            let mut old=current.clone();
            match which{
                0=>old.next_node_id=0,1=>old.next_pin_id=0,2=>old.next_edge_id=0,
                3=>old.next_node_id=old.nodes.keys().next_back().unwrap().get(),
                4=>old.next_pin_id=old.pins.keys().next_back().unwrap().get(),
                5=>old.next_edge_id=old.edges.keys().next_back().unwrap().get(),
                _=>old.graph_id=GraphId::new(old.graph_id.get()+100),
            }
            let mut next=current.clone();
            assert!(restore_native_allocator_reservations(&mut next,&old).is_err());
            assert_eq!(next,current,"rejected metadata changes no topology, defaults, IDs or allocator");
        }
    }
}
