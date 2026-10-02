//! Optional system Speech Dispatcher, dynamically loaded. Only this client's
//! utterances are cancelled; no global desktop speech settings are modified.
use rvn_ui::accessibility::AccessibilitySettings;
use std::{
    ffi::{c_void, CStr, CString},
    os::raw::{c_char, c_int},
};
type Connection = *mut c_void;
type Control = unsafe extern "C" fn(Connection) -> c_int;
type Number = unsafe extern "C" fn(Connection, c_int) -> c_int;
type Text = unsafe extern "C" fn(Connection, *const c_char) -> c_int;
#[repr(C)]
struct Voice {
    name: *mut c_char,
    language: *mut c_char,
    variant: *mut c_char,
}
pub(super) struct Backend {
    connection: Connection,
    close: unsafe extern "C" fn(Connection),
    cancel: Control,
    say: unsafe extern "C" fn(Connection, c_int, *const c_char) -> c_int,
    rate: Number,
    volume: Number,
    language: Text,
    mode: Number,
    list: unsafe extern "C" fn(Connection) -> *mut *mut Voice,
    free: unsafe extern "C" fn(*mut *mut Voice),
    _library: libloading::Library,
}
impl Backend {
    pub(super) fn new() -> Result<Self, String> {
        // The library and opaque connection stay on a single dedicated thread.
        // Every signature is from Speech Dispatcher's stable libspeechd C API.
        unsafe {
            let library=libloading::Library::new("libspeechd.so.2").map_err(|_|"Speech Dispatcher is not installed. Install or enable your system speech service.".to_string())?;
            let open = *library
                .get::<unsafe extern "C" fn(
                    *const c_char,
                    *const c_char,
                    *const c_char,
                    c_int,
                ) -> Connection>(b"spd_open\0")
                .map_err(|e| e.to_string())?;
            let close = *library
                .get::<unsafe extern "C" fn(Connection)>(b"spd_close\0")
                .map_err(|e| e.to_string())?;
            let cancel = *library
                .get::<Control>(b"spd_cancel\0")
                .map_err(|e| e.to_string())?;
            let say = *library
                .get::<unsafe extern "C" fn(Connection, c_int, *const c_char) -> c_int>(
                    b"spd_say\0",
                )
                .map_err(|e| e.to_string())?;
            let rate = *library
                .get::<Number>(b"spd_set_voice_rate\0")
                .map_err(|e| e.to_string())?;
            let volume = *library
                .get::<Number>(b"spd_set_volume\0")
                .map_err(|e| e.to_string())?;
            let language = *library
                .get::<Text>(b"spd_set_language\0")
                .map_err(|e| e.to_string())?;
            let mode = *library
                .get::<Number>(b"spd_set_data_mode\0")
                .map_err(|e| e.to_string())?;
            let list = *library
                .get::<unsafe extern "C" fn(Connection) -> *mut *mut Voice>(
                    b"spd_list_synthesis_voices\0",
                )
                .map_err(|e| e.to_string())?;
            let free = *library
                .get::<unsafe extern "C" fn(*mut *mut Voice)>(b"free_spd_voices\0")
                .map_err(|e| e.to_string())?;
            let client = CString::new("rust-vn").unwrap();
            let connection = open(client.as_ptr(), client.as_ptr(), std::ptr::null(), 0);
            if connection.is_null() {
                return Err("Speech Dispatcher is unavailable. Enable your system speech service and try again.".into());
            }
            let backend = Self {
                connection,
                close,
                cancel,
                say,
                rate,
                volume,
                language,
                mode,
                list,
                free,
                _library: library,
            };
            let voices = (backend.list)(connection);
            let available = !voices.is_null() && !(*voices).is_null();
            if !voices.is_null() {
                (backend.free)(voices);
            }
            if !available {
                return Err("No system speech voice is available. Install a Speech Dispatcher voice and try again.".into());
            }
            Ok(backend)
        }
    }
    pub(super) fn configure(&mut self, settings: &AccessibilitySettings) -> Result<(), String> {
        settings.validate()?;
        unsafe {
            let rate = if settings.speech_rate < 1.0 {
                (settings.speech_rate - 1.0) * 200.0
            } else {
                (settings.speech_rate - 1.0) * 100.0
            };
            if (self.mode)(self.connection, 0) < 0
                || (self.rate)(self.connection, rate.round() as c_int) < 0
                || (self.volume)(
                    self.connection,
                    (settings.speech_volume * 200.0 - 100.0).round() as c_int,
                ) < 0
            {
                return Err("Speech Dispatcher rejected the voice settings".into());
            }
            if let Some(language) = &settings.language {
                // SET LANGUAGE can succeed even when no voice speaks it.
                // Select an actually installed language, including the base
                // language fallback used by SAPI and the Web adapter.
                let voices = (self.list)(self.connection);
                let mut languages = Vec::new();
                if !voices.is_null() {
                    for index in 0..16_384 {
                        let voice = *voices.add(index);
                        if voice.is_null() {
                            break;
                        }
                        if !(*voice).language.is_null() {
                            let value = CStr::from_ptr((*voice).language).to_string_lossy();
                            if value.len() <= 64 {
                                languages.push(value.into_owned());
                            }
                        }
                    }
                    (self.free)(voices);
                }
                let requested = language.to_ascii_lowercase().replace('_', "-");
                let chosen = languages
                    .iter()
                    .find(|value| value.to_ascii_lowercase().replace('_', "-") == requested)
                    .or_else(|| {
                        languages.iter().find(|value| {
                            value.to_ascii_lowercase().split(['-', '_']).next()
                                == requested.split('-').next()
                        })
                    })
                    .ok_or_else(|| format!("No system speech voice is available for {language}"))?;
                let language = CString::new(chosen.as_str()).map_err(|e| e.to_string())?;
                if (self.language)(self.connection, language.as_ptr()) < 0 {
                    return Err("Speech Dispatcher rejected the requested language".into());
                }
            }
        }
        Ok(())
    }
    pub(super) fn speak(&mut self, text: &str) -> Result<(), String> {
        rvn_ui::accessibility::validate_speech(text)?;
        let text = CString::new(text).map_err(|e| e.to_string())?;
        self.stop()?;
        if unsafe { (self.say)(self.connection, 2, text.as_ptr()) } < 0 {
            Err("Speech Dispatcher could not speak this text".into())
        } else {
            Ok(())
        }
    }
    pub(super) fn stop(&mut self) -> Result<(), String> {
        if unsafe { (self.cancel)(self.connection) } < 0 {
            Err("Speech Dispatcher could not stop playback".into())
        } else {
            Ok(())
        }
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        unsafe {
            (self.cancel)(self.connection);
            (self.close)(self.connection);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Requires a real enabled Linux Speech Dispatcher service; produces speech"]
    fn system_voice_accepts_literal_unicode_settings_and_client_scoped_stop() {
        let mut voice = Backend::new().unwrap();
        voice
            .configure(&AccessibilitySettings {
                language: Some("fr-FR".into()),
                speech_volume: 0.3,
                ..Default::default()
            })
            .unwrap();
        voice
            .speak("Test rust-VN. La lecture vocale utilise une voix système.")
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1200));
        voice.stop().unwrap();
        assert!(voice.speak("bad\0text").is_err());
        assert!(voice
            .configure(&AccessibilitySettings {
                language: Some("zz-QQ".into()),
                ..Default::default()
            })
            .unwrap_err()
            .contains("No system speech voice"));
    }
}
