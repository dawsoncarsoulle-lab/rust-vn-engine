//! Independent foreground colors for hover, press, disabled and selection.
use rvn_ui::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);
    std::fs::create_dir(&root)?;
    std::fs::create_dir(root.join("assets"))?;
    let mut doc = Document::from_template(ThemePreset::Sobre);
    let page = &mut doc.pages[0];
    page.elements.clear();
    page.graphs.clear();
    for (i, (id, text)) in [
        ("interactive", "Survol et clic"),
        ("disabled", "Désactivé"),
        ("selected", "Sélectionné"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = Element::new(id.into(), Kind::Button);
        e.rect = [500.0, 220.0 + i as f32 * 180.0, 900.0, 120.0];
        e.text = text.into();
        e.font_size = 48.0;
        e.foreground = [0.6, 0.6, 0.6, 1.0];
        e.appearance.text_states = TextStateColors {
            hover: Some([1.0; 4]),
            pressed: Some([1.0, 0.0, 0.0, 1.0]),
            disabled: Some([0.25, 0.25, 0.25, 1.0]),
            focus: Some([0.0, 0.5, 1.0, 1.0]),
            selected: Some([0.0, 1.0, 0.0, 1.0]),
        };
        if id == "disabled" {
            e.enabled = false;
        }
        if id == "selected" {
            e.action = Action::OpenPage("title".into());
        }
        page.elements.push(e);
    }
    if std::env::args().any(|a| a == "--images") {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rvn_cli/template/default/assets");
        for (from, to) in [
            ("backgrounds/title_forest.png", "normal.png"),
            ("backgrounds/clearing_day.png", "hover.png"),
            ("ui/textbox2.png", "disabled.png"),
            ("backgrounds/clearing_finale.png", "selected.png"),
            ("cgs/demo.png", "focus.png"),
        ] {
            std::fs::copy(assets.join(from), root.join("assets").join(to))?;
        }
        for e in &mut doc.pages[0].elements {
            e.asset = Some("normal.png".into());
            e.layout_options.image_fit = ImageFit::Stretch;
            e.appearance.image_states = StateImages {
                hover: Some(ResourceOverride::Path("hover.png".into())),
                pressed: Some(ResourceOverride::Clear),
                disabled: Some(ResourceOverride::Path("disabled.png".into())),
                focus: Some(ResourceOverride::Path("focus.png".into())),
                selected: Some(ResourceOverride::Path("selected.png".into())),
            };
        }
    }
    if std::env::args().any(|a| a == "--images") {
        for (id, x, states) in [
            ("cover_normal", 100.0, false),
            ("cover_states", 1550.0, true),
        ] {
            let mut panel = Element::new(id.into(), Kind::Panel);
            panel.rect = [x, 200.0, 220.0, 540.0];
            panel.asset = Some("normal.png".into());
            panel.layout_options.image_fit = ImageFit::Cover;
            if states {
                panel.appearance.image_states.hover =
                    Some(ResourceOverride::Path("hover.png".into()));
            }
            doc.pages[0].elements.push(panel);
        }
    }
    std::fs::write(root.join("menus.rvnui"), doc.to_json()?)?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Couleurs interactives des textes\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    std::fs::write(
        root.join("test.rvn"),
        "label start\n\"Le texte du menu ne change pas l’histoire.\"\n",
    )?;
    Ok(())
}
