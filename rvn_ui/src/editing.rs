//! Atomic composition operations used by the visual editor.
use crate::*;

#[derive(Clone,Copy,Debug)]
pub enum Alignment {Left,CenterX,Right,Top,CenterY,Bottom,DistributeX,DistributeY}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum TreePlacement {Inside(Option<String>),Before(String),After(String)}

impl Document {
    /// Reorder or nest a layer without silently changing its rendered position.
    pub fn move_in_tree(&mut self,page:usize,id:&str,placement:&TreePlacement)->Result<(),String>{
        fn parent(es:&[Element],id:&str,owner:Option<&str>)->Option<Option<String>>{for e in es{if e.id==id{return Some(owner.map(str::to_owned));}if let Some(p)=parent(&e.children,id,Some(&e.id)){return Some(p);}}None}
        fn locked(es:&[Element],id:&str,inherited:bool)->bool{es.iter().any(|e|if e.id==id{inherited||e.locked}else{locked(&e.children,id,inherited||e.locked)})}
        let elements=&self.pages.get(page).ok_or("Page absente")?.elements;
        let old_parent=parent(elements,id,None).ok_or("Élément absent")?;
        if locked(elements,id,false){return Err("Cet élément ou son conteneur est verrouillé".into());}
        let next_parent=match placement{TreePlacement::Inside(p)=>p.clone(),TreePlacement::Before(target)|TreePlacement::After(target)=>{if target==id{return Ok(());}parent(elements,target,None).ok_or("Cible absente")?}};
        if next_parent.as_deref().is_some_and(|p|locked(elements,p,false)){return Err("Le conteneur cible est verrouillé".into());}
        let old_rect=self.layout_page(page,self.reference).into_iter().find(|e|e.id==id).map(|e|e.rect);
        let mut trial=self.clone();trial.reparent(page,id,next_parent.as_deref())?;
        if old_parent!=next_parent{if let Some(rect)=old_rect{
            let owner=next_parent.as_deref().and_then(|p|trial.layout_page(page,trial.reference).into_iter().find(|e|e.id==p));
            let origin=owner.as_ref().map(|e|[e.rect[0]+e.layout_options.padding[0],e.rect[1]+e.layout_options.padding[1]]).unwrap_or([0.0;2]);
            let source=trial.find_element_mut(page,id).unwrap();source.rect=[rect[0]-origin[0],rect[1]-origin[1],rect[2],rect[3]];source.anchors=[0.0;4];
        }}
        if let TreePlacement::Before(target)|TreePlacement::After(target)=placement{
            let siblings=if let Some(p)=next_parent{&mut trial.find_element_mut(page,&p).unwrap().children}else{&mut trial.pages[page].elements};
            let source=siblings.iter().position(|e|e.id==id).ok_or("Élément déplacé absent")?;let e=siblings.remove(source);
            let target=siblings.iter().position(|e|e.id==*target).ok_or("Cible absente après déplacement")?;let index=target+usize::from(matches!(placement,TreePlacement::After(_)));siblings.insert(index,e);
        }
        *self=trial;Ok(())
    }
    /// Validate the edited component definitions, not their stale published copies.
    pub fn update_component_workspaces(&mut self,workspaces:&BTreeMap<String,String>)->Result<(),String>{
        let mut trial=self.clone();for page in &self.pages{if let Some(id)=workspaces.get(&page.id){if page.elements.len()!=1{return Err(format!("Composant {id} : conservez une seule racine"));}trial.components.insert(id.clone(),page.elements[0].clone());}}
        trial.validate_authoring(&workspaces.keys().cloned().collect())?;*self=trial;Ok(())
    }
    /// Snapshot an authored element without replacing it or losing page events.
    pub fn capture_component(&mut self,page:usize,element:&str,name:&str)->Result<(),String>{
        let name=name.trim();if name.is_empty(){return Err("Donnez un nom au composant".into());}if self.components.contains_key(name){return Err("Ce nom de composant existe déjà".into());}
        let source=self.find_element(page,element).ok_or("Sélectionnez un élément à transformer en composant")?;
        let mut template=self.resolved_element(source);template.component=None;template.inherit_text=false;template.inherit_action=false;template.inherit_binding=false;
        if let Some(placed)=self.layout_page(page,self.reference).iter().find(|e|e.id==element){template.rect=[0.0,0.0,placed.rect[2],placed.rect[3]];}else{template.rect[0]=0.0;template.rect[1]=0.0;}
        template.anchors=[0.0;4];
        // Components contain visual content and simple actions, not page graphs.
        let mut ids=BTreeSet::new();fn collect(e:&Element,ids:&mut BTreeSet<String>){ids.insert(e.id.clone());for child in &e.children{collect(child,ids);}}collect(&template,&mut ids);
        if self.pages[page].graphs.iter().any(|g|g.target.as_ref().is_some_and(|id|ids.contains(id))){return Err("Ce groupe possède des interactions de page. Conservez-les sur la page et créez un composant visuel sans ces interactions.".into());}
        self.components.insert(name.into(),template);Ok(())
    }
    /// Removing a page must not silently break an existing navigation action.
    pub fn remove_page(&mut self,index:usize)->Result<(),String>{
        if self.pages.len()<=1{return Err("Conservez au moins une page".into());}
        let page=self.pages.get(index).ok_or("Page absente")?;let id=&page.id;
        fn references(es:&[Element],id:&str)->bool{es.iter().any(|e|e.action==Action::OpenPage(id.into())||references(&e.children,id))}
        for (i,p) in self.pages.iter().enumerate(){if i!=index&&(references(&p.elements,id)||p.graphs.iter().any(|g|g.nodes.iter().any(|n|n.op==Op::Action(Action::OpenPage(id.clone()))))){return Err(format!("La page « {} » utilise encore cette destination",p.name));}}
        if self.components.values().any(|e|references(std::slice::from_ref(e),id)){return Err("Un composant utilise encore cette destination".into());}
        self.pages.remove(index);Ok(())
    }
    /// Change the fixed anchor without moving the element at the reference size.
    pub fn set_anchor(&mut self,page:usize,id:&str,anchor:[f32;2])->Result<(),String>{
        if anchor.iter().any(|v|!v.is_finite()||!(0.0..=1.0).contains(v)){return Err("Ancre invalide".into());}
        fn parent<'a>(es:&'a [Element],id:&str,p:Option<&'a Element>)->Option<Option<&'a Element>>{for e in es{if e.id==id{return Some(p)}if let Some(v)=parent(&e.children,id,Some(e)){return Some(v)}}None}
        let source=self.pages.get(page).ok_or("Page absente")?;let parent=parent(&source.elements,id,None).ok_or("Élément absent")?;
        if parent.is_some_and(|p|matches!(p.kind,Kind::Horizontal|Kind::Vertical|Kind::Grid)){return Err("Les ancres sont pilotées par ce conteneur".into());}
        let size=if let Some(parent)=parent{let p=self.layout_page(page,self.reference).into_iter().find(|e|e.id==parent.id).ok_or("Parent masqué")?;[p.rect[2]-p.layout_options.padding[0]-p.layout_options.padding[2],p.rect[3]-p.layout_options.padding[1]-p.layout_options.padding[3]]}else{self.reference};
        let e=self.find_element_mut(page,id).ok_or("Élément absent")?;
        for axis in 0..2{e.rect[axis]+=(e.anchors[axis]-anchor[axis])*size[axis];e.rect[axis+2]+=(e.anchors[axis+2]-e.anchors[axis])*size[axis];e.anchors[axis]=anchor[axis];e.anchors[axis+2]=anchor[axis];}
        Ok(())
    }
    pub fn copy_elements(&self,page:usize,ids:&[String])->Vec<Element>{self.selection_roots(page,ids).iter().filter_map(|id|self.find_element(page,id).cloned()).collect()}
    pub fn paste_elements(&mut self,page:usize,elements:&[Element])->Result<Vec<String>,String>{
        self.paste_elements_into(page,elements,None)
    }
    /// Copy into a container atomically, preserving the one-root component model.
    pub fn paste_elements_into(&mut self,page:usize,elements:&[Element],parent:Option<&str>)->Result<Vec<String>,String>{
        self.paste_elements_into_authoring(page,elements,parent,&BTreeMap::new())
    }
    pub fn paste_elements_into_authoring(&mut self,page:usize,elements:&[Element],parent:Option<&str>,workspaces:&BTreeMap<String,String>)->Result<Vec<String>,String>{
        if elements.is_empty(){return Err("Le presse-papiers de composition est vide".into())}
        if page>=self.pages.len(){return Err("Page absente".into())}
        let mut ids:BTreeSet<String>=self.outline(page).into_iter().map(|e|e.0).collect();
        fn rename(e:&mut Element,ids:&mut BTreeSet<String>){let base=e.id.clone();let mut n=1;while ids.contains(&format!("{base}_copy{n}")){n+=1}e.id=format!("{base}_copy{n}");ids.insert(e.id.clone());for child in &mut e.children{rename(child,ids)}}
        let mut trial=self.clone();let mut created=vec![];
        for source in elements{let mut e=source.clone();rename(&mut e,&mut ids);e.rect[0]+=24.0;e.rect[1]+=24.0;created.push(e.id.clone());trial.pages[page].elements.push(e);}
        if let Some(parent)=parent{for id in &created{trial.reparent(page,id,Some(parent))?;}}
        trial.update_component_workspaces(workspaces)?;*self=trial;Ok(created)
    }
    pub fn selection_roots(&self,page:usize,ids:&[String])->Vec<String>{
        fn walk(es:&[Element],ids:&[String],out:&mut Vec<String>){for e in es{if ids.contains(&e.id){out.push(e.id.clone())}else{walk(&e.children,ids,out)}}}
        let mut out=vec![];if let Some(p)=self.pages.get(page){walk(&p.elements,ids,&mut out)}out
    }
    pub fn align_elements(&mut self,page:usize,ids:&[String],alignment:Alignment)->Result<(),String>{
        if ids.len()<2{return Err("Sélectionnez au moins deux éléments (Maj + clic)".into())}
        fn parent<'a>(es:&'a [Element],id:&str,p:Option<&'a Element>)->Option<Option<&'a Element>>{for e in es{if e.id==id{return Some(p)}if let Some(found)=parent(&e.children,id,Some(e)){return Some(found)}}None}
        let p=self.pages.get(page).ok_or("Page absente")?;
        let parents:Vec<_>=ids.iter().map(|id|parent(&p.elements,id,None).ok_or("Élément absent")).collect::<Result<_,_>>()?;
        if parents.iter().any(|p|p.map(|e|&e.id)!=parents[0].map(|e|&e.id)){return Err("L’alignement nécessite des éléments dans le même conteneur".into())}
        if parents[0].is_some_and(|p|matches!(p.kind,Kind::Horizontal|Kind::Vertical|Kind::Grid)){return Err("La disposition est pilotée par le conteneur : modifiez son espacement".into())}
        if ids.iter().any(|id|self.find_element(page,id).is_some_and(|e|e.locked)){return Err("Déverrouillez les éléments avant de les aligner".into())}
        let layout=self.layout_page(page,self.reference);
        let mut rects:Vec<_>=ids.iter().map(|id|layout.iter().find(|e|&e.id==id).map(|e|(id.clone(),e.rect)).ok_or("Un élément sélectionné est masqué")).collect::<Result<_,_>>()?;
        let axis=usize::from(matches!(alignment,Alignment::Top|Alignment::CenterY|Alignment::Bottom|Alignment::DistributeY));
        let lo=rects.iter().map(|(_,r)|r[axis]).fold(f32::INFINITY,f32::min);
        let hi=rects.iter().map(|(_,r)|r[axis]+r[axis+2]).fold(f32::NEG_INFINITY,f32::max);
        let distribute=matches!(alignment,Alignment::DistributeX|Alignment::DistributeY);
        if distribute&&ids.len()<3{return Err("La répartition nécessite au moins trois éléments".into())}
        rects.sort_by(|a,b|a.1[axis].total_cmp(&b.1[axis]));
        let gap=(hi-lo-rects.iter().map(|(_,r)|r[axis+2]).sum::<f32>())/(ids.len()-1) as f32;
        let mut cursor=lo;
        for (id,r) in rects{let target=match alignment{Alignment::Left|Alignment::Top=>lo,Alignment::Right|Alignment::Bottom=>hi-r[axis+2],Alignment::CenterX|Alignment::CenterY=>(lo+hi-r[axis+2])*0.5,_=>cursor};self.find_element_mut(page,&id).unwrap().rect[axis]+=target-r[axis];cursor+=r[axis+2]+gap;}
        Ok(())
    }
}

