//! Initial normal selection closes actual catalogues without rewriting UI choices.
//! Dynamic requests and cross-domain restore require their own host preparation.
use crate::portable_text_resources::LocaleDeclarations;
use crate::LocaleConfigFile;

/// Called only when the real loader already selected a closed provider.
/// `explicit_closed` preserves the existing opt-in/debug declared-domain policy.
/// The compatible normal provider owns UI choices plus its two selected values;
/// LocaleConfigFile remains unchanged and still supplies the UI resource.
pub(super) fn initial_closed_declarations(
    config: &LocaleConfigFile,
    explicit_closed: bool,
) -> LocaleDeclarations {
    let available = if explicit_closed {
        config.available.clone()
    } else {
        let mut seen = std::collections::HashSet::new();
        let mut closure = Vec::new();
        for language in config.available.iter()
            .chain(std::iter::once(&config.default))
            .chain(std::iter::once(&config.current))
        {
            if seen.insert(language.as_str()) {
                closure.push(language.clone());
            }
        }
        closure
    };
    let files = available.iter()
        .map(|language| (language.clone(), format!("{language}.toml")))
        .collect();
    LocaleDeclarations {
        default_lang: config.default.clone(),
        current_lang: config.current.clone(),
        available,
        files,
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "locale_selection_tests.rs"]
mod tests;
