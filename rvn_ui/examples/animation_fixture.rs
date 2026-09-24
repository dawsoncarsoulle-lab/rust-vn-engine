use rvn_ui::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);
    std::fs::create_dir(&root)?;
    std::fs::create_dir(root.join("assets"))?;
    let mut doc = Document::from_template(ThemePreset::ScienceFiction);
    let page = &mut doc.pages[0];
    page.elements.clear();
    page.graphs.clear();
    let mut panel = Element::new("animated".into(), Kind::Panel);
    panel.rect = [600.0, 280.0, 700.0, 200.0];
    panel.normal = [0.1, 0.2, 0.4, 1.0];
    if std::env::args().any(|a| a == "--shadow") {
        page.background = [0.7, 0.7, 0.7, 1.0];
        panel.appearance.shadow = Shadow {
            offset: [24.0, 30.0],
            blur: 16.0,
            color: [0.0, 0.0, 0.0, 0.8],
        };
    }
    let mut text = Element::new("caption".into(), Kind::Text);
    text.rect = [40.0, 65.0, 620.0, 80.0];
    text.text = "Fondu · Mouvement · Échelle · Couleur".into();
    text.font_size = 32.0;
    panel.children.push(text);
    page.elements.push(panel);
    let kinds = [
        (AnimationKind::Fade, [0.0; 4], [1.0, 0.0, 0.0, 0.0]),
        (AnimationKind::Move, [-200.0, 100.0, 0.0, 0.0], [0.0; 4]),
        (
            AnimationKind::Scale,
            [0.5, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
        ),
        (
            AnimationKind::Color,
            [0.7, 0.1, 0.1, 1.0],
            [0.05, 0.35, 0.5, 1.0],
        ),
    ];
    page.graphs.push(Graph {
        id: "entrance".into(),
        target: None,
        event: Event::Open,
        entry: 0,
        nodes: kinds
            .into_iter()
            .enumerate()
            .map(|(i, (kind, from, to))| Node {
                id: i as u32,
                position: [i as f32 * 240.0, 0.0],
                op: Op::Animate {
                    element: "animated".into(),
                    clip: AnimationClip {
                        kind,
                        duration: 3.0,
                        curve: Curve::EaseInOut,
                        from,
                        to,
                    },
                },
                next: if i < 3 { Some(i as u32 + 1) } else { None },
            })
            .collect(),
    });
    if std::env::args().any(|a| a == "--frame") {
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../rvn_cli/template/default/assets/ui/textbox2.png"),
            root.join("assets/frame.png"),
        )?;
        let frame = &mut page.elements[0];
        frame.asset = Some("frame.png".into());
        frame.layout_options.image_fit = ImageFit::NineSlice;
        frame.layout_options.slice = [140.0, 90.0, 140.0, 90.0];
        for (id, rect) in [
            ("wide", [250.0, 80.0, 1350.0, 150.0]),
            ("portrait", [80.0, 280.0, 350.0, 600.0]),
        ] {
            let mut e = Element::new(id.into(), Kind::Image);
            e.rect = rect;
            e.normal = [0.0; 4];
            e.asset = Some("frame.png".into());
            e.layout_options.image_fit = ImageFit::NineSlice;
            e.layout_options.slice = [140.0, 90.0, 140.0, 90.0];
            page.elements.push(e);
        }
    }
    doc.validate()?;
    std::fs::write(
        root.join("menus.rvnui"),
        serde_json::to_string_pretty(&doc)?,
    )?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Test des effets UI\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    std::fs::write(
        root.join("test.rvn"),
        "label start\n\"Scène inchangée pendant les effets.\"\n",
    )?;
    Ok(())
}
