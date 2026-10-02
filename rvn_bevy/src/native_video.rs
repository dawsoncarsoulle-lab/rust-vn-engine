//! Desktop video playback: bounded asynchronous decoding, streaming audio and
//! timestamped textures. The core owns state; this host reports its clock and
//! never advances narrative statements or runs script callbacks itself.
use crate::{
    resources::{VnEngine, VnState},
    video_audio::{Buffer, VideoAudio, CAPACITY},
    video_decoder::{Chunk, Metadata},
    video_transport::{Packet, Worker},
    vn_command::VnCommand,
};
use bevy::{
    audio::{AddAudioSource, AudioSinkPlayback, AudioSourceBundle, PlaybackSettings, Volume},
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use rvn_core::video::{Feedback, Playback, VideoView};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};

pub struct NativeVideoPlugin;
impl Plugin for NativeVideoPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<VideoAudio>()
            .init_resource::<Players>()
            .add_systems(
                PostUpdate,
                (
                    receive.before(bevy::ui::UiSystem::Layout),
                    crate::video_visual::cinematic_visibility
                        .after(bevy::transform::TransformSystem::TransformPropagate)
                        .before(bevy::render::view::VisibilitySystems::VisibilityPropagate),
                ),
            )
            .add_systems(
                Update,
                (
                    tick.before(crate::systems::stepping_system),
                    crate::video_visual::cinematic_input
                        .before(crate::systems::player_input_system),
                ),
            );
        if std::env::var_os("RVN_QA_VIDEO").is_some() {
            assert!(
                std::env::var_os("RVN_QA_OUTPUT")
                    .map(std::path::PathBuf::from)
                    .is_some_and(|path| path.is_dir()),
                "Video QA requires an existing isolated output directory"
            );
            app.insert_resource(bevy::winit::WinitSettings {
                focused_mode: bevy::winit::UpdateMode::Continuous,
                unfocused_mode: bevy::winit::UpdateMode::Continuous,
            })
            .add_systems(
                PostUpdate,
                qa_drive
                    .after(receive)
                    .after(bevy::transform::TransformSystem::TransformPropagate),
            );
        }
    }
}
// receive runs after the story, tick before it. Separate schedules avoid a
// cycle and let terminal feedback unblock the story on the following frame.
#[derive(Resource, Default)]
struct Players {
    players: BTreeMap<String, Player>,
    reader: bevy::ecs::event::ManualEventReader<VnCommand>,
}
struct Frame {
    seconds: f64,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}
