//! Proposed integration tests. NOT_RUN; this file is not installed in rvn_core.
//! Fixture bytes come from pre-C native autosaves, never SaveData::from_state.
//! The accompanying private SHA256 index is a required runner input. Runtime
//! FNV witnesses only detect drift; they are not cryptographic provenance.
use rvn_core::{
    renderer::SceneRenderBatch, save::SaveData, save::SaveError, Engine,
    HistoryDisplay, LoadCompatibility, Renderer, SpriteState,
};
use rvn_parser::{Hotspot, Position, Transition, Value};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

const INDEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../validation-ue5-windows-20261002/ue-parity-v1-3-baseline/phase-c-vertical-preparation/original-save-load-tests-stage-0001/paths-index.private.json"
));

#[derive(Deserialize)]
struct FileProof {
    path: String,
    bytes: usize,
    fnv1a64: String,
    sha256: String,
}
#[derive(Deserialize)]
struct NativeFixture {
    id: String,
    original: FileProof,
    copy: FileProof,
    main_script: String,
    start_label: String,
    expected_identity: String,
    expected_pc: usize,
    expected_stack: Vec<usize>,
    expected_globals: usize,
    last_dialogue_pc: usize,
}
#[derive(Deserialize)]
struct PathsIndex {
    fixtures: Vec<NativeFixture>,
    unversioned: FileProof,
    protected_files: Vec<FileProof>,
}

fn index() -> PathsIndex {
    serde_json::from_slice(INDEX).expect("The reviewed private fixture index must be valid JSON")
}
fn read_verified(proof: &FileProof) -> Result<Vec<u8>, String> {
    let bytes = fs::read(&proof.path)
        .map_err(|error| format!("Original fixture read failed: {}: {error}", proof.path))?;
    if bytes.len() != proof.bytes {
        return Err(format!("Original fixture length changed: {}", proof.path));
    }
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    if format!("{hash:016X}") != proof.fnv1a64 {
        return Err(format!(
            "Original fixture bytes changed: {}; reviewed SHA256 {}",
            proof.path, proof.sha256
        ));
    }
    Ok(bytes)
}
fn protected_bytes(index: &PathsIndex) -> BTreeMap<String, Vec<u8>> {
    index.protected_files.iter().map(|proof| {
        (proof.path.clone(), read_verified(proof).unwrap_or_else(|error| panic!("{error}")))
    }).collect()
}
fn unchanged(before: &BTreeMap<String, Vec<u8>>) {
    for (path, bytes) in before {
        assert_eq!(
            &fs::read(path).unwrap_or_else(|error| panic!("Protected file disappeared: {path}: {error}")),
            bytes, "Original/copied save, project or provenance changed: {path}"
        );
    }
}

/// CPU presenter only: records actual Engine publications and supports saved
/// logical capabilities. It provides no GPU, decoder, speech or audio proof.
#[derive(Clone, Default, Debug, PartialEq)]
struct Host {
    output: Vec<String>,
    loaded: Vec<LoadCompatibility>,
}
impl Renderer for Host {
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> { Ok(()) }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.output.push(format!("batch:{}:{:?}", batch.state.pc, batch.display));
    }
    fn supports_programmable_ui(&self) -> bool { true }
    fn supports_custom_canvas(&self) -> bool { true }
    fn supports_composable_motion(&self) -> bool { true }
    fn supports_layered_characters(&self) -> bool { true }
    fn supports_video(&self) -> bool { true }
    fn supports_accessibility(&self) -> bool { true }
    fn update_interfaces(&mut self, views: &[rvn_ui::programmable::ScreenView]) -> Result<(), String> {
        self.output.push(format!("interfaces:{views:?}")); Ok(())
    }
    fn update_layered_characters(&mut self, views: &[rvn_core::composition::LayeredView]) -> Result<(), String> {
        self.output.push(format!("layered:{views:?}")); Ok(())
    }
    fn update_motions(&mut self, views: &[rvn_core::motion::MotionView]) -> Result<(), String> {
        self.output.push(format!("motions:{views:?}")); Ok(())
    }
    fn update_videos(&mut self, views: &[rvn_core::video::VideoView]) -> Result<(), String> {
        self.output.push(format!("videos:{views:?}")); Ok(())
    }
    fn update_accessibility(&mut self, settings: &rvn_ui::accessibility::AccessibilitySettings) -> Result<(), String> {
        self.output.push(format!("accessibility:{settings:?}")); Ok(())
    }
    fn accessibility_speech(&mut self, request: &rvn_ui::accessibility::SpeechRequest) -> Result<(), String> {
        self.output.push(format!("speech:{request:?}")); Ok(())
    }
    fn loaded_compatibility(&mut self, compatibility: LoadCompatibility) { self.loaded.push(compatibility); }
    fn set_background(&mut self, path: &str, _: &Transition) { self.output.push(format!("background:{path}")); }
    fn show_sprite(&mut self, id: &str, emotion: Option<&str>, _: &Position, _: &Transition, _: Option<&SpriteState>) {
        self.output.push(format!("sprite:{id}:{emotion:?}"));
    }
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, character: Option<&str>, text: &str) { self.output.push(format!("dialogue:{character:?}:{text}")); }
    fn show_choice(&mut self, _: &[String]) -> usize { 0 }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize { 0 }
    fn music_play(&mut self, file: &str, _: &Transition, _: Option<&str>) { self.output.push(format!("music:{file}")); }
    fn music_stop(&mut self, _: &Transition) { self.output.push("music:stop".into()); }
    fn music_set_volume(&mut self, volume: f32) { self.output.push(format!("volume:{volume}")); }
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}

