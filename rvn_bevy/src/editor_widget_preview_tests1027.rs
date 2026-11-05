//! Production loader selection and physical staged IO, not a native claim.
use super::*;
use rvn_core::widget_class_source_0950::{CompilerWidgetClass,CompilerWidgetMember,WidgetMemberKind};
use rvn_parser::widget_asset::{WidgetBlueprintV1,WidgetGraphRef16};
use std::sync::atomic::{AtomicU64,Ordering};
const UUID:&str="17411a3e431e4c9388b78a2b9825b130";
const SOURCE:&str="screen panel(){return []}\nlabel start\n\"Ready\"\nreturn\n";
struct Fixture{root:PathBuf,stage:PathBuf,manifest:String,media:String,seal:wire::LaunchSeal1027}
impl Fixture{
 fn new()->Self{
  static NEXT:AtomicU64=AtomicU64::new(1);let root=std::env::temp_dir().join(format!("rvn-widget-preview-bevy1027-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&root).unwrap();let root=root.canonicalize().unwrap();let stage=root.join("disposable");fs::create_dir(&stage).unwrap();fs::create_dir(root.join("assets")).unwrap();
  fs::write(root.join("main.rvn"),SOURCE).unwrap();fs::write(stage.join("main.rvn"),SOURCE).unwrap();
  let mut graphs=rvn_graph::import_script(&rvn_parser::parse(SOURCE).unwrap()).unwrap();for g in &mut graphs{if matches!(g.kind,rvn_graph::GraphKind::Screen{..}){g.graph_id=rvn_graph::GraphId::new(91913);}}
  let workspace=rvn_graph::SourceWorkspace::link(&root,&root.join("main.rvn"),&graphs).unwrap();let program=workspace.project().resolved_program().unwrap();let screen=workspace.project().graphs().iter().find(|g|matches!(g.kind,rvn_graph::GraphKind::Screen{..})).unwrap();assert_eq!(screen.graph_id.get(),91913);
  let id=WidgetGraphRef16::from_u64(91913);let descriptor=WidgetBlueprintV1{uuid:UUID.into(),source:"main.rvn".into(),screen_graph:id,members:vec![id]}.encode().unwrap();fs::write(root.join("assets/panel.rvnwidget"),&descriptor).unwrap();
  let original_manifest=format!("[project]\nmain_script='main.rvn'\n[paths]\nassets='assets'\n[widget_blueprints]\n'{UUID}'='panel.rvnwidget'\n");fs::write(root.join("rvn.toml"),&original_manifest).unwrap();assert_eq!(index(&original_manifest).unwrap().get(UUID).map(String::as_str),Some("panel.rvnwidget"));
  let rows=[(UUID,Some("panel.rvnwidget"))];let admission=rvn_graph::prepare_widget_runtime_classes(&root,&program,WidgetIndexInput::Table(&rows),LocatorCase::Insensitive,&[rvn_graph::WidgetRuntimeAsset{locator:"panel.rvnwidget",bytes:descriptor.as_bytes()}],&[rvn_graph::WidgetRuntimeSource{source:"main.rvn",project:workspace.project()}]).unwrap();
  let compiler=admission.classes().values().map(|c|CompilerWidgetClass{index_uuid:c.uuid().into(),locator:c.locator().into(),descriptor_bytes:c.descriptor_bytes().to_vec(),members:c.members().iter().map(|m|CompilerWidgetMember{graph:m.graph().get(),kind:match m.kind(){rvn_graph::WidgetDefinitionKind::Screen=>WidgetMemberKind::Screen,rvn_graph::WidgetDefinitionKind::Handler=>WidgetMemberKind::Handler,rvn_graph::WidgetDefinitionKind::Function=>WidgetMemberKind::Function},definition:m.definition().into(),root_index:m.root_index(),origin:m.origin().clone()}).collect()}).collect();
  let frozen=wire::CapturedWidgets1027::from_compiler(&root,&program,compiler).unwrap();let media=root.join("assets").to_string_lossy().replace('\\',"/");let manifest=format!("[project]\nmain_script='main.rvn'\n[paths]\nassets='{media}'\nsaves='saves'\n[widget_blueprints]\n'{UUID}'='panel.rvnwidget'\n");fs::write(stage.join("rvn.toml"),&manifest).unwrap();let staged=rvn_parser::parse_file_program_with_uses(stage.join("main.rvn")).unwrap();let bytes=frozen.encode_for(&stage,&staged,&manifest,&frozen.index().unwrap(),17).unwrap();let seal=wire::LaunchSeal1027::for_bytes(17,&bytes).unwrap();fs::write(stage.join(wire::SIDECAR),bytes).unwrap();Self{root,stage,manifest,media,seal}
 }
 fn program(&self)->ParsedProgram{rvn_parser::parse_file_program_with_uses(self.stage.join("main.rvn")).unwrap()}
 fn prepare(&self)->Result<PreparedWidgets1027,String>{prepare_for_arguments(&self.stage,&self.manifest,&self.media,&self.program(),vec![self.seal.argument()])}
}
impl Drop for Fixture{fn drop(&mut self){assert!(self.root.file_name().unwrap().to_string_lossy().starts_with("rvn-widget-preview-bevy1027-"));let _=fs::remove_dir_all(&self.root);}}
#[test]
fn widget_preview1027_actual_sealed_loader_accepts_original_absolute_media_without_relaxing_disk_loader(){
 let f=Fixture::new();let saved=fs::read(f.root.join("assets/panel.rvnwidget")).unwrap();let source=fs::read(f.root.join("main.rvn")).unwrap();let presentation=fs::read(f.root.join(".rvn-authoring.json")).unwrap();
 let original_manifest=fs::read_to_string(f.root.join("rvn.toml")).unwrap();let original_program=rvn_parser::parse_file_program_with_uses(f.root.join("main.rvn")).unwrap();assert!(!super::super::widget_runtime_loader_0951::prepare(&f.root,&original_manifest,"assets",&original_program).unwrap().classes().is_empty());
 assert!(super::super::widget_runtime_loader_0951::prepare(&f.stage,&f.manifest,&f.media,&f.program()).is_err());
 let classes=f.prepare().unwrap().finish(&f.program()).unwrap();assert_eq!(classes.class(UUID).unwrap().descriptor().screen_graph.get(),91913);assert!(!f.stage.join("assets").exists());assert!(!f.stage.join("saves").exists());
 assert_eq!(fs::read(f.root.join("assets/panel.rvnwidget")).unwrap(),saved);assert_eq!(fs::read(f.root.join("main.rvn")).unwrap(),source);assert_eq!(fs::read(f.root.join(".rvn-authoring.json")).unwrap(),presentation);assert_eq!(fs::read_to_string(f.root.join("rvn.toml")).unwrap(),original_manifest);
 let legacy="[paths]\nassets='D:/legacy-absolute-media'\n";assert!(prepare_for_arguments(&f.stage,legacy,"D:/legacy-absolute-media",&f.program(),vec![]).unwrap().classes().is_empty());
}
#[test]
fn widget_preview1027_actual_first_read_and_finish_refuse_byte_rewrite_manifest_source_and_foreign_nonce(){
 let f=Fixture::new();let bytes=fs::read(f.stage.join(wire::SIDECAR)).unwrap();let mut changed=bytes.clone();changed.push(b'\n');fs::write(f.stage.join(wire::SIDECAR),&changed).unwrap();assert!(f.prepare().err().unwrap().contains("seal"));fs::write(f.stage.join(wire::SIDECAR),&bytes).unwrap();
 let prepared=f.prepare().unwrap();fs::write(f.stage.join(wire::SIDECAR),&changed).unwrap();assert!(prepared.finish(&f.program()).is_err());fs::write(f.stage.join(wire::SIDECAR),&bytes).unwrap();
 let prepared=f.prepare().unwrap();fs::write(f.stage.join("rvn.toml"),format!("{}#changed\n",f.manifest)).unwrap();assert!(prepared.finish(&f.program()).is_err());fs::write(f.stage.join("rvn.toml"),&f.manifest).unwrap();
 let program=f.program();let prepared=f.prepare().unwrap();fs::write(f.stage.join("main.rvn"),SOURCE.replace("Ready","Changed")).unwrap();assert!(prepared.finish(&program).is_err());fs::write(f.stage.join("main.rvn"),SOURCE).unwrap();
 let other=rvn_core::editor_enum_current1027::LaunchSeal1027::for_bytes(18,b"not a grant").unwrap();assert!(prepare_for_arguments(&f.stage,&f.manifest,&f.media,&f.program(),vec![f.seal.argument(),other.argument()]).is_err());
}
