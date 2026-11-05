use rvn_graph::*;
use rvn_parser::localized_text::{DeferredText, LocalizedText, TextIdentity, TextLocalization};
use std::collections::HashMap;
const OWNER: &str = "b9ab9b6d1c9448ad8dcd799c23b4b550";
fn graph() -> GraphDocument {
    let mut graph = GraphDocument::new(GraphId(1), GraphKind::Init);
    graph.enable_text_owner(TextOwnerId::parse(OWNER).unwrap()).unwrap();
    graph
}
fn add(g: &mut GraphDocument, kind: NodeKind) -> NodeId { g.add_catalog_node(kind, [0., 0.]).unwrap() }
fn pin(g: &GraphDocument, n: NodeId, key: &str) -> PinId { g.pin_by_key(n,key).unwrap().id }
fn text(source: &str, key: &str) -> LocalizedText {
    LocalizedText {source: source.into(), localization: TextLocalization::Localizable {
        identity: TextIdentity {owner: format!("asset:{OWNER}"), namespace: "VN".into(), key:key.into()},
        developer_notes: "Kept notes".into(),
    }}
}
fn eval(source: &str) -> rvn_parser::Value {
    let parsed=rvn_parser::parse(&format!("set value = {source}")).unwrap();
    let [rvn_parser::Statement::SetVar {value,..}]=parsed.as_slice() else {panic!()};
    rvn_core::eval::eval_expr(value,&HashMap::new()).unwrap()
}

#[test]
fn shared_source_edit_forks_once_unique_source_and_snapshots_keep_identity() {
    let mut g=graph();
    let original=add(&mut g,NodeKind::TextValue);let p=pin(&g,original,"value");
    g.set_property(original,"value",PropertyValue::String("Salut [name]".into())).unwrap();
    assert_eq!(transpile_value_output(&g,original,"value").unwrap(),"\"Salut [name]\"");
    assert!(g.adopt_pin_text(p,||"first".into()).unwrap());
    g.edit_pin_text(p,GraphTextEdit::Notes("Kept notes".into()),||panic!()).unwrap();
    let duplicated=add(&mut g,NodeKind::TextValue);let q=pin(&g,duplicated,"value");
    g.set_pin_text(q,g.pin_text(p).unwrap().unwrap()).unwrap();
    // Wire fan-out and inferred variables are derived, not more authored Texts.
    let cast=add(&mut g,NodeKind::ConvertTextToString);
    g.connect(p,pin(&g,cast,"value")).unwrap();
    g.add_variable("cache",ValueType::InterpolatedText,PropertyValue::String("Salut [name]".into())).unwrap();
    assert_eq!(g.authored_text_literals().unwrap().len(),2);
    let before=g.clone();
    g.edit_pin_text(q,GraphTextEdit::Source("Autre".into()),||"forked".into()).unwrap();
    assert_eq!(g.pin_text(p).unwrap(),before.pin_text(p).unwrap());
    let changed=g.pin_text(q).unwrap().unwrap();
    assert_eq!(changed.localization.identity().unwrap().key,"forked");
    assert!(matches!(&changed.localization,TextLocalization::Localizable {developer_notes,..} if developer_notes=="Kept notes"));
    let after=g.clone();
    g=before.clone();assert_eq!(g,before);g=after.clone();assert_eq!(g,after);
    g.edit_pin_text(q,GraphTextEdit::Source("Encore".into()),||panic!("Unique source keeps key")).unwrap();
    let exact=g.clone();
    assert!(g.edit_pin_text(q,GraphTextEdit::Namespace("[bad]".into()),||panic!()).is_err());
    assert_eq!(g,exact);
    assert!(g.set_property(duplicated,"value",PropertyValue::String("Bypass".into())).is_err());
    assert_eq!(g,exact);
    g.edit_pin_text(q,GraphTextEdit::Localizable(false),||panic!()).unwrap();
    assert_eq!(g.pin_text(q).unwrap().unwrap().localization,TextLocalization::CultureInvariant);
    g.edit_pin_text(q,GraphTextEdit::Localizable(true),||"enabled".into()).unwrap();
    assert_eq!(g.pin_text(q).unwrap().unwrap().localization.identity().unwrap().key,"enabled");
    g.edit_pin_text(q,GraphTextEdit::Source(String::new()),||panic!()).unwrap();
    assert_eq!(g.pin_text(q).unwrap().unwrap().localization,TextLocalization::Empty);
    g.edit_pin_text(q,GraphTextEdit::Source("Restored".into()),||"restored".into()).unwrap();
    assert_eq!(g.pin_text(q).unwrap().unwrap().localization.identity().unwrap().key,"restored");
    assert_eq!(GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap(),g);
    let rvn_parser::Value::Text(value)=eval(&transpile_value_output(&g,original,"value").unwrap()) else {panic!()};
    assert_eq!(value.source_display().unwrap(),"Salut [name]");
}