fn history(game: &Engine<Host>) -> Vec<(serde_json::Value, serde_json::Value)> {
    game.history.entries().iter().map(|entry| {
        (serde_json::to_value(&entry.state).unwrap(), serde_json::to_value(&entry.display).unwrap())
    }).collect()
}
fn assert_native_load(id: &str) -> (Engine<Host>, SaveData) {
    let paths = index();
    let protected = protected_bytes(&paths);
    let fixture = paths.fixtures.iter().find(|fixture| fixture.id == id).unwrap();
    let raw = read_verified(&fixture.copy).unwrap();
    assert_eq!(raw, read_verified(&fixture.original).unwrap());
    let saved: SaveData = serde_json::from_slice(&raw)
        .unwrap_or_else(|error| panic!("Historical SaveData cannot be read without edits ({id}): {error}"));
    assert_eq!(saved.format_version, 9);
    assert!(saved.scene_execution.is_none(), "Do not manufacture an explicit-scene continuation for v9");
    assert_eq!(saved.story_identity.as_deref(), Some(fixture.expected_identity.as_str()));
    assert_eq!(saved.pc, fixture.expected_pc);
    assert_eq!(saved.call_stack, fixture.expected_stack);
    assert_eq!(saved.vars.len(), fixture.expected_globals);
    assert_eq!(saved.last_dialogue.as_ref().unwrap().pc, fixture.last_dialogue_pc);

    // Match the actual current runtime loader; do not splice menus into a
    // recent test string, rename the saved script display label or patch PCs.
    let program = rvn_parser::parse_file_program_with_uses(Path::new(&fixture.main_script))
        .unwrap_or_else(|error| panic!("Historical configured program cannot be loaded: {id}: {error}"));
    assert_eq!(program.context, rvn_parser::NarrativeContext::Legacy);
    let mut game = Engine::from_program(program, Host::default(), 64).unwrap();
    game.enter_label(&fixture.start_label).unwrap();
    let prepared_identity = game.state.story_identity.clone();
    game.state.vars.insert("audit_load_sentinel".into(), Value::Int(193));
    game.history.push(game.state.clone(), Some(HistoryDisplay::Dialogue { character: None, text: "before original load".into() }));
    game.renderer.output.clear();
    game.renderer.loaded.clear();
    let before_state = serde_json::to_value(&game.state).unwrap();
    let before_history = history(&game);
    let before_epochs = (game.runtime_epoch(), game.interface_epoch());
    let before_host = game.renderer.clone();
    let expected_state = saved.clone().into_game_state();
    let result = game.load_data(saved.clone());
    unchanged(&protected);
    match result {
        Ok(compatibility) => {
            assert_eq!(compatibility, LoadCompatibility::Verified);
            assert_eq!(prepared_identity.as_deref(), Some(fixture.expected_identity.as_str()),
                "Prepared identity must come from the exact historical program, not an overwritten save identity");
            assert_eq!(game.renderer.loaded, [LoadCompatibility::Verified]);
            assert_eq!(serde_json::to_value(&game.state).unwrap(), serde_json::to_value(&expected_state).unwrap(),
                "Every serialized GameState field, including RNG and recorded display input, must survive the actual legacy load");
            assert!(game.history.is_empty());
            assert!(game.state.scene_execution.is_none());
            assert!(!game.renderer.output.is_empty(), "Actual renderer restoration must be observable");
        }
        Err(error) => {
            // A genuine refusal is preserved and diagnosed, never turned into
            // a successful restoration or hidden by accepting any Err.
            assert_eq!(serde_json::to_value(&game.state).unwrap(), before_state);
            assert_eq!(history(&game), before_history);
            assert_eq!((game.runtime_epoch(), game.interface_epoch()), before_epochs);
            assert_eq!(game.renderer, before_host);
            panic!("Actual historical {id} load refused safely, but Verified restoration remains unqualified; current prepared identity {prepared_identity:?}: {error}");
        }
    }
    (game, saved)
}

