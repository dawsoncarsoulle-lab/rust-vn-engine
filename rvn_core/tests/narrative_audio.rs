use rvn_core::audio_playback::{self,AudioAdmission,AudioClockRoute,AudioFamily,AudioFeedback,AudioPlayback,AudioReport};
use rvn_core::renderer::SceneRenderBatch;
use rvn_core::scene_tasks::TaskOwner;
use rvn_core::{Engine,Interaction,Renderer,SpriteState};
use rvn_parser::{Hotspot,Position,Transition};

#[derive(Default)]
struct Host { batches:Vec<SceneRenderBatch>,reject:bool,legacy_audio:Vec<String> }
impl Renderer for Host {
    fn validate_scene_batch(&self,batch:&SceneRenderBatch)->Result<(),String> {
        audio_playback::describe(&batch.state,batch.runtime_epoch)?;
        if self.reject {Err("Audio batch rejected before publication".into())} else {Ok(())}
    }
    fn publish_scene_batch(&mut self,batch:SceneRenderBatch){self.batches.push(batch);}
    fn set_background(&mut self,_:&str,_:&Transition){}
    fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
    fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
    fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
    fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
    fn show_choice(&mut self,_:&[String])->usize{0}
    fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{0}
    fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
    fn music_stop(&mut self,_:&Transition){}
    fn music_set_volume(&mut self,_:f32){}
    fn sfx_play(&mut self,file:&str,_:&Transition){self.legacy_audio.push(format!("sfx:{file}"));}
    fn sfx_stop(&mut self,file:&str,_:&Transition){self.legacy_audio.push(format!("stop:{file}"));}
    fn voice_play(&mut self,file:&str){self.legacy_audio.push(format!("voice:{file}"));}
    fn voice_stop(&mut self){self.legacy_audio.push("voice-stop".into());}
}
fn new_game(source:&str)->Engine<Host>{Engine::from_program(rvn_parser::parse_program(source).unwrap(),Host::default(),32).unwrap()}
fn line(game:&mut Engine<Host>)->String{match game.step_until_interaction().unwrap().unwrap(){Interaction::Dialogue{text,..}=>text,other=>panic!("Expected dialogue, got {other:?}")}}
fn next(game:&mut Engine<Host>)->String{game.advance_dialogue().unwrap();line(game)}
fn json(game:&Engine<Host>)->serde_json::Value{serde_json::json!({"state":&game.state,"runtime":game.runtime_epoch(),"interface":game.interface_epoch(),"history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),"batches":game.renderer.batches.len(),"legacy":&game.renderer.legacy_audio})}
fn source()->&'static str{r#"
init { sfx.play("common.ogg") }
scene label P
sfx.play("same.ogg") with fade(1000)
sfx.play("same.ogg")
voice "hello"
"parent"
call C
"returned"
jump finish
scene label C
sfx.play("same.ogg")
voice "hello"
"child"
return
label finish
"#}

#[test]
fn same_file_playbacks_have_distinct_registry_identities_and_no_legacy_queue() {
    let mut game=new_game(source());assert_eq!(line(&mut game),"parent");
    let views=game.audio_playback_views().unwrap();assert_eq!(views.len(),4);
    let same:Vec<_>=views.iter().filter(|view|view.playback.file=="sfx/same.ogg").collect();assert_eq!(same.len(),2);
    assert_ne!(same[0].token.target,same[1].token.target);assert_ne!(same[0].token.task.key,same[1].token.task.key);
    assert_eq!(views.iter().filter(|view|view.token.task.key.owner==TaskOwner::Game).count(),1);
    assert!(game.renderer.legacy_audio.is_empty());
    let last=game.renderer.batches.last().unwrap();assert_eq!(audio_playback::describe(&last.state,last.runtime_epoch).unwrap(),views);
}
#[test]
fn jump_before_consumer_drain_revokes_scene_audio_and_retains_game_init() {
    let mut game=new_game("init{sfx.play(\"init.ogg\")}\nscene label A\nsfx.play(\"abandoned.ogg\")\nvoice \"abandoned\"\njump B\nscene label B\n\"b\"");
    assert_eq!(line(&mut game),"b");let views=game.audio_playback_views().unwrap();assert_eq!(views.len(),1);assert_eq!(views[0].playback.file,"sfx/init.ogg");assert_eq!(views[0].token.task.key.owner,TaskOwner::Game);
    assert!(game.renderer.legacy_audio.is_empty());
    let batch=game.renderer.batches.last().unwrap();assert_eq!(audio_playback::describe(&batch.state,batch.runtime_epoch).unwrap(),views);
}
#[test]
fn call_parks_scene_readers_return_renews_authority_without_replaying_payload() {
    let mut game=new_game(source());line(&mut game);let parent=game.audio_playback_views().unwrap();assert_eq!(next(&mut game),"child");
    let child=game.audio_playback_views().unwrap();assert_eq!(child.len(),6);
    for old in parent.iter().filter(|view|view.token.task.key.owner!=TaskOwner::Game) {
        let parked=child.iter().find(|view|view.token.target==old.token.target).unwrap();assert!(parked.parked);assert_eq!(parked.playback,old.playback);
        assert_eq!(game.audio_report_admission(game.instance_id(),&old.token),AudioAdmission::Stale);assert_eq!(game.audio_report_admission(game.instance_id(),&parked.token),AudioAdmission::Deferred);
    }
    assert_eq!(next(&mut game),"returned");let resumed=game.audio_playback_views().unwrap();assert_eq!(resumed.len(),4);
    for old in parent.iter().filter(|view|view.token.task.key.owner!=TaskOwner::Game) {
        let current=resumed.iter().find(|view|view.token.target==old.token.target).unwrap();assert!(!current.parked);assert_eq!(current.playback,old.playback);assert_ne!(current.token.task.revision,old.token.task.revision);
    }
}
#[test]
fn sfx_fade_uses_core_clock_once_and_parked_scene_clock_is_stable() {
    let mut game=new_game(source());line(&mut game);let before=game.renderer.batches.len();
    game.tick_game_frame(0.25,rvn_core::GameFrameReports::default()).unwrap();assert_eq!(game.renderer.batches.len(),before+1);
    let fade=game.audio_playback_views().unwrap().into_iter().find(|view|view.playback.envelope.is_some()).unwrap();assert_eq!(fade.playback.envelope.as_ref().unwrap().clock.elapsed,0.25);assert_eq!(fade.playback.gain(),0.25);
    next(&mut game);game.tick_game_frame(0.25,rvn_core::GameFrameReports::default()).unwrap();
    let parked=game.audio_playback_views().unwrap().into_iter().find(|view|view.token.target==fade.token.target).unwrap();assert!(parked.parked);assert_eq!(parked.playback,fade.playback);
    next(&mut game);game.tick_game_frame(0.75,rvn_core::GameFrameReports::default()).unwrap();
    let finished=game.audio_playback_views().unwrap().into_iter().find(|view|view.token.target==fade.token.target).unwrap();assert!(finished.playback.envelope.is_none());assert_eq!(finished.playback.gain(),1.0);
}
#[test]
fn story_stop_includes_parked_and_game_reads_but_preserves_host_and_repeated_fade() {
    let mut game=new_game(source());line(&mut game);
    let epoch=game.runtime_epoch();audio_playback::register(&mut game.state,epoch,TaskOwner::Host{role:rvn_ui::PageRole::Pause.id().into()},AudioPlayback::new(AudioFamily::Sfx,"sfx/same.ogg".into(),0.0).unwrap()).unwrap();
    next(&mut game);let host=game.audio_playback_views().unwrap().into_iter().find(|view|matches!(view.token.task.key.owner,TaskOwner::Host{..})).unwrap();
    assert_eq!(audio_playback::stop_story(&mut game.state,AudioFamily::Sfx,Some("sfx/same.ogg"),1.0).unwrap(),3);
    let before=game.state.clone();assert_eq!(audio_playback::stop_story(&mut game.state,AudioFamily::Sfx,Some("sfx/same.ogg"),1.0).unwrap(),0);assert_eq!(serde_json::to_value(&game.state).unwrap(),serde_json::to_value(&before).unwrap());
    assert_eq!(game.audio_playback_views().unwrap().into_iter().find(|view|view.token.target==host.token.target).unwrap(),host);
    assert_eq!(audio_playback::stop_story(&mut game.state,AudioFamily::Voice,None,0.0).unwrap(),2);
    assert_eq!(audio_playback::stop_story(&mut game.state,AudioFamily::Sfx,Some("sfx/same.ogg"),0.0).unwrap(),3);
    assert!(game.audio_playback_views().unwrap().iter().all(|view|view.playback.file!="sfx/same.ogg" || matches!(view.token.task.key.owner,TaskOwner::Host{..})));

}
#[test]
fn audio_cap_invalid_payload_and_revision_exhaustion_refuse_without_allocation() {
    let mut game=new_game("scene label P\n\"p\"");line(&mut game);let epoch=game.runtime_epoch();
    let before=json(&game);let invalid=AudioPlayback{family:AudioFamily::Voice,file:"bad".into(),envelope:None,stopping:true,checkpoint:None};assert!(audio_playback::register(&mut game.state,epoch,TaskOwner::Game,invalid).is_err());assert_eq!(json(&game),before);
    for _ in 0..64{audio_playback::register(&mut game.state,epoch,TaskOwner::Game,AudioPlayback::new(AudioFamily::Sfx,"same.ogg".into(),0.0).unwrap()).unwrap();}
    let before=json(&game);assert!(audio_playback::register(&mut game.state,epoch,TaskOwner::Game,AudioPlayback::new(AudioFamily::Sfx,"same.ogg".into(),0.0).unwrap()).is_err());assert_eq!(json(&game),before);
    game.state.scene_execution.as_mut().unwrap().resources.next_revision=u64::MAX;
    let before=json(&game);assert!(audio_playback::stop_story(&mut game.state,AudioFamily::Sfx,None,1.0).is_err());assert_eq!(json(&game),before);
}
#[test]
fn stale_error_payload_is_neutral_and_current_error_refuses_entire_frame() {
    let mut game=new_game(source());line(&mut game);let old=game.audio_playback_views().unwrap().into_iter().find(|view|view.token.task.key.owner!=TaskOwner::Game).unwrap();next(&mut game);
    let before=json(&game);let (_,receipt)=game.tick_game_frame_with_audio(0.0,rvn_core::GameFrameReports::default(),vec![AudioReport{instance_id:game.instance_id(),token:old.token,feedback:AudioFeedback::Error("\0".into())}],true).unwrap();assert_eq!(receipt[0].disposition,rvn_core::engine::AudioReportDisposition::RejectedStale);assert_eq!(json(&game),before);
    let current=game.audio_playback_views().unwrap().into_iter().find(|view|!view.parked &&view.token.task.key.owner!=TaskOwner::Game).unwrap();let before=json(&game);
    assert!(game.tick_game_frame_with_audio(1.0,rvn_core::GameFrameReports::default(),vec![AudioReport{instance_id:game.instance_id(),token:current.token,feedback:AudioFeedback::Error("decoder failed".into())}],true).is_err());assert_eq!(json(&game),before);
}
#[test]
fn audio_end_ordinal_receipts_defer_blocked_owner_and_publish_once_after_admission() {
    let mut game=new_game(source());line(&mut game);let token=game.audio_playback_views().unwrap()[0].token.clone();let reports=vec![AudioReport{instance_id:game.instance_id(),token:token.clone(),feedback:AudioFeedback::End},AudioReport{instance_id:game.instance_id(),token:token.clone(),feedback:AudioFeedback::End}];
    let before=json(&game);let (_,receipt)=game.tick_game_frame_with_audio(1.0,rvn_core::GameFrameReports::default(),reports.clone(),false).unwrap();assert_eq!(receipt.iter().map(|receipt|receipt.ordinal).collect::<Vec<_>>(),vec![0,1]);assert!(receipt.iter().all(|receipt|receipt.disposition==rvn_core::engine::AudioReportDisposition::Deferred));assert_eq!(json(&game),before);
    let before_batches=game.renderer.batches.len();let (outcome,receipt)=game.tick_game_frame_with_audio(0.0,rvn_core::GameFrameReports::default(),reports,true).unwrap();assert!(outcome.published);assert_eq!(game.renderer.batches.len(),before_batches+1);assert!(receipt.iter().all(|receipt|receipt.disposition==rvn_core::engine::AudioReportDisposition::Consumed));assert!(game.audio_playback_views().unwrap().iter().all(|view|view.token.target!=token.target));
}
#[test]
fn renderer_refusal_preserves_audio_registration_pc_history_and_epochs() {
    let mut game=new_game("scene label P\nsfx.play(\"click.ogg\")\n\"p\"");game.enter_label("P").unwrap();game.renderer.reject=true;let before=json(&game);assert!(game.step_silent().is_err());assert_eq!(json(&game),before);
}
#[test]
fn legacy_sound_voice_hooks_remain_immediate_with_unchanged_paths() {
    let mut game=Engine::new(rvn_parser::parse("sfx.play(\"sfx/click.ogg\")\nvoice \"hello\"\nvoice stop\n\"line\"").unwrap(),Host::default(),32).unwrap();assert_eq!(line(&mut game),"line");assert_eq!(game.renderer.legacy_audio,vec!["sfx:sfx/click.ogg","voice:hello","voice-stop"]);assert!(game.audio_playback_views().unwrap().is_empty());
}
#[test]
fn old_empty_resource_shapes_remain_readable_and_active_audio_checkpoint_is_explicitly_open() {
    let mut game=new_game("scene label P\n\"p\"");line(&mut game);let wire=serde_json::to_value(&game.state).unwrap();assert!(!wire.to_string().contains("\"audio\""));let old:rvn_core::GameState=serde_json::from_value(wire).unwrap();rvn_core::scene_resources::validate_current_resource_state(&old).unwrap();audio_playback::require_portable_checkpoint(&old).unwrap();
    let mut game=new_game(source());line(&mut game);let before=json(&game);assert!(audio_playback::require_portable_checkpoint(&game.state).unwrap_err().contains("trustworthy decoder checkpoint"));assert_eq!(json(&game),before);
    assert!(rvn_core::save::SaveData::try_from_state(&game.state,1,"checkpoint".into(),"main.rvn".into()).is_err());assert_eq!(json(&game),before);
    let unsafe_wire=rvn_core::save::SaveData::from_state(&game.state,1,"raw negative DTO".into(),"main.rvn".into());
    assert!(game.load_data(unsafe_wire).is_err());assert_eq!(json(&game),before);
    audio_playback::stop_story(&mut game.state,AudioFamily::Sfx,None,0.0).unwrap();audio_playback::stop_story(&mut game.state,AudioFamily::Voice,None,0.0).unwrap();
    let clean=rvn_core::save::SaveData::try_from_state(&game.state,1,"completed".into(),"main.rvn".into()).unwrap();assert_eq!(clean.format_version,10);
    assert!(!serde_json::to_string(&clean).unwrap().contains("sfx_playback"));assert!(!serde_json::to_string(&clean).unwrap().contains("voice_playback"));

}
#[test]
fn host_audio_route_preserves_story_pc_stack_rng_and_timer() {
    let mut game=new_game("scene label P\ntimer 10 => jump P\n\"p\"");line(&mut game);let epoch=game.runtime_epoch();audio_playback::register(&mut game.state,epoch,TaskOwner::Host{role:rvn_ui::PageRole::Pause.id().into()},AudioPlayback::new(AudioFamily::Sfx,"host.ogg".into(),1.0).unwrap()).unwrap();
    let before=serde_json::json!({"pc":game.state.pc,"stack":&game.state.call_stack,"random":&game.state.random,"tasks":&game.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().tasks});
    let (outcome,_)=game.tick_host_frame_with_audio(0.25,rvn_core::engine::HostFrameReports::default(),Vec::new(),true).unwrap();assert!(outcome.frame.published);let after=serde_json::json!({"pc":game.state.pc,"stack":&game.state.call_stack,"random":&game.state.random,"tasks":&game.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().tasks});assert_eq!(before,after);
    let audio=game.audio_playback_views().unwrap();assert_eq!(audio[0].playback.gain(),0.25);
    let mut candidate=game.state.clone();audio_playback::advance_clocks(&mut candidate,epoch,AudioClockRoute::Story,1.0).unwrap();assert_eq!(serde_json::to_value(&candidate).unwrap(),serde_json::to_value(&game.state).unwrap());
}

#[test]
fn independent_engine_epoch_zero_does_not_admit_another_instances_reader_or_terminal() {
    let source="scene label A\nsfx.play(\"same.ogg\")\n\"a\"";
    let mut old=new_game(source);line(&mut old);
    let mut game=new_game(source);line(&mut game);
    let old_view=old.audio_playback_views().unwrap().remove(0);let new_view=game.audio_playback_views().unwrap().remove(0);
    assert_eq!(old.runtime_epoch(),game.runtime_epoch());assert_eq!(old_view.token,new_view.token);
    assert_ne!(old.instance_id(),game.instance_id());
    let before=json(&game);
    let (_,receipts)=game.tick_game_frame_with_audio(1.0,rvn_core::GameFrameReports::default(),vec![
        AudioReport{instance_id:old.instance_id(),token:old_view.token.clone(),feedback:AudioFeedback::Error("\0".into())},
        AudioReport{instance_id:old.instance_id(),token:old_view.token,feedback:AudioFeedback::End},
    ],true).unwrap();
    assert!(receipts.iter().all(|receipt|receipt.disposition==rvn_core::engine::AudioReportDisposition::RejectedStale));
    assert_eq!(json(&game),before);assert_eq!(game.audio_playback_views().unwrap(),vec![new_view]);
    let wire=serde_json::to_string(&game.state).unwrap();assert!(!wire.contains("instance_id"));
}
#[test]
fn decoder_error_at_fade_stop_boundary_refuses_before_exact_due_cleanup() {
    let mut game=new_game("scene label A\nsfx.play(\"same.ogg\")\n\"a\"");line(&mut game);
    audio_playback::stop_story(&mut game.state,AudioFamily::Sfx,None,1.0).unwrap();
    let token=game.audio_playback_views().unwrap()[0].token.clone();let instance_id=game.instance_id();let before=json(&game);
    assert!(game.tick_game_frame_with_audio(1.0,rvn_core::GameFrameReports::default(),vec![AudioReport{instance_id,token,feedback:AudioFeedback::Error("decode failure at end".into())}],true).is_err());
    assert_eq!(json(&game),before);assert_eq!(game.audio_playback_views().unwrap()[0].playback.envelope.as_ref().unwrap().clock.elapsed,0.0);
    game.tick_game_frame(1.0,rvn_core::GameFrameReports::default()).unwrap();assert!(game.audio_playback_views().unwrap().is_empty());
}

#[cfg(not(target_arch="wasm32"))]
#[test]
fn actual_writer_refusal_preserves_existing_slot_bytes_and_omits_volatile_host_audio() {
    let mut game=new_game("scene label A\n\"before\"\nsfx.play(\"same.ogg\")\n\"after\"");
    assert_eq!(line(&mut game),"before");
    let epoch=game.runtime_epoch();
    audio_playback::register(&mut game.state,epoch,TaskOwner::Host{role:rvn_ui::PageRole::Pause.id().into()},AudioPlayback::new(AudioFamily::Sfx,"host.ogg".into(),0.0).unwrap()).unwrap();
    let directory=std::env::temp_dir().join(format!("rvn-owned-audio-writer-{}-{}",std::process::id(),game.instance_id()));
    assert!(!directory.exists());
    let manager=rvn_core::save::SaveManager::new(&directory,4).unwrap();
    game.save(&manager,1,"prior accepted checkpoint".into(),"main.rvn".into()).unwrap();
    let path=directory.join("slot_01.json");let original=std::fs::read(&path).unwrap();
    let original_json:serde_json::Value=serde_json::from_slice(&original).unwrap();
    assert_eq!(original_json["format_version"],10);
    assert!(!std::str::from_utf8(&original).unwrap().contains("sfx_playback"));
    assert_eq!(next(&mut game),"after");let before=json(&game);
    let error=game.save(&manager,1,"must not overwrite".into(),"main.rvn".into()).unwrap_err();
    assert!(error.to_string().contains("trustworthy decoder checkpoint"));
    assert_eq!(std::fs::read(&path).unwrap(),original);assert_eq!(json(&game),before);
    assert_eq!(manager.load(1).unwrap().format_version,10);
    audio_playback::stop_story(&mut game.state,AudioFamily::Sfx,None,0.0).unwrap();
    assert_eq!(game.audio_playback_views().unwrap().len(),1);
    assert!(matches!(game.audio_playback_views().unwrap()[0].token.task.key.owner,TaskOwner::Host{..}));
    game.save(&manager,1,"after exact cleanup".into(),"main.rvn".into()).unwrap();
    let clean=std::fs::read(&path).unwrap();
    assert!(!std::str::from_utf8(&clean).unwrap().contains("sfx_playback"));
    std::fs::remove_file(&path).unwrap();std::fs::remove_dir(&directory).unwrap();
}
