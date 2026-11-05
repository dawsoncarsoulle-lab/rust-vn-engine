//! Real Graph/source/Engine narrative contracts. Prepared only, NOT RUN.
//! The current Init-only gate is not replaced until the full backend is wired.
use rvn_graph::*;
use rvn_core::{Engine,Interaction,TerminalRenderer};
use rvn_core::random::RandomState;
use rvn_parser::{ParsedProgram,Value};
use std::{path::PathBuf,sync::atomic::{AtomicU64,Ordering}};

// import_script supplies a neutral Init view even when no Init is authored.
// Such a root has no executable consumer and no Native source identity.
// Keep that view untouched; opt-in every actual source scope as before.
fn enable_native_source_scopes(graphs:&mut [GraphDocument],authored:&rvn_parser::Script){
    let source_has_init=authored.iter().any(|statement|matches!(statement,rvn_parser::Statement::Init{..}));
    for graph in graphs{
        let source_less_init=!source_has_init && graph.kind==GraphKind::Init
            && graph.nodes.len()==1 && graph.edges.is_empty() && graph.characters.is_empty()
            && graph.nodes.values().all(|node|node.kind==NodeKind::Init);
        if source_less_init{
            assert!(graph.inspector_evaluation.is_legacy(),"never demote an actual Native document");
        }else{graph.enable_native_frame_preparation().unwrap();}
    }
}

