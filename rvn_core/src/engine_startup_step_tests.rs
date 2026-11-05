use super::*;
use std::sync::{Arc,atomic::{AtomicBool,AtomicUsize,Ordering}};
fn program(source:&str)->rvn_parser::ParsedProgram{
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    let root=std::path::PathBuf::from(std::env::var("RVN_STARTUP_STEP_FIXTURES").expect("private step fixtures"));
    let directory=root.join(format!("step-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("main.rvn"),source).unwrap();rvn_parser::parse_file_program_with_uses(directory.join("main.rvn")).unwrap()
}
fn pending(source:&str)->EngineStartup<crate::TerminalRenderer>{Engine::prepare_debugger_startup(program(source),crate::TerminalRenderer,8,HashMap::new(),TextRuntimeContext::default(),None).unwrap()}
fn live<R:Renderer>(s:&EngineStartup<R>)->(serde_json::Value,Option<[u8;32]>){(serde_json::to_value(s.state()).unwrap(),s.session.as_ref().map(|session|session.startup_step_anchor().unwrap()))}
const NATIVE:&str="init {flow.consume_v1(\"scope\",\"consumer\") {capture \"draw\"=random(1,9) capture \"sum\"=__rvn_captured_v1(\"draw\")+1 set boot=__rvn_captured_v1(\"sum\")}}\nscene label start\n\"Done\"\n";
#[test]
fn startup_step_readonly_observation_revokes_same_pc_changed_continuation_before_effects(){
    let mut s=pending(NATIVE);let before=live(&s);let position=s.snapshot().position;
    let source=s.observation_source().unwrap();assert!(s.accepts_observation_source(&source));assert_eq!(live(&s),before);
    let prepared=s.prepare_step(StepKind::Into).unwrap();assert!(prepared.render_batch().is_none());assert_eq!(live(&s),before);
    s.begin_prepared_step_commit(prepared).unwrap().commit();assert_eq!(s.snapshot().position,position);assert_eq!(serde_json::to_value(s.state()).unwrap(),before.0);
    assert!(!s.accepts_observation_source(&source));let current=s.observation_source().unwrap();assert!(s.accepts_observation_source(&current));
    let before=live(&s);for _ in 0..4{let _=s.video_views().unwrap();assert!(s.accepts_observation_source(&current));}assert_eq!(live(&s),before);
}
#[test]
fn startup_step_video_projection_between_real_steps_preserves_full_source_guard(){
    let mut s=pending("init {set first=random(1,9) set second=2}\nscene label start\n\"Done\"\n");
    for _ in 0..8{
        if s.is_complete(){break;}
        let before=live(&s);let cache=s.engine.debugger_anchor().unwrap();
        assert!(s.video_views().unwrap().is_empty());
        assert_eq!(live(&s),before);assert_eq!(s.engine.debugger_anchor().unwrap(),cache);
        let prepared=s.prepare_step(StepKind::Continue).unwrap();
        assert!(s.video_views().unwrap().is_empty());
        s.begin_prepared_step_commit(prepared).unwrap().commit();
    }
    assert!(s.is_complete());assert_eq!(s.state().vars["second"],Value::Int(2));
    let s=pending(NATIVE);let prepared=s.prepare_step(StepKind::Into).unwrap();
    s.engine.video_views().unwrap();
    assert!(s.validate_prepared_step(&prepared).is_err(),"Real source cache mutation remains authenticated");
}
#[test]
fn startup_step_prepare_drop_preserves_real_state_trace_cursor_and_random(){
    let s=pending(NATIVE);let before=live(&s);
    let a=s.prepare_step(StepKind::Into).unwrap();let b=s.prepare_step(StepKind::Into).unwrap();
    assert_eq!(a.snapshot(),b.snapshot());assert_eq!(live(&s),before);drop(a);drop(b);assert_eq!(live(&s),before);
}
#[test]
fn startup_step_changed_configuration_and_source_reject_before_live_trace_mutation(){
    let mut s=pending(NATIVE);let prepared=s.prepare_step(StepKind::Into).unwrap();
    s.configure(vec![],vec![wire::Watch{scope:wire::WatchScope::Globals,name:"boot".into()}]).unwrap();let before=live(&s);
    assert!(s.begin_prepared_step_commit(prepared).is_err());assert_eq!(live(&s),before);
    let prepared=s.prepare_step(StepKind::Into).unwrap();s.engine.state.vars.insert("changed".into(),Value::Int(3));let before=live(&s);
    assert!(s.begin_prepared_step_commit(prepared).is_err());assert_eq!(live(&s),before);
}
#[test]
fn startup_step_committed_plan_revokes_other_plan_and_retains_one_rng_draw(){
    let mut s=pending(NATIVE);s.engine.state.random=crate::random::RandomState::seeded(42);s.prepare_session().unwrap();let initial=s.state().random;
    let a=s.prepare_step(StepKind::Into).unwrap();let b=s.prepare_step(StepKind::Into).unwrap();s.begin_prepared_step_commit(a).unwrap().commit();
    let before=live(&s);assert!(s.begin_prepared_step_commit(b).is_err());assert_eq!(live(&s),before);
    let mut expected=initial;let mut library=FunctionLibrary::from_script(&rvn_parser::parse("set expected=random(1,9)+1").unwrap()).unwrap();
    let vars=library.execute_with_random(&rvn_parser::parse("set expected=random(1,9)+1").unwrap(),&HashMap::new(),&mut expected).unwrap();
    for _ in 0..100{if s.is_complete(){break;}let plan=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(plan).unwrap().commit();}
    assert!(s.is_complete());assert_eq!(s.state().random,expected);assert_eq!(s.state().vars["boot"],vars["expected"]);
}
struct RejectRenderer{reject:Arc<AtomicBool>,published:Arc<AtomicUsize>}
impl Renderer for RejectRenderer{
    fn supports_programmable_ui(&self)->bool{true}
    fn supports_video(&self)->bool{true}
    fn validate_scene_batch(&self,_:&crate::renderer::SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::Relaxed){Err("actual test renderer refuses".into())}else{Ok(())}}
    fn publish_scene_batch(&mut self,_:crate::renderer::SceneRenderBatch){self.published.fetch_add(1,Ordering::Relaxed);}
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
#[test]
fn startup_step_owned_video_rows_survive_logical_if_cold_restore_and_unique_handoff(){
    let source="init {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0})) if true {set selected=1} set last=2}\nscene label start\n\"Done\"\n";
    let make=||Engine::prepare_debugger_startup(program(source),RejectRenderer{reject:Arc::new(AtomicBool::new(false)),published:Arc::new(AtomicUsize::new(0))},8,HashMap::new(),TextRuntimeContext::default(),None).unwrap();
    let mut s=make();let plan=s.prepare_step(StepKind::Continue).unwrap();
    let rows=plan.render_batch().unwrap().videos.clone();assert_eq!(rows.len(),1);assert!(rows[0].resource_token.is_some());
    s.begin_prepared_step_commit(plan).unwrap().commit();assert_eq!(s.video_views().unwrap(),rows);
    let before=live(&s);let source_anchor=s.engine.debugger_anchor().unwrap();
    for _ in 0..4{assert_eq!(s.video_views().unwrap(),rows);assert_eq!(live(&s),before);assert_eq!(s.engine.debugger_anchor().unwrap(),source_anchor);}
    let save=s.capture_save15(1,"real video before If".into()).unwrap();
    let mut cold=make();cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap();
    let cold_rows=cold.video_views().unwrap();assert_eq!(cold_rows.len(),1);assert_eq!(cold_rows[0].id,rows[0].id);assert_eq!(cold_rows[0].track,rows[0].track);
    assert_eq!(cold_rows,cold.engine.video_views().unwrap());
    // Read the actual derived cache only through the public projection from
    // here on. A private Engine projection mutation above revokes the session.
    let mut cold=make();cold.restore_save15(save).unwrap();
    let before=live(&cold);let plan=cold.prepare_step(StepKind::Into).unwrap();assert!(plan.render_batch().is_none());
    assert_eq!(cold.video_views().unwrap(),cold_rows);assert_eq!(live(&cold),before);
    cold.begin_prepared_step_commit(plan).unwrap().commit();assert_eq!(cold.video_views().unwrap(),cold_rows);
    for _ in 0..20{if cold.is_complete(){break;}let plan=cold.prepare_step(StepKind::Continue).unwrap();cold.begin_prepared_step_commit(plan).unwrap().commit();}
    assert!(cold.is_complete());let views=cold.video_views().unwrap();let engine=cold.into_engine().unwrap();assert_eq!(engine.video_views().unwrap(),views);assert_eq!(engine.state.vars["last"],Value::Int(2));
}
impl StartupBatchSink for RejectRenderer{
    type PreparedPublication=StartupPublicationSource;
    type Publication=(StartupPublicationSource,crate::renderer::SceneRenderBatch);
    fn prepare_startup_publication(&self,source:&StartupPublicationSource,batch:&crate::renderer::SceneRenderBatch)->Result<Self::PreparedPublication,String>{self.validate_scene_batch(batch)?;Ok(source.clone())}
    fn publish_startup_publication(&mut self,source:StartupPublicationSource,batch:crate::renderer::SceneRenderBatch,prepared:Self::PreparedPublication)->Self::Publication{assert_eq!(source,prepared);self.published.fetch_add(1,Ordering::Relaxed);(source,batch)}
}
fn video_startup(source:&str)->EngineStartup<RejectRenderer>{
    Engine::prepare_debugger_startup(program(source),RejectRenderer{reject:Arc::new(AtomicBool::new(false)),published:Arc::new(AtomicUsize::new(0))},8,HashMap::new(),TextRuntimeContext::default(),None).unwrap()
}
#[test]
fn startup_step_video_actions_are_real_accepted_candidates_and_seek_zero_stays_explicit(){
    use StartupVideoCommandKind as K;
    let source="init {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0})) video.pause(\"intro\") video.resume(\"intro\") video.seek(\"intro\",0) video.volume(\"intro\",0.3) video.stop(\"intro\")}\nscene label start\n\"Done\"\n";
    let mut s=video_startup(source);
    for kind in [K::Play,K::Pause,K::Resume,K::Seek,K::Volume,K::Stop]{
        assert!(s.engine.startup_video_trace.is_none());let before=live(&s);
        let discarded=s.prepare_step(StepKind::Continue).unwrap();assert_eq!(discarded.video_commands().len(),1);drop(discarded);assert_eq!(live(&s),before);
        let plan=s.prepare_step(StepKind::Continue).unwrap();assert_eq!(plan.video_commands().len(),1);assert_eq!(plan.video_commands()[0].kind(),kind);assert_eq!(plan.video_commands()[0].name(),"intro");
        assert_eq!(live(&s),before);s.begin_prepared_step_commit(plan).unwrap().commit();assert!(s.engine.startup_video_trace.is_none());
    }
    assert!(s.is_complete());assert!(s.state().videos.tracks.is_empty());
}
#[test]
fn startup_step_video_actions_include_only_selected_accepted_callback_children(){
    use StartupVideoCommandKind as K;
    let source="handler finished(event){if false {video.stop(\"intro\")} video.seek(\"intro\",0) video.pause(\"intro\")}\ninit {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0,\"skippable\":true,\"on_end\":\"finished\"})) video.skip(\"intro\")}\nscene label start\n\"Done\"\n";
    let mut s=video_startup(source);let play=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(play).unwrap().commit();let before=live(&s);
    let plan=s.prepare_step(StepKind::Continue).unwrap();assert_eq!(live(&s),before);
    assert_eq!(plan.video_commands().iter().map(|action|action.kind()).collect::<Vec<_>>(),[K::Skip,K::Seek,K::Pause]);assert!(plan.video_commands().iter().all(|action|action.name()=="intro"));
    s.begin_prepared_step_commit(plan).unwrap().commit();assert_eq!(s.state().videos.tracks["intro"].playback,crate::video::Playback::Paused);
    let normal=Engine::from_program(program(source),RejectRenderer{reject:Arc::new(AtomicBool::new(false)),published:Arc::new(AtomicUsize::new(0))},8).unwrap();assert!(normal.startup_video_trace.is_none());assert_eq!(normal.state.videos,s.state().videos);
}
#[test]
fn startup_step_video_action_budget_refuses_without_truncation_or_live_callback_effect(){
    let mut trace=video_commands::StartupVideoTrace::default();
    for _ in 0..128{trace.record(&crate::eval::UiCommand::VideoStop{name:"absent".into()}).unwrap();}
    assert!(trace.record(&crate::eval::UiCommand::VideoStop{name:"absent".into()}).is_err());assert_eq!(trace.snapshot().unwrap().len(),128);
    let name="a".repeat(1024);let commands=format!("video.stop(\"{name}\") ").repeat(80);
    let source=format!("handler finished(event){{set rejected=random(1,9) {commands}}}\ninit {{video.play(\"intro\",video_clip(\"clip.webm\",{{\"volume\":0,\"skippable\":true,\"on_end\":\"finished\"}})) video.skip(\"intro\")}}\nscene label start\n\"Done\"\n");
    let mut s=video_startup(&source);let plan=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(plan).unwrap().commit();let before=live(&s);
    let error=s.prepare_step(StepKind::Continue).err().expect("Actual accepted action bytes must exceed the explicit budget");assert!(error.to_string().contains("64 KiB"),"{error}");assert_eq!(live(&s),before);assert!(!s.state().vars.contains_key("rejected"));assert!(s.engine.startup_video_trace.is_none());
    let normal=Engine::from_program(program(&source),RejectRenderer{reject:Arc::new(AtomicBool::new(false)),published:Arc::new(AtomicUsize::new(0))},8).unwrap();assert!(normal.startup_video_trace.is_none());
}
#[test]
fn startup_step_bounded_if_transitions_publish_no_renderer_and_refusal_keeps_progress(){
    let source=format!("init {{{}set leaf=1{}}}\nscene label start\n\"Done\"\n","if random(1,9)>0 {".repeat(40),"}".repeat(40));
    let mut s=video_startup(&source);s.engine.state.random=crate::random::RandomState::seeded(42);s.prepare_session().unwrap();let initial=s.state().random;
    let reject=s.engine.renderer.reject.clone();let published=s.engine.renderer.published.clone();reject.store(true,Ordering::Relaxed);
    let before=live(&s);let plan=s.prepare_step(StepKind::Continue).unwrap();assert_eq!(plan.progress(),DebugProgress::Running);assert!(plan.render_batch().is_none());assert!(plan.video_commands().is_empty());assert_eq!(live(&s),before);
    s.begin_prepared_step_commit(plan).unwrap().commit();assert_eq!(published.load(Ordering::Relaxed),0);assert!(!s.state().vars.contains_key("leaf"));
    let script=rvn_parser::parse(&"set expected=random(1,9) ".repeat(32)).unwrap();let mut expected=initial;FunctionLibrary::from_script(&script).unwrap().execute_with_random(&script,&HashMap::new(),&mut expected).unwrap();assert_eq!(s.state().random,expected);
    let before=live(&s);assert!(s.prepare_step(StepKind::Continue).is_err());assert_eq!(live(&s),before);assert_eq!(published.load(Ordering::Relaxed),0);
    reject.store(false,Ordering::Relaxed);let plan=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(plan).unwrap().commit();assert!(s.is_complete());assert_eq!(published.load(Ordering::Relaxed),1);
    let script=rvn_parser::parse(&"set expected=random(1,9) ".repeat(40)).unwrap();let mut expected=initial;FunctionLibrary::from_script(&script).unwrap().execute_with_random(&script,&HashMap::new(),&mut expected).unwrap();assert_eq!(s.state().random,expected);assert_eq!(s.state().vars["leaf"],Value::Int(1));
}
#[test]
fn startup_step_renderer_and_pcm_refusals_leave_live_continuation_unchanged(){
    let reject=Arc::new(AtomicBool::new(false));let published=Arc::new(AtomicUsize::new(0));
    let mut s=Engine::prepare_debugger_startup(program(NATIVE),RejectRenderer{reject:reject.clone(),published:published.clone()},8,HashMap::new(),TextRuntimeContext::default(),None).unwrap();
    let prepared=s.prepare_step(StepKind::Continue).unwrap();assert!(prepared.render_batch().is_some());let before=live(&s);
    reject.store(true,Ordering::Relaxed);assert!(s.begin_prepared_step_commit(prepared).is_err());assert_eq!(live(&s),before);assert_eq!(published.load(Ordering::Relaxed),0);
    assert!(s.prepare_step(StepKind::Continue).is_err());assert_eq!(live(&s),before);reject.store(false,Ordering::Relaxed);
    let prepared=s.prepare_step(StepKind::Continue).unwrap();let batch=prepared.render_batch().unwrap();
    let invalid=crate::audio_checkpoint::AudioCheckpointSnapshot{instance_id:s.snapshot().instance+1,runtime_epoch:batch.runtime_epoch,rows:vec![],music:vec![]};
    assert!(s.begin_prepared_step_commit(prepared).unwrap().commit_with_audio_checkpoint(invalid).is_err());assert_eq!(live(&s),before);assert_eq!(published.load(Ordering::Relaxed),0);
    let prepared=s.prepare_step(StepKind::Continue).unwrap();let batch=prepared.render_batch().unwrap();
    let valid=crate::audio_checkpoint::AudioCheckpointSnapshot{instance_id:s.snapshot().instance,runtime_epoch:batch.runtime_epoch,rows:vec![],music:vec![]};
    s.begin_prepared_step_commit(prepared).unwrap().commit_with_audio_checkpoint(valid).unwrap();assert!(s.is_complete());assert_eq!(published.load(Ordering::Relaxed),1);
}
#[test]
fn startup_step_live_does_not_require_portable_locale_policy(){
    let root=std::path::PathBuf::from(std::env::var("RVN_STARTUP_STEP_FIXTURES").unwrap()).join(format!("external-{}",std::process::id()));std::fs::create_dir(&root).unwrap();std::fs::write(root.join("en.toml"),"[strings]\nhello=\"Hello\"\n").unwrap();
    let locale=LocaleManager::new(&root,"en","en",vec!["en".into()]).unwrap();assert!(locale.portable_receipt().is_err());
    let mut s=Engine::prepare_debugger_startup(program("init {set boot=7}\nscene label start\n\"Ready\"\n"),crate::TerminalRenderer,8,HashMap::new(),TextRuntimeContext::new(Default::default(),Some(locale)),None).unwrap();
    s.step(StepKind::Continue).unwrap();assert_eq!(s.state().vars["boot"],Value::Int(7));
}
#[test]
fn startup_step_execution_budget_failure_keeps_real_live_consumer_continuation(){
    let source="function costly(){local i=0 while i<1000000 {local i=i+1} return i}\ninit {flow.consume_v1(\"scope\",\"budget\"){capture \"large\"=costly() set boot=__rvn_captured_v1(\"large\")}}\nscene label start\n\"Ready\"\n";
    let mut s=pending(source);let initial=s.state().random;
    for _ in 0..1000 {
        let before=live(&s);
        match s.prepare_step(StepKind::Continue){
            Ok(plan)=>{assert_eq!(live(&s),before);s.begin_prepared_step_commit(plan).unwrap().commit();}
            Err(error)=>{assert!(error.to_string().contains("100000")||error.to_string().contains("budget")||error.to_string().contains("limit"),"{error}");assert_eq!(live(&s),before);assert_eq!(s.state().random,initial);assert!(!s.state().vars.contains_key("boot"));return;}
        }
    }
    panic!("actual consumer budget did not stop its retained evaluator");
}
#[test]
fn startup_step_real_locale_revision_change_revokes_plan_without_trace_mutation(){
    let mut s=pending(NATIVE);
    let catalogs=["en","fr"].into_iter().map(|lang|(lang.into(),crate::locale::LocaleCatalog::from(crate::locale::LocaleTable{lang:lang.into(),strings:HashMap::from([("hello".into(),lang.into())])}))).collect();
    let locale=LocaleManager::from_frozen_catalogs("en","en",vec!["en".into(),"fr".into()],catalogs).unwrap();
    s.engine.locale=Some(locale);s.prepare_session().unwrap();let plan=s.prepare_step(StepKind::Into).unwrap();
    s.engine.locale.as_mut().unwrap().set_language("fr").unwrap();let before=live(&s);
    assert!(s.begin_prepared_step_commit(plan).is_err());assert_eq!(live(&s),before);
}
#[test]
fn startup_step_host_guard_prepares_before_mutation_and_transfers_owned_batch_once(){
    let reject=Arc::new(AtomicBool::new(false));let published=Arc::new(AtomicUsize::new(0));
    let mut s=Engine::prepare_debugger_startup(program(NATIVE),RejectRenderer{reject,published:published.clone()},8,HashMap::new(),TextRuntimeContext::default(),None).unwrap();
    let before=live(&s);let prepared=s.prepare_step(StepKind::Continue).unwrap();assert_eq!(live(&s),before);
    {let guard=s.begin_prepared_host_step_commit(prepared).unwrap();assert!(guard.render_batch().is_some());drop(guard);}assert_eq!(live(&s),before);assert_eq!(published.load(Ordering::Relaxed),0);
    let prepared=s.prepare_step(StepKind::Continue).unwrap();let snapshot=crate::audio_checkpoint::AudioCheckpointSnapshot{instance_id:s.snapshot().instance,runtime_epoch:prepared.render_batch().unwrap().runtime_epoch,rows:vec![],music:vec![]};
    let committed=s.begin_prepared_host_step_commit(prepared).unwrap().commit_with_audio_checkpoint(snapshot).unwrap();
    assert_eq!(committed.progress,DebugProgress::Complete);let(source,batch)=committed.publication.unwrap();assert_eq!(source.instance_id(),s.snapshot().instance);assert_eq!(serde_json::to_value(&batch.state).unwrap(),serde_json::to_value(&s.engine.state).unwrap());assert_eq!(published.load(Ordering::Relaxed),1);
    assert!(s.is_complete());let engine=s.into_engine().unwrap();assert!(engine.state.vars.contains_key("boot"));
}
#[test]
fn startup_step_host_guard_requires_pcm_fence_for_publishing_command(){
    let published=Arc::new(AtomicUsize::new(0));let mut s=Engine::prepare_debugger_startup(program(NATIVE),RejectRenderer{reject:Arc::new(AtomicBool::new(false)),published:published.clone()},8,HashMap::new(),TextRuntimeContext::default(),None).unwrap();
    let before=live(&s);let prepared=s.prepare_step(StepKind::Continue).unwrap();assert!(s.begin_prepared_host_step_commit(prepared).unwrap().commit_computation().is_err());assert_eq!(live(&s),before);assert_eq!(published.load(Ordering::Relaxed),0);
    let prepared=s.prepare_step(StepKind::Into).unwrap();assert!(prepared.render_batch().is_none());let committed=s.begin_prepared_host_step_commit(prepared).unwrap().commit_computation().unwrap();assert!(committed.publication.is_none());assert_eq!(published.load(Ordering::Relaxed),0);
}

#[test]
fn startup_step_next_command_crosses_only_real_unvisited_statement_boundaries(){
    let source="init {set first=1 set second=2 set third=3}\nscene label start\n\"Done\"\n";
    let mut s=video_startup(source);
    for expected in [Some(StepKind::Continue),Some(StepKind::Continue),None]{
        let before=live(&s);let plan=s.prepare_step(StepKind::Continue).unwrap();
        assert_eq!(plan.next_step_kind(),expected);assert_eq!(live(&s),before);
        s.begin_prepared_step_commit(plan).unwrap().commit();
    }
    assert!(s.is_complete());assert_eq!(s.state().vars["third"],Value::Int(3));
}
#[test]
fn startup_step_next_command_stops_at_actual_statement_and_producer_breakpoints(){
    let mut s=video_startup("init {set first=1 set second=2 set third=3}\nscene label start\n\"Done\"\n");
    let bp=s.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.statement&&site.route==[wire::Route::Statement(1)])).unwrap().breakpoint;
    s.configure(vec![bp],vec![]).unwrap();
    let first=s.prepare_step(StepKind::Continue).unwrap();assert_eq!(first.next_step_kind(),Some(StepKind::Continue));s.begin_prepared_step_commit(first).unwrap().commit();
    let before=live(&s);let stopped=s.prepare_step(StepKind::Continue).unwrap();assert_eq!(stopped.progress(),DebugProgress::Stopped);assert_eq!(stopped.next_step_kind(),None);assert!(stopped.render_batch().is_none());assert_eq!(live(&s),before);
    s.begin_prepared_step_commit(stopped).unwrap().commit();assert_eq!(s.state().vars["first"],Value::Int(1));assert!(!s.state().vars.contains_key("second"));
    let resume=s.prepare_step(StepKind::Continue).unwrap();assert_eq!(resume.next_step_kind(),Some(StepKind::Continue));s.begin_prepared_step_commit(resume).unwrap().commit();assert_eq!(s.state().vars["second"],Value::Int(2));
    let mut s=video_startup(NATIVE);
    let bp=s.site_page(0).unwrap().entries.into_iter().find(|entry|entry.origin.as_ref().and_then(|origin|origin.scheduled_step.as_ref()).is_some_and(|producer|producer.producer_identity=="draw")).unwrap().breakpoint;
    s.configure(vec![bp],vec![]).unwrap();let before=live(&s);
    let plan=s.prepare_step(StepKind::Continue).unwrap();assert_eq!(plan.next_step_kind(),None);assert_eq!(plan.progress(),DebugProgress::Stopped);assert_eq!(live(&s),before);assert!(plan.render_batch().is_none());
    s.begin_prepared_step_commit(plan).unwrap().commit();assert!(!s.state().vars.contains_key("boot"));
    for kind in [StepKind::Into,StepKind::Over]{let s=video_startup(NATIVE);let plan=s.prepare_step(kind).unwrap();assert_eq!(plan.next_step_kind(),None);}
    assert!(video_startup(NATIVE).prepare_step(StepKind::Out).is_err());
}
#[test]
fn startup_video_wait_keeps_actual_initializer_parked_before_next_effect(){
    let source="init {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0})) video.wait(\"intro\") set after_wait=random(1,9)}\nscene label start\n\"Done\"\n";
    let mut s=video_startup(source);
    for _ in 0..2{let plan=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(plan).unwrap().commit();}
    assert_eq!(s.state().videos.waiting.as_deref(),Some("intro"));
    let before=live(&s);
    // Exercise the real commit path if the owner wrongly admits this step;
    // retaining a speculative candidate alone would not prove the live bug.
    if let Ok(plan)=s.prepare_step(StepKind::Continue){s.begin_prepared_step_commit(plan).unwrap().commit();}
    assert!(!s.state().vars.contains_key("after_wait"),"An executed actual VideoWait must park Init before the next live effect");
    assert_eq!(live(&s),before);assert!(!s.state().vars.contains_key("after_wait"));assert!(!s.is_complete());
}
#[test]
fn startup_video_wait_clock_is_real_pure_and_final_wait_cannot_transfer_owner(){
    let source="init {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0})) video.wait(\"intro\")}\nscene label start\n\"Done\"\n";
    let mut s=video_startup(source);let plan=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(plan).unwrap().commit();
    let view=s.video_views().unwrap().remove(0);assert!(s.video_clock_source(&view).unwrap().is_none());
    let plan=s.prepare_step(StepKind::Continue).unwrap();assert!(plan.waiting_for_video());assert_eq!(plan.next_step_kind(),Some(StepKind::Continue));s.begin_prepared_step_commit(plan).unwrap().commit();
    assert!(s.waiting_for_video());assert!(!s.is_complete());assert!(s.session.is_none());
    let before=live(&s);let observation=s.observation_source().unwrap();let source=s.video_clock_source(&view).unwrap().unwrap();
    for _ in 0..8{assert!(s.accepts_video_clock_source(&source));assert_eq!(s.video_views().unwrap(),vec![view.clone()]);}
    assert!(s.accepts_observation_source(&observation));assert_eq!(live(&s),before);
    let snapshot=s.snapshot();assert!(!snapshot.evaluating);assert_eq!(snapshot.site.unwrap().route,vec![wire::Route::Statement(1)]);
    let raw=&s.program.sources[0].source;assert_eq!(&raw[snapshot.origin.unwrap().span.unwrap()],"video.wait(\"intro\")");
    let mut forged=view.clone();forged.track.position=1.0;assert!(s.video_clock_source(&forged).unwrap().is_none());
    let other=video_startup("init {set other=1}\nscene label start\n\"Done\"\n");assert!(!other.accepts_video_clock_source(&source));
    s.configure(vec![],vec![wire::Watch{scope:wire::WatchScope::Globals,name:"missing".into()}]).unwrap();assert!(!s.accepts_observation_source(&observation));assert!(!s.accepts_video_clock_source(&source));
    assert!(s.into_engine().is_err());
}
#[test]
fn startup_video_wait_cold_phase_has_no_expression_trace_and_rebinds_real_source(){
    let source="init {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0})) set sampled=random(1,9) video.wait(\"intro\") set after_wait=random(1,9)}\nscene label start\n\"Done\"\n";
    let mut warm=video_startup(source);
    for _ in 0..3{let plan=warm.prepare_step(StepKind::Continue).unwrap();warm.begin_prepared_step_commit(plan).unwrap().commit();}
    let breakpoint=warm.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.statement&&site.route==[wire::Route::Statement(3)])).unwrap().breakpoint;
    warm.configure(vec![breakpoint],vec![wire::Watch{scope:wire::WatchScope::Globals,name:"sampled".into()}]).unwrap();
    let random=warm.state().random;let value=warm.state().vars["sampled"].clone();
    let save=warm.capture_save15(1,"actual parked Init".into()).unwrap();let debugger=save.debugger.as_ref().unwrap();
    assert!(debugger.trace.is_none());assert_eq!(debugger.startup.as_ref().unwrap().waiting_video_name(),Some("intro"));
    let encoded=save.encode().unwrap();assert!(!serde_json::from_str::<serde_json::Value>(&encoded).unwrap()["debugger"].as_object().unwrap().contains_key("trace"));
    let old_clock=warm.video_clock_source(&warm.video_views().unwrap()[0]).unwrap().unwrap();
    let mut cold=video_startup(source);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&encoded).unwrap()).unwrap();
    assert!(cold.waiting_for_video());assert!(cold.session.is_none());assert_eq!(cold.state().random,random);assert_eq!(cold.state().vars["sampled"],value);
    assert!(!cold.accepts_video_clock_source(&old_clock));let view=cold.video_views().unwrap().remove(0);assert!(cold.video_clock_source(&view).unwrap().is_some());
    assert_eq!(cold.snapshot().breakpoints.len(),1);assert_eq!(cold.watches.len(),1);assert!(!cold.state().vars.contains_key("after_wait"));
    assert!(cold.prepare_step(StepKind::Continue).is_err());
    let before=live(&cold);let mut forged=save.clone();forged.debugger.as_mut().unwrap().startup.as_mut().unwrap().body=0;
    assert!(cold.restore_save15(forged).is_err());assert_eq!(live(&cold),before);
    let mut forged=save.clone();forged.debugger.as_mut().unwrap().last_site.as_mut().unwrap().route=vec![wire::Route::Producer(0)];
    assert!(cold.restore_save15(forged).is_err());assert_eq!(live(&cold),before);
    let mut changed=video_startup(&(source.to_owned()+"\n \n"));let before=live(&changed);assert!(changed.restore_save15(save).is_err());assert_eq!(live(&changed),before);
}
#[test]
fn startup_video_wait_trace_absence_is_exclusive_and_forged_phase_fails_before_restore(){
    let ordinary=video_startup(NATIVE);let ordinary=ordinary.capture_save15(1,"real pending expression".into()).unwrap();
    let trace=ordinary.debugger.as_ref().unwrap().trace.clone().unwrap();let mut forged=ordinary.clone();forged.debugger.as_mut().unwrap().trace=None;
    assert!(forged.encode().is_err());
    let source="init {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0})) video.wait(\"intro\")}\nscene label start\n\"Done\"\n";
    let mut waiting=video_startup(source);for _ in 0..2{let plan=waiting.prepare_step(StepKind::Continue).unwrap();waiting.begin_prepared_step_commit(plan).unwrap().commit();}
    let save=waiting.capture_save15(1,"real wait".into()).unwrap();let before=live(&waiting);
    let mut forged=save.clone();forged.debugger.as_mut().unwrap().trace=Some(trace);assert!(forged.encode().is_err());
    let mut forged=save.clone();forged.debugger.as_mut().unwrap().startup.as_mut().unwrap().waiting=Some(PortableStartupWait::WaitingVideo{name:"absent".into()});
    assert!(waiting.restore_save15(forged).is_err());assert_eq!(live(&waiting),before);
    let mut forged=serde_json::to_value(&save).unwrap();forged["debugger"]["startup"]["waiting"]["kind"]=serde_json::Value::String("ArbitraryBoundary".into());
    assert!(crate::save::consumer_checkpoint::Save15::decode(&serde_json::to_string(&forged).unwrap()).is_err());
}

