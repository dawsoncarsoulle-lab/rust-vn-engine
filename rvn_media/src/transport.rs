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
pub struct Worker {
    receiver: Mutex<Receiver<Packet>>,
}
impl Worker {
    pub fn spawn(path: PathBuf, position: f64) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(2);
        std::thread::Builder::new()
            .name("rvn-video".into())
            .spawn(move || {
                if let Err(problem) = decode(path, position, &sender) {
                    let _ = sender.send(Packet::Error(problem));
                }
            })
            .map_err(|error| format!("Cannot start video decoder: {error}"))?;
        Ok(Self {
            receiver: Mutex::new(receiver),
        })
    }
    pub fn poll(&self) -> Result<Option<Packet>, String> {
        match self
            .receiver
            .lock()
            .map_err(|_| "Video decoder channel was poisoned")?
            .try_recv()
        {
            Ok(packet) => Ok(Some(packet)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Video decoder disconnected before finishing".into())
            }
        }
    }
}
fn decode(path: PathBuf, position: f64, sender: &SyncSender<Packet>) -> Result<(), String> {
    let mut decoder = Decoder::open(&path)?;
    if position > 0.0 {
        decoder.seek(position)?;
    }
    if sender
        .send(Packet::Ready(decoder.metadata.clone()))
        .is_err()
    {
        return Ok(());
    }
    loop {
        let chunks = decoder.next()?;
        let ended = chunks.iter().any(|chunk| matches!(chunk, Chunk::End));
        for chunk in chunks {
            if sender.send(Packet::Data(chunk)).is_err() {
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
