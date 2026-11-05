use rvn_core::{Engine,Renderer,SpriteState,Interaction};
use rvn_core::renderer::SceneRenderBatch;
use rvn_core::display_reveal::RevealCheckpoint;
use rvn_core::save::{SaveData,SaveManager};
use rvn_parser::{Hotspot,Position,Transition};
#[derive(Default)]
struct Host {
    batches: Vec<SceneRenderBatch>,
    reject: bool,
}
impl Renderer for Host {
    fn supports_video(&self) -> bool {
        true
    }
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn supports_custom_canvas(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("Rejected exact resident projection".into())
        } else {
            Ok(())
        }
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.batches.push(batch);
    }
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(
        &mut self,
        _: &str,
        _: Option<&str>,
        _: &Position,
        _: &Transition,
        _: Option<&SpriteState>,
    ) {
    }
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {
        0
    }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
        0
    }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}

const SOURCE: &str = "scene label P\n\"é🌟 longue\"\n\"returned\"\njump done\nscene label C\n\"child\"\nreturn\nlabel done\n";
fn game(source:&str)->Engine<Host> {
    let mut game=Engine::from_program(rvn_parser::parse_program(source).unwrap(),Host::default(),32).unwrap();
    game.step_until_interaction().unwrap();game
}
fn wire(game:&Engine<Host>)->serde_json::Value {
    serde_json::json!({"state":game.state,"history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),"runtime":game.runtime_epoch(),"interface":game.interface_epoch(),"instance":game.instance_id(),"source":format!("{:?}",game.display_reveal_source()),"publications":game.renderer.batches.len()})
}
fn partial(text:&str)->RevealCheckpoint {RevealCheckpoint::observed(text,2,true,0.25,1.375).unwrap()}
fn current_text(game:&Engine<Host>)->String {
    let Some(Interaction::Dialogue{text,..})=game.displayed_interaction().unwrap() else {panic!("no frozen dialogue")};text
}

#[test]
fn reveal_unicode_tags_completed_language_and_real_terminal_backlog_preserve_metrics() {
    let mut partial=RevealCheckpoint::observed("é🌟 longue",2,true,0.25,1.375).unwrap();
    let same=partial.for_text("é🌟 longue").unwrap();assert_eq!(same,partial);
    let shorter=partial.for_text("界").unwrap();assert_eq!(shorter.visible_chars,1);assert!(!shorter.typing);assert_eq!(shorter.char_progress.to_bits(),1.375f32.to_bits());
    let terminal=RevealCheckpoint::observed("fin",3,false,0.125,3.75).unwrap();
    assert_eq!(terminal.for_text("fin").unwrap(),terminal);
    let translated=terminal.for_text("終わり 🌟").unwrap();assert_eq!(translated.visible_chars,translated.total_chars);assert!(!translated.typing);
    partial.typing=false;assert!(partial.validate().is_err());
    let invalid_tags=RevealCheckpoint::observed("{unknown}é🌟",0,true,0.,0.).unwrap();
    assert_eq!(usize::try_from(invalid_tags.total_chars).unwrap(),rvn_core::display_reveal::rendered_rich_text("{unknown}é🌟").plain_text().chars().count());
}

#[test]
fn reveal_feedback_changes_only_observed_checkpoint_and_latest_exact_history_without_publication() {
    let mut game=game(SOURCE);let source=game.display_reveal_source().unwrap();let text=current_text(&game);
    let before_state=game.state.clone();let before_history=game.history.len();let batches=game.renderer.batches.len();
    let reveal=partial(&text);assert!(game.record_display_reveal(&source,reveal.clone()).unwrap());
    let mut expected=before_state;expected.scene_execution.as_mut().unwrap().display.as_mut().unwrap().reveal=Some(reveal.clone());
    assert_eq!(serde_json::to_value(&game.state).unwrap(),serde_json::to_value(&expected).unwrap());
    assert_eq!(game.history.len(),before_history);assert_eq!(game.renderer.batches.len(),batches);
    assert_eq!(game.history.entries().last().unwrap().state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().reveal.as_ref(),Some(&reveal));
    assert_eq!(game.display_reveal_source().as_ref(),Some(&source));
}

#[test]
fn reveal_parent_display_source_survives_callback_call_and_updates_parent_history_not_child_owner() {
    let mut game=game(SOURCE);let source=game.display_reveal_source().unwrap();let parent=source.owner;
    game.execute_timer_action("call C").unwrap();assert_ne!(game.state.scene_execution.as_ref().unwrap().owner(),parent);
    assert_eq!(game.display_reveal_source().as_ref(),Some(&source));
    let before=game.state.random;let reveal=partial(&current_text(&game));
    assert!(game.record_display_reveal(&source,reveal.clone()).unwrap());assert_eq!(game.state.random,before);
    let latest=game.history.entries().last().unwrap();assert_eq!(latest.state.scene_execution.as_ref().unwrap().owner(),parent);
    assert_eq!(latest.state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().reveal.as_ref(),Some(&reveal));
    game.step_until_interaction().unwrap();let before=wire(&game);
    let mut corrupt=reveal;corrupt.char_progress=f32::NAN;
    assert!(!game.record_display_reveal(&source,corrupt).unwrap());assert_eq!(wire(&game),before);
}

#[test]
fn reveal_new_interaction_and_independent_epoch_zero_engine_reject_old_source_before_payload() {
    let mut original=game(SOURCE);let source=original.display_reveal_source().unwrap();
    let mut replacement=game(SOURCE);assert_eq!(replacement.runtime_epoch(),original.runtime_epoch());assert_ne!(replacement.instance_id(),original.instance_id());
    let before=wire(&replacement);let mut invalid=partial("é🌟 longue");invalid.rendered_text="x".repeat(rvn_core::display_reveal::MAX_REVEAL_TEXT_BYTES+1);invalid.pause_remaining=f32::NAN;
    assert!(!replacement.record_display_reveal(&source,invalid.clone()).unwrap());assert_eq!(wire(&replacement),before);
    original.advance_dialogue().unwrap();original.step_until_interaction().unwrap();let before=wire(&original);
    assert_ne!(original.display_reveal_source().unwrap().display_generation,source.display_generation);
    assert!(!original.record_display_reveal(&source,invalid).unwrap());assert_eq!(wire(&original),before);
}

#[test]
fn reveal_rollback_restores_observed_progress_without_replay_and_rebases_authorization() {
    let mut game=game(SOURCE);let source=game.display_reveal_source().unwrap();let reveal=partial(&current_text(&game));
    game.record_display_reveal(&source,reveal.clone()).unwrap();game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
    let second=game.display_reveal_source().unwrap();let history=game.history.len();assert!(game.rollback());
    assert_eq!(game.history.len(),history-1);assert_ne!(game.display_reveal_source().unwrap().runtime_epoch,second.runtime_epoch);
    assert!(game.rollback());assert_eq!(current_text(&game),"é🌟 longue");
    assert_eq!(game.state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().reveal.as_ref(),Some(&reveal));
    let before=wire(&game);let mut invalid=reveal;invalid.char_progress=f32::NAN;
    assert!(!game.record_display_reveal(&source,invalid).unwrap());assert_eq!(wire(&game),before);
}

#[test]
fn reveal_load_restores_partial_state_rebases_display_owner_and_rejects_corrupt_header_atomically() {
    let mut game=game(SOURCE);let old=game.display_reveal_source().unwrap();let reveal=partial(&current_text(&game));game.record_display_reveal(&old,reveal.clone()).unwrap();
    let saved=SaveData::try_from_state(&game.state,1,"reveal".into(),"main.rvn".into()).unwrap();assert_eq!(saved.format_version,11);
    let mut forged=saved.clone();forged.format_version=10;let before=wire(&game);assert!(game.load_data(forged).is_err());assert_eq!(wire(&game),before);
    let mut corrupt=saved.clone();corrupt.scene_execution.as_mut().unwrap().display.as_mut().unwrap().reveal.as_mut().unwrap().visible_chars=u32::MAX;
    assert!(game.load_data(corrupt).is_err());assert_eq!(wire(&game),before);
    game.load_data(saved).unwrap();assert!(game.history.is_empty());let source=game.display_reveal_source().unwrap();assert_ne!(source.runtime_epoch,old.runtime_epoch);
    assert_eq!(source.owner,game.state.scene_execution.as_ref().unwrap().owner());assert_eq!(game.state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().reveal.as_ref(),Some(&reveal));
    let before=wire(&game);assert!(!game.record_display_reveal(&old,reveal).unwrap());assert_eq!(wire(&game),before);
}

#[test]
fn reveal_locale_changes_use_frozen_expression_context_without_rng_replay_and_keep_source() {
    use rvn_core::locale::{LocaleManager,LocaleTable};use std::collections::HashMap;
    let mut game=game("init {set n=7}\nscene label P\n\"valeur [n]\"\n");
    let tables=HashMap::from([("fr".into(),LocaleTable{lang:"fr".into(),strings:HashMap::new()}),("jp".into(),LocaleTable{lang:"jp".into(),strings:HashMap::from([("valeur [n]".into(),"🌟[n] 界".into())])})]);
    game.locale=Some(LocaleManager::from_tables(std::env::temp_dir(),"fr","fr",vec!["fr".into(),"jp".into()],tables));
    let source=game.display_reveal_source().unwrap();let old=partial(&current_text(&game));game.record_display_reveal(&source,old.clone()).unwrap();let random=game.state.random;
    game.state.vars.insert("n".into(),rvn_parser::Value::Int(99));game.locale.as_mut().unwrap().set_language("jp").unwrap();
    assert_eq!(current_text(&game),"🌟7 界");assert_eq!(game.display_reveal_source().as_ref(),Some(&source));assert_eq!(game.state.random,random);
    let remapped=old.for_text(&current_text(&game)).unwrap();assert_eq!(remapped.visible_chars,2);assert!(game.record_display_reveal(&source,remapped).unwrap());
}

struct Directory(std::path::PathBuf);
impl Directory {fn new()->Self {static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);let n=NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed);let path=std::env::temp_dir().join(format!("rvn-reveal-{}-{n}",std::process::id()));std::fs::create_dir(&path).unwrap();Self(path)}}
impl Drop for Directory {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}