struct Mask {
    worker: Option<Worker>,
    frames: VecDeque<Frame>,
    current: Option<Frame>,
    metadata: Option<Metadata>,
    eof: bool,
}
struct Player {
    view: VideoView,
    worker: Option<Worker>,
    metadata: Option<Metadata>,
    pending: Option<Packet>,
    frames: VecDeque<Frame>,
    eof: bool,
    image: Handle<Image>,
    entity: Option<Entity>,
    caption: Option<Entity>,
    audio: Option<VideoAudio>,
    audio_entity: Option<Entity>,
    audio_handle: Option<Handle<VideoAudio>>,
    position: f64,
    last_report: f64,
    age: f64,
    drawn: bool,
    mask: Option<Mask>,
    last_frame: Option<Frame>,
}
fn safe_path(root: &std::path::Path, path: &str) -> Result<std::path::PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("Cannot open video asset directory: {error}"))?;
    let candidate = root
        .join(path)
        .canonicalize()
        .map_err(|error| format!("Cannot open video '{path}': {error}"))?;
    if !candidate.starts_with(&root) {
        return Err("Video symlink escapes the project asset directory".into());
    }
    Ok(candidate)
}
fn start(world: &mut World, view: VideoView) -> Result<Player, String> {
    let mut image = Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::render::texture::ImageSampler::linear();
    let image = world.resource_mut::<Assets<Image>>().add(image);
    // A loaded completed clip still needs its last picture, but must never
    // create another soundtrack or emit its completion handler again.
    let decode = !view.track.finished()
        || (view.track.playback == Playback::Ended && view.track.clip.keep_last_frame);
    let seek = if view.track.finished() {
        (view.track.position - 0.25).max(0.0)
    } else {
        view.track.position
    };
    let worker = if decode {
        Some(Worker::spawn(
            safe_path(
                &world
                    .resource::<crate::project_paths::ProjectPaths>()
                    .assets,
                &view.track.clip.source,
            )?,
            seek,
        )?)
    } else {
        None
    };
    let position = view.track.position;
    let mask = if !decode {
        None
    } else {
        view.track
            .clip
            .mask
            .as_ref()
            .map(|path| {
                Ok::<_, String>(Mask {
                    worker: Some(Worker::spawn(
                        safe_path(
                            &world
                                .resource::<crate::project_paths::ProjectPaths>()
                                .assets,
                            path,
                        )?,
                        seek,
                    )?),
                    frames: VecDeque::new(),
                    current: None,
                    metadata: None,
                    eof: false,
                })
            })
            .transpose()?
    };
    Ok(Player {
        view,
        worker,
        metadata: None,
        pending: None,
        frames: VecDeque::new(),
        eof: false,
        image,
        entity: None,
        caption: None,
        audio: None,
        audio_entity: None,
        audio_handle: None,
        position,
        last_report: position,
        age: 0.0,
        drawn: false,
        mask,
        last_frame: None,
    })
}
fn remove(world: &mut World, mut player: Player) {
    if let Some(audio) = player.audio.take() {
        if let Ok(mut buffer) = audio.0.lock() {
            buffer.running = false;
            buffer.ended = true;
        }
    }
    for entity in [player.entity, player.caption, player.audio_entity]
        .into_iter()
        .flatten()
    {
        if let Some(entity) = world.get_entity_mut(entity) {
            entity.despawn_recursive();
        }
    }
    // Dropping the channel cancels back-pressure waits without blocking UI.
    player.worker = None;
}
fn report(world: &mut World, view: &VideoView, feedback: Feedback) {
    let waiting = world
        .resource::<VnEngine>()
        .0
        .state
        .videos
        .waiting
        .is_some();
    let result = world
        .resource_mut::<VnEngine>()
        .0
        .video_feedback(view.epoch, &view.id, feedback);
    if let Err(problem) = result {
        world
            .resource_mut::<crate::resources::ScriptErrorMessage>()
            .0 = problem.to_string();
        world
            .resource_mut::<NextState<VnState>>()
            .set(VnState::Error);
    }
    let commands = world.resource_mut::<VnEngine>().0.renderer.take_pending();
    for command in commands {
        world.send_event(command);
    }
    if waiting
        && world
            .resource::<VnEngine>()
            .0
            .state
            .videos
            .waiting
            .is_none()
    {
        if *world.resource::<State<VnState>>().get() == VnState::Menu {
            world
                .resource_mut::<crate::resources::MenuState>()
                .return_to = Some(VnState::Stepping);
        } else {
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
    }
}
fn receive(world: &mut World) {
    let mut players = world.remove_resource::<Players>().unwrap();
    let views = players
        .reader
        .read(world.resource::<Events<VnCommand>>())
        .filter_map(|event| {
            if let VnCommand::Videos(views) = event {
                Some(views.clone())
            } else {
                None
            }
        })
        .last();
    if let Some(views) = views {
        let old = std::mem::take(&mut players.players);
        for (id, player) in old {
            if views.iter().any(|view| view.id == id) {
                players.players.insert(id, player);
            } else {
                remove(world, player);
            }
        }
        for view in views {
            // Retag unchanged players after an unrelated video's update. Only
            // an actual seek/replacement restarts the decoder and its voice.
            let keep = players.players.get(&view.id).is_some_and(|player| {
                let mut old = player.view.track.clip.clone();
                old.volume = view.track.clip.volume;
                old == view.track.clip
                    && (player.last_report - view.track.position).abs() < 1e-8
                    && !(player.view.track.finished() && !view.track.finished())
            });
            if keep {
                let player = players.players.get_mut(&view.id).unwrap();
                player.view = view;
                continue;
            }
            if let Some(player) = players.players.remove(&view.id) {
                remove(world, player);
            }
            match start(world, view.clone()) {
                Ok(player) => {
                    players.players.insert(view.id.clone(), player);
                }
                Err(error) => {
                    report(world, &view, Feedback::Error(error.clone()));
                    warn!("{error}");
                }
            }
        }
    }
    world.insert_resource(players);
}
fn pump(world: &mut World, player: &mut Player) -> Result<(), String> {
    if let Some(mask) = &mut player.mask {
        for _ in 0..64 {
            if mask.frames.len() >= 3 || mask.eof {
                break;
            }
            let Some(worker) = &mask.worker else { break };
            let Some(packet) = worker.poll()? else { break };
            match packet {
                Packet::Ready(metadata) => mask.metadata = Some(metadata),
                Packet::Data(Chunk::Frame {
                    seconds,
                    width,
                    height,
                    rgba,
                }) => mask.frames.push_back(Frame {
                    seconds,
                    width,
                    height,
                    rgba,
                }),
                Packet::Data(Chunk::Audio { .. }) => {} // A mask never produces a second soundtrack.
                Packet::Data(Chunk::End) => {
                    mask.eof = true;
                    mask.worker = None;
                }
                Packet::Error(error) => return Err(format!("Video mask: {error}")),
            }
        }
    }
    if let (Some(metadata), Some(mask)) = (&player.metadata, &player.mask) {
        if let Some(other) = &mask.metadata {
            if (metadata.width, metadata.height) != (other.width, other.height) {
                return Err("Video mask dimensions must match the video".into());
            }
            if metadata
                .duration
                .zip(other.duration)
                .is_some_and(|(source, mask)| mask + 0.1 < source)
            {
                return Err("Video mask ends before the video".into());
            }
        }
    }
    if player.eof || player.worker.is_none() {
        return Ok(());
    }
    for _ in 0..64 {
        if player.frames.len() >= 3 {
            return Ok(());
        }
        if player.audio.as_ref().is_some_and(|audio| {
            audio
                .0
                .lock()
                .map(|buffer| buffer.len() > CAPACITY / 2)
                .unwrap_or(true)
        }) {
            return Ok(());
        }
        let packet = match player.pending.take() {
            Some(packet) => Some(packet),
            None => player.worker.as_ref().unwrap().poll()?,
        };
        let Some(packet) = packet else { break };
        match packet {
            Packet::Ready(metadata) => {
                if metadata.duration.is_none() {
                    return Err("Video has no usable duration metadata".into());
                }
                if metadata.audio && !player.view.track.finished() {
                    let audio = VideoAudio(Arc::new(Mutex::new(Buffer::new(player.position))));
                    let handle = world
                        .resource_mut::<Assets<VideoAudio>>()
                        .add(audio.clone());
                    let entity = world
                        .spawn(AudioSourceBundle {
                            source: handle.clone(),
                            settings: PlaybackSettings {
                                volume: Volume::new(player.view.track.clip.volume),
                                ..default()
                            },
                        })
                        .id();
                    player.audio = Some(audio);
                    player.audio_handle = Some(handle);
                    player.audio_entity = Some(entity);
                }
                player.metadata = Some(metadata);
            }
            Packet::Data(Chunk::Frame {
                seconds,
                width,
                height,
                rgba,
            }) => player.frames.push_back(Frame {
                seconds,
                width,
                height,
                rgba,
            }),
            Packet::Data(Chunk::Audio { seconds, samples }) => {
                if let Some(audio) = &player.audio {
                    audio
                        .0
                        .lock()
                        .map_err(|_| "Video audio lock was poisoned")?
                        .push(seconds, samples)?;
                }
            }
            Packet::Data(Chunk::End) => {
                player.eof = true;
                player.worker = None;
                if let Some(audio) = &player.audio {
                    audio
                        .0
                        .lock()
                        .map_err(|_| "Video audio lock was poisoned")?
                        .ended = true;
                }
                break;
            }
            Packet::Error(error) => return Err(error),
        }
    }
    Ok(())
}
fn create_visual(world: &mut World, player: &mut Player) {
    if player
        .entity
        .is_some_and(|entity| world.get_entity(entity).is_none())
    {
        player.entity = None;
        player.caption = None;
    }
    if player.entity.is_none() {
        if let Some((entity, caption)) =
            crate::video_visual::create(world, &player.view.track.clip, player.image.clone())
        {
            player.entity = Some(entity);
            player.caption = Some(caption);
        }
    }
}
fn draw(world: &mut World, player: &mut Player) {
    create_visual(world, player);
    let Some(entity) = player.entity else { return };
    let mut frame = None;
    while player
        .frames
        .front()
        .is_some_and(|frame| frame.seconds <= player.position + 0.002)
    {
        frame = player.frames.pop_front();
    }
    // Seeking to a non-frame timestamp while paused still needs a picture.
    if frame.is_none() && !player.drawn && player.view.track.playback == Playback::Paused {
        frame = player.frames.pop_front();
    }
    let mut changed = frame.is_some();
    if frame.is_some() {
        player.last_frame = frame;
    }
    if let Some(mask) = &mut player.mask {
        while mask
            .frames
            .front()
            .is_some_and(|frame| frame.seconds <= player.position + 0.002)
        {
            mask.current = mask.frames.pop_front();
            changed = true;
        }
        if mask.current.is_none() && player.view.track.playback == Playback::Paused {
            mask.current = mask.frames.pop_front();
            changed = true;
        }
        if mask.current.is_none() {
            return;
        }
    }
    if changed {
        if let Some(frame) = &player.last_frame {
            let mut pixels = frame.rgba.clone();
            if let Some(mask) = player.mask.as_ref().and_then(|mask| mask.current.as_ref()) {
                if (frame.width, frame.height) != (mask.width, mask.height) {
                    return;
                }
                apply_mask(&mut pixels, &mask.rgba);
            }
            if let Some(image) = world.resource_mut::<Assets<Image>>().get_mut(&player.image) {
                image.resize(Extent3d {
                    width: frame.width,
                    height: frame.height,
                    depth_or_array_layers: 1,
                });
                image.data = pixels;
            }
            if let Some(mut image) = world.get_mut::<UiImage>(entity) {
                image.texture = player.image.clone();
            } else if let Some(mut texture) = world.get_mut::<Handle<Image>>(entity) {
                *texture = player.image.clone();
            }
            player.drawn = true;
        }
    }
    let subtitle = world
        .resource::<VnEngine>()
        .0
        .video_views()
        .ok()
        .and_then(|views| views.into_iter().find(|view| view.id == player.view.id))
        .and_then(|view| view.subtitle)
        .unwrap_or_default();
    if let Some(caption) = player.caption {
        if let Some(mut text) = world.get_mut::<Text>(caption) {
            text.sections[0].value = subtitle.clone();
        }
        if let Some(mut visibility) = world.get_mut::<Visibility>(caption) {
            *visibility = if subtitle.is_empty() {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
    }
    let terminal = player.view.track.finished();
    if let Some(mut visibility) = world.get_mut::<Visibility>(entity) {
        *visibility = if terminal
            && !player.view.track.clip.keep_last_frame
            && player.view.track.clip.fallback.is_none()
        {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    if player.view.track.playback == Playback::Failed {
        if let Some(path) = &player.view.track.clip.fallback {
            let texture = world.resource::<AssetServer>().load::<Image>(path.clone());
            if let Some(mut image) = world.get_mut::<UiImage>(entity) {
                image.texture = texture;
            } else if let Some(mut image) = world.get_mut::<Handle<Image>>(entity) {
                *image = texture;
            }
        }
    }
}
fn apply_mask(pixels: &mut [u8], mask: &[u8]) {
    for (color, mask) in pixels.chunks_exact_mut(4).zip(mask.chunks_exact(4)) {
        // Integer luminance, preserving any source alpha. White is opaque.
        let alpha =
            (54 * u32::from(mask[0]) + 183 * u32::from(mask[1]) + 19 * u32::from(mask[2]) + 128)
                / 256;
        color[3] = ((u32::from(color[3]) * alpha + 127) / 255) as u8;
    }
}
fn tick(world: &mut World) {
    let mut players = world.remove_resource::<Players>().unwrap();
    let active = !world
        .resource::<crate::accessibility::Accessibility>()
        .blocked
        && matches!(
            world.resource::<State<VnState>>().get(),
            VnState::Stepping | VnState::Waiting | VnState::Animating
        );
    let delta = world.resource::<Time>().delta_seconds_f64();
    for player in players.players.values_mut() {
        player.age += delta;
        if let Err(error) = pump(world, player) {
            player.worker = None;
            player.mask = None;
            player.eof = true;
            if let Some(audio) = &player.audio {
                if let Ok(mut buffer) = audio.0.lock() {
                    buffer.running = false;
                    buffer.ended = true;
                }
            }
            report(world, &player.view, Feedback::Error(error));
            continue;
        }
        let running = active && player.view.track.playback == Playback::Playing;
        if let Some(audio) = &player.audio {
            if let Ok(mut buffer) = audio.0.lock() {
                buffer.running = running;
                player.position = buffer.position;
            }
            if let Some(entity) = player.audio_entity {
                if let Some(sink) = world.get::<AudioSink>(entity) {
                    sink.set_volume(
                        player.view.track.clip.volume
                            * world
                                .resource::<crate::systems::settings_menu::Settings>()
                                .sfx_volume,
                    );
                    if running {
                        sink.play();
                    } else {
                        sink.pause();
                    }
                } else if player.age > 5.0 && running {
                    report(
                        world,
                        &player.view,
                        Feedback::Error("Video audio output is unavailable".into()),
                    );
                    player.worker = None;
                    player.eof = true;
                }
            }
        } else if running && player.metadata.is_some() && (!player.frames.is_empty() || player.eof)
        {
            player.position += delta;
        }
        if let Some(duration) = player
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.duration)
        {
            player.position = player.position.min(duration);
            if !player.view.track.finished() {
                report(
                    world,
                    &player.view,
                    Feedback::Position {
                        seconds: player.position,
                        duration,
                    },
                );
                player.last_report = player.position;
            }
            let audio_done = player.audio.as_ref().is_none_or(|audio| {
                audio
                    .0
                    .lock()
                    .map(|buffer| buffer.len() == 0)
                    .unwrap_or(false)
            });
            if running
                && player.eof
                && audio_done
                && (player.position >= duration - 0.02 || player.frames.is_empty())
            {
                if player.view.track.clip.looping {
                    if let Some(audio) = &player.audio {
                        if let Ok(mut buffer) = audio.0.lock() {
                            buffer.running = false;
                        }
                    }
                    // A loop is a new seek, never a terminal narrative event.
                    let view = player.view.clone();
                    let id = view.id.clone();
                    let epoch = view.epoch;
                    match safe_path(
                        &world
                            .resource::<crate::project_paths::ProjectPaths>()
                            .assets,
                        &view.track.clip.source,
                    )
                    .and_then(|path| Worker::spawn(path, 0.0))
                    {
                        Ok(worker) => {
                            player.worker = Some(worker);
                            player.eof = false;
                            player.metadata = None;
                            player.frames.clear();
                            player.position = 0.0;
                            player.last_report = 0.0;
                            player.last_frame = None;
                            player.drawn = false;
                            if let Some(entity) = player.audio_entity.take() {
                                world.entity_mut(entity).despawn_recursive();
                            }
                            player.audio = None;
                            player.audio_handle = None;
                            if let Some(mask) = &view.track.clip.mask {
                                match safe_path(
                                    &world
                                        .resource::<crate::project_paths::ProjectPaths>()
                                        .assets,
                                    mask,
                                )
                                .and_then(|path| Worker::spawn(path, 0.0))
                                {
                                    Ok(worker) => {
                                        player.mask = Some(Mask {
                                            worker: Some(worker),
                                            frames: VecDeque::new(),
                                            current: None,
                                            metadata: None,
                                            eof: false,
                                        })
                                    }
                                    Err(error) => {
                                        player.worker = None;
                                        player.mask = None;
                                        report(world, &view, Feedback::Error(error));
                                        continue;
                                    }
                                }
                            }
                            let _ = world.resource_mut::<VnEngine>().0.video_feedback(
                                epoch,
                                &id,
                                Feedback::Position {
                                    seconds: 0.0,
                                    duration,
                                },
                            );
                        }
                        Err(error) => report(world, &view, Feedback::Error(error)),
                    }
                } else {
                    report(world, &player.view, Feedback::End);
                }
            }
        }
        draw(world, player);
    }
    world.insert_resource(players);
}

fn qa_drive(world: &mut World) {
    #[derive(Resource)]
    struct Run {
        started: std::time::Instant,
        phase: usize,
        entered: f64,
        saved: Option<rvn_core::save::SaveData>,
        paused: f64,
        cinematic_seen: bool,
    }
    if !world.contains_resource::<Run>() {
        world.insert_resource(Run {
            started: std::time::Instant::now(),
            phase: 0,
            entered: 0.0,
            saved: None,
            paused: 0.0,
            cinematic_seen: false,
        });
    }
    let now = world.resource::<Run>().started.elapsed().as_secs_f64();
    assert!(
        now < 60.0,
        "Video QA timed out at phase {}, pc {}, state {:?}, videos {:?}",
        world.resource::<Run>().phase,
        world.resource::<VnEngine>().0.state.pc,
        world.resource::<State<VnState>>().get(),
        world.resource::<VnEngine>().0.state.videos
    );
    assert_ne!(
        *world.resource::<State<VnState>>().get(),
        VnState::Error,
        "{}",
        world.resource::<crate::resources::ScriptErrorMessage>().0
    );
    assert!(
        !world
            .resource::<VnEngine>()
            .0
            .state
            .videos
            .tracks
            .values()
            .any(|track| track.playback == Playback::Failed),
        "{:?}",
        world.resource::<VnEngine>().0.state.videos
    );
    if *world.resource::<State<VnState>>().get() == VnState::TitleScreen {
        if now > 0.5 {
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
        return;
    }
    if world
        .resource::<VnEngine>()
        .0
        .state
        .videos
        .waiting
        .as_deref()
        == Some("cinema")
    {
        assert!(world
            .resource::<VnEngine>()
            .0
            .current_interaction()
            .unwrap()
            .is_none());
        world.resource_mut::<Run>().cinematic_seen = true;
    }
    if *world.resource::<State<VnState>>().get() != VnState::Waiting
        || now - world.resource::<Run>().entered < 0.35
    {
        return;
    }
    let path = std::path::PathBuf::from(std::env::var_os("RVN_QA_OUTPUT").unwrap());
    let capture = |world: &mut World, name: &str| {
        let window = world
            .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
            .single(world);
        world
            .resource_mut::<bevy::render::view::screenshot::ScreenshotManager>()
            .save_screenshot_to_disk(window, path.join(name))
            .unwrap();
    };
    let advance = |world: &mut World| {
        world
            .resource_mut::<VnEngine>()
            .0
            .advance_dialogue()
            .unwrap();
        world
            .resource_mut::<NextState<VnState>>()
            .set(VnState::Stepping);
    };
    let phase = world.resource::<Run>().phase;
    let text = world
        .resource::<VnEngine>()
        .0
        .current_interaction()
        .unwrap();
    let Some(rvn_core::Interaction::Dialogue { text, .. }) = text else {
        return;
    };
    let rendered: String = world
        .query_filtered::<&Text, With<crate::components::DialogueText>>()
        .iter(world)
        .flat_map(|text| text.sections.iter().map(|part| part.value.as_str()))
        .collect();
    assert_eq!(
        rendered, text,
        "Video waits must not leave stale dialogue onscreen"
    );
    match phase {
        0 => {
            let track = &world.resource::<VnEngine>().0.state.videos.tracks["intro"];
            if track.position < 0.25 {
                return;
            }
            let player = &world.resource::<Players>().players["intro"];
            assert!(player.drawn);
            assert!(world
                .get::<AudioSink>(player.audio_entity.unwrap())
                .is_some());
            let save = rvn_core::save::SaveData::from_state(
                &world.resource::<VnEngine>().0.state,
                1,
                "Mid video".into(),
                "main.rvn".into(),
            );
            world.resource_mut::<Run>().saved = Some(save);
            capture(world, "01_playing.png");
            advance(world);
        }
        1 => {
            let track = &world.resource::<VnEngine>().0.state.videos.tracks["intro"];
            assert_eq!(track.playback, Playback::Paused);
            let position = track.position;
            world.resource_mut::<Run>().paused = position;
            capture(world, "02_paused.png");
        }
        2 => {
            assert!(
                (world.resource::<VnEngine>().0.state.videos.tracks["intro"].position
                    - world.resource::<Run>().paused)
                    .abs()
                    < 0.01
            );
            let data = world.resource::<Run>().saved.clone().unwrap();
            world.resource_mut::<VnEngine>().0.load_data(data).unwrap();
            for command in world.resource_mut::<VnEngine>().0.renderer.take_pending() {
                world.send_event(command);
            }
        }
        3 => {
            assert_eq!(
                world.resource::<Players>().players.len(),
                1,
                "Loading must not duplicate players or voices"
            );
            assert!(world.resource::<Players>().players["intro"].drawn);
            capture(world, "03_loaded.png");
            advance(world);
        }
        4 => {
            assert_eq!(
                world.resource::<VnEngine>().0.state.videos.tracks["intro"].playback,
                Playback::Paused
            );
            advance(world);
        }
        5 => {
            let track = &world.resource::<VnEngine>().0.state.videos.tracks["intro"];
            assert!((track.position - 0.8).abs() < 0.001);
            assert_eq!(track.playback, Playback::Paused);
            assert!(world.resource::<Players>().players["intro"].drawn);
            capture(world, "04_seeked.png");
            advance(world);
        }
        6 => {
            assert_eq!(
                world.resource::<VnEngine>().0.state.videos.tracks["intro"].playback,
                Playback::Ended
            );
            assert_eq!(
                world.resource::<VnEngine>().0.state.vars["completed"],
                rvn_parser::Value::Int(1)
            );
            capture(world, "05_finished.png");
            world.resource_mut::<Run>().saved = Some(rvn_core::save::SaveData::from_state(
                &world.resource::<VnEngine>().0.state,
                1,
                "Ended video".into(),
                "main.rvn".into(),
            ));
            advance(world);
        }
        7 => {
            assert!(world.resource::<Players>().players.is_empty());
            capture(world, "06_closed.png");
            assert!(world.resource_mut::<VnEngine>().0.rollback());
            assert!(world.resource_mut::<VnEngine>().0.rollback());
            for command in world.resource_mut::<VnEngine>().0.renderer.take_pending() {
                world.send_event(command);
            }
            world
                .resource_mut::<NextState<VnState>>()
                .set(VnState::Stepping);
        }
        8 => {
            let player = &world.resource::<Players>().players["intro"];
            if !player.drawn {
                return;
            }
            assert!(
                player.audio.is_none(),
                "Rollback of an ended clip must not create a voice"
            );
            assert_eq!(
                world.resource::<VnEngine>().0.state.vars["completed"],
                rvn_parser::Value::Int(1)
            );
            capture(world, "07_rollback_last_frame.png");
            advance(world);
        }
        9 => {
            assert!(world.resource::<Players>().players.is_empty());
            let data = world.resource::<Run>().saved.clone().unwrap();
            world.resource_mut::<VnEngine>().0.load_data(data).unwrap();
            for command in world.resource_mut::<VnEngine>().0.renderer.take_pending() {
                world.send_event(command);
            }
        }
        10 => {
            let player = &world.resource::<Players>().players["intro"];
            if !player.drawn {
                return;
            }
            assert!(player.audio.is_none());
            assert_eq!(
                world.resource::<VnEngine>().0.state.vars["completed"],
                rvn_parser::Value::Int(1)
            );
            capture(world, "08_loaded_last_frame.png");
            advance(world);
        }
        11 => {
            assert!(world.resource::<Players>().players.is_empty());
            advance(world);
        }
        12 => {
            assert!(world.resource::<Run>().cinematic_seen);
            assert!(world
                .resource::<VnEngine>()
                .0
                .state
                .videos
                .waiting
                .is_none());
            capture(world, "09_after_cinematic.png");
            std::fs::write(path.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"result":"pass","scenario":"native VP8/Vorbis video: real RGBA textures and streaming AudioSink, pause clock stability, save/load with one voice, seek preview, end event once, removal, last-frame load and rollback without another voice, blocking cinematic","windows_validated":false})).unwrap()).unwrap();
        }
        _ => {
            world.send_event(bevy::app::AppExit::Success);
            return;
        }
    }
    let mut run = world.resource_mut::<Run>();
    run.phase += 1;
    run.entered = now;
}
