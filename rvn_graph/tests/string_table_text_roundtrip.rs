use rvn_core::{
    string_table::{StringTableAsset, StringTableEntry, StringTableResources},
    Engine, Interaction, TerminalRenderer,
};
use rvn_graph::*;

#[test]
fn typed_string_table_asset_id_roundtrips_and_executes_with_null_and_missing_assets() {
    use rvn_parser::Value;
    let source=r#"init {
set reference=__rvn_string_table_asset("asset:table")
set identity=__rvn_string_table_id(reference)
set direct=__rvn_string_table_id(__rvn_string_table_asset("asset:table"))
set missing=__rvn_string_table_id(__rvn_string_table_asset("missing"))
set absent=__rvn_string_table_id(__rvn_string_table_asset(""))
set raw=__rvn_string_table_source(identity,"key")
}
scene label start
"[raw]"
"#;
    let script=rvn_parser::parse(source).unwrap();
    let graphs=import_script(&script).unwrap();
    let init=graphs.iter().find(|g|g.kind==GraphKind::Init).unwrap();
    assert_eq!(init.variables["reference"].value_type,ValueType::Asset(AssetKind::StringTable));
    for node in init.nodes.values().filter(|n|n.kind==NodeKind::StringTableId) {
        assert_eq!(init.pin_by_key(node.id,"asset").unwrap().value_type,ValueType::Asset(AssetKind::StringTable));
        assert_eq!(init.effective_pin_type(init.pin_by_key(node.id,"result").unwrap().id),Some(ValueType::String));
    }
    let cold:Vec<_>=graphs.iter().map(|g|GraphDocument::from_json(&serde_json::to_string(g).unwrap()).unwrap()).collect();
    assert_eq!(cold,graphs);assert_eq!(reimport_script(&script,&cold).unwrap(),cold);
    let compiled=transpile_project(&cold).unwrap();assert_eq!(&compiled.ast[..script.len()],script.as_slice());
    let mut resources=StringTableResources::default();
    resources.publish(StringTableAsset{id:"asset:table".into(),namespace:"VN".into(),entries:[("key".into(),StringTableEntry{source:"Raw {Name}".into(),developer_notes:String::new(),metadata:Default::default()})].into()}).unwrap();
    let mut game=Engine::new_with_text_resources(compiled.ast,TerminalRenderer,8,resources).unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Raw {Name}"));
    for key in ["identity","direct"] {assert_eq!(game.state.vars[key],Value::Str("asset:table".into()));}
    for key in ["missing","absent"] {assert_eq!(game.state.vars[key],Value::Str(String::new()));}
}

#[test]
fn string_table_asset_pins_refuse_string_and_other_assets_and_report_invalid_identity() {
    let ty=ValueType::Asset(AssetKind::StringTable);
    for other in [ValueType::String,ValueType::InterpolatedText,ValueType::Asset(AssetKind::Script),ValueType::Asset(AssetKind::Music),ValueType::Int] {
        assert!(!ty.accepts(&other));assert!(ty.conversion_path_from(&other).is_none());
    }
    for bad in ["7","true","\"asset:table\"","len(\"id\")","__rvn_invariant_text(\"id\")"] {
        let source=format!("init {{set id=__rvn_string_table_id({bad})}}\nscene label start\n");
        assert!(import_script(&rvn_parser::parse(&source).unwrap()).is_err(),"{bad}");
    }
    for bad in ["7","true","len(\"id\")","__rvn_invariant_text(\"id\")"] {
        let source=format!("init {{set id=__rvn_string_table_asset({bad})}}\nscene label start\n");
        assert!(import_script(&rvn_parser::parse(&source).unwrap()).is_err(),"{bad}");
    }
    for source in ["function probe(id) {return __rvn_string_table_id(__rvn_string_table_asset(id))}\nscene label start\n", "init {set id=__rvn_string_table_id(\"\")}\nscene label start\n"] {
        let graphs=import_script(&rvn_parser::parse(source).unwrap()).unwrap();
        let compiled=transpile_project(&graphs).unwrap();
        assert!(import_script(&compiled.ast).is_ok());
    }
    let mut graph=GraphDocument::new(GraphId::new(19),GraphKind::Init);
    let reference=graph.add_catalog_node(NodeKind::StringTableAsset,[0.,0.]).unwrap();
    graph.set_property(reference,"table",PropertyValue::String("bad\nidentity".into())).unwrap();
    let diagnostics=graph.validate();
    assert!(diagnostics.iter().any(|d|d.code=="invalid_string_table_asset"&&d.node==Some(reference)));
    assert!(transpile(&graph).is_err());
}

#[test]
fn older_authored_table_callable_names_keep_their_own_graph_and_runtime_behavior() {
    let source=r#"function string_table_source(table,key) {return "Authored older function"}
init {set raw=string_table_source("asset:table","key")}
scene label start
"[raw]"
"#;
    let script=rvn_parser::parse(source).unwrap();
    assert!(rvn_parser::validate_logic(&script,false).is_empty());
    let graphs=import_script(&script).unwrap();
    let init=graphs.iter().find(|g|matches!(g.kind,GraphKind::Init)).unwrap();
    assert!(!init.nodes.values().any(|n|n.kind.is_string_table_query()));
    assert!(init.nodes.values().any(|n|n.kind==NodeKind::FunctionCall&&n.properties.get("function")==Some(&PropertyValue::String("string_table_source".into()))));
    let cold:Vec<_>=graphs.iter().map(|g|GraphDocument::from_json(&serde_json::to_string(g).unwrap()).unwrap()).collect();
    let compiled=transpile_project(&cold).unwrap();
    let mut game=Engine::new(compiled.ast,TerminalRenderer,8).unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Authored older function"));
}

