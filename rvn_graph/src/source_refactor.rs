//! A callable refactor publishes source, presentation and menu references as
//! one recoverable transaction. Its receipt also supports exact CAS Undo.
use super::*;
pub(super) const REFACTOR_JOURNAL:&str=".rvn-refactor.transaction.json";

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct SourceRefactorFile{pub path:String,pub before:Option<String>,pub after:Option<String>}
#[derive(Clone,Debug)]
pub struct SourceRefactorReceipt{
    pub files:Vec<SourceRefactorFile>,
    pub before_graphs:Vec<GraphDocument>,pub after_graphs:Vec<GraphDocument>,
    pub backup:PathBuf,
}
#[derive(Serialize,Deserialize)]
struct RefactorJournal{source:String,graphs:Vec<GraphDocument>,files:Vec<SourceRefactorFile>}

fn target(root:&Path,relative:&str)->Result<PathBuf,String>{
    if relative==JOURNAL||relative==REFACTOR_JOURNAL||relative==RECOVERY||relative.starts_with(".rvn-backups"){
        return Err("A refactor cannot replace an authoring journal or recovery.".into());
    }
    let path=Path::new(relative);
    if relative.is_empty()||path.components().any(|component|!matches!(component,Component::Normal(_))){return Err("Unsafe file path in refactor metadata".into());}
    let path=root.join(path);let parent=path.parent().ok_or("Missing refactor parent")?.canonicalize().map_err(|error|error.to_string())?;
    if !parent.starts_with(root){return Err("A refactor target escapes the project folder.".into());}
    if fs::symlink_metadata(&path).is_ok_and(|metadata|metadata.file_type().is_symlink()){return Err("A refactor does not follow symbolic links.".into());}
    Ok(parent.join(path.file_name().ok_or("Missing refactor file name")?))
}
fn check_files(root:&Path,files:&[SourceRefactorFile],recovering:bool)->Result<(),String>{
    let mut paths=std::collections::BTreeSet::new();
    for file in files{
        let path=target(root,&file.path)?;
        let key=if cfg!(windows){path.to_string_lossy().to_lowercase()}else{path.to_string_lossy().into_owned()};
        if !paths.insert(key){return Err("A refactor lists the same file more than once.".into());}
        if fs::symlink_metadata(&path).is_ok_and(|metadata|metadata.file_type().is_symlink()){
            return Err("A refactor does not follow symbolic links.".into());
        }
        let current=read_optional(&path)?;
        if current!=file.before&&(!recovering||current!=file.after){return Err(format!("Refactor conflict on {}. Open changes and backups were retained.",path.display()));}
    }Ok(())
}
fn replace(path:&Path,text:&Option<String>)->Result<(),String>{
    match text{Some(text)=>atomic_write(path,text),None=>match fs::remove_file(path){Ok(())=>Ok(()),Err(error)if error.kind()==std::io::ErrorKind::NotFound=>Ok(()),Err(error)=>Err(error.to_string())}}
}
fn apply(root:&Path,journal:&RefactorJournal,recovering:bool)->Result<(),String>{
    check_files(root,&journal.files,recovering)?;
    let mut published=Vec::<&SourceRefactorFile>::new();
    for file in &journal.files{
        let path=target(root,&file.path)?;let current=read_optional(&path)?;
        if current==file.after{continue;}
        let result=if current!=file.before{Err(format!("Refactor conflict on {}",path.display()))}else{replace(&path,&file.after)};
        if let Err(error)=result{
            // Compensate only bytes still owned by this exact transaction.
            // If another writer intervened, retain its edit and the journal.
            let mut failures=Vec::new();
            for previous in published.iter().rev(){let path=target(root,&previous.path)?;
                if read_optional(&path)?!=previous.after{failures.push(path.display().to_string());continue;}
                if let Err(reason)=replace(&path,&previous.before){failures.push(format!("{}: {reason}",path.display()));}
            }
            if failures.is_empty()&&!recovering{let _=fs::remove_file(root.join(REFACTOR_JOURNAL));}
            return Err(format!("{error}. Refactor backups retained. {}",if failures.is_empty(){"Published files restored.".into()}else{format!("Recovery needed: {}",failures.join("; "))}));
        }
        published.push(file);
    }
    fs::remove_file(root.join(REFACTOR_JOURNAL)).map_err(|error|error.to_string())?;Ok(())
}
pub(super) fn recover_refactor(root:&Path)->Result<(),String>{
    let Some(contents)=read_optional(&root.join(REFACTOR_JOURNAL))?else{return Ok(());};
    let journal:RefactorJournal=serde_json::from_str(&contents).map_err(|error|format!("Invalid refactor journal: {error}"))?;
    let source=journal.files.iter().find(|file|file.path==journal.source).and_then(|file|file.after.clone()).ok_or("Refactor journal has no source")?;
    let snapshot:Snapshot=serde_json::from_str(journal.files.iter().find(|file|file.path==SIDECAR).and_then(|file|file.after.as_deref()).ok_or("Refactor journal has no presentation")?).map_err(|error|error.to_string())?;
    let project=SourceProject::open_snapshot(&source_path(root,&journal.source)?,source,&journal.graphs,snapshot.imports)?;
    project.check_imports()?;
    // All targets are checked before touching a single file on recovery.
    apply(root,&journal,true)
}
impl SourceWorkspace{
    pub fn refactor(&mut self,graphs:&[GraphDocument],additional:&[SourceRefactorFile])->Result<SourceRefactorReceipt,String>{
        let _lock=lock(&self.root)?;recover(&self.root)?;
        if let Some(error)=&self.source_error{return Err(error.clone());}
        self.check_recovery_owner()?;
        if self.expected_recovery.is_some(){return Err("Save or discard recovered edits before renaming a declaration.".into());}
        let current=fs::read_to_string(self.source_path()).map_err(|error|error.to_string())?;
        self.check_save_inputs(&self.source_path(),self.project.source(),&self.expected_sidecar)?;
        let mut next=self.project.clone();next.apply_visual(&current,graphs)?;
        let snapshot=Snapshot{version:1,source:self.relative_source.clone(),source_snapshot:next.source().into(),graphs:next.graphs().to_vec(),imports:next.imports_snapshot()};
        let sidecar=serde_json::to_string_pretty(&snapshot).map_err(|error|error.to_string())?;
        let mut files=vec![SourceRefactorFile{path:self.relative_source.clone(),before:Some(current),after:Some(next.source().into())},SourceRefactorFile{path:SIDECAR.into(),before:self.expected_sidecar.clone(),after:Some(sidecar.clone())}];
        let imported:std::collections::BTreeSet<_>=self.project.resolved_source_files()?.into_iter().skip(1).map(|(path,_)|path).collect();
        for file in additional{
            if file.path==self.relative_source||file.path==SIDECAR{return Err("Additional refactor files cannot replace source or presentation.".into());}
            let target=target(&self.root,&file.path)?;let target=target.canonicalize().unwrap_or(target);
            if imported.contains(&target){return Err("Imported RVN sources are read-only in this authoring workspace; refactor their own source explicitly.".into());}
            files.push(file.clone());
        }
        let backup=self.publish_refactor(next.clone(),&files)?;
        let receipt=SourceRefactorReceipt{files,before_graphs:self.project.graphs().to_vec(),after_graphs:next.graphs().to_vec(),backup};
        self.project=next;self.expected_sidecar=Some(sidecar);self.snapshot_matches_project=true;Ok(receipt)
    }
    pub fn replay_refactor(&mut self,receipt:&SourceRefactorReceipt,redo:bool)->Result<(),String>{
        let _lock=lock(&self.root)?;recover(&self.root)?;self.check_recovery_owner()?;
        if self.expected_recovery.is_some(){return Err("Recovered edits must be resolved before replaying a refactor.".into());}
        self.check_save_inputs(&self.source_path(),self.project.source(),&self.expected_sidecar)?;
        let files:Vec<_>=receipt.files.iter().map(|file|if redo{file.clone()}else{SourceRefactorFile{path:file.path.clone(),before:file.after.clone(),after:file.before.clone()}}).collect();
        let source=files.iter().find(|file|file.path==self.relative_source).and_then(|file|file.after.clone()).ok_or("Missing refactor source")?;
        let graphs=if redo{&receipt.after_graphs}else{&receipt.before_graphs};let next=SourceProject::open_at(&self.source_path(),source,graphs)?;
        self.publish_refactor(next.clone(),&files)?;
        self.expected_sidecar=files.iter().find(|file|file.path==SIDECAR).and_then(|file|file.after.clone());
        self.project=next;self.snapshot_matches_project=self.expected_sidecar.is_some();Ok(())
    }
    fn publish_refactor(&self,next:SourceProject,files:&[SourceRefactorFile])->Result<PathBuf,String>{
        self.project.check_imports()?;next.check_imports()?;
        check_files(&self.root,files,false)?;
        let parent=self.root.join(".rvn-backups");fs::create_dir_all(&parent).map_err(|error|error.to_string())?;
        let backup=parent.join(format!("refactor-{}",unique_name()));fs::create_dir(&backup).map_err(|error|error.to_string())?;
        for file in files{if let Some(before)=&file.before{let path=backup.join(&file.path);fs::create_dir_all(path.parent().ok_or("Missing backup folder")?).map_err(|error|error.to_string())?;atomic_write(&path,before)?;}}
        check_files(&self.root,files,false)?;
        let journal=RefactorJournal{source:self.relative_source.clone(),graphs:next.graphs().to_vec(),files:files.to_vec()};
        self.project.check_imports()?;next.check_imports()?;
        atomic_write(&self.root.join(REFACTOR_JOURNAL),&serde_json::to_string(&journal).map_err(|error|error.to_string())?)?;
        apply(&self.root,&journal,false)?;Ok(backup)
    }
}

