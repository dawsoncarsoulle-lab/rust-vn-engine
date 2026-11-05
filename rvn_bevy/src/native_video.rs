//! Desktop video playback: bounded asynchronous decoding, streaming audio and
//! timestamped textures. The core owns state; this host reports its clock and
//! never advances narrative statements or runs script callbacks itself.
use crate::{
    resources::{VnEngine, VnState},
    video_audio::{Buffer, VideoAudio, CAPACITY},
    video_decoder::{Chunk, Metadata},
    video_transport::{Packet, Worker,DecodedProof},
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
                ((
                    receive.run_if(crate::scene_publication::projection_ready).before(bevy::ui::UiSystem::Layout),
                    crate::video_visual::cinematic_visibility
                        .run_if(bevy::ecs::schedule::common_conditions::resource_exists::<VnEngine>)
                        .after(bevy::transform::TransformSystem::TransformPropagate)
                        .before(bevy::render::view::VisibilitySystems::VisibilityPropagate),
                )).distributive_run_if(crate::runtime_restart::presentation_available),
            )
            .add_systems(
                Update,
                ((
                    tick.run_if(crate::scene_publication::ready).in_set(crate::scene_publication::HostClockSet),
                    crate::video_visual::cinematic_input.run_if(crate::scene_publication::ready)
                        .run_if(bevy::ecs::schedule::common_conditions::resource_exists::<VnEngine>)
                        .before(crate::systems::player_input_system),
                )).distributive_run_if(crate::runtime_restart::presentation_available),
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
                (qa_drive
                    .after(receive)
                    .after(bevy::transform::TransformSystem::TransformPropagate)).distributive_run_if(crate::runtime_restart::presentation_available),
            );
        }
    }
}
// receive runs after the story, tick before it. Separate schedules avoid a
// cycle and let terminal feedback unblock the story on the following frame.
#[derive(Resource, Default)]
struct Players {
    players: BTreeMap<String, Player>,
    startup_errors: BTreeMap<String, (VideoView, PendingTerminal)>,
    reader: bevy::ecs::event::ManualEventReader<VnCommand>,
    scene_views: Option<Vec<VideoView>>,
    restore_staging:Mutex<Option<atomic_restore::PendingRestore>>,
    restore_authority:Arc<()>,
}
#[path="native_video_restore.rs"]
mod atomic_restore;
#[path="native_video_startup_reports.rs"]
mod startup_reports;
pub(crate) use startup_reports::{prepare_positions,install_positions,PreparedStartupPositions};
pub(crate) use atomic_restore::{prepare_startup,verify_startup,prepare_restore,verify_restore,install_restore,PreparedNativeRestore,NativeAdmission,capture_save15,admits_exact};
#[cfg(test)]
pub(crate) fn init_restore_runtime_for_test(app:&mut App){app.init_asset::<Image>().init_asset::<VideoAudio>().init_resource::<Players>();}
#[cfg(test)]
pub(crate) fn clear_current_for_cold_test(world:&mut World){
    let mut owner=world.remove_resource::<Players>().unwrap();
    for(_,player)in std::mem::take(&mut owner.players){
        if let Some(handle)=&player.audio_handle{world.resource_mut::<Assets<VideoAudio>>().remove(handle.id());}
        world.resource_mut::<Assets<Image>>().remove(player.image.id());
        remove(world,player);
    }
    world.insert_resource(owner);
}
#[cfg(test)]
pub(crate) fn restored_sources_for_test(world:&World)->Vec<(VideoView,Option<VideoAudio>,u32,u32)>{
    world.resource::<Players>().players.values().map(|p|{let image=world.resource::<Assets<Image>>().get(&p.image).unwrap();(p.view.clone(),p.audio.clone(),image.width(),image.height())}).collect()
}
#[cfg(test)]
pub(crate) fn start_current_for_test(world:&mut World)->Result<VideoAudio,String>{
    let view=world.resource::<VnEngine>().0.video_views().map_err(|e|e.to_string())?.into_iter().next().ok_or("No actual current video")?;
    let current=world.resource_mut::<Players>().players.remove(&view.id);
    let mut player=if let Some(player)=current{player}else{start(world,view)?};let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
    loop{pump(world,&mut player)?;if player.metadata.is_some()&&!player.frames.is_empty()&&player.audio.as_ref().is_some_and(|a|a.0.lock().unwrap().len()>0){break;}if std::time::Instant::now()>deadline{return Err("Actual ordinary worker timed out".into());}std::thread::sleep(std::time::Duration::from_millis(1));}
    let audio=player.audio.as_ref().unwrap().clone();world.resource_mut::<Players>().players.insert(player.view.id.clone(),player);Ok(audio)
}
#[cfg(test)]
pub(crate) fn draw_current_for_test(world:&mut World){
    let mut owner=world.remove_resource::<Players>().unwrap();
    for player in owner.players.values_mut(){
        if let Some(audio)=&player.audio{player.position=audio.0.lock().unwrap().position;}
        draw(world,player);
    }
    world.insert_resource(owner);
}
#[cfg(test)]
pub(crate) fn pump_draw_report_current_for_test(world:&mut World)->Result<(),String>{
    let mut owner=world.remove_resource::<Players>().unwrap();
    let result=(||{for player in owner.players.values_mut(){
        pump(world,player)?;
        if let Some(audio)=&player.audio{player.position=audio.0.lock().unwrap().position;}
        draw(world,player);
        if let Some(duration)=player.metadata.as_ref().and_then(|m|m.duration){
            player.position=player.position.min(duration);
            let admitted=world.resource_mut::<VnEngine>().0.video_feedback_from_view(&player.view,Feedback::Position{seconds:player.position,duration}).map_err(|e|e.to_string())?;
            if !admitted{return Err("Actual NativeVideo progress source was refused".into());}
            player.last_report=player.position;
            player.view=world.resource::<VnEngine>().0.video_views().map_err(|e|e.to_string())?.into_iter().find(|v|v.id==player.view.id).ok_or("Actual reported video vanished")?;
        }
    }Ok(())})();
    world.insert_resource(owner);result
}
#[cfg(test)]
pub(crate) fn consume_current_commands_for_test(world:&mut World){
    // Same Event publication and real NativeVideo receiver as the historical
    // legacy step path. No queue is discarded and no source flag is forged.
    for command in world.resource_mut::<VnEngine>().0.renderer.take_pending(){world.send_event(command);}
    receive(world);
}
#[cfg(test)]
pub(crate) fn replace_runtime_for_test(world:&mut World){world.insert_resource(Players::default());}
#[cfg(test)]
pub(crate) fn cancel_runtime_for_test(world:&mut World)->Option<std::sync::Weak<std::sync::atomic::AtomicBool>>{
    let runtime=world.remove_resource::<Players>()?;
    atomic_restore::pending_cancel_for_test(&runtime)
}
pub(crate) fn install_scene(world: &mut World, views: Vec<VideoView>) {
    let mut players = world.remove_resource::<Players>().unwrap_or_default();
    players.startup_errors.retain(|id,(old,_)|views.iter().any(|view|view.id==*id && same_capability(old,view)));
    // Retire old capabilities before any clock/input can run. A compatible
    // player retains its decoder and sound; unrelated state publications do
    // not repeatedly restart playback.
    for (id, mut player) in std::mem::take(&mut players.players) {
        if let Some(view) = views.iter().find(|view|view.id == id && reusable(&player,view)) {
            player.view = view.clone();
            if !crate::video_visual::projection_visible(view) {
                crate::video_visual::retire_visual(world, &mut player.entity, &mut player.caption);
                player.drawn = false;
            }
            players.players.insert(id,player);
        } else { remove(world,player); }
    }
    // Decoder jobs start during reconciliation after all logical models and
    // epochs are installed. Failure retains its own original video token.
    players.scene_views = Some(views);
    world.insert_resource(players);
}
fn reusable(player: &Player, view: &VideoView) -> bool {
    let mut old = player.view.track.clip.clone();
    old.volume = view.track.clip.volume;
    player.view.epoch==view.epoch && player.view.resource_token==view.resource_token
        && old == view.track.clip && (player.last_report-view.track.position).abs() < 1e-8
        && !(player.view.track.finished() && !view.track.finished())
}
struct Frame {
    seconds: f64,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    proof:Option<rvn_media::checkpoint::PictureCursor>,
}
struct Mask {
    worker: Option<Worker>,
    frames: VecDeque<Frame>,
    current: Option<Frame>,
    decoded_source:Option<([u8;32],rvn_media::checkpoint::DecodedSourceProfile)>,
    exact:Option<rvn_media::checkpoint::MaskCheckpoint>,
    metadata: Option<Metadata>,
    eof: bool,
}
struct Player {
    instance:u64,
    view: VideoView,
    worker: Option<Worker>,
    metadata: Option<Metadata>,
    decoded_source:Option<([u8;32],rvn_media::checkpoint::DecodedSourceProfile)>,
    pending: Option<(Packet,Option<DecodedProof>)>,
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
    terminal: PendingTerminal,
}
// Keep the existing terminal/backpressure model; the owning Core/World route
// will admit Startup into the same bounded feedback/nonces protocol.
type PendingTerminal=crate::scene_publication::PendingVideoTerminal;
struct OwnerProjection{instance:u64,views:Vec<VideoView>}
fn owner_projection(world:&World)->Result<OwnerProjection,String>{
 match(world.get_resource::<VnEngine>(),world.get_resource::<crate::debugger_startup::DebugBootstrap>()){
  (Some(game),None)=>Ok(OwnerProjection{instance:game.0.instance_id(),views:game.0.video_views().map_err(|e|e.to_string())?}),
  // Projection data is not a clock or current-Source admission. Keeping it
  // readable also drains bounded decoded frames during native-only staging.
  (None,Some(startup))=>Ok(OwnerProjection{instance:startup.owner.snapshot().instance,views:startup.owner.video_views().map_err(|e|e.to_string())?}),
  _=>Err("Native video requires one actual current owner".into()),
 }
}
fn same_capability(old:&VideoView,new:&VideoView)->bool {
    old.id==new.id && old.epoch==new.epoch && old.resource_token==new.resource_token
}
fn retain_error(player:&mut Player,error:String) {
    player.terminal.retain(Feedback::Error(error));
    player.worker=None;player.mask=None;player.eof=true;
    if let Some(audio)=&player.audio {if let Ok(mut buffer)=audio.0.lock(){buffer.running=false;buffer.ended=true;}}
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
#[cfg(test)]
mod residency_tests {
    use super::*;
    #[test]
    fn native_terminal_error_remains_exact_until_admission_and_never_turns_into_end() {
        let game=crate::video_visual::tests::resident_game_fixture();
        let view=crate::video_visual::tests::embedded_view(&game);
        let mut world=World::new();world.insert_resource(VnEngine(game));
        world.init_resource::<crate::scene_publication::VideoFeedbackQueue>();
        world.init_resource::<crate::resources::ScriptErrorMessage>();world.init_resource::<NextState<VnState>>();
        for _ in 0..rvn_core::scene_resources::MAX_RESOURCES {
            assert!(report(&mut world,&view,Feedback::Position{seconds:0.0,duration:2.0}));
        }
        let accepted=crate::scene_publication::video_reports_for_test(&world);
        let mut pending=PendingTerminal::default();pending.retain(Feedback::Error("worker packet error".into()));
        assert!(!pending.submit(&mut world,&view));assert!(!pending.admitted);
        pending.retain(Feedback::End);
        assert!(matches!(&pending.feedback,Some(Feedback::Error(message)) if message=="worker packet error"));
        let current=crate::scene_publication::video_reports_for_test(&world);
        assert_eq!(current.len(),accepted.len());
        for ((old,old_report),(new,new_report)) in accepted.iter().zip(&current) {
            assert_eq!(old,new);
            match (old_report,new_report) {
                (crate::scene_publication::VideoFeedback::Report(Feedback::Position{seconds:a,duration:b}),crate::scene_publication::VideoFeedback::Report(Feedback::Position{seconds:c,duration:d}))=>assert_eq!((a.to_bits(),b.to_bits()),(c.to_bits(),d.to_bits())),
                _=>panic!("Accepted report changed on terminal backpressure"),
            }
        }
        world.insert_resource(crate::scene_publication::VideoFeedbackQueue::default());
        assert!(pending.submit(&mut world,&view));assert!(pending.admitted);
        assert!(pending.submit(&mut world,&view));
        let delivered=crate::scene_publication::video_reports_for_test(&world);assert_eq!(delivered.len(),1);
        assert_eq!(delivered[0].0.resource_token,view.resource_token);
        assert!(matches!(&delivered[0].1,crate::scene_publication::VideoFeedback::Report(Feedback::Error(message)) if message=="worker packet error"));
    }
    #[test]
    fn parked_native_player_retains_audio_image_and_drains_bounded_frames_before_exact_return() {
        use crate::composed_motion::{InterfaceTarget, InterfaceProjectionTarget};
        let game=crate::video_visual::tests::resident_game_fixture();
        let original=crate::video_visual::tests::embedded_view(&game);
        let binding=original.ui_projection.as_ref().unwrap();
        let mut world=World::new();
        world.init_resource::<Assets<Image>>();
        world.insert_resource(VnEngine(game));
        let parent=world.spawn((InterfaceTarget{screen:"shared".into(),element:"player".into()},
            InterfaceProjectionTarget{interface:binding.interface.clone(),order:binding.order,host_role:binding.host_role})).id();
        let image=world.resource_mut::<Assets<Image>>().add(Image::new_fill(
            Extent3d{width:1,height:1,depth_or_array_layers:1},TextureDimension::D2,
            &[1,2,3,255],TextureFormat::Rgba8UnormSrgb,RenderAssetUsages::default()));
        let audio=VideoAudio(Arc::new(Mutex::new(Buffer::new(0.0))));
        { let mut buffer=audio.0.lock().unwrap();buffer.push(0.0,vec![0.2,0.4]).unwrap();buffer.running=true; }
        let audio_entity=world.spawn_empty().id();
        let mut player=Player{instance:world.resource::<VnEngine>().0.instance_id(),view:original.clone(),worker:None,metadata:None,decoded_source:None,pending:None,
            frames:VecDeque::from([Frame{seconds:0.0,width:1,height:1,rgba:vec![4,5,6,255],proof:None}]),eof:false,
            image:image.clone(),entity:None,caption:None,audio:Some(audio.clone()),audio_entity:Some(audio_entity),audio_handle:None,
            position:0.0,last_report:0.0,age:0.0,drawn:true,last_frame:None,
            mask:Some(Mask{worker:None,frames:VecDeque::from([Frame{seconds:0.0,width:1,height:1,rgba:vec![255,255,255,255],proof:None}]),current:None,decoded_source:None,exact:None,metadata:None,eof:false}),terminal:default()};
        create_visual(&mut world,&mut player);
        let old_visual=player.entity.unwrap();
        let old_caption=player.caption.unwrap();
        assert_eq!(world.get::<Parent>(old_visual).unwrap().get(),parent);
        world.insert_resource(Players{players:BTreeMap::from([("embedded".into(),player)]),..default()});
        { let mut engine=world.resource_mut::<VnEngine>();engine.0.advance_dialogue().unwrap();engine.0.step_until_interaction().unwrap(); }
        let hidden=crate::video_visual::tests::embedded_view(&world.resource::<VnEngine>().0);
        install_scene(&mut world,vec![hidden.clone()]);
        assert!(world.get_entity(old_visual).is_none() && world.get_entity(old_caption).is_none());
        assert!(world.get_entity(parent).is_some() && world.get_entity(audio_entity).is_some());
        let mut players=world.remove_resource::<Players>().unwrap();
        let player=players.players.get_mut("embedded").unwrap();
        assert_eq!(player.image,image);
        assert!(Arc::ptr_eq(&player.audio.as_ref().unwrap().0,&audio.0));
        assert_eq!(player.view.resource_token,original.resource_token);
        draw(&mut world,player);
        assert!(player.entity.is_none() && player.caption.is_none());
        assert!(player.frames.is_empty() && player.mask.as_ref().unwrap().frames.is_empty());
        assert_eq!(player.last_frame.as_ref().unwrap().rgba,vec![4,5,6,255]);
        assert_eq!(world.resource::<Assets<Image>>().get(&image).unwrap().data,vec![1,2,3,255]);
        { let buffer=audio.0.lock().unwrap();assert!(buffer.running && !buffer.ended);assert_eq!(buffer.len(),2); }
        world.insert_resource(players);
        { let mut engine=world.resource_mut::<VnEngine>();engine.0.advance_dialogue().unwrap();engine.0.step_until_interaction().unwrap(); }
        let returned=crate::video_visual::tests::embedded_view(&world.resource::<VnEngine>().0);
        install_scene(&mut world,vec![returned]);
        let mut players=world.remove_resource::<Players>().unwrap();
        let player=players.players.get_mut("embedded").unwrap();
        draw(&mut world,player);
        assert_eq!(world.get::<Parent>(player.entity.unwrap()).unwrap().get(),parent);
        assert_eq!(world.resource::<Assets<Image>>().get(&image).unwrap().data,vec![4,5,6,255]);
        assert!(player.drawn && player.frames.is_empty());
        assert!(Arc::ptr_eq(&player.audio.as_ref().unwrap().0,&audio.0));
        assert!(world.get_entity(audio_entity).is_some());
    }
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
        Some(atomic_restore::ordinary_worker(
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
                    worker: Some(atomic_restore::ordinary_worker(
                        safe_path(
                            &world
                                .resource::<crate::project_paths::ProjectPaths>()
                                .assets,
                            path,
                        )?,
                        seek,
                    )?),
                    frames: VecDeque::new(),
                    current: None,decoded_source:None,exact:None,
                    metadata: None,
                    eof: false,
                })
            })
            .transpose()?
    };
    Ok(Player {
        instance:world.resource::<VnEngine>().0.instance_id(),
        view,
        worker,
        metadata: None,decoded_source:None,
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
        terminal: default(),
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
fn report(world: &mut World, view: &VideoView, feedback: Feedback)->bool {
    crate::scene_publication::queue_video_feedback(world,view,feedback)
}
fn receive(world: &mut World) {
    if !world.contains_resource::<VnEngine>()||world.contains_resource::<crate::debugger_startup::DebugBootstrap>(){return;}
    let mut players = world.remove_resource::<Players>().unwrap();
    let events = players
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
    let views = players.scene_views.take().or(events);
    if let Some(views) = views {
        players.startup_errors.retain(|id,(old,_)|views.iter().any(|view|view.id==*id && same_capability(old,view)));
        let old = std::mem::take(&mut players.players);
        for (id, player) in old {
            if views.iter().any(|view| view.id == id) {
                players.players.insert(id, player);
            } else {
                remove(world, player);
            }
        }
        for view in views {
            if players.startup_errors.get(&view.id).is_some_and(|(old,_)|same_capability(old,&view)) {continue;}
            players.startup_errors.remove(&view.id);
            // Retag unchanged players after an unrelated video's update. Only
            // an actual seek/replacement restarts the decoder and its voice.
            let keep = players.players.get(&view.id).is_some_and(|player|reusable(player,&view));
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
                    let mut terminal=PendingTerminal::default();
                    terminal.retain(Feedback::Error(error.clone()));
                    terminal.submit(world,&view);
                    players.startup_errors.insert(view.id.clone(),(view,terminal));
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
            let Some((packet,proof)) = worker.poll_proven()? else { break };
            match packet {
                Packet::Ready(metadata) =>{mask.decoded_source=match proof{Some(DecodedProof::Ready{asset_sha256,profile})=>Some((asset_sha256,profile)),_=>None};mask.metadata = Some(metadata);},
                Packet::Data(Chunk::Frame {
                    seconds,
                    width,
                    height,
                    rgba,
                }) => mask.frames.push_back(Frame {
                    seconds,
                    width,
                    height,
                    rgba:rgba.clone(),proof:match proof{Some(DecodedProof::Video{ordinal,pts,time_base})=>Some(rvn_media::checkpoint::PictureCursor{ordinal,pts,time_base,rgba_sha256:<sha2::Sha256 as sha2::Digest>::digest(&rgba).into()}),_=>None},
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
            None => player.worker.as_ref().unwrap().poll_proven()?,
        };
        let Some((packet,proof)) = packet else { break };
        match packet {
            Packet::Ready(metadata) => {
                player.decoded_source=match &proof{Some(DecodedProof::Ready{asset_sha256,profile})=>Some((*asset_sha256,profile.clone())),_=>None};
                if metadata.duration.is_none() {
                    return Err("Video has no usable duration metadata".into());
                }
                if metadata.audio && !player.view.track.finished() {
                    let audio = current_audio_source(world,&player.view,player.position)?;
                    if let Some(DecodedProof::Ready{asset_sha256,profile})=&proof{audio.0.lock().map_err(|_|"Native buffer poisoned")?.bind_exact(*asset_sha256,profile.clone())?;}
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
                rgba:rgba.clone(),proof:match proof{Some(DecodedProof::Video{ordinal,pts,time_base})=>Some(rvn_media::checkpoint::PictureCursor{ordinal,pts,time_base,rgba_sha256:<sha2::Sha256 as sha2::Digest>::digest(&rgba).into()}),_=>None},
            }),
            Packet::Data(Chunk::Audio { seconds, samples }) => {
                if let Some(audio) = &player.audio {
                    let mut buffer=audio.0.lock().map_err(|_|"Video audio lock was poisoned")?;
                    if let Some(DecodedProof::Audio{first_sample})=proof{buffer.push_exact(first_sample,samples)?;}else{buffer.push(seconds,samples)?;}
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
    if !crate::video_visual::projection_visible(&player.view) {
        crate::video_visual::retire_visual(world, &mut player.entity, &mut player.caption);
        player.drawn = false;
        return;
    }
    if player
        .entity
        .is_some_and(|entity| world.get_entity(entity).is_none())
    {
        player.entity = None;
        player.caption = None;
        player.drawn = false;
    }
    if player.entity.is_none() {
        if let Some((entity, caption)) =
            crate::video_visual::create(world, &player.view, player.image.clone())
        {
            player.entity = Some(entity);
            player.caption = Some(caption);
        }
    }
}
fn draw(world: &mut World, player: &mut Player) {
    // Authenticated owner projections only. A replacement startup cannot
    // inherit another Engine's image/caption merely by reusing its video ID.
    let Ok(owner)=owner_projection(world)else{return;};
    if owner.instance!=player.instance{return;}
    let Some(actual)=owner.views.into_iter().find(|view|{let mut clip=player.view.track.clip.clone();clip.volume=view.track.clip.volume;same_capability(view,&player.view)&&clip==view.track.clip})else{return;};
    create_visual(world, player);
    let mut frame = None;
    while player
        .frames
        .front()
        .is_some_and(|frame| frame.seconds <= player.position + 0.002)
    {
        frame = player.frames.pop_front();
    }
    // Seeking to a non-frame timestamp while paused still needs a picture.
    if frame.is_none() && player.last_frame.is_none() && player.view.track.playback == Playback::Paused {
        frame = player.frames.pop_front();
    }
    let mut changed = frame.is_some() || !player.drawn;
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
    // Hidden UI must still drain bounded frame/mask queues. Otherwise video
    // back-pressure would eventually stall its Game soundtrack and decoder.
    let Some(entity) = player.entity else { return };
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
    let subtitle = actual.subtitle.unwrap_or_default();
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
    let startup=world.contains_resource::<crate::debugger_startup::DebugBootstrap>();
    let game=world.get_resource::<VnEngine>();
    let scene_game=game.is_some_and(|game|game.0.state.scene_execution.is_some());
    // A Startup projection is never a clock permit. Its real World policy
    // must admit the owner's opaque clock source; the historical Game branch
    // retains its existing state policy. Ambiguous/no owners stay closed.
    let active=!startup&&game.is_some()&&if scene_game {crate::scene_publication::game_active(world)} else {!world
        .resource::<crate::accessibility::Accessibility>()
        .blocked
        && matches!(
            world.resource::<State<VnState>>().get(),
            VnState::Stepping | VnState::Waiting | VnState::Animating
        )};
    let delta = world.resource::<Time>().delta_seconds_f64();
    for (view,terminal) in players.startup_errors.values_mut() {terminal.submit(world,view);}
    for player in players.players.values_mut() {
        if player.terminal.feedback.is_some() {
            player.terminal.submit(world,&player.view);
            draw(world,player);
            continue;
        }
        player.age += delta;
        if let Err(error) = pump(world, player) {
            retain_error(player,error);
            player.terminal.submit(world,&player.view);
            continue;
        }
        if player.audio.as_ref().is_some_and(|audio|audio.0.lock().is_ok_and(|buffer|buffer.playout_abandoned())){retain_error(player,"Video output dropped a decoded right sample before delivery".into());player.terminal.submit(world,&player.view);continue;}
        let owner_active=if startup||scene_game {
            // Existing World entry point owns Startup source + mode/focus /
            // loading admission. Until that bridge is installed it returns
            // false for Startup, so this adapter cannot open a free clock.
            crate::scene_publication::video_clock_allowed(world,&player.view)
                &&!(startup&&world.contains_resource::<VnEngine>())
        }else{active};
        let running = owner_active && player.view.track.playback == Playback::Playing;
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
                    retain_error(player,"Video audio output is unavailable".into());
                    player.terminal.submit(world,&player.view);
                    continue;
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
            if !player.view.track.finished() && (!startup||owner_active) {
                if report(
                    world,
                    &player.view,
                    Feedback::Position {
                        seconds: player.position,
                        duration,
                    },
                ) {player.last_report = player.position;}
            }
            let audio_done = player.audio.as_ref().is_none_or(|audio| {
                audio
                    .0
                    .lock()
                    .map(|buffer| buffer.playout_drained())
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
                    match safe_path(
                        &world
                            .resource::<crate::project_paths::ProjectPaths>()
                            .assets,
                        &view.track.clip.source,
                    )
                    .and_then(|path| atomic_restore::ordinary_worker(path, 0.0))
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
                                .and_then(|path| atomic_restore::ordinary_worker(path, 0.0))
                                {
                                    Ok(worker) => {
                                        player.mask = Some(Mask {
                                            worker: Some(worker),
                                            frames: VecDeque::new(),
                                            current: None,decoded_source:None,exact:None,
                                            metadata: None,
                                            eof: false,
                                        })
                                    }
                                    Err(error) => {
                                        retain_error(player,error);
                                        player.terminal.submit(world,&view);
                                        continue;
                                    }
                                }
                            }
                            report(world,&view,Feedback::Position {
                                    seconds: 0.0,
                                    duration,
                                });
                        }
                        Err(error) => {retain_error(player,error);player.terminal.submit(world,&view);},
                    }
                } else {
                    player.terminal.retain(Feedback::End);
                    player.terminal.submit(world,&player.view);
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

#[cfg(test)]
pub(crate) fn current_pixels_for_test(world:&World)->Vec<u8>{let owner=world.resource::<Players>();let player=owner.players.values().next().unwrap();world.resource::<Assets<Image>>().get(&player.image).unwrap().data.clone()}
#[cfg(test)]
pub(crate) fn finish_actual_current_for_test(world:&mut World)->Result<(),String>{
    let mut owner=world.remove_resource::<Players>().unwrap();
    let result=(||{for player in owner.players.values_mut(){
        if !player.eof||player.audio.as_ref().is_some_and(|a|a.0.lock().map_or(true,|b|!b.ended||b.len()!=0)){return Err("Actual native decoder/reader has not reached EOF".into());}
        draw(world,player);
        if !world.resource_mut::<VnEngine>().0.video_feedback_from_view(&player.view,Feedback::End).map_err(|e|e.to_string())?{return Err("Actual EOF capability was refused".into());}
        player.view=world.resource::<VnEngine>().0.video_views().map_err(|e|e.to_string())?.into_iter().find(|v|v.id==player.view.id).ok_or("Actual terminal source vanished")?;
        if let Some(audio)=&player.audio{audio.0.lock().unwrap().running=false;}
    }Ok(())})();world.insert_resource(owner);result
}

#[cfg(test)]
pub(crate) use atomic_restore::{key_for_test,key_contrasts_for_test};

#[cfg(test)]
pub(crate) fn pending_source_for_test(world:&World)->Option<std::sync::Weak<std::sync::atomic::AtomicBool>>{atomic_restore::pending_cancel_for_test(world.resource::<Players>())}

#[cfg(test)]
#[path="native_startup_tests.rs"]
mod startup_contract_tests;
#[cfg(test)]
#[path="native_video_startup_driver_tests.rs"]
mod scheduled_driver_tests;

fn current_audio_source(world:&World,view:&VideoView,position:f64)->Result<VideoAudio,String>{
 match(world.get_resource::<VnEngine>(),world.get_resource::<crate::debugger_startup::DebugBootstrap>()){
  (Some(_),None)=>crate::video_audio::source_for_current(world,view,position),
  (None,Some(_))=>crate::video_audio::source_for_startup_current(world,view,position),
  _=>Err("Native decoder requires one actual current owner".into()),
 }
}

#[cfg(test)]
#[path="native_video_current_source_tests.rs"]
mod current_source_tests;
