//! Existing Source lock/recovery writer, exact disk bytes and real graph models.
//! Definitions only. These CPU tests do not supply native editor evidence.
use super::*;
use crate::source_function_library_recovery1023::FunctionLibraryWire;
use rvn_parser::function_library_asset1023::{FunctionLibraryMember,LibraryFunctionAccess};
const A:&str="11111111111141118111111111111111";
const B:&str="22222222222242228222222222222222";
fn id(value:&str)->FunctionLibraryId{FunctionLibraryId::parse(value).unwrap()}
fn asset(owner:&str,members:&[u64])->FunctionLibraryAsset{FunctionLibraryAsset::new(id(owner),members.iter().map(|graph_id|FunctionLibraryMember{graph_id:*graph_id,access:LibraryFunctionAccess::Public,pure:false}).collect()).unwrap()}
fn record(owner:&str,members:&[u64])->Record{Record::new(id(owner),None,Some(LibraryRecoveryState::new(format!("assets/{owner}.rvnlibrary"),FunctionLibraryWire::from_asset(&asset(owner,members)).unwrap()).unwrap())).unwrap()}
struct Fixture{root:PathBuf}
impl Fixture{
 fn new()->Self{let root=std::env::temp_dir().join(format!("rvn-library-dirty1027-{}",unique_name()));fs::create_dir(&root).unwrap();fs::create_dir(root.join("assets")).unwrap();fs::write(root.join("rvn.toml"),"[paths]\nassets=\"assets\"\n").unwrap();fs::write(root.join("story.rvn"),"// actual Saved baseline\nscene label start\n\"Before\"\n").unwrap();Self{root}}
 fn link(&self)->SourceWorkspace{SourceWorkspace::link_scoped_libraries1027(&self.root,&self.root.join("story.rvn"),&[]).unwrap()}
 fn saved(&self)->Vec<Vec<u8>>{["story.rvn","rvn.toml",SIDECAR].iter().map(|p|fs::read(self.root.join(p)).unwrap()).collect()}
}
impl Drop for Fixture{fn drop(&mut self){let _=fs::remove_dir_all(&self.root);}}
fn birth(workspace:&mut SourceWorkspace,owner:&str){let expected=workspace.recovery_cohort_libraries1027().unwrap();let graphs=expected.as_ref().map(|c|c.batch.graphs.clone()).unwrap_or_else(||workspace.project.graphs().to_vec());workspace.replace_recovery_with_library_changes1027(expected.as_ref(),&graphs,&[],&[],&[],&[LibraryRecoveryChange1027::Upsert{before:None,after:record(owner,&[])}],&[],&[],&[],||()).unwrap();}
fn add_named(workspace:&mut SourceWorkspace,owner:&str,name:&str)->(GraphId,SourceLibraryMemberIdentity1027){let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let graph_id=workspace.next_library_member_graph_id1027(Some(&expected)).unwrap();let receipt=workspace.prepare_library_member_identity1027(&id(owner),graph_id,Some(&expected)).unwrap();let mut graphs=expected.batch.graphs.clone();graphs.push(crate::prepare_initial_library_member_in_current1027(id(owner),graph_id,name,&graphs).unwrap());let before=expected.libraries.iter().find(|r|r.id()==&id(owner)).unwrap().clone();let mut members=before.after().unwrap().wire().asset().unwrap().members().iter().map(|m|m.graph_id).collect::<Vec<_>>();members.push(graph_id.get());let after=record(owner,&members);workspace.replace_recovery_with_library_changes1027(Some(&expected),&graphs,&[],&[],&[],&[LibraryRecoveryChange1027::Upsert{before:Some(before),after}],&[graph_id],std::slice::from_ref(&receipt),&[],||()).unwrap();(graph_id,receipt)}
fn add(workspace:&mut SourceWorkspace,owner:&str)->(GraphId,SourceLibraryMemberIdentity1027){add_named(workspace,owner,"NewFunction")}
#[test]
fn library_recovery1027_empty_newborn_with_same_graphs_and_id_reservation_never_becomes_reserve_only_archive(){
 let fixture=Fixture::new();let mut workspace=fixture.link();let graphs=workspace.project.graphs().to_vec();
 let reserved=workspace.next_member_graph_id(&graphs).unwrap();workspace.commit_member_graph_id(reserved).unwrap();workspace.write_recovery(&graphs).unwrap();
 birth(&mut workspace,A);let cohort=workspace.recovery_cohort_libraries1027().unwrap().unwrap();
 assert_eq!(cohort.batch.graphs,graphs);assert_eq!(cohort.libraries.len(),1);assert!(cohort.libraries[0].after().unwrap().wire().asset().unwrap().members().is_empty());
 let saved=fixture.saved();let recovery=fs::read(fixture.root.join(RECOVERY)).unwrap();let high=workspace.graph_id_high_water();
 assert_eq!(workspace.archive_reservation_only_recovery1025(),Ok(false),"the sole Current empty descriptor is editable data even without changed graphs");
 assert_eq!(fixture.saved(),saved);assert_eq!(fs::read(fixture.root.join(RECOVERY)).unwrap(),recovery);assert_eq!(workspace.graph_id_high_water(),high);assert_eq!(workspace.recovery_cohort_libraries1027().unwrap(),Some(cohort.clone()));
 let mut cold=SourceWorkspace::open_scoped_libraries1027(&fixture.root,0).unwrap().unwrap();assert_eq!(cold.archive_reservation_only_recovery1025(),Ok(false));assert_eq!(cold.recovery_cohort_libraries1027().unwrap(),Some(cohort));assert_eq!(fixture.saved(),saved);assert_eq!(fs::read(fixture.root.join(RECOVERY)).unwrap(),recovery);
 assert!(!fixture.root.join(format!("assets/{A}.rvnlibrary")).exists());assert!(!fixture.root.join(JOURNAL).exists());assert!(!fixture.root.join(refactor::REFACTOR_JOURNAL).exists());
}
#[test]
fn library_recovery1027_birth_and_member_edit_use_one_cache_and_never_publish_saved(){
 let fixture=Fixture::new();let mut workspace=fixture.link();let saved=fixture.saved();birth(&mut workspace,A);let(graph_id,_)=add(&mut workspace,A);assert_eq!(fixture.saved(),saved);assert!(!fixture.root.join(format!("assets/{A}.rvnlibrary")).exists());let cohort=workspace.recovery_cohort_libraries1027().unwrap().unwrap();assert_eq!(cohort.libraries.len(),1);assert!(cohort.batch.graphs.iter().any(|g|g.graph_id==graph_id));let raw:serde_json::Value=serde_json::from_str(workspace.expected_recovery.as_deref().unwrap()).unwrap();assert_eq!(raw["version"],5);assert_eq!(raw["libraries"].as_array().unwrap().len(),1);assert!(workspace.recovery_batch().is_err(),"old consumers may not silently flatten Library records");let prepared=workspace.prepare_library_recovery_current1027().unwrap();assert_eq!(prepared.assets()[0].members()[0].graph_id,graph_id.get());assert_eq!(fixture.saved(),saved);
}
#[test]
fn library_recovery1027_delete_restore_keeps_actual_id_and_rejects_foreign_or_cancelled_receipts(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);let(graph_id,receipt)=add(&mut workspace,A);let before=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let original_graphs=before.batch.graphs.clone();let graphs=original_graphs.iter().filter(|g|g.graph_id!=graph_id).cloned().collect::<Vec<_>>();let record_before=before.libraries[0].clone();let empty=record(A,&[]);workspace.replace_recovery_with_library_changes1027(Some(&before),&graphs,&[],&[],&[],&[LibraryRecoveryChange1027::Upsert{before:Some(record_before.clone()),after:empty.clone()}],&[],&[],&[],||()).unwrap();let removed=workspace.recovery_cohort_libraries1027().unwrap().unwrap();assert!(workspace.prepare_library_member_identity1027(&id(A),graph_id,Some(&removed)).is_err());let mut cold=SourceWorkspace::open_scoped_libraries1027(&fixture.root,0).unwrap().unwrap();let cold_expected=cold.recovery_cohort_libraries1027().unwrap().unwrap();let cache=fs::read(fixture.root.join(RECOVERY)).unwrap();let mut called=false;assert!(cold.replace_recovery_with_library_changes1027(Some(&cold_expected),&original_graphs,&[],&[],&[],&[LibraryRecoveryChange1027::Upsert{before:Some(empty.clone()),after:record_before.clone()}],&[],std::slice::from_ref(&receipt),&[],||{called=true}).is_err());assert!(!called);assert_eq!(fs::read(fixture.root.join(RECOVERY)).unwrap(),cache);
 workspace.replace_recovery_with_library_changes1027(Some(&removed),&original_graphs,&[],&[],&[],&[LibraryRecoveryChange1027::Upsert{before:Some(empty),after:record_before}],&[],std::slice::from_ref(&receipt),&[],||()).unwrap();assert_eq!(workspace.recovery_cohort_libraries1027().unwrap().unwrap().batch.graphs,original_graphs);assert_eq!(workspace.graph_id_high_water(),removed.high_water1027());
}
#[test]
fn library_recovery1027_cancelled_plan_cannot_restore_an_identity_committed_by_another_plan(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let next=workspace.next_library_member_graph_id1027(Some(&expected)).unwrap();let cancelled=workspace.prepare_library_member_identity1027(&id(A),next,Some(&expected)).unwrap();let(graph_id,real)=add(&mut workspace,A);assert_eq!(graph_id,next);assert!(!cancelled.committed.load(std::sync::atomic::Ordering::SeqCst));assert!(real.committed.load(std::sync::atomic::Ordering::SeqCst));let before=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let full=before.batch.graphs.clone();let graphs=full.iter().filter(|g|g.graph_id!=graph_id).cloned().collect::<Vec<_>>();let current=before.libraries[0].clone();let empty=record(A,&[]);workspace.replace_recovery_with_library_changes1027(Some(&before),&graphs,&[],&[],&[],&[LibraryRecoveryChange1027::Upsert{before:Some(current.clone()),after:empty.clone()}],&[],&[],&[],||()).unwrap();let removed=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let bytes=fs::read(fixture.root.join(RECOVERY)).unwrap();assert!(workspace.replace_recovery_with_library_changes1027(Some(&removed),&full,&[],&[],&[],&[LibraryRecoveryChange1027::Upsert{before:Some(empty),after:current}],&[],&[cancelled],&[],||panic!("refused plan reached commit")).is_err());assert_eq!(fs::read(fixture.root.join(RECOVERY)).unwrap(),bytes);assert_eq!(workspace.next_library_member_graph_id1027(Some(&removed)).unwrap().get(),graph_id.get()+1);
}
#[test]
fn library_recovery1027_unrelated_autosave_preserves_records_and_failed_frontier_is_atomic(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);let stale=workspace.recovery_cohort_libraries1027().unwrap().unwrap();birth(&mut workspace,B);let current=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let bytes=fs::read(fixture.root.join(RECOVERY)).unwrap();assert!(workspace.replace_recovery_with_library_changes1027(Some(&stale),&stale.batch.graphs,&[],&[],&[],&[],&[],&[],&[],||panic!("stale frontier reached commit")).is_err());assert_eq!(fs::read(fixture.root.join(RECOVERY)).unwrap(),bytes);workspace.write_recovery(&current.batch.graphs).unwrap();assert_eq!(workspace.recovery_cohort_libraries1027().unwrap().unwrap(),current);
}
fn save_candidate(workspace:&SourceWorkspace,owner:&str)->(String,Vec<GraphDocument>,Vec<SourceRefactorFile>){
 let current=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let record=current.libraries.iter().find(|r|r.id()==&id(owner)).unwrap();let asset=record.after().unwrap().wire().asset().unwrap();let members=current.batch.graphs.iter().filter(|g|g.library1027.as_ref().and_then(|m|m.owner.as_ref())==Some(&id(owner))).cloned().collect::<Vec<_>>();let compiled=crate::prepare_library_current_cohort1027(&members,std::slice::from_ref(&asset)).unwrap();let source=compiled.source().to_owned()+workspace.project.source();let script=rvn_parser::parse(&source).unwrap();let resources=crate::LibraryGraphResources1027::prepare(compiled.scopes(),std::slice::from_ref(&asset)).unwrap();// A partial Save publishes selected Current members on the original Saved
 // presentation baseline. Unselected Main/B Current must stay only in recovery.
 let mut published_templates=workspace.project.graphs().to_vec();
 for member in &members{if let Some(saved)=published_templates.iter_mut().find(|g|g.graph_id==member.graph_id){*saved=member.clone();}else{published_templates.push(member.clone());}}
 let graphs=crate::import_library_program1027(&script,&resources,&published_templates).unwrap();let manifest=fs::read_to_string(workspace.root.join("rvn.toml")).unwrap();let name=format!("{owner}.rvnlibrary");let after=format!("{manifest}\n[function_libraries]\n\"{owner}\"=\"{name}\"\n");let mut files=crate::source_function_library_recovery1023::publication_files(std::slice::from_ref(record)).unwrap();files.push(SourceRefactorFile{path:"rvn.toml".into(),before:Some(manifest),after:Some(after)});(source,graphs,files)
}
#[test]
fn library_recovery1027_partial_save_a_keeps_b_and_main_current_dirty_and_cold_reopens_same_records(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);add(&mut workspace,A);birth(&mut workspace,B);add(&mut workspace,B);let current=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let mut dirty=current.batch.graphs.clone();let main=dirty.iter_mut().find(|g|g.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_none()).unwrap();main.nodes.values_mut().next().unwrap().position=[319.,127.];workspace.replace_recovery_with_library_changes1027(Some(&current),&dirty,&[],&[],&[],&[],&[],&[],&[],||()).unwrap();let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let before_saved=workspace.project.graphs().to_vec();let(source,graphs,files)=save_candidate(&workspace,A);let mut committed=false;workspace.save_library_recovery_with1027(&expected,&[id(A)],&source,&graphs,&files,||{committed=true}).unwrap();assert!(committed);let residual=workspace.recovery_cohort_libraries1027().unwrap().unwrap();assert_eq!(residual.batch.graphs,expected.batch.graphs);assert_eq!(residual.libraries,expected.libraries.iter().filter(|r|r.id()!=&id(A)).cloned().collect::<Vec<_>>());assert!(fixture.root.join(format!("assets/{A}.rvnlibrary")).exists());assert!(!fixture.root.join(format!("assets/{B}.rvnlibrary")).exists());let main_saved=workspace.project.graphs().iter().find(|g|g.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_none()).unwrap();assert!(before_saved.contains(main_saved));assert_ne!(main_saved,dirty.iter().find(|g|g.graph_id==main_saved.graph_id).unwrap());assert!(workspace.project.source().contains("// actual Saved baseline"));let cold=SourceWorkspace::open_scoped_libraries1027(&fixture.root,0).unwrap().unwrap();assert_eq!(cold.recovery_cohort_libraries1027().unwrap().unwrap(),residual);
}
#[test]
fn library_recovery1027_partial_save_refuses_unselected_source_edit_before_any_publication(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);add(&mut workspace,A);birth(&mut workspace,B);let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let(source,_,files)=save_candidate(&workspace,A);let source=source.replace("\"Before\"","\"Unauthorized Main edit\"");let script=rvn_parser::parse(&source).unwrap();let selected=expected.libraries.iter().find(|r|r.id()==&id(A)).unwrap().after().unwrap().wire().asset().unwrap();let scopes=script.iter().filter_map(|s|if let rvn_parser::Statement::FunctionLibrary{scope}=s{Some(scope.clone())}else{None}).collect::<Vec<_>>();let resources=crate::LibraryGraphResources1027::prepare(&scopes,std::slice::from_ref(&selected)).unwrap();let graphs=crate::import_library_program1027(&script,&resources,&expected.batch.graphs).unwrap();let saved=fixture.saved();let recovery=fs::read(fixture.root.join(RECOVERY)).unwrap();let error=workspace.save_library_recovery_with1027(&expected,&[id(A)],&source,&graphs,&files,||panic!("unselected edit reached commit")).unwrap_err();assert!(error.contains("unselected"),"{error}");assert_eq!(fixture.saved(),saved);assert_eq!(fs::read(fixture.root.join(RECOVERY)).unwrap(),recovery);assert!(!fixture.root.join(format!("assets/{A}.rvnlibrary")).exists());assert!(!fixture.root.join(refactor::REFACTOR_JOURNAL).exists());
}
#[test]
fn library_recovery1027_structural_dirty_edit_survives_compile_error_without_implicit_repair_or_save(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);let(graph_id,_)=add(&mut workspace,A);let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let mut graphs=expected.batch.graphs.clone();let graph=graphs.iter_mut().find(|g|g.graph_id==graph_id).unwrap();let entry=graph.nodes.values().find(|n|n.kind==crate::NodeKind::FunctionEntry).unwrap().id;let call=graph.add_catalog_node(crate::NodeKind::UnitCall,[256.,128.]).unwrap();let output=graph.pin_by_key(entry,"exec_out").unwrap().id;let input=graph.pin_by_key(call,"exec_in").unwrap().id;graph.connect(output,input).unwrap();crate::validate_library_membership_shape1027(&graphs,&[asset(A,&[graph_id.get()])]).unwrap();let saved=fixture.saved();workspace.replace_recovery_with_library_changes1027(Some(&expected),&graphs,&[],&[],&[],&[],&[],&[],&[],||()).unwrap();let cache=fs::read(fixture.root.join(RECOVERY)).unwrap();assert!(workspace.prepare_library_recovery_current1027().is_err());assert_eq!(workspace.recovery_cohort_libraries1027().unwrap().unwrap().batch.graphs,graphs);assert_eq!(fs::read(fixture.root.join(RECOVERY)).unwrap(),cache);assert_eq!(fixture.saved(),saved);
}
#[test]
fn library_recovery1027_partial_save_journal_replays_exact_half_publication_and_keeps_residual_cohort(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);add(&mut workspace,A);birth(&mut workspace,B);let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let before_source=workspace.project.source().to_owned();let before_sidecar=workspace.expected_sidecar.clone();let before_recovery=workspace.expected_recovery.clone();let before_resources=workspace.library_journal_readset1027();let(source,graphs,additional)=save_candidate(&workspace,A);workspace.save_library_recovery_with1027(&expected,&[id(A)],&source,&graphs,&additional,||()).unwrap();let after_sidecar=workspace.expected_sidecar.clone();let after_recovery=workspace.expected_recovery.clone();let after_resources=workspace.library_journal_readset1027();let residual=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let mut files=vec![SourceRefactorFile{path:"story.rvn".into(),before:Some(before_source),after:Some(source)},SourceRefactorFile{path:SIDECAR.into(),before:before_sidecar,after:after_sidecar}];files.extend(additional);let changes=expected.libraries.iter().filter(|r|r.id()==&id(A)).cloned().map(|before|LibraryRecoveryChange1027::Remove{before}).collect::<Vec<_>>();let wire=serde_json::json!({"source":"story.rvn","graphs":graphs,"files":files,"library_before1027":before_resources,"library_after1027":after_resources,"recovery":{"kind":"source_library_recovery_transition_v5","version":5,"mode":"partial_libraries","before_recovery":before_recovery,"after_recovery":after_recovery,"changes":[],"library1027":{"changes":changes,"saved":[id(A)]}}});
 // Recreate a legal interruption: Source after, every other target before,
 // recovery before. No guessed new metadata or independently rebuilt cache.
 for file in &files{let path=fixture.root.join(&file.path);match &file.before{Some(bytes)=>fs::write(path,bytes).unwrap(),None=>{if path.exists(){fs::remove_file(path).unwrap();}}}}fs::write(fixture.root.join("story.rvn"),files[0].after.as_ref().unwrap()).unwrap();fs::write(fixture.root.join(RECOVERY),before_recovery.as_ref().unwrap()).unwrap();fs::write(fixture.root.join(refactor::REFACTOR_JOURNAL),serde_json::to_vec(&wire).unwrap()).unwrap();let cold=SourceWorkspace::open_scoped_libraries1027(&fixture.root,0).unwrap().unwrap();assert_eq!(cold.recovery_cohort_libraries1027().unwrap().unwrap(),residual);for file in &files{assert_eq!(read_optional(&fixture.root.join(&file.path)).unwrap(),file.after)}assert_eq!(cold.expected_recovery,after_recovery);assert!(!fixture.root.join(refactor::REFACTOR_JOURNAL).exists());
}
#[test]
fn library_recovery1027_partial_save_refuses_foreign_manifest_keys_before_any_file_or_cache_changes(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);add(&mut workspace,A);birth(&mut workspace,B);let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let(source,graphs,files)=save_candidate(&workspace,A);let saved=fixture.saved();let recovery=fs::read(fixture.root.join(RECOVERY)).unwrap();
 for extra in ["\n[project]\ntitle=\"Not part of Save A\"\n","\n[project]\nmain_script=\"another.rvn\"\n","\n[enum_assets]\n\"99999999999949998999999999999999\"=\"foreign.rvnenum\"\n"]{let mut files=files.clone();files.iter_mut().find(|f|f.path=="rvn.toml").unwrap().after.as_mut().unwrap().push_str(extra);let error=workspace.save_library_recovery_with1027(&expected,&[id(A)],&source,&graphs,&files,||panic!("foreign manifest edit reached commit")).unwrap_err();assert!(error.contains("unselected manifest"),"{error}");assert_eq!(fixture.saved(),saved);assert_eq!(fs::read(fixture.root.join(RECOVERY)).unwrap(),recovery);assert_eq!(workspace.recovery_cohort_libraries1027().unwrap().unwrap(),expected);assert!(!fixture.root.join(format!("assets/{A}.rvnlibrary")).exists());assert!(!fixture.root.join(refactor::REFACTOR_JOURNAL).exists());}
}
#[test]
fn library_recovery1027_partial_save_keeps_real_main_placement_dirty_and_cold_restore_keeps_same_saved_map(){
 let fixture=Fixture::new();let mut workspace=fixture.link();let graphs=workspace.project.graphs().to_vec();let main=graphs.iter().find(|g|matches!(g.kind,crate::GraphKind::Label{..})).unwrap().graph_id;let placements=crate::SourceAssetPlacements::from_entries([crate::SourceAssetPlacement{graph_id:main,namespace:crate::SourcePlacementNamespace::SourceGraphs,directory:"MainPendingFolder".into()}]).unwrap();workspace.commit_asset_placement_edit(&None,&graphs,placements.clone(),&[],&[],&[],&[],||()).unwrap();let saved_map=workspace.published_asset_placements.clone();assert_ne!(saved_map,Some(placements.clone()));assert!(workspace.has_pending_asset_placements().unwrap());birth(&mut workspace,A);add(&mut workspace,A);birth(&mut workspace,B);let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let(source,graphs,files)=save_candidate(&workspace,A);workspace.save_library_recovery_with1027(&expected,&[id(A)],&source,&graphs,&files,||()).unwrap();assert_eq!(workspace.published_asset_placements,saved_map);assert_eq!(workspace.asset_placements,Some(placements.clone()));assert!(workspace.has_pending_asset_placements().unwrap());let residual=workspace.recovery_cohort_libraries1027().unwrap().unwrap();assert_eq!(residual.batch.asset_placements,Some(placements.clone()));let mut cold=SourceWorkspace::open_scoped_libraries1027(&fixture.root,0).unwrap().unwrap();assert_eq!(cold.published_asset_placements,saved_map);let actual=cold.recovery_cohort_libraries1027().unwrap().unwrap();assert_eq!(actual,residual);cold.restore_library_recovery_with1027(&actual,|saved,cohort|{assert_eq!(saved,graphs);assert_eq!(cohort,&residual);Ok(())}).unwrap();assert_eq!(cold.published_asset_placements,saved_map);assert_eq!(cold.asset_placements,Some(placements));assert!(cold.has_pending_asset_placements().unwrap());assert_eq!(cold.recovery_cohort_libraries1027().unwrap().unwrap(),residual);
}
#[test]
fn library_recovery1027_saved_member_body_only_keeps_nominal_cohort_and_never_flattens_without_records(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);let(member,_)=add(&mut workspace,A);let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let(source,graphs,files)=save_candidate(&workspace,A);workspace.save_library_recovery_with1027(&expected,&[id(A)],&source,&graphs,&files,||()).unwrap();let saved=fixture.saved();let mut graphs=workspace.project.graphs().to_vec();graphs.iter_mut().find(|g|g.graph_id==member).unwrap().nodes.values_mut().next().unwrap().position=[137.,241.];workspace.write_recovery(&graphs).unwrap();let cohort=workspace.recovery_cohort_libraries1027().unwrap().unwrap();assert!(cohort.libraries.is_empty(),"body-only dirty state must not invent a descriptor change");assert_eq!(cohort.batch.graphs,graphs);assert!(workspace.recovery_batch().unwrap_err().contains("legacy flatten refused"));assert!(workspace.recovery().is_err());workspace.write_recovery(&graphs).unwrap();assert_eq!(workspace.recovery_cohort_libraries1027().unwrap().unwrap(),cohort);assert_eq!(fixture.saved(),saved);let cold=SourceWorkspace::open_scoped_libraries1027(&fixture.root,0).unwrap().unwrap();assert_eq!(cold.recovery_cohort_libraries1027().unwrap().unwrap(),cohort);assert!(cold.recovery_batch().is_err());
}
#[test]
fn library_recovery1027_save_two_libraries_accepts_interleaved_ids_without_reordering_current_models(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);let(a1,_)=add_named(&mut workspace,A,"AFirst");birth(&mut workspace,B);let(b1,_)=add_named(&mut workspace,B,"BFirst");let(a2,_)=add_named(&mut workspace,A,"ASecond");assert!(a1<b1&&b1<a2);let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let members=expected.batch.graphs.iter().filter(|g|g.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_some()).cloned().collect::<Vec<_>>();assert_eq!(members.iter().map(|g|g.graph_id).collect::<Vec<_>>(),vec![a1,b1,a2]);let assets=expected.libraries.iter().map(|r|r.after().unwrap().wire().asset().unwrap()).collect::<Vec<_>>();let prepared=crate::prepare_library_current_cohort1027(&members,&assets).unwrap();let source=prepared.source().to_owned()+workspace.project.source();let script=rvn_parser::parse(&source).unwrap();let resources=crate::LibraryGraphResources1027::prepare(prepared.scopes(),&assets).unwrap();let published=crate::import_library_program1027(&script,&resources,&expected.batch.graphs).unwrap();assert_eq!(published.iter().filter(|g|g.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_some()).map(|g|g.graph_id).collect::<Vec<_>>(),vec![a1,a2,b1]);let manifest=fs::read_to_string(fixture.root.join("rvn.toml")).unwrap();let after=format!("{manifest}\n[function_libraries]\n\"{A}\"=\"{A}.rvnlibrary\"\n\"{B}\"=\"{B}.rvnlibrary\"\n");let mut files=crate::source_function_library_recovery1023::publication_files(&expected.libraries).unwrap();files.push(SourceRefactorFile{path:"rvn.toml".into(),before:Some(manifest),after:Some(after)});workspace.save_library_recovery_with1027(&expected,&[id(A),id(B)],&source,&published,&files,||()).unwrap();let residual=workspace.recovery_cohort_libraries1027().unwrap().unwrap();assert!(residual.libraries.is_empty());assert_eq!(residual.batch.graphs,expected.batch.graphs,"Save must not sort the actual Current model cohort to match Source grouping");assert_eq!(workspace.project.graphs(),published);let cold=SourceWorkspace::open_scoped_libraries1027(&fixture.root,0).unwrap().unwrap();assert_eq!(cold.recovery_cohort_libraries1027().unwrap().unwrap(),residual);
}
#[test]
fn library_recovery1027_same_warm_clone_prepares_only_data_and_stale_clone_cannot_advance_after_original_commit(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let snapshot=workspace.clone();let graph_id=snapshot.next_library_member_graph_id1027(Some(&expected)).unwrap();let receipt=snapshot.prepare_library_member_identity1027(&id(A),graph_id,Some(&expected)).unwrap();assert!(!receipt.committed.load(std::sync::atomic::Ordering::SeqCst));let mut graphs=expected.batch.graphs.clone();graphs.push(crate::prepare_initial_library_member1027(id(A),graph_id,"PreparedFromSnapshot").unwrap());workspace.replace_recovery_with_library_changes1027(Some(&expected),&graphs,&[],&[],&[],&[LibraryRecoveryChange1027::Upsert{before:Some(expected.libraries[0].clone()),after:record(A,&[graph_id.get()])}],&[graph_id],std::slice::from_ref(&receipt),&[],||()).unwrap();assert!(receipt.committed.load(std::sync::atomic::Ordering::SeqCst));assert!(snapshot.next_library_member_graph_id1027(Some(&expected)).is_err());assert!(snapshot.prepare_library_member_identity1027(&id(A),graph_id,Some(&expected)).is_err());assert_eq!(workspace.recovery_cohort_libraries1027().unwrap().unwrap().batch.graphs,graphs);
 // This is the existing SourceWorkspace warm-lineage Clone contract, not an
 // App SourceRc/Project/epoch capability. App must still keep those guards.
}

