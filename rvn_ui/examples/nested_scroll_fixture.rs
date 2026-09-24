//! Reproducible native QA fixture. Refuses to overwrite an existing directory.
use rvn_ui::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Provide a new output directory")?,
    );
    std::fs::create_dir(&root)?;
    std::fs::create_dir(root.join("assets"))?;
    let mut doc = Document::from_template(ThemePreset::ScienceFiction);
    let mut outer = Element::new("outer".into(), Kind::Scroll);
    outer.name = "Zone extérieure".into();
    outer.rect = [100.0, 100.0, 1100.0, 600.0];
    outer.normal = [0.03, 0.10, 0.14, 1.0];
    let mut column = Element::new("column".into(), Kind::Vertical);
    column.rect = [10.0, 10.0, 600.0, 1200.0];
    column.layout_options.gap = 20.0;
    column.normal = [0.0; 4];
    for i in 0..12 {
        let mut button = Element::new(format!("row_{i}"), Kind::Button);
        button.rect = [0.0, 0.0, 570.0, 70.0];
        button.text = format!("Ligne extérieure {}", i + 1);
        column.children.push(button);
    }
    outer.children.push(column);
    let mut inner = Element::new("inner".into(), Kind::Scroll);
    inner.name = "Zone intérieure".into();
    inner.rect = [650.0, 40.0, 420.0, 240.0];
    inner.normal = [0.08, 0.25, 0.20, 1.0];
    for i in 0..9 {
        let mut button = Element::new(format!("nested_{i}"), Kind::Button);
        button.rect = [10.0, i as f32 * 80.0 + 10.0, 390.0, 65.0];
        button.text = format!("Ligne intérieure {}", i + 1);
        inner.children.push(button);
    }
    outer.children.push(inner);
    let dialogue = std::env::args().any(|a| a == "--dialogue");
    let page = if dialogue {
        doc.pages
            .iter()
            .position(|p| p.role == Some(PageRole::Dialogue))
            .unwrap()
    } else {
        0
    };
    doc.pages[page].elements = vec![outer];
    doc.pages[page].graphs.clear();
    if dialogue {
        let mut text = Element::new("dialogue_text".into(), Kind::Text);
        text.binding = Some("dialogue.text".into());
        text.rect = [100.0, 850.0, 1700.0, 150.0];
        doc.pages[page].elements.push(text);
    }
    doc.validate()?;
    std::fs::write(root.join("menus.rvnui"), doc.to_json()?)?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Test des conteneurs imbriqués\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    std::fs::write(
        root.join("test.rvn"),
        "label start\n\"Scène inchangée pendant le défilement.\"\n",
    )?;
    println!("{}", root.display());
    Ok(())
}
