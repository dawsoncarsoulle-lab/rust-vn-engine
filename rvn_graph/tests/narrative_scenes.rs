use rvn_graph::*;
use rvn_parser::{NarrativeContext, Statement};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

const SOURCE: &str = "// scène été 🍃\nfunction points(n) { local result = n + 1 return result }\nscreen panel() { local title = \"Local\" return title }\nhandler clicked(event) { local seen = 0 set seen = 1 }\ninit { set score = 0 }\nlabel helper\n local helper_count = 2 // préserver\n return\nscene label start\n local visits = 1 // visite\n call helper\n set score = points(score)\n jump next\nscene label next\n \"Suite\"\n return\n";

fn parsed(source: &str) -> rvn_parser::Script { rvn_parser::parse_program(source).unwrap().script }
fn label<'a>(graphs: &'a [GraphDocument], name: &str) -> &'a GraphDocument {
    graphs.iter().find(|graph| matches!(&graph.kind, GraphKind::Label {name: found} if found == name)).unwrap()
}
fn change_local(graphs: &mut [GraphDocument], graph_name: &str, binding: &str, value: i64) {
    let graph = graphs.iter_mut().find(|graph| matches!(&graph.kind, GraphKind::Label {name} if name == graph_name)).unwrap();
    let node = graph.nodes.values().find(|node| node.kind == NodeKind::LocalVariable && node.properties.get("name") == Some(&PropertyValue::String(binding.into()))).unwrap().id;
    let input = graph.pin_by_key(node, "value").unwrap();
    assert!(matches!(input.default_value, Some(PropertyValue::Int(_))));
    assert!(!graph.edges.values().any(|edge| edge.input == input.id), "authored integer is an unconnected pin default");
    graph.set_pin_default(node, "value", PropertyValue::Int(value)).unwrap();
}

#[test]
fn legacy_v5_default_and_v4_migration_never_gain_scene_metadata_or_empty_v6() {
    let legacy = import_script(&rvn_parser::parse("label start\nscene forest\nreturn\n").unwrap()).unwrap();
    for graph in &legacy {
        assert_eq!(graph.schema_version, 5);
        assert_eq!(graph.narrative, None);
        let json = serde_json::to_string(graph).unwrap();
        assert!(!json.contains("narrative"));
        let (loaded, report) = GraphDocument::from_json_with_report(&json).unwrap();
        assert_eq!(loaded, *graph);
        assert_eq!((report.from, report.to), (5, 5));
        assert!(report.steps.is_empty());
    }
    let mut old = serde_json::to_value(label(&legacy, "start")).unwrap();
    old["schema_version"] = 4.into();
    let (loaded, report) = GraphDocument::from_json_with_report(&old.to_string()).unwrap();
    assert_eq!((report.from, report.to), (4, 5));
    assert_eq!(report.steps, ["v4_to_v5_programmable_canvas"]);
    assert_eq!(loaded.narrative, None);
    assert!(matches!(transpile(&loaded).unwrap().ast[0], Statement::Label {..}));
}

#[test]
fn metadata_v6_roundtrip_is_explicit_validated_and_identity_preserving() {
    let graphs = import_script(&parsed(SOURCE)).unwrap();
    let start = label(&graphs, "start");
    let (loaded, report) = GraphDocument::from_json_with_report(&serde_json::to_string(start).unwrap()).unwrap();
    assert_eq!(loaded, *start);
    assert_eq!((report.from, report.to), (6, 6));
    assert!(report.steps.is_empty());
    let mut legacy = start.clone();
    legacy.set_narrative_metadata(None).unwrap();
    assert_eq!(legacy.schema_version, 5);
    assert_eq!((legacy.graph_id, &legacy.nodes, &legacy.pins, &legacy.edges), (start.graph_id, &start.nodes, &start.pins, &start.edges));
    for (version, narrative) in [(6, serde_json::Value::Null), (5, serde_json::json!("scene_entry")), (7, serde_json::json!("scene_entry"))] {
        let mut value = serde_json::to_value(start).unwrap();
        value["schema_version"] = version.into(); value["narrative"] = narrative;
        assert!(GraphDocument::from_json(&value.to_string()).is_err());
    }
    let mut init = graphs.iter().find(|graph| graph.kind == GraphKind::Init).unwrap().clone();
    let before = init.clone();
    assert!(init.set_narrative_metadata(Some(NarrativeMetadata::SceneEntry)).is_err());
    assert_eq!(init, before);
}

