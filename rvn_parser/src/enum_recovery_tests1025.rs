use super::*;
use crate::enum_asset::EnumCommand;
const A: &str = "17411a3e431e4c9388b78a2b9825b130";
const B: &str = "797679920f7142be99d9d6fe5553ca34";
fn value(uuid: &str, name: &str) -> EnumAsset { EnumAsset::new(EnumAssetId::parse(uuid).unwrap(), name.into()).unwrap() }
#[test]
fn enum_recovery1025_empty_unpublished_roundtrip_has_no_fake_saved_wire_or_ordinal_identity() {
    let record = EnumRecoveryRecord1025::unpublished("assets/enums/Alpha.rvnenum".into(), value(A, "Alpha")).unwrap();
    assert_eq!(record.saved_wire(), None); assert_eq!(record.baseline(), &EnumRecoveryBaseline1025::Unpublished);
    let wire = record.encode().unwrap(); assert_eq!(EnumRecoveryRecord1025::decode(wire.as_bytes()).unwrap(), record);
    assert_eq!(record.asset_id().to_hex(), A); assert!(record.current().members().is_empty());
}
#[test]
fn enum_recovery1025_published_baseline_preserves_exact_saved_wire_and_current_edits() {
    let saved = value(A, "Alpha"); let exact_saved = format!(" \n{}\n", saved.encode().unwrap());
    let current = saved.prepare(EnumCommand::DescribeAsset("Current dirty value".into())).unwrap().after().clone();
    let record = EnumRecoveryRecord1025::published("assets/Alpha.rvnenum".into(), current.clone(), exact_saved.clone()).unwrap();
    let decoded = EnumRecoveryRecord1025::decode(record.encode().unwrap().as_bytes()).unwrap();
    assert_eq!(decoded.saved_wire(), Some(exact_saved.as_str())); assert_eq!(decoded.current(), &current);
}
#[test]
fn enum_recovery1025_rejects_wrong_name_escape_suffix_and_foreign_saved_identity() {
    for path in ["../Alpha.rvnenum", "assets/Other.rvnenum", "assets/Alpha.json", "C:/Alpha.rvnenum"] {
        assert!(EnumRecoveryRecord1025::unpublished(path.into(), value(A, "Alpha")).is_err());
    }
    assert!(EnumRecoveryRecord1025::published("assets/Alpha.rvnenum".into(), value(A, "Alpha"), value(B, "Alpha").encode().unwrap()).is_err());
}
#[test]
fn enum_recovery1025_rejects_unknown_versions_fields_and_duplicate_keys() {
    let record = EnumRecoveryRecord1025::unpublished("assets/Alpha.rvnenum".into(), value(A, "Alpha")).unwrap();
    let wire = record.encode().unwrap();
    for bad in [wire.replacen("\"version\":1", "\"version\":2", 1), wire.replacen("\"version\":1", "\"version\":1,\"extra\":0", 1), wire.replacen("\"version\":1", "\"version\":1,\"version\":1", 1)] {
        assert!(EnumRecoveryRecord1025::decode(bad.as_bytes()).is_err());
    }
}
#[test]
fn enum_recovery1025_rejects_noncanonical_current_but_not_valid_exact_saved_formatting() {
    let record = EnumRecoveryRecord1025::unpublished("assets/Alpha.rvnenum".into(), value(A, "Alpha")).unwrap();
    let mut wire: serde_json::Value = serde_json::from_str(&record.encode().unwrap()).unwrap();
    let current = wire["current_wire"].as_str().unwrap().to_owned(); wire["current_wire"] = format!(" {current}").into();
    assert!(EnumRecoveryRecord1025::decode(serde_json::to_string(&wire).unwrap().as_bytes()).is_err());
}
#[test]
fn enum_recovery1025_batch_rejects_duplicate_uuid_and_windows_locator_alias() {
    let a = EnumRecoveryRecord1025::unpublished("assets/Alpha.rvnenum".into(), value(A, "Alpha")).unwrap();
    let duplicate = EnumRecoveryRecord1025::unpublished("elsewhere/Alpha.rvnenum".into(), value(A, "Alpha")).unwrap();
    assert!(validate_enum_recovery1025(&[a.clone(), duplicate], true).is_err());
    let alias = EnumRecoveryRecord1025::unpublished("assets/alpha.rvnenum".into(), value(B, "alpha")).unwrap();
    assert!(validate_enum_recovery1025(&[a.clone(), alias.clone()], true).is_err());
    validate_enum_recovery1025(&[a, alias], false).unwrap();
}
#[test]
fn enum_recovery1025_baseline_does_not_accept_unknown_dirty_flag_substitute() {
    let record = EnumRecoveryRecord1025::unpublished("assets/Alpha.rvnenum".into(), value(A, "Alpha")).unwrap();
    let mut wire: serde_json::Value = serde_json::from_str(&record.encode().unwrap()).unwrap();
    for bad in [serde_json::json!({"kind":"unpublished","dirty":true}),
        serde_json::json!({"kind":"unpublished","saved_wire":null}),
        serde_json::json!({"kind":"published","saved_wire":null}),
        serde_json::json!({"kind":"published"})] {
        wire["baseline"] = bad;
        assert!(EnumRecoveryRecord1025::decode(serde_json::to_string(&wire).unwrap().as_bytes()).is_err());
    }
    let original = record.encode().unwrap();
    let duplicate = original.replacen("\"kind\":\"unpublished\"", "\"kind\":\"unpublished\",\"kind\":\"unpublished\"", 1);
    assert!(EnumRecoveryRecord1025::decode(duplicate.as_bytes()).is_err());
}
#[test]
fn enum_recovery1025_rejects_count_and_raw_byte_budget_before_accepting_batch() {
    let record = EnumRecoveryRecord1025::unpublished("assets/Alpha.rvnenum".into(), value(A, "Alpha")).unwrap();
    assert!(validate_enum_recovery1025(&vec![record; ENUM_RECOVERY_MAX_ASSETS1025 + 1], true).is_err());
    assert!(EnumRecoveryRecord1025::decode(&vec![b' '; MAX_RECORD_BYTES + 1]).is_err());
}
