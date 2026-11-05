use rvn_graph::*;

#[test]
fn global_initial_projection_uses_declarations_without_evaluating_or_mutating() {
    let source="init {set score=7\nset value=true\nset before=value\nset repeated=2\nset repeated=5\nif true {set conditional=3}\nwhile false {set looped=4}}\nscene label start\nset score=9\nset later=11\nlocal shadow=2\nreturn\n";
    let project=SourceProject::open(source,&[]).unwrap();
    let before=project.graphs().to_vec();
    let values=project.global_declaration_values(&before).unwrap();
    assert_eq!(values["score"],GlobalDeclarationValue::Literal(PropertyValue::Int(7)));
    assert_eq!(values["value"],GlobalDeclarationValue::Literal(PropertyValue::Bool(true)));
    for name in ["before","repeated","conditional","looped"] {
        assert_eq!(values[name],GlobalDeclarationValue::Runtime);
    }
    assert!(!values.contains_key("later"));assert!(!values.contains_key("shadow"));
    assert_eq!(project.source(),source);assert_eq!(project.graphs(),before);
    let legacy=global_declaration_values_for_graphs(&before).unwrap();assert_eq!(legacy,values);
    let script=transpile_project(&before).unwrap().ast;let original=script.clone();
    assert_eq!(global_declaration_values_for_script(&script).unwrap(),values);assert_eq!(script,original);
    let invalid=rvn_parser::parse_program("scene label start\nreturn\nscene label start\nreturn\n").unwrap().script;
    assert!(global_declaration_values_for_script(&invalid).is_err());
}

#[test]
fn global_initial_projection_follows_current_init_and_refuses_invalid_links() {
    let source="init {set value=7}\nscene label start\nset later=value\nreturn\n";
    let project=SourceProject::open(source,&[]).unwrap();let baseline=project.graphs().to_vec();
    let mut edited=baseline.clone();let init=edited.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();
    let set=init.nodes.values().find(|n|n.kind==NodeKind::SetVariable).unwrap().id;
    init.set_pin_default(set,"value",PropertyValue::Int(-12)).unwrap();
    let edited_before=edited.clone();
    assert_eq!(project.global_declaration_values(&edited).unwrap()["value"],GlobalDeclarationValue::Literal(PropertyValue::Int(-12)));
    assert_eq!(project.graphs(),baseline);assert_eq!(edited,edited_before);assert_eq!(project.source(),source);
    assert_eq!(project.global_declaration_values(&baseline).unwrap()["value"],GlobalDeclarationValue::Literal(PropertyValue::Int(7)));
    let init=edited.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();
    let input=init.pin_by_key(set,"value").unwrap().id;
    let edge=init.edges.values_mut().next().unwrap();edge.input=input;
    assert!(project.global_declaration_values(&edited).is_err());assert_eq!(project.graphs(),baseline);
}

#[test]
fn global_initial_projection_keeps_signed_unicode_literals_and_all_top_level_inits() {
    let source="init {set signed=-12\nset text=\"Été 🍃\"}\ninit {set signed=4\nset float=-1.25}\nscene label start\nreturn\n";
    let project=SourceProject::open(source,&[]).unwrap();let before=project.graphs().to_vec();
    let values=project.global_declaration_values(&before).unwrap();
    assert_eq!(values["signed"],GlobalDeclarationValue::Runtime);
    assert_eq!(values["text"],GlobalDeclarationValue::Literal(PropertyValue::String("Été 🍃".into())));
    assert_eq!(values["float"],GlobalDeclarationValue::Literal(PropertyValue::Float(-1.25)));
    assert_eq!(project.graphs(),before);assert_eq!(project.source(),source);
}

#[test]
fn global_initial_projection_resolves_read_only_imports_and_refuses_changed_source() {
    let root=std::env::temp_dir().join(format!("rvn-global-initial-import-{}-{}",std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    let _cleanup=Cleanup(root.clone());
    let imported="init {set imported=4}\n";let source="use \"chapter.rvn\"\nscene label start\nset later=imported\nreturn\n";
    std::fs::write(root.join("chapter.rvn"),imported).unwrap();std::fs::write(root.join("main.rvn"),source).unwrap();
    let project=SourceProject::open_at(&root.join("main.rvn"),source,&[]).unwrap();let graphs=project.graphs().to_vec();
    assert!(project.is_imported_global("imported"));
    assert_eq!(project.global_declaration_values(&graphs).unwrap()["imported"],GlobalDeclarationValue::Literal(PropertyValue::Int(4)));
    assert_eq!(project.graphs(),graphs);assert_eq!(std::fs::read_to_string(root.join("chapter.rvn")).unwrap(),imported);
    std::fs::write(root.join("chapter.rvn"),"init {set imported=8}\n").unwrap();
    assert!(project.global_declaration_values(&graphs).is_err());assert_eq!(project.graphs(),graphs);
}

fn graphs(source: &str) -> Vec<GraphDocument> {
    import_script(&rvn_parser::parse_program(source).unwrap().script).unwrap()
}
fn start(graphs: &[GraphDocument]) -> &GraphDocument {
    graphs.iter().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name=="start")).unwrap()
}

