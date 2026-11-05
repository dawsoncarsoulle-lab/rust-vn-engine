use rvn_graph::*;

#[test]
fn scene_boundary_type_save_cold_runtime_resets_child_and_restores_parent_on_call() {
    for transfer in ["jump","call"] {
        let continuation=if transfer=="call" {"\"Restored [value]\"\njump done\n"}else{""};
        let ending=if transfer=="call" {"return\nscene label done\n\"Done\"\n"}else{""};
        let source=format!("// Scene boundary\ninit {{set value=true}}\nscene label start\n// Local declaration\nlocal value=true\n\"Saved local [value]\"\n{transfer} next_scene\n{continuation}scene label next_scene\n\"Child global [value]\"\n{ending}");
        let (root,project)=fragment_rename_fixture(&source,"");
        std::fs::write(root.join("rvn.toml"),"[project]\nmain_script='main.rvn'\nstart_label='start'\n").unwrap();
        let mut workspace=SourceWorkspace::link(&root,&root.join("main.rvn"),&[]).unwrap();
        let before=project.graphs().to_vec();let start=before.iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
        let proposal=start.prepare_local_variable_type_slot("value",ValueType::Int).unwrap();
        assert!(proposal.requires_project_analysis());assert!(proposal.requires_confirmation());assert!(proposal.reconstruct(start).is_err());
        let next=project.reconstruct_local_variable_type(&proposal,start,&before).unwrap();
        let mut edited=before.clone();*edited.iter_mut().find(|g|g.graph_id==start.graph_id).unwrap()=next;
        assert_eq!(project.graphs(),before);assert_eq!(project.source(),source);
        workspace.save(&edited).unwrap();drop(workspace);
        let cold=SourceWorkspace::open(&root).unwrap().unwrap();
        let cold_start=cold.project().graphs().iter().find(|g|g.graph_id==start.graph_id).unwrap();
        assert_eq!(cold_start.variables["value"].value_type,ValueType::Int);
        assert_eq!(cold_start.local_variable_initializers("value").unwrap()[0].literal_value,Some(PropertyValue::Int(0)));
        for g in before.iter().filter(|g|g.graph_id!=start.graph_id){assert_eq!(cold.project().graphs().iter().find(|n|n.graph_id==g.graph_id),Some(g));}
        assert_eq!(cold.project().source().matches("// Scene boundary").count(),1);
        assert_eq!(cold.project().source().matches("// Local declaration").count(),1);
        let program=cold.project().resolved_program().unwrap();
        let mut runtime=rvn_core::Engine::from_program(program,rvn_core::TerminalRenderer,32).unwrap();
        for text in if transfer=="call" {vec!["Saved local 0","Child global true","Restored 0","Done"]} else {vec!["Saved local 0","Child global true"]} {
            assert_eq!(runtime.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:text.into()}));
            runtime.advance_dialogue().unwrap();
        }
        assert!(runtime.step_until_interaction().unwrap().is_none());
    }
}

#[test]
fn scene_boundary_type_imported_scene_is_readonly_and_external_changes_refuse() {
    let main="use \"helper.rvn\"\nscene label start\nlocal value=true\n\"Local [value]\"\njump child\n";
    let helper="scene label child\n\"Child\"\nreturn\n";
    let (root,project)=fragment_rename_fixture(main,helper);let before=project.graphs().to_vec();
    let graph=before.iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let proposal=graph.prepare_local_variable_type_slot("value",ValueType::Int).unwrap();
    assert!(project.reconstruct_local_variable_type(&proposal,graph,&before).is_ok());
    assert_eq!(std::fs::read_to_string(root.join("helper.rvn")).unwrap(),helper);
    std::fs::write(root.join("helper.rvn"),"label child\nreturn\n").unwrap();
    assert!(project.reconstruct_local_variable_type(&proposal,graph,&before).is_err());
    assert_eq!(project.graphs(),before);assert_eq!(std::fs::read_to_string(root.join("main.rvn")).unwrap(),main);
}

#[test]
fn scene_boundary_type_fragment_unknown_duplicate_and_stale_owner_refuse_atomically() {
    let source="scene label start\nlocal value=true\n\"Local [value]\"\njump child\nscene label child\n\"Child\"\nreturn\n";
    let project=SourceProject::open(source,&[]).unwrap();let before=project.graphs().to_vec();
    let owner=before.iter().position(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let child=before.iter().position(|g|matches!(&g.kind,GraphKind::Label{name}if name=="child")).unwrap();let graph=&before[owner];
    let proposal=graph.prepare_local_variable_type_slot("value",ValueType::Int).unwrap();
    for mode in 0..4 {
        let mut edited=before.clone();
        match mode {
            0=>edited[child].narrative=Some(NarrativeMetadata::SceneFragment),
            1=>{edited.remove(child);},
            2=>edited.push(before[child].clone()),
            _=>edited[owner].nodes.values_mut().next().unwrap().position[0]+=1.0,
        }
        assert!(project.reconstruct_local_variable_type(&proposal,graph,&edited).is_err());
        assert_eq!(project.graphs(),before);assert_eq!(project.source(),source);
    }
}

const LINKED_DIAGNOSTIC_SOURCE: &str = "init {set stable=0}\nscene label start\nlocal value=true\nif value {\"First\"}\nreturn\nscene label next_scene\nlocal value=true\nif value {\"Second\"}\nreturn\n";

fn linked_diagnostic_invalid_graphs(project:&SourceProject)->Vec<GraphDocument>{
    project.graphs().iter().map(|graph|{
        if graph.variable_scope("value")==Some(VariableScope::Local){
            graph.prepare_local_variable_type_slot("value",ValueType::Int).unwrap().reconstruct(graph).unwrap()
        }else{graph.clone()}
    }).collect()
}

#[test]
fn linked_source_diagnostic_collects_both_scope_failures_without_publishing(){
    for linked in [false,true]{
        let (directory,mut project)=fragment_rename_fixture(LINKED_DIAGNOSTIC_SOURCE,"");
        if !linked{project=SourceProject::open(LINKED_DIAGNOSTIC_SOURCE,&[]).unwrap();}
        let original=project.graphs().to_vec();let edited=linked_diagnostic_invalid_graphs(&project);
        let legacy=project.clone().apply_visual(LINKED_DIAGNOSTIC_SOURCE,&edited).unwrap_err();
        let failure=project.apply_visual_with_diagnostics(LINKED_DIAGNOSTIC_SOURCE,&edited).unwrap_err();
        assert_eq!(failure.message,legacy);assert_eq!(failure.graph_errors.len(),2);
        for scope in &failure.graph_errors{
            let authored=edited.iter().find(|graph|graph.graph_id==scope.document.graph_id).unwrap();
            assert_eq!(authored.kind,scope.document.kind);
            assert!(matches!(scope.error,TranspileError::InvalidGraph(_)));
            assert!(transpile(&scope.document).is_err());
        }
        assert_ne!(failure.graph_errors[0].document.graph_id,failure.graph_errors[1].document.graph_id);
        assert_eq!(project.graphs(),original);assert_eq!(project.source(),LINKED_DIAGNOSTIC_SOURCE);
        assert_eq!(std::fs::read_to_string(directory.join("main.rvn")).unwrap(),LINKED_DIAGNOSTIC_SOURCE);
    }
}

#[test]
fn linked_source_diagnostic_source_conflict_precedes_invalid_graphs(){
    let (_directory,mut project)=fragment_rename_fixture(LINKED_DIAGNOSTIC_SOURCE,"");
    let original=project.graphs().to_vec();let edited=linked_diagnostic_invalid_graphs(&project);
    let failure=project.apply_visual_with_diagnostics("// External edit\n",&edited).unwrap_err();
    assert!(failure.message.contains("RVN source changed"));assert!(failure.graph_errors.is_empty());
    assert_eq!(project.graphs(),original);assert_eq!(project.source(),LINKED_DIAGNOSTIC_SOURCE);
}

#[test]
fn linked_source_diagnostic_success_preserves_comments_and_legacy_contract(){
    let source="// Source note\nscene label start\n// Declaration note\nlocal value=true\n\"Ready\"\nreturn\n";
    let (_directory,mut project)=fragment_rename_fixture(source,"");
    let edited=linked_diagnostic_invalid_graphs(&project);let mut legacy=project.clone();
    project.apply_visual_with_diagnostics(source,&edited).unwrap();legacy.apply_visual(source,&edited).unwrap();
    assert_eq!(project.source(),legacy.source());assert_eq!(project.graphs(),legacy.graphs());
    assert_eq!(project.source().matches("// Source note").count(),1);
    assert_eq!(project.source().matches("// Declaration note").count(),1);
    assert_ne!(project.source(),source);assert!(transpile_project(project.graphs()).is_ok());
}

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


#[test]
fn source_linked_variable_rename_preserves_comments_ids_positions_and_exact_semantic_key() {
    let source="// Projet été\ninit {\n // Défaut\n set flag = true // Garder\n}\nlabel start\n // Lecture\n set flag = flag // Usage\n return\nlabel other\n \"Intact\"\n return\n";
    let mut project=SourceProject::open(source,&[]).unwrap();let mut graphs=project.graphs().to_vec();
    let original=graphs.clone();
    for graph in &mut graphs {if graph.variables.contains_key("flag") {graph.rename_variable("flag","Confiance été").unwrap();}}
    project.apply_visual(source,&graphs).unwrap();
    let linked=project.source();
    for comment in ["// Projet été","// Défaut","// Garder","// Lecture","// Usage"] {assert!(linked.contains(comment),"Lost {comment}");}
    assert!(linked.ends_with("label other\n \"Intact\"\n return\n"));
    let script=rvn_parser::parse(linked).unwrap();
    assert!(script.iter().any(|statement|matches!(statement,rvn_parser::Statement::SetVar{name,value:rvn_parser::Expr::Var(read)} if name=="Confiance été"&&read==name)));
    for (old,current) in original.iter().zip(project.graphs()) {
        assert_eq!(old.graph_id,current.graph_id);assert_eq!(old.edges,current.edges);
        assert_eq!(old.nodes.keys().collect::<Vec<_>>(),current.nodes.keys().collect::<Vec<_>>());
        assert_eq!(old.pins.keys().collect::<Vec<_>>(),current.pins.keys().collect::<Vec<_>>());
        for (id,node) in &old.nodes {assert_eq!(node.position,current.nodes[id].position);}
    }
    let saved=linked.to_owned();let before=project.graphs().to_vec();
    project.apply_visual(&saved,&before).unwrap();assert_eq!(project.source(),saved);
    let reopened=SourceProject::open(&saved,&[]).unwrap();
    assert!(reopened.graphs().iter().any(|graph|graph.variables.contains_key("Confiance été")));
}


#[test]
fn variable_reference_local_rename_updates_dialogue_then_runs_after_save_and_cold_open() {
    let source="// Keep local visit\nscene label start\n local scene_value = 10 // Default retained\n \"scene_value : local [scene_value], compound [scene_value + abs(-scene_value)]\"\n return\nscene label other\n local scene_value = 100\n \"Other [scene_value]\"\n return\n";
    let mut project=SourceProject::open(source,&[]).unwrap();
    let original=project.graphs().to_vec(); let mut graphs=original.clone();
    let start=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name} if name=="start")).unwrap();
    start.rename_variable("scene_value","Visite été").unwrap();
    project.apply_visual(source,&graphs).unwrap();
    let saved=project.source().to_owned();
    assert!(saved.contains("// Keep local visit") && saved.contains("// Default retained"));
    assert!(saved.ends_with("scene label other\n local scene_value = 100\n \"Other [scene_value]\"\n return\n"));
    let reopened=SourceProject::open(&saved,project.graphs()).unwrap();
    let generated=transpile_project(reopened.graphs()).unwrap();
    let mut engine=rvn_core::Engine::from_program(rvn_parser::parse_program(&generated.source).unwrap(),rvn_core::TerminalRenderer,16).unwrap();
    assert_eq!(engine.step_until_interaction().expect("Renamed local must resolve in real dialogue runtime"),
        Some(rvn_core::Interaction::Dialogue{character:None,text:"scene_value : local 10, compound 20".into()}));
    for (before,after) in original.iter().zip(reopened.graphs()) {
        assert_eq!(before.graph_id,after.graph_id); assert_eq!(before.edges,after.edges);
        assert_eq!(before.nodes.keys().collect::<Vec<_>>(),after.nodes.keys().collect::<Vec<_>>());
        assert_eq!(before.pins.keys().collect::<Vec<_>>(),after.pins.keys().collect::<Vec<_>>());
        for(id,node)in &before.nodes{assert_eq!(node.position,after.nodes[id].position);}
    }
    let again=reopened.graphs().to_vec(); project.apply_visual(&saved,&again).unwrap();
    assert_eq!(project.source(),saved);
}

