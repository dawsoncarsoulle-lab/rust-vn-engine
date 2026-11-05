//! Actual Core publication -> shared installer -> existing dialogue controller.
//! CPU World coverage only; no physical device, GPU, native or pixel acceptance.
use super::*;
use crate::components::{CharacterNameText, DialogueBox, DialogueText};
use crate::resources::{CharacterRegistry, DialogueHistory, TypewriterConfig};
use bevy::ecs::system::RunSystemOnce;
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture_directory() -> std::path::PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::path::PathBuf::from(std::env::var("RVN_NATIVE_TYPEWRITER_FIXTURES").expect("owned fixture root"));
    let directory = root.join(format!("character-world-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn actual_program(source: &str) -> rvn_parser::ParsedProgram {
    let directory=fixture_directory();
    let authored=directory.join("authored.rvn");
    std::fs::write(&authored,source).unwrap();
    let parsed=rvn_parser::parse_file_program_with_uses(&authored).unwrap();
    let mut graphs=rvn_graph::import_script(&parsed.script).unwrap();
    let mut exported=String::new();
    for graph in &mut graphs {
        if matches!(graph.kind,rvn_graph::GraphKind::Init) && graph.nodes.len()==1 && graph.edges.is_empty() {continue;}
        graph.enable_native_frame_preparation().unwrap();
        let strict=rvn_graph::prepare_native_frame_graph(graph).unwrap();
        let roundtrip=rvn_graph::import_native_frame_script(&strict.ast).unwrap();
        let restored=roundtrip.iter().find(|g|g.graph_id==graph.graph_id).unwrap();
        assert_eq!(restored.edges,graph.edges);
        assert_eq!(restored.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());
        std::fs::write(directory.join(format!("graph-{}.json",graph.graph_id)),graph.to_pretty_json().unwrap()).unwrap();
        exported.push_str(&strict.source);exported.push('\n');
    }
    let main=directory.join("main.rvn");std::fs::write(&main,exported).unwrap();
    rvn_parser::parse_file_program_with_uses(&main).unwrap()
}

fn engine(program:rvn_parser::ParsedProgram)->rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
    rvn_core::Engine::from_program(program,crate::bevy_renderer::BevyRenderer::new(),32).unwrap()
}

fn app(game:rvn_core::Engine<crate::bevy_renderer::BevyRenderer>)->App {
    let mut app=App::new();
    app.insert_resource(VnEngine(game))
        .insert_resource(State::new(crate::resources::VnState::Waiting))
        .init_resource::<NextState<crate::resources::VnState>>()
        .insert_resource(Time::<()>::default())
        .init_resource::<crate::accessibility::Accessibility>()
        .init_resource::<crate::resources::ScriptErrorMessage>()
        .init_resource::<ScenePresentation>()
        .init_resource::<VideoFeedbackQueue>()
        .init_resource::<VnRenderState>()
        .init_resource::<ChoiceFocus>()
        .init_resource::<ImagemapState>()
        .init_resource::<TypewriterState>()
        .init_resource::<TypewriterConfig>()
        .init_resource::<CharacterRegistry>()
        .init_resource::<DialogueHistory>()
        .add_event::<VnCommand>().add_event::<PlayerInput>()
        .add_event::<crate::display_reveal::PresentedDialogue>()
        .add_systems(Update,crate::systems::dialogue_system);
    app.world_mut().spawn((Style::default(),DialogueBox));
    app.world_mut().spawn((Text::from_section("",TextStyle::default()),CharacterNameText));
    app.world_mut().spawn((Text::from_section("",TextStyle::default()),DialogueText));
    app
}

fn shown(app:&mut App)->String {
    app.world_mut().query_filtered::<&Text,With<CharacterNameText>>()
        .single(app.world()).sections[0].value.clone()
}
fn present(app:&mut App) {
    consume(app.world_mut());
    assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
    assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
    app.world_mut().run_schedule(Update);
}

#[test]
fn character_world_true_graph_nameplate_history_use_accepted_map_and_keep_id() {
    let p=actual_program("scene label start\ncharacter.create(\"hero\",\"Héroïne😀\")\nhero \"bonjour\"\n");
    let mut game=engine(p);game.step_until_interaction().unwrap();
    assert!(matches!(game.displayed_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:Some(id),..})if id=="hero"));
    let mut app=app(game);present(&mut app);
    assert_eq!(shown(&mut app),"Héroïne😀");
    let history=&app.world().resource::<DialogueHistory>().lines;
    assert_eq!(history.len(),1);assert_eq!(history[0].character,"Héroïne😀");assert_eq!(history[0].text,"bonjour");
    let installed=app.world().resource::<ScenePresentation>().batch.as_ref().unwrap();
    assert_eq!(installed.state.layered.display_name("hero"),"Héroïne😀");
    let presented=app.world().resource::<Events<crate::display_reveal::PresentedDialogue>>();
    assert!(presented.iter_current_update_events().any(|d|d.character.as_deref()==Some("hero")));
}

