//! Real Unit Graph/source boundary. No evaluator or gameplay allocation lives here.
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn entry(graph: &GraphDocument) -> Result<NodeId, String> {
    if !matches!(graph.kind, GraphKind::UnitFunction { .. }) || !crate::supports_unit_schema1026(graph.schema_version) {
        return Err("An actual schema14 Unit declaration graph is required".into());
    }
    let roots: Vec<_> = graph.nodes.values().filter(|node| node.kind == NodeKind::FunctionEntry).map(|node| node.id).collect();
    let [root] = roots.as_slice() else { return Err("Unit requires exactly one FunctionEntry".into()); };
    Ok(*root)
}

pub(crate) fn parameters(graph: &GraphDocument) -> Result<Vec<&GraphPin>, String> {
    let root = entry(graph)?;
    crate::entry_parameter_identity1003::validate_graph_parameter_metadata(graph)?;
    let node = &graph.nodes[&root];
    let mut result = Vec::new();let mut ids = BTreeSet::new();let mut names = BTreeSet::new();let mut exec = 0;
    for id in &node.pins {
        let pin = graph.pins.get(id).ok_or("Unit entry references absent pin")?;
        if pin.node != root || pin.id != *id || !ids.insert(*id) { return Err("Malformed Unit entry pin identity".into()); }
        if pin.key == "exec_out" && pin.direction == PinDirection::Output && pin.value_type == ValueType::Execution
            && pin.parameter_id.is_none() { exec += 1;continue; }
        if pin.direction != PinDirection::Output || pin.cardinality != PinCardinality::Many
            || pin.parameter_id.is_none() || pin.value_type != ValueType::Any || pin.default_value.is_some()
            || rvn_parser::variable_binding_source_name(&pin.label).is_none() || !names.insert(pin.label.as_str()) {
            return Err("Unit parameters require ordered real untyped data outputs with unique lexical names".into());
        }
        result.push(pin);
    }
    if exec != 1 || result.len() > 128 || graph.pins.iter().any(|(id,pin)| pin.node == root && !ids.contains(id)) {
        return Err("Incomplete Unit entry pin set".into());
    }
    match node.properties.get("parameters") {
        None => (),Some(PropertyValue::StringList(names)) if names.is_empty() => (),
        _ => return Err("Unit signature must come from its actual entry pins, not a parallel metadata signature".into()),
    }
    Ok(result)
}

pub(crate) fn resolve_call<'a>(project: &'a [GraphDocument], caller: &'a GraphDocument, call: NodeId)
    -> Result<(&'a GraphDocument, Vec<&'a GraphPin>), String> {
    let node = caller.nodes.get(&call).ok_or("Unit call node is absent")?;
    if node.kind != NodeKind::UnitCall || !crate::supports_unit_schema1026(caller.schema_version) {
        return Err("Actual schema14 UnitCall required".into());
    }
    let name = match node.properties.get("function") {Some(PropertyValue::String(name)) if !name.is_empty()=>name,_=>return Err("UnitCall has no explicit callee name".into())};
    let candidates: Vec<_> = project.iter().filter(|graph| graph.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_none()).filter(|graph| match &graph.kind {
        GraphKind::Function{name:n}|GraphKind::UnitFunction{name:n}|GraphKind::Screen{name:n}|GraphKind::Handler{name:n} => n==name,_=>false,
    }).collect();
    let [callee] = candidates.as_slice() else {return Err("Unit callee must resolve uniquely in the actual project/source context".into());};
    if !matches!(callee.kind, GraphKind::UnitFunction{..}) {return Err("UnitCall cannot invoke a value Function, Screen or Handler".into());}
    let parameters = parameters(callee)?;
    let mut exec_in = 0;let mut exec_out = 0;let mut inputs = Vec::new();let mut seen = BTreeSet::new();
    for id in &node.pins {
        let pin = caller.pins.get(id).ok_or("Call port is absent")?;
        if pin.node!=call || pin.id!=*id || !seen.insert(*id) {return Err("Malformed actual call port ownership".into());}
        if pin.value_type==ValueType::Execution {
            if pin.parameter_id.is_some(){return Err("Execution port cannot carry a parameter identity".into());}
            match (pin.key.as_str(),pin.direction) {("exec_in",PinDirection::Input)=>exec_in+=1,("exec_out",PinDirection::Output)=>exec_out+=1,_=>return Err("UnitCall execution shape must be exec_in/exec_out".into())}
        } else if pin.direction==PinDirection::Input && pin.value_type==ValueType::Any && pin.cardinality==PinCardinality::One {
            inputs.push(pin);
        } else {return Err("UnitCall has no result/data output".into());}
    }
    if exec_in!=1 || exec_out!=1 || inputs.len()!=parameters.len() || caller.pins.iter().any(|(id,pin)|pin.node==call&&!seen.contains(id)) {
        return Err("UnitCall execution/arity must match its resolved declaration, including zero parameters".into());
    }
    let mut ordered = Vec::new();
    for parameter in parameters {
        let identity=parameter.parameter_id.as_ref().ok_or("Unit parameter identity absent")?;
        let matched:Vec<_>=inputs.iter().copied().filter(|pin|pin.parameter_id.as_ref()==Some(identity)
            &&pin.key==format!("argument_{}",identity.as_str())).collect();
        let [input]=matched.as_slice()else{return Err("Actual call argument identity is stale, duplicated or belongs to another callee".into());};
        ordered.push(*input);
    }
    Ok((*callee,ordered))
}

