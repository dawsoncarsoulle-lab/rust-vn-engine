use bevy::audio::Volume;
use bevy::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
use crate::components::MusicMarker;
use crate::components::{AudioFade, SfxSource};
#[cfg(target_arch = "wasm32")]
use crate::resources::PendingMusicPlayback;
use crate::resources::{MusicAssetRegistry, MusicEntity, MusicPlaybackState, MusicVolume};
use crate::systems::settings_menu::Settings;
use crate::vn_command::VnCommand;
use rvn_parser::Transition;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
export function resume_all_audio_contexts() {
    if (globalThis.__rvnResumeAllAudioContexts) {
        globalThis.__rvnResumeAllAudioContexts();
    }
}

export function rvn_browser_music_play(path, volume) {
    const url = path.startsWith('assets/') ? path : `assets/${path}`;
    const vol = Math.max(0, Math.min(1, Number(volume) || 0));
    let audio = globalThis.__rvnMusicAudio;
    if (!audio || audio.dataset.rvnSrc !== url) {
        if (audio) {
            audio.pause();
            audio.src = '';
        }
        audio = new Audio(url);
        audio.dataset.rvnSrc = url;
        audio.loop = true;
        audio.preload = 'auto';
        audio.crossOrigin = 'anonymous';
        globalThis.__rvnMusicAudio = audio;
    }
    audio.volume = vol;
    const play = audio.play();
    if (play && typeof play.catch === 'function') {
        play.catch(() => {});
    }
}

export function rvn_browser_music_stop() {
    const audio = globalThis.__rvnMusicAudio;
    if (audio) {
        audio.pause();
        audio.currentTime = 0;
    }
}

export function rvn_browser_music_set_volume(volume) {
    const audio = globalThis.__rvnMusicAudio;
    if (audio) {
        audio.volume = Math.max(0, Math.min(1, Number(volume) || 0));
    }
}
"#)]
extern "C" {
    fn resume_all_audio_contexts();
    fn rvn_browser_music_play(path: &str, volume: f64);
    fn rvn_browser_music_stop();
    fn rvn_browser_music_set_volume(volume: f64);
}

