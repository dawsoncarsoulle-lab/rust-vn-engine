//! Real owner/callback/entry receipt contracts; no Native/device acceptance.
use super::*;
fn checkpoint<R:Renderer>(owner:&EngineStartup<R>)->crate::audio_checkpoint::AudioCheckpointSnapshot{
    crate::audio_checkpoint::AudioCheckpointSnapshot{instance_id:owner.snapshot().instance,runtime_epoch:owner.snapshot().epoch,rows:vec![],music:vec![]}
}
const TERMINAL:&str="handler finished(event){set callback_draw=random(1,9) set callbacks=callbacks+1}\ninit {set callbacks=0 video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0,\"on_end\":\"finished\"})) video.wait(\"intro\") set after_wait=1}\nscene label start\n\"Done\"\n";
fn waiting()->EngineStartup<RejectRenderer>{
    let mut s=video_startup(TERMINAL);for _ in 0..3{let p=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(p).unwrap().commit();}assert!(s.waiting_for_video());s
}
fn stop_in_callback(s:&mut EngineStartup<RejectRenderer>){
    let bp=s.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.function.as_deref()==Some("handler:finished")&&site.statement)).unwrap().breakpoint;
    s.configure(vec![bp],vec![]).unwrap();let view=s.video_views().unwrap()[0].clone();let p=s.prepare_video_continuation(&StartupVideoReport{view,feedback:GameVideoFeedback::Report(crate::video::Feedback::End)}).unwrap();s.begin_prepared_video_continuation(p).unwrap();
    let p=s.prepare_step(StepKind::Continue).unwrap();assert!(p.terminal_completion.is_none());assert!(p.render_batch().is_none(),"The actual interior stop has no publication or PCM checkpoint");let committed=s.begin_prepared_host_step_commit(p).unwrap().commit_computation().unwrap();assert!(committed.terminal.is_none());assert!(!s.state().vars.contains_key("callback_draw"));
}
#[test]
fn startup_driver_terminal_receipt_appears_only_after_actual_callback_commit_and_pcm_anchor(){
    let mut s=waiting();stop_in_callback(&mut s);let before=live(&s);let prior=s.video_views().unwrap()[0].clone();
    let p=s.prepare_step(StepKind::Continue).unwrap();assert!(p.terminal_completion.is_some());assert_eq!(live(&s),before);
    let audio=checkpoint(&s);let committed=s.begin_prepared_host_step_commit(p).unwrap().commit_with_audio_checkpoint(audio).unwrap();let receipt=committed.terminal.expect("Actual completed operation issues its receipt");
    assert_eq!(receipt.prior_view(),&prior);assert!(matches!(receipt.feedback(),GameVideoFeedback::Report(crate::video::Feedback::End)));assert!(s.accepts_video_terminal_receipt(&receipt));assert_eq!(s.state().vars["callbacks"],Value::Int(1));assert!(!s.state().vars.contains_key("after_wait"));
    s.configure(vec![],vec![wire::Watch{scope:wire::WatchScope::Globals,name:"callbacks".into()}]).unwrap();assert!(!s.accepts_video_terminal_receipt(&receipt),"Configuration/source revokes the exact completed receipt");
}
#[test]
fn startup_driver_cold_callback_reissues_receipt_from_actual_recreated_owner_without_replay(){
    let mut warm=waiting();stop_in_callback(&mut warm);let save=warm.capture_save15(1,"actual callback".into()).unwrap();let mut cold=video_startup(TERMINAL);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap();
    let mut expected=warm.state().random;let draw=expected.integer(1,9).unwrap();let mut receipts=Vec::new();
    for s in [&mut warm,&mut cold]{let p=s.prepare_step(StepKind::Continue).unwrap();let audio=checkpoint(s);let done=s.begin_prepared_host_step_commit(p).unwrap().commit_with_audio_checkpoint(audio).unwrap();let receipt=done.terminal.unwrap();assert!(s.accepts_video_terminal_receipt(&receipt));assert_eq!(s.state().vars["callbacks"],Value::Int(1));assert_eq!(s.state().vars["callback_draw"],Value::Int(draw));assert_eq!(s.state().random,expected);receipts.push(receipt);}
    assert!(!cold.accepts_video_terminal_receipt(&receipts[0]));assert!(!warm.accepts_video_terminal_receipt(&receipts[1]));
}
#[test]
fn startup_driver_entry_prepares_real_transfer_before_consuming_complete_owner_and_refuses_atomically(){
    let mut s=video_startup("init {set boot=1}\nscene label start\nlocal value=3\n\"Done\"\n");while !s.is_complete(){let p=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(p).unwrap().commit();}
    let before=live(&s);let instance=s.snapshot().instance;let count=s.engine.renderer.published.load(Ordering::Relaxed);let p=s.prepare_gameplay_entry(Some("start")).unwrap();assert!(p.render_batch().unwrap().state.scene_execution.is_some());assert_eq!(live(&s),before);
    s.engine.renderer.reject.store(true,Ordering::Relaxed);assert!(s.begin_prepared_host_step_commit(p).is_err());assert_eq!(live(&s),before);assert_eq!(s.engine.renderer.published.load(Ordering::Relaxed),count);
    s.engine.renderer.reject.store(false,Ordering::Relaxed);let p=s.prepare_gameplay_entry(Some("start")).unwrap();let audio=checkpoint(&s);let done=s.begin_prepared_host_step_commit(p).unwrap().commit_with_audio_checkpoint(audio).unwrap();assert!(done.terminal.is_none());let e=s.into_engine().unwrap();assert_eq!(e.instance_id(),instance);assert_eq!(e.state.vars["boot"],Value::Int(1));assert!(e.state.scene_execution.is_some());
}
#[test]
fn startup_driver_pending_owner_cannot_prepare_entry_and_changed_source_revokes_complete_entry(){
    let mut s=video_startup("init {set boot=1}\nscene label start\n\"Done\"\n");let before=live(&s);assert!(s.prepare_gameplay_entry(Some("start")).is_err());assert_eq!(live(&s),before);
    while !s.is_complete(){let p=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(p).unwrap().commit();}
    let p=s.prepare_gameplay_entry(Some("start")).unwrap();s.engine.state.vars.insert("changed".into(),Value::Int(2));let before=live(&s);assert!(s.begin_prepared_host_step_commit(p).is_err());assert_eq!(live(&s),before);
}

