use rvn_graph::*;

#[test]
fn deleting_a_character_used_in_a_different_scope_is_atomic() {
    let source = "init { character.create(\"iris\", \"Iris\") }\nlabel start\n\"Hello\"\nreturn\nlabel other\niris \"Still needed\"\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let before = project.graphs().to_vec();
    let mut edited = before.clone();
    edited
        .iter_mut()
        .find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start"))
        .unwrap()
        .characters
        .remove("iris");
    assert!(project
        .apply_visual(source, &edited)
        .unwrap_err()
        .contains("still used"));
    assert_eq!(project.source(), source);
    assert_eq!(project.graphs(), before);
}

#[test]
fn removed_character_checks_recurse_without_rejecting_legacy_implicit_speakers() {
    let source = "init { character.create(\"iris\", \"Iris\") }\nlabel start\nif true { iris \"Needed inside branch\" }\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut edited = project.graphs().to_vec();
    edited[0].characters.remove("iris");
    assert!(project
        .apply_visual(source, &edited)
        .unwrap_err()
        .contains("still used"));
    assert_eq!(project.source(), source);
    let legacy = "init { character.create(\"unused\", \"Unused\") }\nlabel start\nimplicit \"Legacy speaker\"\nreturn\n";
    let mut project = SourceProject::open(legacy, &[]).unwrap();
    let mut edited = project.graphs().to_vec();
    edited[0].characters.remove("unused");
    project.apply_visual(legacy, &edited).unwrap();
    assert!(project.source().contains("implicit \"Legacy speaker\""));
}

#[test]
fn deleting_initialization_cannot_silently_delete_project_characters() {
    let source = "init { character.create(\"iris\", \"Iris\") }\nlabel start\n\"Hello\"\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let before = project.graphs().to_vec();
    let mut edited = before.clone();
    edited.retain(|graph| graph.kind != GraphKind::Init);
    assert!(project
        .apply_visual(source, &edited)
        .unwrap_err()
        .contains("shared character declarations"));
    assert_eq!(project.source(), source);
    assert_eq!(project.graphs(), before);
}

const SOURCE: &str = "// Project note\nfunction reward(n) {\n    // Keep this calculation note\n    return n + 2 // Tail note\n}\n\ninit {\n    // Saved score\n    set score = 0\n}\n\nlabel start\n    // Opening note\n    \"Hello\"\n    set score = reward(3)\n    jump done\n\nlabel done\n    \"Done\"\n    return\n";

#[test]
fn music_volume_source_roundtrip_preserves_comments_identity_and_actual_level() {
    let original = "// Music note\nlabel start\n    // Opening mix\n    music.volume(\"0.22\") // Preserve tail\n    \"Hello\"\n    return\nlabel other\n    // Untouched scene\n    \"Later\"\n    return\n";
    let mut project = SourceProject::open(original, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|graph| {
            graph.kind
                == GraphKind::Label {
                    name: "start".into(),
                }
        })
        .unwrap();
    let volume = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::MusicVolume)
        .unwrap()
        .id;
    graph.nodes.get_mut(&volume).unwrap().position = [430.0, 210.0];
    project.apply_visual(original, &graphs).unwrap();
    assert_eq!(
        project.source(),
        original,
        "A node placement must not rewrite the authored source"
    );
    let linked = project.source().to_owned();
    let mut graphs = project.graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|graph| {
            graph.kind
                == GraphKind::Label {
                    name: "start".into(),
                }
        })
        .unwrap();
    let input = graph.pin_by_key(volume, "level").unwrap().id;
    let literal = graph.pins[&graph
        .edges
        .values()
        .find(|edge| edge.input == input)
        .unwrap()
        .output]
        .node;
    graph
        .set_property(literal, "value", PropertyValue::Float(0.4))
        .unwrap();
    project.apply_visual(&linked, &graphs).unwrap();
    let edited = project.source().to_owned();
    for comment in [
        "// Music note",
        "// Opening mix",
        "// Preserve tail",
        "// Untouched scene",
    ] {
        assert_eq!(edited.matches(comment).count(), 1);
    }
    assert!(edited.ends_with("label other\n    // Untouched scene\n    \"Later\"\n    return\n"));
    assert!(rvn_parser::parse(&edited).unwrap().iter().any(
        |statement| matches!(statement,rvn_parser::Statement::MusicVolume{level} if *level==0.4)
    ));
    let reopened = SourceProject::open(&edited, project.graphs()).unwrap();
    let graph = reopened
        .graphs()
        .iter()
        .find(|graph| {
            graph.kind
                == GraphKind::Label {
                    name: "start".into(),
                }
        })
        .unwrap();
    assert_eq!(graph.nodes[&volume].position, [430.0, 210.0]);
    assert_eq!(graph.nodes[&literal].properties["value"],PropertyValue::Float(0.4),"Reopening must preserve the author's existing visual value when the parsed float semantics are equivalent");
    assert_eq!(reopened.source(), edited);
}

