use rvn_core::{
    locale::{LocaleManager, LocaleTable},
    string_table::*,
};
use std::collections::{BTreeMap, HashMap};

fn reference(table: &str, key: &str) -> StringTableReference {
    StringTableReference {
        table: table.into(),
        key: key.into(),
    }
}
fn asset(source: &str) -> StringTableAsset {
    StringTableAsset {
        id: "asset:table-fixture".into(),
        namespace: "VN.Greetings".into(),
        entries: BTreeMap::from([(
            "hello".into(),
            StringTableEntry {metadata:Default::default(),
                source: source.into(),
                developer_notes: "Salutation — été".into(),
            },
        )]),
    }
}

fn query(library: &rvn_core::eval::FunctionLibrary, name: &str, args: &[&str]) -> Result<rvn_parser::Value, rvn_core::eval::EvalError> {
    library.eval(&rvn_parser::Expr::Call { name:name.into(),args:args.iter().map(|s|rvn_parser::Expr::Str((*s).into())).collect() }, &HashMap::new())
}

#[test]
fn string_table_asset_identity_survives_content_publication_unload_and_value_save() {
    use rvn_core::eval::FunctionLibrary;
    use rvn_parser::{Expr,Value};
    let id="asset:table-fixture";
    let mut library=FunctionLibrary::default();
    let before=library.text_resources().clone();
    for candidate in ["",id,"missing"] {
        assert_eq!(query(&library,"__rvn_string_table_asset",&[candidate]).unwrap(),Value::Str(String::new()));
    }
    assert_eq!(library.text_resources(),&before);
    let token=library.text_resources_mut().begin_load(id).unwrap();
    assert_eq!(query(&library,"__rvn_string_table_asset",&[id]).unwrap(),Value::Str(String::new()));
    library.text_resources_mut().fail_load(&token,"Not loaded".into()).unwrap();
    assert_eq!(query(&library,"__rvn_string_table_asset",&[id]).unwrap(),Value::Str(String::new()));
    library.text_resources_mut().publish(asset("Before")).unwrap();
    let snapshot=library.clone();
    let reference=query(&library,"__rvn_string_table_asset",&[id]).unwrap();
    assert_eq!(reference,Value::Dict(BTreeMap::from([
        ("kind".into(),Value::Str("string_table_asset".into())),
        ("id".into(),Value::Str(id.into())),
    ])));
    let stored:Value=serde_json::from_str(&serde_json::to_string(&reference).unwrap()).unwrap();
    let call=Expr::Call{name:"__rvn_string_table_id".into(),args:vec![Expr::Var("reference".into())]};
    let vars=HashMap::from([("reference".into(),stored)]);
    library.text_resources_mut().publish(asset("Changed")).unwrap();
    assert_eq!(library.eval(&call,&vars).unwrap(),Value::Str(id.into()));
    library.text_resources_mut().unload(id).unwrap();
    let empty_registry=library.text_resources().clone();
    assert_eq!(library.eval(&call,&vars).unwrap(),Value::Str(id.into()));
    assert_eq!(library.text_resources(),&empty_registry);
    assert_eq!(query(&library,"__rvn_string_table_asset",&[id]).unwrap(),Value::Str(String::new()));
    assert_eq!(query(&snapshot,"__rvn_string_table_asset",&[id]).unwrap(),reference);
    assert_eq!(query(&library,"__rvn_string_table_id",&[""]).unwrap(),Value::Str(String::new()));
    for name in ["__rvn_string_table_asset","__rvn_string_table_id"] {
        assert!(query(&library,name,&["bad\nidentity"]).is_err());
        assert!(query(&library,name,&[]).is_err());
        assert!(library.eval(&Expr::Call{name:name.into(),args:vec![Expr::Int(8)]},&HashMap::new()).is_err());
        assert_eq!(library.text_resources(),&empty_registry);
    }
}