#[test]
fn string_table_query_nodes_roundtrip_typed_rvn_and_execute_metadata_on_existing_engine() {
    use rvn_parser::Value;
    let source=r#"init {
set registered=__rvn_string_table_registered("asset:table")
set present=__rvn_string_table_entry_registered("asset:table","empty")
set namespace=__rvn_string_table_namespace("asset:table")
set raw=__rvn_string_table_source("asset:table","key")
set meta=__rvn_string_table_metadata("asset:table","key","speaker")
set tables=__rvn_registered_string_tables()
set keys=__rvn_string_table_keys("asset:table")
set metadata=__rvn_string_table_metadata_ids("asset:table","key")
}
scene label start
"[raw] [meta]"
"#;
    let script=rvn_parser::parse(source).unwrap(); assert!(rvn_parser::validate_logic(&script,false).is_empty());
    let graphs=import_script(&script).unwrap();
    let init=graphs.iter().find(|g|matches!(g.kind,GraphKind::Init)).unwrap();
    assert_eq!(init.nodes.values().filter(|n|n.kind.is_string_table_query()).count(),8);
    for kind in ALL_NODE_KINDS.iter().copied().filter(|k|k.is_string_table_query()) {
        let (name,keys,result)=kind.string_table_query_with_contract(StringTableQueryContract::LegacyStringV1).unwrap();
        assert!(rvn_parser::builtin_arity(name).unwrap().contains(&keys.len()));
        let node=init.nodes.values().find(|n|n.kind==kind).unwrap();
        assert_eq!(init.effective_pin_type(init.pin_by_key(node.id,"result").unwrap().id),Some(result));
        assert!(!node_definition(kind).pins.iter().any(|pin|pin.value_type.is_execution()));
        for key in keys {assert_eq!(init.pin_by_key(node.id,key).unwrap().value_type,ValueType::String);}
    }
    let cold:Vec<_>=graphs.iter().map(|g|GraphDocument::from_json(&serde_json::to_string(g).unwrap()).unwrap()).collect();
    assert_eq!(cold,graphs); assert_eq!(reimport_script(&script,&cold).unwrap(),cold);
    let compiled=transpile_project(&cold).unwrap(); assert_eq!(&compiled.ast[..script.len()],script.as_slice());
    let mut resources=StringTableResources::default();
    resources.publish(StringTableAsset{id:"asset:table".into(),namespace:"VN".into(),entries:[
        ("key".into(),StringTableEntry{source:"Source {Name}".into(),developer_notes:String::new(),metadata:[("Speaker".into(),"Éloïse".into())].into()}),
        ("empty".into(),StringTableEntry{source:String::new(),developer_notes:String::new(),metadata:Default::default()})].into()}).unwrap();
    let mut game=Engine::new_with_text_resources(compiled.ast,TerminalRenderer,8,resources).unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Source {Name} Éloïse"));
    assert_eq!(game.state.vars["registered"],Value::Bool(true)); assert_eq!(game.state.vars["present"],Value::Bool(true));
    assert_eq!(game.state.vars["namespace"],Value::Str("VN".into()));
    assert_eq!(game.state.vars["tables"],Value::List(vec![Value::Str("asset:table".into())]));
    assert_eq!(game.state.vars["keys"],Value::List(vec![Value::Str("empty".into()),Value::Str("key".into())]));
    assert_eq!(game.state.vars["metadata"],Value::List(vec![Value::Str("Speaker".into())]));
}

#[test]
fn string_table_query_inline_defaults_remain_editable_and_bad_typed_inputs_are_rejected() {
    for kind in ALL_NODE_KINDS.iter().copied().filter(|k|k.is_string_table_query()) {
        let mut graph=GraphDocument::new(GraphId::new(77),GraphKind::Function{name:"probe".into()});
        let node=graph.add_catalog_node(kind,[200.,100.]).unwrap();
        let (_,keys,_)=kind.string_table_query().unwrap();
        for key in keys {let value=if graph.pin_by_key(node,key).unwrap().value_type==ValueType::Name {PropertyValue::Name(NameDefault::new(&format!("value-{key}")).unwrap())}else{PropertyValue::String(format!("value-{key}"))};graph.set_pin_default(node,key,value).unwrap();}
        assert_eq!(graph.materialize_visible_defaults().unwrap(),0);
        for key in keys {let expected=if graph.pin_by_key(node,key).unwrap().value_type==ValueType::Name {PropertyValue::Name(NameDefault::new(&format!("value-{key}")).unwrap())}else{PropertyValue::String(format!("value-{key}"))};assert_eq!(graph.pin_by_key(node,key).unwrap().default_value,Some(expected));}
    }
    let bad=rvn_parser::parse("init {set result=__rvn_string_table_registered(7)}\nscene label start\n").unwrap();
    assert!(import_script(&bad).is_err());
    for expression in ["len(\"key\")", "true", "__rvn_invariant_text(\"id\")"] {
        let source=format!("init {{set result=__rvn_string_table_registered({expression})}}\nscene label start\n");
        assert!(import_script(&rvn_parser::parse(&source).unwrap()).is_err(),"{expression}");
    }
    let dynamic=rvn_parser::parse("function probe(id) {return __rvn_string_table_registered(id)}\nscene label start\n").unwrap();
    assert!(import_script(&dynamic).is_ok());
    let arity=rvn_parser::parse("init {set result=__rvn_string_table_metadata(\"id\",\"key\")}\nscene label start\n").unwrap();
    assert!(!rvn_parser::validate_logic(&arity,false).is_empty()); assert!(import_script(&arity).is_err());
}

#[test]
fn generic_table_constructor_retains_typed_output_through_graph_codegen_and_cold_reimport() {
    let source = r#"init {
set pattern=__rvn_string_table_text("asset:table","key")
set message=__rvn_format_text(pattern,{"Name":"Visitor"})
}
scene label start
"[message]"
"#;
    let script = rvn_parser::parse(source).unwrap();
    assert!(rvn_parser::validate_logic(&script, false).is_empty());
    let graphs = import_script(&script).unwrap();
    let graph = graphs
        .iter()
        .find(|g| matches!(g.kind, GraphKind::Init))
        .unwrap();
    let node = graph
        .nodes
        .values()
        .find(|n| {
            n.properties.get("function")
                == Some(&PropertyValue::String("__rvn_string_table_text".into()))
        })
        .unwrap();
    assert_eq!(
        graph.effective_pin_type(graph.pin_by_key(node.id, "result").unwrap().id),
        Some(ValueType::InterpolatedText)
    );
    let cold: Vec<_> = graphs
        .iter()
        .map(|g| GraphDocument::from_json(&serde_json::to_string(g).unwrap()).unwrap())
        .collect();
    assert_eq!(cold, graphs);
    assert_eq!(reimport_script(&script, &cold).unwrap(), cold);
    let generated = transpile_project(&cold).unwrap();
    assert_eq!(&generated.ast[..script.len()], script.as_slice());
    let mut resources = StringTableResources::default();
    resources
        .publish(StringTableAsset {
            id: "asset:table".into(),
            namespace: "VN".into(),
            entries: [(
                "key".into(),
                StringTableEntry {metadata:Default::default(),
                    source: "Hello {Name}!".into(),
                    developer_notes: "Independent fixture".into(),
                },
            )]
            .into(),
        })
        .unwrap();
    let mut game =
        Engine::new_with_text_resources(generated.ast, TerminalRenderer, 8, resources).unwrap();
    assert!(
        matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Hello Visitor!")
    );
    assert_eq!(
        rvn_core::save::SaveData::try_from_state(&game.state, 1, String::new(), String::new())
            .unwrap()
            .format_version,
        13
    );
}

