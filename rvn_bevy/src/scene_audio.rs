//! Exact one-shot reader cache. Core owns identity, eligibility and SFX clocks.
//! No decoder position is guessed or serialized by this adapter.
use bevy::{asset::LoadState,audio::{AudioSinkPlayback,Volume},prelude::*};
use rvn_core::audio_playback::{self,AudioAdmission,AudioFamily,AudioFeedback,AudioPlaybackView,AudioReport};
use rvn_core::scene_resources::{ResourceToken,TargetIdentity};
use std::collections::{HashMap,HashSet};

#[derive(Component,Clone,Debug,PartialEq,Eq)]
pub(crate) struct OwnedAudio {pub token:ResourceToken}
#[derive(Resource,Default)]
struct AudioPlayers {instance_id:Option<u64>,epoch:Option<u64>,entries:HashMap<TargetIdentity,Reader>}
struct Reader {
    view:AudioPlaybackView,source:Handle<AudioSource>,checkpoint:Option<crate::checkpoint_pcm::CheckpointPcm>,pcm:Option<Handle<crate::checkpoint_pcm::CheckpointPcm>>,entity:Option<Entity>,ever_started:bool,
    terminal:Option<AudioFeedback>,submitted:Option<ResourceToken>,
}
pub(crate) struct PreparedAudio {instance_id:u64,epoch:u64,items:Vec<PreparedItem>}
struct PreparedItem {view:AudioPlaybackView,path:String,gain:f32}

pub(crate) fn source_path(family:AudioFamily,file:&str)->String {
    if family==AudioFamily::Voice && !file.contains('.') {format!("{file}.ogg")} else {file.into()}
}
fn final_gain(world:&World,view:&AudioPlaybackView)->Result<f32,String> {
    let preference=match view.playback.family {
        AudioFamily::Sfx=>world.get_resource::<crate::systems::settings_menu::Settings>().map(|settings|settings.sfx_volume).unwrap_or(1.0),
        AudioFamily::Voice=>1.0,
    };
    if !preference.is_finite() || !(0.0..=1.0).contains(&preference) {return Err("Invalid SFX user gain".into());}
    let logical=view.playback.gain();
    if !logical.is_finite() || !(0.0..=1.0).contains(&logical) {return Err("Invalid logical one-shot gain".into());}
    Ok((f64::from(logical)*f64::from(preference)).clamp(0.0,1.0) as f32)
}
pub(crate) fn prepare(world:&World,state:&rvn_core::GameState,epoch:u64)->Result<PreparedAudio,String> {
    let instance_id=crate::text_runtime_owner::current(world)?.instance;
    let views=audio_playback::describe(state,epoch)?;
    if !views.is_empty() && (world.get_resource::<AssetServer>().is_none() || world.get_resource::<Assets<AudioSource>>().is_none()) {return Err("Owned audio requires the real audio asset runtime".into());}
    let mut items=Vec::with_capacity(views.len());let mut identities=HashSet::new();
    for view in views {
        if !identities.insert(view.token.target.clone()) {return Err("Duplicate exact audio reader identity".into());}
        let path=source_path(view.playback.family,&view.playback.file);
        if path.len()>4096 {return Err("Resolved voice fallback exceeds source path cap".into());}
        if let Some(reader)=world.get_resource::<AudioPlayers>().filter(|players|players.instance_id==Some(instance_id) && players.epoch==Some(epoch)).and_then(|players|players.entries.get(&view.token.target)) {
            if reader.view.token.task.key!=view.token.task.key || reader.view.playback.file!=view.playback.file || reader.view.playback.family!=view.playback.family {return Err("An audio reader cannot adopt another operation or source".into());}
        }
        let gain=final_gain(world,&view)?;
        items.push(PreparedItem{view,path,gain});
    }
    Ok(PreparedAudio{instance_id,epoch,items})
}
fn dispose(world:&mut World,reader:Reader) {
    if let Some(checkpoint)=&reader.checkpoint {let _=checkpoint.stop();}
    if let Some(entity)=reader.entity {if let Some(sink)=world.get::<AudioSink>(entity){sink.stop();}if let Some(entity)=world.get_entity_mut(entity){entity.despawn();}}
}
/// Called only after every publication preflight succeeds. Entity changes and
/// asynchronous asset requests begin after pending/output ownership is taken.
pub(crate) fn install(world:&mut World,plan:PreparedAudio) {
    let mut players=world.remove_resource::<AudioPlayers>().unwrap_or_default();
    if players.instance_id!=Some(plan.instance_id) || players.epoch!=Some(plan.epoch) {
        for (_,reader) in players.entries.drain(){dispose(world,reader);}
        players.epoch=Some(plan.epoch);
        players.instance_id=Some(plan.instance_id);
    }
    if let Some(mut queue)=world.get_resource_mut::<AudioFeedbackQueue>() {
        queue.rows.retain(|row|row.report.instance_id==plan.instance_id && row.report.token.task.runtime_epoch==plan.epoch);
    }
    let wanted:HashSet<_>=plan.items.iter().map(|item|item.view.token.target.clone()).collect();
    let absent:Vec<_>=players.entries.keys().filter(|identity|!wanted.contains(*identity)).cloned().collect();
    for identity in absent {dispose(world,players.entries.remove(&identity).unwrap());}
    let failed=world.get_resource::<crate::resources::VnEngine>().is_some_and(|engine|engine.0.renderer.scene_frame_failed);
    for item in plan.items {
        let identity=item.view.token.target.clone();
        if !players.entries.contains_key(&identity) {
            let source=world.resource::<AssetServer>().load(item.path);
            players.entries.insert(identity.clone(),Reader{view:item.view.clone(),source,checkpoint:None,pcm:None,entity:None,ever_started:false,terminal:None,submitted:None});
        }
        let reader=players.entries.get_mut(&identity).unwrap();
        let newly_prepared=reader.checkpoint.is_none();
        if let Some((checkpoint,pcm))=crate::checkpoint_sources::one_shot_source(world,&identity){reader.checkpoint=Some(checkpoint);reader.pcm=Some(pcm);}
        if reader.view.token!=item.view.token {reader.submitted=None;}
        reader.view=item.view;
        let paused=reader.view.parked || failed;
        if let Some(entity)=reader.entity {
            if world.get_entity(entity).is_none() {
                reader.entity=None;reader.terminal.get_or_insert(AudioFeedback::Error("Exact audio reader disappeared; checkpoint is unavailable".into()));
            } else {
                world.entity_mut(entity).insert(OwnedAudio{token:reader.view.token.clone()});
                if let Some(mut settings)=world.get_mut::<PlaybackSettings>(entity){settings.volume=Volume::new(item.gain);settings.paused=paused;}
                if let Some(sink)=world.get::<AudioSink>(entity){sink.set_volume(item.gain);if paused {sink.pause();} else if reader.terminal.is_none() {sink.play();}}
            }
        }
        if reader.entity.is_none() && !reader.ever_started && !paused && reader.terminal.is_none() && !newly_prepared && reader.pcm.is_some() {
            let entity=world.spawn((bevy::audio::AudioSourceBundle::<crate::checkpoint_pcm::CheckpointPcm>{source:reader.pcm.clone().unwrap(),settings:PlaybackSettings::ONCE.with_volume(Volume::new(item.gain)),..default()},OwnedAudio{token:reader.view.token.clone()})).id();
            reader.entity=Some(entity);reader.ever_started=true;
            crate::systems::audio::resume_audio_contexts_after_spawn();
        }
    }
    world.insert_resource(players);
}

