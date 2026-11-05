//! PRIVATE: real consumed-sample cursor through Bevy0.14.2 Decodable.
//! This source cursor excludes device/resampler buffers and makes no audible parity claim.
use rodio::Source;
use crate::audio_checkpoint::{
    AudioCheckpoint, CHECKPOINT_VERSION, MAX_ENCODED_BYTES, MAX_PCM_SAMPLES,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug)]
pub(crate) struct PcmClip {
    samples: Arc<[i16]>,
    identity: AudioCheckpoint,
}

impl PcmClip {
    pub(crate) fn pcm_digest(&self) -> [u8; 32] {
        self.identity.pcm_sha256
    }
    pub(crate) fn asset_digest(&self) -> [u8; 32] {
        self.identity.asset_sha256
    }
    pub(crate) fn pcm_bytes(&self) -> u64 {
        self.identity.total_samples * 2
    }
    pub(crate) fn allocation_key(&self) -> usize {
        Arc::as_ptr(&self.samples) as *const i16 as usize
    }
    pub(crate) fn validate_checkpoint(&self, checkpoint: &AudioCheckpoint) -> Result<(), String> {
        checkpoint.validate()?;
        if !self.identity.same_source(checkpoint) {
            return Err("Saved audio bytes or decoded PCM changed; original save preserved".into());
        }
        Ok(())
    }
    pub(crate) fn decode(project_asset: &str, encoded: &[u8]) -> Result<Arc<Self>, String> {
        Self::decode_shared(project_asset, Arc::from(encoded), 256 * 1024 * 1024)
    }
    /// Production shares AudioSource.bytes, not an encoded Vec copy. The PCM
    /// budget includes old shared allocations once, Vec growth overlap and the
    /// transient Vec -> Arc copy. Encoded AssetServer buffers have their separate
    /// 64 MiB per-source bound; decoder/device internal buffers are not PCM rows.
    pub(crate) fn decode_shared(
        project_asset: &str,
        encoded: Arc<[u8]>,
        available: u64,
    ) -> Result<Arc<Self>, String> {
        if !rvn_ui::programmable::safe_asset_path(project_asset)
            || encoded.is_empty()
            || encoded.len() > MAX_ENCODED_BYTES
        {
            return Err("Invalid or oversized authorized project audio asset".into());
        }
        // Bytes were supplied by the project's existing AssetServer. No path traversal,
        // external downloads, network I/O or asset ownership is invented here.
        // Same local Rodio decoder/formats/i16 conversion as Bevy AudioSource.
        // Its WAV constructor/iterator contain documented panics. Convert those
        // to refusal here, before installing anything; panic=unwind is required.
        let (channels, sample_rate, samples) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut decoder = rodio::Decoder::new(std::io::Cursor::new(encoded.clone()))
                    .map_err(|error| format!("Audio format refused: {error}"))?;
                let channels = decoder.channels();
                let sample_rate = decoder.sample_rate();
                if channels == 0
                    || channels > crate::audio_checkpoint::MAX_CHANNELS
                    || sample_rate == 0
                    || sample_rate > crate::audio_checkpoint::MAX_SAMPLE_RATE
                {
                    return Err("Audio source format exceeds the checkpoint bounds".into());
                }
                let mut samples = Vec::new();
                loop {
                    if (decoder.channels(), decoder.sample_rate()) != (channels, sample_rate) {
                        return Err(
                            "Changing audio formats need an explicit checkpoint representation"
                                .into(),
                        );
                    }
                    let Some(sample) = decoder.next() else {
                        break;
                    };
                    if samples.len() as u64 >= MAX_PCM_SAMPLES {
                        return Err(
                            "Decoded project audio exceeds the PCM checkpoint budget".into()
                        );
                    }
                    if samples.len() == samples.capacity() {
                        let old = samples.capacity() as u64;
                        let maximum = available / 4;
                        let wanted = old
                            .saturating_mul(2)
                            .max(4096)
                            .min(MAX_PCM_SAMPLES)
                            .min(maximum.max(old + 1));
                        if old
                            .checked_add(wanted)
                            .and_then(|count| count.checked_mul(2))
                            .is_none_or(|peak| peak > available)
                        {
                            return Err(
                                "PCM staging/reallocation exceeds its actual buffer budget".into(),
                            );
                        }
                        let requested = usize::try_from(wanted - old)
                            .map_err(|_| "PCM allocation size overflow")?;
                        samples
                            .try_reserve_exact(requested)
                            .map_err(|_| "PCM staging allocation refused")?;
                    }
                    if (samples.capacity() as u64)
                        .checked_add(samples.len() as u64 + 1)
                        .and_then(|count| count.checked_mul(2))
                        .is_none_or(|peak| peak > available)
                    {
                        return Err(
                            "PCM staging and immutable copy exceed the shared buffer budget".into(),
                        );
                    }
                    samples.push(sample);
                }
                Ok::<_, String>((channels, sample_rate, samples))
            }))
            .map_err(|_| "Audio decoder panicked before checkpoint publication".to_string())??;
        Self::from_pcm(
            Sha256::digest(&encoded).into(),
            channels,
            sample_rate,
            samples,
        )
    }

    fn from_pcm(
        asset_sha256: [u8; 32],
        channels: u16,
        sample_rate: u32,
        samples: Vec<i16>,
    ) -> Result<Arc<Self>, String> {
        let mut hash = Sha256::new();
        hash.update(sample_rate.to_le_bytes());
        hash.update(channels.to_le_bytes());
        for sample in &samples {
            hash.update(sample.to_le_bytes());
        }
        let identity = AudioCheckpoint {
            version: CHECKPOINT_VERSION,
            asset_sha256,
            pcm_sha256: hash.finalize().into(),
            channels,
            sample_rate,
            total_samples: samples.len() as u64,
            next_sample: 0,
            next_channel: 0,
            looping: false,
            loops_completed: 0,
        };
        identity.validate()?;
        Ok(Arc::new(Self {
            samples: samples.into(),
            identity,
        }))
    }

    pub(crate) fn restore(
        self: &Arc<Self>,
        gate: CheckpointGate,
        checkpoint: &AudioCheckpoint,
        paused: bool,
    ) -> Result<CheckpointPcm, String> {
        self.validate_checkpoint(checkpoint)?;
        Ok(CheckpointPcm {
            clip: self.clone(),
            gate,
            cursor: Arc::new(Mutex::new(CursorState {
                next_sample: checkpoint.next_sample,
                looping: checkpoint.looping,
                loops_completed: checkpoint.loops_completed,
                paused,
                claimed: false,
                stopped: false,
                fault: None,
            })),
        })
    }

    pub(crate) fn start(self: &Arc<Self>, gate: CheckpointGate, paused: bool) -> CheckpointPcm {
        self.restore(gate, &self.identity, paused)
            .expect("PcmClip construction validated its immutable identity")
    }

    /// Music may use a native looping source, not Rodio repeat_infinite clones.
    /// Owned SFX/Voice remain one-shot and their Core validation rejects looping.
    pub(crate) fn start_looped(
        self: &Arc<Self>,
        gate: CheckpointGate,
        paused: bool,
    ) -> CheckpointPcm {
        let mut checkpoint = self.identity.clone();
        checkpoint.looping = true;
        self.restore(gate, &checkpoint, paused)
            .expect("The exact looping checkpoint is valid")
    }
}

