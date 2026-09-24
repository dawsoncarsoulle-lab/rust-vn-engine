//! Visual fixture: show, preserve, remove twice, reappear. Optional --fade.
//! Pass a NEW output directory; put the story assets under its assets/ folder.
use rvn_graph::*;
use std::{fs, path::PathBuf};

fn connect(g: &mut GraphDocument, a: NodeId, ak: &str, b: NodeId, bk: &str) {
    g.connect(
        g.pin_by_key(a, ak).unwrap().id,
        g.pin_by_key(b, bk).unwrap().id,
    )
    .unwrap();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(std::env::args().nth(1).expect("new output directory"));
    fs::create_dir(&dir)?;
    let mut g = GraphDocument::new(
        GraphId::new(901),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    g.characters.insert("mara".into(), "Mara".into());
    let mut previous = g.add_catalog_node(NodeKind::Label, [0.0, 0.0])?;
    for (i, (path, text)) in [
        (
            Some("sprites/mara/neutral.png"),
            "Image branchee : Mara doit etre visible.",
        ),
        (
            None,
            "Pin vide : Mara reste visible, sans changer de sprite.",
        ),
        (None, "Apres Retirer le sprite : Mara parle hors champ."),
        (
            Some("sprites/mara/determinee.png"),
            "Nouvelle image branchee : Mara reapparait.",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let x = 350.0 + i as f64 * 650.0;
        if i == 2 {
            for offset in 0..2 {
                let hide = g.add_catalog_node(
                    NodeKind::SpriteHide,
                    [x - 300.0 + offset as f64 * 180.0, -220.0],
                )?;
                g.set_pin_default(hide, "character", PropertyValue::String("mara".into()))?;
                if std::env::args().any(|a| a == "--fade") {
                    g.set_pin_default(hide, "transition", PropertyValue::String("fade".into()))?;
                }
                connect(&mut g, previous, "exec_out", hide, "exec_in");
                previous = hide;
            }
        }
        let d = g.add_catalog_node(NodeKind::Dialogue, [x, 0.0])?;
        let c = g.add_catalog_node(NodeKind::CharacterValue, [x - 250.0, 180.0])?;
        g.set_property(c, "character", PropertyValue::String("mara".into()))?;
        g.set_pin_default(d, "text", PropertyValue::String(text.into()))?;
        connect(&mut g, previous, "exec_out", d, "exec_in");
        connect(&mut g, c, "value", d, "character");
        if let Some(path) = path {
            let a = g.add_catalog_node(NodeKind::SpriteAsset, [x - 250.0, 350.0])?;
            g.set_property(a, "path", PropertyValue::String(path.into()))?;
            connect(&mut g, a, "value", c, "sprite");
        }
        previous = d;
    }
    g.materialize_visible_defaults()?;
    fs::write(dir.join("test.rvngraph"), g.to_pretty_json()?)?;
    fs::write(dir.join("test.rvn"), transpile(&g)?.source)?;
    fs::write(dir.join("rvn.toml"), "[project]\ntitle = \"Sprite pin QA\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    Ok(())
}