#[test]
fn reveal_manual_quick_auto_writers_use_format11_and_refuse_bad_metrics_before_any_file_change() {
    let directory=Directory::new();let manager=SaveManager::new(&directory.0,8).unwrap();let mut game=game(SOURCE);
    let source=game.display_reveal_source().unwrap();game.record_display_reveal(&source,partial(&current_text(&game))).unwrap();
    manager.save(&game.state,1,"manual".into(),"main.rvn".into()).unwrap();manager.save_quicksave(&game.state,"quick".into(),"main.rvn".into()).unwrap();manager.save_autosave(&game.state,"auto".into(),"main.rvn".into()).unwrap();
    for saved in [manager.load(1).unwrap(),manager.load_quicksave().unwrap(),manager.load_autosave().unwrap()] {assert_eq!(saved.format_version,11);assert!(saved.scene_execution.unwrap().display.unwrap().reveal.is_some());}
    let before:Vec<_>=["slot_01.json","quicksave.json","autosave.json"].iter().map(|name|std::fs::read(directory.0.join(name)).unwrap()).collect();
    let mut invalid=game.state.clone();invalid.scene_execution.as_mut().unwrap().display.as_mut().unwrap().reveal.as_mut().unwrap().char_progress=f32::INFINITY;
    let original=serde_json::to_value(&game.state).unwrap();
    assert!(manager.save(&invalid,1,"bad".into(),"main.rvn".into()).is_err());assert!(manager.save_quicksave(&invalid,"bad".into(),"main.rvn".into()).is_err());assert!(manager.save_autosave(&invalid,"bad".into(),"main.rvn".into()).is_err());
    let after:Vec<_>=["slot_01.json","quicksave.json","autosave.json"].iter().map(|name|std::fs::read(directory.0.join(name)).unwrap()).collect();assert_eq!(after,before);assert_eq!(serde_json::to_value(&game.state).unwrap(),original);
}

