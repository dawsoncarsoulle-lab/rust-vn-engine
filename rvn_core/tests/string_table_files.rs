use rvn_core::{
    locale::{LocaleCatalog, LocaleManager, LocaleTable},
    string_table::{StringTableAsset, StringTableEntry, StringTableReference, StringTableResources},
    string_table_file::{StringTableFile, MAX_STRING_TABLE_FILE_BYTES},
    Engine, Interaction, TerminalRenderer, TextRuntimeContext,
};
use rvn_parser::{localized_text::TextIdentity, Value};
use std::collections::{BTreeMap, HashMap};

const ID: &str = "17411a3e431e4c9388b78a2b9825b130";
const OTHER: &str = "3fc40c36f4e544ed82943e4c6fe9d87d";

#[test]
fn csv_replacement_preserves_identity_and_namespace_with_native_duplicate_rules() {
    let original = StringTableFile::new(asset("Original")).unwrap();
    let bytes = original.encode().unwrap();
    let imported = original.import_csv(b"Key,SourceString,DevNotes,Category,Comment\nhello,First,Notes1,Salutation,First comment\nshort\nhello,Last,Notes2,,Last comment\nempty,,\n").unwrap();
    assert_eq!(imported.table().id, ID);
    assert_eq!(imported.table().namespace, "VN.été");
    assert_eq!(imported.table().entries.len(), 2);
    let hello = &imported.table().entries["hello"];
    assert_eq!(hello.source, "Last"); assert_eq!(hello.developer_notes, "Notes2");
    assert_eq!(hello.metadata_value("Category"), Some("Salutation"));
    assert_eq!(hello.metadata_value("Comment"), Some("Last comment"));
    assert_eq!(imported.table().entries["empty"].source, "");
    assert_eq!(original.encode().unwrap(), bytes);
    assert_eq!(StringTableFile::decode(&imported.encode().unwrap()).unwrap(), imported);
}

#[test]
fn csv_first_reserved_columns_and_last_equivalent_metadata_columns_match_native() {
    let original = StringTableFile::new(asset("Original")).unwrap();
    let csv = "KEY,SourceString,DevNotes,key,sourcestring,devnotes,Cat,cat,É,é,e\u{301},None,nOnE,_1,None_0\nhello,Primary,Notes,metadata key,metadata source,metadata notes,First,Last,upper,lower,combined,ignored,ignored,ignored,numbered\n";
    let imported = original.import_csv(csv.as_bytes()).unwrap();
    let entry = &imported.table().entries["hello"];
    assert_eq!(entry.source, "Primary"); assert_eq!(entry.developer_notes, "Notes");
    assert_eq!(entry.metadata_value("KEY"), Some("metadata key"));
    assert_eq!(entry.metadata_value("sourcestring"), Some("metadata source"));
    assert_eq!(entry.metadata_value("devnotes"), Some("metadata notes"));
    assert_eq!(entry.metadata_value("CAT"), Some("Last")); assert!(entry.metadata.contains_key("cat"));
    assert_eq!(entry.metadata_value("É"), Some("upper")); assert_eq!(entry.metadata_value("é"), Some("lower"));
    assert_eq!(entry.metadata_value("e\u{301}"), Some("combined"));
    assert_eq!(entry.metadata_value("None_0"), Some("numbered"));
    assert_eq!(entry.metadata.len(), 8);
}

#[test]
fn csv_reverse_escape_passes_and_ascii_export_match_actual_windows_reference() {
    let mut table = asset("unused"); table.namespace="ASCII".into(); table.entries.clear();
    let original = StringTableFile::new(table).unwrap();
    let input = concat!("Key,SourceString,DevNotes,Meta\n",
        "one,one\\nend,one\\tend,one\\nend\n",
        "two,two\\\\nend,two\\\\tend,two\\\\nend\n",
        "three,three\\\\\\nend,three\\\\\\tend,three\\\\\\nend\n");
    let imported=original.import_csv(input.as_bytes()).unwrap();
    assert_eq!(imported.table().entries["one"].source, "one\nend");
    assert_eq!(imported.table().entries["two"].source, "two\\\nend");
    assert_eq!(imported.table().entries["three"].source, "three\\\nend");
    assert_eq!(imported.table().entries["two"].metadata_value("Meta"), Some("two\\\nend"));
    let export=imported.export_csv().unwrap(); assert!(export.is_ascii()); assert!(!export.starts_with(&[0xef,0xbb,0xbf]));
    let text=std::str::from_utf8(&export).unwrap();
    assert!(text.contains("\"one\",\"one\\nend\",\"one\\tend\",\"one\nend\"\n"));
    assert!(text.contains("\"two\",\"two\\\\\\nend\",\"two\\\\\\tend\",\"two\\\nend\"\n"));
    assert_eq!(imported.import_csv(&export).unwrap(),imported);
}

