//! Disposable native fixture: filled, empty and protected card decorations.
use rvn_ui::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);
    std::fs::create_dir(&root)?;
    std::fs::create_dir(root.join("assets"))?;
    let mut doc = Document::from_template(ThemePreset::ScienceFiction);
    let card = doc.add_save_card();
    let template = doc.components.get_mut(&card).unwrap();
    template.children.clear();
    for (i, (condition, text)) in [
        ("save.empty", "EMPLACEMENT VIDE"),
        ("save.filled", "SAUVEGARDE PRESENTE"),
        ("save.locked", "PROTEGEE"),
        ("save.unlocked", "NON PROTEGEE"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut label = Element::new(format!("state_{i}"), Kind::Text);
        label.text = text.into();
        label.rect = [16.0, 24.0 + i as f32 * 52.0, 448.0, 45.0];
        label.visibility_binding = Some(condition.into());
        template.children.push(label);
    }
    let mut filled_only = template.clone();
    filled_only.id = "filled_only".into();
    filled_only.visibility_binding = Some("save.filled".into());
    for child in &mut filled_only.children {
        child.text = format!("FILTRE {}", child.text);
    }
    doc.components.insert("filled_only".into(), filled_only);
    let page = doc.pages.iter_mut().find(|p| p.id == "save").unwrap();
    page.elements.clear();
    let mut list = Element::new("all_cards".into(), Kind::SaveList);
    list.rect = [60.0, 40.0, 1800.0, 630.0];
    list.list.columns = 3;
    list.list.rows = 2;
    list.list.slots = 6;
    list.list.template = Some(card);
    page.elements.push(list.clone());
    list.id = "filtered_cards".into();
    list.rect = [60.0, 720.0, 1800.0, 300.0];
    list.list.rows = 1;
    list.list.columns = 6;
    list.list.template = Some("filled_only".into());
    page.elements.push(list);
    let elements = page.elements.clone();
    doc.pages
        .iter_mut()
        .find(|p| p.id == "load")
        .unwrap()
        .elements = elements;
    std::fs::write(root.join("menus.rvnui"), doc.to_json()?)?;
    std::fs::write(root.join("rvn.toml"), "[project]\ntitle = \"Cartes conditionnelles\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    std::fs::write(
        root.join("test.rvn"),
        "label start\n\"La progression reste intacte.\"\n",
    )?;
    Ok(())
}