mod name_table_contract_0239 {
use rvn_graph::*;
use rvn_core::{Engine, Interaction, TerminalRenderer, string_table::{StringTableAsset,StringTableEntry,StringTableResources}};
use rvn_parser::Value;

fn resource() -> StringTableResources {
    let mut resources=StringTableResources::default();
    resources.publish(StringTableAsset{id:"asset:table".into(),namespace:"VN".into(),entries:[("key".into(),StringTableEntry{source:"Source".into(),developer_notes:String::new(),metadata:[("Speaker".into(),"Éloïse".into())].into()})].into()}).unwrap();
    resources
}

#[test]
fn modern_and_legacy_table_boundaries_keep_types_source_and_ids_over_three_cold_cycles() {
    let source=r#"init {
set reference=__rvn_string_table_asset("asset:table")
set oldid=__rvn_string_table_id(reference)
set id=__rvn_string_table_id_name(reference)
set tables=__rvn_registered_string_tables_name()
set oldtables=__rvn_registered_string_tables()
set keys=__rvn_string_table_keys_name(id)
set metadata=__rvn_string_table_metadata_ids_name(id,"key")
set content=__rvn_string_table_metadata_name(__rvn_literal_name("ASSET:TABLE"),"key",__rvn_literal_name("speaker"))
}
scene label start
"[content]"
return
label __blueprint_project_end
"#;
    let mut script=rvn_parser::parse(source).unwrap();
    let mut graphs=import_script(&script).unwrap();
    let initial=graphs.clone();
    for _ in 0..3 {
        let init=graphs.iter().find(|g|g.kind==GraphKind::Init).unwrap();
        assert_eq!(init.schema_version,GRAPH_NAME_TABLE_SCHEMA_VERSION);
        for (variable,ty) in [("id",ValueType::Name),("oldid",ValueType::String),("tables",ValueType::List(Box::new(ValueType::Name))),("oldtables",ValueType::List(Box::new(ValueType::String))),("keys",ValueType::List(Box::new(ValueType::String))),("metadata",ValueType::List(Box::new(ValueType::Name)))] {
            assert_eq!(init.variables[variable].value_type,ty, "{variable}");
        }
        assert!(init.nodes.values().any(|n|n.string_table_contract==Some(StringTableQueryContract::LegacyStringV1)));
        assert!(init.nodes.values().any(|n|n.string_table_contract==Some(StringTableQueryContract::NameV2)));
        assert!(init.nodes.values().all(|n|!n.properties.contains_key("string_table_contract")));
        let cold:Vec<_>=graphs.iter().map(|g|GraphDocument::from_json(&serde_json::to_string(g).unwrap()).unwrap()).collect();
        assert_eq!(cold,graphs);
        let compiled=transpile_project(&cold).unwrap();
        assert_eq!(&compiled.ast[..script.len()],script.as_slice());
        let mut game=Engine::new_with_text_resources(compiled.ast.clone(),TerminalRenderer,8,resource()).unwrap();
        assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..}) if text=="Éloïse"));
        assert!(matches!(&game.state.vars["id"],Value::Name(id) if id.as_str()=="asset:table"));
        assert_eq!(game.state.vars["oldid"],Value::Str("asset:table".into()));
        assert!(matches!(&game.state.vars["tables"],Value::List(items) if matches!(items.as_slice(),[Value::Name(id)] if id.as_str()=="asset:table")));
        assert_eq!(game.state.vars["oldtables"],Value::List(vec![Value::Str("asset:table".into())]));
        assert!(matches!(&game.state.vars["metadata"],Value::List(items) if matches!(items.as_slice(),[Value::Name(id)] if id.as_str()=="Speaker")));
        graphs=reimport_script(&compiled.ast,&cold).unwrap();
        assert_eq!(graphs,initial,"reimport must retain nodes, pins, edges and variable bindings");
        script=compiled.ast;
    }
}

#[test]
fn legacy_query_marker_migration_is_idempotent_without_promoting_headers_or_retaping_pins() {
    for version in [5,6,7,8] {
        let source=if version==6 {"scene label start\nset ids=__rvn_registered_string_tables()\n"} else {"init {set ids=__rvn_registered_string_tables()}\nlabel start\n"};
        let mut graphs=import_script(&rvn_parser::parse(source).unwrap()).unwrap();
        let graph=graphs.iter_mut().find(|g|g.nodes.values().any(|n|n.kind==NodeKind::RegisteredStringTables)).unwrap();
        if version>=7 { graph.enable_text_owner(TextOwnerId::parse("11111111111111111111111111111111").unwrap()).unwrap(); }
        if version==8 {
            let node=graph.add_catalog_node(NodeKind::TextValue,[700.,0.]).unwrap();
            let pin=graph.pin_by_key(node,"value").unwrap().id;
            graph.set_pin_text_table_reference(pin,rvn_parser::localized_text::StringTableReference{table:"22222222222222222222222222222222".into(),key:"key".into()},&[]).unwrap();
        }
        assert_eq!(graph.schema_version,version);
        let expected=graph.clone();
        let mut old=serde_json::to_value(&expected).unwrap();
        for node in old["nodes"].as_object_mut().unwrap().values_mut() { node.as_object_mut().unwrap().remove("string_table_contract"); }
        let (loaded,report)=GraphDocument::from_json_with_report(&old.to_string()).unwrap();
        assert_eq!(loaded,expected);assert_eq!((report.from,report.to),(version,version));
        assert!(report.steps.contains(&"explicit_legacy_string_table_contracts"));
        let (again,report)=GraphDocument::from_json_with_report(&serde_json::to_string(&loaded).unwrap()).unwrap();
        assert_eq!(again,loaded);assert!(!report.steps.contains(&"explicit_legacy_string_table_contracts"));
    }
}

#[test]
fn ordinary_catalogue_uses_name_pins_and_array_compatibility_is_not_silently_relaxed() {
    let mut graph=GraphDocument::new(GraphId::new(41),GraphKind::Function{name:"probe".into()});
    for kind in ALL_NODE_KINDS.iter().copied().filter(|kind|kind.has_string_table_contract()) {
        let node=graph.add_catalog_node(kind,[0.,0.]).unwrap();
        assert_eq!(graph.nodes[&node].string_table_contract,Some(StringTableQueryContract::NameV2));
        if kind==NodeKind::StringTableId { assert_eq!(graph.pin_by_key(node,"result").unwrap().value_type,ValueType::Name); }
        else {
            let (_,keys,result)=kind.string_table_query().unwrap();
            assert_eq!(graph.pin_by_key(node,"result").unwrap().value_type,result);
            for key in keys {
                let pin=graph.pin_by_key(node,key).unwrap();
                assert_eq!(pin.value_type,StringTableQueryContract::NameV2.input_type(key));
                if matches!(*key,"table"|"metadata") { assert_eq!(pin.default_value,Some(PropertyValue::Name(NameDefault::default()))); }
            }
        }
    }
    assert_eq!(graph.schema_version,9);graph.validate_narrative_metadata().unwrap();
    let names=ValueType::List(Box::new(ValueType::Name));let strings=ValueType::List(Box::new(ValueType::String));
    assert!(!strings.accepts(&names));assert!(strings.conversion_path_from(&names).is_none());
    assert!(!names.accepts(&strings));assert!(names.conversion_path_from(&strings).is_none());
    let source=graph.add_catalog_node(NodeKind::RegisteredStringTables,[0.,0.]).unwrap();
    graph.add_variable("legacy",strings.clone(),PropertyValue::StringList(vec![])).unwrap();
    let set=graph.add_catalog_node(NodeKind::SetVariable,[300.,0.]).unwrap();graph.set_property(set,"name",PropertyValue::String("legacy".into())).unwrap();
    let before=graph.clone();
    assert!(graph.connect_with_conversions(graph.pin_by_key(source,"result").unwrap().id,graph.pin_by_key(set,"value").unwrap().id,[150.,0.]).is_err());
    assert_eq!(graph,before);
}

