use rvn_graph::*;
use rvn_parser::parse;

#[test]
fn interface_ownership_roundtrips_as_an_explicit_boolean_without_changing_legacy_open() {
    let source=r#"screen inventory(value){return component("root","panel",{},[])} handler open_menu(event){ui.open("inventory",[0],false,2) ui.open_story("inventory",[event["value"]],true,7)} label start return"#;
    let mut graphs=import_script(&parse(source).unwrap()).unwrap();
    let handler=graphs.iter().find(|graph|matches!(graph.kind,GraphKind::Handler{..})).unwrap();
    let nodes=handler.nodes.values().filter(|node|node.kind==NodeKind::UiOpen).collect::<Vec<_>>();
    assert_eq!(nodes.len(),2);assert!(nodes.iter().any(|node|node.properties.get("story")==Some(&PropertyValue::Bool(false))));assert!(nodes.iter().any(|node|node.properties.get("story")==Some(&PropertyValue::Bool(true))));
    let identities=nodes.iter().map(|node|node.id).collect::<Vec<_>>();
    for _ in 0..3 {
        let generated=transpile_project(&graphs).unwrap();assert!(generated.source.contains("ui.open("));assert!(generated.source.contains("ui.open_story("));
        let body=generated.ast.iter().find_map(|statement|if let rvn_parser::Statement::Handler{body,..}=statement{Some(body)}else{None}).unwrap();
        assert!(body.iter().any(|statement|matches!(statement,rvn_parser::Statement::UiOpen{arguments,modal,layer,story:false,..}
            if arguments==&rvn_parser::Expr::ListLit(vec![rvn_parser::Expr::Int(0)])&&modal==&rvn_parser::Expr::Bool(false)&&layer==&rvn_parser::Expr::Int(2))));
        assert!(body.iter().any(|statement|matches!(statement,rvn_parser::Statement::UiOpen{arguments,modal,layer,story:true,..}
            if arguments==&rvn_parser::Expr::ListLit(vec![rvn_parser::Expr::Index{target:Box::new(rvn_parser::Expr::Var("event".into())),index:Box::new(rvn_parser::Expr::Str("value".into()))}])&&modal==&rvn_parser::Expr::Bool(true)&&layer==&rvn_parser::Expr::Int(7))));
        graphs=reimport_script(&parse(&generated.source).unwrap(),&graphs).unwrap();
        let handler=graphs.iter().find(|graph|matches!(graph.kind,GraphKind::Handler{..})).unwrap();
        assert!(identities.iter().all(|id|handler.nodes.contains_key(id)));assert_eq!(handler.nodes.values().filter(|node|node.kind==NodeKind::UiOpen&&node.properties.get("story")==Some(&PropertyValue::Bool(true))).count(),1);
    }
    let handler=graphs.iter_mut().find(|graph|matches!(graph.kind,GraphKind::Handler{..})).unwrap();let open=handler.nodes.values_mut().find(|node|node.kind==NodeKind::UiOpen&&node.properties.get("story")==Some(&PropertyValue::Bool(false))).unwrap();
    open.properties.remove("story");assert!(transpile_project(&graphs).unwrap().source.contains("ui.open("));
    let handler=graphs.iter_mut().find(|graph|matches!(graph.kind,GraphKind::Handler{..})).unwrap();let open=handler.nodes.values_mut().find(|node|node.kind==NodeKind::UiOpen).unwrap();open.properties.insert("story".into(),PropertyValue::Int(1));assert!(transpile_project(&graphs).is_err());
    let mut new=GraphDocument::new(GraphId::new(1),GraphKind::Label{name:"start".into()});let node=new.add_catalog_node(NodeKind::UiOpen,[0.0,0.0]).unwrap();assert_eq!(new.nodes[&node].properties.get("story"),Some(&PropertyValue::Bool(false)));
}

#[test]
fn interfaces_and_handlers_roundtrip_in_both_modes_without_losing_source_comments_or_layout() {
    let source = r#"// Shared interface component.
function heading(text) { return {"id":"heading", "kind":"text", "text":text} }
screen inventory(title) {
    // Author's inventory note.
    local children = [heading(title)]
    return {"id":"root", "kind":"column", "children":children}
}
handler selected(event) {
    local value = event["value"]
    set chosen = value
    ui.focus("inventory", "name")
    ui.close("inventory")
}
init { set chosen = "" }
label start
    ui.open("inventory", ["Inventory"], false, 1)
    "Continue"
    return
"#;
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    assert!(graphs
        .iter()
        .any(|graph| matches!(graph.kind, GraphKind::Screen { .. })));
    assert!(graphs
        .iter()
        .any(|graph| matches!(graph.kind, GraphKind::Handler { .. })));
    for _ in 0..3 {
        let generated = transpile_project(&graphs).unwrap();
        assert_eq!(
            generated
                .ast
                .iter()
                .filter(|s| matches!(s, rvn_parser::Statement::Screen { .. }))
                .count(),
            1
        );
        let imported = reimport_script(&parse(&generated.source).unwrap(), &graphs).unwrap();
        for graph in &graphs {
            assert!(imported
                .iter()
                .any(|other| other.kind == graph.kind && other.graph_id == graph.graph_id));
        }
        graphs = imported;
    }
    let mut graphs = project.graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Screen { .. }))
        .unwrap();
    let node = graph.nodes.values_mut().next().unwrap();
    node.position = [321.0, 123.0];
    project.apply_visual(source, &graphs).unwrap();
    assert_eq!(project.source(), source);
    let value = graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Screen { .. }))
        .unwrap()
        .pins
        .values_mut()
        .find(|pin| pin.default_value == Some(PropertyValue::String("root".into())))
        .unwrap();
    // Descriptions remain ordinary dictionaries and reusable function calls,
    // rather than an editor-only format hidden from RVN authors.
    value.default_value = Some(PropertyValue::String("main_root".into()));
    project.apply_visual(source, &graphs).unwrap();
    assert!(project.source().contains("// Author's inventory note."));
    assert!(project.source().contains("main_root"));
}