#[test]
fn linking_imagemap_to_an_existing_presentation_preserves_source_and_branch_owners() {
    let prior = import_script(
        &rvn_parser::parse(
            "init{character.create(\"narrator\",\"\")}\nlabel start\nnarrator \"Finished.\"\n",
        )
        .unwrap(),
    )
    .unwrap();
    let source="// Source note\ninit{character.create(\"narrator\",\"\")}\nlabel start\n    // Area note\n    imagemap {\n        background: \"stage.png\"\n        hotspot {name: \"door\" area: (20,20,200,180)} => {\n            // Inside note\n            narrator \"Clicked.\"\n        }\n    }\n    narrator \"Finished.\"\n";
    let mut project = SourceProject::open(source, &prior).unwrap();
    assert_eq!(project.source(), source);
    let mut edited = project.graphs().to_vec();
    let graph = edited
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Label { .. }))
        .unwrap();
    let map = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::Imagemap)
        .unwrap()
        .id;
    graph
        .set_imagemap_hotspots(map, vec!["door:25:30:210:190".into()])
        .unwrap();
    project.apply_visual(source, &edited).unwrap();
    for comment in ["// Source note", "// Area note", "// Inside note"] {
        assert_eq!(project.source().matches(comment).count(), 1);
    }
    assert!(project.source().ends_with("    narrator \"Finished.\"\n"));
    assert_eq!(
        project
            .graphs()
            .iter()
            .find(|graph| matches!(graph.kind, GraphKind::Label { .. }))
            .unwrap()
            .nodes[&map]
            .properties["hotspots"],
        PropertyValue::StringList(vec!["door:25:30:210:190".into()])
    );
    project.refresh(project.source().to_string()).unwrap();
    transpile_project(project.graphs()).unwrap();
}

fn change_number(graphs: &mut [GraphDocument], kind: GraphKind, from: i64, to: i64) {
    let graph = graphs.iter_mut().find(|graph| graph.kind == kind).unwrap();
    let id = graph
        .nodes
        .values()
        .find(|node| {
            node.kind == NodeKind::Literal
                && node.properties.get("value") == Some(&PropertyValue::Int(from))
        })
        .unwrap()
        .id;
    graph
        .set_property(id, "value", PropertyValue::Int(to))
        .unwrap();
}

#[test]
fn edits_multiple_scopes_without_erasing_comments_or_unmodified_scope_bytes() {
    let mut project = SourceProject::open(SOURCE, &[]).unwrap();
    let mut edited = project.graphs().to_vec();
    change_number(
        &mut edited,
        GraphKind::Function {
            name: "reward".into(),
        },
        2,
        5,
    );
    let init = edited
        .iter_mut()
        .find(|g| g.kind == GraphKind::Init)
        .unwrap();
    let set = init
        .nodes
        .values()
        .find(|n| n.kind == NodeKind::SetVariable)
        .unwrap()
        .id;
    let value = init.pin_by_key(set, "value").unwrap().id;
    init.pins.get_mut(&value).unwrap().default_value = Some(PropertyValue::Int(4));
    project.apply_visual(SOURCE, &edited).unwrap();
    for note in [
        "// Project note",
        "// Keep this calculation note",
        "// Tail note",
        "// Saved score",
        "// Opening note",
    ] {
        assert!(
            project.source().contains(note),
            "Lost {note}: {}",
            project.source()
        );
    }
    assert!(project
        .source()
        .ends_with("label done\n    \"Done\"\n    return\n"));
    let ast = rvn_parser::parse(project.source()).unwrap();
    let mut engine = rvn_core::Engine::new(ast, rvn_core::TerminalRenderer, 16).unwrap();
    engine.step_until_interaction().unwrap();
    assert_eq!(engine.state.vars["score"], rvn_parser::Value::Int(4));
    engine.advance_dialogue().unwrap();
    engine.step_until_interaction().unwrap();
    assert_eq!(engine.state.vars["score"], rvn_parser::Value::Int(8));
}

#[test]
fn layout_only_saves_preserve_the_source_exactly_and_reimport_keeps_layout() {
    let mut project = SourceProject::open(SOURCE, &[]).unwrap();
    let mut edited = project.graphs().to_vec();
    for graph in &mut edited {
        for node in graph.nodes.values_mut() {
            node.position = [12000.0, -1200.0];
        }
    }
    project.apply_visual(SOURCE, &edited).unwrap();
    assert_eq!(project.source(), SOURCE);
    project.refresh(SOURCE).unwrap();
    assert_eq!(project.graphs(), edited);
}

