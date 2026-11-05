use super::*;
#[path="engine_unit_live_debug_contract_tests1021.rs"]
mod unit_live_tests;
#[path="engine_debugger_producer_tests.rs"]
mod producer_tests;

#[test]
fn debugger_table_publication_invalidates_sessions_and_state_sources_before_any_commit() {
    use crate::string_table::{StringTableAsset, StringTableEntry};
    let mut engine=make_engine(SOURCE);
    let source=engine.debug_state_source().unwrap();
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![]).unwrap();
    assert!(engine.accepts_debug_session(&session));
    engine.text_resources_mut().publish(StringTableAsset { id:"asset:table".into(),namespace:"VN".into(),
        entries:[("key".into(),StringTableEntry{metadata:Default::default(),source:"Changed".into(),developer_notes:String::new()})].into() }).unwrap();
    let state=serde_json::to_value(&engine.state).unwrap();
    assert!(!engine.accepts_debug_state_source(&source));
    assert!(!engine.accepts_debug_session(&session));
    assert!(engine.debug_step(&mut session,StepKind::Continue).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(),state);
    assert!(engine.renderer.batches.is_empty());
}

#[derive(Default)]
pub(in crate::engine) struct Host{batches:Vec<crate::renderer::SceneRenderBatch>,reject:bool}
impl Host{
    pub(in crate::engine) fn retained_count1027(&self)->usize{self.batches.len()}
    pub(in crate::engine) fn retained_state1027(&self)->Option<&GameState>{self.batches.last().map(|batch|&batch.state)}
    pub(in crate::engine) fn reject_scene1027(&mut self,reject:bool){self.reject=reject;}
}
impl Renderer for Host{
    fn validate_scene_batch(&self,_:&crate::renderer::SceneRenderBatch)->Result<(),String>{if self.reject{Err("real renderer refused".into())}else{Ok(())}}
    fn publish_scene_batch(&mut self,batch:crate::renderer::SceneRenderBatch){self.batches.push(batch);}
    fn set_background(&mut self,_:&str,_:&Transition){}
    fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
    fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
    fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
    fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
    fn show_choice(&mut self,_:&[String])->usize{0}
    fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[rvn_parser::Hotspot])->usize{0}
    fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
    fn music_stop(&mut self,_:&Transition){}
    fn music_set_volume(&mut self,_:f32){}
    fn sfx_play(&mut self,_:&str,_:&Transition){}
    fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
fn make_engine(source:&str)->Engine<Host>{
    let mut engine=Engine::from_program(rvn_parser::parse_program(source).unwrap(),Host::default(),16).unwrap();
    engine.enter_label("start").unwrap();engine.renderer.batches.clear();engine
}
fn finish(engine:&mut Engine<Host>,session:&mut DebugSession){
    for _ in 0..128{if engine.debug_step(session,StepKind::Continue).unwrap().0==DebugProgress::Complete{return;}}
    panic!("actual debug operation did not finish")
}
const SOURCE:&str="function inner(p) { local own = p + 1 return own + random(1,9) }\nfunction outer(p) { local caller = 99 return inner(p) }\ninit { set score = 0 }\nscene label start\nset score = outer(4)\n\"Done\"\n";

#[test]
fn debugger_engine_step_into_over_out_publishes_one_actual_operation_after_owned_candidate_stops(){
    let mut engine=make_engine(SOURCE);engine.state.random=crate::random::RandomState::seeded(42);
    let before=engine.debugger_anchor().unwrap();let mut session=engine.debug_begin(DebugOperation::Statement,vec![]).unwrap();
    assert_eq!(engine.debugger_anchor().unwrap(),before);assert!(engine.renderer.batches.is_empty());
    assert_eq!(engine.debug_step(&mut session,StepKind::Into).unwrap().0,DebugProgress::Stopped);
    assert_eq!(session.snapshot().site.as_ref().and_then(|site|site.function.as_deref()),Some("outer"));
    engine.debug_step(&mut session,StepKind::Into).unwrap();engine.debug_step(&mut session,StepKind::Into).unwrap();
    assert_eq!(session.snapshot().site.as_ref().and_then(|site|site.function.as_deref()),Some("inner"));
    assert_eq!(engine.debugger_anchor().unwrap(),before);assert!(engine.renderer.batches.is_empty());
    engine.debug_step(&mut session,StepKind::Over).unwrap();
    assert_eq!(session.snapshot().site.as_ref().and_then(|site|site.function.as_deref()),Some("inner"));
    let progress=engine.debug_step(&mut session,StepKind::Out).unwrap().0;
    if progress!=DebugProgress::Complete{finish(&mut engine,&mut session);}
    assert_eq!(engine.renderer.batches.len(),1);assert_eq!(engine.state.vars["score"],Value::Int({let mut rng=crate::random::RandomState::seeded(42);5+rng.integer(1,9).unwrap()}));
    assert!(!engine.state.vars.contains_key("caller"));assert!(!engine.state.vars.contains_key("own"));
    assert!(engine.debug_step(&mut session,StepKind::Continue).is_err());assert_eq!(engine.renderer.batches.len(),1);
}

#[test]
fn debugger_function_breakpoint_uses_real_utf8_crlf_site_and_rejects_pure_or_missing_ast_routes(){
    let source="// été 🍃\r\nfunction helper(p) {\r\nlocal result = p + 1\r\nreturn result\r\n}\r\nscene label start\r\nset score = helper(3)\r\n\"Done\"\r\n";
    let parsed=rvn_parser::parse_program(source).unwrap();let body_span=parsed.origins[0].span.children[0].range.clone();
    let mut engine=make_engine(source);let program=engine.narrative.as_ref().unwrap();
    let pc=engine.script.iter().position(|statement|matches!(statement,Statement::Function{name,..}if name=="helper")).unwrap();
    let position=program.position(pc);let site=wire::Site{function:Some("helper".into()),route:vec![wire::Route::Statement(0)],depth:1,statement:true};
    let bp=wire::Breakpoint{position:position.clone(),site:Some(site.clone()),enabled:true};
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp.clone()]).unwrap();
    assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);
    let snapshot=session.snapshot();assert_eq!(snapshot.site,Some(site.clone()));assert_eq!(snapshot.origin.as_ref().unwrap().span,Some(body_span.clone()));
    assert!(source.is_char_boundary(body_span.start)&&source.is_char_boundary(body_span.end));assert!(source[body_span].contains("result"));assert!(snapshot.origin.as_ref().unwrap().graph.is_none());
    let mut missing=bp.clone();missing.site.as_mut().unwrap().route=vec![wire::Route::Statement(9)];assert!(engine.debug_begin(DebugOperation::Statement,vec![missing]).is_err());
    let mut pure=bp;pure.site.as_mut().unwrap().statement=false;assert!(engine.debug_begin(DebugOperation::Statement,vec![pure]).is_err());
    let declaration=wire::Breakpoint{position,site:None,enabled:true};assert!(engine.debug_begin(DebugOperation::Statement,vec![declaration]).is_err());
    finish(&mut engine,&mut session);assert_eq!(engine.state.vars["score"],Value::Int(4));
}

