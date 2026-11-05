use super::*;
fn owner() -> FunctionLibraryId { FunctionLibraryId::parse("11111111111111111111111111111111").unwrap() }
#[test]
fn library_asset1023_empty_factory_membership_cold_decode_never_seeds_a_function() {
    let library = FunctionLibraryAsset::new(owner(), Vec::new()).unwrap();
    let bytes = library.encode().unwrap();
    let cold = FunctionLibraryAsset::decode(&bytes).unwrap();
    assert_eq!(cold, library);
    assert!(cold.members().is_empty());
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value.as_object().unwrap().keys().cloned().collect::<BTreeSet<_>>(), ["version", "library", "members"].into_iter().map(String::from).collect());
}
#[test]
fn library_asset1023_uuid_and_existing_graph_identity_survive_roundtrip_without_name_or_body_copies() {
    let library = FunctionLibraryAsset::new(owner(), vec![FunctionLibraryMember { graph_id: 47, access: LibraryFunctionAccess::Public, pure: false }]).unwrap();
    let cold = FunctionLibraryAsset::decode(&library.encode().unwrap()).unwrap();
    assert_eq!(cold.id(), &owner()); assert_eq!(cold.members()[0].graph_id, 47);
    let reference = QualifiedLibraryFunction { library: cold.id().clone(), graph_id: 47 };
    reference.validate().unwrap();
    assert!(QualifiedLibraryFunction { graph_id: 0, ..reference }.validate().is_err());
    assert!(FunctionLibraryId::parse("same display name is not a library ID").is_err());
}
#[test]
fn library_asset1023_strict_schema_rejects_future_unknown_duplicate_fields_and_invalid_members_without_downgrade() {
    let library = FunctionLibraryAsset::new(owner(), vec![FunctionLibraryMember { graph_id: 47, access: LibraryFunctionAccess::Public, pure: false }]).unwrap();
    let bytes = library.encode().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for key in ["version", "future_field"] {
        let mut next = value.clone(); next[key] = serde_json::json!(99);
        assert!(FunctionLibraryAsset::decode(&serde_json::to_vec(&next).unwrap()).is_err());
    }
    let mut duplicate = String::from_utf8(bytes).unwrap(); duplicate.insert_str(1, "\"version\":1,");
    assert!(FunctionLibraryAsset::decode(duplicate.as_bytes()).is_err());
    let member = library.members()[0].clone();
    assert!(FunctionLibraryAsset::new(owner(), vec![member.clone(), member]).is_err());
    assert!(FunctionLibraryAsset::new(owner(), vec![FunctionLibraryMember { graph_id: 0, access: LibraryFunctionAccess::Public, pure: false }]).is_err());
}
