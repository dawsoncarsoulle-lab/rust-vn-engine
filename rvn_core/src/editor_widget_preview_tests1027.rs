//! Real Graph/Source admission and exact relocated compiler data; CPU only.
use super::*;
use std::{fs,path::PathBuf,sync::atomic::{AtomicU64,Ordering}};
use rvn_parser::widget_asset::{WidgetBlueprintV1,WidgetGraphRef16};
const UUID:&str="17411a3e431e4c9388b78a2b9825b130";
const TEXT:&str="screen panel(){return []}\nfunction caption(){return 7}\nlabel start\n\"Ready\"\nreturn\n";
struct Fixture{root:PathBuf,stage:PathBuf,capture:CapturedWidgets1027,index:BTreeMap<String,String>,manifest:String}
impl Fixture{
 fn new()->Self{
  static NEXT:AtomicU64=AtomicU64::new(1);let root=std::env::temp_dir().join(format!("rvn-widget-preview-core1027-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&root).unwrap();let root=root.canonicalize().unwrap();let stage=root.join("disposable");fs::create_dir(&stage).unwrap();
  fs::write(root.join("main.rvn"),TEXT).unwrap();fs::write(stage.join("main.rvn"),TEXT).unwrap();
  let mut graphs=rvn_graph::import_script(&rvn_parser::parse(TEXT).unwrap()).unwrap();for g in &mut graphs{if matches!(g.kind,rvn_graph::GraphKind::Screen{..}){g.graph_id=rvn_graph::GraphId::new(91913);}}
  let project=rvn_graph::SourceProject::open_at(&root.join("main.rvn"),TEXT,&graphs).unwrap();let program=project.resolved_program().unwrap();
  let screen=project.graphs().iter().find(|g|matches!(g.kind,rvn_graph::GraphKind::Screen{..})).unwrap();assert_eq!(screen.graph_id.get(),91913);
  let id=WidgetGraphRef16::from_u64(screen.graph_id.get());let descriptor=WidgetBlueprintV1{uuid:UUID.into(),source:"main.rvn".into(),screen_graph:id,members:vec![id]}.encode().unwrap();
  let rows=[(UUID,Some("panel.rvnwidget"))];let admission=rvn_graph::prepare_widget_runtime_classes(&root,&program,WidgetIndexInput::Table(&rows),LocatorCase::Insensitive,&[rvn_graph::WidgetRuntimeAsset{locator:"panel.rvnwidget",bytes:descriptor.as_bytes()}],&[rvn_graph::WidgetRuntimeSource{source:"main.rvn",project:&project}]).unwrap();
  let compiler=admission.classes().values().map(|c|CompilerWidgetClass{index_uuid:c.uuid().into(),locator:c.locator().into(),descriptor_bytes:c.descriptor_bytes().to_vec(),members:c.members().iter().map(|m|CompilerWidgetMember{graph:m.graph().get(),kind:match m.kind(){rvn_graph::WidgetDefinitionKind::Screen=>WidgetMemberKind::Screen,rvn_graph::WidgetDefinitionKind::Handler=>WidgetMemberKind::Handler,rvn_graph::WidgetDefinitionKind::Function=>WidgetMemberKind::Function},definition:m.definition().into(),root_index:m.root_index(),origin:m.origin().clone()}).collect()}).collect();
  let capture=CapturedWidgets1027::from_compiler(&root,&program,compiler).unwrap();let index=capture.index().unwrap();let manifest=format!("[paths]\nassets='D:/unchanged-original-media'\n[widget_blueprints]\n'{UUID}'='panel.rvnwidget'\n");Self{root,stage,capture,index,manifest}
 }
 fn program(&self)->ParsedProgram{rvn_parser::parse_file_program_with_uses(self.stage.join("main.rvn")).unwrap()}
 fn bytes(&self)->Vec<u8>{self.capture.encode_for(&self.stage,&self.program(),&self.manifest,&self.index,17).unwrap()}
}
impl Drop for Fixture{fn drop(&mut self){assert!(self.root.file_name().unwrap().to_string_lossy().starts_with("rvn-widget-preview-core1027-"));let _=fs::remove_dir_all(&self.root);}}
#[test]
fn widget_preview1027_real_legacy_source_identity_relocates_without_graph_id_allocation(){
 let f=Fixture::new();let before=fs::read(f.root.join("main.rvn")).unwrap();let bytes=f.bytes();let seal=LaunchSeal1027::for_bytes(17,&bytes).unwrap();seal.validate_bytes(&bytes).unwrap();
 let classes=decode_for1027(&bytes,&f.stage,&f.program(),&f.manifest,&f.index,17).unwrap();let class=classes.class(UUID).unwrap();assert_eq!(class.descriptor().screen_graph.get(),91913);assert_eq!(class.identity().source,"main.rvn");assert_eq!(class.identity().definition,"panel");assert!(classes.matches_program(&f.program()).unwrap());assert_eq!(fs::read(f.root.join("main.rvn")).unwrap(),before);
}
#[test]
fn widget_preview1027_repeated_or_renamed_equal_source_and_changed_context_refuse(){
 let f=Fixture::new();let mut duplicated=f.program();duplicated.sources.push(duplicated.sources[0].clone());assert!(f.capture.encode_for(&f.stage,&duplicated,&f.manifest,&f.index,17).is_err());
 fs::write(f.stage.join("same-bytes.rvn"),TEXT).unwrap();let renamed=rvn_parser::parse_file_program_with_uses(f.stage.join("same-bytes.rvn")).unwrap();assert!(f.capture.encode_for(&f.stage,&renamed,&f.manifest,&f.index,17).is_err());
 let mut context=f.program();context.context=rvn_parser::NarrativeContext::ExplicitScenes;assert!(f.capture.encode_for(&f.stage,&context,&f.manifest,&f.index,17).is_err());
}
#[test]
fn widget_preview1027_valid_wire_rewrite_needs_original_independent_byte_seal(){
 let f=Fixture::new();let bytes=f.bytes();let original=LaunchSeal1027::for_bytes(17,&bytes).unwrap();let mut e:Envelope=serde_json::from_slice(&bytes).unwrap();
 let mut descriptor=WidgetBlueprintV1::decode(&e.classes[0].descriptor).unwrap();let id=WidgetGraphRef16::from_u64(91914);descriptor.screen_graph=id;descriptor.members=vec![id];e.classes[0].descriptor=descriptor.encode().unwrap().into_bytes();e.classes[0].members[0].graph=91914;
 let replacement=serde_json::to_vec(&e).unwrap();assert!(decode_for1027(&replacement,&f.stage,&f.program(),&f.manifest,&f.index,17).is_ok(),"valid data alone is not original launch authority");assert!(original.validate_bytes(&replacement).is_err());original.validate_bytes(&bytes).unwrap();
 let parsed=launch_argument1027([original.argument()]).unwrap().unwrap();parsed.validate_bytes(&bytes).unwrap();assert!(launch_argument1027([original.argument(),original.argument()]).is_err());assert!(launch_argument1027([format!("{ARGUMENT}17")]).is_err());assert!(decode_for1027(&bytes,&f.stage,&f.program(),&(f.manifest.clone()+"#changed\n"),&f.index,17).is_err());
}