#[test]
fn variable_reference_rename_updates_choice_labels_and_legacy_conditions_only_as_symbols() {
    let source="init {set scene_value=10}\nlabel start\nchoice {\n \"scene_value [scene_value + abs(-scene_value)]\" => {return}\n}\nreturn\n";
    let mut graphs=SourceProject::open(source,&[]).unwrap().graphs().to_vec();
    let start=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name} if name=="start")).unwrap();
    let choice=start.nodes.values().find(|n|n.kind==NodeKind::Choice).unwrap().id;
    start.set_property(choice,"option_0_condition",PropertyValue::String("scene_value > 0 and dict(\"scene_value\", scene_value)[\"scene_value\"] == 10".into())).unwrap();
    let edges=start.edges.clone(); let position=start.nodes[&choice].position;
    start.rename_variable("scene_value","Confiance été").unwrap();
    let Some(PropertyValue::StringList(labels))=start.nodes[&choice].properties.get("options") else{panic!("Choice labels missing");};
    assert_eq!(start.pin_by_key(choice,"option_0").unwrap().label,labels[0]);
    assert!(labels[0].starts_with("scene_value "),"Ordinary label text must remain exact");
    assert_eq!(start.edges,edges);assert_eq!(start.nodes[&choice].position,position);
    graphs.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap().rename_variable("scene_value","Confiance été").unwrap();
    let generated=transpile_project(&graphs).unwrap();
    assert!(generated.source.contains("dict(\\\"scene_value\\\"")==false,"Expression strings are escaped only by their actual owner");
    let mut engine=rvn_core::Engine::new(generated.ast,rvn_core::TerminalRenderer,16).unwrap();
    assert_eq!(engine.step_until_interaction().expect("Renamed choice references must resolve"),
        Some(rvn_core::Interaction::Choice{options:vec!["scene_value 20".into()]}));
}

#[test]
fn variable_reference_rename_refuses_an_invalid_interpolation_without_partial_mutation() {
    let mut graphs=SourceProject::open("init {set old=1}\nlabel start\n\"Valid [old]\"\nreturn\n",&[]).unwrap().graphs().to_vec();
    let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name} if name=="start")).unwrap();
    let broken=graph.add_catalog_node(NodeKind::TextValue,[123.0,456.0]).unwrap();
    graph.set_property(broken,"value",PropertyValue::String("Draft [old +".into())).unwrap();
    let before=graph.clone();
    assert!(graph.rename_variable("old","Nouvelle clé").is_err(),"An unparseable draft must not silently keep stale references");
    assert_eq!(*graph,before,"Refusal must preserve all declarations, nodes, pins, links and drafts");
}


#[test]
fn variable_reference_quoted_name_and_literal_brackets_keep_both_escaping_layers() {
    for old in ["chemin\\été", "valeur]été", "prix`été", "prix\"été"] {
        let old_source=rvn_parser::variable_binding_source_name(old).unwrap();
        let dialogue=serde_json::to_string(&format!("old [[literal]] : [{} + 1]",old_source)).unwrap();
        let source=format!("scene label start\n local {old_source}=10\n {dialogue}\n return\n");
        let mut project=SourceProject::open(&source,&[]).unwrap();let mut graphs=project.graphs().to_vec();
        let start=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
        start.rename_variable(old,"Nouvelle clé`\\]").unwrap();
        project.apply_visual(&source,&graphs).unwrap();
        let cold=SourceProject::open(project.source(),project.graphs()).unwrap();
        let output=transpile_project(cold.graphs()).unwrap();
        let mut engine=rvn_core::Engine::from_program(rvn_parser::parse_program(&output.source).unwrap(),rvn_core::TerminalRenderer,16).unwrap();
        assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"old [literal] : 11".into()}));
    }
}

#[test]
fn variable_reference_parameter_and_iterator_rename_run_without_identity_changes() {
    let source="function sum(values){local result=0 for item in values {set result=result+item} return result}\nlabel start\n\"Total [sum([1,2])]\"\nreturn\n";
    let mut graphs=SourceProject::open(source,&[]).unwrap().graphs().to_vec();
    let function=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Function{..})).unwrap();
    let original=function.clone();
    assert_eq!(function.variable_scope("values"),Some(VariableScope::Parameter));
    function.rename_variable("values","Valeurs été").unwrap();
    function.rename_variable("item","Élément été").unwrap();
    assert_eq!(function.edges,original.edges);
    assert_eq!(function.nodes.keys().collect::<Vec<_>>(),original.nodes.keys().collect::<Vec<_>>());
    assert_eq!(function.pins.keys().collect::<Vec<_>>(),original.pins.keys().collect::<Vec<_>>());
    let output=transpile_project(&graphs).unwrap();
    let mut engine=rvn_core::Engine::new(output.ast,rvn_core::TerminalRenderer,16).unwrap();
    assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"Total 3".into()}));
}

// Source-first declaration regressions, including real Engine execution.

#[test]
fn unused_declaration_is_saved_without_usage_and_initializes_real_runtime_after_cold_open() {
    for source in ["// Preserve project\nlabel start\n\"Before\"\nreturn\n",
        "// Preserve project\ninit {set existing=7 // Keep initializer\n}\nlabel start\n\"Before\"\nreturn\n"] {
        let mut project=SourceProject::open(source,&[]).unwrap();
        let mut edited=project.graphs().to_vec();
        let start=edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
        start.add_variable("Confiance été",ValueType::Bool,PropertyValue::Bool(false)).unwrap();
        let untouched_nodes=start.nodes.clone();let untouched_pins=start.pins.clone();let untouched_edges=start.edges.clone();
        project.apply_visual(source,&edited).unwrap();
        let saved=project.source().to_owned();
        let cold=SourceProject::open(&saved,project.graphs()).unwrap();
        let reopened=cold.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
        assert_eq!(reopened.variables["Confiance été"].default_value,PropertyValue::Bool(false));
        assert_eq!(reopened.variables["Confiance été"].value_type,ValueType::Bool);
        assert_eq!(reopened.nodes,untouched_nodes);assert_eq!(reopened.pins,untouched_pins);assert_eq!(reopened.edges,untouched_edges);
        assert!(saved.contains("// Preserve project"));
        if source.contains("// Keep initializer") { assert!(saved.contains("set existing=7 // Keep initializer")); }
        let mut runtime=rvn_core::Engine::from_program(rvn_parser::parse_program(&saved).unwrap(),rvn_core::TerminalRenderer,16).unwrap();
        assert!(runtime.step_until_interaction().unwrap().is_some());
        assert_eq!(runtime.state.vars.get("Confiance été"),Some(&rvn_parser::Value::Bool(false)));
        let again=project.graphs().to_vec();project.apply_visual(&saved,&again).unwrap();assert_eq!(project.source(),saved);
    }
}

#[test]
fn unused_declaration_conflicting_defaults_in_two_owners_are_refused_atomically() {
    let source="label start\n\"Before\"\nreturn\nlabel other\n\"Other\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();let before=project.graphs().to_vec();let mut edited=before.clone();
    for (name,default) in [("start",false),("other",true)] {
        edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name:actual}if actual==name)).unwrap()
            .add_variable("Confiance été",ValueType::Bool,PropertyValue::Bool(default)).unwrap();
    }
    assert!(project.apply_visual(source,&edited).is_err());
    assert_eq!(project.source(),source);assert_eq!(project.graphs(),before);
}

#[test]
fn unused_declaration_does_not_replace_an_existing_parameter_shadow() {
    let source="function echoed(shared){return shared}\nlabel start\n\"Echo [echoed(9)]\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();let mut edited=project.graphs().to_vec();
    let function=edited.iter().find(|g|matches!(g.kind,GraphKind::Function{..})).unwrap().clone();
    edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap()
        .add_variable("shared",ValueType::Bool,PropertyValue::Bool(false)).unwrap();
    project.apply_visual(source,&edited).unwrap();
    let cold=SourceProject::open(project.source(),project.graphs()).unwrap();
    let reopened=cold.graphs().iter().find(|g|g.graph_id==function.graph_id).unwrap();
    assert_eq!(reopened.variable_scope("shared"),Some(VariableScope::Parameter));
    assert_eq!(reopened.variables["shared"],function.variables["shared"]);
    assert_eq!(reopened.nodes,function.nodes);assert_eq!(reopened.pins,function.pins);assert_eq!(reopened.edges,function.edges);
    let mut runtime=rvn_core::Engine::from_program(rvn_parser::parse_program(project.source()).unwrap(),rvn_core::TerminalRenderer,16).unwrap();
    assert_eq!(runtime.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"Echo 9".into()}));
    assert_eq!(runtime.state.vars.get("shared"),Some(&rvn_parser::Value::Bool(false)));
}

#[test]
fn unused_declaration_undo_redo_after_save_removes_and_restores_canonical_data() {
    let source="// Keep\nlabel start\n\"Before\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();let mut edited=project.graphs().to_vec();
    edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap()
        .add_variable("Confiance été",ValueType::Bool,PropertyValue::Bool(false)).unwrap();
    project.apply_visual(source,&edited).unwrap();let saved=project.source().to_owned();
    let mut removed=project.graphs().to_vec();
    removed.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap().variables.remove("Confiance été");
    project.apply_visual(&saved,&removed).unwrap();let after_undo=project.source().to_owned();
    assert!(SourceProject::open(&after_undo,&[]).unwrap().graphs().iter().all(|g|!g.variables.contains_key("Confiance été")));
    let mut restored=project.graphs().to_vec();
    restored.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap()
        .add_variable("Confiance été",ValueType::Bool,PropertyValue::Bool(false)).unwrap();
    project.apply_visual(&after_undo,&restored).unwrap();
    assert!(SourceProject::open(project.source(),&[]).unwrap().graphs().iter().any(|g|g.variables.contains_key("Confiance été")));
    assert!(project.source().contains("// Keep"));
}

#[test]
fn unused_declaration_used_by_a_dialogue_cannot_be_deleted_from_one_owner() {
    let source="init {set shared=3}\nlabel start\n\"Value [shared]\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();let before=project.graphs().to_vec();let mut edited=before.clone();
    edited.iter_mut().find(|graph|matches!(graph.kind,GraphKind::Label{..})).unwrap().variables.remove("shared");
    let error=project.apply_visual(source,&edited).unwrap_err();assert!(error.contains("still referenced"),"{error}");
    assert_eq!(project.source(),source);assert_eq!(project.graphs(),before);
}

#[test]
fn unused_declaration_primitive_defaults_survive_source_cold_open_and_real_runtime() {
    for (kind,default,value) in [
        (ValueType::Bool,PropertyValue::Bool(false),rvn_parser::Value::Bool(false)),
        (ValueType::Int,PropertyValue::Int(7),rvn_parser::Value::Int(7)),
        (ValueType::Float,PropertyValue::Float(2.5),rvn_parser::Value::Float(2.5)),
        (ValueType::String,PropertyValue::String("été \\\" [literal]".into()),rvn_parser::Value::Str("été \\\" [literal]".into())),
    ] {
        let source="label start\n\"Ready\"\nreturn\n";let mut project=SourceProject::open(source,&[]).unwrap();let mut edited=project.graphs().to_vec();
        edited.iter_mut().find(|graph|matches!(graph.kind,GraphKind::Label{..})).unwrap().add_variable("Valeur été",kind.clone(),default.clone()).unwrap();
        project.apply_visual(source,&edited).unwrap();let cold=SourceProject::open(project.source(),project.graphs()).unwrap();
        let graph=cold.graphs().iter().find(|graph|matches!(graph.kind,GraphKind::Label{..})).unwrap();
        assert_eq!(graph.variables["Valeur été"].value_type,kind);assert_eq!(graph.variables["Valeur été"].default_value,default);
        let mut runtime=rvn_core::Engine::from_program(rvn_parser::parse_program(project.source()).unwrap(),rvn_core::TerminalRenderer,16).unwrap();
        runtime.step_until_interaction().unwrap();assert_eq!(runtime.state.vars.get("Valeur été"),Some(&value));
    }
}