#[test]
fn explicit_constructor_references_participate_without_counting_nested_fanout_twice() {
    let mut g=graph();let constructor=add(&mut g,NodeKind::FunctionCall);
    g.set_property(constructor,"function",PropertyValue::String("__rvn_localizable_text".into())).unwrap();
    g.resize_value_inputs(constructor,5).unwrap();
    for (i,value) in ["Bonjour",&format!("asset:{OWNER}"),"VN","shared","Explicit notes"].iter().enumerate() {
        g.set_pin_default(constructor,&format!("item_{i}"),PropertyValue::String((*value).into())).unwrap();
    }
    let n=add(&mut g,NodeKind::TextValue);let p=pin(&g,n,"value");
    g.set_pin_text(p,text("Bonjour","shared")).unwrap();
    assert_eq!(g.authored_text_literals().unwrap().len(),2);
    g.edit_pin_text(p,GraphTextEdit::Source("Autre".into()),||"new".into()).unwrap();
    assert_eq!(g.pin_text(p).unwrap().unwrap().localization.identity().unwrap().key,"new");
    let exact=g.clone();let mut foreign=text("Foreign","key");
    if let TextLocalization::Localizable {identity,..}=&mut foreign.localization {identity.owner="asset:other".into()};
    assert!(g.set_pin_text(p,foreign).is_err());assert_eq!(g,exact);
    let mut invalid=serde_json::to_value(&g).unwrap();invalid["schema_version"]=5.into();
    assert!(GraphDocument::from_json(&invalid.to_string()).is_err());
}

#[test]
fn deferred_format_captures_text_arguments_translates_after_save_and_clears_connected_pattern() {
    let mut g=graph();let format=add(&mut g,NodeKind::FormatText);let f=pin(&g,format,"format");
    g.set_pin_text(f,text("Hello {Name}! Count {Count}, {Flag}","pattern")).unwrap();
    let person=add(&mut g,NodeKind::TextValue);let p=pin(&g,person,"value");
    g.set_pin_text(p,text("Visitor","person")).unwrap();
    g.connect(p,pin(&g,format,"argument_Name")).unwrap();
    g.set_pin_default(format,"argument_Count",PropertyValue::Int(3)).unwrap();
    g.set_pin_default(format,"argument_Flag",PropertyValue::Bool(true)).unwrap();
    let source=transpile_value_output(&g,format,"result").unwrap();
    let rvn_parser::Value::Text(value)=eval(&source) else {panic!("Format flattened")};
    assert!(matches!(value.as_ref(),DeferredText::Format {..}));
    assert_eq!(value.source_display().unwrap(),"Hello Visitor! Count 3, true");
    let mut engine=rvn_core::Engine::new(rvn_parser::parse(&format!("init {{set message={source}}}\nlabel start\n\"[message]\"\n")).unwrap(),rvn_core::TerminalRenderer,7).unwrap();
    assert!(matches!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue {..})));
    let saved=rvn_core::save::SaveData::try_from_state(&engine.state,1,"Text".into(),"main.rvn".into()).unwrap();
    assert_eq!(saved.format_version,12);
    let cold:rvn_core::save::SaveData=serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    let rvn_core::save::SaveValue::Text(stored)=&cold.vars["message"] else {panic!()};
    let pattern=text("ignored","pattern").localization.identity().unwrap().locale_key();
    let person_key=text("ignored","person").localization.identity().unwrap().locale_key();
    let locale=rvn_core::locale::LocaleManager::from_tables("unused","en","fr",vec!["en".into(),"fr".into()],HashMap::from([
        ("fr".into(),rvn_core::locale::LocaleTable {lang:"fr".into(),strings:HashMap::from([
            (pattern,"Bonjour {Name}! Nombre {Count}, {Flag}".into()),(person_key,"Éloïse".into())])})]));
    assert_eq!(rvn_core::localized_text::render_text(stored,Some(&locale),&rvn_core::localized_text::VnTextScalars).unwrap(),"Bonjour Éloïse! Nombre 3, true");
    let before=g.clone();let edge=g.connect(p,f).unwrap();
    assert!(g.nodes[&format].pins.iter().all(|p|!g.pins[p].key.starts_with("argument_")));
    assert!(g.pin_text(f).unwrap().is_none());
    assert!(g.set_format_text_pattern(format,"Bypass").is_err());
    g.remove_edge(edge);assert_eq!(g.pin_text(f).unwrap().unwrap().localization,TextLocalization::Empty);
    assert_eq!(GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap(),g);
    g=before.clone();assert_eq!(g,before);
}

