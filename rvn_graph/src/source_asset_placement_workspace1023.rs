//! Editorial publication within the existing Source lock and journal only.
use super::*;
#[derive(Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssetPlacementPublication {
    pub before_recovery:Option<String>,
    pub after_recovery:String,
    pub references:Vec<SourceRefactorFile>,
}
pub(super) struct PreparedAssetPlacementEdit {
    pub(super) transaction:Transaction,
    pub(super) high:u64,
    pub(super) published:Option<SourceAssetPlacements>,
    pub(super) after_sidecar:String,
    pub(super) after_recovery:String,
}
pub(super) fn strict_asset_placements<'de,D:serde::Deserializer<'de>>(deserializer:D)->Result<Option<SourceAssetPlacements>,D::Error>{
    SourceAssetPlacements::deserialize(deserializer).map(Some)
}
pub(super) fn validate_snapshot_editorial(snapshot:&Snapshot)->Result<(),String>{
    match (snapshot.version,&snapshot.asset_placements){
        (1,None)=>Ok(()),
        (2,Some(placements))=>placements.validate_graphs(&snapshot.graphs),
        _=>Err("Unsupported or inconsistent Source asset placement snapshot format".into()),
    }
}
pub(super) fn validate_recovery_editorial(recovery:&Recovery)->Result<(),String>{
    if recovery.version==5{
        if recovery.libraries.is_empty(){return Err("Recovery v5 requires typed Library participants".into())}
        library_recovery1027::validate_records1027(&recovery.libraries,&recovery.widgets,&recovery.enums)?;
        enum_recovery1025::validate_records1025(&recovery.enums,&recovery.widgets)?;
        if let Some(placements)=&recovery.asset_placements_after{placements.validate_graphs(&recovery.graphs)?;}return Ok(())
    }
    if !recovery.libraries.is_empty(){return Err("Library participants require Recovery v5; legacy downgrade refused".into())}
    if recovery.version==4{
        if recovery.enums.is_empty(){return Err("Recovery v4 requires typed Enum participants".into());}
        enum_recovery1025::validate_records1025(&recovery.enums,&recovery.widgets)?;
        if let Some(placements)=&recovery.asset_placements_after{placements.validate_graphs(&recovery.graphs)?;}
        return Ok(());
    }
    if !recovery.enums.is_empty(){return Err("Enum participants require Recovery v4; legacy flatten refused".into());}
    match (recovery.version,&recovery.asset_placements_after){
        (1,None)=>Ok(()),
        (2,None)if !recovery.widget_memberships.is_empty()=>Ok(()),
        (3,Some(placements))=>placements.validate_graphs(&recovery.graphs),
        _=>Err("Unsupported or inconsistent Source asset placement recovery format".into()),
    }
}
impl SourceWorkspace {
    pub fn asset_placements(&self)->Option<&SourceAssetPlacements>{self.asset_placements.as_ref()}
    pub fn has_incomplete_source_publication(&self)->bool{self.root.join(JOURNAL).exists()||self.root.join(refactor::REFACTOR_JOURNAL).exists()}
    pub fn check_asset_placement_baseline(&self)->Result<(),String>{self.check_recovery_baseline()?;self.check_recovery_owner()}
    pub fn has_asset_placement_recovery(&self)->Result<bool,String>{Ok(self.recovery_cohort_libraries1027()?.is_some_and(|cohort|cohort.batch.asset_placements.is_some()))}
    pub fn saved_asset_placements(&self)->Result<Option<SourceAssetPlacements>,String>{Ok(self.published_asset_placements.clone())}
    pub fn published_asset_placements(&self)->Option<&SourceAssetPlacements>{self.published_asset_placements.as_ref()}
    pub fn has_pending_asset_placements(&self)->Result<bool,String>{Ok(self.asset_placements!=self.published_asset_placements)}
    pub(super) fn placements_for_graphs(&self,graphs:&[GraphDocument])->Result<Option<SourceAssetPlacements>,String>{
        let ids=graphs.iter().map(|graph|graph.graph_id).collect::<std::collections::BTreeSet<_>>();
        if ids.len()!=graphs.len(){return Err("Source asset placement candidate repeats a graph identity".into());}
        self.asset_placements.as_ref().map(|placements|SourceAssetPlacements::from_entries(placements.entries().filter(|row|ids.contains(&row.graph_id)).cloned())).transpose()
    }
    /// Typed recovery installation, not an editable clone or unguarded setter.
    /// The caller installs prevalidated models under the same writer guard.
    pub fn restore_asset_placement_recovery_with<T>(&mut self,install:impl FnOnce(&[GraphDocument],&SourceRecoveryBatch)->Result<T,String>)->Result<T,String>{
        let _lock=lock(&self.root)?;
        self.check_recovery_baseline()?;self.check_recovery_owner()?;
        let batch=self.recovery_batch()?.ok_or("The owned typed recovery is absent")?;
        let placements=batch.asset_placements.as_ref().ok_or("This recovery has no Source asset placement participant")?;
        placements.validate_graphs(&batch.graphs)?;
        let adopted_placements=placements.clone();
        let result=install(self.project.graphs(),&batch)?;
        self.asset_placements=Some(adopted_placements);self.snapshot_matches_project=false;Ok(result)
    }
    pub(super) fn prepare_asset_placement_edit(&self,expected:&Option<SourceAssetPlacements>,graphs:&[GraphDocument],
        placements:&SourceAssetPlacements,fresh:&[crate::GraphId],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],
        references:&[SourceRefactorFile])->Result<PreparedAssetPlacementEdit,String>{
        if &self.asset_placements!=expected{return Err("Source asset placement preparation is stale".into());}
        if references.iter().any(|file|file.before!=file.after){return Err("Placement references must be read-only exact CAS participants".into());}
        refactor::check_files(&self.root,references,false)?;
        placements.validate_graphs(graphs)?;
        validate_recovery_graph_shapes(graphs)?;
        crate::text_owner::validate_text_owner_edit(self.project.graphs(),graphs)?;
        widget_recovery::validate(widgets,graphs,self.project.graphs(),&self.relative_source)?;
        widget_recovery::validate_memberships(memberships,widgets,graphs,self.project.graphs(),&self.relative_source)?;
        refactor::check_files(&self.root,&widget_recovery::membership_baseline_files(memberships),false)?;
        let previous=self.recovery_batch()?;
        if let Some(previous)=&previous{
            widget_recovery::preserve_identities(previous,graphs,widgets)?;
            widget_recovery::preserve_memberships(previous,graphs,memberships)?;
            if previous.asset_placements.as_ref().is_some_and(|old|Some(old)!=self.asset_placements.as_ref()){
                return Err("Restore or discard the actual placement recovery before replacing it".into());
            }
        }
        let mut high=self.graph_id_high_water();
        for (index,id) in fresh.iter().enumerate(){
            if id.get()<=high||(index>0&&high.checked_add(1)!=Some(id.get()))||!graphs.iter().any(|graph|graph.graph_id==*id){
                return Err("Prepared placement graph identities are stale, unordered or absent".into());
            }
            high=id.get();
        }
        if graphs.iter().any(|graph|graph.graph_id.get()>self.graph_id_high_water()&&!fresh.contains(&graph.graph_id)){
            return Err("A placement candidate contains an undeclared fresh graph identity".into());
        }
        // New authoring identities still use the actual program preparation.
        if !fresh.is_empty(){
            let current=fs::read_to_string(self.source_path()).map_err(|error|error.to_string())?;
            let mut candidate=self.project.clone();candidate.apply_visual(&current,graphs)?;
        }
        let before=self.expected_sidecar.clone().ok_or("Placement adoption requires the real Source sidecar")?;
        let mut snapshot:Snapshot=serde_json::from_str(&before).map_err(|error|error.to_string())?;
        validate_snapshot_editorial(&snapshot)?;
        snapshot.version=2;snapshot.asset_placements=Some(snapshot.asset_placements.unwrap_or_default());
        snapshot.graph_id_high_water=Some(high.max(validated_snapshot_high_water(&snapshot)?));
        let after_sidecar=serde_json::to_string_pretty(&snapshot).map_err(|error|error.to_string())?;
        let enum_records1025=self.recovery_batch()?.map(|batch|batch.enums).unwrap_or_default();
        enum_recovery1025::validate_records1025(&enum_records1025,widgets)?;
        let recovery=Recovery{libraries:Vec::new(),enums:enum_records1025.clone(),graph_id_high_water:Some(high.max(graphs.iter().map(|graph|graph.graph_id.get()).max().unwrap_or(0))),
            version:if enum_records1025.is_empty(){3}else{4},source:self.relative_source.clone(),baseline_source:self.project.source().into(),baseline_sidecar:Some(after_sidecar.clone()),
            graphs:graphs.to_vec(),widgets:widgets.to_vec(),widget_memberships:memberships.to_vec(),asset_placements_after:Some(placements.clone())};
        validate_recovery_editorial(&recovery)?;
        let after_recovery=serde_json::to_string(&recovery).map_err(|error|error.to_string())?;
        let transaction=Transaction{library_readset1027:self.library_journal_readset1027(),reservation_archive:None,
            editorial_publication:Some(AssetPlacementPublication{before_recovery:self.expected_recovery.clone(),after_recovery:after_recovery.clone(),references:references.to_vec()}),
            source:self.relative_source.clone(),before_source:self.project.source().into(),after_source:self.project.source().into(),before_sidecar:Some(before),after_sidecar:after_sidecar.clone()};
        self.project.check_imports()?;self.check_recovery_baseline()?;self.check_recovery_owner()?;
        refactor::check_files(&self.root,references,false)?;
        Ok(PreparedAssetPlacementEdit{transaction,high,published:snapshot.asset_placements,after_sidecar,after_recovery})
    }
    /// A complete candidate publication. All fallible work precedes install;
    /// the existing Source journal makes sidecar+recovery IO restartable.
    pub fn commit_asset_placement_edit<T>(&mut self,expected:&Option<SourceAssetPlacements>,graphs:&[GraphDocument],
        placements:SourceAssetPlacements,fresh:&[crate::GraphId],widgets:&[WidgetRecovery],memberships:&[WidgetMembershipRecovery],
        references:&[SourceRefactorFile],install:impl FnOnce()->T)->Result<T,String>{
        let _lock=lock(&self.root)?;
        recover(&self.root)?;self.check_recovery_baseline()?;self.check_recovery_owner()?;
        let prepared=self.prepare_asset_placement_edit(expected,graphs,&placements,fresh,widgets,memberships,references)?;
        atomic_write(&self.root.join(JOURNAL),&serde_json::to_string(&prepared.transaction).map_err(|error|error.to_string())?)?;
        recover(&self.root)?;
        // No fallible operation, query, IO or VM may follow the model move.
        self.expected_sidecar=Some(prepared.after_sidecar);self.expected_recovery=Some(prepared.after_recovery);
        self.published_asset_placements=prepared.published;
        self.asset_placements=Some(placements);self.graph_id_high_water=prepared.high;self.snapshot_matches_project=false;
        Ok(install())
    }
}
fn ordinary_optional_file(root:&Path,name:&str)->Result<(),String>{
    match fs::symlink_metadata(root.join(name)){
        Ok(metadata)=>{
            #[cfg(windows)]
            {use std::os::windows::fs::MetadataExt;if metadata.file_attributes()&0x400!=0{return Err("Source editorial files may not be reparse points".into());}}
            if !metadata.is_file()||metadata.file_type().is_symlink(){return Err("Source editorial participant is not an ordinary file".into());}Ok(())
        },
        Err(error)if error.kind()==std::io::ErrorKind::NotFound=>Ok(()),Err(error)=>Err(error.to_string()),
    }
}
pub(super) fn recover_asset_placement_publication(root:&Path,transaction:&Transaction,publication:&AssetPlacementPublication)->Result<(),String>{
    if transaction.reservation_archive.is_some()||transaction.before_source!=transaction.after_source{
        return Err("Editorial placement publication may not change the narrative source".into());
    }
    if publication.references.iter().any(|file|file.before!=file.after){return Err("Editorial journal reference is not read-only".into());}
    refactor::check_files(root,&publication.references,false)?;
    for name in [SIDECAR,RECOVERY,JOURNAL]{ordinary_optional_file(root,name)?;}
    let before=transaction.before_sidecar.as_deref().ok_or("Editorial journal has no saved baseline")?;
    let old:Snapshot=serde_json::from_str(before).map_err(|error|error.to_string())?;
    let next:Snapshot=serde_json::from_str(&transaction.after_sidecar).map_err(|error|error.to_string())?;
    validate_snapshot_editorial(&old)?;validate_snapshot_editorial(&next)?;
    if next.version!=2||old.source!=transaction.source||next.source!=transaction.source
        ||old.source_snapshot!=transaction.before_source||next.source_snapshot!=transaction.before_source
        ||next.asset_placements!=Some(old.asset_placements.clone().unwrap_or_default())
        ||validated_snapshot_high_water(&next)?<validated_snapshot_high_water(&old)?{
        return Err("Editorial journal changed its saved definition or dropped identity reservations".into());
    }
    let mut old_fields:serde_json::Value=serde_json::from_str(before).map_err(|error|error.to_string())?;
    let mut next_fields:serde_json::Value=serde_json::from_str(&transaction.after_sidecar).map_err(|error|error.to_string())?;
    for value in [&mut old_fields,&mut next_fields]{let map=value.as_object_mut().ok_or("Invalid editorial sidecar")?;for field in ["version","asset_placements","graph_id_high_water"]{map.remove(field);}}
    if old_fields!=next_fields{return Err("Editorial journal may not replace saved graph/source/import fields".into());}
    let recovery:Recovery=serde_json::from_str(&publication.after_recovery).map_err(|error|error.to_string())?;
    validate_recovery_editorial(&recovery)?;
    if !matches!(recovery.version,3|4)||recovery.source!=transaction.source||recovery.baseline_source!=transaction.after_source
        ||recovery.baseline_sidecar.as_deref()!=Some(transaction.after_sidecar.as_str())
        ||validated_recovery_high_water(&recovery,validated_snapshot_high_water(&next)?)?>validated_snapshot_high_water(&next)?{
        return Err("Editorial recovery does not own its adopted Source baseline and reservation".into());
    }
    enum_recovery1025::validate_records1025(&recovery.enums,&recovery.widgets)?;
    enum_recovery1025::check_baselines1025(root,&recovery.enums,&[],false)?;
    validate_recovery_graph_shapes(&recovery.graphs)?;
    widget_recovery::validate(&recovery.widgets,&recovery.graphs,&next.graphs,&transaction.source)?;
    widget_recovery::validate_memberships(&recovery.widget_memberships,&recovery.widgets,&recovery.graphs,&next.graphs,&transaction.source)?;
    refactor::check_files(root,&widget_recovery::membership_baseline_files(&recovery.widget_memberships),false)?;
    validate_published_snapshot_project(root,&next)?.check_imports()?;
    let sidecar=read_optional(&root.join(SIDECAR))?;let current_recovery=read_optional(&root.join(RECOVERY))?;
    if sidecar.as_deref()!=Some(before)&&sidecar.as_deref()!=Some(transaction.after_sidecar.as_str())
        ||current_recovery!=publication.before_recovery&&current_recovery.as_deref()!=Some(publication.after_recovery.as_str()){
        return Err("Editorial publication conflicts with an external sidecar or recovery; journal retained".into());
    }
    if let Some(previous)=publication.before_recovery.as_deref(){
        let previous:Recovery=serde_json::from_str(previous).map_err(|error|error.to_string())?;validate_recovery_editorial(&previous)?;
        if previous.source!=old.source||previous.baseline_source!=old.source_snapshot||previous.baseline_sidecar.as_deref()!=Some(before){return Err("Editorial publication lost the previous recovery baseline".into());}
        let batch=SourceRecoveryBatch{graphs:previous.graphs,widgets:previous.widgets,widget_memberships:previous.widget_memberships,asset_placements:previous.asset_placements_after,enums:previous.enums};
        if batch.enums!=recovery.enums{return Err("Editorial publication changed unmentioned Enum recovery".into());}
        widget_recovery::preserve_identities(&batch,&recovery.graphs,&recovery.widgets)?;
        widget_recovery::preserve_memberships(&batch,&recovery.graphs,&recovery.widget_memberships)?;
    }
    // Back up original participants before touching either durable target.
    let backup=root.join(".rvn-backups").join(unique_name());fs::create_dir_all(&backup).map_err(|error|error.to_string())?;
    atomic_write(&backup.join("presentation.json"),before)?;
    if let Some(original)=&publication.before_recovery{atomic_write(&backup.join("recovery.json"),original)?;}
    refactor::check_files(root,&publication.references,false)?;
    let source=source_path(root,&transaction.source)?;
    if fs::read_to_string(source).map_err(|error|error.to_string())?!=transaction.before_source
        ||read_optional(&root.join(SIDECAR))?!=sidecar||read_optional(&root.join(RECOVERY))?!=current_recovery{
        return Err("Editorial publication inputs changed before commit; journal retained".into());
    }
    if sidecar.as_deref()!=Some(transaction.after_sidecar.as_str()){atomic_write(&root.join(SIDECAR),&transaction.after_sidecar)?;}
    if read_optional(&root.join(RECOVERY))?!=current_recovery{return Err("Editorial recovery changed after protocol adoption; journal retained".into());}
    if current_recovery.as_deref()!=Some(publication.after_recovery.as_str()){atomic_write(&root.join(RECOVERY),&publication.after_recovery)?;}
    fs::remove_file(root.join(JOURNAL)).map_err(|error|error.to_string())?;Ok(())
}