#[test]
fn modern_schema_preserves_narrative_and_text_opt_ins_without_header_downgrades() {
    let mut graph=GraphDocument::new(GraphId::new(7),GraphKind::Label{name:"start".into()});
    graph.set_narrative_metadata(Some(NarrativeMetadata::SceneEntry)).unwrap();
    graph.add_catalog_node(NodeKind::StringTableId,[0.,0.]).unwrap();assert_eq!(graph.schema_version,9);
    graph.enable_text_owner(TextOwnerId::parse("11111111111111111111111111111111").unwrap()).unwrap();assert_eq!(graph.schema_version,9);
    let node=graph.add_catalog_node(NodeKind::TextValue,[500.,0.]).unwrap();let pin=graph.pin_by_key(node,"value").unwrap().id;
    graph.set_pin_text_table_reference(pin,rvn_parser::localized_text::StringTableReference{table:"22222222222222222222222222222222".into(),key:"key".into()},&[]).unwrap();assert_eq!(graph.schema_version,9);
    graph.set_narrative_metadata(None).unwrap();assert_eq!(graph.schema_version,9);graph.validate_narrative_metadata().unwrap();
    assert_eq!(GraphDocument::from_json(&serde_json::to_string(&graph).unwrap()).unwrap(),graph);
    let mut forged=serde_json::to_value(&graph).unwrap();forged["schema_version"]=serde_json::json!(8);
    assert!(GraphDocument::from_json(&forged.to_string()).is_err());
}

#[test]
fn modern_source_requires_explicit_name_conversion_and_rejects_wrong_identifiers() {
    for expression in ["7", "true", "\"asset:table\"", "__rvn_invariant_text(\"asset:table\")"] {
        let source=format!("init {{set present=__rvn_string_table_registered_name({expression})}}\nscene label start\n");
        assert!(import_script(&rvn_parser::parse(&source).unwrap()).is_err(),"{expression}");
    }
    for source in [
        "init {set present=__rvn_string_table_entry_registered_name(__rvn_literal_name(\"asset:table\"),__rvn_literal_name(\"key\"))}\nscene label start\n",
        "init {set present=__rvn_string_table_registered(__rvn_literal_name(\"asset:table\"))}\nscene label start\n",
    ] { assert!(import_script(&rvn_parser::parse(source).unwrap()).is_err()); }
    let converted="init {set present=__rvn_string_table_registered_name(__rvn_string_to_name(\"asset:table\"))}\nscene label start\n";
    let graphs=import_script(&rvn_parser::parse(converted).unwrap()).unwrap();assert!(transpile_project(&graphs).is_ok());
}

#[test]
fn promoting_unmarked_legacy_in_memory_graph_records_contracts_without_retaping_and_wrong_kind_is_atomic() {
    let script=rvn_parser::parse("init {set oldtables=__rvn_registered_string_tables()}\nlabel start\n").unwrap();
    let graphs=import_script(&script).unwrap();
    let mut raw=serde_json::to_value(graphs.iter().find(|g|g.kind==GraphKind::Init).unwrap()).unwrap();
    for node in raw["nodes"].as_object_mut().unwrap().values_mut() { node.as_object_mut().unwrap().remove("string_table_contract"); }
    // Exercise the public serde representation without the normal migration hook.
    let mut graph:GraphDocument=serde_json::from_value(raw).unwrap();
    assert_eq!(graph.schema_version,5);graph.validate_narrative_metadata().unwrap();
    let before=graph.clone();let compiled_before=transpile(&before).unwrap();
    assert!(graph.add_table_query_node(NodeKind::Dialogue,[99.,99.],StringTableQueryContract::NameV2).is_err());
    assert_eq!(graph,before,"wrong-kind preflight must not allocate IDs, promote schema or mark old nodes");
    let modern=graph.add_catalog_node(NodeKind::RegisteredStringTables,[600.,0.]).unwrap();
    assert_eq!(graph.schema_version,9);assert_eq!(graph.variables,before.variables);
    assert_eq!(graph.graph_id,before.graph_id);assert_eq!(graph.edges,before.edges);
    for (id,pin) in &before.pins { assert_eq!(&graph.pins[id],pin); }
    for (id,old) in &before.nodes {
        let actual=&graph.nodes[id];
        if old.kind.has_string_table_contract() { assert_eq!(actual.string_table_contract,Some(StringTableQueryContract::LegacyStringV1)); }
        let mut normalized=actual.clone();normalized.string_table_contract=old.string_table_contract;assert_eq!(&normalized,old);
    }
    assert_eq!(graph.nodes[&modern].string_table_contract,Some(StringTableQueryContract::NameV2));
    assert_eq!(graph.pin_by_key(modern,"result").unwrap().value_type,ValueType::List(Box::new(ValueType::Name)));
    graph.validate_narrative_metadata().unwrap();
    assert_eq!(transpile(&graph).unwrap().source,compiled_before.source,"unused modern producer must not rewrite old RVN semantics");
    assert_eq!(GraphDocument::from_json(&serde_json::to_string(&graph).unwrap()).unwrap(),graph);
}

}


mod qualified_name_query_defaults_0250 {
use rvn_graph::*;
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("rvn-name-default-0250-{}-{}", std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let (Ok(path), Ok(temp)) = (self.0.canonicalize(), std::env::temp_dir().canonicalize()) {
            if path.parent() == Some(temp.as_path()) && path.file_name().unwrap().to_string_lossy().starts_with("rvn-name-default-0250-") {
                fs::remove_dir_all(path).unwrap();
            }
        }
    }
}

