use super::*;
use rvn_parser::enum_asset::{EnumAsset,EnumAssetId};
use rvn_parser::enum_recovery1025::EnumRecoveryRecord1025 as Record;
use rvn_parser::widget_asset::{WidgetBlueprintV1,WidgetGraphRef16};
const A:&str="17411a3e431e4c9388b78a2b9825b130";
const B:&str="797679920f7142be99d9d6fe5553ca34";
const W1:&str="8b72c6c4be2f4def914906d0733a4b11";
const W2:&str="01f6b701df234bd1b43339949000ab22";
const BASE:&str="init {set clicks=0}\nlabel start\n\"Ready\"\nreturn\n";
const ADDED:&str="screen panel_one(){return component(\"root\",\"text\",{\"text\":\"One\"},[])}\nscreen panel_two(){return component(\"root\",\"text\",{\"text\":\"Two\"},[])}\n";
struct Fixture{root:PathBuf,source:SourceWorkspace,baseline:Vec<GraphDocument>,graphs:Vec<GraphDocument>,widgets:Vec<WidgetRecovery>,a:Record,b:Record}
impl Fixture{
    fn new()->Self{
        let root=std::env::temp_dir().join(format!("rvn-mixed-discard1025-{}",unique_name()));fs::create_dir_all(root.join("assets/widgets")).unwrap();fs::write(root.join("main.rvn"),BASE).unwrap();fs::write(root.join("rvn.toml"),"[project]\ntitle='Mixed Discard'\n").unwrap();
        let source=SourceWorkspace::link(&root,&root.join("main.rvn"),&[]).unwrap();let baseline=source.project.graphs().to_vec();assert!(baseline.len()>=2);
        let mut graphs=SourceProject::open_at(&root.join("main.rvn"),format!("{ADDED}{BASE}"),&baseline).unwrap().graphs().to_vec();
        for (index,saved) in baseline.iter().take(2).enumerate(){let graph=graphs.iter_mut().find(|graph|graph.graph_id==saved.graph_id).unwrap();graph.nodes.values_mut().next().unwrap().title_override=Some(format!("Dirty original graph {index}"));}
        let widgets=[(W1,"panel_one"),(W2,"panel_two")].into_iter().map(|(uuid,name)|{
            let id=graphs.iter().find(|graph|matches!(&graph.kind,crate::GraphKind::Screen{name:actual,..} if actual==name)).unwrap().graph_id;
            WidgetRecovery{locator:format!("assets/widgets/{name}.rvnwidget"),blueprint_json:WidgetBlueprintV1{uuid:uuid.into(),source:"main.rvn".into(),screen_graph:WidgetGraphRef16::from_u64(id.get()),members:vec![WidgetGraphRef16::from_u64(id.get())]}.encode().unwrap()}
        }).collect();
        let make=|id:&str,name:&str|Record::unpublished(format!("assets/{name}.rvnenum"),EnumAsset::new(EnumAssetId::parse(id).unwrap(),name.into()).unwrap()).unwrap();
        Self{root,source,baseline,graphs,widgets,a:make(A,"Alpha"),b:make(B,"Beta")}
    }
    fn write(&mut self){
        self.source.write_recovery_with_widgets(&self.graphs,&self.widgets).unwrap();
        for record in [self.a.clone(),self.b.clone()]{self.source.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Upsert{before:None,after:record}],&[],||()).unwrap();}
    }
    fn after_graphs(&self)->Vec<GraphDocument>{
        let discarded=self.widgets[0].blueprint().unwrap().screen_graph.get();
        self.graphs.iter().filter(|graph|graph.graph_id.get()!=discarded).map(|graph|if graph.graph_id==self.baseline[0].graph_id{self.baseline[0].clone()}else{graph.clone()}).collect()
    }
    fn references(&self)->Vec<SourceRefactorFile>{let wire=fs::read_to_string(self.root.join("rvn.toml")).unwrap();vec![SourceRefactorFile{path:"rvn.toml".into(),before:Some(wire.clone()),after:Some(wire)}]}
    fn bytes(&self)->Vec<Option<Vec<u8>>>{["main.rvn",SIDECAR,"rvn.toml",RECOVERY].iter().map(|path|match fs::read(self.root.join(path)){Ok(value)=>Some(value),Err(error)if error.kind()==std::io::ErrorKind::NotFound=>None,Err(error)=>panic!("{error}")}).collect()}
    fn prepared(&self)->PreparedCohortDiscard1025{
        let expected=self.source.recovery_batch().unwrap();self.source.prepare_recovery_cohort_discard1025(expected.as_ref(),&self.after_graphs(),&self.widgets[1..],&[],&[EnumRecoveryChange1025::Remove{before:self.a.clone()}],&self.references()).unwrap()
    }
}
impl Drop for Fixture{fn drop(&mut self){let path=self.root.canonicalize().unwrap();let temp=std::env::temp_dir().canonicalize().unwrap();assert_eq!(path.parent(),Some(temp.as_path()));assert!(path.file_name().unwrap().to_string_lossy().starts_with("rvn-mixed-discard1025-"));let _=fs::remove_dir_all(path);}}
#[test]
fn mixed_discard1025_one_journal_lock_and_callback_preserve_graph_widget_and_enum_peers(){
    let mut f=Fixture::new();f.write();let original=f.bytes();let expected=f.source.recovery_batch().unwrap();let graphs=f.after_graphs();let widgets=f.widgets[1..].to_vec();let refs=f.references();let root=f.root.clone();let called=std::cell::Cell::new(0);
    f.source.discard_recovery_cohort_with1025(expected.as_ref(),&graphs,&widgets,&[],&[EnumRecoveryChange1025::Remove{before:f.a.clone()}],&refs,||{
        assert!(!root.join(REFACTOR_JOURNAL).exists());assert!(lock(&root).is_err(),"same real Source writer lock still held through install");called.set(called.get()+1);
    }).unwrap();assert_eq!(called.get(),1);
    let after=f.source.recovery_batch().unwrap().unwrap();assert_eq!(after.graphs,graphs);assert_eq!(after.widgets,widgets);assert_eq!(after.enums,vec![f.b.clone()]);assert_eq!(f.bytes()[..3],original[..3]);
    for path in [f.a.locator(),f.b.locator(),&f.widgets[0].locator,&f.widgets[1].locator]{assert!(!f.root.join(path).exists());}
    let cold=SourceWorkspace::open(&f.root).unwrap().unwrap();assert_eq!(cold.recovery_batch().unwrap().unwrap(),after);
}
#[test]
fn mixed_discard1025_stale_complete_before_and_non_remove_delta_refuse_without_partial_discard(){
    let mut f=Fixture::new();f.write();let stale=f.source.recovery_batch().unwrap();let mut newer=f.graphs.clone();newer[0].nodes.values_mut().next().unwrap().title_override=Some("Later actual cache".into());f.source.write_recovery_with_widgets(&newer,&f.widgets).unwrap();let bytes=f.bytes();let called=std::cell::Cell::new(false);
    assert!(f.source.discard_recovery_cohort_with1025(stale.as_ref(),&f.after_graphs(),&f.widgets[1..],&[],&[EnumRecoveryChange1025::Remove{before:f.a.clone()}],&[],||called.set(true)).is_err());
    let actual=f.source.recovery_batch().unwrap();assert!(f.source.discard_recovery_cohort_with1025(actual.as_ref(),&f.after_graphs(),&f.widgets[1..],&[],&[EnumRecoveryChange1025::Upsert{before:Some(f.b.clone()),after:f.b.clone()}],&[],||called.set(true)).is_err());
    assert!(!called.get());assert_eq!(f.bytes(),bytes);assert!(!f.root.join(REFACTOR_JOURNAL).exists());
}
#[test]
fn mixed_discard1025_removed_widget_and_readonly_reference_conflicts_preserve_all_before_images(){
    for physical_widget in [true,false]{
        let mut f=Fixture::new();f.write();let expected=f.source.recovery_batch().unwrap();let refs=f.references();
        if physical_widget{fs::write(f.root.join(&f.widgets[0].locator),"External Widget file").unwrap();}else{fs::write(f.root.join("rvn.toml"),"External manifest edit").unwrap();}
        let bytes=f.bytes();let called=std::cell::Cell::new(false);assert!(f.source.discard_recovery_cohort_with1025(expected.as_ref(),&f.after_graphs(),&f.widgets[1..],&[],&[EnumRecoveryChange1025::Remove{before:f.a.clone()}],&refs,||called.set(true)).is_err());
        assert!(!called.get());assert_eq!(f.bytes(),bytes);assert!(!f.root.join(REFACTOR_JOURNAL).exists());
        if physical_widget{assert_eq!(fs::read(f.root.join(&f.widgets[0].locator)).unwrap(),b"External Widget file");}
    }
}
#[test]
fn mixed_discard1025_widget_identity_cannot_drop_while_member_survives_and_enum_is_not_removed_first(){
    let mut f=Fixture::new();f.write();let expected=f.source.recovery_batch().unwrap();let bytes=f.bytes();let called=std::cell::Cell::new(false);assert!(f.source.discard_recovery_cohort_with1025(expected.as_ref(),&f.graphs,&f.widgets[1..],&[],&[EnumRecoveryChange1025::Remove{before:f.a.clone()}],&[],||called.set(true)).is_err());assert!(!called.get());assert_eq!(f.bytes(),bytes);assert_eq!(f.source.recovery_batch().unwrap(),expected);
}
#[test]
fn mixed_discard1025_absent_previous_cache_accepts_complete_live_graph_cohort_without_publishing_source(){
    let mut f=Fixture::new();assert!(f.source.recovery_batch().unwrap().is_none());let original=f.bytes();let mut graphs=f.baseline.clone();graphs[1].nodes.values_mut().next().unwrap().title_override=Some("Live peer before first autosave".into());let called=std::cell::Cell::new(false);
    f.source.discard_recovery_cohort_with1025(None,&graphs,&[],&[],&[],&f.references(),||called.set(true)).unwrap();assert!(called.get());let after=f.source.recovery_batch().unwrap().unwrap();assert_eq!(after.graphs,graphs);assert!(after.widgets.is_empty()&&after.enums.is_empty());assert_eq!(f.bytes()[..3],original[..3]);
}
#[test]
fn mixed_discard1025_actual_prepared_journal_replays_before_after_and_refuses_foreign_readset(){
    for stage in 0..3{
        let mut f=Fixture::new();f.write();let prepared={let _guard=lock(&f.source.root).unwrap();f.prepared()};let wire=serde_json::to_string(&prepared.journal).unwrap();
        if stage==1{replace_recovery1025(&f.source.root,&prepared.cache).unwrap();}
        if stage==2{fs::write(f.root.join("rvn.toml"),"External change during interrupted discard").unwrap();}
        let expected=f.bytes();fs::write(f.root.join(REFACTOR_JOURNAL),&wire).unwrap();
        let cold=SourceWorkspace::open(&f.root);
        if stage==2{assert!(cold.is_err());assert_eq!(f.bytes(),expected);assert_eq!(fs::read_to_string(f.root.join(REFACTOR_JOURNAL)).unwrap(),wire);}
        else{let after=cold.unwrap().unwrap().recovery_batch().unwrap().unwrap();assert_eq!(after.graphs,f.after_graphs());assert_eq!(after.widgets,f.widgets[1..]);assert_eq!(after.enums,vec![f.b.clone()]);assert!(!f.root.join(REFACTOR_JOURNAL).exists());}
    }
}
#[test]
fn mixed_discard1025_previous_typed_mode_reader_refuses_new_mode_before_replay(){
    #[allow(dead_code)]#[derive(Deserialize)]#[serde(rename_all="snake_case")]enum OldMode{RecoveryOnly,Metadata,CompleteSource,DiscardAll}
    #[allow(dead_code)]#[derive(Deserialize)]struct OldTransition{kind:String,version:u32,mode:OldMode,before_recovery:Option<String>,after_recovery:Option<String>,changes:Vec<EnumRecoveryChange1025>}
    #[allow(dead_code)]#[derive(Deserialize)]#[serde(untagged)]enum OldRecovery{Legacy(String),Transition(OldTransition)}
    #[allow(dead_code)]#[derive(Deserialize)]struct OldJournal{recovery:Option<OldRecovery>}
    let mut f=Fixture::new();f.write();let before=f.bytes();let prepared=f.prepared();let wire=serde_json::to_string(&prepared.journal).unwrap();assert!(serde_json::from_str::<OldJournal>(&wire).is_err());assert_eq!(f.bytes(),before);assert!(!f.root.join(REFACTOR_JOURNAL).exists());
}
