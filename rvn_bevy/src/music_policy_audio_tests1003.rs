//! D-CATALOG-DYNAMIC-VOLUME: the independent UE clip, actual Bevy publication,
//! real AudioPlugin queue and public AudioSink. Hardware test never constructs
//! a sink or applies its gain itself. Offline PCM prerequisite is separate.
use super::*;
use bevy::audio::{AddAudioSource, AudioPlugin, AudioSinkPlayback, Decodable, Source};
use crate::resources::{VnEngine, VnState};
use rvn_graph::{GraphKind, NodeKind, ValueType};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::PathBuf, sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};
#[path="../../rvn_core/tests/support/strict_native_consumer.rs"]
mod strict_native_consumer;

const ASSET: &str = "music/policy1003.wav";
const CLIP_SHA: &str = "548C5E07E49F7BBDDE4DA7E9665CEC76059D72B290108D1E683A2B445C76844D";
const LEVELS: [f32; 7] = [1., 0.25, 0.625, 0.5, 0., 1., 0.75];
const TOTAL_SAMPLES: u64 = 20 * 48_000 * 2;

fn independent_wave() -> Vec<u8> {
    let path=PathBuf::from(std::env::var_os("RVN_MUSIC_POLICY1003_WAV")
        .expect("Root must bind the independent20s WAV input; no generated substitute"));
    let bytes=fs::read(&path).expect("actual authorized independent WAV input");
    assert_eq!(bytes.len(),3_840_044);
    assert_eq!(format!("{:X}",Sha256::digest(&bytes)),CLIP_SHA,"UE/Rust must use the exact same independent encoded asset");
    bytes
}
fn cursor(reader:&CheckpointPcm)->rvn_core::audio_checkpoint::AudioCheckpoint {
    reader.gate().snapshot(&[reader],|mut rows|Ok(rows.remove(0))).unwrap()
}
fn total_consumed(value:&rvn_core::audio_checkpoint::AudioCheckpoint)->u64 {
    value.loops_completed.checked_mul(value.total_samples).unwrap().checked_add(value.next_sample).unwrap()
}
fn output_directory(name:&str)->PathBuf {
    static NEXT:AtomicU64=AtomicU64::new(1);
    let root=PathBuf::from(std::env::var_os("RVN_NATIVE_TYPEWRITER_FIXTURES")
        .expect("existing isolated private audio/source output directory"));
    let result=root.join(format!("music1003-{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    fs::create_dir_all(&result).unwrap();result
}
fn actual_native_program(dir:&PathBuf)->rvn_parser::ParsedProgram {
    // Exactly one authored Play. Every volume write uses the real Float GET,
    // rather than a source-less header or an externally overwritten VM value.
    let mut source=String::from("scene label start\nset sampled=random(1,9)\nset play_calls=0\nset volume_writes=0\nset level=1.0\nmusic.play(\"policy1003.wav\")\nset play_calls=play_calls+1\n\"volume0\"\n");
    for(index,level)in LEVELS.iter().enumerate().skip(1) {
        source.push_str(&format!("set level={level:.3}\nmusic.volume(level)\nset volume_writes=volume_writes+1\n\"volume{index}\"\n"));
    }
    source.push_str("return\nscene label finished\n\"finished\"\nreturn\n");
    let file=dir.join("authored.rvn");fs::write(&file,&source).unwrap();
    let original=rvn_parser::parse_file_program_with_uses(&file).unwrap();
    let mut graphs=rvn_graph::import_script(&original.script).unwrap();
    let graph=graphs.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
    graph.enable_native_frame_preparation().unwrap();graph.refresh_variable_usage_bindings().unwrap();
    let volumes=graph.nodes.values().filter(|node|node.kind==NodeKind::MusicVolume).collect::<Vec<_>>();
    assert_eq!(volumes.len(),6);
    for node in volumes {
        let input=graph.pin_by_key(node.id,"level").unwrap();
        let edges=graph.edges.values().filter(|edge|edge.input==input.id).collect::<Vec<_>>();assert_eq!(edges.len(),1);
        assert_eq!(graph.effective_pin_type(edges[0].output),Some(ValueType::Float));
        assert_eq!(graph.nodes[&graph.pins[&edges[0].output].node].kind,NodeKind::VariableGet);
    }
    strict_native_consumer::finish_program(dir.clone(),original,graphs,&[NodeKind::MusicPlay,NodeKind::MusicVolume])
}
fn app_with_actual_asset(game:rvn_core::Engine<crate::bevy_renderer::BevyRenderer>,bytes:Vec<u8>)->App {
    let directory=bevy::asset::io::memory::Dir::default();
    directory.insert_asset(std::path::Path::new(ASSET),bytes);
    let reader=bevy::asset::io::memory::MemoryAssetReader{root:directory};let mut app=App::new();
    app.register_asset_source(bevy::asset::io::AssetSourceId::Default,bevy::asset::io::AssetSource::build().with_reader(move||Box::new(reader.clone())));
    app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{watch_for_changes_override:Some(false),..default()}));
    // The production plugin alone owns OutputStream/Sink creation, queues and
    // its actual background device consumer. No private AudioSink field access.
    app.add_plugins(AudioPlugin::default()).add_audio_source::<CheckpointPcm>();
    app.insert_resource(VnEngine(game)).insert_resource(State::new(VnState::Waiting))
        .init_resource::<NextState<VnState>>().init_resource::<crate::resources::VnRenderState>()
        .init_resource::<crate::resources::ChoiceFocus>().init_resource::<crate::resources::ImagemapState>()
        .init_resource::<crate::resources::TypewriterState>().init_resource::<crate::accessibility::Accessibility>()
        .init_resource::<crate::resources::ScriptErrorMessage>().init_resource::<crate::resources::MusicEntity>()
        .init_resource::<crate::resources::MusicAssetRegistry>().init_resource::<crate::resources::MusicPlaybackState>()
        .init_resource::<crate::resources::MusicVolume>().init_resource::<crate::systems::settings_menu::Settings>()
        .init_resource::<crate::scene_publication::ScenePresentation>().init_resource::<crate::scene_audio::AudioFeedbackQueue>()
        .add_event::<crate::vn_command::VnCommand>().add_event::<crate::vn_command::PlayerInput>();
    assert_eq!(app.world().resource::<crate::systems::settings_menu::Settings>().music_volume,1.);
    app
}
struct OwnedNativeApp(App);
impl Drop for OwnedNativeApp {
    fn drop(&mut self) {
        // On assertions/errors too: only the sinks created by our actual App.
        for entity in self.0.world().iter_entities(){if let Some(sink)=entity.get::<AudioSink>(){sink.stop();}}
    }
}
fn pump(app:&mut App) {
    crate::scene_publication::consume(app.world_mut());app.update();
    assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed,"{}",app.world().resource::<crate::resources::ScriptErrorMessage>().0);
}
fn current_music(world:&World)->rvn_core::audio_checkpoint::MusicCheckpointView {
    let engine=&world.resource::<VnEngine>().0;
    let rows=rvn_core::audio_checkpoint::music_views(&engine.state,engine.runtime_epoch()).unwrap();
    assert_eq!(rows.len(),1);assert!(rows[0].authority.outgoing.is_none());rows.into_iter().next().unwrap()
}
fn measurement(world:&World,target:&TargetIdentity,original:&CheckpointPcm,entity:Entity,handle:&Handle<CheckpointPcm>,level:f32)->serde_json::Value {
    let cache=world.resource::<PreparedSourceCache>();assert_eq!(cache.entries.len(),1);
    let entry=&cache.entries[target];assert!(entry.reader.same_reader(original));assert_eq!(entry.entity,Some(entity));assert_eq!(entry.handle.as_ref(),Some(handle));
    assert_eq!(world.get::<Handle<CheckpointPcm>>(entity),Some(handle));
    let sink=world.get::<AudioSink>(entity).expect("The real AudioPlugin must have created a public AudioSink");
    assert!(!sink.is_paused());assert!(!sink.empty());assert_eq!(sink.volume(),level,"only production checkpoint_sources::install may apply this gain");
    let settings=world.get::<PlaybackSettings>(entity).unwrap();assert_eq!(settings.volume.get(),level);assert!(!settings.paused);
    assert_eq!(original.fault().unwrap(),None,"No re-claim/restart of this actual decoder");
    let position=cursor(original);assert_eq!((position.channels,position.sample_rate,position.total_samples),(2,48_000,TOTAL_SAMPLES));assert!(position.looping);
    let engine=&world.resource::<VnEngine>().0;assert_eq!(engine.state.music.volume,level);assert_eq!(engine.state.vars["play_calls"],rvn_parser::Value::Int(1));
    serde_json::json!({"cursor":position,"total_consumed_interleaved_samples":total_consumed(&position),"source_seconds":total_consumed(&position)as f64/96_000.,"cumulative_percent_proxy":total_consumed(&position)as f64/TOTAL_SAMPLES as f64,"entity":format!("{entity:?}"),"sink_volume":sink.volume(),"sink_paused":sink.is_paused(),"sink_empty":sink.empty(),"logical_volume":engine.state.music.volume,"play_calls_authored_counter":engine.state.vars["play_calls"],"volume_writes":engine.state.vars["volume_writes"],"pc":engine.state.pc})
}