fn roundtrip_catalog_query_defaults(explicit_capsules: bool) {
    for authored in ["asset:MixedCase0250", "None", "", "_1"] {
        let source = format!("init {{set probe=__rvn_string_table_metadata_name(__rvn_literal_name({authored:?}),\"key\",__rvn_literal_name({authored:?}))}}\nscene label start\n\"[probe]\"\nreturn\nlabel __blueprint_project_end\n");
        let ast = rvn_parser::parse(&source).unwrap();
        let mut graphs = import_script(&ast).unwrap();
        let init = graphs.iter_mut().find(|g| g.kind == GraphKind::Init).unwrap();
        let query = init.nodes.values().find(|n| n.kind == NodeKind::StringTableMetadata).unwrap().id;
        if !explicit_capsules {
            // Build an ordinary catalogue node, rather than changing an imported contract.
            init.remove_node(query).unwrap();
            let literals: Vec<_> = init.nodes.values().filter(|n| n.kind == NodeKind::Literal).map(|n| n.id).collect();
            for literal in literals { init.remove_node(literal).unwrap(); }
            let query = init.add_catalog_node(NodeKind::StringTableMetadata, [734.25, -219.5]).unwrap();
            assert_eq!(init.nodes[&query].string_table_contract, Some(StringTableQueryContract::NameV2));
            for key in ["table", "metadata"] { init.set_pin_default(query, key, PropertyValue::Name(NameDefault::new(authored).unwrap())).unwrap(); }
            init.set_pin_default(query, "key", PropertyValue::String("key".into())).unwrap();
            let set = init.nodes.values().find(|n| n.kind == NodeKind::SetVariable).unwrap().id;
            let output = init.pin_by_key(query, "result").unwrap().id;
            let input = init.pin_by_key(set, "value").unwrap().id;
            init.connect(output, input).unwrap();
        } else {
            init.nodes.get_mut(&query).unwrap().position = [734.25, -219.5];
        }
        let baseline = graphs.clone();
        let original_node_count: usize = graphs.iter().map(|g| g.nodes.len()).sum();
        let fixture = Fixture::new(); let path = fixture.0.join("story.rvn");
        fs::write(&path, &source).unwrap();
        let mut workspace = SourceWorkspace::link(&fixture.0, &path, &graphs).unwrap();
        assert_eq!(workspace.project().graphs(), baseline, "link case {authored:?}, capsules={explicit_capsules}");
        for cycle in 0..3 {
            let compiled = transpile_project(&graphs).unwrap(); assert_eq!(compiled.ast, ast);
            graphs = reimport_script(&compiled.ast, &graphs).unwrap();
            assert_eq!(graphs, baseline, "reimport cycle {cycle}, {authored:?}, capsules={explicit_capsules}");
            let cold: Vec<_> = graphs.iter().map(|g| GraphDocument::from_json(&serde_json::to_string(g).unwrap()).unwrap()).collect();
            assert_eq!(cold, baseline); workspace.save(&cold).unwrap();
            workspace = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
            assert!(workspace.source_error.is_none());
            graphs = workspace.project().graphs().to_vec();
            assert_eq!(graphs, baseline, "cold SourceWorkspace cycle {cycle}, {authored:?}, capsules={explicit_capsules}");
            assert_eq!(graphs.iter().map(|g| g.nodes.len()).sum::<usize>(), original_node_count);
            let init = graphs.iter().find(|g| g.kind == GraphKind::Init).unwrap();
            let query = init.nodes.values().find(|n| n.kind == NodeKind::StringTableMetadata).unwrap();
            for key in ["table", "metadata"] {
                let pin = init.pin_by_key(query.id, key).unwrap(); assert_eq!(pin.value_type, ValueType::Name);
                let wired = init.edges.values().any(|edge| edge.input == pin.id);
                assert_eq!(wired, explicit_capsules);
                if !wired { assert_eq!(pin.default_value, Some(PropertyValue::Name(NameDefault::new(authored).unwrap()))); }
            }
        }
    }
}

#[test]
fn catalogue_name_defaults_keep_authored_spelling_pins_ids_layout_and_node_count_through_three_cold_source_cycles() { roundtrip_catalog_query_defaults(false); }

#[test]
fn explicit_legacy_name_capsules_keep_their_wires_ids_layout_and_node_count_through_three_cold_source_cycles() { roundtrip_catalog_query_defaults(true); }

}


mod qualified_name_text_nodes_0255 {
use rvn_graph::*;
use rvn_core::{Engine,Interaction,TerminalRenderer};
use rvn_parser::{Value,Name};
use std::{fs,path::PathBuf,sync::atomic::{AtomicU64,Ordering}};
const SOURCE:&str=r#"init {
set named=__rvn_string_table_text_name(__rvn_literal_name("asset:Mixed0255"),"Key")
set exact=__rvn_string_table_text("asset:Mixed0255","Key")
set valid=__rvn_text_table_valid(named)
set id=__rvn_text_table_id(named)
set key=__rvn_text_table_key(named)
}
scene label start
"[valid] [__rvn_name_to_string(id)] [key]"
return
label __blueprint_project_end
"#;
fn inspector(graph:&GraphDocument)->&GraphNode {
    let nodes:Vec<_>=graph.nodes.values().filter(|n|n.kind==NodeKind::GetStringTableIdAndKey).collect();
    assert_eq!(nodes.len(),1,"One real shared inspector, not three fake nodes");nodes[0]
}
fn init(graphs:&[GraphDocument])->&GraphDocument {graphs.iter().find(|g|g.kind==GraphKind::Init).unwrap()}

#[test]
fn catalogue_contract_is_name_string_text_and_exactly_three_typed_value_outputs() {
    for (kind,expected) in [(NodeKind::TextFromStringTable,vec![("table",PinDirection::Input,ValueType::Name),("key",PinDirection::Input,ValueType::String),("result",PinDirection::Output,ValueType::InterpolatedText)]),
        (NodeKind::GetStringTableIdAndKey,vec![("text",PinDirection::Input,ValueType::InterpolatedText),("valid",PinDirection::Output,ValueType::Bool),("table_id",PinDirection::Output,ValueType::Name),("key",PinDirection::Output,ValueType::String)])] {
        let definition=node_definition(kind);assert!(!definition.has_dynamic_pins);
        assert_eq!(definition.pins.iter().map(|p|(p.key,p.direction,p.value_type.clone())).collect::<Vec<_>>(),expected);
        assert!(definition.pins.iter().all(|p|!p.value_type.is_execution()));
        assert_eq!(ALL_NODE_KINDS.iter().filter(|k|**k==kind).count(),1);
    }
}

#[test]
fn three_outputs_select_correct_pins_keep_shared_node_and_cold_ids_through_three_source_cycles_and_actual_engine() {
    let ast=rvn_parser::parse(SOURCE).unwrap();let mut graphs=import_script(&ast).unwrap();
    let graph=graphs.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();
    let id=inspector(graph).id;graph.nodes.get_mut(&id).unwrap().position=[591.5,-225.25];
    let baseline=graphs.clone();assert_eq!(init(&graphs).schema_version,GRAPH_NAME_TEXT_SCHEMA_VERSION);
    for _ in 0..3 {
        let graph=init(&graphs);let node=inspector(graph);
        for (key,ty) in [("valid",ValueType::Bool),("table_id",ValueType::Name),("key",ValueType::String)] {
            let pin=graph.pin_by_key(node.id,key).unwrap();assert_eq!(graph.effective_pin_type(pin.id),Some(ty));
            assert!(graph.edges.values().any(|e|e.output==pin.id));
        }
        let cold:Vec<_>=graphs.iter().map(|g|GraphDocument::from_json(&serde_json::to_string(g).unwrap()).unwrap()).collect();assert_eq!(cold,baseline);
        let compiled=transpile_project(&cold).unwrap();assert_eq!(compiled.ast,ast);
        let mut engine=Engine::new(compiled.ast.clone(),TerminalRenderer,9).unwrap();
        assert!(matches!(engine.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="true asset:Mixed0255 Key"));
        assert_eq!(engine.state.vars["valid"],Value::Bool(true));assert_eq!(engine.state.vars["id"],Value::Name(Name::new("asset:Mixed0255").unwrap()));assert_eq!(engine.state.vars["key"],Value::Str("Key".into()));
        graphs=reimport_script(&compiled.ast,&cold).unwrap();assert_eq!(graphs,baseline);
    }
}

#[test]
fn selected_name_and_string_outputs_work_inside_typed_consumers_instead_of_first_bool_output() {
    let source=SOURCE.replace("set key=__rvn_text_table_key(named)","set key=__rvn_text_table_key(named)\nset registered=__rvn_string_table_registered_name(__rvn_text_table_id(named))\nset asset=__rvn_string_table_asset(__rvn_text_table_key(named))");
    let ast=rvn_parser::parse(&source).unwrap();let graphs=import_script(&ast).unwrap();inspector(init(&graphs));assert_eq!(transpile_project(&graphs).unwrap().ast,ast);
}

