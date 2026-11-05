use super::*;

#[test]
fn library_codec_byte_limit_and_future_version_precede_member_allocation() {
    let oversized = vec![b' '; MAX_FUNCTION_LIBRARY_BYTES + 1];
    assert!(FunctionLibraryAsset::decode(&oversized).unwrap_err().contains("byte limit"));
    let future = br#"{"version":2,"library":"11111111111111111111111111111111","members":[{"graph_id":"not-a-member"}]}"#;
    assert!(FunctionLibraryAsset::decode(future).unwrap_err().contains("Unsupported"));
    let current = br#"{"version":1,"library":"11111111111111111111111111111111","members":[{"graph_id":"not-a-member"}]}"#;
    assert!(FunctionLibraryAsset::decode(current).is_err());
}

#[test]
fn library_codec_accepts_exact_membership_boundary_and_rejects_one_more() {
    let id = FunctionLibraryId::parse("11111111111111111111111111111111").unwrap();
    let members = (1..=MAX_FUNCTION_LIBRARY_MEMBERS as u64)
        .map(|graph_id| FunctionLibraryMember { graph_id, access: LibraryFunctionAccess::Public, pure: false })
        .collect::<Vec<_>>();
    let asset = FunctionLibraryAsset::new(id.clone(), members.clone()).unwrap();
    let encoded = asset.encode().unwrap();
    assert_eq!(FunctionLibraryAsset::decode(&encoded).unwrap(), asset);
    let mut too_many = members;
    too_many.push(FunctionLibraryMember { graph_id: MAX_FUNCTION_LIBRARY_MEMBERS as u64 + 1, access: LibraryFunctionAccess::Private, pure: false });
    assert!(FunctionLibraryAsset::new(id, too_many.clone()).is_err());
    let oversized_wire = serde_json::json!({"version":1,"library":"11111111111111111111111111111111","members":too_many});
    let bytes = serde_json::to_vec(&oversized_wire).unwrap();
    assert!(bytes.len() < MAX_FUNCTION_LIBRARY_BYTES);
    assert!(FunctionLibraryAsset::decode(&bytes).unwrap_err().contains("sequence"));
}

#[test]
fn library_codec_strict_wire_and_identity_remain_exact() {
    let empty = br#"{"version":1,"library":"11111111111111111111111111111111","members":[]}"#;
    assert!(FunctionLibraryAsset::decode(empty).unwrap().members().is_empty());
    for wire in [
        r#"{"version":1,"library":"11111111111111111111111111111111","members":[],"extra":0}"#,
        r#"{"version":1,"version":1,"library":"11111111111111111111111111111111","members":[]}"#,
        r#"{"version":1,"library":"1111111111111111111111111111111A","members":[]}"#,
        r#"{"version":1,"library":"00000000000000000000000000000000","members":[]}"#,
    ] { assert!(FunctionLibraryAsset::decode(wire.as_bytes()).is_err()); }
}
