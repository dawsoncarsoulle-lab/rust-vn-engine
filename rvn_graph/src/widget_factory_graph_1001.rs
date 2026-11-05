//! Widget factory authoring uses the existing source-owned Consumer and Engine.
//! Class data is not an object; outputs read the last actual assigned result.
use crate::{GraphDocument,GraphKind,GraphNode,NarrativeMetadata,NodeId,NodeKind,PinDirection,PropertyValue,ValueType};
use std::collections::BTreeSet;

impl GraphDocument {
    /// Static authoring eligibility, never a live Scene or source capability.
    /// A fragment retains its caller's activation; runtime admission still
    /// requires the actual reached active Scene and authenticated operands.
    pub fn widget_factory_authoring_context(&self)->Result<NarrativeMetadata,String>{
        if self.inspector_evaluation.is_legacy() || !matches!(self.kind,GraphKind::Label{..}) || self.narrative.is_none(){
            return Err("Create Widget authoring requires an explicitly opted-in Native Label with SceneEntry or SceneFragment metadata; execution requires the actual reached active Scene. Init/Handler factory-result continuation is not installed".into());
        }
        // This existing validator does not recurse through Graph::validate.
        self.validate_narrative_metadata()?;
        self.narrative.ok_or_else(||"Create Widget narrative metadata is absent".into())
    }
}

pub(crate) fn class_expression(node:&GraphNode)->Result<String,String>{
    let Some(PropertyValue::String(uuid))=node.properties.get("uuid") else{return Err("Widget Class requires its authored UUID or explicit empty class".into());};
    if uuid.is_empty(){return Ok("__rvn_widget_class_none_v1()".into());}
    rvn_parser::localized_text::validate_text_asset_id(uuid)?;
    Ok(format!("__rvn_widget_class_v1({})",serde_json::to_string(uuid).map_err(|e|e.to_string())?))
}
pub(crate) fn is_local_assignment(node:&GraphNode)->bool {
    node.kind==NodeKind::LocalVariable || node.kind==NodeKind::WidgetCreate&&node.properties.get("local")==Some(&PropertyValue::Bool(true))
}
pub(crate) fn result_name(node:&GraphNode)->Result<&str,String>{
    let Some(PropertyValue::String(name))=node.properties.get("name") else{return Err("Create Widget requires its actual source result binding".into());};
    if !rvn_parser::is_variable_binding_name(name)||name.starts_with("persistent."){return Err("Create Widget result binding is invalid or persistent".into());}
    Ok(name)
}
pub(crate) fn local(node:&GraphNode)->Result<bool,String>{match node.properties.get("local"){Some(PropertyValue::Bool(value))=>Ok(*value),_=>Err("Create Widget requires an explicit local/global result binding policy".into())}}

