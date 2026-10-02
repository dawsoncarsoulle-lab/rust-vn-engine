use rvn_graph::*;
use rvn_core::{Engine, Interaction, TerminalRenderer, random::RandomState};
use rvn_parser::{parse, Value};

fn pin(graph: &GraphDocument, node: NodeId, key: &str) -> PinId {
    graph.pin_by_key(node, key).unwrap().id
}

fn connect(graph: &mut GraphDocument, from: NodeId, output: &str, to: NodeId, input: &str) {
    graph.connect(pin(graph, from, output), pin(graph, to, input)).unwrap();
}

fn local_graph(source: &str) -> GraphDocument {
    import_script(&parse(source).unwrap()).unwrap().into_iter()
        .find(|graph| matches!(graph.kind, GraphKind::Function { .. })).unwrap()
}

fn omit_old_output(graph: &mut GraphDocument, node: NodeId) {
    let output = pin(graph, node, "value_out");
    assert!(!graph.edges.values().any(|edge| edge.output == output));
    graph.pins.remove(&output);
    graph.nodes.get_mut(&node).unwrap().pins.retain(|id| *id != output);
}

#[test]
fn local_and_project_set_catalogs_expose_the_same_assigned_value_socket() {
    for kind in [NodeKind::SetVariable, NodeKind::LocalVariable] {
        let definition = node_definition(kind);
        let output = definition.pins.iter().find(|pin| pin.key == "value_out").unwrap();
        assert_eq!(output.direction, PinDirection::Output);
        assert_eq!(output.cardinality, PinCardinality::Many);
        assert_eq!(output.value_type, ValueType::Any);
        assert!(output.default_value.is_none());
    }
}

#[test]
fn local_set_output_uses_the_input_value_type_without_borrowing_a_consumers_type() {
    for (value, ty) in [(PropertyValue::Bool(true), ValueType::Bool),
        (PropertyValue::Int(7), ValueType::Int), (PropertyValue::Float(1.25), ValueType::Float),
        (PropertyValue::String("Été".into()), ValueType::String)] {
        let mut graph = GraphDocument::new(GraphId::new(1), GraphKind::Function { name: "read".into() });
        let local = graph.add_catalog_node(NodeKind::LocalVariable, [0.0, 0.0]).unwrap();
        graph.set_property(local, "name", PropertyValue::String("value".into())).unwrap();
        graph.set_pin_default(local, "value", value).unwrap();
        let before = graph.clone();
        assert_eq!(graph.effective_pin_type(pin(&graph, local, "value_out")), Some(ty));
        assert_eq!(graph, before, "Scope and type projection must not mutate the graph");
    }
    let mut graph = local_graph("function read(){local text=string_to_text(\"Hello\") return text}");
    let local = graph.nodes.values().find(|node| node.kind == NodeKind::LocalVariable).unwrap().id;
    assert_eq!(graph.effective_pin_type(pin(&graph, local, "value_out")), Some(ValueType::InterpolatedText));
    graph.set_property(local, "name", PropertyValue::String("unknown".into())).unwrap();
    let value = pin(&graph, local, "value");
    graph.edges.retain(|_, edge| edge.input != value);
    graph.pins.get_mut(&value).unwrap().default_value = None;
    graph.pins.get_mut(&value).unwrap().value_type = ValueType::Any;
    let output = pin(&graph, local, "value_out");
    graph.pins.get_mut(&output).unwrap().value_type = ValueType::Any;
    assert_eq!(graph.effective_pin_type(pin(&graph, local, "value_out")), Some(ValueType::Any));
}