#[test]
fn unused_declaration_addition_preserves_existing_init_nodes_and_data_pin_defaults() {
    let source="init {set existing=2+4}\nlabel start\n\"Ready [existing]\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();let mut edited=project.graphs().to_vec();let old=edited.iter().find(|graph|graph.kind==GraphKind::Init).unwrap().clone();
    edited.iter_mut().find(|graph|matches!(graph.kind,GraphKind::Label{..})).unwrap().add_variable("Unused",ValueType::Bool,PropertyValue::Bool(false)).unwrap();
    project.apply_visual(source,&edited).unwrap();let init=project.graphs().iter().find(|graph|graph.kind==GraphKind::Init).unwrap();
    for (id,node) in &old.nodes {assert_eq!(init.nodes.get(id),Some(node));}
    for (id,pin) in &old.pins {assert_eq!(init.pins.get(id),Some(pin));}
    for (id,edge) in &old.edges {let current=&init.edges[id];assert_eq!(current.input,edge.input);if old.pins[&edge.output].node!=old.nodes.values().find(|node|node.kind==NodeKind::Init).unwrap().id {assert_eq!(current,edge);}}
    let mut runtime=rvn_core::Engine::from_program(rvn_parser::parse_program(project.source()).unwrap(),rvn_core::TerminalRenderer,16).unwrap();
    assert_eq!(runtime.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"Ready 6".into()}));
}

// Dependency contract: readonly and shared fragments must not become orphaned.
fn fragment_rename_fixture(main: &str, imported: &str) -> (FragmentRenameDirectory, SourceProject) {
    let root=std::env::temp_dir().join(format!("RvN-Fragment-Rename-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&root).unwrap();std::fs::write(root.join("main.rvn"),main).unwrap();std::fs::write(root.join("helper.rvn"),imported).unwrap();
    let project=SourceProject::open_at(&root.join("main.rvn"),main,&[]).unwrap();(FragmentRenameDirectory(root),project)
}
fn fragment_rename_start(project: &SourceProject) -> Vec<GraphDocument> {
    let mut edited=project.graphs().to_vec();edited.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap().rename_variable("scene_value","Visite été").unwrap();edited
}
struct FragmentRenameDirectory(std::path::PathBuf);
impl std::ops::Deref for FragmentRenameDirectory {
    type Target=std::path::Path;
    fn deref(&self)->&Self::Target { &self.0 }
}
impl Drop for FragmentRenameDirectory {
    fn drop(&mut self) {
        let actual=self.0.canonicalize().unwrap();
        assert_eq!(actual.parent(),Some(std::env::temp_dir().canonicalize().unwrap().as_path()));
        assert!(actual.file_name().unwrap().to_string_lossy().starts_with("RvN-Fragment-Rename-"));
        std::fs::remove_dir_all(actual).unwrap();
    }
}
fn fragment_rename_cleanup(root:FragmentRenameDirectory) { drop(root); }

#[test]
fn fragment_dependency_local_rename_refuses_readonly_inherited_read_and_write_atomically() {
    let main="use \"helper.rvn\"\nscene label start\nlocal scene_value=10\ncall helper\n\"After [scene_value]\"\nreturn\n";
    let helper="label helper\nset scene_value=scene_value+1\nreturn\n";
    let(root,mut project)=fragment_rename_fixture(main,helper);let before=project.graphs().to_vec();let edited=fragment_rename_start(&project);
    assert!(project.apply_visual(main,&edited).is_err(),"Inherited reference cannot remain orphaned after rename");
    assert_eq!(project.source(),main);assert_eq!(project.graphs(),before);assert_eq!(std::fs::read_to_string(root.join("helper.rvn")).unwrap(),helper);
    fragment_rename_cleanup(root);
}
#[test]
fn fragment_dependency_local_rename_checks_transitive_calls_and_cycles() {
    let main="use \"helper.rvn\"\nscene label start\nlocal scene_value=10\ncall helper\nreturn\n";
    let helper="label helper\ncall leaf\nreturn\nlabel leaf\n\"Leaf [scene_value]\"\nif false {call helper}\nreturn\n";
    let(root,mut project)=fragment_rename_fixture(main,helper);let before=project.graphs().to_vec();let edited=fragment_rename_start(&project);
    assert!(project.apply_visual(main,&edited).is_err(),"Transitive dependencies must participate without cycling forever");
    assert_eq!(project.source(),main);assert_eq!(project.graphs(),before);fragment_rename_cleanup(root);
}
#[test]
fn fragment_dependency_unrelated_import_and_child_scene_keep_their_own_names() {
    let main="use \"helper.rvn\"\nscene label start\nlocal scene_value=10\ncall child\n\"Parent [scene_value]\"\nreturn\nscene label child\nlocal scene_value=20\nreturn\nscene label other\nlocal scene_value=30\ncall helper\nreturn\n";
    let helper="label helper\nset scene_value=scene_value+1\nreturn\n";
    let(root,mut project)=fragment_rename_fixture(main,helper);let edited=fragment_rename_start(&project);project.apply_visual(main,&edited).unwrap();
    assert!(project.source().contains("local scene_value=20"));assert!(project.source().contains("local scene_value=30"));
    let mut runtime=rvn_core::Engine::from_program(project.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
    runtime.enter_label("start").unwrap();
    assert_eq!(runtime.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"Parent 10".into()}));
    assert_eq!(std::fs::read_to_string(root.join("helper.rvn")).unwrap(),helper);fragment_rename_cleanup(root);
}
#[test]
fn fragment_dependency_local_rename_refuses_owned_shared_fragment_until_dependencies_are_migrated() {
    let main="scene label start\nlocal scene_value=10\ncall helper\nreturn\nscene label other\nlocal scene_value=20\ncall helper\nreturn\nlabel helper\nset scene_value=scene_value+1\nreturn\n";
    let mut project=SourceProject::open(main,&[]).unwrap();let before=project.graphs().to_vec();let edited=fragment_rename_start(&project);
    assert!(project.apply_visual(main,&edited).is_err(),"Shared helper cannot be blindly renamed for only one caller");
    assert_eq!(project.source(),main);assert_eq!(project.graphs(),before);
}

#[test]
fn fragment_call_order_global_helper_before_local_rename_preserves_import_and_cold_runtime() {
    for (prefix,global_dialogue) in [
        ("call helper",true),
        ("if true {call helper}",true),
        ("if false {call helper}",false),
        ("while false {call helper}",false),
    ] {
        let main=format!("use \"helper.rvn\"\ninit {{set scene_value=10}}\nscene label start\n{prefix}\nlocal scene_value=2\n\"Local [scene_value]\"\nreturn\n");
        let helper="label helper\n\"Global [scene_value]\"\nreturn\n";
        let(root,project)=fragment_rename_fixture(&main,helper);
        let mut workspace=rvn_graph::SourceWorkspace::link(&root,&root.join("main.rvn"),project.graphs()).unwrap();
        let baseline=workspace.project().graphs().to_vec();
        let edited=fragment_rename_start(workspace.project());
        workspace.save(&edited).expect("A call before Local still reads the global, not the renamed declaration");
        assert_eq!(std::fs::read_to_string(root.join("helper.rvn")).unwrap(),helper);
        let saved=std::fs::read(root.join("main.rvn")).unwrap();
        let sidecar=std::fs::read(root.join(".rvn-authoring.json")).unwrap();
        let cold=rvn_graph::SourceWorkspace::open(&root).unwrap().unwrap();
        for current in [workspace.project(),cold.project()] {
            let mut runtime=rvn_core::Engine::from_program(current.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
            runtime.enter_label("start").unwrap();
            if global_dialogue {
                assert_eq!(runtime.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"Global 10".into()}));
                runtime.advance_dialogue().unwrap();
            }
            assert_eq!(runtime.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"Local 2".into()}));
            assert_eq!(runtime.state.vars.get("scene_value"),Some(&rvn_parser::Value::Int(10)));
            for old in baseline.iter().filter(|graph|!matches!(&graph.kind,GraphKind::Label{name}if name=="start")) {
                assert_eq!(current.graphs().iter().find(|graph|graph.graph_id==old.graph_id),Some(old));
            }
        }
        assert_eq!(std::fs::read(root.join("main.rvn")).unwrap(),saved);
        assert_eq!(std::fs::read(root.join(".rvn-authoring.json")).unwrap(),sidecar);
        assert_eq!(std::fs::read_to_string(root.join("helper.rvn")).unwrap(),helper);
        fragment_rename_cleanup(root);
    }
}

#[test]
fn fragment_call_order_same_target_after_local_still_refuses_atomically() {
    for body in [
        "call helper\nlocal scene_value=2\ncall helper",
        "call helper\nlocal scene_value=2\njump helper",
        "if true {local scene_value=2}\ncall helper",
        "while true {call helper\nlocal scene_value=2}",
    ] {
    let tail=if body.ends_with("jump helper") { "" } else { "return\n" };
    let main=format!("use \"helper.rvn\"\ninit {{set scene_value=10}}\nscene label start\n{body}\n{tail}");
    let helper="label helper\n\"Value [scene_value]\"\nreturn\n";
    let(root,mut project)=fragment_rename_fixture(&main,helper);
    let baseline=project.graphs().to_vec();let edited=fragment_rename_start(&project);
    let error=project.apply_visual(&main,&edited).unwrap_err();
    assert!(error.contains("caller's binding"),"{error}");
    assert_eq!(project.source(),main);assert_eq!(project.graphs(),baseline);
    assert_eq!(std::fs::read_to_string(root.join("main.rvn")).unwrap(),main);
    assert_eq!(std::fs::read_to_string(root.join("helper.rvn")).unwrap(),helper);
    fragment_rename_cleanup(root);
    }
}


#[test]
fn fragment_call_order_helper_introducing_inherited_slot_before_local_refuses_atomically() {
    let main="use \"helper.rvn\"\ninit {set scene_value=10}\nscene label start\ncall helper\nlocal scene_value=2\n\"Local [scene_value]\"\nreturn\n";
    let helper="label helper\n\"Before [scene_value]\"\nlocal scene_value=30\n\"Helper [scene_value]\"\nreturn\n";
    let(root,mut project)=fragment_rename_fixture(main,helper);
    let baseline=project.graphs().to_vec();
    let run=|project:&SourceProject| {
        let mut runtime=rvn_core::Engine::from_program(project.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
        runtime.enter_label("start").unwrap();
        for text in ["Before 10","Helper 30","Local 2"] {
            assert_eq!(runtime.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:text.into()}));
            if text!="Local 2" { runtime.advance_dialogue().unwrap(); }
        }
        assert_eq!(runtime.state.vars.get("scene_value"),Some(&rvn_parser::Value::Int(10)));
        runtime.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals.clone()
    };
    assert_eq!(run(&project),std::collections::HashMap::from([("scene_value".into(),rvn_parser::Value::Int(2))]));
    let edited=fragment_rename_start(&project);
    if project.apply_visual(main,&edited).is_ok() {
        panic!("A callee-introduced slot was not migrated: two live slots after rename {:?}",run(&project));
    }
    assert_eq!(project.source(),main);assert_eq!(project.graphs(),baseline);
    assert_eq!(std::fs::read_to_string(root.join("main.rvn")).unwrap(),main);
    assert_eq!(std::fs::read_to_string(root.join("helper.rvn")).unwrap(),helper);
    fragment_rename_cleanup(root);
}

#[test]
fn fragment_call_order_transitive_shared_leaf_keeps_both_incoming_binding_states() {
    let main="use \"helper.rvn\"\ninit {set scene_value=10}\nscene label start\ncall z_before\nlocal scene_value=2\ncall a_after\nreturn\n";
    let helper="label z_before\ncall leaf\nreturn\nlabel a_after\ncall leaf\nreturn\nlabel leaf\n\"Value [scene_value]\"\nreturn\n";
    let(root,mut project)=fragment_rename_fixture(main,helper);
    let baseline=project.graphs().to_vec();let edited=fragment_rename_start(&project);
    assert!(project.apply_visual(main,&edited).is_err(),"The shared leaf is reached before and after Local; a global visit must not suppress the local dependency");
    assert_eq!(project.source(),main);assert_eq!(project.graphs(),baseline);
    assert_eq!(std::fs::read_to_string(root.join("main.rvn")).unwrap(),main);
    assert_eq!(std::fs::read_to_string(root.join("helper.rvn")).unwrap(),helper);
    fragment_rename_cleanup(root);
}