#[test]
fn hot_reimport_of_unrelated_source_edit_keeps_inspector_uuid_pins_layout_and_sharing() {
    let ast=rvn_parser::parse(SOURCE).unwrap();let mut graphs=import_script(&ast).unwrap();
    let graph=graphs.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();let id=inspector(graph).id;graph.nodes.get_mut(&id).unwrap().position=[123.25,421.5];
    let old=inspector(init(&graphs)).clone();let oldpins:Vec<_>=old.pins.iter().map(|p|init(&graphs).pins[p].clone()).collect();
    let changed=rvn_parser::parse(&SOURCE.replace("set exact=", "set unrelated=42\nset exact=")).unwrap();
    let next=reimport_script(&changed,&graphs).unwrap();let node=inspector(init(&next));assert_eq!(node,&old);
    assert_eq!(node.pins.iter().map(|p|init(&next).pins[p].clone()).collect::<Vec<_>>(),oldpins);
    assert_eq!(transpile_project(&next).unwrap().ast,changed);
}

static SERIAL:AtomicU64=AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {fn new()->Self {let path=std::env::temp_dir().join(format!("rvn-name-text-0255-{}-{}",std::process::id(),SERIAL.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&path).unwrap();Self(path)}}
impl Drop for Fixture {fn drop(&mut self){if let(Ok(path),Ok(temp))=(self.0.canonicalize(),std::env::temp_dir().canonicalize()){if path.parent()==Some(temp.as_path())&&path.file_name().unwrap().to_string_lossy().starts_with("rvn-name-text-0255-"){fs::remove_dir_all(path).unwrap();}}}}

#[test]
fn shared_inspector_survives_three_saved_cold_source_workspace_reopens() {
    let fixture=Fixture::new();let path=fixture.0.join("story.rvn");fs::write(&path,SOURCE).unwrap();
    let mut workspace=SourceWorkspace::link(&fixture.0,&path,&[]).unwrap();let baseline=workspace.project().graphs().to_vec();
    for _ in 0..3 {workspace.save(&baseline).unwrap();workspace=SourceWorkspace::open(&fixture.0).unwrap().unwrap();assert!(workspace.source_error.is_none());assert_eq!(workspace.project().graphs(),baseline);inspector(init(workspace.project().graphs()));}
}

#[test]
fn strict_boundary_rejects_wrong_name_key_text_and_arity_without_conversions() {
    for expr in ["__rvn_string_table_text_name(\"id\",\"key\")","__rvn_string_table_text_name(__rvn_literal_name(\"id\"),7)","__rvn_text_table_id(__rvn_literal_name(\"id\"))","__rvn_text_table_key(7)","__rvn_text_table_valid(\"literal\")","__rvn_text_table_valid()"] {
        let ast=rvn_parser::parse(&format!("init {{set probe={expr}}}\nlabel start\n")).unwrap();assert!(import_script(&ast).is_err(),"{expr}");
    }
}

#[test]
fn new_inspector_mixed_schema_preserves_existing_query_contracts_and_rejects_downgrade() {
    let mut graph=GraphDocument::new(GraphId(1),GraphKind::Init);let legacy=graph.add_table_query_node(NodeKind::StringTableRegistered,[0.,0.],StringTableQueryContract::LegacyStringV1).unwrap();
    graph.nodes.get_mut(&legacy).unwrap().string_table_contract=None;let before=graph.pins.clone();assert_eq!(graph.schema_version,5);
    graph.add_catalog_node(NodeKind::GetStringTableIdAndKey,[0.,0.]).unwrap();assert_eq!(graph.schema_version,12);assert_eq!(graph.nodes[&legacy].string_table_contract,Some(StringTableQueryContract::LegacyStringV1));
    for (id,pin) in before {assert_eq!(graph.pins[&id],pin);}
    graph.add_catalog_node(NodeKind::StringTableRegistered,[100.,0.]).unwrap();assert_eq!(graph.schema_version,12);
    graph.set_narrative_metadata(None).unwrap();assert_eq!(graph.schema_version,12);assert!(GraphDocument::from_json(&serde_json::to_string(&graph).unwrap()).is_ok());
    for schema in 5..=11 {let mut forged=graph.clone();forged.schema_version=schema;assert!(GraphDocument::from_json(&serde_json::to_string(&forged).unwrap()).is_err());}
}

#[test]
fn none_blank_and_suffix_names_keep_authored_defaults_and_empty_keys_are_valid_dynamic_history() {
    for name in ["None","","_1","asset:Mixed0255"] {
        let ast=rvn_parser::parse(&format!("init {{set named=__rvn_string_table_text_name(__rvn_literal_name({name:?}),\"\")\nset id=__rvn_text_table_id(named)}}\nlabel start\nreturn\nlabel __blueprint_project_end\n")).unwrap();
        let graphs=import_script(&ast).unwrap();let graph=init(&graphs);let ctor=graph.nodes.values().find(|n|n.kind==NodeKind::TextFromStringTable).unwrap();
        assert_eq!(graph.pin_by_key(ctor.id,"table").unwrap().default_value,Some(PropertyValue::Name(NameDefault::new(name).unwrap())));
        let compiled=transpile_project(&graphs).unwrap();assert_eq!(compiled.ast,ast);let engine=Engine::new(compiled.ast,TerminalRenderer,9).unwrap();assert_eq!(engine.state.vars["id"],Value::Name(Name::new(name).unwrap()));
    }
}

#[test]
fn format_with_nested_table_has_false_top_level_identity_and_none_empty_outputs() {
    let ast=rvn_parser::parse(&SOURCE.replace("__rvn_string_table_text_name(__rvn_literal_name(\"asset:Mixed0255\"),\"Key\")","__rvn_format_text(__rvn_invariant_text(\"{value}\"),dict(\"value\",__rvn_string_table_text_name(__rvn_literal_name(\"asset:Mixed0255\"),\"Key\")))")).unwrap();
    let graphs=import_script(&ast).unwrap();inspector(init(&graphs));let compiled=transpile_project(&graphs).unwrap();assert_eq!(compiled.ast,ast);
    let engine=Engine::new(compiled.ast,TerminalRenderer,9).unwrap();assert_eq!(engine.state.vars["valid"],Value::Bool(false));assert_eq!(engine.state.vars["id"],Value::Name(Name::default()));assert_eq!(engine.state.vars["key"],Value::Str(String::new()));
}

#[test]
fn user_function_input_requires_explicit_capture_and_capture_shares_one_inspector() {
    let functions="function fetch(){return __rvn_string_table_text_name(__rvn_literal_name(\"asset:Mixed0255\"),\"Key\")}\n";
    let direct=format!("{functions}init {{set valid=__rvn_text_table_valid(fetch())}}\nlabel start\n");
    let error=import_script(&rvn_parser::parse(&direct).unwrap()).unwrap_err();assert!(error.contains("Capture an effectful"),"{error}");
    let captured=format!("{functions}{}",SOURCE.replace("__rvn_string_table_text_name(__rvn_literal_name(\"asset:Mixed0255\"),\"Key\")","fetch()"));
    let ast=rvn_parser::parse(&captured).unwrap();let graphs=import_script(&ast).unwrap();inspector(init(&graphs));assert_eq!(transpile_project(&graphs).unwrap().ast,ast);
    let engine=Engine::new(ast,TerminalRenderer,9).unwrap();assert_eq!(engine.state.vars["valid"],Value::Bool(true));
}

#[test]
fn empty_unconnected_text_inspector_emits_three_stateless_false_none_empty_projections() {
    let mut graphs=import_script(&rvn_parser::parse(SOURCE).unwrap()).unwrap();
    let graph=graphs.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();let node=inspector(graph).id;let input=graph.pin_by_key(node,"text").unwrap().id;
    let edges:Vec<_>=graph.edges.values().filter(|edge|edge.input==input).map(|edge|edge.id).collect();for edge in edges {graph.edges.remove(&edge);}
    let compiled=transpile_project(&graphs).unwrap();let engine=Engine::new(compiled.ast,TerminalRenderer,9).unwrap();
    assert_eq!(engine.state.vars["valid"],Value::Bool(false));assert_eq!(engine.state.vars["id"],Value::Name(Name::default()));assert_eq!(engine.state.vars["key"],Value::Str(String::new()));
}

#[test]
fn typed_nodes_can_coexist_with_owned_legacy_table_pin_without_rewriting_its_uuid_reference_or_schema() {
    let mut graph=GraphDocument::new(GraphId(1),GraphKind::Init);graph.enable_text_owner(TextOwnerId::parse("b9ab9b6d1c9448ad8dcd799c23b4b550").unwrap()).unwrap();
    let literal=graph.add_catalog_node(NodeKind::TextValue,[1.,2.]).unwrap();let pin=graph.pin_by_key(literal,"value").unwrap().id;
    let reference=rvn_parser::localized_text::StringTableReference{table:"17411a3e431e4c9388b78a2b9825b130".into(),key:"old".into()};graph.set_pin_text_table_reference(pin,reference.clone(),&[]).unwrap();assert_eq!(graph.schema_version,8);
    let original=graph.pins[&pin].clone();graph.add_catalog_node(NodeKind::TextFromStringTable,[3.,4.]).unwrap();assert_eq!(graph.schema_version,10);graph.set_narrative_metadata(None).unwrap();assert_eq!(graph.schema_version,10);assert_eq!(graph.pins[&pin],original);
    let cold=GraphDocument::from_json(&serde_json::to_string(&graph).unwrap()).unwrap();assert_eq!(cold,graph);assert_eq!(cold.pin_text_table_reference(pin).unwrap(),Some(&reference));
}

#[test]
fn function_parameter_context_shares_one_inspector_across_local_assignments_without_losing_outputs() {
    let source=format!("function inspect(text){{set valid=__rvn_text_table_valid(text)\nset id=__rvn_text_table_id(text)\nset key=__rvn_text_table_key(text)\nreturn dict(\"valid\",valid,\"id\",id,\"key\",key)}}\n{}",SOURCE.replace("set exact=","set result=inspect(named)\nset exact="));
    let ast=rvn_parser::parse(&source).unwrap();let graphs=import_script(&ast).unwrap();let function=graphs.iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="inspect")).unwrap();inspector(function);
    let compiled=transpile_project(&graphs).unwrap();assert_eq!(compiled.ast,ast);let engine=Engine::new(compiled.ast,TerminalRenderer,9).unwrap();
    assert!(matches!(&engine.state.vars["result"],Value::Dict(entries)if entries["valid"]==Value::Bool(true)&&entries["id"]==Value::Name(Name::new("asset:Mixed0255").unwrap())&&entries["key"]==Value::Str("Key".into())));
}