#[test]
fn startup_all_source_audio_missing_actual_host_reader_refuses_capture_without_live_effect(){
    let source="screen panel(){return component(\"root\",\"column\",{},[])}\ninit {set first=1 set after=2}\nscene label start\n\"Done\"\n";
    let mut s=video_startup(source);
    s.engine.synchronize_source_menu(rvn_ui::PageRole::Pause,Some("panel"),Value::Dict(Default::default()),true,500).unwrap();
    let screen=&s.engine.state.ui.screens[0];let owner=crate::scene_tasks::TaskOwner::Host{role:screen.host_role.unwrap().id().into()};let epoch=s.engine.runtime_epoch();
    crate::audio_playback::register(&mut s.engine.state,epoch,owner,crate::audio_playback::AudioPlayback::new(crate::audio_playback::AudioFamily::Sfx,"sfx/host.wav".into(),0.0).unwrap()).unwrap();
    s.prepare_session().unwrap();assert_eq!(crate::audio_playback::describe(s.state(),epoch).unwrap().len(),1);let before=live(&s);
    assert!(s.capture_save15(1,"must include actual Host".into()).is_err(),"Missing actual Host Source must refuse, never omit the reader");assert_eq!(live(&s),before);
    let missing=crate::audio_checkpoint::AudioCheckpointSnapshot{instance_id:s.snapshot().instance,runtime_epoch:epoch,rows:vec![],music:vec![]};
    assert!(s.capture_save15_with_fenced_media(2,"missing row".into(),missing.clone(),vec![]).is_err());assert_eq!(live(&s),before);
    let plan=s.prepare_step(StepKind::Continue).unwrap();assert!(s.begin_prepared_step_commit(plan).unwrap().commit_with_audio_checkpoint(missing).is_err());assert_eq!(live(&s),before);
}

