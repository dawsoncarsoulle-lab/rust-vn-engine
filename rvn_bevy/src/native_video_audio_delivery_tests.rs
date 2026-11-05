//! Real FFmpeg WebM->PCM and real Bevy Decodable/Source delivery only.
//! This narrow CPU contract does not fabricate or qualify a World/Core owner.
use super::*;
use rvn_media::{transport::{Worker,Packet,DecodedProof},decoder::Chunk};
use std::{path::PathBuf,time::{Duration,Instant}};
struct AudioPacket{seconds:f64,samples:Vec<f32>,first:Option<u64>}
struct Decoded{packets:Vec<AudioPacket>,profile:Option<([u8;32],rvn_media::checkpoint::DecodedSourceProfile)>}
fn decode(exact:bool)->Decoded{
 let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/clip.webm");
 let worker=if exact{Worker::spawn_proven(path,None)}else{Worker::spawn_with_keepalive(path,0.125,None)}.unwrap();
 let end=Instant::now()+Duration::from_secs(10);let mut profile=None;let mut packets=Vec::new();let mut total=0usize;let mut nonzero=false;
 loop{assert!(Instant::now()<end,"Actual decoder EOF timeout");match worker.poll_proven().unwrap(){
  Some((Packet::Ready(metadata),proof))=>{assert!(metadata.audio);assert!(metadata.duration.is_some());match proof{Some(DecodedProof::Ready{asset_sha256,profile:p})=>profile=Some((asset_sha256,p)),None=>assert!(!exact),_=>panic!("Wrong Ready proof")}},
  Some((Packet::Data(Chunk::Audio{seconds,samples}),proof))=>{assert_eq!(samples.len()%2,0);total=total.checked_add(samples.len()).unwrap();assert!(total<=1_000_000);nonzero|=samples.iter().any(|sample|*sample!=0.0);let first=match proof{Some(DecodedProof::Audio{first_sample})=>Some(first_sample),None=>{assert!(!exact);None},_=>panic!("Wrong Audio proof")};if !samples.is_empty(){packets.push(AudioPacket{seconds,samples,first});}},
  Some((Packet::Data(Chunk::End),_))=>break,
  Some((Packet::Error(error),_))=>panic!("{error}"),_=>std::thread::yield_now(),
 }}
 assert!(!packets.is_empty()&&nonzero);assert_eq!(profile.is_some(),exact);Decoded{packets,profile}
}
fn final_right(exact:bool)->(VideoAudio,Samples,crate::checkpoint_pcm::CheckpointGate,f32){
 let decoded=decode(exact);let gate=crate::checkpoint_pcm::CheckpointGate::default();let mut buffer=Buffer::new(if exact{0.0}else{decoded.packets[0].seconds});buffer.gate=Some(gate.clone());buffer.running=true;
 if let Some((asset,profile))=decoded.profile{buffer.bind_exact(asset,profile).unwrap();}
 let audio=VideoAudio(Arc::new(Mutex::new(buffer)));let mut source=audio.decoder();let expected=*decoded.packets.last().unwrap().samples.last().unwrap();let count=decoded.packets.len();let end=Instant::now()+Duration::from_secs(10);
 for(index,packet)in decoded.packets.into_iter().enumerate(){let last=index+1==count;
  {let mut b=audio.0.lock().unwrap();if exact{b.push_exact(packet.first.unwrap(),packet.samples).unwrap();}else{b.push(packet.seconds,packet.samples).unwrap();}if last{b.ended=true;}}
  loop{let done={let b=audio.0.lock().unwrap();if last{if exact{b.len()==1}else{b.len()==0&&b.pending_right.count.load(Ordering::Acquire)==1}}else{b.len()==0&&b.pending_right.count.load(Ordering::Acquire)==0}};if done{break;}assert!(Instant::now()<end);assert!(source.next().is_some());}
 }
 {let b=audio.0.lock().unwrap();assert!(b.ended);assert!(!b.playout_drained());assert!(!b.playout_abandoned());if exact{assert_eq!(b.len(),1);assert_eq!(b.pending_right.count.load(Ordering::Acquire),0);}else{assert_eq!(b.len(),0);assert_eq!(b.pending_right.count.load(Ordering::Acquire),1);}}
 (audio,source,gate,expected)
}
#[test]
fn real_legacy_eof_waits_for_local_right_delivery_through_fence_and_pause(){
 let(audio,mut source,gate,expected)=final_right(false);
 gate.publish(&[],|_|{assert_eq!(source.next(),Some(0.0));assert!(!audio.0.lock().unwrap().playout_drained());Ok(())}).unwrap();
 gate.set_preview_paused(true).unwrap();for _ in 0..4{assert_eq!(source.next(),Some(0.0));}assert!(!audio.0.lock().unwrap().playout_drained());gate.set_preview_paused(false).unwrap();
 let mut delivered=false;for _ in 0..2{let value=source.next().unwrap();if audio.0.lock().unwrap().playout_drained(){assert_eq!(value.to_bits(),expected.to_bits());delivered=true;break;}}assert!(delivered);assert_eq!(source.next(),None);
}
#[test]
fn dropping_real_legacy_source_with_local_right_marks_abandoned_and_never_false_eof(){
 let(audio,source,_gate,_expected)=final_right(false);drop(source);let b=audio.0.lock().unwrap();assert_eq!(b.pending_right.count.load(Ordering::Acquire),0);assert!(b.playout_abandoned());assert!(!b.playout_drained());assert_eq!(b.len(),0);
}
#[test]
fn dropping_exact_source_preserves_shared_right_for_true_reader_reclaim_and_eof(){
 let(audio,source,_gate,expected)=final_right(true);let offset=audio.0.lock().unwrap().delivered_sample_offset().unwrap();assert_eq!(offset%2,1);drop(source);{let b=audio.0.lock().unwrap();assert!(!b.playout_drained()&&!b.playout_abandoned());assert_eq!(b.len(),1);}
 let mut reclaimed=audio.decoder();for _ in 0..2{let value=reclaimed.next().unwrap();if audio.0.lock().unwrap().delivered_sample_offset()==Some(offset+1){assert_eq!(value.to_bits(),expected.to_bits());break;}}assert_eq!(audio.0.lock().unwrap().delivered_sample_offset(),Some(offset+1));assert!(audio.0.lock().unwrap().playout_drained());assert_eq!(reclaimed.next(),None);
}