#[test]
fn string_table_asset_saved_game_restores_identity_without_loading_missing_content() {
    use rvn_core::{save::SaveData,Engine,Interaction,LoadCompatibility,TerminalRenderer};
    use rvn_parser::Value;
    let script=rvn_parser::parse(r#"init {set reference=__rvn_string_table_asset("asset:table-fixture")}
label start
"Before [__rvn_string_table_id(reference)]"
set fresh=__rvn_string_table_asset("asset:table-fixture")
"After [__rvn_string_table_id(reference)]/[__rvn_string_table_id(fresh)]"
"#).unwrap();
    let mut ready=StringTableResources::default();ready.publish(asset("Before")).unwrap();
    let mut game=Engine::new_with_text_resources(script.clone(),TerminalRenderer,8,ready).unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Before asset:table-fixture"));
    let reference=game.state.vars["reference"].clone();
    let saved=SaveData::from_state(&game.state,1,"asset reference".into(),"story.rvn".into());
    let serialized=serde_json::to_string(&saved).unwrap();
    let mut restored=Engine::new(script,TerminalRenderer,8).unwrap();
    let resources=restored.text_resources().clone();
    assert_eq!(restored.load_data(serde_json::from_str(&serialized).unwrap()).unwrap(),LoadCompatibility::Verified);
    assert_eq!(restored.state.vars["reference"],reference);
    restored.advance_dialogue().unwrap();
    assert!(matches!(restored.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="After asset:table-fixture/"));
    assert_eq!(restored.state.vars["reference"],reference);
    assert_eq!(restored.state.vars["fresh"],Value::Str(String::new()));
    assert_eq!(restored.text_resources(),&resources);
}

#[test]
fn string_table_asset_dynamic_values_enforce_subtype_without_mutating_resources() {
    use rvn_core::eval::{EvalError,FunctionLibrary};
    use rvn_parser::{Expr,Value};
    let mut library=FunctionLibrary::default();
    library.text_resources_mut().publish(asset("Before")).unwrap();
    let before=library.text_resources().clone();
    let call=Expr::Call{name:"__rvn_string_table_id".into(),args:vec![Expr::Var("dynamic".into())]};
    let record=|kind:&str,id:Value| Value::Dict(BTreeMap::from([
        ("kind".into(),Value::Str(kind.into())),("id".into(),id),
    ]));
    let mut extra=match record("string_table_asset",Value::Str("asset:table-fixture".into())) {Value::Dict(fields)=>fields,_=>unreachable!()};
    extra.insert("content".into(),Value::Str("foreign".into()));
    let bad=[
        Value::Str("asset:table-fixture".into()),Value::Bool(false),Value::Int(8),
        Value::List(vec![]),Value::Dict(BTreeMap::new()),
        record("video_clip",Value::Str("asset:table-fixture".into())),
        record("string_table_asset",Value::Int(8)),
        record("string_table_asset",Value::Str(String::new())),
        record("string_table_asset",Value::Str("bad\nidentity".into())),
        record("string_table_asset",Value::Str(" ".into())),
        record("string_table_asset",Value::Str("x".repeat(4097))),
        Value::Dict(extra),
    ];
    for value in bad {
        let vars=HashMap::from([("dynamic".into(),value)]);
        assert!(matches!(library.eval(&call,&vars),Err(EvalError::InvalidFunction(_))));
        assert_eq!(library.text_resources(),&before);
    }
    let vars=HashMap::from([("dynamic".into(),Value::Str(String::new()))]);
    assert_eq!(library.eval(&call,&vars).unwrap(),Value::Str(String::new()));
    assert_eq!(library.text_resources(),&before);
}

#[test]
fn string_table_queries_are_read_only_snapshot_local_and_distinguish_empty_from_missing() {
    use rvn_core::eval::FunctionLibrary;
    use rvn_parser::Value;
    let mut library=FunctionLibrary::default();
    let mut table=asset("Source {Name}");
    table.entries.get_mut("hello").unwrap().metadata=BTreeMap::from([
        ("Speaker".into(),"Éloïse".into()),("É".into(),"Accent upper".into()),("é".into(),"Accent lower".into())]);
    table.entries.insert("empty".into(),StringTableEntry { source:String::new(), developer_notes:String::new(),metadata:Default::default() });
    library.text_resources_mut().publish(table.clone()).unwrap();
    library.text_resources_mut().begin_load("pending").unwrap();
    let failed=library.text_resources_mut().begin_load("failed").unwrap();
    library.text_resources_mut().fail_load(&failed,"Independent failure".into()).unwrap();
    library.text_resources_mut().redirect(reference("alias","hello"),reference(&table.id,"hello")).unwrap();
    let stamp=library.text_resources().revision_stamp(); let snapshot=library.clone();
    let table_id=table.id.clone(); let id=table_id.as_str();
    assert_eq!(query(&library,"__rvn_registered_string_tables",&[]).unwrap(),Value::List(vec![Value::Str(id.into())]));
    for absent in ["missing","pending","failed","alias"] {
        assert_eq!(query(&library,"__rvn_string_table_registered",&[absent]).unwrap(),Value::Bool(false));
        assert_eq!(query(&library,"__rvn_string_table_namespace",&[absent]).unwrap(),Value::Str(String::new()));
        assert_eq!(query(&library,"__rvn_string_table_keys",&[absent]).unwrap(),Value::List(vec![]));
    }
    assert_eq!(query(&library,"__rvn_string_table_registered",&[id]).unwrap(),Value::Bool(true));
    assert_eq!(query(&library,"__rvn_string_table_namespace",&[id]).unwrap(),Value::Str("VN.Greetings".into()));
    assert_eq!(query(&library,"__rvn_string_table_keys",&[id]).unwrap(),Value::List(vec![Value::Str("empty".into()),Value::Str("hello".into())]));
    for (key,exists) in [("empty",true),("missing",false),("HELLO",false)] {
        assert_eq!(query(&library,"__rvn_string_table_entry_registered",&[id,key]).unwrap(),Value::Bool(exists));
        assert_eq!(query(&library,"__rvn_string_table_source",&[id,key]).unwrap(),Value::Str(String::new()));
        assert_eq!(query(&library,"__rvn_string_table_metadata_ids",&[id,key]).unwrap(),Value::List(vec![]));
    }
    assert_eq!(query(&library,"__rvn_string_table_source",&[id,"hello"]).unwrap(),Value::Str("Source {Name}".into()));
    for (name,value) in [("speaker","Éloïse"),("É","Accent upper"),("é","Accent lower"),("None",""),("missing","")] {
        assert_eq!(query(&library,"__rvn_string_table_metadata",&[id,"hello",name]).unwrap(),Value::Str(value.into()));
    }
    assert_eq!(query(&library,"__rvn_string_table_metadata_ids",&[id,"hello"]).unwrap(),Value::List(vec![Value::Str("Speaker".into()),Value::Str("É".into()),Value::Str("é".into())]));
    assert_eq!(library.text_resources().revision_stamp(),stamp);
    table.entries.get_mut("hello").unwrap().source="Changed".into(); library.text_resources_mut().publish(table).unwrap();
    assert_eq!(query(&library,"__rvn_string_table_source",&[id,"hello"]).unwrap(),Value::Str("Changed".into()));
    assert_eq!(query(&snapshot,"__rvn_string_table_source",&[id,"hello"]).unwrap(),Value::Str("Source {Name}".into()));
    assert_eq!(query(&FunctionLibrary::default(),"__rvn_registered_string_tables",&[]).unwrap(),Value::List(vec![]));
}

#[test]
fn string_table_queries_reject_bad_calls_and_bound_large_result_without_mutation() {
    use rvn_core::eval::{FunctionLibrary,EvalError}; use rvn_parser::{Expr,Value};
    let mut library=FunctionLibrary::default();
    let mut table=asset(&"X".repeat(1_048_577));
    for i in 0..4096 { table.entries.insert(format!("k{i:04}"),StringTableEntry{source:String::new(),developer_notes:String::new(),metadata:Default::default()}); }
    library.text_resources_mut().publish(table).unwrap(); let stamp=library.text_resources().revision_stamp();
    assert!(matches!(query(&library,"__rvn_string_table_source",&["asset:table-fixture","hello"]),Err(EvalError::ExecutionLimit{..})));
    assert!(matches!(query(&library,"__rvn_string_table_keys",&["asset:table-fixture"]),Err(EvalError::ExecutionLimit{..})));
    assert!(query(&library,"__rvn_string_table_metadata",&["asset:table-fixture","hello"]).is_err());
    assert!(library.eval(&Expr::Call{name:"__rvn_string_table_registered".into(),args:vec![Expr::Int(3)]},&HashMap::new()).is_err());
    assert_eq!(query(&library,"__rvn_string_table_registered",&["asset:table-fixture"]).unwrap(),Value::Bool(true));
    assert_eq!(library.text_resources().revision_stamp(),stamp);
}

#[test]
fn catalog_snapshots_share_immutable_sources_until_replacement_without_stale_results() {
    let r = reference("asset:table-fixture", "hello");
    let mut live = StringTableResources::default();
    live.publish(asset("Before")).unwrap();
    let snapshot = live.clone();
    let table = |resources: &StringTableResources| match resources.resolve(&r).unwrap() {
        StringTableResolution::Ready { table, .. } => table as *const StringTableAsset,
        _ => panic!("ready snapshot"),
    };
    assert_eq!(table(&live), table(&snapshot));
    assert_eq!(live.revision_stamp(), snapshot.revision_stamp());
    live.publish(asset("After")).unwrap();
    assert_ne!(table(&live), table(&snapshot));
    assert_ne!(live.revision_stamp(), snapshot.revision_stamp());
    assert_eq!(live.display(&r, None).unwrap(), "After");
    assert_eq!(snapshot.display(&r, None).unwrap(), "Before");
    live.unload(&r.table).unwrap();
    assert_eq!(
        live.resolve(&r).unwrap(),
        StringTableResolution::MissingTable
    );
    assert_eq!(snapshot.display(&r, None).unwrap(), "Before");
}

#[test]
fn host_shared_publication_noop_preserves_snapshot_identity_and_loading_is_not_a_noop() {
    use std::sync::Arc;
    let mut live = StringTableResources::default();
    let source = Arc::new(asset("First"));
    live.publish_shared(source.clone()).unwrap();
    let stamp = live.revision_stamp();
    let snapshot = live.clone();
    assert!(std::ptr::eq(live.ready_table(&source.id).unwrap(), source.as_ref()));
    assert_eq!(live.publish_if_changed(Arc::new(asset("First"))).unwrap(), None);
    assert_eq!(live.revision_stamp(), stamp);
    assert_eq!(source.content_bytes().unwrap(), source.id.len()+source.namespace.len()+"hello".len()+"First".len()+"Salutation — été".len());
    let token = live.begin_load(&source.id).unwrap();
    assert!(live.ready_table(&source.id).is_none());
    assert!(live.publish_if_changed(source.clone()).unwrap().is_some());
    assert!(!live.finish_load(&token, asset("Stale")).unwrap());
    assert_eq!(snapshot.ready_table(&source.id).unwrap().entries["hello"].source, "First");
    assert!(live.publish_if_changed(Arc::new(asset("New"))).unwrap().is_some());
    assert_eq!(snapshot.ready_table(&source.id).unwrap().entries["hello"].source, "First");
}

#[test]
fn divergent_snapshots_cannot_complete_each_others_same_revision_loads() {
    let mut left = StringTableResources::default();
    left.publish(asset("Initial")).unwrap();
    let mut right = left.clone();
    let left_token = left.begin_load("asset:table-fixture").unwrap();
    let right_token = right.begin_load("asset:table-fixture").unwrap();
    assert_eq!(left.revision(), right.revision());
    assert_ne!(left_token, right_token);
    let before = right.clone();
    let stamp = right.revision_stamp();
    assert!(!right
        .finish_load(&left_token, asset("Wrong branch"))
        .unwrap());
    assert!(!right.fail_load(&left_token, "Wrong branch".into()).unwrap());
    assert_eq!(right, before);
    assert_eq!(right.revision_stamp(), stamp);
    assert!(right.finish_load(&right_token, asset("Right")).unwrap());
    assert!(left.finish_load(&left_token, asset("Left")).unwrap());
    assert_eq!(
        right
            .display(&reference("asset:table-fixture", "hello"), None)
            .unwrap(),
        "Right"
    );
    assert_eq!(
        left.display(&reference("asset:table-fixture", "hello"), None)
            .unwrap(),
        "Left"
    );
}

#[test]
fn reference_survives_serialization_without_freezing_display_or_notes() {
    let persistent = reference("asset:table-fixture", "hello");
    let encoded = serde_json::to_string(&persistent).unwrap();
    assert_eq!(encoded, r#"{"table":"asset:table-fixture","key":"hello"}"#);
    let restored: StringTableReference = serde_json::from_str(&encoded).unwrap();
    restored.validate().unwrap();
    let table = asset("Bonjour {Name}!");
    assert_eq!(
        serde_json::from_str::<StringTableAsset>(&serde_json::to_string(&table).unwrap()).unwrap(),
        table
    );
    let mut resources = StringTableResources::default();
    assert_eq!(
        resources.resolve(&restored).unwrap(),
        StringTableResolution::MissingTable
    );
    resources.publish(table).unwrap();
    assert_eq!(
        resources.display(&restored, None).unwrap(),
        "Bonjour {Name}!"
    );
    resources.publish(asset("Bonsoir {Name}!")).unwrap();
    assert_eq!(
        resources.display(&restored, None).unwrap(),
        "Bonsoir {Name}!"
    );
    assert_eq!(serde_json::to_string(&restored).unwrap(), encoded);
}

#[test]
fn async_completion_cannot_resurrect_unloaded_or_replaced_tables() {
    let mut resources = StringTableResources::default();
    let r = reference("asset:table-fixture", "hello");
    let first = resources.begin_load(&r.table).unwrap();
    assert_eq!(
        resources.resolve(&r).unwrap(),
        StringTableResolution::Loading
    );
    let second = resources.begin_load(&r.table).unwrap();
    let before = resources.clone();
    assert!(!resources.finish_load(&first, asset("stale")).unwrap());
    assert_eq!(resources, before);
    assert!(resources.finish_load(&second, asset("current")).unwrap());
    let before = resources.clone();
    assert!(!resources.fail_load(&second, "late failure".into()).unwrap());
    assert_eq!(resources, before);
    let pending = resources.begin_load(&r.table).unwrap();
    resources.unload(&r.table).unwrap();
    let before = resources.clone();
    assert!(!resources
        .finish_load(&pending, asset("resurrected"))
        .unwrap());
    assert_eq!(resources, before);
    assert_eq!(
        resources.resolve(&r).unwrap(),
        StringTableResolution::MissingTable
    );
}

#[test]
fn failed_load_is_distinct_from_missing_and_can_be_retried() {
    let r = reference("asset:table-fixture", "hello");
    let mut resources = StringTableResources::default();
    let token = resources.begin_load(&r.table).unwrap();
    assert!(resources
        .fail_load(&token, "invalid authored asset".into())
        .unwrap());
    assert_eq!(
        resources.resolve(&r).unwrap(),
        StringTableResolution::Failed("invalid authored asset")
    );
    assert!(resources
        .display(&r, None)
        .unwrap_err()
        .contains("invalid authored asset"));
    let retry = resources.begin_load(&r.table).unwrap();
    assert!(resources.finish_load(&retry, asset("recovered")).unwrap());
    assert_eq!(resources.display(&r, None).unwrap(), "recovered");
}

#[test]
fn publication_rejects_invalid_metadata_and_mismatched_loads_atomically() {
    let mut resources = StringTableResources::default();
    resources.publish(asset("original")).unwrap();
    let before = resources.clone();
    let mut invalid = asset("changed");
    invalid.namespace = "invalid namespace".into();
    assert!(resources.publish(invalid).is_err());
    assert_eq!(resources, before);
    let token = resources.begin_load("asset:another-table").unwrap();
    let before = resources.clone();
    assert!(resources.finish_load(&token, asset("wrong ID")).is_err());
    assert_eq!(resources, before);
    assert!(resources.fail_load(&token, "x".repeat(4097)).is_err());
    assert_eq!(resources, before);
}

#[test]
fn missing_or_cleared_entry_never_returns_a_cached_previous_source() {
    let r = reference("asset:table-fixture", "hello");
    let mut resources = StringTableResources::default();
    resources.publish(asset("old")).unwrap();
    let old_revision = resources.revision();
    let mut replacement = asset("new");
    replacement.entries.clear();
    resources.publish(replacement).unwrap();
    assert!(resources.revision() > old_revision);
    assert_eq!(
        resources.resolve(&r).unwrap(),
        StringTableResolution::MissingEntry
    );
    assert!(resources.display(&r, None).is_err());
    resources.publish(asset("")).unwrap();
    assert_eq!(resources.display(&r, None).unwrap(), "");
}

#[test]
fn redirects_keep_persistent_identity_and_reject_cycles_without_mutation() {
    let r = reference("asset:old-table", "old-key");
    let intermediary = reference("asset:middle-table", "middle-key");
    let target = reference("asset:table-fixture", "hello");
    let mut resources = StringTableResources::default();
    resources.publish(asset("current source")).unwrap();
    resources.redirect(r.clone(), intermediary.clone()).unwrap();
    resources
        .redirect(intermediary.clone(), target.clone())
        .unwrap();
    assert_eq!(resources.display(&r, None).unwrap(), "current source");
    assert_eq!(r, reference("asset:old-table", "old-key"));
    let before = resources.clone();
    assert!(resources.redirect(target.clone(), r.clone()).is_err());
    assert_eq!(resources, before);
    assert!(!resources.redirect(r.clone(), intermediary.clone()).unwrap());
    assert_eq!(resources, before);
    resources.remove_redirect(&intermediary).unwrap();
    assert_eq!(
        resources.resolve(&r).unwrap(),
        StringTableResolution::MissingTable
    );
}

#[test]
fn culture_uses_table_identity_and_fallback_without_legacy_source_collision() {
    let table = asset("Bonjour");
    let metadata = table.localization("hello").unwrap();
    let key = metadata.identity().unwrap().locale_key();
    let cache = HashMap::from([
        (
            "fr".into(),
            LocaleTable {
                lang: "fr".into(),
                strings: HashMap::from([(key.clone(), "Salut".into())]),
            },
        ),
        (
            "en".into(),
            LocaleTable {
                lang: "en".into(),
                strings: HashMap::from([
                    (key, "Hello".into()),
                    ("Bonjour".into(), "unrelated legacy translation".into()),
                ]),
            },
        ),
        (
            "de".into(),
            LocaleTable {
                lang: "de".into(),
                strings: HashMap::new(),
            },
        ),
    ]);
    let mut locale = LocaleManager::from_tables(
        "unused",
        "fr",
        "en",
        vec!["fr".into(), "en".into(), "de".into()],
        cache,
    );
    let r = reference("asset:table-fixture", "hello");
    let mut resources = StringTableResources::default();
    resources.publish(table).unwrap();
    assert_eq!(resources.display(&r, Some(&locale)).unwrap(), "Hello");
    locale.set_language("de").unwrap();
    assert_eq!(resources.display(&r, Some(&locale)).unwrap(), "Salut");
    let mut renamed = asset("source fallback");
    renamed.namespace = "VN.NewNamespace".into();
    resources.publish(renamed).unwrap();
    assert_eq!(
        resources.display(&r, Some(&locale)).unwrap(),
        "source fallback"
    );
    assert_eq!(r.key, "hello");
}

#[test]
fn invalid_identities_and_oversized_tables_fail_before_publication() {
    let mut resources = StringTableResources::default();
    for key in [
        "",
        "two words",
        "[key]",
        "comma,key",
        "quote'key",
        "newline\nkey",
    ] {
        assert!(resources
            .resolve(&reference("asset:table-fixture", key))
            .is_err());
    }
    assert!(resources.begin_load(" \t").is_err());
    let before = resources.clone();
    assert!(resources
        .publish(asset(&"x".repeat(8 * 1_048_576)))
        .is_err());
    assert_eq!(resources, before);
    assert!(!resources.unload("missing").unwrap());
    assert!(!resources
        .remove_redirect(&reference("asset:table-fixture", "hello"))
        .unwrap());
    assert_eq!(resources, before);
}

#[test]
fn tokens_from_another_host_or_recreated_registry_cannot_complete_a_load() {
    let mut first = StringTableResources::default();
    let mut second = StringTableResources::default();
    let a = first.begin_load("asset:table-fixture").unwrap();
    let b = second.begin_load("asset:table-fixture").unwrap();
    assert_ne!(a, b);
    assert_eq!(a, a.clone());
    let before = second.clone();
    assert!(!second.finish_load(&a, asset("foreign completion")).unwrap());
    assert_eq!(second, before);
    assert!(second.finish_load(&b, asset("own completion")).unwrap());
    let mut recreated = StringTableResources::default();
    recreated.begin_load("asset:table-fixture").unwrap();
    let before = recreated.clone();
    assert!(!recreated.fail_load(&a, "old host error".into()).unwrap());
    assert_eq!(recreated, before);
}

#[test]
fn redirect_chains_are_bounded_even_when_a_later_edit_extends_a_predecessor() {
    let mut resources = StringTableResources::default();
    for index in 0..64 {
        resources
            .redirect(
                reference("asset:chain", &index.to_string()),
                reference("asset:chain", &(index + 1).to_string()),
            )
            .unwrap();
    }
    assert_eq!(
        resources.resolve(&reference("asset:chain", "0")).unwrap(),
        StringTableResolution::MissingTable
    );
    resources
        .redirect(
            reference("asset:chain", "64"),
            reference("asset:chain", "65"),
        )
        .unwrap();
    assert_eq!(
        resources.resolve(&reference("asset:chain", "0")).unwrap(),
        StringTableResolution::RedirectLimit
    );
    let before = resources.clone();
    assert!(resources
        .redirect(reference("asset:old", "x"), reference("asset:chain", "0"))
        .is_err());
    assert_eq!(resources, before);
}

// Regression coverage for the additive typed table boundary.
fn typed_query(library: &rvn_core::eval::FunctionLibrary, name: &str, values: &[rvn_parser::Value]) -> Result<rvn_parser::Value, rvn_core::eval::EvalError> {
    let vars: HashMap<_, _> = values.iter().enumerate().map(|(i, value)| (format!("arg{i}"), value.clone())).collect();
    library.eval(&rvn_parser::Expr::Call { name: name.into(), args: (0..values.len()).map(|i| rvn_parser::Expr::Var(format!("arg{i}"))).collect() }, &vars)
}
fn typed_name(text: &str) -> rvn_parser::Value { rvn_parser::Value::Name(rvn_parser::Name::new(text).unwrap()) }

#[test]
fn typed_table_name_queries_preserve_legacy_results_and_snapshot_identity() {
    use rvn_parser::Value;
    let mut library = rvn_core::eval::FunctionLibrary::default();
    let mut table = asset("Hello");
    table.entries.get_mut("hello").unwrap().metadata.insert("Speaker".into(), "Éloïse".into());
    library.text_resources_mut().publish(table).unwrap();
    let before = library.text_resources().revision_stamp();
    let id = typed_name("ASSET:TABLE-FIXTURE");
    assert_eq!(typed_query(&library, "__rvn_string_table_registered_name", &[id.clone()]).unwrap(), Value::Bool(true));
    assert_eq!(typed_query(&library, "__rvn_string_table_source_name", &[id.clone(), Value::Str("HELLO".into())]).unwrap(), Value::Str(String::new()));
    assert_eq!(typed_query(&library, "__rvn_string_table_metadata_name", &[id.clone(), Value::Str("hello".into()), typed_name("speaker")]).unwrap(), Value::Str("Éloïse".into()));
    assert_eq!(typed_query(&library, "__rvn_registered_string_tables_name", &[]).unwrap(), Value::List(vec![typed_name("asset:table-fixture")]));
    assert_eq!(typed_query(&library, "__rvn_string_table_metadata_ids_name", &[id.clone(), Value::Str("hello".into())]).unwrap(), Value::List(vec![typed_name("Speaker")]));
    for (function, arguments, spelling) in [
        ("__rvn_registered_string_tables_name", vec![], "asset:table-fixture"),
        ("__rvn_string_table_metadata_ids_name", vec![id.clone(), Value::Str("hello".into())], "Speaker"),
    ] {
        let Value::List(items) = typed_query(&library, function, &arguments).unwrap() else { panic!("typed collection expected") };
        let Value::Name(first) = &items[0] else { panic!("Name element expected") };
        assert_eq!(first.as_str(), spelling, "Name equality must not conceal display spelling changes");
    }
    assert_eq!(typed_query(&library, "__rvn_string_table_keys_name", &[id]).unwrap(), Value::List(vec![Value::Str("hello".into())]));
    // Old scripts retain exact String outputs and their case-sensitive query behavior.
    assert_eq!(query(&library, "__rvn_registered_string_tables", &[]).unwrap(), Value::List(vec![Value::Str("asset:table-fixture".into())]));
    assert_eq!(query(&library, "__rvn_string_table_registered", &["ASSET:TABLE-FIXTURE"]).unwrap(), Value::Bool(false));
    assert_eq!(library.text_resources().revision_stamp(), before);
    let reference = query(&library, "__rvn_string_table_asset", &["asset:table-fixture"]).unwrap();
    library.text_resources_mut().unload("asset:table-fixture").unwrap();
    let retained = typed_query(&library, "__rvn_string_table_id_name", &[reference]).unwrap();
    assert_eq!(retained, typed_name("asset:table-fixture"));
    let Value::Name(retained_name) = &retained else { panic!("Name identity expected") };
    assert_eq!(retained_name.as_str(), "asset:table-fixture");
    let encoded = serde_json::to_string(&retained).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&encoded).unwrap(), retained);
    assert_eq!(typed_query(&library, "__rvn_string_table_id_name", &[Value::Str(String::new())]).unwrap(), typed_name("None"));
}

