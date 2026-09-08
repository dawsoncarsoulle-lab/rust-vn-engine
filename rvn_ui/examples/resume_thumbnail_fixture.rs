//! Two different scenes for isolated automatic/quick-save screenshot checks.
use rvn_ui::*;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let root=std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);std::fs::create_dir(&root)?;std::fs::create_dir(root.join("assets"))?;
    let media=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../rvn_cli/template/default/assets/backgrounds");
    for file in ["title_forest.png","clearing_day.png"]{std::fs::copy(media.join(file),root.join("assets").join(file))?;}
    std::fs::write(root.join("menus.rvnui"),Document::from_template(ThemePreset::Sobre).to_json()?)?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Miniatures des sauvegardes\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    std::fs::write(root.join("test.rvn"),"label start\nscene \"title_forest.png\"\n\"Première scène : la miniature doit montrer ce dialogue.\"\nscene \"clearing_day.png\"\n\"Deuxième scène : la sauvegarde rapide doit conserver la première.\"\n")?;Ok(())
}
