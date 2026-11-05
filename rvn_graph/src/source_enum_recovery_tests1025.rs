use super::*;
use rvn_parser::enum_asset::{EnumAsset,EnumAssetId,EnumCommand};
const A:&str="17411a3e431e4c9388b78a2b9825b130";
const B:&str="797679920f7142be99d9d6fe5553ca34";
fn record(id:&str,name:&str)->Record{Record::unpublished(format!("assets/{name}.rvnenum"),EnumAsset::new(EnumAssetId::parse(id).unwrap(),name.into()).unwrap()).unwrap()}
#[test]
fn enum_recovery1025_changes_are_exact_targeted_and_never_replace_unmentioned_peers(){
    let a=record(A,"Alpha");let b=record(B,"Beta");
    let newer=Record::unpublished(a.locator().into(),a.current().prepare(EnumCommand::DescribeAsset("new".into())).unwrap().after().clone()).unwrap();
    let original=vec![a.clone(),b.clone()];
    let updated=apply_changes1025(&original,&[EnumRecoveryChange1025::Upsert{before:Some(a.clone()),after:newer.clone()}],true).unwrap();
    assert_eq!(updated,vec![newer.clone(),b.clone()]);
    assert!(apply_changes1025(&updated,&[EnumRecoveryChange1025::Remove{before:a.clone()}],true).is_err());
    assert!(apply_changes1025(&original,&[EnumRecoveryChange1025::Upsert{before:None,after:a.clone()}],true).is_err());
    assert!(apply_changes1025(&original,&[EnumRecoveryChange1025::Remove{before:a.clone()},EnumRecoveryChange1025::Upsert{before:None,after:a}],true).is_err());
    assert_eq!(apply_changes1025(&updated,&[EnumRecoveryChange1025::Remove{before:newer}],true).unwrap(),vec![b]);
}
#[test]
fn enum_recovery1025_recovery_only_cannot_forge_published_baseline_ack(){
    let a=record(A,"Alpha");
    let published=Record::published(a.locator().into(),a.current().clone(),a.current().encode().unwrap()).unwrap();
    assert!(apply_changes1025(&[a.clone()],&[EnumRecoveryChange1025::Upsert{before:Some(a),after:published}],true).is_err());
}
#[test]
fn enum_recovery1025_mutation_wire_requires_explicit_before_and_rejects_unknown_fields(){
    let a=record(A,"Alpha");
    let wire=serde_json::to_value(EnumRecoveryChange1025::Upsert{before:None,after:a}).unwrap();
    let mut missing=wire.clone();missing.as_object_mut().unwrap().remove("before");
    assert!(serde_json::from_value::<EnumRecoveryChange1025>(missing).is_err());
    let mut extra=wire.clone();extra["grant_saved"]=true.into();
    assert!(serde_json::from_value::<EnumRecoveryChange1025>(extra).is_err());
    assert_eq!(serde_json::from_value::<EnumRecoveryChange1025>(wire).unwrap().asset_id().to_hex(),A);
}