#[test]
fn explicitly_wired_constructor_name_capsule_survives_three_reimports_and_saved_cold_source_reopens() {
    let ast=rvn_parser::parse(SOURCE).unwrap();let mut graphs=import_script(&ast).unwrap();let graph=graphs.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();
    let ctor=graph.nodes.values().find(|n|n.kind==NodeKind::TextFromStringTable).unwrap().id;
    let capsule=graph.add_catalog_node(NodeKind::Literal,[-222.5,-333.25]).unwrap();graph.set_property(capsule,"value",PropertyValue::Name(NameDefault::new("asset:Mixed0255").unwrap())).unwrap();
    let output=graph.pin_by_key(capsule,"value").unwrap().id;let input=graph.pin_by_key(ctor,"table").unwrap().id;graph.connect(output,input).unwrap();
    let baseline=graphs.clone();let fixture=Fixture::new();let path=fixture.0.join("story.rvn");fs::write(&path,SOURCE).unwrap();let mut workspace=SourceWorkspace::link(&fixture.0,&path,&baseline).unwrap();
    for _ in 0..3 {
        let compiled=transpile_project(&graphs).unwrap();assert_eq!(compiled.ast,ast);graphs=reimport_script(&compiled.ast,&graphs).unwrap();assert_eq!(graphs,baseline);
        workspace.save(&graphs).unwrap();workspace=SourceWorkspace::open(&fixture.0).unwrap().unwrap();graphs=workspace.project().graphs().to_vec();assert_eq!(graphs,baseline);
        assert!(init(&graphs).edges.values().any(|e|e.output==output&&e.input==input));
    }
}

// Append to the existing 0255 name_text_nodes target, reusing its imports and inspector().
fn motion_curve_probe_graph(graphs: &mut [GraphDocument]) -> &mut GraphDocument {
    graphs.iter_mut().find(|graph| {
        matches!(&graph.kind, GraphKind::Function { name } if name == "probe_curve")
    }).unwrap()
}

