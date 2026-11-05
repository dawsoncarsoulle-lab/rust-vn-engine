//! PRIVATE integration contracts. Pure registry validation, not decoder/device proof.
use rvn_core::audio_checkpoint::{checkpointed_state, AudioCheckpoint, AudioCheckpointRow, AudioCheckpointSnapshot};
use rvn_core::audio_playback::{AudioFamily, AudioPlayback};
use rvn_core::renderer::SceneRenderBatch;
use rvn_core::scene_tasks::TaskOwner;
use rvn_core::{Engine, Interaction, Renderer, SpriteState};
use rvn_parser::{Hotspot, Position, Transition};

#[derive(Default)]
struct Host;
impl Renderer for Host {
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> { Ok(()) }
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
    fn sfx_play(&mut self,_:&str,_:&Transition){}
    fn sfx_stop(&mut self,_:&str,_:&Transition){}
    fn voice_play(&mut self,_:&str){}
    fn voice_stop(&mut self){}
}
fn source()->&'static str {"init{sfx.play(\"game.ogg\")}\nscene label P\nsfx.play(\"same.ogg\") with fade(1000)\n\"parent\"\ncall C\n\"returned\"\njump finish\nscene label C\nsfx.play(\"same.ogg\")\n\"child\"\nreturn\nlabel finish\n"}
fn game()->Engine<Host> {
    let mut game=Engine::from_program(rvn_parser::parse_program(source()).unwrap(),Host,16).unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(Interaction::Dialogue{..})));game
}
fn snapshot(game:&Engine<Host>)->AudioCheckpointSnapshot {
    AudioCheckpointSnapshot {instance_id:game.instance_id(),runtime_epoch:game.runtime_epoch(),rows:game.audio_playback_views().unwrap()
        .into_iter().filter(|view|!matches!(view.token.task.key.owner,TaskOwner::Host{..})).enumerate().map(|(index,view)| AudioCheckpointRow {
            token:view.token,checkpoint:AudioCheckpoint{version:1,asset_sha256:[index as u8;32],pcm_sha256:[19;32],channels:2,sample_rate:48_000,total_samples:8,next_sample:3,next_channel:1,looping:false,loops_completed:0}
        }).collect(),music:Vec::new()}
}
fn exact(game:&Engine<Host>)->serde_json::Value {
    serde_json::json!({"state":game.state,"history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),"runtime":game.runtime_epoch(),"interface":game.interface_epoch(),"instance":game.instance_id()})
}

#[test]
fn fresh_checkpoint_clone_covers_live_parked_and_game_without_mutating_the_engine() {
    let mut game=game();game.tick_game_frame(0.25,Default::default()).unwrap();
    game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
    let epoch=game.runtime_epoch();rvn_core::audio_playback::register(&mut game.state,epoch,TaskOwner::Host{role:rvn_ui::PageRole::Pause.id().into()},AudioPlayback::new(AudioFamily::Sfx,"sfx/host.ogg".into(),0.0).unwrap()).unwrap();
    let before=exact(&game);let snapshot=snapshot(&game);
    let copy=checkpointed_state(&game.state,game.instance_id(),epoch,&snapshot).unwrap();
    assert_eq!(exact(&game),before);assert_eq!(copy.pc,game.state.pc);assert_eq!(copy.call_stack,game.state.call_stack);
    assert_eq!(copy.random,game.state.random);
    let actual=copy.scene_execution.as_ref().unwrap().resources.tasks.iter().filter_map(|task|task.audio.as_ref().map(|audio|(task,audio))).collect::<Vec<_>>();
    assert!(actual.iter().any(|(task,_)|task.parked));assert!(actual.iter().any(|(task,_)|task.key.owner==TaskOwner::Game));
    for (task,audio) in actual {
        if matches!(task.key.owner,TaskOwner::Host{..}) {assert!(audio.checkpoint.is_none());}
        else {let row=snapshot.rows.iter().find(|row|row.token.target==task.target).unwrap();assert_eq!(audio.checkpoint.as_ref(),Some(&row.checkpoint));}
    }
    // Foundation does not silently remove the writer's fresh-capture guard.
    assert!(rvn_core::save::SaveData::try_from_state(&copy,1,"still guarded".into(),"main.rvn".into()).is_err());
}

#[test]
fn checkpoint_refusals_preserve_pc_rng_flags_registry_and_rollback_history() {
    let mut game=game();game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
    let original=snapshot(&game);let before=exact(&game);
    for change in 0..7 {
        let mut bad=original.clone();match change {
            0=>bad.instance_id+=1,1=>bad.runtime_epoch+=1,2=>{bad.rows.pop();},
            3=>bad.rows[1]=bad.rows[0].clone(),4=>bad.rows[0].token.task.revision+=1,
            5=>bad.rows[0].checkpoint.next_channel=0,_=>bad.rows[0].checkpoint.next_sample=9,
        }
        assert!(checkpointed_state(&game.state,game.instance_id(),game.runtime_epoch(),&bad).is_err());assert_eq!(exact(&game),before);
    }
}

#[test]
fn identical_independent_engine_cannot_accept_another_instances_checkpoint_snapshot() {
    let first=game();let second=game();let snapshot=snapshot(&first);
    assert_eq!(first.runtime_epoch(),second.runtime_epoch());assert_eq!(first.audio_playback_views().unwrap(),second.audio_playback_views().unwrap());
    let before=exact(&second);assert!(checkpointed_state(&second.state,second.instance_id(),second.runtime_epoch(),&snapshot).is_err());assert_eq!(exact(&second),before);
}

#[test]
fn phase_and_loop_metadata_are_strict_and_old_audio_wire_without_checkpoint_stays_exact() {
    let game=game();let playback=game.audio_playback_views().unwrap()[0].playback.clone();
    let wire=serde_json::to_string(&playback).unwrap();assert!(!wire.contains("checkpoint"));
    let roundtrip:AudioPlayback=serde_json::from_str(&wire).unwrap();assert_eq!(roundtrip,playback);
    let mut checkpoint=snapshot(&game).rows[0].checkpoint.clone();
    let mut bad=checkpoint.clone();bad.next_channel=2;assert!(bad.validate().is_err());
    bad=checkpoint.clone();bad.total_samples=7;assert!(bad.validate().is_err());
    bad=checkpoint.clone();bad.loops_completed=1;assert!(bad.validate().is_err());
    checkpoint.looping=true;checkpoint.loops_completed=3;checkpoint.validate().unwrap();
    let mut one_shot=playback;one_shot.checkpoint=Some(checkpoint);assert!(one_shot.validate().is_err());
}

fn music_game()->Engine<Host> {
    let mut game=Engine::from_program(rvn_parser::parse_program("scene label P\nmusic.play(\"one.wav\") with fade(1000)\n\"first\"\nmusic.play(\"two.wav\") with fade(1000)\n\"second\"").unwrap(),Host,8).unwrap();
    game.step_until_interaction().unwrap();game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();game
}
fn music_snapshot(game:&Engine<Host>)->AudioCheckpointSnapshot {
    // Core descriptor fixture only. Actual decoded samples are independently
    // exercised by the Bevy Decodable/UniformSourceIterator contracts.
    AudioCheckpointSnapshot{instance_id:game.instance_id(),runtime_epoch:game.runtime_epoch(),rows:Vec::new(),
        music:rvn_core::audio_checkpoint::music_views(&game.state,game.runtime_epoch()).unwrap().into_iter().enumerate().map(|(index,view)|
            rvn_core::audio_checkpoint::MusicCheckpoint{target:view.authority.target,file:view.file,
                checkpoint:AudioCheckpoint{version:1,asset_sha256:[index as u8;32],pcm_sha256:[42;32],channels:2,sample_rate:48_000,total_samples:8,next_sample:3,next_channel:1,looping:true,loops_completed:7}}
        ).collect()}
}

#[test]
fn music_checkpoint_shape_rejects_orphans_duplicates_changed_files_and_save10_without_mutating_the_original() {
    let game=music_game();let observed=music_snapshot(&game);assert_eq!(observed.music.len(),2);let before=exact(&game);
    let copy=checkpointed_state(&game.state,game.instance_id(),game.runtime_epoch(),&observed).unwrap();
    let data=rvn_core::save::SaveData::from_state(&copy,1,"descriptor fixture".into(),"main.rvn".into());
    assert_eq!(data.format_version,11);data.validate_checkpoint_shape().unwrap();
    for change in 0..6 {
        let mut bad=observed.clone();match change {
            0=>{bad.music.pop();},1=>bad.music[1]=bad.music[0].clone(),2=>bad.music[0].target.incarnation+=1,
            3=>bad.music[0].file="music/replaced.wav".into(),4=>bad.music[0].checkpoint.looping=false,
            _=>bad.music[0].checkpoint.next_channel=0,
        }
        assert!(checkpointed_state(&game.state,game.instance_id(),game.runtime_epoch(),&bad).is_err());assert_eq!(exact(&game),before);
    }
    let mut old_reader=data.clone();old_reader.format_version=10;assert!(old_reader.validate_checkpoint_shape().is_err());
    let mut duplicate=data.clone();let rows=&mut duplicate.scene_execution.as_mut().unwrap().resources.music_checkpoints;rows.push(rows[0].clone());
    assert!(duplicate.validate_checkpoint_shape().is_err());
    let mut orphan=data;orphan.scene_execution.as_mut().unwrap().resources.music_checkpoints[0].target.incarnation+=1;
    assert!(orphan.validate_checkpoint_shape().is_err());assert_eq!(exact(&game),before);
}

#[test]
fn due_retired_music_cleanup_purges_only_its_exact_checkpoint_and_keeps_the_current_reader_metadata() {
    let mut game=music_game();let observed=music_snapshot(&game);
    game.state=checkpointed_state(&game.state,game.instance_id(),game.runtime_epoch(),&observed).unwrap();
    let epoch=game.runtime_epoch();let views=rvn_core::audio_checkpoint::music_views(&game.state,epoch).unwrap();
    let retired=views.iter().find(|view|view.authority.outgoing.is_some()).unwrap();let current=views.iter().find(|view|view.authority.outgoing.is_none()).unwrap();
    let token=rvn_core::scene_resources::ResourceToken{task:retired.authority.outgoing.clone().unwrap(),target:retired.authority.target.clone()};
    let unchanged_current=current.checkpoint.clone().unwrap();
    let task=game.state.scene_execution.as_mut().unwrap().resources.tasks.iter_mut().find(|task|task.target==token.target).unwrap();
    let effect=task.effect.as_mut().unwrap();effect.clock.elapsed=effect.clock.duration;
    let completion=rvn_core::scene_resources::complete_due_effect(&mut game.state,&token,epoch).unwrap().unwrap();
    assert_eq!(completion.retired_target,Some(token.target.clone()));
    let rows=&game.state.scene_execution.as_ref().unwrap().resources.music_checkpoints;assert_eq!(rows.len(),1);
    assert_eq!(rows[0].target,current.authority.target);assert_eq!(rows[0].checkpoint,unchanged_current);
    assert!(rvn_core::scene_resources::complete_due_effect(&mut game.state,&token,epoch).unwrap().is_none());
    rvn_core::audio_checkpoint::require_saved_checkpoints(&game.state).unwrap();
}