fn root(name:&str)->PathBuf{
    static NEXT:AtomicU64=AtomicU64::new(0);
    let base=PathBuf::from(std::env::var("RVN_NATIVE_NARRATIVE0732_FIXTURES").expect("isolated D fixture root required"));
    let root=base.join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir_all(&root).unwrap();root
}
fn connect_output(g:&mut GraphDocument,owner:NodeId,role:&str,name:&str){
    let assignment=g.nodes.values().find(|n|n.kind==NodeKind::SetVariable&&g.variable_node_name(n.id).as_deref()==Some(name)).unwrap().id;
    let input=g.pin_by_key(assignment,"value").unwrap().id;let edges:Vec<_>=g.edges.values().filter(|e|e.input==input).map(|e|(e.id,g.pins[&e.output].node)).collect();
    for(id,_)in &edges{g.edges.remove(id);}g.connect(g.pin_by_key(owner,role).unwrap().id,input).unwrap();
    for(_,node)in edges{let n=&g.nodes[&node];if matches!(n.kind,NodeKind::Literal|NodeKind::VariableGet)&&!n.pins.iter().any(|p|g.edges.values().any(|e|e.input==*p||e.output==*p)){g.remove_node(node).unwrap();}}
}
fn compiled(name:&str,authored:&str,edits:&[(&str,NodeKind,&str,&[&str])])->(ParsedProgram,String){
    let authored_script=rvn_parser::parse(authored).unwrap();
    let mut graphs=import_script(&authored_script).unwrap();enable_native_source_scopes(&mut graphs,&authored_script);
    for(scene,kind,role,names)in edits{
        let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name==scene)).unwrap();let owner=graph.nodes.values().find(|n|n.kind==*kind).unwrap().id;
        *graph=graph.prepare_native_loop_demand(owner).unwrap().graph;for name in *names{connect_output(graph,owner,role,name);}
    }
    let generated=transpile_project(&graphs).expect("real Graph/source lowering must succeed, no source bypass");assert!(generated.source.contains("flow.consume_v3("));assert_eq!(rvn_parser::parse(&generated.source).unwrap(),generated.ast);
    let again=transpile_project(&import_native_frame_script(&generated.ast).unwrap()).unwrap();assert_eq!(again.ast,generated.ast,"actual source import must preserve the contract");
    let path=root(name).join("narrative.rvn");std::fs::write(&path,&generated.source).unwrap();(rvn_parser::parse_file_program_with_uses(path).unwrap(),generated.source)
}
fn engine(program:&ParsedProgram)->Engine<TerminalRenderer>{let mut owner=Engine::from_program_with_debugger(program.clone(),TerminalRenderer,16).unwrap();owner.state.random=RandomState::seeded(732);owner.enter_label("start").unwrap();owner}
fn text(owner:&mut Engine<TerminalRenderer>)->String{match owner.step_until_interaction().unwrap(){Some(Interaction::Dialogue{text,..})=>text,other=>panic!("a real dialogue must be reached: {other:?}")}}
fn dialogues(owner:&mut Engine<TerminalRenderer>,count:usize)->Vec<String>{let mut out=Vec::new();for i in 0..count{if i!=0{owner.advance_dialogue().unwrap();}out.push(text(owner));}out}
fn expected(draws:usize)->RandomState{let mut state=RandomState::seeded(732);for _ in 0..draws{state.integer(1,1).unwrap();}state}
fn no_private_globals(owner:&Engine<TerminalRenderer>){assert!(!owner.state.vars.keys().any(|name|name.starts_with("rvn_native_")));}
const SUPPLIER:&str="function supplier(){local token=random(1,1) return [11,22,33]}";
#[test]
fn narrative0732_real_index_output_survives_dialogues_without_element_demand(){
    let raw=format!("{SUPPLIER}\nscene label start\nset visits=0\nfor item in supplier(){{set visits=visits+1 set seen=0 \"[seen]\"}}\n\"Done\"\n");
    let(program,_)=compiled("index",&raw,&[("start",NodeKind::ForEach,"array_index",&["seen"])]);let mut owner=engine(&program);
    assert_eq!(dialogues(&mut owner,4),["0","1","2","Done"]);assert_eq!(owner.state.vars["visits"],Value::Int(3));assert_eq!(owner.state.random,expected(4));no_private_globals(&owner);
}
#[test]
fn narrative0732_real_element_output_reopens_one_gather_per_reached_body(){
    let raw=format!("{SUPPLIER}\nscene label start\nfor item in supplier(){{set seen=0 \"[seen]\"}}\n\"Done\"\n");
    let(program,_)=compiled("element",&raw,&[("start",NodeKind::ForEach,"array_element",&["seen"])]);let mut owner=engine(&program);
    assert_eq!(dialogues(&mut owner,4),["11","22","33","Done"]);assert_eq!(owner.state.random,expected(7));no_private_globals(&owner);
}
#[test]
fn narrative0732_real_current_range_bound_changes_the_next_comparison(){
    for(bound,shown)in [(0,vec!["0","Done"]),(4,vec!["0","1","2","3","4","Done"]) ]{
        let raw=format!("scene label start\nset bound=2\nflow.for_loop(i,0,bound){{set seen=0 set bound={bound} \"[seen]\"}}\n\"Done\"\n");
        let(program,_)=compiled("range-bound",&raw,&[("start",NodeKind::ForLoopWithBreak,"index",&["seen"])]);let mut owner=engine(&program);assert_eq!(dialogues(&mut owner,shown.len()),shown);no_private_globals(&owner);
    }
}
#[test]
fn narrative0732_helper_call_parks_parent_loop_and_restores_its_actual_index(){
    let raw="scene label start\nfor item in [11,22]{set before=0 call helper set after=0 \"[before]:[after]\"}\n\"Done\"\njump end\nlabel helper\nfor other in [7,8]{set helper_seen=0}\nreturn\nscene label end\n\"End\"\n";
    let(program,_)=compiled("helper",raw,&[("start",NodeKind::ForEach,"array_index",&["before","after"]),("helper",NodeKind::ForEach,"array_index",&["helper_seen"])]);let mut owner=engine(&program);
    assert_eq!(dialogues(&mut owner,3),["0:0","1:1","Done"]);assert_eq!(owner.state.vars["helper_seen"],Value::Int(1));no_private_globals(&owner);
}
#[test]
fn narrative0732_scene_call_preserves_the_parent_loop_activation_and_continuation(){
    let raw="scene label start\nfor item in [11,22]{set before=0 call child set after=0 \"[before]:[after]\"}\n\"Done\"\njump end\nscene label child\nfor other in [7,8]{set child_seen=0}\nreturn\nscene label end\n\"End\"\n";
    let(program,_)=compiled("scene-call",raw,&[("start",NodeKind::ForEach,"array_index",&["before","after"]),("child",NodeKind::ForEach,"array_index",&["child_seen"])]);let mut owner=engine(&program);
    assert_eq!(dialogues(&mut owner,3),["0:0","1:1","Done"]);assert_eq!(owner.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().scene,"start");no_private_globals(&owner);
}
#[test]
fn narrative0732_real_body_save_cold_and_source_relocation_do_not_replay_suppliers(){
    let raw=format!("{SUPPLIER}\nscene label start\nfor item in supplier(){{set seen=0 \"[seen]\"}}\n\"Done\"\n");let(program,source)=compiled("cold",&raw,&[("start",NodeKind::ForEach,"array_element",&["seen"])]);let mut live=engine(&program);assert_eq!(text(&mut live),"11");let random=live.state.random;assert_eq!(random,expected(2));
    let bytes=live.capture_save15(&program,None,7,"real narrative loop".into()).unwrap().encode().unwrap();let file=root("relocated").join("moved.rvn");std::fs::write(&file,source).unwrap();let relocated=rvn_parser::parse_file_program_with_uses(file).unwrap();let mut cold=engine(&relocated);
    cold.restore_save15(&relocated,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();assert_eq!(cold.state.random,random);assert_eq!(cold.displayed_interaction().unwrap(),live.displayed_interaction().unwrap());
    assert_eq!(dialogues(&mut live,4),["11","22","33","Done"]);assert_eq!(dialogues(&mut cold,4),["11","22","33","Done"]);assert_eq!(cold.state.random,expected(7));assert_eq!(cold.state.random,live.state.random);assert_eq!(cold.state.vars,live.state.vars);no_private_globals(&cold);
}
#[test]
fn narrative0732_rollback_restores_body_cursor_rng_and_revokes_the_consumed_input(){
    let raw=format!("{SUPPLIER}\nscene label start\nfor item in supplier(){{set seen=0 \"[seen]\"}}\n\"Done\"\n");let(program,_)=compiled("rollback",&raw,&[("start",NodeKind::ForEach,"array_element",&["seen"])]);let mut owner=engine(&program);assert_eq!(text(&mut owner),"11");let random=owner.state.random;let vars=owner.state.vars.clone();let stale=owner.preview_input_source().unwrap();owner.advance_dialogue().unwrap();assert!(owner.rollback());assert_eq!(owner.state.random,random);assert_eq!(owner.state.vars,vars);assert_eq!(owner.displayed_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"11".into()}));
    assert!(owner.debug_begin(rvn_core::engine::DebugOperation::Advance(stale),vec![]).is_err());assert_eq!(dialogues(&mut owner,4),["11","22","33","Done"]);assert_eq!(owner.state.random,expected(7));no_private_globals(&owner);
}
#[test]
fn narrative0732_historical_graph_and_scene_snapshot_omit_new_optional_loop_fields(){
    let raw="scene label start\nset total=0\nset items=[1,2,3]\nfor item in items{set total=total+item set items=[]}\n\"[total]\"\n";let graphs=import_script(&rvn_parser::parse(raw).unwrap()).unwrap();let compiled=transpile_project(&graphs).unwrap();assert!(!compiled.source.contains("consume_v3"));let path=root("legacy").join("old.rvn");std::fs::write(&path,compiled.source).unwrap();let program=rvn_parser::parse_file_program_with_uses(path).unwrap();let mut owner=engine(&program);assert_eq!(text(&mut owner),"6");
    let json=serde_json::to_string(&owner.state).unwrap();assert!(!json.contains("native_loop_frames_v1"));assert!(!json.contains("parked_native_loops_v1"));no_private_globals(&owner);
}

