//! PRIVATE: real consumed-sample cursor through Bevy0.14.2 Decodable.
//! This source cursor excludes device/resampler buffers and makes no audible parity claim.
use bevy::{audio::{Decodable, Source}, prelude::*};
use bevy::asset as bevy_asset;
use bevy::reflect as bevy_reflect;
use rvn_core::audio_checkpoint::{AudioCheckpoint, CHECKPOINT_VERSION, MAX_ENCODED_BYTES, MAX_PCM_SAMPLES};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug)]
pub(crate) struct PcmClip {
    samples: Arc<[i16]>,
    identity: AudioCheckpoint,
}

impl PcmClip {
    pub(crate) fn pcm_digest(&self)->[u8;32] {self.identity.pcm_sha256}
    pub(crate) fn asset_digest(&self)->[u8;32] {self.identity.asset_sha256}
    pub(crate) fn pcm_bytes(&self)->u64 {self.identity.total_samples*2}
    pub(crate) fn allocation_key(&self)->usize {Arc::as_ptr(&self.samples) as *const i16 as usize}
    pub(crate) fn validate_checkpoint(&self,checkpoint:&AudioCheckpoint)->Result<(),String> {
        checkpoint.validate()?;
        if !self.identity.same_source(checkpoint) {return Err("Saved audio bytes or decoded PCM changed; original save preserved".into());}
        Ok(())
    }
    pub(crate) fn decode(project_asset: &str, encoded: &[u8]) -> Result<Arc<Self>, String> {
        Self::decode_shared(project_asset,Arc::from(encoded),256*1024*1024)
    }
    /// Production shares AudioSource.bytes, not an encoded Vec copy. The PCM
    /// budget includes old shared allocations once, Vec growth overlap and the
    /// transient Vec -> Arc copy. Encoded AssetServer buffers have their separate
    /// 64 MiB per-source bound; decoder/device internal buffers are not PCM rows.
    pub(crate) fn decode_shared(project_asset:&str,encoded:Arc<[u8]>,available:u64)->Result<Arc<Self>,String> {
        if !rvn_ui::programmable::safe_asset_path(project_asset) || encoded.is_empty()
            || encoded.len() > MAX_ENCODED_BYTES {
            return Err("Invalid or oversized authorized project audio asset".into());
        }
        // Bytes were supplied by the project's existing AssetServer. No path traversal,
        // external downloads, network I/O or asset ownership is invented here.
        // Same local Rodio decoder/formats/i16 conversion as Bevy AudioSource.
        // Its WAV constructor/iterator contain documented panics. Convert those
        // to refusal here, before installing anything; panic=unwind is required.
        let (channels, sample_rate, samples) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut decoder = rodio::Decoder::new(std::io::Cursor::new(encoded.clone()))
                .map_err(|error| format!("Audio format refused: {error}"))?;
            let channels = decoder.channels(); let sample_rate = decoder.sample_rate();
            if channels == 0 || channels > rvn_core::audio_checkpoint::MAX_CHANNELS
                || sample_rate == 0 || sample_rate > rvn_core::audio_checkpoint::MAX_SAMPLE_RATE {
                return Err("Audio source format exceeds the checkpoint bounds".into());
            }
            let mut samples = Vec::new();
            loop {
                if (decoder.channels(), decoder.sample_rate()) != (channels, sample_rate) {
                    return Err("Changing audio formats need an explicit checkpoint representation".into());
                }
                let Some(sample) = decoder.next() else { break; };
                if samples.len() as u64 >= MAX_PCM_SAMPLES {
                    return Err("Decoded project audio exceeds the PCM checkpoint budget".into());
                }
                if samples.len()==samples.capacity() {
                    let old=samples.capacity() as u64;
                    let maximum=available/4;
                    let wanted=old.saturating_mul(2).max(4096).min(MAX_PCM_SAMPLES).min(maximum.max(old+1));
                    if old.checked_add(wanted).and_then(|count|count.checked_mul(2)).is_none_or(|peak|peak>available) {return Err("PCM staging/reallocation exceeds its actual buffer budget".into());}
                    let requested=usize::try_from(wanted-old).map_err(|_|"PCM allocation size overflow")?;
                    samples.try_reserve_exact(requested).map_err(|_|"PCM staging allocation refused")?;
                }
                if (samples.capacity() as u64).checked_add(samples.len() as u64+1).and_then(|count|count.checked_mul(2)).is_none_or(|peak|peak>available) {return Err("PCM staging and immutable copy exceed the shared buffer budget".into());}
                samples.push(sample);
            }
            Ok::<_, String>((channels, sample_rate, samples))
        })).map_err(|_| "Audio decoder panicked before checkpoint publication".to_string())??;
        Self::from_pcm(Sha256::digest(&encoded).into(), channels, sample_rate, samples)
    }

    fn from_pcm(asset_sha256: [u8; 32], channels: u16, sample_rate: u32, samples: Vec<i16>) -> Result<Arc<Self>, String> {
        let mut hash = Sha256::new();
        hash.update(sample_rate.to_le_bytes()); hash.update(channels.to_le_bytes());
        for sample in &samples { hash.update(sample.to_le_bytes()); }
        let identity = AudioCheckpoint { version: CHECKPOINT_VERSION, asset_sha256,
            pcm_sha256: hash.finalize().into(), channels, sample_rate,
            total_samples: samples.len() as u64, next_sample: 0, next_channel: 0,
            looping: false, loops_completed: 0 };
        identity.validate()?;
        Ok(Arc::new(Self { samples: samples.into(), identity }))
    }

    pub(crate) fn restore(self: &Arc<Self>, gate: CheckpointGate, checkpoint: &AudioCheckpoint, paused: bool) -> Result<CheckpointPcm, String> {
        self.validate_checkpoint(checkpoint)?;
        Ok(CheckpointPcm { clip: self.clone(), gate, cursor: Arc::new(Mutex::new(CursorState {
            next_sample: checkpoint.next_sample,
            looping: checkpoint.looping, loops_completed: checkpoint.loops_completed,
            paused, claimed: false, stopped: false, fault: None,
        })) })
    }

    pub(crate) fn start(self: &Arc<Self>, gate: CheckpointGate, paused: bool) -> CheckpointPcm {
        self.restore(gate, &self.identity, paused).expect("PcmClip construction validated its immutable identity")
    }

    /// Music may use a native looping source, not Rodio repeat_infinite clones.
    /// Owned SFX/Voice remain one-shot and their Core validation rejects looping.
    pub(crate) fn start_looped(self: &Arc<Self>, gate: CheckpointGate, paused: bool) -> CheckpointPcm {
        let mut checkpoint = self.identity.clone(); checkpoint.looping = true;
        self.restore(gate, &checkpoint, paused).expect("The exact looping checkpoint is valid")
    }
}

