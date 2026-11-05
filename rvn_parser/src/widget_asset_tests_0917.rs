use super::*;
const A: &str = "17411a3e431e4c9388b78a2b9825b130";
const B: &str = "797679920f7142be99d9d6fe5553ca34";
fn sample() -> WidgetBlueprintV1 {
    WidgetBlueprintV1 {
        uuid: A.into(),
        source: "scripts/main.rvn".into(),
        screen_graph: WidgetGraphRef16::from_u64(u64::MAX),
        members: vec![
            WidgetGraphRef16::from_u64(u64::MAX),
            WidgetGraphRef16::from_u64(9_007_199_254_740_993),
        ],
    }
}
#[test]
fn widget_codec0917_opaque_refs_preserve_high_bits_and_never_use_json_numbers() {
    for value in [0, 1, 9_007_199_254_740_993, u64::MAX] {
        let id = WidgetGraphRef16::from_u64(value);
        let encoded = serde_json::to_string(&id).unwrap();
        assert_eq!(encoded, format!("\"{value:016x}\""));
        assert_eq!(
            serde_json::from_str::<WidgetGraphRef16>(&encoded).unwrap(),
            id
        );
        assert_eq!(
            id.to_string().parse::<WidgetGraphRef16>().unwrap().get(),
            value
        );
    }
    for invalid in [
        "f",
        "00000000000000AF",
        "fffffffffffffffff",
        "-000000000000001",
        "000000000000000g",
        "000000000000000é",
    ] {
        assert!(WidgetGraphRef16::parse(invalid).is_err(), "{invalid}");
    }
    for invalid in ["0", "9007199254740993", "null", "{}", "[]"] {
        assert!(
            serde_json::from_str::<WidgetGraphRef16>(invalid).is_err(),
            "{invalid}"
        );
    }
}
#[test]
fn widget_codec0917_canonical_v1_is_byte_compatible_and_references_only() {
    let value = sample();
    let wire = value.encode().unwrap();
    let expected=format!("{{\n  \"version\": 1,\n  \"type\": \"rvn.widget-blueprint\",\n  \"uuid\": \"{A}\",\n  \"source\": \"scripts/main.rvn\",\n  \"screen_graph\": \"ffffffffffffffff\",\n  \"members\": [\n    \"ffffffffffffffff\",\n    \"0020000000000001\"\n  ]\n}}\n");
    assert_eq!(wire, expected);
    assert_eq!(WidgetBlueprintV1::decode(wire.as_bytes()).unwrap(), value);
    assert_eq!(
        WidgetBlueprintV1::decode(expected.as_bytes())
            .unwrap()
            .encode()
            .unwrap(),
        expected
    );
    let json: serde_json::Value = serde_json::from_str(&wire).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 6);
    for absent in [
        "tree",
        "nodes",
        "properties",
        "defaults",
        "private_state",
        "receiver",
    ] {
        assert!(json.get(absent).is_none());
    }
}
#[test]
fn widget_codec0917_strict_shape_version_uuid_and_duplicate_fields_fail_closed() {
    let wire = sample().encode().unwrap();
    let base: serde_json::Value = serde_json::from_str(&wire).unwrap();
    for (key, value) in [
        ("version", serde_json::json!(0)),
        ("version", serde_json::json!(2)),
        ("type", serde_json::json!("rvn.string-table")),
        ("screen_graph", serde_json::json!(12)),
        ("screen_graph", serde_json::json!("f")),
        ("screen_graph", serde_json::json!("00000000000000AF")),
        ("uuid", serde_json::json!("0".repeat(32))),
        ("uuid", serde_json::json!(A.to_uppercase())),
        ("source", serde_json::json!("../main.rvn")),
        ("nodes", serde_json::json!([])),
        ("version", serde_json::json!(1.5)),
    ] {
        let mut json = base.clone();
        json[key] = value;
        assert!(
            WidgetBlueprintV1::decode(&serde_json::to_vec(&json).unwrap()).is_err(),
            "{key}"
        );
    }
    for key in [
        "version",
        "type",
        "uuid",
        "source",
        "screen_graph",
        "members",
    ] {
        let mut json = base.clone();
        json.as_object_mut().unwrap().remove(key);
        assert!(
            WidgetBlueprintV1::decode(&serde_json::to_vec(&json).unwrap()).is_err(),
            "missing{key}"
        );
        let needle = format!("\"{key}\":");
        let doubled = wire.replacen(&needle, &format!("\"{key}\":{}, {needle}", base[key]), 1);
        assert!(
            WidgetBlueprintV1::decode(doubled.as_bytes()).is_err(),
            "duplicate{key}"
        );
    }
}
#[test]
fn widget_codec0917_group_and_byte_bounds_refuse_without_allocating_authority() {
    let mut value = sample();
    value.members.push(value.screen_graph);
    assert!(value.encode().is_err());
    value.members.clear();
    assert!(value.encode().is_err());
    value.members.push(WidgetGraphRef16::from_u64(42));
    assert!(value.encode().is_err());
    assert!(WidgetBlueprintV1::decode(&vec![b' '; MAX_FILE_BYTES + 1]).is_err());
    assert!(WidgetBlueprintV1::decode(&[0xff, 0xfe]).is_err());
    value.screen_graph = WidgetGraphRef16::from_u64(0);
    value.members = (0..110_000).map(WidgetGraphRef16::from_u64).collect();
    assert!(value.encode().unwrap_err().contains("byte budget"));
}
#[test]
fn widget_codec0917_lexical_paths_keep_the_existing_exact_contract() {
    for value in ["main.rvn", "scripts/main.rvn", "sous-dossier/écran.rvn"] {
        assert!(validate_relative_locator(value, Some("rvn")).is_ok());
    }
    for value in [
        "",
        "../main.rvn",
        "/main.rvn",
        "C:/main.rvn",
        "a\\main.rvn",
        "a/./main.rvn",
        "a//main.rvn",
        "main.rvn/",
        "main.rvn?",
        "a./main.rvn",
        "a /main.rvn",
        "main.RVN",
        "main.rvn#",
        "main\n.rvn",
    ] {
        assert!(
            validate_relative_locator(value, Some("rvn")).is_err(),
            "{value}"
        );
    }
    let deep = format!("{}main.rvn", "a/".repeat(64));
    assert!(validate_relative_locator(&deep, Some("rvn")).is_err());
    assert!(validate_relative_locator(&format!("{}.rvn", "a".repeat(4096)), Some("rvn")).is_err());
    // Lexical validation deliberately does not claim filesystem/link safety.
    assert!(validate_relative_locator("missing/main.rvn", Some("rvn")).is_ok());
}
#[test]
fn widget_codec0917_index_shape_identity_and_explicit_case_policy_are_shared() {
    assert!(
        validate_index(WidgetIndexInput::Absent, LocatorCase::Sensitive)
            .unwrap()
            .is_empty()
    );
    assert!(validate_index(WidgetIndexInput::NotTable, LocatorCase::Sensitive).is_err());
    assert!(validate_index(
        WidgetIndexInput::Table(&[(A, None)]),
        LocatorCase::Sensitive
    )
    .is_err());
    assert!(validate_index(
        WidgetIndexInput::Table(&[
            (A, Some("widgets/a.rvnwidget")),
            (A, Some("widgets/b.rvnwidget"))
        ]),
        LocatorCase::Sensitive
    )
    .is_err());
    let entries = [
        (A, Some("widgets/A.rvnwidget")),
        (B, Some("widgets/a.rvnwidget")),
    ];
    assert_eq!(
        validate_index(WidgetIndexInput::Table(&entries), LocatorCase::Sensitive)
            .unwrap()
            .len(),
        2
    );
    assert!(validate_index(WidgetIndexInput::Table(&entries), LocatorCase::Insensitive).is_err());
    assert!(validate_index(
        WidgetIndexInput::Table(&[(A, Some("../a.rvnwidget"))]),
        LocatorCase::Sensitive
    )
    .is_err());
    assert!(validate_index(
        WidgetIndexInput::Table(&[("bad", Some("a.rvnwidget"))]),
        LocatorCase::Sensitive
    )
    .is_err());
}
#[test]
fn widget_codec0917_cross_asset_graph_claims_refuse_without_graph_or_runtime_dependencies() {
    let one = [WidgetGraphRef16::from_u64(1), WidgetGraphRef16::from_u64(2)];
    let two = [WidgetGraphRef16::from_u64(3)];
    assert!(validate_group_memberships([(A, one.as_slice()), (B, two.as_slice())]).is_ok());
    assert!(validate_group_memberships([(A, one.as_slice()), (B, one.as_slice())]).is_err());
    let shape = WidgetBlueprintV1 {
        screen_graph: WidgetGraphRef16::from_u64(8675309),
        members: vec![WidgetGraphRef16::from_u64(8675309)],
        ..sample()
    };
    assert!(shape.encode().is_ok(),"An opaque authored reference is not evidence that its source graph exists; actual source binding is a separate required admission");
}
