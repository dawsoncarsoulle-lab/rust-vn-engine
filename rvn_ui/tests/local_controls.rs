use rvn_ui::*;
#[test]
fn inherited_controls_offer_the_same_events_and_invalid_targets_are_rejected() {
    let mut doc = Document::defaults();
    let mut template = Element::new("template".into(), Kind::Slider);
    template.binding = Some("music_volume".into());
    doc.components.insert("volume".into(), template.clone());
    let mut instance = Element::new("instance".into(), Kind::Slider);
    instance.component = Some("volume".into());
    instance.inherit_binding = true;
    assert!(!instance.supports_value_changed());
    assert!(doc.resolved_element(&instance).supports_value_changed());
    doc.pages[0].elements = vec![instance];
    doc.pages[0].graphs = vec![Graph {
        id: "volume_changed".into(),
        target: Some("instance".into()),
        event: Event::ValueChanged,
        entry: 0,
        nodes: vec![Node {
            id: 0,
            position: [0.0; 2],
            next: None,
            op: Op::Set {
                variable: "changed".into(),
                value: true.into(),
            },
        }],
    }];
    doc.validate().unwrap();
    template.binding = None;
    template.local_control = Some(LocalControl::for_kind(Kind::Slider, "ui.volume".into()));
    doc.components.insert("volume".into(), template);
    doc.validate().unwrap();
    doc.pages[0].graphs[0].event = Event::Click;
    assert!(doc.validate().unwrap_err().contains("Changement de valeur"));
    doc.pages[0].graphs[0].event = Event::ValueChanged;
    doc.pages[0].graphs[0].target = None;
    assert!(doc.validate().unwrap_err().contains("Changement de valeur"));
    let mut bad = Element::new("bad".into(), Kind::CheckBox);
    bad.binding = Some("music_volume".into());
    assert!(!bad.supports_value_changed());
}
#[test]
fn imported_controls_and_graphs_receive_independent_variable_names() {
    let mut doc = Document::defaults();
    let mut e = Element::new("control".into(), Kind::CheckBox);
    e.local_control = Some(LocalControl::for_kind(e.kind, "ui.hints".into()));
    doc.pages[0].elements = vec![e];
    let mut incoming = doc.clone();
    incoming.pages[0].elements[0]
        .local_control
        .as_mut()
        .unwrap()
        .initial = true.into();
    incoming.pages[0].graphs = vec![Graph {
        id: "toggle".into(),
        target: Some("control".into()),
        event: Event::ValueChanged,
        entry: 0,
        nodes: vec![Node {
            id: 0,
            position: [0.0; 2],
            next: None,
            op: Op::Set {
                variable: "ui.hints".into(),
                value: false.into(),
            },
        }],
    }];
    let start = doc.pages.len();
    doc.merge_design(incoming).unwrap();
    let imported = &doc.pages[start];
    assert_eq!(
        imported.elements[0]
            .local_control
            .as_ref()
            .unwrap()
            .variable,
        "import_1_ui.hints"
    );
    assert!(
        matches!(&imported.graphs[0].nodes[0].op,Op::Set{variable,..} if variable=="import_1_ui.hints")
    );
    let mut session = Session::default();
    session.initialize_controls(&doc).unwrap();
    assert_eq!(session.variables["ui.hints"], false);
    assert_eq!(session.variables["import_1_ui.hints"], true);
    doc.validate().unwrap();
}
#[test]
fn local_configuration_roundtrips_and_transient_values_do_not_change_it() {
    let mut doc = Document::defaults();
    let mut e = Element::new("local".into(), Kind::CheckBox);
    e.local_control = Some(LocalControl::for_kind(e.kind, "ui.hints".into()));
    doc.pages[0].elements = vec![e];
    let source = doc.clone();
    let json = doc.to_json().unwrap();
    assert_eq!(Document::from_json(&json).unwrap(), doc);
    let mut session = Session::default();
    session.initialize_controls(&doc).unwrap();
    assert_eq!(session.variables["ui.hints"], false);
    session
        .set_control_value(
            Kind::CheckBox,
            doc.pages[0].elements[0].local_control.as_ref().unwrap(),
            true.into(),
        )
        .unwrap();
    let presented = session.present(&doc);
    assert_eq!(
        presented.pages[0].elements[0]
            .local_control
            .as_ref()
            .unwrap()
            .initial,
        true
    );
    assert_eq!(doc, source);
    let mut duplicate = doc.pages[0].elements[0].clone();
    duplicate.id = "duplicate".into();
    duplicate.local_control.as_mut().unwrap().initial = true.into();
    doc.pages[0].elements.push(duplicate);
    assert!(doc
        .validate()
        .unwrap_err()
        .contains("configurations différentes"));
}
#[test]
fn local_controls_reject_double_commands_and_incompatible_graph_values() {
    let mut doc = Document::defaults();
    let mut e = Element::new("local".into(), Kind::CheckBox);
    e.local_control = Some(LocalControl::for_kind(e.kind, "ui.hints".into()));
    e.action = Action::NewGame;
    doc.pages[0].elements = vec![e];
    assert!(doc.validate().is_err());
    doc.pages[0].elements[0].action = Action::None;
    doc.pages[0].graphs = vec![Graph {
        id: "value".into(),
        target: Some("local".into()),
        event: Event::Click,
        entry: 0,
        nodes: vec![Node {
            id: 0,
            position: [0.0; 2],
            next: None,
            op: Op::Set {
                variable: "ui.hints".into(),
                value: true.into(),
            },
        }],
    }];
    assert!(doc.validate().unwrap_err().contains("Changement de valeur"));
    doc.pages[0].graphs[0].event = Event::ValueChanged;
    doc.validate().unwrap();
    doc.pages[0].graphs[0].nodes[0].op = Op::Set {
        variable: "ui.hints".into(),
        value: "incorrect".into(),
    };
    assert!(doc.validate().unwrap_err().contains("valeur incompatible"));
}
