//! Real SourceIO fixtures. No App/native/authority acknowledgement is fabricated.
use super::*;
use rvn_parser::enum_asset::{EnumAsset,EnumAssetId};
use rvn_parser::enum_recovery1025::EnumRecoveryRecord1025 as Record;
use rvn_parser::widget_asset::{WidgetBlueprintV1,WidgetGraphRef16};
const ENUM:&str="17411a3e431e4c9388b78a2b9825b130";
const WIDGET:&str="8b72c6c4be2f4def914906d0733a4b11";
const BASE:&str="init {set clicks=0}\nlabel start\n\"Ready\"\nreturn\n";
struct Fixture{root:PathBuf,w:SourceWorkspace}
impl Fixture{
    fn new()->Self{Self::with_source(BASE,None)}
    fn with_source(source:&str,helper:Option<&str>)->Self{
        let root=std::env::temp_dir().join(format!("rvn-reserve-only1025-{}",unique_name()));fs::create_dir_all(root.join("assets/widgets")).unwrap();fs::write(root.join("main.rvn"),source).unwrap();fs::write(root.join("rvn.toml"),"[project]\nmain_script='main.rvn'\n").unwrap();
        if let Some(helper)=helper{fs::write(root.join("helper.rvn"),helper).unwrap();}
        let w=SourceWorkspace::link(&root,&root.join("main.rvn"),&[]).unwrap();Self{root,w}
    }
    fn reserve(&mut self)->u64{let graphs=self.w.project().graphs().to_vec();let id=self.w.next_member_graph_id(&graphs).unwrap();self.w.commit_member_graph_id(id).unwrap();self.w.write_recovery(&graphs).unwrap();id.get()}
    fn bytes(&self)->Vec<Option<Vec<u8>>>{["main.rvn",SIDECAR,RECOVERY,"rvn.toml"].iter().map(|p|match fs::read(self.root.join(p)){Ok(v)=>Some(v),Err(e)if e.kind()==std::io::ErrorKind::NotFound=>None,Err(e)=>panic!("{e}")}).collect()}
    fn value(&self)->serde_json::Value{serde_json::from_slice(&fs::read(self.root.join(RECOVERY)).unwrap()).unwrap()}
    fn write_value(&self,v:&serde_json::Value){fs::write(self.root.join(RECOVERY),serde_json::to_vec(v).unwrap()).unwrap();}
    fn enum_record(&self)->Record{Record::unpublished("assets/Alpha.rvnenum".into(),EnumAsset::new(EnumAssetId::parse(ENUM).unwrap(),"Alpha".into()).unwrap()).unwrap()}
}
impl Drop for Fixture{fn drop(&mut self){if let(Ok(root),Ok(temp))=(self.root.canonicalize(),std::env::temp_dir().canonicalize()){assert_eq!(root.parent(),Some(temp.as_path()));assert!(root.file_name().unwrap().to_string_lossy().starts_with("rvn-reserve-only1025-"));let _=fs::remove_dir_all(root);}}}
#[test]
fn reserve_only1025_actual_mixed_widget_enum_discard_cold_reopen_archives_after_durable_high_water(){
    let mut f=Fixture::new();let baseline=f.w.project().graphs().to_vec();let original=f.bytes();
    let text=format!("screen panel(){{return component(\"root\",\"text\",{{\"text\":\"One\"}},[])}}\n{BASE}");
    let graphs=SourceProject::open_at(&f.root.join("main.rvn"),text,&baseline).unwrap().graphs().to_vec();let id=graphs.iter().find(|g|matches!(&g.kind,crate::GraphKind::Screen{name,..}if name=="panel")).unwrap().graph_id;
    f.w.commit_member_graph_id(id).unwrap();
    let widget=WidgetRecovery{locator:"assets/widgets/panel.rvnwidget".into(),blueprint_json:WidgetBlueprintV1{uuid:WIDGET.into(),source:"main.rvn".into(),screen_graph:WidgetGraphRef16::from_u64(id.get()),members:vec![WidgetGraphRef16::from_u64(id.get())]}.encode().unwrap()};
    f.w.write_recovery_with_widgets(&graphs,&[widget.clone()]).unwrap();let en=f.enum_record();f.w.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Upsert{before:None,after:en.clone()}],&[],||()).unwrap();
    let expected=f.w.recovery_batch().unwrap();let installed=std::cell::Cell::new(false);
    f.w.discard_recovery_cohort_with1025(expected.as_ref(),&baseline,&[],&[],&[EnumRecoveryChange1025::Remove{before:en}],&[],||installed.set(true)).unwrap();assert!(installed.get());
    let batch=f.w.recovery_batch().unwrap().expect("legitimate reserve-only cache after actual mixed discard");assert_eq!(batch.graphs,baseline);assert!(batch.enums.is_empty()&&batch.widgets.is_empty()&&batch.widget_memberships.is_empty());
    assert_eq!(f.bytes()[..2],original[..2]);let cache=fs::read(f.root.join(RECOVERY)).unwrap();let before_sidecar:serde_json::Value=serde_json::from_slice(&original[1].clone().unwrap()).unwrap();
    let mut cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert_eq!(cold.graph_id_high_water(),id.get());assert!(cold.archive_reservation_only_recovery1025().unwrap());
    assert!(cold.recovery_batch().unwrap().is_none());assert_eq!(fs::read(f.root.join("main.rvn")).unwrap(),original[0].clone().unwrap());assert_eq!(fs::read(f.root.join("rvn.toml")).unwrap(),original[3].clone().unwrap());
    let mut after:serde_json::Value=serde_json::from_slice(&fs::read(f.root.join(SIDECAR)).unwrap()).unwrap();assert_eq!(after["graph_id_high_water"],serde_json::json!(id.get()));let mut before=before_sidecar;after.as_object_mut().unwrap().remove("graph_id_high_water");before.as_object_mut().unwrap().remove("graph_id_high_water");assert_eq!(after,before,"only monotone reservation is published");
    let backups=fs::read_dir(f.root.join(".rvn-backups")).unwrap().map(|e|e.unwrap().path().join("recovery.json")).filter(|p|p.exists()).map(|p|fs::read(p).unwrap()).collect::<Vec<_>>();assert!(backups.contains(&cache),"original cache archived byte-for-byte");
    assert!(!f.root.join(&widget.locator).exists());assert!(!f.root.join("assets/Alpha.rvnenum").exists());let stable=fs::read(f.root.join(SIDECAR)).unwrap();
    let mut again=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(!again.archive_reservation_only_recovery1025().unwrap());assert!(again.next_member_graph_id(again.project().graphs()).unwrap().get()>id.get());assert_eq!(fs::read(f.root.join(SIDECAR)).unwrap(),stable);
}
#[test]
fn reserve_only1025_raw_graph_edits_unknown_payload_and_unmarked_counters_never_auto_archive(){
    for case in 0..8{let mut f=Fixture::new();f.reserve();let mut v=f.value();match case{
        0=>{v["graphs"][0]["unknown_future_graph_payload"]=serde_json::json!({"important":1});},
        1=>{v["unknown_future_counter"]=serde_json::json!(7);},
        2=>{v.as_object_mut().unwrap().remove("graph_id_high_water");},
        3=>{v["graph_id_high_water"]=serde_json::Value::Null;},
        4=>{v["asset_placements_after"]=serde_json::json!({"unknown_table":[]});},
        5=>{v["version"]=serde_json::json!(u32::MAX);},
        6=>{let mut graphs=f.w.project().graphs().to_vec();graphs[0].nodes.values_mut().next().unwrap().title_override=Some("Actual edited graph presentation".into());f.w.write_recovery(&graphs).unwrap();v=f.value();},
        _=>{v["graph_id_high_water"]=serde_json::json!(0);},
    }f.write_value(&v);let mut cold=SourceWorkspace::open(&f.root).unwrap().unwrap();let before=f.bytes();assert_ne!(cold.archive_reservation_only_recovery1025(),Ok(true),"case {case} cannot become reservation-only");assert_eq!(f.bytes(),before,"case {case}: no byte was discarded");assert!(!f.root.join(JOURNAL).exists());}
}
#[test]
fn reserve_only1025_typed_enum_payload_retains_actual_recovery_and_durable_reservation(){
    let mut f=Fixture::new();let high=f.reserve();let en=f.enum_record();f.w.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Upsert{before:None,after:en.clone()}],&[],||()).unwrap();let before=f.bytes();
    let mut cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(!cold.archive_reservation_only_recovery1025().unwrap());assert_eq!(f.bytes(),before);assert_eq!(cold.recovery_batch().unwrap().unwrap().enums,vec![en]);assert_eq!(cold.graph_id_high_water(),high);assert!(!f.root.join("assets/Alpha.rvnenum").exists());
}
#[test]
fn reserve_only1025_writer_lock_foreign_cache_source_and_sidecar_conflicts_preserve_before_images(){
    for case in 0..6{let mut f=if case==5{Fixture::with_source(&format!("use \"helper.rvn\"\n{BASE}"),Some("function helper(){return 0}\n"))}else{Fixture::new()};f.reserve();let mut cold=SourceWorkspace::open(&f.root).unwrap().unwrap();let guard=if case==0{Some(lock(&f.root).unwrap())}else{None};
        match case{1=>{let mut v=f.value();v["graph_id_high_water"]=serde_json::json!(cold.graph_id_high_water()+1);f.write_value(&v);},2=>fs::write(f.root.join("main.rvn"),format!("{BASE}// external\n")).unwrap(),3=>{let mut v:serde_json::Value=serde_json::from_slice(&fs::read(f.root.join(SIDECAR)).unwrap()).unwrap();v["external_unknown_metadata"]=serde_json::json!(true);fs::write(f.root.join(SIDECAR),serde_json::to_vec(&v).unwrap()).unwrap();},4=>{fs::write(f.root.join("main.rvn"),format!("{BASE}// refreshed comment only\n")).unwrap();cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert_eq!(cold.project().graphs(),f.w.project().graphs());},5=>{fs::write(f.root.join("helper.rvn"),"function helper(){return 0}\n// changed imported bytes only\n").unwrap();cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert_eq!(cold.project().source(),f.w.project().source());assert_eq!(cold.project().graphs(),f.w.project().graphs());},_=>{}}
        let before=f.bytes();let result=cold.archive_reservation_only_recovery1025();if case<4{assert!(result.is_err(),"case {case}");}else{assert_eq!(result,Ok(false),"refreshed authored/import baseline is not a reservation-only archive");}assert_eq!(f.bytes(),before);assert!(!f.root.join(JOURNAL).exists());drop(guard);
    }
}
#[test]
fn reserve_only1025_existing_reservation_journal_replays_before_or_published_sidecar_without_id_reuse(){
    for stage in 0..2{let mut f=Fixture::new();
        let mut baseline:serde_json::Value=serde_json::from_slice(&fs::read(f.root.join(SIDECAR)).unwrap()).unwrap();baseline["preserved_future_sidecar_metadata"]=serde_json::json!({"value":17});fs::write(f.root.join(SIDECAR),serde_json::to_vec(&baseline).unwrap()).unwrap();f.w=SourceWorkspace::open(&f.root).unwrap().unwrap();
        let high=f.reserve();let source=fs::read(f.root.join("main.rvn")).unwrap();
        let transaction={let _guard=lock(&f.root).unwrap();f.w.prepare_reservation_archive().unwrap()};let wire=serde_json::to_string(&transaction).unwrap();fs::write(f.root.join(JOURNAL),&wire).unwrap();if stage==1{fs::write(f.root.join(SIDECAR),&transaction.after_sidecar).unwrap();}
        let mut cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert_eq!(fs::read(f.root.join("main.rvn")).unwrap(),source);assert!(cold.recovery_batch().unwrap().is_none());assert!(!cold.archive_reservation_only_recovery1025().unwrap());assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>high);let sidecar:serde_json::Value=serde_json::from_slice(&fs::read(f.root.join(SIDECAR)).unwrap()).unwrap();assert_eq!(sidecar["preserved_future_sidecar_metadata"],serde_json::json!({"value":17}));assert!(!f.root.join(JOURNAL).exists());
    }
    // Fault-injected durable sidecar/cache before-images, without a pending
    // journal: same/larger durable reserve must not revive an empty dialog.
    for extra in [0,5]{let mut f=Fixture::new();let high=f.reserve();let cache=fs::read(f.root.join(RECOVERY)).unwrap();
        let mut published:serde_json::Value=serde_json::from_slice(&fs::read(f.root.join(SIDECAR)).unwrap()).unwrap();published["graph_id_high_water"]=serde_json::json!(high+extra);let published=serde_json::to_vec_pretty(&published).unwrap();fs::write(f.root.join(SIDECAR),&published).unwrap();
        let mut cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert_eq!(cold.graph_id_high_water(),high+extra);assert!(cold.next_member_graph_id(cold.project().graphs()).unwrap().get()>high+extra);
        let backups=f.root.join(".rvn-backups");let held=f.root.join("held-backups");if backups.exists(){fs::rename(&backups,&held).unwrap();}fs::write(&backups,"block archive directory").unwrap();
        assert!(cold.archive_reservation_only_recovery1025().is_err());assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(),cache);assert_eq!(fs::read(f.root.join(SIDECAR)).unwrap(),published);assert_eq!(cold.graph_id_high_water(),high+extra);
        fs::remove_file(&backups).unwrap();if held.exists(){fs::rename(&held,&backups).unwrap();}
        assert!(cold.archive_reservation_only_recovery1025().unwrap());assert_eq!(fs::read(f.root.join(SIDECAR)).unwrap(),published,"already-durable sidecar bytes are not rewritten");assert!(cold.recovery_batch().unwrap().is_none());assert!(!cold.archive_reservation_only_recovery1025().unwrap());let again=SourceWorkspace::open(&f.root).unwrap().unwrap();assert!(again.next_member_graph_id(again.project().graphs()).unwrap().get()>high+extra);
    }
}
