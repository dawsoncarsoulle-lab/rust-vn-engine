//! SAPI is initialized and used exclusively on the speech worker's COM apartment.
use rvn_ui::accessibility::AccessibilitySettings;
use windows::{
    core::{w, PCWSTR},
    Win32::{Globalization::LocaleNameToLCID, Media::Speech::*, System::Com::*},
};
pub(super) struct Backend {
    voice: Option<ISpVoice>,
    default_voice: Option<ISpObjectToken>,
}
impl Backend {
    pub(super) fn new() -> Result<Self, String> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|e| format!("Could not initialize Windows speech: {e}"))?;
            let result = (|| {
                let voice: ISpVoice = CoCreateInstance(&SpVoice, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| format!("Windows speech is unavailable: {e}"))?;
                let default_voice = voice
                    .GetVoice()
                    .map_err(|_| "No Windows speech voice is installed".to_string())?;
                Ok(Self {
                    voice: Some(voice),
                    default_voice: Some(default_voice),
                })
            })();
            if result.is_err() {
                CoUninitialize();
            }
            result
        }
    }
    pub(super) fn configure(&mut self, settings: &AccessibilitySettings) -> Result<(), String> {
        settings.validate()?;
        unsafe {
            let voice = self.voice.as_ref().unwrap();
            voice
                .SetRate(
                    (settings.speech_rate.log2() * 10.0)
                        .round()
                        .clamp(-10.0, 10.0) as i32,
                )
                .map_err(|e| e.to_string())?;
            voice
                .SetVolume((settings.speech_volume * 100.0).round() as u16)
                .map_err(|e| e.to_string())?;
            if let Some(language) = &settings.language {
                let wide: Vec<u16> = language.encode_utf16().chain(Some(0)).collect();
                let lcid = LocaleNameToLCID(PCWSTR(wide.as_ptr()), 0);
                if lcid == 0 {
                    return Err(format!(
                        "Windows does not recognize speech language {language}"
                    ));
                }
                let category: ISpObjectTokenCategory =
                    CoCreateInstance(&SpObjectTokenCategory, None, CLSCTX_INPROC_SERVER)
                        .map_err(|e| e.to_string())?;
                category
                    .SetId(SPCAT_VOICES, false)
                    .map_err(|e| e.to_string())?;
                let tokens = category
                    .EnumTokens(PCWSTR::null(), PCWSTR::null())
                    .map_err(|e| e.to_string())?;
                let mut count = 0;
                tokens.GetCount(&mut count).map_err(|e| e.to_string())?;
                let mut fallback = None;
                let mut exact = None;
                for index in 0..count.min(1024) {
                    let token = tokens.Item(index).map_err(|e| e.to_string())?;
                    let Ok(attributes) = token.OpenKey(w!("Attributes")) else {
                        continue;
                    };
                    let Ok(value) = attributes.GetStringValue(w!("Language")) else {
                        continue;
                    };
                    let locales = value.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(value.0.cast()));
                    for id in locales
                        .split(';')
                        .filter_map(|id| u32::from_str_radix(id.trim(), 16).ok())
                    {
                        if id == lcid {
                            exact = Some(token.clone());
                        }
                        if id & 0x3ff == lcid & 0x3ff && fallback.is_none() {
                            fallback = Some(token.clone());
                        }
                    }
                }
                let token = exact.or(fallback).ok_or_else(|| {
                    format!("No Windows speech voice is installed for {language}")
                })?;
                voice.SetVoice(&token).map_err(|e| e.to_string())?;
            } else {
                voice
                    .SetVoice(self.default_voice.as_ref().unwrap())
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    pub(super) fn speak(&mut self, text: &str) -> Result<(), String> {
        rvn_ui::accessibility::validate_speech(text)?;
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        // Force literal text: authored speech must never be interpreted as a
        // filename, SSML, XML directive or embedded audio resource.
        unsafe {
            self.voice
                .as_ref()
                .unwrap()
                .Speak(
                    PCWSTR(wide.as_ptr()),
                    (SPF_ASYNC.0 | SPF_PURGEBEFORESPEAK.0 | SPF_IS_NOT_XML.0) as u32,
                    None,
                )
                .map_err(|e| format!("Windows speech failed: {e}"))
        }
    }
    pub(super) fn stop(&mut self) -> Result<(), String> {
        unsafe {
            self.voice
                .as_ref()
                .unwrap()
                .Speak(
                    PCWSTR::null(),
                    (SPF_ASYNC.0 | SPF_PURGEBEFORESPEAK.0) as u32,
                    None,
                )
                .map_err(|e| e.to_string())
        }
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        let _ = self.stop();
        self.voice.take();
        self.default_voice.take();
        unsafe {
            CoUninitialize();
        }
    }
}