/// All sources in one renderer use this gate. Snapshot closures clone Core state
/// while next() is fenced; disk/JSON writer I/O runs only after the guard drops.
#[derive(Clone, Default, Debug)]
pub(crate) struct CheckpointGate(Arc<Mutex<GateState>>);
#[derive(Default, Debug)]
struct GateState {
    preview_paused: bool,
}
impl CheckpointGate {
    // Session control shares the existing source fence. It is not a saved
    // residence, clock, playhead or second cursor; newly published readers
    // observe the same pause without overwriting their individual residence.
    pub(crate) fn set_preview_paused(&self, paused: bool) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|_| "Audio preview fence is poisoned")?
            .preview_paused = paused;
        Ok(())
    }
    /// Preflight every cursor before changing any residence. Core's immutable
    /// snapshot/metadata commit runs under this same fence. A refusal changes
    /// neither pauses nor cursors, and no disk or renderer work occurs here.
    pub(crate) fn publish<T>(
        &self,
        readers: &[(&CheckpointPcm, bool)],
        build: impl FnOnce(Vec<AudioCheckpoint>) -> Result<T, String>,
    ) -> Result<T, String> {
        let _fence = self
            .0
            .lock()
            .map_err(|_| "Audio publication fence is poisoned")?;
        let mut identities = std::collections::HashSet::new();
        let mut guards = Vec::with_capacity(readers.len());
        let mut rows = Vec::with_capacity(readers.len());
        for (source, _) in readers {
            if !Arc::ptr_eq(&self.0, &source.gate.0)
                || !identities.insert(Arc::as_ptr(&source.cursor))
            {
                return Err("Foreign or duplicate Source in audio publication".into());
            }
            let cursor = source
                .cursor
                .lock()
                .map_err(|_| "Audio cursor is poisoned")?;
            if let Some(fault) = &cursor.fault {
                return Err(fault.clone());
            }
            if cursor.stopped {
                return Err("A revoked Source cannot be published".into());
            }
            let mut row = source.clip.identity.clone();
            row.next_sample = cursor.next_sample;
            row.next_channel = (cursor.next_sample % u64::from(row.channels)) as u16;
            row.looping = cursor.looping;
            row.loops_completed = cursor.loops_completed;
            row.validate()?;
            rows.push(row);
            guards.push(cursor);
        }
        let result = build(rows)?;
        for (guard, (_, paused)) in guards.iter_mut().zip(readers) {
            guard.paused = *paused;
        }
        Ok(result)
    }
    pub(crate) fn snapshot<T>(
        &self,
        readers: &[&CheckpointPcm],
        build: impl FnOnce(Vec<AudioCheckpoint>) -> Result<T, String>,
    ) -> Result<T, String> {
        self.snapshot_with_status(readers, |rows| {
            build(rows.into_iter().map(|(checkpoint, _)| checkpoint).collect())
        })
    }
    pub(crate) fn snapshot_with_status<T>(
        &self,
        readers: &[&CheckpointPcm],
        build: impl FnOnce(Vec<(AudioCheckpoint, bool)>) -> Result<T, String>,
    ) -> Result<T, String> {
        let _fence = self
            .0
            .lock()
            .map_err(|_| "Audio checkpoint fence is poisoned")?;
        let mut rows = Vec::with_capacity(readers.len());
        let mut identities = std::collections::HashSet::new();
        for source in readers {
            if !Arc::ptr_eq(&self.0, &source.gate.0)
                || !identities.insert(Arc::as_ptr(&source.cursor))
            {
                return Err("Foreign or duplicate decoder in one checkpoint fence".into());
            }
            let cursor = source
                .cursor
                .lock()
                .map_err(|_| "Audio cursor is poisoned")?;
            if let Some(fault) = &cursor.fault {
                return Err(fault.clone());
            }
            if cursor.stopped {
                return Err("A revoked decoder cannot provide a portable checkpoint".into());
            }
            let mut row = source.clip.identity.clone();
            row.next_sample = cursor.next_sample;
            row.next_channel = (cursor.next_sample % u64::from(row.channels)) as u16;
            row.looping = cursor.looping;
            row.loops_completed = cursor.loops_completed;
            row.validate()?;
            rows.push((row, cursor.paused));
        }
        build(rows)
    }
}

