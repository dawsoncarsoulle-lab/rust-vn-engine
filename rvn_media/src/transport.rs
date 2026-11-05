//! A decoder lives on its own thread. Dropping the receiver cancels it, even
//! when it is blocked by back pressure. No decoder or filesystem work runs on
//! the game or audio thread; at most two decoded chunks wait in transit.
use crate::decoder::{Chunk, Decoder, Metadata};
use std::{
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, SyncSender},
        Mutex,
    },
};

pub enum Packet {
    Ready(Metadata),
    Data(Chunk),
    Error(String),
}
/// Neutral facts supplied by the actual Decoder. Runtime proof, not a token.
pub use crate::checkpoint::DecodedSourceProfile;
/*#[derive(Clone,Debug,PartialEq,Eq)]
pub struct DecodedSourceProfile{
    pub version:u32,pub video_stream:u32,pub audio_stream:Option<u32>,pub video_time_base:(i32,i32),pub audio_time_base:Option<(i32,i32)>,pub codec_version:u32,pub format_version:u32,pub util_version:u32,pub configuration:String,pub sample_rate:u32,pub channels:u16,
}
*/
#[derive(Clone,Debug)]
pub enum DecodedProof{
    Ready{asset_sha256:[u8;32],profile:DecodedSourceProfile},
    Audio{first_sample:u64},
    Video{ordinal:u64,pts:i64,time_base:(i32,i32)},
}
struct Envelope{packet:Packet,proof:Option<DecodedProof>}
pub struct Worker {
    receiver: Mutex<Receiver<Envelope>>,
}
impl Worker {
    pub fn spawn(path: PathBuf, position: f64) -> Result<Self, String> {
        Self::spawn_with_keepalive(path,position,None)
    }
    /// The actual decoder thread retains an owned staged input until it closes.
    pub fn spawn_with_keepalive(path:PathBuf,position:f64,keepalive:Option<std::sync::Arc<dyn Send+Sync>>)->Result<Self,String>{
        Self::spawn_inner(path,position,keepalive,false)
    }
    /// Decode the canonical stream from its real origin. Ordinals count actual
    /// returned samples/frames; no absolute cursor is derived from seconds.
    pub fn spawn_proven(path:PathBuf,keepalive:Option<std::sync::Arc<dyn Send+Sync>>)->Result<Self,String>{
        Self::spawn_inner(path,0.0,keepalive,true)
    }
    fn spawn_inner(path:PathBuf,position:f64,keepalive:Option<std::sync::Arc<dyn Send+Sync>>,proven:bool)->Result<Self,String>{
        let (sender, receiver) = mpsc::sync_channel(2);
        std::thread::Builder::new()
            .name("rvn-video".into())
            .spawn(move || {
                let _owned_input=keepalive;
                if let Err(problem) = decode(path, position, &sender,proven) {
                    let _ = sender.send(Envelope{packet:Packet::Error(problem),proof:None});
                }
            })
            .map_err(|error| format!("Cannot start video decoder: {error}"))?;
        Ok(Self {
            receiver: Mutex::new(receiver),
        })
    }
    pub fn poll(&self) -> Result<Option<Packet>, String> {
        self.poll_proven().map(|value|value.map(|(packet,_)|packet))
    }
    pub fn poll_proven(&self)->Result<Option<(Packet,Option<DecodedProof>)>,String>{
        match self
            .receiver
            .lock()
            .map_err(|_| "Video decoder channel was poisoned")?
            .try_recv()
        {
            Ok(envelope) => Ok(Some((envelope.packet,envelope.proof))),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Video decoder disconnected before finishing".into())
            }
        }
    }
}
fn decode(path: PathBuf, position: f64, sender: &SyncSender<Envelope>,proven:bool) -> Result<(), String> {
    let asset_sha256=if proven{
        use std::io::Read;use sha2::{Digest,Sha256};
        let size=std::fs::metadata(&path).map_err(|e|e.to_string())?.len();if size>1024*1024*1024{return Err("Proven native input exceeds1GiB".into());}let mut input=std::fs::File::open(&path).map_err(|e|e.to_string())?;let mut hash=Sha256::new();let mut bytes=0u64;let mut block=[0u8;65536];loop{let count=input.read(&mut block).map_err(|e|e.to_string())?;if count==0{break;}bytes=bytes.checked_add(count as u64).filter(|n|*n<=size).ok_or("Native input grew before canonical decode")?;hash.update(&block[..count]);}if bytes!=size{return Err("Native input changed before canonical decode".into());}Some(hash.finalize().into())
    }else{None};
    let mut decoder = if proven{Decoder::open_canonical(&path)?}else{Decoder::open(&path)?};
    if position > 0.0 {
        decoder.seek(position)?;
    }
    if sender
        .send(Envelope{packet:Packet::Ready(decoder.metadata.clone()),proof:asset_sha256.map(|asset_sha256|DecodedProof::Ready{asset_sha256,profile:decoder.source_profile()})})
        .is_err()
    {
        return Ok(());
    }
    let(mut sample_ordinal,mut frame_ordinal)=(0u64,0u64);
    loop {
        let chunks = decoder.next()?;
        let ended = chunks.iter().any(|chunk| matches!(chunk, Chunk::End));
        for chunk in chunks {
            let proof=if proven{match &chunk{
                Chunk::Audio{samples,..}=>{let first_sample=sample_ordinal;sample_ordinal=sample_ordinal.checked_add(samples.len()as u64).filter(|n|*n<=8_294_400_000).ok_or("Canonical native sample ordinal exceeds24h")?;Some(DecodedProof::Audio{first_sample})},
                Chunk::Frame{..}=>{let ordinal=frame_ordinal;frame_ordinal=frame_ordinal.checked_add(1).ok_or("Canonical frame ordinals exhausted")?;Some(DecodedProof::Video{ordinal,pts:decoder.take_canonical_video_pts().ok_or("Canonical frame provenance absent")?,time_base:decoder.source_profile().video_time_base})},Chunk::End=>None,
            }}else{None};
            if sender.send(Envelope{packet:Packet::Data(chunk),proof}).is_err() {
                return Ok(());
            }
        }
        if ended {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_worker_delivers_metadata_frames_sound_and_end_and_can_be_cancelled() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/videos/assets/clip.webm");
        let worker = Worker::spawn(fixture.clone(), 0.4).unwrap();
        let limit = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let (mut ready, mut frames, mut sound, mut ended) = (false, 0, 0, false);
        while !ended {
            assert!(std::time::Instant::now() < limit, "Decoder did not finish");
            match worker.poll().unwrap() {
                Some(Packet::Ready(meta)) => {
                    assert_eq!(meta.width, 320);
                    ready = true;
                }
                Some(Packet::Data(Chunk::Frame { seconds, .. })) => {
                    assert!(seconds >= 0.4);
                    frames += 1;
                }
                Some(Packet::Data(Chunk::Audio { samples, .. })) => sound += samples.len(),
                Some(Packet::Data(Chunk::End)) => ended = true,
                Some(Packet::Error(error)) => panic!("{error}"),
                None => std::thread::yield_now(),
            }
        }
        assert!(ready && frames > 10 && sound > 48000);
        // A consumer can close without joining a thread blocked on send.
        drop(Worker::spawn(fixture, 0.0).unwrap());
    }
}

#[cfg(test)]
#[path = "exact_cursor_tests.rs"]
mod exact_tests;
