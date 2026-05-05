use bevy::audio::Volume;
use bevy::prelude::*;

use crate::components::{AudioFade, MusicMarker, SfxSource};
use crate::resources::{MusicEntity, MusicPlaybackState, MusicVolume, PendingMusicPlayback};
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
"#)]
extern "C" {
    fn resume_all_audio_contexts();
}

pub fn audio_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut vn_events: EventReader<VnCommand>,
    mut music_entity: ResMut<MusicEntity>,
    mut music_playback: ResMut<MusicPlaybackState>,
    music_volume: Res<MusicVolume>,
    settings: Res<Settings>,
    sfx_query: Query<(Entity, &SfxSource)>,
    sink_query: Query<&AudioSink>,
) {
    if music_volume.is_changed() {
        if let Some(entity) = music_entity.0 {
            if let Ok(sink) = sink_query.get(entity) {
                sink.set_volume(*Volume::new(music_volume.0));
            }
        }
    }

    let cmds: Vec<VnCommand> = vn_events.read().cloned().collect();

    for cmd in cmds {
        match cmd {
            VnCommand::MusicPlay {
                file,
                transition,
                previous: _,
            } => {
                let fade_ms = match &transition {
                    Transition::Fade { duration_ms } | Transition::Dissolve { duration_ms } => {
                        Some(*duration_ms)
                    }
                    Transition::None => None,
                };
                music_playback.last_request = Some(PendingMusicPlayback {
                    file: file.clone(),
                    fade_ms,
                });
                #[cfg(target_arch = "wasm32")]
                {
                    music_playback.web_music_replay_pending = true;
                }
                spawn_music(
                    &mut commands,
                    &asset_server,
                    &mut music_entity,
                    &music_volume,
                    &file,
                    fade_ms,
                );
            }

            VnCommand::MusicStop => {
                if let Some(entity) = music_entity.0.take() {
                    commands.entity(entity).despawn();
                }
                music_playback.last_request = None;
                #[cfg(target_arch = "wasm32")]
                {
                    music_playback.web_music_replay_pending = false;
                }
                info!("[music] stop");
            }

            VnCommand::MusicSetVolume { level } => {
                if let Some(entity) = music_entity.0 {
                    if let Ok(sink) = sink_query.get(entity) {
                        sink.set_volume(*Volume::new(level.clamp(0.0, 1.0)));
                    }
                }
                info!("[music] volume {:.2}", level);
            }

            VnCommand::SfxPlay { file } => {
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
                for (entity, src) in sfx_query.iter() {
                    if src.file == file {
                        commands.entity(entity).despawn();
                        info!("[sfx] stop {}", file);
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
    music_volume: Res<MusicVolume>,
) {
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

        if music_playback.web_music_replay_pending {
            if let Some(request) = music_playback.last_request.clone() {
                spawn_music(
                    &mut commands,
                    &asset_server,
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
fn resume_audio_contexts_after_spawn() {
    resume_all_audio_contexts();
}

#[cfg(not(target_arch = "wasm32"))]
fn resume_audio_contexts_after_spawn() {}

pub fn spawn_music(
    commands: &mut Commands,
    asset_server: &AssetServer,
    music_entity: &mut MusicEntity,
    music_volume: &MusicVolume,
    file: &str,
    fade_ms: Option<u32>,
) {
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

        let source: Handle<AudioSource> = asset_server.load(file.to_string());
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
        info!("[music] fade-in {} ({:.1}s)", file, duration);
    } else {
        if let Some(entity) = music_entity.0.take() {
            commands.entity(entity).despawn();
        }
        let source: Handle<AudioSource> = asset_server.load(file.to_string());
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
        info!("[music] play {}", file);
    }
}

/// Interpole le volume des entités en cours de fade.
/// Séparé de `audio_system` pour pouvoir tourner chaque frame
/// sans dépendre des VnEvents.
pub fn audio_fade_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut AudioFade, &AudioSink)>,
) {
    for (entity, mut fade, sink) in query.iter_mut() {
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