#[derive(Clone,Debug,PartialEq,Eq)]
struct QueuedAudio {nonce:u64,report:AudioReport}
#[derive(Resource,Default)]
pub(crate) struct AudioFeedbackQueue {next_nonce:u64,rows:Vec<QueuedAudio>}
#[derive(Clone,Debug)]
pub(crate) struct AudioQueueSnapshot {rows:Vec<QueuedAudio>}
impl AudioQueueSnapshot {pub(crate) fn reports(&self)->Vec<AudioReport>{self.rows.iter().map(|row|row.report.clone()).collect()}}
impl AudioFeedbackQueue {
    fn push(&mut self,report:AudioReport)->Result<(),String> {
        if self.rows.len()>=audio_playback::MAX_AUDIO_PLAYBACKS {return Err("Audio feedback queue is full; exact terminal retained by its reader".into());}
        let nonce=self.next_nonce.checked_add(1).ok_or("Audio feedback nonce exhausted")?;
        self.rows.push(QueuedAudio{nonce,report});self.next_nonce=nonce;Ok(())
    }
    pub(crate) fn snapshot(&self)->AudioQueueSnapshot {AudioQueueSnapshot{rows:self.rows.clone()}}
    pub(crate) fn acknowledge(&mut self,snapshot:&AudioQueueSnapshot,receipts:&[rvn_core::engine::AudioReportReceipt])->Result<(),String> {
        let mut consumed=HashSet::new();
        for receipt in receipts {
            let row=snapshot.rows.get(receipt.ordinal).ok_or("Audio receipt ordinal is absent")?;
            if !consumed.insert(row.nonce) || !self.rows.iter().any(|current|current==row) {return Err("Audio receipt does not identify the exact queue snapshot".into());}
        }
        let removed:HashSet<_>=receipts.iter().filter(|receipt|receipt.disposition!=rvn_core::engine::AudioReportDisposition::Deferred).map(|receipt|snapshot.rows[receipt.ordinal].nonce).collect();
        self.rows.retain(|row|!removed.contains(&row.nonce));Ok(())
    }
}
fn failure(world:&mut World,message:String) {
    if let Some(mut engine)=world.get_resource_mut::<crate::resources::VnEngine>(){engine.0.renderer.scene_frame_failed=true;}
    if let Some(mut error)=world.get_resource_mut::<crate::resources::ScriptErrorMessage>(){error.0=message;}
    if let Some(mut next)=world.get_resource_mut::<NextState<crate::resources::VnState>>(){next.set(crate::resources::VnState::Error);}
}
fn bounded_decoder_error(message:String)->String {
    let mut message=message.replace('\0',"\\0");
    if message.len()>4096 {
        let mut end=4093;while !message.is_char_boundary(end){end-=1;}
        message.truncate(end);message.push_str("...");
    }
    if message.is_empty(){"Unspecified audio decoder error".into()}else{message}
}
pub(crate) fn snapshot_after_stale_ack(world:&mut World)->Result<AudioQueueSnapshot,String> {
    world.init_resource::<AudioFeedbackQueue>();let snapshot=world.resource::<AudioFeedbackQueue>().snapshot();
    let receipts={let engine=&world.resource::<crate::resources::VnEngine>().0;
        snapshot.rows.iter().enumerate().filter_map(|(ordinal,row)|(engine.audio_report_admission(row.report.instance_id,&row.report.token)==AudioAdmission::Stale).then_some(rvn_core::engine::AudioReportReceipt{ordinal,disposition:rvn_core::engine::AudioReportDisposition::RejectedStale})).collect::<Vec<_>>()};
    world.resource_mut::<AudioFeedbackQueue>().acknowledge(&snapshot,&receipts)?;
    Ok(world.resource::<AudioFeedbackQueue>().snapshot())
}
fn poll(world:&mut World) {
    if !world.get_resource::<crate::resources::VnEngine>().is_some_and(|engine|engine.0.state.audio_resource_registry().is_some()){return;}
    if let Err(message)=snapshot_after_stale_ack(world){failure(world,message);return;}
    let mut players=world.remove_resource::<AudioPlayers>().unwrap_or_default();
    let current=world.resource::<crate::resources::VnEngine>().0.instance_id();
    if players.instance_id!=Some(current) {world.insert_resource(players);return;}
    for reader in players.entries.values_mut() {
        if reader.terminal.is_none() {
            if let Some(checkpoint)=&reader.checkpoint {match checkpoint.fault(){Ok(Some(error))=>{reader.terminal=Some(AudioFeedback::Error(bounded_decoder_error(error)));},Err(error)=>{reader.terminal=Some(AudioFeedback::Error(bounded_decoder_error(error)));},_=>{}}}
            if let Some(LoadState::Failed(error))=world.resource::<AssetServer>().get_load_state(reader.source.id()) {reader.terminal=Some(AudioFeedback::Error(bounded_decoder_error(format!("Audio source failed: {error}"))));}
            if let Some(entity)=reader.entity {
                if world.get_entity(entity).is_none(){reader.terminal=Some(AudioFeedback::Error("Exact audio reader disappeared".into()));}
                else if world.get::<AudioSink>(entity).is_some_and(|sink|sink.empty()){reader.terminal=Some(AudioFeedback::End);}
            }
        }
        if reader.submitted.as_ref()==Some(&reader.view.token){continue;}
        if let Some(feedback)=reader.terminal.clone() {
            let result=world.resource_mut::<AudioFeedbackQueue>().push(AudioReport{instance_id:current,token:reader.view.token.clone(),feedback});
            match result {Ok(())=>reader.submitted=Some(reader.view.token.clone()),Err(message)=>{failure(world,message);break;}}
        }
    }
    world.insert_resource(players);
}
fn refresh(world:&mut World) {
    if world.contains_resource::<crate::debugger_startup::DebugBootstrap>() {
        // Init's real installer already transferred its accepted Sources/PCM.
        // Finish/reconcile only that projection; the Game publication helper
        // below cannot record a checkpoint against a nonexistent VnEngine.
        // A pending/failed/ambiguous owner keeps the last accepted readers.
        let Ok(owner)=crate::text_runtime_owner::current(world) else{return;};
        if owner.blocked || !world.resource::<crate::debugger_startup::DebugBootstrap>().accepts_actual_audio_broker(){return;}
        let (state,epoch)=(owner.state.clone(),owner.epoch);
        match prepare(world,&state,epoch) {
            Ok(audio)=>install(world,audio),
            Err(message)=>{
                world.resource_mut::<crate::debugger_startup::DebugBootstrap>().failed=Some(message.clone());
                if let Some(mut diagnostic)=world.get_resource_mut::<crate::resources::ScriptErrorMessage>(){diagnostic.0=message;}
                if let Some(mut next)=world.get_resource_mut::<NextState<crate::resources::VnState>>(){next.set(crate::resources::VnState::Error);}
            }
        }
        return;
    }
    if let Some(engine)=world.get_resource::<crate::resources::VnEngine>() {
        if engine.0.renderer.scene_frame_failed {
            crate::checkpoint_sources::pause_after_failure(world);
            // Failure revokes playback admission without installing an
            // unpublished state, replaying, seeking or advancing clocks.
            let entities=world.get_resource::<AudioPlayers>().map(|players|players.entries.values().filter_map(|reader|reader.entity).collect::<Vec<_>>()).unwrap_or_default();
            for entity in entities {
                if let Some(mut settings)=world.get_mut::<PlaybackSettings>(entity){settings.paused=true;}
                if let Some(sink)=world.get::<AudioSink>(entity){sink.pause();}
            }
            return;
        }
        if engine.0.renderer.scene_batch_pending() {return;}
    }
    let Some((state,epoch))=world.get_resource::<crate::resources::VnEngine>().filter(|engine|engine.0.state.audio_resource_registry().is_some()).map(|engine|(engine.0.state.clone(),engine.0.runtime_epoch())) else {
        if let Some(players)=world.remove_resource::<AudioPlayers>(){for (_,reader) in players.entries{dispose(world,reader);}}return;
    };
    let audio=match prepare(world,&state,epoch){Ok(plan)=>plan,Err(message)=>{failure(world,message);return;}};
    match crate::checkpoint_sources::prepare(world,&state,epoch) {
        Ok(Some(sources))=>match crate::checkpoint_sources::publish(world,&sources){Ok(())=>{crate::checkpoint_sources::install(world,sources);install(world,audio);},Err(message)=>failure(world,message)},
        Ok(None)=>{},Err(message)=>failure(world,message),
    }
}
pub(crate) struct SceneAudioPlugin;

