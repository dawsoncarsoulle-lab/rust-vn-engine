//! Opt-in headless verification of the exact Atlas video and shared decoder.
//! No Bevy app, window, audio device, media regeneration or gameplay mutation.
use std::{collections::BTreeSet, fs, path::{Path,PathBuf}, time::{Duration,Instant,SystemTime,UNIX_EPOCH}};
use rvn_media::{decoder::{Chunk,Decoder,Metadata},transport::{Packet,Worker}};
use sha2::{Digest,Sha256};

const LIMIT:Duration=Duration::from_secs(20);
const WIDTH:u32=1280;
const HEIGHT:u32=720;

fn hash(bytes:&[u8])->String {format!("{:x}",Sha256::digest(bytes))}
fn file_hash(path:&Path)->Result<String,String>{Ok(hash(&fs::read(path).map_err(|error|format!("{}: {error}",path.display()))?))}
fn require(condition:bool,message:&str)->Result<(),String>{if condition{Ok(())}else{Err(message.into())}}

#[derive(Default)]
struct Statistics {
    frames:usize,audio_samples:usize,end_events:usize,audio_energy:f64,audio_peak:f32,
    first_frame:Option<f64>,last_frame:Option<f64>,first_audio:Option<f64>,audio_until:Option<f64>,
    first_rgba:Option<String>,last_rgba:Option<String>,unique_rgba:BTreeSet<String>,floor:f64,
}
impl Statistics {
    fn consume(&mut self,chunk:Chunk)->Result<(),String>{
        match chunk {
            Chunk::Frame{seconds,width,height,rgba}=>{
                require(self.end_events==0,"Frame delivered after End")?;
                require((width,height)==(WIDTH,HEIGHT),"Unexpected Atlas frame resolution")?;
                require(rgba.len()==WIDTH as usize*HEIGHT as usize*4,"RGBA frame is not tightly packed")?;
                require(seconds.is_finite()&&seconds+0.0001>=self.floor,"Invalid/pre-seek video timestamp")?;
                require(self.last_frame.is_none_or(|previous|seconds+0.0001>=previous),"Video timestamps moved backwards")?;
                self.first_frame.get_or_insert(seconds);self.last_frame=Some(seconds);self.frames+=1;
                let fingerprint=hash(&rgba);self.first_rgba.get_or_insert_with(||fingerprint.clone());
                self.last_rgba=Some(fingerprint.clone());self.unique_rgba.insert(fingerprint);
            }
            Chunk::Audio{seconds,samples}=>{
                require(self.end_events==0,"Audio delivered after End")?;
                require(!samples.is_empty()&&samples.len()%2==0,"PCM chunk is not nonempty stereo")?;
                require(seconds.is_finite()&&seconds+0.0001>=self.floor,"Invalid/pre-seek audio timestamp")?;
                require(samples.iter().all(|sample|sample.is_finite()),"Non-finite audio sample")?;
                require(self.audio_until.is_none_or(|previous|seconds+0.0001>=previous),"Audio timestamps overlap/move backwards")?;
                self.first_audio.get_or_insert(seconds);self.audio_until=Some(seconds+samples.len() as f64/2.0/48_000.0);
                self.audio_samples+=samples.len();
                for sample in samples {self.audio_energy+=f64::from(sample)*f64::from(sample);self.audio_peak=self.audio_peak.max(sample.abs());}
            }
            Chunk::End=>{self.end_events+=1;require(self.end_events==1,"Duplicate terminal End")?;}
        }
        Ok(())
    }
    fn validate(&self,expected_frames:usize,seconds:f64)->Result<(),String>{
        require(self.frames==expected_frames,"Unexpected number of decoded Atlas frames")?;
        require(self.end_events==1,"Decoder did not deliver exactly one End")?;
        let audio_duration=self.audio_samples as f64/2.0/48_000.0;
        require((audio_duration-seconds).abs()<0.15,"Decoded audio duration does not match the clip")?;
        require(self.audio_energy>1e-6,"Atlas audio decoded as silence")?;
        require(self.unique_rgba.len()>2,"Atlas image frames did not change")?;
        require(self.last_frame.is_some_and(|last|(last-(self.floor+seconds)).abs()<0.1),"Last video timestamp is not close to the expected end")?;
        require(self.audio_until.is_some_and(|last|(last-(self.floor+seconds)).abs()<0.15),"Last audio timestamp is not close to the expected end")?;
        Ok(())
    }
    fn json(&self)->serde_json::Value{serde_json::json!({
        "frames":self.frames,"resolution":[WIDTH,HEIGHT],"first_frame_seconds":self.first_frame,"last_frame_seconds":self.last_frame,
        "rgba_first_sha256":self.first_rgba,"rgba_last_sha256":self.last_rgba,"distinct_rgba_frames":self.unique_rgba.len(),
        "pcm_samples":self.audio_samples,"pcm_channels":2,"pcm_rate_hz":48000,"pcm_duration_seconds":self.audio_samples as f64/96000.0,
        "audio_first_seconds":self.first_audio,"audio_end_seconds":self.audio_until,"audio_peak":self.audio_peak,
        "audio_rms":(self.audio_energy/self.audio_samples.max(1) as f64).sqrt(),"end_events":self.end_events,
        "finite_samples_and_timestamps":true,"channel_timestamps_monotonic":true
    })}
}

