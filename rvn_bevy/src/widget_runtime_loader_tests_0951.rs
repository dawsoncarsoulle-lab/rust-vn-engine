//! Prepared CPU/real-loader tests. No UI/native-parity qualification is claimed.
use super::*;
use crate::{load_game,load_runtime_game,RuntimeLaunchOwner};
use rvn_parser::{Value,widget_asset::{WidgetBlueprintV1,WidgetGraphRef16}};
use std::sync::atomic::{AtomicU64,Ordering};
const UUID:&str="17411a3e431e4c9388b78a2b9825b130";
// PersistentData is runtime input, not a source type declaration. The real
// unreachable narrative label declares the persistent Int in its legal domain:
// Init must still observe the independently seeded value 7, never its reset 0.
fn fixture_graphs(raw:&str)->Vec<rvn_graph::GraphDocument>{
 let graphs=rvn_graph::import_script(&rvn_parser::parse(raw).unwrap()).unwrap();
 let init=graphs.iter().find(|graph|matches!(graph.kind,rvn_graph::GraphKind::Init)).unwrap();
 assert_eq!(init.variables["persistent.counter"].value_type,rvn_graph::ValueType::Int,"the real source binder must admit an authored typed persistent variable");
 assert!(graphs.iter().any(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Label{name}if name=="fixture_declare_counter")),"the declaration belongs to a real narrative label, never a pure Handler");
 graphs
}
const LEGACY:&str="screen panel(value){return []}\ninit{set seen=persistent.counter}\nlabel start\n\"Ready\"\nreturn\nlabel fixture_declare_counter\nset persistent.counter=0\nreturn\n";
struct Project{root:PathBuf}
impl Project{
 fn new(native:bool)->Self{
  static NEXT:AtomicU64=AtomicU64::new(0);
  let path=std::env::temp_dir().join(format!("rvn-widget-native-loader-0951-{}-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),NEXT.fetch_add(1,Ordering::Relaxed)));
  fs::create_dir(&path).unwrap();let root=fs::canonicalize(path).unwrap();fs::create_dir(root.join("assets")).unwrap();fs::create_dir(root.join("locales")).unwrap();
  let (source,graphs)=if native{
   let raw="screen panel(value){local seen=value while false{set seen=seen+1} return []}\ninit{set seen=persistent.counter}\nlabel start\n\"Ready\"\nreturn\nlabel fixture_declare_counter\nset persistent.counter=0\nreturn\n";
   let mut graphs=fixture_graphs(raw);for graph in &mut graphs{graph.enable_native_frame_preparation().unwrap();}
   let compiled=rvn_graph::transpile_project(&graphs).expect("actual Graph Native lowering");assert!(compiled.source.contains("flow.consume_v2("),"real Graph While emits Native2; never relabel v1 text");(compiled.source,graphs)
  }else{let mut graphs=fixture_graphs(LEGACY);for graph in &mut graphs{if matches!(&graph.kind,rvn_graph::GraphKind::Screen{name}if name=="panel"){graph.graph_id=rvn_graph::GraphId::new(91913);}}(LEGACY.into(),graphs)};
  fs::write(root.join("main.rvn"),source).unwrap();
  let workspace=if native{rvn_graph::SourceWorkspace::link_native_preparation(&root,&root.join("main.rvn"),&graphs)}else{rvn_graph::SourceWorkspace::link(&root,&root.join("main.rvn"),&graphs)}.unwrap();
  let screen=workspace.project().graphs().iter().find(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Screen{name}if name=="panel")).unwrap();if !native{assert_eq!(screen.graph_id.get(),91913,"real source link preserves nontrivial authoring history identity");}let id=WidgetGraphRef16::from_u64(screen.graph_id.get());
  let descriptor=WidgetBlueprintV1{uuid:UUID.into(),source:"main.rvn".into(),screen_graph:id,members:vec![id]}.encode().unwrap();fs::write(root.join("assets/panel.rvnwidget"),descriptor).unwrap();
  fs::write(root.join("rvn.toml"),format!("[project]\ntitle='Actual class loader'\nmain_script='main.rvn'\nstart_label='start'\n[paths]\nassets='assets'\nlocales='locales'\nsaves='saves'\ntheme='theme.toml'\n[widget_blueprints]\n'{UUID}'='panel.rvnwidget'\n")).unwrap();
  fs::write(root.join("theme.toml"),"[title_screen]\nenabled=false\n").unwrap();
  let manager=crate::PersistentDataManager::new(root.join("saves")).unwrap();let mut data=crate::PersistentData::default();data.script_vars.insert("persistent.counter".into(),Value::Int(7));manager.save(&data).unwrap();Self{root}
 }
 fn program(&self)->ParsedProgram{rvn_parser::parse_file_program_with_uses(self.root.join("main.rvn")).unwrap()}
 fn manifest(&self)->String{fs::read_to_string(self.root.join("rvn.toml")).unwrap()}
}
impl Drop for Project{fn drop(&mut self){let _=fs::remove_dir_all(&self.root);}}
// CPU-only handoff to this test host, not Source/World/paint acceptance. The
// fixture has no media readers, speech, or menu effects to execute. Transfer the
// actual complete batch/output before Save, as the real host boundary requires;
// never flip a Ready/failure flag or weaken Renderer checkpoint validation.
fn handoff_zero_media_publication(game:&mut rvn_core::Engine<crate::bevy_renderer::BevyRenderer>){
 let source=game.debug_state_source().unwrap();let before=serde_json::to_value(&game.state).unwrap();
 assert!(rvn_core::audio_playback::describe(&game.state,game.runtime_epoch()).unwrap().is_empty());
 assert!(rvn_core::audio_checkpoint::music_views(&game.state,game.runtime_epoch()).unwrap().is_empty());
 assert!(!rvn_core::frame_audio_checkpoint::has_host_readers(&game.state).unwrap());
 assert!(game.state.music.current_file.is_none()&&game.state.cinematic.current.is_none()&&game.state.videos.tracks.is_empty());
 assert!(!game.renderer.menu_pending&&!game.renderer.scene_frame_failed);
 if let Some(batch)=game.renderer.scene_batch.as_ref(){
  assert_eq!(serde_json::to_value(&batch.state).unwrap(),before);assert_eq!(batch.runtime_epoch,game.runtime_epoch());assert_eq!(batch.interface_epoch,game.interface_epoch());
  assert!(batch.videos.is_empty()&&batch.menu_requests.is_empty()&&batch.speech_requests.is_empty()&&batch.retired_widget_speech.is_empty());
 }
 assert!(game.renderer.pending.iter().all(|command|!matches!(command,crate::vn_command::VnCommand::MusicPlay{..}|crate::vn_command::VnCommand::SfxPlay{..}|crate::vn_command::VnCommand::SfxPlayTransition{..}|crate::vn_command::VnCommand::VoicePlay{..}|crate::vn_command::VnCommand::ShowCinematic{..}|crate::vn_command::VnCommand::Speech(_)|crate::vn_command::VnCommand::RetiredWidgetSpeech(_)|crate::vn_command::VnCommand::SourceMenu(_))));
 let _owned_batch=game.renderer.scene_batch.take();let _owned_commands=game.renderer.take_pending();
 assert!(!game.renderer.scene_batch_pending());assert!(game.accepts_debug_state_source(&source));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
}
#[test]
fn actual_native_loader_admits_class_before_init_and_creates_distinct_instances_from_canonical_root(){
 let project=Project::new(false);let sidecar=fs::read(project.root.join(".rvn-authoring.json")).unwrap();let descriptor=fs::read(project.root.join("assets/panel.rvnwidget")).unwrap();
 let mut launch=load_game(&project.root.join("assets").join("..")).unwrap();assert_eq!(launch.engine.state.vars["seen"],Value::Int(7));assert!(launch.engine.widget_classes().class(UUID).is_some());
 let first=launch.engine.create_widget(UUID,"first",vec![Value::Int(1)]).unwrap();let second=launch.engine.create_widget(UUID,"second",vec![Value::Int(2)]).unwrap();assert_ne!(first,second);assert_eq!(launch.engine.state.ui.screens[0].definition(),"panel");assert_eq!(launch.engine.state.ui.screens[1].arguments,vec![Value::Int(2)]);assert!(launch.engine.interface_hosts().unwrap().is_empty());
 launch.engine.add_widget_to_viewport(&first,true,0).unwrap();assert_eq!(launch.engine.interface_hosts().unwrap().len(),1);assert!(launch.engine.interface_views().unwrap().is_empty());
 assert_eq!(fs::read(project.root.join(".rvn-authoring.json")).unwrap(),sidecar);assert_eq!(fs::read(project.root.join("assets/panel.rvnwidget")).unwrap(),descriptor);assert!(!project.root.join(JOURNALS[0]).exists());
}
#[test]
fn actual_native2_loader_retains_one_startup_owner_and_independent_authored_consumer_frames(){
 let project=Project::new(true);fs::remove_file(project.root.join(".rvn-authoring.json")).unwrap();let launch=load_runtime_game(&project.root).unwrap();let RuntimeLaunchOwner::Startup(mut pending)=launch.engine else{panic!("Native/class launch must retain the actual pending owner")};
 assert!(!pending.owner.state().vars.contains_key("seen"),"loader admission cannot execute Init");let mut fuel=0;while !pending.owner.is_complete(){fuel+=1;assert!(fuel<=100);pending.owner.step(rvn_core::editor_debugger::StepKind::Continue).unwrap();}
 let mut engine=pending.owner.into_engine().unwrap();assert_eq!(engine.state.vars["seen"],Value::Int(7));engine.enter_label("start").unwrap();
 let a=engine.create_widget(UUID,"native_a",vec![Value::Int(1)]).unwrap();let b=engine.create_widget(UUID,"native_b",vec![Value::Int(2)]).unwrap();assert_ne!(a,b);
 let p=engine.state.ui.screens[0].consumer_projection.as_ref().expect("actual Native Screen projection");let q=engine.state.ui.screens[1].consumer_projection.as_ref().unwrap();assert_eq!(p.owner.definition,"panel");assert_eq!(q.owner.definition,"panel");assert_ne!(p.owner.target,q.owner.target);assert_eq!(p.owner.target.alias,"native_a");assert!(p.root.is_none()&&q.root.is_none());
 assert!(!engine.state.ui.screens[0].consumer_frames.is_empty());assert!(!engine.state.ui.screens[1].consumer_frames.is_empty());
}
#[test]
fn actual_native_loader_reopen_save15_keeps_class_and_instances_without_reviving_handles(){
 let project=Project::new(false);let program=project.program();let mut first=load_game(&project.root).unwrap().engine;assert_eq!(first.state.vars["seen"],Value::Int(7));assert_eq!(first.get_persistent_var("persistent.counter"),Some(&Value::Int(7)));
 let a=first.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();first.create_widget(UUID,"two",vec![Value::Int(2)]).unwrap();first.add_widget_to_viewport(&a,true,0).unwrap();handoff_zero_media_publication(&mut first);let saved=first.capture_save15(&program,None,1,"actual loader reopen".into()).unwrap().encode().unwrap();
 let mut expected=first.state.ui.clone();for instance in &mut expected.screens{if let Some(binding)=&mut instance.widget_class{binding.pending_materialization=binding.viewport_attached;}}let state=serde_json::to_value(&expected).unwrap();let mut reopened=load_game(&project.root).unwrap().engine;reopened.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&saved).unwrap()).unwrap();assert_eq!(serde_json::to_value(&reopened.state.ui).unwrap(),state);assert!(reopened.add_widget_to_viewport(&a,false,0).is_err());
 let current=reopened.widget_instance_handle("one").unwrap();reopened.remove_widget_from_viewport(&current).unwrap();assert!(reopened.interface_hosts().unwrap().is_empty());let second=reopened.widget_instance_handle("two").unwrap();reopened.add_widget_to_viewport(&second,false,0).unwrap();assert_eq!(reopened.interface_hosts().unwrap().len(),1);
}
#[test]
fn actual_native_loader_rejects_descriptor_source_membership_and_pending_transaction_without_recovery(){
 let project=Project::new(false);let linked=project.root.join(".rvn-authoring.json");let cached=fs::read(&linked).unwrap();fs::remove_file(&linked).unwrap();assert!(load_game(&project.root).is_err(),"Missing legacy identity presentation never falls back to graph-ID inference");fs::write(&linked,cached).unwrap();let path=project.root.join("assets/panel.rvnwidget");let original=fs::read(&path).unwrap();let sidecar=fs::read(project.root.join(".rvn-authoring.json")).unwrap();
 let mut descriptor=WidgetBlueprintV1::decode(&original).unwrap();descriptor.members.push(WidgetGraphRef16::from_u64(u64::MAX));fs::write(&path,descriptor.encode().unwrap()).unwrap();assert!(load_runtime_game(&project.root).is_err());assert_eq!(fs::read(project.root.join(".rvn-authoring.json")).unwrap(),sidecar);
 fs::write(&path,&original).unwrap();let journal=project.root.join(JOURNALS[0]);fs::write(&journal,b"must not execute recovery in runtime loader").unwrap();assert!(load_game(&project.root).is_err());assert_eq!(fs::read(&journal).unwrap(),b"must not execute recovery in runtime loader");fs::remove_file(journal).unwrap();
 descriptor=WidgetBlueprintV1::decode(&original).unwrap();descriptor.source="other.rvn".into();fs::write(project.root.join("other.rvn"),fs::read(project.root.join("main.rvn")).unwrap()).unwrap();fs::write(&path,descriptor.encode().unwrap()).unwrap();assert!(load_game(&project.root).is_err());assert_eq!(fs::read(project.root.join(".rvn-authoring.json")).unwrap(),sidecar);
}
#[test]
fn native_loader_final_fence_refuses_metadata_and_source_changes_before_engine_build(){
 let project=Project::new(false);let program=project.program();let manifest=project.manifest();
 let prepared=prepare(&project.root,&manifest,"assets",&program).unwrap();let descriptor=project.root.join("assets/panel.rvnwidget");let original=fs::read(&descriptor).unwrap();fs::write(&descriptor,b"{}").unwrap();assert!(prepared.finish(&program).is_err());fs::write(&descriptor,original).unwrap();
 let prepared=prepare(&project.root,&manifest,"assets",&program).unwrap();let source=project.root.join("main.rvn");fs::write(&source,LEGACY.replace("Ready","Changed")).unwrap();assert!(prepared.finish(&program).is_err());
}