#[test]
fn normal_core_loop_media_wait_cold_resume_retains_headers_callbacks_rng_and_arc(){
 const SOURCE:&str="handler finished(event){set callbacks=callbacks+1 set callback_draw=random(1,9)}\ninit {set callbacks=0 set seen=0 for item in [1,2] {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0,\"on_end\":\"finished\"})) video.wait(\"intro\") set seen=seen+item} set done=true}\nscene label start\n\"Ready\"\n";
 let mut warm=video_startup(SOURCE);for _ in 0..64{if warm.waiting_for_video(){break;}let plan=warm.prepare_step(StepKind::Continue).unwrap();warm.begin_prepared_step_commit(plan).unwrap().commit();}assert!(warm.waiting_for_video());let random=warm.state().random;
 let save=warm.capture_save15(1,"loop wait cold".into()).unwrap();let mut cold=video_startup(SOURCE);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap();assert_eq!(cold.state().random,random);assert!(cold.waiting_for_video());
 for owner in [&mut warm,&mut cold]{let frozen=std::sync::Arc::clone(&owner.program);let mut computation_steps=0;let mut publication_steps=0;
  for _ in 0..128{if owner.is_complete(){break;}
   if owner.waiting_for_video(){let view=owner.video_views().unwrap().into_iter().find(|view|view.id=="intro").unwrap();let report=StartupVideoReport{view,feedback:GameVideoFeedback::Report(crate::video::Feedback::End)};let plan=owner.prepare_video_continuation(&report).unwrap();owner.begin_prepared_video_continuation(plan).unwrap();}
   let plan=owner.prepare_step(StepKind::Continue).unwrap();
   if plan.render_batch().is_some(){
    let anchor=checkpoint(owner);let done=owner.begin_prepared_host_step_commit(plan).unwrap().commit_with_audio_checkpoint(anchor).unwrap();assert!(done.publication.is_some());publication_steps+=1;
   }else{
    let done=owner.begin_prepared_host_step_commit(plan).unwrap().commit_computation().unwrap();assert!(done.publication.is_none());computation_steps+=1;
   }
   assert!(std::sync::Arc::ptr_eq(&owner.program,&frozen));
  }
  assert!(owner.is_complete());assert!(computation_steps>0,"The actual loop header/continuation must exercise its computation-only commit");assert!(publication_steps>0,"The actual media loop must still exercise fenced publication commits");assert_eq!(owner.state().vars["seen"],Value::Int(3));assert_eq!(owner.state().vars["callbacks"],Value::Int(2));assert_eq!(owner.state().vars["done"],Value::Bool(true));
 }
 assert_eq!(warm.state().random,cold.state().random);assert_eq!(warm.state().vars,cold.state().vars);
}
#[test]
fn normal_core_synchronous_constructor_reports_real_host_wait_without_polling(){
 let owner=video_startup("init {for item in [1] {video.play(\"intro\",video_clip(\"clip.webm\",{\"volume\":0})) video.wait(\"intro\") set forbidden=1}}\nscene label start\n\"Ready\"\n");let error=owner.finish_synchronous().err().expect("A synchronous API cannot invent a terminal clock");assert!(error.to_string().contains("host must retain and drive EngineStartup"),"{error}");
}