#[test]
fn source_conflicts_invalid_edits_and_invalid_refresh_are_atomic() {
    let mut project = SourceProject::open(SOURCE, &[]).unwrap();
    let original = project.graphs().to_vec();
    let mut edited = original.clone();
    change_number(
        &mut edited,
        GraphKind::Function {
            name: "reward".into(),
        },
        2,
        5,
    );
    assert!(project
        .apply_visual(&format!("{SOURCE}// external edit\n"), &edited)
        .is_err());
    assert!(project.refresh("function incomplete(").is_err());
    assert_eq!(project.source(), SOURCE);
    assert_eq!(project.graphs(), original);
    let call = edited
        .iter_mut()
        .find(|g| matches!(&g.kind, GraphKind::Label { name } if name == "start"))
        .unwrap();
    let id = call
        .nodes
        .values()
        .find(|n| n.kind == NodeKind::FunctionCall)
        .unwrap()
        .id;
    call.set_property(id, "function", PropertyValue::String("not_defined".into()))
        .unwrap();
    assert!(project.apply_visual(SOURCE, &edited).is_err());
    assert_eq!(project.source(), SOURCE);
    assert_eq!(project.graphs(), original);
}

#[test]
fn repeated_visual_source_roundtrips_retain_comments_and_node_identity() {
    let mut project = SourceProject::open(SOURCE, &[]).unwrap();
    let graph = project
        .graphs()
        .iter()
        .find(|g| matches!(&g.kind, GraphKind::Function { name } if name == "reward"))
        .unwrap();
    let id = graph
        .nodes
        .values()
        .find(|n| n.properties.get("value") == Some(&PropertyValue::Int(2)))
        .unwrap()
        .id;
    for number in 3..7 {
        let mut edited = project.graphs().to_vec();
        change_number(
            &mut edited,
            GraphKind::Function {
                name: "reward".into(),
            },
            number - 1,
            number,
        );
        let source = project.source().to_owned();
        project.apply_visual(&source, &edited).unwrap();
        let source = project.source().to_owned();
        project.refresh(source).unwrap();
        let graph = project
            .graphs()
            .iter()
            .find(|g| matches!(&g.kind, GraphKind::Function { name } if name == "reward"))
            .unwrap();
        assert_eq!(
            graph.nodes[&id].properties["value"],
            PropertyValue::Int(number)
        );
        assert_eq!(project.source().matches("// Tail note").count(), 1);
    }
}