// The three synchronous Sequence/Break shapes are observed in the local UE
// cold VM receipt F57E61...; latent/World paths are not inferred from them.
fn compiled_ranges(name:&str,authored:&str)->ParsedProgram{
    let authored_script=rvn_parser::parse(authored).unwrap();
    let mut graphs=import_script(&authored_script).unwrap();enable_native_source_scopes(&mut graphs,&authored_script);
    for graph in &mut graphs{
        let ranges:Vec<_>=graph.nodes.values().filter(|n|n.kind==NodeKind::ForLoopWithBreak).map(|n|n.id).collect();
        for owner in ranges{*graph=graph.prepare_native_loop_demand(owner).unwrap().graph;}
    }
    let generated=transpile_project(&graphs).unwrap();assert_eq!(rvn_parser::parse(&generated.source).unwrap(),generated.ast);
    let restored=import_native_frame_script(&generated.ast).unwrap();assert_eq!(transpile_project(&restored).unwrap().ast,generated.ast);
    let path=root(name).join("break.rvn");std::fs::write(&path,generated.source).unwrap();rvn_parser::parse_file_program_with_uses(path).unwrap()
}
#[test]
fn narrative0732_graph_break_keeps_engaged_sequence_tail_and_completed_without_next_last(){
    let raw="function last(){local draw=random(1,1) return 2}\nscene label start\nset trace=\"\"\nflow.for_loop(i,0,last()){flow.sequence(){{flow.break()} {set trace=trace+\"T\"}}}\nset trace=trace+\"C\"\n\"[trace]\"\n";
    let program=compiled_ranges("break-only",raw);let mut owner=engine(&program);
    assert_eq!(text(&mut owner),"TC");assert_eq!(owner.state.random,expected(1));no_private_globals(&owner);
}
#[test]
fn narrative0732_graph_body_break_keeps_real_body_tail_completed_order(){
    let raw="function last(){local draw=random(1,1) return 2}\nscene label start\nset trace=\"\"\nflow.for_loop(i,0,last()){flow.sequence(){{set trace=trace+\"B\" flow.break()} {set trace=trace+\"T\"}}}\nset trace=trace+\"C\"\n\"[trace]\"\n";
    let program=compiled_ranges("body-break",raw);let mut owner=engine(&program);
    assert_eq!(text(&mut owner),"BTC");assert_eq!(owner.state.random,expected(1));no_private_globals(&owner);
}
#[test]
fn narrative0732_graph_nested_inner_break_preserves_outer_continuation_and_comparisons(){
    let raw="function outerlast(){local draw=random(1,1) return 1}\nfunction innerlast(){local draw=random(1,1) return 2}\nscene label start\nset trace=\"\"\nflow.for_loop(o,0,outerlast()){flow.sequence(){{set trace=trace+\"O\" flow.for_loop(i,0,innerlast()){flow.sequence(){{set trace=trace+\"I\" flow.break()} {set trace=trace+\"T\"}}} set trace=trace+\"C\" set trace=trace+\"N\"} {set trace=trace+\"Z\"}}}\nset trace=trace+\"X\"\n\"[trace]\"\n";
    let program=compiled_ranges("nested-break",raw);let mut owner=engine(&program);
    assert_eq!(text(&mut owner),"OITCNZOITCNZX");assert_eq!(owner.state.random,expected(5));no_private_globals(&owner);
}

