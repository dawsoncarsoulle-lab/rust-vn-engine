use rvn_graph::*;
use std::{fs, path::PathBuf};

fn directory(name: &str) -> PathBuf {
    let base = PathBuf::from(
        std::env::var("RVN_NATIVE_SOURCE_TEST_DIR").expect("private fixture directory required"),
    );
    let path = base.join(format!(
        "origin-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
fn workspace(name: &str, source: &str) -> (PathBuf, SourceWorkspace) {
    let root = directory(name);
    let main = root.join("main.rvn");
    fs::write(&main, source).unwrap();
    let workspace = SourceWorkspace::link_native_preparation(&root, &main, &[]).unwrap();
    (root, workspace)
}
fn source() -> String {
    include_str!("fixtures/native_source_single.rvn").into()
}
fn wire(g: &mut GraphDocument, a: NodeId, ak: &str, b: NodeId, bk: &str) {
    g.connect(
        g.pin_by_key(a, ak).unwrap().id,
        g.pin_by_key(b, bk).unwrap().id,
    )
    .unwrap();
}

#[test]
fn lexer_ranges_are_exact_original_utf8_with_unicode_crlf_comments_and_reparse() {
    let raw = format!(
        "// préface 世界😀\n{}",
        source().replace(" = ", " = /*invalid*/ ")
    );
    // Invalid lexer bytes cannot be silently dropped or normalized.
    assert!(workspace_invalid(&raw));
    let raw = format!(
        "// préface 世界😀\n{}",
        source().replace(
            "        inspect",
            "        // identity remains source authoritative\n        inspect"
        )
    )
    .replace('\n', "\r\n");
    let (root, workspace) = workspace("utf8", &raw);
    let frozen = workspace.freeze_native_program().unwrap();
    let map = NativeDebugSourceMap::from_program(&frozen).unwrap();
    let producers: Vec<_> = map
        .origins()
        .iter()
        .filter(|o| matches!(o.role, NativeOriginRole::Producer { .. }))
        .collect();
    assert_eq!(producers.len(), 3);
    for origin in producers {
        let prefix = &raw[origin.span.clone()];
        assert!(prefix.starts_with("capture ") || prefix.starts_with("inspect "));
        let expression = &raw[origin.expression.clone().unwrap()];
        assert!(!expression.starts_with("capture "));
        assert!(!expression.ends_with('\n'));
        assert!(!expression.ends_with('\r'));
        assert!(origin.span.start > raw.find("flow.consume_v1").unwrap());
        assert_eq!(
            map.at_byte(
                &root.join("main.rvn").canonicalize().unwrap(),
                origin.expression.as_ref().unwrap().start
            )[0]
            .node,
            origin.node
        );
    }
    assert!(raw.contains("世界"));
}
fn workspace_invalid(raw: &str) -> bool {
    let root = directory("invalid");
    let main = root.join("main.rvn");
    fs::write(&main, raw).unwrap();
    SourceWorkspace::link_native_preparation(&root, &main, &[]).is_err()
}

#[test]
fn inspector_has_one_step_and_three_real_typed_output_pins() {
    let (_, workspace) = workspace("ports", &source());
    let program = workspace.freeze_native_program().unwrap();
    let map = NativeDebugSourceMap::from_program(&program).unwrap();
    let origin = map
        .origins()
        .iter()
        .find(|o| {
            matches!(
                o.role,
                NativeOriginRole::Producer {
                    inspector: true,
                    ..
                }
            )
        })
        .unwrap();
    assert_eq!(origin.node, NodeId::new(3));
    for (key, pin) in [("valid", 6), ("table_id", 7), ("key", 8)] {
        let endpoints = map.endpoint(&origin.source, origin.graph, origin.node, PinId::new(pin));
        assert_eq!(endpoints.len(), 1);
        assert!(endpoints[0].ports.iter().any(|port| port.role == key
            && port.pin == PinId::new(pin)
            && port.direction == PinDirection::Output));
    }
    assert!(map
        .endpoint(&origin.source, origin.graph, origin.node, PinId::new(9000))
        .is_empty());
}

#[test]
fn real_writer_edit_and_cold_source_only_rebuild_ranges_ids_and_ports() {
    let (root, mut workspace) = workspace("writer", &source());
    let before = NativeDebugSourceMap::from_workspace(&workspace).unwrap();
    let mut graphs = workspace.project().graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|g| g.graph_id == GraphId::new(900))
        .unwrap();
    graph
        .set_pin_default(
            NodeId::new(2),
            "key",
            PropertyValue::String("longer 😀\nclé avec espace\r\nfin".into()),
        )
        .unwrap();
    workspace.save(&graphs).unwrap();
    let after = NativeDebugSourceMap::from_workspace(&workspace).unwrap();
    assert_ne!(before.origins(), after.origins());
    fs::remove_file(root.join(".rvn-authoring.json")).unwrap();
    let cold =
        SourceWorkspace::link_native_preparation(&root, &root.join("main.rvn"), &[]).unwrap();
    let reopened = NativeDebugSourceMap::from_workspace(&cold).unwrap();
    assert_eq!(after.origins(), reopened.origins());
    for (old, new) in before.origins().iter().zip(after.origins()) {
        assert_eq!(
            (old.graph, old.node, &old.ports),
            (new.graph, new.node, &new.ports)
        );
    }
}

#[test]
fn frozen_ast_origin_or_source_forgery_refused_and_inclusion_ordinals_are_real() {
    let root = directory("include");
    fs::write(root.join("probe.rvn"), source()).unwrap();
    fs::write(
        root.join("main.rvn"),
        "use \"probe.rvn\"\nlabel start\n\"done\"\n",
    )
    .unwrap();
    let workspace =
        SourceWorkspace::link_native_preparation(&root, &root.join("main.rvn"), &[]).unwrap();
    let program = workspace.freeze_native_program().unwrap();
    let map = NativeDebugSourceMap::from_program(&program).unwrap();
    assert!(map
        .origins()
        .iter()
        .all(|o| o.source_ordinal == 1 && o.source.ends_with("probe.rvn")));
    let mut bad = program.clone();
    bad.origins[0].span.range.start += 1;
    assert!(NativeDebugSourceMap::from_program(&bad).is_err());
    let mut bad = program.clone();
    bad.sources[1].source = bad.sources[1]
        .source
        .replace("Hello\\n世界", "other\\n世界");
    assert!(NativeDebugSourceMap::from_program(&bad).is_err());
    let bad = source().replace("valid=6@0,table_id=7@0", "valid=6@0,table_id=6@0");
    assert!(workspace_invalid(&bad));
}

#[test]
fn mixed_legacy_terminals_select_only_the_actual_pin_and_native_is_one_step() {
    let mut g = GraphDocument::new(
        GraphId::new(902),
        GraphKind::Function {
            name: "mixed".into(),
        },
    );
    let root = g
        .add_catalog_node(NodeKind::FunctionEntry, [0., 0.])
        .unwrap();
    let legacy = g
        .add_catalog_node(NodeKind::GetStringTableIdAndKey, [100., 0.])
        .unwrap();
    g.nodes.get_mut(&legacy).unwrap().inspector_contract =
        Some(InspectorEvaluationContract::LegacyStateless);
    let native = g
        .add_catalog_node(NodeKind::GetStringTableIdAndKey, [200., 0.])
        .unwrap();
    let list = g
        .add_catalog_node(NodeKind::ListLiteral, [300., 0.])
        .unwrap();
    g.resize_value_inputs(list, 6).unwrap();
    for (i, (node, key)) in [legacy, native]
        .into_iter()
        .flat_map(|node| {
            ["valid", "table_id", "key"]
                .into_iter()
                .map(move |key| (node, key))
        })
        .enumerate()
    {
        wire(&mut g, node, key, list, &format!("item_{i}"));
    }
    let ret = g
        .add_catalog_node(NodeKind::FunctionReturn, [500., 0.])
        .unwrap();
    wire(&mut g, root, "exec_out", ret, "exec_in");
    wire(&mut g, list, "list", ret, "value");
    let raw = prepare_native_frame_graph(&g).unwrap().source;
    let (_, workspace) = workspace("mixed", &raw);
    let map = NativeDebugSourceMap::from_workspace(&workspace).unwrap();
    assert_eq!(map.origins().iter().filter(|o| o.node == legacy).count(), 3);
    assert_eq!(map.origins().iter().filter(|o| o.node == native).count(), 1);
    for role in ["valid", "table_id", "key"] {
        let pin = g.pin_by_key(legacy, role).unwrap().id;
        let endpoints = map.endpoint(
            &workspace.freeze_native_program().unwrap().sources[0]
                .path
                .clone()
                .unwrap(),
            g.graph_id,
            legacy,
            pin,
        );
        assert_eq!(endpoints.len(), 1);
        assert!(
            matches!(&endpoints[0].role,NativeOriginRole::Producer{legacy_terminal:Some(actual),..}if actual==role)
        );
    }
    let cold = GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();
    assert_eq!(
        cold.nodes[&legacy].inspector_contract,
        g.nodes[&legacy].inspector_contract
    );
}

#[test]
fn branches_commands_and_empty_branch_end_keep_distinct_genuine_node_ranges() {
    let mut graph = GraphDocument::new(
        GraphId::new(970),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let root = graph.add_catalog_node(NodeKind::Label, [0., 0.]).unwrap();
    let inspector = graph
        .add_catalog_node(NodeKind::GetStringTableIdAndKey, [100., 0.])
        .unwrap();
    let branch = graph.add_catalog_node(NodeKind::If, [300., 0.]).unwrap();
    wire(&mut graph, root, "exec_out", branch, "exec_in");
    wire(&mut graph, inspector, "valid", branch, "condition");
    let command = graph
        .add_catalog_node(NodeKind::AccessibilitySpeak, [500., 0.])
        .unwrap();
    wire(&mut graph, branch, "then", command, "exec_in");
    wire(&mut graph, inspector, "key", command, "text");
    let end = graph.add_branch_end(branch, [700., 0.]).unwrap();
    wire(&mut graph, command, "exec_out", end, "exec_in");
    let raw = prepare_native_frame_graph(&graph).unwrap().source;
    let (_, workspace) = workspace("branch", &raw);
    let map = NativeDebugSourceMap::from_workspace(&workspace).unwrap();
    for node in [branch, command, end] {
        assert!(map
            .origins()
            .iter()
            .any(|o| o.node == node && o.role == NativeOriginRole::Consumer));
    }
    let body = map
        .origins()
        .iter()
        .find(|o| o.node == branch && o.role == NativeOriginRole::ConsumerBody)
        .unwrap();
    assert!(raw[body.span.clone()].starts_with("if "));
    assert!(!map
        .origins()
        .iter()
        .any(|o| o.node == end && matches!(o.role, NativeOriginRole::Producer { .. })));
}

#[test]
fn excessive_frozen_source_rejected_before_lexer_or_graph_expansion() {
    let (_, workspace) = workspace("budget", &source());
    let mut program = workspace.freeze_native_program().unwrap();
    program.sources[0]
        .source
        .push_str(&format!("//{}", "x".repeat(16 * 1024 * 1024)));
    assert!(NativeDebugSourceMap::from_program(&program)
        .err()
        .unwrap()
        .contains("16 MiB"));
}

#[test]
fn historical_intrinsics_without_source_identity_do_not_receive_invented_graph_ids() {
    let root = directory("legacy");
    let main = root.join("main.rvn");
    fs::write(&main,"function old(){return __rvn_text_table_valid(__rvn_invariant_text(\"old\"))}\nlabel start\n\"legacy\"\n").unwrap();
    let program = rvn_parser::parse_file_program_with_uses(&main).unwrap();
    let map = NativeDebugSourceMap::from_program(&program).unwrap();
    assert!(map.origins().is_empty());
    assert!(map.at_byte(&main.canonicalize().unwrap(), 1).is_empty());
    assert_eq!(
        rvn_parser::parse_file_program_with_uses(&main).unwrap(),
        program
    );
}

#[test]
fn init_function_screen_and_handler_source_provenance_uses_the_actual_lexical_root() {
    for (index, kind) in [
        GraphKind::Init,
        GraphKind::Function {
            name: "root_fn".into(),
        },
        GraphKind::Screen {
            name: "root_screen".into(),
        },
        GraphKind::Handler {
            name: "root_handler".into(),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut graph = GraphDocument::new(GraphId::new(980 + index as u64), kind);
        let entry = match graph.kind {
            GraphKind::Init => NodeKind::Init,
            GraphKind::Function { .. } => NodeKind::FunctionEntry,
            GraphKind::Screen { .. } => NodeKind::ScreenEntry,
            _ => NodeKind::HandlerEntry,
        };
        let root = graph.add_catalog_node(entry, [0., 0.]).unwrap();
        let inspector = graph
            .add_catalog_node(NodeKind::GetStringTableIdAndKey, [100., 0.])
            .unwrap();
        let consumer = if entry == NodeKind::Init {
            let set = graph
                .add_catalog_node(NodeKind::SetVariable, [200., 0.])
                .unwrap();
            graph.variables.insert(
                "found".into(),
                VariableDefinition {
                    name: "found".into(),
                    value_type: ValueType::Bool,
                    default_value: PropertyValue::Bool(false),
                },
            );
            graph
                .set_property(set, "name", PropertyValue::String("found".into()))
                .unwrap();
            wire(&mut graph, inspector, "valid", set, "value");
            set
        } else {
            let ret = graph
                .add_catalog_node(NodeKind::FunctionReturn, [200., 0.])
                .unwrap();
            wire(&mut graph, inspector, "valid", ret, "value");
            ret
        };
        wire(&mut graph, root, "exec_out", consumer, "exec_in");
        let raw = prepare_native_frame_graph(&graph).unwrap().source;
        let (_, workspace) = workspace(&format!("lexical-{index}"), &raw);
        let map = NativeDebugSourceMap::from_workspace(&workspace).unwrap();
        let declaration = map
            .origins()
            .iter()
            .find(|origin| origin.role == NativeOriginRole::Declaration)
            .unwrap();
        assert_eq!(
            (declaration.graph, declaration.node, declaration.kind),
            (graph.graph_id, root, entry)
        );
        assert_eq!(declaration.definition_root_ordinal, declaration.route[0]);
        assert!(map
            .origins()
            .iter()
            .filter(|origin| origin.role != NativeOriginRole::Declaration)
            .all(|origin| origin.scope == declaration.scope
                && origin.definition_root_ordinal == declaration.definition_root_ordinal));
    }
}
