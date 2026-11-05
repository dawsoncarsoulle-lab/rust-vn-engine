//! PREPARED / NOT COMPILED. Actual First driver, World installer and Native
//! readers. Queue reports are explicit host inputs, not a device/EOF claim.
//! No AudioPlugin/RenderDevice or gameplay owner is installed during Init.
use super::*;
use bevy::audio::Decodable;
use crate::{debugger_startup::DebugBootstrap, resources::VnState};
use std::{path::PathBuf, time::{Duration,Instant}};

const SCRIPT:&str=r#"handler boot_end(event) {set completions=completions+1}
init {
 set completions=0
 set sampled=random(1,9)
 video.play("intro",video_clip("clip.webm",{"mask":"mask.webm","on_end":"boot_end"}))
 video.wait("intro")
 set after_wait=random(1,9)
}
scene label start
"Ready"
"#;

fn fixture()->(PathBuf,App,Entity){
    let(root,mut app)=super::startup_contract_tests::fixture_source(SCRIPT);
    app.init_resource::<crate::scene_publication::ScenePresentation>()
        .init_resource::<crate::scene_publication::VideoFeedbackQueue>()
        .init_resource::<crate::resources::VnRenderState>()
        .init_resource::<crate::accessibility::Accessibility>()
        .init_resource::<crate::systems::settings_menu::Settings>()
        .init_resource::<crate::editor_preview::PreviewState>()
        .insert_resource(State::new(VnState::Waiting))
        .init_resource::<NextState<VnState>>()
        .add_systems(First,crate::editor_debugger::drain)
        .add_systems(Update,crate::scene_publication::startup_feedback_schedule_for_test);
    app.world_mut().resource_mut::<crate::systems::settings_menu::Settings>().language="en".into();
    let window=app.world_mut().spawn((Window{focused:true,..default()},bevy::window::PrimaryWindow)).id();
    crate::debugger_startup::driver::run(app.world_mut(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
    until(&mut app,|world|world.resource::<DebugBootstrap>().owner.waiting_for_video());
    assert!(!app.world().contains_resource::<VnEngine>());
    assert!(!app.world().resource::<DebugBootstrap>().publication_pending);
    assert!(app.world().resource::<Players>().players.contains_key("intro"));
    assert!(!app.world().resource::<DebugBootstrap>().owner.state().vars.contains_key("after_wait"));
    (root,app,window)
}
fn until(app:&mut App,done:impl Fn(&World)->bool){
    let end=Instant::now()+Duration::from_secs(20);
    loop{app.update();if let Some(owner)=app.world().get_resource::<DebugBootstrap>(){assert!(owner.failed.is_none(),"Actual driver: {:?}",owner.failed);}
        if done(app.world()){return;}assert!(Instant::now()<end,"Actual First/Update Startup schedule timed out");std::thread::sleep(Duration::from_millis(2));}
}
fn view(app:&App)->VideoView{app.world().resource::<DebugBootstrap>().owner.video_views().unwrap()[0].clone()}
fn nonces(app:&App)->Vec<u64>{crate::scene_publication::video_nonces_for_test(app.world())}
fn queue(app:&mut App,view:&VideoView,feedback:Feedback){assert!(crate::scene_publication::queue_video_feedback(app.world_mut(),view,feedback));}
fn state(app:&App)->serde_json::Value{serde_json::to_value(app.world().resource::<DebugBootstrap>().owner.state()).unwrap()}
fn pcm(root:&std::path::Path)->Vec<f32>{
    let worker=rvn_media::transport::Worker::spawn_proven(root.join("assets/clip.webm"),None).unwrap();
    let mut result=Vec::new();let end=Instant::now()+Duration::from_secs(10);
    loop{assert!(Instant::now()<end);match worker.poll().unwrap(){
        Some(rvn_media::transport::Packet::Data(rvn_media::decoder::Chunk::Audio{samples,..}))=>{assert!(result.len()+samples.len()<=1_000_000);result.extend(samples);},
        Some(rvn_media::transport::Packet::Data(rvn_media::decoder::Chunk::End))=>break,
        Some(rvn_media::transport::Packet::Error(error))=>panic!("{error}"),_=>std::thread::yield_now()}}
    assert!(result.len()>10004);assert_ne!(result[10003].to_bits(),0.0f32.to_bits());result
}
fn deliver(app:&mut App,audio:&VideoAudio,reader:&mut crate::video_audio::Samples,expected:&[f32],target:u64){
    let end=Instant::now()+Duration::from_secs(10);
    while audio.0.lock().unwrap().delivered_sample_offset().unwrap()<target{
        assert!(Instant::now()<end);let before=audio.0.lock().unwrap().delivered_sample_offset().unwrap()as usize;
        let value=reader.next().unwrap();let after=audio.0.lock().unwrap().delivered_sample_offset().unwrap()as usize;
        if after==before+1{assert_eq!(value.to_bits(),expected[before].to_bits());}else{assert_eq!(after,before);assert_eq!(value,0.0);std::thread::sleep(Duration::from_millis(1));}
        if after%128==0||after==before{let mut players=app.world_mut().remove_resource::<Players>().unwrap();for player in players.players.values_mut(){super::pump(app.world_mut(),player).unwrap();player.position=audio.0.lock().unwrap().position;super::draw(app.world_mut(),player);}app.world_mut().insert_resource(players);}
    }
}

#[test]
fn startup_scheduled_driver_position_keeps_real_pcm_right_image_and_focus_gate(){
    let(root,mut app,window)=fixture();let expected=pcm(&root);super::tick(app.world_mut());app.update();
    let player=&app.world().resource::<Players>().players["intro"];let image=player.image.clone();let entity=player.audio_entity;let audio=player.audio.as_ref().unwrap().clone();let mut reader=audio.decoder();
    deliver(&mut app,&audio,&mut reader,&expected,10003);
    let before=state(&app);let current=view(&app);queue(&mut app,&current,Feedback::Position{seconds:0.1,duration:2.0});app.update();
    let player=&app.world().resource::<Players>().players["intro"];assert_eq!(player.image,image);assert_eq!(player.audio_entity,entity);assert!(Arc::ptr_eq(&audio.0,&player.audio.as_ref().unwrap().0));assert_eq!(audio.0.lock().unwrap().delivered_sample_offset(),Some(10003));assert!(nonces(&app).is_empty());
    assert_eq!(state(&app)["random"],before["random"]);assert_eq!(state(&app)["vars"],before["vars"]);
    app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();for _ in 0..4{assert_eq!(reader.next(),Some(0.0));}assert_eq!(audio.0.lock().unwrap().delivered_sample_offset(),Some(10003));
    app.world_mut().get_mut::<Window>(window).unwrap().focused=true;super::tick(app.world_mut());
    assert_eq!(reader.next().unwrap().to_bits(),expected[10003].to_bits());assert_eq!(audio.0.lock().unwrap().delivered_sample_offset(),Some(10004));
}

#[test]
fn startup_scheduled_driver_drains_129_positions_before_terminal_with_original_nonces_and_single_callback(){
    let(_,mut app,_)=fixture();let current=view(&app);let random=app.world().resource::<DebugBootstrap>().owner.state().random;
    for _ in 0..129{queue(&mut app,&current,Feedback::Position{seconds:0.1,duration:2.0});}
    queue(&mut app,&current,Feedback::End);queue(&mut app,&current,Feedback::End);
    let original=nonces(&app);let first_terminal=original[129];app.update();
    let remaining=nonces(&app);assert!(remaining.len()>2&&remaining.len()<131,"The real 128/64KiB contract must chunk, never discard the large queue");assert!(remaining.iter().all(|id|original.contains(id)));assert!(remaining.contains(&first_terminal));
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.state().vars["completions"],rvn_parser::Value::Int(0));
    until(&mut app,|world|crate::debugger_startup::driver::terminal_pending(world));
    assert!(nonces(&app).contains(&first_terminal),"Beginning a callback cannot acknowledge its terminal");
    until(&mut app,|world|!world.contains_resource::<DebugBootstrap>());
    let engine=&app.world().resource::<VnEngine>().0;assert_eq!(engine.state.vars["completions"],rvn_parser::Value::Int(1));let mut expected=random;let after=expected.integer(1,9).unwrap();assert_eq!(engine.state.random,expected);assert_eq!(engine.state.vars["after_wait"],rvn_parser::Value::Int(after));
    assert!(!nonces(&app).contains(&first_terminal));
}

#[test]
fn startup_scheduled_driver_invalid_position_refuses_entire_selected_chunk_without_ack_or_native_mutation(){
    let(_,mut app,_)=fixture();let current=view(&app);let before=state(&app);let image=app.world().resource::<Players>().players["intro"].image.clone();let audio=app.world().resource::<Players>().players["intro"].audio.as_ref().unwrap().clone();
    queue(&mut app,&current,Feedback::Position{seconds:0.1,duration:2.0});queue(&mut app,&current,Feedback::Position{seconds:f64::NAN,duration:2.0});let ids=nonces(&app);app.update();
    assert!(app.world().resource::<DebugBootstrap>().failed.is_some());assert_eq!(state(&app),before);assert_eq!(nonces(&app),ids);let player=&app.world().resource::<Players>().players["intro"];assert_eq!(player.view,current);assert_eq!(player.image,image);assert!(Arc::ptr_eq(&audio.0,&player.audio.as_ref().unwrap().0));assert!(!audio.0.lock().unwrap().revoked);
}

#[test]
fn startup_scheduled_driver_rejects_stale_before_nan_and_adopts_actual_member_only(){
    let(_,mut app,_)=fixture();let current=view(&app);let mut stale=current.clone();stale.epoch=stale.epoch.checked_add(1).unwrap();
    queue(&mut app,&stale,Feedback::Position{seconds:f64::NAN,duration:f64::NAN});queue(&mut app,&current,Feedback::Position{seconds:0.1,duration:2.0});let random=app.world().resource::<DebugBootstrap>().owner.state().random;app.update();
    assert!(app.world().resource::<DebugBootstrap>().failed.is_none());assert!(nonces(&app).is_empty());assert_eq!(view(&app).track.position,0.1);assert_eq!(app.world().resource::<Players>().players["intro"].view,view(&app));assert_eq!(app.world().resource::<DebugBootstrap>().owner.state().random,random);
}

#[test]
fn startup_scheduled_driver_deferred_blocked_position_cannot_starve_genuine_resume(){
    let(_,mut app,_)=fixture();let current=view(&app);queue(&mut app,&current,Feedback::AutoplayBlocked("actual host policy input".into()));
    until(&mut app,|world|world.resource::<DebugBootstrap>().owner.waiting_for_video()&&world.resource::<DebugBootstrap>().owner.video_views().unwrap()[0].track.playback==Playback::Blocked&&!crate::debugger_startup::driver::terminal_pending(world));
    let blocked=view(&app);queue(&mut app,&blocked,Feedback::Position{seconds:0.1,duration:2.0});assert!(crate::scene_publication::queue_video_resume(app.world_mut(),&blocked));let ids=nonces(&app);let resume=*ids.last().unwrap();
    until(&mut app,|world|world.resource::<DebugBootstrap>().owner.waiting_for_video()&&world.resource::<DebugBootstrap>().owner.video_views().unwrap()[0].track.playback==Playback::Playing&&!crate::debugger_startup::driver::terminal_pending(world));
    assert!(!nonces(&app).contains(&resume));assert!(!app.world().resource::<DebugBootstrap>().owner.state().vars.contains_key("after_wait"));
}
