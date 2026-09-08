//! Isolated native verification of controls that never touch narrative settings.
use rvn_ui::*;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let root=std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);std::fs::create_dir(&root)?;std::fs::create_dir(root.join("assets"))?;
    let mut doc=Document::from_template(ThemePreset::Sobre);let page=&mut doc.pages[0];page.elements.clear();page.graphs.clear();
    for (i,kind) in [Kind::CheckBox,Kind::Slider,Kind::Select].into_iter().enumerate(){let mut e=Element::new(format!("local_{i}"),kind);e.text=["Afficher les indications","Intensité de l’effet","Présentation"][i].into();e.rect=[460.0,220.0+i as f32*150.0,1000.0,90.0];e.focus_order=i as i32;let mut control=LocalControl::for_kind(kind,format!("ui.control_{i}"));if kind==Kind::Select{control.options=vec!["Sobre".into(),"Papier".into(),"Science-fiction".into()];control.initial="Sobre".into();}e.local_control=Some(control);page.elements.push(e);page.graphs.push(Graph{id:format!("change_{i}"),target:Some(format!("local_{i}")),event:Event::ValueChanged,entry:0,nodes:vec![Node{id:0,position:[0.0;2],op:Op::Branch{variable:format!("changed_{i}"),equals:true.into(),otherwise:Some(1)},next:Some(2)},Node{id:1,position:[200.0,0.0],op:Op::Set{variable:format!("changed_{i}"),value:true.into()},next:None},Node{id:2,position:[200.0,100.0],op:Op::Set{variable:format!("duplicate_{i}"),value:true.into()},next:None}]});}
    let mut template=doc.pages[0].elements[2].clone();template.id="selector_template".into();
    doc.components.insert("selector".into(),template);
    let selector=&mut doc.pages[0].elements[2];selector.component=Some("selector".into());selector.inherit_binding=true;selector.local_control=None;
    std::fs::write(root.join("menus.rvnui"),doc.to_json()?)?;std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Contrôles locaux\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;std::fs::write(root.join("test.rvn"),"label start\n\"Ce dialogue ne doit pas avancer.\"\n")?;Ok(())
}
