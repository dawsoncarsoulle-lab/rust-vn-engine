//! Regression at the real Library-observed Widget group-save frontier. The
//! independent batch must preserve the complete ordered Graph presentation.
use super::*;
use crate::{GraphKind,NodeKind,PropertyValue,SourceRefactorFile,SourceWorkspace};
use std::time::{SystemTime,UNIX_EPOCH};

struct Fixture{root:std::path::PathBuf}
impl Fixture{
 fn new()->Self{
  let root=std::env::temp_dir().join(format!("rvn-widget-stage1031-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
  std::fs::create_dir(&root).unwrap();std::fs::create_dir(root.join("assets")).unwrap();
  std::fs::write(root.join("main.rvn"),include_str!("source_widget_group_stage_fixture1031.rvn")).unwrap();
  std::fs::write(root.join("rvn.toml"),"[project]\ntitle='Native Widget group save'\n").unwrap();Self{root}
 }
 fn workspace(&self)->SourceWorkspace{
  // Derive the actual existing-document identity shape: narrative first,
  // then a genuine Widget definition addition. Never assign GraphIds by hand.
  let narrative="label start\n\"Before\"\njump other\nlabel other\n\"Other\"\n";
  let initial=SourceProject::open(narrative,&[]).unwrap();
  let definitions=format!("screen Widget_1(){{return []}}\nhandler Widget_1_event(event){{return 0}}\nfunction Widget_1_value(){{return 0}}\n{narrative}");
  let prior=SourceProject::open(definitions,initial.graphs()).unwrap();
  let workspace=SourceWorkspace::link_scoped_libraries1027(&self.root,&self.root.join("main.rvn"),prior.graphs()).unwrap();
  assert!(workspace.source_observes_libraries1027(),"the real Library-observed staging path must not be bypassed");workspace
 }
 fn files(&self)->Vec<SourceRefactorFile>{
  let manifest=std::fs::read_to_string(self.root.join("rvn.toml")).unwrap();
  vec![SourceRefactorFile{path:"rvn.toml".into(),before:Some(manifest.clone()),after:Some(manifest)},SourceRefactorFile{path:"assets/widget-save-proof.txt".into(),before:None,after:Some("independent real group participant".into())}]
 }
}
impl Drop for Fixture{fn drop(&mut self){
 let(Ok(root),Ok(temp))=(self.root.canonicalize(),std::env::temp_dir().canonicalize())else{return;};
 if root.parent()==Some(temp.as_path())&&root.file_name().and_then(|v|v.to_str()).is_some_and(|s|s.starts_with("rvn-widget-stage1031-")){let _=std::fs::remove_dir_all(root);}
}}
fn edit_value(graphs:&mut[GraphDocument],native:bool){
 let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Function{name,..}if name=="Widget_1_value")).unwrap();
 if native{graph.enable_native_frame_preparation().unwrap();}
 // Plain Expr::Str imports as Literal/value (import.rs expression_node),
 // not an owned/localized TextValue. Use the exact existing typed inspector
 // setter, retaining output type, pin identity, owner and graph schema.
 let literals=graph.nodes.values().filter(|n|n.kind==NodeKind::Literal&&n.properties.get("value")==Some(&PropertyValue::String("Texte dynamique A".into()))).map(|n|n.id).collect::<Vec<_>>();
 assert_eq!(literals.len(),1,"the actual imported function must have exactly one plain String producer: {:?}",graph.nodes);
 let literal=literals[0];let output=graph.pin_by_key(literal,"value").unwrap();
 assert_eq!(output.value_type,crate::ValueType::String);assert!(output.text_localization.is_none()&&output.text_table_reference.is_none()&&output.default_value.is_none());
 let mut expected=graph.clone();expected.nodes.get_mut(&literal).unwrap().properties.insert("value".into(),PropertyValue::String("Texte dynamique B".into()));
 graph.set_property(literal,"value",PropertyValue::String("Texte dynamique B".into())).unwrap();
 assert_eq!(*graph,expected,"the typed String edit must not replace its real graph/pin/Source identity or invent FText data");
 // Real ProjectDocuments capture is keyed by the existing GraphId. Preserve
 // that actual submitted ordering, including Legacy scenes before Widget defs.
 graphs.sort_by_key(|graph|graph.graph_id);
}
fn differences(before:&[GraphDocument],after:&[GraphDocument])->String{
 let mut detail=format!("before order: {:?}\nafter order: {:?}\n",before.iter().map(|g|(g.graph_id,&g.kind)).collect::<Vec<_>>(),after.iter().map(|g|(g.graph_id,&g.kind)).collect::<Vec<_>>());
 for g in before{
  match after.iter().find(|other|other.graph_id==g.graph_id){
   Some(other)if other!=g=>{let a=serde_json::to_value(g).unwrap();let b=serde_json::to_value(other).unwrap();let fields=a.as_object().unwrap().keys().chain(b.as_object().unwrap().keys()).collect::<std::collections::BTreeSet<_>>();for field in fields{if a.get(field)!=b.get(field){detail.push_str(&format!("graph {:?} field {field}: before {:?}; after {:?}\n",g.graph_id,a.get(field),b.get(field)));}}},
   None=>detail.push_str(&format!("lost graph {:?}\n",g.graph_id)),_=>{},
  }
 }detail
}
#[test]
fn widget_stage1031_native_function_edit_keeps_exact_ordered_projection_when_library_resources_are_staged(){
 let f=Fixture::new();let w=f.workspace();let mut edited=w.project().graphs().to_vec();edit_value(&mut edited,true);
 let mut candidate=w.project().clone();let current=candidate.source().to_owned();candidate.apply_visual(&current,&edited).unwrap();
 let staged=candidate.stage_library_files1027(&f.root,&f.root.join("main.rvn"),&f.files()).unwrap();
 assert_eq!(staged.source(),candidate.source());
 assert!(staged.graphs()==candidate.graphs(),"{}",differences(candidate.graphs(),staged.graphs()));
 assert_eq!(std::fs::read_to_string(f.root.join("main.rvn")).unwrap(),include_str!("source_widget_group_stage_fixture1031.rvn"));
 assert!(!f.root.join("assets/widget-save-proof.txt").exists(),"preparation may not publish a participant");
}
#[test]
fn widget_stage1031_real_group_save_and_cold_reopen_keep_native_and_legacy_function_edits_and_all_widget_slots(){
 for native in [false,true]{
  let f=Fixture::new();let mut w=f.workspace();let mut edited=w.project().graphs().to_vec();edit_value(&mut edited,native);
  let mut expected=w.project().clone();let current=expected.source().to_owned();expected.apply_visual(&current,&edited).unwrap();
  let error=w.save_with_files(&edited,&f.files()).err();
  assert!(error.is_none(),"actual independent group save refused native={native}: {error:?}; no coherence guard may be removed");
  assert_eq!(w.project().source(),expected.source());assert_eq!(w.project().graphs(),expected.graphs());
  assert_eq!(std::fs::read_to_string(f.root.join("assets/widget-save-proof.txt")).unwrap(),"independent real group participant");
  drop(w);let cold=SourceWorkspace::open_scoped_libraries1027(&f.root,0).unwrap().unwrap();assert!(cold.source_observes_libraries1027());assert_eq!(cold.project().source(),expected.source());
  assert!(cold.project().graphs()==expected.graphs(),"cold native={native}: {}",differences(expected.graphs(),cold.project().graphs()));
  let screen=cold.project().graphs().iter().find(|g|matches!(&g.kind,GraphKind::Screen{name,..}if name=="Widget_1")).unwrap();
  assert!(screen.nodes.values().any(|n|n.kind==NodeKind::FunctionCall),"dynamic text binding remains a genuine call");
 }
}
