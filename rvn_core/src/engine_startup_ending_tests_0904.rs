//! Real source-owned Init command and prepared publication. CPU, not native.
use super::*;
use std::sync::Mutex;

#[derive(Clone,Default)]
struct EndingHost{
    events:Arc<Mutex<Vec<String>>>,
    reject:Arc<AtomicBool>,
    published:Arc<AtomicUsize>,
}
impl Renderer for EndingHost{
    fn validate_scene_batch(&self,_:&crate::renderer::SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("actual ending host rejects batch".into())}else{Ok(())}}
    fn publish_scene_batch(&mut self,_:crate::renderer::SceneRenderBatch){
        // A real epoch replacement may clear an outgoing command queue.
        // Startup effects therefore belong after replacement, before drain.
        self.events.lock().unwrap().clear();self.published.fetch_add(1,Ordering::SeqCst);
    }
    fn unlock_ending(&mut self,id:&str){self.events.lock().unwrap().push(id.to_owned());}
    fn set_background(&mut self,_:&str,_:&Transition){panic!("visual state belongs to Startup's coherent batch")}
    fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
    fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
    fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
    fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
    fn show_choice(&mut self,_:&[String])->usize{0}
    fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[rvn_parser::Hotspot])->usize{panic!("no invented input")}
    fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
    fn music_stop(&mut self,_:&Transition){}
    fn music_set_volume(&mut self,_:f32){}
    fn sfx_play(&mut self,_:&str,_:&Transition){}
    fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
impl StartupBatchSink for EndingHost{
    type PreparedPublication=StartupPublicationSource;
    type Publication=Vec<String>;
    fn prepare_startup_publication(&self,source:&StartupPublicationSource,batch:&crate::renderer::SceneRenderBatch)->Result<Self::PreparedPublication,String>{self.validate_scene_batch(batch)?;Ok(source.clone())}
    fn publish_startup_publication(&mut self,source:StartupPublicationSource,batch:crate::renderer::SceneRenderBatch,prepared:Self::PreparedPublication)->Self::Publication{assert_eq!(source,prepared);self.publish_scene_batch(batch);std::mem::take(&mut *self.events.lock().unwrap())}
    fn validate_startup_effects(&self,_:&StartupPublicationSource,_:&StartupHostEffects)->Result<(),String>{Ok(())}
    fn publish_startup_publication_with_effects(&mut self,source:StartupPublicationSource,batch:crate::renderer::SceneRenderBatch,prepared:Self::PreparedPublication,effects:StartupHostEffects)->Self::Publication{assert_eq!(source,prepared);self.publish_scene_batch(batch);effects.publish(self);std::mem::take(&mut *self.events.lock().unwrap())}
}
fn make(source:&str,host:EndingHost)->EngineStartup<EndingHost>{Engine::prepare_debugger_startup(program(source),host,8,HashMap::new(),TextRuntimeContext::default(),None).unwrap()}
const ENDING:&str="init {flow.consume_v2(\"boot\",\"ending\"){capture \"draw\"=random(1,9) unlock_ending \"good\"}}\nscene label start\n\"Done\"\n";
fn pcm<R:Renderer>(s:&EngineStartup<R>,p:&PreparedStartupStep)->crate::audio_checkpoint::AudioCheckpointSnapshot{crate::audio_checkpoint::AudioCheckpointSnapshot{instance_id:s.snapshot().instance,runtime_epoch:p.render_batch().unwrap().runtime_epoch,rows:vec![],music:vec![]}}