#[test]
fn character_world_refused_publication_cannot_rename_old_pixels_or_history() {
    let p=actual_program("scene label start\ncharacter.create(\"hero\",\"First\")\nhero \"before\"\ncharacter.create(\"hero\",\"Candidate\")\nhero \"after\"\n");
    let mut game=engine(p);game.step_until_interaction().unwrap();let mut app=app(game);present(&mut app);
    {let mut game=app.world_mut().resource_mut::<VnEngine>();game.0.advance_dialogue().unwrap();game.0.step_until_interaction().unwrap();}
    assert_eq!(app.world().resource::<VnEngine>().0.state.layered.display_name("hero"),"Candidate");
    consume_with_music_installer(app.world_mut(),|_,_,_|Err("fixture publication refusal".into()));
    app.world_mut().run_system_once(crate::systems::setup::synchronize_character_registry);
    assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
    assert_eq!(app.world().resource::<CharacterRegistry>().display_name("hero"),"First");
    assert_eq!(shown(&mut app),"First");
    assert_eq!(app.world().resource::<DialogueHistory>().lines.len(),1);
    assert_eq!(app.world().resource::<DialogueHistory>().lines[0].text,"before");
}

#[test]
fn character_world_actual_rollback_replaces_name_projection_and_dialogue() {
    let p=actual_program("scene label start\ncharacter.create(\"hero\",\"First\")\nhero \"before\"\ncharacter.create(\"hero\",\"Second\")\nhero \"after\"\n");
    let mut game=engine(p);game.step_until_interaction().unwrap();let mut app=app(game);present(&mut app);
    {let mut game=app.world_mut().resource_mut::<VnEngine>();game.0.advance_dialogue().unwrap();game.0.step_silent().unwrap();}
    assert_eq!(app.world().resource::<VnEngine>().0.state.layered.display_name("hero"),"Second");
    // Roll back at the actual reached boundary, before taking another display
    // snapshot; no forged pc/owner, replay or direct name-map mutation.
    assert!(app.world_mut().resource_mut::<VnEngine>().0.rollback());
    present(&mut app);
    assert_eq!(shown(&mut app),"First");
    assert_eq!(app.world().resource::<CharacterRegistry>().display_name("hero"),"First");
    assert_eq!(app.world().resource::<DialogueHistory>().lines.last().unwrap().text,"before");
}

#[test]
fn character_world_cold_save15_new_world_projects_exact_restored_names() {
    let p=actual_program("scene label start\ncharacter.create(\"hero\",\"Saved😀\")\nhero \"saved line\"\n");
    let mut live=engine(p.clone());live.step_until_interaction().unwrap();
    // The actual Bevy renderer refuses saves before its World publication is
    // settled. Exercise that real installer instead of clearing the latch.
    let mut live=app(live);present(&mut live);
    let bytes=live.world().resource::<VnEngine>().0.capture_save15(&p,None,1,"names".into()).unwrap().encode().unwrap();
    let expected_rng=live.world().resource::<VnEngine>().0.state.random;
    let mut cold=engine(p.clone());assert!(cold.state.layered.display_names.is_empty());
    cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap();
    let mut app=app(cold);
    app.world_mut().resource_mut::<CharacterRegistry>().0.insert("stale".into(),"Rejected".into());
    present(&mut app);
    assert_eq!(shown(&mut app),"Saved😀");
    assert_eq!(app.world().resource::<CharacterRegistry>().display_name("stale"),"stale");
    assert_eq!(app.world().resource::<VnEngine>().0.state.random,expected_rng);
    assert_eq!(app.world().resource::<DialogueHistory>().lines[0].character,"Saved😀");
}

#[test]
fn character_world_legacy_sync_runs_before_actual_dialogue_without_startup_engine() {
    let p=rvn_parser::parse_program("label start\ncharacter.create(\"hero\",\"Legacy name\")\nhero \"legacy line\"\n").unwrap();
    // The historical raw-AST constructor is the Legacy route. parse_program
    // plus from_program may intentionally select the newer narrative context.
    let game=rvn_core::Engine::new(p.script,crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
    assert!(game.state.scene_execution.is_none());
    let mut app=app(game);
    let root=fixture_directory();
    app.init_resource::<crate::systems::debug_overlay::DebugOverlayState>()
        .init_resource::<crate::systems::debug_overlay::DebugStepRequest>()
        .init_resource::<crate::save_thumbnails::SaveThumbnails>()
        .insert_resource(crate::project_paths::ProjectPaths::new(root.clone(),root.clone(),root.clone(),root.clone(),root.clone()))
        .insert_resource(crate::resources::PersistentDataResource {
            manager:rvn_core::persistent::PersistentDataManager::new(&root).unwrap(),
            data:rvn_core::persistent::PersistentData::default(),
        });
    app.add_systems(Update,crate::systems::setup::synchronize_character_registry.before(crate::systems::dialogue_system));
    // Silent Core stepping stops at an interaction. The real Bevy stepping
    // system produces its dialogue event; do not fabricate that output here.
    app.world_mut().run_system_once(crate::systems::stepping_system);
    assert!(app.world().resource::<Events<VnCommand>>().iter_current_update_events().any(|c|matches!(c,VnCommand::ShowDialogue{..})));
    app.world_mut().run_schedule(Update);
    assert_eq!(shown(&mut app),"Legacy name");
    assert_eq!(app.world().resource::<DialogueHistory>().lines[0].character,"Legacy name");
    app.world_mut().remove_resource::<VnEngine>();
    app.world_mut().run_system_once(crate::systems::setup::synchronize_character_registry);
    assert_eq!(app.world().resource::<CharacterRegistry>().display_name("hero"),"Legacy name");
}
