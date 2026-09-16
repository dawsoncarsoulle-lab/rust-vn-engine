use rvn_graph::*;
use rvn_parser::Statement;

#[test]
fn import_has_typed_variables_editable_text_branches_and_call_continuation() {
    let source = rvn_parser::parse(r#"
init { character.create("narrator", "") set trust = 5 }
label start
    call intro
    jump morning
label intro
    narrator "Été : [trust + 1]."
    choice {
        "Oui" => { set trust = trust + 1 }
        "Non" if trust >= 5 => { set trust = trust - 1 }
    }
    return
label morning
    typewriter = true
    typewriter.speed(38)
    music.stop() with fade(1500)
    narrator "Fin."
"#).unwrap();
    let graphs = import_script(&source).unwrap();
    assert_eq!(graphs.len(),4);
    for graph in &graphs {
        assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),*graph);
        assert_eq!(graph.variables["trust"].value_type,ValueType::Int);
        assert_eq!(graph.characters["narrator"],"");
        assert!(graph.validate().iter().all(|d|d.severity!=DiagnosticSeverity::Error));
    }
    let result=transpile_project(&graphs).unwrap();
    assert!(result.source.contains("call intro\n    jump morning"));
    assert!(result.source.contains("music.stop() with fade(1500)"));
    let original=source.iter().find(|s|matches!(s,Statement::Dialogue { .. })).unwrap();
    assert!(result.ast.contains(original));
    let intro=&graphs[2];
    assert!(intro.nodes.values().any(|n|n.kind==NodeKind::MathAdd));
    assert!(intro.nodes.values().any(|n|n.kind==NodeKind::TextValue));
}

#[test]
fn import_preserves_implicit_label_flow_explicitly_and_refuses_lossy_input() {
    let graphs=import_script(&rvn_parser::parse("label z\n narrator \"a\"\nlabel a\n narrator \"b\"\n").unwrap()).unwrap();
    assert!(transpile(&graphs[1]).unwrap().source.contains("jump a"));
    assert!(import_script(&rvn_parser::parse("label a\n music.volume(\"0.5\")\n").unwrap()).is_err());
    assert!(import_script(&rvn_parser::parse("label a\n jump a\n narrator \"unreachable\"\n").unwrap()).is_err());
    assert!(import_script(&rvn_parser::parse("use \"unresolved.rvn\"\n").unwrap()).is_err());
}

#[test]
fn project_end_cannot_fall_into_the_next_sorted_graph() {
    use rvn_core::{Engine,Interaction,TerminalRenderer};
    let graphs=import_script(&rvn_parser::parse("label z\n narrator \"first\"\nlabel a\n narrator \"last\"\n").unwrap()).unwrap();
    let mut engine=Engine::new(transpile_project(&graphs).unwrap().ast,TerminalRenderer,20).unwrap();
    engine.state.pc=engine.script.iter().position(|s|matches!(s,Statement::Label { name } if name=="z")).unwrap();
    for expected in ["first","last"] {
        assert!(matches!(engine.step_until_interaction().unwrap(),Some(Interaction::Dialogue { text,.. }) if text==expected));
        engine.advance_dialogue().unwrap();
    }
    assert!(engine.step_until_interaction().unwrap().is_none());
    assert!(engine.is_finished());
}

#[test]
fn legacy_call_load_adds_an_unconnected_continuation() {
    let mut g=GraphDocument::new(GraphId::new(1),GraphKind::Init);
    let n=g.add_node(NodeKind::Call,[0.0,0.0]);
    g.add_pin(n,"target","Label",PinDirection::Input,ValueType::Label,PinCardinality::One).unwrap();
    let restored=GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();
    assert!(restored.pin_by_key(n,"exec_out").is_some());
    assert!(restored.edges.is_empty());
    assert_eq!(GraphDocument::from_json(&restored.to_pretty_json().unwrap()).unwrap(),restored);
}

#[test]
fn explicit_final_jump_does_not_generate_an_unreachable_exit() {
    let script=rvn_parser::parse("label start\n jump finish\nlabel finish\n narrator \"Done\"\n").unwrap();
    let compiled=transpile_project(&import_script(&script).unwrap()).unwrap();
    assert!(!compiled.ast.windows(2).any(|pair|matches!((&pair[0],&pair[1]),(Statement::Jump{..},Statement::Jump{..}))));
    assert_eq!(compiled.source.matches("jump __blueprint_project_end").count(),1);
}

#[test]
fn label_and_position_defaults_become_visible_typed_references() {
    let mut g=GraphDocument::new(GraphId::new(1),GraphKind::Label{name:"start".into()});
    let root=g.add_catalog_node(NodeKind::Label,[0.0,0.0]).unwrap();
    let jump=g.add_catalog_node(NodeKind::Jump,[300.0,0.0]).unwrap();
    g.set_pin_default(jump,"target",PropertyValue::String("start".into())).unwrap();
    g.connect(g.pin_by_key(root,"exec_out").unwrap().id,g.pin_by_key(jump,"exec_in").unwrap().id).unwrap();
    let sprite=g.add_catalog_node(NodeKind::SpriteShow,[0.0,300.0]).unwrap();
    g.materialize_visible_defaults().unwrap();
    for (owner,key,kind,ty) in [(jump,"target",NodeKind::LabelValue,ValueType::Label),(sprite,"position",NodeKind::PositionValue,ValueType::Position)] {
        let input=g.pin_by_key(owner,key).unwrap();assert!(input.default_value.is_none());
        let edge=g.edges.values().find(|e|e.input==input.id).unwrap();
        assert_eq!(g.pins[&edge.output].value_type,ty);
        assert_eq!(g.nodes[&g.pins[&edge.output].node].kind,kind);
    }
    let before=g.clone();assert_eq!(g.materialize_visible_defaults().unwrap(),0);assert_eq!(g,before);
    let id=g.pin_by_key(jump,"target").unwrap().id;
    let edge=g.edges.values().find(|e|e.input==id).unwrap().id;
    g.remove_edge(edge);
    assert!(transpile(&g).is_err(),"Disconnecting a label must not leave a hidden target");
}