#[test]
fn csv_quotes_physical_newlines_and_unicode_encoding_keep_their_distinct_contracts() {
    let original=StringTableFile::new(asset("Original")).unwrap();
    let csv="Key,SourceString,DevNotes,Meta\r\n\"physical\",\"First\r\nsecond\rthird\nfourth\",\"Notes\tÉloïse\",\"Meta\nactual newline\"\r\n\"quotes\",\"A \"\"quote\"\" and 'apostrophe'\",\"Notes \"\"quoted\"\"\",\"Meta \"\"quote\"\"\"\n";
    let imported=original.import_csv(csv.as_bytes()).unwrap();
    let entry=&imported.table().entries["physical"];
    assert_eq!(entry.source,"First\r\nsecond\rthird\nfourth");assert_eq!(entry.developer_notes,"Notes\tÉloïse");
    assert_eq!(entry.metadata_value("Meta"),Some("Meta\nactual newline"));
    assert_eq!(imported.table().entries["quotes"].source,"A \"quote\" and 'apostrophe'");
    let export=imported.export_csv().unwrap();assert!(export.starts_with(&[0xff,0xfe]));
    assert_eq!(imported.import_csv(&export).unwrap(),imported);
    let mut big=vec![0xfe,0xff];for unit in csv.encode_utf16(){big.extend_from_slice(&unit.to_be_bytes());}
    assert_eq!(original.import_csv(&big).unwrap(),imported);
    let mut utf8=vec![0xef,0xbb,0xbf];utf8.extend_from_slice(csv.as_bytes());assert_eq!(original.import_csv(&utf8).unwrap(),imported);
}

#[test]
fn csv_special_headers_reproduce_measured_reference_export_reimport_limitation() {
    let original=StringTableFile::new(asset("Original")).unwrap();
    let input="Key,SourceString,DevNotes,\"Meta,comma\",\"Meta\"\"quote\",\"Meta\nline\"\nheaders,Source headers,Notes headers,Comma column,Quote column,Line column\n";
    let imported=original.import_csv(input.as_bytes()).unwrap();
    assert_eq!(imported.table().entries["headers"].metadata.len(),3);
    let text=String::from_utf8(imported.export_csv().unwrap()).unwrap();
    assert!(text.starts_with("Key,SourceString,DevNotes,Meta\nline,Meta\"quote,Meta,comma\n"));
    // Actual native output's column order is independent of the Rust sort.
    let native_order=b"Key,SourceString,DevNotes,Meta,comma,Meta\"quote,Meta\nline\n\"headers\",\"Source headers\",\"Notes headers\",\"Comma column\",\"Quote column\",\"Line column\"\n";
    let reimported=imported.import_csv(native_order).unwrap();
    assert_eq!(reimported.table().entries["headers"].metadata,BTreeMap::from([("comma".into(),"Quote column".into()),("Meta\"quote".into(),"Line column".into())]));
    assert_eq!(StringTableFile::decode(&imported.encode().unwrap()).unwrap(),imported);
}

#[test]
fn csv_invalid_input_never_changes_the_current_file_or_published_resources() {
    let original=StringTableFile::new(asset("Original")).unwrap();let bytes=original.encode().unwrap();
    let mut resources=StringTableResources::default();resources.publish(original.table().clone()).unwrap();let stamp=resources.revision_stamp();
    for invalid in [&b"Key,SourceString\n"[..],b"Key,Notes\nhello,missing\n",b"SourceString\nmissing\n",b"\xff\xfeA",b"\xff\xfe\x00\xd8",b"Key,SourceString\nhello,\xff\n",b"Key,SourceString\nhello,\0data\n",b"Key,SourceString\nNUL\0key,source\n"] {
        assert!(original.import_csv(invalid).is_err(),"{invalid:?}");
        assert_eq!(original.encode().unwrap(),bytes);assert_eq!(resources.revision_stamp(),stamp);assert_eq!(resources.ready_table(ID),Some(original.table()));
    }
    let short=original.import_csv(b"Key,SourceString\nshort\n").unwrap();assert!(short.table().entries.is_empty());
}

