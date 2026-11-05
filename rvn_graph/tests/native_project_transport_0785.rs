//! Project source transport is not a Graph node. CPU source/identity tests only.
use rvn_graph::*;
use rvn_parser::{Script, Statement};

fn graphs(raw: &str, native_labels: &[&str]) -> Vec<GraphDocument> {
    let mut graphs = import_script(&rvn_parser::parse(raw).unwrap()).unwrap();
    for graph in &mut graphs {
        *graph = GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
        if matches!(&graph.kind, GraphKind::Label { name } if native_labels.contains(&name.as_str())) {
            graph.enable_native_frame_preparation().unwrap();
        }
    }
    graphs
}
fn sample() -> Vec<GraphDocument> {
    graphs("label start\n\"first\"\nreturn\nlabel other\n\"second\"\n", &["start", "other"])
}
fn exact(original: &[GraphDocument]) -> Script {
    let before = original.to_vec();
    let compiled = transpile_project(original).unwrap();
    let source_ast = rvn_parser::parse(&compiled.source).unwrap();
    assert_eq!(source_ast, compiled.ast);
    let ast_before = source_ast.clone();
    let imported = import_script(&source_ast).unwrap();
    assert_eq!(source_ast, ast_before, "import never edits the executable AST");
    assert_eq!(original, before);
    for graph in original.iter().filter(|g| !g.inspector_evaluation.is_legacy()) {
        let actual = imported.iter().find(|g| g.graph_id == graph.graph_id).unwrap();
        assert_eq!(actual.kind, graph.kind);
        assert_eq!(actual.nodes.keys().collect::<Vec<_>>(), graph.nodes.keys().collect::<Vec<_>>());
        assert_eq!(actual.pins.keys().collect::<Vec<_>>(), graph.pins.keys().collect::<Vec<_>>());
        assert_eq!(actual.edges, graph.edges);
    }
    assert_eq!(transpile_project(&imported).unwrap().ast, source_ast);
    // The native public importer also enters the same project adapter, even
    // when standalone graph preparation does not ask for project validation.
    let native = import_native_frame_script(&source_ast).unwrap();
    assert_eq!(transpile_project(&native).unwrap().ast, source_ast);
    source_ast
}
fn raw_footer(script: &Script) -> usize {
    script.iter().position(|s| matches!(s, Statement::Jump { target } if target == "__blueprint_project_end")).unwrap()
}
fn reject(script: &Script) {
    let before = script.clone();
    assert!(import_script(script).is_err());
    assert!(import_native_frame_script(script).is_err());
    assert_eq!(script, &before);
}
#[test]
fn transport0785_multiple_native_labels_keep_real_nodes_and_exact_project_ast() {
    let ast = exact(&sample());
    assert_eq!(ast.iter().filter(|s| matches!(s, Statement::Jump { target } if target == "__blueprint_project_end")).count(), 2);
}
#[test]
fn transport0785_disconnected_native_output_never_falls_through_to_next_label() {
    let mut docs = sample();
    let start = docs.iter_mut().find(|g| matches!(&g.kind, GraphKind::Label { name } if name == "start")).unwrap();
    let ret = start.nodes.values().find(|n| n.kind == NodeKind::Return).unwrap().id;
    start.remove_node(ret).unwrap();
    let ast = exact(&docs);
    let position = ast.iter().position(|s| matches!(s, Statement::Label { name } if name == "other")).unwrap();
    assert!(matches!(&ast[position-1], Statement::Jump { target } if target == "__blueprint_project_end"));
}
#[test]
fn transport0785_authored_native_jump_is_preserved_with_its_real_identity() {
    let docs = graphs("label start\n\"first\"\njump other\nlabel other\n\"second\"\n", &["start", "other"]);
    let ast = exact(&docs);
    assert!(ast.iter().any(|s| matches!(s, Statement::ScheduledConsumer { body, .. }
        if matches!(body.as_slice(), [Statement::Jump { target }] if target == "other"))));
}
#[test]
fn transport0785_authored_native_return_is_not_replaced_by_transport() {
    let ast = exact(&sample());
    assert!(ast.iter().any(|s| matches!(s, Statement::ScheduledConsumer { body, .. }
        if matches!(body.as_slice(), [Statement::Return]))));
}
#[test]
fn transport0785_mixed_legacy_native_project_reexports_exactly() {
    exact(&graphs("label start\n\"native\"\nreturn\nlabel legacy\n\"legacy\"\n", &["start"]));
}
#[test]
fn transport0785_collision_suffix_is_only_generated_underscores() {
    let docs = graphs("label start\n\"native\"\nreturn\nlabel __blueprint_project_end\n\"occupied\"\nreturn\nlabel __blueprint_project_end_spoof\n", &["start"]);
    let ast = exact(&docs);
    assert!(matches!(ast.last(), Some(Statement::Label { name }) if name == "__blueprint_project_end_"));
    assert!(ast.iter().any(|s| matches!(s, Statement::Label { name } if name == "__blueprint_project_end_spoof")));
}
#[test]
fn transport0785_forged_noncanonical_suffix_is_refused() {
    let mut ast = transpile_project(&sample()).unwrap().ast;
    for s in &mut ast {
        match s {
            Statement::Jump { target } if target == "__blueprint_project_end" => *target = "__blueprint_project_end_fake".into(),
            Statement::Label { name } if name == "__blueprint_project_end" => *name = "__blueprint_project_end_fake".into(),
            _ => {}
        }
    }
    reject(&ast);
}
#[test]
fn transport0785_wrong_footer_target_is_not_a_default_or_new_node() {
    let mut ast = transpile_project(&sample()).unwrap().ast;
    let i = raw_footer(&ast);
    let Statement::Jump { target } = &mut ast[i] else { unreachable!() };
    *target = "other".into();
    reject(&ast);
}
#[test]
fn transport0785_jump_before_native_body_is_not_transport() {
    let mut ast = transpile_project(&sample()).unwrap().ast;
    let i = raw_footer(&ast);
    ast.swap(i-1, i);
    reject(&ast);
}
#[test]
fn transport0785_nonempty_final_sentinel_is_refused() {
    let mut ast = transpile_project(&sample()).unwrap().ast;
    ast.push(Statement::Return);
    reject(&ast);
}
#[test]
fn transport0785_duplicate_final_sentinel_is_refused() {
    let mut ast = transpile_project(&sample()).unwrap().ast;
    ast.insert(0, Statement::Label { name: "__blueprint_project_end".into() });
    reject(&ast);
}
#[test]
fn transport0785_extra_unowned_jump_cannot_hide_behind_valid_footer() {
    let mut ast = transpile_project(&sample()).unwrap().ast;
    let i = raw_footer(&ast);
    ast.insert(i, Statement::Jump { target: "other".into() });
    reject(&ast);
}
#[test]
fn transport0785_legacy_only_fallthrough_and_export_source_stay_exact() {
    let docs = graphs("label start\n\"first\"\nlabel other\n\"second\"\n", &[]);
    let emitted = transpile_project(&docs).unwrap();
    let expected = rvn_parser::parse("init {}\nlabel start\n\"first\"\njump other\nlabel other\n\"second\"\njump __blueprint_project_end\nlabel __blueprint_project_end\n").unwrap();
    assert_eq!(emitted.ast, expected);
    assert!(!emitted.source.contains("flow.consume"));
    let imported = import_script(&emitted.ast).unwrap();
    let again = transpile_project(&imported).unwrap();
    assert_eq!(again.ast, emitted.ast);
    assert_eq!(again.source, emitted.source);
}