pub(crate) fn validate_project(project: &[GraphDocument]) -> Result<(),String> {
    crate::entry_parameter_identity1003::validate_project_parameter_metadata(project)?;
    let mut names=BTreeSet::new();
    for graph in project {
        if let GraphKind::UnitFunction{name}=&graph.kind {if !names.insert((graph.library1027.as_ref().and_then(|m|m.owner.as_ref()),name)){return Err("Duplicate Unit declaration in the same nominal namespace".into());}parameters(graph)?;}
        for node in graph.nodes.values() {
            if node.kind==NodeKind::UnitCall && graph.library_call1027(node.id).is_none() {resolve_call(project,graph,node.id)?;}
            if node.kind==NodeKind::UnitReturn {
                entry(graph)?;
                if node.pins.len()!=1 || graph.pins.get(&node.pins[0]).is_none_or(|pin|pin.key!="exec_in"||pin.direction!=PinDirection::Input||pin.value_type!=ValueType::Execution||pin.parameter_id.is_some()) {
                    return Err("UnitReturn is bare completion with exec_in only".into());
                }
            }
            if node.kind==NodeKind::FunctionCall {
                if graph.library_call1027(node.id).is_none() {if let Some(PropertyValue::String(name))=node.properties.get("function") {
                    if project.iter().any(|callee|callee.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_none()&&matches!(&callee.kind,GraphKind::UnitFunction{name:n}if n==name)) {return Err("A Unit declaration cannot be used as a value FunctionCall".into());}
                }}
            }
        }
    }
    Ok(())
}

/// Prepare actual immutable entry-pin declarations before importing any body,
/// so calls before declarations and recursion use the same canonical pins.
pub(crate) fn declarations(blocks:&[(GraphKind,rvn_parser::Script,Vec<String>)],previous:&[GraphDocument],
    allocate:&mut dyn FnMut()->Result<String,String>) -> Result<BTreeMap<String,GraphDocument>,String> {
    let mut declared=BTreeSet::new();
    for (kind,_,names) in blocks {
        if let GraphKind::UnitFunction{name}=kind {
            if !declared.insert(name) || rvn_parser::variable_binding_source_name(name).is_none()
                || names.len()>128 || names.iter().collect::<BTreeSet<_>>().len()!=names.len()
                || names.iter().any(|name|rvn_parser::variable_binding_source_name(name).is_none()) {
                return Err("Invalid/duplicate Unit declaration or lexical parameter; no identity allocated".into());
            }
        }
    }
    let mut result=BTreeMap::new();let mut identities=BTreeSet::new();
    for(index,(kind,_,names))in blocks.iter().enumerate(){
        let GraphKind::UnitFunction{name}=kind else{continue;};
        if result.contains_key(name){return Err("Duplicate Unit definition".into());}
        let mut graph=GraphDocument::new(GraphId::new(index as u64+1),kind.clone());graph.schema_version=GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION;
        let root=graph.add_catalog_node(NodeKind::FunctionEntry,[0.,0.]).map_err(|e|format!("{e:?}"))?;
        let old:Vec<_>=previous.iter().filter(|g|g.kind==*kind).collect();
        if old.is_empty() && !previous.is_empty() && !names.is_empty() {
            return Err("New, renamed or forked Unit text declaration requires explicit owning authoring/refactor mapping and prepared parameter identities; no identity allocated or guessed".into());
        }
        let retained=match old.as_slice(){[]=>None,[graph]=>Some(parameters(graph)?),_=>return Err("Ambiguous prior Unit owner".into())};
        if retained.as_ref().is_some_and(|pins|pins.iter().map(|p|p.label.as_str()).ne(names.iter().map(String::as_str))){
            return Err("Unit text signature changed without an explicit authoring/refactor mapping; no parameter identity guessed".into());
        }
        for(index,label)in names.iter().enumerate(){
            let identity=if let Some(pins)=&retained{pins[index].parameter_id.clone().ok_or("Prior opaque identity missing")?}else{ParameterId::parse(allocate()?)?};
            if !identities.insert(identity.clone()){return Err("Allocated/retained Unit parameter identity collision".into());}
            let pin=graph.add_pin(root,format!("parameter_{}",identity.as_str()),label,PinDirection::Output,ValueType::Any,PinCardinality::Many).map_err(|e|format!("{e:?}"))?;
            graph.pins.get_mut(&pin).ok_or("Parameter port allocation disappeared")?.parameter_id=Some(identity);
        }
        parameters(&graph)?;result.insert(name.clone(),graph);
    }
    Ok(result)
}

/// Scope compilation retains actual project authority, even for an arity-zero call.
pub(crate) fn compile_scope(graph:&GraphDocument, project:&[GraphDocument])->Result<TranspiledScript,TranspileError> {
    if crate::uses_unit_context1026(graph) {
        crate::codegen::transpile_unit_in_project(graph,project).map(|script|TranspiledScript{source:script.source,ast:script.ast})
    } else { crate::native_source_binding::compile_scope(graph,true) }
}

#[cfg(test)]
#[path="unit_graph_source_tests1003.rs"]
mod tests;
