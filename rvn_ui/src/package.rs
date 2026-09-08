use crate::*;

/// Portable data-only package. Media remain independent files after import.
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct ThemePackage {pub version:u32,pub document:Document,pub resources:BTreeMap<String,Vec<u8>>}
fn safe_path(path:&str)->Result<(),String>{if path.is_empty()||Path::new(path).components().any(|c|!matches!(c,std::path::Component::Normal(_))){return Err(format!("Chemin de ressource interdit : {path}"))}Ok(())}
fn local_variables(doc:&Document)->BTreeSet<String>{
    fn walk(es:&[Element],names:&mut BTreeSet<String>){for e in es{if let Some(c)=&e.local_control{names.insert(c.variable.clone());}walk(&e.children,names);}}
    let mut names=BTreeSet::new();for p in &doc.pages{walk(&p.elements,&mut names);for g in &p.graphs{for n in &g.nodes{if let Op::Set{variable,..}|Op::Branch{variable,..}=&n.op{if !variable.starts_with("state."){names.insert(variable.clone());}}}}}for e in doc.components.values(){walk(std::slice::from_ref(e),&mut names);}names
}
impl Document {
    pub fn merge_design(&mut self,mut incoming:Document)->Result<(),String>{
        incoming.validate()?;let variables=local_variables(self);let mut number=1;let prefix=loop{let p=format!("import_{number}_");if !self.pages.iter().any(|v|v.id.starts_with(&p))&&!self.styles.keys().any(|id|id.starts_with(&p))&&!self.components.keys().any(|id|id.starts_with(&p))&&!variables.iter().any(|id|id.starts_with(&p)){break p}number+=1;};
        fn action(a:&mut Action,prefix:&str){if let Action::OpenPage(id)=a{*id=format!("{prefix}{id}");}}
        fn elements(es:&mut [Element],prefix:&str){for e in es{if let Some(control)=&mut e.local_control{control.variable=format!("{prefix}{}",control.variable);}if let Some(style)=&mut e.style{*style=format!("{prefix}{style}");}if let Some(component)=&mut e.component{*component=format!("{prefix}{component}");}if let Some(template)=&mut e.list.template{*template=format!("{prefix}{template}");}action(&mut e.action,prefix);elements(&mut e.children,prefix);}}
        for p in &mut incoming.pages{p.id=format!("{prefix}{}",p.id);if self.pages.iter().any(|old|old.role.is_some()&&old.role==p.role){p.role=None}elements(&mut p.elements,&prefix);for g in &mut p.graphs{for n in &mut g.nodes{match &mut n.op{Op::Action(a)=>action(a,&prefix),Op::Set{variable,..}|Op::Branch{variable,..} if !variable.starts_with("state.")=>*variable=format!("{prefix}{variable}"),_=>{}}}}}
        let mut trial=self.clone();for (id,style) in incoming.styles{trial.styles.insert(format!("{prefix}{id}"),style);}for (id,mut e) in incoming.components{elements(std::slice::from_mut(&mut e),&prefix);trial.components.insert(format!("{prefix}{id}"),e);}trial.pages.extend(incoming.pages);trial.validate()?;*self=trial;Ok(())
    }
    pub fn resource_paths(&self)->BTreeSet<String>{
        fn walk(es:&[Element],out:&mut BTreeSet<String>){for e in es{for path in [&e.asset,&e.font,&e.overrides.font].into_iter().flatten(){out.insert(path.clone());}if let Some(ResourceOverride::Path(path))=&e.overrides.image{out.insert(path.clone());}out.extend(e.appearance.image_states.paths().cloned());out.extend(e.overrides.image_states.paths().cloned());walk(&e.children,out)}}
        let mut out=BTreeSet::new();for page in &self.pages{walk(&page.elements,&mut out);for graph in &page.graphs{for node in &graph.nodes{if let Op::Image{path,..}|Op::Sound{path}=&node.op{out.insert(path.clone());}}}}
        for e in self.components.values(){walk(std::slice::from_ref(e),&mut out)}for style in self.styles.values(){out.extend(style.image_states.paths().cloned());if let Some(font)=&style.font{out.insert(font.clone());}if let Some(ResourceOverride::Path(path))=&style.image{out.insert(path.clone());}}out
    }
}
impl ThemePackage {
    pub fn review(&self,existing:&Document)->Result<String,String>{
        self.validate()?;let mut trial=existing.clone();let start=trial.pages.len();trial.merge_design(self.document.clone())?;
        let mut lines=vec!["CONTENU DU DESIGN".into(),format!("{} pages · {} styles · {} composants · {} fichiers",self.document.pages.len(),self.document.styles.len(),self.document.components.len(),self.resources.len()),String::new()];
        for (source,copy) in self.document.pages.iter().zip(&trial.pages[start..]){
            lines.push(format!("{} → {}",source.name,copy.id));
            if source.role.is_some()&&copy.role.is_none(){lines.push("  Rôle déjà utilisé : votre page actuelle reste active.".into());}
        }
        lines.push("\nSTYLES ET COMPOSANTS RENOMMÉS".into());
        let old_variables=local_variables(existing);let added_variables:Vec<_>=local_variables(&trial).difference(&old_variables).cloned().collect();
        if !added_variables.is_empty(){lines.push("Variables d’interface indépendantes :".into());lines.extend(added_variables);}
        for id in trial.styles.keys().filter(|id|!existing.styles.contains_key(*id)){lines.push(id.clone());}
        for id in trial.components.keys().filter(|id|!existing.components.contains_key(*id)){lines.push(id.clone());}
        lines.push("\nRESSOURCES (copies dans un nouveau dossier)".into());
        for(path,bytes)in &self.resources{lines.push(format!("{path} — {} octets",bytes.len()));}
        if self.resources.is_empty(){lines.push("Aucun média inclus".into());}
        lines.push("\nAucun fichier ou élément existant ne sera écrasé.".into());Ok(lines.join("\n"))
    }
    pub fn collect(document:&Document,assets:&Path)->Result<Self,String>{
        document.validate()?;let mut resources=BTreeMap::new();let root=assets.canonicalize().map_err(|e|e.to_string())?;let mut total=0;
        for path in document.resource_paths(){safe_path(&path)?;let file=assets.join(&path).canonicalize().map_err(|e|format!("{path} : {e}"))?;if !file.starts_with(&root){return Err("Ressource hors du dossier Assets".into())}
            let data=std::fs::read(file).map_err(|e|e.to_string())?;total+=data.len();if total>64*1024*1024{return Err("Le modèle dépasse 64 Mo de ressources".into())}resources.insert(path.clone(),data);
            for name in ["LICENSE.txt","OFL.txt","LICENCE.txt"]{let license=Path::new(&path).parent().unwrap_or(Path::new("")).join(name);let key=license.to_string_lossy().to_string();if resources.contains_key(&key){continue}let source=assets.join(&license);if source.exists(){let source=source.canonicalize().map_err(|e|e.to_string())?;if !source.starts_with(&root){return Err("Licence hors du dossier Assets".into())}let bytes=std::fs::read(source).map_err(|e|e.to_string())?;total+=bytes.len();if total>64*1024*1024{return Err("Le modèle dépasse 64 Mo de ressources".into())}resources.insert(key,bytes);}}
        }let package=Self{version:1,document:document.clone(),resources};package.validate()?;Ok(package)
    }
    pub fn from_json(source:&str)->Result<Self,String>{if source.len()>256*1024*1024{return Err("Paquet trop volumineux".into())}let package:Self=serde_json::from_str(source).map_err(|e|e.to_string())?;package.validate()?;Ok(package)}
    pub fn validate(&self)->Result<(),String>{if self.version!=1{return Err("Version de paquet non prise en charge".into())}self.document.validate()?;let mut size=0;for (path,bytes) in &self.resources{safe_path(path)?;size+=bytes.len();}if size>64*1024*1024{return Err("Ressources trop volumineuses".into())}for path in self.document.resource_paths(){if !self.resources.contains_key(&path){return Err(format!("Ressource absente du paquet : {path}"))}}Ok(())}
    /// Import into a fresh namespaced directory: no existing resource is overwritten.
    pub fn import(&self,assets:&Path)->Result<Document,String>{
        self.validate()?;std::fs::create_dir_all(assets).map_err(|e|e.to_string())?;
        // A resource cannot simultaneously be a file and another resource's
        // parent directory. Reject this before creating the import directory.
        for path in self.resources.keys(){let mut parent=Path::new(path).parent();while let Some(p)=parent{if self.resources.contains_key(&p.to_string_lossy().to_string()){return Err(format!("Conflit fichier/dossier dans le paquet : {path}"));}parent=p.parent();}}
        let mut n=1;let directory=loop{let p=assets.join(format!("menu_theme_{n}"));match std::fs::create_dir(&p){Ok(())=>break p,Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>n+=1,Err(e)=>return Err(e.to_string())}};
        let copied=(||->Result<(),String>{for (path,bytes) in &self.resources{let target=directory.join(path);std::fs::create_dir_all(target.parent().unwrap()).map_err(|e|e.to_string())?;std::fs::write(target,bytes).map_err(|e|e.to_string())?;}Ok(())})();
        if let Err(error)=copied{let _=std::fs::remove_dir_all(&directory);return Err(error);}
        let prefix=directory.file_name().unwrap().to_string_lossy();let mut doc=self.document.clone();
        fn remap(es:&mut [Element],prefix:&str){for e in es{for path in [&mut e.asset,&mut e.font,&mut e.overrides.font].into_iter().flatten(){*path=format!("{prefix}/{path}");}if let Some(ResourceOverride::Path(path))=&mut e.overrides.image{*path=format!("{prefix}/{path}");}e.appearance.image_states.remap(prefix);e.overrides.image_states.remap(prefix);remap(&mut e.children,prefix)}}
        for p in &mut doc.pages{remap(&mut p.elements,&prefix);for g in &mut p.graphs{for n in &mut g.nodes{if let Op::Image{path,..}|Op::Sound{path}=&mut n.op{*path=format!("{prefix}/{path}");}}}}
        for e in doc.components.values_mut(){remap(std::slice::from_mut(e),&prefix)}for style in doc.styles.values_mut(){style.image_states.remap(&prefix);if let Some(font)=&mut style.font{*font=format!("{prefix}/{font}");}if let Some(ResourceOverride::Path(path))=&mut style.image{*path=format!("{prefix}/{path}");}}Ok(doc)
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn state_images_are_packaged_remapped_and_never_silently_lost(){let mut d=Document::defaults();d.pages[0].elements[0].appearance.image_states.hover=Some(ResourceOverride::Path("hover.png".into()));d.styles.insert("states".into(),StylePatch{image_states:StateImages{focus:Some(ResourceOverride::Path("focus.png".into())),..Default::default()},..Default::default()});assert_eq!(d.resource_paths(),BTreeSet::from(["hover.png".into(),"focus.png".into()]));let package=ThemePackage{version:1,document:d.clone(),resources:BTreeMap::new()};assert!(package.validate().is_err());let before=d.clone();let mut patch=StateImages::default();patch.disabled=Some(ResourceOverride::Clear);patch.apply(&mut d.pages[0].elements[0].appearance.image_states);assert!(d.pages[0].elements[0].appearance.image_states.hover.is_some());assert_eq!(Document::from_json(&d.to_json().unwrap()).unwrap(),d);assert_ne!(before,d);}
    #[test]fn review_lists_copies_and_role_conflicts_without_mutation(){
        let existing=Document::from_template(ThemePreset::Sobre);let before=existing.clone();
        let p=ThemePackage{version:1,document:Document::from_template(ThemePreset::ScienceFiction),resources:BTreeMap::new()};
        let report=p.review(&existing).unwrap();assert!(report.contains("import_1_title"));assert!(report.contains("Rôle déjà utilisé"));assert!(report.contains("import_1_button"));assert!(report.contains("Aucun média inclus"));assert_eq!(existing,before);
    }
    #[test]fn conflicting_resource_directories_leave_no_partial_import(){
        let root=std::env::temp_dir().join(format!("rvn-package-conflict-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let p=ThemePackage{version:1,document:Document::defaults(),resources:[("ui".into(),vec![1]),("ui/frame.png".into(),vec![2])].into_iter().collect()};
        assert!(p.import(&root).is_err());assert!(!root.join("menu_theme_1").exists());std::fs::remove_dir(root).unwrap();
    }
    #[test]fn complete_design_import_preserves_existing_roles(){let mut d=Document::from_template(ThemePreset::Sobre);let before=d.pages.clone();d.merge_design(Document::from_template(ThemePreset::ScienceFiction)).unwrap();assert_eq!(&d.pages[..before.len()],before.as_slice());assert!(d.pages[before.len()..].iter().all(|p|p.role.is_none()));d.validate().unwrap();}
    #[test]fn rejects_parent_escape(){let mut p=ThemePackage{version:1,document:Document::defaults(),resources:BTreeMap::new()};p.resources.insert("../secret".into(),vec![]);assert!(p.validate().is_err());}
    #[test]fn requires_all_media(){let mut d=Document::defaults();d.pages[0].elements[0].font=Some("fonts/example.ttf".into());let p=ThemePackage{version:1,document:d,resources:BTreeMap::new()};assert!(p.validate().is_err());}
    #[test]fn collects_sound_and_local_image_dependencies(){let mut d=Document::defaults();d.pages[0].elements[0].overrides.image=Some(ResourceOverride::Path("ui/frame.png".into()));d.pages[0].graphs.push(Graph{id:"sound".into(),target:None,event:Event::Open,entry:0,nodes:vec![Node{id:0,position:[0.0;2],op:Op::Sound{path:"sfx/click.wav".into()},next:None}]});let paths=d.resource_paths();assert!(paths.contains("ui/frame.png"));assert!(paths.contains("sfx/click.wav"));let p=ThemePackage{version:1,document:d,resources:BTreeMap::new()};assert!(p.validate().is_err());}
}
