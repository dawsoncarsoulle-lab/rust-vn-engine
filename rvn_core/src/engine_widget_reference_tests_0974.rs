//! Uses the existing Graph-admitted Fixture/Host, not a substitute factory.
use super::*;

fn first(engine: &mut Engine<Host>, name: &str) -> Value {
    let handle = engine
        .create_widget(A, name, vec![Value::Str("allowed".into())])
        .unwrap();
    engine.widget_reference_value(&handle).unwrap()
}
fn load_reference_save(
    engine: &mut Engine<Host>,
    fixture: &Fixture,
    session: Option<&DebugSession>,
    through_wire: bool,
) -> Option<DebugSession> {
    let save = engine
        .capture_save15(&fixture.program, session, 1, "typed reference".into())
        .unwrap();
    let save = if through_wire {
        crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()
    } else {
        save
    };
    engine.restore_save15(&fixture.program, save).unwrap()
}
#[test]
fn reference0974_two_real_instances_and_nested_links_hydrate_at_the_new_epoch_without_alias_rebinding(
) {
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let a = first(&mut engine, "first");
    let b = first(&mut engine, "second");
    assert_ne!(a, b);
    engine.state.vars.insert("first_ref".into(), a.clone());
    let class = engine.widget_classes.class(B).unwrap().identity();
    let class_value = Value::WidgetClass(Box::new(Some(
        rvn_parser::widget_reference::WidgetClassReference::new(class.uuid.clone(), class.receipt)
            .unwrap(),
    )));
    engine
        .state
        .vars
        .insert("uninstantiated_class".into(), class_value.clone());
    engine.state.vars.insert(
        "links".into(),
        Value::List(vec![
            a.clone(),
            b.clone(),
            Value::Dict(std::collections::BTreeMap::from([(
                "same".into(),
                a.clone(),
            )])),
        ]),
    );
    let before_epoch = engine.runtime_epoch();
    assert!(load_reference_save(&mut engine, &fixture, None, true).is_none());
    assert!(engine.runtime_epoch() > before_epoch);
    assert_eq!(engine.state.vars["uninstantiated_class"], class_value);
    assert!(engine.widget_reference_handle(&a).is_err());
    assert!(engine.widget_reference_handle(&b).is_err());
    let restored = engine.state.vars["first_ref"].clone();
    let handle = engine.widget_reference_handle(&restored).unwrap();
    assert_eq!(handle.alias(), "first");
    let Value::List(links) = &engine.state.vars["links"] else {
        panic!("links")
    };
    assert_eq!(links[0], restored);
    assert_ne!(links[0], links[1]);
    let Value::Dict(repeated) = &links[2] else {
        panic!("nested links")
    };
    assert_eq!(repeated["same"], restored);
    assert_eq!(
        engine.widget_reference_handle(&links[1]).unwrap().alias(),
        "second"
    );
}
#[test]
fn reference0974_same_alias_replacement_foreign_engine_and_portable_data_cannot_revive_a_reference()
{
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let original = first(&mut engine, "same");
    let inert: Value = serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
    assert!(engine.widget_reference_handle(&inert).is_err());
    let mut other = fixture.engine();
    let _ = first(&mut other, "same");
    assert!(other.widget_reference_handle(&original).is_err());
    other.state.vars.insert("foreign".into(), original.clone());
    let before = serde_json::to_vec(&other.state).unwrap();
    assert!(other
        .capture_save15(&fixture.program, None, 1, "foreign".into())
        .is_err());
    assert_eq!(serde_json::to_vec(&other.state).unwrap(), before);
    let handle = engine.widget_reference_handle(&original).unwrap();
    engine.destroy_widget(handle).unwrap();
    let replacement = first(&mut engine, "same");
    assert_ne!(replacement, original);
    assert!(engine.widget_reference_handle(&original).is_err());
    assert!(engine.widget_reference_handle(&replacement).is_ok());
}
#[test]
fn reference0974_forged_class_or_logical_incarnation_refuses_restore_before_any_publication() {
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let value = first(&mut engine, "first");
    engine.state.vars.insert("reference".into(), value);
    let save = engine
        .capture_save15(&fixture.program, None, 1, "guard".into())
        .unwrap();
    let original: serde_json::Value = serde_json::from_str(&save.encode().unwrap()).unwrap();
    for mutation in 0..3 {
        let mut forged = original.clone();
        let reference = &mut forged["story"]["base"]["vars"]["reference"]["Widget"]["reference"];
        match mutation {
            0 => reference["incarnation"] = serde_json::json!(u64::MAX),
            1 => {
                reference["class"]["uuid"] = serde_json::json!(B);
                reference["class"]["receipt"] = serde_json::to_value(
                    engine.widget_classes.class(B).unwrap().identity().receipt,
                )
                .unwrap();
            }
            _ => {
                let byte = reference["class"]["receipt"][0].as_u64().unwrap();
                reference["class"]["receipt"][0] = serde_json::json!(byte ^ 1);
            }
        }
        let forged = crate::save::consumer_checkpoint::Save15::decode(
            &serde_json::to_string(&forged).unwrap(),
        )
        .unwrap();
        let before = serde_json::to_vec(&engine.state).unwrap();
        let epoch = engine.runtime_epoch();
        let batches = engine.renderer.batches.lock().unwrap().len();
        assert!(engine.restore_save15(&fixture.program, forged).is_err());
        assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
        assert_eq!(engine.runtime_epoch(), epoch);
        assert_eq!(engine.renderer.batches.lock().unwrap().len(), batches);
    }
}
#[test]
fn reference0974_destroyed_and_replaced_links_capture_as_typed_null_without_adopting_the_new_alias()
{
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let value = first(&mut engine, "first");
    let handle = engine.widget_reference_handle(&value).unwrap();
    engine.destroy_widget(handle).unwrap();
    let replacement = first(&mut engine, "first");
    engine.state.vars.insert("dead".into(), value.clone());
    engine
        .state
        .vars
        .insert("replacement".into(), replacement.clone());
    let before = serde_json::to_vec(&engine.state).unwrap();
    let save = engine
        .capture_save15(&fixture.program, None, 1, "destroyed".into())
        .unwrap();
    assert_eq!(
        serde_json::to_vec(&engine.state).unwrap(),
        before,
        "capture projects only its disposable snapshot"
    );
    let wire = save.encode().unwrap();
    let data: serde_json::Value = serde_json::from_str(&wire).unwrap();
    assert_eq!(
        data["story"]["base"]["vars"]["dead"],
        serde_json::json!({"Widget":{"state":"null"}})
    );
    let save = crate::save::consumer_checkpoint::Save15::decode(&wire).unwrap();
    engine.restore_save15(&fixture.program, save).unwrap();
    assert!(matches!(&engine.state.vars["dead"],Value::Widget(reference)if reference.is_null()));
    assert!(engine
        .widget_reference_handle(&engine.state.vars["dead"])
        .is_err());
    assert!(engine
        .widget_reference_handle(&engine.state.vars["replacement"])
        .is_ok());
    assert!(engine.widget_reference_handle(&value).is_err());
    assert!(engine.widget_reference_handle(&replacement).is_err());
}
#[test]
fn reference0974_native_stopped_handler_preserves_references_and_resumes_once_after_actual_save15()
{
    let fixture = Fixture::new(true);
    let mut engine = fixture.engine();
    let value = first(&mut engine, "typed");
    let handle = engine.widget_reference_handle(&value).unwrap();
    engine.add_widget_to_viewport(&handle, false, 0).unwrap();
    engine.state.vars.insert("reference".into(), value.clone());
    let event = input(
        &engine,
        "typed",
        rvn_ui::programmable::ScreenEventKind::Click,
    );
    let mut session = engine
        .debug_begin(DebugOperation::Input(event), vec![])
        .unwrap();
    let mut stopped = false;
    for _ in 0..128 {
        let (progress, _) = engine
            .debug_step(&mut session, crate::editor_debugger::StepKind::Into)
            .unwrap();
        assert_ne!(progress, DebugProgress::Complete);
        if session.snapshot().frames.iter().any(|frame| {
            frame.bindings.iter().any(|(name, value)| {
                name == "roll"
                    && matches!(
                        value,
                        crate::editor_debugger::WatchValue::Value(Value::Int(_))
                    )
            })
        }) {
            stopped = true;
            break;
        }
    }
    assert!(stopped);
    assert_eq!(engine.state.vars["hits"], Value::Int(0));
    let mut session = load_reference_save(&mut engine, &fixture, Some(&session), false).unwrap();
    assert!(engine.widget_reference_handle(&value).is_err());
    assert!(engine
        .widget_reference_handle(&engine.state.vars["reference"])
        .is_ok());
    let mut completed = false;
    for _ in 0..256 {
        let (progress, _) = engine
            .debug_step(&mut session, crate::editor_debugger::StepKind::Continue)
            .unwrap();
        if progress == DebugProgress::Complete {
            completed = true;
            break;
        }
    }
    assert!(completed);
    assert_eq!(engine.state.vars["hits"], Value::Int(1));
    assert!(engine
        .debug_step(&mut session, crate::editor_debugger::StepKind::Continue)
        .is_err());
    assert_eq!(engine.state.vars["hits"], Value::Int(1));
}
#[test]
fn reference0974_old_save_and_persistent_writers_refuse_before_replacing_any_file() {
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let value = first(&mut engine, "first");
    engine.state.vars.insert("reference".into(), value.clone());
    assert!(crate::save::SaveData::try_from_state(
        &engine.state,
        1,
        "legacy".into(),
        "main.rvn".into()
    )
    .is_err());
    let manager =
        crate::persistent::PersistentDataManager::new(fixture.root.join("persistent-test"))
            .unwrap();
    let mut data = crate::persistent::PersistentData::default();
    manager.save(&data).unwrap();
    let bytes = fs::read(manager.path()).unwrap();
    data.script_vars
        .insert("persistent.reference".into(), Value::List(vec![value]));
    assert!(manager.save(&data).is_err());
    assert_eq!(fs::read(manager.path()).unwrap(), bytes);
}
#[test]
fn reference0974_runtime_values_reject_implicit_text_json_and_boolean_coercions_but_keep_identity_equality(
) {
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let value = first(&mut engine, "first");
    let vars = HashMap::from([("reference".into(), value.clone())]);
    let library = crate::eval::FunctionLibrary::default();
    let expression = |source: &str| {
        let script = rvn_parser::parse(&format!("set answer={source}")).unwrap();
        let Statement::SetVar { value, .. } = &script[0] else {
            panic!("assignment")
        };
        value.clone()
    };
    assert_eq!(
        library
            .eval(&expression("reference == reference"), &vars)
            .unwrap(),
        Value::Bool(true)
    );
    assert!(library
        .eval(&expression("\"prefix\" + reference"), &vars)
        .is_err());
    let containers = HashMap::from([("items".into(), Value::List(vec![value.clone()]))]);
    assert!(library
        .eval(&expression("\"prefix\" + items"), &containers)
        .is_err());
    let repeated = Value::List(vec![Value::List(vec![value.clone(); 4096]); 4]);
    assert!(crate::value_limits::value_work(&repeated).is_err());
    let nulls = Value::List(vec![
        Value::List(vec![
            Value::Widget(Box::new(
                rvn_parser::widget_reference::WidgetReference::null()
            ));
            4096
        ]);
        4
    ]);
    assert!(
        crate::value_limits::value_work(&nulls).is_err(),
        "boxed null references retain storage too"
    );
    assert!(crate::eval::eval_bool(&expression("reference"), &vars).is_err());
    assert!(crate::ui::value_to_json(&Value::List(vec![value])).is_err());
}

