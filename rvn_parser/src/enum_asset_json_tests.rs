use super::*;
fn value()->EnumAsset{EnumAsset::new(EnumAssetId::parse("0123456789abcdef0123456789abcdef").unwrap(),"QuestState".into()).unwrap()}
#[test]fn enum_wire_create_edit_encode_decode_preserves_nominal_refs_and_tombstones(){
    let mut value=value();let add=value.prepare(EnumCommand::Add{member:EnumMemberId::parse("0123456789abcdef0000000000000001").unwrap(),display_name:"État initial".into(),description:"Ligne1\nLigne2".into()}).unwrap();let member=add.after().members()[0].id;add.commit(&mut value).unwrap();let reference=value.value(member).unwrap();let text=value.encode().unwrap();let mut reopened=EnumAsset::decode(text.as_bytes()).unwrap();assert_eq!(value,reopened);assert_eq!(reopened.ordinal(reference),Ok(0));reopened.prepare(EnumCommand::Remove(member)).unwrap().commit(&mut reopened).unwrap();let reopened=EnumAsset::decode(reopened.encode().unwrap().as_bytes()).unwrap();assert!(reopened.ordinal(reference).is_err());assert!(reopened.parts().retired.contains(&member));
}
#[test]fn enum_wire_rejects_unknown_versions_types_fields_and_duplicate_json_keys(){
    let wire=value().encode().unwrap();for changed in [wire.replace("\"version\": 1","\"version\": 2"),wire.replace("rvn.enum","rvn.widget-blueprint"),wire.replacen('{',"{\"unknown\":true,",1),wire.replacen('{',"{\"version\":1,",1)]{assert!(EnumAsset::decode(changed.as_bytes()).is_err());}
}
#[test]fn enum_wire_uses_canonical_existing_uuid_and_member_cursors(){
    let wire=value().encode().unwrap();assert!(wire.contains("\"type\": \"rvn.enum\""));assert!(wire.contains("\"next_label_index\": \"0000000000000001\""));assert!(EnumAsset::decode(wire.replace("0123456789abcdef0123456789abcdef","0123456789ABCDEF0123456789abcdef").as_bytes()).is_err());
}
#[test]fn enum_wire_rejects_oversize_before_parsing(){assert!(EnumAsset::decode(&vec![b' ';ENUM_MAX_FILE_BYTES+1]).is_err());}
