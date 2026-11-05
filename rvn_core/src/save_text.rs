//! Typed traversal of every saved Value location. JSON object keys are never
//! inspected for a string named "Text": ordinary dictionaries remain legacy.
use super::*;

fn required_text_format(value: &Value) -> Result<u32, String> {
    Ok(match value {
        Value::Enum(_) => return Err("Enum values require the source-admitted nominal checkpoint16".into()),
        Value::Widget(_) | Value::WidgetClass(_) => return Err("Widget references require the admitted Save15 adapter".into()),
        Value::Text(text) => if text.has_name_string_table()? { NAME_STRING_TABLE_SAVE_FORMAT_VERSION } else if text.has_string_table()? { STRING_TABLE_SAVE_FORMAT_VERSION } else { TEXT_SAVE_FORMAT_VERSION },
        Value::List(items) => items.iter().try_fold(0, |required, value| Ok::<_, String>(required.max(required_text_format(value)?)))?,
        Value::Dict(items) => items.values().try_fold(0, |required, value| Ok::<_, String>(required.max(required_text_format(value)?)))?,
        _ => 0,
    })
}

fn visit_context(
    scene: Option<&crate::scene::SceneExecution>, ui: &crate::ui::UiState,
    dialogue: Option<&crate::types::DialogueSnapshot>,
    mut value: impl FnMut(&Value) -> Result<(), String>,
) -> Result<(), String> {
    fn interfaces(ui: &crate::ui::UiState, value: &mut impl FnMut(&Value) -> Result<(), String>) -> Result<(), String> {
        for screen in ui.screens.iter().filter(|screen| screen.host_role.is_none()) {
            for argument in &screen.arguments { value(argument)?; }
            for item in screen.values.values() { value(item)?; }
            for canvas in screen.canvas_states.values() { value(&canvas.state)?; }
        }
        Ok(())
    }
    let values = |items: &HashMap<String, Value>, value: &mut dyn FnMut(&Value) -> Result<(), String>| {
        for item in items.values() { value(item)?; }
        Ok::<_, String>(())
    };
    interfaces(ui, &mut value)?;
    if let Some(dialogue) = dialogue {
        values(&dialogue.vars, &mut value)?;
        if let Some(globals) = &dialogue.lexical_globals { values(globals, &mut value)?; }
    }
    if let Some(scene) = scene {
        values(&scene.pre_scene_temporaries, &mut value)?;
        interfaces(&scene.game_tasks.suspended_ui, &mut value)?;
        let mut activation = |active: &crate::scene::SceneActivation| {
            values(&active.locals, &mut value)?;
            values(&active.temporaries, &mut value)?;
            interfaces(&active.tasks.suspended_ui, &mut value)
        };
        if let Some(active) = &scene.active { activation(active)?; }
        for frame in &scene.frames { if let Some(parked) = &frame.parked { activation(parked)?; } }
        for frame in &scene.frames {
            if let Some(items) = &frame.pre_scene_temporaries { values(items, &mut value)?; }
            if let Some(tasks) = &frame.pre_scene_resources { interfaces(&tasks.suspended_ui, &mut value)?; }
        }
        if let Some(display) = &scene.display {
            values(&display.variables, &mut value)?;
            values(&display.lexical_globals, &mut value)?;
        }
        scene.visit_native_loop_frames(|frame| {
            for record in &frame.gather { record.visit_value_roots(&mut value)?; }
            Ok::<_, String>(())
        })?;
    }
    Ok(())
}

pub(super) fn state_required_text_format(state: &GameState) -> Result<u32, String> {
    let mut required = 0;
    let mut check = |value: &Value| {
        crate::value_limits::value_work(value).map_err(|error| error.to_string())?;
        required = required.max(required_text_format(value)?);
        Ok(())
    };
    for value in state.vars.values() { check(value)?; }
    visit_context(state.scene_execution.as_ref(), &state.ui, state.last_dialogue.as_ref(), check)?;
    Ok(required)
}

pub(super) fn validate(data: &SaveData) -> Result<(), SaveError> {
    fn check(value: &SaveValue, depth: usize, nodes: &mut usize, bytes: &mut usize, required: &mut u32) -> Result<(), String> {
        *nodes += 1;
        if depth > 64 || *nodes > 65_536 { return Err("Saved values exceed their depth or node budget".into()); }
        let charge = |size: usize, bytes: &mut usize| {
            *bytes = bytes.checked_add(size).ok_or("Saved value size overflow")?;
            if *bytes > crate::value_limits::MAX_VALUE_BYTES { return Err("Saved value exceeds 1 MiB".into()); }
            Ok::<_, String>(())
        };
        match value {
            SaveValue::Enum(_) => return Err("Enum values require the source-admitted nominal checkpoint16".into()),
            SaveValue::Widget(_) | SaveValue::WidgetClass(_) => return Err("Widget references require the admitted Save15 adapter".into()),
            SaveValue::Text(text) => {
                text.visit_storage(depth + 1, nodes, bytes)?;
                *required = (*required).max(if text.has_name_string_table()? { NAME_STRING_TABLE_SAVE_FORMAT_VERSION } else if text.has_string_table()? { STRING_TABLE_SAVE_FORMAT_VERSION } else { TEXT_SAVE_FORMAT_VERSION });
            },
            SaveValue::Str(text) => charge(text.len(), bytes)?,
            SaveValue::Float(number) if !number.is_finite() => return Err("Saved number is not finite".into()),
            SaveValue::List(items) => {
                if items.len() > crate::value_limits::MAX_COLLECTION_ITEMS { return Err("Saved collection exceeds 4,096 items".into()); }
                for value in items { check(value, depth + 1, nodes, bytes, required)?; }
            },
            SaveValue::Dict(items) => {
                if items.len() > crate::value_limits::MAX_COLLECTION_ITEMS { return Err("Saved collection exceeds 4,096 items".into()); }
                for (key, value) in items { charge(key.len(), bytes)?; check(value, depth + 1, nodes, bytes, required)?; }
            },
            _ => {},
        }
        Ok(())
    }
    let mut required = 0;
    for value in data.vars.values() {
        check(value, 0, &mut 0, &mut 0, &mut required).map_err(SaveError::Incompatible)?;
    }
    visit_context(data.scene_execution.as_ref(), &data.ui, data.last_dialogue.as_ref(), |value| {
        crate::value_limits::value_work(value).map_err(|error| error.to_string())?;
        required = required.max(required_text_format(value)?);
        Ok(())
    }).map_err(SaveError::Incompatible)?;
    if required > data.format_version {
        return Err(SaveError::Incompatible(format!("Portable Text values require save format {required}")));
    }
    Ok(())
}