#[test]
fn reveal_absent_old_scene10_and_legacy9_keep_wire_shape_without_synthetic_progress() {
    let modern=game(SOURCE);let saved=SaveData::try_from_state(&modern.state,1,"old".into(),"main.rvn".into()).unwrap();assert_eq!(saved.format_version,10);
    let wire=serde_json::to_value(&saved).unwrap();assert!(wire["scene_execution"]["display"].get("reveal").is_none());
    let mut restored=game(SOURCE);restored.load_data(serde_json::from_value(wire).unwrap()).unwrap();assert!(restored.state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().reveal.is_none());
    let mut legacy=Engine::new(rvn_parser::parse("label start\n\"line\"\n").unwrap(),Host::default(),32).unwrap();legacy.step_until_interaction().unwrap();
    let saved=SaveData::try_from_state(&legacy.state,1,"legacy".into(),"main.rvn".into()).unwrap();assert_eq!(saved.format_version,9);assert!(legacy.display_reveal_source().is_none());
}

#[test]
fn reveal_current_invalid_payload_and_renderer_restore_refusal_leave_history_source_epochs_exact() {
    let mut game=game(SOURCE);let source=game.display_reveal_source().unwrap();let before=wire(&game);let mut corrupt=partial(&current_text(&game));corrupt.pause_remaining=f32::NAN;
    assert!(game.record_display_reveal(&source,corrupt).is_err());assert_eq!(wire(&game),before);
    let saved=SaveData::try_from_state(&game.state,1,"saved".into(),"main.rvn".into()).unwrap();game.renderer.reject=true;
    assert!(game.load_data(saved).is_err());assert_eq!(wire(&game),before);
}