#[test]
fn source_transaction_recovery_and_structural_reimport_preserve_text_pin_identity() {
    let root=std::env::temp_dir().join(format!("rvn-authored-text-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&root).unwrap();struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    let _fixture=Fixture(root.clone());let path=root.join("main.rvn");
    let source="// preserve authored trivia\nscene label start\n\"Salut\"\nreturn\n";
    std::fs::write(&path,source).unwrap();let mut workspace=SourceWorkspace::link(&root,&path,&[]).unwrap();
    let mut graphs=workspace.project().graphs().to_vec();
    let g=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Label {..})).unwrap();
    g.enable_text_owner(TextOwnerId::parse(OWNER).unwrap()).unwrap();
    let n=g.nodes.values().find(|n|n.kind==NodeKind::TextValue).unwrap().id;let p=pin(g,n,"value");
    g.set_pin_text(p,text("Salut [name]","greeting")).unwrap();
    let format=add(g,NodeKind::FormatText);let pattern=pin(g,format,"format");
    g.set_pin_text(pattern,text("Result: {Name}","pattern")).unwrap();
    let outgoing=g.edges.values().find(|e|e.output==p).unwrap().clone();
    g.remove_edge(outgoing.id);
    g.connect(p,pin(g,format,"argument_Name")).unwrap();
    g.connect(pin(g,format,"result"),outgoing.input).unwrap();
    workspace.write_recovery(&graphs).unwrap();assert_eq!(workspace.recovery().unwrap().unwrap(),graphs);
    workspace.save(&graphs).unwrap();assert!(workspace.recovery().unwrap().is_none());
    let saved=std::fs::read_to_string(&path).unwrap();assert!(saved.starts_with("// preserve authored trivia"));
    assert!(saved.contains("__rvn_localizable_text"));let published=workspace.project().graphs().to_vec();drop(workspace);
    let cold=SourceWorkspace::open(&root).unwrap().unwrap();assert_eq!(cold.project().graphs(),published);drop(cold);
    // Change graph structure externally, then edit the literal source explicitly.
    let changed=saved.replace("return","set count = 1\nreturn").replace("Salut [name]","Autre [name]");
    std::fs::write(&path,&changed).unwrap();let refreshed=SourceWorkspace::open(&root).unwrap().unwrap();
    let g=refreshed.project().graphs().iter().find(|g|matches!(g.kind,GraphKind::Label {..})).unwrap();
    let n=g.nodes.values().find(|n|n.kind==NodeKind::TextValue).unwrap().id;
    assert_eq!(pin(g,n,"value"),p);
    let value=g.pin_text(p).unwrap().unwrap();assert_eq!(value,text("Autre [name]","greeting"));
    assert_eq!(g.nodes[&format].kind,NodeKind::FormatText);
    assert_eq!(g.pin_text(pattern).unwrap().unwrap(),text("Result: {Name}","pattern"));
}

#[test]
fn owned_reimport_preserves_optional_notes_constructor_arity_and_dynamic_payloads() {
    for suffix in ["", ", \"\"", ", \"Notes\""] {
        let source=format!("init {{set message=__rvn_localizable_text(\"Hello\",\"asset:{OWNER}\",\"VN\",\"key\"{suffix})}}\nlabel start\nreturn\n");
        let mut graphs=import_script(&rvn_parser::parse(&source).unwrap()).unwrap();
        graphs[0].enable_text_owner(TextOwnerId::parse(OWNER).unwrap()).unwrap();
        let changed=source.replace("}\nlabel", " set count=1}\nlabel");
        let parsed=rvn_parser::parse(&changed).unwrap();
        let restored=reimport_script(&parsed,&graphs).unwrap();
        assert_eq!(&transpile_project(&restored).unwrap().ast[..parsed.len()],parsed.as_slice());
        assert!(restored[0].nodes.values().any(|n|n.kind==NodeKind::TextValue));
    }
    let source=format!("init {{set content=\"Hello\" set message=__rvn_localizable_text(content,\"asset:{OWNER}\",\"VN\",\"key\")}}\nlabel start\nreturn\n");
    let mut graphs=import_script(&rvn_parser::parse(&source).unwrap()).unwrap();
    graphs[0].enable_text_owner(TextOwnerId::parse(OWNER).unwrap()).unwrap();
    let changed=rvn_parser::parse(&source.replace("}\nlabel"," set count=1}\nlabel")).unwrap();
    let restored=reimport_script(&changed,&graphs).unwrap();
    assert_eq!(&transpile_project(&restored).unwrap().ast[..changed.len()],changed.as_slice());
    assert!(restored[0].authored_text_literals().unwrap().is_empty());
}