#[cfg(test)]mod tests{
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture{fn new()->Self{let root=std::env::temp_dir().join(format!("rvn-callable-refactor-{}",unique_name()));fs::create_dir(&root).unwrap();
        fs::write(root.join("story.rvn"),"// unrelated project note\nfunction caption(value) {\n // calculation note\n return value\n}\nscreen form(){return component(\"root\",\"text\",{\"text\":caption(\"caption\")},[])}\nlabel start\n\"Value [caption(4)]\"\nreturn\nlabel untouched\n // leave this tail exact\n \"caption is ordinary text\"\nreturn\n").unwrap();
        fs::write(root.join("menus.rvnui"),"{\"source_screens\":{\"title\":\"form\"}}\n").unwrap();Self(root)}
        fn linked(&self)->SourceWorkspace{SourceWorkspace::link(&self.0,&self.0.join("story.rvn"),&[]).unwrap()}
    }
    impl Drop for Fixture{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
    fn changed(workspace:&SourceWorkspace)->Vec<GraphDocument>{let id=workspace.project().graphs().iter().find(|graph|crate::callable_name(&graph.kind)==Some("caption")).unwrap().graph_id;crate::rename_callable_graphs(workspace.project().graphs(),id,"format_caption").unwrap()}
    #[test]fn publication_and_project_undo_restore_source_sidecar_and_menus_byte_exact(){
        let fixture=Fixture::new();let mut workspace=fixture.linked();let before_source=fs::read(fixture.0.join("story.rvn")).unwrap();let before_sidecar=fs::read(fixture.0.join(SIDECAR)).unwrap();let before_menu=fs::read_to_string(fixture.0.join("menus.rvnui")).unwrap();let graphs=changed(&workspace);
        let receipt=workspace.refactor(&graphs,&[SourceRefactorFile{path:"menus.rvnui".into(),before:Some(before_menu.clone()),after:Some("{\"source_screens\":{\"title\":\"form\"},\"refactor\":true}\n".into())}]).unwrap();
        let after_source=fs::read(fixture.0.join("story.rvn")).unwrap();let after_sidecar=fs::read(fixture.0.join(SIDECAR)).unwrap();let after_menu=fs::read(fixture.0.join("menus.rvnui")).unwrap();
        let source=std::str::from_utf8(&after_source).unwrap();assert!(source.contains("format_caption"));assert!(source.contains("// calculation note"));assert!(source.ends_with("label untouched\n // leave this tail exact\n \"caption is ordinary text\"\nreturn\n"));assert!(receipt.backup.join("story.rvn").exists());assert!(!fixture.0.join(REFACTOR_JOURNAL).exists());
        workspace.replay_refactor(&receipt,false).unwrap();assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(),before_source);assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(),before_sidecar);assert_eq!(fs::read_to_string(fixture.0.join("menus.rvnui")).unwrap(),before_menu);
        workspace.replay_refactor(&receipt,true).unwrap();assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(),after_source);assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(),after_sidecar);assert_eq!(fs::read(fixture.0.join("menus.rvnui")).unwrap(),after_menu);
        let reopened=SourceWorkspace::open(&fixture.0).unwrap().unwrap();assert!(reopened.project().graphs().iter().any(|graph|crate::callable_name(&graph.kind)==Some("format_caption")));
    }
    #[test]fn menu_conflict_refuses_all_files_and_undo_conflict_keeps_the_published_project(){
        let fixture=Fixture::new();let mut workspace=fixture.linked();let before=fs::read(fixture.0.join("story.rvn")).unwrap();let sidecar=fs::read(fixture.0.join(SIDECAR)).unwrap();let graphs=changed(&workspace);
        let file=SourceRefactorFile{path:"menus.rvnui".into(),before:Some("stale menu".into()),after:Some("new menu".into())};assert!(workspace.refactor(&graphs,&[file]).is_err());assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(),before);assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(),sidecar);assert!(!fixture.0.join(REFACTOR_JOURNAL).exists());
        let before_menu=fs::read_to_string(fixture.0.join("menus.rvnui")).unwrap();let receipt=workspace.refactor(&graphs,&[SourceRefactorFile{path:"menus.rvnui".into(),before:Some(before_menu),after:Some("our menu".into())}]).unwrap();let published=fs::read(fixture.0.join("story.rvn")).unwrap();
        fs::write(fixture.0.join("menus.rvnui"),"external writer").unwrap();assert!(workspace.replay_refactor(&receipt,false).is_err());assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(),published);assert_eq!(fs::read_to_string(fixture.0.join("menus.rvnui")).unwrap(),"external writer");
    }
    #[test]fn an_interrupted_refactor_checks_all_targets_before_recovery_and_keeps_external_edits(){
        let fixture=Fixture::new();let mut workspace=fixture.linked();let receipt=workspace.refactor(&changed(&workspace),&[]).unwrap();
        // Simulate source already replaced but presentation not yet replaced.
        let sidecar=receipt.files.iter().find(|file|file.path==SIDECAR).unwrap();replace(&fixture.0.join(SIDECAR),&sidecar.before).unwrap();let journal=RefactorJournal{source:"story.rvn".into(),graphs:receipt.after_graphs.clone(),files:receipt.files.clone()};atomic_write(&fixture.0.join(REFACTOR_JOURNAL),&serde_json::to_string(&journal).unwrap()).unwrap();
        let reopened=SourceWorkspace::open(&fixture.0).unwrap().unwrap();assert_eq!(reopened.project().graphs(),receipt.after_graphs);assert!(!fixture.0.join(REFACTOR_JOURNAL).exists());
        replace(&fixture.0.join(SIDECAR),&sidecar.before).unwrap();fs::write(fixture.0.join("story.rvn"),"// competing writer\nlabel start\nreturn\n").unwrap();atomic_write(&fixture.0.join(REFACTOR_JOURNAL),&serde_json::to_string(&journal).unwrap()).unwrap();let expected=fs::read(fixture.0.join(SIDECAR)).unwrap();
        assert!(SourceWorkspace::open(&fixture.0).is_err());assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(),expected);assert!(fixture.0.join(REFACTOR_JOURNAL).exists());assert!(fs::read_to_string(fixture.0.join("story.rvn")).unwrap().contains("competing writer"));
    }
}
