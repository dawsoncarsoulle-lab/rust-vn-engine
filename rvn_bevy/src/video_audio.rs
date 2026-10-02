//! Bounded streaming stereo PCM. Underflow emits silence without advancing
//! the media clock: a slow decoder cannot silently skip video or sound.
use bevy::{asset as bevy_asset, reflect as bevy_reflect};
use bevy::{
    audio::{Decodable, Source},
    prelude::*,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
const RATE: usize = 48_000;
pub(crate) const CAPACITY: usize = RATE * 2 * 2;
pub(crate) struct Buffer {
    samples: VecDeque<f32>,
    pub position: f64,
    queued_until: f64,
    pub running: bool,
    pub ended: bool,
}
impl Buffer {
    pub fn new(position: f64) -> Self {
        Self {
            samples: VecDeque::new(),
            position,
            queued_until: position,
            running: false,
            ended: false,
        }
    }
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
#[derive(Asset, TypePath, Clone)]
pub(crate) struct VideoAudio(pub Arc<Mutex<Buffer>>);
pub(crate) struct Samples {
    buffer: Arc<Mutex<Buffer>>,
    right: Option<f32>,
}
impl Decodable for VideoAudio {
    type DecoderItem = f32;
    type Decoder = Samples;
    fn decoder(&self) -> Samples {
        Samples {
            buffer: self.0.clone(),
            right: None,
        }
    }
}
impl Iterator for Samples {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        // An audio callback must never wait for the game thread.
        // Acquire an entire stereo frame at once. Contention must not insert
        // silence between its left and right samples and swap channel phase.
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        let Ok(mut buffer) = self.buffer.try_lock() else {
            self.right = Some(0.0);
            return Some(0.0);
        };
        if !buffer.running {
            self.right = Some(0.0);
            return Some(0.0);
        }
        match buffer.samples.pop_front() {
            Some(left) => {
                self.right = Some(buffer.samples.pop_front().expect("PCM is stereo aligned"));
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
