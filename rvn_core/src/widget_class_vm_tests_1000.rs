//! Uses the existing source-admitted Fixture, VM, renderer and Save15 adapters.
use super::*;
use crate::eval::FunctionLibrary;
use crate::random::RandomState;

fn expr(source: &str) -> rvn_parser::Expr {
    let script = rvn_parser::parse(&format!("set result={source}")).unwrap();
    let [Statement::SetVar { value, .. }] = script.as_slice() else { panic!("expression fixture"); };
    value.clone()
}
fn query(uuid: &str) -> rvn_parser::Expr { expr(&format!("__rvn_widget_class_v1(\"{uuid}\")")) }
fn null() -> Value { Value::WidgetClass(Box::new(None)) }

#[test]
fn class_vm1000_actual_engine_catalog_reaches_pure_vm_without_instantiation_or_rng_or_publication() {
    let fixture = Fixture::new(false);
    let engine = fixture.engine();
    assert!(Arc::ptr_eq(&engine.functions.widget_classes, &engine.widget_classes));
    let before = serde_json::to_vec(&engine.state).unwrap();
    let batches = engine.renderer.batches.lock().unwrap().len();
    let mut random = RandomState::seeded(1000);
    let expected = crate::widget_class_getter_0981::resolve_class_data(&fixture.classes, Some(A)).unwrap();
    let value = engine.functions.eval_with_random(&query(A), &Default::default(), &mut random).unwrap();
    assert_eq!(value, Value::WidgetClass(Box::new(expected)));
    assert_eq!(engine.functions.eval(&expr("__rvn_widget_class_none_v1()"), &Default::default()).unwrap(), null());
    assert_eq!(random, RandomState::seeded(1000));
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    assert_eq!(engine.renderer.batches.lock().unwrap().len(), batches);
}

#[test]
fn class_vm1000_missing_catalog_unknown_uuid_and_wrong_type_never_fall_back_to_null_and_rng_rolls_back() {
    let fixture = Fixture::new(false);
    let engine = fixture.engine();
    let empty = FunctionLibrary::default();
    assert!(empty.eval(&query(A), &Default::default()).is_err());
    assert_eq!(empty.eval(&expr("__rvn_widget_class_none_v1()"), &Default::default()).unwrap(), null());
    for source in ["__rvn_widget_class_v1(\"missing\")", "__rvn_widget_class_v1(3)", "__rvn_widget_class_v1()", "__rvn_widget_class_none_v1(1)", "__rvn_widget_class_none_v1()+\"x\"", "not __rvn_widget_class_none_v1()", "__rvn_widget_create_v1(__rvn_widget_class_none_v1(),[])"] {
        assert!(engine.functions.eval(&expr(source), &Default::default()).is_err(), "{source}");
    }
    let mut random = RandomState::seeded(41);
    assert!(engine.functions.eval_with_random(&expr("[random(1,99),__rvn_widget_class_v1(\"missing\")]"), &Default::default(), &mut random).is_err());
    assert_eq!(random, RandomState::seeded(41));
}

#[test]
fn class_vm1000_nonnull_historical_json_is_identical_and_typed_null_roundtrips_through_value_and_savevalue() {
    let fixture = Fixture::new(false);
    let class = crate::widget_class_getter_0981::resolve_class_data(&fixture.classes, Some(A)).unwrap().unwrap();
    let legacy_value_json = serde_json::json!({"WidgetClass": &class});
    let value = Value::WidgetClass(Box::new(Some(class.clone())));
    assert_eq!(serde_json::to_value(&value).unwrap(), legacy_value_json);
    assert_eq!(serde_json::from_value::<Value>(legacy_value_json).unwrap(), value);
    let legacy_save_json = serde_json::json!({"type":"WidgetClass","v":&class});
    assert_eq!(serde_json::to_value(crate::save::SaveValue::from(&value)).unwrap(), legacy_save_json);
    assert_eq!(Value::from(serde_json::from_value::<crate::save::SaveValue>(legacy_save_json).unwrap()), value);
    let null = null();
    assert_eq!(serde_json::to_value(&null).unwrap(), serde_json::json!({"WidgetClass":null}));
    assert_eq!(serde_json::from_slice::<Value>(&serde_json::to_vec(&null).unwrap()).unwrap(), null);
    assert_eq!(Value::from(serde_json::from_slice::<crate::save::SaveValue>(&serde_json::to_vec(&crate::save::SaveValue::from(&null)).unwrap()).unwrap()), null);
    assert_ne!(null, Value::Widget(Box::new(rvn_parser::widget_reference::WidgetReference::null())));
    assert_ne!(null, Value::Str("null".into()));
    // 24k nulls fit the node/depth/individual collection bounds, but their
    // retained boxes exceed1MiB on supported32/64-bit targets.
    let row = Value::List(vec![null.clone();80]);
    let oversized = Value::List(vec![row;300]);
    assert!(crate::value_limits::value_work(&oversized).is_err(), "typed null boxes must obey the same retained-value byte bound");
    // Decoding class metadata is inert, not a grant. Its current validation
    // boundary must reject an invalid identity; serde is not that admission.
    let inert = serde_json::from_value::<Value>(serde_json::json!({"WidgetClass":{"uuid":"","receipt":vec![0;32]}})).unwrap();
    assert!(crate::value_limits::value_work(&inert).is_err());
    assert!(crate::widget_class_getter_0981::validate_class_values(&fixture.classes, &inert).is_err());
    assert!(serde_json::from_value::<Value>(serde_json::json!({"WidgetClass":"not a class"})).is_err());
}