#[path="../tests/private_pcm0370/startup_checkpoint_pcm.rs"]
mod actual_startup_pcm;
#[test]
fn startup_all_source_audio_actual_wav_fence_keeps_host_cursor_and_source_atomic(){
    use actual_startup_pcm::{PcmClip,CheckpointGate};
    let source="screen panel(){return component(\"root\",\"column\",{},[])}\ninit {set first=1 set after=2}\nscene label start\n\"Done\"\n";
    let mut s=video_startup(source);s.engine.synchronize_source_menu(rvn_ui::PageRole::Pause,Some("panel"),Value::Dict(Default::default()),true,500).unwrap();
    let screen=&s.engine.state.ui.screens[0];let owner=crate::scene_tasks::TaskOwner::Host{role:screen.host_role.unwrap().id().into()};let epoch=s.engine.runtime_epoch();
    crate::audio_playback::register(&mut s.engine.state,epoch,owner,crate::audio_playback::AudioPlayback::new(crate::audio_playback::AudioFamily::Sfx,"sfx/host.wav".into(),0.0).unwrap()).unwrap();s.prepare_session().unwrap();
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().join("fixtures/pcm/host.wav");let bytes=std::fs::read(path).unwrap();let clip=PcmClip::decode("sfx/host.wav",&bytes).unwrap();let gate=CheckpointGate::default();let reader=clip.start(gate.clone(),false);let mut actual=reader.decoder();for _ in 0..3{assert!(actual.next().is_some());}
    let checkpoint=gate.snapshot(&[&reader],|mut rows|Ok(rows.remove(0))).unwrap();assert_eq!(checkpoint.next_sample,3);assert_eq!(checkpoint.next_channel,1);
    let view=crate::audio_playback::describe(s.state(),epoch).unwrap().remove(0);let snapshot=crate::audio_checkpoint::AudioCheckpointSnapshot{instance_id:s.snapshot().instance,runtime_epoch:epoch,rows:vec![crate::audio_checkpoint::AudioCheckpointRow{token:view.token,checkpoint:checkpoint.clone()}],music:vec![]};let before=live(&s);
    let saved=s.capture_save15_with_fenced_media(1,"real Init Host Source".into(),snapshot.clone(),vec![]).unwrap();assert_eq!(live(&s),before);assert!(saved.encode().unwrap().contains("\"next_sample\":3"));
    for mutate in 0..4{let mut rejected=snapshot.clone();match mutate{0=>rejected.rows.clear(),1=>rejected.rows.push(rejected.rows[0].clone()),2=>rejected.runtime_epoch+=1,_=>rejected.rows[0].token.task.revision+=1};assert!(s.capture_save15_with_fenced_media(1,"invalid row".into(),rejected.clone(),vec![]).is_err());let prepared=s.prepare_step(StepKind::Continue).unwrap();assert!(s.begin_prepared_step_commit(prepared).unwrap().commit_with_audio_checkpoint(rejected).is_err());assert_eq!(live(&s),before);}
    let plan=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(plan).unwrap().commit_with_audio_checkpoint(snapshot.clone()).unwrap();assert_eq!(s.state().vars["first"],Value::Int(1));assert_eq!(crate::audio_playback::describe(s.state(),epoch).unwrap()[0].playback.checkpoint,Some(checkpoint.clone()));
    // Complete the actual Init while retaining its real Host. The owner has not
    // transferred, so all-Source capture still cannot fall back to Game-only.
    let plan=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(plan).unwrap().commit_with_audio_checkpoint(snapshot.clone()).unwrap();assert!(s.is_complete());assert!(s.session.is_none());assert!(s.capture_save15(2,"missing actual reader".into()).is_err());assert!(s.capture_save15_with_fenced_media(2,"complete owner cannot lose Startup authority".into(),snapshot,vec![]).is_err());
    assert_eq!(gate.snapshot(&[&reader],|mut rows|Ok(rows.remove(0))).unwrap(),checkpoint);
}

