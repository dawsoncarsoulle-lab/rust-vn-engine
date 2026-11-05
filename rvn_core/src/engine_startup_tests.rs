use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
#[derive(Default)]
struct Host {
    batches: Vec<crate::renderer::SceneRenderBatch>,
    reject: Arc<AtomicBool>,
}
impl Renderer for Host {
    fn validate_scene_batch(&self, _: &crate::renderer::SceneRenderBatch) -> Result<(), String> {
        if self.reject.load(Ordering::Relaxed) {
            Err("actual batch refused".into())
        } else {
            Ok(())
        }
    }
    fn publish_scene_batch(&mut self, batch: crate::renderer::SceneRenderBatch) {
        self.batches.push(batch);
    }
    fn set_background(&mut self, _: &str, _: &Transition) {
        panic!("startup must publish through the coherent batch")
    }
    fn show_sprite(
        &mut self,
        _: &str,
        _: Option<&str>,
        _: &Position,
        _: &Transition,
        _: Option<&SpriteState>,
    ) {
    }
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {
        0
    }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[rvn_parser::Hotspot]) -> usize {
        0
    }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}
fn folder(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let base = PathBuf::from(
        std::env::var("RVN_STARTUP_FIXTURE_DIR").expect("private startup fixture directory"),
    );
    let p = base.join(format!(
        "{name}-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn program(raw: &str) -> ParsedProgram {
    let p = folder("source").join("main.rvn");
    fs::write(&p, raw).unwrap();
    rvn_parser::parse_file_program_with_uses(p).unwrap()
}
fn startup(raw: &str) -> EngineStartup<Host> {
    Engine::prepare_debugger_startup(
        program(raw),
        Host::default(),
        8,
        HashMap::new(),
        Default::default(),
        None,
    )
    .unwrap()
}

#[test]
fn startup_nested_if_sfx_has_real_child_stop_and_exactly_one_condition_draw(){
    let raw="init {if random(1,9)>0 {set sampled=1 sfx.play(\"delayed.wav\")}else{set sampled=99}}\nscene label start\n\"Done\"\n";
    let mut s=startup(raw);let before=s.state().random;let mut expected=before;
    let Statement::Init{body}=&s.program.script[0]else{unreachable!()};let Statement::If{condition,..}=&body[0]else{unreachable!()};
    assert!(s.engine.functions.eval_bool_scoped_with_locale(condition,&HashMap::new(),&HashMap::new(),&mut expected,None).unwrap());
    let route=vec![wire::Route::Statement(0),wire::Route::Then,wire::Route::Statement(1)];
    let bp=s.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.route==route)).unwrap().breakpoint;
    let origin=bp.position.origin.clone();s.configure(vec![bp.clone()],vec![]).unwrap();
    for _ in 0..12{if s.snapshot().site.as_ref().is_some_and(|site|site.route==route){break;}s.step(StepKind::Continue).unwrap();}
    assert_eq!(s.snapshot().site.unwrap().route,route);assert_eq!(s.snapshot().position.pc,0);assert_eq!(s.state().random,expected);assert_eq!(s.state().vars["sampled"],Value::Int(1));
    assert!(crate::audio_playback::describe(s.state(),s.engine.runtime_epoch()).unwrap().is_empty());
    assert!(s.snapshot().origin.as_ref().is_some_and(|origin|origin.span.is_some()));let _=origin;
    finish(&mut s);assert_eq!(s.state().random,expected);assert_eq!(crate::audio_playback::describe(s.state(),s.engine.runtime_epoch()).unwrap().len(),1);
    let run=Engine::from_program(program(raw),Host::default(),8).unwrap();assert_eq!(run.state.vars["sampled"],Value::Int(1));assert_eq!(crate::audio_playback::describe(&run.state,run.runtime_epoch()).unwrap().len(),1);
}