fn sparse_ids() -> Vec<GraphDocument> {
    let mut docs = graphs("label start\n\"first\"\nreturn\nlabel other\n\"second\"\n", &[]);
    for graph in &mut docs {
        graph.graph_id = match &graph.kind {
            GraphKind::Init => GraphId::new(1),
            GraphKind::Label { name } if name == "start" => GraphId::new(4),
            GraphKind::Label { name } if name == "other" => GraphId::new(6),
            _ => panic!("unexpected fixture scope"),
        };
        if matches!(graph.kind, GraphKind::Label { .. }) {
            graph.enable_native_frame_preparation().unwrap();
        }
    }
    docs
}
#[test]
fn transport0788_sparse_authoritative_ids_keep_init_footer_neighbors_and_all_pins() {
    let docs = sparse_ids();
    let ast = exact(&docs);
    let imported = import_script(&ast).unwrap();
    let ids: std::collections::BTreeSet<_> = imported.iter().map(|g| g.graph_id).collect();
    assert_eq!(ids.len(), imported.len());
    assert!(imported.iter().any(|g| g.graph_id == GraphId::new(1) && g.kind == GraphKind::Init));
    let footer = imported.iter().find(|g| matches!(&g.kind, GraphKind::Label { name } if name == "__blueprint_project_end")).unwrap();
    assert!(footer.inspector_evaluation.is_legacy());
    assert_eq!(footer.graph_id, GraphId::new(7));
    for source in docs.iter().filter(|g| !g.inspector_evaluation.is_legacy()) {
        let actual = imported.iter().find(|g| g.graph_id == source.graph_id).unwrap();
        assert_eq!(actual.pins, source.pins);
        assert_eq!(actual.edges, source.edges);
        for (id, node) in &source.nodes {
            assert_eq!(actual.nodes[id].kind, node.kind);
            assert_eq!(actual.nodes[id].pins, node.pins);
        }
    }
}
#[test]
fn transport0788_actual_native_graph_id_collision_is_refused_not_remapped() {
    let mut ast = transpile_project(&sparse_ids()).unwrap().ast;
    let mut changed = 0;
    for statement in &mut ast {
        if let Statement::ScheduledConsumer { scope, .. } = statement {
            if let Some(tail) = scope.strip_prefix("g6/") {
                *scope = format!("g4/{tail}");
                changed += 1;
            }
        }
    }
    assert!(changed > 0);
    let before = ast.clone();
    assert!(import_script(&ast).unwrap_err().contains("Source Native GraphId collision"));
    assert!(import_native_frame_script(&ast).unwrap_err().contains("Source Native GraphId collision"));
    assert_eq!(ast, before);
}
