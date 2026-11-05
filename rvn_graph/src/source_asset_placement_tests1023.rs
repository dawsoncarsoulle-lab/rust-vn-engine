use super::*;
use crate::GraphKind;
fn entry(id:u64,namespace:SourcePlacementNamespace,directory:&str)->SourceAssetPlacement{
    SourceAssetPlacement{graph_id:GraphId::new(id),namespace,directory:directory.into()}
}
fn graph(id:u64,name:&str)->GraphDocument{GraphDocument::new(GraphId::new(id),GraphKind::Label{name:name.into()})}
#[test]
fn source_placement1023_values_roundtrip_canonical_identity_order_and_preserve_namespaces(){
    let a=entry(7,SourcePlacementNamespace::Physical,"assets/Scènes/Été");
    let b=entry(3,SourcePlacementNamespace::SourceGraphs,"Blueprints/Chapitre");
    let map=SourceAssetPlacements::from_entries([a.clone(),b.clone()]).unwrap();
    assert_eq!(map.entries().map(|row|row.graph_id.get()).collect::<Vec<_>>(),[3,7]);
    assert_eq!(map.get(a.graph_id),Some(&a));assert_eq!(map.get(b.graph_id),Some(&b));
    let wire=serde_json::to_string(&map).unwrap();
    assert_eq!(wire,serde_json::to_string(&SourceAssetPlacements::from_entries([b,a]).unwrap()).unwrap());
    assert_eq!(serde_json::from_str::<SourceAssetPlacements>(&wire).unwrap(),map);
}
#[test]
fn source_placement1023_reuses_existing_relative_path_contract_without_normalizing_foreign_paths(){
    for path in ["","/outside","C:/outside","../outside","assets/../outside","assets//child","assets\\child","assets/bad.","assets/bad ","assets/#fragment","assets/?query"]{
        assert!(SourceAssetPlacements::from_entries([entry(1,SourcePlacementNamespace::Physical,path)]).is_err(),"{path}");
    }
    let deep=std::iter::repeat("x").take(65).collect::<Vec<_>>().join("/");
    let long="a".repeat(4097);
    for path in [&deep,&long]{assert!(SourceAssetPlacements::from_entries([entry(1,SourcePlacementNamespace::Physical,path)]).is_err());}
}
#[test]
fn source_placement1023_duplicate_rows_and_zero_identity_are_rejected_not_last_wins(){
    let first=entry(1,SourcePlacementNamespace::Physical,"assets/first");
    for second in [first.clone(),entry(1,SourcePlacementNamespace::Physical,"assets/second")]{
        assert!(SourceAssetPlacements::from_entries([first.clone(),second]).is_err());
    }
    assert!(SourceAssetPlacements::from_entries([entry(0,SourcePlacementNamespace::Physical,"assets")]).is_err());
    let row=serde_json::to_value(first).unwrap();
    assert!(serde_json::from_value::<SourceAssetPlacements>(serde_json::json!({"version":1,"entries":[row.clone(),row]})).is_err());
}
#[test]
fn source_placement1023_wire_unknown_versions_fields_and_null_are_explicit_refusals(){
    let good=serde_json::to_value(SourceAssetPlacements::from_entries([entry(1,SourcePlacementNamespace::Physical,"assets")]).unwrap()).unwrap();
    for field in ["namespace","directory","graph_id"]{
        let mut wire=good.clone();wire["entries"][0][field]=serde_json::Value::Null;
        assert!(serde_json::from_value::<SourceAssetPlacements>(wire).is_err());
    }
    let mut unknown=good.clone();unknown["version"]=serde_json::json!(2);assert!(serde_json::from_value::<SourceAssetPlacements>(unknown).is_err());
    let mut extra=good.clone();extra["discard_me"]=serde_json::json!(true);assert!(serde_json::from_value::<SourceAssetPlacements>(extra).is_err());
    let mut extra=good.clone();extra["entries"][0]["locator"]=serde_json::json!("fake.rvngraph");assert!(serde_json::from_value::<SourceAssetPlacements>(extra).is_err());
    let mut namespace=good;namespace["entries"][0]["namespace"]=serde_json::json!("latest_alias");assert!(serde_json::from_value::<SourceAssetPlacements>(namespace).is_err());
    assert!(serde_json::from_value::<SourceAssetPlacements>(serde_json::Value::Null).is_err());
}
#[test]
fn source_placement1023_exact_graph_identity_survives_rename_but_never_rebinds_same_name(){
    let map=SourceAssetPlacements::from_entries([entry(3,SourcePlacementNamespace::Physical,"assets/scenes")]).unwrap();
    map.validate_graphs(&[graph(3,"first")]).unwrap();map.validate_graphs(&[graph(3,"renamed")]).unwrap();
    assert!(map.validate_graphs(&[graph(4,"first")]).is_err());
    assert!(map.validate_graphs(&[graph(3,"first"),graph(3,"other")]).is_err());
    assert!(map.validate_graphs(&[]).is_err());
}
#[test]
fn source_placement1023_metadata_never_promotes_an_ordinary_label_to_scene(){
    let ordinary=graph(3,"ordinary");let before=ordinary.clone();
    let map=SourceAssetPlacements::from_entries([entry(3,SourcePlacementNamespace::Physical,"assets/scenes")]).unwrap();
    map.validate_graphs(std::slice::from_ref(&ordinary)).unwrap();
    assert_eq!(ordinary.narrative,None);assert_eq!(ordinary,before);
}
#[test]
fn source_placement1023_empty_value_is_explicit_and_does_not_supply_legacy_default_placements(){
    let empty=SourceAssetPlacements::default();assert!(empty.is_empty());assert_eq!(empty.len(),0);empty.validate_graphs(&[graph(1,"untouched")]).unwrap();
    assert_eq!(serde_json::to_value(&empty).unwrap(),serde_json::json!({"version":1,"entries":[]}));
    assert_eq!(serde_json::from_str::<SourceAssetPlacements>("{\"version\":1,\"entries\":[]}").unwrap(),empty);
    assert!(empty.get(GraphId::new(1)).is_none());
}
