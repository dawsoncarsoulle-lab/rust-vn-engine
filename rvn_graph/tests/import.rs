use rvn_graph::*;
use rvn_parser::Statement;

#[test]
fn import_has_typed_variables_editable_text_branches_and_call_continuation() {
    let source = rvn_parser::parse(
        r#"
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
"#,
    )
    .unwrap();
    let graphs = import_script(&source).unwrap();
    assert_eq!(graphs.len(), 4);
    for graph in &graphs {
        assert_eq!(
            GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),
            *graph
        );
        assert_eq!(graph.variables["trust"].value_type, ValueType::Int);
        assert_eq!(graph.characters["narrator"], "");
        assert!(graph
            .validate()
            .iter()
            .all(|d| d.severity != DiagnosticSeverity::Error));
    }
    let result = transpile_project(&graphs).unwrap();
    assert!(result.source.contains("call intro\n    jump morning"));
    assert!(result.source.contains("music.stop() with fade(1500)"));
    let original = source
        .iter()
        .find(|s| matches!(s, Statement::Dialogue { .. }))
        .unwrap();
    assert!(result.ast.contains(original));
    let intro = &graphs[2];
    assert!(intro.nodes.values().any(|n| n.kind == NodeKind::MathAdd));
    assert!(intro.nodes.values().any(|n| n.kind == NodeKind::TextValue));
}

#[test]
fn import_preserves_implicit_label_flow_explicitly_and_refuses_lossy_input() {
    let graphs = import_script(
        &rvn_parser::parse("label z\n narrator \"a\"\nlabel a\n narrator \"b\"\n").unwrap(),
    )
    .unwrap();
    assert!(transpile(&graphs[1]).unwrap().source.contains("jump a"));
    assert!(
        import_script(&rvn_parser::parse("label a\n music.volume(\"0.5\")\n").unwrap()).is_ok()
    );
    assert!(import_script(
        &rvn_parser::parse("label a\n jump a\n narrator \"unreachable\"\n").unwrap()
    )
    .is_err());
    assert!(import_script(&rvn_parser::parse("use \"unresolved.rvn\"\n").unwrap()).is_err());
}

#[test]
fn project_end_cannot_fall_into_the_next_sorted_graph() {
    use rvn_core::{Engine, Interaction, TerminalRenderer};
    let graphs = import_script(
        &rvn_parser::parse("label z\n narrator \"first\"\nlabel a\n narrator \"last\"\n").unwrap(),
    )
    .unwrap();
    let mut engine = Engine::new(
        transpile_project(&graphs).unwrap().ast,
        TerminalRenderer,
        20,
    )
    .unwrap();
    engine.state.pc = engine
        .script
        .iter()
        .position(|s| matches!(s,Statement::Label { name } if name=="z"))
        .unwrap();
    for expected in ["first", "last"] {
        assert!(
            matches!(engine.step_until_interaction().unwrap(),Some(Interaction::Dialogue { text,.. }) if text==expected)
        );
        engine.advance_dialogue().unwrap();
    }
    assert!(engine.step_until_interaction().unwrap().is_none());
    assert!(engine.is_finished());
}