pub fn audio_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut vn_events: EventReader<VnCommand>,
    mut music_entity: ResMut<MusicEntity>,
    mut _music_playback: ResMut<MusicPlaybackState>,
    music_registry: Res<MusicAssetRegistry>,
    music_volume: Res<MusicVolume>,
    settings: Res<Settings>,
    sfx_query: Query<(Entity, &SfxSource), (Without<crate::scene_retired::RetiredProjection>,Without<crate::scene_audio::OwnedAudio>)>,
    sink_query: Query<&AudioSink, Without<crate::scene_retired::RetiredProjection>>,
    voice_query: Query<(Entity, &Name), (Without<crate::scene_retired::RetiredProjection>,Without<crate::scene_audio::OwnedAudio>)>,
    engine: Option<Res<crate::resources::VnEngine>>,
    startup: Option<Res<crate::debugger_startup::DebugBootstrap>>,
    mut error: Option<ResMut<crate::resources::ScriptErrorMessage>>,
    mut next: Option<ResMut<NextState<crate::resources::VnState>>>,
) {
    // Root must run this pure check during batch preflight before any Current
    // handler. This local refusal is a diagnostic guard, not ECS rollback.
    let owner=match(engine.as_deref(),startup.as_deref()) {
        (Some(engine),None)=>Some((&engine.0.state,engine.0.runtime_epoch(),engine.0.renderer.scene_batch_pending()||engine.0.renderer.scene_frame_failed)),
        (None,Some(startup))=>Some((startup.owner.state(),startup.owner.snapshot().epoch,startup.publication_pending||startup.failed.is_some()||!startup.accepts_actual_audio_broker())),
        (None,None)=>None,
        _=>return,
    };
    if owner.is_some_and(|(_,_,blocked)|blocked){return;}
    let modern_audio=owner.is_some_and(|(state,_,_)|state.audio_resource_registry().is_some());
    #[cfg(target_arch = "wasm32")]
    let modern_web=modern_audio;
    let logical_gain=owner.filter(|_|modern_audio).map(|(state,epoch,_)|current_music_gain_from_state(state,epoch)).transpose();
    let modern_gain=match logical_gain.and_then(|gain| gain.map(|logical|crate::music_mix::final_gain(logical,settings.music_volume)).transpose()) {
        Ok(gain)=>gain,
        Err(message)=>{
            if let Some(error)=error.as_deref_mut() {error.0=message.clone();}
            if let Some(next)=next.as_deref_mut() {next.set(crate::resources::VnState::Error);}
            bevy::log::error!("Current music projection refused: {message}");
            return;
        }
    };
    let current_volume=MusicVolume(modern_gain.unwrap_or(music_volume.0));
    if music_volume.is_changed() {
        if let Some(entity) = music_entity.0 {
            if let Ok(sink) = sink_query.get(entity) {
                sink.set_volume(*Volume::new(current_volume.0));
            }
        }
    }

    let cmds: Vec<VnCommand> = vn_events.read().cloned().collect();

    for cmd in cmds {
        match cmd {
            VnCommand::MusicPlay { file, transition } => {
                if modern_audio {continue;}
                #[cfg(target_arch = "wasm32")]
                if modern_web {
                    _music_playback.last_request=None; _music_playback.web_music_replay_pending=false;
                    continue; // Complete exact snapshot already installed by WebMusic, no singleton replay.
                }
                let fade_ms = if modern_gain.is_some() {None} else {match &transition {
                    Transition::None => None,
                    other => Some(other.duration_ms()),
                }};
                #[cfg(target_arch = "wasm32")]
                {
                    _music_playback.last_request = Some(PendingMusicPlayback {
                        file: file.clone(),
                        fade_ms,
                    });
                    _music_playback.web_music_replay_pending = true;
                }
                spawn_music(
                    &mut commands,
                    &asset_server,
                    &music_registry,
                    &mut music_entity,
                    &current_volume,
                    &file,
                    fade_ms,
                );
            }

            VnCommand::MusicStop => {
                if modern_audio {continue;}
                #[cfg(target_arch = "wasm32")]
                if modern_web {
                    _music_playback.last_request=None; _music_playback.web_music_replay_pending=false;
                    continue; // Exact Current removal/outgoing is represented in the complete snapshot.
                }
                if let Some(entity) = music_entity.0.take() {
                    commands.entity(entity).despawn();
                }
                #[cfg(target_arch = "wasm32")]
                rvn_browser_music_stop();
                #[cfg(target_arch = "wasm32")]
                {
                    _music_playback.last_request = None;
                    _music_playback.web_music_replay_pending = false;
                }
                info!("[music] stop");
            }

            VnCommand::MusicSetVolume { level } => {
                #[cfg(target_arch = "wasm32")]
                if modern_web { continue; } // Indexed Current gain refresh requires no MusicEntity.
                let level=modern_gain.unwrap_or(level.clamp(0.0,1.0));
                #[cfg(target_arch = "wasm32")]
                rvn_browser_music_set_volume(level as f64);
                if let Some(entity) = music_entity.0 {
                    if let Ok(sink) = sink_query.get(entity) {
                        sink.set_volume(*Volume::new(level.clamp(0.0, 1.0)));
                    }
                }
                info!("[music] volume {:.2}", level);
            }

            VnCommand::SfxPlayTransition { file, transition } => {
                if modern_audio {continue;}
                let target=settings.sfx_volume;
                let duration=transition.duration_ms() as f32 / 1000.0;
                commands.spawn((AudioBundle{source:asset_server.load(file.clone()),settings:PlaybackSettings::ONCE.with_volume(Volume::new(0.0))},SfxSource{file},AudioFade{from_vol:0.0,to_vol:target,duration_secs:duration,elapsed_secs:0.0,despawn_on_done:false}));
                resume_audio_contexts_after_spawn();
            }
            VnCommand::SfxStopTransition { file, transition } => {
                if modern_audio {continue;}
                let duration=transition.duration_ms() as f32 / 1000.0;
                for (entity,src) in sfx_query.iter() {
                    if src.file!=file {continue;}
                    if duration>0.0 {
                        if let Ok(sink)=sink_query.get(entity) {commands.entity(entity).insert(AudioFade{from_vol:bevy::audio::AudioSinkPlayback::volume(sink),to_vol:0.0,duration_secs:duration,elapsed_secs:0.0,despawn_on_done:true});}
                        else {commands.entity(entity).despawn();} // Pending sources must not start after Stop.
                    } else {commands.entity(entity).despawn();}
                }
            }
            VnCommand::SfxPlay { file } => {
                if modern_audio {continue;}
                let source: Handle<AudioSource> = asset_server.load(file.clone());
                commands.spawn((
                    AudioBundle {
                        source,
                        settings: PlaybackSettings::ONCE
                            .with_volume(Volume::new(settings.sfx_volume)),
                    },
                    SfxSource { file: file.clone() },
                ));
                resume_audio_contexts_after_spawn();
                info!("[sfx] play {}", file);
            }

            VnCommand::SfxStop { file } => {
                if modern_audio {continue;}
                for (entity, src) in sfx_query.iter() {
                    if src.file == file {
                        commands.entity(entity).despawn();
                        info!("[sfx] stop {}", file);
                    }
                }
            }

            VnCommand::VoicePlay { file } => {
                if modern_audio {continue;}
                let path = if file.contains('.') {
                    file.to_string()
                } else {
                    format!("{}.ogg", file)
                };
                commands.spawn((
                    Name::new(format!("voice:{}", path)),
                    AudioBundle {
                        source: asset_server.load(&path),
                        settings: PlaybackSettings::DESPAWN,
                    },
                ));
            }
            VnCommand::VoiceStop => {
                if modern_audio {continue;}
                for (entity, name) in voice_query.iter() {
                    if name.as_str().starts_with("voice:") {
                        commands.entity(entity).despawn();
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn audio_unlock_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut music_entity: ResMut<MusicEntity>,
    mut music_playback: ResMut<MusicPlaybackState>,
    music_registry: Res<MusicAssetRegistry>,
    music_volume: Res<MusicVolume>,
    mut engine: Option<ResMut<crate::resources::VnEngine>>,
    mut error: Option<ResMut<crate::resources::ScriptErrorMessage>>,
    mut next: Option<ResMut<NextState<crate::resources::VnState>>>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let _=(&engine,&error,&next);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (
            &mut commands,
            &asset_server,
            &mouse,
            &keys,
            &touches,
            &mut music_entity,
            &mut music_playback,
            &music_registry,
            &music_volume,
        );
    }

    #[cfg(target_arch = "wasm32")]
    {
        let input_seen = mouse.get_just_pressed().next().is_some()
            || keys.get_just_pressed().next().is_some()
            || touches.iter_just_pressed().next().is_some();
        if !input_seen {
            return;
        }

        resume_all_audio_contexts();
        music_playback.web_resume_attempts = music_playback.web_resume_attempts.saturating_add(1);

        if engine.as_deref().is_some_and(|engine|engine.0.state.audio_resource_registry().is_some()) {
            music_playback.last_request=None;music_playback.web_music_replay_pending=false;
            if let Err(message)=crate::web_music::unlock() {
                if let Some(engine)=engine.as_deref_mut(){engine.0.renderer.scene_frame_failed=true;}
                if let Some(error)=error.as_deref_mut(){error.0=message;}
                if let Some(next)=next.as_deref_mut(){next.set(crate::resources::VnState::Error);}
            }
            return;
        }
        if music_playback.web_music_replay_pending {
            if let Some(request) = music_playback.last_request.clone() {
                spawn_music(
                    &mut commands,
                    &asset_server,
                    &music_registry,
                    &mut music_entity,
                    &music_volume,
                    &request.file,
                    request.fade_ms,
                );
                info!(
                    "[audio] reprise web demandée, musique relancée ({})",
                    request.file
                );
            } else {
                info!("[audio] reprise web demandée");
            }
            music_playback.web_music_replay_pending = false;
        } else {
            info!("[audio] reprise web demandée");
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn resume_audio_contexts_after_spawn() {
    resume_all_audio_contexts();
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn resume_audio_contexts_after_spawn() {}

pub fn spawn_music(
    commands: &mut Commands,
    asset_server: &AssetServer,
    music_registry: &MusicAssetRegistry,
    music_entity: &mut MusicEntity,
    music_volume: &MusicVolume,
    file: &str,
    fade_ms: Option<u32>,
) {
    let resolved_file = music_registry.resolve(file);
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (commands, asset_server, music_entity, fade_ms);
        rvn_browser_music_play(&resolved_file, music_volume.0 as f64);
        resume_audio_contexts_after_spawn();
        info!("[music] play navigateur {}", resolved_file);
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(ms) = fade_ms {
            let duration = ms as f32 / 1000.0;

            if let Some(old_entity) = music_entity.0.take() {
                commands.entity(old_entity).insert(AudioFade {
                    from_vol: music_volume.0,
                    to_vol: 0.0,
                    duration_secs: duration,
                    elapsed_secs: 0.0,
                    despawn_on_done: true,
                });
                info!("[music] fade-out {:.1}s", duration);
            }

            let source: Handle<AudioSource> = asset_server.load(resolved_file.clone());
            let new_entity = commands
                .spawn((
                    AudioBundle {
                        source,
                        settings: PlaybackSettings::LOOP.with_volume(Volume::new(0.0)),
                    },
                    MusicMarker,
                    AudioFade {
                        from_vol: 0.0,
                        to_vol: music_volume.0,
                        duration_secs: duration,
                        elapsed_secs: 0.0,
                        despawn_on_done: false,
                    },
                ))
                .id();
            music_entity.0 = Some(new_entity);
            resume_audio_contexts_after_spawn();
            info!("[music] fade-in {} ({:.1}s)", resolved_file, duration);
        } else {
            if let Some(entity) = music_entity.0.take() {
                commands.entity(entity).despawn();
            }
            let source: Handle<AudioSource> = asset_server.load(resolved_file.clone());
            let entity = commands
                .spawn((
                    AudioBundle {
                        source,
                        settings: PlaybackSettings::LOOP.with_volume(Volume::new(music_volume.0)),
                    },
                    MusicMarker,
                ))
                .id();
            music_entity.0 = Some(entity);
            resume_audio_contexts_after_spawn();
            info!("[music] play {}", resolved_file);
        }
    }
}

/// Interpole le volume des entités en cours de fade.
/// Séparé de `audio_system` pour pouvoir tourner chaque frame
/// sans dépendre des VnEvents.
pub fn audio_fade_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut AudioFade, &AudioSink,Option<&crate::scene_effects::AudioLease>), (Without<crate::scene_retired::RetiredProjection>,Without<crate::scene_audio::OwnedAudio>)>,
    engine:Option<Res<crate::resources::VnEngine>>,
    state:Option<Res<State<crate::resources::VnState>>>,
) {
    for (entity, mut fade, sink,lease) in query.iter_mut() {
        if lease.is_some() {continue;}
        if !crate::scene_effects::allowed(engine.as_deref(),state.as_deref(),lease.map(|lease|&lease.0)) {continue;}
        fade.elapsed_secs += time.delta_seconds();
        let t = (fade.elapsed_secs / fade.duration_secs).clamp(0.0, 1.0);
        let vol = fade.from_vol + (fade.to_vol - fade.from_vol) * t;
        sink.set_volume(*Volume::new(vol));

        if t >= 1.0 {
            commands.entity(entity).remove::<AudioFade>();
            if fade.despawn_on_done {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Pure Core-authoritative Current gain. Retired payloads have their own exact
/// target and are never selected by the global music alias. Shared sampling is
/// a C12 dependency; no clock or source state is captured here.
pub(crate) fn current_story_music_gain(engine: Option<&crate::resources::VnEngine>)->Result<Option<f32>,String> {
    let Some(engine)=engine else {return Ok(None);};
    if engine.0.state.audio_resource_registry().is_none() {return Ok(None);}
    current_music_gain_from_state(&engine.0.state,engine.0.runtime_epoch()).map(Some)
}
/// Shared by the Root batch preflight and the Current handler. The candidate
/// batch state may be checked before installing any ECS model.
pub(crate) fn current_music_gain_from_state(state:&rvn_core::GameState,runtime_epoch:u64)->Result<f32,String> {
    use rvn_core::scene_resources::{EffectParameters, TargetKind};
    use rvn_core::scene_tasks::TaskOwner;
    let desired=state.music.volume;
    let Some(resources)=state.audio_resource_registry() else {return Ok(desired);};
    let active=state.scene_execution.as_ref().and_then(|execution|execution.owner());
    if !desired.is_finite() || !(0.0..=1.0).contains(&desired) {return Err("Invalid Current music gain".into());}
    if state.music.current_file.is_none() {return Ok(desired);}
    let target=resources.target(&TaskOwner::Game,TargetKind::Music,"music")
        .ok_or_else(||"Current music has no exact Game incarnation".to_string())?;
    let mut gain=None;
    for task in resources.active_effects(active) {
        if &task.target!=target {continue;}
        let effect=task.effect.as_ref().ok_or_else(||"Current music effect has no descriptor".to_string())?;
        effect.validate()?;
        if effect.presentation.is_some() {return Err("Retired presentation cannot target Current music".into());}
        let EffectParameters::AudioFade{from,to,despawn_on_done:false}=&effect.parameters else {return Err("Invalid Current music effect channel".into());};
        if gain.is_some() {return Err("Ambiguous Current music fade projection".into());}
        resources.effect_token(runtime_epoch,&task.key,active)
            .ok_or_else(||"Stale Current music effect descriptor".to_string())?;
        gain=Some(rvn_core::effect_sampling::sample_audio(*from,*to,&effect.clock));
    }
    Ok(gain.unwrap_or(desired))
}

#[cfg(all(test,not(target_arch="wasm32")))]
mod retired_current_contract_tests {
    use super::*;

    fn game(source:&str)->rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        game.step_until_interaction().unwrap();
        game
    }
    fn outgoing(game:&rvn_core::Engine<crate::bevy_renderer::BevyRenderer>)->rvn_core::scene_resources::ResourceToken {
        let execution=game.state.scene_execution.as_ref().unwrap();
        let tasks:Vec<_>=execution.resources.tasks.iter().filter(|task|task.effect.as_ref().is_some_and(|effect|effect.presentation.is_some())).collect();
        assert_eq!(tasks.len(),1);
        execution.resources.effect_token(game.runtime_epoch(),&tasks[0].key,execution.owner()).unwrap()
    }
    fn assets(app:&mut App) {
        app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{watch_for_changes_override:Some(false),..default()}));
        app.init_asset::<Image>();
    }

    fn audio_app(game:rvn_core::Engine<crate::bevy_renderer::BevyRenderer>)->App {
        let mut app=App::new();assets(&mut app);app.init_asset::<AudioSource>().init_asset_loader::<bevy::audio::AudioLoader>().init_asset::<crate::checkpoint_pcm::CheckpointPcm>().insert_resource(crate::resources::VnEngine(game)).init_resource::<MusicEntity>().init_resource::<MusicPlaybackState>().init_resource::<MusicAssetRegistry>().insert_resource(MusicVolume(0.4)).init_resource::<Settings>().init_resource::<crate::resources::ScriptErrorMessage>().insert_resource(NextState::<crate::resources::VnState>::default()).add_event::<VnCommand>().add_systems(Update,audio_system);app
    }
    #[test]
    fn current_music_initial_playback_volume_is_core_f64_sample_before_any_sink_exists() {
        let game=game("scene label A\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\") with fade(4000)\n\"hold\"\n");
        let mut wrapped=crate::resources::VnEngine(game);
        assert_eq!(current_story_music_gain(Some(&wrapped)).unwrap(),Some(0.0));
        wrapped.0.tick_game_frame(1.0,rvn_core::GameFrameReports::default()).unwrap();
        let mut app=audio_app(wrapped.0);crate::checkpoint_sources::install_ready_sources_for_test(app.world_mut()).unwrap();let before=serde_json::to_value(&app.world().resource::<crate::resources::VnEngine>().0.state).unwrap();
        let expected=current_story_music_gain(Some(app.world().resource::<crate::resources::VnEngine>())).unwrap().unwrap();
        assert_eq!(expected,(f64::from(0.4f32)*0.25) as f32);
        app.world_mut().send_event(VnCommand::MusicPlay{file:"theme.ogg".into(),transition:Transition::Fade{duration_ms:4000}});app.world_mut().run_schedule(Update);
        let entity=app.world().resource::<MusicEntity>().0.unwrap();assert!(app.world().get::<AudioSink>(entity).is_none());
        assert_eq!(app.world().get::<PlaybackSettings>(entity).unwrap().volume.get(),expected);
        assert!(app.world().get::<AudioFade>(entity).is_none());assert!(app.world().get::<MusicMarker>(entity).is_some());
        assert_eq!(serde_json::to_value(&app.world().resource::<crate::resources::VnEngine>().0.state).unwrap(),before);
    }
    #[test]
    fn stop_commands_preserve_retired_music_after_exact_role_handoff() {
        let game=game("scene label A\nmusic.volume(0.25)\nmusic.play(\"theme.ogg\")\n\"before\"\nmusic.stop() with fade(2000)\n\"hold\"\n");
        let mut app=audio_app(game);crate::checkpoint_sources::install_ready_sources_for_test(app.world_mut()).unwrap();
        let current=app.world().resource::<MusicEntity>().0.unwrap();
        {let mut game=app.world_mut().resource_mut::<crate::resources::VnEngine>();game.0.advance_dialogue().unwrap();game.0.step_until_interaction().unwrap();}
        let token=outgoing(&app.world().resource::<crate::resources::VnEngine>().0);
        crate::checkpoint_sources::install_ready_sources_for_test(app.world_mut()).unwrap();
        let retired=crate::checkpoint_sources::music_entity_for_test(app.world(),&token.target).unwrap();assert_eq!(retired,current);
        // The actual Core stop removed only the Current role; raw legacy
        // events cannot address this exact independent modern outgoing reader.
        assert!(app.world().get::<MusicMarker>(current).is_none());assert!(app.world().resource::<MusicEntity>().0.is_none());
        app.world_mut().send_event(VnCommand::MusicStop);app.world_mut().send_event(VnCommand::SfxStop{file:"theme.ogg".into()});app.world_mut().send_event(VnCommand::VoiceStop);app.world_mut().run_schedule(Update);
        assert!(app.world().get::<MusicMarker>(current).is_none());assert!(app.world().resource::<MusicEntity>().0.is_none());
        assert!(app.world().get_entity(retired).is_some());assert_eq!(app.world().get::<PlaybackSettings>(retired).unwrap().volume.get(),0.25);
        assert_eq!(&app.world().get::<crate::scene_retired::RetiredProjection>(retired).unwrap().token,&token);
    }
    #[test]
    fn current_music_gain_rejects_ambiguous_exact_effects_before_audio_spawn() {
        let mut game=game("scene label A\nmusic.play(\"theme.ogg\") with fade(4000)\n\"hold\"\n");
        // Settle the genuine valid Core publication before injecting the
        // deliberately ambiguous Current descriptor. A pending publication
        // must block this handler before any new diagnostic or audio spawn.
        let batch=game.renderer.scene_batch.take().expect("actual valid Core batch");
        assert_eq!(batch.runtime_epoch,game.runtime_epoch());
        let execution=game.state.scene_execution.as_ref().unwrap();
        assert_eq!(batch.effects,execution.resources.active_effects(execution.owner()).into_iter().cloned().collect::<Vec<_>>());
        assert!(!game.renderer.scene_batch_pending());
        let execution=game.state.scene_execution.as_mut().unwrap();let effect=execution.resources.tasks.iter().find(|task|matches!(task.effect.as_ref().map(|effect|&effect.parameters),Some(rvn_core::scene_resources::EffectParameters::AudioFade{..}))).unwrap().clone();
        let mut second=effect;second.key.local_id.push_str("/ambiguous");execution.resources.tasks.push(second);
        let mut app=audio_app(game);let before=serde_json::to_value(&app.world().resource::<crate::resources::VnEngine>().0.state).unwrap();
        assert!(current_story_music_gain(Some(app.world().resource::<crate::resources::VnEngine>())).unwrap_err().contains("Ambiguous"));
        app.world_mut().send_event(VnCommand::MusicPlay{file:"theme.ogg".into(),transition:Transition::None});app.world_mut().run_schedule(Update);
        assert!(app.world().resource::<MusicEntity>().0.is_none());assert_eq!(app.world_mut().query::<&PlaybackSettings>().iter(app.world()).count(),0);
        assert!(app.world().resource::<crate::resources::ScriptErrorMessage>().0.contains("Ambiguous"));
        assert_eq!(serde_json::to_value(&app.world().resource::<crate::resources::VnEngine>().0.state).unwrap(),before);
    }
}

// Additive tests of actual Core state -> Current helper -> Root reconcile.
// No AudioSink, native playback, full publication or handoff is simulated.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod music_volume_current_contract_tests {
    use super::*;
    use rvn_core::scene_resources::{ResourceTaskKind, ResourceToken, TargetKind};
    use rvn_core::scene_tasks::TaskOwner;

    fn dialogue(game: &mut rvn_core::Engine<crate::bevy_renderer::BevyRenderer>) -> String {
        match game.step_until_interaction().unwrap().unwrap() {
            rvn_core::Interaction::Dialogue { text, .. } => text,
            other => panic!("Expected dialogue, got {other:?}"),
        }
    }

    fn started(source: &str) -> rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
        let mut game = rvn_core::Engine::from_program(
            rvn_parser::parse_program(source).unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),
            32,
        )
        .unwrap();
        assert_eq!(dialogue(&mut game), "parent");
        game
    }

    fn current_token(game: &rvn_core::Engine<crate::bevy_renderer::BevyRenderer>) -> ResourceToken {
        let execution = game.state.scene_execution.as_ref().unwrap();
        let target = execution
            .resources
            .target(&TaskOwner::Game, TargetKind::Music, "music")
            .unwrap();
        let task = execution
            .resources
            .tasks
            .iter()
            .find(|task| task.kind == ResourceTaskKind::Effect && &task.target == target)
            .unwrap();
        execution
            .resources
            .effect_token(game.runtime_epoch(), &task.key, execution.owner())
            .unwrap()
    }

    fn projection_app(game: rvn_core::Engine<crate::bevy_renderer::BevyRenderer>) -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<crate::accessibility::Accessibility>();
        let volume = game.state.music.volume;
        app.insert_resource(crate::resources::VnEngine(game))
            .insert_resource(MusicVolume(volume))
            .init_resource::<MusicEntity>();
        let entity = app
            .world_mut()
            .spawn((
                MusicMarker,
                PlaybackSettings::LOOP.with_volume(Volume::new(0.99)),
            ))
            .id();
        app.world_mut().resource_mut::<MusicEntity>().0 = Some(entity);
        (app, entity)
    }

    fn project(world: &mut World) {
        let (tasks, epoch, volume, before) = {
            let mut engine = world.resource_mut::<crate::resources::VnEngine>();
            let game = &mut engine.0;
            assert!(!game.renderer.scene_frame_failed);
            let execution = game.state.scene_execution.as_ref().unwrap();
            let tasks: Vec<_> = execution.resources.active_effects(execution.owner())
                .into_iter().cloned().collect();
            // This unit fixture models the settled projection boundary only.
            // Consume the actual Core batch before invoking a handler that must
            // refuse to mix Current while publication is still pending. Full
            // broker/publication is covered by the separate Root world tests.
            let batch = game.renderer.scene_batch.take()
                .expect("the real Core operation must publish before projection");
            assert_eq!(batch.runtime_epoch, game.runtime_epoch());
            assert_eq!(batch.effects, tasks);
            assert_eq!(serde_json::to_value(&batch.state.music).unwrap(),
                serde_json::to_value(&game.state.music).unwrap());
            assert_eq!(&batch.state.scene_execution.as_ref().unwrap().resources,
                &execution.resources);
            assert!(!game.renderer.scene_batch_pending());
            (tasks, game.runtime_epoch(), game.state.music.volume,
                serde_json::to_value(&game.state).unwrap())
        };
        world.resource_mut::<MusicVolume>().0 = volume;
        crate::scene_effects::install(world, tasks, epoch);
        crate::scene_effects::reconcile_for_test(world);
        assert_eq!(
            serde_json::to_value(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(),
            before
        );
    }

    fn gain(world: &World, entity: Entity) -> f32 {
        assert!(world.get::<AudioSink>(entity).is_none());
        world.get::<PlaybackSettings>(entity).unwrap().volume.get()
    }

    #[test]
    fn pending_publication_keeps_current_gain_until_the_real_batch_is_consumed() {
        let mut game = started("scene label P\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\") with fade(4000)\n\"parent\"\n");
        game.tick_game_frame(1.0, rvn_core::GameFrameReports::default()).unwrap();
        assert!(game.renderer.scene_batch_pending());
        let before = serde_json::to_value(&game.state).unwrap();
        let epoch = game.runtime_epoch();
        let tasks = {
            let execution = game.state.scene_execution.as_ref().unwrap();
            execution.resources.active_effects(execution.owner()).into_iter().cloned().collect()
        };
        let (mut app, entity) = projection_app(game);
        let world = app.world_mut();
        crate::scene_effects::install(world, tasks, epoch);
        crate::scene_effects::reconcile_for_test(world);
        assert_eq!(gain(world, entity), 0.99);
        let game = &world.resource::<crate::resources::VnEngine>().0;
        assert!(game.renderer.scene_batch_pending());
        assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
        project(world);
        assert_eq!(gain(world, entity), (f64::from(0.4f32) * 0.25) as f32);
        assert_eq!(serde_json::to_value(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(), before);
    }

    #[test]
    fn current_pending_music_settings_park_and_return_at_exact_core_phase_without_setter() {
        let mut game = started("scene label P\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\") with fade(4000)\n\"parent\"\ncall C\n\"returned\"\njump done\nscene label C\n\"child\"\nreturn\nlabel done\n");
        game.tick_game_frame(1.0, rvn_core::GameFrameReports::default())
            .unwrap();
        let old = current_token(&game);
        let current = old.target.clone();
        let (mut app, entity) = projection_app(game);
        let world = app.world_mut();
        project(world);
        assert_eq!(gain(world, entity), (f64::from(0.4f32) * 0.25) as f32);
        assert_eq!(
            world
                .get::<crate::scene_effects::AudioLease>(entity)
                .unwrap()
                .0,
            old
        );
        {
            let game = &mut world.resource_mut::<crate::resources::VnEngine>().0;
            game.advance_dialogue().unwrap();
            assert_eq!(dialogue(game), "child");
            game.tick_game_frame(0.75, rvn_core::GameFrameReports::default())
                .unwrap();
            let execution = game.state.scene_execution.as_ref().unwrap();
            let task = execution
                .resources
                .tasks
                .iter()
                .find(|task| task.key == old.task.key)
                .unwrap();
            assert!(task.parked);
            assert_eq!(task.effect.as_ref().unwrap().clock.elapsed, 1.0);
        }
        project(world);
        assert_eq!(gain(world, entity), 0.4f32);
        assert!(world
            .get::<crate::scene_effects::AudioLease>(entity)
            .is_none());
        assert!(world.get::<AudioFade>(entity).is_none());
        {
            let game = &mut world.resource_mut::<crate::resources::VnEngine>().0;
            game.advance_dialogue().unwrap();
            assert_eq!(dialogue(game), "returned");
            let fresh = current_token(game);
            assert_eq!(fresh.target, current);
            assert_ne!(fresh.task.revision, old.task.revision);
        }
        project(world);
        assert_eq!(gain(world, entity), (f64::from(0.4f32) * 0.25) as f32);
        assert_eq!(world.resource::<MusicEntity>().0, Some(entity));
        world
            .resource_mut::<crate::resources::VnEngine>()
            .0
            .tick_game_frame(0.25, rvn_core::GameFrameReports::default())
            .unwrap();
        project(world);
        assert_eq!(
            gain(world, entity),
            (f64::from(0.4f32) * (1.25 / 4.0)) as f32
        );
        let game = &world.resource::<crate::resources::VnEngine>().0;
        let execution = game.state.scene_execution.as_ref().unwrap();
        assert!(!execution
            .resources
            .accepts(&old, game.runtime_epoch(), execution.owner()));
    }

    #[test]
    fn current_volume_reconcile_revokes_new_incarnation_fade_without_touching_retired_music() {
        let mut game = started("scene label P\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\") with fade(4000)\n\"parent\"\ncall C\n\"returned\"\njump done\nscene label C\nmusic.play(\"theme.ogg\") with fade(2000)\n\"child before\"\nmusic.volume(0.8)\n\"child after\"\nreturn\nlabel done\n");
        game.tick_game_frame(1.0, rvn_core::GameFrameReports::default())
            .unwrap();
        let parent_old = current_token(&game);
        game.advance_dialogue().unwrap();
        assert_eq!(dialogue(&mut game), "child before");
        game.tick_game_frame(0.25, rvn_core::GameFrameReports::default())
            .unwrap();
        let child_old = current_token(&game);
        assert_ne!(child_old.target, parent_old.target);
        let execution = game.state.scene_execution.as_ref().unwrap();
        let retired_task = execution
            .resources
            .tasks
            .iter()
            .find(|task| {
                task.effect
                    .as_ref()
                    .is_some_and(|effect| effect.presentation.is_some())
            })
            .unwrap()
            .clone();
        let retired_token = execution
            .resources
            .effect_token(game.runtime_epoch(), &retired_task.key, execution.owner())
            .unwrap();
        let (mut app, current) = projection_app(game);
        let world = app.world_mut();
        // This is the post-handoff boundary. Root's own tests exercise transfer
        // of the original player; this test exercises Current helper/isolation.
        let retired = world
            .spawn((
                crate::scene_retired::RetiredProjection {
                    token: retired_token.clone(),
                },
                PlaybackSettings::LOOP.with_volume(Volume::new(0.31)),
            ))
            .id();
        project(world);
        assert_eq!(
            gain(world, current),
            (f64::from(0.4f32) * (0.25 / 2.0)) as f32
        );
        let retired_settings = *world.get::<PlaybackSettings>(retired).unwrap();
        {
            let game = &mut world.resource_mut::<crate::resources::VnEngine>().0;
            game.advance_dialogue().unwrap();
            assert_eq!(dialogue(game), "child after");
            let execution = game.state.scene_execution.as_ref().unwrap();
            assert_eq!(
                execution
                    .resources
                    .target(&TaskOwner::Game, TargetKind::Music, "music"),
                Some(&child_old.target)
            );
            assert!(!execution.resources.accepts(
                &parent_old,
                game.runtime_epoch(),
                execution.owner()
            ));
            assert!(!execution.resources.accepts(
                &child_old,
                game.runtime_epoch(),
                execution.owner()
            ));
            assert_eq!(
                execution
                    .resources
                    .tasks
                    .iter()
                    .find(|task| task.key == retired_task.key)
                    .unwrap(),
                &retired_task
            );
        }
        project(world);
        assert_eq!(gain(world, current), 0.8f32);
        assert!(world.get::<AudioFade>(current).is_none());
        assert!(world
            .get::<crate::scene_effects::AudioLease>(current)
            .is_none());
        assert_eq!(
            world.get::<PlaybackSettings>(retired).unwrap().volume.get(),
            retired_settings.volume.get()
        );
        assert_eq!(
            world.get::<PlaybackSettings>(retired).unwrap().paused,
            retired_settings.paused
        );
        assert_eq!(
            &world
                .get::<crate::scene_retired::RetiredProjection>(retired)
                .unwrap()
                .token,
            &retired_token
        );
        {
            let game = &mut world.resource_mut::<crate::resources::VnEngine>().0;
            game.advance_dialogue().unwrap();
            assert_eq!(dialogue(game), "returned");
        }
        project(world);
        assert_eq!(gain(world, current), 0.8f32);
        assert_eq!(world.resource::<MusicEntity>().0, Some(current));
        // The child-owned outgoing descriptor is now abandoned. Its real ECS
        // lifecycle belongs to the separate Root Retired installer; this test
        // verifies that Current never rebinds the cancelled parent fade.
        assert!(world
            .resource::<crate::resources::VnEngine>()
            .0
            .state
            .scene_execution
            .as_ref()
            .unwrap()
            .resources
            .tasks
            .iter()
            .all(|task| task.key != retired_task.key));
    }
}