#[test]
fn two_scene_entries_and_contextual_helpers_keep_existing_label_catalog_and_lexical_scopes() {
    let graphs = import_script(&parsed(SOURCE)).unwrap();
    assert_eq!(label(&graphs, "start").narrative, Some(NarrativeMetadata::SceneEntry));
    assert_eq!(label(&graphs, "next").narrative, Some(NarrativeMetadata::SceneEntry));
    assert_eq!(label(&graphs, "helper").narrative, Some(NarrativeMetadata::SceneFragment));
    assert!(graphs.iter().filter(|graph| !matches!(graph.kind, GraphKind::Label {..})).all(|graph| graph.narrative.is_none() && graph.schema_version == 5));
    assert!(label(&graphs, "start").nodes.values().any(|node| node.kind == NodeKind::Label));
    let compiled = transpile_project(&graphs).unwrap();
    assert_eq!(compiled.source.matches("scene label ").count(), 2);
    assert!(compiled.source.contains("label helper\n"));
    assert!(matches!(compiled.ast.last(), Some(Statement::Label {name}) if name.starts_with("__blueprint_project_end")));
    assert_eq!(label(&graphs, "start").variable_scope("visits"), Some(VariableScope::Local));
    assert_eq!(label(&graphs, "helper").variable_scope("helper_count"), Some(VariableScope::Local));
    assert_eq!(label(&graphs, "next").variable_scope("visits"), None);
    assert_eq!(label(&graphs, "next").variable_scope("score"), Some(VariableScope::Global));
}

#[test]
fn narrative_for_binding_stays_global_unless_authored_local_declares_it() {
    let body = "for item in [1,2] { set score = item }\nreturn\n";
    let legacy = import_script(&parsed(&format!("label start\n{body}"))).unwrap();
    let modern = import_script(&parsed(&format!("scene label start\n{body}"))).unwrap();
    assert_eq!(label(&legacy, "start").variable_scope("item"), Some(VariableScope::Global));
    assert_eq!(label(&modern, "start").variable_scope("item"), Some(VariableScope::Global));
    assert_eq!(label(&modern, "start").variable_scope("score"), Some(VariableScope::Global));
    assert!(transpile_project(&modern).unwrap().source.contains("for item in"));
    let declared = import_script(&parsed(&format!("scene label start\nlocal item = 0\n{body}"))).unwrap();
    assert_eq!(label(&declared, "start").variable_scope("item"), Some(VariableScope::Local));
    assert!(transpile_project(&declared).unwrap().source.contains("local item = 0"));
}

#[test]
fn scene_local_shadow_does_not_poison_project_global_type_in_other_scene() {
    let graphs = import_script(&parsed("init { set score = 0 }\nscene label start\nlocal score = \"Local\"\nreturn\nscene label next\nset score = 2\nreturn\n")).unwrap();
    assert_eq!(label(&graphs, "start").variable_scope("score"), Some(VariableScope::Local));
    assert_eq!(label(&graphs, "start").variables["score"].value_type, ValueType::String);
    assert_eq!(label(&graphs, "next").variable_scope("score"), Some(VariableScope::Global));
    assert_eq!(label(&graphs, "next").variables["score"].value_type, ValueType::Int);
    assert!(graphs.iter().all(|graph| !graph.validate().iter().any(|diagnostic| diagnostic.code == "variable_type_conflict")));
}

#[test]
fn reimport_promotes_only_explicit_semantics_and_keeps_all_previous_ids_and_layout() {
    let legacy = "label start\n\"Hello\"\nreturn\nlabel helper\nreturn\n";
    let mut previous = import_script(&parsed(legacy)).unwrap();
    for graph in &mut previous { for node in graph.nodes.values_mut() { node.position = [432.0, 128.0]; } }
    let modern = legacy.replacen("label start", "scene label start", 1);
    let next = reimport_script(&parsed(&modern), &previous).unwrap();
    for old in &previous {
        let new = next.iter().find(|graph| graph.graph_id == old.graph_id).unwrap();
        assert_eq!((&old.nodes, &old.pins, &old.edges), (&new.nodes, &new.pins, &new.edges));
        if matches!(old.kind, GraphKind::Label {..}) { assert_eq!(new.schema_version, 6); }
        else { assert_eq!(new.schema_version, 5); }
    }
    let back = reimport_script(&parsed(legacy), &next).unwrap();
    assert_eq!(back, previous);
}

