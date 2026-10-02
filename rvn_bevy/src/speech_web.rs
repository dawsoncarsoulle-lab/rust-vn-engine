use rvn_ui::accessibility::{AccessibilitySettings, SpeechRequest};
use std::{cell::RefCell, collections::VecDeque};
use wasm_bindgen::prelude::*;
thread_local! {static REPORTS:RefCell<VecDeque<Result<(),String>>>=RefCell::new(VecDeque::new());}
#[wasm_bindgen(inline_js = r#"
let speechGeneration=0;
export function rvnSpeechStop(){++speechGeneration;window.speechSynthesis?.cancel();}
export async function rvnSpeechSpeak(text,rate,volume,language,report){
    const generation=++speechGeneration;
    try {
    const synth=window.speechSynthesis;
    if(!synth||!window.SpeechSynthesisUtterance){report('Speech synthesis is unavailable in this browser');return;}
    synth.cancel();
    let voices=synth.getVoices();
    if(!voices.length){
        await new Promise(resolve=>{
            let timer;
            const done=()=>{clearTimeout(timer);synth.removeEventListener('voiceschanged',done);resolve();};
            synth.addEventListener('voiceschanged',done);timer=setTimeout(done,1500);
        });
        voices=synth.getVoices();
    }
    if(generation!==speechGeneration)return;
    if(!voices.length){report('No browser speech voice is available. Install a system voice or use another browser.');return;}
    const requested=(language||document.documentElement.lang||navigator.language||'').toLowerCase();
    const voice=voices.find(voice=>voice.lang.toLowerCase()===requested)||voices.find(voice=>voice.lang.split('-')[0].toLowerCase()===requested.split('-')[0]);
    if(language&&!voice){report(`No browser speech voice is available for ${language}`);return;}
    const utterance=new SpeechSynthesisUtterance(text);utterance.voice=voice||voices.find(voice=>voice.default)||voices[0];
    utterance.lang=utterance.voice.lang;utterance.rate=rate;utterance.volume=volume;
    utterance.onerror=event=>{if(generation===speechGeneration&&!['canceled','interrupted'].includes(event.error))report(`Browser speech failed: ${event.error}`);};
    utterance.onstart=()=>{if(generation===speechGeneration)report('');};
    synth.speak(utterance);
    } catch(problem) {if(generation===speechGeneration)report(`Browser speech failed: ${problem.message||problem}`);}
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    fn rvnSpeechStop() -> Result<(), JsValue>;
    fn rvnSpeechSpeak(
        text: &str,
        rate: f32,
        volume: f32,
        language: &str,
        report: &js_sys::Function,
    );
}
pub(super) struct Speech {
    callback: Closure<dyn FnMut(String)>,
}
impl Default for Speech {
    fn default() -> Self {
        Self {
            callback: Closure::new(|problem: String| {
                REPORTS.with(|reports| {
                    let mut reports = reports.borrow_mut();
                    if reports.len() == 8 {
                        reports.pop_front();
                    }
                    reports.push_back(if problem.is_empty() {
                        Ok(())
                    } else {
                        Err(problem)
                    });
                });
            }),
        }
    }
}
impl Speech {
    pub(super) fn request(
        &self,
        settings: &AccessibilitySettings,
        request: SpeechRequest,
    ) -> Result<(), String> {
        settings.validate()?;
        match request {
            SpeechRequest::Stop => {
                rvnSpeechStop().map_err(|problem| format!("Browser speech failed: {problem:?}"))?
            }
            SpeechRequest::Speak(text) => {
                rvn_ui::accessibility::validate_speech(&text)?;
                rvnSpeechSpeak(
                    &text,
                    settings.speech_rate,
                    settings.speech_volume,
                    settings.language.as_deref().unwrap_or(""),
                    self.callback.as_ref().unchecked_ref(),
                );
            }
        }
        Ok(())
    }
    pub(super) fn report(&self) -> Option<Result<(), String>> {
        REPORTS.with(|reports| reports.borrow_mut().pop_front())
    }
}
impl Drop for Speech {
    fn drop(&mut self) {
        let _ = rvnSpeechStop();
    }
}
