//! Reproducible fixture for destructive actions from a custom pause page.
use rvn_ui::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);
    std::fs::create_dir(&root)?;
    std::fs::create_dir(root.join("assets"))?;
    let mut doc = Document::from_template(ThemePreset::Sobre);
    if std::env::args().any(|a| a == "--graphs") {
        let page = doc
            .pages
            .iter_mut()
            .find(|p| p.role == Some(PageRole::Confirm))
            .unwrap();
        for e in &page.elements {
            if matches!(e.action, Action::Confirm | Action::Back) {
                page.graphs.push(Graph {
                    id: format!("{}_decision", e.id),
                    target: Some(e.id.clone()),
                    event: Event::Click,
                    entry: 0,
                    nodes: vec![
                        Node {
                            id: 0,
                            position: [0.0; 2],
                            op: Op::Set {
                                variable: "confirmation_clicked".into(),
                                value: true.into(),
                            },
                            next: Some(1),
                        },
                        Node {
                            id: 1,
                            position: [240.0, 0.0],
                            op: Op::Action(e.action.clone()),
                            next: None,
                        },
                    ],
                });
            }
        }
        for (event, key) in [
            (Event::Open, "confirmation_opened"),
            (Event::Close, "confirmation_closed"),
        ] {
            page.graphs.push(Graph {
                id: key.into(),
                target: None,
                event,
                entry: 0,
                nodes: vec![Node {
                    id: 0,
                    position: [0.0; 2],
                    op: Op::Set {
                        variable: key.into(),
                        value: true.into(),
                    },
                    next: None,
                }],
            });
        }
        for e in &page.elements {
            if matches!(e.action, Action::Confirm | Action::Back) {
                for (event, key) in [
                    (Event::Focus, "confirmation_focused"),
                    (Event::Hover, "confirmation_hovered"),
                    (Event::HoverLeave, "confirmation_left"),
                ] {
                    page.graphs.push(Graph {
                        id: format!("{}_{key}", e.id),
                        target: Some(e.id.clone()),
                        event,
                        entry: 0,
                        nodes: vec![Node {
                            id: 0,
                            position: [0.0; 2],
                            op: Op::Set {
                                variable: key.into(),
                                value: true.into(),
                            },
                            next: None,
                        }],
                    });
                }
            }
        }
        let mut help = Element::new("confirmation_help".into(), Kind::Panel);
        help.rect = [600.0, 100.0, 700.0, 100.0];
        help.focus_order = -1;
        page.elements.push(help);
        for (event, key) in [
            (Event::Click, "help_clicked"),
            (Event::Hover, "help_hovered"),
            (Event::Focus, "help_focused"),
        ] {
            page.graphs.push(Graph {
                id: key.into(),
                target: Some("confirmation_help".into()),
                event,
                entry: 0,
                nodes: vec![Node {
                    id: 0,
                    position: [0.0; 2],
                    op: Op::Set {
                        variable: key.into(),
                        value: true.into(),
                    },
                    next: None,
                }],
            });
        }
    } else {
        doc.pages.retain(|p| p.role != Some(PageRole::Confirm));
    } // Exercise native fallback.
    let page = doc
        .pages
        .iter_mut()
        .find(|p| p.role == Some(PageRole::Pause))
        .unwrap();
    page.elements.clear();
    for (id, text, action, y) in [
        ("restart", "Recommencer", Action::NewGame, 240.0),
        (
            "continue",
            "Continuer la sauvegarde",
            Action::Continue,
            400.0,
        ),
    ] {
        let mut e = Element::new(id.into(), Kind::Button);
        e.text = text.into();
        e.rect = [600.0, y, 720.0, 90.0];
        e.action = action;
        page.elements.push(e);
    }
    doc.validate()?;
    std::fs::write(
        root.join("menus.rvnui"),
        serde_json::to_string_pretty(&doc)?,
    )?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Confirmations de partie\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    std::fs::write(
        root.join("test.rvn"),
        "label start\n\"Premier dialogue sauvegardé.\"\n\"Deuxième dialogue en cours.\"\n",
    )?;
    Ok(())
}