#[derive(Debug)]
struct CursorState {
    next_sample: u64,
    looping: bool,
    loops_completed: u64,
    paused: bool,
    claimed: bool,
    stopped: bool,
    fault: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct CheckpointPcm {
    clip: Arc<PcmClip>,
    gate: CheckpointGate,
    cursor: Arc<Mutex<CursorState>>,
}

impl CheckpointPcm {
    pub(crate) fn gate(&self) -> CheckpointGate {
        self.gate.clone()
    }
    pub(crate) fn same_reader(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cursor, &other.cursor) && Arc::ptr_eq(&self.gate.0, &other.gate.0)
    }
    pub(crate) fn clip(&self) -> &Arc<PcmClip> {
        &self.clip
    }
    pub(crate) fn fault(&self) -> Result<Option<String>, String> {
        let _fence = self
            .gate
            .0
            .lock()
            .map_err(|_| "Audio checkpoint fence is poisoned")?;
        Ok(self
            .cursor
            .lock()
            .map_err(|_| "Audio cursor is poisoned")?
            .fault
            .clone())
    }

    pub(crate) fn set_paused(&self, paused: bool) -> Result<(), String> {
        let _fence = self
            .gate
            .0
            .lock()
            .map_err(|_| "Audio pause fence is poisoned")?;
        let mut cursor = self.cursor.lock().map_err(|_| "Audio cursor is poisoned")?;
        if let Some(fault) = &cursor.fault {
            return Err(fault.clone());
        }
        if cursor.stopped {
            return Err("Audio decoder was revoked".into());
        }
        cursor.paused = paused;
        Ok(())
    }