#[test]
fn debugger_refuses_stale_instance_changed_owner_state_and_renderer_failure_without_history_or_random_commit(){
    let mut engine=make_engine(SOURCE);let mut other=make_engine(SOURCE);let mut session=engine.debug_begin(DebugOperation::Statement,vec![]).unwrap();
    assert!(!other.accepts_debug_session(&session));let other_before=other.debugger_anchor().unwrap();assert!(other.debug_step(&mut session,StepKind::Into).is_err());assert_eq!(other.debugger_anchor().unwrap(),other_before);
    let before=engine.debugger_anchor().unwrap();engine.renderer.reject=true;
    let mut refused=false;for _ in 0..64{match engine.debug_step(&mut session,StepKind::Continue){Err(error)=>{assert!(error.to_string().contains("real renderer refused"));refused=true;break;},Ok(_)=>{}}}
    assert!(refused);assert_eq!(engine.debugger_anchor().unwrap(),before);assert!(engine.renderer.batches.is_empty());
    engine.renderer.reject=false;let failed=serde_json::to_vec(&session.snapshot()).unwrap();
    assert!(engine.debug_step(&mut session,StepKind::Continue).unwrap_err().to_string().contains("cannot be resumed"));
    assert_eq!(serde_json::to_vec(&session.snapshot()).unwrap(),failed);assert_eq!(engine.debugger_anchor().unwrap(),before);assert!(engine.renderer.batches.is_empty());
    let mut fresh=engine.debug_begin(DebugOperation::Statement,vec![]).unwrap();
    engine.state.vars.insert("external".into(),Value::Int(1));let changed=engine.debugger_anchor().unwrap();assert!(engine.debug_step(&mut fresh,StepKind::Continue).is_err());assert_eq!(engine.debugger_anchor().unwrap(),changed);assert!(engine.renderer.batches.is_empty());
}

#[test]
fn debugger_pc_breakpoint_stops_before_random_evaluation_and_continue_consumes_it_once(){
    let mut engine=make_engine(SOURCE);let position=engine.narrative.as_ref().unwrap().position(engine.state.pc);
    let before=engine.debugger_anchor().unwrap();let mut session=engine.debug_begin(DebugOperation::Statement,vec![wire::Breakpoint{position,site:None,enabled:true}]).unwrap();
    assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);assert_eq!(engine.debugger_anchor().unwrap(),before);
    finish(&mut engine,&mut session);assert_eq!(engine.renderer.batches.len(),1);assert_ne!(engine.debugger_anchor().unwrap(),before);
}

fn heavy_source()->String {
    format!("function heavy() {{ local n = 0\n{}return n }}\nfunction outer() {{local answer=heavy() return answer+7}}\nscene label start\nset score=outer()\n\"Done\"\n","set n=n+1\n".repeat(4500))
}
#[test]
fn debugger_over_and_out_retain_original_caller_across_multiple_bounded_drains(){
    for kind in [StepKind::Over,StepKind::Out]{
        let mut engine=make_engine(&heavy_source());let before=engine.debugger_anchor().unwrap();
        let mut session=engine.debug_begin(DebugOperation::Statement,vec![]).unwrap();
        assert_eq!(engine.debug_step(&mut session,StepKind::Into).unwrap().0,DebugProgress::Stopped);
        assert_eq!(session.snapshot().site.as_ref().and_then(|site|site.function.as_deref()),Some("outer"));
        assert_eq!(engine.debug_step(&mut session,kind).unwrap().0,DebugProgress::Running);
        assert_eq!(session.armed_step_kind(),Some(kind));
        assert_eq!(session.snapshot().site.as_ref().and_then(|site|site.function.as_deref()),Some("heavy"));
        let retained=format!("{:?}",session.snapshot());
        assert!(engine.debug_step(&mut session,StepKind::Into).is_err());
        assert_eq!(format!("{:?}",session.snapshot()),retained);assert_eq!(session.armed_step_kind(),Some(kind));
        assert_eq!(engine.debugger_anchor().unwrap(),before);assert!(engine.renderer.batches.is_empty());
        let mut progress=DebugProgress::Running;
        for _ in 0..8{progress=engine.debug_step(&mut session,kind).unwrap().0;if progress!=DebugProgress::Running{break;}}
        if kind==StepKind::Over{
            assert_eq!(progress,DebugProgress::Stopped);
            let site=session.snapshot().site.unwrap();assert_eq!(site.function.as_deref(),Some("outer"));assert_eq!(site.depth,1);
            assert_eq!(site.route,vec![wire::Route::Statement(1)]);assert_eq!(session.armed_step_kind(),None);
            assert_eq!(engine.debugger_anchor().unwrap(),before);finish(&mut engine,&mut session);
        }else{assert_eq!(progress,DebugProgress::Complete);}
        assert_eq!(engine.state.vars["score"],Value::Int(4507));assert_eq!(engine.renderer.batches.len(),1);
    }
}

