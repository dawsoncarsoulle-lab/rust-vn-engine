//! Actual Graph/source-admitted class catalog, existing Host and factory only.
use super::*;
use crate::widget_class_getter_0981::{resolve_class_data, validate_class_data};

#[test]
fn class_getter0981_resolves_actual_source_identity_receipt_without_instantiating_or_mutating_engine(
) {
    let fixture = Fixture::new(false);
    let engine = fixture.engine();
    let state = serde_json::to_vec(&engine.state).unwrap();
    let published = engine.renderer.batches.lock().unwrap().len();
    let value = resolve_class_data(engine.widget_classes(), Some(A))
        .unwrap()
        .unwrap();
    let actual = engine.widget_classes().class(A).unwrap().identity();
    assert_eq!(value.uuid(), actual.uuid.as_str());
    assert_eq!(value.receipt(), &actual.receipt);
    assert_eq!(
        resolve_class_data(engine.widget_classes(), Some(A))
            .unwrap()
            .as_ref(),
        Some(&value)
    );
    validate_class_data(engine.widget_classes(), Some(&value)).unwrap();
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), state);
    assert_eq!(engine.renderer.batches.lock().unwrap().len(), published);
    assert!(engine
        .state
        .ui
        .screens
        .iter()
        .all(|screen| screen.widget_class.is_none()));
}
#[test]
fn class_getter0981_nullable_class_is_explicit_and_unknown_or_string_uuid_never_becomes_null() {
    let fixture = Fixture::new(false);
    let class: Option<rvn_parser::widget_reference::WidgetClassReference> =
        resolve_class_data(&fixture.classes, None).unwrap();
    assert!(class.is_none());
    validate_class_data(&fixture.classes, class.as_ref()).unwrap();
    assert!(resolve_class_data(&fixture.classes, Some("missing")).is_err());
    assert!(resolve_class_data(&fixture.classes, Some("")).is_err());
    assert!(resolve_class_data(&fixture.classes, Some("null")).is_err());
    assert!(resolve_class_data(&fixture.classes, Some(A))
        .unwrap()
        .is_some());
    // No Value::Str conversion or WidgetReference::null is used by this port.
}
#[test]
fn class_getter0981_rejects_forged_or_old_same_uuid_source_receipt_but_preserves_typed_wire() {
    let fixture = Fixture::new(false);
    let actual = resolve_class_data(&fixture.classes, Some(A))
        .unwrap()
        .unwrap();
    let wire = serde_json::to_vec(&actual).unwrap();
    let decoded: rvn_parser::widget_reference::WidgetClassReference =
        serde_json::from_slice(&wire).unwrap();
    validate_class_data(&fixture.classes, Some(&decoded)).unwrap();
    let mut receipt = *actual.receipt();
    receipt[0] ^= 1;
    let forged =
        rvn_parser::widget_reference::WidgetClassReference::new(A.into(), receipt).unwrap();
    assert!(validate_class_data(&fixture.classes, Some(&forged)).is_err());
    let changed = Fixture::new(true);
    let changed_class = resolve_class_data(&changed.classes, Some(A))
        .unwrap()
        .unwrap();
    assert_eq!(changed_class.uuid(), actual.uuid());
    assert_ne!(changed_class.receipt(), actual.receipt());
    assert!(validate_class_data(&changed.classes, Some(&actual)).is_err());
    validate_class_data(&changed.classes, Some(&changed_class)).unwrap();
}
#[test]
fn class_getter0981_class_data_survives_instance_retirement_without_rebinding_an_object() {
    let fixture = Fixture::new(false);
    let mut engine = fixture.engine();
    let class = resolve_class_data(engine.widget_classes(), Some(A))
        .unwrap()
        .unwrap();
    let handle = engine
        .create_widget(A, "getters_test", vec![Value::Str("allowed".into())])
        .unwrap();
    engine.destroy_widget(handle).unwrap();
    assert_eq!(
        resolve_class_data(engine.widget_classes(), Some(A))
            .unwrap()
            .as_ref(),
        Some(&class)
    );
    validate_class_data(engine.widget_classes(), Some(&class)).unwrap();
    assert!(engine
        .state
        .ui
        .screens
        .iter()
        .all(|screen| screen.name != "getters_test"));
}