#[test]
fn csv_streaming_parser_keeps_blank_line_trailing_cell_and_quoted_tail_rules() {
    let original=StringTableFile::new(asset("Original")).unwrap();
    let imported=original.import_csv(b"\n\r\nKey,SourceString,DevNotes,Meta\n\nhello,\"quoted\"tail,notes,\nworld, \"literal quote\",,value\r").unwrap();
    assert_eq!(imported.table().entries["hello"].source,"quotedtail");assert!(imported.table().entries["hello"].metadata.is_empty());
    assert_eq!(imported.table().entries["world"].source," \"literal quote\"");
    assert_eq!(imported.table().entries["world"].metadata_value("Meta"),Some("value"));
}

#[test]
fn csv_byte_and_cell_overhead_limits_reject_before_publication() {
    let original=StringTableFile::new(asset("Original")).unwrap();
    let mut huge=b"Key,SourceString\nhello,".to_vec();huge.resize(8*1_048_576+huge.len()+1,b'x');
    assert!(original.import_csv(&huge).is_err());
    let many=format!("Key,SourceString{}\nhello,Source\n",",".repeat(65_539));assert!(original.import_csv(many.as_bytes()).is_err());
    let wire=vec![b'x';MAX_STRING_TABLE_FILE_BYTES+1];assert!(original.import_csv(&wire).is_err());
    assert_eq!(original.table().entries["hello"].source,"Original");
}

#[test]
fn csv_escaped_wire_cells_can_exceed_decoded_budget_without_losing_valid_content() {
    let mut table=asset("unused");table.entries.clear();table.entries.insert("hello".into(),StringTableEntry{source:"'".repeat(4*1_048_576+64),developer_notes:String::new(),metadata:BTreeMap::new()});
    let original=StringTableFile::new(table).unwrap();let csv=original.export_csv().unwrap();assert!(csv.len()>8*1_048_576);
    assert_eq!(original.import_csv(&csv).unwrap(),original);
}

#[test]
fn csv_escape_letters_follow_local_source_ascii_case_default_and_nul_is_explicit() {
    let original=StringTableFile::new(asset("Original")).unwrap();
    let imported=original.import_csv(b"Key,SourceString,DevNotes,Meta\nupper,First\\Nsecond\\Rthird\\Tend,Notes\\Tend,Meta\\Nend\n").unwrap();
    let entry=&imported.table().entries["upper"];assert_eq!(entry.source,"First\nsecond\rthird\tend");assert_eq!(entry.developer_notes,"Notes\tend");assert_eq!(entry.metadata_value("Meta"),Some("Meta\nend"));
    let nul=StringTableFile::new(asset("NUL\0source")).unwrap();assert!(nul.export_csv().is_err());assert_eq!(StringTableFile::decode(&nul.encode().unwrap()).unwrap(),nul);
}

#[test]
fn csv_export_capacity_indexes_equivalent_names_locally_and_keeps_large_metadata() {
    let mut table=asset("Original");table.entries.clear();
    let metadata=(0..65_535).map(|i|(format!("Field{i:05}"),"Value".into())).collect();
    table.entries.insert("a".into(),StringTableEntry{source:"A".into(),developer_notes:String::new(),metadata});
    table.entries.insert("b".into(),StringTableEntry{source:"B".into(),developer_notes:String::new(),metadata:BTreeMap::from([("field00001".into(),"Other".into())])});
    let original=StringTableFile::new(table).unwrap();let bytes=original.export_csv().unwrap();assert!(bytes.len()<4*1_048_576);
    let imported=original.import_csv(&bytes).unwrap();assert_eq!(imported.table().entries["a"],original.table().entries["a"]);
    assert_eq!(imported.table().entries["b"].metadata_value("FIELD00001"),Some("Other"));assert_eq!(imported.table().entries["b"].metadata.len(),1);
    assert_eq!(imported.table().entries["a"].metadata.len(),65_535);
}
fn asset(source: &str) -> StringTableAsset {
    StringTableAsset { id: ID.into(), namespace: "VN.été".into(), entries: BTreeMap::from([
        ("hello".into(), StringTableEntry {metadata:Default::default(), source: source.into(), developer_notes: "Salutation — paramètres {Name}".into() }),
        ("Hello".into(), StringTableEntry {metadata:Default::default(), source: String::new(), developer_notes: String::new() }),
    ]) }
}

