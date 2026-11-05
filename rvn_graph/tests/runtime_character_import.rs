use rvn_graph::*;
use rvn_parser::Statement;

#[test]
fn runtime_character_creation_keeps_its_conditional_visit_instead_of_becoming_initialization(){
    for spawn in [false,true]{
        let source=format!("init{{set spawn={spawn}\nset score=0}}\nscene label start\nif spawn{{character.create(\"iris\",\"Iris 🍃\")\nset score=2}}\n\"Fin\"\nreturn\n");
        let parsed=rvn_parser::parse_program(&source).unwrap();
        let graphs=import_script(&parsed.script).unwrap();
        assert!(graphs.iter().all(|graph|graph.characters.is_empty()),"a runtime creation must not be hoisted to the init registry");
        let graph=graphs.iter().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
        assert_eq!(graph.nodes.values().filter(|node|node.kind==NodeKind::CharacterCreate).count(),1);
        let compiled=transpile_project(&graphs).unwrap();
        assert!(!compiled.ast.iter().any(|statement|matches!(statement,Statement::Init{body}if body.iter().any(|statement|matches!(statement,Statement::CharacterCreate{..})))));
        assert!(compiled.ast.iter().any(|statement|matches!(statement,Statement::If{then_branch,..}if then_branch.iter().any(|statement|matches!(statement,Statement::CharacterCreate{id,display_name}if id=="iris"&&display_name=="Iris 🍃")))));
        for program in [parsed,rvn_parser::parse_program(&compiled.source).unwrap()]{
            let mut engine=rvn_core::Engine::from_program(program,rvn_core::TerminalRenderer,16).unwrap();
            engine.enter_label("start").unwrap();
            assert!(matches!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text=="Fin"));
            assert_eq!(engine.state.vars["score"],rvn_parser::Value::Int(if spawn{2}else{0}));
        }
    }
}

#[test]
fn source_character_node_edit_keeps_scope_comments_and_cold_graph_identity(){
    let source="// scène originale\ninit{set score=0}\nscene label start\n// création locale\ncharacter.create(\"iris\",\"Iris\")\n\"Fin\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();
    let mut edited=project.graphs().to_vec();
    let graph=edited.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let graph_id=graph.graph_id;
    let node=graph.nodes.values().find(|node|node.kind==NodeKind::CharacterCreate).unwrap().id;
    // Import exposes this literal as a real Literal producer. Editing an
    // inactive input default must not bypass its connected value.
    let input=graph.pin_by_key(node,"display_name").unwrap().id;
    let output=graph.edges.values().find(|edge|edge.input==input).unwrap().output;
    let text=graph.pins[&output].node;
    assert_eq!(graph.nodes[&text].kind,NodeKind::Literal);
    graph.set_property(text,"value",PropertyValue::String("Iris Montréal 🍃".into())).unwrap();
    project.apply_visual(source,&edited).unwrap();
    assert!(project.source().contains("// scène originale"));
    assert!(project.source().contains("// création locale"));
    assert!(project.graphs().iter().all(|graph|graph.characters.is_empty()));
    let cold=SourceProject::open(project.source(),project.graphs()).unwrap();
    let graph=cold.graphs().iter().find(|graph|graph.graph_id==graph_id).unwrap();
    assert_eq!(graph.nodes[&node].kind,NodeKind::CharacterCreate);
    let ast=rvn_parser::parse_program(cold.source()).unwrap().script;
    assert!(ast.iter().any(|statement|matches!(statement,Statement::CharacterCreate{id,display_name}if id=="iris"&&display_name=="Iris Montréal 🍃")));
    assert!(!ast.iter().any(|statement|matches!(statement,Statement::Init{body}if body.iter().any(|statement|matches!(statement,Statement::CharacterCreate{..})))));
}
