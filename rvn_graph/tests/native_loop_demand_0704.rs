//! Explicit migration -> real Graph pins -> .rvn -> real Init owner.
//! Prepared, NOT RUN. These CPU tests add no native or pixel acceptance.
use rvn_graph::*;
use rvn_core::{Engine,TerminalRenderer};
use rvn_core::engine::EngineStartup;
use rvn_core::editor_debugger::StepKind;
use rvn_parser::Value;
use std::{collections::HashMap,path::PathBuf,sync::atomic::{AtomicU64,Ordering}};
fn root(name:&str)->PathBuf{
    static NEXT:AtomicU64=AtomicU64::new(0);
    let base=PathBuf::from(std::env::var("RVN_NATIVE_LOOP0704_FIXTURES").expect("isolated fixture root on D required"));
    let path=base.join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir_all(&path).unwrap();path
}
fn finish(owner:&mut EngineStartup<TerminalRenderer>){for _ in 0..512{if owner.is_complete(){return;}owner.step(StepKind::Continue).unwrap();}panic!("bounded fixture did not finish");}
fn seed(name:&str,source:&str,kind:NodeKind)->(Vec<GraphDocument>,usize,NodeId,PathBuf){
    let mut graphs=import_script(&rvn_parser::parse(source).unwrap()).unwrap();
    let index=graphs.iter().position(|g|g.kind==GraphKind::Init).unwrap();let owner=graphs[index].nodes.values().find(|n|n.kind==kind).unwrap().id;
    for graph in &mut graphs{graph.enable_native_frame_preparation().unwrap();}
    graphs[index]=graphs[index].prepare_native_loop_demand(owner).unwrap().graph;
    let path=root(name);(graphs,index,owner,path)
}
fn load(graphs:&[GraphDocument],path:&std::path::Path)->(EngineStartup<TerminalRenderer>,String){
    let compiled=transpile_project(graphs).expect("actual Graph/source project must pass authoritative roundtrip");
    assert!(compiled.source.contains("flow.consume_v3("));assert_eq!(rvn_parser::parse(&compiled.source).unwrap(),compiled.ast);
    let file=path.join("native.rvn");std::fs::write(&file,&compiled.source).unwrap();
    let program=rvn_parser::parse_file_program_with_uses(&file).unwrap();
    let imported=import_native_frame_script(&program.script).unwrap();
    let again=transpile_project(&imported).unwrap();assert_eq!(again.ast,compiled.ast,"actual source import must preserve the explicit contract");
    let owner=Engine::prepare_debugger_startup(program,TerminalRenderer,16,HashMap::new(),Default::default(),None).unwrap();assert!(!owner.is_complete());(owner,compiled.source)
}
fn output_to_value(g:&mut GraphDocument,owner:NodeId,role:&str,name:&str){
    let assignment=g.nodes.values().find(|n|n.kind==NodeKind::SetVariable&&g.variable_node_name(n.id).as_deref()==Some(name)).unwrap().id;
    let input=g.pin_by_key(assignment,"value").unwrap().id;
    // This explicit edit replaces the old variable read only at this selected
    // input; no migration guesses that all named item reads mean ArrayElement.
    let edges:Vec<_>=g.edges.values().filter(|e|e.input==input).map(|e|(e.id,g.pins[&e.output].node)).collect();for (id,_) in &edges{g.edges.remove(id);}
    g.connect(g.pin_by_key(owner,role).unwrap().id,input).unwrap();
    // Remove only the genuinely detached probe GET/literal created by import,
    // using the actual node API after its last outgoing edge is gone.
    let unused:Vec<_>=edges.iter().map(|(_,node)|*node).filter(|id|{let n=&g.nodes[id];matches!(n.kind,NodeKind::VariableGet|NodeKind::Literal)&&!n.pins.iter().any(|p|g.edges.values().any(|e|e.output==*p||e.input==*p))}).collect();
    for node in unused{g.remove_node(node).unwrap();}
}

