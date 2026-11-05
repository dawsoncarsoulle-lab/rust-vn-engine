use rvn_graph::*;

#[test]
fn binding_types_distinguish_global_bool_read_before_integer_local_shadow() {
    let source = "init {set value=true}\nscene label start\nif value {\"Before\"}\nlocal value=2\n\"[value]\"\nreturn\n";
    let project = SourceProject::open(source, &[]).expect("valid global and local shadow must import");
    let graph = project.graphs().iter().find(|graph|
        matches!(&graph.kind, GraphKind::Label {name} if name == "start")).unwrap();
    let branch = graph.nodes.values().find(|node| node.kind == NodeKind::If).unwrap();
    let condition = graph.pin_by_key(branch.id, "condition").unwrap().id;
    let edge = graph.edges.values().find(|edge| edge.input == condition).unwrap();
    assert_eq!(graph.effective_pin_type(edge.output), Some(ValueType::Bool));
    let binding = graph.variable_binding_projection("value").unwrap();
    assert_eq!(binding.input_contexts[&condition], VariableBindingContext::ProjectGlobal);
    let local = binding.local_declarations[0];
    assert_eq!(graph.effective_pin_type(graph.pin_by_key(local,"value_out").unwrap().id), Some(ValueType::Int));
    assert!(transpile(graph).is_ok());
}

#[test]
fn binding_types_join_conditional_and_zero_iteration_local_shadows() {
    for body in ["if false {local value=2}", "while false {local value=2}", "for item in [] {local value=2}"] {
        let source=format!("init {{set value=true}}\nscene label start\n{body}\nif value {{\"Maybe global\"}}\nreturn\n");
        let graph=scene(&source);
        let projection=graph.variable_binding_projection("value").unwrap();
        let mixed:Vec<_>=projection.read_contexts.iter().filter(|(_,context)|**context==VariableBindingContext::GlobalOrLocal).collect();
        assert!(!mixed.is_empty(),"{body}");
        for (pin,_) in mixed { assert_eq!(graph.effective_pin_type(*pin),Some(ValueType::Any),"{body}"); }
        assert!(transpile(&graph).is_ok());
    }
}