#[test]
fn original_native_source7_loads_v9_at_verified_position_without_rewriting_any_fixture() {
    let (game, saved) = assert_native_load("source-runtime-7");
    assert_eq!(game.state.pc, 39);
    assert!(game.state.call_stack.is_empty());
    assert_eq!(game.state.typewriter.chars_per_sec, 35);
    assert!(game.state.typewriter.enabled);
    assert!(game.state.ui.screens.is_empty());
    assert_eq!(game.state.random, saved.random);
    assert_eq!(game.state.display_random, saved.display_random);
}

#[test]
fn original_native_atlas8_loads_v9_with_nested_calls_and_exact_canvas_clock() {
    let (game, saved) = assert_native_load("atlas-runtime-8");
    assert_eq!(game.state.pc, 249);
    assert_eq!(game.state.call_stack, [71, 81]);
    assert_eq!(game.state.last_dialogue.as_ref().unwrap().pc, 248);
    let hud = game.state.ui.screens.iter().find(|screen| screen.name == "atlas_hud").unwrap();
    assert_eq!(hud.order, 0);
    assert_eq!(hud.canvas_states["atlas_hud_surface"].elapsed.to_bits(), 1.9963328988524154f64.to_bits());
    assert!(game.state.layered.characters.contains_key("keeper"));
    assert_eq!(game.state.music.current_file.as_deref(), Some("music/fog_theme.wav"));
    assert_eq!(game.state.music.volume.to_bits(), 0.32f32.to_bits());
    assert_eq!(game.state.random, saved.random);
    assert_eq!(game.state.display_random, saved.display_random);
    assert!(!game.interface_views().unwrap().is_empty());
    assert!(!game.layered_character_views().unwrap().is_empty());
    // No audio playhead, decoder, partial reveal or successful inter-process
    // native resume is inferred from these logical CPU assertions.
}

#[test]
fn genuine_unversioned_slot_refuses_explicit_scene_atomically_without_invented_provenance() {
    let paths = index();
    let protected = protected_bytes(&paths);
    let raw = read_verified(&paths.unversioned).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert!(json.get("format_version").is_none());
    assert!(json.get("story_identity").is_none());
    let saved: SaveData = serde_json::from_slice(&raw).unwrap();
    assert_eq!(saved.format_version, 1, "Use the actual reader's default, not a synthetic save version");
    assert!(saved.story_identity.is_none());
    assert!(saved.scene_execution.is_none());
    assert_eq!(saved.pc, 4);
    // This is a deliberately new explicit-scene control program, never
    // presented as the unresolved historical program of the original slot.
    let program = rvn_parser::parse_program("scene label audit_control\n\"safe control\"\n").unwrap();
    let mut game = Engine::from_program(program, Host::default(), 16).unwrap();
    game.state.vars.insert("preserve_me".into(), Value::Int(91));
    game.history.push(game.state.clone(), Some(HistoryDisplay::Dialogue { character: None, text: "keep history".into() }));
    let before = serde_json::to_value(&game.state).unwrap();
    let before_history = history(&game);
    let before_epochs = (game.runtime_epoch(), game.interface_epoch());
    let before_host = game.renderer.clone();
    let error = game.load_data(saved).expect_err("Unknown legacy continuation must not invent scene frames");
    match error {
        SaveError::Incompatible(reason) => assert!(reason.contains("explicit scene continuation"), "Actual compatibility diagnosis: {reason}"),
        other => panic!("Expected the genuine scene/legacy compatibility guard, got {other}"),
    }
    assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
    assert_eq!(history(&game), before_history);
    assert_eq!((game.runtime_epoch(), game.interface_epoch()), before_epochs);
    assert_eq!(game.renderer, before_host);
    unchanged(&protected);
    assert_eq!(read_verified(&paths.unversioned).unwrap(), raw);
}
