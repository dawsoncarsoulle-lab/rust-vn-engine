//! PRIVATE graph preparation. Runtime publication still requires binding the
//! exact source/program fingerprint and graph origins to an owned activation.
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug,Clone,Copy,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum InspectorEvaluationContract {
    #[default]
    LegacyStateless,
    NativeFrameV1,
}
impl InspectorEvaluationContract {
    pub fn is_legacy(&self)->bool { *self==Self::LegacyStateless }
}
const EXEC_LINKS_PROPERTY:&str="@source_exec_links_v1";
/// Source-only explicit capability; no graph schema, runtime owner or debug
/// traversal is inferred. Legacy documents keep absent markers and bytes.
pub(crate) fn uses_exec_links(graph:&GraphDocument)->Result<bool,String>{
    let mut marked=None;
    for node in graph.nodes.values(){
        if let Some(value)=node.properties.get(EXEC_LINKS_PROPERTY){
            if value!=&PropertyValue::Bool(true)||!matches!(node.kind,NodeKind::Init|NodeKind::Label|NodeKind::FunctionEntry|NodeKind::ScreenEntry|NodeKind::HandlerEntry)||marked.replace(node.id).is_some(){
                return Err("ExecLinksV1 capability must belong to one actual definition root".into());
            }
        }
    }Ok(marked.is_some())
}
impl GraphDocument {
    /// Prepare the versioned source topology on this document only. The
    /// existing catalogue allows Range.break and typed Widget macro control
    /// fan-in. Ordinary input cardinalities/general shared consumers are unchanged.
    pub fn enable_exec_links_source_preparation(&mut self)->Result<(),String>{
        self.validate_inspector_evaluation_contract()?;
        if self.inspector_evaluation.is_legacy(){return Err("ExecLinksV1 requires actual NativeFrame source preparation first".into());}
        let demand_range=self.nodes.values().any(|node|node.kind==NodeKind::ForLoopWithBreak&&node.native_loop_contract==Some(crate::NativeLoopContract::DemandV1));
        if !demand_range {
            crate::standard_macros_1020::validate_shared_control_ingress(self)?;
        }
        let kind=match self.kind{GraphKind::Init=>NodeKind::Init,GraphKind::Label{..}=>NodeKind::Label,
            GraphKind::Function{..}|GraphKind::UnitFunction{..}=>NodeKind::FunctionEntry,GraphKind::Screen{..}=>NodeKind::ScreenEntry,
            GraphKind::Handler{..}=>NodeKind::HandlerEntry,GraphKind::Imagemap{..}=>return Err("ExecLinksV1 requires an actual attached definition root".into())};
        let roots=self.nodes.values().filter(|node|node.kind==kind).map(|node|node.id).collect::<Vec<_>>();
        let [root]=roots.as_slice()else{return Err("ExecLinksV1 requires one actual definition root".into());};
        uses_exec_links(self)?;
        let mut candidate=self.clone();
        candidate.nodes.get_mut(root).unwrap().properties.insert(EXEC_LINKS_PROPERTY.into(),PropertyValue::Bool(true));
        // A new macro ingress capability is published only after the existing
        // source codec reconstructs every real node/pin/edge exactly. Preserve
        // the established Range preparation path and its own later admission.
        if !demand_range {
            prepare_native_frame_graph(&candidate).map_err(|error|error.to_string())?;
        }
        *self=candidate;
        Ok(())
    }