fn function_bp(engine:&Engine<Host>,name:&str,route:Vec<wire::Route>)->wire::Breakpoint{
    let pc=engine.script.iter().position(|statement|matches!(statement,Statement::Function{name:actual,..}if actual==name)).unwrap();
    wire::Breakpoint{position:engine.narrative.as_ref().unwrap().position(pc),site:Some(wire::Site{function:Some(name.into()),route,depth:1,statement:true}),enabled:true}
}
#[test]
fn debugger_stopped_visit_skips_once_for_step_stops_then_rearms_loop_and_recursive_breakpoints(){
    let mut engine=make_engine(SOURCE);let first=function_bp(&engine,"outer",vec![wire::Route::Statement(0)]);
    let next=function_bp(&engine,"outer",vec![wire::Route::Statement(1)]);
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![first,next]).unwrap();
    assert_eq!(engine.debug_step(&mut session,StepKind::Into).unwrap().0,DebugProgress::Stopped);
    assert_eq!(session.snapshot().site.unwrap().route,vec![wire::Route::Statement(0)]);
    assert_eq!(engine.debug_step(&mut session,StepKind::Over).unwrap().0,DebugProgress::Stopped);
    assert_eq!(session.snapshot().site.unwrap().route,vec![wire::Route::Statement(1)]);
    finish(&mut engine,&mut session);assert_eq!(engine.renderer.batches.len(),1);

    let mut engine=make_engine("function loop() {local total=0 while total<3 {set total=total+1} return total}\nscene label start\nset score=loop()\n\"Done\"\n");
    let breakpoint=function_bp(&engine,"loop",vec![wire::Route::Statement(1),wire::Route::Body,wire::Route::Statement(0)]);
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![breakpoint]).unwrap();
    session.set_watches(vec![wire::Watch{scope:wire::WatchScope::Frame(1),name:"total".into()}]).unwrap();
    for total in 0..3{
        assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);
        assert_eq!(session.snapshot().watches[0].value,wire::WatchValue::Value(Value::Int(total)));
    }
    finish(&mut engine,&mut session);assert_eq!(engine.state.vars["score"],Value::Int(3));

    let mut engine=make_engine("function recur(n) {if n>0 {return recur(n-1)} return n}\nscene label start\nset score=recur(4)\n\"Done\"\n");
    let breakpoint=function_bp(&engine,"recur",vec![wire::Route::Statement(0)]);
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![breakpoint]).unwrap();
    for depth in 1..=5{
        // An explicit BP in a recursive callee takes precedence over Over's
        // retained caller/successor target, including the same authored site.
        if depth>1{
            assert_eq!(engine.debug_step(&mut session,StepKind::Over).unwrap().0,DebugProgress::Stopped);
            assert_eq!(session.snapshot().site.unwrap().route,vec![wire::Route::Statement(0),wire::Route::Then,wire::Route::Statement(0)]);
        }
        let command=if depth==1{StepKind::Continue}else{StepKind::Over};
        assert_eq!(engine.debug_step(&mut session,command).unwrap().0,DebugProgress::Stopped);
        assert_eq!(session.snapshot().site.unwrap().depth,depth);
    }
    finish(&mut engine,&mut session);assert_eq!(engine.state.vars["score"],Value::Int(0));
}

#[test]
fn debugger_configuration_validates_caps_sites_and_watches_before_any_session_replacement(){
    let engine=make_engine(SOURCE);let breakpoint=function_bp(&engine,"outer",vec![wire::Route::Statement(0)]);
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![breakpoint.clone()]).unwrap();
    let watches=vec![wire::Watch{scope:wire::WatchScope::Globals,name:"score".into()}];
    engine.configure_debug_session(&mut session,vec![breakpoint.clone()],watches.clone()).unwrap();
    let before=engine.debugger_anchor().unwrap();let old=session.breakpoints.clone();let old_watches=session.watches.clone();
    assert!(engine.configure_debug_session(&mut session,vec![breakpoint.clone();wire::MAX_BREAKPOINTS+1],vec![]).is_err());
    let mut invalid=breakpoint.clone();invalid.site.as_mut().unwrap().route=vec![wire::Route::Statement(90)];
    assert!(engine.configure_debug_session(&mut session,vec![invalid],vec![]).is_err());
    assert!(engine.configure_debug_session(&mut session,vec![],vec![wire::Watch{scope:wire::WatchScope::Globals,name:"bad.name".into()}]).is_err());
    assert_eq!(session.breakpoints,old);assert_eq!(session.watches,old_watches);assert_eq!(engine.debugger_anchor().unwrap(),before);
    let other=make_engine(SOURCE);assert!(other.configure_debug_session(&mut session,vec![],vec![]).is_err());
    assert_eq!(session.breakpoints,old);assert_eq!(session.watches,old_watches);
}


#[test]
fn debugger_exact_unicode_global_and_lexical_frame_watches_use_real_candidate_values() {
    let mut engine=make_engine("function helper(`Param été`) {local `Total été`=`Param été`+1 return `Total été`}\ninit {set `Confiance été`=5}\nscene label start\nset `Résultat été`=helper(41)\n\"Done\"\n");
    let watches=vec![wire::Watch{scope:wire::WatchScope::Globals,name:"Confiance été".into()},wire::Watch{scope:wire::WatchScope::Frame(1),name:"Total été".into()}];
    let command=wire::Command::Configure{breakpoints:vec![],watches:watches.clone()};
    assert_eq!(wire::Command::decode(&command.encode().unwrap()).unwrap(),command);
    let before=engine.debugger_anchor().unwrap();
    let breakpoint=function_bp(&engine,"helper",vec![wire::Route::Statement(1)]);
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![breakpoint]).unwrap();session.set_watches(watches).unwrap();
    assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);
    let snapshot=session.snapshot();assert_eq!(snapshot.watches[0].value,wire::WatchValue::Value(Value::Int(5)));assert_eq!(snapshot.watches[1].value,wire::WatchValue::Value(Value::Int(42)));
    assert_eq!(engine.debugger_anchor().unwrap(),before);finish(&mut engine,&mut session);
    assert_eq!(engine.state.vars["Résultat été"],Value::Int(42));assert!(!engine.state.vars.contains_key("Total été"));
    let committed=engine.debug_current_snapshot(&[wire::Watch{scope:wire::WatchScope::Globals,name:"Résultat été".into()}]).unwrap();
    assert_eq!(committed.watches[0].value,wire::WatchValue::Value(Value::Int(42)));
    for name in ["bad.name","__rvn_private","bad\nname",""] {
        let command=wire::Command::Configure{breakpoints:vec![],watches:vec![wire::Watch{scope:wire::WatchScope::Globals,name:name.into()}]};
        assert!(wire::Command::decode(&command.encode().unwrap()).is_err());
    }
}