#[path="source_library_full_discard_tests1030.rs"]
mod full_discard1030;

#[test]
fn library_recovery1031_exact_current_saved_vector_retires_cache_and_returns_genuine_clean_receipt(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);add(&mut workspace,A);
 let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let current=expected.batch.graphs.clone();
 let(source,grouped,files)=save_candidate(&workspace,A);
 assert_eq!(current.len(),grouped.len());for graph in &current{assert_eq!(Some(graph),grouped.iter().find(|g|g.graph_id==graph.graph_id));}
 // The caller chooses the exact Current presentation of those genuine fresh
 // source-compatible graphs. The writer never sorts Current to obtain clean.
 let installed=std::cell::Cell::new(false);
 let(value,publication)=workspace.save_library_recovery_receipt1030(&expected,&[id(A)],&source,&current,&files,||{installed.set(true);73}).unwrap();
 assert_eq!(value,73);assert!(installed.get());assert!(publication.cohort1029().is_none());
 assert!(workspace.recovery_cohort_libraries1027().unwrap().is_none());assert!(workspace.expected_recovery.is_none());assert!(!fixture.root.join(RECOVERY).exists());
 assert_eq!(workspace.project.graphs(),current);assert_eq!(workspace.project.source(),source);assert!(workspace.snapshot_matches_project);assert!(workspace.source_error.is_none());
 for file in &files{assert_eq!(read_optional(&fixture.root.join(&file.path)).unwrap(),file.after);}
 let cold=SourceWorkspace::open_scoped_libraries1027(&fixture.root,0).unwrap().unwrap();
 assert!(cold.recovery_cohort_libraries1027().unwrap().is_none());assert_eq!(cold.project.graphs(),current);assert_eq!(cold.project.source(),source);assert!(cold.snapshot_matches_project);
}