#[test]
fn typed_table_queries_reject_wrong_types_and_legacy_name_collisions_without_mutation() {
    use rvn_parser::Value;
    let mut library = rvn_core::eval::FunctionLibrary::default();
    library.text_resources_mut().publish(asset("First")).unwrap();
    assert!(typed_query(&library, "__rvn_string_table_registered_name", &[Value::Str("asset:table-fixture".into())]).is_err());
    assert!(typed_query(&library, "__rvn_string_table_metadata_name", &[typed_name("asset:table-fixture"), typed_name("hello"), typed_name("Speaker")]).is_err());
    assert!(typed_query(&library, "__rvn_string_table_registered_name", &[]).is_err());
    for absent in ["", "None", "_1", "missing"] {
        assert_eq!(typed_query(&library, "__rvn_string_table_registered_name", &[typed_name(absent)]).unwrap(), Value::Bool(false));
    }
    let mut peer = asset("Second"); peer.id = "ASSET:TABLE-FIXTURE".into();
    library.text_resources_mut().publish(peer).unwrap();
    let snapshot = library.text_resources().clone();
    assert!(typed_query(&library, "__rvn_registered_string_tables_name", &[]).is_err());
    assert!(typed_query(&library, "__rvn_string_table_registered_name", &[typed_name("asset:table-fixture")]).is_err());
    assert_eq!(query(&library, "__rvn_string_table_source", &["asset:table-fixture", "hello"]).unwrap(), Value::Str("First".into()));
    assert_eq!(query(&library, "__rvn_string_table_source", &["ASSET:TABLE-FIXTURE", "hello"]).unwrap(), Value::Str("Second".into()));
    assert_eq!(library.text_resources(), &snapshot);
}