#[test]
fn authored_file_round_trip_retains_identity_case_unicode_notes_and_empty_data_sources() {
    let original = StringTableFile::new(asset("quote\" — 漢字😀\nline\rreturn\ttab\\n")).unwrap();
    let bytes = original.encode().unwrap();
    assert!(bytes.ends_with(b"\n"));
    let restored = StringTableFile::decode(&bytes).unwrap();
    assert_eq!(restored, original);
    assert_eq!(restored.encode().unwrap(), bytes);
    assert!(restored.table().entries["Hello"].source.is_empty());
    assert!(!restored.table().entries["hello"].developer_notes.is_empty());
    let mut empty = restored.table().clone(); empty.namespace.clear(); empty.entries.clear();
    let empty = restored.edited(empty).unwrap();
    assert_eq!(StringTableFile::decode(&empty.encode().unwrap()).unwrap(), empty);
}

#[test]
fn unknown_versions_fields_duplicate_json_keys_and_bad_ids_are_refused_explicitly() {
    let base = format!(r#"{{"format_version":1,"id":"{ID}","namespace":"VN","entries":{{"hello":{{"source":"Hello"}}}}}}"#);
    for malformed in [
        base.replace("\"format_version\":1", "\"format_version\":3"),
        base.replace("\"format_version\":1", "\"format_version\":0"),
        base.replace("\"namespace\":\"VN\"", "\"namespace\":\"VN\",\"future\":true"),
        base.replace("\"source\":\"Hello\"", "\"source\":\"Hello\",\"display_cache\":\"stale\""),
        base.replace(&format!("\"id\":\"{ID}\""), &format!("\"id\":\"{ID}\",\"id\":\"{OTHER}\"")),
        base.replace("\"hello\":{\"source\":\"Hello\"}", "\"hello\":{\"source\":\"Hello\"},\"hello\":{\"source\":\"Changed\"}"),
        base.replace("\"hello\":{\"source\":\"Hello\"}", "\"hello\":{\"source\":\"Hello\"},\"\\u0068ello\":{\"source\":\"Changed\"}"),
    ] { assert!(StringTableFile::decode(malformed.as_bytes()).is_err(), "{malformed}"); }
    for bad in ["", "00000000000000000000000000000000", "17411A3E431E4C9388B78A2B9825B130", "asset:legacy", "../table"] {
        assert!(StringTableFile::decode(base.replace(ID, bad).as_bytes()).is_err());
        let mut candidate = asset("Hello"); candidate.id = bad.into();
        assert!(StringTableFile::new(candidate).is_err());
    }
}

#[test]
fn metadata_format_migrates_explicitly_and_legacy_output_remains_byte_stable() {
    let legacy = StringTableFile::new(asset("Hello")).unwrap();
    let old_bytes = legacy.encode().unwrap();
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&old_bytes).unwrap()["format_version"], 1);
    let mut table = legacy.table().clone();
    let entry = table.entries.get_mut("hello").unwrap();
    for (name, value) in [("Cat", "First"), ("cat", "Last"), ("É", "upper"), ("é", "lower"), ("e\u{301}", "combined"), ("None_0", "numbered"), ("EmptyValue", "")] {
        entry.set_metadata(name.into(), value.into()).unwrap();
    }
    assert_eq!(entry.metadata_value("CAT"), Some("Last"));
    assert_eq!(entry.metadata_value("É"), Some("upper"));
    assert_eq!(entry.metadata_value("é"), Some("lower"));
    assert_eq!(entry.metadata.len(), 6);
    let extended = legacy.edited(table).unwrap();
    let bytes = extended.encode().unwrap();
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["format_version"], 2);
    assert_eq!(StringTableFile::decode(&bytes).unwrap(), extended);
    assert_eq!(StringTableFile::decode(&bytes).unwrap().encode().unwrap(), bytes);
    assert_eq!(legacy.encode().unwrap(), old_bytes);
    let mut cleared = extended.table().clone(); cleared.entries.get_mut("hello").unwrap().metadata.clear();
    assert_eq!(extended.edited(cleared).unwrap().encode().unwrap(), old_bytes);
    let mut resources = StringTableResources::default(); resources.publish(extended.into_table()).unwrap();
    assert_eq!(resources.display(&StringTableReference { table: ID.into(), key: "hello".into() }, None).unwrap(), "Hello");
}