fn interleaved_candidate1031(workspace:&SourceWorkspace,expected:&SourceRecoveryCohortLibrary1027)->(String,Vec<GraphDocument>,Vec<SourceRefactorFile>){
 let members=expected.batch.graphs.iter().filter(|g|g.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_some()).cloned().collect::<Vec<_>>();
 let assets=expected.libraries.iter().map(|r|r.after().unwrap().wire().asset().unwrap()).collect::<Vec<_>>();
 let prepared=crate::prepare_library_current_cohort1027(&members,&assets).unwrap();let source=prepared.source().to_owned()+workspace.project.source();
 let script=rvn_parser::parse(&source).unwrap();let resources=crate::LibraryGraphResources1027::prepare(prepared.scopes(),&assets).unwrap();
 let published=crate::import_library_program1027(&script,&resources,&expected.batch.graphs).unwrap();
 let manifest=fs::read_to_string(workspace.root.join("rvn.toml")).unwrap();
 let after=format!("{manifest}\n[function_libraries]\n\"{A}\"=\"{A}.rvnlibrary\"\n\"{B}\"=\"{B}.rvnlibrary\"\n");
 let mut files=crate::source_function_library_recovery1023::publication_files(&expected.libraries).unwrap();
 files.push(SourceRefactorFile{path:"rvn.toml".into(),before:Some(manifest),after:Some(after)});(source,published,files)
}
#[test]
fn library_recovery1031_interleaved_current_survives_actual_receipt_repeat_autosave_and_cold_reopen_without_false_saved_vector(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);let(a1,_)=add_named(&mut workspace,A,"AFirst");birth(&mut workspace,B);let(b1,_)=add_named(&mut workspace,B,"BFirst");let(a2,_)=add_named(&mut workspace,A,"ASecond");
 let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let(source,published,files)=interleaved_candidate1031(&workspace,&expected);
 let library_ids=|graphs:&[GraphDocument]|graphs.iter().filter(|g|g.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_some()).map(|g|g.graph_id).collect::<Vec<_>>();
 assert_eq!(library_ids(&expected.batch.graphs),vec![a1,b1,a2]);assert_eq!(library_ids(&published),vec![a1,a2,b1]);
 let installed=std::cell::Cell::new(false);let(_,publication)=workspace.save_library_recovery_receipt1030(&expected,&[id(A),id(B)],&source,&published,&files,||installed.set(true)).unwrap();assert!(installed.get());
 let residual=workspace.recovery_cohort_libraries1027().unwrap().unwrap();assert_eq!(publication.cohort1029(),Some(&residual));
 assert!(residual.libraries.is_empty());assert_eq!(residual.batch,expected.batch);assert_eq!(library_ids(&residual.batch.graphs),vec![a1,b1,a2]);
 assert_eq!(workspace.project.graphs(),published);assert_eq!(workspace.project.source(),source);
 for graph in &residual.batch.graphs{assert_eq!(Some(graph),published.iter().find(|g|g.graph_id==graph.graph_id),"every document Saved image is exact; the remaining distinction is the original complete Current vector");}
 let wire:serde_json::Value=serde_json::from_str(workspace.expected_recovery.as_deref().unwrap()).unwrap();assert_eq!(wire["version"],1);assert!(wire.get("libraries").is_none(),"no fake v5-empty descriptor change");
 assert!(workspace.recovery_batch().is_err(),"legacy flattening cannot adopt a typed Library Current");
 let saved=fixture.saved();workspace.write_recovery(&expected.batch.graphs).unwrap();
 assert_eq!(workspace.recovery_cohort_libraries1027().unwrap(),Some(residual.clone()));assert_eq!(fixture.saved(),saved);
 let cold=SourceWorkspace::open_scoped_libraries1027(&fixture.root,0).unwrap().unwrap();
 assert_eq!(cold.recovery_cohort_libraries1027().unwrap(),Some(residual));assert_eq!(cold.project.graphs(),published);assert_eq!(cold.project.source(),source);assert!(cold.recovery_batch().is_err());
}