#[test]
fn append_local_value_output_emits_a_real_local_read_and_preserves_source_roundtrip() {
    let mut graph = local_graph("function caption(){local result=\"Iris\" + \" Vale\" return result}");
    let local = graph.nodes.values().find(|node| node.kind == NodeKind::LocalVariable).unwrap().id;
    let ret = graph.nodes.values().find(|node| node.kind == NodeKind::FunctionReturn).unwrap().id;
    let input = pin(&graph, local, "value");
    let return_input = pin(&graph, ret, "value");
    graph.edges.retain(|_, edge| edge.input != input && edge.input != return_input);
    let append = graph.add_catalog_node(NodeKind::StringAppend, [90.0, 180.0]).unwrap();
    graph.set_pin_default(append, "left", PropertyValue::String("Iris".into())).unwrap();
    graph.set_pin_default(append, "right", PropertyValue::String(" Vale".into())).unwrap();
    connect(&mut graph, append, "value", local, "value");
    connect(&mut graph, local, "value_out", ret, "value");
    assert_eq!(graph.effective_pin_type(pin(&graph, local, "value_out")), Some(ValueType::String));
    let compiled = transpile(&graph).unwrap();
    assert!(compiled.source.contains("local result = (\"Iris\" + \" Vale\")"));
    assert!(compiled.source.contains("return result"));
    assert!(!compiled.source.contains("set result"));
    assert_eq!(transpile_value_output(&graph, local, "value_out").unwrap(), "result");
    let reopened = reimport_script(&compiled.ast, &[graph.clone()]).unwrap();
    let retained = reopened.iter().find(|doc| doc.kind == graph.kind).unwrap();
    assert_eq!(retained, &graph, "Visible output, wires, Append and positions survive reopening");
}

#[test]
fn local_output_fanout_reads_the_assigned_random_value_without_re_evaluation_or_global_leak() {
    let source = "function sample(){local roll=random(1,1000000) return [roll,roll]}\nlabel start\nset pair=sample()\n\"Result [pair]\"\n";
    let script = parse(source).unwrap();
    let mut graphs = import_script(&script).unwrap();
    let graph = graphs.iter_mut().find(|graph| matches!(graph.kind, GraphKind::Function { .. })).unwrap();
    let local = graph.nodes.values().find(|node| node.kind == NodeKind::LocalVariable).unwrap().id;
    let readers: Vec<_> = graph.nodes.values().filter(|node| node.kind == NodeKind::VariableGet
        && node.properties.get("name") == Some(&PropertyValue::String("roll".into())))
        .map(|node| node.id).collect();
    let output = pin(graph, local, "value_out");
    for reader in readers {
        let old_output = pin(graph, reader, "value");
        for edge in graph.edges.values_mut().filter(|edge| edge.output == old_output) { edge.output = output; }
        graph.remove_node(reader).unwrap();
    }
    let compiled = transpile_project(&graphs).unwrap();
    assert_eq!(compiled.source.matches("random(").count(), 1);
    let mut original = Engine::new(script, TerminalRenderer, 16).unwrap();
    let mut generated = Engine::new(compiled.ast, TerminalRenderer, 16).unwrap();
    for game in [&mut original, &mut generated] {
        game.state.random = RandomState::seeded(29);
        assert!(matches!(game.step_until_interaction().unwrap(), Some(Interaction::Dialogue { .. })));
        let Value::List(pair) = &game.state.vars["pair"] else { panic!("Expected returned pair") };
        assert_eq!(pair[0], pair[1]);
        assert!(!game.state.vars.contains_key("roll"));
    }
    assert_eq!(generated.state.vars["pair"], original.state.vars["pair"]);
    assert_eq!(generated.state.random, original.state.random);
}

#[test]
fn old_schema_current_local_set_cache_is_upgraded_once_without_touching_existing_data() {
    let mut old = local_graph("function read(){local width=7 return width}");
    let local = old.nodes.values().find(|node| node.kind == NodeKind::LocalVariable).unwrap().id;
    old.nodes.get_mut(&local).unwrap().position = [345.5, -87.25];
    omit_old_output(&mut old, local);
    let source = transpile(&old).unwrap().source;
    let mut raw = serde_json::to_value(&old).unwrap();
    raw["next_pin_id"] = serde_json::json!(0);
    let (upgraded, report) = GraphDocument::from_json_with_report(&raw.to_string()).unwrap();
    assert_eq!(report.from, GRAPH_SCHEMA_VERSION);
    assert_eq!(report.to, GRAPH_SCHEMA_VERSION);
    assert_eq!(report.steps, ["assignment_value_outputs"]);
    assert_eq!(upgraded.edges, old.edges);
    assert_eq!(upgraded.variables, old.variables);
    for (id, model) in &old.pins { assert_eq!(&upgraded.pins[id], model); }
    for (id, model) in &old.nodes {
        let mut expected = model.clone();
        if *id == local { expected.pins.push(pin(&upgraded, local, "value_out")); }
        assert_eq!(upgraded.nodes[id], expected);
    }
    assert_eq!(transpile(&upgraded).unwrap().source, source);
    assert_eq!(upgraded.effective_pin_type(pin(&upgraded, local, "value_out")), Some(ValueType::Int));
    let (again, report) = GraphDocument::from_json_with_report(&upgraded.to_pretty_json().unwrap()).unwrap();
    assert_eq!(again, upgraded);
    assert!(report.steps.is_empty());
}

