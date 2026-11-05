//! Real WAV/Rodio Source and shared gate behind the Core renderer boundary.
//! No Bevy World, Decodable wrapper, sink, device or audible parity is claimed.
use super::*;
use pcm::{CheckpointGate, CheckpointPcm, PcmClip, PcmSource};
use rvn_core::audio_checkpoint::{AudioCheckpointRow, AudioCheckpointSnapshot};
use rvn_core::audio_playback::{AudioFamily, AudioPlaybackView};
use rvn_core::scene_resources::TargetIdentity;
#[path = "checkpoint_pcm.rs"]
mod pcm;
struct Bound {
    view: AudioPlaybackView,
    reader: CheckpointPcm,
    source: PcmSource,
}
struct BoundMusic {
    view: rvn_core::audio_checkpoint::MusicCheckpointView,
    reader: CheckpointPcm,
    source: PcmSource,
}
#[derive(Default)]
struct Readers {
    instance: Option<u64>,
    epoch: Option<u64>,
    bound: Vec<Bound>,
    music: Vec<BoundMusic>,
}
#[derive(Clone, Default)]
pub(super) struct PcmHost {
    graphics: Host,
    gate: CheckpointGate,
    readers: Arc<Mutex<Readers>>,
    clips: Arc<Mutex<std::collections::BTreeMap<String, Arc<PcmClip>>>>,
}
impl PcmHost {
    pub(super) fn publication_count(&self) -> usize {
        self.graphics.batches.lock().unwrap().len()
    }
    fn file(file: &str) -> Result<std::path::PathBuf, String> {
        let name = match file {
            "sfx/host.wav" => "host.wav",
            "voice/voice.wav" => "voice.wav",
            "music/host.wav" => "host.wav",
            _ => return Err("Fixture asset is not authorized".into()),
        };
        Ok(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures/pcm")
            .join(name))
    }
    fn decoded(&self, file: &str) -> Result<Arc<PcmClip>, String> {
        let bytes = std::fs::read(Self::file(file)?).map_err(|e| e.to_string())?;
        let clip = PcmClip::decode(file, &bytes)?;
        self.clips
            .lock()
            .map_err(|_| "Fixture clip cache poisoned")?
            .insert(file.into(), clip.clone());
        Ok(clip)
    }
    pub(super) fn bind_actual(
        &self,
        state: &rvn_core::GameState,
        instance: u64,
        epoch: u64,
    ) -> Result<(), String> {
        let views = rvn_core::audio_playback::describe(state, epoch)?;
        let music = rvn_core::audio_checkpoint::music_views(state, epoch)?;
        let mut live = self
            .readers
            .lock()
            .map_err(|_| "Fixture reader registry poisoned")?;
        let same = live.instance == Some(instance) && live.epoch == Some(epoch);
        let mut old = std::mem::take(&mut live.bound);
        let mut old_music = std::mem::take(&mut live.music);
        let mut bound = Vec::new();
        for view in views {
            if same {
                if let Some(index) = old.iter().position(|b| {
                    b.view.token == view.token
                        && b.view.playback.file == view.playback.file
                        && b.view.playback.family == view.playback.family
                }) {
                    let mut row = old.remove(index);
                    row.view = view;
                    bound.push(row);
                    continue;
                }
            }
            let cached = {
                self.clips
                    .lock()
                    .map_err(|_| "Fixture clip cache poisoned")?
                    .get(&view.playback.file)
                    .cloned()
            };
            let clip = match cached {
                Some(clip) => clip,
                None => self.decoded(&view.playback.file)?,
            };
            let reader = match &view.playback.checkpoint {
                Some(checkpoint) => clip.restore(self.gate.clone(), checkpoint, view.parked)?,
                None => clip.start(self.gate.clone(), view.parked),
            };
            let source = reader.decoder();
            bound.push(Bound {
                view,
                reader,
                source,
            });
        }
        let mut bound_music = Vec::new();
        for view in music {
            if same {
                if let Some(index) = old_music
                    .iter()
                    .position(|b| b.view.authority == view.authority && b.view.file == view.file)
                {
                    let mut row = old_music.remove(index);
                    row.view = view;
                    bound_music.push(row);
                    continue;
                }
            }
            let cached = {
                self.clips
                    .lock()
                    .map_err(|_| "Fixture clip cache poisoned")?
                    .get(&view.file)
                    .cloned()
            };
            let clip = match cached {
                Some(clip) => clip,
                None => self.decoded(&view.file)?,
            };
            let reader = match &view.checkpoint {
                Some(cp) => clip.restore(self.gate.clone(), cp, view.parked)?,
                None => clip.start_looped(self.gate.clone(), view.parked),
            };
            let source = reader.decoder();
            bound_music.push(BoundMusic {
                view,
                reader,
                source,
            });
        }
        let retired: Vec<_> = old
            .iter()
            .map(|b| (&b.reader, true))
            .chain(old_music.iter().map(|b| (&b.reader, true)))
            .collect();
        self.gate.publish(&retired, |_| Ok(()))?;
        for row in old {
            row.reader.stop()?;
        }
        for row in old_music {
            row.reader.stop()?;
        }
        live.instance = Some(instance);
        live.epoch = Some(epoch);
        live.bound = bound;
        live.music = bound_music;
        Ok(())
    }
    pub(super) fn pull(&self, target: &TargetIdentity, count: usize) -> Result<Vec<i16>, String> {
        let mut rows = self
            .readers
            .lock()
            .map_err(|_| "Fixture reader registry poisoned")?;
        if let Some(row) = rows
            .music
            .iter_mut()
            .find(|r| &r.view.authority.target == target)
        {
            return Ok(row.source.by_ref().take(count).collect());
        }
        let row = rows
            .bound
            .iter_mut()
            .find(|r| &r.view.token.target == target)
            .ok_or("Actual fixture Source is absent")?;
        Ok(row.source.by_ref().take(count).collect())
    }
    pub(super) fn cursor(
        &self,
        target: &TargetIdentity,
    ) -> Result<rvn_core::audio_checkpoint::AudioCheckpoint, String> {
        let rows = self
            .readers
            .lock()
            .map_err(|_| "Fixture reader registry poisoned")?;
        if let Some(row) = rows
            .music
            .iter()
            .find(|r| &r.view.authority.target == target)
        {
            return self
                .gate
                .snapshot(&[&row.reader], |mut values| Ok(values.remove(0)));
        }
        let row = rows
            .bound
            .iter()
            .find(|r| &r.view.token.target == target)
            .ok_or("Actual fixture Source is absent")?;
        self.gate
            .snapshot(&[&row.reader], |mut rows| Ok(rows.remove(0)))
    }
    fn capture(
        &self,
        state: &rvn_core::GameState,
        instance: u64,
        epoch: u64,
        all: bool,
    ) -> Result<AudioCheckpointSnapshot, String> {
        let wanted = rvn_core::audio_playback::describe(state, epoch)?;
        let wanted: Vec<_> = wanted
            .iter()
            .filter(|v| {
                all || !matches!(
                    v.token.task.key.owner,
                    rvn_core::scene_tasks::TaskOwner::Host { .. }
                )
            })
            .collect();
        let music = rvn_core::audio_checkpoint::music_views(state, epoch)?;
        let music: Vec<_> = music
            .iter()
            .filter(|v| all || !matches!(v.owner, rvn_core::scene_tasks::TaskOwner::Host { .. }))
            .collect();
        let rows = self
            .readers
            .lock()
            .map_err(|_| "Fixture reader registry poisoned")?;
        if (!wanted.is_empty() || !music.is_empty())
            && (rows.instance != Some(instance) || rows.epoch != Some(epoch))
        {
            return Err("Unsettled actual fixture Engine/Source epoch".into());
        }
        let bound = wanted
            .iter()
            .map(|v| {
                rows.bound
                    .iter()
                    .find(|b| {
                        b.view.token == v.token
                            && b.view.playback.family == v.playback.family
                            && b.view.playback.file == v.playback.file
                            && b.view.parked == v.parked
                    })
                    .ok_or("Actual Source binding is missing/stale")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bound_music = music
            .iter()
            .map(|view| {
                rows.music
                    .iter()
                    .find(|b| {
                        b.view.authority == view.authority
                            && b.view.file == view.file
                            && b.view.parked == view.parked
                    })
                    .ok_or("Actual Music Source is missing/stale")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let readers: Vec<_> = bound
            .iter()
            .map(|b| &b.reader)
            .chain(bound_music.iter().map(|b| &b.reader))
            .collect();
        self.gate.snapshot_with_status(&readers, |captured| {
            let mut audio = Vec::new();
            let mut music = Vec::new();
            for (index, (checkpoint, paused)) in captured.into_iter().enumerate() {
                if index < bound.len() {
                    if paused != bound[index].view.parked {
                        return Err("Fixture Source residence differs".into());
                    }
                    audio.push(AudioCheckpointRow {
                        token: bound[index].view.token.clone(),
                        checkpoint,
                    });
                } else {
                    let row = bound_music[index - bound.len()];
                    if paused != row.view.parked {
                        return Err("Fixture Music residence differs".into());
                    }
                    music.push(rvn_core::audio_checkpoint::MusicCheckpoint {
                        target: row.view.authority.target.clone(),
                        file: row.view.file.clone(),
                        checkpoint,
                    });
                }
            }
            let snapshot = AudioCheckpointSnapshot {
                instance_id: instance,
                runtime_epoch: epoch,
                rows: audio,
                music,
            };
            if all {
                rvn_core::frame_audio_checkpoint::checkpointed_frame_state(
                    state, instance, epoch, &snapshot,
                )?;
            } else {
                rvn_core::audio_checkpoint::checkpointed_state(state, instance, epoch, &snapshot)?;
            }
            Ok(snapshot)
        })
    }
    fn validate_audio(&self, state: &rvn_core::GameState, all: bool) -> Result<(), String> {
        if all {
            rvn_core::frame_audio_checkpoint::require_frame_checkpoints(state)?;
        } else {
            rvn_core::audio_checkpoint::require_saved_checkpoints(state)?;
        }
        for view in rvn_core::audio_playback::describe(state, 0)? {
            if !all
                && matches!(
                    view.token.task.key.owner,
                    rvn_core::scene_tasks::TaskOwner::Host { .. }
                )
            {
                continue;
            }
            let clip = self.decoded(&view.playback.file)?;
            clip.validate_checkpoint(
                view.playback
                    .checkpoint
                    .as_ref()
                    .ok_or("Saved actual Source cursor is absent")?,
            )?;
        }
        for view in rvn_core::audio_checkpoint::music_views(state, 0)? {
            if !all && matches!(view.owner, rvn_core::scene_tasks::TaskOwner::Host { .. }) {
                continue;
            }
            self.decoded(&view.file)?
                .validate_checkpoint(view.checkpoint.as_ref().ok_or("Music cursor absent")?)?;
        }
        Ok(())
    }
}
impl Renderer for PcmHost {
    fn capture_audio_checkpoint(
        &self,
        s: &rvn_core::GameState,
        i: u64,
        e: u64,
    ) -> Result<AudioCheckpointSnapshot, String> {
        self.capture(s, i, e, false)
    }
    fn capture_frame_audio_checkpoint(
        &self,
        s: &rvn_core::GameState,
        i: u64,
        e: u64,
    ) -> Result<AudioCheckpointSnapshot, String> {
        self.capture(s, i, e, true)
    }
    fn validate_audio_restore(&self, s: &rvn_core::GameState) -> Result<(), String> {
        self.validate_audio(s, false)
    }
    fn validate_frame_audio_restore(&self, s: &rvn_core::GameState) -> Result<(), String> {
        self.validate_audio(s, true)
    }
    fn validate_scene_batch(&self, b: &SceneRenderBatch) -> Result<(), String> {
        self.graphics.validate_scene_batch(b)?;
        for view in rvn_core::audio_playback::describe(&b.state, b.runtime_epoch)? {
            let clip = self.decoded(&view.playback.file)?;
            if let Some(saved) = &view.playback.checkpoint {
                clip.validate_checkpoint(saved)?;
            }
        }
        for view in rvn_core::audio_checkpoint::music_views(&b.state, b.runtime_epoch)? {
            let clip = self.decoded(&view.file)?;
            if let Some(cp) = &view.checkpoint {
                clip.validate_checkpoint(cp)?;
            }
        }
        Ok(())
    }
    fn publish_scene_batch(&mut self, b: SceneRenderBatch) {
        let instance = self.readers.lock().unwrap().instance;
        if let Some(instance) = instance {
            self.bind_actual(&b.state, instance, b.runtime_epoch)
                .expect("Actual fixture Sources preflighted");
        }
        self.graphics.publish_scene_batch(b);
    }
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn supports_custom_canvas(&self) -> bool {
        true
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn update_interfaces(&mut self, _: &[rvn_ui::programmable::ScreenView]) -> Result<(), String> {
        Ok(())
    }
    fn set_background(&mut self, a: &str, b: &Transition) {
        self.graphics.set_background(a, b)
    }
    fn show_sprite(
        &mut self,
        a: &str,
        b: Option<&str>,
        c: &Position,
        d: &Transition,
        e: Option<&SpriteState>,
    ) {
        self.graphics.show_sprite(a, b, c, d, e)
    }
    fn hide_sprite(&mut self, a: &str, b: &Transition, c: &SpriteState) {
        self.graphics.hide_sprite(a, b, c)
    }
    fn move_sprite(&mut self, a: &str, b: &Position, c: &Transition, d: &SpriteState) {
        self.graphics.move_sprite(a, b, c, d)
    }
    fn show_dialogue(&mut self, a: Option<&str>, b: &str) {
        self.graphics.show_dialogue(a, b)
    }
    fn show_choice(&mut self, a: &[String]) -> usize {
        self.graphics.show_choice(a)
    }
    fn show_imagemap(&mut self, a: &str, b: Option<&str>, c: &[Hotspot]) -> usize {
        self.graphics.show_imagemap(a, b, c)
    }
    fn music_play(&mut self, a: &str, b: &Transition, c: Option<&str>) {
        self.graphics.music_play(a, b, c)
    }
    fn music_stop(&mut self, a: &Transition) {
        self.graphics.music_stop(a)
    }
    fn music_set_volume(&mut self, a: f32) {
        self.graphics.music_set_volume(a)
    }
    fn sfx_play(&mut self, a: &str, b: &Transition) {
        self.graphics.sfx_play(a, b)
    }
    fn sfx_stop(&mut self, a: &str, b: &Transition) {
        self.graphics.sfx_stop(a, b)
    }
}