    pub fn enable_native_frame_preparation(&mut self)->Result<(),String> {
        // Signature/Unit containers retain their schema and require explicit
        // backend integration before combining source capabilities.
        if crate::uses_unit_context1026(self) {
            return Err("NativeFrame preparation for Unit/signature capability remains unadmitted".into());
        }
        if self.schema_version > GRAPH_NAME_TEXT_SCHEMA_VERSION && ![GRAPH_NATIVE_FRAME_SCHEMA_VERSION, GRAPH_MIXED_INSPECTOR_SCHEMA_VERSION, GRAPH_NATIVE_LOOP_DEMAND_SCHEMA_VERSION, GRAPH_NOMINAL_ENUM_SCHEMA_VERSION].contains(&self.schema_version) {
            return Err("NativeFrame preparation for this signature/container capability is not admitted".into());
        }
        let historical_native=self.schema_version==GRAPH_NATIVE_FRAME_SCHEMA_VERSION&&!self.inspector_evaluation.is_legacy();
        for node in self.nodes.values_mut().filter(|node|node.kind==NodeKind::GetStringTableIdAndKey) {
            node.inspector_contract.get_or_insert(if historical_native {InspectorEvaluationContract::NativeFrameV1}else{InspectorEvaluationContract::LegacyStateless});
        }
        self.inspector_evaluation=InspectorEvaluationContract::NativeFrameV1;
        self.schema_version=self.schema_version.max(GRAPH_MIXED_INSPECTOR_SCHEMA_VERSION);
        Ok(())
    }
    pub fn validate_inspector_evaluation_contract(&self)->Result<(),String> {
        for node in self.nodes.values() {
            if node.kind!=NodeKind::GetStringTableIdAndKey && node.inspector_contract.is_some() {return Err("Inspector evaluation contract on unrelated node".into());}
            if node.inspector_contract==Some(InspectorEvaluationContract::NativeFrameV1) && (self.schema_version<GRAPH_MIXED_INSPECTOR_SCHEMA_VERSION || self.inspector_evaluation.is_legacy()) {return Err("Per-node Native inspector requires its mixed schema/container".into());}
            if self.schema_version>=GRAPH_MIXED_INSPECTOR_SCHEMA_VERSION && node.kind==NodeKind::GetStringTableIdAndKey && node.inspector_contract.is_none() {return Err("Mixed inspector schema requires an explicit contract for every inspector".into());}
        }
        match (self.schema_version,self.inspector_evaluation) {
            (GRAPH_NATIVE_FRAME_SCHEMA_VERSION|GRAPH_MIXED_INSPECTOR_SCHEMA_VERSION|GRAPH_NATIVE_LOOP_DEMAND_SCHEMA_VERSION,InspectorEvaluationContract::NativeFrameV1)=>Ok(()),
            (GRAPH_NATIVE_FRAME_SCHEMA_VERSION|GRAPH_MIXED_INSPECTOR_SCHEMA_VERSION|GRAPH_NATIVE_LOOP_DEMAND_SCHEMA_VERSION,_)=>Err("Schema11 requires explicit NativeFrameV1".into()),
            (_,InspectorEvaluationContract::NativeFrameV1)=>Err("NativeFrameV1 requires opt-in schema11".into()),
            _=>Ok(()),
        }
    }
}
#[derive(Default)]
pub(crate) struct GatheredConsumer {
    pub seen:BTreeSet<NodeId>,
    pub legacy_outputs:BTreeSet<PinId>,
    pub steps:Vec<(bool,String,String,NodeId)>,
}

