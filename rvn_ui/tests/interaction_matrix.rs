use rvn_ui::*;

#[test]
fn every_event_dispatches_each_operation_only_for_its_owner() {
    let operations = vec![
        Op::Action(Action::Settings),
        Op::Visible {
            element: "button_0".into(),
            value: false,
        },
        Op::Enabled {
            element: "button_0".into(),
            value: false,
        },
        Op::Text {
            element: "button_0".into(),
            value: "Été".into(),
        },
        Op::Image {
            element: "button_0".into(),
            path: "image.png".into(),
        },
        Op::Sound {
            path: "sound.ogg".into(),
        },
        Op::Animate {
            element: "button_0".into(),
            clip: AnimationClip {
                kind: AnimationKind::Fade,
                duration: 0.3,
                curve: Curve::Linear,
                from: [0.0; 4],
                to: [1.0; 4],
            },
        },
        Op::Set {
            variable: "local".into(),
            value: true.into(),
        },
        Op::Branch {
            variable: "local".into(),
            equals: true.into(),
            otherwise: None,
        },
    ];
    for event in [
        Event::Click,
        Event::Hover,
        Event::HoverLeave,
        Event::Focus,
        Event::ValueChanged,
        Event::Open,
        Event::Close,
    ] {
        for op in &operations {
            let target = if matches!(event, Event::Open | Event::Close) {
                None
            } else {
                Some("button_0".to_string())
            };
            let mut page = Document::defaults().pages.remove(0);
            page.graphs = vec![Graph {
                id: "matrix".into(),
                target: target.clone(),
                event: event.clone(),
                entry: 0,
                nodes: vec![Node {
                    id: 0,
                    position: [0.0; 2],
                    op: op.clone(),
                    next: None,
                }],
            }];
            let before = page.clone();
            let mut session = Session::default();
            assert!(session
                .dispatch(&page, Some("other"), event.clone(), Action::None)
                .unwrap()
                .is_empty());
            assert!(session.variables.is_empty());
            let effects = session
                .dispatch(&page, target.as_deref(), event.clone(), Action::None)
                .unwrap();
            assert_eq!(
                effects.len(),
                if matches!(op, Op::Set { .. } | Op::Branch { .. }) {
                    0
                } else {
                    1
                },
                "{event:?} / {op:?}"
            );
            if matches!(op, Op::Set { .. }) {
                assert_eq!(session.variables.get("local"), Some(&true.into()));
            }
            assert_eq!(page, before, "Runtime must not edit authoring data");
        }
    }
}

#[test]
fn true_and_false_branches_preserve_destinations() {
    let graph = Graph {
        id: "branch".into(),
        target: None,
        event: Event::Open,
        entry: 0,
        nodes: vec![
            Node {
                id: 0,
                position: [0.0; 2],
                op: Op::Branch {
                    variable: "local".into(),
                    equals: true.into(),
                    otherwise: Some(2),
                },
                next: Some(1),
            },
            Node {
                id: 1,
                position: [0.0; 2],
                op: Op::Text {
                    element: "result".into(),
                    value: "true".into(),
                },
                next: None,
            },
            Node {
                id: 2,
                position: [0.0; 2],
                op: Op::Text {
                    element: "result".into(),
                    value: "false".into(),
                },
                next: None,
            },
        ],
    };
    for value in [true, false] {
        let mut session = Session::default();
        session.variables.insert("local".into(), value.into());
        assert_eq!(
            session.execute(&graph).unwrap(),
            vec![Effect::Text("result".into(), value.to_string())]
        );
    }
}