#[test]
fn visual_scene_and_helper_edits_preserve_utf8_trivia_other_scopes_and_ids() {
    let mut project = SourceProject::open(SOURCE, &[]).unwrap();
    let before = project.graphs().to_vec();
    let mut edited = before.clone();
    change_local(&mut edited, "helper", "helper_count", 4);
    change_local(&mut edited, "start", "visits", 3);
    project.apply_visual(SOURCE, &edited).unwrap();
    assert_eq!(project.source(), SOURCE.replace("helper_count = 2", "helper_count = 4").replace("visits = 1", "visits = 3"));
    assert_eq!(project.narrative_context(), NarrativeContext::ExplicitScenes);
    assert_eq!(label(project.graphs(), "helper").variables["helper_count"].default_value, PropertyValue::Int(4));
    assert_eq!(label(project.graphs(), "start").variables["visits"].default_value, PropertyValue::Int(3));
    for submitted in &edited {
        let saved = project.graphs().iter().find(|graph| graph.graph_id == submitted.graph_id).unwrap();
        assert_eq!((&saved.nodes, &saved.pins, &saved.edges), (&submitted.nodes, &submitted.pins, &submitted.edges));
    }
    let reopened = SourceProject::open(project.source(), project.graphs()).unwrap();
    assert_eq!(reopened.graphs(), project.graphs());
    assert_eq!(label(reopened.graphs(), "next"), label(&before, "next"));
    assert_eq!(label(reopened.graphs(), "start").graph_id, label(&before, "start").graph_id);
}

#[test]
fn removing_last_scene_cannot_publish_unactivated_helper_locals_or_mutate_baseline() {
    let source = "scene label start\nlocal visits = 1\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let before = project.graphs().to_vec();
    let mut edited = before.clone();
    edited.iter_mut().find(|graph| matches!(graph.kind, GraphKind::Label {..})).unwrap()
        .set_narrative_metadata(Some(NarrativeMetadata::SceneFragment)).unwrap();
    assert!(project.apply_visual(source, &edited).is_err());
    assert_eq!(project.source(), source);
    assert_eq!(project.graphs(), before);
}

#[test]
fn scene_entry_rename_preserves_metadata_history_identity_and_authored_comment() {
    let source = "// entrée à conserver\nscene label start // en-tête\nlocal visits = 1\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut edited = project.graphs().to_vec();
    let graph = edited.iter_mut().find(|graph| matches!(graph.kind, GraphKind::Label {..})).unwrap();
    let id = graph.graph_id;
    let nodes = graph.nodes.clone();
    graph.kind = GraphKind::Label { name: "intro".into() };
    project.apply_visual(source, &edited).unwrap();
    assert_eq!(project.source(), source.replacen("scene label start", "scene label intro", 1));
    let renamed = label(project.graphs(), "intro");
    assert_eq!((renamed.graph_id, renamed.narrative, &renamed.nodes), (id, Some(NarrativeMetadata::SceneEntry), &nodes));
    let previous = SourceProject::open(source, project.graphs()).unwrap();
    assert_eq!(previous.narrative_context(), NarrativeContext::ExplicitScenes);
    assert!(project.resolved_program().unwrap().script.iter().any(|statement| matches!(statement, Statement::SceneLabel {name} if name == "intro")));
}

#[test]
fn contextual_choice_condition_refactor_retains_scene_and_helper_metadata() {
    let source = "function eligible(n) { return n > 0 }\nscene label start\nlocal visits = 1\nchoice { \"Continue\" if eligible(visits) => { jump helper } }\nlabel helper\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let graphs = project.graphs().to_vec();
    let id = graphs.iter().find(|graph| matches!(&graph.kind, GraphKind::Function {name} if name == "eligible")).unwrap().graph_id;
    let next = rename_callable_graphs(&graphs, id, "allowed").unwrap();
    project.apply_visual(source, &next).unwrap();
    assert!(project.source().contains("if allowed(visits)"));
    assert_eq!(label(project.graphs(), "start").narrative, Some(NarrativeMetadata::SceneEntry));
    assert_eq!(label(project.graphs(), "helper").narrative, Some(NarrativeMetadata::SceneFragment));
}

