use rvn_core::{Engine,Interaction,TerminalRenderer,TextRuntimeContext,LoadCompatibility,
    save::{SaveData,NAME_STRING_TABLE_SAVE_FORMAT_VERSION},string_table::{StringTableAsset,StringTableEntry,StringTableResources}};
use rvn_parser::Value;
use std::collections::{BTreeMap,HashMap};
const ID:&str="17411a3e431e4c9388b78a2b9825b130";
fn table(key:&str,source:&str)->StringTableAsset{StringTableAsset{id:ID.into(),namespace:"VN".into(),entries:BTreeMap::from([
    (key.into(),StringTableEntry{source:source.into(),developer_notes:"Contexte0326".into(),metadata:Default::default()})])}}
#[test]fn loaded_raw_key_survives_actual_engine_save14_cold_restore_and_later_resource_replacement(){
    for key in ["clé😀\nfin","clé😀\r\nfin","clé avec espace","clé\"quote","clé\tfin", ""]{
        let qkey=serde_json::to_string(key).unwrap();let qid=serde_json::to_string(&ID.to_ascii_uppercase()).unwrap();
        let script=format!("init {{ set named=__rvn_string_table_text_name(__rvn_literal_name({qid}),{qkey}) set key=__rvn_text_table_key(named) }}\nscene label start\n\"Before0326\"\n\"[named]\"\n\"End0326\"\nreturn\n");
        let program=rvn_parser::parse_program(&script).unwrap();
        let mut resources=StringTableResources::default();resources.publish(table(key,"Source0326")).unwrap();
        let mut original=Engine::from_program_with_persistent_and_text_context(program.clone(),TerminalRenderer,8,HashMap::new(),TextRuntimeContext::new(resources.clone(),None)).unwrap();
        assert_eq!(original.state.vars["key"],Value::Str(key.into()));
        assert!(matches!(original.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Before0326"));
        let saved=SaveData::try_from_state(&original.state,1,"Keys0326".into(),"main.rvn".into()).unwrap();
        assert_eq!(saved.format_version,NAME_STRING_TABLE_SAVE_FORMAT_VERSION);
        let serialized=serde_json::to_vec(&saved).unwrap();let json:serde_json::Value=serde_json::from_slice(&serialized).unwrap();
        assert_eq!(json["vars"]["named"]["v"]["kind"],"name_string_table");assert_eq!(json["vars"]["named"]["v"]["reference"]["key"],key);
        let mut cold=Engine::from_program_with_persistent_and_text_context(program,TerminalRenderer,8,HashMap::new(),TextRuntimeContext::new(resources,None)).unwrap();
        let snapshot=cold.text_resources().clone();assert_eq!(cold.load_data(serde_json::from_slice(&serialized).unwrap()).unwrap(),LoadCompatibility::Verified);
        assert_eq!(cold.text_resources(),&snapshot);assert_eq!(cold.state.vars["named"],original.state.vars["named"]);
        assert_eq!(cold.state.vars["key"],Value::Str(key.into()));let retained=cold.state.vars["named"].clone();
        cold.text_resources_mut().publish(table(key,"Replaced0326")).unwrap();cold.advance_dialogue().unwrap();
        assert!(matches!(cold.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Replaced0326"));
        assert_eq!(cold.state.vars["named"],retained);assert_eq!(cold.state.vars["key"],Value::Str(key.into()));
    }
}