#[test]
fn startup_completed_owner_cannot_export_game_shaped_save_before_unique_handoff(){
    let mut s=video_startup("init {set once=random(1,9)}\nscene label start\n\"Done\"\n");let program=s.program().clone();let plan=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(plan).unwrap().commit();assert!(s.is_complete());let before=live(&s);let instance=s.snapshot().instance;
    assert!(s.capture_save15(1,"completed still owned".into()).is_err());let rows=crate::audio_checkpoint::AudioCheckpointSnapshot{instance_id:instance,runtime_epoch:s.snapshot().epoch,rows:vec![],music:vec![]};assert!(s.capture_save15_with_fenced_media(1,"completed fenced still owned".into(),rows,vec![]).is_err());assert_eq!(live(&s),before);
    let engine=s.into_engine().unwrap();assert_eq!(engine.instance_id(),instance);assert_eq!(serde_json::to_value(&engine.state).unwrap(),before.0);let save=engine.capture_save15(&program,None,1,"actual Game after transfer".into()).unwrap();assert!(save.debugger.is_none());
}

#[test]
fn startup_accepted_video_epochs_match_actual_owner_after_plain_and_fenced_publication(){
    for label in ["scene label","label"]{for fenced in [false,true]{
        let source=format!("init {{video.play(\"intro\",video_clip(\"clip.webm\",{{\"volume\":0}})) video.wait(\"intro\")}}\n{label} start\n\"Done\"\n");let mut owner=video_startup(&source);
        for _ in 0..2{let before=live(&owner);let prepared=owner.prepare_step(StepKind::Continue).unwrap();let expected=prepared.render_batch().unwrap().videos[0].clone();assert_eq!(live(&owner),before);
            if fenced{let audio=crate::audio_checkpoint::AudioCheckpointSnapshot{instance_id:owner.snapshot().instance,runtime_epoch:owner.snapshot().epoch,rows:vec![],music:vec![]};owner.begin_prepared_host_step_commit(prepared).unwrap().commit_with_audio_checkpoint(audio).unwrap();}else{owner.begin_prepared_step_commit(prepared).unwrap().commit();}
            assert_eq!(owner.video_views().unwrap(),vec![expected.clone()]);assert_eq!(owner.engine.video_epoch,expected.epoch);assert!(owner.engine.video_view_current(&expected),"{label}, fenced={fenced}");
        }
        let view=owner.video_views().unwrap()[0].clone();assert!(owner.video_clock_source(&view).unwrap().is_some());let source=owner.observation_source().unwrap();for _ in 0..4{assert_eq!(owner.video_views().unwrap()[0],view);}assert!(owner.accepts_observation_source(&source));
    }}
}
#[test]
fn startup_video_position_receipt_uses_actual_member_no_batch_epoch_seek_or_rng(){
    for label in ["scene label","label"]{
        let text=format!("init {{video.play(\"intro\",video_clip(\"clip.webm\",{{\"volume\":0}})) video.wait(\"intro\") set after_wait=random(1,9)}}\n{label} start\n\"Done\"\n");let mut s=video_startup(&text);for _ in 0..2{let p=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(p).unwrap().commit();}
        let before=live(&s);let random=s.state().random;let published=s.engine.renderer.published.load(Ordering::Relaxed);let view=s.video_views().unwrap()[0].clone();let old_clock=s.video_clock_source(&view).unwrap().unwrap_or_else(||panic!("actual {label} clock missing: waiting={}, session={}, current={}, view={view:?}",s.waiting_for_video(),s.session.is_some(),s.engine.video_view_current(&view)));
        let prepared=s.prepare_video_feedback(&[StartupVideoReport{view:view.clone(),feedback:GameVideoFeedback::Report(crate::video::Feedback::Position{seconds:0.5,duration:2.0})}]).unwrap();assert_eq!(live(&s),before);assert!(s.accepts_video_clock_source(&old_clock));
        let committed=s.begin_prepared_video_feedback_commit(prepared).unwrap().commit();assert_eq!(committed.receipts(),&[VideoReportReceipt{ordinal:0,disposition:VideoReportDisposition::Consumed}]);let receipt=&committed.accepted_reports()[0];assert_eq!(receipt.ordinal(),0);assert_eq!(receipt.kind(),StartupVideoReportKind::Position);assert_eq!(receipt.prior_view(),&view);assert!(s.accepts_video_report_receipt(receipt));
        let after=s.video_views().unwrap()[0].clone();assert_eq!(receipt.accepted_view(),&after);assert_eq!(after.track.position,0.5);assert_eq!(after.epoch,view.epoch);assert_eq!(after.resource_token,view.resource_token);assert_eq!(after.ui_projection,view.ui_projection);assert_eq!(s.state().random,random);assert_eq!(s.engine.renderer.published.load(Ordering::Relaxed),published);assert!(!s.accepts_video_clock_source(&old_clock));assert!(s.video_clock_source(&after).unwrap().is_some());assert!(s.waiting_for_video());assert!(!s.state().vars.contains_key("after_wait"));
        let observed=s.observation_source().unwrap();for _ in 0..8{assert!(s.accepts_video_report_receipt(receipt));assert_eq!(s.video_views().unwrap()[0],after);}assert!(s.accepts_observation_source(&observed));
        let other=video_startup(&text);assert!(!other.accepts_video_report_receipt(receipt));
        let save=s.capture_save15(1,"real position inside Init wait".into()).unwrap();let mut cold=video_startup(&text);cold.restore_save15(save).unwrap();assert_eq!(cold.video_views().unwrap()[0].track.position,0.5);assert!(!cold.accepts_video_report_receipt(receipt));assert_eq!(cold.state().random,s.state().random);
    }
}
#[test]
fn startup_video_position_rejects_source_config_budget_and_invalid_payload_atomically(){
    let mut s=video_startup("init {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0})) video.wait(\"intro\") set after_wait=1}\nscene label start\n\"Done\"\n");for _ in 0..2{let p=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(p).unwrap().commit();}let view=s.video_views().unwrap()[0].clone();let before=live(&s);
    let position=|seconds|StartupVideoReport{view:view.clone(),feedback:GameVideoFeedback::Report(crate::video::Feedback::Position{seconds,duration:2.0})};
    assert!(s.prepare_video_feedback(&[position(0.5),position(f64::NAN)]).is_err());assert_eq!(live(&s),before);
    let oversized=(0..129).map(|_|position(0.1)).collect::<Vec<_>>();assert!(s.prepare_video_feedback(&oversized).is_err());assert_eq!(live(&s),before);
    let large_source=format!("init {{video.play(\"intro\",video_clip(\"{}.webm\",{{\"volume\":0}})) video.wait(\"intro\")}}\nscene label start\n\"Done\"\n","a".repeat(1000));let mut bounded=video_startup(&large_source);for _ in 0..2{let p=bounded.prepare_step(StepKind::Continue).unwrap();bounded.begin_prepared_step_commit(p).unwrap().commit();}let member=bounded.video_views().unwrap()[0].clone();let before_bytes=live(&bounded);let reports=(0..128).map(|_|StartupVideoReport{view:member.clone(),feedback:GameVideoFeedback::Report(crate::video::Feedback::Position{seconds:0.1,duration:2.0})}).collect::<Vec<_>>();let error=bounded.prepare_video_feedback(&reports).err().expect("Actual admitted metadata must exceed byte budget before candidate cloning");assert!(error.to_string().contains("64 KiB"),"{error}");assert_eq!(live(&bounded),before_bytes);
    let prepared=s.prepare_video_feedback(&[position(0.2)]).unwrap();s.configure(vec![],vec![wire::Watch{scope:wire::WatchScope::Globals,name:"after_wait".into()}]).unwrap();let configured=live(&s);assert!(s.begin_prepared_video_feedback_commit(prepared).is_err());assert_eq!(live(&s),configured);
    let mut stale=view.clone();stale.epoch+=1;let reports=[StartupVideoReport{view:stale,feedback:GameVideoFeedback::Report(crate::video::Feedback::Error("\0".repeat(100_000)))},StartupVideoReport{view:view.clone(),feedback:GameVideoFeedback::Report(crate::video::Feedback::End)},position(0.2),position(0.3)];let prepared=s.prepare_video_feedback(&reports).unwrap();assert_eq!(live(&s),configured);let committed=s.begin_prepared_video_feedback_commit(prepared).unwrap().commit();assert_eq!(committed.receipts().iter().map(|r|r.disposition).collect::<Vec<_>>(),vec![VideoReportDisposition::RejectedStale,VideoReportDisposition::Deferred,VideoReportDisposition::Consumed,VideoReportDisposition::Consumed]);assert_eq!(committed.accepted_reports().len(),2);assert!(committed.accepted_reports().iter().all(|receipt|s.accepts_video_report_receipt(receipt)&&receipt.accepted_view().track.position==0.3));assert!(s.waiting_for_video());assert!(!s.state().vars.contains_key("after_wait"));
}

