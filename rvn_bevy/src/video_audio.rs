//! Bounded streaming stereo PCM. Underflow emits silence without advancing
//! the media clock: a slow decoder cannot silently skip video or sound.
use bevy::{asset as bevy_asset, reflect as bevy_reflect};
use bevy::{
    audio::{Decodable, Source},
    prelude::*,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex,atomic::{AtomicUsize,AtomicBool,Ordering}},
    time::Duration,
};
const RATE: usize = 48_000;
pub(crate) const CAPACITY: usize = RATE * 2 * 2;
struct ExactDelivery {
 asset:[u8;32],profile:rvn_media::checkpoint::DecodedSourceProfile,next:u64,queued:u64,readers:usize,
}
#[derive(Default)]
struct PendingRight{count:AtomicUsize,abandoned:AtomicBool}
pub(crate) struct Buffer {
    samples: VecDeque<f32>,
    pending_right:Arc<PendingRight>,
    exact:Option<ExactDelivery>,
    pub position: f64,
    queued_until: f64,
    pub running: bool,
    pub ended: bool,
    gate:Option<crate::checkpoint_pcm::CheckpointGate>,
    authority:Option<VideoBufferAuthority>,
    pub(crate) revoked:bool,
}
impl Buffer {
    #[cfg(test)]
    pub(crate) fn delivered_sample_offset(&self)->Option<u64>{self.exact.as_ref().map(|e|e.next)}
    pub fn new(position: f64) -> Self {
        Self {
            samples: VecDeque::new(),pending_right:Arc::new(PendingRight::default()),exact:None,
            position,
            queued_until: position,
            running: false,
            ended: false,
            gate:None, authority:None, revoked:false,
        }
    }
    pub(crate) fn bind_exact(&mut self,asset:[u8;32],profile:rvn_media::checkpoint::DecodedSourceProfile)->Result<(),String>{
        profile.validate()?;if self.exact.is_some()||!self.samples.is_empty(){return Err("Native canonical source was already populated".into());}
        self.exact=Some(ExactDelivery{asset,profile,next:0,queued:0,readers:0});Ok(())
    }
    pub(crate) fn push_exact(&mut self,first:u64,samples:Vec<f32>)->Result<(),String>{
        let exact=self.exact.as_mut().ok_or("Native packet has no canonical source")?;
        if first!=exact.queued||samples.len()%2!=0||samples.iter().any(|s|!s.is_finite())||self.samples.len()+samples.len()>CAPACITY{return Err("Noncontiguous or oversized canonical PCM packet".into());}
        exact.queued=first.checked_add(samples.len()as u64).filter(|n|*n<=rvn_media::checkpoint::MAX_SAMPLES).ok_or("Native output ordinal overflow")?;self.samples.extend(samples);Ok(())
    }
    /// Gate caller must keep the producer fence held. Shared queue includes
    /// the right sample after a left delivery; no sampler-local right is lost.
    pub(crate) fn exact_cursor(&self)->Result<([u8;32],rvn_media::checkpoint::DecodedSourceProfile,rvn_media::checkpoint::AudioCursor),String>{
        let exact=self.exact.as_ref().ok_or("Native source is position-only")?;
        if exact.readers!=1{return Err("Canonical native source requires its one actual reader".into());}
        let count=self.samples.len().min(rvn_media::checkpoint::BOUNDARY_SAMPLES);
        if count==0&&!self.ended{return Err("Canonical boundary is not yet decoded".into());}
        let cursor=rvn_media::checkpoint::AudioCursor{next_sample:exact.next,next_channel:(exact.next%2)as u16,boundary_samples:count as u16,boundary_sha256:rvn_media::checkpoint::pcm_boundary(exact.next,self.samples.iter().take(count).copied()),ended:self.ended&&count==0};cursor.validate()?;
        Ok((exact.asset,exact.profile.clone(),cursor))
    }
    /// Pre-roll only advances through actual decoded samples, never seconds.
    pub(crate) fn preroll_exact(&mut self,target:&rvn_media::checkpoint::AudioCursor)->Result<bool,String>{
        target.validate()?;let exact=self.exact.as_mut().ok_or("Native pre-roll lacks source provenance")?;
        if exact.next>target.next_sample{return Err("Native pre-roll passed requested ordinal".into());}
        while exact.next<target.next_sample{if self.samples.pop_front().is_none(){return if self.ended{Err("Native ordinal is beyond decoded EOF".into())}else{Ok(false)};}exact.next+=1;}
        if self.samples.len()<(target.boundary_samples as usize){return if self.ended{Err("Native boundary exceeds EOF".into())}else{Ok(false)};}
        if target.ended&&(!self.ended||!self.samples.is_empty()){return if self.ended{Err("Native EOF cursor disagrees with stream".into())}else{Ok(false)};}
        let hash=rvn_media::checkpoint::pcm_boundary(exact.next,self.samples.iter().take(target.boundary_samples as usize).copied());if hash!=target.boundary_sha256{return Err("Native decoded PCM boundary differs at saved ordinal".into());}Ok(true)
    }
    /// EOF requires the actual final right delivery, even for historical
    /// position-only Sources whose right is sampler-local rather than queued.
    pub(crate) fn playout_drained(&self)->bool{self.samples.is_empty()&&self.pending_right.count.load(Ordering::Acquire)==0&&!self.playout_abandoned()}
    pub(crate) fn playout_abandoned(&self)->bool{self.pending_right.abandoned.load(Ordering::Acquire)}
    pub fn len(&self) -> usize {
        self.samples.len()
    }
    pub fn push(&mut self, seconds: f64, samples: Vec<f32>) -> Result<(), String> {
        if !seconds.is_finite()
            || samples.len() % 2 != 0
            || samples.iter().any(|sample| !sample.is_finite())
        {
            return Err("Invalid video audio buffer".into());
        }
        // Match packet timestamps without replaying overlaps after seeks.
        let gap = ((seconds - self.queued_until).max(0.0) * RATE as f64).round() as usize * 2;
        let skip = ((self.queued_until - seconds).max(0.0) * RATE as f64).round() as usize * 2;
        let wanted = samples.len().saturating_sub(skip);
        if gap > CAPACITY || self.samples.len() + gap + wanted > CAPACITY {
            return Err("Video audio exceeds its two-second buffer".into());
        }
        self.samples.extend(std::iter::repeat_n(0.0, gap));
        self.samples.extend(samples.into_iter().skip(skip));
        self.queued_until += ((gap + wanted) / 2) as f64 / RATE as f64;
        Ok(())
    }
}
/// Derived only from actual Core views/preparation by the NativeVideo owner.
#[derive(Clone,Debug,PartialEq)]
pub(crate) struct VideoBufferAuthority {
    instance:u64,id:String,epoch:u64,token:Option<rvn_core::scene_resources::ResourceToken>,clip:rvn_ui::video::VideoClip,
}
impl VideoBufferAuthority {
    fn of(instance:u64,view:&rvn_core::video::VideoView)->Self{Self{instance,id:view.id.clone(),epoch:view.epoch,token:view.resource_token.clone(),clip:view.track.clip.clone()}}
    pub(crate) fn matches(&self,instance:u64,view:&rvn_core::video::VideoView)->bool{
        let mut clip=self.clip.clone();clip.volume=view.track.clip.volume;
        self.instance==instance && self.id==view.id && self.epoch==view.epoch && self.token==view.resource_token && clip==view.track.clip
    }
}
impl Buffer {
    pub(crate) fn verifies_owner(&self,instance:u64,gate:&crate::checkpoint_pcm::CheckpointGate)->bool{
        self.authority.as_ref().is_some_and(|authority|authority.instance==instance)&&self.gate.as_ref().is_some_and(|actual|actual.same_gate(gate))
    }
}
fn owned_buffer(position:f64,gate:crate::checkpoint_pcm::CheckpointGate,authority:VideoBufferAuthority)->VideoAudio{
    let mut buffer=Buffer::new(position);buffer.gate=Some(gate);buffer.authority=Some(authority);
    VideoAudio(Arc::new(Mutex::new(buffer)))
}
pub(crate) fn source_for_current(world:&World,view:&rvn_core::video::VideoView,position:f64)->Result<VideoAudio,String>{
    let game=&world.get_resource::<crate::resources::VnEngine>().ok_or("Native audio requires its actual Engine")?.0;
    let authority=VideoBufferAuthority::of(game.instance_id(),view);
    if !game.video_views().map_err(|error|error.to_string())?.iter().any(|actual|authority.matches(game.instance_id(),actual)){
        return Err("Native video audio belongs to a stale Core capability".into());
    }
    Ok(owned_buffer(position,game.renderer.audio_checkpoints.gate(),authority))
}
pub(crate) fn source_for_prepared(world:&World,core:&rvn_core::engine::PreparedSave15,view:&rvn_core::video::VideoView)->Result<VideoAudio,String>{
    let game=&world.get_resource::<crate::resources::VnEngine>().ok_or("Native staging requires its actual Engine")?.0;
    if !game.accepts_prepared_save15(core)||!core.render_batch().videos.iter().any(|actual|actual==view){return Err("Native candidate audio has no authenticated Core preparation".into());}
    Ok(owned_buffer(view.track.position,game.renderer.audio_checkpoints.gate(),VideoBufferAuthority::of(game.instance_id(),view)))
}
/// Only the actual pending startup owner can admit the future reader. The
/// opaque source and exact batch member are recalculated before publication.
pub(crate) fn source_for_startup(world:&World,core:&rvn_core::engine::PreparedStartupStep,view:&rvn_core::video::VideoView)->Result<VideoAudio,String>{
    if world.contains_resource::<crate::resources::VnEngine>(){return Err("Native startup audio cannot share a gameplay owner".into());}
    let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>().ok_or("Native startup audio requires its actual owner")?;
    let authority=authority_for_startup(world,core,view)?;
    Ok(owned_buffer(view.track.position,startup.broker.gate(),authority))
}
#[derive(Asset, TypePath, Clone)]
pub(crate) struct VideoAudio(pub Arc<Mutex<Buffer>>);
pub(crate) struct Samples {
    buffer: Arc<Mutex<Buffer>>,
    right: Option<f32>,
    right_is_media:bool,
    pending_right:Arc<PendingRight>,
    gate:Option<crate::checkpoint_pcm::CheckpointGate>,
    output_channel:u8,
    exact_claim:bool,
    invalid_claim:bool,
}
impl Decodable for VideoAudio {
    type DecoderItem = f32;
    type Decoder = Samples;
    fn decoder(&self) -> Samples {
        let (gate,exact_claim,invalid_claim,pending_right)=if let Ok(mut buffer)=self.0.lock(){let gate=buffer.gate.clone();let pending=buffer.pending_right.clone();if let Some(exact)=buffer.exact.as_mut(){exact.readers+=1;(gate,true,exact.readers!=1,pending)}else{(gate,false,false,pending)}}else{(None,false,true,Arc::new(PendingRight::default()))};
        Samples {
            buffer: self.0.clone(),
            right: None,right_is_media:false,pending_right,
            gate,exact_claim,invalid_claim,
            output_channel:0,
        }
    }
}
impl Samples {
    fn keep_media_right(&mut self,right:f32){debug_assert!(self.right.is_none());self.right=Some(right);self.right_is_media=true;self.pending_right.count.fetch_add(1,Ordering::Release);}
    fn deliver_right(&mut self)->Option<f32>{let value=self.right.take();if value.is_some()&&self.right_is_media{self.right_is_media=false;let previous=self.pending_right.count.fetch_sub(1,Ordering::AcqRel);debug_assert!(previous>0);}value}
    fn silence(&mut self)->Option<f32>{self.output_channel=(self.output_channel+1)%2;Some(0.0)}
}
impl Drop for Samples{fn drop(&mut self){if self.right_is_media{self.pending_right.abandoned.store(true,Ordering::Release);let previous=self.pending_right.count.fetch_sub(1,Ordering::AcqRel);debug_assert!(previous>0);self.right_is_media=false;}if self.exact_claim{if let Ok(mut buffer)=self.buffer.lock(){if let Some(exact)=buffer.exact.as_mut(){exact.readers=exact.readers.saturating_sub(1);}}}}}
impl Iterator for Samples {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.invalid_claim{return None;}
        if let Some(gate)=self.gate.clone(){
            let buffer_arc=self.buffer.clone();
            let value=gate.try_read(|preview_paused|{
                if preview_paused{return self.silence();}
                let Ok(mut buffer)=buffer_arc.try_lock()else{return self.silence();};
                if buffer.revoked{return None;}
                if !buffer.running{return self.silence();}
                if buffer.exact.is_some(){
                    let next=buffer.exact.as_ref().unwrap().next;
                    if self.output_channel!=(next%2)as u8{return self.silence();}
                    return match buffer.samples.pop_front(){Some(sample)=>{buffer.exact.as_mut().unwrap().next+=1;if next%2==0{buffer.position+=1.0/RATE as f64;}self.output_channel=(self.output_channel+1)%2;Some(sample)},None if buffer.ended=>None,None=>self.silence()};
                }
                if self.right.is_some(){
                    if self.output_channel!=1{return self.silence();}
                    self.output_channel=0;return self.deliver_right();
                }
                if self.output_channel!=0{return self.silence();}
                match buffer.samples.pop_front(){
                    Some(left)=>{self.keep_media_right(buffer.samples.pop_front().expect("PCM is stereo aligned"));buffer.position+=1.0/RATE as f64;self.output_channel=1;Some(left)},
                    None if buffer.ended=>None,
                    None=>self.silence(),
                }
            });
            return value.unwrap_or_else(||self.silence());
        }
        // An audio callback must never wait for the game thread.
        // Acquire an entire stereo frame at once. Contention must not insert
        // silence between its left and right samples and swap channel phase.
        if let Some(right) = self.deliver_right() {
            return Some(right);
        }
        let buffer_arc=self.buffer.clone();
        let Ok(mut buffer) = buffer_arc.try_lock() else {
            self.right = Some(0.0);
            return Some(0.0);
        };
        if !buffer.running {
            self.right = Some(0.0);
            return Some(0.0);
        }
        match buffer.samples.pop_front() {
            Some(left) => {
                self.keep_media_right(buffer.samples.pop_front().expect("PCM is stereo aligned"));
                buffer.position += 1.0 / RATE as f64;
                Some(left)
            }
            None if buffer.ended => None,
            None => {
                self.right = Some(0.0);
                Some(0.0)
            }
        }
    }
}
impl Source for Samples {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        RATE as u32
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

// Explicit editor suspension also closes the streaming media-clock boundary.
// The ordinary owner-aware video tick alone resumes a buffer; this never seeks,
// consumes PCM, changes residence or restarts a decoding worker.
pub(crate) fn pause_sources(world:&World)->Result<(),String>{
    if let Some(sources)=world.get_resource::<Assets<VideoAudio>>(){
        for (_,source) in sources.iter(){
            source.0.lock().map_err(|_|"Video preview audio buffer is poisoned")?.running=false;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_pause_and_underflow_do_not_advance_the_clock_and_seek_overlap_is_discarded() {
        let audio = VideoAudio(Arc::new(Mutex::new(Buffer::new(0.5))));
        let mut samples = audio.decoder();
        assert_eq!(samples.next(), Some(0.0));
        assert_eq!(samples.next(), Some(0.0));
        assert_eq!(audio.0.lock().unwrap().position, 0.5);
        {
            let mut buffer = audio.0.lock().unwrap();
            buffer.push(0.5, vec![0.2, 0.4, 0.3, 0.5]).unwrap();
            buffer.running = true;
        }
        assert_eq!(samples.next(), Some(0.2));
        assert_eq!(samples.next(), Some(0.4));
        assert!((audio.0.lock().unwrap().position - (0.5 + 1.0 / 48000.0)).abs() < 1e-10);
        {
            let mut buffer = audio.0.lock().unwrap();
            buffer.running = false;
        }
        assert_eq!(samples.next(), Some(0.0));
        assert_eq!(samples.next(), Some(0.0));
        {
            let mut buffer = audio.0.lock().unwrap();
            buffer.running = true;
        }
        assert_eq!(samples.next(), Some(0.3));
        assert_eq!(samples.next(), Some(0.5));
        let position = audio.0.lock().unwrap().position;
        assert_eq!(samples.next(), Some(0.0));
        assert_eq!(audio.0.lock().unwrap().position, position);
        let mut buffer = audio.0.lock().unwrap();
        assert!(buffer.push(0.5, vec![1.0, 1.0]).is_ok());
        assert_eq!(buffer.len(), 0);
        assert!(buffer.push(10.0, vec![1.0, 1.0]).is_err());
        assert!(buffer.push(0.5, vec![f32::NAN, 0.0]).is_err());
    }
    #[test]
    fn contention_never_splits_a_stereo_frame() {
        let audio = VideoAudio(Arc::new(Mutex::new(Buffer::new(0.0))));
        let mut samples = audio.decoder();
        {
            let mut buffer = audio.0.lock().unwrap();
            buffer.push(0.0, vec![0.2, 0.4, 0.3, 0.5]).unwrap();
            buffer.running = true;
        }
        assert_eq!(samples.next(), Some(0.2));
        let guard = audio.0.lock().unwrap();
        assert_eq!(samples.next(), Some(0.4));
        assert_eq!(samples.next(), Some(0.0));
        drop(guard);
        assert_eq!(samples.next(), Some(0.0));
        assert_eq!(samples.next(), Some(0.3));
        assert_eq!(samples.next(), Some(0.5));
    }
}

/// An actual admitted owner/batch issues this private capability, never a DTO.
pub(crate) fn authority_for_startup(world:&World,core:&rvn_core::engine::PreparedStartupStep,view:&rvn_core::video::VideoView)->Result<VideoBufferAuthority,String>{
 if world.contains_resource::<crate::resources::VnEngine>(){return Err("Native startup reader cannot share gameplay owner".into());}
 let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>().ok_or("Actual startup reader owner absent")?;
 let source=startup.owner.prepared_publication_source(core).map_err(|e|e.to_string())?;
 let batch=core.render_batch().ok_or("Computation-only startup has no native reader")?;
 if batch.runtime_epoch!=source.runtime_epoch()||!batch.videos.iter().any(|actual|actual==view){return Err("Native startup reader has no authenticated batch member".into());}
 Ok(VideoBufferAuthority::of(source.instance_id(),view))
}
impl Buffer{
 pub(crate) fn verifies_view(&self,instance:u64,view:&rvn_core::video::VideoView)->bool{self.authority.as_ref().is_some_and(|authority|authority.matches(instance,view))}
 /// Only Native's owned prepared plan calls this inside broker.publish's gate,
 /// after full Source/batch re-admission. Logical controls do not reset PCM.
 pub(crate) fn install_prepared_authority(&mut self,authority:VideoBufferAuthority,playing:bool){self.authority=Some(authority);self.running=playing;}
}

/// Ordinary/loop decoder start for the actual still-owned bootstrap. The view
/// must already belong to the accepted owner projection. No prepared step,
/// future DTO, or media position can manufacture this current capability.
pub(crate) fn source_for_startup_current(world:&World,view:&rvn_core::video::VideoView,position:f64)->Result<VideoAudio,String>{
 if world.contains_resource::<crate::resources::VnEngine>(){return Err("Current Startup audio cannot share gameplay owner".into());}
 let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>().ok_or("Current Startup audio requires actual owner")?;
 if startup.publication_pending||startup.failed.is_some()||!startup.accepts_actual_audio_broker(){return Err("Current Startup audio owner/publication/broker is not admitted".into());}
 let instance=startup.owner.snapshot().instance;
 if !startup.owner.video_views().map_err(|e|e.to_string())?.iter().any(|actual|actual==view){return Err("Current Startup audio view is not an accepted owner member".into());}
 if view.track.finished()||!position.is_finite()||!(0.0..=86400.0).contains(&position)||position.to_bits()!=view.track.position.to_bits()&&!(view.track.clip.looping&&position==0.0){return Err("Current Startup decoder start differs from its real playback operation".into());}
 // This is presentation position only. Canonical sample ordinals can be bound
 // later only from Ready/Audio proofs issued by the actual decoded worker.
 Ok(owned_buffer(position,startup.broker.gate(),VideoBufferAuthority::of(instance,view)))
}

#[cfg(all(test,feature="video"))]
#[path="native_video_audio_delivery_tests.rs"]
mod actual_delivery_tests;
