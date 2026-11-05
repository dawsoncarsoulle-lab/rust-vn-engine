//! Actual IO against the production recovery replacement; no fake publisher.
use super::*;
use crate::{GraphKind,NodeKind,PropertyValue};
use rvn_parser::enum_asset::{EnumAsset,EnumAssetId,EnumCommand};
use rvn_parser::enum_recovery1025::EnumRecoveryRecord1025 as Record;
use std::cell::Cell;
struct Fixture {root:PathBuf,source:PathBuf}
impl Fixture {
    fn new()->Self{
        let root=std::env::temp_dir().join(format!("rvn-content-enum-recovery1027-{}",unique_name()));
        fs::create_dir(&root).unwrap();fs::create_dir(root.join("assets")).unwrap();
        let source=root.join("story.rvn");fs::write(&source,"init { set held = 1 }\nscene label start\nreturn\n").unwrap();
        Self{root,source}
    }
    fn workspace(&self)->SourceWorkspace{SourceWorkspace::link(&self.root,&self.source,&[]).unwrap()}
    fn enum_models(&self)->(Record,Record){
        let asset=EnumAsset::new(EnumAssetId::parse("17411a3e431e4c9388b78a2b9825b130").unwrap(),"Alpha".into()).unwrap();
        let wire=asset.encode().unwrap();fs::write(self.root.join("assets/Alpha.rvnenum"),&wire).unwrap();
        let before=Record::published("assets/Alpha.rvnenum".into(),asset.clone(),wire.clone()).unwrap();
        let changed=asset.prepare(EnumCommand::DescribeAsset("Current, never implicitly saved".into())).unwrap().after().clone();
        let after=Record::published("assets/Alpha.rvnenum".into(),changed,wire).unwrap();(before,after)
    }
}
impl Drop for Fixture{
    fn drop(&mut self){
        let(Ok(root),Ok(temp))=(self.root.canonicalize(),std::env::temp_dir().canonicalize())else{return;};
        if root.parent()==Some(temp.as_path())&&root.file_name().and_then(|n|n.to_str()).is_some_and(|n|n.starts_with("rvn-content-enum-recovery1027-")){
            let _=fs::remove_dir_all(root);
        }
    }
}
fn edited(workspace:&SourceWorkspace)->Vec<GraphDocument>{
    let mut graphs=workspace.project().graphs().to_vec();
    let init=graphs.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();
    // The production importer stores this constant in the SET value input;
    // inspector edits must use the existing literal-input abstraction, which
    // also handles a materialized/shared producer without editing it globally.
    let assignments=init.nodes.values().filter(|n|n.kind==NodeKind::SetVariable&&n.properties.get("name")==Some(&PropertyValue::String("held".into()))).map(|n|n.id).collect::<Vec<_>>();
    assert_eq!(assignments.len(),1,"Fixture must contain one actual held Init assignment");
    let assignment=assignments[0];
    assert_eq!(init.literal_input_value(assignment,"value").unwrap(),Some(PropertyValue::Int(1)));
    init.set_literal_input(assignment,"value",PropertyValue::Int(2)).unwrap();
    assert_eq!(init.literal_input_value(assignment,"value").unwrap(),Some(PropertyValue::Int(2)));
    let compiled=crate::transpile_project(&graphs).unwrap();
    let initializers=compiled.ast.iter().filter_map(|s|if let rvn_parser::Statement::Init{body}=s{Some(body)}else{None}).flatten().filter_map(|s|if let rvn_parser::Statement::SetVar{name,value}=s{(name=="held").then_some(value.clone())}else{None}).collect::<Vec<_>>();
    assert_eq!(initializers,vec![rvn_parser::Expr::Int(2)]);
    graphs
}
#[test]
fn content_enum_recovery1027_actual_atomic_graph_and_current_enum_never_save_authored_baselines(){
    let f=Fixture::new();let mut workspace=f.workspace();let (_saved,current)=f.enum_models();
    let authored=fs::read(&f.source).unwrap();let sidecar=fs::read(f.root.join(SIDECAR)).unwrap();let enum_file=fs::read(f.root.join("assets/Alpha.rvnenum")).unwrap();
    let graphs=edited(&workspace);let called=Cell::new(false);
    workspace.replace_recovery_with_enum_changes1027(&graphs,&[],&[],&[EnumRecoveryChange1025::Upsert{before:None,after:current.clone()}],&[],||{
        let actual:Recovery=serde_json::from_slice(&fs::read(f.root.join(RECOVERY)).unwrap()).unwrap();
        assert_eq!(actual.graphs,graphs);assert_eq!(actual.enums,vec![current.clone()]);called.set(true);
    }).unwrap();
    assert!(called.get());assert_eq!(fs::read(&f.source).unwrap(),authored);assert_eq!(fs::read(f.root.join(SIDECAR)).unwrap(),sidecar);assert_eq!(fs::read(f.root.join("assets/Alpha.rvnenum")).unwrap(),enum_file);
    let batch=workspace.recovery_batch().unwrap().unwrap();assert_eq!(batch.graphs,graphs);assert_eq!(batch.enums,vec![current]);
    assert_ne!(workspace.project().graphs(),graphs.as_slice());
}
#[test]
fn content_enum_recovery1027_stale_before_refuses_actual_file_and_callback_without_partial_graph_move(){
    let f=Fixture::new();let mut workspace=f.workspace();let(saved,current)=f.enum_models();let graphs=edited(&workspace);
    workspace.replace_recovery_with_enum_changes1027(&graphs,&[],&[],&[EnumRecoveryChange1025::Upsert{before:None,after:current.clone()}],&[],||()).unwrap();
    let before=fs::read(f.root.join(RECOVERY)).unwrap();let called=Cell::new(false);
    assert!(workspace.replace_recovery_with_enum_changes1027(workspace.project().graphs().to_vec().as_slice(),&[],&[],&[EnumRecoveryChange1025::Upsert{before:Some(saved),after:current}],&[],||called.set(true)).is_err());
    assert!(!called.get());assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(),before);assert_eq!(workspace.recovery_batch().unwrap().unwrap().graphs,graphs);
}
#[test]
fn content_enum_recovery1027_manifest_cas_refuses_without_creating_cache_or_calling_commit(){
    let f=Fixture::new();let mut workspace=f.workspace();let graphs=edited(&workspace);fs::write(f.root.join("rvn.toml"),"external").unwrap();let called=Cell::new(false);
    let reference=SourceRefactorFile{path:"rvn.toml".into(),before:Some("captured".into()),after:Some("captured".into())};
    assert!(workspace.replace_recovery_with_enum_changes1027(&graphs,&[],&[],&[],&[reference],||called.set(true)).is_err());
    assert!(!called.get());assert!(!f.root.join(RECOVERY).exists());assert_eq!(fs::read_to_string(f.root.join("rvn.toml")).unwrap(),"external");
}
#[test]
fn content_enum_recovery1027_cannot_turn_current_enum_into_a_saved_ack(){
    let f=Fixture::new();let mut workspace=f.workspace();let(_,current)=f.enum_models();let graphs=edited(&workspace);
    workspace.replace_recovery_with_enum_changes1027(&graphs,&[],&[],&[EnumRecoveryChange1025::Upsert{before:None,after:current.clone()}],&[],||()).unwrap();
    let before=fs::read(f.root.join(RECOVERY)).unwrap();let forged=Record::published(current.locator().into(),current.current().clone(),current.current().encode().unwrap()).unwrap();let called=Cell::new(false);
    assert!(workspace.replace_recovery_with_enum_changes1027(&graphs,&[],&[],&[EnumRecoveryChange1025::Upsert{before:Some(current),after:forged}],&[],||called.set(true)).is_err());
    assert!(!called.get());assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(),before);
}