    pub(crate) fn stop(&self) -> Result<(), String> {
        let _fence = self
            .gate
            .0
            .lock()
            .map_err(|_| "Audio stop fence is poisoned")?;
        self.cursor
            .lock()
            .map_err(|_| "Audio cursor is poisoned")?
            .stopped = true;
        Ok(())
    }
}

pub(crate) struct PcmSource {
    reader: CheckpointPcm,
    admitted: bool,
    duration: Option<Duration>,
    output_channel: u16,
}
impl CheckpointPcm {
    pub(crate) fn decoder(&self) -> PcmSource {
        let admitted = self
            .gate
            .0
            .lock()
            .ok()
            .and_then(|_fence| {
                self.cursor.lock().ok().map(|mut cursor| {
                    if cursor.claimed || cursor.stopped || cursor.fault.is_some() {
                        cursor.fault.get_or_insert_with(|| {
                            "A decoder cannot silently restart its exact playback".into()
                        });
                        false
                    } else {
                        cursor.claimed = true;
                        true
                    }
                })
            })
            .unwrap_or(false);
        let duration = self.cursor.lock().ok().and_then(|cursor| {
            (!cursor.looping).then(|| {
                Duration::from_secs_f64(
                    (self.clip.identity.total_samples - cursor.next_sample
                        + cursor.next_sample % u64::from(self.clip.identity.channels))
                        as f64
                        / f64::from(self.clip.identity.channels)
                        / f64::from(self.clip.identity.sample_rate),
                )
            })
        });
        PcmSource {
            reader: self.clone(),
            admitted,
            duration,
            output_channel: 0,
        }
    }
}

impl Iterator for PcmSource {
    type Item = i16;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.admitted {
            return None;
        }
        let _fence = self.reader.gate.0.lock().ok()?;
        let mut cursor = self.reader.cursor.lock().ok()?;
        let channels = self.reader.clip.identity.channels;
        if cursor.stopped || cursor.fault.is_some() {
            return None;
        }
        if cursor.next_sample == self.reader.clip.identity.total_samples {
            if !cursor.looping {
                return None;
            }
            if cursor.paused || _fence.preview_paused {
                self.output_channel = (self.output_channel + 1) % channels;
                return Some(0);
            }
            let Some(next) = cursor.loops_completed.checked_add(1) else {
                cursor.fault = Some("Audio loop counter exhausted; cursor unchanged".into());
                return None;
            };
            cursor.loops_completed = next;
            cursor.next_sample = 0;
        }
        if cursor.paused
            || _fence.preview_paused
            || self.output_channel != (cursor.next_sample % u64::from(channels)) as u16
        {
            // Restore and pause/resume can start at another output channel.
            // Explicit (<one frame) padding preserves PCM orientation without
            // advancing, rewinding or rounding its actual source cursor.
            self.output_channel = (self.output_channel + 1) % channels;
            return Some(0);
        }
        let sample = self.reader.clip.samples[cursor.next_sample as usize];
        cursor.next_sample += 1;
        self.output_channel = (self.output_channel + 1) % channels;
        Some(sample)
    }
}

impl Source for PcmSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.reader.clip.identity.channels
    }
    fn sample_rate(&self) -> u32 {
        self.reader.clip.identity.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.duration
    }
}