#[test]
fn narrative0732_public_pending_first_and_test_cold_restore_do_not_replay_the_real_supplier(){
    use rvn_core::{engine::{DebugOperation,DebugProgress},editor_debugger::StepKind};
    let raw="function supplier(value){local draw=random(1,1) return value}\nscene label start\nflow.for_loop(i,supplier(0),supplier(2)){set seen=0 \"[seen]\"}\n\"Done\"\n";
    let(program,_)=compiled("public-phase-cold",raw,&[("start",NodeKind::ForLoopWithBreak,"index",&["seen"])]);
    let statement=program.script.iter().find(|s|matches!(s,rvn_parser::Statement::ScheduledConsumer{version:3,..})).unwrap();
    let binding=rvn_parser::scheduled_source::native_loop_contract::binding(statement).unwrap().unwrap();
    for phase in [0,1]{
        let mut live=engine(&program);
        for _ in 0..32{
            if matches!(live.peek_statement(),Some(rvn_parser::Statement::ScheduledConsumer{version:3,..})){break;}
            let mut prior=live.debug_begin(DebugOperation::Statement,vec![]).unwrap();let mut complete=false;
            for _ in 0..128{if live.debug_step(&mut prior,StepKind::Continue).unwrap().0==DebugProgress::Complete{complete=true;break;}}
            assert!(complete,"the actual preceding operation must publish through Engine");
        }
        assert!(matches!(live.peek_statement(),Some(rvn_parser::Statement::ScheduledConsumer{version:3,..})));
        let before=serde_json::to_vec(&live.state).unwrap();let mut session=live.debug_begin(DebugOperation::Statement,vec![]).unwrap();let mut retained=None;
        for _ in 0..512{
            let snapshot=live.debug_session_snapshot(&session);
            if snapshot.evaluating&&snapshot.site.is_some(){
                let save=live.capture_save15(&program,Some(&session),17,"actual public Native phase".into()).unwrap();let json=serde_json::to_value(&save).unwrap();
                if json["debugger"]["trace"]["pending"]["request"]["variables"][&binding.phase]==serde_json::to_value(Value::Int(phase)).unwrap(){retained=Some(save.encode().unwrap());break;}
            }
            assert_ne!(live.debug_step(&mut session,StepKind::Into).unwrap().0,DebugProgress::Complete,"the requested real phase must be reached before the header publishes");
        }
        let bytes=retained.expect("actual public Into must stop inside the selected First/Test evaluation");
        assert_eq!(serde_json::to_vec(&live.state).unwrap(),before,"debug discovery is not owner publication");
        let mut cold=engine(&program);let mut restored=cold.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap().expect("the real suspended Statement must restore its session");
        // Restore may renew callback/host incarnations through the real API.
        // Compare the semantic owner continuation, not process-local identities.
        assert_eq!(cold.state.pc,live.state.pc);assert_eq!(cold.state.vars,live.state.vars);assert_eq!(cold.state.random,RandomState::seeded(732));
        assert_eq!(cold.state.scene_execution.as_ref().unwrap().live_position,live.state.scene_execution.as_ref().unwrap().live_position);
        assert_eq!(cold.state.scene_execution.as_ref().unwrap().owner(),live.state.scene_execution.as_ref().unwrap().owner());
        assert!(cold.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().native_loop_frames_v1.is_empty());
        for (owner,session)in [(&mut live,&mut session),(&mut cold,&mut restored)]{
            let mut complete=false;for _ in 0..512{if owner.debug_step(session,StepKind::Continue).unwrap().0==DebugProgress::Complete{complete=true;break;}}assert!(complete);
            assert_eq!(owner.state.random,expected(2),"First and Test each ran once across the actual publication");
            assert_eq!(dialogues(owner,4),["0","1","2","Done"]);assert_eq!(owner.state.random,expected(5));no_private_globals(owner);
        }
        assert_eq!(cold.state.random,live.state.random);assert_eq!(cold.state.vars,live.state.vars);
    }
}