// Binding-order and inherited-fragment dependency regressions.
#[test]
fn shadow_dependency_declaration_removal_checks_readonly_initializer_global_before_local() {
    let main="use \"helper.rvn\"\ninit {set score=10}\nscene label start\ncall helper\nreturn\n";
    let helper="label helper\nlocal score=score+1\n\"Helper [score]\"\nreturn\n";
    let(root,mut project)=fragment_rename_fixture(main,helper);
    let mut runtime=rvn_core::Engine::from_program(project.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
    runtime.enter_label("start").unwrap();
    assert_eq!(runtime.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"Helper 11".into()}));
    let before=project.graphs().to_vec();let mut edited=before.clone();
    edited.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap().variables.remove("score");
    assert!(project.apply_visual(main,&edited).is_err(),"A local initializer can depend on the global before introducing its local binding");
    assert_eq!(project.source(),main);assert_eq!(project.graphs(),before);
    assert_eq!(std::fs::read_to_string(root.join("helper.rvn")).unwrap(),helper);
    fragment_rename_cleanup(root);
}

#[test]
fn shadow_dependency_local_fragment_binding_outlives_its_return() {
    let main="scene label start\nlocal scene_value=10\ncall helper\n\"After [scene_value]\"\nreturn\nlabel helper\nlocal scene_value=30\nreturn\n";
    let mut project=SourceProject::open(main,&[]).unwrap();
    let mut runtime=rvn_core::Engine::from_program(project.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
    assert_eq!(runtime.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"After 30".into()}));
    let before=project.graphs().to_vec();let edited=fragment_rename_start(&project);
    assert!(project.apply_visual(main,&edited).is_err(),"Renaming only the caller would change the observable value from30 to10");
    assert_eq!(project.source(),main);assert_eq!(project.graphs(),before);
}


#[test]
fn shadow_dependency_optional_branch_loop_and_condition_reads_keep_the_global() {
    for (body,expected) in [
        ("if false {local score=2}\n\"After [score]\"", "After 10"),
        ("while false {local score=2}\n\"After [score]\"", "After 10"),
        ("if score==10 {local score=2} else {local score=3}\n\"After [score]\"", "After 2"),
    ] {
        let source=format!("init {{set score=10}}\nscene label start\n{body}\nreturn\n");
        let mut project=SourceProject::open(&source,&[]).unwrap();let before=project.graphs().to_vec();
        let mut runtime=rvn_core::Engine::from_program(project.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
        assert_eq!(runtime.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:expected.into()}));
        let mut edited=before.clone();edited.iter_mut().find(|graph|graph.kind==GraphKind::Init).unwrap().variables.remove("score");
        assert!(project.apply_visual(&source,&edited).is_err(),"Global reads before a definite local binding must participate: {body}");
        assert_eq!(project.source(),source);assert_eq!(project.graphs(),before);
    }
}

#[test]
fn shadow_dependency_definite_local_and_lexical_set_allow_unrelated_global_removal() {
    for source in [
        "init {set score=10}\nscene label start\nlocal score=2\n\"After [score]\"\nreturn\n",
        "init {set score=10}\nscene label start\nif true {local score=2} else {local score=3}\n\"After [score]\"\nreturn\n",
        "init {set score=10}\nfunction calc(){set score=2 return score}\nscene label start\n\"After [calc()]\"\nreturn\n",
    ] {
        let mut project=SourceProject::open(source,&[]).unwrap();let mut edited=project.graphs().to_vec();
        edited.iter_mut().find(|graph|graph.kind==GraphKind::Init).unwrap().variables.remove("score");
        project.apply_visual(source,&edited).unwrap();
        let cold=SourceProject::open(project.source(),project.graphs()).unwrap();
        let mut runtime=rvn_core::Engine::from_program(cold.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
        assert_eq!(runtime.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"After 2".into()}));
        assert!(!runtime.state.vars.contains_key("score"));
    }
}

fn binding_rename_runtime(project:&SourceProject,expected:&[&str]) {
    let mut engine=rvn_core::Engine::from_program(project.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
    engine.enter_label("start").unwrap();
    for text in expected {
        assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:(*text).into()}));
        engine.advance_dialogue().unwrap();
    }
    assert_eq!(engine.state.vars.get("score"),Some(&rvn_parser::Value::Int(10)));
}

#[test]
fn binding_rename_local_initializer_preserves_its_global_read_and_cold_runtime() {
    let source="// Keep global owner\ninit {set score=10}\nscene label start\nlocal score=score+1 // Read global before local\n\"Local [score]\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();
    binding_rename_runtime(&project,&["Local 11"]);
    let before=project.graphs().to_vec();let mut edited=before.clone();
    let start=edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    start.rename_variable("score","Score local été").unwrap();
    project.apply_visual(source,&edited).unwrap();
    binding_rename_runtime(&project,&["Local 11"]);
    let cold=SourceProject::open(project.source(),project.graphs()).unwrap();
    binding_rename_runtime(&cold,&["Local 11"]);
    assert!(project.source().contains("// Keep global owner")&&project.source().contains("// Read global before local"));
    let init=before.iter().find(|g|g.kind==GraphKind::Init).unwrap();
    assert_eq!(project.graphs().iter().find(|g|g.kind==GraphKind::Init).unwrap(),init,"The global owner and its default must remain exact");
    let old=before.iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let after=project.graphs().iter().find(|g|g.graph_id==old.graph_id).unwrap();
    assert_eq!(old.edges,after.edges);assert_eq!(old.nodes.keys().collect::<Vec<_>>(),after.nodes.keys().collect::<Vec<_>>());
    assert_eq!(old.pins.keys().collect::<Vec<_>>(),after.pins.keys().collect::<Vec<_>>());
    assert_eq!(after.variable_scope("score"),Some(VariableScope::Global));
    assert_eq!(after.variable_scope("Score local été"),Some(VariableScope::Local));
    for node in old.nodes.values().filter(|n|n.kind==NodeKind::VariableGet) {assert_eq!(after.nodes.get(&node.id),Some(node),"Initializer global Get must keep its real identity/name");}
}

#[test]
fn binding_rename_shared_text_producer_keeps_global_and_local_consumers_distinct() {
    let source="init {set score=10}\nscene label start\n\"Value [score]\"\nlocal score=score+1\n\"Value [score]\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();let mut baseline=project.graphs().to_vec();
    let graph=baseline.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let dialogues:Vec<_>=graph.nodes.values().filter(|n|n.kind==NodeKind::Dialogue).map(|n|n.id).collect();assert_eq!(dialogues.len(),2);
    let first=graph.pin_by_key(dialogues[0],"text").unwrap().id;let last=graph.pin_by_key(dialogues[1],"text").unwrap().id;
    let output=graph.edges.values().find(|e|e.input==first).unwrap().output;
    graph.edges.values_mut().find(|e|e.input==last).unwrap().output=output;
    let producer=graph.pins[&output].node;let producer_before=graph.nodes[&producer].clone();
    project.apply_visual(source,&baseline).unwrap();
    binding_rename_runtime(&project,&["Value 10","Value 11"]);
    let current=project.source().to_owned();let mut edited=project.graphs().to_vec();
    let graph=edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();let before=graph.clone();
    graph.rename_variable("score","Score local été").unwrap();
    project.apply_visual(&current,&edited).unwrap();
    binding_rename_runtime(&project,&["Value 10","Value 11"]);
    let cold=SourceProject::open(project.source(),project.graphs()).unwrap();binding_rename_runtime(&cold,&["Value 10","Value 11"]);
    let after=project.graphs().iter().find(|g|g.graph_id==before.graph_id).unwrap();
    assert_eq!(after.nodes.get(&producer),Some(&producer_before),"Keep the original global template and its identity");
    assert_eq!(after.edges.values().find(|e|e.input==first),before.edges.values().find(|e|e.input==first));
    assert_ne!(after.edges.values().find(|e|e.input==last).unwrap().output,output,"Only the local consumer receives the renamed producer");
    for edge in before.edges.values().filter(|e|e.input!=last){assert_eq!(after.edges.get(&edge.id),Some(edge));}
    for node in before.nodes.values(){assert_eq!(after.nodes[&node.id].position,node.position);}
}

#[test]
fn binding_rename_shared_get_keeps_initializer_and_later_value_reads_distinct() {
    let source="init {set score=10}\nscene label start\nlocal score=score+1\n\"Local [score]\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();let mut baseline=project.graphs().to_vec();
    let graph=baseline.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let get=graph.nodes.values().find(|n|n.kind==NodeKind::VariableGet&&n.properties.get("name")==Some(&PropertyValue::String("score".into()))).unwrap().id;
    let get_before=graph.nodes[&get].clone();let output=graph.pin_by_key(get,"value").unwrap().id;
    let dialogue=graph.nodes.values().find(|n|n.kind==NodeKind::Dialogue).unwrap().id;let input=graph.pin_by_key(dialogue,"text").unwrap().id;
    let old=graph.edges.values().find(|e|e.input==input).unwrap().id;graph.remove_edge(old);
    graph.connect_with_conversions(output,input,[640.0,120.0]).unwrap();
    project.apply_visual(source,&baseline).unwrap();binding_rename_runtime(&project,&["11"]);
    let current=project.source().to_owned();let mut edited=project.graphs().to_vec();
    edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap().rename_variable("score","Score local été").unwrap();
    project.apply_visual(&current,&edited).unwrap();binding_rename_runtime(&project,&["11"]);
    let cold=SourceProject::open(project.source(),project.graphs()).unwrap();binding_rename_runtime(&cold,&["11"]);
    let after=project.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    assert_eq!(after.nodes.get(&get),Some(&get_before),"Keep original global Get instead of rewriting the initializer");
    assert!(after.nodes.values().any(|n|n.kind==NodeKind::VariableGet&&n.properties.get("name")==Some(&PropertyValue::String("Score local été".into()))));
}

#[test]
fn binding_rename_definite_branches_preserve_global_initializers_on_both_routes() {
    for (condition,expected) in [("true","Value 11"),("false","Value 12")] {
        let source=format!("init {{set score=10}}\nscene label start\nif {condition} {{local score=score+1}} else {{local score=score+2}}\n\"Value [score]\"\nreturn\n");
        let mut project=SourceProject::open(&source,&[]).unwrap();binding_rename_runtime(&project,&[expected]);
        let before=project.graphs().to_vec();let mut graphs=before.clone();
        let start=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
        start.rename_variable("score","Score local été").unwrap();
        project.apply_visual(&source,&graphs).unwrap();binding_rename_runtime(&project,&[expected]);
        let cold=SourceProject::open(project.source(),project.graphs()).unwrap();binding_rename_runtime(&cold,&[expected]);
        let old=before.iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
        let after=project.graphs().iter().find(|g|g.graph_id==old.graph_id).unwrap();
        assert_eq!(old.edges,after.edges);
        for node in old.nodes.values().filter(|n|n.kind==NodeKind::VariableGet){assert_eq!(after.nodes.get(&node.id),Some(node));}
        assert_eq!(after.variables["score"].default_value,PropertyValue::Int(10));
    }
}

#[test]
fn binding_rename_ambiguous_branch_and_loop_refuse_atomically() {
    for body in [
        "if false {local score=score+1}\n\"Value [score]\"",
        "while false {local score=score+1}\n\"Value [score]\"",
    ] {
        let source=format!("init {{set score=10}}\nscene label start\n{body}\nreturn\n");
        let project=SourceProject::open(&source,&[]).unwrap();binding_rename_runtime(&project,&["Value 10"]);
        let mut graph=project.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap().clone();
        let before=graph.clone();let before_bytes=graph.to_pretty_json().unwrap();
        let error=graph.rename_variable("score","Score local été").unwrap_err();
        assert!(error.to_string().contains("distinct bindings"),"{error}");
        assert_eq!(graph,before);assert_eq!(graph.to_pretty_json().unwrap(),before_bytes);
        assert_eq!(project.source(),source);
    }
}