#[cfg(test)]mod anchor_tests{
    use super::*;
    #[test]fn fixed_anchor_preserves_reference_geometry_and_moves_with_viewport(){let mut d=Document::defaults();let before=d.layout_page(0,d.reference).into_iter().find(|e|e.id=="button_0").unwrap().rect;d.set_anchor(0,"button_0",[1.0,1.0]).unwrap();let after=d.layout_page(0,d.reference).into_iter().find(|e|e.id=="button_0").unwrap().rect;assert_eq!(before,after);let wide=d.layout_page(0,[2560.0,1080.0]).into_iter().find(|e|e.id=="button_0").unwrap().rect;assert_eq!(wide[0],before[0]+640.0);assert_eq!(wide[1],before[1]);}
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn tree_drop_reorders_and_preserves_geometry_when_nesting(){
        let mut d=Document::defaults();d.pages[0].elements=["a","b","c"].into_iter().map(|id|Element::new(id.into(),Kind::Panel)).collect();
        d.move_in_tree(0,"a",&TreePlacement::Before("c".into())).unwrap();assert_eq!(d.pages[0].elements.iter().map(|e|e.id.as_str()).collect::<Vec<_>>(),vec!["b","a","c"]);
        d.move_in_tree(0,"b",&TreePlacement::After("c".into())).unwrap();assert_eq!(d.pages[0].elements.iter().map(|e|e.id.as_str()).collect::<Vec<_>>(),vec!["a","c","b"]);
        d.find_element_mut(0,"b").unwrap().rect=[300.0,200.0,500.0,400.0];let before=d.layout_page(0,d.reference).into_iter().find(|e|e.id=="a").unwrap().rect;
        d.move_in_tree(0,"a",&TreePlacement::Inside(Some("b".into()))).unwrap();assert_eq!(d.layout_page(0,d.reference).into_iter().find(|e|e.id=="a").unwrap().rect,before);
        let snapshot=d.clone();assert!(d.move_in_tree(0,"b",&TreePlacement::Inside(Some("a".into()))).is_err());assert_eq!(d,snapshot);
        d.find_element_mut(0,"b").unwrap().locked=true;let snapshot=d.clone();assert!(d.move_in_tree(0,"a",&TreePlacement::After("c".into())).is_err());assert_eq!(d,snapshot);
    }
    #[test]fn pagination_component_workspace_allows_style_anchor_and_reparent_edits(){
        let mut d=Document::defaults();let mut root=Element::new("root".into(),Kind::Panel);
        let mut next=Element::new("next".into(),Kind::Button);next.action=Action::SavePage(SavePage::Next);
        root.children.push(next);root.children.push(Element::new("group".into(),Kind::Panel));
        d.components.insert("pager".into(),root.clone());let page=d.pages.len();
        d.pages.push(Page{id:"work".into(),name:"Composant".into(),background:[0.0;4],elements:vec![root],graphs:vec![],role:None});
        let workspaces=[("work".into(),"pager".into())].into_iter().collect();
        assert!(d.validate().is_err());
        d.styles.insert("pager_style".into(),StylePatch{foreground:Some([0.0,1.0,1.0,1.0]),..Default::default()});
        d.find_element_mut(page,"next").unwrap().style=Some("pager_style".into());
        d.update_component_workspaces(&workspaces).unwrap();
        d.set_anchor(page,"next",[1.0,1.0]).unwrap();d.update_component_workspaces(&workspaces).unwrap();
        d.reparent(page,"next",Some("group")).unwrap();d.update_component_workspaces(&workspaces).unwrap();
        assert_eq!(d.components["pager"],d.pages[page].elements[0]);
        assert_eq!(d.components["pager"].children[0].children[0].action,Action::SavePage(SavePage::Next));
    }
    #[test]fn component_workspace_rejects_self_insertion_before_publication(){
        let mut d=Document::defaults();let id=d.add_save_card();let mut root=d.components[&id].clone();let mut child=Element::new("recursive".into(),Kind::Panel);child.component=Some(id.clone());root.children.push(child);d.pages.push(Page{id:"work".into(),name:"Composant".into(),background:[0.0;4],elements:vec![root],graphs:vec![],role:None});let before=d.clone();assert!(d.update_component_workspaces(&[("work".into(),id)].into_iter().collect()).unwrap_err().contains("Cycle"));assert_eq!(d,before);
    }
    #[test]fn capture_component_preserves_original_and_rejects_conflicts(){
        let mut d=Document::defaults();let before=d.pages.clone();d.capture_component(0,"button_0","Mon bouton").unwrap();assert_eq!(d.pages,before);let snapshot=d.clone();assert!(d.capture_component(0,"button_0","Mon bouton").is_err());assert_eq!(d,snapshot);
        d.pages[0].graphs.push(Graph{id:"click".into(),target:Some("button_0".into()),event:Event::Click,entry:0,nodes:vec![Node{id:0,position:[0.0;2],op:Op::Action(Action::Back),next:None}]});assert!(d.capture_component(0,"button_0","Autre bouton").unwrap_err().contains("interactions"));assert!(!d.components.contains_key("Autre bouton"));
    }
    #[test]fn paste_into_component_keeps_one_root_and_fails_atomically(){
        let mut d=Document::defaults();d.pages[0].elements=vec![Element::new("root".into(),Kind::Panel)];
        let copies=vec![Element::new("caption".into(),Kind::Text)];
        let first=d.paste_elements_into(0,&copies,Some("root")).unwrap();
        let second=d.paste_elements_into(0,&copies,Some("root")).unwrap();
        assert_ne!(first,second);assert_eq!(d.pages[0].elements.len(),1);assert_eq!(d.pages[0].elements[0].children.len(),2);
        let before=d.clone();assert!(d.paste_elements_into(0,&copies,Some(&first[0])).is_err());assert_eq!(d,before);
    }
    #[test]fn page_removal_rejects_live_navigation_without_mutation(){let mut d=Document::defaults();let id=d.pages[1].id.clone();d.pages[0].elements[1].action=Action::OpenPage(id);let before=d.clone();assert!(d.remove_page(1).is_err());assert_eq!(before,d);d.pages[0].elements[1].action=Action::None;d.remove_page(1).unwrap();d.validate().unwrap();}
    fn fixture()->Document{let mut d=Document::defaults();d.pages[0].elements=(0..3).map(|i|{let mut e=Element::new(format!("e{i}"),Kind::Button);e.rect=[i as f32*150.0,i as f32*30.0,50.0,20.0];e}).collect();d}
    #[test]fn aligns_without_changing_actions(){let mut d=fixture();let ids=vec!["e0".into(),"e1".into(),"e2".into()];d.align_elements(0,&ids,Alignment::Bottom).unwrap();assert!(d.pages[0].elements.iter().all(|e|e.rect[1]==60.0));assert_eq!(d.pages[0].elements[1].rect[0],150.0);}
    #[test]fn refuses_locked_selection_atomically(){let mut d=fixture();d.pages[0].elements[1].locked=true;let before=d.clone();assert!(d.align_elements(0,&["e0".into(),"e1".into()],Alignment::Left).is_err());assert_eq!(d,before);}
    #[test]fn distributes_equal_gaps(){let mut d=fixture();d.pages[0].elements[1].rect[0]=80.0;d.align_elements(0,&["e0".into(),"e1".into(),"e2".into()],Alignment::DistributeX).unwrap();assert_eq!(d.pages[0].elements[1].rect[0],150.0);}
    #[test]fn paste_generates_unique_ids_and_preserves_source(){let mut d=fixture();let copies=d.copy_elements(0,&["e0".into(),"e1".into()]);let a=d.paste_elements(0,&copies).unwrap();let b=d.paste_elements(0,&copies).unwrap();assert_ne!(a,b);assert_eq!(d.pages[0].elements[0],copies[0]);assert_eq!(d.pages[0].elements.len(),7);d.validate().unwrap();}
    #[test]fn selected_descendant_is_not_copied_twice(){let mut d=fixture();let child=d.pages[0].elements.remove(1);d.pages[0].elements[0].children.push(child);assert_eq!(d.copy_elements(0,&["e0".into(),"e1".into()]).len(),1);}
}