#[test]
fn generated_statement_origins_use_real_nodes_and_distinguish_repeated_branch_statements() {
    let source = "scene label start\nif true { set score = 1 } else { set score = 1 }\n\"Hello\"\nreturn\n";
    let graphs = import_script(&parsed(source)).unwrap();
    let graph = label(&graphs, "start");
    let compiled = transpile_with_origins(graph).unwrap();
    assert_eq!(compiled.ast.len(), compiled.origins.len());
    let branch_index = compiled.ast.iter().position(|statement| matches!(statement, Statement::If {..})).unwrap();
    let branch = &compiled.origins[branch_index];
    assert_eq!(graph.nodes[&branch.node.unwrap()].kind, NodeKind::If);
    assert_eq!(branch.children.len(), 2);
    assert_ne!(branch.children[0].node, branch.children[1].node);
    assert_ne!(branch.children[0].span.range, branch.children[1].span.range);
    for child in &branch.children {
        assert_eq!(child.graph, Some(graph.graph_id));
        assert_eq!(graph.nodes[&child.node.unwrap()].kind, NodeKind::SetVariable);
        assert!(compiled.source[child.span.range.clone()].starts_with("set score ="));
    }
    let project = transpile_project_with_origins(&graphs).unwrap();
    let sentinel = project.origins.last().unwrap();
    assert_eq!((sentinel.graph, sentinel.node), (None, None));
    assert!(project.source[sentinel.span.range.clone()].starts_with("label __blueprint_project_end"));
    assert_eq!(project.source, transpile_project(&graphs).unwrap().source);
}

