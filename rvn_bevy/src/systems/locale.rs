use crate::resources::{LocaleConfig, VnEngine};
use bevy::prelude::*;

/// Hot-reload des fichiers de locale.
/// Surveille les timestamps et recharge si le fichier a changé sur le disque.
pub fn locale_reload_system(mut engine: ResMut<VnEngine>, mut locale_cfg: ResMut<LocaleConfig>) {
    let Some(locale) = &engine.0.locale else {
        return;
    };
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
        if let Some(locale) = &mut engine.0.locale {
            locale.reload_current();
            info!("[locale] hot-reloaded — langue : {}", current);
        }
    }
}

/// Surveille la variable `__lang` dans le moteur.
/// `set __lang = "en"` dans le script change la langue instantanément.
pub fn locale_lang_watch_system(mut engine: ResMut<VnEngine>) {
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
            if let Some(locale) = &mut engine.0.locale {
                match locale.set_language(&lang) {
                    Ok(_) => info!("[locale] langue changée → {}", lang),
                    Err(e) => error!("[locale] impossible de charger `{}` : {}", lang, e),
                }
            }
        }
    }
}
