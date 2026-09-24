//! Copy an editor-created design for isolated visual QA; never overwrite a project.
use std::{fs, path::Path};
fn copy_assets(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_assets(&entry.path(), &target.join(entry.file_name()))?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target.join(entry.file_name()))?;
        } else {
            return Err(std::io::Error::other("Unsupported asset file type"));
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let name = args.get(1).ok_or("Design required")?;
    if !["Sobre", "Illustré", "Science-fiction"].contains(&name.as_str()) {
        return Err("Unknown design".into());
    }
    let root = Path::new(args.get(2).ok_or("Fresh output directory required")?);
    let width: u32 = args.get(3).ok_or("Width required")?.parse()?;
    let height: u32 = args.get(4).ok_or("Height required")?.parse()?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/menu-designs")
        .join(name);
    let document = fs::read(source.join("menus.rvnui"))?;
    rvn_ui::Document::from_json(std::str::from_utf8(&document)?)?.validate()?;
    fs::create_dir(root)?;
    copy_assets(&source.join("assets"), &root.join("assets"))?;
    fs::write(root.join("menus.rvnui"), document)?;
    fs::write(root.join("preview.rvn"),"label start\n\"Une interface entièrement personnalisée, avec des accents et une narration sans cartouche de personnage.\"\n")?;
    fs::write(root.join("rvn.toml"),format!("[project]\ntitle = {name:?}\nmain_script = \"preview.rvn\"\nstart_label = \"start\"\n[window]\nwidth = {width}\nheight = {height}\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\nmenus = \"menus.rvnui\"\n"))?;
    Ok(())
}