#[test]
fn narrative0732_real_body_save_rejects_stored_private_alias_even_when_its_value_matches(){
    let raw="scene label start\nfor item in [11,22]{set seen=0 \"[seen]\"}\n\"Done\"\n";
    let(program,_)=compiled("private-alias-forgery",raw,&[("start",NodeKind::ForEach,"array_index",&["seen"])]);let mut live=engine(&program);assert_eq!(text(&mut live),"0");
    let statement=program.script.iter().find(|s|matches!(s,rvn_parser::Statement::ScheduledConsumer{version:3,..})).unwrap();let binding=rvn_parser::scheduled_source::native_loop_contract::binding(statement).unwrap().unwrap();
    let save=live.capture_save15(&program,None,19,"actual entered body".into()).unwrap();let image=serde_json::to_value(save).unwrap();assert_eq!(image["story"]["base"]["scene_execution"]["active"]["locals"],serde_json::json!({}));
    for alias in [&binding.index,&binding.cursor,&binding.phase]{
        let mut forged=image.clone();forged["story"]["base"]["scene_execution"]["active"]["locals"][alias]=serde_json::to_value(Value::Int(0)).unwrap();
        let mut refused=engine(&program);let before=serde_json::to_vec(&refused.state).unwrap();let error=refused.restore_save15(&program,serde_json::from_value(forged).unwrap()).err().expect("a stored private source alias grants no real phase/body ownership");
        assert!(error.to_string().contains("projection"),"{error}");assert_eq!(serde_json::to_vec(&refused.state).unwrap(),before);
    }
    let mut good=engine(&program);good.restore_save15(&program,serde_json::from_value(image).unwrap()).unwrap();assert_eq!(dialogues(&mut good,3),["0","1","Done"]);no_private_globals(&good);
}