#[test]
fn binding_rename_execution_value_output_reads_local_without_cloning_global_assignment() {
    let source="init {set score=10}\nscene label start\nset score=score\nlocal score=score+1\n\"Value [score]\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();let mut graphs=project.graphs().to_vec();
    let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let set=graph.nodes.values().find(|n|n.kind==NodeKind::SetVariable).unwrap().id;
    let output=graph.pin_by_key(set,"value_out").unwrap().id;
    let dialogue=graph.nodes.values().find(|n|n.kind==NodeKind::Dialogue).unwrap().id;
    let input=graph.pin_by_key(dialogue,"text").unwrap().id;
    graph.remove_edge(graph.edges.values().find(|e|e.input==input).unwrap().id);
    graph.connect_with_conversions(output,input,[640.0,120.0]).unwrap();
    project.apply_visual(source,&graphs).unwrap();binding_rename_runtime(&project,&["11"]);
    let current=project.source().to_owned();let mut edited=project.graphs().to_vec();
    let graph=edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();let before=graph.clone();
    graph.rename_variable("score","Score local été").unwrap();
    project.apply_visual(&current,&edited).unwrap();binding_rename_runtime(&project,&["11"]);
    let cold=SourceProject::open(project.source(),project.graphs()).unwrap();binding_rename_runtime(&cold,&["11"]);
    let after=project.graphs().iter().find(|g|g.graph_id==before.graph_id).unwrap();
    assert_eq!(after.nodes.get(&set),before.nodes.get(&set));
    assert_eq!(after.nodes.values().filter(|n|n.kind==NodeKind::SetVariable).count(),1);
    for edge in before.edges.values().filter(|e|graph_pin_is_execution(&before,e.input)) {assert_eq!(after.edges.get(&edge.id),Some(edge));}
}

fn graph_pin_is_execution(graph:&GraphDocument,pin:PinId)->bool { graph.pins[&pin].value_type.is_execution() }


#[test]
fn asset_defaults1021_real_source_save_cold_reopen_retains_fallback_but_wire_controls_rvn() {
    let source = "label start\nsfx.play(\"fallback.wav\")\n";
    let (root, _) = fragment_rename_fixture(source, "");
    let mut workspace = SourceWorkspace::link(&root, &root.join("main.rvn"), &[]).unwrap();
    let mut edited = workspace.project().graphs().to_vec();
    let graph = edited.iter_mut().find(|g| matches!(&g.kind, GraphKind::Label { name } if name == "start")).unwrap();
    let effect = graph.nodes.values().find(|n| n.kind == NodeKind::SfxPlay).unwrap().id;
    graph.set_pin_default(effect, "file", PropertyValue::String("fallback.wav".into())).unwrap();
    let input = graph.pin_by_key(effect, "file").unwrap().id;
    let prior_edge = graph.edges.values().find(|edge| edge.input == input).unwrap().clone();
    let output = prior_edge.output;
    let producer = graph.pins[&output].node;
    assert_eq!(graph.nodes[&producer].kind, NodeKind::SoundEffectAsset);
    graph.remove_edge(prior_edge.id);
    graph.set_property(producer, "path", PropertyValue::String("wired.wav".into())).unwrap();
    let edge = graph.connect(output, input).unwrap();
    let graph_id = graph.graph_id;
    workspace.save(&edited).unwrap();
    let saved = workspace.project().graphs().iter().find(|g| g.graph_id == graph_id).unwrap().clone();
    assert_eq!(saved.pins[&input].default_value, Some(PropertyValue::String("fallback.wav".into())));
    let actual_source = std::fs::read_to_string(root.join("main.rvn")).unwrap();
    let actual = SourceProject::open(actual_source.clone(), &[]).unwrap();
    let actual_program = rvn_parser::parse_program(&actual_source).unwrap();
    assert!(actual_program.script.iter().any(|statement| matches!(statement,
        rvn_parser::Statement::SfxPlay { file, .. } if file == "wired.wav")));
    assert!(actual.graphs().iter().flat_map(|graph| graph.nodes.values()).any(|node|
        node.kind == NodeKind::SoundEffectAsset && node.properties.get("path") == Some(&PropertyValue::String("wired.wav".into()))),
        "plain import materializes the actual Asset selection, not a hidden fallback");
    drop(workspace);
    let cold = SourceWorkspace::open(&root).unwrap().unwrap();
    let reopened = cold.project().graphs().iter().find(|g| g.graph_id == graph_id).unwrap();
    assert_eq!(reopened, &saved, "the source sidecar preserves real authoring identities and the hidden selection");
    let mut disconnected = reopened.clone();
    disconnected.remove_edge(edge);
    let exported = transpile(&disconnected).unwrap();
    assert!(exported.ast.iter().any(|statement| matches!(statement,
        rvn_parser::Statement::SfxPlay { file, .. } if file == "fallback.wav")));
    assert_eq!(std::fs::read_to_string(root.join("main.rvn")).unwrap(), actual_source);
}


fn asset_defaults1021_native_ab() -> GraphDocument {
    let project = SourceProject::open("label start\nsfx.play(\"B.wav\")\n\"after\"\n", &[]).unwrap();
    let mut graph = project.graphs().iter().find(|g| matches!(&g.kind, GraphKind::Label { name } if name == "start")).unwrap().clone();
    let effect = graph.nodes.values().find(|n| n.kind == NodeKind::SfxPlay).unwrap().id;
    let input = graph.pin_by_key(effect, "file").unwrap().id;
    assert!(graph.edges.values().any(|edge| edge.input == input));
    graph.set_pin_default(effect, "file", PropertyValue::String("A.wav".into())).unwrap();
    graph.enable_native_frame_preparation().unwrap();
    graph
}

#[test]
fn asset_defaults1021_native_source_alone_has_no_a_but_exact_sidecar_restores_a_and_all_identities() {
    let graph = asset_defaults1021_native_ab();
    let before = graph.clone();
    let input = graph.pins.values().find(|p| p.key == "file").unwrap().id;
    let compiled = prepare_native_frame_graph(&graph).unwrap();
    assert_eq!(graph, before);
    let actual = import_native_frame_script(&compiled.ast).unwrap();
    let actual = actual.iter().find(|g| g.graph_id == graph.graph_id).unwrap();
    assert_eq!(actual.edges, graph.edges);
    assert!(actual.nodes.keys().eq(graph.nodes.keys()));
    assert!(actual.pins.keys().eq(graph.pins.keys()));
    assert_eq!(actual.pins[&input].default_value, None, "NativeIdentity intentionally encodes no authoring fallback A");
    let restored = reimport_native_frame_script(&compiled.ast, &[graph.clone()]).unwrap();
    let restored = restored.iter().find(|g| g.graph_id == graph.graph_id).unwrap();
    assert_eq!(restored, &graph, "Graph + source + exact existing sidecar roundtrips all authored state");
    assert_eq!(prepare_native_frame_graph(restored).unwrap().ast, compiled.ast);
    let (root, _) = fragment_rename_fixture("label start\nsfx.play(\"B.wav\")\n", "");
    std::fs::write(root.join("main.rvn"), &compiled.source).unwrap();
    let workspace = SourceWorkspace::link_native_preparation(&root, &root.join("main.rvn"), &[graph.clone()]).unwrap();
    drop(workspace);
    let cold = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert_eq!(cold.project().graphs().iter().find(|g| g.graph_id == graph.graph_id), Some(&graph));
}

#[test]
fn asset_defaults1021_native_old_presentation_does_not_override_current_source_wire_or_type() {
    let graph = asset_defaults1021_native_ab();
    let compiled = prepare_native_frame_graph(&graph).unwrap();
    let input = graph.pins.values().find(|p| p.key == "file").unwrap().id;
    for mode in 0..2 {
        let mut stale = graph.clone();
        if mode == 0 {
            let edge = stale.edges.values().find(|e| e.input == input).unwrap().id;
            stale.remove_edge(edge);
            let other = stale.add_catalog_node(NodeKind::SoundEffectAsset, [-400.0, 0.0]).unwrap();
            stale.set_property(other, "path", PropertyValue::String("unbacked.wav".into())).unwrap();
            let output = stale.pin_by_key(other, "value").unwrap().id;
            stale.connect(output, input).unwrap();
        } else {
            stale.pins.get_mut(&input).unwrap().value_type = ValueType::Asset(AssetKind::Music);
        }
        let before = stale.clone();
        let restored = reimport_native_frame_script(&compiled.ast, &[stale.clone()]).unwrap();
        let restored = restored.iter().find(|g| g.graph_id == graph.graph_id).unwrap();
        assert_eq!(restored.edges, graph.edges, "current declared source, not the old sidecar, owns wiring");
        assert_eq!(restored.pins[&input].value_type, graph.pins[&input].value_type);
        assert_eq!(restored.pins[&input].default_value,
            if mode == 0 { Some(PropertyValue::String("A.wav".into())) } else { None },
            "retain the compatible slot only; a foreign typed slot grants no fallback");
        assert_eq!(stale, before);
        assert_eq!(prepare_native_frame_graph(restored).unwrap().ast, compiled.ast);
    }
}