#[test]
fn library_recovery1031_forged_after_journal_cannot_reorder_the_original_current_vector_before_replay_io(){
 let fixture=Fixture::new();let mut workspace=fixture.link();birth(&mut workspace,A);add(&mut workspace,A);birth(&mut workspace,B);
 let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let before_source=workspace.project.source().to_owned();let before_sidecar=workspace.expected_sidecar.clone();let before_recovery=workspace.expected_recovery.clone();let before_resources=workspace.library_journal_readset1027();
 let(source,published,additional)=save_candidate(&workspace,A);workspace.save_library_recovery_with1027(&expected,&[id(A)],&source,&published,&additional,||()).unwrap();
 let after_sidecar=workspace.expected_sidecar.clone();let after_resources=workspace.library_journal_readset1027();
 let mut altered:serde_json::Value=serde_json::from_str(workspace.expected_recovery.as_deref().unwrap()).unwrap();altered["graphs"].as_array_mut().unwrap().reverse();
 let forged=serde_json::to_string(&altered).unwrap();
 let mut files=vec![SourceRefactorFile{path:"story.rvn".into(),before:Some(before_source),after:Some(source)},SourceRefactorFile{path:SIDECAR.into(),before:before_sidecar,after:after_sidecar}];files.extend(additional);
 let changes=expected.libraries.iter().filter(|r|r.id()==&id(A)).cloned().map(|before|LibraryRecoveryChange1027::Remove{before}).collect::<Vec<_>>();
 let journal=serde_json::json!({"source":"story.rvn","graphs":published,"files":files,"library_before1027":before_resources,"library_after1027":after_resources,"recovery":{"kind":"source_library_recovery_transition_v5","version":5,"mode":"partial_libraries","before_recovery":before_recovery,"after_recovery":forged,"changes":[],"library1027":{"changes":changes,"saved":[id(A)]}}});
 // A real legal interruption: all Saved participants are AFTER, Current cache
 // still BEFORE. Only the journal's proposed AFTER order has been corrupted.
 fs::write(fixture.root.join(RECOVERY),before_recovery.as_ref().unwrap()).unwrap();let bytes=serde_json::to_vec(&journal).unwrap();fs::write(fixture.root.join(refactor::REFACTOR_JOURNAL),&bytes).unwrap();
 let saved=fixture.saved();let recovery=fs::read(fixture.root.join(RECOVERY)).unwrap();let descriptor=fs::read(fixture.root.join(format!("assets/{A}.rvnlibrary"))).unwrap();drop(workspace);
 let error=SourceWorkspace::open_scoped_libraries1027(&fixture.root,0).unwrap_err();
 assert!(error.contains("Partial Library Save lost an unselected Current"),"{error}");
 assert_eq!(fixture.saved(),saved);assert_eq!(fs::read(fixture.root.join(RECOVERY)).unwrap(),recovery);assert_eq!(fs::read(fixture.root.join(format!("assets/{A}.rvnlibrary"))).unwrap(),descriptor);assert_eq!(fs::read(fixture.root.join(refactor::REFACTOR_JOURNAL)).unwrap(),bytes);
}