#[cfg(all(test,not(target_arch="wasm32")))]
#[path="startup_audio_projection_tests.rs"]
mod startup_projection_tests;
impl Plugin for SceneAudioPlugin {
    fn build(&self,app:&mut App){use bevy::audio::AddAudioSource;if app.is_plugin_added::<bevy::audio::AudioPlugin>(){app.add_audio_source::<crate::checkpoint_pcm::CheckpointPcm>();}else{app.init_asset::<crate::checkpoint_pcm::CheckpointPcm>();}app.init_resource::<AudioPlayers>().init_resource::<AudioFeedbackQueue>().add_systems(Update,((
        poll.before(crate::scene_publication::GameFrameSet),
        refresh.after(crate::scene_publication::HostPublicationSet).after(crate::systems::audio_system),
    )).distributive_run_if(crate::runtime_restart::presentation_available));}
}

// Append to the new scene_audio module. CPU contracts use the actual Bevy
// AssetServer/AudioLoader and Core registry, with no decoder, AudioPlugin or
// hardware sink. Explicit typed readiness below is not a playback checkpoint.
#[cfg(all(test,not(target_arch="wasm32")))]
mod owned_audio_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use crate::resources::{VnEngine,VnState};

    fn game(source:&str)->rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        game.step_until_interaction().unwrap();game
    }
    fn app(source:&str)->App {
        let directory=bevy::asset::io::memory::Dir::default();
        for path in ["sfx/init.ogg","sfx/same.ogg","sfx/abandoned.ogg","sfx/legacy.ogg","hello.ogg"] {
            directory.insert_asset(std::path::Path::new(path),crate::checkpoint_sources::fixture_wave_for_test());
        }
        let reader=bevy::asset::io::memory::MemoryAssetReader{root:directory};
        let mut app=App::new();
        app.register_asset_source(bevy::asset::io::AssetSourceId::Default,bevy::asset::io::AssetSource::build().with_reader(move||Box::new(reader.clone())));
        app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{watch_for_changes_override:Some(false),..default()}));
        app.init_asset::<AudioSource>().init_asset_loader::<bevy::audio::AudioLoader>().init_asset::<crate::checkpoint_pcm::CheckpointPcm>();
        app.insert_resource(VnEngine(game(source))).insert_resource(State::new(VnState::Waiting))
            .init_resource::<NextState<VnState>>().init_resource::<crate::resources::VnRenderState>()
            .init_resource::<crate::resources::ChoiceFocus>().init_resource::<crate::resources::ImagemapState>()
            .init_resource::<crate::resources::TypewriterState>().init_resource::<crate::accessibility::Accessibility>()
            .init_resource::<crate::resources::ScriptErrorMessage>().init_resource::<crate::resources::MusicEntity>()
            .init_resource::<crate::resources::MusicAssetRegistry>().init_resource::<crate::resources::MusicPlaybackState>()
            .init_resource::<crate::resources::MusicVolume>().init_resource::<crate::systems::settings_menu::Settings>()
            .init_resource::<crate::scene_publication::ScenePresentation>().init_resource::<AudioPlayers>()
            .init_resource::<AudioFeedbackQueue>().add_event::<crate::vn_command::VnCommand>().add_event::<crate::vn_command::PlayerInput>();
        app
    }
    fn apply(world:&mut World) {
        let engine=&world.resource::<VnEngine>().0;
        let plan=prepare(world,&engine.state,engine.runtime_epoch()).unwrap();install(world,plan);
    }
    fn consume_ready(world:&mut World) {
        let _=crate::checkpoint_sources::prime_assets_for_test(world);
        crate::scene_publication::consume(world);
    }
    fn ready(world:&mut World) {
        let handles=world.resource::<AudioPlayers>().entries.values().map(|reader|reader.source.clone()).collect::<Vec<_>>();
        for handle in handles {world.resource_mut::<Assets<AudioSource>>().insert(handle.id(),AudioSource{bytes:std::sync::Arc::<[u8]>::from(crate::checkpoint_sources::fixture_wave_for_test())});}
        apply(world);
    }
    fn advance(world:&mut World) {
        let mut wrapper=world.resource_mut::<VnEngine>();
        wrapper.0.advance_dialogue().unwrap();wrapper.0.step_until_interaction().unwrap();
        drop(wrapper);consume_ready(world);
    }
    fn state(world:&World)->serde_json::Value {serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap()}
    fn readers(world:&World)->Vec<(TargetIdentity,Entity,ResourceToken,bool)> {
        world.resource::<AudioPlayers>().entries.iter().map(|(id,reader)|(id.clone(),reader.entity.unwrap(),reader.view.token.clone(),reader.view.parked)).collect()
    }

    #[test]
    fn audio_publication_coalesces_abandoned_scene_oneshots_and_keeps_actual_game_init_reader() {
        let mut app=app("init{sfx.play(\"init.ogg\")}\nscene label A\nsfx.play(\"abandoned.ogg\")\nvoice \"abandoned\"\njump B\nscene label B\n\"b\"");
        let world=app.world_mut();consume_ready(world);
        assert!(!world.resource::<VnEngine>().0.renderer.scene_batch_pending());
        let cache=world.resource::<AudioPlayers>();assert_eq!(cache.entries.len(),1);
        let entry=cache.entries.values().next().unwrap();assert_eq!(entry.view.playback.file,"sfx/init.ogg");assert_eq!(entry.view.token.task.key.owner,rvn_core::scene_tasks::TaskOwner::Game);assert!(!entry.ever_started);
        assert!(world.resource::<AssetServer>().get_handle::<AudioSource>("sfx/abandoned.ogg").is_none());
        assert!(!world.resource::<Events<crate::vn_command::VnCommand>>().iter_current_update_events().any(|command|matches!(command,crate::vn_command::VnCommand::SfxPlay{..}|crate::vn_command::VnCommand::VoicePlay{..}|crate::vn_command::VnCommand::SfxPlayTransition{..})));
        ready(world);let entries=readers(world);assert_eq!(entries.len(),1);let entity=entries[0].1;
        assert!(world.get::<OwnedAudio>(entity).is_some());assert!(world.get::<crate::components::SfxSource>(entity).is_none());assert!(world.get::<crate::scene_retired::RetiredProjection>(entity).is_none());
        assert!(world.get::<AudioSink>(entity).is_none());
    }

    #[test]
    fn simultaneous_same_file_readers_keep_distinct_entities_and_apply_user_gain_only_once() {
        let mut app=app("scene label A\nsfx.play(\"same.ogg\") with fade(1000)\nsfx.play(\"same.ogg\")\nvoice \"hello\"\n\"a\"");
        let world=app.world_mut();world.resource_mut::<crate::systems::settings_menu::Settings>().sfx_volume=0.4;
        consume_ready(world);ready(world);
        let before=state(world);let samples=readers(world);assert_eq!(samples.len(),3);assert_eq!(samples.iter().map(|row|row.1).collect::<HashSet<_>>().len(),3);
        let handles=world.resource::<AudioPlayers>().entries.values().filter(|reader|reader.view.playback.family==AudioFamily::Sfx).map(|reader|reader.source.clone()).collect::<Vec<_>>();assert_eq!(handles[0],handles[1]);
        world.resource_mut::<VnEngine>().0.tick_game_frame(0.25,rvn_core::GameFrameReports::default()).unwrap();consume_ready(world);
        let engine_state=state(world);assert_ne!(engine_state,before);apply(world);assert_eq!(state(world),engine_state);
        for reader in world.resource::<AudioPlayers>().entries.values() {
            let entity=reader.entity.unwrap();let expected=if reader.view.playback.family==AudioFamily::Voice {1.0}else{(f64::from(reader.view.playback.gain())*f64::from(0.4f32)) as f32};
            assert_eq!(world.get::<PlaybackSettings>(entity).unwrap().volume.get().to_bits(),expected.to_bits());
            assert_eq!(world.get::<OwnedAudio>(entity).unwrap().token,reader.view.token);
        }
        assert_eq!(world.resource::<AudioPlayers>().entries.values().find(|reader|reader.view.playback.family==AudioFamily::Voice).unwrap().source,world.resource::<AssetServer>().get_handle::<AudioSource>("hello.ogg").unwrap());
    }

    #[test]
    fn parked_scene_reader_and_persistent_error_return_with_new_authority_without_restarting() {
        let mut app=app("init{sfx.play(\"init.ogg\")}\nscene label P\nsfx.play(\"same.ogg\") with fade(4000)\n\"parent\"\ncall C\n\"returned\"\nscene label C\n\"child\"\nreturn");
        let world=app.world_mut();consume_ready(world);ready(world);let original=readers(world);
        let scene=original.iter().find(|row|row.2.task.key.owner!=rvn_core::scene_tasks::TaskOwner::Game).unwrap().clone();
        advance(world);assert_eq!(world.resource::<AudioPlayers>().entries[&scene.0].entity,Some(scene.1));assert!(world.get::<PlaybackSettings>(scene.1).unwrap().paused);
        let game=original.iter().find(|row|row.2.task.key.owner==rvn_core::scene_tasks::TaskOwner::Game).unwrap();assert!(!world.get::<PlaybackSettings>(game.1).unwrap().paused);
        world.resource_mut::<AudioPlayers>().entries.get_mut(&scene.0).unwrap().terminal=Some(AudioFeedback::Error("persistent decode failure".into()));poll(world);
        let parked=world.resource::<AudioFeedbackQueue>().snapshot();assert_eq!(parked.rows.len(),1);assert_eq!(world.resource::<VnEngine>().0.audio_report_admission(parked.rows[0].report.instance_id,&parked.rows[0].report.token),AudioAdmission::Deferred);
        advance(world);poll(world);let resumed=world.resource::<AudioFeedbackQueue>().snapshot();assert_eq!(resumed.rows.len(),1);
        assert_eq!(resumed.rows[0].report.token.target,scene.0);assert_ne!(resumed.rows[0].report.token.task.revision,parked.rows[0].report.token.task.revision);
        assert_eq!(resumed.rows[0].report.feedback,parked.rows[0].report.feedback);assert_eq!(world.resource::<AudioPlayers>().entries[&scene.0].entity,Some(scene.1));
        assert!(world.resource::<AudioPlayers>().entries[&scene.0].ever_started);assert_eq!(world.resource::<AudioPlayers>().entries[&scene.0].view.playback.envelope.as_ref().unwrap().clock.elapsed,0.0);
    }

    #[test]
    fn engine_replacement_at_same_epoch_and_target_disposes_old_reader_and_rejects_old_terminal() {
        let source="scene label A\nsfx.play(\"same.ogg\")\n\"a\"";let mut app=app(source);let world=app.world_mut();consume_ready(world);ready(world);
        let old=readers(world).remove(0);let old_instance=world.resource::<VnEngine>().0.instance_id();
        let report=AudioReport{instance_id:old_instance,token:old.2.clone(),feedback:AudioFeedback::End};world.resource_mut::<AudioFeedbackQueue>().push(report.clone()).unwrap();
        let fresh=game(source);assert_eq!(fresh.runtime_epoch(),old.2.task.runtime_epoch);assert_eq!(fresh.audio_playback_views().unwrap()[0].token,old.2);assert_ne!(fresh.instance_id(),old_instance);
        world.insert_resource(VnEngine(fresh));let before=state(world);
        let (_,receipt)=world.resource_mut::<VnEngine>().0.tick_game_frame_with_audio(0.0,rvn_core::GameFrameReports::default(),vec![report],true).unwrap();assert_eq!(receipt[0].disposition,rvn_core::engine::AudioReportDisposition::RejectedStale);assert_eq!(state(world),before);
        consume_ready(world);assert!(world.get_entity(old.1).is_none());assert!(world.resource::<AudioFeedbackQueue>().rows.is_empty());ready(world);
        let new=readers(world).remove(0);assert_eq!(new.0,old.0);assert_ne!(new.1,old.1);assert!(world.resource::<AudioPlayers>().entries[&new.0].terminal.is_none());
    }

    #[test]
    fn exact_queue_nonce_acknowledgement_and_backpressure_retain_the_unadmitted_terminal() {
        let mut app=app("scene label A\nsfx.play(\"same.ogg\")\n\"a\"");let world=app.world_mut();consume_ready(world);ready(world);
        let reader=readers(world).remove(0);let instance_id=world.resource::<VnEngine>().0.instance_id();let report=AudioReport{instance_id,token:reader.2.clone(),feedback:AudioFeedback::End};
        for _ in 0..64 {world.resource_mut::<AudioFeedbackQueue>().push(report.clone()).unwrap();}
        let full=world.resource::<AudioFeedbackQueue>().snapshot();let old_next=world.resource::<AudioFeedbackQueue>().next_nonce;
        assert!(world.resource_mut::<AudioFeedbackQueue>().push(report.clone()).is_err());assert_eq!(world.resource::<AudioFeedbackQueue>().rows,full.rows);assert_eq!(world.resource::<AudioFeedbackQueue>().next_nonce,old_next);
        world.resource_mut::<AudioPlayers>().entries.get_mut(&reader.0).unwrap().terminal=Some(AudioFeedback::End);poll(world);
        assert_eq!(world.resource::<AudioFeedbackQueue>().rows,full.rows);assert_eq!(world.resource::<AudioPlayers>().entries[&reader.0].terminal,Some(AudioFeedback::End));assert!(world.resource::<VnEngine>().0.renderer.scene_frame_failed);
        let bad=vec![rvn_core::engine::AudioReportReceipt{ordinal:0,disposition:rvn_core::engine::AudioReportDisposition::Consumed},rvn_core::engine::AudioReportReceipt{ordinal:0,disposition:rvn_core::engine::AudioReportDisposition::Consumed}];
        assert!(world.resource_mut::<AudioFeedbackQueue>().acknowledge(&full,&bad).is_err());assert_eq!(world.resource::<AudioFeedbackQueue>().rows,full.rows);
        let deferred=vec![rvn_core::engine::AudioReportReceipt{ordinal:0,disposition:rvn_core::engine::AudioReportDisposition::Deferred}];world.resource_mut::<AudioFeedbackQueue>().acknowledge(&full,&deferred).unwrap();assert_eq!(world.resource::<AudioFeedbackQueue>().rows,full.rows);
        let consumed=(0..64).map(|ordinal|rvn_core::engine::AudioReportReceipt{ordinal,disposition:rvn_core::engine::AudioReportDisposition::Consumed}).collect::<Vec<_>>();world.resource_mut::<AudioFeedbackQueue>().acknowledge(&full,&consumed).unwrap();
        world.resource_mut::<VnEngine>().0.renderer.scene_frame_failed=false;poll(world);assert_eq!(world.resource::<AudioFeedbackQueue>().rows.len(),1);assert!(world.resource::<AudioFeedbackQueue>().rows[0].nonce>old_next);
        let before=world.resource::<AudioFeedbackQueue>().snapshot();world.resource_mut::<AudioFeedbackQueue>().next_nonce=u64::MAX;assert!(world.resource_mut::<AudioFeedbackQueue>().push(report).is_err());assert_eq!(world.resource::<AudioFeedbackQueue>().rows,before.rows);
    }

    #[test]
    fn invalid_audio_gain_preflight_preserves_pending_outputs_assets_and_ecs_before_publication() {
        let mut app=app("scene label A\nsfx.play(\"same.ogg\")\n\"a\"");let world=app.world_mut();
        world.resource_mut::<crate::systems::settings_menu::Settings>().sfx_volume=f32::NAN;
        world.resource_mut::<VnEngine>().0.renderer.pending.push(crate::vn_command::VnCommand::VoiceStop);
        world.send_event(crate::vn_command::VnCommand::VoicePlay{file:"existing.ogg".into()});
        let sentinel=world.spawn(Transform::from_xyz(1.0,2.0,3.0)).id();let before=state(world);
        let pending=format!("{:?}",world.resource::<VnEngine>().0.renderer.scene_batch.as_ref().unwrap());let output=format!("{:?}",world.resource::<VnEngine>().0.renderer.pending);let events=world.resource::<Events<crate::vn_command::VnCommand>>().len();let entities=world.entities().len();
        assert!(world.resource::<AssetServer>().get_handle::<AudioSource>("sfx/same.ogg").is_none());consume_ready(world);
        assert_eq!(state(world),before);assert_eq!(format!("{:?}",world.resource::<VnEngine>().0.renderer.scene_batch.as_ref().unwrap()),pending);
        assert_eq!(format!("{:?}",world.resource::<VnEngine>().0.renderer.pending),output);assert_eq!(world.resource::<Events<crate::vn_command::VnCommand>>().len(),events);assert_eq!(world.entities().len(),entities);
        assert_eq!(world.get::<Transform>(sentinel).unwrap().translation,Vec3::new(1.0,2.0,3.0));assert!(world.resource::<AudioPlayers>().entries.is_empty());assert!(world.resource::<AssetServer>().get_handle::<AudioSource>("sfx/same.ogg").is_none());
        assert!(world.resource::<VnEngine>().0.renderer.scene_frame_failed);assert!(world.resource::<crate::resources::ScriptErrorMessage>().0.contains("SFX user gain"));
    }

    #[test]
    fn actual_update_current_decoder_error_keeps_exact_queue_and_clock_under_failure_latch() {
        let mut app=app("scene label A\nsfx.play(\"same.ogg\") with fade(1000)\n\"a\"");
        consume_ready(app.world_mut());ready(app.world_mut());
        app.add_plugins((crate::scene_publication::ScenePublicationPlugin,SceneAudioPlugin));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::ZERO));
        let identity=readers(app.world()).remove(0).0;app.world_mut().resource_mut::<AudioPlayers>().entries.get_mut(&identity).unwrap().terminal=Some(AudioFeedback::Error("actual terminal failure".into()));
        let before=state(app.world());app.update();let queued=app.world().resource::<AudioFeedbackQueue>().snapshot();assert_eq!(queued.rows.len(),1);assert_eq!(state(app.world()),before);
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);assert!(app.world().resource::<crate::resources::ScriptErrorMessage>().0.contains("actual terminal failure"));
        let entity=app.world().resource::<AudioPlayers>().entries[&identity].entity.unwrap();assert!(app.world().get::<PlaybackSettings>(entity).unwrap().paused);
        app.update();assert_eq!(app.world().resource::<AudioFeedbackQueue>().rows,queued.rows);assert_eq!(state(app.world()),before);
    }

    #[test]
    fn legacy_sfx_transition_reaches_actual_audio_system_without_touching_owned_reader_roles() {
        use rvn_core::Renderer;
        let mut app=app("scene label A\nsfx.play(\"same.ogg\")\n\"a\"");let world=app.world_mut();consume_ready(world);ready(world);let owned=readers(world).remove(0).1;
        world.entity_mut(owned).insert(crate::components::SfxSource{file:"sfx/legacy.ogg".into()});
        let mut renderer=crate::bevy_renderer::BevyRenderer::new();renderer.sfx_play("sfx/legacy.ogg",&rvn_parser::Transition::Fade{duration_ms:1000});let commands=renderer.take_pending();assert!(matches!(&commands[0],crate::vn_command::VnCommand::SfxPlayTransition{file,..} if file=="sfx/legacy.ogg"));
        for command in commands {world.send_event(command);}world.run_system_once(crate::systems::audio_system);
        let legacy=world.query_filtered::<(Entity,&crate::components::AudioFade,&PlaybackSettings),Without<OwnedAudio>>().iter(world).map(|(entity,fade,settings)|(entity,fade.from_vol,fade.to_vol,fade.duration_secs,settings.volume.get())).collect::<Vec<_>>();
        assert_eq!(legacy.len(),1);assert_eq!((legacy[0].1,legacy[0].2,legacy[0].3,legacy[0].4),(0.0,1.0,1.0,0.0));
        world.send_event(crate::vn_command::VnCommand::SfxStopTransition{file:"sfx/legacy.ogg".into(),transition:rvn_parser::Transition::Fade{duration_ms:1000}});world.run_system_once(crate::systems::audio_system);
        assert!(world.get_entity(legacy[0].0).is_none());assert!(world.get_entity(owned).is_some());assert!(world.get::<OwnedAudio>(owned).is_some());assert!(world.get::<crate::components::AudioFade>(owned).is_none());
    }