#[test]
fn reveal_jump_preserves_the_existing_display_owner_until_an_actual_new_interaction_is_recorded() {
    let mut game=game(SOURCE);let source=game.display_reveal_source().unwrap();let before_history=game.history.len();
    game.execute_timer_action("jump C").unwrap();assert_ne!(game.state.scene_execution.as_ref().unwrap().owner(),source.owner);
    assert_eq!(game.display_reveal_source().as_ref(),Some(&source));
    let reveal=partial(&current_text(&game));assert!(game.record_display_reveal(&source,reveal.clone()).unwrap());
    assert_eq!(game.history.len(),before_history);assert_eq!(game.history.entries().last().unwrap().state.scene_execution.as_ref().unwrap().owner(),source.owner);
    game.step_until_interaction().unwrap();assert_eq!(current_text(&game),"child");assert!(!game.accepts_display_reveal_source(&source));
}

#[test]
fn reveal_reader_and_delayed_thumbnail_writers_refuse_underdeclared_progress_without_rewriting_files() {
    let directory=Directory::new();let manager=SaveManager::new(&directory.0,8).unwrap();let mut game=game(SOURCE);
    let source=game.display_reveal_source().unwrap();game.record_display_reveal(&source,partial(&current_text(&game))).unwrap();
    let mut dto=SaveData::try_from_state(&game.state,1,"corrupt".into(),"main.rvn".into()).unwrap();dto.format_version=10;
    let bytes=serde_json::to_vec_pretty(&dto).unwrap();
    for file in ["slot_01.json","quicksave.json","autosave.json"]{std::fs::write(directory.0.join(file),&bytes).unwrap();}
    assert!(manager.load(1).is_err());assert!(manager.load_quicksave().is_err());assert!(manager.load_autosave().is_err());
    assert!(manager.set_thumbnail(1,"late.png".into()).is_err());
    assert!(manager.set_resume_thumbnail(rvn_core::save::ResumeSlot::Quick,&dto,"late.png".into()).is_err());
    assert!(manager.set_resume_thumbnail(rvn_core::save::ResumeSlot::Auto,&dto,"late.png".into()).is_err());
    for file in ["slot_01.json","quicksave.json","autosave.json"]{assert_eq!(std::fs::read(directory.0.join(file)).unwrap(),bytes);}
    let mut choice=dto;choice.format_version=11;choice.scene_execution.as_mut().unwrap().display.as_mut().unwrap().display=rvn_core::HistoryDisplay::Choice{options:vec!["invalid".into()]};
    let before=wire(&game);assert!(game.load_data(choice).is_err());assert_eq!(wire(&game),before);
}