#[test]
fn actual_world_pause_sources_closes_exact_and_cached_right_until_owner_tick_reopens(){
 for exact in [false,true]{
  let(audio,mut source,gate,expected)=final_right(exact);
  let mut world=World::new();world.init_resource::<Assets<VideoAudio>>();
  let handle=world.resource_mut::<Assets<VideoAudio>>().add(audio.clone());
  let before={let b=audio.0.lock().unwrap();(b.position.to_bits(),b.len(),b.delivered_sample_offset(),b.pending_right.count.load(Ordering::Acquire))};
  // Same existing helper the World can call even when the Native tick itself
  // is gated off. It closes only media Sources and opens no global gate.
  gate.publish(&[],|_|{super::pause_sources(&world)?;Ok(())}).unwrap();
  for _ in 0..7{assert_eq!(source.next(),Some(0.0),"Suspension cannot be Source EOF or leak the pending right");}
  {let b=audio.0.lock().unwrap();assert!(!b.running&&!b.revoked&&!b.playout_drained());assert_eq!((b.position.to_bits(),b.len(),b.delivered_sample_offset(),b.pending_right.count.load(Ordering::Acquire)),before);}
  assert!(world.resource::<Assets<VideoAudio>>().contains(handle.id()));
  // A real owner-aware tick is the only production caller that reopens the
  // buffer. This CPU Source test exercises that existing field transition;
  // it does not manufacture a Startup clock receipt or qualify that caller.
  audio.0.lock().unwrap().running=true;
  let mut delivered=false;
  for _ in 0..2{let value=source.next().unwrap();if audio.0.lock().unwrap().playout_drained(){assert_eq!(value.to_bits(),expected.to_bits());delivered=true;break;}}
  assert!(delivered);assert_eq!(source.next(),None);
 }
}