#[test]
fn visual_scope_creation_and_deletion_keep_untouched_source_and_comments() {
    let source = "// project\nfunction unused(n) {\n // retain deleted scope note\n return n + 1\n}\nlabel start\n\"unchanged\"\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    graphs.retain(|graph| !matches!(&graph.kind, GraphKind::Function { name } if name == "unused"));
    let mut additions = import_script(
        &rvn_parser::parse(
            "function new_score(n) { return n * 2 }\nlabel extra\n\"extra\"\nreturn\n",
        )
        .unwrap(),
    )
    .unwrap();
    additions.retain(|graph| graph.kind != GraphKind::Init);
    for (index, graph) in additions.iter_mut().enumerate() {
        graph.graph_id = GraphId::new(100 + index as u64);
    }
    graphs.extend(additions);
    project.apply_visual(source, &graphs).unwrap();
    assert!(project.source().contains("// retain deleted scope note"));
    assert!(project
        .source()
        .contains("label start\n\"unchanged\"\nreturn"));
    let ast = rvn_parser::parse(project.source()).unwrap();
    assert!(!ast.iter().any(|statement| matches!(statement, rvn_parser::Statement::Function { name, .. } if name == "unused")));
    assert!(ast.iter().any(|statement| matches!(statement, rvn_parser::Statement::Function { name, .. } if name == "new_score")));
    assert!(ast.iter().any(
        |statement| matches!(statement, rvn_parser::Statement::Label { name } if name == "extra")
    ));
    let before = project.graphs().to_vec();
    project.refresh(project.source().to_string()).unwrap();
    let by_id = |graphs: &[GraphDocument]| {
        graphs
            .iter()
            .map(|graph| (graph.graph_id, graph.clone()))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    assert_eq!(by_id(project.graphs()), by_id(&before));
}

#[test]
fn adding_a_label_cannot_change_an_existing_implicit_fallthrough() {
    let source = "label start\n\"end without return\"\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let mut extra = import_script(&rvn_parser::parse("label extra\nreturn\n").unwrap())
        .unwrap()
        .into_iter()
        .find(|graph| matches!(graph.kind, GraphKind::Label { .. }))
        .unwrap();
    extra.graph_id = GraphId::new(100);
    graphs.push(extra);
    assert!(project
        .apply_visual(source, &graphs)
        .unwrap_err()
        .contains("fallthrough"));
    assert_eq!(project.source(), source);
}

#[test]
fn character_creation_from_a_label_is_project_wide_and_preserves_authored_init() {
    let source="// project\ninit {\n // cast note\n character.create(\"iris\",\"Iris\")\n // variable note\n set score = 0\n}\nlabel start\n\"hello\"\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    graphs
        .iter_mut()
        .find(|graph| matches!(graph.kind, GraphKind::Label { .. }))
        .unwrap()
        .characters
        .insert("noe".into(), "Noé 🍃".into());
    project.apply_visual(source, &graphs).unwrap();
    let ast = rvn_parser::parse(project.source()).unwrap();
    assert!(ast.iter().any(|statement|matches!(statement,rvn_parser::Statement::Init{body} if body.iter().any(|statement|matches!(statement,rvn_parser::Statement::CharacterCreate{id,display_name} if id=="noe"&&display_name=="Noé 🍃")))),"{}",project.source());
    assert!(project.source().contains(" // cast note\n character.create(\"iris\",\"Iris\")\n // variable note\n set score = 0"),"{}",project.source());
    assert!(project
        .source()
        .ends_with("label start\n\"hello\"\nreturn\n"));
    assert_eq!(project.source().matches("init {").count(), 1);
    assert!(project.graphs().iter().all(|graph| graph
        .characters
        .get("noe")
        .is_some_and(|name| name == "Noé 🍃")));
    let graphs = project.graphs().to_vec();
    let saved = project.source().to_owned();
    project.refresh(saved.clone()).unwrap();
    assert_eq!(project.graphs(), graphs);
    project.apply_visual(&saved, &graphs).unwrap();
    assert_eq!(project.source(), saved);
}

#[test]
fn source_character_conflicts_never_overwrite_the_script_or_presentation() {
    let mut project = SourceProject::open(SOURCE, &[]).unwrap();
    let baseline = project.graphs().to_vec();
    let mut graphs = baseline.clone();
    graphs[0].characters.insert("iris".into(), "Iris".into());
    graphs[1].characters.insert("iris".into(), "Other".into());
    assert!(project
        .apply_visual(SOURCE, &graphs)
        .unwrap_err()
        .contains("Conflicting character"));
    assert_eq!(project.source(), SOURCE);
    assert_eq!(project.graphs(), baseline);
}

#[test]
fn undo_saved_character_creation_and_edit_display_name_preserve_shared_registry_and_comments() {
    let source="// project\ninit {\n // existing cast\n character.create(\"iris\",\"Iris\") // name note\n set score = 0\n}\nlabel start\n\"hello\"\nreturn\nlabel other\n\"other\"\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let label = graphs
        .iter_mut()
        .find(|graph| matches!(&graph.kind,GraphKind::Label{name} if name=="start"))
        .unwrap();
    label.characters.insert("noe".into(), "Noé 🍃".into());
    project.apply_visual(source, &graphs).unwrap();
    let created = project.source().to_owned();
    let mut graphs = project.graphs().to_vec();
    // Undo in one graph, after a save has propagated the registry to all peers.
    graphs
        .iter_mut()
        .find(|graph| matches!(&graph.kind,GraphKind::Label{name} if name=="start"))
        .unwrap()
        .characters
        .remove("noe");
    project.apply_visual(&created, &graphs).unwrap();
    assert!(!project.source().contains("Noé 🍃"));
    assert!(project
        .graphs()
        .iter()
        .all(|graph| !graph.characters.contains_key("noe")));
    assert!(project
        .source()
        .contains("character.create(\"iris\",\"Iris\") // name note\n set score = 0"));
    let saved = project.source().to_owned();
    let mut graphs = project.graphs().to_vec();
    graphs[0]
        .characters
        .insert("iris".into(), "Iris nouvelle".into());
    project.apply_visual(&saved, &graphs).unwrap();
    assert!(project.source().contains("Iris nouvelle"));
    assert!(project.source().contains("// name note\n set score = 0"));
    assert!(project
        .graphs()
        .iter()
        .all(|graph| graph.characters["iris"] == "Iris nouvelle"));
    let saved = project.source().to_owned();
    let graphs = project.graphs().to_vec();
    project.refresh(saved).unwrap();
    assert_eq!(project.graphs(), graphs);
}
