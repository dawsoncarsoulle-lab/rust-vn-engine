//! User-designed pagination uses the same actions as interaction graphs.
use rvn_ui::*;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let root=std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);
    std::fs::create_dir(&root)?;std::fs::create_dir(root.join("assets"))?;
    let mut doc=Document::from_template(ThemePreset::Sobre);
    let column_order=std::env::args().any(|a|a=="--column-order");
    let templates:std::collections::BTreeSet<_>=doc.pages.iter().flat_map(|p|&p.elements).filter(|e|e.kind==Kind::SaveList).filter_map(|e|e.list.template.clone()).collect();
    for template in templates{let mut number=Element::new("slot_number".into(),Kind::Text);number.binding=Some("save.slot".into());number.rect=[12.0,12.0,90.0,32.0];number.font_size=26.0;doc.components.get_mut(&template).unwrap().children.push(number);}
    for page in doc.pages.iter_mut().filter(|p|matches!(p.role,Some(PageRole::Save|PageRole::Load))){
        page.elements.retain(|e|e.kind==Kind::SaveList);let list=&mut page.elements[0];list.rect=[100.0,150.0,1720.0,710.0];list.list.slots=18;list.list.order=if column_order{ListOrder::Columns}else{ListOrder::Rows};
        let mut title=Element::new("page_number".into(),Kind::Text);title.text="Page".into();title.binding=Some("save.page".into());title.rect=[700.0,40.0,600.0,70.0];page.elements.push(title);
        for(i,(id,text,target))in [("previous","Précédent",SavePage::Previous),("one","1",SavePage::Number(1)),("two","2",SavePage::Number(2)),("three","3",SavePage::Number(3)),("next","Suivant",SavePage::Next)].into_iter().enumerate(){let mut button=Element::new(id.into(),Kind::Button);button.text=text.into();button.rect=[220.0+i as f32*300.0,940.0,260.0,64.0];button.action=Action::SavePage(target);button.appearance.selected=Some([0.0,0.55,0.7,1.0]);page.elements.push(button);}
        // The graph must replace the contradictory shortcut, not run twice.
        page.elements.last_mut().unwrap().action=Action::SavePage(SavePage::Previous);
        page.graphs.push(Graph{id:"next_page".into(),target:Some("next".into()),event:Event::Click,entry:7,nodes:vec![Node{id:7,position:[240.0,100.0],op:Op::Action(Action::SavePage(SavePage::Next)),next:None}]});
    }
    std::fs::write(root.join("menus.rvnui"),doc.to_json()?)?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Pagination dessinée\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    std::fs::write(root.join("test.rvn"),"label start\n\"La pagination ne modifie pas ce dialogue.\"\n")?;Ok(())
}
