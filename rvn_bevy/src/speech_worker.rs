//! Keep optional platform services off the render/input thread. The bounded
//! mailbox replaces stale utterances rather than building an unbounded queue.
use rvn_ui::accessibility::{AccessibilitySettings, SpeechRequest};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
#[derive(Clone)]
struct Task {
    settings: AccessibilitySettings,
    request: SpeechRequest,
    generation: u64,
}
pub(super) struct Speech {
    pending: Arc<Mutex<Option<Task>>>,
    wake: Option<mpsc::SyncSender<()>>,
    generation: Arc<AtomicU64>,
    reports: Arc<Mutex<Option<(u64, Result<(), String>)>>>,
}
impl Default for Speech {
    fn default() -> Self {
        let pending = Arc::new(Mutex::new(None::<Task>));
        let work = pending.clone();
        let (wake, receiver) = mpsc::sync_channel(1);
        let generation = Arc::new(AtomicU64::new(0));
        let current = generation.clone();
        let reports = Arc::new(Mutex::new(None));
        let report = reports.clone();
        let spawn = std::thread::Builder::new()
            .name("rvn-system-speech".into())
            .spawn(move || {
                #[cfg(target_os = "linux")]
                type Backend = super::speech_linux::Backend;
                #[cfg(target_os = "windows")]
                type Backend = super::speech_windows::Backend;
                #[cfg(any(target_os = "linux", target_os = "windows"))]
                let mut backend: Option<Backend> = None;
                while receiver.recv().is_ok() {
                    let task = work.lock().ok().and_then(|mut pending| pending.take());
                    let Some(task) = task else {
                        continue;
                    };
                    #[cfg(any(target_os = "linux", target_os = "windows"))]
                    let result = (|| {
                        if matches!(task.request, SpeechRequest::Stop) {
                            return if let Some(backend) = &mut backend {
                                backend.stop()
                            } else {
                                Ok(())
                            };
                        }
                        if backend.is_none() {
                            backend = Some(Backend::new()?);
                        }
                        let voice = backend.as_mut().unwrap();
                        voice.configure(&task.settings)?;
                        match &task.request {
                            SpeechRequest::Speak(text) => voice.speak(text),
                            SpeechRequest::Stop => voice.stop(),
                        }
                    })();
                    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
                    let result = Err("System speech is unsupported on this platform".into());
                    if result.is_err() {
                        #[cfg(any(target_os = "linux", target_os = "windows"))]
                        {
                            backend = None;
                        }
                    }
                    if current.load(Ordering::Acquire) == task.generation {
                        if let Ok(mut report) = report.lock() {
                            *report = Some((task.generation, result));
                        }
                    }
                }
                // Dropping the connection purges only our own outstanding speech.
            });
        Self {
            pending,
            wake: spawn.ok().map(|_| wake),
            reports,
            generation,
        }
    }
}
impl Speech {
    pub(super) fn request(
        &self,
        settings: &AccessibilitySettings,
        request: SpeechRequest,
    ) -> Result<(), String> {
        let Some(wake) = &self.wake else {
            return Err("Could not start the system speech worker".into());
        };
        settings.validate()?;
        if let SpeechRequest::Speak(text) = &request {
            rvn_ui::accessibility::validate_speech(text)?;
        }
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        *self.pending.lock().map_err(|_| "Speech worker failed")? = Some(Task {
            settings: settings.clone(),
            request,
            generation,
        });
        match wake.try_send(()) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => Ok(()),
            Err(_) => Err("System speech worker is unavailable".into()),
        }
    }
    pub(super) fn report(&self) -> Option<Result<(), String>> {
        let (generation, result) = self.reports.lock().ok()?.take()?;
        (generation == self.generation.load(Ordering::Acquire)).then_some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_discards_stale_voice_errors_and_requests_stay_bounded() {
        let (wake, receiver) = mpsc::sync_channel(1);
        let speech = Speech {
            pending: Arc::new(Mutex::new(None)),
            wake: Some(wake),
            generation: Arc::new(AtomicU64::new(0)),
            reports: Arc::new(Mutex::new(None)),
        };
        speech
            .request(
                &AccessibilitySettings::default(),
                SpeechRequest::Speak("Éloïse".into()),
            )
            .unwrap();
        *speech.reports.lock().unwrap() = Some((1, Err("Old language missing".into())));
        speech
            .request(&AccessibilitySettings::default(), SpeechRequest::Stop)
            .unwrap();
        assert!(speech.report().is_none());
        assert_eq!(speech.generation.load(Ordering::Acquire), 2);
        assert!(matches!(
            speech.pending.lock().unwrap().as_ref().unwrap().request,
            SpeechRequest::Stop
        ));
        assert!(receiver.try_recv().is_ok());
        assert!(receiver.try_recv().is_err());
        *speech.reports.lock().unwrap() = Some((2, Ok(())));
        assert_eq!(speech.report(), Some(Ok(())));
        assert!(speech.report().is_none());
        let before = speech.generation.load(Ordering::Acquire);
        assert!(speech
            .request(
                &AccessibilitySettings::default(),
                SpeechRequest::Speak("\0".into())
            )
            .is_err());
        assert_eq!(speech.generation.load(Ordering::Acquire), before);
    }
}