#[test]
fn reference0974_actual_history_replacement_rebinds_its_saved_object_and_revokes_both_external_incarnations(
) {
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let original = first(&mut engine, "same");
    engine
        .state
        .vars
        .insert("reference".into(), original.clone());
    let snapshot = engine.state.clone();
    let original_handle = engine.widget_reference_handle(&original).unwrap();
    engine.destroy_widget(original_handle).unwrap();
    let replacement = first(&mut engine, "same");
    engine.history.clear();
    engine.history.push(snapshot, None);
    let epoch = engine.runtime_epoch();
    assert!(engine.rollback());
    assert!(engine.runtime_epoch() > epoch);
    assert!(engine.widget_reference_handle(&original).is_err());
    assert!(engine.widget_reference_handle(&replacement).is_err());
    let restored = &engine.state.vars["reference"];
    assert!(engine.widget_reference_handle(restored).is_ok());
    let (Value::Widget(before), Value::Widget(after)) = (&original, restored) else {
        panic!("typed references")
    };
    assert_eq!(before.portable(), after.portable());
    assert_ne!(before, after);
}

#[test]
fn reference0974_foreign_runtime_reference_in_a_history_candidate_refuses_without_replacing_the_live_owner(
) {
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let _local = first(&mut engine, "same");
    let mut other = fixture.engine();
    let foreign = first(&mut other, "same");
    let mut candidate = engine.state.clone();
    candidate.vars.insert("foreign".into(), foreign);
    engine.history.clear();
    engine.history.push(candidate, None);
    let before = serde_json::to_vec(&engine.state).unwrap();
    let epoch = engine.runtime_epoch();
    let batches = engine.renderer.batches.lock().unwrap().len();
    assert!(!engine.rollback());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    assert_eq!(engine.runtime_epoch(), epoch);
    assert_eq!(engine.renderer.batches.lock().unwrap().len(), batches);
}