#[test]
fn local_inline_name_and_value_stay_inside_set_instead_of_creating_hidden_identity_literals() {
    let mut graph = GraphDocument::new(GraphId::new(1), GraphKind::Function { name: "read".into() });
    let local = graph.add_catalog_node(NodeKind::LocalVariable, [0.0, 0.0]).unwrap();
    graph.set_property(local, "name", PropertyValue::String("caption".into())).unwrap();
    graph.set_pin_default(local, "name", PropertyValue::String("caption".into())).unwrap();
    graph.set_pin_default(local, "value", PropertyValue::String("Hello".into())).unwrap();
    let before = graph.clone();
    assert_eq!(graph.materialize_visible_defaults().unwrap(), 0);
    assert_eq!(graph, before);
}

#[test]
fn source_linked_legacy_cache_keeps_identity_layout_and_exact_authored_bytes() {
    let source = "// Preserve original comments\nfunction caption() {\n  local result = \"Iris\" // tail\n  return result\n}\nlabel start\nreturn\n";
    let project = SourceProject::open(source, &[]).unwrap();
    let mut old = project.graphs().to_vec();
    let graph = old.iter_mut().find(|graph| matches!(graph.kind, GraphKind::Function { .. })).unwrap();
    let local = graph.nodes.values().find(|node| node.kind == NodeKind::LocalVariable).unwrap().id;
    graph.nodes.get_mut(&local).unwrap().position = [412.0, 228.0];
    omit_old_output(graph, local);
    let previous = old.clone();
    let reopened = SourceProject::open(source, &old).unwrap();
    assert_eq!(old, previous, "Read-only reopen never mutates the supplied cache");
    assert_eq!(reopened.source(), source);
    let graph = reopened.graphs().iter().find(|graph| matches!(graph.kind, GraphKind::Function { .. })).unwrap();
    assert_eq!(graph.nodes[&local].position, [412.0, 228.0]);
    assert_eq!(graph.graph_id, previous.iter().find(|old| old.kind == graph.kind).unwrap().graph_id);
    assert!(graph.pin_by_key(local, "value_out").is_some());
    assert_eq!(graph.node_variable_scope(local), Some(VariableScope::Local));
}

#[test]
fn local_set_rename_updates_real_identity_and_keeps_assigned_output_type() {
    let mut graph = local_graph("function read(){local width=7 return width}");
    let local = graph.nodes.values().find(|node| node.kind == NodeKind::LocalVariable).unwrap().id;
    graph.rename_variable("width", "height").unwrap();
    assert_eq!(graph.pin_by_key(local, "name").unwrap().default_value, Some(PropertyValue::String("height".into())));
    assert_eq!(transpile_value_output(&graph, local, "value_out").unwrap(), "height");
    let compiled = transpile(&graph).unwrap().source;
    assert!(compiled.contains("local height = 7"));
    assert!(compiled.contains("return height"));
    assert_eq!(graph.effective_pin_type(pin(&graph, local, "value_out")), Some(ValueType::Int));
}