#[test]
fn invalid_metadata_and_hidden_legacy_extensions_are_rejected_before_publication() {
    let base = format!(r#"{{"format_version":2,"id":"{ID}","namespace":"VN","entries":{{"hello":{{"source":"Hello","metadata":{{"Cat":"First"}}}}}}}}"#);
    for malformed in [
        base.replace("\"format_version\":2", "\"format_version\":1"),
        base.replace("\"Cat\":\"First\"", "\"Cat\":\"First\",\"cat\":\"Last\""),
        base.replace("\"Cat\":\"First\"", "\"Cat\":\"First\",\"Cat\":\"Last\""),
        base.replace("{\"Cat\":\"First\"}", "null"),
        base.replace("\"Cat\"", "\"None\""),
        base.replace("\"Cat\"", "\"_1\""),
        base.replace("\"Cat\"", "\"bad\\u0000name\""),
    ] { assert!(StringTableFile::decode(malformed.as_bytes()).is_err(), "{malformed}"); }
    let original = asset("Hello"); let mut resources = StringTableResources::default();resources.publish(original.clone()).unwrap();
    let stamp = resources.revision_stamp();
    for metadata in [BTreeMap::from([("Cat".into(), "A".into()), ("cat".into(), "B".into())]), BTreeMap::from([("None".into(), "Absent".into())])] {
        let mut candidate = original.clone();candidate.entries.get_mut("hello").unwrap().metadata = metadata;
        assert!(resources.publish(candidate).is_err());assert_eq!(resources.revision_stamp(), stamp);
        assert_eq!(resources.ready_table(ID), Some(&original));
    }
}

#[test]
fn metadata_names_and_values_count_towards_table_and_project_budgets() {
    let mut table = asset("Hello");let before = table.content_bytes().unwrap();
    table.entries.get_mut("hello").unwrap().set_metadata("Speaker".into(), "Éloïse".into()).unwrap();
    assert_eq!(table.content_bytes().unwrap(), before + "Speaker".len() + "Éloïse".len());
    table.entries.get_mut("hello").unwrap().metadata.insert("Payload".into(), "X".repeat(8 * 1_048_576));
    assert!(StringTableFile::new(table).is_err());
}

#[test]
fn ordinary_edits_keep_asset_identity_and_invalid_edits_do_not_change_the_document() {
    let original = StringTableFile::new(asset("Hello")).unwrap();
    let bytes = original.encode().unwrap();
    let mut candidate = original.table().clone(); candidate.id = OTHER.into();
    assert!(original.edited(candidate.clone()).is_err());
    // Duplication is explicit and must receive a separately supplied ID.
    let duplicate = StringTableFile::new(candidate).unwrap();
    assert_eq!(duplicate.table().id, OTHER);
    assert_eq!(duplicate.table().entries, original.table().entries);
    let mut invalid = original.table().clone(); invalid.namespace = "[invalid]".into();
    assert!(original.edited(invalid).is_err());
    let mut invalid = original.table().clone(); invalid.entries.insert("NUL\0key".into(), StringTableEntry {metadata:Default::default(), source: "Invalid key".into(), developer_notes: String::new() });
    assert!(original.edited(invalid).is_err());
    let mut edited = original.table().clone(); edited.namespace = "New.Namespace".into();
    edited.entries.get_mut("hello").unwrap().source = "New source".into();
    let edited = original.edited(edited).unwrap();
    assert_eq!(edited.table().id, ID);
    assert_eq!(original.encode().unwrap(), bytes);
}

#[test]
fn both_wire_and_decoded_content_are_bounded_before_publication() {
    assert!(StringTableFile::decode(&vec![b' '; MAX_STRING_TABLE_FILE_BYTES + 1]).is_err());
    let mut oversized = asset("Hello");
    oversized.entries.get_mut("hello").unwrap().source = "X".repeat(8 * 1_048_576);
    assert!(StringTableFile::new(oversized).is_err());
    // Canonical encoding accounts for escape expansion as well as entry keys
    // and JSON structure, so decoded-valid control sources remain representable.
    let mut escaped = asset("\u{0000}"); escaped.entries.get_mut("hello").unwrap().source = "\u{0000}".repeat(1_048_576);
    let escaped = StringTableFile::new(escaped).unwrap();
    let encoded = escaped.encode().unwrap();
    assert!(encoded.len() > 6 * 1_048_576);
    assert_eq!(StringTableFile::decode(&encoded).unwrap(), escaped);
}

#[test]
fn decoded_asset_is_available_before_init_and_updated_source_uses_current_witness_without_rewriting_reference() {
    let file = StringTableFile::decode(&StringTableFile::new(asset("Visitor")).unwrap().encode().unwrap()).unwrap();
    let identity = TextIdentity { owner: format!("string-table:{ID}"), namespace: "VN.été".into(), key: "hello".into() };
    let locale = LocaleManager::from_catalogs("unused", "en", "fr", vec!["en".into(), "fr".into()], HashMap::from([
        ("fr".into(), LocaleCatalog::with_text_sources(LocaleTable { lang: "fr".into(), strings: HashMap::from([(identity.locale_key(), "Éloïse".into())]) },
            HashMap::from([(identity.locale_key(), "Visitor".into())])).unwrap()),
    ]));
    let mut resources = StringTableResources::default(); resources.publish(file.table().clone()).unwrap();
    let script = format!(r#"init {{
set name=__rvn_string_table_text("{ID}","hello")
set converted=text_to_string(name)
}}
scene label start
"[name]"
"#);
    let mut game = Engine::from_program_with_persistent_and_text_context(rvn_parser::parse_program(&script).unwrap(), TerminalRenderer, 8, HashMap::new(), TextRuntimeContext::new(resources, Some(locale))).unwrap();
    assert_eq!(game.state.vars["converted"], Value::Str("Éloïse".into()));
    assert!(matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Éloïse"));
    let retained = serde_json::to_string(&game.state.vars["name"]).unwrap();
    let mut changed = file.table().clone(); changed.entries.get_mut("hello").unwrap().source = "Changed".into();
    let changed = file.edited(changed).unwrap();
    game.text_resources_mut().publish(StringTableFile::decode(&changed.encode().unwrap()).unwrap().into_table()).unwrap();
    assert_eq!(game.text_resources().display(&StringTableReference { table: ID.into(), key: "hello".into() }, game.locale.as_ref()).unwrap(), "Changed");
    assert_eq!(serde_json::to_string(&game.state.vars["name"]).unwrap(), retained);
    assert_eq!(game.state.vars["converted"], Value::Str("Éloïse".into()));
}


#[test]
fn metadata_field_capacity_is_shared_and_near_budget_files_remain_persistable() {
    let mut table=asset("Hello");
    for key in ["hello","Hello"] {
        table.entries.get_mut(key).unwrap().metadata=(0..32_768).map(|n|(format!("Meta{n:05}"),"\0".repeat(117))).collect();
    }
    assert!(table.content_bytes().unwrap()>7*1_048_576);
    let file=StringTableFile::new(table.clone()).unwrap();let bytes=file.encode().unwrap();
    assert!(bytes.len()<MAX_STRING_TABLE_FILE_BYTES);assert!(bytes.len()>40*1_048_576);
    assert_eq!(StringTableFile::decode(&bytes).unwrap(),file);
    let mut resources=StringTableResources::default();resources.publish(table.clone()).unwrap();let stamp=resources.revision_stamp();
    let mut exceeded=table.clone();exceeded.entries.get_mut("Hello").unwrap().set_metadata("Additional".into(),String::new()).unwrap();
    assert!(exceeded.content_bytes().unwrap()<8*1_048_576);
    assert!(resources.publish(exceeded).is_err());assert_eq!(resources.revision_stamp(),stamp);assert_eq!(resources.ready_table(ID),Some(&table));
}
