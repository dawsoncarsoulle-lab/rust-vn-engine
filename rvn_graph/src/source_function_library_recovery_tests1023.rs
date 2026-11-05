use super::*;
use rvn_parser::function_library_asset1023::{FunctionLibraryMember,LibraryFunctionAccess};
use rvn_parser::parser::function_libraries1023::parse_library_scope;
fn id(value:u64)->FunctionLibraryId{FunctionLibraryId::parse(format!("{value:032x}")).unwrap()}
fn state(value:u64,path:&str,members:&[u64])->LibraryRecoveryState{
    let asset=FunctionLibraryAsset::new(id(value),members.iter().map(|graph_id|FunctionLibraryMember{graph_id:*graph_id,access:LibraryFunctionAccess::Public,pure:false}).collect()).unwrap();
    LibraryRecoveryState::new(path.into(),FunctionLibraryWire::from_asset(&asset).unwrap()).unwrap()
}
fn scope(value:u64,graph:u64)->FunctionLibraryScope{
    parse_library_scope(&format!("functions.library_v1(\"{value:032x}\"){{functions.member_v1(\"{graph}\",[]){{function shared()->(){{return}}}}}}")).unwrap()
}
#[test]
fn source_library_recovery1023_empty_birth_uses_real_codec_and_exact_existing_file_plan_without_seeding_graph(){
    let after=state(1,"assets/shared.rvnlibrary",&[]);
    let change=FunctionLibraryRecovery::new(id(1),None,Some(after.clone())).unwrap();
    let restored=FunctionLibraryRecovery::decode(&change.encode().unwrap()).unwrap();
    assert_eq!(restored,change);assert!(change.validate_source_scopes(&[],&[]).is_ok());
    let candidate=validate_set(&[],&[change.clone()]).unwrap();assert_eq!(candidate.len(),1);assert_eq!(candidate[0].state(),&after);
    let files=publication_files(&[change]).unwrap();assert_eq!(files.len(),1);
    assert_eq!(files[0].path,after.locator());assert_eq!(files[0].before,None);assert_eq!(files[0].after.as_deref(),Some(after.wire().as_str()));
    assert!(after.wire().asset().unwrap().members().is_empty());
}
#[test]
fn source_library_recovery1023_move_keeps_original_uuid_and_graph_scope_with_two_exact_cas_file_plans(){
    let before=state(1,"assets/old.rvnlibrary",&[41]);let after=state(1,"assets/nested/new.rvnlibrary",&[41]);
    let baseline=FunctionLibraryBaseline::new(id(1),before.clone()).unwrap();
    let change=FunctionLibraryRecovery::new(id(1),Some(before.clone()),Some(after.clone())).unwrap();
    let actual=scope(1,41);change.validate_source_scopes(&[actual.clone()],&[actual]).unwrap();
    let candidate=validate_set(&[baseline],&[change.clone()]).unwrap();assert_eq!(candidate[0].id(),&id(1));assert_eq!(candidate[0].state(),&after);
    let files=publication_files(&[change]).unwrap();assert_eq!(files.len(),2);
    let removed=files.iter().find(|file|file.path==before.locator()).unwrap();
    assert_eq!(removed.before.as_deref(),Some(before.wire().as_str()));assert!(removed.after.is_none());
    let created=files.iter().find(|file|file.path==after.locator()).unwrap();assert!(created.before.is_none());assert_eq!(created.after.as_deref(),Some(after.wire().as_str()));
}
#[test]
fn source_library_recovery1023_wrong_original_bytes_owner_and_destination_collisions_are_refused_without_mutation(){
    let before=state(1,"assets/original.rvnlibrary",&[41]);let other=state(2,"assets/occupied.rvnlibrary",&[]);
    let baseline=vec![FunctionLibraryBaseline::new(id(1),before.clone()).unwrap(),FunctionLibraryBaseline::new(id(2),other.clone()).unwrap()];let original=baseline.clone();
    let wrong_before=FunctionLibraryRecovery::new(id(1),Some(state(1,"assets/original.rvnlibrary",&[42])),None).unwrap();assert!(validate_set(&baseline,&[wrong_before]).is_err());
    assert!(FunctionLibraryRecovery::new(id(2),Some(before.clone()),None).is_err());
    let occupied=FunctionLibraryRecovery::new(id(1),Some(before.clone()),Some(state(1,other.locator(),&[41]))).unwrap();assert!(validate_set(&baseline,&[occupied]).is_err());
    let a=FunctionLibraryRecovery::new(id(1),Some(before.clone()),Some(state(1,"assets/shared.rvnlibrary",&[41]))).unwrap();
    let b=FunctionLibraryRecovery::new(id(2),Some(other),Some(state(2,"assets/shared.rvnlibrary",&[]))).unwrap();
    assert!(publication_files(&[a.clone(),b.clone()]).is_err());assert!(validate_set(&baseline,&[a,b]).is_err());assert_eq!(baseline,original);
}
#[test]
fn source_library_recovery1023_continuation_preserves_original_before_and_explicit_net_zero_unpublished_removal(){
    let birth=FunctionLibraryRecovery::new(id(1),None,Some(state(1,"assets/new.rvnlibrary",&[]))).unwrap();
    let undone=FunctionLibraryRecovery::new(id(1),None,None).unwrap();
    validate_continuation(&[birth.clone()],&[undone.clone()]).unwrap();
    assert!(validate_set(&[],&[undone.clone()]).unwrap().is_empty());assert!(publication_files(&[undone.clone()]).unwrap().is_empty());
    assert!(FunctionLibraryRecovery::decode(&undone.encode().unwrap()).unwrap().validate_source_scopes(&[],&[]).is_ok());
    assert!(validate_continuation(&[birth.clone()],&[]).is_err(),"unrelated writer cannot drop the typed participant");
    let rebased=FunctionLibraryRecovery::new(id(1),Some(state(1,"assets/new.rvnlibrary",&[])),None).unwrap();
    assert!(validate_continuation(&[birth.clone()],&[rebased]).is_err());
    assert!(validate_continuation(&[birth.clone()],&[birth.clone(),birth]).is_err());
}
#[test]
fn source_library_recovery1023_strict_versions_paths_and_embedded_codec_cannot_downgrade_or_change_owner(){
    let change=FunctionLibraryRecovery::new(id(1),None,Some(state(1,"assets/new.rvnlibrary",&[]))).unwrap();
    let mut wire=serde_json::to_value(&change).unwrap();wire["version"]=serde_json::json!(2);wire["after"]=serde_json::json!("not parsed as state");
    assert!(FunctionLibraryRecovery::decode(&serde_json::to_vec(&wire).unwrap()).unwrap_err().contains("Unsupported"));
    for path in ["../escape.rvnlibrary","/absolute.rvnlibrary","C:/outside.rvnlibrary","assets/foreign.rvnwidget","assets//empty.rvnlibrary"]{
        assert!(LibraryRecoveryState::new(path.into(),state(1,"assets/a.rvnlibrary",&[]).wire().clone()).is_err());
    }
    for altered in [r#"{"version":2,"library":"00000000000000000000000000000001","members":[]}"#,r#"{"version":1,"library":"00000000000000000000000000000001","members":[],"unknown":0}"#]{assert!(FunctionLibraryWire::read(altered.into()).is_err());}
    let mut wire=serde_json::to_value(&change).unwrap();wire["id"]=serde_json::json!(id(2));assert!(FunctionLibraryRecovery::decode(&serde_json::to_vec(&wire).unwrap()).is_err());
    let mut wire=serde_json::to_value(&change).unwrap();wire["unexpected"]=serde_json::json!(true);assert!(FunctionLibraryRecovery::decode(&serde_json::to_vec(&wire).unwrap()).is_err());
}
#[test]
fn source_library_recovery1023_real_source_membership_is_separate_and_exact_never_a_copied_body(){
    let before=state(1,"assets/a.rvnlibrary",&[41]);let change=FunctionLibraryRecovery::new(id(1),Some(before),None).unwrap();
    let actual=scope(1,41);change.validate_source_scopes(&[actual.clone()],&[]).unwrap();
    assert!(change.validate_source_scopes(&[scope(1,42)],&[]).is_err());
    assert!(change.validate_source_scopes(&[actual.clone(),actual.clone()],&[]).is_err());
    assert!(change.validate_source_scopes(&[],&[]).is_err());
    assert!(change.validate_source_scopes(&[actual.clone()],&[actual]).is_err(),"deleting metadata while keeping actual member scope is not recovery");
    let bytes=change.encode().unwrap();let text=String::from_utf8(bytes).unwrap();assert!(!text.contains("function shared"));
}
#[test]
fn source_library_recovery1023_exact_original_wire_formatting_survives_roundtrip_and_remains_the_cas_baseline(){
    let raw="{\n  \"version\":1, \"library\":\"00000000000000000000000000000001\",\n\"members\":[]\n}\n";
    let before=LibraryRecoveryState::new("assets/raw.rvnlibrary".into(),FunctionLibraryWire::read(raw.into()).unwrap()).unwrap();
    let change=FunctionLibraryRecovery::new(id(1),Some(before.clone()),Some(state(1,"assets/raw.rvnlibrary",&[]))).unwrap();
    let decoded=FunctionLibraryRecovery::decode(&change.encode().unwrap()).unwrap();assert_eq!(decoded.before().unwrap().wire().as_str(),raw);
    let files=publication_files(&[decoded.clone()]).unwrap();assert_eq!(files[0].before.as_deref(),Some(raw));
    let alternate=FunctionLibraryBaseline::new(id(1),state(1,"assets/raw.rvnlibrary",&[])).unwrap();
    assert!(validate_set(&[alternate],&[decoded]).is_err(),"semantic metadata equality cannot replace exact original CAS bytes");
    validate_set(&[FunctionLibraryBaseline::new(id(1),before).unwrap()],&[change]).unwrap();
}