const CONSUMER_SAVE_SOURCE:&str=r#"
function producer() { set draw=random(1,1) return __rvn_string_table_text_name(__rvn_literal_name("asset:Table"),"hello") }
function consumer() {
    local caller_only=37
    flow.consume_v1("scope","first") { capture "p"=producer() inspect "i"=__rvn_captured_v1("p") set before=__rvn_terminal_v1("i","table_id") }
    flow.consume_v1("scope","second") { inspect "i"=__rvn_invariant_text("literal") return [__rvn_terminal_v1("i","valid"),__rvn_terminal_v1("i","table_id"),__rvn_terminal_v1("i","key")] }
}
scene label start
set result=consumer()
"Done"
"#;
fn consumer_program(path:&str)->rvn_parser::ParsedProgram {
    let mut program=rvn_parser::parse_program(CONSUMER_SAVE_SOURCE).unwrap();let path=Some(std::path::PathBuf::from(path));program.sources[0].path=path.clone();for origin in &mut program.origins{origin.source=path.clone();}program
}
fn closed_nonempty_text_context()->TextRuntimeContext {
    use crate::string_table::{StringTableAsset,StringTableEntry,StringTableResources};
    use crate::locale::{LocaleCatalog,LocaleManager,LocaleTable};
    let mut resources=StringTableResources::default();resources.publish(StringTableAsset{id:"asset:Table".into(),namespace:"VN".into(),entries:[("hello".into(),StringTableEntry{source:"Hello".into(),developer_notes:"real fixture notes".into(),metadata:[("speaker".into(),"hero".into())].into()})].into()}).unwrap();
    let locale=LocaleManager::from_frozen_catalogs("en","fr",vec!["en".into(),"fr".into()],HashMap::from([
        ("en".into(),LocaleCatalog::with_text_sources(LocaleTable{lang:"en".into(),strings:HashMap::from([("greeting".into(),"Hello".into())])},HashMap::new()).unwrap()),
        ("fr".into(),LocaleCatalog::with_text_sources(LocaleTable{lang:"fr".into(),strings:HashMap::from([("greeting".into(),"Bonjour".into())])},HashMap::new()).unwrap()),
    ])).unwrap();TextRuntimeContext::new(resources,Some(locale))
}
fn consumer_engine(program:rvn_parser::ParsedProgram)->Engine<Host>{
    let mut engine=Engine::from_program_with_debugger_and_text_context(program,Host::default(),16,HashMap::new(),closed_nonempty_text_context()).unwrap();engine.enter_label("start").unwrap();engine.renderer.batches.clear();engine.state.random=crate::random::RandomState::seeded(4);engine
}
#[test]
fn consume_v2_nested_real_source_cold_save_restores_parked_caller_without_rng_replay() {
    let source=include_str!("../tests/fixtures/consume_v2_nested_save.rvn");
    let make=|path:&str|{let mut p=rvn_parser::parse_program(source).unwrap();let path=Some(std::path::PathBuf::from(path));p.sources[0].path=path.clone();for o in &mut p.origins{o.source=path.clone();}p};
    let original=make("C:/original/consume_v2_nested_save.rvn");let relocated=make("D:/relocated/consume_v2_nested_save.rvn");
    let mut engine=consumer_engine(original.clone());let before=serde_json::to_value(&engine.state).unwrap();
    let session=stopped_consumer(&mut engine);assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);
    let save=engine.capture_save15(&original,Some(&session),1,"v2 nested producer".into()).unwrap();
    let mut cold=consumer_engine(relocated.clone());let mut resumed=cold.restore_save15(&relocated,crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap().unwrap();
    resumed.set_watches(vec![wire::Watch{scope:wire::WatchScope::Frame(1),name:"caller_only".into()}]).unwrap();
    assert_eq!(resumed.snapshot().watches[0].value,wire::WatchValue::Value(Value::Int(37)));
    finish(&mut cold,&mut resumed);assert_eq!(cold.state.vars["result"],Value::List(vec![Value::Int(12),Value::Int(37)]));
    let mut expected=crate::random::RandomState::seeded(4);expected.integer(1,1).unwrap();assert_eq!(cold.state.random,expected);
    assert!(!cold.state.vars.contains_key("bad"));assert_eq!(cold.renderer.batches.len(),2);
}
#[test]
fn consume_v2_nested_child_body_cold_save_reconstructs_innermost_real_consumer_phase() {
    let source=include_str!("../tests/fixtures/consume_v2_nested_save.rvn");let program=rvn_parser::parse_program(source).unwrap();let mut engine=consumer_engine(program.clone());
    let pc=engine.script.iter().position(|s|matches!(s,Statement::Function{name,..}if name=="caller")).unwrap();
    let route=vec![wire::Route::Statement(1),wire::Route::Body,wire::Route::Statement(0),wire::Route::Then,wire::Route::Statement(0),wire::Route::Body,wire::Route::Statement(0),wire::Route::Branch(1),wire::Route::Statement(0),wire::Route::Body,wire::Route::Statement(0),wire::Route::Then,wire::Route::Statement(0),wire::Route::Body,wire::Route::Statement(0)];
    let bp=wire::Breakpoint{position:engine.narrative.as_ref().unwrap().position(pc),site:Some(wire::Site{function:Some("caller".into()),route,depth:1,statement:true}),enabled:true};
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp]).unwrap();assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);
    let save=engine.capture_save15(&program,Some(&session),1,"v2 active child body".into()).unwrap();let mut cold=consumer_engine(program.clone());
    let mut resumed=cold.restore_save15(&program,crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap().unwrap();finish(&mut cold,&mut resumed);
    assert_eq!(cold.state.vars["result"],Value::List(vec![Value::Int(12),Value::Int(37)]));let mut expected=crate::random::RandomState::seeded(4);expected.integer(1,1).unwrap();assert_eq!(cold.state.random,expected);
}
fn stopped_consumer(engine:&mut Engine<Host>)->DebugSession {
    let pc=engine.script.iter().position(|s|matches!(s,Statement::Function{name,..}if name=="producer")).unwrap();
    let bp=wire::Breakpoint{position:engine.narrative.as_ref().unwrap().position(pc),site:Some(wire::Site{function:Some("producer".into()),route:vec![wire::Route::Statement(1)],depth:2,statement:true}),enabled:true};
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp]).unwrap();assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);
    assert_eq!(session.snapshot().site.as_ref().unwrap().function.as_deref(),Some("producer"));session
}
#[test]
fn consumer_player_save15_real_slot_cold_relocated_resume_retains_caller_and_single_random_effect(){
    use crate::save::consumer_checkpoint::Save15;
    let original=consumer_program("C:/original/story.rvn");let relocated=consumer_program("D:/relocated/story.rvn");
    let mut engine=consumer_engine(original.clone());let before=serde_json::to_value(&engine.state).unwrap();let session=stopped_consumer(&mut engine);
    assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);assert!(engine.renderer.batches.is_empty());
    let save=engine.capture_save15(&original,Some(&session),1,"mid producer".into()).unwrap();let json=save.encode().unwrap();
    assert_eq!(save.format_version,15);assert!(json.contains("Bonjour")&&json.contains("real fixture notes"));
    let directory=std::env::temp_dir().join(format!("rvn-save15-0302-{}",std::process::id()));let manager=crate::save::SaveManager::new(&directory,4).unwrap();
    manager.save15_slot(&save).unwrap();manager.save15_autosave(&save).unwrap();manager.save15_quicksave(&save).unwrap();
    assert_eq!(serde_json::to_value(manager.load15_autosave().unwrap()).unwrap(),serde_json::to_value(&save).unwrap());assert_eq!(serde_json::to_value(manager.load15_quicksave().unwrap()).unwrap(),serde_json::to_value(&save).unwrap());
    assert!(manager.load(1).is_err(),"historical reader must never silently downgrade Save15");
    let mut cold=consumer_engine(relocated.clone());let old_epoch=cold.runtime_epoch();let old_source=cold.debug_state_source().unwrap();
    let mut resumed=cold.restore_save15(&relocated,manager.load15_slot(1).unwrap()).unwrap().unwrap();
    assert!(cold.runtime_epoch()>old_epoch);assert!(!cold.accepts_debug_state_source(&old_source));assert!(cold.accepts_debug_session(&resumed));
    assert_eq!(resumed.snapshot().site.as_ref().unwrap().function.as_deref(),Some("producer"));assert_eq!(resumed.snapshot().frames.len(),3);
    resumed.set_watches(vec![wire::Watch{scope:wire::WatchScope::Frame(1),name:"caller_only".into()}]).unwrap();
    assert_eq!(resumed.snapshot().watches[0].value,wire::WatchValue::Value(Value::Int(37)));
    assert_eq!(cold.renderer.batches.len(),1,"restore publishes exactly one preflighted batch");
    finish(&mut cold,&mut resumed);let mut expected=crate::random::RandomState::seeded(4);expected.integer(1,1).unwrap();
    assert_eq!(cold.state.random,expected);assert_eq!(cold.state.vars["result"],Value::List(vec![Value::Bool(false),Value::Name(rvn_parser::Name::new("asset:Table").unwrap()),Value::Str("hello".into())]));
    assert_eq!(cold.renderer.batches.len(),2,"resume commits once after restored candidate finishes");
    let mut downgrade:serde_json::Value=serde_json::from_str(&json).unwrap();downgrade["format_version"]=serde_json::json!(14);assert!(Save15::decode(&downgrade.to_string()).is_err());
    manager.set_protected(1,true).unwrap();assert!(manager.save15_slot(&save).is_err());assert_eq!(serde_json::to_value(manager.load15_slot(1).unwrap()).unwrap(),serde_json::to_value(&save).unwrap());
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn consumer_save15_rejects_changed_source_resources_cursor_and_renderer_before_live_state_or_publication(){
    use crate::save::consumer_checkpoint::Save15;
    let program=consumer_program("C:/actual/story.rvn");let mut engine=consumer_engine(program.clone());let session=stopped_consumer(&mut engine);let save=engine.capture_save15(&program,Some(&session),1,"checkpoint".into()).unwrap();
    let mut cold=consumer_engine(program.clone());let baseline=serde_json::to_value(&cold.state).unwrap();let epoch=cold.runtime_epoch();
    let mut bad=serde_json::to_value(&save).unwrap();bad["source"]["source_bytes_in_inclusion_order"][0]=serde_json::json!("changed source");let bad=Save15::decode(&bad.to_string()).unwrap();assert!(cold.restore_save15(&program,bad).is_err());
    let mut bad=serde_json::to_value(&save).unwrap();bad["debugger"]["trace"]["pending"]["tasks"][0]["kind"]=serde_json::json!("injected_effect");let bad=Save15::decode(&bad.to_string()).unwrap();assert!(cold.restore_save15(&program,bad).is_err());
    let mut bad=serde_json::to_value(&save).unwrap();bad["story"]["frames"][0]["owner"]=serde_json::json!({"kind":"scene","activation":987});let bad=Save15::decode(&bad.to_string()).unwrap();assert!(cold.restore_save15(&program,bad).is_err());
    cold.renderer.reject=true;assert!(cold.restore_save15(&program,save.clone()).is_err());cold.renderer.reject=false;
    assert_eq!(serde_json::to_value(&cold.state).unwrap(),baseline);assert_eq!(cold.runtime_epoch(),epoch);assert!(cold.renderer.batches.is_empty());
    cold.text_resources_mut().publish(crate::string_table::StringTableAsset{id:"asset:Table".into(),namespace:"VN".into(),entries:Default::default()}).unwrap();
    assert!(cold.restore_save15(&program,save).is_err());assert_eq!(serde_json::to_value(&cold.state).unwrap(),baseline);assert!(cold.renderer.batches.is_empty());
}

