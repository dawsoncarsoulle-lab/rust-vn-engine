use rvn_graph::*;

fn choice() -> (GraphDocument, NodeId) {
    let mut graph=GraphDocument::new(GraphId::new(95301),GraphKind::Label{name:"start".into()});
    let entry=graph.add_catalog_node(NodeKind::Label,[0.0,0.0]).unwrap();
    let choice=graph.add_catalog_node(NodeKind::Choice,[240.0,160.0]).unwrap();
    let output=graph.pin_by_key(entry,"exec_out").unwrap().id;
    let input=graph.pin_by_key(choice,"exec_in").unwrap().id;
    graph.connect(output,input).unwrap();
    for name in ["First","Second","Third"]{graph.add_choice_option(choice,name).unwrap();}
    (graph,choice)
}

#[test]
fn vacant_choice_conditions_are_boolean_always_and_false_is_authored_not_a_legacy_placeholder() {
    let (mut graph,choice)=choice();
    let pin=graph.pin_by_key(choice,"option_0_condition").unwrap().id;
    assert_eq!(graph.effective_pin_type(pin),Some(ValueType::Bool));
    assert_eq!(graph.choice_condition_default(pin),Some(true));
    assert!(!transpile(&graph).unwrap().source.contains(" if "));
    graph.set_pin_default(choice,"option_0_condition",PropertyValue::Bool(false)).unwrap();
    assert_eq!(graph.choice_condition_default(pin),Some(false));
    assert!(transpile(&graph).unwrap().source.contains("\"First\" if false"));
    let before=graph.clone();
    assert_eq!(graph.materialize_visible_defaults().unwrap(),0);
    assert_eq!(graph,before);
    let reopened=GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
    assert_eq!(reopened,before);
    assert!(graph.set_pin_default(choice,"option_0_condition",PropertyValue::Int(1)).is_err());
    let integer=graph.add_catalog_node(NodeKind::Literal,[-260.0,0.0]).unwrap();
    graph.set_property(integer,"value",PropertyValue::Int(1)).unwrap();
    assert!(graph.connect(graph.pin_by_key(integer,"value").unwrap().id,pin).is_err());
}

#[test]
fn removing_a_middle_answer_preserves_remaining_branch_and_condition_ids_defaults_and_edges() {
    let (mut graph,choice)=choice();
    let last_branch=graph.pin_by_key(choice,"option_2").unwrap().id;
    let last_condition=graph.pin_by_key(choice,"option_2_condition").unwrap().id;
    let first=graph.pin_by_key(choice,"option_0_condition").unwrap().id;
    graph.set_pin_default(choice,"option_2_condition",PropertyValue::Bool(false)).unwrap();
    let literal=graph.add_catalog_node(NodeKind::Literal,[-200.0,0.0]).unwrap();
    graph.set_property(literal,"value",PropertyValue::Bool(true)).unwrap();
    let output=graph.pin_by_key(literal,"value").unwrap().id;
    let edge=graph.connect(output,last_condition).unwrap();
    let removed=graph.pin_by_key(choice,"option_1_condition").unwrap().id;
    let before=graph.clone();
    graph.remove_choice_option(choice,1).unwrap();
    assert!(!graph.pins.contains_key(&removed));
    assert_eq!(graph.pin_by_key(choice,"option_1").unwrap().id,last_branch);
    assert_eq!(graph.pin_by_key(choice,"option_1_condition").unwrap().id,last_condition);
    assert_eq!(graph.pin_by_key(choice,"option_0_condition").unwrap().id,first);
    assert_eq!(graph.edges[&edge].input,last_condition);
    assert_eq!(graph.choice_condition_default(last_condition),Some(false));
    assert_eq!(graph.nodes[&choice].properties["options"],PropertyValue::StringList(vec!["First".into(),"Third".into()]));
    graph.remove_edge(edge);
    assert!(transpile(&graph).unwrap().source.contains("\"Third\" if false"));
    let stable=graph.clone();
    assert!(graph.remove_choice_option(choice,9).is_err());assert_eq!(graph,stable);
    graph=before;assert_eq!(graph.pin_by_key(choice,"option_2_condition").unwrap().id,last_condition);
    assert!(graph.edges.contains_key(&edge));
}

#[test]
fn imported_and_cached_choice_guards_become_visible_wires_without_source_loss() {
    let source="// Preserve this comment and spacing.\ninit { set trust = 5 }\nlabel start\nchoice {\n    \"Yes\" if trust >= 5 => { \"Allowed\" }\n    \"Always\" => { \"Next\" }\n    \"Explicit true\" if true => { \"Sure\" }\n}\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();
    let graph=project.graphs().iter().find(|graph|matches!(graph.kind,GraphKind::Label{..})).unwrap();
    let choice=graph.nodes.values().find(|node|node.kind==NodeKind::Choice).unwrap().id;
    for index in [0,2] {
        let pin=graph.pin_by_key(choice,&format!("option_{index}_condition")).unwrap();
        assert!(graph.edges.values().any(|edge|edge.input==pin.id));
        assert_eq!(pin.value_type,ValueType::Bool);
    }
    let original=project.graphs().to_vec();
    project.apply_visual(source,&original).unwrap();assert_eq!(project.source(),source);
    // Recreate the previous presentation cache's free-form guard while keeping
    // its execution graph, identities, positions and source document intact.
    let mut legacy=original.clone();
    let graph=legacy.iter_mut().find(|graph|matches!(graph.kind,GraphKind::Label{..})).unwrap();
    let pin=graph.pin_by_key(choice,"option_0_condition").unwrap().id;
    let edge=graph.edges.values().find(|edge|edge.input==pin).unwrap().id;
    graph.remove_edge(edge);
    graph.set_property(choice,"option_0_condition",PropertyValue::String("trust >= 5".into())).unwrap();
    graph.nodes.get_mut(&choice).unwrap().position=[790.0,370.0];
    let old_ids=graph.nodes.keys().copied().collect::<Vec<_>>();
    let reopened=SourceProject::open(source,&legacy).unwrap();
    let graph=reopened.graphs().iter().find(|graph|matches!(graph.kind,GraphKind::Label{..})).unwrap();
    assert_eq!(graph.nodes[&choice].position,[790.0,370.0]);
    for id in old_ids {assert!(graph.nodes.contains_key(&id));}
    assert_eq!(graph.pin_by_key(choice,"option_0_condition").unwrap().id,pin);
    assert!(graph.edges.values().any(|edge|edge.input==pin));
    assert_eq!(reopened.source(),source);
    let mut reopened=reopened;let edited=reopened.graphs().to_vec();
    reopened.apply_visual(source,&edited).unwrap();assert_eq!(reopened.source(),source);
}

#[test]
fn old_truthy_non_boolean_guards_are_retained_without_fake_boolean_conversions() {
    let source="label start\nchoice { \"Legacy\" if 3 => { \"Kept\" } }\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();
    let graph=project.graphs().iter().find(|graph|matches!(graph.kind,GraphKind::Label{..})).unwrap();
    let choice=graph.nodes.values().find(|node|node.kind==NodeKind::Choice).unwrap().id;
    let pin=graph.pin_by_key(choice,"option_0_condition").unwrap().id;
    assert!(graph.choice_condition_default(pin).is_none());
    assert_eq!(graph.nodes[&choice].properties["option_0_condition"],PropertyValue::String("3".into()));
    let edited=project.graphs().to_vec();project.apply_visual(source,&edited).unwrap();
    assert_eq!(project.source(),source);
}
