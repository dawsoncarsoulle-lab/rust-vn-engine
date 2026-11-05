use crate::resources::{LocaleConfig, VnEngine};
use bevy::prelude::*;

/// Hot-reload des fichiers de locale.
/// Surveille les timestamps et recharge si le fichier a changé sur le disque.
pub fn locale_reload_system(
    mut engine: ResMut<VnEngine>,
    mut locale_cfg: ResMut<LocaleConfig>,
    mut output: EventWriter<crate::vn_command::VnCommand>,
    mut error: ResMut<crate::resources::ScriptErrorMessage>,
    mut next: ResMut<NextState<crate::resources::VnState>>,
    typing: Option<Res<crate::resources::TypewriterState>>,
    binding: Option<Res<crate::display_reveal::DialogueRevealBinding>>,
) {
    let Some(locale) = &engine.0.locale else {
        return;
    };
    if locale.provider_policy()==rvn_core::locale::LocaleProviderPolicy::FrozenCatalogs {return;}
    let current = locale.current_lang().to_string();
    let default = locale.default_lang().to_string();

    let mut needs_reload = false;

    let current_path = locale.locale_path(&current);
    if let Ok(meta) = std::fs::metadata(&current_path) {
        if let Ok(modified) = meta.modified() {
            if modified > locale_cfg.last_modified_current {
                locale_cfg.last_modified_current = modified;
                needs_reload = true;
            }
        }
    }

    if current != default {
        let default_path = locale.locale_path(&default);
        if let Ok(meta) = std::fs::metadata(&default_path) {
            if let Ok(modified) = meta.modified() {
                if modified > locale_cfg.last_modified_default {
                    locale_cfg.last_modified_default = modified;
                    needs_reload = true;
                }
            }
        }
    }

    if needs_reload {
        if let Some(typing)=typing.as_deref() {
            if let Err(problem)=crate::display_reveal::flush(&mut engine,typing,binding.as_deref()) {
                engine.0.renderer.scene_frame_failed=true;error.0=problem;next.set(crate::resources::VnState::Error);return;
            }
        }
        if let Some(locale) = &mut engine.0.locale {
            locale.reload_current();
            info!("[locale] hot-reloaded — langue : {}", current);
        }
        match refresh_localized_interfaces(&mut engine) {
            Ok(()) => {
                for command in engine.0.renderer.take_pending() {
                    output.send(command);
                }
                if engine.0.state.scene_execution.is_some() {
                    match engine.0.displayed_interaction() {
                        Ok(Some(rvn_core::Interaction::Dialogue{character,text}))=>{output.send(crate::display_reveal::command(&engine,character,text));},
                        Ok(_)=>{},Err(problem)=>{error.0=problem.to_string();next.set(crate::resources::VnState::Error);}
                    }
                }
            }
            Err(problem) => {
                error.0 = format!("Interface locale reload failed: {problem}");
                next.set(crate::resources::VnState::Error);
            }
        }
    }
}

/// Surveille la variable `__lang` dans le moteur.
/// `set __lang = "en"` dans le script change la langue instantanément.
pub fn locale_lang_watch_system(
    mut engine: ResMut<VnEngine>,
    mut output: EventWriter<crate::vn_command::VnCommand>,
    mut error: ResMut<crate::resources::ScriptErrorMessage>,
    mut next: ResMut<NextState<crate::resources::VnState>>,
    typing: Option<Res<crate::resources::TypewriterState>>,
    binding: Option<Res<crate::display_reveal::DialogueRevealBinding>>,
) {
    use rvn_parser::Value;

    let lang_opt = engine.0.get_var("__lang").and_then(|v| {
        if let Value::Str(s) = v {
            Some(s.clone())
        } else {
            None
        }
    });

    if let Some(lang) = lang_opt {
        let current = engine
            .0
            .locale
            .as_ref()
            .map(|l| l.current_lang().to_string())
            .unwrap_or_default();

        if lang != current {
            if let Some(typing)=typing.as_deref() {
                if let Err(problem)=crate::display_reveal::flush(&mut engine,typing,binding.as_deref()) {
                    engine.0.renderer.scene_frame_failed=true;error.0=problem;next.set(crate::resources::VnState::Error);return;
                }
            }
            if let Some(locale) = &mut engine.0.locale {
                match locale.set_language(&lang) {
                    Ok(_) => info!("[locale] langue changée → {}", lang),
                    Err(e) => error!("[locale] impossible de charger `{}` : {}", lang, e),
                }
            }
            match refresh_localized_interfaces(&mut engine) {
                Ok(()) => {
                    for command in engine.0.renderer.take_pending() {
                        output.send(command);
                    }
                    if engine.0.state.scene_execution.is_some() {
                        match engine.0.displayed_interaction() {
                            Ok(Some(rvn_core::Interaction::Dialogue{character,text}))=>{output.send(crate::display_reveal::command(&engine,character,text));},
                            Ok(_)=>{},Err(problem)=>{error.0=problem.to_string();next.set(crate::resources::VnState::Error);}
                        }
                    }
                }
                Err(problem) => {
                    error.0 = format!("Interface language refresh failed: {problem}");
                    next.set(crate::resources::VnState::Error);
                }
            }
        }
    }
}

pub(crate) fn refresh_localized_interfaces(engine: &mut VnEngine) -> Result<(), rvn_core::RuntimeError> {
    engine.0.refresh_interfaces()?;
    if engine.0.state.scene_execution.is_some()
        && matches!(engine.0.displayed_interaction()?, Some(rvn_core::Interaction::Choice { .. }))
    {
        engine.0.publish_current_scene_interaction()?;
    }
    Ok(())
}
