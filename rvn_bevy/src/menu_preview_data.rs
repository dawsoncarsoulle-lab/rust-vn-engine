//! Demo data is available only inside a disposable editor-preview directory.
use bevy::prelude::*;
use crate::{project_paths::ProjectPaths,resources::*};
fn isolated_directory(root:&std::path::Path,saves:&std::path::Path)->Option<std::path::PathBuf>{
    let root=root.canonicalize().ok()?;let temp=std::env::temp_dir().canonicalize().ok()?;
    if root.parent()!=Some(temp.as_path())||!root.file_name()?.to_str()?.starts_with("rvn-menu-preview-")||saves!=root.join("saves"){return None;}
    if std::fs::symlink_metadata(saves).is_ok_and(|m|m.file_type().is_symlink()){return None;}
    Some(root)
}

pub(crate) struct MenuPreviewDataPlugin;
impl Plugin for MenuPreviewDataPlugin{
    fn build(&self,app:&mut App){
        if std::env::var("RVN_UI_PREVIEW_DATA").as_deref()==Ok("filled"){
            app.add_systems(Update,seed_demo);
            app.add_systems(OnEnter(VnState::Waiting),seed_history);
        }
    }
}
fn seed_demo(paths:Res<ProjectPaths>,engine:Res<VnEngine>,cgs:Res<CgAssetRegistry>,mut persistent:ResMut<PersistentDataResource>,mut history:ResMut<DialogueHistory>,mut done:Local<bool>){
    if *done{return;}*done=true;
    let allowed=(||{
        let root=isolated_directory(&paths.root,&paths.saves)?;
        std::fs::create_dir_all(&paths.saves).ok()?;
        if !paths.saves.canonicalize().ok()?.starts_with(&root){return None;}
        Some(())
    })().is_some();
    if !allowed{error!("Données de démonstration refusées hors d’un aperçu temporaire isolé");return;}
    let result=(||->Result<(),String>{
        let manager=rvn_core::save::SaveManager::new(&paths.saves,1000).map_err(|e|e.to_string())?;
        for(slot,title)in [(1,"Exemple — Une nouvelle histoire"),(2,"Exemple — Sauvegarde avec un titre plus long et des accents")]{
            if manager.load(slot).is_err(){engine.0.save(&manager,slot,title.into(),"preview.rvn".into()).map_err(|e|e.to_string())?;}
        }
        if persistent.data.last_resume_target.is_none(){let data=manager.load(1).map_err(|e|e.to_string())?;persistent.data.last_resume_target=Some(rvn_core::persistent::LastResumeTarget::manual(1,data.timestamp));}
        let mut ids:Vec<_>=cgs.0.keys().cloned().collect();ids.sort();if let Some(id)=ids.first(){persistent.data.seen_cgs.insert(id.clone());}
        history.add(String::new(),"Exemple de narration sans cartouche de personnage.".into());
        for i in 1..=12{history.add("Personnage de démonstration".into(),format!("Ligne {i} — Voici un dialogue assez long pour vérifier les retours à la ligne, les accents et le défilement. Ces phrases sont des données temporaires de l’aperçu et n’appartiennent pas à votre scénario."));}
        Ok(())
    })();
    if let Err(error)=result{error!("Données de démonstration : {error}");}
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn demo_requires_owned_layout_before_any_write(){
        let temp=std::env::temp_dir().canonicalize().unwrap();let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();let root=temp.join(format!("rvn-menu-preview-isolation-{stamp}"));std::fs::create_dir(&root).unwrap();
        assert!(isolated_directory(&root,&root.join("saves")).is_some());assert!(!root.join("saves").exists());
        assert!(isolated_directory(&root,&temp.join("real-saves")).is_none());assert!(isolated_directory(&temp,&temp.join("saves")).is_none());
        #[cfg(unix)]{std::os::unix::fs::symlink(&temp,root.join("saves")).unwrap();assert!(isolated_directory(&root,&root.join("saves")).is_none());std::fs::remove_file(root.join("saves")).unwrap();}
        std::fs::remove_dir(root).unwrap();
    }
}
fn seed_history(paths:Res<ProjectPaths>,mut history:ResMut<DialogueHistory>,mut done:Local<bool>){
    if *done{return;}*done=true;
    let Ok(root)=paths.root.canonicalize()else{return};let Ok(temp)=std::env::temp_dir().canonicalize()else{return};
    if root.parent()!=Some(temp.as_path())||!root.file_name().is_some_and(|n|n.to_string_lossy().starts_with("rvn-menu-preview-")){return;}
    history.add(String::new(),"Exemple de narration sans nom affiché.".into());
    for i in 1..=12{history.add("Personnage de démonstration".into(),format!("Ligne {i} — Voici un dialogue assez long pour vérifier les retours à la ligne, les accents et le défilement. Ces phrases sont temporaires et n’appartiennent pas à votre scénario."));}
}