#[test]
fn music_volume_import_uses_float_pin_and_reparses_historical_quoted_syntax() {
    let source = "init {music.volume(\"0.22\")}\nlabel start\nmusic.volume(\"0.32\")\nmusic.volume(\"1\")\n\"Music\"\nreturn\n";
    let ast = rvn_parser::parse(source).unwrap();
    let graphs = import_script(&ast).unwrap();
    let levels = graphs
        .iter()
        .flat_map(|graph| {
            graph
                .nodes
                .values()
                .filter(|node| node.kind == NodeKind::MusicVolume)
                .map(|node| {
                    let pin = graph.pin_by_key(node.id, "level").unwrap();
                    assert_eq!(pin.value_type, ValueType::Float);
                    let edge = graph
                        .edges
                        .values()
                        .find(|edge| edge.input == pin.id)
                        .expect("The imported numeric value must be visible as a literal node");
                    let literal = &graph.nodes[&graph.pins[&edge.output].node];
                    assert_eq!(literal.kind, NodeKind::Literal);
                    let Some(PropertyValue::Float(value)) = literal.properties.get("value") else {
                        panic!("Volume must use a real floating literal")
                    };
                    *value as f32
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(levels, vec![0.22, 0.32, 1.0]);
    let output = transpile_project(&graphs).unwrap();
    assert_eq!(
        output.source.matches("music.volume(\"").count(),
        3,
        "The historical parser requires quoted method arguments"
    );
    assert!(output
        .ast
        .iter()
        .any(|statement| matches!(statement,Statement::MusicVolume{level} if *level==0.32)));
    assert!(output.ast.iter().any(|statement|matches!(statement,Statement::Init{body} if body.iter().any(|statement|matches!(statement,Statement::MusicVolume{level} if *level==0.22)))));
    assert_eq!(rvn_parser::parse(&output.source).unwrap(), output.ast);
    assert_eq!(
        import_script(&output.ast)
            .unwrap()
            .iter()
            .flat_map(|graph| graph.nodes.values())
            .filter(|node| node.kind == NodeKind::MusicVolume)
            .count(),
        3
    );
}

#[test]
fn legacy_call_load_adds_an_unconnected_continuation() {
    let mut g = GraphDocument::new(GraphId::new(1), GraphKind::Init);
    let n = g.add_node(NodeKind::Call, [0.0, 0.0]);
    g.add_pin(
        n,
        "target",
        "Label",
        PinDirection::Input,
        ValueType::Label,
        PinCardinality::One,
    )
    .unwrap();
    let restored = GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();
    assert!(restored.pin_by_key(n, "exec_out").is_some());
    assert!(restored.edges.is_empty());
    assert_eq!(
        GraphDocument::from_json(&restored.to_pretty_json().unwrap()).unwrap(),
        restored
    );
}

#[test]
fn explicit_final_jump_does_not_generate_an_unreachable_exit() {
    let script =
        rvn_parser::parse("label start\n jump finish\nlabel finish\n narrator \"Done\"\n").unwrap();
    let compiled = transpile_project(&import_script(&script).unwrap()).unwrap();
    assert!(!compiled.ast.windows(2).any(|pair| matches!(
        (&pair[0], &pair[1]),
        (Statement::Jump { .. }, Statement::Jump { .. })
    )));
    assert_eq!(
        compiled
            .source
            .matches("jump __blueprint_project_end")
            .count(),
        1
    );
}

#[test]
fn label_and_position_defaults_become_visible_typed_references() {
    let mut g = GraphDocument::new(
        GraphId::new(1),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let root = g.add_catalog_node(NodeKind::Label, [0.0, 0.0]).unwrap();
    let jump = g.add_catalog_node(NodeKind::Jump, [300.0, 0.0]).unwrap();
    g.set_pin_default(jump, "target", PropertyValue::String("start".into()))
        .unwrap();
    g.connect(
        g.pin_by_key(root, "exec_out").unwrap().id,
        g.pin_by_key(jump, "exec_in").unwrap().id,
    )
    .unwrap();
    let sprite = g
        .add_catalog_node(NodeKind::SpriteShow, [0.0, 300.0])
        .unwrap();
    g.materialize_visible_defaults().unwrap();
    for (owner, key, kind, ty) in [
        (jump, "target", NodeKind::LabelValue, ValueType::Label),
        (
            sprite,
            "position",
            NodeKind::PositionValue,
            ValueType::Position,
        ),
    ] {
        let input = g.pin_by_key(owner, key).unwrap();
        assert!(input.default_value.is_none());
        let edge = g.edges.values().find(|e| e.input == input.id).unwrap();
        assert_eq!(g.pins[&edge.output].value_type, ty);
        assert_eq!(g.nodes[&g.pins[&edge.output].node].kind, kind);
    }
    let before = g.clone();
    assert_eq!(g.materialize_visible_defaults().unwrap(), 0);
    assert_eq!(g, before);
    let id = g.pin_by_key(jump, "target").unwrap().id;
    let edge = g.edges.values().find(|e| e.input == id).unwrap().id;
    g.remove_edge(edge);
    assert!(
        transpile(&g).is_err(),
        "Disconnecting a label must not leave a hidden target"
    );
}


#[test]
fn timer_jump_and_cancel_import_keep_actual_nodes_durations_and_targets_in_native_source() {
    let script=rvn_parser::parse("label start\ntimer 5.25 => jump done\ntimer cancel\n\"Ready\"\nreturn\nlabel done\n\"Done\"\n").unwrap();
    let mut graphs=import_script(&script).expect("existing Timer catalogue nodes must import their lossless RVN form");
    let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let timer=graph.nodes.values().find(|node|node.kind==NodeKind::Timer).unwrap().id;
    // Import creates visible producer nodes; a hidden default is not the operand.
    for (key,kind,property,value) in [("duration",NodeKind::Literal,"value",PropertyValue::Float(5.25)),("target",NodeKind::LabelValue,"label",PropertyValue::String("done".into()))] {
        let input=graph.pin_by_key(timer,key).unwrap();assert!(input.default_value.is_none());
        let edge=graph.edges.values().find(|edge|edge.input==input.id).unwrap();
        let source=&graph.nodes[&graph.pins[&edge.output].node];assert_eq!(source.kind,kind);assert_eq!(source.properties.get(property),Some(&value));
    }
    assert_eq!(graph.nodes.values().filter(|node|node.kind==NodeKind::TimerCancel).count(),1);
    let ordinary=transpile_project(&graphs).unwrap();assert!(ordinary.ast.contains(&Statement::Timer{duration_secs:5.25,action:"jump done".into()}));assert!(ordinary.ast.contains(&Statement::TimerCancel));
    enable_native_timer_labels_with_real_legacy_init(&mut graphs);
    let native=transpile_project(&graphs).expect("actual native Label Consumer path with unchanged empty Legacy Init");
    assert!(native.source.contains("flow.consume_v2("));
    let program=rvn_parser::parse_program(&native.source).unwrap();rvn_parser::scheduled_source::scheduled_step_sources(&program).unwrap();
    let restored=import_native_frame_script(&native.ast).unwrap();
    for original in &graphs{let actual=restored.iter().find(|g|g.graph_id==original.graph_id).unwrap();assert_eq!(actual.edges,original.edges);assert_eq!(actual.pins.keys().collect::<Vec<_>>(),original.pins.keys().collect::<Vec<_>>());assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),original.nodes.keys().collect::<Vec<_>>());}
}

#[test]
fn timer_call_is_refused_instead_of_silently_changing_existing_jump_node_semantics() {
    let script=rvn_parser::parse("label start\ntimer 5 => call done\n\"Ready\"\nreturn\nlabel done\nreturn\n").unwrap();
    assert!(import_script(&script).unwrap_err().contains("losslessly"));
}


#[test]
fn native_timer_literal_fields_cannot_borrow_a_different_constant_source_output() {
    let script=rvn_parser::parse("label start\ntimer 5.25 => jump done\n\"Ready\"\nreturn\nlabel done\n\"Done\"\n").unwrap();
    let mut graphs=import_script(&script).unwrap();enable_native_timer_labels_with_real_legacy_init(&mut graphs);
    let native=transpile_project(&graphs).unwrap();
    for replacement in ["timer 9.25 => jump done","timer 5.25 => jump start"] {
        let bad=native.source.replace("timer 5.25 => jump done",replacement);assert_ne!(bad,native.source);
        let program=rvn_parser::parse_program(&bad).unwrap();
        assert!(rvn_parser::scheduled_source::scheduled_step_sources(&program).is_err());
        assert!(import_native_frame_script(&program.script).is_err());
        assert!(rvn_core::Engine::from_program(program,rvn_core::TerminalRenderer,8).is_err());
    }
}


#[test]
fn timer_seconds_above_one_do_not_relax_static_music_volume_domain() {
    // Keep the historical parser's valid clamp, then test the actual static
    // admission against separately forged AST and actual reparsed source bytes.
    let historical=rvn_parser::parse("label start\nmusic.volume(\"1.5\")\n\"Ready\"\n").unwrap();
    assert!(historical.iter().any(|s|matches!(s,Statement::MusicVolume{level}if *level==1.0)));
    let script=rvn_parser::parse("label start\nmusic.volume(\"0.5\")\n\"Ready\"\n").unwrap();
    let mut graphs=import_script(&script).unwrap();enable_native_timer_labels_with_real_legacy_init(&mut graphs);
    let native=transpile_project(&graphs).unwrap();let mut program=rvn_parser::parse_program(&native.source).unwrap();
    let mut changed=0;
    for statement in &mut program.script {
        if let Statement::ScheduledConsumer{steps,body,..}=statement {
            if let [Statement::MusicVolume{level}]=body.as_mut_slice() {
                assert_eq!(*level,0.5);*level=1.5;
                let mut captured=0;
                for step in steps {
                    if let rvn_parser::ConsumerStep::Capture{value:rvn_parser::Expr::Float(value),..}=step {
                        if *value==0.5{*value=1.5;captured+=1;}
                    }
                }
                assert_eq!(captured,1);changed+=1;
            }
        }
    }
    assert_eq!(changed,1);
    let error=rvn_parser::scheduled_source::scheduled_step_sources(&program).unwrap_err();
    assert!(error.contains("Scheduled root source/AST mismatch"),"{error}");
    // Reach the numeric-domain check with actual reparsed source bytes. The
    // historical command clamps to one, while its real captured constant is
    // still 1.5; a Timer's wider duration domain cannot admit that volume.
    assert_eq!(native.source.matches("0.5").count(),2);
    let forged_source=native.source.replace("0.5","1.5");
    let forged=rvn_parser::parse_program(&forged_source).unwrap();
    let mut commands=0;let mut constants=0;
    for statement in &forged.script {
        if let Statement::ScheduledConsumer{steps,body,..}=statement {
            if let [Statement::MusicVolume{level}]=body.as_slice() {
                assert_eq!(*level,1.0);commands+=1;
                constants+=steps.iter().filter(|step|matches!(step,
                    rvn_parser::ConsumerStep::Capture{value:rvn_parser::Expr::Float(value),..}if *value==1.5)).count();
            }
        }
    }
    assert_eq!((commands,constants),(1,1));
    let error=rvn_parser::scheduled_source::scheduled_step_sources(&forged).unwrap_err();
    assert!(error.contains("Audio volume constant differs"),"{error}");
    assert!(import_native_frame_script(&forged.script).is_err());
    assert!(rvn_core::Engine::from_program(forged,rvn_core::TerminalRenderer,8).is_err());
}

/// Timer tests exercise actual Label consumers. The implicit empty Init is a
/// real Legacy context view; no statement or Native identity is fabricated.
fn enable_native_timer_labels_with_real_legacy_init(graphs:&mut [GraphDocument]) {
    let init=graphs.iter().find(|graph|graph.kind==GraphKind::Init).expect("ordinary import keeps the implicit Init view");
    assert!(init.inspector_evaluation.is_legacy());assert_eq!(init.nodes.len(),1);assert!(init.nodes.values().all(|node|node.kind==NodeKind::Init));assert!(init.edges.is_empty());
    for graph in graphs.iter_mut().filter(|graph|matches!(graph.kind,GraphKind::Label{..})) {graph.enable_native_frame_preparation().unwrap();}
    assert!(graphs.iter().filter(|graph|graph.kind==GraphKind::Init).all(|graph|graph.inspector_evaluation.is_legacy()));
}
#[test]
fn empty_implicit_init_native_identity_gap_is_explicit_and_never_filled_with_an_invented_statement() {
    let script=rvn_parser::parse("label start\nreturn\n").unwrap();let graphs=import_script(&script).unwrap();
    let init=graphs.iter().find(|graph|graph.kind==GraphKind::Init).unwrap();assert!(init.edges.is_empty());assert_eq!(init.nodes.len(),1);
    let ordinary=transpile_with_origins(init).unwrap();assert!(matches!(ordinary.ast.as_slice(),[Statement::Init{body}]if body.is_empty()));
    let mut requested=init.clone();requested.enable_native_frame_preparation().unwrap();let guarded=requested.clone();
    // Historical Graph12 refusal remains preserved in its actual old receipt.
    // A source-aware declaration now transports the root; AST-only import
    // still cannot invent that provenance and remains the negative boundary.
    let declared=prepare_native_frame_graph(&requested).unwrap();
    assert_eq!(declared.ast,ordinary.ast,"declaration adds no statement, consumer or PC");
    assert!(import_native_frame_script(&declared.ast).is_err());
    let restored=import_native_frame_source(&declared.source,declared.context).unwrap();
    assert!(restored.iter().any(|graph|graph.graph_id==requested.graph_id&&!graph.inspector_evaluation.is_legacy()));
    assert_eq!(requested,guarded,"preparation must not mutate the requested graph");
}
