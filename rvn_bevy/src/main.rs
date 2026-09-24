#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        std::env::current_exe()
            .ok()
            .map(|p| p.with_file_name("data"))
            .filter(|p| p.join("rvn.toml").is_file())
            .unwrap_or_else(|| std::path::PathBuf::from("./data"))
            .to_string_lossy()
            .into_owned()
    });

    if let Err(e) = rvn_bevy::run_game_from_path(path) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
export function install_audio_context_unlock() {
    if (globalThis.__rvnAudioUnlockInstalled) return;
    globalThis.__rvnAudioUnlockInstalled = true;

    const contexts = globalThis.__rvnAudioContexts || [];
    globalThis.__rvnAudioContexts = contexts;
    const OriginalAudioContext = globalThis.AudioContext || globalThis.webkitAudioContext;
    if (!OriginalAudioContext) return;

    function WrappedAudioContext(...args) {
        const context = new OriginalAudioContext(...args);
        if (!contexts.includes(context)) {
            contexts.push(context);
        }
        return context;
    }
    WrappedAudioContext.prototype = OriginalAudioContext.prototype;
    Object.setPrototypeOf(WrappedAudioContext, OriginalAudioContext);

    if (globalThis.AudioContext) globalThis.AudioContext = WrappedAudioContext;
    if (globalThis.webkitAudioContext) globalThis.webkitAudioContext = WrappedAudioContext;

    const resumeAll = () => {
        for (const context of contexts) {
            if (context && context.state !== 'running') {
                context.resume().catch(() => {});
            }
        }
    };
    globalThis.__rvnResumeAllAudioContexts = resumeAll;

    globalThis.addEventListener('pointerdown', resumeAll, true);
    globalThis.addEventListener('mousedown', resumeAll, true);
    globalThis.addEventListener('touchstart', resumeAll, true);
    globalThis.addEventListener('keydown', resumeAll, true);
}
"#)]
extern "C" {
    fn install_audio_context_unlock();
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    install_audio_context_unlock();
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = rvn_bevy::run_game_web(".").await {
            web_sys::console::error_1(&e.into());
        }
    });
}