#[test]
fn startup_terminal_end_uses_actual_callback_breakpoint_and_rng_once_before_next_init(){
    for label in ["scene label","label"]{
        let text=format!("handler finished(event){{set callback_draw=random(1,9) set callbacks=callbacks+1}}\ninit {{set callbacks=0 video.play(\"intro\",video_clip(\"clip.webm\",{{\"volume\":0,\"on_end\":\"finished\"}})) video.wait(\"intro\") set after_wait=random(1,9)}}\n{label} start\n\"Done\"\n");let mut owner=video_startup(&text);for _ in 0..3{let p=owner.prepare_step(StepKind::Continue).unwrap();owner.begin_prepared_step_commit(p).unwrap().commit();}assert!(owner.waiting_for_video());
        let breakpoint=owner.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.function.as_deref()==Some("handler:finished")&&site.statement)).expect("Actual handler source site").breakpoint;owner.configure(vec![breakpoint],vec![]).unwrap();let before=live(&owner);let before_random=owner.state().random;let mut expected_random=before_random;let expected=expected_random.integer(1,9).unwrap();let view=owner.video_views().unwrap()[0].clone();let clock=owner.video_clock_source(&view).unwrap().unwrap();
        let prepared=owner.prepare_video_continuation(&StartupVideoReport{view:view.clone(),feedback:GameVideoFeedback::Report(crate::video::Feedback::End)}).unwrap();assert_eq!(live(&owner),before);owner.begin_prepared_video_continuation(prepared).unwrap();assert!(!owner.waiting_for_video());assert!(!owner.accepts_video_clock_source(&clock));assert_eq!(owner.state().random,before_random);let admitted=owner.capture_save15(1,"actual pending callback".into()).unwrap();assert!(serde_json::to_value(admitted).unwrap()["debugger"]["startup"]["callback"].is_object());
        let plan=owner.prepare_step(StepKind::Continue).unwrap();assert!(plan.render_batch().is_none());assert_eq!(plan.progress(),DebugProgress::Stopped);owner.begin_prepared_step_commit(plan).unwrap().commit();assert_eq!(owner.snapshot().site.unwrap().function.as_deref(),Some("handler:finished"));assert!(!owner.state().vars.contains_key("callback_draw"));
        let plan=owner.prepare_step(StepKind::Continue).unwrap();assert!(plan.render_batch().is_some());owner.begin_prepared_step_commit(plan).unwrap().commit();assert_eq!(owner.state().vars["callback_draw"],Value::Int(expected));assert_eq!(owner.state().vars["callbacks"],Value::Int(1));assert_eq!(owner.state().random,expected_random);assert!(!owner.state().vars.contains_key("after_wait"));assert!(!owner.waiting_for_video());assert!(!owner.is_complete());assert!(owner.state().videos.waiting.is_none());assert!(owner.state().videos.tracks["intro"].finished());
        let plan=owner.prepare_step(StepKind::Continue).unwrap();owner.begin_prepared_step_commit(plan).unwrap().commit();assert!(owner.is_complete());assert_eq!(owner.state().vars["callbacks"],Value::Int(1));
    }
}

