use rvn_graph::*;

#[test]
fn source_save_and_reopen_preserve_strict_policy_and_append_without_rewriting_rvn() {
    let source = "// Existing author note\nfunction total() { return 7 + 2 }\nfunction join() { return \"north\" + \"south\" }\nlabel start\n\"Ready\"\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let mut authored = project.graphs().to_vec();
    let total = authored
        .iter_mut()
        .find(|graph| {
            graph.kind
                == GraphKind::Function {
                    name: "total".into(),
                }
        })
        .unwrap();
    let numeric = total
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::MathAdd)
        .unwrap()
        .id;
    total.enable_blueprint_operator_policy(numeric).unwrap();
    total.nodes.get_mut(&numeric).unwrap().position = [144.0, 208.0];
    let join = authored
        .iter_mut()
        .find(|graph| {
            graph.kind
                == GraphKind::Function {
                    name: "join".into(),
                }
        })
        .unwrap();
    let append = join
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::MathAdd)
        .unwrap()
        .id;
    join.nodes.get_mut(&append).unwrap().kind = NodeKind::StringAppend;
    join.nodes.get_mut(&append).unwrap().position = [352.0, 80.0];
    for (key, label) in [("left", "A"), ("right", "B"), ("value", "Return Value")] {
        let pin = join.pin_by_key(append, key).unwrap().id;
        join.pins.get_mut(&pin).unwrap().value_type = ValueType::String;
        join.pins.get_mut(&pin).unwrap().label = label.into();
    }
    let before = authored.clone();
    project.apply_visual(source, &authored).unwrap();
    assert_eq!(
        project.source(),
        source,
        "Typing metadata and presentation do not rewrite author source"
    );
    assert_eq!(
        authored, before,
        "The transaction must not modify caller-owned graphs"
    );
    let reopened = SourceProject::open(source, project.graphs()).unwrap();
    assert_eq!(reopened.graphs(), project.graphs());
    let total = reopened
        .graphs()
        .iter()
        .find(|graph| {
            graph.kind
                == GraphKind::Function {
                    name: "total".into(),
                }
        })
        .unwrap();
    assert!(total.has_blueprint_operator_policy(numeric));
    assert_eq!(total.nodes[&numeric].position, [144.0, 208.0]);
    let operand = total.pin_by_key(numeric, "left").unwrap().id;
    assert!(!total.accepts_pin_source(operand, &ValueType::String));
    let join = reopened
        .graphs()
        .iter()
        .find(|graph| {
            graph.kind
                == GraphKind::Function {
                    name: "join".into(),
                }
        })
        .unwrap();
    assert_eq!(join.nodes[&append].kind, NodeKind::StringAppend);
    assert_eq!(join.nodes[&append].position, [352.0, 80.0]);
    assert!(!join.accepts_pin_source(
        join.pin_by_key(append, "right").unwrap().id,
        &ValueType::Int
    ));
    assert_eq!(reopened.source(), source);
}

#[test]
fn opening_and_saving_legacy_dynamic_expressions_does_not_enable_strict_policy() {
    let source = "// Legacy expressions are legal RVN\nfunction legacy() { return \"score: \" + 7 }\nfunction compare() { return true == \"true\" }\nlabel start\n\"Ready\"\nreturn\n";
    let mut project = SourceProject::open(source, &[]).unwrap();
    let authored = project.graphs().to_vec();
    assert!(authored
        .iter()
        .flat_map(|graph| graph.nodes.values())
        .filter(|node| node.kind.supports_blueprint_operator_policy())
        .all(|node| !node.uses_blueprint_operator_policy()));
    project.apply_visual(source, &authored).unwrap();
    let reopened = SourceProject::open(source, project.graphs()).unwrap();
    assert_eq!(reopened.source(), source);
    assert_eq!(reopened.graphs(), authored);
}