fn metadata_check(meta:&Metadata)->Result<(),String>{
    require((meta.width,meta.height)==(WIDTH,HEIGHT),"Unexpected video metadata resolution")?;
    require(meta.audio,"Atlas prologue is missing its Vorbis audio track")?;
    require(meta.duration.is_some_and(|duration|(duration-6.0).abs()<0.15),"Atlas duration is not approximately six seconds")
}
fn drain(decoder:&mut Decoder,floor:f64)->Result<Statistics,String>{
    let deadline=Instant::now()+LIMIT;
    let mut statistics=Statistics{floor,..Default::default()};
    for _ in 0..50_000 {
        require(Instant::now()<deadline,"Direct decode exceeded its twenty-second QA budget")?;
        for chunk in decoder.next()?{statistics.consume(chunk)?;}
        if statistics.end_events!=0 {
            require(decoder.next()?.is_empty()&&decoder.next()?.is_empty(),"Decoder emitted data again after End")?;
            return Ok(statistics);
        }
    }
    Err("Direct decode exceeded its packet budget without End".into())
}
fn worker_decode(path:&Path)->Result<Statistics,String>{
    let worker=Worker::spawn(path.into(),0.0)?;
    let deadline=Instant::now()+LIMIT;let mut ready=false;
    let mut statistics=Statistics::default();
    while statistics.end_events==0 {
        require(Instant::now()<deadline,"Worker exceeded its twenty-second QA budget")?;
        match worker.poll()? {
            Some(Packet::Ready(meta))=>{require(!ready,"Duplicate worker metadata")?;metadata_check(&meta)?;ready=true;}
            Some(Packet::Data(chunk))=>{require(ready,"Worker data preceded metadata")?;statistics.consume(chunk)?;}
            Some(Packet::Error(error))=>return Err(error),
            None=>std::thread::sleep(Duration::from_millis(1)),
        }
    }
    require(ready,"Worker did not report metadata")?;
    Ok(statistics)
}