#[test]
fn consumer_save15_true_engine_set_and_local_roots_resume_original_value_with_existing_rebased_frame(){
    for binding in ["set result","local result"] {
        let source=format!(r#"
function producer(real) {{ local draw=random(1,1) if real {{ return __rvn_string_table_text_name(__rvn_literal_name("asset:Table"),"hello") }} return __rvn_invariant_text("literal") }}
scene label start
flow.consume_v1("same_scope","true") {{ capture "p"=producer(true) inspect "i"=__rvn_captured_v1("p") set first=__rvn_terminal_v1("i","table_id") }}
"Before"
flow.consume_v1("same_scope","false") {{ capture "p"=producer(false) inspect "i"=__rvn_captured_v1("p") {binding}=[__rvn_terminal_v1("i","valid"),__rvn_terminal_v1("i","table_id"),__rvn_terminal_v1("i","key")] }}
"After"
"#);
        let make_program=|path:&str|{let mut p=rvn_parser::parse_program(&source).unwrap();let path=Some(std::path::PathBuf::from(path));p.sources[0].path=path.clone();for origin in &mut p.origins{origin.source=path.clone();}p};
        let original=make_program("C:/real/root.rvn");let relocated=make_program("D:/real/root.rvn");
        let mut engine=consumer_engine(original.clone());engine.step_until_interaction().unwrap();engine.advance_dialogue().unwrap();engine.renderer.batches.clear();
        let before=serde_json::to_value(&engine.state).unwrap();let before_random=engine.state.random;
        let pc=engine.script.iter().position(|s|matches!(s,Statement::Function{name,..}if name=="producer")).unwrap();
        let bp=wire::Breakpoint{position:engine.narrative.as_ref().unwrap().position(pc),site:Some(wire::Site{function:Some("producer".into()),route:vec![wire::Route::Statement(2)],depth:1,statement:true}),enabled:true};
        let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp]).unwrap();assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);
        assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);assert!(engine.renderer.batches.is_empty());
        let save=engine.capture_save15(&original,Some(&session),1,binding.into()).unwrap();
        let mut cold=consumer_engine(relocated.clone());let mut resumed=cold.restore_save15(&relocated,crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap().unwrap();
        assert_eq!(resumed.snapshot().frames.len(),2);finish(&mut cold,&mut resumed);
        let expected_value=Value::List(vec![Value::Bool(false),Value::Name(rvn_parser::Name::new("asset:Table").unwrap()),Value::Str("hello".into())]);
        if binding.starts_with("local"){assert_eq!(cold.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals["result"],expected_value);assert!(!cold.state.vars.contains_key("result"));}
        else{assert_eq!(cold.state.vars["result"],expected_value);}
        let mut expected_random=before_random;expected_random.integer(1,1).unwrap();assert_eq!(cold.state.random,expected_random,"real root must resume after its single already-completed producer draw");
        assert_eq!(cold.renderer.batches.len(),2);
    }
}