#[test]
fn narrative0774_guarded_library_uses_actual_authored_functions_for_prepared_body_and_fresh(){
    let raw=format!("{SUPPLIER}\nscene label start\nfor item in supplier(){{set seen=0 \"[seen]\"}}\n\"Done\"\n");
    let(program,_)=compiled("guarded-library-union",&raw,&[("start",NodeKind::ForEach,"array_element",&["seen"])]);
    let prepared=rvn_core::program::PreparedNarrative::from_parsed(program.clone()).unwrap();
    let error=rvn_core::eval::FunctionLibrary::from_script(&prepared.script).err().expect("public C guard must still reject the compiled detached index read");
    // The compiled ArrayElement step is detached from its authored v3 loop.
    // Its precise owner check precedes the generic private-index read check.
    assert!(matches!(&error,rvn_core::EvalError::InvalidFunction(message)
        if message=="Native ArrayElement has no actual declared source loop owner"),"{error}");
    let authored=rvn_core::eval::FunctionLibrary::from_script(&program.script).unwrap();assert!(authored.has_function("supplier",0));
    assert!(Engine::from_narrative(prepared,TerminalRenderer,16).is_err(),"a prepared AST without frozen source is still no Engine authority");
    let mut owner=engine(&program);assert_eq!(dialogues(&mut owner,4),["11","22","33","Done"]);assert_eq!(owner.state.random,expected(7));no_private_globals(&owner);
    let mut fresh=owner.fresh(TerminalRenderer,16).unwrap();fresh.state.random=RandomState::seeded(732);fresh.enter_label("start").unwrap();assert_eq!(dialogues(&mut fresh,4),["11","22","33","Done"]);assert_eq!(fresh.state.random,expected(7));no_private_globals(&fresh);
}

// Generic fixture regressions. These do not grant a Native identity to Init{}.
#[test]
fn narrative0824_source_less_init_stays_neutral_while_real_scene_is_native_exact(){
    let authored=rvn_parser::parse("scene label start\nset seen=7\n\"Ready\"\n").unwrap();
    assert!(!authored.iter().any(|statement|matches!(statement,rvn_parser::Statement::Init{..})));
    let mut docs=import_script(&authored).unwrap();
    let before=docs.iter().find(|graph|graph.kind==GraphKind::Init).unwrap().clone();
    assert_eq!(before.nodes.len(),1);assert!(before.edges.is_empty());assert!(before.inspector_evaluation.is_legacy());
    let mut unsupported=before.clone();unsupported.enable_native_frame_preparation().unwrap();let opted=unsupported.clone();
    let declared=prepare_native_frame_graph(&unsupported).unwrap();
    assert!(matches!(declared.ast.as_slice(),[rvn_parser::Statement::Init{body}]if body.is_empty()));
    assert!(import_native_frame_script(&declared.ast).is_err(),"AST-only import cannot invent declarative source identity");
    assert_eq!(unsupported,opted);assert!(!unsupported.inspector_evaluation.is_legacy());
    enable_native_source_scopes(&mut docs,&authored);
    assert_eq!(docs.iter().find(|graph|graph.kind==GraphKind::Init).unwrap(),&before);
    assert!(docs.iter().filter(|graph|graph.kind!=GraphKind::Init).all(|graph|!graph.inspector_evaluation.is_legacy()));
    let generated=transpile_project(&docs).unwrap();assert_eq!(rvn_parser::parse(&generated.source).unwrap(),generated.ast);
    let restored=import_native_frame_script(&generated.ast).unwrap();assert_eq!(transpile_project(&restored).unwrap().ast,generated.ast);
    for graph in docs.iter().filter(|graph|!graph.inspector_evaluation.is_legacy()){
        let actual=restored.iter().find(|actual|actual.graph_id==graph.graph_id).unwrap();
        assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
        assert_eq!(actual.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());assert_eq!(actual.edges,graph.edges);
    }
    assert!(restored.iter().find(|graph|graph.kind==GraphKind::Init).unwrap().inspector_evaluation.is_legacy());
}
#[test]
fn narrative0824_authored_nonempty_init_keeps_real_native_identity_and_all_wires(){
    let authored=rvn_parser::parse("init{set sentinel=7}\nscene label start\nset seen=sentinel\n\"Ready\"\n").unwrap();
    let mut docs=import_script(&authored).unwrap();enable_native_source_scopes(&mut docs,&authored);
    assert!(docs.iter().all(|graph|!graph.inspector_evaluation.is_legacy()));
    let generated=transpile_project(&docs).unwrap();assert_eq!(rvn_parser::parse(&generated.source).unwrap(),generated.ast);
    let restored=import_native_frame_script(&generated.ast).unwrap();assert_eq!(transpile_project(&restored).unwrap().ast,generated.ast);
    for graph in &docs{
        let actual=restored.iter().find(|actual|actual.graph_id==graph.graph_id).unwrap();
        assert!(!actual.inspector_evaluation.is_legacy());assert_eq!(actual.kind,graph.kind);
        assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
        assert_eq!(actual.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());assert_eq!(actual.edges,graph.edges);
    }
}