#[test]
fn class_vm1000_actual_save15_cold_restore_keeps_nullable_class_data_and_current_catalog_without_init_replay() {
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let value = engine.functions.eval(&query(A), &Default::default()).unwrap();
    let links = Value::List(vec![null(), value.clone()]);
    engine.state.vars.insert("classes".into(), links.clone());
    engine.state.vars.insert("hits".into(), Value::Int(17));
    let save = engine.capture_save15(&fixture.program, None, 1, "nullable class VM".into()).unwrap();
    let wire = save.encode().unwrap();
    let save = crate::save::consumer_checkpoint::Save15::decode(&wire).unwrap();
    let startup = Engine::prepare_widget_class_startup(fixture.program.clone(), fixture.classes.clone(), Host::default(), 16, Default::default(), Default::default()).unwrap();
    let (cold, debugger) = startup.restore_completed_save15(save).unwrap();
    assert!(debugger.is_none());
    assert!(Arc::ptr_eq(&cold.functions.widget_classes, &cold.widget_classes));
    assert_eq!(cold.state.vars["classes"], links);
    assert_eq!(cold.state.vars["hits"], Value::Int(17), "completed save must not replay Init");
    assert_eq!(cold.functions.eval(&query(A), &Default::default()).unwrap(), value);
    let mut receipt = fixture.classes.class(A).unwrap().identity().receipt;
    receipt[0] ^= 1;
    let forged = rvn_parser::widget_reference::WidgetClassReference::new(A.into(), receipt).unwrap();
    engine.state.vars.insert("classes".into(), Value::List(vec![null(), Value::WidgetClass(Box::new(Some(forged)))]));
    let before = serde_json::to_vec(&engine.state).unwrap();
    let published = engine.renderer.batches.lock().unwrap().len();
    assert!(engine.capture_save15(&fixture.program, None, 2, "forged after null".into()).is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    assert_eq!(engine.renderer.batches.lock().unwrap().len(), published);
}

#[test]
fn class_vm1000_debugger_uses_same_catalog_and_refuses_stale_retained_class_receipt_without_rng_commit() {
    use crate::eval::debugger_eval::{Probe, PortableTrace};
    let fixture = Fixture::new(false);
    let engine = fixture.engine();
    let expression = expr(&format!("[random(1,99),__rvn_widget_class_v1(\"{A}\"),__rvn_widget_class_none_v1()]"));
    let library = &engine.functions;
    let mut expected_random = RandomState::seeded(31);
    let expected = library.eval_with_random(&expression, &Default::default(), &mut expected_random).unwrap();
    let probe = Probe::shared();
    let mut debug = library.clone(); debug.debug_probe = Some(probe.clone());
    let mut random = RandomState::seeded(31);
    assert!(debug.eval_with_random(&expression, &Default::default(), &mut random).is_err());
    assert_eq!(random, RandomState::seeded(31));
    let mut continuation = Probe::pending(&probe, library).unwrap().unwrap();
    // The existing portable adapter rebinds the process-local stamp only after
    // actual class/source admission; a newly allocated Arc of the same catalog
    // is valid and must not be rejected by pointer identity.
    let mut trace = Probe::portable_trace(&probe, &continuation).unwrap();
    let admission = crate::widget_reference_values_0974::WidgetValueAdmission::new(&engine.state, engine.widget_classes.clone(), engine.instance_id(), engine.runtime_epoch()).unwrap();
    trace.export_widget_values(&admission).unwrap();
    let wire = serde_json::to_vec(&trace).unwrap();
    assert!(!String::from_utf8_lossy(&wire).contains("widget_classes"), "the catalog context is not a wire field or a decoded grant");
    let cold_engine = fixture.engine();
    assert!(!Arc::ptr_eq(&engine.widget_classes, &cold_engine.widget_classes));
    let mut restored: PortableTrace = serde_json::from_slice(&wire).unwrap();
    let cold_admission = crate::widget_reference_values_0974::WidgetValueAdmission::new(&cold_engine.state, cold_engine.widget_classes.clone(), cold_engine.instance_id(), cold_engine.runtime_epoch()).unwrap();
    restored.admit_widget_values(Arc::new(cold_admission)).unwrap();
    let cold_probe = Probe::shared();
    let mut cold_debug = cold_engine.functions.clone(); cold_debug.debug_probe = Some(cold_probe.clone());
    let mut cold_random = RandomState::seeded(31);
    assert!(cold_debug.eval_with_random(&expression, &Default::default(), &mut cold_random).is_err());
    let mut cold_continuation = Probe::pending(&cold_probe, &cold_engine.functions).unwrap().unwrap();
    cold_continuation.restore_portable(&restored, None).unwrap();
    for _ in 0..1024 { if cold_continuation.done() { break; } cold_continuation.step().unwrap(); }
    assert!(cold_continuation.done());
    Probe::finish(&cold_probe, &cold_continuation).unwrap(); Probe::rewind(&cold_probe).unwrap();
    assert_eq!(cold_debug.eval_with_random(&expression, &Default::default(), &mut cold_random).unwrap(), expected);
    assert_eq!(cold_random, expected_random);
    for _ in 0..1024 { if continuation.done() { break; } continuation.step().unwrap(); }
    assert!(continuation.done());
    Probe::finish(&probe, &continuation).unwrap(); Probe::rewind(&probe).unwrap();
    assert_eq!(debug.eval_with_random(&expression, &Default::default(), &mut random).unwrap(), expected);
    assert_eq!(random, expected_random);
    let mut same_catalog = library.clone();
    same_catalog.widget_classes = Arc::new((*cold_engine.widget_classes).clone());
    assert!(!Arc::ptr_eq(&library.widget_classes, &same_catalog.widget_classes));
    same_catalog.debug_probe = Some(probe.clone());
    Probe::rewind(&probe).unwrap();
    let mut same_random = RandomState::seeded(31);
    assert_eq!(same_catalog.eval_with_random(&expression, &Default::default(), &mut same_random).unwrap(), expected);
    assert_eq!(same_random, expected_random);
    let changed = Fixture::new(true);
    assert_ne!(fixture.classes.class(A).unwrap().identity().receipt, changed.classes.class(A).unwrap().identity().receipt);
    let mut current = library.clone();
    current.widget_classes = changed.engine().widget_classes.clone();
    current.debug_probe = Some(probe.clone());
    Probe::rewind(&probe).unwrap();
    let mut refused_random = RandomState::seeded(31);
    assert!(current.eval_with_random(&expression, &Default::default(), &mut refused_random).is_err());
    assert_eq!(refused_random, RandomState::seeded(31));
    // A failed lookup also depends on its catalog, despite retaining no class
    // value. Do not replay that old failure after the real class becomes admitted.
    let absent = FunctionLibrary::default();
    let negative_probe = Probe::shared();
    let mut absent_debug = absent.clone(); absent_debug.debug_probe = Some(negative_probe.clone());
    let requested = query(A);
    assert!(absent_debug.eval(&requested, &Default::default()).is_err());
    let mut failed = Probe::pending(&negative_probe, &absent).unwrap().unwrap();
    for _ in 0..1024 { if failed.done() { break; } let _ = failed.step(); }
    assert!(failed.done());
    Probe::finish(&negative_probe, &failed).unwrap(); Probe::rewind(&negative_probe).unwrap();
    assert!(library.eval(&requested, &Default::default()).is_ok(), "the current catalog now resolves the same lookup");
    let mut present_debug = absent.clone();
    present_debug.widget_classes = library.widget_classes.clone();
    present_debug.debug_probe = Some(negative_probe.clone());
    assert!(present_debug.eval(&requested, &Default::default()).is_err());
    assert!(Probe::pending(&negative_probe, library).is_err(), "catalog replacement must mark the trace stale, not return a previously cached missing-class error");
}

#[test]
fn class_vm1000_class_lookup_survives_real_widget_retirement_without_rebinding_a_widget_object() {
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let before = engine.functions.eval(&query(A), &Default::default()).unwrap();
    let handle = engine.create_widget(A, "vm_class_retired", vec![Value::Str("allowed".into())]).unwrap();
    let object = engine.widget_reference_value(&handle).unwrap();
    engine.destroy_widget(handle).unwrap();
    assert!(engine.widget_reference_handle(&object).is_err());
    assert_eq!(engine.functions.eval(&query(A), &Default::default()).unwrap(), before);
}
