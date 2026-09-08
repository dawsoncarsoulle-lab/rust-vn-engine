//! Long custom choices; the last destination is selected through controller input.
use rvn_ui::*;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let root=std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);
    std::fs::create_dir(&root)?;
    std::fs::create_dir(root.join("assets"))?;
    std::fs::write(root.join("menus.rvnui"),Document::from_template(ThemePreset::ScienceFiction).to_json()?)?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Réponses longues à la manette\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    let mut script=String::from("label start\nchoice {\n");
    for i in 0..12{script.push_str(&format!("\"Réponse {} : explorer les archives et retrouver les souvenirs oubliés de cette journée.\" => {{ \"Destination {i} atteinte.\" }}\n",i+1));}
    script.push_str("}\n");
    std::fs::write(root.join("test.rvn"),script)?;
    Ok(())
}
