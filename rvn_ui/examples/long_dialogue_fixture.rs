use rvn_ui::*;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let root=std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);std::fs::create_dir(&root)?;std::fs::create_dir(root.join("assets"))?;
    let mut doc=Document::from_template(ThemePreset::Sobre);let page=doc.pages.iter_mut().find(|p|p.role==Some(PageRole::Dialogue)).unwrap();
    for e in &mut page.elements{if e.binding.as_deref()==Some("dialogue.text"){e.rect=[120.0,830.0,1640.0,140.0];if std::env::args().any(|a|a=="--styled-scroll"){e.appearance.scrollbar=ScrollbarStyle{track:[0.0,0.12,0.2,1.0],thumb:[1.0,0.5,0.0,1.0],hover:[0.0,1.0,1.0,1.0],pressed:[1.0,0.0,1.0,1.0],width:30.0,radius:0.0};}}}
    doc.validate()?;std::fs::write(root.join("menus.rvnui"),doc.to_json()?)?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Dialogue long consultable\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    let text=(1..=20).map(|i|format!("Paragraphe {i} : les archives conservent les souvenirs, les émotions et les décisions. Même un long dialogue doit rester lisible, avec ses accents : é, à, ç.\n")).collect::<String>();
    std::fs::write(root.join("test.rvn"),format!("label start\n{}\n\"Court dialogue suivant.\"\n",serde_json::to_string(&text)?))?;Ok(())
}
