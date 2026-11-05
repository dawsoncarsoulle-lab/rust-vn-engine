//! One genuine strict Graph/source/owner path for the existing fourteen classic families.
//! Static operands stay static; the separately wired RNG assignment owns its one draw.
use rvn_core::{Engine,GameState,Renderer,renderer::SceneRenderBatch,types::SpriteState};
use rvn_graph::*;
use rvn_parser::{Hotspot,ParsedProgram,Position,Transition,Value};
use std::{path::PathBuf,sync::{Arc,Mutex,atomic::{AtomicBool,AtomicUsize,Ordering}}};
#[path="support/strict_native_consumer.rs"]
mod strict_native_consumer;
#[derive(Clone,Default)]
struct Host{batches:Arc<Mutex<Vec<SceneRenderBatch>>>,events:Arc<Mutex<Vec<String>>>,reject:Arc<AtomicBool>}
impl Renderer for Host{
 fn validate_scene_batch(&self,_:&SceneRenderBatch)->Result<(),String>{if self.reject.load(Ordering::SeqCst){Err("Actual classic family renderer refuses publication".into())}else{Ok(())}}
 fn publish_scene_batch(&mut self,batch:SceneRenderBatch){self.batches.lock().unwrap().push(batch);}
 fn unlock_ending(&mut self,id:&str){self.events.lock().unwrap().push(format!("ending:{id}"));}
 fn set_background(&mut self,_:&str,_:&Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{panic!("No choice producer belongs to this actual fixture")}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{panic!("No fabricated input response")}
 fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&Transition){}
 fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
#[derive(Clone,Copy)]
struct Family{id:&'static str,kind:NodeKind,body:&'static str,setup:&'static str,reroute:Option<&'static str>}
// These identities are the existing CLASSIC_EFFECT_TRANSACTION14 rows of matrix0994.
const FAMILIES:&[Family]=&[
 Family{id:"D-CATALOG-Config",kind:NodeKind::Config,body:"bg: \"garden.png\"",setup:"",reroute:Some("value")},
 Family{id:"D-CATALOG-CharacterCreate",kind:NodeKind::CharacterCreate,body:"character.create(\"hero\",\"Hero\")",setup:"",reroute:Some("display_name")},
 Family{id:"D-CATALOG-Scene",kind:NodeKind::Scene,body:"scene \"garden.png\"",setup:"",reroute:Some("background")},
 Family{id:"D-CATALOG-CinematicShow",kind:NodeKind::CinematicShow,body:"cinematic \"cg.png\" with fade",setup:"",reroute:Some("cinematic")},
 Family{id:"D-CATALOG-CinematicHide",kind:NodeKind::CinematicHide,body:"cinematic hide with dissolve",setup:"cinematic \"previous.png\"\n",reroute:Some("transition")},
 Family{id:"D-CATALOG-UnlockEnding",kind:NodeKind::UnlockEnding,body:"unlock_ending \"ending\"",setup:"",reroute:Some("id")},
 Family{id:"D-CATALOG-SpriteShow",kind:NodeKind::SpriteShow,body:"hero.show() at center",setup:"",reroute:Some("character")},
 Family{id:"D-CATALOG-SpriteHide",kind:NodeKind::SpriteHide,body:"hero.hide()",setup:"hero.show() at center\n",reroute:Some("character")},
 Family{id:"D-CATALOG-SpriteMove",kind:NodeKind::SpriteMove,body:"hero.move() at left",setup:"hero.show() at center\n",reroute:Some("position")},
 Family{id:"D-CATALOG-SpriteAnimate",kind:NodeKind::SpriteAnimate,body:"hero.animate(\"bounce\",loop:true,speed:1.25,height:27.0,duration:0.75)",setup:"hero.show() at center\n",reroute:Some("animation")},
 Family{id:"D-CATALOG-SpriteStopAnimation",kind:NodeKind::SpriteStopAnimation,body:"hero.stop_animation()",setup:"hero.show() at center\nhero.animate(\"bounce\")\n",reroute:Some("character")},
 Family{id:"D-CATALOG-SpriteEffect",kind:NodeKind::SpriteEffect,body:"hero.effect(scale:1.25,flip_x:true,flip_y:true,rotation:-45.0,tint:\"#4a8c\")",setup:"hero.show() at center\n",reroute:Some("scale")},
 Family{id:"D-CATALOG-Timer",kind:NodeKind::Timer,body:"timer 0.25 => jump start",setup:"",reroute:Some("duration")},
 Family{id:"D-CATALOG-TimerCancel",kind:NodeKind::TimerCancel,body:"timer cancel",setup:"timer 0.5 => jump start\n",reroute:None},
];
fn directory(name:&str)->PathBuf{
 static NEXT:AtomicUsize=AtomicUsize::new(0);
 let root=PathBuf::from(std::env::var("RVN_NATIVE_TYPEWRITER_FIXTURES").expect("Actual private fixture root is required"));
 let p=root.join(format!("classic0999-{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir_all(&p).unwrap();p
}
fn authored(name:&str,source:&str)->(PathBuf,ParsedProgram,Vec<GraphDocument>){
 let dir=directory(name);let file=dir.join("authored.rvn");std::fs::write(&file,source).unwrap();let parsed=rvn_parser::parse_file_program_with_uses(file).unwrap();let graphs=import_script(&parsed.script).unwrap();(dir,parsed,graphs)
}
fn compiled(f:Family,init:bool,game:bool,reroute:bool)->ParsedProgram{compiled_adapter(f,init,game,reroute,false)}
fn compiled_adapter(f:Family,init:bool,game:bool,reroute:bool,adapter:bool)->ParsedProgram{
 let body_text=if adapter&&f.kind==NodeKind::Timer {"timer 16777216 => jump start"}else{f.body};
 let body=format!("{}set sampled=random(1,9)\n\"before\"\n{}\n\"after\"\n",f.setup,body_text);
 let source=if init{format!("init{{{}set sampled=random(1,9)\n{}\n}}\nscene label start\n\"after\"\n",f.setup,body_text)}else{format!("{}label start\n{body}scene label later\n\"later\"\n",if game{""}else{"scene "})};
 let source=if init && f.kind==NodeKind::CharacterCreate {"init{set sampled=random(1,9)\n}\nscene label start\n\"after\"\n".to_owned()}else{source};
 let(dir,parsed,mut graphs)=authored(f.id,&source);
 if init && f.kind==NodeKind::CharacterCreate {
  // Ordinary Init character declarations author the legacy shared registry.
  // Create the actual executable Init catalogue node, as Character18 does.
  let graph=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Init)).unwrap();
  let previous=graph.nodes.values().find(|n|n.kind==NodeKind::SetVariable).unwrap().id;
  let node=graph.add_catalog_node(NodeKind::CharacterCreate,[400.,240.]).unwrap();
  for(key,value)in[("id","hero"),("display_name","Hero")]{graph.set_pin_default(node,key,PropertyValue::String(value.into())).unwrap();}
  graph.connect(graph.pin_by_key(previous,"exec_out").unwrap().id,graph.pin_by_key(node,"exec_in").unwrap().id).unwrap();graph.materialize_visible_defaults().unwrap();
 }
 if adapter {
  let graph=graphs.iter_mut().find(|g|if init{matches!(g.kind,GraphKind::Init)}else{matches!(&g.kind,GraphKind::Label{name}if name=="start")}).unwrap();
  let command=graph.nodes.values().filter(|n|n.kind==f.kind).max_by_key(|n|n.id).unwrap().id;
  if f.kind==NodeKind::SpriteShow {
   let character_input=graph.pin_by_key(command,"character").unwrap().id;let output=graph.edges.values().find(|edge|edge.input==character_input).unwrap().output;let character=graph.pins[&output].node;assert_eq!(graph.nodes[&character].kind,NodeKind::CharacterValue);
   let asset=graph.add_catalog_node(NodeKind::SpriteAsset,[100.,300.]).unwrap();graph.set_property(asset,"path",PropertyValue::String("hero.png".into())).unwrap();
   let mut output=graph.pin_by_key(asset,"value").unwrap().id;
   if reroute {let route=graph.add_catalog_node(NodeKind::Reroute,[200.,300.]).unwrap();graph.set_reroute_type(route,ValueType::Asset(AssetKind::Sprite)).unwrap();graph.connect(output,graph.pin_by_key(route,"value").unwrap().id).unwrap();output=graph.pin_by_key(route,"value_out").unwrap().id;}
   graph.connect(output,graph.pin_by_key(character,"sprite").unwrap().id).unwrap();
  }else if f.kind==NodeKind::Timer {
   let input=graph.pin_by_key(command,"duration").unwrap().id;let old=graph.edges.values().find(|edge|edge.input==input).unwrap().clone();let old_node=graph.pins[&old.output].node;graph.remove_edge(old.id);graph.remove_node(old_node).unwrap();
   let conversion=graph.add_catalog_node(NodeKind::ConvertIntToFloat,[300.,400.]).unwrap();
   //16777217 must round to16777216 in the existing RVN f32 domain. Game/plain
   // also covers a genuine inline Int pin default without inventing a wire.
   if game&&!reroute {graph.set_pin_default(conversion,"value",PropertyValue::Int(16777217)).unwrap();}
   else {
    let integer=graph.add_catalog_node(NodeKind::Literal,[100.,400.]).unwrap();graph.set_property(integer,"value",PropertyValue::Int(16777217)).unwrap();let mut output=graph.pin_by_key(integer,"value").unwrap().id;
    if reroute {let route=graph.add_catalog_node(NodeKind::Reroute,[200.,400.]).unwrap();graph.set_reroute_type(route,ValueType::Int).unwrap();graph.connect(output,graph.pin_by_key(route,"value").unwrap().id).unwrap();output=graph.pin_by_key(route,"value_out").unwrap().id;}
    graph.connect(output,graph.pin_by_key(conversion,"value").unwrap().id).unwrap();
   }
   graph.connect(graph.pin_by_key(conversion,"result").unwrap().id,input).unwrap();
  }else{panic!("Only the two actual static adapters belong to this fixture");}
 }
 if reroute{
  if let Some(role)=f.reroute{
   let graph=graphs.iter_mut().find(|g|if init{matches!(g.kind,GraphKind::Init)}else{matches!(&g.kind,GraphKind::Label{name}if name=="start")}).unwrap();
   let consumer=graph.nodes.values().filter(|n|n.kind==f.kind).max_by_key(|n|n.id).unwrap().id;let input=graph.pin_by_key(consumer,role).unwrap().id;
   let old=graph.edges.values().find(|e|e.input==input).expect("Actual imported default must have its visible source wire").clone();let ty=graph.effective_pin_type(old.output).unwrap();
   let node=graph.add_catalog_node(NodeKind::Reroute,[640.,400.]).unwrap();graph.set_reroute_type(node,ty).unwrap();
   let ri=graph.pin_by_key(node,"value").unwrap().id;let ro=graph.pin_by_key(node,"value_out").unwrap().id;
   graph.remove_edge(old.id);graph.connect(old.output,ri).unwrap();graph.connect(ro,input).unwrap();
  }
 }
 strict_native_consumer::finish_program(dir,parsed,graphs,&[f.kind])
}
fn effect(state:&GameState,host:&Host,f:Family,adapter:bool){match f.kind{
 NodeKind::Config|NodeKind::Scene=>assert_eq!(state.background_image,"garden.png"),
 NodeKind::CharacterCreate=>assert_eq!(state.layered.display_names["hero"],"Hero"),
 NodeKind::CinematicShow=>assert_eq!(state.cinematic.current.as_deref(),Some("cg.png")),
 NodeKind::CinematicHide=>assert!(state.cinematic.current.is_none()),
 NodeKind::UnlockEnding=>assert_eq!(*host.events.lock().unwrap(),vec!["ending:ending"]),
 NodeKind::SpriteShow=>{assert!(state.sprites["hero"].visible);if adapter{assert_eq!(state.sprites["hero"].emotion.as_deref(),Some("./hero.png"));}},
 NodeKind::SpriteHide=>assert!(!state.sprites["hero"].visible),
 NodeKind::SpriteMove=>assert_eq!(state.sprites["hero"].position,Position::Left),
 NodeKind::SpriteEffect=>{let execution=state.scene_execution.as_ref().unwrap();assert_eq!(execution.sprite_styles["hero"].scale,1.25);assert!(execution.sprite_styles["hero"].flip_x);assert_eq!(execution.sprite_styles["hero"].rotation,-45.0);assert!(execution.sprite_styles["hero"].flip_y);assert_eq!(execution.sprite_styles["hero"].tint.as_deref(),Some("#4a8c"));},
 NodeKind::SpriteAnimate|NodeKind::SpriteStopAnimation=>{let execution=state.scene_execution.as_ref().unwrap();let tasks:Vec<_>=execution.resources.active_effects(execution.owner()).into_iter().filter(|task|task.effect.as_ref().is_some_and(|effect|matches!(effect.parameters,rvn_core::scene_resources::EffectParameters::SpriteAnimation{..}))).collect();assert_eq!(tasks.len(),usize::from(f.kind==NodeKind::SpriteAnimate));
 if f.kind==NodeKind::SpriteAnimate{let actual=tasks[0].effect.as_ref().unwrap();assert!(actual.clock.looping);assert_eq!(actual.clock.duration,0.75);assert_eq!(actual.clock.elapsed,0.0);match &actual.parameters{rvn_core::scene_resources::EffectParameters::SpriteAnimation{animation:rvn_core::scene_resources::SpriteEffectKind::Bounce{height},..}=>assert_eq!(*height,27.0),other=>panic!("Actual bounce parameters were lost: {other:?}")};}},
 NodeKind::Timer|NodeKind::TimerCancel=>{let execution=state.scene_execution.as_ref().unwrap();let tasks=execution.active.as_ref().map(|a|&a.tasks).unwrap_or(&execution.game_tasks);if f.kind==NodeKind::Timer{assert_eq!(tasks.timer.as_ref().unwrap().duration,if adapter{16777216.0}else{0.25});}else{assert!(tasks.timer.is_none());}},
 _=>panic!("Not an original fourteen-family criterion: {}",f.id),
}}
fn actual_consumer_frame(e:&Engine<Host>,game:bool)->&rvn_core::consumer_schedule::BoundConsumerFrame{
 // The project includes a real later Scene even when start is a Game label.
 // The constructor's persistent Game slot is not the active Scene's frame.
 let execution=e.state.scene_execution.as_ref().expect("The complete real project retains its Scene entries");
 if let Some(active)=execution.active.as_ref(){
  assert!(!game,"The actual Game entry must not fabricate a Scene activation");assert_eq!(active.scene,"start");
  assert!(matches!(e.script.get(active.entry_pc),Some(rvn_parser::Statement::SceneLabel{name})if name=="start"),"The activation must point to its actual compiled Scene entry");
  let frame=active.consumer_frame.as_ref().expect("The actual Scene activation retains its own NativeFrame");
  assert_eq!(frame.owner,rvn_core::consumer_schedule::ConsumerOwner::Scene{activation:active.id});
  if let Some(game_frame)=e.state.consumer_frame.as_ref(){assert_eq!(game_frame.owner,rvn_core::consumer_schedule::ConsumerOwner::Game);}
  frame
 }else{
  assert!(game,"The actual Scene entry must have its real activation");
  let frame=e.state.consumer_frame.as_ref().expect("The actual Game owner retains its NativeFrame");assert_eq!(frame.owner,rvn_core::consumer_schedule::ConsumerOwner::Game);frame
 }
}
fn reject_frozen_forgery(p:&ParsedProgram){
 let binding=rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(p).unwrap();
 for mode in 0..3{
  let mut changed=p.clone();match mode{0=>changed.sources[0].source.push_str("\n// foreign source bytes\n"),1=>changed.sources.clear(),_=>changed.origins.clear()};
  let host=Host::default();assert!(Engine::from_program_with_consumer_binding(changed,binding.clone(),host.clone(),8).is_err(),"Changed actual source metadata mode{mode} must refuse before Init");
  assert!(host.batches.lock().unwrap().is_empty());assert!(host.events.lock().unwrap().is_empty());
 }
}
fn run_family(f:Family){run_family_once(f,false);if matches!(f.kind,NodeKind::SpriteShow|NodeKind::Timer){run_family_once(f,true);}}
fn run_family_once(f:Family,adapter:bool){
 if f.kind==NodeKind::SpriteEffect {
  for field in ["scale","rotation"]{
   for number in ["-45.0","-45","0.0","1.25"]{assert!(rvn_parser::parse(&format!("hero.effect({field}:{number})\n")).is_ok());}
   for bad in ["true","-name","--1"]{let error=rvn_parser::parse(&format!("hero.effect({field}:{bad})\n")).unwrap_err().to_string();assert!(error.contains("finite signed numeric literal in sprite.effect"),"Actual signed-literal refusal: {error}");}
   assert!(rvn_parser::parse(&format!("hero.effect({field}:1+2)\n")).is_err());
  }
 }
 for reroute in [false,true]{
  for game in [false,true]{
   let p=compiled_adapter(f,false,game,reroute,adapter);let binding=rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&p).unwrap();let host=Host::default();
   let mut e=Engine::from_program_with_consumer_binding(p.clone(),binding,host.clone(),8).unwrap();e.state.random=rvn_core::random::RandomState::seeded(17);let initial_rng=e.state.random;let mut expected=initial_rng;let sampled=expected.integer(1,9).unwrap();
   // The real silent-step API drains every silent command to the next dialogue.
   // Draw the separately wired assignment before this interaction boundary,
   // then stop on the actual command before enabling renderer rejection.
   e.step_until_interaction().unwrap();assert_eq!(e.state.random,expected);assert_eq!(e.state.vars["sampled"],Value::Int(sampled));let before_boundary=e.state.clone();e.advance_dialogue().unwrap();assert_eq!(e.state.pc,before_boundary.pc+1,"Exactly the displayed source dialogue was consumed; the following static command is still unexecuted");
   let kind=serde_json::to_string(&f.kind).unwrap();let marker=format!(":{}|",kind.trim_matches('"'));
   assert!(matches!(e.peek_statement(),Some(rvn_parser::Statement::ScheduledConsumer{version:2,consumer,body,..})if consumer.contains(&marker)&&body.len()==1),"The next real source command must be the exact tested classic consumer before its first publication");
   assert_eq!(e.state.random,expected);assert_eq!(e.state.vars["sampled"],Value::Int(sampled));
   let session=e.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();let before=serde_json::to_value(&e.state).unwrap();let batches=host.batches.lock().unwrap().len();let events=host.events.lock().unwrap().clone();
   host.reject.store(true,Ordering::SeqCst);let refusal=e.step_silent().expect_err("The exact pending classic consumer must reach actual renderer refusal");assert!(refusal.to_string().contains("Actual classic family renderer refuses publication"),"Actual renderer refusal: {refusal}");assert_eq!(serde_json::to_value(&e.state).unwrap(),before);assert_eq!(host.batches.lock().unwrap().len(),batches);assert_eq!(*host.events.lock().unwrap(),events);assert!(e.accepts_debug_session(&session));
   host.reject.store(false,Ordering::SeqCst);e.step_silent().unwrap();effect(&e.state,&host,f,adapter);effect(&host.batches.lock().unwrap().last().unwrap().state,&host,f,adapter);assert_eq!(e.state.random,expected);assert_eq!(host.batches.lock().unwrap().len(),batches+1);
   actual_consumer_frame(&e,game);
   e.step_until_interaction().unwrap();let save=e.capture_save15(&p,None,1,f.id.into()).unwrap();let wire=save.encode().unwrap();let cold_host=Host::default();let mut cold=Engine::from_program(p.clone(),cold_host.clone(),8).unwrap();cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&wire).unwrap()).unwrap();assert_eq!(cold.state.random,expected);actual_consumer_frame(&cold,game);if f.kind!=NodeKind::UnlockEnding{effect(&cold.state,&cold_host,f,adapter);}assert!(cold_host.events.lock().unwrap().is_empty());
   let after=serde_json::to_value(&cold.state).unwrap();let count=cold_host.batches.lock().unwrap().len();cold.step_silent().unwrap();assert_eq!(serde_json::to_value(&cold.state).unwrap(),after);assert_eq!(cold_host.batches.lock().unwrap().len(),count);
   // Save15 above reached and recorded the actual after-dialogue. Rollback
   // restores/popslast, including that current interaction, not skiplast.
   let targets=e.history.entries().iter().rev().map(|entry|entry.state.clone()).collect::<Vec<_>>();assert_eq!(targets.len(),2,"Only the two actual before/after interactions belong to this fixture");
   assert_eq!(targets[0].pc,e.state.pc);assert_eq!(serde_json::to_value(&targets[1]).unwrap(),serde_json::to_value(&before_boundary).unwrap());
   let history=|engine:&Engine<Host>|engine.history.entries().iter().map(|entry|serde_json::to_value((&entry.state,&entry.display)).unwrap()).collect::<Vec<_>>();
   for (index,target) in targets.iter().enumerate(){
    let state=serde_json::to_value(&e.state).unwrap();let prior_history=history(&e);let count=host.batches.lock().unwrap().len();let events=host.events.lock().unwrap().clone();let session=e.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();
    host.reject.store(true,Ordering::SeqCst);assert!(!e.rollback());assert_eq!(serde_json::to_value(&e.state).unwrap(),state);assert_eq!(history(&e),prior_history);assert_eq!(host.batches.lock().unwrap().len(),count);assert_eq!(*host.events.lock().unwrap(),events);assert!(e.accepts_debug_session(&session));
    host.reject.store(false,Ordering::SeqCst);assert!(e.rollback());assert_eq!(e.history.len()+1,prior_history.len());assert_eq!(serde_json::to_value(&e.state).unwrap(),serde_json::to_value(target).unwrap(),"The actual latest history entry must restore in full, not an invented earlier PC");assert_eq!(e.state.random,target.random);assert_eq!(e.state.pc,target.pc);assert_eq!(host.batches.lock().unwrap().len(),count+1);actual_consumer_frame(&e,game);
    assert_eq!(e.displayed_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:if index==0{"after"}else{"before"}.into()}));
   }
   assert_eq!(e.state.random,before_boundary.random);assert_eq!(e.state.pc,before_boundary.pc);
   reject_frozen_forgery(&p);
  }
  let p=compiled_adapter(f,true,false,reroute,adapter);reject_frozen_forgery(&p);let host=Host::default();let mut owner=Engine::prepare_debugger_startup(p,host.clone(),8,Default::default(),Default::default(),None).unwrap();let initial=owner.state().random;let mut expected=initial;expected.integer(1,9).unwrap();
  for _ in 0..96{
   if owner.is_complete(){break;}
   let before=serde_json::to_value(owner.state()).unwrap();let count=host.batches.lock().unwrap().len();let events=host.events.lock().unwrap().clone();let plan=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();let stale=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();assert_eq!(serde_json::to_value(owner.state()).unwrap(),before);assert_eq!(host.batches.lock().unwrap().len(),count);assert_eq!(*host.events.lock().unwrap(),events);
   owner.begin_prepared_step_commit(plan).unwrap().commit();let after=serde_json::to_value(owner.state()).unwrap();assert!(owner.begin_prepared_step_commit(stale).is_err());assert_eq!(serde_json::to_value(owner.state()).unwrap(),after);
  }
  assert!(owner.is_complete(),"Actual Init continuation did not complete for {}",f.id);assert_eq!(owner.state().random,expected);effect(owner.state(),&host,f,adapter);
 }
}
macro_rules! family_case{($name:ident,$index:expr)=>{#[test]fn $name(){run_family(FAMILIES[$index]);}}}
family_case!(strict0999_config_actual_graph_owner_source_and_transactions,0);
family_case!(strict0999_character_create_actual_graph_owner_source_and_transactions,1);
family_case!(strict0999_scene_actual_graph_owner_source_and_transactions,2);
family_case!(strict0999_cinematic_show_actual_graph_owner_source_and_transactions,3);
family_case!(strict0999_cinematic_hide_actual_graph_owner_source_and_transactions,4);
family_case!(strict0999_unlock_ending_actual_graph_owner_source_and_transactions,5);
family_case!(strict0999_sprite_show_actual_graph_owner_source_and_transactions,6);
family_case!(strict0999_sprite_hide_actual_graph_owner_source_and_transactions,7);
family_case!(strict0999_sprite_move_actual_graph_owner_source_and_transactions,8);
family_case!(strict0999_sprite_animate_actual_graph_owner_source_and_transactions,9);
family_case!(strict0999_sprite_stop_animation_actual_graph_owner_source_and_transactions,10);
family_case!(strict0999_sprite_effect_actual_graph_owner_source_and_transactions,11);
family_case!(strict0999_timer_actual_graph_owner_source_and_transactions,12);
family_case!(strict0999_timer_cancel_actual_graph_owner_source_and_transactions,13);

#[test]
fn strict0999_connected_character_sprite_authenticates_actual_image_sources_before_publication(){
 let p=compiled_adapter(FAMILIES[6],false,false,false,true);let raw=&p.sources[0].source;
 let line=raw.lines().find(|line|line.trim_start().starts_with("capture ")&&line.contains(":sprite_asset|")).expect("The true connected Sprite must have its own earlier captured source");
 assert!(line.ends_with("= \"hero.png\""));
 let wrong_path=raw.replacen(line,&line.replace("= \"hero.png\"","= \"other.png\""),1);
 refuse_reparsed_source("sprite-path-forgery",&wrong_path,"Static Character Sprite path differs from the actual ShowSprite appearance");
 let foreign_kind=raw.replacen(line,&line.replacen(":sprite_asset|",":scene_asset|",1),1);
 refuse_reparsed_source("sprite-kind-forgery",&foreign_kind,"Static Character Sprite requires a genuine SpriteAsset producer");
 let absent=raw.replacen(&format!("{line}\n"),"",1);assert_ne!(&absent,raw);
 refuse_reparsed_source("sprite-source-absent",&absent,"Static Character Sprite source is missing, forward or ambiguous");
 reject_frozen_forgery(&p);
 // A conflicting legacy emotion port cannot silently borrow the Sprite path.
 let(_,_,mut graphs)=authored("sprite-conflict","scene label start\nhero.show() at center\n\"after\"\n");
 let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();graph.enable_native_frame_preparation().unwrap();
 let command=graph.nodes.values().find(|node|node.kind==NodeKind::SpriteShow).unwrap().id;let input=graph.pin_by_key(command,"character").unwrap().id;let output=graph.edges.values().find(|edge|edge.input==input).unwrap().output;let character=graph.pins[&output].node;
 let sprite=graph.add_catalog_node(NodeKind::SpriteAsset,[100.,300.]).unwrap();graph.set_property(sprite,"path",PropertyValue::String("hero.png".into())).unwrap();graph.connect(graph.pin_by_key(sprite,"value").unwrap().id,graph.pin_by_key(character,"sprite").unwrap().id).unwrap();
 graph.set_pin_default(command,"emotion",PropertyValue::String("conflicting.png".into())).unwrap();let before=graph.clone();let error=prepare_native_frame_graph(graph).unwrap_err().to_string();assert!(error.contains("native connected Character Sprite cannot also claim an unused legacy emotion operand"),"Actual conflicting authored source diagnostic: {error}");assert_eq!(*graph,before);
 // Dialogue still emits an appearance and a Dialogue, which cannot share one
 // v2 execution consumer. This distinct multi-command adapter remains open.
 let(_,_,mut graphs)=authored("dialogue-appearance-open","scene label start\nhero \"line\"\n");let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();graph.enable_native_frame_preparation().unwrap();
 let character=graph.nodes.values().find(|node|node.kind==NodeKind::CharacterValue).unwrap().id;let sprite=graph.add_catalog_node(NodeKind::SpriteAsset,[100.,300.]).unwrap();graph.set_property(sprite,"path",PropertyValue::String("hero.png".into())).unwrap();graph.connect(graph.pin_by_key(sprite,"value").unwrap().id,graph.pin_by_key(character,"sprite").unwrap().id).unwrap();let before=graph.clone();let error=prepare_native_frame_graph(graph).unwrap_err().to_string();assert!(error.contains("un consommateur v2 représentable, sans boucle pré-évaluée ni continuation opaque"),"The distinct real Dialogue appearance limit remains explicit: {error}");assert_eq!(*graph,before);
}

fn refuse_reparsed_source(name:&str,raw:&str,diagnostic:&str){
 let file=directory(name).join("forged.rvn");std::fs::write(&file,raw).unwrap();let host=Host::default();
 let error=match rvn_parser::parse_file_program_with_uses(&file){Err(error)=>error,Ok(program)=>Engine::from_program(program,host.clone(),8).err().expect("Forged actual source must refuse before any Init/producer/publication").to_string()};
 assert!(error.contains(diagnostic),"Expected actual {diagnostic:?} source refusal: {error}");assert!(host.batches.lock().unwrap().is_empty());assert!(host.events.lock().unwrap().is_empty());
}
#[test]
fn strict0999_all_fourteen_foreign_consumer_kinds_and_duplicate_actual_producers_refuse_before_publication(){
 for family in FAMILIES{
  let program=compiled(*family,true,false,false);let raw=&program.sources[0].source;let kind=serde_json::to_string(&family.kind).unwrap();let marker=format!(":{}|",kind.trim_matches('"'));assert!(raw.contains(&marker));
  let header=raw.lines().find(|line|line.contains("flow.consume_v2")&&line.contains(&marker)).unwrap();
  let start=raw.find(header).unwrap()+header.len();let indent:String=header.chars().take_while(|c|c.is_whitespace()).collect();let end=start+raw[start..].find(&format!("\n{indent}}}")).unwrap();
  let command=raw[start..end].lines().rev().find(|line|!line.trim().is_empty()).unwrap();let command_start=start+raw[start..end].rfind(command).unwrap();
  let mut wrong=raw.clone();wrong.replace_range(command_start..command_start+command.len(),&format!("{indent}    music.volume(0.5)"));assert_ne!(&wrong,raw);refuse_reparsed_source("foreign-kind",&wrong,"Static consumer identity differs from its actual command kind");
  let line=raw.lines().find(|line|line.trim_start().starts_with("capture ")&&line.contains("random(1, 9)")).expect("The real separately wired random assignment declares an actual producer");let duplicate=raw.replacen(&format!("{line}\n"),&format!("{line}\n{line}\n"),1);assert_ne!(&duplicate,raw);
  // Duplicate the first producer of the real random SetVariable, without
  // editing a cached AST, borrowing another consumer, or fabricating a grant.
  refuse_reparsed_source("duplicate-producer",&duplicate,"producteur unique (1024 maximum)");
 }
}
#[test]
fn strict0999_static_numeric_conversion_authenticates_integer_sources_and_refuses_dynamic_evaluation(){
 let p=compiled_adapter(FAMILIES[12],false,false,false,true);let raw=&p.sources[0].source;
 let conversion=raw.lines().find(|line|line.trim_start().starts_with("capture ")&&line.contains(":convert_int_to_float|")).expect("The actual conversion retains its identity and expression");
 assert!(conversion.ends_with("+ 0.0)"));let changed=raw.replacen(conversion,&conversion.replace("+ 0.0)","+ 1.0)"),1);
 refuse_reparsed_source("conversion-offset-forgery",&changed,"Static IntToFloat cannot alter its actual float-zero operand");
 let integer=raw.lines().find(|line|line.trim_start().starts_with("capture ")&&line.contains(":literal|")&&line.ends_with("= 16777217")).expect("The actual integer source retains its own producer");
 let changed=raw.replacen(integer,&integer.replace("= 16777217","= 16777217.0"),1);
 refuse_reparsed_source("conversion-float-forgery",&changed,"Static IntToFloat cannot impersonate a float or computed source as an integer");
 let absent=raw.replacen(&format!("{integer}\n"),"",1);assert_ne!(&absent,raw);refuse_reparsed_source("conversion-source-absent",&absent,"Static adapter source is not active");
 reject_frozen_forgery(&p);
 let(_,_,mut graphs)=authored("conversion-dynamic-refusal","scene label start\ntimer 1 => jump start\n\"after\"\n");let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();graph.enable_native_frame_preparation().unwrap();
 let command=graph.nodes.values().find(|node|node.kind==NodeKind::Timer).unwrap().id;let input=graph.pin_by_key(command,"duration").unwrap().id;let old=graph.edges.values().find(|edge|edge.input==input).unwrap().clone();let old_node=graph.pins[&old.output].node;graph.remove_edge(old.id);graph.remove_node(old_node).unwrap();
 let random=graph.add_catalog_node(NodeKind::FunctionCall,[100.,400.]).unwrap();graph.set_property(random,"function",PropertyValue::String("random".into())).unwrap();graph.resize_value_inputs(random,2).unwrap();graph.set_pin_default(random,"item_0",PropertyValue::Int(1)).unwrap();graph.set_pin_default(random,"item_1",PropertyValue::Int(9)).unwrap();
 let conversion=graph.add_catalog_node(NodeKind::ConvertIntToFloat,[300.,400.]).unwrap();graph.connect(graph.pin_by_key(random,"result").unwrap().id,graph.pin_by_key(conversion,"value").unwrap().id).unwrap();graph.connect(graph.pin_by_key(conversion,"result").unwrap().id,input).unwrap();
 let before=graph.clone();let error=prepare_native_frame_graph(graph).unwrap_err().to_string();assert!(error.contains("requiert une constante RVN, pas une expression dynamique"),"A real random producer is never evaluated by the static adapter: {error}");assert_eq!(*graph,before);
}

#[path="support/native_classic_native_fixture1003.rs"]
mod native_fixture1003;
