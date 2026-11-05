//! Producer-captured dialogue authorization and an admitted bounded pending queue.
//! This presenter is shared by native and browser builds; no clock enters JS.
use bevy::prelude::*;
use bevy::ecs::event::ManualEventReader;
use rvn_core::display_reveal::{DisplayRevealSource, RevealCheckpoint};
use crate::{resources::{TypewriterState, VnEngine}, vn_command::VnCommand};
use std::collections::VecDeque;

const MAX_PENDING_DIALOGUES: usize = 128;
const MAX_PENDING_TEXT_BYTES: usize = 8_000_000;

#[derive(Resource, Default)]
pub(crate) struct DialogueRevealBinding {
    pub(crate) source: Option<DisplayRevealSource>,
    pub(crate) text: String,
}

#[derive(Resource, Default, Clone)]
pub(crate) struct DialogueRevealQueue {
    pub(crate) commands: VecDeque<VnCommand>,
}

#[derive(Event, Clone)]
pub(crate) struct PresentedDialogue {
    pub(crate) source: DisplayRevealSource,
    pub(crate) character: Option<String>,
    pub(crate) text: String,
}

pub(crate) struct DialogueRevealPlugin;
impl Plugin for DialogueRevealPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogueRevealBinding>().init_resource::<DialogueRevealQueue>()
            .add_event::<PresentedDialogue>()
            .add_systems(Update, collect.after(crate::systems::stepping_system).before(crate::systems::dialogue_system));
    }
}

pub(crate) fn command(engine: &VnEngine, character: Option<String>, text: String) -> VnCommand {
    if let Some(source) = engine.0.display_reveal_source() {
        let checkpoint = engine.0.state.scene_execution.as_ref().and_then(|execution| execution.display.as_ref())
            .and_then(|display| display.reveal.clone());
        VnCommand::ScopedShowDialogue { source, checkpoint, character, text }
    } else { VnCommand::ShowDialogue { character, text } }
}

pub(crate) fn preflight_scene(world:&World,display:&Option<rvn_core::Interaction>)->Result<(),String> {
    let Some(engine)=world.get_resource::<VnEngine>() else{return Ok(());};
    if engine.0.state.scene_execution.is_none(){return Ok(());}
    if let Some(rvn_core::Interaction::Dialogue{character,text})=display {
        let captured=command(engine,character.clone(),text.clone());
        if payload(&captured,Some(engine))?.is_none(){return Err("Scene dialogue has no current captured display authority".into());}
    }
    Ok(())
}

/// Uses only the opaque future source produced by authenticated Core preparation.
pub(crate) fn preflight_prepared(prepared:&rvn_core::engine::PreparedSave15)->Result<(),String> {
    let batch=prepared.render_batch();
    if batch.state.scene_execution.is_none(){return Ok(());}
    if let Some(rvn_core::Interaction::Dialogue{text,..})=&batch.display {
        prepared.display_reveal_source().ok_or("Prepared dialogue has no authenticated future authority")?;
        if text.len()>rvn_core::display_reveal::MAX_REVEAL_TEXT_BYTES{return Err("Dialogue exceeds the portable text budget".into());}
        if let Some(reveal)=batch.state.scene_execution.as_ref().and_then(|e|e.display.as_ref()).and_then(|d|d.reveal.as_ref()){reveal.for_text(text)?;}
    }
    Ok(())
}

pub(crate) fn payload<'a>(command: &'a VnCommand, engine: Option<&VnEngine>)
    -> Result<Option<(&'a Option<String>, &'a String, Option<&'a DisplayRevealSource>, Option<&'a RevealCheckpoint>)>, String> {
    match command {
        VnCommand::ShowDialogue { character, text } => {
            if engine.is_some_and(|engine| engine.0.state.scene_execution.is_some()) { return Ok(None); }
            Ok(Some((character, text, None, None)))
        }
        VnCommand::ScopedShowDialogue { source, checkpoint, character, text } => {
            let Some(engine) = engine else { return Ok(None); };
            if !engine.0.accepts_display_reveal_source(source) { return Ok(None); }
            if text.len() > rvn_core::display_reveal::MAX_REVEAL_TEXT_BYTES { return Err("Dialogue exceeds the portable text budget".into()); }
            if engine.0.displayed_interaction().map_err(|error|error.to_string())? != Some(rvn_core::Interaction::Dialogue { character: character.clone(), text: text.clone() }) { return Ok(None); }
            if let Some(checkpoint) = checkpoint { checkpoint.for_text(text)?; }
            Ok(Some((character, text, Some(source), checkpoint.as_ref())))
        }
        _ => Ok(None),
    }
}

