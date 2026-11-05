use super::*;
use crate::checkpoint::*;
struct RealDecode{asset:[u8;32],profile:DecodedSourceProfile,pcm:Vec<f32>,frames:Vec<PictureCursor>}
fn actual()->RealDecode{
    let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/clip.webm");
    let worker=Worker::spawn_proven(path,None).unwrap();
    let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
    let(mut source,mut pcm,mut frames)=(None,Vec::new(),Vec::new());
    loop{assert!(std::time::Instant::now()<deadline);match worker.poll_proven().unwrap(){
        Some((Packet::Ready(_),Some(DecodedProof::Ready{asset_sha256,profile})))=>{profile.validate().unwrap();source=Some((asset_sha256,profile));},
        Some((Packet::Data(Chunk::Audio{samples,..}),Some(DecodedProof::Audio{first_sample})))=>{assert_eq!(first_sample,pcm.len()as u64);pcm.extend(samples);},
        Some((Packet::Data(Chunk::Frame{rgba,..}),Some(DecodedProof::Video{ordinal,pts,time_base})))=>{assert_eq!(ordinal,frames.len()as u64);frames.push(PictureCursor{ordinal,pts,time_base,rgba_sha256:<sha2::Sha256 as sha2::Digest>::digest(rgba).into()});},
        Some((Packet::Data(Chunk::End),_))=>break,
        Some((Packet::Error(e),_))=>panic!("{e}"),Some(_)=>panic!("Missing actual provenance"),None=>std::thread::yield_now(),
    }}let(asset,profile)=source.unwrap();RealDecode{asset,profile,pcm,frames}
}
#[test]
fn two_cold_decoders_prove_same_real_sample_bits_and_pts_ordinals(){
 let first=actual();let second=actual();assert_eq!(first.asset,second.asset);assert_eq!(first.profile,second.profile);assert!(first.pcm.len()>48_000);assert!(first.frames.len()>20);assert_eq!(first.frames,second.frames);
 assert_eq!(first.pcm.iter().map(|x|x.to_bits()).collect::<Vec<_>>(),second.pcm.iter().map(|x|x.to_bits()).collect::<Vec<_>>());
 // Odd offset means the next real right sample. No timeline division occurs.
 let offset=10_003;let count=BOUNDARY_SAMPLES;let boundary=pcm_boundary(offset as u64,first.pcm[offset..offset+count].iter().copied());
 assert_eq!(boundary,pcm_boundary(offset as u64,second.pcm[offset..offset+count].iter().copied()));
 assert_ne!(boundary,pcm_boundary(offset as u64+1,second.pcm[offset+1..offset+1+count].iter().copied()));
}
#[test]
fn actual_output_cursor_rejects_channel_ordinal_and_profile_corruption(){
 let d=actual();let mut cursor=AudioCursor{next_sample:5,next_channel:1,boundary_samples:64,boundary_sha256:pcm_boundary(5,d.pcm[5..69].iter().copied()),ended:false};cursor.validate().unwrap();cursor.next_channel=0;assert!(cursor.validate().is_err());cursor.next_channel=1;cursor.next_sample=MAX_SAMPLES+1;assert!(cursor.validate().is_err());
 let mut profile=d.profile;profile.audio_time_base=Some((1,0));assert!(profile.validate().is_err());
}
#[test]
fn canonical_decode_uses_all_real_origin_samples_and_never_a_seek_floor(){
 let d=actual();let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/clip.webm");let mut decoder=Decoder::open_canonical(&path).unwrap();let mut pcm=Vec::new();loop{let mut end=false;for chunk in decoder.next().unwrap(){match chunk{Chunk::Audio{samples,..}=>pcm.extend(samples),Chunk::End=>end=true,_=>{}}}if end{break;}}
 assert_eq!(d.pcm.iter().map(|x|x.to_bits()).collect::<Vec<_>>(),pcm.iter().map(|x|x.to_bits()).collect::<Vec<_>>());
}