/// Authoring source projection only. The existing sidecar is required to
/// restore wholly isolated Widget macros; they have no emitted origin, PC,
/// runtime cell or SourceMap entry. Strict Native preparation below is unchanged.
pub fn prepare_native_authoring_graph(graph:&GraphDocument)->Result<TranspiledScriptWithOrigins,TranspileError> {
    prepare_native_authoring_graph_in_context(graph,None)
}
pub(crate) fn prepare_native_authoring_graph_in_context(graph:&GraphDocument,context:Option<&rvn_parser::Script>)->Result<TranspiledScriptWithOrigins,TranspileError> {
    let execution=crate::document::native_authoring_execution_projection(graph)
        .map_err(TranspileError::GeneratedSourceInvalid)?;
    prepare_native_frame_graph_in_context(&execution,context)
}
/// Source bytes and genuine graph statement ranges are available for a future
/// owner/fingerprint binding. This object is not a runtime activation token.
pub fn prepare_native_frame_graph(graph:&GraphDocument)->Result<TranspiledScriptWithOrigins,TranspileError> {
    prepare_native_frame_graph_in_context(graph,None)
}
pub(crate) fn prepare_native_frame_graph_in_context(graph:&GraphDocument,context:Option<&rvn_parser::Script>)->Result<TranspiledScriptWithOrigins,TranspileError> {
    let compiled=prepare_native_unchecked(graph)?;
    let restored=if crate::empty_native_init_0984::empty_graph(graph) {
        crate::empty_native_init_0984::import_in_context(&compiled.source,&compiled.ast,context)
            .map_err(TranspileError::GeneratedSourceInvalid)?
    }else{
        match context {
            Some(resolved)=>crate::import::import_native_preparation_with_context(&compiled.ast,resolved),
            None=>crate::import::import_native_preparation(&compiled.ast),
        }.map_err(TranspileError::GeneratedSourceInvalid)?
    };
    let matching:Vec<_>=restored.iter().filter(|candidate|candidate.graph_id==graph.graph_id&&!candidate.inspector_evaluation.is_legacy()).collect();
    let [restored]=matching.as_slice() else {return Err(TranspileError::GeneratedSourceInvalid("Native graph source identity was not reconstructed uniquely".into()));};
    if restored.edges!=graph.edges||restored.nodes.keys().ne(graph.nodes.keys())||restored.pins.keys().ne(graph.pins.keys()) {
        // Describe the rejected topology only; these observations neither
        // repair identities nor grant authority to a provisional graph.
        let missing_nodes=graph.nodes.iter().filter_map(|(id,n)|(!restored.nodes.contains_key(id)).then(||format!("{id}:{:?}",n.kind))).take(4).collect::<Vec<_>>();
        let extra_nodes=restored.nodes.iter().filter_map(|(id,n)|(!graph.nodes.contains_key(id)).then(||format!("{id}:{:?}",n.kind))).take(4).collect::<Vec<_>>();
        let missing_pins=graph.pins.iter().filter_map(|(id,p)|(!restored.pins.contains_key(id)).then(||format!("{id}:{}.{}",p.node,p.key))).take(4).collect::<Vec<_>>();
        let extra_pins=restored.pins.iter().filter_map(|(id,p)|(!graph.pins.contains_key(id)).then(||format!("{id}:{}.{}",p.node,p.key))).take(4).collect::<Vec<_>>();
        let missing_edges=graph.edges.iter().filter_map(|(id,e)|(restored.edges.get(id)!=Some(e)).then(||format!("{id}:{}->{}",e.output,e.input))).take(4).collect::<Vec<_>>();
        let extra_edges=restored.edges.iter().filter_map(|(id,e)|(graph.edges.get(id)!=Some(e)).then(||format!("{id}:{}->{}",e.output,e.input))).take(4).collect::<Vec<_>>();
        return Err(TranspileError::GeneratedSourceInvalid(format!("Native graph identities/wiring cannot be represented authoritatively; nodes authored={} restored={} missing={missing_nodes:?} extra={extra_nodes:?}; pins authored={} restored={} missing={missing_pins:?} extra={extra_pins:?}; edges authored={} restored={} missing_or_changed={missing_edges:?} extra_or_changed={extra_edges:?}",graph.nodes.len(),restored.nodes.len(),graph.pins.len(),restored.pins.len(),graph.edges.len(),restored.edges.len())));
    }
    Ok(compiled)
}
pub(crate) fn prepare_native_unchecked(graph:&GraphDocument)->Result<TranspiledScriptWithOrigins,TranspileError> {
    crate::codegen::prepare_native_emission(graph,native_scope(graph)?)
}
/// Read-only lexical evidence. This tolerates only retained incompatible wires;
/// runtime/source publication still uses the strict preparation entry point.
pub(crate) fn prepare_native_binding_analysis(graph:&GraphDocument)->Result<TranspiledScriptWithOrigins,TranspileError> {
    crate::codegen::prepare_native_binding_analysis(graph,native_scope(graph)?)
}
pub(crate) fn native_scope(graph:&GraphDocument)->Result<String,TranspileError> {
    if graph.inspector_evaluation!=InspectorEvaluationContract::NativeFrameV1 {
        return Err(TranspileError::GeneratedSourceInvalid("Explicit NativeFrameV1 opt-in required".into()));
    }
    let root_kind=match graph.kind {GraphKind::Init=>NodeKind::Init,GraphKind::Label{..}=>NodeKind::Label,GraphKind::Function{..}|GraphKind::UnitFunction{..}=>NodeKind::FunctionEntry,GraphKind::Screen{..}=>NodeKind::ScreenEntry,GraphKind::Handler{..}=>NodeKind::HandlerEntry,GraphKind::Imagemap{..}=>return Err(TranspileError::GeneratedSourceInvalid("Imagemap independent root source contract needs explicit owner attachment".into()))};
    let roots:Vec<_>=graph.nodes.values().filter(|node|node.kind==root_kind).collect();
    let [root]=roots.as_slice() else {return Err(TranspileError::GeneratedSourceInvalid("One genuine function root required".into()));};
    let scope=format!("g{}/{}",graph.graph_id,encode_node(graph,root.id).map_err(TranspileError::GeneratedSourceInvalid)?);
    if scope.len()>256 {return Err(TranspileError::GeneratedSourceInvalid("Native source scope exceeds 256 UTF-8 bytes".into()));}
    Ok(scope)
}