#[test]
fn asset_defaults1021_legacy_external_changed_scope_retains_unique_hidden_selection_and_new_logic() {
    let project = SourceProject::open("label start\nsfx.play(\"B.wav\")\n", &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let graph = graphs.iter_mut().find(|g| matches!(&g.kind, GraphKind::Label { name } if name == "start")).unwrap();
    let effect = graph.nodes.values().find(|n| n.kind == NodeKind::SfxPlay).unwrap().id;
    let input = graph.pin_by_key(effect, "file").unwrap().id;
    let graph_id = graph.graph_id;
    graph.set_pin_default(effect, "file", PropertyValue::String("A.wav".into())).unwrap();
    let before = graphs.clone();
    let mut project = SourceProject::open(project.source(), &graphs).unwrap();
    let source = "label start\nsfx.play(\"B.wav\")\n\"External change\"\n";
    project.refresh(source).unwrap();
    assert_eq!(project.source(), source);
    let refreshed = project.graphs().iter().find(|g| g.graph_id == graph_id).unwrap();
    assert_eq!(refreshed.pins[&input].default_value, Some(PropertyValue::String("A.wav".into())));
    assert!(rvn_parser::parse(project.source()).unwrap().iter().any(|statement| matches!(statement,
        rvn_parser::Statement::Dialogue { .. })), "the new source statement is admitted");
    asset_defaults1021_assert_runtime_sfx(project.source(), &["sfx/B.wav"]);
    assert_eq!(graphs, before, "refresh never mutates the supplied previous presentation");
}


// Observe actual Legacy callbacks or coherent Native Scene publications.
// This is a logical batch/token recorder, not a PCM reader or UI backend;
// it grants no feedback, checkpoint, owner reconstruction or choice response.
#[derive(Default)]
struct AssetDefaultsAudioProbe {
    played: Vec<String>,
    native_started: Vec<rvn_core::audio_playback::AudioPlaybackView>,
    batches: Vec<rvn_core::renderer::SceneRenderBatch>,
}
impl rvn_core::Renderer for AssetDefaultsAudioProbe {
    fn validate_scene_batch(&self, batch: &rvn_core::renderer::SceneRenderBatch) -> Result<(), String> {
        if !batch.interfaces.is_empty() || !batch.motions.is_empty() || !batch.layered.is_empty()
            || !batch.videos.is_empty() || !batch.effects.is_empty() || !batch.menu_requests.is_empty()
            || !batch.speech_requests.is_empty() || !batch.retired_widget_speech.is_empty()
            || batch.state.accessibility != Default::default()
            || !rvn_core::audio_checkpoint::music_views(&batch.state, batch.runtime_epoch)?.is_empty()
        { return Err("Asset Sfx recorder has no other scene resource backend".into()); }
        let audio=rvn_core::audio_playback::describe(&batch.state,batch.runtime_epoch)?;
        for view in audio {
            view.playback.validate()?;
            if view.playback.family!=rvn_core::audio_playback::AudioFamily::Sfx
                || view.playback.envelope.is_some() || view.playback.stopping || view.playback.checkpoint.is_some()
                || view.parked || view.token.target.kind!=rvn_core::scene_resources::TargetKind::SfxPlayback
                || matches!(&view.token.task.key.owner,rvn_core::scene_tasks::TaskOwner::Host{..})
            { return Err("Asset Sfx recorder requires a genuine current unfaded Story Sfx token".into()); }
        }
        Ok(())
    }
    fn publish_scene_batch(&mut self, batch: rvn_core::renderer::SceneRenderBatch) {
        let audio=rvn_core::audio_playback::describe(&batch.state,batch.runtime_epoch)
            .expect("the identical atomic publication was preflighted");
        for view in audio {
            // A later dialogue redraw may retain the same actual playback.
            // Count a publication once by its complete real token, never path.
            if !self.native_started.iter().any(|prior|prior.token==view.token) {
                self.played.push(view.playback.file.clone());self.native_started.push(view);
            }
        }
        self.batches.push(batch);
    }
    fn set_background(&mut self, _: &str, _: &rvn_parser::Transition) {}
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &rvn_parser::Position,
        _: &rvn_parser::Transition, _: Option<&rvn_core::SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &rvn_parser::Transition, _: &rvn_core::SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &rvn_parser::Position,
        _: &rvn_parser::Transition, _: &rvn_core::SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize { panic!("not a fixture choice answer") }
    fn music_play(&mut self, _: &str, _: &rvn_parser::Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &rvn_parser::Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, file: &str, _: &rvn_parser::Transition) { self.played.push(file.into()); }
    fn sfx_stop(&mut self, _: &str, _: &rvn_parser::Transition) {}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[rvn_parser::Hotspot]) -> usize {
        panic!("not a fixture imagemap answer")
    }
}
fn asset_defaults1021_assert_runtime_sfx(source: &str, expected: &[&str]) {
    let program = rvn_parser::parse_program(source).unwrap();
    let mut engine = rvn_core::Engine::from_program(program, AssetDefaultsAudioProbe::default(), 8).unwrap();
    for _ in 0..16 {
        match engine.step_until_interaction().unwrap() {
            None => {
                assert_eq!(engine.renderer.played, expected.iter().map(|s| s.to_string()).collect::<Vec<_>>());
                return;
            }
            Some(rvn_core::Interaction::Dialogue { .. }) => engine.advance_dialogue().unwrap(),
            other => panic!("unexpected actual Engine interaction: {other:?}"),
        }
    }
    panic!("bounded fixture did not finish");
}
fn asset_defaults1021_rewire(graph: &mut GraphDocument, path: &str) -> (NodeId, PinId, EdgeId) {
    let effect = graph.nodes.values().find(|n| n.kind == NodeKind::SfxPlay).unwrap().id;
    let input = graph.pin_by_key(effect, "file").unwrap().id;
    let edge = graph.edges.values().find(|e| e.input == input).unwrap().clone();
    let old_producer = graph.pins[&edge.output].node;
    assert_eq!(graph.nodes[&old_producer].kind, NodeKind::SoundEffectAsset);
    assert_eq!(graph.edges.values().filter(|e| e.output == edge.output).count(), 1);
    graph.remove_node(old_producer).unwrap();
    let producer = graph.add_catalog_node(NodeKind::SoundEffectAsset, [-400.0, 0.0]).unwrap();
    graph.set_property(producer, "path", PropertyValue::String(path.into())).unwrap();
    let output = graph.pin_by_key(producer, "value").unwrap().id;
    let edge = graph.connect(output, input).unwrap();
    (effect, input, edge)
}
fn asset_defaults1021_delete_effect(graph: &mut GraphDocument, effect: NodeId) {
    let exec_in = graph.pin_by_key(effect, "exec_in").unwrap().id;
    let exec_out = graph.pin_by_key(effect, "exec_out").unwrap().id;
    let incoming = graph.edges.values().find(|e| e.input == exec_in).unwrap().output;
    let outgoing = graph.edges.values().find(|e| e.output == exec_out).unwrap().input;
    let producers = graph.edges.values().filter(|e| graph.pins[&e.input].node == effect
        && !graph.pins[&e.input].value_type.is_execution())
        .map(|e| graph.pins[&e.output].node).collect::<std::collections::BTreeSet<_>>();
    graph.remove_node(effect).unwrap();
    graph.connect(incoming, outgoing).unwrap();
    for producer in producers {
        if !graph.edges.values().any(|e| graph.pins[&e.output].node == producer) {
            graph.remove_node(producer).unwrap();
        }
    }
}

#[test]
fn asset_defaults1021_native_explicit_rewire_delete_and_unconnected_value_follow_actual_source() {
    let original = asset_defaults1021_native_ab();
    let original_before = original.clone();
    asset_defaults1021_assert_runtime_sfx(&prepare_native_frame_graph(&original).unwrap().source, &["sfx/B.wav"]);
    let mut changed = original.clone();
    let (effect, input, edge) = asset_defaults1021_rewire(&mut changed, "C.wav");
    let compiled = prepare_native_frame_graph(&changed).unwrap();
    let restored = reimport_native_frame_script(&compiled.ast, &[original.clone()]).unwrap();
    let restored = restored.iter().find(|g| g.graph_id == original.graph_id).unwrap();
    assert_eq!(restored.edges, changed.edges, "source owns the new actual edge");
    assert!(restored.nodes.keys().eq(changed.nodes.keys()));
    assert!(restored.pins.keys().eq(changed.pins.keys()));
    assert_eq!(restored.nodes[&effect].position, original.nodes[&effect].position);
    assert_eq!(prepare_native_frame_graph(restored).unwrap().ast, compiled.ast);
    assert_eq!(restored.pins[&input].default_value, Some(PropertyValue::String("A.wav".into())));
    assert!(restored.edges.contains_key(&edge));
    asset_defaults1021_assert_runtime_sfx(&compiled.source, &["sfx/C.wav"]);
    let mut disconnected = restored.clone();
    let producer = disconnected.pins[&disconnected.edges[&edge].output].node;
    disconnected.remove_node(producer).unwrap();
    asset_defaults1021_assert_runtime_sfx(&prepare_native_frame_graph(&disconnected).unwrap().source, &["sfx/A.wav"]);
    disconnected.set_pin_default(effect, "file", PropertyValue::String("D.wav".into())).unwrap();
    let explicit = prepare_native_frame_graph(&disconnected).unwrap();
    let explicit_reimport = reimport_native_frame_script(&explicit.ast, &[restored.clone()]).unwrap();
    let explicit_reimport = explicit_reimport.iter().find(|g| g.graph_id == original.graph_id).unwrap();
    assert_eq!(explicit_reimport.pins[&input].default_value, Some(PropertyValue::String("D.wav".into())));
    assert!(!explicit_reimport.edges.values().any(|e| e.input == input));
    asset_defaults1021_assert_runtime_sfx(&explicit.source, &["sfx/D.wav"]);
    let (root, _) = fragment_rename_fixture("label start\n\"after\"\n", "");
    std::fs::write(root.join("main.rvn"), &compiled.source).unwrap();
    let workspace = SourceWorkspace::link_native_preparation(&root, &root.join("main.rvn"), &[restored.clone()]).unwrap();
    drop(workspace);
    let cold = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert_eq!(cold.project().graphs().iter().find(|g| g.graph_id == changed.graph_id), Some(restored));
    let mut deleted = changed.clone();
    asset_defaults1021_delete_effect(&mut deleted, effect);
    let deleted_source = prepare_native_frame_graph(&deleted).unwrap();
    let deleted_reimport = reimport_native_frame_script(&deleted_source.ast, &[changed.clone()]).unwrap();
    let deleted_reimport = deleted_reimport.iter().find(|g| g.graph_id == changed.graph_id).unwrap();
    assert!(!deleted_reimport.nodes.contains_key(&effect));
    assert!(!deleted_reimport.pins.contains_key(&input));
    assert_eq!(deleted_reimport.edges, deleted.edges);
    asset_defaults1021_assert_runtime_sfx(&deleted_source.source, &[]);
    assert_eq!(original, original_before);
    assert_eq!(std::fs::read_to_string(root.join("main.rvn")).unwrap(), compiled.source);
}

#[test]
fn asset_defaults1021_visual_rewire_and_delete_save_cold_exactly_without_old_topology_veto() {
    let (root, _) = fragment_rename_fixture("label start\nsfx.play(\"B.wav\")\n\"after\"\n", "");
    let mut workspace = SourceWorkspace::link(&root, &root.join("main.rvn"), &[]).unwrap();
    let mut edited = workspace.project().graphs().to_vec();
    let graph = edited.iter_mut().find(|g| matches!(&g.kind, GraphKind::Label { name } if name == "start")).unwrap();
    let effect = graph.nodes.values().find(|n| n.kind == NodeKind::SfxPlay).unwrap().id;
    graph.set_pin_default(effect, "file", PropertyValue::String("A.wav".into())).unwrap();
    let graph_id = graph.graph_id;
    workspace.save(&edited).unwrap();
    let graph = edited.iter_mut().find(|g| g.graph_id == graph_id).unwrap();
    let (_, input, edge) = asset_defaults1021_rewire(graph, "C.wav");
    let expected = graph.clone();
    workspace.save(&edited).unwrap();
    asset_defaults1021_assert_runtime_sfx(workspace.project().source(), &["sfx/C.wav"]);
    drop(workspace);
    let mut cold = SourceWorkspace::open(&root).unwrap().unwrap();
    assert!(cold.source_error.is_none());
    assert_eq!(cold.project().graphs().iter().find(|g| g.graph_id == graph_id), Some(&expected));
    assert_eq!(expected.pins[&input].default_value, Some(PropertyValue::String("A.wav".into())));
    let mut disconnected = expected.clone();
    disconnected.remove_edge(edge);
    asset_defaults1021_assert_runtime_sfx(&transpile(&disconnected).unwrap().source, &["sfx/A.wav"]);
    let mut deleted = cold.project().graphs().to_vec();
    asset_defaults1021_delete_effect(deleted.iter_mut().find(|g| g.graph_id == graph_id).unwrap(), effect);
    cold.save(&deleted).unwrap();
    drop(cold);
    let reopened = SourceWorkspace::open(&root).unwrap().unwrap();
    assert!(reopened.source_error.is_none());
    let actual = reopened.project().graphs().iter().find(|g| g.graph_id == graph_id).unwrap();
    assert!(!actual.nodes.contains_key(&effect));
    assert!(!actual.pins.contains_key(&input));
    asset_defaults1021_assert_runtime_sfx(reopened.project().source(), &[]);
}