#[test]
fn startup_terminal_admission_refuses_stale_payload_budget_and_configuration_without_effects(){
    let mut owner=video_startup("handler failed(event){set bad=1}\ninit {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0,\"on_error\":\"failed\"})) video.wait(\"intro\") set next=1}\nscene label start\n\"Done\"\n");for _ in 0..2{let p=owner.prepare_step(StepKind::Continue).unwrap();owner.begin_prepared_step_commit(p).unwrap().commit();}let view=owner.video_views().unwrap()[0].clone();let before=live(&owner);let mut stale=view.clone();stale.epoch+=1;
    assert!(owner.prepare_video_continuation(&StartupVideoReport{view:stale,feedback:GameVideoFeedback::Report(crate::video::Feedback::Error("\0".repeat(100_000)))}).is_err());assert_eq!(live(&owner),before);
    assert!(owner.prepare_video_continuation(&StartupVideoReport{view:view.clone(),feedback:GameVideoFeedback::Report(crate::video::Feedback::Error("x".repeat(70_000)))}).is_err());assert_eq!(live(&owner),before);
    let prepared=owner.prepare_video_continuation(&StartupVideoReport{view,feedback:GameVideoFeedback::Report(crate::video::Feedback::Error("actual decoder failure".into()))}).unwrap();owner.configure(vec![],vec![wire::Watch{scope:wire::WatchScope::Globals,name:"bad".into()}]).unwrap();let before=live(&owner);assert!(owner.begin_prepared_video_continuation(prepared).is_err());assert_eq!(live(&owner),before);assert!(!owner.state().vars.contains_key("bad"));
}