#[test]
fn startup_nested_if_cold_save_resumes_actual_child_without_condition_replay(){
    let raw="init {if random(1,9)>0 {set first=7 if false {set cold=1}else{set selected=9} sfx.play(\"delayed.wav\")}}\nscene label start\n\"Done\"\n";
    let mut s=startup(raw);let route=vec![wire::Route::Statement(0),wire::Route::Then,wire::Route::Statement(1),wire::Route::Else,wire::Route::Statement(0)];
    let bp=s.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.route==route)).unwrap().breakpoint;
    s.configure(vec![bp],vec![]).unwrap();
    for _ in 0..24{if s.snapshot().site.as_ref().is_some_and(|site|site.route==route){break;}s.step(StepKind::Continue).unwrap();}
    assert_eq!(s.snapshot().site.unwrap().route,route);let random=s.state().random;assert_eq!(s.state().vars["first"],Value::Int(7));assert!(!s.state().vars.contains_key("cold"));
    let save=s.capture_save15(1,"nested genuine Init".into()).unwrap();let bytes=save.encode().unwrap();let decoded=crate::save::consumer_checkpoint::Save15::decode(&bytes).unwrap();
    let mut cold=startup(raw);cold.restore_save15(decoded).unwrap();assert_eq!(cold.snapshot().site.unwrap().route,route);assert_eq!(cold.state().random,random);
    finish(&mut cold);assert_eq!(cold.state().random,random);assert_eq!(cold.state().vars["selected"],Value::Int(9));assert!(!cold.state().vars.contains_key("cold"));
    let mut forged=save;forged.debugger.as_mut().unwrap().startup.as_mut().unwrap().nested=vec![wire::Route::Producer(0)];
    let mut target=startup(raw);let before=serde_json::to_value(target.state()).unwrap();assert!(target.restore_save15(forged).is_err());assert_eq!(serde_json::to_value(target.state()).unwrap(),before);
}

#[test]
fn startup_false_if_skips_effects_and_nested_source_indices_remain_frozen(){
    let mut s=startup("init {if false {sfx.play(\"cold.wav\")}else{set first=1 set second=2}}\nscene label start\n\"Done\"\n");
    finish(&mut s);assert_eq!(s.state().vars["first"],Value::Int(1));assert_eq!(s.state().vars["second"],Value::Int(2));assert!(crate::audio_playback::describe(s.state(),s.engine.runtime_epoch()).unwrap().is_empty());
}
fn finish(s: &mut EngineStartup<Host>) {
    for _ in 0..256 {
        if s.is_complete() {
            return;
        }
        s.step(StepKind::Continue).unwrap();
    }
    panic!("startup did not complete")
}
fn producer(s: &EngineStartup<Host>, name: &str) -> wire::Breakpoint {
    let mut offset = 0;
    loop {
        let page = s.site_page(offset).unwrap();
        if let Some(e) = page.entries.iter().find(|e| {
            e.origin
                .as_ref()
                .and_then(|o| o.scheduled_step.as_ref())
                .is_some_and(|p| p.producer_identity == name)
        }) {
            return e.breakpoint.clone();
        }
        offset += page.entries.len();
        assert!(offset < page.total, "genuine producer not offered");
    }
}
const NATIVE:&str="init {flow.consume_v1(\"scope\",\"consumer\") {capture \"draw\"=random(1,9) capture \"sum\"=__rvn_captured_v1(\"draw\")+1 set boot=__rvn_captured_v1(\"sum\")}}\nscene label start\n\"Done\"\n";