#[test]
fn declaration_value_is_not_the_last_assignment_and_query_preserves_entire_graph() {
    let source="scene label start\nlocal duration = 2\n\"Before\"\nset duration = 7\n\"After\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();
    let graph=start(project.graphs());let before=graph.clone();
    assert_eq!(graph.variables["duration"].default_value,PropertyValue::Int(7),"Legacy write inference remains intact");
    let declarations=graph.local_variable_initializers("duration").unwrap();
    assert_eq!(declarations.len(),1);
    assert_eq!(declarations[0].literal_value,Some(PropertyValue::Int(2)));
    assert_eq!(graph.nodes[&declarations[0].node].kind,NodeKind::LocalVariable);
    assert_eq!(*graph,before);
    let roundtrip=GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
    assert_eq!(roundtrip.local_variable_initializers("duration").unwrap(),declarations);
    let unchanged=project.graphs().to_vec();
    project.apply_visual(source,&unchanged).unwrap();
    assert_eq!(project.source(),source);
    assert_eq!(start(project.graphs()),&before);
}

#[test]
fn connected_calculation_and_global_before_local_keep_expression_and_wire_identity() {
    let source="init {set score = 10}\nscene label start\nlocal score = score + 1\nset score = 40\nreturn\n";
    let graphs=graphs(source);let graph=start(&graphs);let before=graph.clone();
    let declarations=graph.local_variable_initializers("score").unwrap();
    assert_eq!(declarations.len(),1);assert_eq!(declarations[0].literal_value,None);
    assert!(matches!(&declarations[0].expression,rvn_parser::Expr::BinOp {..}));
    let input=graph.pin_by_key(declarations[0].node,"value").unwrap().id;
    assert!(graph.edges.values().any(|edge|edge.input==input));
    assert_eq!(*graph,before);
    assert!(graphs.iter().find(|g|g.kind==GraphKind::Init).unwrap().local_variable_initializers("score").unwrap().is_empty());
}

#[test]
fn repeated_branch_declarations_remain_distinct_and_disconnected_local_is_excluded() {
    let mut graph=start(&graphs("scene label start\nif true {local visits = 2} else {local visits = 3}\nlocal visits = 4\nreturn\n")).clone();
    let disconnected=graph.add_catalog_node(NodeKind::LocalVariable,[800.0,700.0]).unwrap();
    graph.set_property(disconnected,"name",PropertyValue::String("visits".into())).unwrap();
    graph.set_pin_default(disconnected,"value",PropertyValue::Int(99)).unwrap();
    let before=graph.clone();let declarations=graph.local_variable_initializers("visits").unwrap();
    assert_eq!(declarations.iter().map(|d|d.literal_value.clone()).collect::<Vec<_>>(),
        vec![Some(PropertyValue::Int(2)),Some(PropertyValue::Int(3)),Some(PropertyValue::Int(4))]);
    assert_eq!(declarations.iter().map(|d|d.node).collect::<std::collections::BTreeSet<_>>().len(),3);
    assert!(!declarations.iter().any(|d|d.node==disconnected));assert_eq!(graph,before);
}

#[test]
fn primitive_defaults_keep_unicode_escape_bool_and_signed_numeric_values() {
    for(expression,expected)in[("true",PropertyValue::Bool(true)),("-12",PropertyValue::Int(-12)),
        ("-1.25",PropertyValue::Float(-1.25)),("\"Été \\\"Iris\\\" \\n 🍃\"",PropertyValue::String("Été \"Iris\" \n 🍃".into()))] {
        let graphs=graphs(&format!("scene label start\nlocal value = {expression}\nset value = 0\nreturn\n"));
        let graph=start(&graphs);let before=graph.clone();let declarations=graph.local_variable_initializers("value").unwrap();
        assert_eq!(declarations.len(),1);assert_eq!(declarations[0].literal_value,Some(expected));assert_eq!(*graph,before);
    }
}

#[test]
fn parameters_implicit_assignments_and_inherited_fragment_reads_are_not_literal_declarations() {
    let graphs=graphs("function read(value){set value = 7 return value}\nfunction implicit(){set count = 2 set count = 7 return count}\ninit {set shared=8}\nscene label start\nlocal shared = 2\ncall helper\nreturn\nlabel helper\nset shared = 7\nreturn\n");
    for (kind,name)in[(GraphKind::Function{name:"read".into()},"value"),
        (GraphKind::Function{name:"implicit".into()},"count"),(GraphKind::Label{name:"helper".into()},"shared")] {
        let graph=graphs.iter().find(|graph|graph.kind==kind).unwrap();let before=graph.clone();
        assert!(graph.local_variable_initializers(name).unwrap().is_empty());assert_eq!(*graph,before);
    }
    assert_eq!(start(&graphs).local_variable_initializers("shared").unwrap()[0].literal_value,Some(PropertyValue::Int(2)));
}
