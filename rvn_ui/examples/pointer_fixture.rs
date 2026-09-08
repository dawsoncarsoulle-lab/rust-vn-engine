//! Isolated native verification of events on non-button elements.
use rvn_ui::*;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let root=std::path::PathBuf::from(std::env::args().nth(1).ok_or("Dossier requis")?);
    std::fs::create_dir(&root)?;std::fs::create_dir(root.join("assets"))?;
    let mut doc=Document::from_template(ThemePreset::Sobre);
    let page=&mut doc.pages[0];page.elements.clear();page.graphs.clear();
    for(id,kind,x)in [("hover_region",Kind::Panel,180.0),("click_image",Kind::Image,900.0)]{
        let mut e=Element::new(id.into(),kind);e.rect=[x,300.0,600.0,280.0];e.normal=[0.1,0.3,0.4,1.0];page.elements.push(e);
    }
    let mut status=Element::new("status".into(),Kind::Text);status.rect=[180.0,100.0,1400.0,100.0];status.text="Prêt".into();page.elements.push(status);
    let mut shortcut=Element::new("text_shortcut".into(),Kind::Text);shortcut.rect=[180.0,700.0,800.0,100.0];shortcut.text="Ouvrir les réglages".into();shortcut.action=Action::Settings;page.elements.push(shortcut);
    for(id,target,event,value)in [("enter","hover_region",Event::Hover,"Survol"),("leave","hover_region",Event::HoverLeave,"Sortie"),("click","click_image",Event::Click,"Clic image")]{
        page.graphs.push(Graph{id:id.into(),target:Some(target.into()),event,entry:0,nodes:vec![Node{id:0,position:[0.0;2],op:Op::Text{element:"status".into(),value:value.into()},next:None}]});
    }
    if std::env::args().any(|a|a=="--controller"){
        let page=doc.pages.iter_mut().find(|p|p.role==Some(PageRole::QuickActions)).unwrap();page.elements.truncate(1);let e=&mut page.elements[0];e.action=Action::None;e.text="Tester la commande".into();e.rect[2]=400.0;
        for(event,key)in [(Event::Focus,"quick_focused"),(Event::Click,"quick_activated")]{page.graphs.push(Graph{id:key.into(),target:Some(e.id.clone()),event,entry:0,nodes:vec![Node{id:0,position:[0.0;2],op:Op::Set{variable:key.into(),value:true.into()},next:None}]});}
    }
    if std::env::args().any(|a|a=="--disabled-control"){let page=doc.pages.iter_mut().find(|p|p.role==Some(PageRole::QuickActions)).unwrap();page.elements[0].enabled=false;}
    for page in &mut doc.pages{if page.role==Some(PageRole::Settings){for e in &mut page.elements{if matches!(e.kind,Kind::Select|Kind::CheckBox){e.overrides.selected=Some([0.55,0.25,0.0,1.0]);}}}}
    // Exercise a control whose setting comes from its reusable component.
    let settings_page=doc.pages.iter_mut().find(|p|p.role==Some(PageRole::Settings)).unwrap();
    let language=settings_page.elements.iter_mut().find(|e|e.binding.as_deref()==Some("language")).unwrap();
    let mut template=language.clone();template.id="language_template".into();
    doc.components.insert("language_selector".into(),template);
    language.component=Some("language_selector".into());language.inherit_binding=true;language.binding=None;
    settings_page.graphs.push(Graph{id:"inherited_language_changed".into(),target:Some(language.id.clone()),event:Event::ValueChanged,entry:0,nodes:vec![Node{id:0,position:[0.0;2],next:None,op:Op::Set{variable:"language_changed".into(),value:true.into()}}]});
    if std::env::args().any(|a|a=="--error"){
        doc.pages[0].graphs.push(Graph{id:"boucle_a_corriger".into(),target:None,event:Event::Open,entry:4,nodes:vec![
            Node{id:4,position:[300.0,160.0],op:Op::Set{variable:"temporary".into(),value:1.into()},next:Some(9)},
            Node{id:9,position:[600.0,160.0],op:Op::Set{variable:"temporary".into(),value:2.into()},next:Some(4)},
        ]});
    }
    if std::env::args().any(|a|a=="--navigation-loop"){
        let mut other=doc.pages[0].clone();other.id="other".into();other.role=None;other.graphs.clear();doc.pages.push(other);
        for (source,target) in [("title","other"),("other","title")]{let page=doc.pages.iter_mut().find(|p|p.id==source).unwrap();page.graphs.push(Graph{id:"navigation_a_corriger".into(),target:None,event:Event::Open,entry:12,nodes:vec![Node{id:12,position:[300.0,140.0],op:Op::Action(Action::OpenPage(target.into())),next:None}]});}
    }
    doc.validate()?;std::fs::write(root.join("menus.rvnui"),serde_json::to_string_pretty(&doc)?)?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Interactions des éléments visuels\"\nmain_script = \"test.rvn\"\nstart_label = \"start\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    std::fs::write(root.join("test.rvn"),"label start\n\"Le menu ne doit pas avancer cette histoire.\"\n")?;Ok(())
}