#[test]
fn typed_table_queries_diagnose_incompatible_legacy_ids_without_discarding_them() {
    for raw in ["None".to_owned(), "_1".to_owned(), "X".repeat(1024)] {
        let mut library = rvn_core::eval::FunctionLibrary::default();
        let mut table = asset("Retained"); table.id = raw.clone();
        library.text_resources_mut().publish(table).unwrap();
        let before = library.text_resources().clone();
        assert!(typed_query(&library, "__rvn_registered_string_tables_name", &[]).is_err());
        assert_eq!(query(&library, "__rvn_string_table_source", &[&raw, "hello"]).unwrap(), rvn_parser::Value::Str("Retained".into()));
        assert_eq!(library.text_resources(), &before);
    }
}

#[test]
fn typed_table_metadata_list_budgets_both_name_strings_before_result_allocation() {
    use rvn_core::eval::EvalError;
    use rvn_parser::Value;
    let mut library = rvn_core::eval::FunctionLibrary::default();
    let mut table = asset("Budget");
    // Legacy String result fits 1 MiB; typed Names store spelling + identity.
    table.entries.get_mut("hello").unwrap().metadata = (0..2048).map(|index| (format!("{index:04}{}", "x".repeat(300)), String::new())).collect();
    library.text_resources_mut().publish(table).unwrap();
    let before = library.text_resources().clone();
    let legacy = query(&library, "__rvn_string_table_metadata_ids", &["asset:table-fixture", "hello"]).unwrap();
    assert!(matches!(legacy, Value::List(ref items) if items.len() == 2048));
    assert!(matches!(typed_query(&library, "__rvn_string_table_metadata_ids_name", &[typed_name("asset:table-fixture"), Value::Str("hello".into())]), Err(EvalError::ExecutionLimit { .. })));
    assert_eq!(library.text_resources(), &before);
}