#[test]
fn startup_terminal_error_callback_and_renderer_refusal_keep_actual_continuation_atomic(){
    let mut owner=video_startup("handler failed(event){set failure_draw=random(1,9) set error_message=event[\"message\"]}\ninit {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0,\"on_error\":\"failed\"})) video.wait(\"intro\") set after_wait=1}\nscene label start\n\"Done\"\n");for _ in 0..2{let p=owner.prepare_step(StepKind::Continue).unwrap();owner.begin_prepared_step_commit(p).unwrap().commit();}let view=owner.video_views().unwrap()[0].clone();let mut expected_random=owner.state().random;let expected=expected_random.integer(1,9).unwrap();let prepared=owner.prepare_video_continuation(&StartupVideoReport{view,feedback:GameVideoFeedback::Report(crate::video::Feedback::Error("actual decoder failure".into()))}).unwrap();owner.begin_prepared_video_continuation(prepared).unwrap();let before=live(&owner);
    owner.engine.renderer.reject.store(true,Ordering::Relaxed);assert!(owner.prepare_step(StepKind::Continue).is_err());assert_eq!(live(&owner),before);assert!(!owner.state().vars.contains_key("failure_draw"));owner.engine.renderer.reject.store(false,Ordering::Relaxed);
    let prepared=owner.prepare_step(StepKind::Continue).unwrap();assert_eq!(live(&owner),before);owner.begin_prepared_step_commit(prepared).unwrap().commit();assert_eq!(owner.state().vars["failure_draw"],Value::Int(expected));assert_eq!(owner.state().vars["error_message"],Value::Str("actual decoder failure".into()));assert_eq!(owner.state().random,expected_random);assert!(!owner.state().vars.contains_key("after_wait"));assert!(!owner.waiting_for_video());
}

#[test]
fn startup_terminal_blocked_and_resume_are_real_commands_without_releasing_wait(){
    for label in ["scene label","label"]{
        let source=format!("init {{video.play(\"intro\",video_clip(\"clip.webm\",{{\"volume\":0}})) video.wait(\"intro\") set after_wait=1}}\n{label} start\n\"Done\"\n");let mut owner=video_startup(&source);for _ in 0..2{let p=owner.prepare_step(StepKind::Continue).unwrap();owner.begin_prepared_step_commit(p).unwrap().commit();}
        let view=owner.video_views().unwrap()[0].clone();let random=owner.state().random;let prepared=owner.prepare_video_continuation(&StartupVideoReport{view,feedback:GameVideoFeedback::Report(crate::video::Feedback::AutoplayBlocked("actual policy".into()))}).unwrap();owner.begin_prepared_video_continuation(prepared).unwrap();let prepared=owner.prepare_step(StepKind::Continue).unwrap();owner.begin_prepared_step_commit(prepared).unwrap().commit();assert!(owner.waiting_for_video());let view=owner.video_views().unwrap()[0].clone();assert_eq!(view.track.playback,crate::video::Playback::Blocked);assert!(owner.video_clock_source(&view).unwrap().is_none());
        let prepared=owner.prepare_video_continuation(&StartupVideoReport{view,feedback:GameVideoFeedback::Resume}).unwrap();owner.begin_prepared_video_continuation(prepared).unwrap();let prepared=owner.prepare_step(StepKind::Continue).unwrap();assert_eq!(prepared.video_commands().iter().map(|c|c.kind()).collect::<Vec<_>>(),vec![StartupVideoCommandKind::Resume]);owner.begin_prepared_step_commit(prepared).unwrap().commit();let view=owner.video_views().unwrap()[0].clone();assert_eq!(view.track.playback,crate::video::Playback::Playing);assert!(owner.video_clock_source(&view).unwrap().is_some());assert_eq!(owner.state().random,random);assert!(!owner.state().vars.contains_key("after_wait"));assert!(owner.waiting_for_video());
    }
}

#[test]
fn startup_terminal_portable_cold_uses_real_callback_phase_and_does_not_replay_init_or_rng(){
    for label in ["scene label","label"]{for error in [false,true]{
        let text=format!("handler finished(event){{set callback_draw=random(1,9) set callbacks=callbacks+1 set event_kind=event[\"kind\"]}}\ninit {{set sampled_init=random(1,9) set callbacks=0 video.play(\"intro\",video_clip(\"clip.webm\",{{\"volume\":0,\"on_end\":\"finished\",\"on_error\":\"finished\"}})) video.wait(\"intro\") set after_wait=random(1,9)}}\n{label} start\n\"Done\"\n");let mut warm=video_startup(&text);for _ in 0..4{let p=warm.prepare_step(StepKind::Continue).unwrap();warm.begin_prepared_step_commit(p).unwrap().commit();}let breakpoint=warm.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.function.as_deref()==Some("handler:finished")&&site.statement)).unwrap().breakpoint;warm.configure(vec![breakpoint],vec![]).unwrap();let sampled=warm.state().vars["sampled_init"].clone();let mut expected_random=warm.state().random;let expected=expected_random.integer(1,9).unwrap();let view=warm.video_views().unwrap()[0].clone();let feedback=if error{crate::video::Feedback::Error("actual saved decode failure".into())}else{crate::video::Feedback::End};let prepared=warm.prepare_video_continuation(&StartupVideoReport{view,feedback:GameVideoFeedback::Report(feedback)}).unwrap();warm.begin_prepared_video_continuation(prepared).unwrap();let p=warm.prepare_step(StepKind::Continue).unwrap();warm.begin_prepared_step_commit(p).unwrap().commit();assert_eq!(warm.snapshot().site.unwrap().function.as_deref(),Some("handler:finished"));
        let save=warm.capture_save15(1,"actual stopped media callback".into()).unwrap();let encoded=save.encode().unwrap();let raw:serde_json::Value=serde_json::from_str(&encoded).unwrap();assert!(raw["debugger"]["startup"]["callback"].is_object());assert!(raw["debugger"]["startup"].get("waiting").is_none());assert!(raw["debugger"]["trace"].is_object());let mut cold=video_startup(&text);assert!(!cold.state().vars.contains_key("sampled_init"));cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&encoded).unwrap()).unwrap();assert_eq!(cold.state().vars["sampled_init"],sampled);assert!(!cold.waiting_for_video());assert_eq!(cold.snapshot().site.unwrap().function.as_deref(),Some("handler:finished"));
        for owner in [&mut warm,&mut cold]{let p=owner.prepare_step(StepKind::Continue).unwrap();owner.begin_prepared_step_commit(p).unwrap().commit();assert_eq!(owner.state().vars["sampled_init"],sampled);assert_eq!(owner.state().vars["callback_draw"],Value::Int(expected));assert_eq!(owner.state().vars["callbacks"],Value::Int(1));assert_eq!(owner.state().vars["event_kind"],Value::Str(if error{"error"}else{"end"}.into()));assert_eq!(owner.state().random,expected_random);assert!(!owner.state().vars.contains_key("after_wait"));assert!(owner.state().videos.waiting.is_none());assert!(!owner.is_complete());}
    }}
}

#[test]
fn startup_terminal_portable_forged_phase_payload_member_and_source_refuse_atomically(){
    let text="handler finished(event){set callback_draw=random(1,9)}\ninit {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0,\"on_end\":\"finished\"})) video.wait(\"intro\") set after_wait=1}\nscene label start\n\"Done\"\n";let mut warm=video_startup(text);for _ in 0..2{let p=warm.prepare_step(StepKind::Continue).unwrap();warm.begin_prepared_step_commit(p).unwrap().commit();}let view=warm.video_views().unwrap()[0].clone();let p=warm.prepare_video_continuation(&StartupVideoReport{view,feedback:GameVideoFeedback::Report(crate::video::Feedback::End)}).unwrap();warm.begin_prepared_video_continuation(p).unwrap();let save=warm.capture_save15(1,"real pending callback".into()).unwrap();let mut cold=video_startup(text);let before=live(&cold);
    let mut forged=save.clone();forged.debugger.as_mut().unwrap().trace=None;assert!(cold.restore_save15(forged).is_err());assert_eq!(live(&cold),before);
    let mut forged=serde_json::to_value(&save).unwrap();forged["debugger"]["startup"]["waiting"]=serde_json::json!({"kind":"WaitingVideo","name":"intro"});assert!(crate::save::consumer_checkpoint::Save15::decode(&serde_json::to_string(&forged).unwrap()).is_err());assert_eq!(live(&cold),before);
    let mut forged=serde_json::to_value(&save).unwrap();forged["debugger"]["startup"]["callback"]["name"]=serde_json::json!("missing");let forged=crate::save::consumer_checkpoint::Save15::decode(&serde_json::to_string(&forged).unwrap()).unwrap();assert!(cold.restore_save15(forged).is_err());assert_eq!(live(&cold),before);
    let mut forged=serde_json::to_value(&save).unwrap();forged["debugger"]["startup"]["callback"]["payload"]=serde_json::json!({"kind":"position","seconds":0.1,"duration":2.0});assert!(crate::save::consumer_checkpoint::Save15::decode(&serde_json::to_string(&forged).unwrap()).is_err());assert_eq!(live(&cold),before);
    let mut changed=video_startup(&(text.to_owned()+"\n "));let before=live(&changed);assert!(changed.restore_save15(save).is_err());assert_eq!(live(&changed),before);
}