/// All sources in one renderer use this gate. Snapshot closures clone Core state
/// while next() is fenced; disk/JSON writer I/O runs only after the guard drops.
#[derive(Clone, Default, Debug)]
pub(crate) struct CheckpointGate(Arc<Mutex<GateState>>);
#[derive(Default,Debug)]
struct GateState { preview_paused:bool }
impl CheckpointGate {
    pub(crate) fn same_gate(&self,other:&Self)->bool{Arc::ptr_eq(&self.0,&other.0)}
    /// Streaming media shares this real producer gate, including cached right
    /// samples. Contention yields silence at the caller, never Source EOF.
    pub(crate) fn try_read<T>(&self,read:impl FnOnce(bool)->T)->Option<T>{
        let fence=self.0.try_lock().ok()?;Some(read(fence.preview_paused))
    }
    // Session control shares the existing source fence. It is not a saved
    // residence, clock, playhead or second cursor; newly published readers
    // observe the same pause without overwriting their individual residence.
    pub(crate) fn set_preview_paused(&self,paused:bool)->Result<(),String>{
        self.0.lock().map_err(|_|"Audio preview fence is poisoned")?.preview_paused=paused;Ok(())
    }
    /// Preflight every cursor before changing any residence. Core's immutable
    /// snapshot/metadata commit runs under this same fence. A refusal changes
    /// neither pauses nor cursors, and no disk or renderer work occurs here.
    pub(crate) fn publish<T>(&self,readers:&[(&CheckpointPcm,bool)],build:impl FnOnce(Vec<AudioCheckpoint>)->Result<T,String>)->Result<T,String> {
        let _fence=self.0.lock().map_err(|_|"Audio publication fence is poisoned")?;
        let mut identities=std::collections::HashSet::new();let mut guards=Vec::with_capacity(readers.len());let mut rows=Vec::with_capacity(readers.len());
        for (source,_) in readers {
            if !Arc::ptr_eq(&self.0,&source.gate.0) || !identities.insert(Arc::as_ptr(&source.cursor)) {return Err("Foreign or duplicate Source in audio publication".into());}
            let cursor=source.cursor.lock().map_err(|_|"Audio cursor is poisoned")?;
            if let Some(fault)=&cursor.fault {return Err(fault.clone());}
            if cursor.stopped {return Err("A revoked Source cannot be published".into());}
            let mut row=source.clip.identity.clone();row.next_sample=cursor.next_sample;row.next_channel=(cursor.next_sample%u64::from(row.channels)) as u16;
            row.looping=cursor.looping;row.loops_completed=cursor.loops_completed;row.validate()?;rows.push(row);guards.push(cursor);
        }
        let result=build(rows)?;
        for (guard,(_,paused)) in guards.iter_mut().zip(readers) {guard.paused=*paused;}
        Ok(result)
    }
    pub(crate) fn snapshot<T>(&self, readers: &[&CheckpointPcm], build: impl FnOnce(Vec<AudioCheckpoint>) -> Result<T, String>) -> Result<T, String> {
        self.snapshot_with_status(readers,|rows|build(rows.into_iter().map(|(checkpoint,_)|checkpoint).collect()))
    }
    pub(crate) fn snapshot_with_status<T>(&self,readers:&[&CheckpointPcm],build:impl FnOnce(Vec<(AudioCheckpoint,bool)>)->Result<T,String>)->Result<T,String> {
        let _fence = self.0.lock().map_err(|_| "Audio checkpoint fence is poisoned")?;
        let mut rows = Vec::with_capacity(readers.len());
        let mut identities = std::collections::HashSet::new();
        for source in readers {
            if !Arc::ptr_eq(&self.0, &source.gate.0) || !identities.insert(Arc::as_ptr(&source.cursor)) {
                return Err("Foreign or duplicate decoder in one checkpoint fence".into());
            }
            let cursor = source.cursor.lock().map_err(|_| "Audio cursor is poisoned")?;
            if let Some(fault) = &cursor.fault { return Err(fault.clone()); }
            if cursor.stopped { return Err("A revoked decoder cannot provide a portable checkpoint".into()); }
            let mut row = source.clip.identity.clone();
            row.next_sample = cursor.next_sample;
            row.next_channel = (cursor.next_sample % u64::from(row.channels)) as u16;
            row.looping = cursor.looping; row.loops_completed = cursor.loops_completed;
            row.validate()?; rows.push((row,cursor.paused));
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

#[derive(Asset, TypePath, Clone, Debug)]
pub(crate) struct CheckpointPcm {
    clip: Arc<PcmClip>,
    gate: CheckpointGate,
    cursor: Arc<Mutex<CursorState>>,
}

impl CheckpointPcm {
    pub(crate) fn gate(&self)->CheckpointGate {self.gate.clone()}
    pub(crate) fn same_reader(&self,other:&Self)->bool {Arc::ptr_eq(&self.cursor,&other.cursor) && Arc::ptr_eq(&self.gate.0,&other.gate.0)}
    pub(crate) fn clip(&self)->&Arc<PcmClip> {&self.clip}
    pub(crate) fn fault(&self) -> Result<Option<String>, String> {
        let _fence = self.gate.0.lock().map_err(|_| "Audio checkpoint fence is poisoned")?;
        Ok(self.cursor.lock().map_err(|_| "Audio cursor is poisoned")?.fault.clone())
    }

    pub(crate) fn set_paused(&self, paused: bool) -> Result<(), String> {
        let _fence = self.gate.0.lock().map_err(|_| "Audio pause fence is poisoned")?;
        let mut cursor = self.cursor.lock().map_err(|_| "Audio cursor is poisoned")?;
        if let Some(fault) = &cursor.fault { return Err(fault.clone()); }
        if cursor.stopped { return Err("Audio decoder was revoked".into()); }
        cursor.paused = paused; Ok(())
    }

    pub(crate) fn stop(&self) -> Result<(), String> {
        let _fence = self.gate.0.lock().map_err(|_| "Audio stop fence is poisoned")?;
        self.cursor.lock().map_err(|_| "Audio cursor is poisoned")?.stopped = true;
        Ok(())
    }
}

pub(crate) struct PcmSource { reader: CheckpointPcm, admitted: bool, duration: Option<Duration>, output_channel: u16 }
impl Decodable for CheckpointPcm {
    type DecoderItem = i16;
    type Decoder = PcmSource;
    fn decoder(&self) -> Self::Decoder {
        let admitted = self.gate.0.lock().ok().and_then(|_fence| self.cursor.lock().ok().map(|mut cursor| {
            if cursor.claimed || cursor.stopped || cursor.fault.is_some() {
                cursor.fault.get_or_insert_with(|| "A decoder cannot silently restart its exact playback".into()); false
            } else { cursor.claimed = true; true }
        })).unwrap_or(false);
        let duration = self.cursor.lock().ok().and_then(|cursor| {
            (!cursor.looping).then(|| Duration::from_secs_f64(
                (self.clip.identity.total_samples - cursor.next_sample + cursor.next_sample % u64::from(self.clip.identity.channels)) as f64
                    / f64::from(self.clip.identity.channels) / f64::from(self.clip.identity.sample_rate)))
        });
        PcmSource { reader: self.clone(), admitted, duration, output_channel: 0 }
    }
}

impl Iterator for PcmSource {
    type Item = i16;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.admitted { return None; }
        let _fence = self.reader.gate.0.lock().ok()?;
        let mut cursor = self.reader.cursor.lock().ok()?;
        let channels = self.reader.clip.identity.channels;
        if cursor.stopped || cursor.fault.is_some() { return None; }
        if cursor.next_sample == self.reader.clip.identity.total_samples {
            if !cursor.looping { return None; }
            if cursor.paused || _fence.preview_paused {
                self.output_channel = (self.output_channel + 1) % channels;
                return Some(0);
            }
            let Some(next) = cursor.loops_completed.checked_add(1) else {
                cursor.fault = Some("Audio loop counter exhausted; cursor unchanged".into()); return None;
            };
            cursor.loops_completed = next; cursor.next_sample = 0;
        }
        if cursor.paused || _fence.preview_paused || self.output_channel != (cursor.next_sample % u64::from(channels)) as u16 {
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
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { self.reader.clip.identity.channels }
    fn sample_rate(&self) -> u32 { self.reader.clip.identity.sample_rate }
    fn total_duration(&self) -> Option<Duration> { self.duration }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clip(channels: u16, samples: &[i16]) -> Arc<PcmClip> {
        PcmClip::from_pcm([17; 32], channels, 48_000, samples.to_vec()).unwrap()
    }
    fn checkpoint(gate: &CheckpointGate, reader: &CheckpointPcm) -> AudioCheckpoint {
        gate.snapshot(&[reader], |mut rows| Ok(rows.remove(0))).unwrap()
    }
    #[test]
    fn cursor_counts_actual_source_pulls_and_pause_has_no_clock_or_sample_advance() {
        let gate = CheckpointGate::default(); let reader = clip(1, &[10,20,30,40]).start(gate.clone(), false);
        let mut source = reader.decoder(); assert_eq!(source.next(), Some(10)); assert_eq!(source.next(), Some(20));
        reader.set_paused(true).unwrap(); assert_eq!(source.by_ref().take(128).collect::<Vec<_>>(), vec![0;128]);
        let saved = checkpoint(&gate, &reader); assert_eq!(saved.next_sample, 2);
        reader.set_paused(false).unwrap(); assert_eq!(source.next(), Some(30));
        assert_eq!(checkpoint(&gate, &reader).next_sample, 3);
    }
    #[test]
    fn stereo_partial_frame_restore_keeps_right_channel_without_rewind_or_sample_skip() {
        let gate = CheckpointGate::default(); let clip = clip(2, &[11,12,21,22,31,32]);
        let reader = clip.start(gate.clone(), false); let mut source = reader.decoder();
        assert_eq!(source.by_ref().take(3).collect::<Vec<_>>(), vec![11,12,21]);
        let saved = checkpoint(&gate, &reader); assert_eq!((saved.next_sample,saved.next_channel), (3,1));
        let restored = clip.restore(CheckpointGate::default(), &saved, false).unwrap();
        let mut aligned = restored.decoder(); assert_eq!(aligned.next(), Some(0));
        assert_eq!(checkpoint(&restored.gate, &restored).next_sample, 3);
        assert_eq!(aligned.next(), Some(22));
        let restored = clip.restore(CheckpointGate::default(), &saved, false).unwrap();
        // Real Rodio converter begins at channel zero; pad keeps R1 on the right.
        // Rodio 0.18.1 ChannelCountConverter::size_hint subtracts channel
        // position from lower=0 and wraps at an odd pull. Drain next() without
        // using that upstream hint; retain every real converted sample/assert.
        let mut uniform = rodio::source::UniformSourceIterator::<_, i16>::new(restored.decoder(), 2, 48_000);
        let actual = std::iter::from_fn(|| uniform.next()).collect::<Vec<_>>();
        assert_eq!(actual, vec![0,22,31,32]);
        assert_eq!(actual[1], 22); assert_eq!(actual[2], 31);
        let wrong: Vec<_> = clip.samples[saved.next_sample as usize..].to_vec();
        assert_eq!(wrong[0], 22); assert_ne!(wrong, actual, "fresh-channel origin would put R1 into L");
    }
    #[test]
    fn stereo_pause_resume_after_an_odd_silent_pull_keeps_channel_orientation() {
        let gate=CheckpointGate::default();let clip=clip(2,&[11,12,21,22,31,32]);
        let reader=clip.start(gate.clone(),false);
        let mut output=rodio::source::UniformSourceIterator::<_,i16>::new(reader.decoder(),2,48_000);
        assert_eq!(output.next(),Some(11));
        reader.set_paused(true).unwrap();assert_eq!(output.next(),Some(0));
        let saved=checkpoint(&gate,&reader);assert_eq!((saved.next_sample,saved.next_channel),(1,1));
        reader.set_paused(false).unwrap();
        // The next output is L1: one explicit zero aligns R0 to the following R.
        assert_eq!(output.next(),Some(0));assert_eq!(checkpoint(&gate,&reader),saved);
        assert_eq!(output.next(),Some(12));assert_eq!(output.next(),Some(21));assert_eq!(output.next(),Some(22));
        assert_eq!(checkpoint(&gate,&reader).next_sample,4);
    }
    #[test]
    fn a_second_decoder_is_diagnosed_and_never_replays_from_zero() {
        let gate=CheckpointGate::default(); let reader=clip(1,&[1,2,3]).start(gate.clone(),false);
        let mut first=reader.decoder(); assert_eq!(first.next(),Some(1));
        assert_eq!(reader.decoder().next(),None); assert!(gate.snapshot(&[&reader], |_|Ok(())).is_err());
        assert_eq!(first.next(),None);
    }
    #[test]
    fn restored_reader_rejects_asset_pcm_metadata_and_phase_drift_before_creation() {
        let gate=CheckpointGate::default(); let clip=clip(2,&[1,2,3,4]); let reader=clip.start(gate.clone(),false);
        let original=checkpoint(&gate,&reader);
        for change in 0..5 {
            let mut bad=original.clone(); match change {0=>bad.asset_sha256[0]^=1,1=>bad.pcm_sha256[0]^=1,
                2=>bad.channels=1,3=>bad.next_sample=5,_=>bad.next_channel=1}
            assert!(clip.restore(gate.clone(),&bad,false).is_err());
        }
        assert_eq!(checkpoint(&gate,&reader),original);
    }
    #[test]
    fn aggregate_fence_refuses_foreign_and_duplicate_readers_without_cursor_change() {
        let gate=CheckpointGate::default(); let reader=clip(1,&[1,2]).start(gate.clone(),false);
        let other=clip(1,&[3,4]).start(CheckpointGate::default(),false);
        assert!(gate.snapshot(&[&reader,&reader], |_|Ok(())).is_err());
        assert!(gate.snapshot(&[&reader,&other], |_|Ok(())).is_err());
        assert_eq!(checkpoint(&gate,&reader).next_sample,0);
    }
    #[test]
    fn source_thread_cannot_advance_any_reader_during_the_same_fenced_state_clone() {
        let gate=CheckpointGate::default();let first=clip(1,&[10,20]).start(gate.clone(),false);
        let second=clip(1,&[30,40]).start(gate.clone(),false);let mut source=second.decoder();
        let (started_rx,done_rx,join)=gate.snapshot(&[&first,&second],|rows| {
            assert!(rows.iter().all(|row|row.next_sample==0));
            let (started_tx,started_rx)=std::sync::mpsc::channel();let (done_tx,done_rx)=std::sync::mpsc::channel();
            let join=std::thread::spawn(move|| {started_tx.send(()).unwrap();done_tx.send(source.next()).unwrap();});
            started_rx.recv().unwrap();assert_eq!(done_rx.try_recv(),Err(std::sync::mpsc::TryRecvError::Empty));
            Ok((started_rx,done_rx,join))
        }).unwrap();drop(started_rx);
        assert_eq!(done_rx.recv().unwrap(),Some(30));join.join().unwrap();
        assert_eq!(checkpoint(&gate,&first).next_sample,0);assert_eq!(checkpoint(&gate,&second).next_sample,1);
    }
    #[test]
    fn completed_decoder_restores_terminal_and_revocation_cannot_invent_a_checkpoint() {
        let gate=CheckpointGate::default(); let clip=clip(1,&[7,8]); let reader=clip.start(gate.clone(),false);
        assert_eq!(reader.decoder().collect::<Vec<_>>(),vec![7,8]); let saved=checkpoint(&gate,&reader);
        assert_eq!(saved.next_sample,2); assert_eq!(clip.restore(gate.clone(),&saved,false).unwrap().decoder().next(),None);
        reader.stop().unwrap(); assert!(gate.snapshot(&[&reader], |_|Ok(())).is_err());
    }
    #[test]
    fn source_duration_end_and_loop_phase_are_exact_for_mono_and_stereo() {
        for channels in [1,2] {
            let gate=CheckpointGate::default(); let clip=clip(channels,&[1,2,3,4]);
            let reader=clip.start(gate.clone(),false); let mut source=reader.decoder();
            assert_eq!(source.channels(),channels); assert_eq!(source.sample_rate(),48_000);
            assert_eq!(source.total_duration(),Some(Duration::from_secs_f64(4.0/f64::from(channels)/48_000.0)));
            assert_eq!(source.next(),Some(1)); let saved=checkpoint(&gate,&reader);
            let restored=clip.restore(CheckpointGate::default(),&saved,false).unwrap(); let source=restored.decoder();
            assert_eq!(source.total_duration(),Some(Duration::from_secs_f64((3+u64::from(saved.next_channel)) as f64/f64::from(channels)/48_000.0)));
            let looped=clip.start_looped(gate.clone(),false); let mut source=looped.decoder();
            assert_eq!(source.total_duration(),None);
            assert_eq!(source.by_ref().take(9).collect::<Vec<_>>(),vec![1,2,3,4,1,2,3,4,1]);
            let saved=checkpoint(&gate,&looped); assert_eq!((saved.next_sample,saved.loops_completed),(1,2));
            looped.set_paused(true).unwrap(); assert_eq!(source.by_ref().take(8).collect::<Vec<_>>(),vec![0;8]);
            assert_eq!(checkpoint(&gate,&looped),saved);
            let restored=clip.restore(CheckpointGate::default(),&saved,false).unwrap();
            let suffix=restored.decoder().take(3+usize::from(saved.next_channel)).collect::<Vec<_>>();
            assert_eq!(suffix,if channels==2 {vec![0,2,3,4]} else {vec![2,3,4]});
        }
    }
    #[test]
    fn loop_overflow_is_a_checked_fault_without_wrapping_the_cursor() {
        let gate=CheckpointGate::default();let clip=clip(1,&[1,2]);
        let mut saved=clip.identity.clone();saved.looping=true;saved.loops_completed=u64::MAX;saved.next_sample=2;
        let reader=clip.restore(gate.clone(),&saved,false).unwrap();assert_eq!(reader.decoder().next(),None);
        let cursor=reader.cursor.lock().unwrap();assert_eq!(cursor.next_sample,2);assert_eq!(cursor.loops_completed,u64::MAX);drop(cursor);
        assert!(gate.snapshot(&[&reader], |_|Ok(())).is_err());
    }
    #[test]
    fn real_wav_decode_is_bounded_and_snapshot_restore_preserves_samples_not_time() {
        let samples=[-1234i16,1234,2345,-2345];
        let mut wav=Vec::new(); wav.extend_from_slice(b"RIFF"); wav.extend_from_slice(&(36u32+samples.len() as u32*2).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt "); wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&48_000u32.to_le_bytes()); wav.extend_from_slice(&192_000u32.to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes()); wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data"); wav.extend_from_slice(&(samples.len() as u32*2).to_le_bytes());
        for sample in samples { wav.extend_from_slice(&sample.to_le_bytes()); }
        let clip=PcmClip::decode("sfx/owned.wav",&wav).unwrap(); assert_eq!(&*clip.samples,&samples);
        let gate=CheckpointGate::default(); let reader=clip.start(gate.clone(),false); let mut source=reader.decoder();
        assert_eq!(source.next(),Some(-1234)); let saved=checkpoint(&gate,&reader);
        let encoded=serde_json::to_vec(&saved).unwrap(); let decoded=serde_json::from_slice(&encoded).unwrap();
        assert_eq!(clip.restore(CheckpointGate::default(),&decoded,false).unwrap().decoder().collect::<Vec<_>>(),vec![0,1234,2345,-2345]);
        assert!(PcmClip::decode("../foreign.wav",&wav).is_err()); assert!(PcmClip::decode("sfx/bad.wav",b"not audio").is_err());
        assert!(PcmClip::decode("sfx/empty.wav",&[]).is_err());
    }
    #[test]
    fn actual_pcm_staging_counts_vec_and_immutable_copy_without_charging_shared_arc_again() {
        let encoded:Arc<[u8]>=crate::checkpoint_sources::fixture_wave_for_test().into();
        // Eight i16 samples: a sixteen-byte Vec and sixteen-byte immutable Arc
        // coexist during conversion. The encoded AssetServer Arc is shared.
        let decoded=PcmClip::decode_shared("sfx/owned.wav",encoded.clone(),32).unwrap();
        assert_eq!(&*decoded.samples,&[100,200,300,400,500,600,700,800]);
        assert_eq!(decoded.pcm_bytes(),16);
        assert!(PcmClip::decode_shared("sfx/owned.wav",encoded.clone(),31).is_err());
        assert!(PcmClip::decode_shared("sfx/owned.wav",encoded.clone(),0).is_err());
        let shared=decoded.clone();assert_eq!(shared.allocation_key(),decoded.allocation_key());
        assert_eq!(shared.pcm_bytes(),16);assert_eq!(Arc::strong_count(&encoded),1);
    }
    #[test]
    fn actual_decoded_pcm_growth_preserves_samples_digests_and_tight_peak_budget() {
        // Cross four capacity boundaries with real interleaved stereo WAV data.
        let samples:Vec<i16>=(0..32_770).map(|index|(index%32_768) as i16).collect();
        let bytes=(samples.len()*2) as u32;
        let mut wave=Vec::new();
        wave.extend_from_slice(b"RIFF");wave.extend_from_slice(&(36+bytes).to_le_bytes());wave.extend_from_slice(b"WAVEfmt ");
        wave.extend_from_slice(&16u32.to_le_bytes());wave.extend_from_slice(&1u16.to_le_bytes());wave.extend_from_slice(&2u16.to_le_bytes());
        wave.extend_from_slice(&48_000u32.to_le_bytes());wave.extend_from_slice(&192_000u32.to_le_bytes());
        wave.extend_from_slice(&4u16.to_le_bytes());wave.extend_from_slice(&16u16.to_le_bytes());wave.extend_from_slice(b"data");wave.extend_from_slice(&bytes.to_le_bytes());
        for sample in &samples {wave.extend_from_slice(&sample.to_le_bytes());}
        let encoded:Arc<[u8]>=wave.into();
        // Vec plus immutable Arc can coexist exactly at four bytes per sample.
        let peak=samples.len() as u64*4;
        let tight=PcmClip::decode_shared("sfx/stereo.wav",encoded.clone(),peak).unwrap();
        let roomy=PcmClip::decode_shared("sfx/stereo.wav",encoded.clone(),256*1024*1024).unwrap();
        assert_eq!(&*tight.samples,&*samples);assert_eq!(&*roomy.samples,&*samples);
        assert_eq!(tight.identity,roomy.identity);let asset_digest:[u8;32]=Sha256::digest(&encoded).into();assert_eq!(tight.asset_digest(),asset_digest);
        assert_eq!((tight.identity.channels,tight.identity.sample_rate,tight.identity.total_samples),(2,48_000,32_770));
        assert_eq!(tight.pcm_bytes(),samples.len() as u64*2);tight.identity.validate().unwrap();
        assert!(PcmClip::decode_shared("sfx/stereo.wav",encoded.clone(),peak-1).is_err());
        assert!(PcmClip::decode_shared("sfx/stereo.wav",encoded.clone(),0).is_err());
        assert_eq!(Arc::strong_count(&encoded),1);
    }
}

#[cfg(test)]
mod preview_pause_contract {
    use super::*;
    #[test]
    fn one_preview_fence_pauses_existing_and_new_readers_without_losing_scene_residence_or_stereo_phase(){
        let clip=PcmClip::from_pcm([31;32],2,48_000,vec![11,12,21,22]).unwrap();
        let gate=CheckpointGate::default();let current=clip.start(gate.clone(),false);let mut source=current.decoder();
        assert_eq!(source.next(),Some(11));
        gate.set_preview_paused(true).unwrap();
        let new=clip.start(gate.clone(),false);let mut fresh=new.decoder();
        for _ in 0..3{assert_eq!(source.next(),Some(0));assert_eq!(fresh.next(),Some(0));}
        let rows=gate.snapshot_with_status(&[&current,&new],Ok).unwrap();
        assert_eq!((rows[0].0.next_sample,rows[1].0.next_sample),(1,0));
        assert!(!rows[0].1&&!rows[1].1);
        new.set_paused(true).unwrap();gate.set_preview_paused(false).unwrap();
        // Output channel zero now needs one explicit alignment sample before R0.
        assert_eq!(source.next(),Some(0));assert_eq!(source.next(),Some(12));
        assert_eq!(fresh.next(),Some(0));assert_eq!(gate.snapshot(&[&new],|rows|Ok(rows[0].next_sample)).unwrap(),0);
    }
}
