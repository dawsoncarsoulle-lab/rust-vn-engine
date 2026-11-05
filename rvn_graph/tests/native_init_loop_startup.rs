//! Prepared real Graph -> native .rvn file -> parsed SourceWorkspace ->
//! owned EngineStartup cases. No Game owner is constructed as a fallback.
use rvn_core::{Engine, TerminalRenderer};
use rvn_core::engine::EngineStartup;
use rvn_core::editor_debugger::StepKind;
use rvn_graph::*;
use rvn_parser::Value;
use std::{collections::HashMap, path::PathBuf, sync::atomic::{AtomicU64,Ordering}};

fn native_fixture(name:&str, authored:&str, kind:NodeKind)->(EngineStartup<TerminalRenderer>,rvn_parser::ParsedProgram){
    let parsed=rvn_parser::parse(authored).unwrap();
    let mut graph=import_script(&parsed).unwrap().into_iter()
        .find(|graph|matches!(graph.kind,GraphKind::Init)).expect("actual imported Init graph");
    assert!(graph.nodes.values().any(|node|node.kind==kind));
    graph=GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
    graph.enable_native_frame_preparation().unwrap();
    let compiled=prepare_native_frame_graph(&graph).unwrap();
    assert!(compiled.source.contains("flow.consume_v2("),"real NativeFrame lowering must be used");
    assert_eq!(rvn_parser::parse(&compiled.source).unwrap(),compiled.ast);
    static NEXT:AtomicU64=AtomicU64::new(0);
    let root=PathBuf::from(std::env::var("RVN_INIT_LOOP_FIXTURES").expect("private Init loop fixtures"))
        .join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("actual.graph.json"),graph.to_pretty_json().unwrap()).unwrap();
    let file=root.join("actual.native.rvn");
    std::fs::write(&file,&compiled.source).unwrap();
    let program=rvn_parser::parse_file_program_with_uses(&file).unwrap();
    let startup=Engine::prepare_debugger_startup(program.clone(),TerminalRenderer,16,HashMap::new(),Default::default(),None)
        .expect("actual Native Init loop must admit its owned continuation");
    assert!(!startup.is_complete(),"admission cannot run Init");
    assert!(!startup.state().vars.contains_key("done"),"admission cannot publish the completed branch");
    (startup,program)
}
fn native_startup(name:&str,authored:&str,kind:NodeKind)->EngineStartup<TerminalRenderer>{native_fixture(name,authored,kind).0}
fn finish(owner:&mut EngineStartup<TerminalRenderer>){
    for _ in 0..256{
        if owner.is_complete(){return;}
        owner.step(StepKind::Continue).unwrap();
    }
    panic!("finite three-iteration fixture failed to finish");
}

#[test]
fn init_native_loop_admission_while_false_has_a_real_source_and_startup_owner(){
    let mut owner=native_startup("while-false","init {while false {}}\n",NodeKind::While);
    finish(&mut owner);
}
#[test]
fn init_native_loop_admission_foreach_empty_has_a_real_source_and_startup_owner(){
    let mut owner=native_startup("foreach-empty","init {for item in [] {}}\n",NodeKind::ForEach);
    finish(&mut owner);
}
#[test]
fn init_native_loop_admission_range_empty_has_a_real_source_and_startup_owner(){
    let mut owner=native_startup("range-empty","init {flow.for_loop(index,1,0) {}}\n",NodeKind::ForLoopWithBreak);
    finish(&mut owner);
}

#[test]
fn init_native_loop_while_reevaluates_actual_header_after_each_owned_child(){
    let mut owner=native_startup("while",
        "init {set seen=0 while seen<3 {set seen=seen+1} set done=true}\n",NodeKind::While);
    finish(&mut owner);
    assert_eq!(owner.state().vars["seen"],Value::Int(3));
    assert_eq!(owner.state().vars["done"],Value::Bool(true));
}
#[test]
fn init_native_loop_each_keeps_one_collection_and_visits_real_body_in_order(){
    let mut owner=native_startup("foreach",
        "init {set sum=0 for item in [1,2,3] {set sum=sum+item} set done=true}\n",NodeKind::ForEach);
    finish(&mut owner);
    assert_eq!(owner.state().vars["sum"],Value::Int(6));
    assert_eq!(owner.state().vars["done"],Value::Bool(true));
}
#[test]
fn init_native_loop_break_exits_nearest_actual_range_and_reaches_completed(){
    let mut owner=native_startup("forbreak",
        "init {set seen=0 flow.for_loop(index,2,5) {set seen=index if index==2 {flow.break()} set unseen=99} set done=true}\n",NodeKind::ForLoopWithBreak);
    finish(&mut owner);
    assert_eq!(owner.state().vars["seen"],Value::Int(2));
    assert!(!owner.state().vars.contains_key("unseen"));
    assert_eq!(owner.state().vars["done"],Value::Bool(true));
}