#[test]
fn generated_choice_origins_keep_occurrence_nodes_and_registry_declarations_use_span_fallback() {
    let graphs = import_script(&parsed("init { character.create(\"iris\",\"Iris\") }\nscene label start\nchoice { \"A\" => { \"same\" } \"B\" => { \"same\" } }\nreturn\n")).unwrap();
    let graph = label(&graphs, "start");
    let compiled = transpile_with_origins(graph).unwrap();
    let init = compiled.ast.iter().position(|statement| matches!(statement, Statement::Init {..})).unwrap();
    assert_eq!((compiled.origins[init].graph, compiled.origins[init].node), (None, None));
    assert_eq!((compiled.origins[init].children[0].graph, compiled.origins[init].children[0].node), (None, None));
    let choice = compiled.ast.iter().position(|statement| matches!(statement, Statement::Choice {..})).unwrap();
    let children = &compiled.origins[choice].children;
    assert_eq!(children.len(), 2);
    assert_ne!(children[0].node, children[1].node);
    for child in children {
        assert_eq!(graph.nodes[&child.node.unwrap()].kind, NodeKind::Dialogue);
        assert_eq!(child.graph, Some(graph.graph_id));
        assert_eq!(&compiled.source[child.span.range.clone()], "\"same\"");
    }
}

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("rvn-c2-scènes-{}-{nonce}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
    fn write(&self, path: &str, source: &str) -> PathBuf { let path = self.0.join(path); fs::write(&path, source).unwrap(); path }
}
impl Drop for Files { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

#[test]
fn source_imported_scene_context_authorizes_owned_helper_but_keeps_library_readonly_and_origin_real() {
    let files = Files::new();
    let library_source = "scene label entry\ncall helper\n";
    let library = files.write("library.rvn", library_source);
    let source = "use \"library.rvn\"\n// helper été\nlabel helper\nlocal visits = 1 // tail\nreturn\n";
    let path = files.write("story.rvn", source);
    let mut project = SourceProject::open_at(&path, source, &[]).unwrap();
    assert!(project.graphs().iter().all(|graph| !matches!(&graph.kind, GraphKind::Label {name} if name == "entry")));
    assert_eq!(label(project.graphs(), "helper").narrative, Some(NarrativeMetadata::SceneFragment));
    let program = project.resolved_program().unwrap();
    let index = program.script.iter().position(|statement| matches!(statement, Statement::SceneLabel {..})).unwrap();
    assert_eq!(program.origins[index].source, Some(library.canonicalize().unwrap()));
    assert_eq!(&library_source[program.origins[index].span.range.clone()], "scene label entry");
    let mut edited = project.graphs().to_vec(); change_local(&mut edited, "helper", "visits", 7);
    project.apply_visual(source, &edited).unwrap();
    assert_eq!(project.source(), source.replace("visits = 1", "visits = 7"));
    assert_eq!(fs::read_to_string(library).unwrap(), library_source);
    assert_eq!(fs::read_to_string(path).unwrap(), source);
}

#[test]
fn changing_frozen_import_refuses_scene_save_trial_without_overwriting_source_or_graphs() {
    let files = Files::new();
    let library = files.write("library.rvn", "label helper\nlocal n = 1\nreturn\n");
    let source = "use \"library.rvn\"\nscene label start\ncall helper\nreturn\n";
    let path = files.write("story.rvn", source);
    let mut project = SourceProject::open_at(&path, source, &[]).unwrap();
    let before = project.graphs().to_vec();
    fs::write(&library, "label helper\nlocal n = 2\nreturn\n").unwrap();
    assert!(project.apply_visual(source, &before).unwrap_err().contains("changed externally"));
    assert_eq!(project.graphs(), before); assert_eq!(project.source(), source);
    assert_eq!(fs::read_to_string(path).unwrap(), source);
    assert!(fs::read_to_string(library).unwrap().contains("n = 2"));
}

#[test]
fn source_workspace_v6_save_reopen_recovery_and_callable_refactor_keep_scene_metadata() {
    let files = Files::new(); let path = files.write("story.rvn", SOURCE);
    let mut workspace = SourceWorkspace::link(&files.0, &path, &[]).unwrap();
    let before_sidecar = fs::read(files.0.join(".rvn-authoring.json")).unwrap();
    let mut edited = workspace.project().graphs().to_vec(); change_local(&mut edited, "start", "visits", 9);
    workspace.write_recovery(&edited).unwrap();
    let recovered = workspace.recovery().unwrap().unwrap();
    assert_eq!(recovered, edited);
    assert_eq!(fs::read_to_string(&path).unwrap(), SOURCE);
    assert_eq!(fs::read(files.0.join(".rvn-authoring.json")).unwrap(), before_sidecar);
    workspace.save(&recovered).unwrap();
    assert_eq!(label(workspace.project().graphs(), "start").variables["visits"].default_value, PropertyValue::Int(9));
    assert_eq!(label(workspace.project().graphs(), "start").nodes, label(&recovered, "start").nodes);
    assert_eq!(label(workspace.project().graphs(), "start").pins, label(&recovered, "start").pins);
    assert_eq!(label(workspace.project().graphs(), "start").edges, label(&recovered, "start").edges);
    let mut reopened = SourceWorkspace::open(&files.0).unwrap().unwrap();
    assert_eq!(reopened.project().graphs(), workspace.project().graphs());
    let id = reopened.project().graphs().iter().find(|graph| matches!(&graph.kind, GraphKind::Function {name} if name == "points")).unwrap().graph_id;
    let before_source = fs::read(&path).unwrap();
    let before_graphs = reopened.project().graphs().to_vec();
    let graphs = rename_callable_graphs(&before_graphs, id, "reward").unwrap();
    let receipt = reopened.refactor(&graphs, &[]).unwrap();
    assert!(reopened.project().source().contains("function reward"));
    assert_eq!(label(reopened.project().graphs(), "start").narrative, Some(NarrativeMetadata::SceneEntry));
    reopened.replay_refactor(&receipt, false).unwrap();
    assert_eq!(fs::read(path).unwrap(), before_source);
    assert_eq!(reopened.project().graphs(), before_graphs);
}

#[test]
fn invalid_v6_recovery_metadata_is_refused_without_normalizing_or_erasing_cache() {
    let files = Files::new(); let path = files.write("story.rvn", SOURCE);
    let mut workspace = SourceWorkspace::link(&files.0, &path, &[]).unwrap();
    let sidecar = fs::read(files.0.join(".rvn-authoring.json")).unwrap();
    let graphs = workspace.project().graphs().to_vec();
    workspace.write_recovery(&graphs).unwrap();
    let cache = files.0.join(".rvn-authoring.recovery.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&cache).unwrap()).unwrap();
    let graph = value["graphs"].as_array_mut().unwrap().iter_mut().find(|graph| graph["narrative"] == "scene_entry").unwrap();
    graph["narrative"] = serde_json::Value::Null;
    let damaged = serde_json::to_vec(&value).unwrap(); fs::write(&cache, &damaged).unwrap();
    assert!(workspace.recovery().unwrap_err().contains("Schema 6"));
    assert_eq!(fs::read(cache).unwrap(), damaged);
    assert_eq!(fs::read_to_string(path).unwrap(), SOURCE);
    assert_eq!(fs::read(files.0.join(".rvn-authoring.json")).unwrap(), sidecar);
}