// Append inside a new sibling test module; the old eight module bodies remain
// exact. Reuse actual private helpers by making the appended case a sibling
// function within owned_audio_tests during the RAW composition.
    #[test]
    fn pending_valid_audio_keeps_playback_but_refused_pending_batch_pauses_existing_exact_reader() {
        let mut app=app("scene label A\nsfx.play(\"same.ogg\") with fade(4000)\n\"a\"");
        let world=app.world_mut();consume_ready(world);ready(world);
        let reader=readers(world).remove(0);let entity=reader.1;
        assert!(!world.get::<PlaybackSettings>(entity).unwrap().paused);
        world.resource_mut::<VnEngine>().0.tick_game_frame(0.25,rvn_core::GameFrameReports::default()).unwrap();
        assert!(world.resource::<VnEngine>().0.renderer.scene_batch_pending());
        let before=state(world);let original_settings=world.get::<PlaybackSettings>(entity).unwrap().clone();
        refresh(world);
        assert_eq!(state(world),before);assert_eq!(world.get::<PlaybackSettings>(entity).unwrap().paused,original_settings.paused);
        assert_eq!(world.get::<PlaybackSettings>(entity).unwrap().volume.get().to_bits(),original_settings.volume.get().to_bits());
        world.resource_mut::<crate::systems::settings_menu::Settings>().sfx_volume=f32::NAN;
        consume_ready(world);
        assert!(world.resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert!(world.resource::<VnEngine>().0.renderer.scene_frame_failed);
        assert_eq!(state(world),before);assert!(!world.get::<PlaybackSettings>(entity).unwrap().paused);
        let queue=world.resource::<AudioFeedbackQueue>().rows.clone();let pending=format!("{:?}",world.resource::<VnEngine>().0.renderer.scene_batch.as_ref());
        refresh(world);
        assert_eq!(state(world),before);assert!(world.get::<PlaybackSettings>(entity).unwrap().paused);
        assert_eq!(world.resource::<AudioFeedbackQueue>().rows,queue);
        assert_eq!(format!("{:?}",world.resource::<VnEngine>().0.renderer.scene_batch.as_ref()),pending);
        assert_eq!(world.resource::<AudioPlayers>().entries[&reader.0].entity,Some(entity));
        assert!(world.resource::<AudioPlayers>().entries[&reader.0].ever_started);
        assert_eq!(world.get::<OwnedAudio>(entity).unwrap().token,reader.2);
        assert!(world.get::<AudioSink>(entity).is_none());
    }
}