// Original NativeFrame22 Asset slots: observe the real Scene publication/input
// boundary. This host supplies no response, resource grant or replacement owner.
#[derive(Clone, Default)]
struct Asset22SceneProbe {
    batches: std::sync::Arc<std::sync::Mutex<Vec<rvn_core::renderer::SceneRenderBatch>>>,
    reject: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl rvn_core::Renderer for Asset22SceneProbe {
    fn validate_scene_batch(&self, _: &rvn_core::renderer::SceneRenderBatch) -> Result<(), String> {
        if self.reject.load(std::sync::atomic::Ordering::SeqCst) {
            Err("Asset22 actual Scene backend refused publication".into())
        } else { Ok(()) }
    }
    fn publish_scene_batch(&mut self, batch: rvn_core::renderer::SceneRenderBatch) { self.batches.lock().unwrap().push(batch); }
    fn set_background(&mut self, _: &str, _: &rvn_parser::Transition) {}
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &rvn_parser::Position, _: &rvn_parser::Transition, _: Option<&rvn_core::SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &rvn_parser::Transition, _: &rvn_core::SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &rvn_parser::Position, _: &rvn_parser::Transition, _: &rvn_core::SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize { panic!("Asset22 does not fabricate a Choice response") }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[rvn_parser::Hotspot]) -> usize { panic!("Asset22 records the actual reached input without answering it") }
    fn music_play(&mut self, _: &str, _: &rvn_parser::Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &rvn_parser::Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &rvn_parser::Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &rvn_parser::Transition) {}
}
#[derive(Clone, Copy)]
struct Asset22Slot { role: &'static str, producer: NodeKind }
#[derive(Clone, Copy)]
struct Asset22Family { kind: NodeKind, body: &'static str, slots: &'static [Asset22Slot] }
const ASSET22_FAMILIES: &[Asset22Family] = &[
    Asset22Family { kind: NodeKind::Scene, body: "scene \"background_B.png\"", slots: &[Asset22Slot { role: "background", producer: NodeKind::SceneAsset }] },
    Asset22Family { kind: NodeKind::CinematicShow, body: "cinematic \"cinematic_B.png\"", slots: &[Asset22Slot { role: "cinematic", producer: NodeKind::CinematicAsset }] },
    Asset22Family { kind: NodeKind::Imagemap, body: "imagemap {background: \"background_B.png\" hover: \"hover_B.png\" hotspot {name: \"real\" area: (0,0,100,100)} => {set chosen=1}}", slots: &[
        Asset22Slot { role: "background", producer: NodeKind::SceneAsset }, Asset22Slot { role: "hover", producer: NodeKind::HoverImageAsset },
    ] },
];
fn asset22_path(slot: Asset22Slot, phase: &str) -> String { format!("{}_{phase}.png", slot.role) }
fn asset22_command(graph: &GraphDocument, family: Asset22Family) -> NodeId {
    let nodes = graph.nodes.values().filter(|node| node.kind == family.kind).map(|node| node.id).collect::<Vec<_>>();
    assert_eq!(nodes.len(), 1); nodes[0]
}
fn asset22_observe_runtime(workspace: &SourceWorkspace, family: Asset22Family, phase: &str) {
    let graphs = workspace.project().graphs().to_vec();
    let graph = graphs.iter().find(|g| matches!(&g.kind, GraphKind::Label { name } if name == "start")).unwrap();
    let command = asset22_command(graph, family);
    let source_bytes = std::fs::read(workspace.source_path()).unwrap();
    let sidecar_path = workspace.source_path().parent().unwrap().join(".rvn-authoring.json");
    let sidecar_bytes = std::fs::read(&sidecar_path).unwrap();
    let program = workspace.freeze_native_program().unwrap();
    let origins = NativeDebugSourceMap::from_program(&program).unwrap();
    let origin = origins.origins().iter().find(|o| o.graph == graph.graph_id && o.node == command && o.role == NativeOriginRole::Consumer).unwrap();
    assert_eq!(origin.source, workspace.source_path().canonicalize().unwrap());
    assert!(origin.span.start < origin.span.end);
    for slot in family.slots {
        let pin = graph.pin_by_key(command, slot.role).unwrap();
        let edge = graph.edges.values().find(|edge| edge.input == pin.id);
        assert!(origin.ports.iter().any(|port| port.pin == pin.id && port.role == slot.role
            && port.value_type == pin.value_type && port.incoming == edge.map(|e| e.id)));
    }
    let binding = rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&program).unwrap();
    let host = Asset22SceneProbe::default();
    let mut engine = rvn_core::Engine::from_program_with_consumer_binding(program, binding, host.clone(), 8).unwrap();
    let mut random = engine.state.random; let draw = random.integer(1, 9).unwrap();
    assert_eq!(engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { character: None, text: "before".into() }));
    assert_eq!(engine.state.random, random); assert_eq!(engine.state.vars["asset_probe"], rvn_parser::Value::Int(draw));
    let active = engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap();
    assert_eq!(active.scene, "start", "the source Scene entry, not a fabricated Widget/Game target, owns this command");
    engine.advance_dialogue().unwrap();
    assert!(matches!(engine.peek_statement(), Some(rvn_parser::Statement::ScheduledConsumer { version: 2, body, .. }) if matches!(body.as_slice(),
        [rvn_parser::Statement::Scene { .. }] | [rvn_parser::Statement::CinematicShow { .. }] | [rvn_parser::Statement::Imagemap { .. }])));
    let before = serde_json::to_value(&engine.state).unwrap(); let source = engine.debug_state_source().unwrap();
    let history = engine.history.entries().iter().map(|entry| serde_json::to_value((&entry.state, &entry.display)).unwrap()).collect::<Vec<_>>();
    let batches = host.batches.lock().unwrap().len();
    host.reject.store(true, std::sync::atomic::Ordering::SeqCst);
    let rejected = if family.kind == NodeKind::Imagemap { engine.step_until_interaction().map(|_| ()) } else { engine.step_silent() };
    let error = rejected.unwrap_err().to_string(); assert!(error.contains("Asset22 actual Scene backend refused publication"), "actual refusal: {error}");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before); assert!(engine.accepts_debug_state_source(&source));
    assert_eq!(engine.history.entries().iter().map(|entry| serde_json::to_value((&entry.state, &entry.display)).unwrap()).collect::<Vec<_>>(), history);
    assert_eq!(host.batches.lock().unwrap().len(), batches);
    host.reject.store(false, std::sync::atomic::Ordering::SeqCst);
    let interaction = if family.kind == NodeKind::Imagemap { engine.step_until_interaction().unwrap() } else { engine.step_silent().unwrap(); None };
    assert_eq!(engine.state.random, random, "static Asset inputs must not gather another random value");
    assert_eq!(host.batches.lock().unwrap().len(), batches + 1, "one actual accepted publication follows the refused candidate");
    let accepted = host.batches.lock().unwrap().last().unwrap().clone();
    match family.kind {
        NodeKind::Scene => {
            let expected = asset22_path(family.slots[0], phase);
            assert_eq!(engine.state.background_image, expected); assert_eq!(accepted.state.background_image, expected);
        },
        NodeKind::CinematicShow => {
            let expected = asset22_path(family.slots[0], phase);
            assert_eq!(engine.state.cinematic.current.as_deref(), Some(expected.as_str()));
            assert_eq!(accepted.state.cinematic.current.as_deref(), Some(expected.as_str()));
        },
        NodeKind::Imagemap => {
            let rvn_core::Interaction::Imagemap { background, hover_image, hotspots } = interaction.unwrap() else { panic!("actual reached Imagemap required"); };
            assert_eq!(background, asset22_path(family.slots[0], phase)); assert_eq!(hover_image, Some(asset22_path(family.slots[1], phase)));
            assert_eq!(hotspots.len(), 1); assert!(!engine.state.vars.contains_key("chosen"));
            assert!(matches!(&accepted.display, Some(rvn_core::Interaction::Imagemap { background: actual, hover_image: hover, .. }) if actual == &background && hover == &hover_image));
            let input = engine.preview_input_source().unwrap(); assert_eq!(input.choice_indices, vec![0]);
            let waiting = serde_json::to_value(&engine.state).unwrap(); let count = host.batches.lock().unwrap().len();
            assert!(engine.submit_hotspot(99).is_err()); assert_eq!(serde_json::to_value(&engine.state).unwrap(), waiting); assert!(engine.accepts_preview_input_source(&input)); assert_eq!(host.batches.lock().unwrap().len(), count);
            engine.submit_hotspot(0).unwrap(); assert!(!engine.accepts_preview_input_source(&input));
            assert_eq!(engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { character: None, text: "after".into() }));
            assert_eq!(engine.state.vars["chosen"], rvn_parser::Value::Int(1));
        },
        _ => unreachable!(),
    }
    if family.kind!=NodeKind::Imagemap {
        assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"after".into()}));
    }
    assert_eq!(workspace.project().graphs(), graphs); assert_eq!(std::fs::read(workspace.source_path()).unwrap(), source_bytes);
    assert_eq!(std::fs::read(sidecar_path).unwrap(), sidecar_bytes);
}
#[test]
fn asset_defaults1021_original22_three_families_hidden_slot_external_source_disconnect_cold_and_real_backend_refusal() {
    for family in ASSET22_FAMILIES.iter().copied() {
        let authored = format!("handler retained_macros(event) {{local orphan_probe=1}}\nscene label start\nset asset_probe=random(1,9)\n\"before\"\n{}\n\"after\"\n", family.body);
        let (root, project) = fragment_rename_fixture(&authored, "");
        let mut graphs = project.graphs().to_vec();
        let graph = graphs.iter_mut().find(|g| matches!(&g.kind, GraphKind::Label { name } if name == "start")).unwrap();
        let command = asset22_command(graph, family); let graph_id = graph.graph_id;
        let slots = family.slots.iter().map(|slot| {
            let pin = graph.pin_by_key(command, slot.role).unwrap().id;
            let edge = graph.edges.values().find(|edge| edge.input == pin).unwrap();
            assert_eq!(graph.nodes[&graph.pins[&edge.output].node].kind, slot.producer);
            (slot.role, pin)
        }).collect::<Vec<_>>();
        for slot in family.slots { graph.set_pin_default(command, slot.role, PropertyValue::String(asset22_path(*slot, "A"))).unwrap(); }
        graph.enable_native_frame_preparation().unwrap(); let migrated = graph.clone();
        let compiled = transpile_project(&graphs).unwrap();
        // Establish the expected document through the actual Native source
        // reconstruction before linking. Legacy inferred binding caches are
        // derived presentation; Native typed plans own the executable inputs.
        let graphs = reimport_native_frame_script(&compiled.ast, &graphs).unwrap();
        let original = graphs.iter().find(|g| g.graph_id == graph_id).unwrap().clone();
        assert_eq!(transpile_project(&graphs).unwrap().ast, compiled.ast);
        assert_eq!(original.variables, migrated.variables);
        assert_eq!(original.pins, migrated.pins); assert_eq!(original.edges, migrated.edges);
        assert!(original.nodes.keys().eq(migrated.nodes.keys()));
        for (role, pin) in &slots {
            assert_eq!(original.pin_by_key(command, role).unwrap().id, *pin);
            assert_eq!(original.pins[pin].default_value, Some(PropertyValue::String(format!("{role}_A.png"))));
        }
        for slot in family.slots { assert!(!compiled.source.contains(&asset22_path(*slot, "A")), "hidden A is not the wired runtime operand"); }
        std::fs::write(root.join("main.rvn"), &compiled.source).unwrap();
        let workspace = SourceWorkspace::link_native_preparation(&root, &root.join("main.rvn"), &graphs).unwrap();
        assert_eq!(workspace.project().graphs().iter().find(|g| g.graph_id == graph_id), Some(&original));
        asset22_observe_runtime(&workspace, family, "B");
        let sidecar_b = std::fs::read(root.join(".rvn-authoring.json")).unwrap();
        let mut changed = workspace.project().graphs().to_vec();
        let graph = changed.iter_mut().find(|g| g.graph_id == graph_id).unwrap();
        for slot in family.slots {
            let input = graph.pin_by_key(command, slot.role).unwrap().id;
            let old = graph.edges.values().find(|edge| edge.input == input).unwrap().clone();
            let producer = graph.pins[&old.output].node; assert_eq!(graph.nodes[&producer].kind, slot.producer);
            assert_eq!(graph.edges.values().filter(|edge| edge.output == old.output).count(), 1);
            graph.remove_node(producer).unwrap();
            let next = graph.add_catalog_node(slot.producer, [-400.0, 100.0]).unwrap();
            graph.set_property(next, "path", PropertyValue::String(asset22_path(*slot, "C"))).unwrap();
            let edge = graph.connect(graph.pin_by_key(next, "value").unwrap().id, input).unwrap();
            assert_ne!(edge, old.id); assert_ne!(next, producer);
        }
        let expected = graph.clone(); let external = transpile_project(&changed).unwrap();
        for slot in family.slots { assert!(!external.source.contains(&asset22_path(*slot, "A"))); }
        // A legitimate external source update wins over the old sidecar's
        // topology. The old live source snapshot must first refuse freezing.
        std::fs::write(root.join("main.rvn"), &external.source).unwrap();
        assert!(workspace.freeze_native_program().is_err());
        assert_eq!(std::fs::read(root.join(".rvn-authoring.json")).unwrap(), sidecar_b); drop(workspace);
        let mut cold = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
        assert!(cold.source_error.is_none(), "actual external refresh: {:?}", cold.source_error);
        assert_eq!(cold.project().source(), external.source);
        let actual = cold.project().graphs().iter().find(|g| g.graph_id == graph_id).unwrap();
        assert_eq!(actual.edges, expected.edges); assert!(actual.nodes.keys().eq(expected.nodes.keys())); assert!(actual.pins.keys().eq(expected.pins.keys()));
        for (role, pin) in &slots {
            assert_eq!(actual.pin_by_key(command, role).unwrap().id, *pin);
            assert_eq!(actual.pins[pin].default_value, Some(PropertyValue::String(format!("{role}_A.png"))));
        }
        assert_eq!(actual.nodes[&command].position, original.nodes[&command].position);
        asset22_observe_runtime(&cold, family, "C");
        let retained = cold.project().graphs().to_vec(); cold.save(&retained).unwrap(); drop(cold);
        let mut cold = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap(); assert!(cold.source_error.is_none());
        assert_eq!(cold.project().graphs(), retained);
        // Retain authentic isolated authoring macros in a different Native
        // scope before editing the Scene. They have no source/VM identity.
        let mut with_presentation = cold.project().graphs().to_vec();
        let handler = with_presentation.iter_mut().find(|g|matches!(&g.kind,GraphKind::Handler{name}if name=="retained_macros")).unwrap();
        handler.enable_native_frame_preparation().unwrap();
        let isolated = [NodeKind::DoOnce,NodeKind::Gate].into_iter().enumerate()
            .map(|(i,kind)|handler.add_catalog_node(kind,[500.0,100.0+i as f64*200.0]).unwrap()).collect::<Vec<_>>();
        let authored_handler = handler.clone();
        assert!(prepare_native_frame_graph(handler).is_err(),"strict source cannot encode isolated macros");
        let authoring = prepare_native_authoring_graph(handler).unwrap();
        assert!(!authoring.source.contains("flow.macro_dispatch_v1("));
        cold.save(&with_presentation).unwrap();drop(cold);
        let mut cold = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();assert!(cold.source_error.is_none());
        let handler = cold.project().graphs().iter().find(|g|g.graph_id==authored_handler.graph_id).unwrap();
        assert_eq!(handler.edges,authored_handler.edges);
        for id in &isolated {
            assert_eq!(handler.nodes[id],authored_handler.nodes[id]);
            for pin in &authored_handler.nodes[id].pins {assert_eq!(handler.pins[pin],authored_handler.pins[pin]);}
        }
        let handler_source = |source:&str| {
            let parsed=rvn_parser::parse_program(source).unwrap();
            let index=parsed.script.iter().position(|s|matches!(s,rvn_parser::Statement::Handler{name,..}if name=="retained_macros")).unwrap();
            source[parsed.origins[index].span.range.clone()].to_owned()
        };
        let untouched_handler_source=handler_source(cold.project().source());
        let mut disconnected = cold.project().graphs().to_vec();
        let graph = disconnected.iter_mut().find(|g| g.graph_id == graph_id).unwrap();
        for (_, input) in &slots {
            let edge = graph.edges.values().find(|edge| edge.input == *input).unwrap(); let producer = graph.pins[&edge.output].node;
            graph.remove_node(producer).unwrap(); assert!(!graph.edges.values().any(|edge| edge.input == *input));
        }
        cold.save(&disconnected).unwrap_or_else(|error|panic!("Asset22 disconnected {:?}: {error}",family.kind)); drop(cold);
        let cold = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap(); assert!(cold.source_error.is_none());
        assert_eq!(handler_source(cold.project().source()),untouched_handler_source,"unmodified sibling source bytes survive the Scene transport rewrite");
        let handler = cold.project().graphs().iter().find(|g|g.graph_id==authored_handler.graph_id).unwrap();
        assert!(handler.nodes.keys().eq(authored_handler.nodes.keys()));
        assert!(handler.pins.keys().eq(authored_handler.pins.keys()));
        assert_eq!(handler.edges,authored_handler.edges);
        for id in &isolated {
            assert_eq!(handler.nodes[id],authored_handler.nodes[id]);
            for pin in &authored_handler.nodes[id].pins {assert_eq!(handler.pins[pin],authored_handler.pins[pin]);}
        }
        let frozen=cold.freeze_native_program().unwrap();
        let source_only=import_native_frame_script(&frozen.script).unwrap();
        let plain_handler=source_only.iter().find(|g|g.graph_id==authored_handler.graph_id).unwrap();
        assert!(isolated.iter().all(|id|!plain_handler.nodes.contains_key(id)));
        let origins=NativeDebugSourceMap::from_program(&frozen).unwrap();
        assert!(!origins.origins().iter().any(|o|o.graph==authored_handler.graph_id&&isolated.contains(&o.node)));
        let actual = cold.project().graphs().iter().find(|g| g.graph_id == graph_id).unwrap();
        for (role, pin) in &slots {
            assert_eq!(actual.pin_by_key(command, role).unwrap().id, *pin);
            assert_eq!(actual.pins[pin].default_value, Some(PropertyValue::String(format!("{role}_A.png"))));
            assert!(!actual.edges.values().any(|edge| edge.input == *pin));
        }
        asset22_observe_runtime(&cold, family, "A");
    }
}

