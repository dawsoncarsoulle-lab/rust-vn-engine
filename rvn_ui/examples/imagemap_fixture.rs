//! Three independently clickable zones with the complete custom interface.
use rvn_ui::*;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let root=std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);
    std::fs::create_dir(&root)?;std::fs::create_dir(root.join("assets"))?;
    let media=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../rvn_cli/template/default/assets/backgrounds");
    std::fs::copy(media.join("title_forest.png"),root.join("assets/map.png"))?;
    std::fs::copy(media.join("clearing_day.png"),root.join("assets/hover.png"))?;
    std::fs::write(root.join("menus.rvnui"),Document::from_template(ThemePreset::ScienceFiction).to_json()?)?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Carte interactive et interface personnalisée\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    let mut scene=String::from("label start\nimagemap {\nbackground: \"map.png\"\nhover: \"hover.png\"\n");
    for(i,x)in [100,650,1200].into_iter().enumerate(){scene.push_str(&format!("hotspot {{ name: \"zone_{i}\" area: ({x}, 200, {}, 700) }} => {{ \"Destination {i} atteinte.\" }}\n",x+300));}
    scene.push_str("}\n");std::fs::write(root.join("test.rvn"),scene)?;Ok(())
}