// Generic numeric Dialogue coverage outside loops; uses the same real Graph path.
fn numeric0836_case(name:&str,authored:&str)->(ParsedProgram,Vec<GraphDocument>,rvn_parser::Script){
    let ast=rvn_parser::parse(authored).unwrap();let mut docs=import_script(&ast).unwrap();
    enable_native_source_scopes(&mut docs,&ast);
    let compiled=transpile_project(&docs).unwrap();assert_eq!(rvn_parser::parse(&compiled.source).unwrap(),compiled.ast);
    let restored=import_native_frame_script(&compiled.ast).unwrap();assert_eq!(transpile_project(&restored).unwrap().ast,compiled.ast);
    for graph in docs.iter().filter(|g|!g.inspector_evaluation.is_legacy()){
        let again=restored.iter().find(|g|g.graph_id==graph.graph_id).unwrap();assert_eq!(again.kind,graph.kind);
        assert_eq!(again.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
        for (id,node) in &graph.nodes{assert_eq!(again.nodes[id].kind,node.kind);assert_eq!(again.nodes[id].pins,node.pins);}
        assert_eq!(again.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());assert_eq!(again.edges,graph.edges);
    }
    let path=root(name).join("numeric.rvn");std::fs::write(&path,&compiled.source).unwrap();
    (rvn_parser::parse_file_program_with_uses(path).unwrap(),docs,compiled.ast)
}
fn numeric0836_dialogue(script:&rvn_parser::Script)->usize{
    script.iter().position(|s|matches!(s,rvn_parser::Statement::ScheduledConsumer{body,..}if matches!(body.as_slice(),[rvn_parser::Statement::Dialogue{..}]))).unwrap()
}
fn numeric0836_binding(script:&rvn_parser::Script)->rvn_parser::scheduled_source::DialogueTextBinding{
    let consumer=&script[numeric0836_dialogue(script)];
    let rvn_parser::Statement::ScheduledConsumer{body,..}=consumer else{panic!()};
    let [rvn_parser::Statement::Dialogue{text,..}]=body.as_slice()else{panic!()};
    assert!(matches!(text.0.as_slice(),[rvn_parser::TextSegment::Interp(rvn_parser::Expr::Var(name))]if name=="score"),"preserve the authored locale key, not a captured-ref template");
    let binding=rvn_parser::scheduled_source::dialogue_text_binding(consumer).unwrap().unwrap();assert_eq!(binding.variable,"score");binding
}
fn numeric0836_locale()->rvn_core::locale::LocaleManager{
    use std::collections::HashMap;use rvn_core::locale::{LocaleManager,LocaleTable};
    let catalogs=HashMap::from([("en".into(),LocaleTable{lang:"en".into(),strings:HashMap::new()}.into()),("fr".into(),LocaleTable{lang:"fr".into(),strings:HashMap::from([("[score]".into(),"Score [score]".into())])}.into())]);
    LocaleManager::from_frozen_catalogs("en","fr",vec!["en".into(),"fr".into()],catalogs).unwrap()
}
const NUMERIC0836:&str="init{set score=7}\nscene label start\nset rolled=random(1,1)\n\"[score]\"\n";
#[test]
fn narrative0836_integer_dialogue_preserves_original_locale_slot_conversions_cold_and_rng(){
    let(p,docs,script)=numeric0836_case("numeric-int",NUMERIC0836);let binding=numeric0836_binding(&script);
    let graph=docs.iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let number=graph.nodes.values().find(|n|n.kind==NodeKind::ConvertNumberToText).expect("real numeric conversion");
    let string=graph.nodes.values().find(|n|n.kind==NodeKind::ConvertStringToText).expect("real Text boundary");
    assert_eq!(binding.output_pin,graph.pin_by_key(string.id,"result").unwrap().id.get());
    let rvn_parser::Statement::ScheduledConsumer{steps,..}=&script[numeric0836_dialogue(&script)]else{panic!()};
    assert!(steps.iter().any(|s|matches!(s,rvn_parser::ConsumerStep::Capture{producer,..}if producer.starts_with(&format!("n{}:convert_number_to_text|",number.id.get())))));
    let mut live=engine(&p);live.locale=Some(numeric0836_locale());let mut expected=RandomState::seeded(732);expected.integer(1,1).unwrap();
    assert_eq!(text(&mut live),"Score 7");assert_eq!(live.state.random,expected);
    let save=live.capture_save15(&p,None,83,"numeric original locale slot".into()).unwrap();let bytes=save.encode().unwrap();
    let mut cold=engine(&p);cold.locale=Some(numeric0836_locale());cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();
    assert_eq!(cold.displayed_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"Score 7".into()}));assert_eq!(cold.state.random,expected);
    let before=serde_json::to_value(&cold.state).unwrap();cold.locale.as_mut().unwrap().set_language("en").unwrap();
    assert_eq!(cold.displayed_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"7".into()}));assert_eq!(serde_json::to_value(&cold.state).unwrap(),before);assert_eq!(cold.state.random,expected);
}
#[test]
fn narrative0836_float_dialogue_keeps_real_numeric_nodes_pins_edges_and_authored_slot(){
    let(p,docs,script)=numeric0836_case("numeric-float","init{set score=1.5}\nscene label start\n\"[score]\"\n");numeric0836_binding(&script);
    assert!(docs.iter().any(|g|g.nodes.values().any(|n|n.kind==NodeKind::ConvertNumberToText)));
    let mut live=engine(&p);let rng=live.state.random;assert_eq!(text(&mut live),"1.5");assert_eq!(live.state.random,rng);
}
#[test]
fn narrative0836_forged_numeric_codec_cannot_replace_the_real_capture_by_equal_value(){
    use rvn_parser::{Statement,ConsumerStep,Expr,BinOpKind};
    let(_,_,script)=numeric0836_case("numeric-codec-negative",NUMERIC0836);let ordinal=numeric0836_dialogue(&script);numeric0836_binding(&script);
    for case in 0..4{
        let mut forged=script.clone();let Statement::ScheduledConsumer{steps,..}=&mut forged[ordinal]else{panic!()};
        let value=steps.iter_mut().find_map(|s|match s{ConsumerStep::Capture{producer,value}if producer.contains(":convert_number_to_text|")=>Some(value),_=>None}).unwrap();
        match case{
            0=>*value=Expr::Str("7".into()),
            _=>{let Expr::BinOp{op,left,right}=value else{panic!("actual emitted numeric expression")};match case{1=>*left=Box::new(Expr::Str("prefix".into())),2=>*op=BinOpKind::Sub,_=>*right=Box::new(Expr::Var("score".into()))}}
        }
        assert!(rvn_parser::scheduled_source::dialogue_text_binding(&forged[ordinal]).is_err(),"case {case} cannot prove the actual retained output");
        assert!(import_native_frame_script(&forged).is_err(),"case {case} cannot reconstruct another codec under the old identity");
    }
}
#[test]
fn narrative0836_capture_order_and_source_output_mismatch_refuse_without_identity_repair(){
    use rvn_parser::Statement;
    let(_,docs,script)=numeric0836_case("numeric-source-negative",NUMERIC0836);let ordinal=numeric0836_dialogue(&script);let binding=numeric0836_binding(&script);
    let mut order=script.clone();let Statement::ScheduledConsumer{steps,..}=&mut order[ordinal]else{panic!()};steps.reverse();
    assert!(rvn_parser::scheduled_source::dialogue_text_binding(&order[ordinal]).is_err());assert!(import_native_frame_script(&order).is_err());
    let graph=docs.iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let get=graph.nodes.values().find(|n|n.kind==NodeKind::VariableGet&&graph.variable_node_name(n.id).as_deref()==Some("score")).unwrap();
    let other=graph.pin_by_key(get.id,"value").unwrap().id.get();assert_ne!(other,binding.output_pin);
    let mut wire=script.clone();let Statement::ScheduledConsumer{consumer,..}=&mut wire[ordinal]else{panic!()};let old=format!("~{}",binding.output_pin);assert_eq!(consumer.matches(&old).count(),1);*consumer=consumer.replace(&old,&format!("~{other}"));
    assert!(import_native_frame_script(&wire).is_err(),"equal formatted value cannot replace the actual conversion output wire");
}