#[test]
fn reveal_repeated_same_position_owner_rollback_feedback_never_rewrites_an_earlier_visit_and_new_line_reassociates() {
    let mut game=game("scene label P\nlabel loop\n\"repeat 🌟\"\njump loop\n");
    let first_source=game.display_reveal_source().unwrap();
    let first=RevealCheckpoint::observed("repeat 🌟",1,true,0.125,0.375).unwrap();
    assert!(game.record_display_reveal(&first_source,first.clone()).unwrap());
    game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
    let second_source=game.display_reveal_source().unwrap();
    assert_eq!(second_source.position,first_source.position);assert_eq!(second_source.owner,first_source.owner);
    assert_ne!(second_source.display_generation,first_source.display_generation);assert_eq!(game.history.len(),2);
    let second=RevealCheckpoint::observed("repeat 🌟",2,true,0.25,0.625).unwrap();
    assert!(game.record_display_reveal(&second_source,second.clone()).unwrap());
    let first_entry=serde_json::to_value(&game.history.entries()[0].state).unwrap();
    assert!(game.rollback());assert_eq!(game.history.len(),1);
    let restored=game.display_reveal_source().unwrap();assert_eq!(restored.position,first_source.position);assert_eq!(restored.owner,first_source.owner);
    assert_ne!(restored.runtime_epoch,second_source.runtime_epoch);
    let later=RevealCheckpoint::observed("repeat 🌟",3,true,0.5,0.875).unwrap();
    assert!(game.record_display_reveal(&restored,later.clone()).unwrap());
    assert_eq!(game.state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().reveal.as_ref(),Some(&later));
    assert_eq!(serde_json::to_value(&game.history.entries()[0].state).unwrap(),first_entry);
    game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
    let fresh=game.display_reveal_source().unwrap();assert_ne!(fresh.display_generation,restored.display_generation);
    assert_eq!(game.history.len(),2);let newest=RevealCheckpoint::observed("repeat 🌟",4,true,0.75,1.125).unwrap();
    assert!(game.record_display_reveal(&fresh,newest.clone()).unwrap());
    assert_eq!(game.history.entries()[1].state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().reveal.as_ref(),Some(&newest));
    assert_eq!(serde_json::to_value(&game.history.entries()[0].state).unwrap(),first_entry);
}

#[test]
fn reveal_trimmed_repeated_visit_stays_unlinked_after_pop_until_a_new_recorded_display() {
    let mut game=Engine::from_program(rvn_parser::parse_program("scene label P\nlabel loop\n\"repeat 🌟\"\njump loop\n").unwrap(),Host::default(),1).unwrap();
    game.step_until_interaction().unwrap();let old=game.display_reveal_source().unwrap();
    assert!(game.record_display_reveal(&old,partial("repeat 🌟")).unwrap());
    game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();assert_eq!(game.history.len(),1);
    let current=game.display_reveal_source().unwrap();assert_eq!(current.position,old.position);assert_eq!(current.owner,old.owner);
    let before=wire(&game);let mut invalid=partial("repeat 🌟");invalid.pause_remaining=f32::NAN;
    assert!(!game.record_display_reveal(&old,invalid).unwrap());assert_eq!(wire(&game),before);
    assert!(game.rollback());assert!(game.history.is_empty());
    let restored=game.display_reveal_source().unwrap();let observed=RevealCheckpoint::observed("repeat 🌟",3,true,0.25,0.5).unwrap();
    assert!(game.record_display_reveal(&restored,observed).unwrap());assert!(game.history.is_empty());
    game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();assert_eq!(game.history.len(),1);
    let fresh=game.display_reveal_source().unwrap();let observed=RevealCheckpoint::observed("repeat 🌟",4,true,0.5,1.25).unwrap();
    assert!(game.record_display_reveal(&fresh,observed.clone()).unwrap());
    assert_eq!(game.history.entries()[0].state.scene_execution.as_ref().unwrap().display.as_ref().unwrap().reveal.as_ref(),Some(&observed));
}
