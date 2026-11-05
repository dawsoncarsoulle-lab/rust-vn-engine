//! Fusion checks prepared from Root481 v4 plus the guarded source-index fix.
//! Real Graph/source identities and public input/debugger APIs only.
use rvn_graph::*;
use rvn_core::{Engine,Renderer,Interaction,renderer::SceneRenderBatch,types::SpriteState};
use rvn_core::engine::{DebugOperation,DebugSession,DebugProgress,TextRuntimeContext};
use rvn_core::editor_debugger::{StepKind,Breakpoint};
use rvn_core::random::RandomState;
use rvn_parser::{ParsedProgram,Position,Transition,Hotspot,Statement,Value};
use std::{collections::BTreeMap,path::PathBuf,sync::{Arc,Mutex,atomic::{AtomicUsize,Ordering}}};
#[path="support/dialogue_mount_0624.rs"] mod mount;
// Same publication-only fixture contract as native_imagemap_consumer.rs.
#[derive(Clone,Default)] struct Host{batches:Arc<Mutex<Vec<SceneRenderBatch>>>}
impl Renderer for Host{
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{Ok(())}
 fn publish_scene_batch(&mut self,b:SceneRenderBatch){self.batches.lock().unwrap().push(b);}
 fn set_background(&mut self,_:&str,_:&Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{panic!("No fixture may supply a renderer Choice answer")}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{panic!("Actual input lease is required")}
 fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&Transition){}
 fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
fn dir(name:&str)->PathBuf{static NEXT:AtomicUsize=AtomicUsize::new(0);let d=PathBuf::from(std::env::var("RVN_FUSION0642_FIXTURES").expect("private fixture output")).join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir_all(&d).unwrap();d}
fn scene(g:&GraphDocument)->bool{matches!(&g.kind,GraphKind::Label{name}if name=="start")}
fn variable_source(g:&GraphDocument,dialogue:NodeId)->Option<NodeId>{
 let mut input=g.pin_by_key(dialogue,"text")?.id;
 for _ in 0..128{let edge=g.edges.values().find(|e|e.input==input)?;let node=&g.nodes[&g.pins[&edge.output].node];
  if matches!(node.kind,NodeKind::VariableGet|NodeKind::SetVariable|NodeKind::LocalVariable){return (g.variable_node_name(node.id).as_deref()==Some("message")).then_some(node.id);}
  if !matches!(node.kind,NodeKind::Reroute|NodeKind::ConvertStringToText){return None;}input=g.pin_by_key(node.id,"value")?.id;
 }panic!("actual data chain exceeds bound")
}
fn generated(name:&str,source:&str,late_assignment:bool,native_init:bool)->(ParsedProgram,GraphDocument){
 let out=dir(name);std::fs::write(out.join("authored.rvn"),source).unwrap();let mut project=SourceProject::open(source,&[]).unwrap();let mut graphs=project.graphs().to_vec();
 let g=graphs.iter_mut().find(|g|scene(g)).unwrap();
 let dialogues=g.nodes.values().filter(|n|n.kind==NodeKind::Dialogue&&variable_source(g,n.id).is_some()).map(|n|n.id).collect::<Vec<_>>();assert_eq!(dialogues.len(),2);
 if late_assignment{let assignments=g.nodes.values().filter(|n|n.kind==NodeKind::SetVariable&&g.variable_node_name(n.id).as_deref()==Some("message")).map(|n|n.id).collect::<Vec<_>>();let [assignment]=assignments.as_slice()else{panic!("one real selected-branch SET")};let output=g.pin_by_key(*assignment,"value_out").unwrap().id;for dialogue in dialogues{mount::replace_text_source(g,dialogue,output,false);}}
 g.enable_native_frame_preparation().unwrap();let expected=g.clone();
 if native_init{let init=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Init)).unwrap();assert!(init.pins.values().any(|p|matches!(&p.default_value,Some(PropertyValue::Name(n))if n.as_str()=="asset:Fusion0642")));init.enable_native_frame_preparation().unwrap();}
 project.apply_visual(source,&graphs).expect("actual mixed graph strict source writer");
 let file=out.join("main.rvn");std::fs::write(&file,project.source()).unwrap();let parsed=rvn_parser::parse_file_program_with_uses(&file).unwrap();
 let restored=import_native_frame_script(&parsed.script).unwrap();let actual=restored.iter().find(|g|g.graph_id==expected.graph_id).unwrap();assert_eq!(actual.edges,expected.edges);assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),expected.nodes.keys().collect::<Vec<_>>());assert_eq!(actual.pins.keys().collect::<Vec<_>>(),expected.pins.keys().collect::<Vec<_>>());
 std::fs::write(out.join("scene.json"),expected.to_pretty_json().unwrap()).unwrap();(parsed,expected)
}
fn finish(e:&mut Engine<Host>,s:&mut DebugSession){for _ in 0..128{if e.debug_step(s,StepKind::Continue).unwrap().0==DebugProgress::Complete{return;}}panic!("actual public debugger operation did not finish")}
fn state(e:&Engine<Host>)->serde_json::Value{serde_json::to_value(&e.state).unwrap()}
#[test]
fn actual_hotspot_selection_then_future_set_data_output_reads_current_message_once(){
 let old=include_str!("../../../fixtures/scene.rvn").replace("\r\n","\n");assert_eq!(old.matches("set chosen=random(1,9)\n\"left branch\"").count(),1);
 let authored=format!("function late_message(){{local draw=random(1,9) return \"after\"}}\ninit {{set message=\"before\"}}\n{}",old.replace("set chosen=random(1,9)\n\"left branch\"","\"[message]\"\nset message=late_message()\n\"[message]\"").replace("set chosen=2","set unselected=2"));
 let (p,_)=generated("hotspot-future-set",&authored,true,false);let mut e=Engine::from_program_with_debugger(p,Host::default(),16).unwrap();e.state.random=RandomState::seeded(11);
 assert!(matches!(e.step_until_interaction().unwrap(),Some(Interaction::Imagemap{hotspots,..})if hotspots.len()==2));assert_eq!(e.state.random,RandomState::seeded(11));
 let input=e.preview_input_source().unwrap();assert_eq!(input.choice_indices,vec![0,1]);let before=state(&e);
 assert!(e.debug_begin(DebugOperation::Hotspot{source:input.clone(),index:9},vec![]).is_err());assert_eq!(state(&e),before);
 let mut selected=e.debug_begin(DebugOperation::Hotspot{source:input.clone(),index:0},vec![]).unwrap();finish(&mut e,&mut selected);assert!(!e.accepts_preview_input_source(&input));
 assert_eq!(e.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"before".into()}));assert_eq!(e.state.random,RandomState::seeded(11));assert!(!e.state.vars.contains_key("unselected"));
 e.advance_dialogue().unwrap();let mut random=RandomState::seeded(11);random.integer(1,9).unwrap();assert_eq!(e.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"after".into()}));assert_eq!(e.state.random,random);assert!(!e.state.vars.contains_key("unselected"));
 let current=state(&e);e.step_silent().unwrap();assert_eq!(state(&e),current);
}
fn resources()->rvn_core::string_table::StringTableResources{
 use rvn_core::string_table::{StringTableResources,StringTableAsset,StringTableEntry};let mut r=StringTableResources::default();
 r.publish(StringTableAsset{id:"asset:Fusion0642".into(),namespace:"VN.Fusion0642".into(),entries:BTreeMap::from([("hello".into(),StringTableEntry{source:"Table before".into(),developer_notes:String::new(),metadata:Default::default()}),("bye".into(),StringTableEntry{source:"Table after".into(),developer_notes:String::new(),metadata:Default::default()})])}).unwrap();r
}
fn typed_engine(p:ParsedProgram)->Engine<Host>{let mut e=Engine::from_program_with_debugger_and_text_context(p,Host::default(),16,Default::default(),TextRuntimeContext::new(resources(),None)).unwrap();e.state.random=RandomState::seeded(11);e}
fn first_dialogue(g:&GraphDocument)->NodeId{let mut node=g.nodes.values().find(|n|n.kind==NodeKind::Label).unwrap().id;for _ in 0..64{if g.nodes[&node].kind==NodeKind::Dialogue{return node;}let pin=g.pin_by_key(node,"exec_out").unwrap().id;let edge=g.edges.values().find(|e|e.output==pin).unwrap();node=g.pins[&edge.input].node;}panic!("actual first execution Dialogue")}
fn producer_breakpoint(e:&Engine<Host>,g:&GraphDocument,node:NodeId,pin:PinId)->Breakpoint{let mut offset=0;loop{let page=e.debug_site_page(offset).unwrap();if let Some(found)=page.entries.iter().find(|entry|entry.origin.as_ref().and_then(|o|o.scheduled_step.as_ref()).and_then(|s|s.endpoint.as_ref()).is_some_and(|source|source.graph==g.graph_id.get()&&source.node==node.get()&&source.ports.iter().any(|p|p.pin==pin.get()))){return found.breakpoint.clone();}offset+=page.entries.len();assert!(offset<page.total,"actual output breakpoint, never synthesized source identity");}}
#[test]
fn static_character_and_typed_name_table_dynamic_dialogue_cold_public_stop_and_rollback(){
 let source="init {set message=__rvn_string_table_text_name(__rvn_literal_name(\"asset:Fusion0642\"),\"hello\")}\nscene label start\ncharacter.create(\"hero\",\"Héroïne😀\")\nhero \"[message]\"\nset message=__rvn_string_table_text_name(__rvn_literal_name(\"asset:Fusion0642\"),\"bye\")\nhero \"[message]\"\n";
 let (p,g)=generated("character-name-table",source,false,true);let first=first_dialogue(&g);let input=g.pin_by_key(first,"text").unwrap().id;let text_edge=g.edges.values().find(|e|e.input==input).unwrap();let producer=g.pins[&text_edge.output].node;
 let character=g.pin_by_key(first,"character").unwrap().id;let character_edge=g.edges.values().find(|e|e.input==character).unwrap();assert_eq!(g.nodes[&g.pins[&character_edge.output].node].kind,NodeKind::CharacterValue);
 let mut warm=typed_engine(p.clone());warm.enter_label("start").unwrap();
 for _ in 0..64{if matches!(warm.peek_statement(),Some(Statement::ScheduledConsumer{body,..})if matches!(body.as_slice(),[Statement::Dialogue{..}])){break;}warm.step_silent().unwrap();}
 assert_eq!(warm.state.layered.display_name("hero"),"Héroïne😀");let before=state(&warm);let breakpoint=producer_breakpoint(&warm,&g,producer,text_edge.output);
 let mut paused=warm.debug_begin(DebugOperation::Statement,vec![breakpoint]).unwrap();assert_eq!(warm.debug_step(&mut paused,StepKind::Continue).unwrap().0,DebugProgress::Stopped);assert_eq!(state(&warm),before);
 let saved=warm.capture_save15(&p,Some(&paused),1,"actual Character/table producer stop".into()).unwrap().encode().unwrap();finish(&mut warm,&mut paused);
 let mut cold=typed_engine(p.clone());let mut resumed=cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&saved).unwrap()).unwrap().unwrap();finish(&mut cold,&mut resumed);assert_eq!(state(&cold),state(&warm));
 let shown=Some(Interaction::Dialogue{character:Some("hero".into()),text:"Table before".into()});assert_eq!(cold.displayed_interaction().unwrap(),shown);assert_eq!(cold.step_until_interaction().unwrap(),shown);let first_state=state(&cold);assert_eq!(cold.state.random,RandomState::seeded(11));
 assert!(matches!(&cold.state.vars["message"],Value::Text(_)));assert!(serde_json::to_string(&cold.state.vars["message"]).unwrap().contains("name_string_table"));
 cold.advance_dialogue().unwrap();assert_eq!(cold.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:Some("hero".into()),text:"Table after".into()}));assert_eq!(cold.state.random,RandomState::seeded(11));let latest=state(&cold);
 assert!(cold.rollback());assert_eq!(state(&cold),latest);assert!(cold.rollback());assert_eq!(state(&cold),first_state);assert_eq!(cold.displayed_interaction().unwrap(),shown);assert_eq!(cold.state.layered.display_name("hero"),"Héroïne😀");
}