struct TempWorkspace(std::path::PathBuf);
impl TempWorkspace {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!("rvn-local-set-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("story.rvn"), source).unwrap();
        Self(path)
    }
}
impl Drop for TempWorkspace {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

#[test]
fn assigned_text_output_survives_source_amend_save_reopen_and_retains_text_semantics() {
    let source = "// Keep my note\nfunction caption(){local text=string_to_text(\"Hello [missing]\") return text}\nlabel start\n\"[caption()]\"\nreturn\n";
    let temp = TempWorkspace::new(source);
    let mut workspace = SourceWorkspace::link(&temp.0, &temp.0.join("story.rvn"), &[]).unwrap();
    let mut graphs = workspace.project().graphs().to_vec();
    let graph = graphs.iter_mut().find(|graph| matches!(graph.kind, GraphKind::Function { .. })).unwrap();
    let local = graph.nodes.values().find(|node| node.kind == NodeKind::LocalVariable).unwrap().id;
    let reader = graph.nodes.values().find(|node| node.kind == NodeKind::VariableGet
        && node.properties.get("name") == Some(&PropertyValue::String("text".into()))).unwrap().id;
    let read_output = pin(graph, reader, "value");
    let assigned = pin(graph, local, "value_out");
    for edge in graph.edges.values_mut().filter(|edge| edge.output == read_output) { edge.output = assigned; }
    graph.remove_node(reader).unwrap();
    graph.nodes.get_mut(&local).unwrap().position = [412.0, 228.0];
    // Amend the authored expression as well, exercising the source-first save
    // rather than only the presentation fast path.
    let literal = graph.nodes.values().find(|node| node.kind == NodeKind::Literal
        && node.properties.get("value") == Some(&PropertyValue::String("Hello [missing]".into()))).unwrap().id;
    graph.set_property(literal, "value", PropertyValue::String("Été [missing]".into())).unwrap();
    workspace.save(&graphs).unwrap();
    let saved = std::fs::read_to_string(temp.0.join("story.rvn")).unwrap();
    assert!(saved.starts_with("// Keep my note\n"));
    assert!(saved.contains("string_to_text(\"Été [missing]\")"));
    assert!(!saved.contains("set text"));
    let reopened = SourceWorkspace::open(&temp.0).unwrap().unwrap();
    let graph = reopened.project().graphs().iter().find(|graph| matches!(graph.kind, GraphKind::Function { .. })).unwrap();
    assert_eq!(graph.nodes[&local].position, [412.0, 228.0]);
    assert_eq!(graph.effective_pin_type(pin(graph, local, "value_out")), Some(ValueType::InterpolatedText));
    assert!(graph.edges.values().any(|edge| edge.output == assigned));
    assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(), *graph);
    let mut original = Engine::new(parse(&saved).unwrap(), TerminalRenderer, 16).unwrap();
    let mut generated = Engine::new(transpile_project(reopened.project().graphs()).unwrap().ast, TerminalRenderer, 16).unwrap();
    assert_eq!(generated.step_until_interaction().unwrap(), original.step_until_interaction().unwrap());
    assert!(!generated.state.vars.contains_key("text"));
}

#[test]
fn legacy_recovery_adds_local_set_output_in_memory_without_rewriting_original_cache() {
    let source = "function read(){local width=7 return width}\nlabel start\nreturn\n";
    let temp = TempWorkspace::new(source);
    let mut workspace = SourceWorkspace::link(&temp.0, &temp.0.join("story.rvn"), &[]).unwrap();
    let mut legacy = workspace.project().graphs().to_vec();
    let graph = legacy.iter_mut().find(|graph| matches!(graph.kind, GraphKind::Function { .. })).unwrap();
    let local = graph.nodes.values().find(|node| node.kind == NodeKind::LocalVariable).unwrap().id;
    omit_old_output(graph, local);
    workspace.write_recovery(&legacy).unwrap();
    let cache = temp.0.join(".rvn-authoring.recovery.json");
    let before = std::fs::read(&cache).unwrap();
    let recovered = workspace.recovery().unwrap().unwrap();
    let graph = recovered.iter().find(|graph| matches!(graph.kind, GraphKind::Function { .. })).unwrap();
    assert!(graph.pin_by_key(local, "value_out").is_some());
    assert_eq!(std::fs::read(cache).unwrap(), before);
    assert_eq!(std::fs::read_to_string(temp.0.join("story.rvn")).unwrap(), source);
}