fn collect(events: Res<Events<VnCommand>>, mut reader: Local<ManualEventReader<VnCommand>>,
    mut queue: ResMut<DialogueRevealQueue>, mut engine: Option<ResMut<VnEngine>>,
    mut diagnostic: Option<ResMut<crate::resources::ScriptErrorMessage>>) {
    // Clone the cursor and queue. Overflow is rejected before admitting a prefix.
    // Once admitted, commands survive Bevy's two-frame event retention in this queue.
    let mut trial_reader = reader.clone();
    let modern = engine.as_ref().is_some_and(|engine|engine.0.state.scene_execution.is_some());
    // Revoked entries are neutral before inspecting their numeric/text payload.
    let mut trial=DialogueRevealQueue{commands:queue.commands.iter().filter(|command|admitted(command,engine.as_deref(),modern)).cloned().collect()};
    for command in trial_reader.read(&events) {
        if !admitted(command,engine.as_deref(),modern){continue;}
        let bytes=trial.commands.iter().map(pending_bytes).fold(0usize,usize::saturating_add);
        if trial.commands.len() >= MAX_PENDING_DIALOGUES || pending_bytes(command) > MAX_PENDING_TEXT_BYTES.saturating_sub(bytes) {
            let error = "Dialogue pending queue capacity exceeded";
            if let Some(engine)=engine.as_deref_mut() { engine.0.renderer.scene_frame_failed=true; }
            if let Some(diagnostic)=diagnostic.as_deref_mut() { diagnostic.0=error.into(); }
            error!("{error}");
            return;
        }
        trial.commands.push_back(command.clone());
    }
    *queue = trial;
    *reader = trial_reader;
}

fn admitted(command:&VnCommand,engine:Option<&VnEngine>,modern:bool)->bool {
    match command {
        VnCommand::ScopedShowDialogue {source,..} => engine.is_some_and(|engine|engine.0.accepts_display_reveal_source(source)),
        VnCommand::ShowDialogue {..} => !modern,
        _ => false,
    }
}

fn pending_bytes(command:&VnCommand)->usize {
    match command {
        VnCommand::ShowDialogue{character,text}=>text.len().saturating_add(character.as_ref().map_or(0,String::len)),
        VnCommand::ScopedShowDialogue{character,text,checkpoint,..}=>text.len()
            .saturating_add(character.as_ref().map_or(0,String::len))
            .saturating_add(checkpoint.as_ref().map_or(0,|checkpoint|checkpoint.rendered_text.len())),
        _=>0,
    }
}

pub(crate) fn flush(engine: &mut VnEngine, typing: &TypewriterState, binding: Option<&DialogueRevealBinding>) -> Result<bool, String> {
    let Some(binding) = binding else { return Ok(false); };
    let Some(source) = &binding.source else { return Ok(false); };
    if !engine.0.accepts_display_reveal_source(source) { return Ok(false); }
    if !matches!(engine.0.displayed_interaction().map_err(|error|error.to_string())?,Some(rvn_core::Interaction::Dialogue{ref text,..}) if text==&binding.text){return Ok(false);}
    let reveal = RevealCheckpoint::observed(&binding.text, typing.visible_chars, typing.typing, typing.pause_remaining, typing.char_progress)?;
    if rvn_core::display_reveal::rendered_rich_text(&binding.text).plain_text() != typing.full_text { return Ok(false); }
    engine.0.record_display_reveal(source, reveal).map_err(|error| error.to_string())
}

pub(crate) fn flush_world(world: &mut World) -> Result<bool,String> {
    let Some(typing)=world.get_resource::<TypewriterState>() else { return Ok(false); };
    let Some(binding)=world.get_resource::<DialogueRevealBinding>() else { return Ok(false); };
    let Some(source)=binding.source.clone() else { return Ok(false); };
    let text=binding.text.clone();
    let Some(engine)=world.get_resource::<VnEngine>() else { return Ok(false); };
    if !engine.0.accepts_display_reveal_source(&source) { return Ok(false); }
    if !matches!(engine.0.displayed_interaction().map_err(|error|error.to_string())?,Some(rvn_core::Interaction::Dialogue{text:ref current,..}) if current==&text){return Ok(false);}
    let reveal=RevealCheckpoint::observed(&text,typing.visible_chars,typing.typing,typing.pause_remaining,typing.char_progress)?;
    if rvn_core::display_reveal::rendered_rich_text(&text).plain_text()!=typing.full_text { return Ok(false); }
    world.resource_mut::<VnEngine>().0.record_display_reveal(&source,reveal).map_err(|error|error.to_string())
}

pub(crate) fn restore(typing: &mut TypewriterState, text: &str, checkpoint: Option<&RevealCheckpoint>) -> Result<(), String> {
    let Some(checkpoint) = checkpoint else { return Ok(()); };
    let checkpoint = checkpoint.for_text(text)?;
    if typing.chars_per_sec <= 0.0 { typing.skip(); return Ok(()); }
    typing.visible_chars = usize::try_from(checkpoint.visible_chars).map_err(|_| "Dialogue cursor exceeds this platform")?;
    typing.typing = checkpoint.typing;
    typing.pause_remaining = checkpoint.pause_remaining;
    typing.char_progress = checkpoint.char_progress;
    Ok(())
}

pub(crate) fn feedback(engine: Option<&mut VnEngine>, typing: &TypewriterState, binding: Option<&DialogueRevealBinding>) {
    if let Some(engine)=engine {
        if let Err(error)=flush(engine,typing,binding) {
            engine.0.renderer.scene_frame_failed=true;
            error!("Dialogue reveal checkpoint refused: {error}");
        }
    }
}

#[cfg(test)]
#[path="display_reveal_tests.rs"]
pub(crate) mod tests;