#[test]
fn library_recovery1031_new_member_with_real_project_globals_preserves_full_source_projection_before_save(){
 let fixture=Fixture::new();
 // Exact authored Source used by the resident App fixture, before its real
 // Library creation/member commands. No native window or receipt is forged.
 let authored="screen form(){return component(\"root\",\"column\",{},[component(\"label\",\"text\",{\"text\":\"Baseline\"},[])])}\ninit {set first = 0\nset second = 0}\nlabel start\nset first = 1\n\"First scene\"\njump next_scene\nlabel next_scene\nset second = 1\n\"Second scene\"\n";
 fs::write(fixture.root.join("story.rvn"),authored).unwrap();let mut workspace=fixture.link();
 birth(&mut workspace,A);let(member,_)=add(&mut workspace,A);
 let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let before=fixture.saved();let recovery=fs::read(fixture.root.join(RECOVERY)).unwrap();
 let(source,published,files)=save_candidate(&workspace,A);
 for graph in &expected.batch.graphs{
  let fresh=published.iter().find(|actual|actual.graph_id==graph.graph_id).expect("the same original GraphId must exist in the actual scoped import");
  if fresh!=graph{
   let old:serde_json::Value=serde_json::to_value(graph).unwrap();let next:serde_json::Value=serde_json::to_value(fresh).unwrap();
   for(key,value)in old.as_object().unwrap(){if next.get(key)!=Some(value){eprintln!("SOURCE1031_FULL_FIELD_DIFF Graph{:?} kind{:?} field={key} before={} after={}",graph.graph_id,graph.kind,value,next.get(key).unwrap_or(&serde_json::Value::Null));}}
  }
  assert_eq!(fresh,graph,"the selected member and all unselected Saved scopes retain complete admitted fields before publication; selected member={member:?}");
 }
 assert_eq!(published.len(),expected.batch.graphs.len());assert_eq!(fixture.saved(),before);assert_eq!(fs::read(fixture.root.join(RECOVERY)).unwrap(),recovery);
 workspace.save_library_recovery_with1027(&expected,&[id(A)],&source,&published,&files,||()).unwrap();
 let cold=SourceWorkspace::open_scoped_libraries1027(&fixture.root,0).unwrap().unwrap();assert_eq!(cold.project.graphs(),published);
}
