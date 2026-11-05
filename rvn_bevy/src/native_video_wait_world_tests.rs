//! Pending autonomous test module, NOT registered/compiled/executed yet.
//! Intended child of native_video::current_source_tests after exact Root merge.
//! No GPU/device qualification: real FFmpeg and Bevy/Rodio Source consumption.
use super::*;
use bevy::audio::Decodable;
const WAIT_SCRIPT:&str=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../fixtures/native-startup-clock-wait.rvn"));

fn actual_wait_world()->(PathBuf,App,Entity){
 let(root,mut app)=fixture_source(WAIT_SCRIPT);
 app.init_resource::<crate::scene_publication::ScenePresentation>()
    .init_resource::<crate::scene_publication::VideoFeedbackQueue>()
    .init_resource::<crate::accessibility::Accessibility>()
    .init_resource::<crate::systems::settings_menu::Settings>()
    .init_resource::<crate::editor_preview::PreviewState>()
    .insert_resource(State::new(VnState::Waiting));
 app.world_mut().resource_mut::<crate::systems::settings_menu::Settings>().language="en".into();
 let window=app.world_mut().spawn((Window{focused:true,..default()},bevy::window::PrimaryWindow)).id();
 // Actual host Continue mode is a test input, not a Core clock capability.
 // All media permission below still requires the actual owner's opaque proof.
 app.world_mut().resource_mut::<DebugBootstrap>().mode=Some(rvn_core::editor_debugger::StepKind::Continue);
 for _ in 0..32{
  if app.world().resource::<DebugBootstrap>().owner.waiting_for_video(){break;}
  let mut plan=crate::debugger_startup::prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
  let end=Instant::now()+Duration::from_secs(20);
  while !plan.ready(){app.update();crate::debugger_startup::refresh_world_step(app.world(),&mut plan).unwrap();assert!(Instant::now()<end,"Actual prepared World media readiness timed out");std::thread::sleep(Duration::from_millis(2));}
  let committed=crate::debugger_startup::commit_world_step(app.world_mut(),plan).unwrap();
  crate::debugger_startup::install_committed_world_step(app.world_mut(),committed);
  assert!(!app.world().resource::<DebugBootstrap>().publication_pending);
  assert!(!app.world().contains_resource::<VnEngine>());
 }
 let startup=app.world().resource::<DebugBootstrap>();
 assert!(startup.owner.waiting_for_video(),"The actual executed Wait must park Init, not run the following RNG");
 assert_eq!(startup.owner.state().videos.waiting.as_deref(),Some("intro"));
 assert!(!startup.owner.state().vars.contains_key("after_wait"));
 assert!(startup.owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).is_err());
 assert!(app.world().resource::<Players>().players.contains_key("intro"),"The true World route must have installed its owned Native reader; no Native-only bypass is allowed");
 (root,app,window)
}
fn actual_pcm(root:&std::path::Path)->Vec<f32>{
 let worker=rvn_media::transport::Worker::spawn_proven(root.join("assets/clip.webm"),None).unwrap();let end=Instant::now()+Duration::from_secs(10);let mut pcm=Vec::new();
 loop{assert!(Instant::now()<end,"Independent actual PCM decode timed out");match worker.poll().unwrap(){Some(rvn_media::transport::Packet::Data(rvn_media::decoder::Chunk::Audio{samples,..}))=>{assert!(pcm.len()+samples.len()<=1_000_000);pcm.extend(samples);},Some(rvn_media::transport::Packet::Data(rvn_media::decoder::Chunk::End))=>break,Some(rvn_media::transport::Packet::Error(e))=>panic!("{e}"),_=>std::thread::yield_now()}}
 assert!(pcm.len()>10_004);assert_ne!(pcm[10_003].to_bits(),0.0f32.to_bits(),"The required pending right must distinguish real PCM from inserted silence");pcm
}
fn pump_actual_native(app:&mut App){
 let mut players=app.world_mut().remove_resource::<Players>().unwrap();
 for player in players.players.values_mut(){super::super::pump(app.world_mut(),player).unwrap();if let Some(audio)=&player.audio{player.position=audio.0.lock().unwrap().position;}super::super::draw(app.world_mut(),player);}
 app.world_mut().insert_resource(players);
}
fn deliver_to(app:&mut App,audio:&VideoAudio,source:&mut crate::video_audio::Samples,pcm:&[f32],target:u64){
 let end=Instant::now()+Duration::from_secs(10);let mut nonzero=false;
 while audio.0.lock().unwrap().delivered_sample_offset().unwrap()<target{
  assert!(Instant::now()<end,"Real admitted reader did not reach the requested delivered ordinal");
  let before=audio.0.lock().unwrap().delivered_sample_offset().unwrap()as usize;
  let value=source.next().expect("A blocked live Source cannot terminate its reader");
  let after=audio.0.lock().unwrap().delivered_sample_offset().unwrap()as usize;
  if after==before+1{assert_eq!(value.to_bits(),pcm[before].to_bits());nonzero|=value!=0.0;}else{assert_eq!(after,before);assert_eq!(value,0.0);std::thread::sleep(Duration::from_millis(1));}
  if after%128==0||after==before{pump_actual_native(app);}
 }
 assert!(nonzero);
}
#[test]
fn actual_wait_focus_then_continue_re_admission_preserves_true_right_cursor_and_reader(){
 let(root,mut app,window)=actual_wait_world();let pcm=actual_pcm(&root);
 let startup=app.world().resource::<DebugBootstrap>();let view=startup.owner.video_views().unwrap()[0].clone();
 let clock=startup.owner.video_clock_source(&view).unwrap().expect("Only the real executed Wait can issue this opaque clock observation");
 assert!(startup.owner.accepts_video_clock_source(&clock));
 assert!(crate::scene_publication::video_clock_allowed(app.world(),&view));
 let state_before=state(app.world());let observation=startup.owner.observation_source().unwrap();
 super::super::tick(app.world_mut());
 let player=&app.world().resource::<Players>().players["intro"];let image=player.image.clone();let audio=player.audio.as_ref().unwrap().clone();let audio_entity=player.audio_entity;
 assert!(audio.0.lock().unwrap().running);let mut source=audio.decoder();
 deliver_to(&mut app,&audio,&mut source,&pcm,10_003);assert_eq!(audio.0.lock().unwrap().delivered_sample_offset(),Some(10_003));
 app.world_mut().get_mut::<Window>(window).unwrap().focused=false;
 assert!(app.world().resource::<DebugBootstrap>().owner.accepts_video_clock_source(&clock),"The Core witness alone does not grant physical delivery after focus loss");
 assert!(!crate::scene_publication::video_clock_allowed(app.world(),&view));super::super::tick(app.world_mut());
 assert!(!audio.0.lock().unwrap().running);
 for _ in 0..8{assert_eq!(source.next(),Some(0.0));}
 assert_eq!(audio.0.lock().unwrap().delivered_sample_offset(),Some(10_003));
 // Focus alone cannot override the actual debugger host mode.
 app.world_mut().get_mut::<Window>(window).unwrap().focused=true;
 app.world_mut().resource_mut::<DebugBootstrap>().mode=None;
 assert!(!crate::scene_publication::video_clock_allowed(app.world(),&view));super::super::tick(app.world_mut());
 for _ in 0..8{assert_eq!(source.next(),Some(0.0));}
 assert_eq!(audio.0.lock().unwrap().delivered_sample_offset(),Some(10_003));
 // Re-admit both actual owner observation and actual World Continue policy.
 app.world_mut().resource_mut::<DebugBootstrap>().mode=Some(rvn_core::editor_debugger::StepKind::Continue);
 let current=app.world().resource::<DebugBootstrap>().owner.video_views().unwrap()[0].clone();
 let admitted=app.world().resource::<DebugBootstrap>().owner.video_clock_source(&current).unwrap().unwrap();
 assert!(app.world().resource::<DebugBootstrap>().owner.accepts_video_clock_source(&admitted));
 assert!(crate::scene_publication::video_clock_allowed(app.world(),&current));super::super::tick(app.world_mut());
 let player=&app.world().resource::<Players>().players["intro"];assert_eq!(player.image,image);assert_eq!(player.audio_entity,audio_entity);assert!(Arc::ptr_eq(&audio.0,&player.audio.as_ref().unwrap().0));
 assert!(audio.0.lock().unwrap().running);
 assert_eq!(source.next().unwrap().to_bits(),pcm[10_003].to_bits(),"The real pending right must resume immediately without inserted silence");
 assert_eq!(audio.0.lock().unwrap().delivered_sample_offset(),Some(10_004));
 assert_eq!(state(app.world()),state_before);assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&observation));
 assert!(!app.world().resource::<DebugBootstrap>().owner.state().vars.contains_key("after_wait"));
 assert!(!app.world().contains_resource::<VnEngine>());
}

