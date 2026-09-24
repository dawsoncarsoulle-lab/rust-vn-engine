//! Long localized labels use font measurement and keep following controls reachable.
use rvn_ui::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);
    std::fs::create_dir(&root)?;
    std::fs::create_dir(root.join("assets"))?;
    let mut doc = Document::from_template(ThemePreset::Sobre);
    let mut column = Element::new("column".into(), Kind::Vertical);
    column.rect = [480.0, 100.0, 960.0, 880.0];
    column.layout_options.gap = 24.0;
    column.normal = [0.0; 4];
    let mut heading = Element::new("heading".into(), Kind::Text);
    heading.rect = [0.0, 0.0, 960.0, 40.0];
    heading.font_size = 48.0;
    heading.text = "Une interface qui s’adapte aux textes longs et aux traductions".into();
    heading.layout_options.auto_height = true;
    let mut button = Element::new("long_button".into(), Kind::Button);
    button.rect = [0.0, 0.0, 960.0, 50.0];
    button.font_size = 36.0;
    button.layout_options.auto_height = true;
    button.text="Retrouver les souvenirs oubliés — à travers les archives, les témoignages et les décisions du personnage. ".repeat(5);
    button.focus_order = 1;
    let mut next = Element::new("after_button".into(), Kind::Button);
    next.rect = [0.0, 0.0, 960.0, 60.0];
    next.text = "Le bouton suivant reste accessible".into();
    next.font_size = 32.0;
    next.focus_order = 2;
    column.children = vec![heading, button, next];
    doc.pages[0].elements = vec![column];
    doc.pages[0].graphs.clear();
    std::fs::write(root.join("menus.rvnui"), doc.to_json()?)?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Hauteur automatique\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    std::fs::write(
        root.join("test.rvn"),
        "label start\n\"Le menu ne modifie pas ce dialogue.\"\n",
    )?;
    Ok(())
}