// Source-authoring dominance, not a runtime executor. Continuations follow the
// existing Rust emitter: Sequence then_0..then_N in order, then completed.
// Missing/uncertain structure refuses; no source identity or capability is made.
#[derive(Clone,PartialEq,Eq,PartialOrd,Ord)]
struct Resume { current:Option<NodeId>, owner:Option<NodeId>, range_exit:Option<NodeId> }
#[derive(Clone,PartialEq,Eq,PartialOrd,Ord)]
struct Path { current:Option<NodeId>, owner:Option<NodeId>, pending:Vec<Resume> }
fn successor(graph:&GraphDocument,node:NodeId,key:&str)->Result<Option<NodeId>,String>{
    let Some(pin)=graph.pin_by_key(node,key)else{return Ok(None)};
    let mut found=None;
    for edge in graph.edges.values().filter(|edge|edge.output==pin.id){
        let target=graph.pins.get(&edge.input).ok_or("Widget factory path has a missing input pin")?;
        if !graph.nodes.contains_key(&target.node){return Err("Widget factory path has a missing node".into());}
        if found.replace(target.node).is_some(){return Err("Widget factory execution output is ambiguous".into());}
    }
    Ok(found)
}
fn resume(mut path:Path)->Option<Path>{let next=path.pending.pop()?;path.current=next.current;path.owner=next.owner;Some(path)}
fn branch_path(graph:&GraphDocument,path:&Path,node:NodeId,key:&str,range_exit:Option<NodeId>)->Result<Path,String>{
    let mut branch=path.clone();branch.pending.push(Resume{current:successor(graph,node,"completed")?,owner:path.owner,range_exit});
    branch.current=successor(graph,node,key)?;branch.owner=Some(node);Ok(branch)
}
fn result_read_without_factory(graph:&GraphDocument,factory:NodeId,consumers:&BTreeSet<NodeId>,roots:&[NodeId])->Result<bool,String>{
    let mut pending:Vec<_>=roots.iter().map(|root|Path{current:Some(*root),owner:None,pending:vec![]}).collect();
    let mut seen=BTreeSet::new();let mut work=0usize;
    while let Some(mut path)=pending.pop(){
        work+=1;if work>65_536||pending.len()>65_536||path.pending.len()>128{return Err("Widget factory dominance expansion budget".into());}
        if !seen.insert(path.clone()){continue;}
        let Some(at)=path.current else{if let Some(next)=resume(path){pending.push(next);}continue;};
        let node=graph.nodes.get(&at).ok_or("Widget factory path has a missing node")?;
        // A ForLoopWithBreak back-edge is an explicit break, even through a
        // nested Sequence. Discard that loop's pending branch continuations.
        if node.kind==NodeKind::ForLoopWithBreak{
            if let Some(index)=path.pending.iter().rposition(|frame|frame.range_exit==Some(at)){
                path.pending.truncate(index+1);if let Some(next)=resume(path){pending.push(next);}continue;
            }
        }
        if consumers.contains(&at){return Ok(true);}
        if at==factory{continue;}
        if node.kind==NodeKind::BranchEnd{
            let Some(PropertyValue::Int(owner))=node.properties.get("owner")else{return Err("Widget factory path has no branch owner".into());};
            if *owner<0||path.owner!=Some(NodeId::new(*owner as u64)){return Err("Widget factory path has a mismatched branch owner".into());}
            if let Some(next)=resume(path){pending.push(next);}continue;
        }
        match node.kind{
            NodeKind::Sequence=>{
                let Some(PropertyValue::Int(count))=node.properties.get("output_count")else{return Err("Widget factory Sequence output count is missing".into());};
                if !(1..=32).contains(count){return Err("Widget factory Sequence output count is invalid".into());}
                path.pending.push(Resume{current:successor(graph,at,"completed")?,owner:path.owner,range_exit:None});
                for index in (1..*count).rev(){path.pending.push(Resume{current:successor(graph,at,&format!("then_{index}"))?,owner:Some(at),range_exit:None});}
                path.current=successor(graph,at,"then_0")?;path.owner=Some(at);pending.push(path);
            }
            NodeKind::If=>{pending.push(branch_path(graph,&path,at,"then",None)?);pending.push(branch_path(graph,&path,at,"else",None)?);}
            NodeKind::ForLoopWithBreak|NodeKind::While|NodeKind::ForEach=>{
                pending.push(branch_path(graph,&path,at,"body",(node.kind==NodeKind::ForLoopWithBreak).then_some(at))?);
                path.current=successor(graph,at,"completed")?;pending.push(path);
            }
            NodeKind::Choice|NodeKind::Imagemap=>{
                let (property,prefix)=if node.kind==NodeKind::Choice{("options","option_")}else{("hotspots","hotspot_")};
                let Some(PropertyValue::StringList(items))=node.properties.get(property)else{return Err("Widget factory path has invalid branch metadata".into());};
                if items.len()>65_536{return Err("Widget factory branch budget".into());}
                for index in 0..items.len(){pending.push(branch_path(graph,&path,at,&format!("{prefix}{index}"),None)?);}
                // Retain the established conservative empty/disabled-input
                // continuation; a factory in a selectable branch alone does
                // not prove availability after the interaction.
                path.current=successor(graph,at,"completed")?;pending.push(path);
            }
            NodeKind::Return|NodeKind::FunctionReturn|NodeKind::Jump=>{},
            NodeKind::Reroute=>{path.current=successor(graph,at,"value_out")?;pending.push(path);}
            _=>{path.current=successor(graph,at,"exec_out")?;pending.push(path);}
        }
    }
    Ok(false)
}