mod name_table_save_checkpoint_0246 {
use rvn_core::{save::SaveData, string_table::{StringTableAsset, StringTableEntry, StringTableResources}, Engine, Interaction, LoadCompatibility, TerminalRenderer};
use rvn_parser::{Name, Value};
use std::collections::BTreeMap;

fn table(source: &str) -> StringTableAsset {
    StringTableAsset {
        id: "asset:Cold0246Case".into(),
        namespace: "VN.Cold0246".into(),
        entries: BTreeMap::from([("hello".into(), StringTableEntry {
            source: source.into(), developer_notes: String::new(),
            metadata: BTreeMap::from([("Speaker".into(), "Éloïse".into())]),
        })]),
    }
}

#[test]
fn typed_table_names_and_lists_restore_from_save_without_loading_then_resolve_new_publication() {
    let source = r#"
init {
    set reference = __rvn_string_table_asset("asset:Cold0246Case")
    set table_id = __rvn_string_table_id_name(reference)
    set upper_id = __rvn_string_to_name("ASSET:COLD0246CASE")
    set names = __rvn_registered_string_tables_name()
    set metadata_names = __rvn_string_table_metadata_ids_name(table_id, "hello")
    set legacy_names = __rvn_registered_string_tables()
    set legacy_id = __rvn_string_table_id(reference)
}
label start
"Before"
set registered = __rvn_string_table_registered_name(upper_id)
set fresh = __rvn_string_table_source_name(upper_id, "hello")
"After [fresh]"
set speaker = __rvn_string_table_metadata_name(upper_id, "hello", __rvn_string_to_name("speaker"))
"Speaker [speaker]"
"#;
    let script = rvn_parser::parse(source).unwrap();
    let mut resources = StringTableResources::default();
    resources.publish(table("Original")).unwrap();
    let mut original = Engine::new_with_text_resources(script.clone(), TerminalRenderer, 16, resources).unwrap();
    assert!(matches!(original.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Before"));
    let saved = SaveData::from_state(&original.state, 1, "typed tables".into(), "main.rvn".into());
    let encoded = serde_json::to_value(&saved).unwrap();
    assert_eq!(encoded["vars"]["table_id"], serde_json::json!({"type":"Name","v":"asset:Cold0246Case"}));
    assert_eq!(encoded["vars"]["upper_id"], serde_json::json!({"type":"Name","v":"ASSET:COLD0246CASE"}));
    assert_eq!(encoded["vars"]["names"], serde_json::json!({"type":"List","v":[{"type":"Name","v":"asset:Cold0246Case"}]}));
    assert_eq!(encoded["vars"]["metadata_names"], serde_json::json!({"type":"List","v":[{"type":"Name","v":"Speaker"}]}));
    assert_eq!(encoded["vars"]["legacy_names"], serde_json::json!({"type":"List","v":[{"type":"Str","v":"asset:Cold0246Case"}]}));
    assert_eq!(encoded["vars"]["legacy_id"], serde_json::json!({"type":"Str","v":"asset:Cold0246Case"}));
    let serialized = serde_json::to_vec(&saved).unwrap();

    let mut missing = Engine::new(script.clone(), TerminalRenderer, 16).unwrap();
    let before = missing.text_resources().clone();
    assert_eq!(missing.load_data(serde_json::from_slice(&serialized).unwrap()).unwrap(), LoadCompatibility::Verified);
    assert_eq!(missing.text_resources(), &before);
    let Value::Name(id) = &missing.state.vars["table_id"] else { panic!("table identity changed type") };
    assert_eq!(id.as_str(), "asset:Cold0246Case");
    assert_eq!(id, &Name::new("ASSET:COLD0246CASE").unwrap());
    missing.advance_dialogue().unwrap();
    assert!(matches!(missing.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "After "));
    assert_eq!(missing.state.vars["registered"], Value::Bool(false));
    assert_eq!(missing.state.vars["fresh"], Value::Str(String::new()));
    assert_eq!(missing.text_resources(), &before);
    assert_eq!(serde_json::to_value(&missing.state.vars["names"]).unwrap(), serde_json::json!({"List":[{"Name":"asset:Cold0246Case"}]}));
    assert_eq!(missing.state.vars["reference"], original.state.vars["reference"]);

    let mut published = Engine::new(script, TerminalRenderer, 16).unwrap();
    assert_eq!(published.load_data(serde_json::from_slice(&serialized).unwrap()).unwrap(), LoadCompatibility::Verified);
    published.text_resources_mut().publish(table("Changed after cold load")).unwrap();
    let snapshot = published.text_resources().clone();
    published.advance_dialogue().unwrap();
    assert!(matches!(published.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "After Changed after cold load"));
    assert_eq!(published.state.vars["registered"], Value::Bool(true));
    published.advance_dialogue().unwrap();
    assert!(matches!(published.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Speaker Éloïse"));
    assert_eq!(published.text_resources(), &snapshot);
    assert_eq!(serde_json::to_value(&published.state.vars["metadata_names"]).unwrap(), serde_json::json!({"List":[{"Name":"Speaker"}]}));
    assert_eq!(published.state.vars["legacy_id"], Value::Str("asset:Cold0246Case".into()));
}

}