fn edited_motion_curve_inspector_fixture(local: bool) -> (Vec<GraphDocument>, NodeId) {
    let keyword = if local { "local" } else { "set" };
    let source = format!(r#"
function curve(t) {{ return t }}
function probe_curve() {{
    {keyword} captured=__rvn_invariant_text(motion_curve("curve",2)["kind"])
    set valid=__rvn_text_table_valid(__rvn_empty_text())
    set id=__rvn_text_table_id(__rvn_empty_text())
    set key=__rvn_text_table_key(__rvn_empty_text())
    return dict("valid",valid,"id",id,"key",key)
}}
init {{ set result=probe_curve() }}
scene label start
return
label __blueprint_project_end
"#);
    let mut graphs = import_script(&rvn_parser::parse(&source).unwrap()).unwrap();
    let graph = motion_curve_probe_graph(&mut graphs);
    let shared = inspector(graph).id;
    let curves: Vec<_> = graph.nodes.values()
        .filter(|node| node.kind == NodeKind::MotionCurve)
        .map(|node| node.id).collect();
    assert_eq!(curves.len(), 1);
    let curve = curves[0];
    assert_eq!(graph.literal_input_value(curve, "function").unwrap(),
        Some(PropertyValue::String("curve".into())));
    assert_eq!(graph.literal_input_value(curve, "samples").unwrap(),
        Some(PropertyValue::Int(2)));

    let calls: Vec<_> = graph.nodes.values()
        .filter(|node| node.kind == NodeKind::FunctionCall
            && node.properties.get("function")
                == Some(&PropertyValue::String("__rvn_invariant_text".into())))
        .map(|node| node.id).collect();
    assert_eq!(calls.len(), 1);
    let raw_text = calls[0];
    let argument = graph.pin_by_key(raw_text, "item_0").unwrap().id;
    let index_output = graph.edges.values()
        .find(|edge| edge.input == argument).unwrap().output;
    assert_eq!(graph.pins[&index_output].key, "value");
    let index = graph.pins[&index_output].node;
    assert_eq!(graph.nodes[&index].kind, NodeKind::Index);
    assert_eq!(graph.literal_input_value(index, "index").unwrap(),
        Some(PropertyValue::String("kind".into())));
    let index_target = graph.pin_by_key(index, "target").unwrap().id;
    let curve_output = graph.pin_by_key(curve, "value").unwrap().id;
    assert!(graph.edges.values().any(|edge| edge.output == curve_output && edge.input == index_target));

    for (output_key, consumer_name) in [("valid", "valid"), ("table_id", "id"), ("key", "key")] {
        let output = graph.pin_by_key(shared, output_key).unwrap().id;
        assert!(graph.edges.values().any(|edge| {
            if edge.output != output { return false; }
            let input = &graph.pins[&edge.input];
            let owner = &graph.nodes[&input.node];
            input.key == "value" && owner.kind == NodeKind::SetVariable
                && owner.properties.get("name") == Some(&PropertyValue::String(consumer_name.into()))
        }));
    }

    let capture_kind = if local { NodeKind::LocalVariable } else { NodeKind::SetVariable };
    let captures: Vec<_> = graph.nodes.values()
        .filter(|node| node.kind == capture_kind
            && node.properties.get("name") == Some(&PropertyValue::String("captured".into())))
        .map(|node| node.id).collect();
    assert_eq!(captures.len(), 1);
    let capture = captures[0];
    let raw_output = graph.pin_by_key(raw_text, "result").unwrap().id;
    assert_eq!(graph.effective_pin_type(raw_output), Some(ValueType::InterpolatedText));
    let capture_input = graph.pin_by_key(capture, "value").unwrap().id;
    assert!(graph.edges.values().any(|edge| edge.output == raw_output && edge.input == capture_input));

    // Rewire a real graph producer, bypassing capture without importing forbidden source.
    let inspector_input = graph.pin_by_key(shared, "text").unwrap().id;
    graph.edges.retain(|_, edge| edge.input != inspector_input);
    graph.connect(raw_output, inspector_input).unwrap();
    (graphs, capture)
}

#[test]
fn edited_graph_inspector_rejects_hidden_motion_curve_user_function_calls() {
    for local in [false, true] {
        let (graphs, _) = edited_motion_curve_inspector_fixture(local);
        let error = transpile_project(&graphs).unwrap_err();
        assert!(error.contains("Capture"), "{error}");
    }
}

#[test]
fn motion_curve_inspector_accepts_set_and_local_capture_without_repeating_initializer() {
    for local in [false, true] {
        let (mut graphs, capture) = edited_motion_curve_inspector_fixture(local);
        let graph = motion_curve_probe_graph(&mut graphs);
        let shared = inspector(graph).id;
        let input = graph.pin_by_key(shared, "text").unwrap().id;
        let captured = graph.pin_by_key(capture, "value_out").unwrap().id;
        graph.edges.retain(|_, edge| edge.input != input);
        graph.connect(captured, input).unwrap();
        let compiled = transpile_project(&graphs).unwrap();
        assert_eq!(compiled.source.matches("motion_curve(").count(), 1);
        let engine = Engine::new(compiled.ast, TerminalRenderer, 9).unwrap();
        let Value::Dict(result) = &engine.state.vars["result"] else {
            panic!("Actual probe function must return its three inspector outputs");
        };
        assert_eq!(result["valid"], Value::Bool(false));
        assert_eq!(result["id"], Value::Name(Name::default()));
        assert_eq!(result["key"], Value::Str(String::new()));
    }
}

}


mod qualified_inspector_rollback_0263 {
use rvn_graph::*;

#[test]
fn inspector_cache_rolls_back_with_rejected_choice_guard_then_valid_guard() {
    let input="__rvn_string_table_text_name(__rvn_literal_name(\"missing\"),\"key\")";
    let source=format!("init {{}}\nscene label start\nchoice {{\n\"Name fallback\" if __rvn_text_table_id({input}) => {{return}}\n\"Boolean wire\" if __rvn_text_table_valid({input}) => {{return}}\n}}\nreturn\nlabel __blueprint_project_end\n");
    let ast=rvn_parser::parse(&source).unwrap();let graphs=import_script(&ast).unwrap();
    let graph=graphs.iter().find(|g|matches!(g.kind,GraphKind::Label{ref name} if name=="start")).unwrap();
    let choice=graph.nodes.values().find(|n|n.kind==NodeKind::Choice).unwrap().id;
    assert!(matches!(graph.nodes[&choice].properties.get("option_0_condition"),Some(PropertyValue::String(value)) if value.contains("__rvn_text_table_id")));
    let condition=graph.pin_by_key(choice,"option_1_condition").unwrap().id;
    let output=graph.edges.values().find(|edge|edge.input==condition).unwrap().output;
    assert_eq!(graph.pins[&output].key,"valid");assert_eq!(graph.nodes[&graph.pins[&output].node].kind,NodeKind::GetStringTableIdAndKey);
    assert_eq!(transpile_project(&graphs).unwrap().ast,ast);
}

#[test]
fn inspector_cache_rolls_back_during_legacy_choice_guard_materialization() {
    let ast=rvn_parser::parse("scene label start\nchoice {\"A\"=>{return} \"B\"=>{return}}\nreturn\nlabel __blueprint_project_end\n").unwrap();
    let mut graphs=import_script(&ast).unwrap();let graph=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Label{ref name} if name=="start")).unwrap();
    let choice=graph.nodes.values().find(|n|n.kind==NodeKind::Choice).unwrap().id;
    let input="__rvn_string_table_text_name(__rvn_literal_name(\"missing\"),\"key\")";
    graph.set_property(choice,"option_0_condition",PropertyValue::String(format!("__rvn_text_table_id({input})"))).unwrap();
    graph.set_property(choice,"option_1_condition",PropertyValue::String(format!("__rvn_text_table_valid({input})"))).unwrap();
    assert_eq!(graph.materialize_choice_conditions().unwrap(),1);
    let condition=graph.pin_by_key(choice,"option_1_condition").unwrap().id;
    let output=graph.edges.values().find(|edge|edge.input==condition).unwrap().output;
    assert_eq!(graph.pins[&output].key,"valid");assert_eq!(graph.nodes[&graph.pins[&output].node].kind,NodeKind::GetStringTableIdAndKey);
    assert!(matches!(graph.nodes[&choice].properties.get("option_0_condition"),Some(PropertyValue::String(value)) if !value.is_empty()));
    let cold=GraphDocument::from_json(&serde_json::to_string(&graph).unwrap()).unwrap();assert_eq!(cold,*graph);
}

}