#[test]
#[ignore="Root-only native audio: explicit hardware opt-in, real default output device, authenticated independent20s WAV; ~35s"]
fn music_policy1003_real_bevy_audio_sink_six_writes_silence_restore_and_twenty_second_loop() {
    assert_eq!(std::env::var("RVN_MUSIC_POLICY1003_HARDWARE").as_deref(),Ok("1"),"No hardware auto-start in CPU/global suites");
    let bytes=independent_wave();let dir=output_directory("native");let program=actual_native_program(&dir);
    let binding=rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&program).unwrap();
    let mut game=rvn_core::Engine::from_program_with_consumer_binding(program,binding,crate::bevy_renderer::BevyRenderer::new(),16).unwrap();game.step_until_interaction().unwrap();
    let mut owned=OwnedNativeApp(app_with_actual_asset(game,bytes));
    let ready=Instant::now();
    let (target,entity,original,handle)=loop {
        pump(&mut owned.0);
        let world=owned.0.world();let view=current_music(world);
        if let Some(cache)=world.get_resource::<PreparedSourceCache>(){
            if let Some(entry)=cache.entries.get(&view.authority.target){
                if let Some(entity)=entry.entity{
                    if world.get::<AudioSink>(entity).is_some(){break(view.authority.target,entity,entry.reader.clone(),entry.handle.clone().unwrap());}
                }
            }
        }
        assert!(ready.elapsed()<Duration::from_secs(10),"No real ready AudioSink before10s: default hardware/output required, no fake/headless fallback");
        std::thread::sleep(Duration::from_millis(10));
    };
    let rng=owned.0.world().resource::<VnEngine>().0.state.random;
    let sampled=owned.0.world().resource::<VnEngine>().0.state.vars["sampled"].clone();
    let start=Instant::now();let mut trace=fs::OpenOptions::new().create_new(true).write(true).open(dir.join("native-measurements.jsonl")).unwrap();
    let mut write_index=0usize;let mut next_observation=Duration::ZERO;let mut previous=total_consumed(&cursor(&original));
    let mut before_silence=None;let mut silent_first=None;let mut silent_last=None;let mut restored=None;
    let last=loop {
        let elapsed=start.elapsed();
        if write_index<6 && elapsed>=Duration::from_secs(2*(write_index as u64+1)) {
            let before=cursor(&original);let next=write_index+1;
            {let mut engine=owned.0.world_mut().resource_mut::<VnEngine>();engine.0.advance_dialogue().unwrap();engine.0.step_until_interaction().unwrap();}
            pump(&mut owned.0);write_index=next;
            let after=cursor(&original);assert!(after.same_source(&before));assert!(total_consumed(&after)>=total_consumed(&before));
            assert!(start.elapsed()<=Duration::from_millis(2000*write_index as u64+500),"Native command exceeded observed UE half-second polling window; report timing failure, never rewrite expected times");
            if next==4{before_silence=Some(total_consumed(&before));silent_first=Some(total_consumed(&after));}
            if next==5{silent_last=Some(total_consumed(&before));restored=Some(total_consumed(&after));}
            let row=measurement(owned.0.world(),&target,&original,entity,&handle,LEVELS[write_index]);
            writeln!(trace,"{}",serde_json::json!({"stage":"VOLUME_COMMITTED","write":write_index,"target_seconds":2*write_index,"wall_seconds":start.elapsed().as_secs_f64(),"before_cursor":before,"after":row})).unwrap();trace.flush().unwrap();
        }
        pump(&mut owned.0);
        let world=owned.0.world();let engine=&world.resource::<VnEngine>().0;assert_eq!(engine.state.random,rng);assert_eq!(engine.state.vars["sampled"],sampled);assert_eq!(engine.state.vars["volume_writes"],rvn_parser::Value::Int(write_index as i64));
        assert_eq!(current_music(world).authority.target,target);
        let row=measurement(world,&target,&original,entity,&handle,LEVELS[write_index]);let current=total_consumed(&cursor(&original));assert!(current>=previous);previous=current;
        if elapsed>=next_observation{
            writeln!(trace,"{}",serde_json::json!({"stage":"SOURCE_SAMPLE","wall_seconds":start.elapsed().as_secs_f64(),"actual":row})).unwrap();trace.flush().unwrap();next_observation=elapsed+Duration::from_millis(500);
        }
        if elapsed>=Duration::from_secs(35){break cursor(&original);}
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(write_index,6);assert!(silent_first.unwrap()>=before_silence.unwrap());assert!(silent_last.unwrap()>silent_first.unwrap(),"Actual background device consumer must advance during zero gain");assert!(restored.unwrap()>=silent_last.unwrap());
    assert!(last.loops_completed>=1);assert!(total_consumed(&last)>TOTAL_SAMPLES,"The exact looping Source crossed20s without another Play/decoder claim");
    writeln!(trace,"{}",serde_json::json!({"stage":"END","writes":6,"one_authored_Play":true,"actual_sink_created_by":"Bevy AudioPlugin queue","device_required":true,"last_cursor":last,"audible_PCM_or_pixel_claim":false})).unwrap();trace.flush().unwrap();
    // OwnedNativeApp stops its own real AudioSink on success and unwind alike.
}