#[test]
fn startup_admission_has_no_init_effect_and_statement_ranges_are_actual() {
    let mut s = startup("init {set boot=7}\nscene label start\n\"Done\"\n");
    assert!(s.state().vars.is_empty());
    assert!(s.engine.renderer.batches.is_empty());
    let snapshot = s.snapshot();
    let origin = snapshot.origin.unwrap();
    let raw = &s.program.sources[0].source;
    assert_eq!(&raw[origin.span.unwrap()], "set boot=7");
    assert!(snapshot.site.unwrap().statement);
    assert!(snapshot.position.pc < s.engine.script.len());
    finish(&mut s);
    assert_eq!(s.state().vars["boot"], Value::Int(7));
    assert_eq!(s.engine.renderer.batches.len(), 1);
    let mut game = s.into_engine().unwrap();
    assert!(matches!(
        game.step_until_interaction().unwrap(),
        Some(Interaction::Dialogue { .. })
    ));
}
#[test]
fn startup_uses_actual_inclusion_init_order_without_replaying_completed_roots() {
    let dir = folder("order");
    fs::write(dir.join("main.rvn"),"init {set order=1}\nuse \"child.rvn\"\ninit {set order=order*10+3}\nscene label start\n\"Done\"\n").unwrap();
    fs::write(dir.join("child.rvn"), "init {set order=order*10+2}\n").unwrap();
    let p = rvn_parser::parse_file_program_with_uses(dir.join("main.rvn")).unwrap();
    let mut s = Engine::prepare_debugger_startup(
        p,
        Host::default(),
        8,
        HashMap::new(),
        Default::default(),
        None,
    )
    .unwrap();
    assert_eq!(
        s.engine
            .startup
            .as_ref()
            .unwrap()
            .roots
            .iter()
            .map(|r| r.source_ordinal)
            .collect::<Vec<_>>(),
        [0, 1, 0]
    );
    finish(&mut s);
    assert_eq!(s.state().vars["order"], Value::Int(123));
    assert_eq!(s.engine.renderer.batches.len(), 3);
}
#[test]
fn startup_true_producer_stop_precedes_rng_and_continuation_consumes_one_draw() {
    let mut s = startup(NATIVE);
    s.engine.state.random = crate::random::RandomState::seeded(42);
    s.prepare_session().unwrap();
    let before = s.state().random;
    let bp = producer(&s, "sum");
    s.configure(vec![bp], Vec::new()).unwrap();
    assert_eq!(s.step(StepKind::Continue).unwrap(), DebugProgress::Stopped);
    assert_eq!(s.state().random, before);
    assert!(s.engine.renderer.batches.is_empty());
    let snapshot = s.snapshot();
    assert_eq!(
        snapshot
            .origin
            .unwrap()
            .scheduled_step
            .unwrap()
            .producer_identity,
        "sum"
    );
    let mut expected = before;
    let mut vars = HashMap::new();
    let values =
        FunctionLibrary::from_script(&rvn_parser::parse("set expected=random(1,9)+1").unwrap())
            .unwrap()
            .execute_with_random(
                &rvn_parser::parse("set expected=random(1,9)+1").unwrap(),
                &vars,
                &mut expected,
            )
            .unwrap();
    vars.extend(values);
    finish(&mut s);
    assert_eq!(s.state().random, expected);
    assert_eq!(s.state().vars["boot"], vars["expected"]);
    assert_eq!(s.engine.renderer.batches.len(), 1);
}
#[test]
fn startup_renderer_refusal_keeps_state_rng_cursor_and_publication_unchanged() {
    let mut s = startup(NATIVE);
    let before = serde_json::to_value(s.state()).unwrap();
    let cursor = s.engine.portable_startup();
    s.engine.renderer.reject.store(true, Ordering::Relaxed);
    assert!(s.step(StepKind::Continue).is_err());
    assert_eq!(serde_json::to_value(s.state()).unwrap(), before);
    assert_eq!(s.engine.portable_startup(), cursor);
    assert!(s.engine.renderer.batches.is_empty());
}
#[test]
fn startup_source_and_overlay_forgery_are_refused_before_construction() {
    let p = program(NATIVE);
    let map = crate::debugger_provenance::SourceMap::from_program(&p).unwrap();
    let overlay = map.clone().staged_overlay(&map, 382).unwrap();
    assert!(Engine::prepare_debugger_startup(
        p.clone(),
        Host::default(),
        8,
        HashMap::new(),
        Default::default(),
        Some((overlay, 383))
    )
    .is_err());
    let mut changed = p;
    changed.origins[0].span.range.start += 1;
    assert!(Engine::prepare_debugger_startup(
        changed,
        Host::default(),
        8,
        HashMap::new(),
        Default::default(),
        None
    )
    .is_err());
}
#[test]
fn startup_old_operation_and_other_owner_are_refused() {
    let mut s = startup("init {set a=1 set b=2}\nscene label start\n\"Done\"\n");
    let operation = s.engine.startup_operation().unwrap();
    let other = startup("init {set a=1 set b=2}\nscene label start\n\"Done\"\n");
    assert!(other.engine.validate_startup_operation(&operation).is_err());
    s.step(StepKind::Continue).unwrap();
    assert!(s.engine.validate_startup_operation(&operation).is_err());
    assert!(s.into_engine().is_err());
}
#[test]
fn startup_save15_cold_relocation_resumes_draw_without_replaying_completed_init() {
    let raw = format!("init {{set prefix=1}}\n{NATIVE}");
    let mut s = startup(&raw);
    s.step(StepKind::Continue).unwrap();
    assert_eq!(s.state().vars["prefix"], Value::Int(1));
    let bp = producer(&s, "sum");
    s.configure(vec![bp], Vec::new()).unwrap();
    assert_eq!(s.step(StepKind::Continue).unwrap(), DebugProgress::Stopped);
    let saved = s.capture_save15(1, "cold startup".into()).unwrap();
    let encoded = saved.encode().unwrap();
    let decoded = crate::save::consumer_checkpoint::Save15::decode(&encoded).unwrap();
    let mut cold = startup(&raw);
    assert!(cold.state().vars.is_empty());
    cold.restore_save15(decoded).unwrap();
    assert_eq!(cold.state().vars["prefix"], Value::Int(1));
    let source = cold.snapshot().origin.unwrap();
    assert_eq!(
        source.source.as_ref(),
        cold.program.origins[1].source.as_ref()
    );
    finish(&mut s);
    finish(&mut cold);
    assert_eq!(cold.state().random, s.state().random);
    assert_eq!(cold.state().vars, s.state().vars);
    assert_eq!(
        cold.engine.renderer.batches.len(),
        2,
        "restore batch plus remaining initializer only"
    );
}