/// Roles are catalogue pin keys. @ is the genuine incoming EdgeId, 0 is an
/// unconnected port. None of these identities is an evaluated user variable.
#[derive(Debug,Clone,PartialEq,Eq)]
pub(crate) struct NativeNodeIdentity {
    pub node:NodeId,
    pub kind:NodeKind,
    pub ports:Vec<(String,PinId,Option<EdgeId>)>,
    pub static_inputs:std::collections::BTreeMap<String,PinId>,
    pub dynamic_inputs:std::collections::BTreeMap<String,PinId>,
    pub legacy_string_sequence:bool,
    pub exec_links_v1:bool,
    pub exec_inputs:std::collections::BTreeMap<String,Vec<rvn_parser::scheduled_exec_links_v1::Incoming>>,
}
pub(crate) fn is_static_audio_input(kind:NodeKind,key:&str)->bool {
    let Ok(value)=serde_json::to_value(kind) else {return false;};
    value.as_str().is_some_and(|kind|rvn_parser::scheduled_operand_contract::is_static_operand_role(kind,key))
}
/// Only the existing variable/read family carries the authored single-slot
/// template while retaining its actual dynamic source output.
pub(crate) fn dialogue_variable_name(graph:&GraphDocument,node:NodeId,key:&str)->Option<String> {
    let mut input=graph.pin_by_key(node,key)?.id;
    let mut seen=BTreeSet::new();
    loop {
        let edge=graph.edges.values().find(|edge|edge.input==input)?;
        let pin=graph.pins.get(&edge.output)?;
        let source=graph.nodes.get(&pin.node)?;
        if seen.len()>=128||!seen.insert(source.id){return None;}
        match source.kind {
            NodeKind::VariableGet|NodeKind::SetVariable|NodeKind::LocalVariable => return graph.variable_node_name(source.id),
            NodeKind::Reroute|NodeKind::ConvertNumberToText=>input=graph.pin_by_key(source.id,"value")?.id,
            NodeKind::ConvertStringToText|NodeKind::ConvertTextToString|NodeKind::ConvertNameToString if source.properties.get("legacy_passthrough")==Some(&PropertyValue::Bool(true))=>input=graph.pin_by_key(source.id,"value")?.id,
            _=>return None,
        }
    }
}
fn is_source_identity_role(kind:NodeKind,key:&str)->bool {
    is_static_audio_input(kind,key)||(kind==NodeKind::Dialogue&&key=="text")
}
pub(crate) fn music_volume_uses_expression(graph:&GraphDocument,node:NodeId)->bool {
    operand_uses_expression(graph,node,"level")
}
pub(crate) fn typewriter_set_uses_expression(graph:&GraphDocument,node:NodeId)->bool {
    operand_uses_expression(graph,node,"enabled")
}
pub(crate) fn typewriter_speed_uses_expression(graph:&GraphDocument,node:NodeId)->bool {
    operand_uses_expression(graph,node,"speed")
}
fn operand_uses_expression(graph:&GraphDocument,node:NodeId,key:&str)->bool {
    let Some(input)=graph.pin_by_key(node,key) else{return false;};
    let mut input=input.id;let mut seen=BTreeSet::new();
    loop {
        let Some(edge)=graph.edges.values().find(|edge|edge.input==input) else{return false;};
        let Some(pin)=graph.pins.get(&edge.output) else{return true;};
        let Some(source)=graph.nodes.get(&pin.node) else{return true;};
        if !seen.insert(source.id)||seen.len()>128{return true;}
        match source.kind {
            NodeKind::Literal=>return false,
            NodeKind::Reroute=>{let Some(pin)=graph.pin_by_key(source.id,"value") else{return true;};input=pin.id;},
            _=>return true,
        }
    }
}
pub(crate) fn encode_node(graph:&GraphDocument,node:NodeId)->Result<String,String> {
    let owner=graph.nodes.get(&node).ok_or("Missing source identity node")?;
    let kind=serde_json::to_string(&owner.kind).map_err(|error|error.to_string())?;
    let exec_links_v1=uses_exec_links(graph)?;
    let mut roles=Vec::new();
    for id in &owner.pins {
        let pin=graph.pins.get(id).ok_or("Missing source identity pin")?;
        if !pin.key.bytes().all(|byte|byte.is_ascii_alphanumeric()||byte==b'_') {return Err("Source identity pin role is not representable".into());}
        let incoming:Vec<_>=graph.edges.values().filter(|edge|edge.input==*id).collect();
        if exec_links_v1&&pin.direction==PinDirection::Input
            &&(pin.value_type==ValueType::Execution
                || owner.kind==NodeKind::Reroute&&graph.resolved_pin_is_execution(pin.id)) {
            let known=node_definition(owner.kind).pins.into_iter().any(|known|known.key==pin.key&&known.direction==PinDirection::Input&&known.value_type==ValueType::Execution);
            if !known{return Err("ExecLinksV1 input must retain its actual catalogue execution role".into());}
            let links=incoming.iter().map(|edge|rvn_parser::scheduled_exec_links_v1::Incoming{edge:edge.id.get(),output_pin:edge.output.get()}).collect::<Vec<_>>();
            let list=rvn_parser::scheduled_exec_links_v1::encode_fragment(&links,rvn_parser::scheduled_exec_links_v1::Limits{bytes:256,links:256,declarations:100_000}).map_err(|e|e.to_string())?;
            roles.push(format!("{}={}@{}",pin.key,id,list));continue;
        }
        if incoming.len()>1 {return Err("Native source identity requires one incoming edge per port".into());}
        // Literal audio AST fields cannot carry captured expressions. Preserve
        // their genuine source output explicitly; equal constants are not identity.
        let output=if (is_static_audio_input(owner.kind,&pin.key)
            && !(owner.kind==NodeKind::MusicVolume && pin.key=="level" && music_volume_uses_expression(graph,node))
            && !(owner.kind==NodeKind::TypewriterSet && pin.key=="enabled" && typewriter_set_uses_expression(graph,node))
            && !(owner.kind==NodeKind::TypewriterSpeed && pin.key=="speed" && typewriter_speed_uses_expression(graph,node))) || (owner.kind==NodeKind::Dialogue && pin.key=="text" && dialogue_variable_name(graph,node,"text").is_some())
            || (crate::standard_macros_1020::kind(owner.kind).is_some() && matches!(pin.key.as_str(),"n"|"start_closed")) {
            incoming.first().map(|edge|format!("~{}",edge.output)).unwrap_or_default()
        } else {String::new()};
        roles.push(format!("{}={}@{}{}",pin.key,id,incoming.first().map_or(0,|edge|edge.id.get()),output));
    }
    let marker=if exec_links_v1{"!exec_links_v1"}else{""};
    let plain=format!("n{}:{}{}|{}",node,kind.trim_matches('"'),marker,roles.join(","));
    let property=match owner.kind{NodeKind::ListLiteral=>Some("items"),NodeKind::FunctionCall=>Some("args"),_=>None};
    let legacy=if let Some(key)=property{
        match owner.properties.get("input_count"){
            Some(PropertyValue::Int(_))=>false,
            Some(_)=>return Err("Native sequence input_count must retain its integer mode".into()),
            None=>{
                let Some(PropertyValue::StringList(values))=owner.properties.get(key)else{return Err("Native property sequence requires its actual StringList".into());};
                let output=if owner.kind==NodeKind::ListLiteral{"list"}else{"result"};
                if roles.len()!=1||graph.pins[&owner.pins[0]].key!=output{return Err("Native property sequence cannot hide typed input ports".into());}
                // Old unmarked nonempty property sources are unambiguous from
                // their one output plus actual string expressions. Preserve
                // that spelling when re-exporting those frozen old bytes.
                !(owner.properties.get("@native_string_sequence_source")==Some(&PropertyValue::String("unmarked_v0".into()))&&!values.is_empty())
            },
        }
    }else{false};
    let value=if legacy{format!("l:string_list_v1/{plain}")}else{plain};
    if value.len()>256 {return Err("Native node identity exceeds 256 UTF-8 bytes; no truncation permitted".into());}
    Ok(value)
}
pub(crate) fn decode_node(value:&str)->Result<NativeNodeIdentity,String> {
    if value.is_empty()||value.len()>256 {return Err("Invalid native node identity size".into());}
    let (value,legacy_string_sequence)=if let Some(rest)=value.strip_prefix("l:"){
        let(mode,value)=rest.split_once('/').ok_or("Native sequence mode requires its node identity")?;
        if mode!="string_list_v1"{return Err("Unknown Native sequence source mode".into());}(value,true)
    }else{(value,false)};
    let (head,roles)=value.split_once('|').ok_or("Native node identity requires explicit pin roles")?;
    let (node,kind)=head.strip_prefix('n').ok_or("Native node identity requires n prefix")?.split_once(':').ok_or("Native node kind missing")?;
    fn id(value:&str)->Result<u64,String> {let number=value.parse::<u64>().map_err(|_|"Invalid native integer identity")?;if number==0||number.to_string()!=value||number==u64::MAX {return Err("Native identities must be canonical positive integers with available successor".into());}Ok(number)}
    let node=NodeId::new(id(node)?);
    let (kind,exec_links_v1)=if let Some(kind)=kind.strip_suffix("!exec_links_v1"){(kind,true)}else{(kind,false)};
    let kind:NodeKind=serde_json::from_str(&serde_json::to_string(kind).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    let mut ports=Vec::new();let mut keys=BTreeSet::new();let mut pins=BTreeSet::new();let mut static_inputs=std::collections::BTreeMap::new();let mut dynamic_inputs=std::collections::BTreeMap::new();
    let mut exec_inputs=std::collections::BTreeMap::new();
    for role in roles.split(',').filter(|value|!value.is_empty()) {
        let (key,ids)=role.split_once('=').ok_or("Native port role missing")?;
        let (pin,edge)=ids.split_once('@').ok_or("Native incoming edge role missing")?;
        if key.is_empty()||!key.bytes().all(|byte|byte.is_ascii_alphanumeric()||byte==b'_')||!keys.insert(key.to_owned()) {return Err("Duplicate or invalid native port role".into());}
        let pin=PinId::new(id(pin)?);if !pins.insert(pin) {return Err("Duplicate native PinId".into());}
        let execution=node_definition(kind).pins.into_iter().any(|known|known.key==key&&known.direction==PinDirection::Input&&known.value_type==ValueType::Execution);
        if edge.starts_with('['){
            if !exec_links_v1||!execution{return Err("ExecLinksV1 list must retain its actual catalogue execution input role".into());}
            let links=rvn_parser::scheduled_exec_links_v1::decode_fragment(edge,rvn_parser::scheduled_exec_links_v1::Limits{bytes:256,links:256,declarations:100_000}).map_err(|e|e.to_string())?;
            exec_inputs.insert(key.into(),links);ports.push((key.into(),pin,None));continue;
        }
        if exec_links_v1&&execution{return Err("ExecLinksV1 requires the entire actual execution input list".into());}
        let (edge,output)=match edge.split_once('~') {Some((edge,output))=>(edge,Some(output)),None=>(edge,None)};
        let edge=if edge=="0" {None}else{Some(EdgeId::new(id(edge)?))};
        if let Some(output)=output {
            if edge.is_none()||!is_source_identity_role(kind,key) {return Err("Static audio source binding requires its actual connected operand".into());}
            if is_static_audio_input(kind,key){static_inputs.insert(key.to_owned(),PinId::new(id(output)?));}
            else{dynamic_inputs.insert(key.to_owned(),PinId::new(id(output)?));}
        }
        ports.push((key.into(),pin,edge));
    }
    if ports.is_empty() {return Err("Native node identity requires ports".into());}
    if legacy_string_sequence{
        let output=match kind{NodeKind::ListLiteral=>"list",NodeKind::FunctionCall=>"result",_=>return Err("Native StringList mode is restricted to its actual sequence node kinds".into())};
        if !matches!(ports.as_slice(),[(role,_,None)]if role==output)||!static_inputs.is_empty()||!dynamic_inputs.is_empty(){return Err("Native StringList mode cannot hide typed or connected input ports".into());}
    }
    Ok(NativeNodeIdentity{node,kind,ports,static_inputs,dynamic_inputs,legacy_string_sequence,exec_links_v1,exec_inputs})
}
pub(crate) fn decode_producer(value:&str)->Result<(NativeNodeIdentity,Option<String>),String> {
    if let Some(value)=value.strip_prefix("p:") {
        let (port,node)=value.split_once('/').ok_or("Legacy source terminal role missing")?;
        if !matches!(port,"valid"|"table_id"|"key") {return Err("Unknown Legacy inspector terminal role".into());}
        let identity=decode_node(node)?;
        if identity.kind!=NodeKind::GetStringTableIdAndKey {return Err("Legacy terminal marker on unrelated node".into());}
        Ok((identity,Some(port.into())))
    } else {decode_node(value).map(|identity|(identity,None))}
}
pub(crate) fn inspector_contract(graph:&GraphDocument,node:NodeId)->InspectorEvaluationContract {
    graph.nodes[&node].inspector_contract.unwrap_or(if graph.schema_version==GRAPH_NATIVE_FRAME_SCHEMA_VERSION&&!graph.inspector_evaluation.is_legacy() {InspectorEvaluationContract::NativeFrameV1}else{InspectorEvaluationContract::LegacyStateless})
}
pub(crate) fn decode_scope(value:&str)->Result<(GraphId,NativeNodeIdentity),String> {
    if value.len()>256||value.contains('\0') {return Err("Invalid native source scope size/content".into());}
    let (graph,root)=value.strip_prefix('g').ok_or("Native graph scope requires g prefix")?.split_once('/').ok_or("Native graph scope requires root identity")?;
    let number=graph.parse::<u64>().map_err(|_|"Invalid GraphId")?;
    if number==0||number==u64::MAX||number.to_string()!=graph {return Err("Noncanonical GraphId".into());}
    Ok((GraphId::new(number),decode_node(root)?))
}

/// Reconcile provisional Legacy allocations against authoritative Native IDs.
/// This is the existing native import algorithm, shared with project adaptation.
pub(crate) fn reconcile_imported_graph_ids(result:&mut [GraphDocument])->Result<(),String> {
    let mut graphs=BTreeSet::new();
    for graph in result.iter().filter(|graph|!graph.inspector_evaluation.is_legacy()) {
        if !graphs.insert(graph.graph_id) {return Err("Source Native GraphId collision".into());}
    }
    let mut next=result.iter().map(|graph|graph.graph_id.get()).max().unwrap_or(0).checked_add(1).ok_or("GraphId exhausted")?;
    for graph in result.iter_mut().filter(|graph|graph.inspector_evaluation.is_legacy()) {
        if !graphs.insert(graph.graph_id) {
            graph.graph_id=GraphId::new(next);graphs.insert(graph.graph_id);next=next.checked_add(1).ok_or("GraphId exhausted")?;
        }
    }
    Ok(())
}

/// PRIVATE source-authoritative path. Ordinary import/export remains refused
/// until runtime, source-workspace transactions, and debugger are connected.
pub fn import_native_frame_script(script:&rvn_parser::Script)->Result<Vec<GraphDocument>,String> {
    import_native_frame_script_in_context(script,None)
}
pub(crate) fn import_native_frame_script_in_context(script:&rvn_parser::Script,context:Option<&rvn_parser::Script>)->Result<Vec<GraphDocument>,String> {
    import_native_frame_script_with_owners(script,context,&[])
}
pub(crate) fn import_native_frame_script_with_owners(
    script:&rvn_parser::Script,context:Option<&rvn_parser::Script>,owners:&[GraphDocument],
)->Result<Vec<GraphDocument>,String>{
    let result=crate::import::import_native_preparation_with_owners(script,context,owners)?;
    finish_native_import(script,result)
}
pub(crate) fn finish_native_import(script:&rvn_parser::Script,mut result:Vec<GraphDocument>)->Result<Vec<GraphDocument>,String> {
    if !script.iter().any(|statement|matches!(statement,rvn_parser::Statement::Init{..})) {
        result.retain(|graph|graph.kind!=GraphKind::Init);
    }
    reconcile_imported_graph_ids(&mut result)?;
    if !result.iter().any(|graph|!graph.inspector_evaluation.is_legacy()) {return Err("Native source contains no explicit versioned consumer".into());}
    Ok(result)
}
pub fn reimport_native_frame_script(script:&rvn_parser::Script,previous:&[GraphDocument])->Result<Vec<GraphDocument>,String> {
    reimport_native_frame_script_in_context(script,previous,None)
}
pub(crate) fn reimport_native_frame_script_in_context(script:&rvn_parser::Script,previous:&[GraphDocument],context:Option<&rvn_parser::Script>)->Result<Vec<GraphDocument>,String> {
    let result=import_native_frame_script_with_owners(script,context,previous)?;
    reconcile_native_presentation(result,script,previous,context)
}
pub(crate) fn reconcile_native_presentation(mut result:Vec<GraphDocument>,script:&rvn_parser::Script,previous:&[GraphDocument],context:Option<&rvn_parser::Script>)->Result<Vec<GraphDocument>,String> {
    if !script.iter().any(|statement|matches!(statement,rvn_parser::Statement::Init{..})) {
        // The Legacy editor has a source-less, empty Init view. Preserve that
        // existing presentation during mixed migration, without emitting a
        // Statement/PC or restoring any cached declaration or execution edge.
        let empty:Vec<_>=previous.iter().filter(|graph|graph.kind==GraphKind::Init
            && graph.inspector_evaluation.is_legacy()
            && graph.edges.is_empty() && graph.nodes.len()==1 && graph.nodes.values().all(|node|node.kind==NodeKind::Init)
            && !graph.validate().iter().any(|d|d.severity==DiagnosticSeverity::Error)).collect();
        if empty.len()>1{return Err("Ambiguous empty Init presentation".into());}
        if let Some(empty_previous)=empty.first(){
            let derived_context=crate::import::import_native_preparation_with_owners(script,context,previous)?;
            let mut derived=derived_context.into_iter().find(|graph|graph.kind==GraphKind::Init).ok_or("Missing source-derived empty Init view")?;
            derived.graph_id=empty_previous.graph_id;
            if let (Some(actual),Some(old))=(derived.nodes.values_mut().find(|node|node.kind==NodeKind::Init),empty_previous.nodes.values().find(|node|node.kind==NodeKind::Init)) {
                actual.position=old.position;actual.title_override=old.title_override.clone();actual.annotation=old.annotation.clone();
            }
            result.push(derived);
        }
    }
    // Native IDs are declared by RVN. Legacy scopes retain their existing
    // presentation identity by actual scope kind/name; importing a Native
    // neighbour must not renumber an untouched scene or its label references.
    let mut reserved:BTreeSet<_>=result.iter().filter(|graph|!graph.inspector_evaluation.is_legacy()).map(|graph|graph.graph_id).collect();
    let mut desired=std::collections::BTreeMap::new();
    for graph in result.iter().filter(|graph|graph.inspector_evaluation.is_legacy()) {
        let matching:Vec<_>=previous.iter().filter(|old|old.kind==graph.kind).collect();
        if matching.len()>1{return Err("Ambiguous Legacy source scope presentation".into());}
        if let Some(old)=matching.first(){if !reserved.contains(&old.graph_id){desired.insert(graph.graph_id,old.graph_id);reserved.insert(old.graph_id);}}
    }
    let mut used:BTreeSet<_>=result.iter().filter(|graph|!graph.inspector_evaluation.is_legacy()).map(|graph|graph.graph_id).collect();
    let mut next=result.iter().chain(previous).map(|graph|graph.graph_id.get()).max().unwrap_or(0).checked_add(1).ok_or("GraphId exhausted")?;
    let mut moved=std::collections::BTreeMap::new();
    for graph in result.iter_mut().filter(|graph|graph.inspector_evaluation.is_legacy()) {
        let old=graph.graph_id;
        let identity=if let Some(id)=desired.get(&old){*id}else if !reserved.contains(&old)&&!used.contains(&old){old}else{
            while reserved.contains(&GraphId::new(next))||used.contains(&GraphId::new(next)){next=next.checked_add(1).ok_or("GraphId exhausted")?;}
            let id=GraphId::new(next);next=next.checked_add(1).ok_or("GraphId exhausted")?;id
        };
        if !used.insert(identity){return Err("Legacy source presentation identity collision".into());}
        graph.graph_id=identity;moved.insert(old,identity);
    }
    for graph in &mut result {for node in graph.nodes.values_mut().filter(|node|node.kind==NodeKind::LabelValue){
        if let Some(PropertyValue::String(target))=node.properties.get_mut("target_graph"){
            if let Ok(old)=serde_json::from_str::<GraphId>(target){if let Some(new)=moved.get(&old){*target=serde_json::to_string(new).map_err(|e|e.to_string())?;}}
        }
    }}
    // These are source-reconstructed, already GraphId/kind-mapped Legacy
    // scopes. Reuse the ordinary Legacy authority: refresh real registries,
    // retain the complete prior presentation only when its full AST matches,
    // otherwise reconcile compatible identities against the changed source.
    // Native scopes and new Legacy scopes never enter this retained-owner port;
    // every input has an actual prior owner, so no graph/parameter allocator is
    // invoked and the mixed mapping above remains authoritative.
    let retained:Vec<_>=result.iter().filter(|graph|graph.inspector_evaluation.is_legacy()
        && previous.iter().any(|old|old.graph_id==graph.graph_id && old.kind==graph.kind
            && old.inspector_evaluation.is_legacy())).cloned().collect();
    if !retained.is_empty() {
        // Reconcile only retained owners; compile their actual references
        // against every source-reconstructed scope, including new declarations.
        let reconciled=crate::import::reconcile_prepared_import_in_context(retained,previous,false,&result)?;
        for graph in reconciled {
            let target=result.iter_mut().find(|current|current.graph_id==graph.graph_id
                && current.kind==graph.kind && current.inspector_evaluation.is_legacy())
                .ok_or("Legacy reconciliation changed its actual mixed source identity")?;
            *target=graph;
        }
    }
    for graph in &mut result {
        let matching:Vec<_>=previous.iter().filter(|old|old.graph_id==graph.graph_id).collect();
        if matching.len()>1 {return Err("Ambiguous previous GraphId".into());}
        if let Some(old)=matching.first() {
            if std::mem::discriminant(&old.kind)!=std::mem::discriminant(&graph.kind) {return Err("Source GraphId changed graph kind".into());}
            // Only after the existing source GraphId/kind reconciliation.
            // This retains editing policy, never executable source identity.
            graph.authoring_defaults = old.authoring_defaults;
            crate::document::restore_isolated_macro_presentation(graph,old)?;
            crate::document::restore_native_allocator_reservations(graph,old)?;
            for node in graph.nodes.values_mut() {
                if let Some(prior)=old.nodes.get(&node.id) {
                    if prior.kind==node.kind&&prior.pins==node.pins {
                        node.position=prior.position;node.annotation=prior.annotation.clone();node.title_override=prior.title_override.clone();
                    }
                }
            }
            restore_connected_inline_presentation(graph,old);
        }
    }
    Ok(result)
}

/// A hidden default belongs to the existing editor slot, never to the frozen
/// executable input. Restore it only after real source reconstruction, when
/// both presentations identify the very same wired slot and semantic owner.
fn restore_connected_inline_presentation(graph:&mut GraphDocument,old:&GraphDocument){
    crate::document::restore_connected_asset_presentation(graph,old);
    if graph.kind!=old.kind{return;}
    fn wires(graph:&GraphDocument)->std::collections::BTreeMap<PinId,Option<&GraphEdge>>{
        let mut incoming=std::collections::BTreeMap::new();
        for edge in graph.edges.values(){
            incoming.entry(edge.input).and_modify(|prior|*prior=None).or_insert(Some(edge));
        }
        incoming
    }
    let current_wires=wires(graph);
    let old_wires=wires(old);
    let mut retained=Vec::new();
    for (id,prior) in &old.pins {
        let Some(pin)=graph.pins.get(id)else{continue;};
        if pin.node!=prior.node||pin.key!=prior.key||pin.direction!=PinDirection::Input
            ||prior.direction!=PinDirection::Input||pin.value_type!=prior.value_type
            ||pin.cardinality!=prior.cardinality||pin.value_type.is_execution()
            ||matches!(&pin.value_type,ValueType::Asset(_))
            ||pin.default_value.is_some()||pin.text_localization.is_some()||pin.text_table_reference.is_some()
            ||prior.default_value.is_none(){continue;}
        let (Some(owner),Some(previous))=(graph.nodes.get(&pin.node),old.nodes.get(&pin.node))else{continue;};
        if owner.kind!=previous.kind||owner.pins!=previous.pins||owner.properties!=previous.properties
            ||!crate::document::retains_inline_pin_default(owner.kind,&pin.value_type){continue;}
        let (Some(Some(current)),Some(Some(previous)))=(current_wires.get(id),old_wires.get(id))else{continue;};
        // A typed consumer fallback belongs to its stable declared slot,
        // not the old producer. Current source still owns the actual cable.
        let scalar_slot=crate::document::scalar_consumer_slot(owner.kind)
            .is_some_and(|(key,value_type)|pin.key==key&&pin.value_type==value_type);
        if current!=previous&&!scalar_slot{continue;}
        retained.push((*id,prior.default_value.clone(),prior.text_localization.clone(),prior.text_table_reference.clone()));
    }
    // End all shared edge/node borrows before updating the presentation only.
    for (id,value,text,table) in retained {
        let pin=graph.pins.get_mut(&id).unwrap();
        pin.default_value=value;pin.text_localization=text;pin.text_table_reference=table;
    }
}
