use rvn_core::{string_table::{StringTableAsset,StringTableEntry,StringTableResources},string_table_file::StringTableFile,
    locale::{LocaleCatalog,LocaleTable}};
use std::collections::BTreeMap;
const ID:&str="17411a3e431e4c9388b78a2b9825b130";
fn table(key:&str)->StringTableAsset{StringTableAsset{id:ID.into(),namespace:"VN".into(),entries:BTreeMap::from([
    (key.into(),StringTableEntry{source:"Source0322".into(),developer_notes:String::new(),metadata:Default::default()})])}}
fn data_roundtrip(key:&str){
    // Data paths, not the UI Add command. First prove the file/resource boundary;
    // then actual export/reimport and locale witness retain exact authored bytes.
    let file=StringTableFile::new(table(key)).expect("DATA_FILE_ADMISSION");
    let bytes=file.encode().unwrap();assert_eq!(StringTableFile::decode(&bytes).unwrap(),file);
    let empty=StringTableFile::new(StringTableAsset{id:ID.into(),namespace:"VN".into(),entries:Default::default()}).unwrap();
    let imported=empty.import_csv(&file.export_csv().unwrap()).expect("CSV_DATA_ADMISSION");
    assert_eq!(imported,file);
    let mut host=StringTableResources::default();host.publish(imported.into_table()).expect("RESOURCE_DATA_ADMISSION");
    let localization=file.table().localization(key).expect("TABLE_LOCALIZATION_ADMISSION");
    let identity=localization.identity().unwrap();
    let mut catalog=LocaleCatalog::from(LocaleTable{lang:"fr".into(),strings:std::collections::HashMap::from([
        (identity.locale_key(),"Traduit0322".into())])});
    catalog.record_string_table_source_witness(file.table(),key).expect("LOCALE_SOURCE_WITNESS_ADMISSION");
    assert_eq!(LocaleCatalog::parse("fr",&catalog.to_toml()).unwrap(),catalog);
}
#[test] fn simple_control(){data_roundtrip("hello");data_roundtrip("clé😀");}
#[test] fn lf_data(){data_roundtrip("clé😀\nfin");}
#[test] fn crlf_data(){data_roundtrip("clé😀\r\nfin");}
#[test] fn space_data(){data_roundtrip("clé avec espace");}
#[test] fn quote_data(){data_roundtrip("clé\"quote");}
#[test] fn tab_data(){data_roundtrip("clé\tfin");}
#[test] fn empty_data_matches_local_unreal_0332(){data_roundtrip("");}

#[test] fn data_identity_does_not_widen_ui_or_legacy_text_and_witness_change_is_atomic(){
    use rvn_parser::localized_text::{validate_text_identifier,StringTableReference};
    for key in ["a b","a\nb","a\r\nb","a\tb","a\"b",""] {
        assert!(StringTableReference{table:ID.into(),key:key.into()}.validate().is_err());
        if !key.is_empty(){assert!(validate_text_identifier(key).is_err());}
    }
    let t=table("a\nb");let meta=t.localization("a\nb").unwrap();let id=meta.identity().unwrap();
    let mut c=LocaleCatalog::from(LocaleTable{lang:"fr".into(),strings:std::collections::HashMap::from([(id.locale_key(),"FR".into())])});
    assert!(c.record_text_source_witness(id,"Source0322").is_err());
    assert!(c.record_string_table_source_witness(&t,"missing").is_err());
    assert!(c.record_string_table_source_witness(&t,"a\nb").unwrap());let before=c.clone();
    let mut changed=t.clone();changed.entries.get_mut("a\nb").unwrap().source="Changed".into();
    assert!(c.record_string_table_source_witness(&changed,"a\nb").is_err());assert_eq!(c,before);
    let bad="@rvn.text:999:bad";assert!(LocaleCatalog::with_text_sources(c.table().clone(),std::collections::HashMap::from([(bad.into(),"S".into())])).is_err());
}
#[test] fn modern_name_text_projects_exact_raw_key_and_source_witness_without_erasing_history(){
    use rvn_core::{locale::LocaleManager,localized_text::{render_text_with_resources,VnTextScalars}};
    use rvn_parser::localized_text::{DeferredText,StringTableReference};
    for key in ["clé😀\nfin","clé😀\r\nfin","clé avec espace","clé\"quote","clé\tfin", ""]{
        let table=table(key);let metadata=table.localization(key).unwrap();let id=metadata.identity().unwrap();
        let mut catalog=LocaleCatalog::from(LocaleTable{lang:"fr".into(),strings:std::collections::HashMap::from([(id.locale_key(),"Traduit0324".into())])});
        catalog.record_string_table_source_witness(&table,key).unwrap();
        let locale=LocaleManager::from_catalogs("unused","fr","fr",vec!["fr".into()],std::collections::HashMap::from([("fr".into(),catalog)]));
        let text=DeferredText::NameStringTable{reference:StringTableReference{table:ID.to_ascii_uppercase(),key:key.into()}};
        let encoded=serde_json::to_vec(&text).unwrap();text.storage_work().unwrap();
        let mut host=StringTableResources::default();host.publish(table.clone()).unwrap();
        assert_eq!(render_text_with_resources(&text,Some(&locale),&VnTextScalars,Some(&host)).unwrap(),"Traduit0324");
        let mut changed=table;changed.entries.get_mut(key).unwrap().source="Changed0324".into();host.publish(changed).unwrap();
        assert_eq!(render_text_with_resources(&text,Some(&locale),&VnTextScalars,Some(&host)).unwrap(),"Changed0324");
        assert_eq!(serde_json::to_vec(&text).unwrap(),encoded);
        let decoded:DeferredText=serde_json::from_slice(&encoded).unwrap();assert_eq!(decoded,text);
    }
}
#[test] fn data_limits_preflight_and_failed_publication_preserve_live_snapshot(){
    let mut host=StringTableResources::default();host.publish(table("hello")).unwrap();let stamp=host.revision_stamp();
    for key in ["NUL\0key".into(),"X".repeat(4097)]{
        assert!(StringTableFile::new(table(&key)).is_err());assert!(host.publish(table(&key)).is_err());assert_eq!(host.revision_stamp(),stamp);
    }
}