#[test]
fn startup_normal_constructors_keep_their_existing_init_semantics_for_both_contexts() {
    for scene in ["scene label start", "label start"] {
        let raw = format!("init {{set boot=7}}\n{scene}\n\"Done\"\n");
        let p = program(&raw);
        let normal = Engine::from_program_with_debugger(p, Host::default(), 8).unwrap();
        assert_eq!(normal.state.vars["boot"], Value::Int(7));
        assert!(normal.startup.is_none());
        let mut deferred = startup(&raw);
        assert!(deferred.state().vars.is_empty());
        finish(&mut deferred);
        assert_eq!(deferred.state().vars, normal.state.vars);
        assert_eq!(deferred.state().pc, normal.state.pc);
    }
}
#[test]
fn startup_saved_ordinals_prefix_and_changed_source_refuse_atomically() {
    let mut s = startup(NATIVE);
    let bp = producer(&s, "sum");
    s.configure(vec![bp], Vec::new()).unwrap();
    s.step(StepKind::Continue).unwrap();
    let save = s.capture_save15(1, "guarded cursor".into()).unwrap();
    let mut cold = startup(NATIVE);
    let before = serde_json::to_value(cold.state()).unwrap();
    for field in ["root", "body", "source_ordinal", "definition_root_ordinal"] {
        let mut forged = save.clone();
        let progress = forged.debugger.as_mut().unwrap().startup.as_mut().unwrap();
        match field {
            "root" => progress.root = 999,
            "body" => progress.body = 999,
            "source_ordinal" => progress.source_ordinal = 999,
            _ => progress.definition_root_ordinal = 999,
        };
        assert!(cold.restore_save15(forged).is_err());
        assert_eq!(serde_json::to_value(cold.state()).unwrap(), before);
        assert!(cold.engine.renderer.batches.is_empty());
    }
    let mut value = serde_json::to_value(&save).unwrap();
    value["debugger"]["trace"]["pending"]["request"]["route_prefix"] =
        serde_json::json!([{"Statement":999}]);
    let forged: crate::save::consumer_checkpoint::Save15 = serde_json::from_value(value).unwrap();
    assert!(cold.restore_save15(forged).is_err());
    assert_eq!(serde_json::to_value(cold.state()).unwrap(), before);
    let mut changed = startup(&format!("// source changed with same AST\n{NATIVE}"));
    assert!(changed.restore_save15(save).is_err());
    assert!(changed.engine.renderer.batches.is_empty());
}
#[test]
fn startup_table_and_program_mutation_revoke_pending_source_before_effects() {
    let mut s = startup(NATIVE);
    let before = serde_json::to_value(s.state()).unwrap();
    let session = s.session.as_ref().unwrap();
    assert!(s.engine.accepts_debug_session(session));
    s.engine
        .text_resources_mut()
        .publish(crate::string_table::StringTableAsset {
            id: "asset:table".into(),
            namespace: "VN".into(),
            entries: Default::default(),
        })
        .unwrap();
    assert!(s.step(StepKind::Continue).is_err());
    assert_eq!(serde_json::to_value(s.state()).unwrap(), before);
    assert!(s.engine.renderer.batches.is_empty());
    let mut s = startup(NATIVE);
    let before = serde_json::to_value(s.state()).unwrap();
    s.engine.script.push(Statement::Return);
    assert!(s.step(StepKind::Continue).is_err());
    assert_eq!(serde_json::to_value(s.state()).unwrap(), before);
    assert!(s.engine.renderer.batches.is_empty());
}
#[test]
fn startup_actual_locale_generation_change_rejects_before_init_publication() {
    use crate::locale::{LocaleCatalog, LocaleManager, LocaleTable};
    let catalogs = HashMap::from([
        (
            "en".into(),
            LocaleCatalog::with_text_sources(
                LocaleTable {
                    lang: "en".into(),
                    strings: HashMap::from([("hello".into(), "Hello".into())]),
                },
                HashMap::new(),
            )
            .unwrap(),
        ),
        (
            "fr".into(),
            LocaleCatalog::with_text_sources(
                LocaleTable {
                    lang: "fr".into(),
                    strings: HashMap::from([("hello".into(), "Bonjour".into())]),
                },
                HashMap::new(),
            )
            .unwrap(),
        ),
    ]);
    let locale =
        LocaleManager::from_frozen_catalogs("en", "en", vec!["en".into(), "fr".into()], catalogs)
            .unwrap();
    let mut s = Engine::prepare_debugger_startup(
        program(NATIVE),
        Host::default(),
        8,
        HashMap::new(),
        TextRuntimeContext::new(Default::default(), Some(locale)),
        None,
    )
    .unwrap();
    let before = serde_json::to_value(s.state()).unwrap();
    s.engine
        .locale
        .as_mut()
        .unwrap()
        .set_language("fr")
        .unwrap();
    assert!(s.step(StepKind::Continue).is_err());
    assert_eq!(serde_json::to_value(s.state()).unwrap(), before);
    assert!(s.engine.renderer.batches.is_empty());
}
#[test]
fn startup_borrowed_budget_and_excessive_container_refuse_before_clone_or_initialization() {
    let mut p = program(NATIVE);
    p.sources[0].source = "x".repeat(crate::debugger_provenance::MAX_BYTES + 1);
    assert!(Engine::prepare_debugger_startup(
        p,
        Host::default(),
        8,
        HashMap::new(),
        Default::default(),
        None
    )
    .is_err());
    let mut p = program(NATIVE);
    let source = p.sources[0].clone();
    p.sources = vec![source; 100_001];
    assert!(Engine::prepare_debugger_startup(
        p,
        Host::default(),
        8,
        HashMap::new(),
        Default::default(),
        None
    )
    .is_err());
}
#[test]
fn startup_chosen_native_branch_producer_has_actual_nested_route_before_random() {
    let raw="init {flow.consume_v2(\"root\",\"branch\") {if true {flow.consume_v1(\"root\",\"child\") {capture \"draw\"=random(1,9) set result=__rvn_captured_v1(\"draw\")}}else{flow.consume_v1(\"root\",\"other\") {capture \"cold\"=random(1,9) set result=__rvn_captured_v1(\"cold\")}}}}\nscene label start\n\"Done\"\n";
    let mut s = startup(raw);
    let before = s.state().random;
    let bp = producer(&s, "draw");
    s.configure(vec![bp.clone()], Vec::new()).unwrap();
    assert_eq!(s.step(StepKind::Continue).unwrap(), DebugProgress::Stopped);
    assert_eq!(s.snapshot().site.unwrap().route, bp.site.unwrap().route);
    assert_eq!(
        s.snapshot()
            .origin
            .unwrap()
            .scheduled_step
            .unwrap()
            .producer_identity,
        "draw"
    );
    assert_eq!(s.state().random, before);
    assert!(s.engine.renderer.batches.is_empty());
    finish(&mut s);
    assert_eq!(s.engine.renderer.batches.len(), 1);
}