fn scalar_fallback1021_source(kind: NodeKind, key: &str, source: &str, a: PropertyValue, b: PropertyValue, c: PropertyValue) {
    let parsed=rvn_parser::parse_program(source).unwrap();
    assert_eq!(parsed.context,rvn_parser::NarrativeContext::ExplicitScenes);
    let mut graphs=import_script(&parsed.script).unwrap();
    let graph=graphs.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
    graph.enable_native_frame_preparation().unwrap();
    let command=graph.nodes.values().find(|node|node.kind==kind).unwrap().id;
    let input=graph.pin_by_key(command,key).unwrap().id;
    let old=graph.edges.values().find(|edge|edge.input==input).unwrap().clone();
    let prior=graph.pins[&old.output].node;
    assert_eq!(graph.edges.values().filter(|edge|edge.output==old.output).count(),1);
    graph.remove_node(prior).unwrap();
    graph.set_pin_default(command,key,a.clone()).unwrap();
    let producer=graph.add_catalog_node(NodeKind::Literal,[-400.0,80.0]).unwrap();
    graph.set_property(producer,"value",b.clone()).unwrap();
    let edge=graph.connect(graph.pin_by_key(producer,"value").unwrap().id,input).unwrap();
    assert_eq!(graph.pins[&input].default_value,Some(a.clone()),"source fixture must retain A through a real public connection; no sidecar injection");
    let id=graph.graph_id;
    let authored=graph.clone();
    let compiled=transpile_project(&graphs).unwrap();
    // The hidden value cannot affect the executable AST, even when changed.
    let mut other=authored.clone();other.set_pin_default(command,key,b.clone()).unwrap();
    assert_eq!(prepare_native_frame_graph(&other).unwrap().ast,prepare_native_frame_graph(&authored).unwrap().ast);
    let plain=import_native_frame_script(&compiled.ast).unwrap();
    let plain=plain.iter().find(|graph|graph.graph_id==id).unwrap();
    assert_eq!(plain.pins[&input].default_value,None,"source alone has no authority to invent hidden A");
    assert_eq!(plain.edges,authored.edges);
    assert!(plain.nodes.keys().eq(authored.nodes.keys()));assert!(plain.pins.keys().eq(authored.pins.keys()));
    let mut explicit=authored.clone();explicit.remove_node(producer).unwrap();
    explicit.set_pin_default(command,key,b.clone()).unwrap();
    let explicit_source=prepare_native_frame_graph(&explicit).unwrap();
    let explicit_replay=reimport_native_frame_script(&explicit_source.ast,&graphs).unwrap();
    let explicit_replay=explicit_replay.iter().find(|graph|graph.graph_id==id).unwrap();
    assert_eq!(explicit_replay.pins[&input].default_value,Some(b),"old hidden A cannot override an explicit current unconnected source value");
    assert!(!explicit_replay.edges.values().any(|edge|edge.input==input));
    let restored=reimport_native_frame_script(&compiled.ast,&graphs).unwrap();
    let restored=restored.iter().find(|graph|graph.graph_id==id).unwrap();
    assert_eq!(restored.pins[&input],authored.pins[&input]);
    let (root,_)=fragment_rename_fixture(source,"");
    std::fs::write(root.join("main.rvn"),&compiled.source).unwrap();
    let workspace=SourceWorkspace::link_native_preparation(&root,&root.join("main.rvn"),&graphs).unwrap();
    let original=workspace.project().graphs().to_vec();
    let sidecar_path=root.join(".rvn-authoring.json");
    let saved_b=std::fs::read(&sidecar_path).unwrap();
    drop(workspace);
    let workspace=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert!(workspace.source_error.is_none(),"{:?}",workspace.source_error);
    assert_eq!(workspace.project().graphs(),original);
    assert_eq!(std::fs::read(&sidecar_path).unwrap(),saved_b,"cold reading does not manufacture new authoring metadata");
    let mut changed=workspace.project().graphs().to_vec();
    let graph=changed.iter_mut().find(|graph|graph.graph_id==id).unwrap();
    let before=graph.clone();
    assert_eq!(before.pins[&input].default_value,Some(a.clone()));
    assert_eq!(before.edges[&edge].output,authored.edges[&edge].output);
    graph.remove_node(producer).unwrap();
    let replacement=graph.add_catalog_node(NodeKind::Literal,[-400.0,180.0]).unwrap();
    assert_ne!(replacement,producer);
    graph.set_property(replacement,"value",c).unwrap();
    let new_edge=graph.connect(graph.pin_by_key(replacement,"value").unwrap().id,input).unwrap();
    assert_ne!(new_edge,edge);
    assert_eq!(graph.pins[&input].default_value,Some(a.clone()));
    let expected=graph.clone();
    let external=transpile_project(&changed).unwrap();
    std::fs::write(root.join("main.rvn"),&external.source).unwrap();
    assert!(workspace.freeze_native_program().is_err(),"old source ownership cannot freeze a new external producer");
    assert_eq!(std::fs::read(&sidecar_path).unwrap(),saved_b);drop(workspace);
    let mut workspace=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert!(workspace.source_error.is_none(),"{:?}",workspace.source_error);
    let graph=workspace.project().graphs().iter().find(|graph|graph.graph_id==id).unwrap();
    assert_eq!(graph.edges,expected.edges);assert!(graph.nodes.keys().eq(expected.nodes.keys()));assert!(graph.pins.keys().eq(expected.pins.keys()));
    assert_eq!(graph.pins[&input],expected.pins[&input],"new source C controls its real edge; the compatible same authored slot still retains A");
    let retained=workspace.project().graphs().to_vec();workspace.save(&retained).unwrap();drop(workspace);
    let mut workspace=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();assert!(workspace.source_error.is_none());
    assert_eq!(workspace.project().graphs(),retained);
    let mut disconnected=workspace.project().graphs().to_vec();
    let graph=disconnected.iter_mut().find(|graph|graph.graph_id==id).unwrap();
    graph.remove_node(replacement).unwrap();
    assert!(!graph.edges.values().any(|edge|edge.input==input));
    assert_eq!(graph.pin_by_key(command,key).unwrap().id,input);
    assert_eq!(graph.pins[&input].default_value,Some(a.clone()),"real disconnect exposes A from its surviving declared input");
    let emitted=transpile_project(&disconnected).unwrap();
    // Unlike hidden A, the unconnected A must now be authored in executable source.
    let plain=import_native_frame_script(&emitted.ast).unwrap();
    let plain=plain.iter().find(|graph|graph.graph_id==id).unwrap();
    assert_eq!(plain.pin_by_key(command,key).unwrap().id,input);
    assert_eq!(plain.pins[&input].default_value,Some(a.clone()));
    let before=std::fs::read(root.join("main.rvn")).unwrap();
    let meta=std::fs::read(&sidecar_path).unwrap();
    std::fs::write(root.join("main.rvn"),format!("{}\n// actual external writer\n",String::from_utf8(before.clone()).unwrap())).unwrap();
    assert!(workspace.save(&disconnected).is_err());assert_eq!(std::fs::read(&sidecar_path).unwrap(),meta);
    assert_eq!(workspace.project().graphs(),retained);
    std::fs::write(root.join("main.rvn"),before).unwrap();
    workspace.save(&disconnected).unwrap();let saved=workspace.project().graphs().to_vec();drop(workspace);
    let cold=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();assert!(cold.source_error.is_none());
    assert_eq!(cold.project().graphs(),saved);
    let graph=cold.project().graphs().iter().find(|graph|graph.graph_id==id).unwrap();
    assert_eq!(graph.pins[&input].default_value,Some(a));
    assert!(!graph.edges.values().any(|edge|edge.input==input));
}
#[test]
fn scalar_fallback1021_bool_real_source_wire_change_disconnect_and_cold() {
    scalar_fallback1021_source(NodeKind::TypewriterSet,"enabled","scene label start\ntypewriter=true\n\"boundary\"\n",PropertyValue::Bool(false),PropertyValue::Bool(true),PropertyValue::Bool(false));
}
#[test]
fn scalar_fallback1021_int_real_source_wire_change_disconnect_and_cold() {
    scalar_fallback1021_source(NodeKind::TypewriterSpeed,"speed","scene label start\ntypewriter.speed(23)\n\"boundary\"\n",PropertyValue::Int(11),PropertyValue::Int(23),PropertyValue::Int(47));
}
#[test]
fn scalar_fallback1021_float_real_source_wire_change_disconnect_and_cold() {
    scalar_fallback1021_source(NodeKind::MusicVolume,"level","scene label start\nmusic.volume(\"0.5\")\n\"boundary\"\n",PropertyValue::Float(0.125),PropertyValue::Float(0.5),PropertyValue::Float(0.75));
}

