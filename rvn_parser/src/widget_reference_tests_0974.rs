use super::*;
use crate::Value;

fn class() -> WidgetClassReference {
    WidgetClassReference::new("17411a3e431e4c9388b78a2b9825b130".into(), [7; 32]).unwrap()
}
fn live(alias: &str, incarnation: u64, instance: u64, epoch: u64) -> WidgetReference {
    WidgetReference::bind_admitted(
        PortableWidgetReference::new(class(), alias.into(), incarnation).unwrap(),
        instance,
        epoch,
    )
    .unwrap()
}
#[test]
fn widget0974_live_reference_roundtrip_is_inert_and_cannot_revive_runtime_provenance() {
    let value = Value::Widget(Box::new(live("first", 9, 8127, 9842)));
    let wire = serde_json::to_string(&value).unwrap();
    assert!(!wire.contains("8127") && !wire.contains("9842"));
    assert!(!wire.contains("binding") && !wire.contains("epoch") && !wire.contains("instance"));
    let restored: Value = serde_json::from_str(&wire).unwrap();
    let Value::Widget(reference) = &restored else {
        panic!("typed Widget wire required")
    };
    assert!(reference.is_inert());
    assert!(!reference.is_bound_to(8127, 9842));
    assert_ne!(restored, value);
}
#[test]
fn widget0974_reference_identity_distinguishes_engine_epoch_target_and_class() {
    let original = live("same", 3, 11, 4);
    assert_eq!(original, original.clone());
    assert_ne!(original, live("same", 4, 11, 4));
    assert_ne!(original, live("same", 3, 12, 4));
    assert_ne!(original, live("same", 3, 11, 5));
    let other =
        WidgetClassReference::new("3fc40c36f4e544ed82943e4c6fe9d87d".into(), [7; 32]).unwrap();
    assert_ne!(
        original,
        WidgetReference::bind_admitted(
            PortableWidgetReference::new(other, "same".into(), 3).unwrap(),
            11,
            4
        )
        .unwrap()
    );
}
#[test]
fn widget0974_portable_reference_rejects_unknown_fields_zero_incarnation_and_invalid_identity() {
    let wire = serde_json::to_value(live("same", 3, 11, 4)).unwrap();
    for (key, value) in [
        ("epoch", serde_json::json!(4)),
        ("entity", serde_json::json!(37)),
        ("binding", serde_json::json!({})),
    ] {
        let mut forged = wire.clone();
        forged.as_object_mut().unwrap().insert(key.into(), value);
        assert!(serde_json::from_value::<WidgetReference>(forged).is_err());
    }
    let mut zero = wire.clone();
    zero["reference"]["incarnation"] = serde_json::json!(0);
    assert!(serde_json::from_value::<WidgetReference>(zero).is_err());
    assert!(PortableWidgetReference::new(class(), "".into(), 1).is_err());
    assert!(PortableWidgetReference::new(class(), "x".repeat(129), 1).is_err());
    assert!(WidgetClassReference::new("not-a-uuid".into(), [0; 32]).is_err());
}
#[test]
fn widget0974_nested_reference_wire_is_typed_and_dictionaries_never_become_references() {
    let value = Value::List(vec![
        Value::Widget(Box::new(live("same", 3, 11, 4))),
        Value::Dict(std::collections::BTreeMap::from([(
            "Widget".into(),
            Value::Str("ordinary user data".into()),
        )])),
    ]);
    let restored: Value = serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
    let Value::List(items) = restored else {
        panic!("list")
    };
    assert!(matches!(&items[0],Value::Widget(reference) if reference.is_inert()));
    assert_eq!(
        items[1],
        Value::Dict(std::collections::BTreeMap::from([(
            "Widget".into(),
            Value::Str("ordinary user data".into())
        )]))
    );
}
#[test]
fn widget0974_class_reference_wire_retains_exact_uuid_and_receipt_without_runtime_binding() {
    let value = Value::WidgetClass(Box::new(Some(class())));
    assert_eq!(
        serde_json::from_str::<Value>(&serde_json::to_string(&value).unwrap()).unwrap(),
        value
    );
    assert_eq!(value.to_string(), "<WidgetClass>");
    assert_eq!(
        Value::Widget(Box::new(live("same", 3, 11, 4))).to_string(),
        "<WidgetReference>"
    );
}

#[test]
fn widget0974_null_reference_has_no_object_or_runtime_authority_and_roundtrips_as_typed_null() {
    let value = Value::Widget(Box::new(WidgetReference::null()));
    let wire = serde_json::to_value(&value).unwrap();
    assert_eq!(wire, serde_json::json!({"Widget":{"state":"null"}}));
    assert_eq!(serde_json::from_value::<Value>(wire).unwrap(), value);
    let Value::Widget(reference) = value else {
        panic!("Widget")
    };
    assert!(reference.is_null());
    assert_eq!(
        reference.retained_bytes(),
        std::mem::size_of::<WidgetReference>()
    );
    assert!(!reference.is_bound_to(11, 4));
    assert!(reference.portable().is_none());
}

#[test]
fn widget0974_existing_value_wire_stays_byte_exact_without_widget_values() {
    let value = Value::Dict(std::collections::BTreeMap::from([(
        "nested".into(),
        Value::List(vec![
            Value::Bool(true),
            Value::Int(2),
            Value::Float(3.5),
            Value::Str("source".into()),
        ]),
    )]));
    let wire =
        r#"{"Dict":{"nested":{"List":[{"Bool":true},{"Int":2},{"Float":3.5},{"Str":"source"}]}}}"#;
    assert_eq!(serde_json::to_string(&value).unwrap(), wire);
    assert_eq!(serde_json::from_str::<Value>(wire).unwrap(), value);
}