#[test]
fn ending0904_native_init_selects_actual_owner_without_changing_legacy_or_story_selection(){
    assert!(Engine::<EndingHost>::program_requires_owned_startup(&program(ENDING)));
    assert!(!Engine::<EndingHost>::program_requires_owned_startup(&program("init{unlock_ending \"old\"}\nlabel start\n\"Done\"")));
    assert!(!Engine::<EndingHost>::program_requires_owned_startup(&program("label start\nflow.consume_v2(\"boot\",\"ending\"){unlock_ending \"story\"}\n\"Done\"")));
}
#[test]
fn ending0904_prepare_drop_refusal_retry_and_stale_plan_publish_once(){
    let h=EndingHost::default();let mut s=make(ENDING,h.clone());let before=live(&s);let mut random=s.state().random;random.integer(1,9).unwrap();
    let discarded=s.prepare_step(StepKind::Continue).unwrap();drop(discarded);assert_eq!(live(&s),before);assert!(h.events.lock().unwrap().is_empty());assert_eq!(h.published.load(Ordering::SeqCst),0);
    let a=s.prepare_step(StepKind::Continue).unwrap();h.reject.store(true,Ordering::SeqCst);assert!(s.begin_prepared_step_commit(a).is_err());assert_eq!(live(&s),before);assert!(h.events.lock().unwrap().is_empty());
    h.reject.store(false,Ordering::SeqCst);let a=s.prepare_step(StepKind::Continue).unwrap();let b=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(a).unwrap().commit();
    assert_eq!(s.state().random,random);assert!(s.is_complete());assert_eq!(*h.events.lock().unwrap(),vec!["good"]);assert_eq!(h.published.load(Ordering::SeqCst),1);
    assert!(s.begin_prepared_step_commit(b).is_err());assert_eq!(*h.events.lock().unwrap(),vec!["good"]);assert_eq!(h.published.load(Ordering::SeqCst),1);
}
#[test]
fn ending0904_old_host_refuses_new_effects_before_mutating_source_or_cursor(){
    let mut s=video_startup(ENDING);let before=live(&s);let p=s.prepare_step(StepKind::Continue).unwrap();
    let error=s.begin_prepared_host_step_commit(p).err().expect("default host has no effect adapter");assert!(error.to_string().contains("one-shot publication adapter"));assert_eq!(live(&s),before);assert_eq!(s.engine.renderer.published.load(Ordering::Relaxed),0);
}
#[test]
fn ending0904_host_pcm_refusal_and_real_ordered_publication_keep_effect_atomic(){
    let h=EndingHost::default();let mut s=make(ENDING,h.clone());let before=live(&s);let p=s.prepare_step(StepKind::Continue).unwrap();let mut wrong=pcm(&s,&p);wrong.instance_id+=1;
    assert!(s.begin_prepared_host_step_commit(p).unwrap().commit_with_audio_checkpoint(wrong).is_err());assert_eq!(live(&s),before);assert!(h.events.lock().unwrap().is_empty());assert_eq!(h.published.load(Ordering::SeqCst),0);
    let p=s.prepare_step(StepKind::Continue).unwrap();let checkpoint=pcm(&s,&p);let committed=s.begin_prepared_host_step_commit(p).unwrap().commit_with_audio_checkpoint(checkpoint).unwrap();assert_eq!(committed.publication.unwrap(),vec!["good"]);assert!(h.events.lock().unwrap().is_empty());assert_eq!(h.published.load(Ordering::SeqCst),1);assert!(s.is_complete());
}
#[test]
fn ending0904_changed_debug_configuration_revokes_effect_before_publication(){
    let h=EndingHost::default();let mut s=make(ENDING,h.clone());let p=s.prepare_step(StepKind::Continue).unwrap();s.configure(vec![],vec![wire::Watch{scope:wire::WatchScope::Globals,name:"after".into()}]).unwrap();let before=live(&s);
    assert!(s.begin_prepared_host_step_commit(p).is_err());assert_eq!(live(&s),before);assert_eq!(h.published.load(Ordering::SeqCst),0);assert!(h.events.lock().unwrap().is_empty());
}
#[test]
fn ending0904_cold_before_executes_once_and_cold_after_does_not_replay_host_effect(){
    let source=ENDING.replace("}}\nscene","} set after=7}\nscene");let h=EndingHost::default();let mut warm=make(&source,h.clone());
    let at_entry=warm.capture_save15(1,"before ending".into()).unwrap();let h_before=EndingHost::default();let mut before=make(&source,h_before.clone());before.restore_save15(at_entry).unwrap();assert!(h_before.events.lock().unwrap().is_empty());
    for s in [&mut warm,&mut before]{s.step(StepKind::Continue).unwrap();assert!(!s.is_complete());}
    assert_eq!(*h.events.lock().unwrap(),vec!["good"]);assert_eq!(*h_before.events.lock().unwrap(),vec!["good"]);
    let at_next=warm.capture_save15(2,"ending already committed".into()).unwrap();let h_after=EndingHost::default();let mut after=make(&source,h_after.clone());after.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&at_next.encode().unwrap()).unwrap()).unwrap();assert!(h_after.events.lock().unwrap().is_empty());
    for s in [&mut warm,&mut before,&mut after]{for _ in 0..8{if s.is_complete(){break;}s.step(StepKind::Continue).unwrap();}assert!(s.is_complete());assert_eq!(s.state().vars["after"],Value::Int(7));}
    assert!(h_after.events.lock().unwrap().is_empty(),"cold data cannot reconstruct an already delivered one-shot");
}
#[test]
fn ending0904_unselected_branch_does_not_evaluate_or_publish_ending(){
    let h=EndingHost::default();let mut s=make(&format!("init{{if false{{flow.consume_v2(\"boot\",\"ending\"){{capture \"draw\"=random(1,9) unlock_ending \"never\"}}}}set after=1}}\nscene label start\n\"Done\""),h.clone());let random=s.state().random;
    for _ in 0..8{if s.is_complete(){break;}s.step(StepKind::Continue).unwrap();}assert!(s.is_complete());assert_eq!(s.state().random,random);assert_eq!(s.state().vars["after"],Value::Int(1));assert!(h.events.lock().unwrap().is_empty());
}
#[test]
fn ending0904_oversized_host_effect_refuses_without_live_rng_or_publication(){
    let source=ENDING.replace("\"good\"",&format!("\"{}\"","x".repeat(70_000)));let h=EndingHost::default();let s=make(&source,h.clone());let before=live(&s);
    let error=s.prepare_step(StepKind::Continue).err().expect("one-shot budget is measured before publication");assert!(error.to_string().contains("64 KiB"),"{error}");assert_eq!(live(&s),before);assert!(h.events.lock().unwrap().is_empty());assert_eq!(h.published.load(Ordering::SeqCst),0);
}
#[test]
fn ending0904_legacy_raw_init_and_visual_batch_keep_existing_routes(){
    let p=program("init{unlock_ending \"old\"}\nlabel start\n\"Done\"");let h=EndingHost::default();let _e=Engine::from_program(p,h.clone(),8).unwrap();assert_eq!(*h.events.lock().unwrap(),vec!["old"]);
    let source="init {flow.consume_v2(\"boot\",\"bg\"){scene \"garden.png\"} flow.consume_v2(\"boot\",\"ending\"){unlock_ending \"good\"}}\nscene label start\n\"Done\"";let h=EndingHost::default();let mut s=make(source,h.clone());
    for _ in 0..8{if s.is_complete(){break;}s.step(StepKind::Continue).unwrap();}assert!(s.is_complete());assert_eq!(s.state().background_image,"garden.png");assert_eq!(*h.events.lock().unwrap(),vec!["good"]);
}