const SUPPLIER:&str="function supplier(){local token=random(1,1) return [11,22,33]}";
#[test]
fn native0704_real_graph_index_output_has_four_header_calls_and_exact_source_roundtrip(){
    let raw=format!("{SUPPLIER}\ninit{{set visits=0 for item in supplier(){{set visits=visits+1 set seen=0}} set done=true}}\n");
    let(mut graphs,index,loop_node,path)=seed("index",&raw,NodeKind::ForEach);output_to_value(&mut graphs[index],loop_node,"array_index","seen");
    let(mut owner,source)=load(&graphs,&path);let actual_index=rvn_parser::scheduled_source::native_loop_contract::index_name(graphs[index].graph_id.get(),loop_node.get());assert!(source.contains(&actual_index));assert!(rvn_parser::is_variable_binding_name(&actual_index));let mut expected=owner.state().random;for _ in 0..4{expected.integer(1,1).unwrap();}
    finish(&mut owner);assert_eq!(owner.state().random,expected);assert_eq!(owner.state().vars["visits"],Value::Int(3));assert_eq!(owner.state().vars["seen"],Value::Int(2));assert_eq!(owner.state().vars["done"],Value::Bool(true));assert!(!owner.state().vars.keys().any(|name|name.starts_with("rvn_native_")));
}
#[test]
fn native0704_real_graph_element_output_has_seven_calls_and_no_synthetic_get(){
    let raw=format!("{SUPPLIER}\ninit{{set visits=0 for item in supplier(){{set visits=visits+1 set seen=0}} set done=true}}\n");
    let(mut graphs,index,loop_node,path)=seed("element",&raw,NodeKind::ForEach);output_to_value(&mut graphs[index],loop_node,"array_element","seen");
    let(mut owner,source)=load(&graphs,&path);let program=rvn_parser::parse_file_program_with_uses(path.join("native.rvn")).unwrap();let map=NativeDebugSourceMap::from_program(&program).unwrap();
    let endpoint=graphs[index].pin_by_key(loop_node,"array_element").unwrap();let graph_id=graphs[index].graph_id;
    assert!(map.origins().iter().any(|origin|origin.graph==graph_id&&origin.node==loop_node&&matches!(&origin.role,NativeOriginRole::Producer{native_loop_output:Some(role),..}if role=="array_element")));
    assert!(source.contains("__rvn_captured_v1"));let selected=map.endpoint(&path.join("native.rvn").canonicalize().unwrap(),graph_id,loop_node,endpoint.id);assert!(selected.iter().any(|o|matches!(o.role,NativeOriginRole::Producer{..})));
    let mut expected=owner.state().random;for _ in 0..7{expected.integer(1,1).unwrap();}finish(&mut owner);assert_eq!(owner.state().random,expected);assert_eq!(owner.state().vars["seen"],Value::Int(33));
}
#[test]
fn native0704_real_graph_range_uses_current_last_and_nearest_break(){
    for (body,count) in [("set visits=visits+1 set last=0",1),("set visits=visits+1 set last=4",5),("set visits=visits+1 flow.break()",1)]{
        let raw=format!("init{{set visits=0 set last=2 flow.for_loop(i,0,last){{{body}}} set done=true}}\n");
        let(graphs,_,_,path)=seed("range",&raw,NodeKind::ForLoopWithBreak);let(mut owner,_)=load(&graphs,&path);finish(&mut owner);assert_eq!(owner.state().vars["visits"],Value::Int(count));assert_eq!(owner.state().vars["done"],Value::Bool(true));
    }
}
#[test]
fn native0704_real_graph_function_owner_exports_explicit_loop_instead_of_snapshot(){
    let raw="function test(){local visits=0 for item in [1,2,3]{set visits=visits+1} return visits}\n";let graphs=import_script(&rvn_parser::parse(raw).unwrap()).unwrap();let graph=graphs.into_iter().find(|g|matches!(g.kind,GraphKind::Function{..})).unwrap();let node=graph.nodes.values().find(|n|n.kind==NodeKind::ForEach).unwrap().id;
    let before=graph.to_pretty_json().unwrap();
    let migration=graph.prepare_native_loop_demand(node).unwrap();
    assert_eq!(graph.to_pretty_json().unwrap(),before,"preparation cannot mutate the old authored graph");
    assert!(migration.legacy_named_reads.is_empty());
    let compiled=transpile(&migration.graph).expect("the existing lexical backend now has an actual Function root");
    assert!(compiled.source.contains("flow.consume_v3("));
    assert_eq!(rvn_parser::parse(&compiled.source).unwrap(),compiled.ast);
    let restored=import_native_frame_script(&compiled.ast).unwrap();
    let again=transpile_project(&restored).unwrap();
    let function=|ast:&rvn_parser::Script|ast.iter().find(|s|matches!(s,rvn_parser::Statement::Function{name,..}if name=="test")).cloned().unwrap();
    assert_eq!(function(&again.ast),function(&compiled.ast));
}
#[test]
fn native0704_real_graph_after_completed_output_is_not_lent_a_body_index(){
    let raw="init{for item in [1,2,3]{} set seen=0}\n";
    let(mut graphs,index,node,_)=seed("scope",raw,NodeKind::ForEach);output_to_value(&mut graphs[index],node,"array_index","seen");assert!(transpile_project(&graphs).is_err());
}
#[test]
fn native0704_real_graph_first_and_last_have_distinct_original_phase_ordinals(){
    let raw="function first(){local token=random(1,1) return 0}\nfunction last(){local token=random(1,1) return 2}\ninit{set visits=0 flow.for_loop(i,first(),last()){set visits=visits+1} set done=true}\n";
    let(graphs,_,_,path)=seed("phase-dag",raw,NodeKind::ForLoopWithBreak);let(mut owner,source)=load(&graphs,&path);
    let ast=rvn_parser::parse(&source).unwrap();let body=ast.iter().find_map(|s|if let rvn_parser::Statement::Init{body}=s{Some(body)}else{None}).unwrap();let statement=body.iter().find(|s|matches!(s,rvn_parser::Statement::ScheduledConsumer{version:3,..})).unwrap();
    let rvn_parser::Statement::ScheduledConsumer{steps,..}=statement else{unreachable!()};
    let first=rvn_parser::scheduled_source::native_loop_contract::phase_expression(statement,rvn_parser::scheduled_source::native_loop_contract::Phase::First).unwrap();let last=rvn_parser::scheduled_source::native_loop_contract::phase_expression(statement,rvn_parser::scheduled_source::native_loop_contract::Phase::Test).unwrap();
    let a=rvn_parser::loop_phase_dependencies::capture_step_indices(steps,first).unwrap();let b=rvn_parser::loop_phase_dependencies::capture_step_indices(steps,last).unwrap();assert!(!a.is_empty());assert!(!b.is_empty());assert_ne!(a,b);
    let mut expected=owner.state().random;for _ in 0..5{expected.integer(1,1).unwrap();}finish(&mut owner);assert_eq!(owner.state().random,expected);assert_eq!(owner.state().vars["visits"],Value::Int(3));
}
#[test]
fn native0704_real_last_producer_pending_can_cold_restore_with_original_ordinals(){
    let raw="function first(){local token=random(1,1) return 0}\nfunction last(){local token=random(1,1) return 2}\ninit{set visits=0 flow.for_loop(i,first(),last()){set visits=visits+1} set done=true}\n";
    let(graphs,index,loop_node,path)=seed("pending-cold",raw,NodeKind::ForLoopWithBreak);
    let phase_key=format!("rvn_native_phase_g{}_n{}",graphs[index].graph_id.get(),loop_node.get());
    let last_node=graphs[index].nodes.values().find(|n|n.kind==NodeKind::FunctionCall&&n.properties.get("function")==Some(&PropertyValue::String("last".into()))).unwrap().id;
    // Visit zero stops after First but before the first Test. Later visits
    // stop after a body, while the candidate comparison uses the next index.
    // The live owner must still own the preceding committed body cursor.
    for stopped_visit in 0..=2{
        let(mut live,_)=load(&graphs,&path);let initial=live.state().random;
        let bp=live.site_page(0).unwrap().entries.into_iter().find(|entry|entry.origin.as_ref().and_then(|o|o.scheduled_step.as_ref()).and_then(|step|step.endpoint.as_ref()).is_some_and(|e|e.node==last_node.get()&&e.kind=="function_call")).expect("actual Last producer site").breakpoint;
        let route=bp.site.as_ref().unwrap().route.clone();live.configure(vec![bp],vec![]).unwrap();let mut reached=false;
        for _ in 0..128{if live.snapshot().site.as_ref().is_some_and(|site|site.route==route)&&live.state().vars.get("visits")==Some(&Value::Int(stopped_visit)){reached=true;break;}live.step(StepKind::Continue).unwrap();}assert!(reached,"pending Test visit {stopped_visit}");
        let before=serde_json::to_vec(live.state()).unwrap();let save=live.capture_save15(9,"real Last producer".into()).unwrap();let bytes=save.encode().unwrap();
        let image:serde_json::Value=serde_json::from_str(&bytes).unwrap();let loops=image["debugger"]["startup"]["loops"].as_array();
        assert_eq!(image["debugger"]["trace"]["pending"]["request"]["variables"][&phase_key],serde_json::to_value(Value::Int(1)).unwrap(),"the observed Last stop must belong to Test, never an excluded Last ordinal in First");
        if stopped_visit==0{assert!(loops.is_none_or(Vec::is_empty),"First completed in the trace does not publish a body cursor");}
        else{let loops=loops.expect("actual earlier body owns its frame");assert_eq!(loops.len(),1);let frame=&loops[0];assert_eq!(frame["visits"],serde_json::json!(stopped_visit));assert_eq!(frame["state"]["kind"],"DemandV1");assert_eq!(frame["state"]["cursor"]["index"],serde_json::json!(stopped_visit-1),"the pending candidate must not advance the live cursor");}
        let prepared=live.prepare_step(StepKind::Into).unwrap();assert_eq!(serde_json::to_vec(live.state()).unwrap(),before,"preparing must leave the actual owner unchanged");drop(prepared);
        assert_eq!(live.capture_save15(9,"real Last producer".into()).unwrap().encode().unwrap(),bytes,"dropping the actual plan must not publish trace, cursor or RNG");
        let(mut cold,_)=load(&graphs,&path);cold.restore_save15(rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();assert_eq!(cold.snapshot().site.as_ref().unwrap().route,route);assert_eq!(cold.state().random,live.state().random);
        finish(&mut live);finish(&mut cold);let mut expected=initial;for _ in 0..5{expected.integer(1,1).unwrap();}assert_eq!(live.state().random,expected,"First must not replay and Test advancement must happen once");assert_eq!(cold.state().random,expected);assert_eq!(cold.state().vars,live.state().vars);assert_eq!(cold.state().vars["visits"],Value::Int(3));
    }
}
#[test]
fn native0704_old_endpoint_observation_wire_omits_new_optional_field(){
    let raw="init{set total=1+2}\n";let mut graphs=import_script(&rvn_parser::parse(raw).unwrap()).unwrap();for graph in &mut graphs{graph.enable_native_frame_preparation().unwrap();}
    let compiled=transpile_project(&graphs).unwrap();assert!(!compiled.source.contains("consume_v3"));let path=root("old-wire").join("old.rvn");std::fs::write(&path,compiled.source).unwrap();let program=rvn_parser::parse_file_program_with_uses(path).unwrap();let steps=rvn_parser::scheduled_source::scheduled_step_sources(&program).unwrap();assert!(!steps.is_empty());
    for step in steps{let endpoint=step.endpoint.unwrap();assert!(endpoint.native_loop_output.is_none());let json=serde_json::to_string(&endpoint).unwrap();assert!(!json.contains("native_loop_output"));let decoded:rvn_parser::scheduled_source::DeclaredEndpoint=serde_json::from_str(&json).unwrap();assert_eq!(endpoint,decoded);}
}
fn fanout_consumer(g:&mut GraphDocument,owner:NodeId,name:&str){
    let assignment=g.nodes.values().find(|n|n.kind==NodeKind::SetVariable&&g.variable_node_name(n.id).as_deref()==Some(name)).unwrap().id;
    let math=g.add_catalog_node(NodeKind::MathAdd,[40.,80.]).unwrap();let element=g.pin_by_key(owner,"array_element").unwrap().id;
    for key in ["left","right"]{g.connect(element,g.pin_by_key(math,key).unwrap().id).unwrap();}
    let input=g.pin_by_key(assignment,"value").unwrap().id;assert!(!g.edges.values().any(|e|e.input==input));g.connect(g.pin_by_key(math,"value").unwrap().id,input).unwrap();
}
#[test]
fn native0704_measured_same_consumer_fanout_has_one_element_demand_per_visit(){
    let raw=format!("{SUPPLIER}\ninit{{set visits=0 set trace=[0] for item in supplier(){{set visits=visits+1 set seen=0 set trace=list_append(trace,seen)}} set done=true}}\n");
    let(mut graphs,index,node,path)=seed("one-fanout",&raw,NodeKind::ForEach);fanout_consumer(&mut graphs[index],node,"seen");let(mut owner,_)=load(&graphs,&path);let mut expected=owner.state().random;for _ in 0..7{expected.integer(1,1).unwrap();}finish(&mut owner);
    assert_eq!(owner.state().random,expected);assert_eq!(owner.state().vars["trace"],Value::List(vec![Value::Int(0),Value::Int(22),Value::Int(44),Value::Int(66)]));
}
#[test]
fn native0704_measured_successive_consumer_fanout_has_two_element_demands_per_visit(){
    let raw=format!("{SUPPLIER}\ninit{{set visits=0 set trace=[0] for item in supplier(){{set visits=visits+1 set seen_a=0 set seen_b=0 set trace=list_append(list_append(trace,seen_a),seen_b)}} set done=true}}\n");
    let(mut graphs,index,node,path)=seed("two-fanout",&raw,NodeKind::ForEach);for name in ["seen_a","seen_b"]{fanout_consumer(&mut graphs[index],node,name);}let(mut owner,_)=load(&graphs,&path);let mut expected=owner.state().random;for _ in 0..10{expected.integer(1,1).unwrap();}finish(&mut owner);
    assert_eq!(owner.state().random,expected);assert_eq!(owner.state().vars["trace"],Value::List([0,22,22,44,44,66,66].into_iter().map(Value::Int).collect()));
}
#[test]
fn native0704_measured_empty_collection_has_one_header_demand_and_no_body_effect(){
    let raw="function supplier(){local token=random(1,1) return []}\ninit{set visits=0 for item in supplier(){set visits=visits+1 set forbidden=99} set done=true}\n";
    let(graphs,_,_,path)=seed("empty",raw,NodeKind::ForEach);let(mut owner,_)=load(&graphs,&path);let mut expected=owner.state().random;expected.integer(1,1).unwrap();finish(&mut owner);
    assert_eq!(owner.state().random,expected);assert_eq!(owner.state().vars["visits"],Value::Int(0));assert!(!owner.state().vars.contains_key("forbidden"));assert_eq!(owner.state().vars["done"],Value::Bool(true));
}
#[test]
fn native0704_measured_first_input_is_captured_once_even_when_body_changes_it(){
    let raw="function first(value){local token=random(1,1) return value}\nfunction last(){local token=random(1,1) return 2}\ninit{set visits=0 set initial=0 set trace=[-1] flow.for_loop(i,first(initial),last()){set visits=visits+1 set seen=0 set trace=list_append(trace,seen) set initial=100} set done=true}\n";
    let(mut graphs,index,node,path)=seed("first-mutable",raw,NodeKind::ForLoopWithBreak);output_to_value(&mut graphs[index],node,"index","seen");let(mut owner,_)=load(&graphs,&path);let mut expected=owner.state().random;for _ in 0..5{expected.integer(1,1).unwrap();}finish(&mut owner);
    assert_eq!(owner.state().random,expected);assert_eq!(owner.state().vars["visits"],Value::Int(3));assert_eq!(owner.state().vars["initial"],Value::Int(100));assert_eq!(owner.state().vars["trace"],Value::List([-1,0,1,2].into_iter().map(Value::Int).collect()));
}

fn phase_source(source:&str)->(rvn_parser::Statement,rvn_parser::scheduled_source::native_loop_contract::Binding,usize,usize){
    use rvn_parser::{Statement as S,ConsumerStep as C,Expr as E};
    let ast=rvn_parser::parse(source).unwrap();let body=ast.iter().find_map(|s|if let S::Init{body}=s{Some(body)}else{None}).unwrap();
    let statement=body.iter().find(|s|matches!(s,S::ScheduledConsumer{version:3,..})).unwrap();let binding=rvn_parser::scheduled_source::native_loop_contract::binding(statement).unwrap().unwrap();
    let S::ScheduledConsumer{steps,..}=statement else{unreachable!()};
    let indices=|phase|{let root=rvn_parser::scheduled_source::native_loop_contract::phase_expression(statement,phase).unwrap();let selected=rvn_parser::loop_phase_dependencies::capture_step_indices(steps,root).unwrap();selected.into_iter().find(|i|matches!(&steps[*i],C::Capture{value:E::Call{name,..},..}if name=="supplier")).unwrap()};
    let first=indices(rvn_parser::scheduled_source::native_loop_contract::Phase::First);let last=indices(rvn_parser::scheduled_source::native_loop_contract::Phase::Test);assert_ne!(first,last);(statement.clone(),binding,first,last)
}
fn phase_fixture(name:&str)->(Vec<GraphDocument>,PathBuf,EngineStartup<TerminalRenderer>,String){
    // Same loaded function at two actual call nodes: a forged caller route
    // cannot be rejected merely because its function name happens to differ.
    let raw="function supplier(value){local token=random(1,1) return value}\ninit{set visits=0 flow.for_loop(i,supplier(0),supplier(2)){set visits=visits+1} set done=true}\n";
    let(graphs,_,_,path)=seed(name,raw,NodeKind::ForLoopWithBreak);let(owner,source)=load(&graphs,&path);(graphs,path,owner,source)
}
#[test]
fn native0727_real_pending_charge_cannot_borrow_an_excluded_actual_producer(){
    use rvn_core::editor_debugger::Route;
    let(graphs,path,mut live,source)=phase_fixture("phase-portable");let initial=live.state().random;let(_,binding,first,last)=phase_source(&source);
    let first_route=serde_json::to_value(Route::Producer(first)).unwrap();let last_route=serde_json::to_value(Route::Producer(last)).unwrap();let mut found=None;
    for _ in 0..256{
        if live.snapshot().evaluating{
            let save=live.capture_save15(12,"actual phase task".into()).unwrap();let image=serde_json::to_value(&save).unwrap();let pending=&image["debugger"]["trace"]["pending"];
            if pending["request"]["variables"][&binding.phase]==serde_json::to_value(Value::Int(1)).unwrap(){
                if let Some(task)=pending["tasks"].as_array().and_then(|tasks|tasks.iter().position(|task|task["kind"]=="charge"&&task["ctx"]["frame"]==0&&task["ctx"]["route"].as_array().is_some_and(|route|route.last()==Some(&last_route)))){found=Some((image,task));break;}
            }
        }
        assert!(!live.is_complete(),"a genuine pending Last function checkpoint is required");live.step(StepKind::Into).unwrap();
    }
    let(image,task)=found.expect("actual public Into inside Last supplier must retain its real caller Charge");
    let(mut good,_)=load(&graphs,&path);good.restore_save15(serde_json::from_value(image.clone()).unwrap()).unwrap();
    let mut forged=image;let route=forged["debugger"]["trace"]["pending"]["tasks"][task]["ctx"]["route"].as_array_mut().unwrap();assert_eq!(route.last(),Some(&last_route));*route.last_mut().unwrap()=first_route;
    let(mut refused,_)=load(&graphs,&path);let before=serde_json::to_vec(refused.state()).unwrap();assert!(refused.restore_save15(serde_json::from_value(forged).unwrap()).is_err());assert_eq!(serde_json::to_vec(refused.state()).unwrap(),before);
    finish(&mut good);finish(&mut live);let mut expected=initial;for _ in 0..5{expected.integer(1,1).unwrap();}assert_eq!(good.state().random,expected);assert_eq!(good.state().vars,live.state().vars);assert_eq!(good.state().vars["visits"],Value::Int(3));
}
#[test]
fn native0727_public_step_sites_include_only_producers_reached_by_the_actual_phase(){
    let(_,_,mut owner,source)=phase_fixture("phase-sites");let(statement,binding,_,_)=phase_source(&source);let mut observed=0;
    for _ in 0..256{
        let snapshot=owner.snapshot();
        if let Some(step)=snapshot.origin.as_ref().and_then(|origin|origin.scheduled_step.as_ref()){
            let rvn_parser::Statement::ScheduledConsumer{consumer,steps,..}=&statement else{unreachable!()};
            if step.consumer_identity==*consumer{
                let save=owner.capture_save15(13,"phase site".into()).unwrap();let image=serde_json::to_value(&save).unwrap();let phase=&image["debugger"]["trace"]["pending"]["request"]["variables"][&binding.phase];
                let phase=if *phase==serde_json::to_value(Value::Int(0)).unwrap(){rvn_parser::scheduled_source::native_loop_contract::Phase::First}else{assert_eq!(*phase,serde_json::to_value(Value::Int(1)).unwrap());rvn_parser::scheduled_source::native_loop_contract::Phase::Test};
                let root=rvn_parser::scheduled_source::native_loop_contract::phase_expression(&statement,phase).unwrap();let selected=rvn_parser::loop_phase_dependencies::capture_step_indices(steps,root).unwrap();assert!(selected.contains(&step.index),"a skipped producer must not become a public source stop");observed+=1;
            }
        }
        if owner.is_complete(){assert!(observed>=5,"First plus four Test visits must have genuine source observations");return;}owner.step(StepKind::Into).unwrap();
    }panic!("finite phase fixture did not complete");
}