#[test]
fn actual_first_driver_closes_cached_right_when_native_tick_is_not_scheduled(){
 let(root,mut app,window)=actual_wait_world();let pcm=actual_pcm(&root);super::super::tick(app.world_mut());
 let audio=app.world().resource::<Players>().players["intro"].audio.as_ref().unwrap().clone();let mut reader=audio.decoder();deliver_to(&mut app,&audio,&mut reader,&pcm,10_003);
 let before=state(app.world());let observed=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
 app.world_mut().get_mut::<Window>(window).unwrap().focused=false;assert!(!crate::scene_publication::video_clock_allowed(app.world(),&app.world().resource::<DebugBootstrap>().owner.video_views().unwrap()[0]));
 // The actual First driver must close even if the Native run condition prevents tick.
 crate::editor_debugger::drain(app.world_mut());assert!(!audio.0.lock().unwrap().running);
 for _ in 0..8{assert_eq!(reader.next(),Some(0.0));}assert_eq!(audio.0.lock().unwrap().delivered_sample_offset(),Some(10_003));
 assert_eq!(state(app.world()),before);assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&observed));
 app.world_mut().get_mut::<Window>(window).unwrap().focused=true;super::super::tick(app.world_mut());assert_eq!(reader.next().unwrap().to_bits(),pcm[10_003].to_bits());
}