#[test]
fn reference0974_cold_engine_readmits_saved_logical_links_without_importing_old_lineage_or_input_authority(
) {
    let fixture = Fixture::new(false);
    let mut old = fixture.engine();
    let value = first(&mut old, "logical");
    old.state.vars.insert("reference".into(), value.clone());
    let save = old
        .capture_save15(&fixture.program, None, 1, "cold".into())
        .unwrap();
    let wire = save.encode().unwrap();
    let save = crate::save::consumer_checkpoint::Save15::decode(&wire).unwrap();
    let mut cold = fixture.engine();
    assert_ne!(old.instance_id(), cold.instance_id());
    cold.restore_save15(&fixture.program, save).unwrap();
    assert!(cold.widget_reference_handle(&value).is_err());
    let restored = &cold.state.vars["reference"];
    let handle = cold.widget_reference_handle(restored).unwrap();
    assert_eq!(handle.alias(), "logical");
    assert!(
        cold.capture_ui_input_source("logical").is_none(),
        "logical detached reference does not grant viewport/input authority"
    );
    let (Value::Widget(before), Value::Widget(after)) = (&value, restored) else {
        panic!("typed references")
    };
    assert_eq!(before.portable(), after.portable());
    assert_ne!(before, after);
}

#[test]
fn reference0974_prepared_restore_source_guard_detects_runtime_binding_tamper_even_when_portable_bytes_match(
) {
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let value = first(&mut engine, "same");
    engine.state.vars.insert("reference".into(), value.clone());
    let save = engine
        .capture_save15(&fixture.program, None, 1, "source guard".into())
        .unwrap();
    let prepared = engine
        .prepare_save15_with_locale(&fixture.program, save, None)
        .unwrap();
    let inert: Value = serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_vec(&value).unwrap(),
        serde_json::to_vec(&inert).unwrap()
    );
    assert_ne!(value, inert);
    engine.state.vars.insert("reference".into(), inert);
    let before = serde_json::to_vec(&engine.state).unwrap();
    let epoch = engine.runtime_epoch();
    let batches = engine.renderer.batches.lock().unwrap().len();
    assert!(engine.commit_prepared_save15(prepared).is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    assert_eq!(engine.runtime_epoch(), epoch);
    assert_eq!(engine.renderer.batches.lock().unwrap().len(), batches);
}
