use rvn_core::{Engine, TextRuntimeContext, Renderer, renderer::SceneRenderBatch, locale::{LocaleCatalog, LocaleManager}};
use rvn_parser::{Hotspot, ParsedProgram, Position, Transition, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};

#[derive(Clone, Default)]
struct Host { batches: Arc<Mutex<Vec<SceneRenderBatch>>>, reject: Arc<AtomicBool> }
impl Renderer for Host {
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

#[test]
fn historical_save15_capture_omits_native_optional_data_and_preserves_old_restore(){
    let p=program();let save=save(&p);let wire=save.encode().unwrap();assert!(!wire.contains("native_media_checkpoint"));assert!(rvn_core::save::consumer_checkpoint::Save15::decode(&wire).unwrap().native_media_checkpoint.is_none());
    let mut live=engine(&p,"en");let plan=live.prepare_save15_with_locale(&p,save,Some(locale("fr"))).unwrap();assert!(plan.native_media_checkpoint().is_none());live.commit_prepared_save15(plan).unwrap();assert_eq!(live.locale.as_ref().unwrap().current_lang(),"fr");
}
#[test]
fn native_wire_data_without_host_admission_cannot_publish_even_an_empty_media_list(){
    let p=program();let mut save=save(&p);save.native_media_checkpoint=Some(Vec::new());let mut live=engine(&p,"en");let plan=live.prepare_save15_with_locale(&p,save,Some(locale("fr"))).unwrap();let before=frozen(&live);let message=match live.commit_prepared_save15(plan){Err(e)=>e.to_string(),Ok(_)=>panic!("Unadmitted native wire data published")};assert!(message.contains("native staged publication adapter"));assert_eq!(frozen(&live),before);
}