#[test]
#[ignore="Root-only offline prerequisite: authenticated independent20s WAV input; no hardware or ECS gain claim"]
fn music_policy1003_independent_pcm_decode_loop_and_rodio_amplification_prerequisite() {
    let bytes=independent_wave();let clip=PcmClip::decode(ASSET,&bytes).unwrap();
    let reader=clip.start_looped(crate::checkpoint_pcm::CheckpointGate::default(),false);
    let mut source=reader.decoder().amplify(0.25);
    let expected=rodio::Decoder::new(std::io::Cursor::new(bytes)).unwrap().take(4096).collect::<Vec<_>>();
    assert!(expected.iter().any(|value|*value!=0));
    for raw in expected {assert_eq!(source.next(),Some((f32::from(raw)*0.25)as i16));}
    let total=24*48_000*2;for _ in 4096..total{assert!(source.next().is_some());}
    let position=cursor(&reader);assert_eq!((position.total_samples,position.next_sample,position.loops_completed),(TOTAL_SAMPLES,4*48_000*2,1));assert_eq!(total_consumed(&position),24*48_000*2);
    assert_eq!(reader.fault().unwrap(),None);
    // This exercises real decoder/Source/Rodio math only. It neither creates an
    // ECS AudioSink nor claims to test checkpoint_sources::install set_volume.
}