// Append within engine_startup_driver_tests; actual trusted owner + actual sink.
#[test]
fn normal_core_legacy_empty_init_entry_publishes_actual_candidate_without_narration_or_rng(){
 for raw in ["init {while false {set unreachable=1}}\nlabel start\nset narrative_started=random(1,9)\n\"Not started\"\n","init {for item in [] {set unreachable=1}}\nlabel start\nset narrative_started=random(1,9)\n\"Not started\"\n"]{
  for label in [Some("start"),None]{
   let mut owner=video_startup(raw);let initial_random=owner.state().random;
   while !owner.is_complete(){let plan=owner.prepare_step(StepKind::Continue).unwrap();assert!(plan.render_batch().is_none(),"The real empty header must be computation only");owner.begin_prepared_host_step_commit(plan).unwrap().commit_computation().unwrap();}
   assert_eq!(owner.engine.renderer.published.load(Ordering::Relaxed),0);assert_eq!(owner.state().random,initial_random);assert!(!owner.state().vars.contains_key("unreachable"));assert!(!owner.state().vars.contains_key("narrative_started"));assert!(owner.engine.narrative.is_none());
   let before=live(&owner);let plan=owner.prepare_gameplay_entry(label).unwrap();let batch=plan.render_batch().expect("Even legacy/no-label entry must prepare a genuine full candidate publication");
   assert!(batch.display.is_none());assert!(batch.state.scene_execution.is_none());assert_eq!(batch.state.random,initial_random);assert!(!batch.state.vars.contains_key("narrative_started"));assert!(plan.video_commands().is_empty());assert_eq!(live(&owner),before);
   owner.engine.renderer.reject.store(true,Ordering::Relaxed);assert!(owner.begin_prepared_host_step_commit(plan).is_err());assert_eq!(live(&owner),before);assert_eq!(owner.engine.renderer.published.load(Ordering::Relaxed),0);
   owner.engine.renderer.reject.store(false,Ordering::Relaxed);let plan=owner.prepare_gameplay_entry(label).unwrap();let audio=checkpoint(&owner);let done=owner.begin_prepared_host_step_commit(plan).unwrap().commit_with_audio_checkpoint(audio).unwrap();
   let(source,batch)=done.publication.expect("Actual host commit must transfer its opaque Source and actual candidate batch");assert_eq!(source.instance_id(),owner.snapshot().instance);assert!(batch.display.is_none());assert_eq!(owner.engine.renderer.published.load(Ordering::Relaxed),1);assert_eq!(owner.state().random,initial_random);assert!(!owner.state().vars.contains_key("narrative_started"));
   let game=owner.into_engine().unwrap();assert!(game.narrative.is_none());assert_eq!(game.state.random,initial_random);
  }
 }
}