fn native_cold_loop(name:&str,raw:&str,kind:NodeKind){
 use rvn_core::editor_debugger::Route;
 let(mut owner,program)=native_fixture(name,raw,kind);
 let route=vec![Route::Statement(1),Route::Body,Route::Statement(0),Route::Body,Route::Statement(0)];
 let bp=owner.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.route==route)).expect("actual Graph nested body source site").breakpoint;
 owner.configure(vec![bp],vec![]).unwrap();
 for _ in 0..64{if owner.snapshot().site.as_ref().is_some_and(|site|site.route==route){break;}owner.step(StepKind::Continue).unwrap();}
 assert_eq!(owner.snapshot().site.as_ref().unwrap().route,route);let random=owner.state().random;
 let encoded=owner.capture_save15(1,"Graph loop cold".into()).unwrap().encode().unwrap();
 // A true cold parse at another filesystem location with the same frozen bytes.
 let path=program.sources[0].path.as_ref().unwrap();let relocated=path.parent().unwrap().join("relocated.native.rvn");std::fs::copy(path,&relocated).unwrap();
 let cold_program=rvn_parser::parse_file_program_with_uses(relocated).unwrap();
 let mut cold=Engine::prepare_debugger_startup(cold_program,TerminalRenderer,16,HashMap::new(),Default::default(),None).unwrap();
 cold.restore_save15(rvn_core::save::consumer_checkpoint::Save15::decode(&encoded).unwrap()).unwrap();
 assert_eq!(cold.snapshot().site.as_ref().unwrap().route,route);assert_eq!(cold.state().random,random);
 finish(&mut owner);finish(&mut cold);assert_eq!(cold.state().vars,owner.state().vars);assert_eq!(cold.state().random,owner.state().random);
}
#[test]
fn init_native_loop_while_cold_source_relocation_resumes_actual_gather(){native_cold_loop("while-cold","init {set seen=0 while random(1,9)>0 and seen<3 {set seen=seen+1} set done=true}\n",NodeKind::While);}
#[test]
fn init_native_loop_each_cold_source_relocation_resumes_actual_gather(){native_cold_loop("each-cold","init {set seen=0 for item in [random(1,9),random(1,9)] {set seen=seen+item} set done=true}\n",NodeKind::ForEach);}
#[test]
fn init_native_loop_range_cold_source_relocation_resumes_actual_gather(){native_cold_loop("range-cold","init {set seen=0 flow.for_loop(i,random(1,2),random(3,4)) {set seen=seen+i} set done=true}\n",NodeKind::ForLoopWithBreak);}

#[derive(Clone)]
struct Published(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl rvn_core::Renderer for Published{
 fn validate_scene_batch(&self,_:&rvn_core::renderer::SceneRenderBatch)->Result<(),String>{Ok(())}
 fn publish_scene_batch(&mut self,_:rvn_core::renderer::SceneRenderBatch){self.0.fetch_add(1,Ordering::Relaxed);}
 fn set_background(&mut self,_:&str,_:&rvn_parser::Transition){self.0.fetch_add(1,Ordering::Relaxed);}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&rvn_parser::Position,_:&rvn_parser::Transition,_:Option<&rvn_core::SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&rvn_parser::Transition,_:&rvn_core::SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&rvn_parser::Position,_:&rvn_parser::Transition,_:&rvn_core::SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{0}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[rvn_parser::Hotspot])->usize{0}
 fn music_play(&mut self,_:&str,_:&rvn_parser::Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&rvn_parser::Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&rvn_parser::Transition){}
 fn sfx_stop(&mut self,_:&str,_:&rvn_parser::Transition){}
}
fn runs_owned_after_admission(name:&str,raw:&str,kind:NodeKind){
 let(_admitted,p)=native_fixture(name,raw,kind);let source=&p.sources[0];let file=source.path.as_ref().unwrap().parent().unwrap().join("actual-normal.native.rvn");
 std::fs::write(&file,format!("init {{scene \"before.png\"}}\n{}",source.source)).unwrap();let program=rvn_parser::parse_file_program_with_uses(file).unwrap();
 let count=std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));let result=Engine::from_program(program,Published(count.clone()),16);
 let engine=result.expect("Normal source constructor must drain finite loops through the same real Startup owner");assert!(count.load(Ordering::Relaxed)>0);assert_eq!(engine.state.background_image,"before.png");assert!(engine.has_frozen_startup_program());assert!(engine.state.vars.contains_key("seen"));
}
#[test]
fn normal_constructor_native_while_uses_owned_continuation(){runs_owned_after_admission("normal-while","init {set seen=0 while seen<3 {set seen=seen+1}}\n",NodeKind::While);}
#[test]
fn normal_constructor_native_each_uses_owned_continuation(){runs_owned_after_admission("normal-each","init {for item in [1] {set seen=item}}\n",NodeKind::ForEach);}
#[test]
fn normal_constructor_native_range_uses_owned_continuation(){runs_owned_after_admission("normal-range","init {flow.for_loop(i,1,2) {set seen=i}}\n",NodeKind::ForLoopWithBreak);}

#[test]
fn normal_constructor_guard_preserves_old_inline_init_effect(){
 let root=PathBuf::from(std::env::var("RVN_INIT_LOOP_FIXTURES").unwrap()).join(format!("old-inline-{}",std::process::id()));std::fs::create_dir_all(&root).unwrap();let file=root.join("actual-old.rvn");std::fs::write(&file,"init {scene \"before.png\"}\n").unwrap();
 let program=rvn_parser::parse_file_program_with_uses(file).unwrap();let count=std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));let engine=Engine::from_program(program,Published(count.clone()),16).expect("existing inline constructor must remain admitted");assert_eq!(count.load(Ordering::Relaxed),1);assert_eq!(engine.state.background_image,"before.png");
}