#[test]
fn startup_function_step_into_and_cold_resume_preserve_real_callee_and_one_random_draw() {
    let raw="function compute(p){local held=p+1 return held+random(1,9)}\ninit {set boot=compute(4)}\nscene label start\n\"Done\"\n";
    let mut s = startup(raw);
    s.engine.state.random = crate::random::RandomState::seeded(77);
    s.prepare_session().unwrap();
    let before = s.state().random;
    assert_eq!(s.step(StepKind::Into).unwrap(), DebugProgress::Stopped);
    let snapshot = s.snapshot();
    assert_eq!(
        snapshot.site.as_ref().unwrap().function.as_deref(),
        Some("compute")
    );
    assert_eq!(s.state().random, before);
    assert!(s.engine.renderer.batches.is_empty());
    let save = s.capture_save15(1, "callee Init".into()).unwrap();
    let mut cold = startup(raw);
    cold.restore_save15(save).unwrap();
    assert_eq!(
        cold.snapshot().site.unwrap().function.as_deref(),
        Some("compute")
    );
    finish(&mut s);
    finish(&mut cold);
    assert_eq!(cold.state().vars, s.state().vars);
    assert_eq!(cold.state().random, s.state().random);
    let mut expected = before;
    let result = FunctionLibrary::from_script(&rvn_parser::parse(raw).unwrap())
        .unwrap()
        .execute_with_random(
            &rvn_parser::parse("set boot=compute(4)").unwrap(),
            &HashMap::new(),
            &mut expected,
        )
        .unwrap();
    assert_eq!(s.state().vars, result);
    assert_eq!(s.state().random, expected);
}
#[test]
fn startup_native_chosen_branch_save15_hydrates_absolute_init_route_without_replaying_condition() {
    let raw="init {flow.consume_v2(\"root\",\"branch\") {capture \"condition\"=true if __rvn_captured_v1(\"condition\") {flow.consume_v1(\"root\",\"child\") {capture \"draw\"=random(1,9) capture \"after\"=__rvn_captured_v1(\"draw\")+1 set result=__rvn_captured_v1(\"after\")}}}}\nscene label start\n\"Done\"\n";
    let mut s = startup(raw);
    let bp = producer(&s, "after");
    s.configure(vec![bp], Vec::new()).unwrap();
    assert_eq!(s.step(StepKind::Continue).unwrap(), DebugProgress::Stopped);
    let route = s.snapshot().site.unwrap().route;
    let save = s.capture_save15(1, "branch Init".into()).unwrap();
    let mut cold = startup(raw);
    cold.restore_save15(save).unwrap();
    assert_eq!(cold.snapshot().site.unwrap().route, route);
    finish(&mut s);
    finish(&mut cold);
    assert_eq!(cold.state().vars, s.state().vars);
    assert_eq!(cold.state().random, s.state().random);
}
#[test]
fn startup_completed_game_save15_cold_restore_does_not_publish_init_again() {
    let raw = "init {set boot=1}\nscene label start\nset boot=boot+8\n\"Done\"\n";
    let mut s = startup(raw);
    finish(&mut s);
    let p = s.program.as_ref().clone();
    let mut game = s.into_engine().unwrap();
    game.enter_label("start").unwrap();
    game.step_until_interaction().unwrap();
    assert_eq!(game.state.vars["boot"], Value::Int(9));
    let save = game
        .capture_save15(&p, None, 1, "completed startup".into())
        .unwrap();
    let cold = startup(raw);
    assert!(cold.engine.renderer.batches.is_empty());
    let (mut restored, session) = cold.restore_completed_save15(save).unwrap();
    assert!(session.is_none());
    assert_eq!(restored.state.vars["boot"], Value::Int(9));
    assert_eq!(restored.renderer.batches.len(), 1);
    assert_eq!(
        restored.renderer.batches[0].state.vars["boot"],
        Value::Int(9)
    );
    assert!(restored.startup.is_none());
    assert!(matches!(
        restored.step_until_interaction().unwrap(),
        Some(Interaction::Dialogue { .. })
    ));
}
#[test]
fn startup_producer_free_child_has_genuine_statement_stop_and_cold_cursor() {
    let raw="init {flow.consume_v2(\"root\",\"branch\") {if true {flow.consume_v1(\"root\",\"child\") {set child=9}}}}\nscene label start\n\"Done\"\n";
    let mut s = startup(raw);
    let mut offset = 0;
    let bp = loop {
        let page = s.site_page(offset).unwrap();
        if let Some(entry) = page.entries.iter().find(|entry| {
            entry
                .origin
                .as_ref()
                .and_then(|origin| origin.span.as_ref())
                .is_some_and(|span| {
                    s.program.sources[0].source[span.clone()].starts_with("flow.consume_v1")
                })
        }) {
            break entry.breakpoint.clone();
        }
        offset += page.entries.len();
        assert!(offset < page.total);
    };
    s.configure(vec![bp.clone()], Vec::new()).unwrap();
    assert_eq!(s.step(StepKind::Continue).unwrap(), DebugProgress::Stopped);
    let snapshot = s.snapshot();
    assert_eq!(snapshot.site.unwrap().route, bp.site.unwrap().route);
    assert!(s.state().vars.is_empty());
    assert!(s.engine.renderer.batches.is_empty());
    let save = s.capture_save15(1, "actual child boundary".into()).unwrap();
    let mut cold = startup(raw);
    cold.restore_save15(save).unwrap();
    finish(&mut s);
    finish(&mut cold);
    assert_eq!(cold.state().vars, s.state().vars);
    assert_eq!(cold.state().vars["child"], Value::Int(9));
}
#[test]
fn startup_prepared_restore_guard_rejects_changed_source_and_renderer_before_commit() {
    let mut s = startup(NATIVE);
    let bp = producer(&s, "sum");
    s.configure(vec![bp], Vec::new()).unwrap();
    s.step(StepKind::Continue).unwrap();
    let save = s.capture_save15(1, "guarded startup".into()).unwrap();
    let mut cold = startup(NATIVE);
    let prepared = cold.prepare_save15_with_locale(save.clone(), None).unwrap();
    cold.engine
        .state
        .vars
        .insert("changed".into(), Value::Int(1));
    assert!(cold.begin_prepared_save15_commit(prepared).is_err());
    assert!(cold.engine.renderer.batches.is_empty());
    let mut cold = startup(NATIVE);
    let before = serde_json::to_value(cold.state()).unwrap();
    let prepared = cold.prepare_save15_with_locale(save.clone(), None).unwrap();
    cold.engine.renderer.reject.store(true, Ordering::Relaxed);
    assert!(cold.begin_prepared_save15_commit(prepared).is_err());
    assert_eq!(serde_json::to_value(cold.state()).unwrap(), before);
    assert!(cold.engine.renderer.batches.is_empty());
    let mut cold = startup(NATIVE);
    let prepared = cold.prepare_save15_with_locale(save, None).unwrap();
    let guard = cold.begin_prepared_save15_commit(prepared).unwrap();
    assert!(guard.render_batch().state.vars.is_empty());
    guard.commit();
    finish(&mut cold);
    finish(&mut s);
    assert_eq!(cold.state().vars, s.state().vars);
    assert_eq!(cold.state().random, s.state().random);
}

#[path="engine_startup_loop_tests.rs"]
mod loop_continuations;
#[path="engine_startup_native_loop_tests.rs"]
mod native_loop_continuations;
