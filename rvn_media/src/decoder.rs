//! Bounded packet-by-packet desktop decoding. This module never shells out to
//! ffmpeg and is linked only to the separately built LGPL shared libraries.
use ffmpeg::{codec, format, frame, media, software, ChannelLayout};
use ffmpeg_next as ffmpeg;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Metadata {
    pub width: u32,
    pub height: u32,
    pub duration: Option<f64>,
    pub audio: bool,
}
#[derive(Debug)]
pub enum Chunk {
    Frame {
        seconds: f64,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Audio {
        seconds: f64,
        samples: Vec<f32>,
    }, // stereo, 48 kHz
    End,
}
struct AudioTrack {
    index: usize,
    clock: f64,
    next_seconds: Option<f64>,
    decoder: ffmpeg::decoder::Audio,
    resampler: software::resampling::Context,
}
pub struct Decoder {
    input: format::context::Input,
    video: ffmpeg::decoder::Video,
    scaler: software::scaling::Context,
    video_index: usize,
    video_clock: f64,
    audio: Option<AudioTrack>,
    pub metadata: Metadata,
    ended: bool,
    seek_floor: f64,
}
fn error(error: impl std::fmt::Display) -> String {
    format!("Video decoder: {error}")
}
fn exhausted(error: ffmpeg::Error) -> Result<(), String> {
    match error {
        ffmpeg::Error::Eof
        | ffmpeg::Error::Other {
            errno: ffmpeg::error::EAGAIN,
        } => Ok(()),
        other => Err(self::error(other)),
    }
}
impl Decoder {
    pub fn open(path: &Path) -> Result<Self, String> {
        // Check before passing ABI-sensitive structures across the boundary.
        // A system library must never quietly replace the packaged runtime.
        if codec::version() >> 16 != 63
            || !codec::license().starts_with("LGPL")
            || codec::configuration().split_whitespace().any(|flag| {
                matches!(
                    flag,
                    "--enable-gpl" | "--enable-nonfree" | "--enable-version3"
                )
            })
        {
            return Err("Video requires the packaged FFmpeg 9 LGPL runtime".into());
        }
        let file = std::fs::metadata(path).map_err(error)?;
        if !file.is_file() || file.len() > 1024 * 1024 * 1024 {
            return Err("Video must be a local file no larger than 1 GiB".into());
        }
        use std::io::Read;
        let mut header = Vec::new();
        std::fs::File::open(path)
            .map_err(error)?
            .take(rvn_ui::webm::HEADER_LIMIT as u64)
            .read_to_end(&mut header)
            .map_err(error)?;
        rvn_ui::webm::validate_header(&header, false)?;
        ffmpeg::init().map_err(error)?;
        let mut options = ffmpeg::Dictionary::new();
        options.set("protocol_whitelist", "file");
        options.set("probesize", "1048576");
        options.set("analyzeduration", "5000000");
        let input = format::input_with_dictionary(path, options).map_err(error)?;
        if !input.format().name().split(',').any(|name| name == "webm") {
            return Err("Only validated WebM containers are supported".into());
        }
        let stream = input
            .streams()
            .best(media::Type::Video)
            .ok_or("Video has no video stream")?;
        if stream.parameters().id() != codec::Id::VP8 {
            return Err("Only validated WebM VP8 video is supported by this runtime".into());
        }
        let video_index = stream.index();
        let video_clock: f64 = stream.time_base().into();
        let video = codec::context::Context::from_parameters(stream.parameters())
            .map_err(error)?
            .decoder()
            .video()
            .map_err(error)?;
        let (width, height) = (video.width(), video.height());
        if width == 0 || height == 0 || width > 3840 || height > 2160 {
            return Err("Video dimensions must be between 1×1 and 3840×2160".into());
        }
        let duration = (input.duration() > 0)
            .then(|| input.duration() as f64 / f64::from(ffmpeg::ffi::AV_TIME_BASE));
        if duration.is_none() {
            return Err("A local video must declare a finite positive duration".into());
        }
        if duration.is_some_and(|duration| !duration.is_finite() || duration > 86_400.0) {
            return Err("Video duration exceeds the 24-hour limit".into());
        }
        let scaler = software::scaling::Context::get(
            video.format(),
            width,
            height,
            format::Pixel::RGBA,
            width,
            height,
            software::scaling::Flags::BILINEAR,
        )
        .map_err(error)?;
        let audio = input
            .streams()
            .best(media::Type::Audio)
            .map(|stream| {
                if stream.parameters().id() != codec::Id::VORBIS {
                    return Err(
                        "Only validated Vorbis video audio is supported by this runtime"
                            .to_string(),
                    );
                }
                let decoder = codec::context::Context::from_parameters(stream.parameters())
                    .map_err(error)?
                    .decoder()
                    .audio()
                    .map_err(error)?;
                if decoder.rate() == 0
                    || decoder.rate() > 192_000
                    || decoder.channels() == 0
                    || decoder.channels() > 8
                {
                    return Err("Unsupported video audio sample rate or channel count".into());
                }
                let resampler = software::resampling::Context::get(
                    decoder.format(),
                    decoder.channel_layout(),
                    decoder.rate(),
                    format::Sample::F32(format::sample::Type::Packed),
                    ChannelLayout::STEREO,
                    48_000,
                )
                .map_err(error)?;
                Ok(AudioTrack {
                    index: stream.index(),
                    clock: stream.time_base().into(),
                    next_seconds: None,
                    decoder,
                    resampler,
                })
            })
            .transpose()?;
        let metadata = Metadata {
            width,
            height,
            duration,
            audio: audio.is_some(),
        };
        Ok(Self {
            input,
            video,
            scaler,
            video_index,
            video_clock,
            audio,
            metadata,
            ended: false,
            seek_floor: 0.0,
        })
    }
    pub fn seek(&mut self, seconds: f64) -> Result<(), String> {
        if !seconds.is_finite()
            || seconds < 0.0
            || seconds > 86_400.0
            || self
                .metadata
                .duration
                .is_some_and(|duration| seconds > duration)
        {
            return Err("Invalid video seek position".into());
        }
        self.input
            .seek((seconds * f64::from(ffmpeg::ffi::AV_TIME_BASE)) as i64, ..)
            .map_err(error)?;
        self.video.flush();
        if let Some(audio) = &mut self.audio {
            audio.decoder.flush();
            audio.next_seconds = None;
            // Discard resampler delay too. Reusing it would replay pre-seek sound.
            let input = *audio.resampler.input();
            audio.resampler = software::resampling::Context::get(
                input.format,
                input.channel_layout,
                input.rate,
                format::Sample::F32(format::sample::Type::Packed),
                ChannelLayout::STEREO,
                48_000,
            )
            .map_err(error)?;
        }
        self.seek_floor = seconds;
        self.ended = false;
        Ok(())
    }
    pub fn next(&mut self) -> Result<Vec<Chunk>, String> {
        if self.ended {
            return Ok(Vec::new());
        }
        let mut packet = ffmpeg::Packet::empty();
        let mut result = Vec::new();
        match packet.read(&mut self.input) {
            Ok(()) => {
                if packet.size() > 64 * 1024 * 1024 {
                    return Err("Video packet exceeds the 64 MiB limit".into());
                }
                if packet.stream() == self.video_index {
                    self.video.send_packet(&packet).map_err(error)?;
                    self.frames(&mut result)?;
                } else if self
                    .audio
                    .as_ref()
                    .is_some_and(|audio| audio.index == packet.stream())
                {
                    self.audio
                        .as_mut()
                        .unwrap()
                        .decoder
                        .send_packet(&packet)
                        .map_err(error)?;
                    self.samples(&mut result)?;
                }
            }
            Err(ffmpeg::Error::Eof) => {
                self.video.send_eof().map_err(error)?;
                self.frames(&mut result)?;
                if let Some(audio) = &mut self.audio {
                    audio.decoder.send_eof().map_err(error)?;
                    self.samples(&mut result)?;
                }
                self.flush_samples(&mut result)?;
                self.ended = true;
                result.push(Chunk::End);
            }
            Err(other) => return Err(error(other)),
        }
        Ok(result)
    }
    fn frames(&mut self, output: &mut Vec<Chunk>) -> Result<(), String> {
        loop {
            let mut frame = frame::Video::empty();
            if let Err(problem) = self.video.receive_frame(&mut frame) {
                return exhausted(problem);
            }
            if frame.width() != self.metadata.width || frame.height() != self.metadata.height {
                return Err("Video resolution changed during playback".into());
            }
            let seconds = frame
                .timestamp()
                .or(frame.pts())
                .ok_or("Video frame has no timestamp")? as f64
                * self.video_clock;
            if seconds + 0.0001 < self.seek_floor {
                continue;
            }
            let mut rgba = frame::Video::empty();
            self.scaler.run(&frame, &mut rgba).map_err(error)?;
            let row_bytes = rgba.width() as usize * 4;
            let used: usize = output
                .iter()
                .map(|chunk| match chunk {
                    Chunk::Frame { rgba, .. } => rgba.len(),
                    Chunk::Audio { samples, .. } => samples.len() * 4,
                    Chunk::End => 0,
                })
                .sum();
            if used + row_bytes * rgba.height() as usize > 128 * 1024 * 1024 {
                return Err("Decoded video packet exceeds the 128 MiB memory limit".into());
            }
            let mut pixels = Vec::with_capacity(row_bytes * rgba.height() as usize);
            for row in 0..rgba.height() as usize {
                let start = row * rgba.stride(0);
                pixels.extend_from_slice(
                    rgba.data(0)
                        .get(start..start + row_bytes)
                        .ok_or("Invalid decoded video row")?,
                );
            }
            output.push(Chunk::Frame {
                seconds: seconds.max(0.0),
                width: rgba.width(),
                height: rgba.height(),
                rgba: pixels,
            });
            if output.len() > 64 {
                return Err("Video packet produced too many frames".into());
            }
        }
    }
    fn samples(&mut self, output: &mut Vec<Chunk>) -> Result<(), String> {
        let Some(audio) = &mut self.audio else {
            return Ok(());
        };
        loop {
            let mut frame = frame::Audio::empty();
            if let Err(problem) = audio.decoder.receive_frame(&mut frame) {
                return exhausted(problem);
            }
            if frame.samples() > 192_000 {
                return Err("Video audio frame exceeds its sample limit".into());
            }
            if frame.rate() != audio.decoder.rate()
                || frame.channel_layout() != audio.resampler.input().channel_layout
                || frame.format() != audio.resampler.input().format
            {
                return Err("Video audio format changed during playback".into());
            }
            let timestamp = frame
                .timestamp()
                .or(frame.pts())
                .ok_or("Video audio frame has no timestamp")? as f64
                * audio.clock;
            let seconds = *audio.next_seconds.get_or_insert(timestamp);
            let capacity = (frame.samples() as f64 * 48_000.0 / f64::from(audio.decoder.rate()))
                .ceil() as usize
                + 256;
            if capacity > 192_256 {
                return Err("Video audio resampling exceeds its sample limit".into());
            }
            let mut stereo = frame::Audio::new(
                format::Sample::F32(format::sample::Type::Packed),
                capacity,
                ChannelLayout::STEREO,
            );
            audio.resampler.run(&frame, &mut stereo).map_err(error)?;
            audio.next_seconds = Some(seconds + stereo.samples() as f64 / 48_000.0);
            Self::append_samples(&stereo, seconds, self.seek_floor, output)?;
            if output.len() > 64 {
                return Err("Video packet produced too many audio frames".into());
            }
        }
    }
    fn append_samples(
        stereo: &frame::Audio,
        seconds: f64,
        floor: f64,
        output: &mut Vec<Chunk>,
    ) -> Result<(), String> {
        let skip = ((floor.max(0.0) - seconds).max(0.0) * 48_000.0).ceil() as usize;
        let buffered: usize = output
            .iter()
            .map(|chunk| match chunk {
                Chunk::Frame { rgba, .. } => rgba.len(),
                Chunk::Audio { samples, .. } => samples.len() * 4,
                Chunk::End => 0,
            })
            .sum();
        let remaining = stereo.samples().saturating_sub(skip);
        if buffered.saturating_add(remaining.saturating_mul(8)) > 128 * 1024 * 1024 {
            return Err("Decoded video packet exceeds its memory limit".into());
        }
        let samples: Vec<f32> = stereo
            .plane::<(f32, f32)>(0)
            .iter()
            .take(stereo.samples())
            .skip(skip)
            .flat_map(|(left, right)| [*left, *right])
            .collect();
        if samples.iter().any(|value| !value.is_finite()) {
            return Err("Video audio contains a non-finite sample".into());
        }
        if !samples.is_empty() {
            output.push(Chunk::Audio {
                seconds: seconds + skip as f64 / 48_000.0,
                samples,
            });
        }
        Ok(())
    }
    fn flush_samples(&mut self, output: &mut Vec<Chunk>) -> Result<(), String> {
        let Some(audio) = &mut self.audio else {
            return Ok(());
        };
        for _ in 0..16 {
            let Some(delay) = audio.resampler.delay() else {
                return Ok(());
            };
            if delay.output > 192_000 {
                return Err("Video audio resampler delay exceeds its limit".into());
            }
            let mut stereo = frame::Audio::new(
                format::Sample::F32(format::sample::Type::Packed),
                delay.output as usize + 256,
                ChannelLayout::STEREO,
            );
            let remaining = audio.resampler.flush(&mut stereo).map_err(error)?;
            let seconds = audio.next_seconds.unwrap_or(0.0);
            Self::append_samples(&stereo, seconds, self.seek_floor, output)?;
            audio.next_seconds = Some(seconds + stereo.samples() as f64 / 48_000.0);
            if remaining.is_none() || stereo.samples() == 0 {
                return Ok(());
            }
        }
        Err("Video audio resampler did not terminate".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_libraries_are_lgpl_not_gpl_or_nonfree() {
        // Refuse accidentally linking the host distribution's GPL build.
        let configuration = ffmpeg::codec::configuration();
        assert!(
            !configuration.contains("--enable-gpl") && !configuration.contains("--enable-nonfree")
        );
        assert!(ffmpeg::codec::license().starts_with("LGPL"));
    }
    #[test]
    fn vp8_vorbis_frames_audio_and_seek_decode_from_the_same_file() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/videos/assets/clip.webm");
        let mut decoder = Decoder::open(&fixture).unwrap();
        assert_eq!(
            (decoder.metadata.width, decoder.metadata.height),
            (320, 180)
        );
        assert!(decoder.metadata.audio);
        let mut frames = 0;
        let mut samples = 0;
        let mut end = false;
        for _ in 0..1000 {
            for chunk in decoder.next().unwrap() {
                match chunk {
                    Chunk::Frame {
                        width,
                        height,
                        rgba,
                        seconds,
                    } => {
                        assert_eq!(rgba.len(), width as usize * height as usize * 4);
                        assert!(seconds.is_finite());
                        frames += 1;
                    }
                    Chunk::Audio {
                        seconds,
                        samples: audio,
                    } => {
                        assert!(seconds.is_finite());
                        assert!(audio.iter().all(|value| value.is_finite()));
                        samples += audio.len();
                    }
                    Chunk::End => end = true,
                }
            }
            if end {
                break;
            }
        }
        assert!(end && frames >= 20 && samples > 48_000);
        decoder.seek(0.8).unwrap();
        let mut sought = false;
        for _ in 0..1000 {
            for chunk in decoder.next().unwrap() {
                if let Chunk::Frame { seconds, .. } = chunk {
                    assert!(seconds >= 0.8);
                    sought = true;
                }
            }
            if sought {
                break;
            }
        }
        assert!(sought);
        assert!(decoder.seek(f64::NAN).is_err());
    }
}