// Factory admission checks the actual producer/default value, never the
// receiving pin's requested class/list type. A literal default must satisfy
// the same contract as a wire; Any cannot manufacture a class capability.
fn validate_inputs(graph:&GraphDocument,node:&GraphNode,errors:&mut Vec<(NodeId,String)>){
    for key in ["class","arguments"]{
        let Some(pin)=graph.pin_by_key(node.id,key)else{errors.push((node.id,format!("Create Widget input {key} is missing")));continue;};
        let mut incoming=graph.edges.values().filter(|edge|edge.input==pin.id);
        let edge=incoming.next();
        if incoming.next().is_some(){errors.push((node.id,format!("Create Widget input {key} has ambiguous sources")));continue;}
        let actual=if let Some(edge)=edge{graph.effective_pin_type(edge.output)}
            else{pin.default_value.as_ref().map(crate::type_inference::property_type)};
        let valid=match(key,actual.as_ref()){
            ("class",Some(ValueType::WidgetClass))=>true,
            ("arguments",Some(ValueType::List(_)))=>true,
            _=>false,
        };
        if !valid{errors.push((node.id,format!("Create Widget input {key} requires {}, not {actual:?}",if key=="class"{"a typed Widget class producer"}else{"an argument list"})));}
    }
}

pub(crate) fn validate(graph:&GraphDocument)->Vec<(NodeId,String)>{
    let mut errors=Vec::new();
    let context_error=graph.nodes.values().any(|node|node.kind==NodeKind::WidgetCreate)
        .then(||graph.widget_factory_authoring_context().err()).flatten();
    let roots:Vec<_>=graph.nodes.values().filter(|n|matches!(n.kind,NodeKind::Label|NodeKind::Init|NodeKind::FunctionEntry|NodeKind::ScreenEntry|NodeKind::HandlerEntry)).map(|n|n.id).collect();
    for node in graph.nodes.values(){
        if node.kind==NodeKind::WidgetClassValue{if let Err(e)=class_expression(node){errors.push((node.id,e));}continue;}
        if node.kind!=NodeKind::WidgetCreate{continue;}
        validate_inputs(graph,node,&mut errors);
        if let Some(error)=&context_error{errors.push((node.id,error.clone()));}
        let name=match result_name(node){Ok(v)=>v,Err(e)=>{errors.push((node.id,e));continue;}};
        if let Err(e)=local(node){errors.push((node.id,e));}
        if graph.nodes.values().any(|other|other.id!=node.id&&matches!(other.kind,NodeKind::WidgetCreate|NodeKind::SetVariable|NodeKind::LocalVariable)&&other.properties.get("name")==Some(&PropertyValue::String(name.into()))){errors.push((node.id,"Create Widget output binding is overwritten or ambiguous; retain a distinct result binding".into()));}
        let mut pending=vec![node.id];
        pending.extend(graph.nodes.values().filter(|n|n.kind==NodeKind::VariableGet&&n.properties.get("name")==Some(&PropertyValue::String(name.into()))).map(|n|n.id));
        let mut seen=BTreeSet::new();let mut consumers=BTreeSet::new();
        while let Some(source)=pending.pop(){if !seen.insert(source){continue;}
            for edge in graph.edges.values(){let(Some(a),Some(b))=(graph.pins.get(&edge.output),graph.pins.get(&edge.input))else{continue;};if a.node!=source||a.value_type.is_execution(){continue;}
                if let Some(target)=graph.nodes.get(&b.node){if target.pins.iter().filter_map(|p|graph.pins.get(p)).any(|p|p.direction==PinDirection::Input&&p.value_type.is_execution()){consumers.insert(b.node);}else{pending.push(b.node);}}
            }
        }
        match result_read_without_factory(graph,node.id,&consumers,&roots){
            Ok(true)=>errors.push((node.id,"Create Widget result is read before its actual factory or from a path not dominated by it".into())),
            Ok(false)=>{},Err(error)=>errors.push((node.id,error)),
        }
    }
    errors
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::{ValueType,PropertyValue};
 use rvn_parser::{Expr,Statement};
 const UUID:&str="17411a3e431e4c9388b78a2b9825b130";
 fn scene(source:&str)->GraphDocument{
  let program=rvn_parser::parse_program(source).unwrap();
  let mut graphs=crate::import_script(&program.script).unwrap();
  let at=graphs.iter().position(|g|matches!(&g.kind,GraphKind::Label{name}if name=="Play")).unwrap();
  let mut graph=graphs.remove(at);graph.enable_native_frame_preparation().unwrap();
  // An explicit fixture authoring choice, not a production Int-to-object cast.
  if let Some(copy)=graph.variables.get_mut("copied"){copy.value_type=ValueType::WidgetObject;}
  let pins:Vec<_>=graph.nodes.values().filter(|n|n.properties.get("name")==Some(&PropertyValue::String("copied".into()))).flat_map(|n|n.pins.clone()).collect();
  for pin in pins{let pin=graph.pins.get_mut(&pin).unwrap();if matches!(pin.key.as_str(),"value"|"value_out"){pin.value_type=ValueType::WidgetObject;}}
  graph
 }
 fn assignment(graph:&GraphDocument,name:&str)->NodeId{graph.nodes.values().find(|n|matches!(n.kind,NodeKind::SetVariable|NodeKind::LocalVariable)&&n.properties.get("name")==Some(&PropertyValue::String(name.into()))).unwrap().id}
 fn class()->Expr{Expr::Call{name:"__rvn_widget_class_v1".into(),args:vec![Expr::Str(UUID.into())]}}
 fn args()->Expr{Expr::ListLit(vec![Expr::Call{name:"random".into(),args:vec![Expr::Int(1),Expr::Int(9)]}])}
 #[test]
 fn factory_graph1001_class_and_object_are_distinct_typed_values_without_implicit_conversion(){
  assert!(!ValueType::WidgetClass.accepts(&ValueType::WidgetObject));
  assert!(!ValueType::WidgetClass.accepts(&ValueType::String));
  assert!(!ValueType::WidgetObject.accepts(&ValueType::WidgetClass));
  assert!(ValueType::WidgetClass.conversion_from(&ValueType::String).is_none());
  let mut graph=scene("scene label Play\nset made=0\n\"Ready\"\nreturn\n");
  let node=graph.add_catalog_node(NodeKind::WidgetClassValue,[0.,0.]).unwrap();
  assert_eq!(class_expression(&graph.nodes[&node]).unwrap(),"__rvn_widget_class_none_v1()");
  graph.set_property(node,"uuid",PropertyValue::String(UUID.into())).unwrap();
  assert!(class_expression(&graph.nodes[&node]).unwrap().contains(UUID));
  graph.set_property(node,"uuid",PropertyValue::String("arbitrary-name".into())).unwrap();
  assert!(class_expression(&graph.nodes[&node]).is_err());
 }
 #[test]
 fn factory_graph1001_real_factory_exports_v2_with_one_command_and_lossless_owned_identities(){
  let mut graph=scene("scene label Play\nset made=0\nset copied=made\n\"Ready\"\nreturn\n");
  let old=assignment(&graph,"made");let factory=graph.replace_assignment_with_widget_factory(old,&class(),&args()).unwrap();
  let copy=assignment(&graph,"copied");let input=graph.pin_by_key(copy,"value").unwrap().id;
  let edge=graph.edges.values().find(|e|e.input==input).unwrap().clone();
  let read=graph.pins[&edge.output].node;assert_eq!(graph.nodes[&read].kind,NodeKind::VariableGet);
  graph.remove_edge(edge.id);graph.remove_node(read);
  graph.connect(graph.pin_by_key(factory,"value_out").unwrap().id,input).unwrap();
  assert_eq!(graph.pin_by_key(factory,"class").unwrap().value_type,ValueType::WidgetClass);
  assert_eq!(graph.pin_by_key(factory,"value_out").unwrap().value_type,ValueType::WidgetObject);
  let compiled=crate::prepare_native_frame_graph(&graph).unwrap();
  let factory_plans:Vec<_>=compiled.ast.iter().filter_map(|s|if let Statement::ScheduledConsumer{version,scope,body,..}=s{body.first().and_then(|leaf|rvn_parser::widget_create_effect::parts(leaf).ok().flatten()).map(|_|(*version,scope))}else{None}).collect();
  assert_eq!(factory_plans.len(),1);assert_eq!(factory_plans[0].0,2);assert!(factory_plans[0].1.starts_with(&format!("g{}/",graph.graph_id)));
  let recovered=crate::import_native_frame_script(&compiled.ast).unwrap();
  let same=recovered.iter().find(|g|g.graph_id==graph.graph_id).unwrap();
  assert_eq!(same.edges,graph.edges);assert_eq!(same.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
  assert_eq!(crate::prepare_native_frame_graph(same).unwrap().ast,compiled.ast);
  let wrong_kind=compiled.source.replace(":widget_create|",":set_variable|");
  let parsed=rvn_parser::parse_program(&wrong_kind).unwrap();
  assert!(rvn_parser::scheduled_source::scheduled_step_sources(&parsed).is_err());
  let wrong_version=compiled.source.replace("flow.consume_v2(","flow.consume_v1(");
  if let Ok(parsed)=rvn_parser::parse_program(&wrong_version){assert!(rvn_parser::scheduled_source::scheduled_step_sources(&parsed).is_err());}
  let wrong_class=compiled.source.replace(":widget_class_value|",":literal|");
  let parsed=rvn_parser::parse_program(&wrong_class).unwrap();
  assert!(crate::import_native_frame_script(&parsed.script).is_err());
 }
 #[test]
 fn factory_graph1001_refuses_a_real_result_read_before_factory_without_editing_the_document(){
  let mut graph=scene("init{set made=0}\nscene label Play\nset copied=made\nset made=0\n\"Ready\"\nreturn\n");
  let before=serde_json::to_vec(&graph).unwrap();let node=assignment(&graph,"made");
  let error=graph.replace_assignment_with_widget_factory(node,&class(),&args()).unwrap_err();
  assert!(error.contains("dominated"),"{error}");assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
 }
 #[test]
 fn factory_graph1001_wrong_class_or_arguments_refuse_transactionally_without_any_any_cast(){
  for(class,args)in [(Expr::Str(UUID.into()),Expr::ListLit(vec![])),(class(),Expr::Int(99))]{
   let mut graph=scene("scene label Play\nset made=0\n\"Ready\"\nreturn\n");let before=serde_json::to_vec(&graph).unwrap();let node=assignment(&graph,"made");
   assert!(graph.replace_assignment_with_widget_factory(node,&class,&args).is_err());assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
  }
  // Direct authored defaults must face the same admission as expression
  // replacement. No string UUID or integer becomes a class/list by its socket.
  for key in ["class","arguments"]{
   let mut graph=scene("scene label Play\nset made=0\n\"Ready\"\nreturn\n");let old=assignment(&graph,"made");
   let factory=graph.replace_assignment_with_widget_factory(old,&class(),&args()).unwrap();
   let input=graph.pin_by_key(factory,key).unwrap().id;
   let incoming:Vec<_>=graph.edges.values().filter(|edge|edge.input==input).map(|edge|edge.id).collect();
   for edge in incoming{graph.remove_edge(edge);}
   graph.set_pin_default(factory,key,if key=="class"{PropertyValue::String(UUID.into())}else{PropertyValue::Int(99)}).unwrap();
   let before=serde_json::to_vec(&graph).unwrap();
   assert!(graph.validate().iter().any(|diagnostic|diagnostic.code=="G_WIDGET_FACTORY"&&diagnostic.message.contains(key)));
   assert!(crate::prepare_native_frame_graph(&graph).is_err());assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
  }
  // A late invalid member of a coherent edit batch must roll back the first
  // valid factory and all actual allocator, binding, pin and edge changes.
  for bad in [Expr::Str(UUID.into()),Expr::Int(99)]{
   let mut graph=scene("scene label Play\nset made=0\nset another=0\n\"Ready\"\nreturn\n");
   let first=assignment(&graph,"made");let second=assignment(&graph,"another");let before=serde_json::to_vec(&graph).unwrap();
   let invalid=if matches!(&bad,Expr::Str(_)){(second,bad,Expr::ListLit(vec![]))}else{(second,class(),bad)};
   let changes=[(first,class(),args()),invalid];
   assert!(graph.replace_assignments_with_widget_factories(&changes).is_err());assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
  }
 }
 #[test]
 fn factory_graph1001_runtime_result_output_never_serializes_a_live_reference_or_replays_the_factory(){
  let mut graph=scene("scene label Play\nset made=0\nset copied=made\n\"Ready\"\nreturn\n");let node=assignment(&graph,"made");graph.replace_assignment_with_widget_factory(node,&class(),&args()).unwrap();
  let json=serde_json::to_string(&graph).unwrap();assert!(!json.contains("runtime_epoch"));
  let loaded:GraphDocument=serde_json::from_str(&json).unwrap();let source=crate::prepare_native_frame_graph(&loaded).unwrap().source;
  assert_eq!(source.matches("__rvn_widget_create_v1(").count(),1);
  assert!(source.contains("widget_create")&&source.contains("widget_class_value"));
 }
 #[test]
 fn factory_graph1001_sequence_continuations_preserve_order_and_refuse_the_real_reverse_read(){
  for(reverse,body)in [
   (false,"flow.sequence(){{set made=0}{set copied=made}}"),
   (false,"flow.sequence(){{flow.sequence(){{set made=0}{}}}{set copied=made}}"),
   (true,"flow.sequence(){{set copied=made}{set made=0}}"),
   (true,"flow.sequence(){{if random(0,1)==1{set made=0}}{set copied=made}}"),
  ]{
   let mut graph=scene(&format!("init{{set made=0}}\nscene label Play\n{body}\n\"Ready\"\nreturn\n"));
   let before=serde_json::to_vec(&graph).unwrap();let assignment=assignment(&graph,"made");
   let result=graph.replace_assignment_with_widget_factory(assignment,&class(),&args());
   if reverse{assert!(result.unwrap_err().contains("dominated"));assert_eq!(serde_json::to_vec(&graph).unwrap(),before);}
   else{result.unwrap();let compiled=crate::prepare_native_frame_graph(&graph).unwrap();assert!(compiled.source.contains("flow.sequence()"));let imported=crate::import_native_frame_script(&compiled.ast).unwrap();let same=imported.iter().find(|g|g.graph_id==graph.graph_id).unwrap();assert_eq!(crate::prepare_native_frame_graph(same).unwrap().ast,compiled.ast);}
  }
 }
 #[test]
 fn factory_graph1001_orphan_input_node_reports_structure_without_panicking_or_mutating(){
  let mut graph=scene("scene label Play\nset made=0\nset copied=made\n\"Ready\"\nreturn\n");let old=assignment(&graph,"made");
  graph.replace_assignment_with_widget_factory(old,&class(),&args()).unwrap();
  let copy=assignment(&graph,"copied");let pin=graph.pin_by_key(copy,"value").unwrap().id;
  let missing=NodeId::new(graph.nodes.keys().map(|node|node.get()).max().unwrap()+1);graph.pins.get_mut(&pin).unwrap().node=missing;
  let before=serde_json::to_vec(&graph).unwrap();let diagnostics=graph.validate();assert!(diagnostics.iter().any(|d|d.code=="missing_node"));
  assert!(crate::prepare_native_frame_graph(&graph).is_err());assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
 }

 #[test]
 fn factory_graph1001_real_loop_and_nearest_break_matrix_keeps_dominance_and_owned_continuations(){
  let cases=[
   ("while-zero",false,"while false{set made=0} set copied=made"),
   ("foreach-zero",false,"for item in []{set made=0} set copied=made"),
   ("range-zero",false,"flow.for_loop(index,1,0){set made=0} set copied=made"),
   ("before-while",true,"set made=0 while random(0,1)==1{set copied=made}"),
   ("before-foreach",true,"set made=0 for item in [1]{set copied=made}"),
   ("before-range",true,"set made=0 flow.for_loop(index,0,1){set copied=made}"),
   // A real range break skips the remaining Sequence branches. Even a read
   // textually preceding its late factory is unreachable, not a bypass.
   ("range-break-skips-read",true,"flow.for_loop(index,0,1){flow.sequence(){{flow.break()}{set copied=made}{set made=0}}}"),
   ("range-break-cannot-prove-completed",false,"flow.for_loop(index,0,1){flow.sequence(){{flow.break()}{set made=0}}} set copied=made"),
   // The inner break returns to inner completed. The outer continuation is
   // live and its result read cannot borrow the skipped inner factory.
   ("nearest-inner-break-return",false,"flow.for_loop(outer,0,1){flow.sequence(){{flow.for_loop(inner,0,1){flow.sequence(){{flow.break()}{set made=0}}}}{set copied=made}}}"),
   ("nested-break-skips-inner-read",true,"flow.for_loop(outer,0,1){flow.for_loop(inner,0,1){flow.sequence(){{flow.break()}{set copied=made}{set made=0}}}}"),
  ];
  for(name,accepted,body)in cases{
   let mut graph=scene(&format!("init{{set made=0}}\nscene label Play\n{body}\n\"Ready\"\nreturn\n"));
   let loops:Vec<_>=graph.nodes.values().filter(|node|matches!(node.kind,NodeKind::ForEach|NodeKind::ForLoopWithBreak)).map(|node|node.id).collect();
   for owner in loops{graph=graph.prepare_native_loop_demand(owner).unwrap().graph;}
   if name.starts_with("nearest-")||name.starts_with("nested-"){
    let inner=graph.nodes.values().find(|node|node.kind==NodeKind::ForLoopWithBreak&&node.properties.get("name")==Some(&PropertyValue::String("inner".into()))).unwrap().id;
    let breaks=graph.edges.values().filter(|edge|graph.pins[&edge.input].node==inner&&graph.pins[&edge.input].key=="break"&&graph.nodes[&graph.pins[&edge.output].node].kind==NodeKind::Sequence).count();
    assert_eq!(breaks,1,"{name}: source importer must target the nearest actual range owner");
   }
   let before=serde_json::to_vec(&graph).unwrap();let old=assignment(&graph,"made");let changed=graph.replace_assignment_with_widget_factory(old,&class(),&args());
   if accepted{
    changed.unwrap_or_else(|error|panic!("{name}: {error}"));let prepared=crate::prepare_native_frame_graph(&graph).unwrap_or_else(|error|panic!("{name}: {error:?}"));
    if name.contains("break"){assert!(prepared.source.contains("flow.break()"),"{name}");}
    let recovered=crate::import_native_frame_script(&prepared.ast).unwrap_or_else(|error|panic!("{name}: {error}"));let same=recovered.iter().find(|g|g.graph_id==graph.graph_id).unwrap();assert_eq!(crate::prepare_native_frame_graph(same).unwrap().ast,prepared.ast,"{name}");
   }else{let error=changed.unwrap_err();assert!(error.contains("dominated"),"{name}: {error}");assert_eq!(serde_json::to_vec(&graph).unwrap(),before,"{name}");}
  }
 }

 fn authoring_contexts1002()->Vec<(&'static str,GraphDocument,Option<NarrativeMetadata>)>{
  let label=||GraphKind::Label{name:"context".into()};
  [
   ("CTX-INIT-LEGACY",GraphKind::Init,false,None),
   ("CTX-INIT-NATIVE",GraphKind::Init,true,None),
   ("CTX-LABEL-LEGACY",label(),false,None),
   ("CTX-GAME",label(),true,Some(NarrativeMetadata::SceneFragment)),
   ("CTX-SCENE",label(),true,Some(NarrativeMetadata::SceneEntry)),
   ("CTX-FRAGMENT",label(),true,Some(NarrativeMetadata::SceneFragment)),
   ("CTX-FUNCTION",GraphKind::Function{name:"pure".into()},false,None),
   ("CTX-SCREEN",GraphKind::Screen{name:"view".into()},false,None),
   ("CTX-HANDLER",GraphKind::Handler{name:"event".into()},false,None),
   ("CTX-IMAGEMAP",GraphKind::Imagemap{name:"map".into()},false,None),
  ].into_iter().enumerate().map(|(index,(name,kind,native,metadata))|{
   let mut graph=GraphDocument::new(crate::GraphId::new(100200+index as u64),kind);
   graph.set_narrative_metadata(metadata).unwrap();
   if native{graph.enable_native_frame_preparation().unwrap();}
   // CTX-GAME is conditionally authorable as a fragment. Only the actual
   // runtime caller can grant/reject its Scene activation; this is no grant.
   (name,graph,metadata.filter(|_|native))
  }).collect()
 }
 #[test]
 fn factory_context1002_ten_documents_share_static_admission_without_hiding_class_data(){
  for(name,mut graph,expected)in authoring_contexts1002(){
   let before=serde_json::to_vec(&graph).unwrap();
   let context=graph.widget_factory_authoring_context();
   assert_eq!(context.as_ref().ok().copied(),expected,"{name}");
   assert_eq!(serde_json::to_vec(&graph).unwrap(),before,"{name}");
   let class=graph.add_catalog_node(NodeKind::WidgetClassValue,[0.,0.]).unwrap();
   assert_eq!(class_expression(&graph.nodes[&class]).unwrap(),"__rvn_widget_class_none_v1()");
   let factory=graph.add_catalog_node(NodeKind::WidgetCreate,[200.,0.]).unwrap();
   let diagnostics=validate(&graph);
   if let Err(error)=context{
    assert!(diagnostics.iter().any(|(node,message)|*node==factory&&message==&error),"{name}: {diagnostics:?}");
   }else{
    assert!(!diagnostics.iter().any(|(_,message)|message.contains("authoring requires")),"{name}");
   }
  }
 }
 #[test]
 fn factory_context1002_corrupt_metadata_is_shared_by_validation_and_transactional_replacement(){
  for change in 0..4{
   let mut graph=scene("scene label Play\nset made=0\n\"Ready\"\nreturn\n");
   let old=assignment(&graph,"made");
   match change{
    0=>graph.narrative=None,
    1=>graph.inspector_evaluation=crate::InspectorEvaluationContract::LegacyStateless,
    2=>graph.schema_version=0,
    _=>graph.kind=GraphKind::Handler{name:"event".into()},
   }
   let error=graph.widget_factory_authoring_context().unwrap_err();
   let before=serde_json::to_vec(&graph).unwrap();
   assert_eq!(graph.replace_assignment_with_widget_factory(old,&class(),&args()).unwrap_err(),error);
   assert_eq!(serde_json::to_vec(&graph).unwrap(),before);
   let factory=graph.add_catalog_node(NodeKind::WidgetCreate,[200.,0.]).unwrap();
   assert!(validate(&graph).iter().any(|(node,message)|*node==factory&&message==&error));
  }
 }
 #[test]
 fn factory_context1002_scene_and_fragment_keep_real_factory_source_roundtrip(){
  for metadata in [NarrativeMetadata::SceneEntry,NarrativeMetadata::SceneFragment]{
   // A fragment is authored inside a real explicit-Scene project. Merely
   // changing cached metadata cannot make its standalone `label` a Scene.
   let source=if metadata==NarrativeMetadata::SceneEntry{
    "scene label Play\nset made=0\n\"Ready\"\nreturn\n"
   }else{
    "scene label Entry\njump Play\nlabel Play\nset made=0\n\"Ready\"\nreturn\n"
   };
   let mut graphs=crate::import_script(&rvn_parser::parse(source).unwrap()).unwrap();
   let index=graphs.iter().position(|g|matches!(&g.kind,GraphKind::Label{name}if name=="Play")).unwrap();
   let graph=&mut graphs[index];
   graph.enable_native_frame_preparation().unwrap();
   assert_eq!(graph.narrative,Some(metadata));
   assert_eq!(graph.widget_factory_authoring_context().unwrap(),metadata);
   let old=assignment(graph,"made");
   graph.replace_assignment_with_widget_factory(old,&class(),&args()).unwrap();
   if metadata==NarrativeMetadata::SceneEntry{
    let prepared=crate::prepare_native_frame_graph(graph).unwrap();
    let recovered=crate::import_native_frame_script(&prepared.ast).unwrap();
    let same=recovered.iter().find(|g|g.graph_id==graph.graph_id).unwrap();
    assert_eq!(same.narrative,Some(metadata));
    assert_eq!(crate::prepare_native_frame_graph(same).unwrap().ast,prepared.ast);
   }else{
    let before=serde_json::to_vec(graph).unwrap();
    let error=crate::prepare_native_frame_graph(graph).unwrap_err().to_string();
    assert!(error.contains("G_WIDGET_FACTORY")&&error.contains("SceneEntry or SceneFragment metadata"),"{error}");
    assert_eq!(serde_json::to_vec(graph).unwrap(),before);
   }
   let graph_id=graph.graph_id;
   let prepared=crate::transpile_project(&graphs).unwrap();
   let recovered=crate::import_native_frame_script(&prepared.ast).unwrap();
   let same=recovered.iter().find(|g|g.graph_id==graph_id).unwrap();
   let original=&graphs[index];
   assert_eq!(same.narrative,Some(metadata));
   assert_eq!(same.edges,original.edges);
   assert_eq!(same.nodes.keys().collect::<Vec<_>>(),original.nodes.keys().collect::<Vec<_>>());
   assert_eq!(same.pins.keys().collect::<Vec<_>>(),original.pins.keys().collect::<Vec<_>>());
   assert_eq!(crate::transpile_project(&recovered).unwrap().ast,prepared.ast);
  }
 }

}