fn verify(path:&Path)->Result<serde_json::Value,String>{
    let path=fs::canonicalize(path).map_err(|error|error.to_string())?;
    let root=Path::new(env!("CARGO_MANIFEST_DIR")).parent().ok_or("Missing Atlas project root")?;
    let media_hash=file_hash(&path)?;
    let canonical=root.join("assets/movies/prologue.webm");
    let web=root.join("dist-web/L_Atlas_des_Brumes/assets/movies/prologue.webm");
    let native=root.join("dist/L_Atlas_des_Brumes-linux-x64/data/assets/movies/prologue.webm");
    let mut matching_copies=Vec::new();
    for candidate in [canonical,web,native] {
        require(file_hash(&candidate)?==media_hash,"Canonical/native/Web copies of Atlas prologue differ")?;
        matching_copies.push(candidate);
    }
    let engine=root.parent().and_then(Path::parent).ok_or("Missing engine directory")?;
    let prefix=PathBuf::from(std::env::var_os("FFMPEG_DIR").ok_or("FFMPEG_DIR must name the audited packaged runtime")?);
    let mut libraries=serde_json::Map::new();
    for name in ["libavcodec.so.63","libavformat.so.63","libavutil.so.61","libswscale.so.10","libswresample.so.7"] {
        let loaded=prefix.join("lib").join(name);
        let packaged=root.join("dist/L_Atlas_des_Brumes-linux-x64/lib").join(name);
        let fingerprint=file_hash(&loaded)?;
        require(file_hash(&packaged)?==fingerprint,"Audit prefix and delivered native video library differ")?;
        libraries.insert(name.into(),serde_json::json!({"runtime_path":loaded,"sha256":fingerprint,"native_package_identical":true}));
    }
    let mut decoder=Decoder::open(&path)?;metadata_check(&decoder.metadata)?;
    let metadata=serde_json::json!({"width":decoder.metadata.width,"height":decoder.metadata.height,"duration_seconds":decoder.metadata.duration,"has_audio":decoder.metadata.audio});
    let full=drain(&mut decoder,0.0)?;full.validate(144,6.0)?;
    decoder.seek(3.0)?;
    let seek=drain(&mut decoder,3.0)?;seek.validate(72,3.0)?;
    require(decoder.seek(f64::NAN).is_err(),"Non-finite seek was accepted")?;
    let worker=worker_decode(&path)?;worker.validate(144,6.0)?;
    require(full.frames==worker.frames&&full.audio_samples==worker.audio_samples&&full.first_rgba==worker.first_rgba&&full.last_rgba==worker.last_rgba,"Worker decode differs from direct decode")?;
    // Receiver drop must not join a producer blocked by the bounded queue.
    // This checks cancellation return, not a claim that an audio device played.
    let cancelled=Worker::spawn(path.clone(),0.0)?;
    std::thread::sleep(Duration::from_millis(20));let dropping=Instant::now();drop(cancelled);
    require(dropping.elapsed()<Duration::from_millis(100),"Cancelling the decoder blocked the consumer")?;
    Ok(serde_json::json!({
        "status":"passed","platform":std::env::consts::OS,"method":"Native rvn_media Decoder + bounded Worker, headless, actual delivered VP8/Vorbis WebM",
        "media":{"path":path,"bytes":fs::metadata(&path).map_err(|error|error.to_string())?.len(),"sha256":media_hash,"matching_canonical_native_web_copies":matching_copies},
        "source":{"main_sha256":file_hash(&root.join("main.rvn"))?,"decoder_sha256":file_hash(&engine.join("rvn_media/src/decoder.rs"))?,"transport_sha256":file_hash(&engine.join("rvn_media/src/transport.rs"))?,"harness_sha256":file_hash(&root.join("qa/native-video.rs"))?},
        "codec":{"container":"WebM","video":"VP8","audio":"Vorbis","codec_abi_major":63,"runtime_policy":"FFmpeg 9 shared LGPL; GPL/nonfree/version3 configurations rejected by Decoder::open","libraries":libraries},
        "metadata":metadata,"direct":full.json(),"seek_three_seconds":seek.json(),"bounded_worker":worker.json(),
        "after_end_empty":true,"non_finite_seek_rejected":true,"receiver_cancellation_nonblocking":true,
        "limits":["No window opened or user window touched","No Bevy GPU/pixel presentation proof","No audio-device or acoustic playback proof","No subtitle/narrative synchronization proof","No Windows runtime proof"]
    }))
}

fn main(){
    let arguments:Vec<_>=std::env::args().skip(1).collect();
    if arguments.len()!=2{eprintln!("Usage: atlas-native-video DELIVERED_PROLOGUE_WEBM NEW_OUTPUT_JSON");std::process::exit(2);}
    let output=Path::new(&arguments[1]);
    if output.exists(){eprintln!("Refusing to replace an existing QA report");std::process::exit(2);}
    let start=Instant::now();
    let started=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64();
    let result=verify(Path::new(&arguments[0]));
    let passed=result.is_ok();
    let mut report=result.unwrap_or_else(|error|serde_json::json!({"status":"failed","error":error,"media":arguments[0]}));
    report["started_unix_seconds"]=started.into();report["elapsed_seconds"]=start.elapsed().as_secs_f64().into();
    let written=(||->Result<(),String>{
        fs::create_dir_all(output.parent().ok_or("Missing report directory")?).map_err(|error|error.to_string())?;
        fs::write(output,serde_json::to_vec_pretty(&report).map_err(|error|error.to_string())?).map_err(|error|error.to_string())
    })();
    if let Err(error)=written{eprintln!("Could not retain QA report: {error}");std::process::exit(1);}
    println!("{}",report);
    if !passed{std::process::exit(1);}
}