#[test]
fn consumer_save15_parked_scene_caller_frame_cold_restore_and_actual_rollback_remain_owned(){
    let source=r#"
scene label start
flow.consume_v1("shared_scope","parent_true") { inspect "i"=__rvn_string_table_text_name(__rvn_literal_name("asset:Table"),"hello") local parent_id=__rvn_terminal_v1("i","table_id") }
call child
flow.consume_v1("shared_scope","parent_false") { inspect "i"=__rvn_invariant_text("literal") set retained=[__rvn_terminal_v1("i","valid"),__rvn_terminal_v1("i","table_id"),__rvn_terminal_v1("i","key")] }
"parent"
flow.consume_v1("shared_scope","parent_changed") { inspect "i"=__rvn_string_table_text_name(__rvn_literal_name("asset:Table"),"different") set changed=__rvn_terminal_v1("i","key") }
"changed"
jump end
scene label child
flow.consume_v1("shared_scope","child_fresh_false") { inspect "i"=__rvn_invariant_text("literal") local child_fresh=[__rvn_terminal_v1("i","valid"),__rvn_terminal_v1("i","table_id"),__rvn_terminal_v1("i","key")] }
"child"
return
scene label end
"end"
"#;
    let make=|path:&str|{let mut p=rvn_parser::parse_program(source).unwrap();let path=Some(std::path::PathBuf::from(path));p.sources[0].path=path.clone();for origin in &mut p.origins{origin.source=path.clone();}p};
    let original=make("C:/old/parked.rvn");let relocated=make("D:/new/parked.rvn");let mut engine=consumer_engine(original.clone());engine.step_until_interaction().unwrap();
    let execution=engine.state.scene_execution.as_ref().unwrap();assert_eq!(execution.frames.len(),1);assert!(execution.frames[0].parked.as_ref().unwrap().consumer_frame.is_some());
    assert_eq!(execution.active.as_ref().unwrap().locals["child_fresh"],Value::List(vec![Value::Bool(false),Value::Name(rvn_parser::Name::new("None").unwrap()),Value::Str(String::new())]));
    let save=engine.capture_save15(&original,None,2,"parked caller".into()).unwrap();let mut cold=consumer_engine(relocated.clone());assert!(cold.restore_save15(&relocated,crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap().is_none());
    assert_eq!(cold.state.scene_execution.as_ref().unwrap().frames.len(),1);cold.advance_dialogue().unwrap();cold.step_until_interaction().unwrap();
    assert_eq!(cold.state.vars["retained"],Value::List(vec![Value::Bool(false),Value::Name(rvn_parser::Name::new("asset:Table").unwrap()),Value::Str("hello".into())]));
    let expected_frame=cold.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().consumer_frame.clone().unwrap();let random=cold.state.random;let pc=cold.state.pc;
    cold.advance_dialogue().unwrap();cold.step_until_interaction().unwrap();assert_eq!(cold.state.vars["changed"],Value::Str("different".into()));let latest=cold.state.pc;assert!(cold.rollback());assert_eq!(cold.state.pc,latest);assert!(cold.rollback());
    assert_eq!(cold.state.pc,pc);assert_eq!(cold.state.random,random);assert_eq!(cold.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().consumer_frame.as_ref().unwrap(),&expected_frame);assert!(!cold.state.vars.contains_key("changed"));
}

#[test]
fn consumer_save15_accepts_full_actual_maximum_scene_stack_with_active_and_game_frames(){
    let mut source=String::new();
    for depth in 0..=crate::scene::MAX_NARRATIVE_FRAMES {
        let label=if depth==0{"start".into()}else{format!("scene{depth}")};
        source.push_str(&format!("scene label {label}\nflow.consume_v1(\"full_stack\",\"consumer{depth}\") {{inspect \"i\"=__rvn_invariant_text(\"literal\") set r=__rvn_terminal_v1(\"i\",\"valid\")}}\n"));
        if depth<crate::scene::MAX_NARRATIVE_FRAMES{source.push_str(&format!("call scene{}\nreturn\n",depth+1));}else{source.push_str("\"deep\"\n");}
    }
    let program=rvn_parser::parse_program(&source).unwrap();let mut engine=consumer_engine(program.clone());engine.step_until_interaction().unwrap();
    assert_eq!(engine.state.scene_execution.as_ref().unwrap().frames.len(),crate::scene::MAX_NARRATIVE_FRAMES);
    let save=engine.capture_save15(&program,None,1,"maximum".into()).unwrap();let encoded=serde_json::to_value(&save).unwrap();assert_eq!(encoded["story"]["frames"].as_array().unwrap().len(),2+crate::scene::MAX_NARRATIVE_FRAMES);
    let mut cold=consumer_engine(program.clone());assert!(cold.restore_save15(&program,crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap().is_none());assert_eq!(cold.state.scene_execution.as_ref().unwrap().frames.len(),crate::scene::MAX_NARRATIVE_FRAMES);
}

#[test]
fn consumer_save15_user_dictionary_plan_like_keys_are_inert_in_real_source_and_retained_data(){
    let dict=r#"{"ScheduledConsumer":{"scope":"ordinary data","steps":[]},"Function":{"body":[]},"source":"C:/ordinary/data","span":{"start":0,"end":1},"route":[],"generated":false}"#;
    let source=format!(r#"function producer() {{local draw=random(1,1) return __rvn_invariant_text("literal")}}
function consumer() {{local data={dict} flow.consume_v1("scope","consumer") {{inspect "i"=producer() return __rvn_terminal_v1("i","valid")}}}}
scene label start
set harmless={dict}
set result=consumer()
"done""#);
    let program=rvn_parser::parse_program(&source).unwrap();let mut engine=consumer_engine(program.clone());engine.preview_step_statement().unwrap();let expected=engine.state.vars["harmless"].clone();let mut session=stopped_consumer(&mut engine);
    session.set_watches(vec![wire::Watch{scope:wire::WatchScope::Frame(1),name:"data".into()}]).unwrap();assert_eq!(session.snapshot().watches[0].value,wire::WatchValue::Value(expected.clone()));
    let save=engine.capture_save15(&program,Some(&session),1,"inert dict".into()).unwrap();let mut cold=consumer_engine(program.clone());let mut resumed=cold.restore_save15(&program,crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap().unwrap();
    assert_eq!(cold.state.vars["harmless"],expected);assert_eq!(resumed.snapshot().watches[0].value,wire::WatchValue::Value(expected));finish(&mut cold,&mut resumed);assert_eq!(cold.state.vars["result"],Value::Bool(false));
}
#[test]
fn consumer_save15_deep_valid_source_ast_roundtrip_exceeds_old_json_limit_without_repeating_producer(){
    let value=std::iter::repeat_n("1",80).collect::<Vec<_>>().join("+");
    let source=format!(r#"function producer() {{local draw=random(1,1) return __rvn_invariant_text("literal")}}
scene label start
flow.consume_v1("deep","consumer") {{inspect "i"=producer() set result={value}}}
"done""#);
    let program=rvn_parser::parse_program(&source).unwrap();let mut engine=consumer_engine(program.clone());let session=stopped_consumer(&mut engine);let save=engine.capture_save15(&program,Some(&session),1,"deep source".into()).unwrap();let json=save.encode().unwrap();
    assert!(serde_json::from_str::<crate::save::consumer_checkpoint::Save15>(&json).is_err(),"fixture must exceed historical serde_json recursion limit");
    let mut cold=consumer_engine(program.clone());let mut resumed=cold.restore_save15(&program,crate::save::consumer_checkpoint::Save15::decode(&json).unwrap()).unwrap().unwrap();finish(&mut cold,&mut resumed);assert_eq!(cold.state.vars["result"],Value::Int(80));let mut expected=crate::random::RandomState::seeded(4);expected.integer(1,1).unwrap();assert_eq!(cold.state.random,expected);
    let state=serde_json::to_value(&cold.state).unwrap();let batches=cold.renderer.batches.len();let too_deep=format!("{}0{}","[".repeat(1025),"]".repeat(1025));assert!(crate::save::consumer_checkpoint::Save15::decode(&too_deep).unwrap_err().to_string().contains("depth budget"));assert_eq!(serde_json::to_value(&cold.state).unwrap(),state);assert_eq!(cold.renderer.batches.len(),batches);
}

#[test]
fn consumer_save15_near_json_nesting_boundary_roundtrip_isolated_process(){
    if std::env::var_os("RVN_0302_DEPTH_PROBE").is_none(){
        let output=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","engine::debugger_commands::tests::consumer_save15_near_json_nesting_boundary_roundtrip_isolated_process","--nocapture"]).env("RVN_0302_DEPTH_PROBE","1").output().unwrap();
        assert!(output.status.success(),"isolated boundary probe failed: {}\n{}",String::from_utf8_lossy(&output.stdout),String::from_utf8_lossy(&output.stderr));
        eprintln!("{}",String::from_utf8_lossy(&output.stdout));return;
    }
    use rvn_parser::{Expr,BinOpKind};use crate::save::consumer_checkpoint::Save15;
    let program=consumer_program("C:/depth/probe.rvn");let mut engine=consumer_engine(program.clone());let session=stopped_consumer(&mut engine);let base=engine.capture_save15(&program,Some(&session),1,"depth probe".into()).unwrap();
    fn json_depth(text:&str)->usize{let(mut depth,mut max,mut string,mut escape)=(0usize,0usize,false,false);for b in text.bytes(){if string{if escape{escape=false;}else if b==b'\\'{escape=true;}else if b==b'"'{string=false;}continue;}match b{b'"'=>string=true,b'{'|b'['=>{depth+=1;max=max.max(depth);},b'}'|b']'=>depth-=1,_=>{}}}max}
    let ast_depth=254;
    let mut admitted=None;
    for blocks in 0..=256 {
        let mut value=Expr::Int(1);for _ in 0..ast_depth{value=Expr::BinOp{op:BinOpKind::Add,left:Box::new(value),right:Box::new(Expr::Int(1))};}
        let mut statement=Statement::SetVar{name:"comparison_witness".into(),value};
        for _ in 0..blocks{statement=Statement::If{condition:Expr::Bool(true),then_branch:vec![statement],else_branch:vec![]};}
        let mut candidate=base.clone();candidate.source.script=vec![statement];
        match candidate.encode(){Ok(json)=>{let depth=json_depth(&json);if depth==crate::save::consumer_checkpoint::MAX_JSON_DEPTH{println!("before decode");let decoded=Save15::decode(&json).unwrap();println!("after decode before equality/drop");assert_eq!(decoded.source.script,candidate.source.script);drop(decoded);println!("after drop");candidate.source.script=vec![Statement::If{condition:Expr::Bool(true),then_branch:std::mem::take(&mut candidate.source.script),else_branch:vec![]}];let refusal=candidate.encode().unwrap_err().to_string();assert!(refusal.contains("depth budget")&&refusal.contains("before allocation"),"{refusal}");let hostile=serde_json::to_string(&candidate).unwrap();assert!(Save15::decode(&hostile).unwrap_err().to_string().contains("before decode"));drop(candidate);admitted=Some(depth);break;}},Err(error)=>{assert!(error.to_string().contains("depth")||error.to_string().contains("nesting"));}}
    }
    let depth=admitted.expect("probe must reach the actual writer/reader boundary, not just depth80");assert_eq!(depth,crate::save::consumer_checkpoint::MAX_JSON_DEPTH);println!("isolated actual admitted JSON depth={depth}");
    let mut malformed=serde_json::to_value(&base).unwrap();let mut dynamic=serde_json::Value::Null;
    for nesting in 1..=640 {dynamic=serde_json::Value::Array(vec![dynamic]);malformed["debugger"]["trace"]["pending"]["tasks"][0]["data"]=dynamic.clone();let json=serde_json::to_string(&malformed).unwrap();let depth=json_depth(&json);if depth==crate::save::consumer_checkpoint::MAX_JSON_DEPTH{println!("dynamic cursor JSON depth={depth} before decode");let decoded=Save15::decode(&json).unwrap();println!("dynamic cursor decoded before drop");drop(decoded);println!("dynamic cursor dropped");break;}assert!(depth<crate::save::consumer_checkpoint::MAX_JSON_DEPTH,"fixture must reach boundary exactly");if nesting==640{panic!("dynamic cursor fixture did not reach boundary");}}
    for nesting in [641,1024,1025]{let near=format!("{}0{}","[".repeat(nesting),"]".repeat(nesting));assert!(Save15::decode(&near).unwrap_err().to_string().contains("depth budget"));}
    let mut hostile=base.clone();let mut value=Expr::Int(1);for _ in 0..257{value=Expr::BinOp{op:BinOpKind::Add,left:Box::new(value),right:Box::new(Expr::Int(1))};}hostile.source.script=vec![Statement::SetVar{name:"hostile".into(),value}];assert!(hostile.encode().unwrap_err().to_string().contains("expression nesting"));
    let mut nested=Statement::SetVar{name:"nested".into(),value:Expr::Int(1)};for _ in 0..257{nested=Statement::If{condition:Expr::Bool(true),then_branch:vec![nested],else_branch:vec![]};}hostile.source.script=vec![nested];assert!(hostile.encode().unwrap_err().to_string().contains("depth"));
}

#[test]
fn consumer_save15_actual_maximum_expression_source_cold_resume_keeps_single_producer_effect(){
    let value=std::iter::repeat_n("1",257).collect::<Vec<_>>().join("+");
    let source=format!(r#"function producer() {{local draw=random(1,1) return __rvn_invariant_text("literal")}}
scene label start
flow.consume_v1("maximum_ast","consumer") {{inspect "i"=producer() set result={value}}}
"done""#);
    let program=rvn_parser::parse_program(&source).unwrap();let mut engine=consumer_engine(program.clone());let session=stopped_consumer(&mut engine);let before=serde_json::to_value(&engine.state).unwrap();let batches=engine.renderer.batches.len();
    let save=engine.capture_save15(&program,Some(&session),1,"actual AST depth256".into()).unwrap();let json=save.encode().unwrap();let mut cold=consumer_engine(program.clone());let mut resumed=cold.restore_save15(&program,crate::save::consumer_checkpoint::Save15::decode(&json).unwrap()).unwrap().unwrap();finish(&mut cold,&mut resumed);assert_eq!(cold.state.vars["result"],Value::Int(257));let mut expected=crate::random::RandomState::seeded(4);expected.integer(1,1).unwrap();assert_eq!(cold.state.random,expected);
    let mut hostile=program.clone();let mut value=rvn_parser::Expr::Int(1);for _ in 0..257{value=rvn_parser::Expr::Neg(Box::new(value));}hostile.script=vec![Statement::SetVar{name:"too_deep".into(),value}];let error=engine.capture_save15(&hostile,Some(&session),1,"hostile".into()).unwrap_err().to_string();assert!(error.contains("expression nesting"));assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);assert_eq!(engine.renderer.batches.len(),batches);
}

// This existing child module is cfg(test) at its parent declaration. Keep the
// production DebugSession layout/visibility and normal source byte-identical.
impl super::DebugSession {
    pub(crate) fn assert_actual_deferred_replay_for_test(
        &self, trace:&crate::eval::debugger_eval::PortableTrace,
    ) {
        assert!(!self.complete && self.error.is_none() && self.armed_step.is_none(),
            "deferred replay fixture requires the actual stopped session");
        let actual=self.continuation.as_ref().expect("actual stopped continuation");
        trace.assert_deferred_replay_for_test(actual);
    }
}