#[test]
fn actual_native_loader_imported_widget_sources_keep_equal_local_graph_ids_separate_without_cache(){
 const OTHER:&str="3fc40c36f4e544ed82943e4c6fe9d87d";
 let project=Project::new(false);fs::remove_file(project.root.join(".rvn-authoring.json")).unwrap();fs::create_dir(project.root.join("widgets")).unwrap();
 for(name,uuid)in [("left",UUID),("right",OTHER)]{
  let raw=format!("screen {name}(value){{local seen=value while false{{set seen=seen+1}} return []}}\n");
  let mut graph=rvn_graph::import_script(&rvn_parser::parse(&raw).unwrap()).unwrap().into_iter().find(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Screen{name:actual}if actual==name)).unwrap();graph.graph_id=rvn_graph::GraphId::new(77919);graph.enable_native_frame_preparation().unwrap();let compiled=rvn_graph::prepare_native_frame_graph(&graph).unwrap();assert!(compiled.source.contains("flow.consume_v2("));
  fs::write(project.root.join(format!("widgets/{name}.rvn")),compiled.source).unwrap();let id=WidgetGraphRef16::from_u64(graph.graph_id.get());let descriptor=WidgetBlueprintV1{uuid:uuid.into(),source:format!("widgets/{name}.rvn"),screen_graph:id,members:vec![id]}.encode().unwrap();fs::write(project.root.join(format!("assets/{name}.rvnwidget")),descriptor).unwrap();
 }
 fs::write(project.root.join("main.rvn"),"use \"widgets/left.rvn\"\nuse \"widgets/right.rvn\"\ninit{set seen=persistent.counter}\nlabel start\n\"Ready\"\nreturn\nlabel fixture_declare_counter\nset persistent.counter=0\nreturn\n").unwrap();
 let manifest=project.manifest().replace("'panel.rvnwidget'","'left.rvnwidget'")+&format!("'{OTHER}'='right.rvnwidget'\n");fs::write(project.root.join("rvn.toml"),manifest).unwrap();
 let mut engine=load_game(&project.root).unwrap().engine;assert_eq!(engine.state.vars["seen"],Value::Int(7));assert_eq!(engine.get_persistent_var("persistent.counter"),Some(&Value::Int(7)));let first=engine.create_widget(UUID,"left_instance",vec![Value::Int(1)]).unwrap();let second=engine.create_widget(OTHER,"right_instance",vec![Value::Int(2)]).unwrap();assert_ne!(first,second);assert_eq!(engine.state.ui.screens[0].definition(),"left");assert_eq!(engine.state.ui.screens[1].definition(),"right");let a=&engine.state.ui.screens[0].consumer_projection.as_ref().unwrap().owner;let b=&engine.state.ui.screens[1].consumer_projection.as_ref().unwrap().owner;assert_ne!(a.source_ordinal,b.source_ordinal);assert_ne!(a.target,b.target);assert!(!project.root.join(".rvn-authoring.json").exists());
 let program=project.program();handoff_zero_media_publication(&mut engine);let wire=engine.capture_save15(&program,None,1,"two source classes".into()).unwrap().encode().unwrap();let mut reopened=load_game(&project.root).unwrap().engine;reopened.restore_save15(&program,rvn_core::save::consumer_checkpoint::Save15::decode(&wire).unwrap()).unwrap();assert!(reopened.widget_instance_handle("left_instance").is_some());assert!(reopened.widget_instance_handle("right_instance").is_some());assert!(reopened.add_widget_to_viewport(&first,false,0).is_err());
}

#[cfg(test)]
#[path="widget_backend_loader_tests_0956.rs"]
mod backend_tests_0956;