#[test]
fn startup_terminal_portable_real_ui_residence_cold_and_replaced_target_rejection(){
    let text=r#"screen media(){return component("root","panel",{},[component("player","panel",{},[])])}
handler finished(event){set callback_draw=random(1,9) set callbacks=callbacks+1}
init {set callbacks=0 ui.open("media",[],false,0) video.play("intro",video_clip("clip.webm",{"target":"ui:media/player","volume":0,"on_end":"finished"})) video.wait("intro") set after_wait=1}
scene label start
"Done"
"#;
    let mut warm=video_startup(text);for _ in 0..4{let p=warm.prepare_step(StepKind::Continue).unwrap();warm.begin_prepared_step_commit(p).unwrap().commit();}
    let view=warm.video_views().unwrap()[0].clone();let actual=view.ui_projection.as_ref().expect("Actual UI target must bind before callback");assert!(crate::ui_residency::resident_interface(warm.state(),&actual.interface).is_ok());
    let mut forged=view.clone();forged.ui_projection.as_mut().unwrap().interface.incarnation+=1;let before=live(&warm);assert!(warm.prepare_video_continuation(&StartupVideoReport{view:forged,feedback:GameVideoFeedback::Report(crate::video::Feedback::End)}).is_err());assert_eq!(live(&warm),before);
    let breakpoint=warm.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.function.as_deref()==Some("handler:finished")&&site.statement)).unwrap().breakpoint;warm.configure(vec![breakpoint],vec![]).unwrap();let p=warm.prepare_video_continuation(&StartupVideoReport{view,feedback:GameVideoFeedback::Report(crate::video::Feedback::End)}).unwrap();warm.begin_prepared_video_continuation(p).unwrap();let p=warm.prepare_step(StepKind::Continue).unwrap();warm.begin_prepared_step_commit(p).unwrap().commit();
    let save=warm.capture_save15(1,"Actual resident UI media callback".into()).unwrap();let mut cold=video_startup(text);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap();let restored=cold.video_views().unwrap()[0].clone();assert!(crate::ui_residency::resident_interface(cold.state(),&restored.ui_projection.as_ref().unwrap().interface).is_ok());
    let mut expected=warm.state().random;let draw=expected.integer(1,9).unwrap();for owner in [&mut warm,&mut cold]{let p=owner.prepare_step(StepKind::Continue).unwrap();owner.begin_prepared_step_commit(p).unwrap().commit();assert_eq!(owner.state().vars["callbacks"],Value::Int(1));assert_eq!(owner.state().vars["callback_draw"],Value::Int(draw));assert_eq!(owner.state().random,expected);assert!(owner.state().videos.waiting.is_none());assert!(!owner.state().vars.contains_key("after_wait"));}
}

#[test]
fn startup_terminal_portable_real_native_handler_producer_cold_does_not_recompute_draw(){
    let text=r#"screen media(){return component("root","panel",{},[component("player","panel",{},[])])}
handler finished(event){flow.consume_v2("handler:finished","gather"){capture "draw"=random(1,9) inspect "i"=__rvn_invariant_text("literal") set output=[__rvn_captured_v1("draw"),__rvn_terminal_v1("i","valid"),__rvn_terminal_v1("i","table_id"),__rvn_terminal_v1("i","key")]} set callbacks=callbacks+1}
init {set callbacks=0 ui.open("media",[],false,0) video.play("intro",video_clip("clip.webm",{"target":"ui:media/player","volume":0,"on_end":"finished"})) video.wait("intro") set after_wait=1}
scene label start
"Done"
"#;
    let mut warm=video_startup(text);for _ in 0..4{let p=warm.prepare_step(StepKind::Continue).unwrap();warm.begin_prepared_step_commit(p).unwrap().commit();}
    let mut offset=0;let breakpoint=loop{let page=warm.site_page(offset).unwrap();if let Some(entry)=page.entries.iter().find(|entry|entry.origin.as_ref().and_then(|origin|origin.scheduled_step.as_ref()).is_some_and(|step|step.scope.as_deref()==Some("handler:finished")&&step.index==1)){break entry.breakpoint.clone();}offset+=page.entries.len();assert!(offset<page.total,"Actual handler inspector producer must be offered");};warm.configure(vec![breakpoint],vec![]).unwrap();
    let initial=warm.state().random;let mut expected=initial;let draw=expected.integer(1,9).unwrap();let view=warm.video_views().unwrap()[0].clone();let p=warm.prepare_video_continuation(&StartupVideoReport{view,feedback:GameVideoFeedback::Report(crate::video::Feedback::End)}).unwrap();warm.begin_prepared_video_continuation(p).unwrap();let p=warm.prepare_step(StepKind::Continue).unwrap();warm.begin_prepared_step_commit(p).unwrap().commit();assert_eq!(warm.snapshot().origin.unwrap().scheduled_step.unwrap().index,1);assert_eq!(warm.state().random,initial,"Stopped candidate draw cannot publish before the true callback");
    let save=warm.capture_save15(1,"actual mid-native media producer".into()).unwrap();let mut cold=video_startup(text);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap();assert_eq!(cold.snapshot().origin.unwrap().scheduled_step.unwrap().index,1);
    for owner in [&mut warm,&mut cold]{let p=owner.prepare_step(StepKind::Continue).unwrap();owner.begin_prepared_step_commit(p).unwrap().commit();let Value::List(output)=&owner.state().vars["output"]else{panic!("Actual scheduled output missing")};assert_eq!(output[0],Value::Int(draw));assert_eq!(output[1],Value::Bool(false));assert_eq!(owner.state().vars["callbacks"],Value::Int(1));assert_eq!(owner.state().random,expected);assert!(owner.state().videos.waiting.is_none());assert!(!owner.state().vars.contains_key("after_wait"));}
}

#[path="engine_startup_driver_tests.rs"]
mod driver_receipts;

#[path="engine_startup_ending_tests_0904.rs"]
mod ending0904;

#[test]
fn startup_boxed_preparation_preserves_actual_drop_stale_rejection_and_entry_contract_0978(){
 let mut owner=pending(NATIVE);owner.engine.state.random=crate::random::RandomState::seeded(42);owner.prepare_session().unwrap();
 let before=live(&owner);let observed=owner.observation_source().unwrap();
 let discarded=owner.prepare_step_boxed(StepKind::Into).unwrap();assert_eq!(live(&owner),before);drop(discarded);assert!(owner.accepts_observation_source(&observed));
 let boxed=owner.prepare_step_boxed(StepKind::Into).unwrap();let by_value=owner.prepare_step(StepKind::Into).unwrap();assert_eq!(boxed.snapshot(),by_value.snapshot());assert_eq!(live(&owner),before);
 owner.begin_prepared_step_commit(*boxed).unwrap().commit();let committed=live(&owner);assert!(owner.begin_prepared_step_commit(by_value).is_err());assert_eq!(live(&owner),committed);assert!(!owner.accepts_observation_source(&observed));
 for _ in 0..100{if owner.is_complete(){break;}let next=owner.prepare_step_boxed(StepKind::Continue).unwrap();owner.begin_prepared_step_commit(*next).unwrap().commit();}
 assert!(owner.is_complete());let mut expected=crate::random::RandomState::seeded(42);let draw=expected.integer(1,9).unwrap();assert_eq!(owner.state().random,expected);assert_eq!(owner.state().vars["boot"],Value::Int(draw+1));
 let before=live(&owner);let boxed=owner.prepare_gameplay_entry_boxed(Some("start")).unwrap();let by_value=owner.prepare_gameplay_entry(Some("start")).unwrap();assert_eq!(boxed.snapshot(),by_value.snapshot());drop(boxed);assert_eq!(live(&owner),before);
 let boxed=owner.prepare_gameplay_entry_boxed(Some("start")).unwrap();owner.engine.state.vars.insert("actual_change".into(),Value::Int(9));let changed=live(&owner);assert!(owner.begin_prepared_step_commit(*boxed).is_err());assert_eq!(live(&owner),changed);
}