#[test]
fn binding_types_reimport_refreshes_domains_without_losing_existing_identities() {
    let source="init {set value=true}\nscene label start\nif value {\"Before\"}\nlocal value=2\n\"[value]\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();
    let previous=project.graphs().to_vec();
    let graph=previous.iter().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let projection=graph.variable_binding_projection("value").unwrap();
    let global_read=*projection.read_contexts.iter().find(|(_,context)|**context==VariableBindingContext::ProjectGlobal).unwrap().0;
    let local=projection.local_declarations[0];
    let mut changed=graph.clone();
    changed.variables.get_mut("value").unwrap().value_type=ValueType::Float;
    for (pin,context) in &projection.read_contexts {
        if *context==VariableBindingContext::ActivationLocal {
            changed.pins.get_mut(pin).unwrap().value_type=ValueType::Float;
        }
    }
    for id in changed.nodes[&local].pins.clone() {
        if matches!(changed.pins[&id].key.as_str(),"value"|"value_out") {
            changed.pins.get_mut(&id).unwrap().value_type=ValueType::Float;
        }
    }
    changed.set_pin_default(local,"value",PropertyValue::Float(2.5)).unwrap();
    let mut graphs=previous.clone();
    *graphs.iter_mut().find(|candidate|candidate.graph_id==graph.graph_id).unwrap()=changed;
    project.apply_visual(source,&graphs).unwrap();
    let after=project.graphs().iter().find(|candidate|candidate.graph_id==graph.graph_id).unwrap();
    assert_eq!(after.effective_pin_type(global_read),Some(ValueType::Bool));
    assert_eq!(after.variables["value"].value_type,ValueType::Float);
    assert_eq!(after.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
    assert_eq!(after.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());
    assert_eq!(after.edges,graph.edges);
    assert_eq!(GraphDocument::from_json(&after.to_pretty_json().unwrap()).unwrap(),*after);
    let reopened=SourceProject::open(project.source(),project.graphs()).unwrap();
    let cold=reopened.graphs().iter().find(|candidate|candidate.graph_id==graph.graph_id).unwrap();
    assert_eq!(cold.effective_pin_type(global_read),Some(ValueType::Bool));
    assert_eq!(cold.variables["value"].value_type,ValueType::Float);
    assert_eq!(cold.edges,graph.edges);
}

#[test]
fn binding_types_do_not_trust_a_source_cache_after_consumer_connections_change() {
    let mut graph=scene("init {set value=true}\nscene label start\nif value {\"Before\"}\nlocal value=2\n\"[value]\"\nreturn\n");
    let projection=graph.variable_binding_projection("value").unwrap();
    let output=*projection.read_contexts.iter().find(|(_,context)|**context==VariableBindingContext::ProjectGlobal).unwrap().0;
    assert_eq!(graph.effective_pin_type(output),Some(ValueType::Bool));
    let text=graph.nodes.values().find(|node|node.kind==NodeKind::Dialogue
        &&projection.statement_contexts.get(&node.id)==Some(&VariableBindingContext::ActivationLocal)).unwrap().id;
    let input=graph.pin_by_key(text,"text").unwrap().id;
    // Simulate a retained imported link; no authoring conversion is inserted.
    let edge=graph.edges.values().find(|edge|edge.input==input).unwrap().id;
    graph.edges.get_mut(&edge).unwrap().output=output;
    let before=graph.clone();
    assert_eq!(graph.effective_pin_type(output),Some(ValueType::Any));
    let contexts=graph.variable_binding_projection("value").unwrap();
    assert_eq!(contexts.read_contexts[&output],VariableBindingContext::GlobalOrLocal);
    assert_eq!(graph,before,"analysis may not repair or mutate caches");
}

fn scene(source: &str) -> GraphDocument {
    SourceProject::open(source, &[]).unwrap().graphs().iter()
        .find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start"))
        .unwrap().clone()
}

#[test]
fn binding_types_keep_global_domain_and_cached_nodes_after_scalar_local_value_edits() {
    let source="init {set value=true}\nscene label start\nif value {\"Before\"}\nlocal value=2\n\"[value]\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();
    let mut edited=project.graphs().to_vec();
    let graph=edited.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let projection=graph.variable_binding_projection("value").unwrap();
    let output=*projection.read_contexts.iter().find(|(_,context)|**context==VariableBindingContext::ProjectGlobal).unwrap().0;
    let local=projection.local_declarations[0];
    let nodes=graph.nodes.clone();
    graph.set_pin_default(local,"value",PropertyValue::Int(9)).unwrap();
    assert_eq!(graph.effective_pin_type(output),Some(ValueType::Bool));
    assert_eq!(graph.nodes,nodes,"a value edit never rewrites unrelated type caches");
    let graph_id=graph.graph_id;
    project.apply_visual(source,&edited).unwrap();
    let after=project.graphs().iter().find(|candidate|candidate.graph_id==graph_id).unwrap();
    assert_eq!(after.nodes,nodes);
    assert_eq!(after.effective_pin_type(output),Some(ValueType::Bool));
    assert_eq!(project.source(),source.replace("local value=2","local value=9"));
}

#[test]
fn binding_types_reject_malformed_or_mismatched_domain_cache_without_guessing_local_type() {
    let original=scene("init {set value=true}\nscene label start\nif value {\"Before\"}\nlocal value=2\nreturn\n");
    let projection=original.variable_binding_projection("value").unwrap();
    let output=*projection.read_contexts.iter().find(|(_,context)|**context==VariableBindingContext::ProjectGlobal).unwrap().0;
    let owner=original.pins[&output].node;
    for cache in [PropertyValue::Bool(true),PropertyValue::String("broken JSON".into()),
        PropertyValue::String(r#"{"name":"different","pins":{"value":"bool"},"stamp":null}"#.into())] {
        let mut graph=original.clone();
        graph.nodes.get_mut(&owner).unwrap().properties.insert("inferred_narrative_binding_types".into(),cache);
        let before=graph.clone();
        assert_eq!(graph.effective_pin_type(output),Some(ValueType::Any));
        assert_eq!(graph,before);
    }
}

#[test]
fn binding_projection_initializer_reads_global_then_writes_local_without_mutation() {
    let graph = scene("init {set score=10}\nscene label start\nset score=score+1\nlocal score=score+2\nset score=score+3\n\"Value [score]\"\nreturn\n");
    let before = graph.to_pretty_json().unwrap();
    let projection = graph.variable_binding_projection("score").unwrap();
    assert_eq!(projection.graph, graph.graph_id);
    assert_eq!(projection.local_declarations.len(), 1);
    let local = projection.local_declarations[0];
    let input = graph.pin_by_key(local, "value").unwrap().id;
    assert_eq!(projection.input_contexts[&input], VariableBindingContext::ProjectGlobal);
    assert_eq!(projection.write_contexts[&local], VariableBindingContext::ActivationLocal);
    assert!(projection.read_contexts.values().any(|context| *context == VariableBindingContext::ProjectGlobal));
    assert!(projection.read_contexts.values().any(|context| *context == VariableBindingContext::ActivationLocal));
    let writes: Vec<_> = graph.nodes.values().filter(|node| node.kind == NodeKind::SetVariable)
        .map(|node| projection.write_contexts[&node.id]).collect();
    assert!(writes.contains(&VariableBindingContext::ProjectGlobal));
    assert!(writes.contains(&VariableBindingContext::ActivationLocal));
    assert_eq!(graph.to_pretty_json().unwrap(), before);
}

#[test]
fn binding_projection_keeps_conditional_and_loop_environment_unions() {
    for body in ["if false {local score=2}", "while false {local score=2}"] {
        let graph = scene(&format!("init {{set score=10}}\nscene label start\n{body}\nset score=score+1\nreturn\n"));
        let projection = graph.variable_binding_projection("score").unwrap();
        let setter = graph.nodes.values().find(|node| node.kind == NodeKind::SetVariable).unwrap().id;
        assert_eq!(projection.write_contexts[&setter], VariableBindingContext::GlobalOrLocal);
        assert_eq!(projection.input_contexts[&graph.pin_by_key(setter,"value").unwrap().id], VariableBindingContext::GlobalOrLocal);
    }
}

#[test]
fn binding_projection_records_call_entry_and_preserves_unknown_return_effects() {
    let graph = scene("init {set score=10}\nscene label start\ncall before\nlocal score=score+1\ncall after\nreturn\nlabel before\nreturn\nlabel after\nreturn\n");
    let projection = graph.variable_binding_projection("score").unwrap();
    assert_eq!(projection.narrative_call_contexts["before"], VariableBindingContext::ProjectGlobal);
    assert_eq!(projection.narrative_call_contexts["after"], VariableBindingContext::ActivationLocal);
    let local = projection.local_declarations[0];
    assert_eq!(projection.input_contexts[&graph.pin_by_key(local,"value").unwrap().id], VariableBindingContext::GlobalOrLocal);
}

#[test]
fn binding_projection_retains_all_repeated_declaration_identities_and_unknown_fragment_entry() {
    let graph = scene("init {set score=10}\nscene label start\nlocal score=score+1\nlocal score=2\n\"[score]\"\nreturn\n");
    let projection = graph.variable_binding_projection("score").unwrap();
    assert_eq!(projection.local_declarations.len(),2);
    assert!(projection.local_declarations.iter().all(|id|
        projection.write_contexts[id] == VariableBindingContext::ActivationLocal));
    let fragment = scene("init {set score=10}\nlabel start\nset score=score+1\nreturn\n");
    let projection = fragment.variable_binding_projection("score").unwrap();
    assert!(projection.input_contexts.values().any(|context| *context == VariableBindingContext::GlobalOrLocal));
}

#[test]
fn binding_projection_parameter_scope_and_unused_nodes_do_not_invent_global_uses() {
    let source="init {set score=10}\nfunction parameter(score) {return score+1}\nscene label start\n\"ok\"\nreturn\n";
    let project=SourceProject::open(source,&[]).unwrap();
    let graph=project.graphs().iter().find(|graph|matches!(&graph.kind,GraphKind::Function{name}if name=="parameter")).unwrap();
    let projection=graph.variable_binding_projection("score").unwrap();
    assert!(projection.local_declarations.is_empty());
    assert!(!projection.read_contexts.is_empty());
    assert!(projection.read_contexts.values().all(|context|*context==VariableBindingContext::ActivationLocal));
    let mut unused=scene(source);
    let get=unused.add_catalog_node(NodeKind::VariableGet,[90.0,90.0]).unwrap();
    unused.set_property(get,"name",PropertyValue::String("score".into())).unwrap();
    let pin=unused.pin_by_key(get,"value").unwrap().id;
    let before=unused.clone();let projection=unused.variable_binding_projection("score").unwrap();
    assert!(!projection.read_contexts.contains_key(&pin));
    assert_eq!(unused,before);
}

#[test]
fn binding_projection_shared_get_preserves_both_consumer_environments() {
    let mut graph=scene("init {set score=10}\nscene label start\nset other=score\nlocal score=2\nset other=score\nreturn\n");
    let setters:Vec<_>=graph.nodes.values().filter(|node|node.kind==NodeKind::SetVariable
        && graph.variable_node_name(node.id).as_deref()==Some("other"))
        .map(|node|graph.pin_by_key(node.id,"value").unwrap().id).collect();
    assert_eq!(setters.len(),2);
    let output=graph.edges.values().find(|edge|edge.input==setters[0]).unwrap().output;
    let second=graph.edges.values().find(|edge|edge.input==setters[1]).unwrap().id;
    graph.edges.get_mut(&second).unwrap().output=output;
    let before=graph.clone();let projection=graph.variable_binding_projection("score").unwrap();
    assert_eq!(projection.read_contexts[&output],VariableBindingContext::GlobalOrLocal);
    assert_eq!(graph,before);
}

#[test]
fn binding_projection_assignment_output_uses_consumer_slot_not_its_old_rhs() {
    let mut graph=scene("init {set score=10}\nscene label start\nset score=score+1\nlocal score=2\nset other=score\nreturn\n");
    let assignment=graph.nodes.values().find(|node|node.kind==NodeKind::SetVariable
        && graph.variable_node_name(node.id).as_deref()==Some("score")).unwrap().id;
    let consumer=graph.nodes.values().find(|node|node.kind==NodeKind::SetVariable
        && graph.variable_node_name(node.id).as_deref()==Some("other")).unwrap().id;
    let output=graph.pin_by_key(assignment,"value_out").unwrap().id;
    let input=graph.pin_by_key(consumer,"value").unwrap().id;
    let edge=graph.edges.values().find(|edge|edge.input==input).unwrap().id;
    graph.edges.get_mut(&edge).unwrap().output=output;
    let before=graph.clone();let projection=graph.variable_binding_projection("score").unwrap();
    assert_eq!(projection.write_contexts[&assignment],VariableBindingContext::ProjectGlobal);
    assert_eq!(projection.read_contexts[&output],VariableBindingContext::ActivationLocal);
    let rhs=graph.pin_by_key(assignment,"value").unwrap().id;
    assert_eq!(projection.input_contexts[&rhs],VariableBindingContext::ProjectGlobal);
    let rhs_reads:Vec<_>=graph.nodes.values().filter(|node|node.kind==NodeKind::VariableGet)
        .filter_map(|node|graph.pin_by_key(node.id,"value"))
        .filter_map(|pin|projection.read_contexts.get(&pin.id)).collect();
    assert!(!rhs_reads.is_empty());
    assert!(rhs_reads.iter().all(|context|**context==VariableBindingContext::ProjectGlobal));
    assert_eq!(graph,before);
}

fn changed_integer_with_retained_bool_consumer() -> (SourceProject, Vec<GraphDocument>, GraphDocument) {
    let source = "// Preserve authored source\nscene label start\nlocal score=true\nset score=score\nif score {\"Yes\"}\nreturn\n";
    let project = SourceProject::open(source, &[]).unwrap();
    let mut graph = project.graphs().iter().find(|graph|
        matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap().clone();
    let declaration = graph.local_variable_initializers("score").unwrap().remove(0);
    let input = graph.pin_by_key(declaration.node, "value").unwrap().id;
    let edges = graph.edges.clone();
    if let Some(producer) = graph.edges.values().find(|edge| edge.input == input).map(|edge| edge.output) {
        let literal = graph.pins[&producer].node;
        assert_eq!(graph.nodes[&literal].kind, NodeKind::Literal);
        graph.set_property(literal, "value", PropertyValue::Int(0)).unwrap();
    } else {
        assert_eq!(graph.pins[&input].default_value, Some(PropertyValue::Bool(true)));
        graph.pins.get_mut(&input).unwrap().default_value = Some(PropertyValue::Int(0));
    }
    let definition = graph.variables.get_mut("score").unwrap();
    definition.value_type = ValueType::Int;
    definition.default_value = PropertyValue::Int(0);
    let pins: Vec<_> = graph.nodes.values().filter(|node|
        graph.variable_node_name(node.id).as_deref() == Some("score"))
        .flat_map(|node| node.pins.iter().copied()).collect();
    for id in pins {
        let pin = graph.pins.get_mut(&id).unwrap();
        if matches!(pin.key.as_str(), "value" | "value_out") { pin.value_type = ValueType::Int; }
    }
    assert_eq!(graph.edges, edges, "a type edit does not auto-convert or delete existing wires");
    let mut graphs = project.graphs().to_vec();
    *graphs.iter_mut().find(|old| old.graph_id == graph.graph_id).unwrap() = graph.clone();
    (project, graphs, graph)
}

#[test]
fn binding_projection_survives_retained_incompatible_link_but_compile_and_source_remain_strict() {
    let (mut project, graphs, graph) = changed_integer_with_retained_bool_consumer();
    let before = graph.clone();
    let source = project.source().to_owned();
    let project_before = project.graphs().to_vec();
    let branch = graph.nodes.values().find(|node| node.kind == NodeKind::If).unwrap().id;
    let condition = graph.pin_by_key(branch, "condition").unwrap().id;
    let edge = graph.edges.values().find(|edge| edge.input == condition).unwrap();
    assert!(graph.validate().iter().any(|diagnostic|
        diagnostic.code == "incompatible_types" && diagnostic.edge == Some(edge.id)));
    assert!(transpile(&graph).is_err());
    assert!(transpile_with_origins(&graph).is_err());
    let projection = graph.variable_binding_projection("score").unwrap();
    assert_eq!(projection.read_contexts[&edge.output], VariableBindingContext::ActivationLocal);
    assert_eq!(projection.input_contexts[&condition], VariableBindingContext::ActivationLocal);
    assert_eq!(projection.local_declarations.len(), 1);
    assert_eq!(graph, before, "analysis never repairs a graph on behalf of the editor");
    assert!(project.apply_visual(&source, &graphs).is_err());
    assert_eq!(project.source(), source);
    assert_eq!(project.graphs(), project_before);
    assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(), graph,
        "the editable retained-link model itself survives serialization");
}

#[test]
fn binding_projection_does_not_bypass_structural_errors_with_type_errors() {
    let (_, _, mut graph) = changed_integer_with_retained_bool_consumer();
    let branch = graph.nodes.values().find(|node| node.kind == NodeKind::If).unwrap().id;
    let condition = graph.pin_by_key(branch, "condition").unwrap().id;
    graph.pins.remove(&condition);
    let before = graph.clone();
    assert!(graph.variable_binding_projection("score").is_err());
    assert_eq!(graph, before);
}

#[test]
fn binding_projection_rejects_invalid_authored_binding_even_when_types_are_incompatible() {
    let (_, _, mut graph) = changed_integer_with_retained_bool_consumer();
    let local = graph.nodes.values().find(|node| node.kind == NodeKind::LocalVariable).unwrap().id;
    graph.nodes.get_mut(&local).unwrap().properties.insert("name".into(),
        PropertyValue::String(String::new()));
    let before = graph.clone();
    assert!(graph.variable_binding_projection("score").is_err());
    assert_eq!(graph, before);
}


#[test]
fn declaration_analysis_survives_retained_type_link_without_evaluation_or_publication() {
    let (mut project, graphs, mut graph) = changed_integer_with_retained_bool_consumer();
    let local=graph.nodes.values().find(|node|node.kind==NodeKind::LocalVariable).unwrap().id;
    graph.nodes.get_mut(&local).unwrap().annotation=Some(NodeAnnotation{text:"Keep declaration é".into(),visible:false,pinned:true});
    let before=graph.clone(); let source=project.source().to_owned(); let project_before=project.graphs().to_vec();
    assert!(graph.local_declaration_initializers().is_err());
    assert!(graph.local_variable_initializers("score").is_err());
    let declarations=graph.local_declaration_initializers_for_binding_analysis().unwrap();
    assert_eq!(declarations["score"].len(),1);
    assert_eq!(declarations["score"][0].node,local);
    assert_eq!(declarations["score"][0].graph,graph.graph_id);
    assert_eq!(declarations["score"][0].expression,rvn_parser::Expr::Int(0));
    assert_eq!(declarations["score"][0].literal_value,Some(PropertyValue::Int(0)));
    assert_eq!(graph,before);
    assert!(transpile(&graph).is_err()); assert!(transpile_with_origins(&graph).is_err());
    assert!(project.apply_visual(&source,&graphs).is_err());
    assert_eq!(project.source(),source); assert_eq!(project.graphs(),project_before);
    let reopened=GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
    assert_eq!(reopened.local_declaration_initializers_for_binding_analysis().unwrap(),declarations);
    assert_eq!(reopened,graph);
}

#[test]
fn declaration_analysis_retains_structure_and_authored_binding_validation() {
    for invalid_name in [false,true] {
        let (_,_,mut graph)=changed_integer_with_retained_bool_consumer();
        if invalid_name {
            let local=graph.nodes.values().find(|node|node.kind==NodeKind::LocalVariable).unwrap().id;
            graph.nodes.get_mut(&local).unwrap().properties.insert("name".into(),PropertyValue::String(String::new()));
        } else {
            let branch=graph.nodes.values().find(|node|node.kind==NodeKind::If).unwrap().id;
            let pin=graph.pin_by_key(branch,"condition").unwrap().id;graph.pins.remove(&pin);
        }
        let before=graph.clone();assert!(graph.local_declaration_initializers_for_binding_analysis().is_err());assert_eq!(graph,before);
    }
}

#[test]
fn declaration_analysis_matches_strict_query_for_a_computed_authored_initializer() {
    let project=SourceProject::open("scene label start\nlocal score=1+2\nset score=7\nreturn\n",&[]).unwrap();
    let graph=project.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let before=graph.clone();let declarations=graph.local_declaration_initializers_for_binding_analysis().unwrap();
    assert_eq!(declarations,graph.local_declaration_initializers().unwrap());
    assert_eq!(declarations["score"][0].literal_value,None);
    assert!(matches!(declarations["score"][0].expression,rvn_parser::Expr::BinOp{..}));assert_eq!(*graph,before);
}


#[test]
fn declaration_analysis_validates_structure_even_without_local_nodes() {
    let project=SourceProject::open("scene label start\nif true {\"Yes\"}\nreturn\n",&[]).unwrap();
    let mut graph=project.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap().clone();
    assert!(graph.local_declaration_initializers_for_binding_analysis().unwrap().is_empty());
    let branch=graph.nodes.values().find(|node|node.kind==NodeKind::If).unwrap().id;
    let pin=graph.pin_by_key(branch,"condition").unwrap().id;graph.pins.remove(&pin);
    let before=graph.clone();
    assert!(graph.local_declaration_initializers_for_binding_analysis().is_err());
    assert_eq!(graph,before);
}


#[test]
fn retained_initializer_edit_keeps_invalid_wires_and_strict_publication() {
    let (mut project, mut graphs, mut graph) = changed_integer_with_retained_bool_consumer();
    let source = project.source().to_owned();
    let project_before = project.graphs().to_vec();
    let initial = graph.local_declaration_initializers_for_binding_analysis().unwrap()["score"][0].clone();
    graph.nodes.get_mut(&initial.node).unwrap().annotation = Some(NodeAnnotation {
        text: "Déclaration conservée 🍃".into(), visible: false, pinned: true,
    });
    let before = graph.clone();
    let diagnostics = graph.validate();
    assert!(diagnostics.iter().any(|d| d.code == "incompatible_types"));
    assert!(graph.replace_local_initializer("score", &initial, PropertyValue::Int(3)).unwrap());
    let edited = graph.local_declaration_initializers_for_binding_analysis().unwrap()["score"][0].clone();
    assert_eq!(edited.node, initial.node); assert_eq!(edited.graph, initial.graph);
    assert_eq!(edited.expression, rvn_parser::Expr::Int(3));
    assert_eq!(edited.literal_value, Some(PropertyValue::Int(3)));
    // Importers may store an authored primitive inline or on a producer.
    // Exactly that value changes; every other model field remains identical.
    let mut expected = before.clone();
    let input = expected.pin_by_key(initial.node, "value").unwrap().id;
    if let Some(output) = expected.edges.values().find(|edge| edge.input == input).map(|edge| edge.output) {
        let producer = expected.pins[&output].node;
        expected.set_property(producer, "value", PropertyValue::Int(3)).unwrap();
    } else {
        expected.pins.get_mut(&input).unwrap().default_value = Some(PropertyValue::Int(3));
    }
    assert_eq!(graph, expected, "only the actual authored primitive may change");
    assert_eq!(graph.edges, before.edges);
    assert_eq!(graph.nodes.keys().collect::<Vec<_>>(), before.nodes.keys().collect::<Vec<_>>());
    assert_eq!(graph.nodes[&initial.node].annotation, before.nodes[&initial.node].annotation);
    assert_eq!(graph.validate(), diagnostics);
    assert!(transpile(&graph).is_err()); assert!(transpile_with_origins(&graph).is_err());
    *graphs.iter_mut().find(|g| g.graph_id == graph.graph_id).unwrap() = graph.clone();
    assert!(project.apply_visual(&source, &graphs).is_err());
    assert_eq!(project.source(), source); assert_eq!(project.graphs(), project_before);
    assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(), graph);
}

#[test]
fn retained_initializer_edit_guards_noop_stale_foreign_and_primitive_type() {
    let (_, _, mut graph) = changed_integer_with_retained_bool_consumer();
    let initial = graph.local_declaration_initializers_for_binding_analysis().unwrap()["score"][0].clone();
    let before = graph.clone();
    assert!(!graph.replace_local_initializer("score", &initial, PropertyValue::Int(0)).unwrap());
    assert_eq!(graph, before);
    for value in [PropertyValue::Bool(false), PropertyValue::Float(1.0), PropertyValue::String("0".into())] {
        assert!(graph.replace_local_initializer("score", &initial, value).is_err());
        assert_eq!(graph, before);
    }
    let mut foreign = initial.clone(); foreign.graph = GraphId::new(graph.graph_id.get() + 1000);
    assert!(graph.replace_local_initializer("score", &foreign, PropertyValue::Int(2)).is_err());
    assert_eq!(graph, before);
    assert!(graph.replace_local_initializer("score", &initial, PropertyValue::Int(2)).unwrap());
    let edited = graph.clone();
    assert!(graph.replace_local_initializer("score", &initial, PropertyValue::Int(4)).is_err());
    assert_eq!(graph, edited);
    assert!(graph.replace_local_initializer("missing", &initial, PropertyValue::Int(4)).is_err());
    assert_eq!(graph, edited);
}

#[test]
fn retained_initializer_edit_does_not_bypass_structure_or_authored_name_even_for_noop() {
    for invalid_name in [false, true] {
        let (_, _, mut graph) = changed_integer_with_retained_bool_consumer();
        let initial = graph.local_declaration_initializers_for_binding_analysis().unwrap()["score"][0].clone();
        if invalid_name {
            graph.nodes.get_mut(&initial.node).unwrap().properties.insert("name".into(), PropertyValue::String(String::new()));
        } else {
            let branch = graph.nodes.values().find(|n| n.kind == NodeKind::If).unwrap().id;
            let pin = graph.pin_by_key(branch, "condition").unwrap().id; graph.pins.remove(&pin);
        }
        let before = graph.clone();
        for value in [0, 3] {
            assert!(graph.replace_local_initializer("score", &initial, PropertyValue::Int(value)).is_err());
            assert_eq!(graph, before);
        }
    }
}
