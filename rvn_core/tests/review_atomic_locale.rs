use rvn_core::{Engine, TextRuntimeContext, Renderer, renderer::SceneRenderBatch, locale::{LocaleCatalog, LocaleManager}};
use rvn_parser::{Hotspot, ParsedProgram, Position, Transition, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};

#[derive(Clone, Default)]
struct Host { batches: Arc<Mutex<Vec<SceneRenderBatch>>>, reject: Arc<AtomicBool>, reject_audio:Arc<AtomicBool> }
impl Renderer for Host {
    fn validate_audio_restore(&self,_:&rvn_core::GameState)->Result<(),String>{if self.reject_audio.load(Ordering::SeqCst){Err("review actual audio refusal".into())}else{Ok(())}}
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> { if self.reject.load(Ordering::SeqCst) { Err("actual host preflight refusal".into()) } else { Ok(()) } }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) { self.batches.lock().unwrap().push(batch); }
    fn supports_programmable_ui(&self) -> bool { true }
    fn update_interfaces(&mut self, _: &[rvn_ui::programmable::ScreenView]) -> Result<(), String> { Ok(()) }
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &Position, _: &Transition, _: Option<&rvn_core::types::SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &rvn_core::types::SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &rvn_core::types::SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize { 0 }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize { 0 }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}
fn locale(lang: &str) -> LocaleManager {
    let identity = rvn_parser::localized_text::TextIdentity { owner: "asset:caption".into(), namespace: "VN".into(), key: "title".into() };
    let catalogs = [("en", "Hello English"), ("fr", "Bonjour")].into_iter().map(|(code, translation)| {
        let content = format!("[strings]\n{}={}\n", serde_json::to_string(&identity.locale_key()).unwrap(), serde_json::to_string(translation).unwrap());
        let mut catalog = LocaleCatalog::parse(code, &content).unwrap();
        catalog.record_text_source_witness(&identity, "Hello").unwrap();
        (code.into(), catalog)
    }).collect();
    LocaleManager::from_frozen_catalogs("en", lang, vec!["en".into(), "fr".into()], catalogs).unwrap()
}
fn program() -> ParsedProgram {
    rvn_parser::parse_program(r#"screen panel(){return component("caption","text",{"text":text_to_string(__rvn_localizable_text("Hello","asset:caption","VN","title",""))},[])} scene label start flow.consume_v2("scene:start","open"){ui.open("panel",[],false,0)} "Hello" "after" "#).unwrap()
}
fn engine(p: &ParsedProgram, lang: &str) -> Engine<Host> {
    Engine::from_program_with_persistent_and_text_context(p.clone(), Host::default(), 16, HashMap::new(), TextRuntimeContext::new(Default::default(), Some(locale(lang)))).unwrap()
}
fn frozen(e: &Engine<Host>) -> (serde_json::Value, rvn_core::locale::LocaleContentSnapshot, u64, usize) {
    (serde_json::to_value(&e.state).unwrap(), e.locale.as_ref().unwrap().portable_receipt().unwrap(), e.runtime_epoch(), e.renderer.batches.lock().unwrap().len())
}
fn save(p: &ParsedProgram) -> rvn_core::save::consumer_checkpoint::Save15 {
    let mut source = engine(p, "fr");
    source.step_silent().unwrap();
    let bytes = source.capture_save15(p, None, 1, "selected French".into()).unwrap().encode().unwrap();
    rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()
}

#[test]
fn cold_selected_locale_and_actual_interface_projection_publish_together() {
    let p = program();
    let mut live = engine(&p, "en");
    let before = frozen(&live);
    assert!(live.restore_save15(&p, save(&p)).is_err());
    assert_eq!(frozen(&live), before);
    let prepared = live.prepare_save15_with_locale(&p, save(&p), Some(locale("fr"))).unwrap();
    assert_eq!(frozen(&live), before);
    assert!(live.commit_prepared_save15(prepared).unwrap().is_none());
    assert_eq!(live.locale.as_ref().unwrap().current_lang, "fr");
    assert!(live.runtime_epoch() > before.2);
    let batch = live.renderer.batches.lock().unwrap().last().unwrap().clone();
    assert_eq!(serde_json::to_value(&batch.state).unwrap(), serde_json::to_value(&live.state).unwrap());
    let mut reference = engine(&p, "fr");
    reference.step_silent().unwrap();
    let expected = reference.interface_views().unwrap();
    assert_eq!(batch.interfaces, expected);
    assert!(format!("{expected:?}").contains("Bonjour"));
}

#[test]
fn actual_renderer_refusal_after_preparation_has_no_partial_language_or_game_effect() {
    let p = program(); let mut e = engine(&p, "en");
    let plan = e.prepare_save15_with_locale(&p, save(&p), Some(locale("fr"))).unwrap();
    let before = frozen(&e);
    e.renderer.reject.store(true, Ordering::SeqCst);
    assert!(e.commit_prepared_save15(plan).is_err());
    assert_eq!(frozen(&e), before);
}

#[test]
fn actual_live_state_change_revokes_prepared_plan_without_reverting_the_change() {
    let p = program(); let mut e = engine(&p, "en");
    let plan = e.prepare_save15_with_locale(&p, save(&p), Some(locale("fr"))).unwrap();
    e.state.vars.insert("live change".into(), Value::Int(19));
    let before = frozen(&e);
    assert!(e.commit_prepared_save15(plan).is_err()); assert_eq!(frozen(&e), before);
}

#[test]
fn identical_cold_engine_cannot_adopt_another_instance_prepared_authority() {
    let p = program(); let e = engine(&p, "en"); let mut other = engine(&p, "en");
    let plan = e.prepare_save15_with_locale(&p, save(&p), Some(locale("fr"))).unwrap();
    let before = frozen(&other);
    assert!(other.commit_prepared_save15(plan).is_err()); assert_eq!(frozen(&other), before);
}

#[test]
fn changed_real_source_bytes_are_refused_before_preparation_or_any_effect() {
    let p = program(); let e = engine(&p, "en"); let mut changed = p.clone(); changed.sources[0].source.push('\n');
    let before = frozen(&e);
    assert!(e.prepare_save15_with_locale(&changed, save(&p), Some(locale("fr"))).is_err()); assert_eq!(frozen(&e), before);
}

#[test]
fn missing_or_wrong_candidate_locale_is_not_authorized_by_the_saved_language_label() {
    let p = program(); let e = engine(&p, "en"); let before = frozen(&e);
    assert!(e.prepare_save15_with_locale(&p, save(&p), None).is_err());
    assert!(e.prepare_save15_with_locale(&p, save(&p), Some(locale("en"))).is_err()); assert_eq!(frozen(&e), before);
}

#[test]
fn selected_locale_change_between_prepare_and_commit_revokes_the_whole_plan() {
    let p = program(); let mut e = engine(&p, "en");
    let plan = e.prepare_save15_with_locale(&p, save(&p), Some(locale("fr"))).unwrap();
    e.locale.as_mut().unwrap().set_language("fr").unwrap(); let before = frozen(&e);
    assert!(e.commit_prepared_save15(plan).is_err()); assert_eq!(frozen(&e), before);
}

#[test]
fn actual_language_round_trip_revokes_preparation_even_when_portable_content_returns_identical() {
    let p = program(); let mut e = engine(&p, "en");
    let plan = e.prepare_save15_with_locale(&p, save(&p), Some(locale("fr"))).unwrap();
    let original = e.locale.as_ref().unwrap().portable_receipt().unwrap();
    e.locale.as_mut().unwrap().set_language("fr").unwrap();
    e.locale.as_mut().unwrap().set_language("en").unwrap();
    assert_eq!(e.locale.as_ref().unwrap().portable_receipt().unwrap(), original);
    let before = frozen(&e);
    assert!(e.commit_prepared_save15(plan).is_err(), "A real locale mutation and return must revoke the prepared live authority");
    assert_eq!(frozen(&e), before);
}

#[test]
fn replacing_a_locale_with_equal_content_revokes_opaque_live_authority() {
    let p=program(); let mut e=engine(&p,"en");
    let plan=e.prepare_save15_with_locale(&p,save(&p),Some(locale("fr"))).unwrap();
    let old=e.locale.as_ref().unwrap().revision_stamp();
    let receipt=e.locale.as_ref().unwrap().portable_receipt().unwrap();
    e.locale=Some(locale("en"));
    assert_eq!(e.locale.as_ref().unwrap().portable_receipt().unwrap(),receipt);
    assert_ne!(e.locale.as_ref().unwrap().revision_stamp(),old);
    let before=frozen(&e);
    assert!(e.commit_prepared_save15(plan).is_err()); assert_eq!(frozen(&e),before);
}

#[test]
fn no_op_and_refused_language_requests_preserve_the_live_stamp_and_exact_wire_receipt() {
    let mut value=locale("en"); let before=value.revision_stamp(); let receipt=value.portable_receipt().unwrap();
    value.set_language("en").unwrap(); assert_eq!(value.revision_stamp(),before);
    assert!(value.set_language("outside_actual_provider").is_err()); assert_eq!(value.revision_stamp(),before);
    value.reload_current(); assert_eq!(value.revision_stamp(),before);
    assert_eq!(value.portable_receipt().unwrap(),receipt);
    let mut isolated=value.clone(); isolated.set_language("fr").unwrap();
    assert_ne!(isolated.revision_stamp(),before); assert_eq!(value.revision_stamp(),before);
    assert_eq!(value.portable_receipt().unwrap(),receipt);
}

#[test]fn audio_refusal_at_prepare_or_after_prepare_never_publishes_or_changes_locale(){let p=program();let mut e=engine(&p,"en");let before=frozen(&e);e.renderer.reject_audio.store(true,Ordering::SeqCst);assert!(e.prepare_save15_with_locale(&p,save(&p),Some(locale("fr"))).is_err());assert_eq!(frozen(&e),before);e.renderer.reject_audio.store(false,Ordering::SeqCst);let plan=e.prepare_save15_with_locale(&p,save(&p),Some(locale("fr"))).unwrap();e.renderer.reject_audio.store(true,Ordering::SeqCst);assert!(e.commit_prepared_save15(plan).is_err());assert_eq!(frozen(&e),before);}
#[test]fn script_history_and_table_authority_revoke_each_independently(){let p=program();for mutation in 0..3{let mut e=engine(&p,"en");let plan=e.prepare_save15_with_locale(&p,save(&p),Some(locale("fr"))).unwrap();match mutation{0=>e.script.push(rvn_parser::Statement::SetVar{name:"changed".into(),value:rvn_parser::Expr::Int(1)}),1=>e.history.push(e.state.clone(),None),_=>{let asset=rvn_core::string_table::StringTableAsset{id:"asset:review".into(),namespace:"Review".into(),entries:Default::default()};e.text_resources_mut().publish(asset).unwrap();e.text_resources_mut().unload("asset:review").unwrap();}}let before=frozen(&e);assert!(e.commit_prepared_save15(plan).is_err());assert_eq!(frozen(&e),before);}}
#[test]fn a_second_prepared_plan_is_revoked_by_first_commit_even_when_portable_state_is_equal(){let p=program();let mut e=engine(&p,"en");let first=e.prepare_save15_with_locale(&p,save(&p),Some(locale("fr"))).unwrap();let second=e.prepare_save15_with_locale(&p,save(&p),Some(locale("fr"))).unwrap();e.commit_prepared_save15(first).unwrap();let before=frozen(&e);assert!(e.commit_prepared_save15(second).is_err());assert_eq!(frozen(&e),before);}
#[test]fn frozen_candidate_catalog_matches_selection_but_no_foreign_language_content(){let p=program();let e=engine(&p,"en");let before=frozen(&e);let foreign=LocaleManager::from_frozen_catalogs("en","fr",vec!["en".into(),"fr".into()],[("en".into(),LocaleCatalog::parse("en","[strings]\n\"x\"=\"other\"").unwrap()),("fr".into(),LocaleCatalog::parse("fr","[strings]\n\"x\"=\"other\"").unwrap())].into()).unwrap();assert!(e.prepare_save15_with_locale(&p,save(&p),Some(foreign)).is_err());assert_eq!(frozen(&e),before);}
#[test]fn cold_candidate_locale_session_resumes_actual_producer_once_and_revokes_old_session(){use rvn_core::engine::{DebugOperation,DebugProgress};use rvn_core::editor_debugger as w;let source=include_str!("fixtures/review_atomic_locale.rvn");let mut p=rvn_parser::parse_program(source).unwrap();let path=Some(std::path::PathBuf::from("C:/review/producer.rvn"));p.sources[0].path=path.clone();for origin in &mut p.origins{origin.source=path.clone();}let prepared=rvn_core::program::PreparedNarrative::from_parsed(p.clone()).unwrap();let pc=prepared.script.iter().position(|s|matches!(s,rvn_parser::Statement::Function{name,..}if name=="producer")).unwrap();let bp=w::Breakpoint{position:prepared.position(pc),site:Some(w::Site{function:Some("producer".into()),route:vec![w::Route::Statement(1)],depth:1,statement:true}),enabled:true};let mut author=engine(&p,"fr");author.state.random=rvn_core::random::RandomState::seeded(4);author.enter_label("start").unwrap();let mut session=author.debug_begin(DebugOperation::Statement,vec![bp]).unwrap();assert_eq!(author.debug_step(&mut session,w::StepKind::Continue).unwrap().0,DebugProgress::Stopped);let save=author.capture_save15(&p,Some(&session),1,"candidate language".into()).unwrap();let mut live=engine(&p,"en");live.enter_label("start").unwrap();let old=live.debug_begin(DebugOperation::Statement,vec![]).unwrap();let before=frozen(&live);let plan=live.prepare_save15_with_locale(&p,save,Some(locale("fr"))).unwrap();assert_eq!(frozen(&live),before);let mut restored=live.commit_prepared_save15(plan).unwrap().unwrap();assert!(!live.accepts_debug_session(&old));assert!(live.accepts_debug_session(&restored));for _ in 0..128{if live.debug_step(&mut restored,w::StepKind::Continue).unwrap().0==DebugProgress::Complete{break;}}assert_eq!(live.state.vars["result"],Value::Str("Bonjour".into()));let mut expected=rvn_core::random::RandomState::seeded(4);expected.integer(1,1).unwrap();assert_eq!(live.state.random,expected);assert_eq!(live.locale.as_ref().unwrap().current_lang,"fr");}

#[test]fn independently_mutated_locale_clone_cannot_replace_live_authority_at_same_generation(){let p=program();let mut e=engine(&p,"en");let mut branched=e.locale.as_ref().unwrap().clone();e.locale.as_mut().unwrap().set_language("fr").unwrap();let plan=e.prepare_save15_with_locale(&p,save(&p),Some(locale("fr"))).unwrap();branched.set_language("fr").unwrap();assert_eq!(branched.portable_receipt().unwrap(),e.locale.as_ref().unwrap().portable_receipt().unwrap());e.locale=Some(branched);let before=frozen(&e);let result=e.commit_prepared_save15(plan);println!("COW real commit accepted={} epoch_before={} epoch_after={} batches_before={} batches_after={}",result.is_ok(),before.2,e.runtime_epoch(),before.3,e.renderer.batches.lock().unwrap().len());assert!(result.is_err(),"independently mutated locale branch must revoke prepared live authority, even with equal content/generation");assert_eq!(frozen(&e),before);}

fn one_locale()->LocaleManager{let identity=rvn_parser::localized_text::TextIdentity{owner:"asset:caption".into(),namespace:"VN".into(),key:"title".into()};let content=format!("[strings]\n{}={}\n",serde_json::to_string(&identity.locale_key()).unwrap(),serde_json::to_string("Hello English").unwrap());let mut catalog=LocaleCatalog::parse("en",&content).unwrap();catalog.record_text_source_witness(&identity,"Hello").unwrap();LocaleManager::from_frozen_catalogs("en","en",vec!["en".into()],[("en".into(),catalog)].into()).unwrap()}
#[test]fn equal_content_locale_replacement_revokes_stopped_debugger_before_actual_publication(){use rvn_core::engine::{DebugOperation,DebugProgress};use rvn_core::editor_debugger as w;let source=include_str!("fixtures/review_atomic_locale.rvn");let p=rvn_parser::parse_program(source).unwrap();let prepared=rvn_core::program::PreparedNarrative::from_parsed(p.clone()).unwrap();let pc=prepared.script.iter().position(|s|matches!(s,rvn_parser::Statement::Function{name,..}if name=="producer")).unwrap();let bp=w::Breakpoint{position:prepared.position(pc),site:Some(w::Site{function:Some("producer".into()),route:vec![w::Route::Statement(1)],depth:1,statement:true}),enabled:true};let mut e=Engine::from_program_with_persistent_and_text_context(p.clone(),Host::default(),16,HashMap::new(),TextRuntimeContext::new(Default::default(),Some(one_locale()))).unwrap();e.enter_label("start").unwrap();let mut session=e.debug_begin(DebugOperation::Statement,vec![bp]).unwrap();assert_eq!(e.debug_step(&mut session,w::StepKind::Continue).unwrap().0,DebugProgress::Stopped);let prior=e.locale.as_ref().unwrap().revision_stamp();let receipt=e.locale.as_ref().unwrap().portable_receipt().unwrap();e.locale=Some(one_locale());assert_ne!(e.locale.as_ref().unwrap().revision_stamp(),prior);assert_eq!(e.locale.as_ref().unwrap().portable_receipt().unwrap(),receipt);let before=frozen(&e);let result=e.debug_step(&mut session,w::StepKind::Continue);println!("SESSION real resume accepted={} result={:?} batches_before={} batches_after={} committed_result={:?}",result.is_ok(),result.as_ref().map(|value|value.0),before.3,e.renderer.batches.lock().unwrap().len(),e.state.vars.get("result"));assert!(result.is_err(),"stopped session must reject replaced live locale authority before publication even if content/generation equal");assert_eq!(frozen(&e),before);}

// Private0367 exclusive guard contracts; these are Core CPU checks, not PCM.
#[test]
fn exclusive_guard_rejects_foreign_snapshot_before_any_core_or_locale_publication(){
    let p=program();let mut live=engine(&p,"en");let before=frozen(&live);
    let prepared=live.prepare_save15_with_locale(&p,save(&p),Some(locale("fr"))).unwrap();
    let epoch=prepared.render_batch().runtime_epoch;let instance=live.instance_id();
    let guard=live.begin_prepared_save15_commit(prepared).unwrap();
    let snapshot=rvn_core::audio_checkpoint::AudioCheckpointSnapshot{instance_id:instance+1,runtime_epoch:epoch,rows:vec![],music:vec![]};
    assert!(guard.commit_with_audio_checkpoint(snapshot).is_err());assert_eq!(frozen(&live),before);
}
#[test]
fn exclusive_guard_rejects_wrong_epoch_snapshot_before_any_publication(){
    let p=program();let mut live=engine(&p,"en");let before=frozen(&live);
    let prepared=live.prepare_save15_with_locale(&p,save(&p),Some(locale("fr"))).unwrap();
    let epoch=prepared.render_batch().runtime_epoch;let instance=live.instance_id();
    let guard=live.begin_prepared_save15_commit(prepared).unwrap();
    let snapshot=rvn_core::audio_checkpoint::AudioCheckpointSnapshot{instance_id:instance,runtime_epoch:epoch+1,rows:vec![],music:vec![]};
    assert!(guard.commit_with_audio_checkpoint(snapshot).is_err());assert_eq!(frozen(&live),before);
}
#[test]
fn exclusive_guard_preflights_renderer_before_snapshot_and_publishes_exact_candidate(){
    let p=program();let mut live=engine(&p,"en");let before=frozen(&live);
    let prepared=live.prepare_save15_with_locale(&p,save(&p),Some(locale("fr"))).unwrap();
    live.renderer.reject_audio.store(true,Ordering::SeqCst);
    assert!(live.begin_prepared_save15_commit(prepared).is_err());assert_eq!(frozen(&live),before);
    live.renderer.reject_audio.store(false,Ordering::SeqCst);
    let prepared=live.prepare_save15_with_locale(&p,save(&p),Some(locale("fr"))).unwrap();
    let epoch=prepared.render_batch().runtime_epoch;let instance=live.instance_id();
    assert!(rvn_core::audio_playback::describe(&prepared.render_batch().state,epoch).unwrap().is_empty());
    assert!(rvn_core::audio_checkpoint::music_views(&prepared.render_batch().state,epoch).unwrap().is_empty());
    let guard=live.begin_prepared_save15_commit(prepared).unwrap();
    // This actual candidate has no audio descriptors; an empty snapshot is exact.
    let snapshot=rvn_core::audio_checkpoint::AudioCheckpointSnapshot{instance_id:instance,runtime_epoch:epoch,rows:vec![],music:vec![]};
    guard.commit_with_audio_checkpoint(snapshot).unwrap();assert_eq!(live.runtime_epoch(),epoch);
    assert_eq!(live.locale.as_ref().unwrap().current_lang(),"fr");assert_eq!(live.renderer.batches.lock().unwrap().len(),before.3+1);
}
#[test]
fn authenticated_warming_rejects_forged_program_without_running_any_statement(){
    let p=program();let live=engine(&p,"en");let before=frozen(&live);let captured=save(&p);
    let state=live.save15_state_for_audio_warming(&p,&captured).unwrap();assert_eq!(frozen(&live),before);assert!(state.vars.is_empty());
    let mut forged=p.clone();forged.sources[0].source.push(' ');
    assert!(live.save15_state_for_audio_warming(&forged,&captured).is_err());assert_eq!(frozen(&live),before);
}
