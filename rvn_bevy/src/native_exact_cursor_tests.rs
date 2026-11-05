use super::*;
use rvn_media::{decoder::Chunk,transport::{Worker,Packet}};
pub(super) fn decoded_pcm(f:&Fixture)->Vec<f32>{
 let worker=Worker::spawn_proven(f.root.join("assets/clip.webm"),None).unwrap();let deadline=Instant::now()+Duration::from_secs(10);let mut pcm=Vec::new();loop{assert!(Instant::now()<deadline);match worker.poll().unwrap(){Some(Packet::Data(Chunk::Audio{samples,..}))=>pcm.extend(samples),Some(Packet::Data(Chunk::End))=>break,Some(Packet::Error(e))=>panic!("{e}"),_=>std::thread::yield_now()}}pcm
}
pub(super) const OFFSET:usize=10_003;
pub(super) fn advance_actual(f:&mut Fixture,audio:&crate::video_audio::VideoAudio,source:&mut crate::video_audio::Samples,pcm:&[f32]){
 let deadline=Instant::now()+Duration::from_secs(10);let mut nonzero=false;
 loop{let before=audio.0.lock().unwrap().delivered_sample_offset().unwrap()as usize;if before==OFFSET{break;}assert!(Instant::now()<deadline);let actual=source.next().expect("Actual native reader must not EOF during pre-roll delivery");let after=audio.0.lock().unwrap().delivered_sample_offset().unwrap()as usize;if after==before+1{assert_eq!(actual.to_bits(),pcm[before].to_bits());nonzero|=actual!=0.0;}else{assert_eq!(after,before);assert_eq!(actual,0.0);std::thread::sleep(Duration::from_millis(1));}if after%128==0||after==before{crate::native_video::pump_draw_report_current_for_test(f.app.world_mut()).unwrap();}}
 crate::native_video::pump_draw_report_current_for_test(f.app.world_mut()).unwrap();assert!(nonzero,"Must compare actual nonzero decoded PCM, not only padding");
}
fn fenced(f:&mut Fixture,p:&rvn_parser::ParsedProgram)->rvn_core::save::consumer_checkpoint::Save15{
 crate::native_video::draw_current_for_test(f.app.world_mut());
 crate::native_video::capture_save15(f.app.world(),p,None,1,"real native consumed cursor".into()).unwrap()
}
#[test]
fn shared_right_sample_and_gate_silence_are_captured_at_the_real_single_sample_offset(){
 let(mut f,p,_,_)=media_fixture(true,false);let pcm=decoded_pcm(&f);let audio=crate::native_video::start_current_for_test(f.app.world_mut()).unwrap();audio.0.lock().unwrap().running=true;let mut source=audio.decoder();
 advance_actual(&mut f,&audio,&mut source,&pcm);
 let gate=f.app.world().resource::<VnEngine>().0.renderer.audio_checkpoints.gate();gate.publish(&[],|_|{assert_eq!(source.next(),Some(0.0));assert_eq!(source.next(),Some(0.0));Ok(())}).unwrap();
 let save=fenced(&mut f,&p);let cursor=save.native_media_checkpoint.as_ref().unwrap()[0].audio.as_ref().unwrap();assert_eq!((cursor.next_sample,cursor.next_channel),(OFFSET as u64,1));assert_eq!(cursor.boundary_sha256,rvn_media::checkpoint::pcm_boundary(OFFSET as u64,pcm[OFFSET..OFFSET+64].iter().copied()));
 assert_eq!(source.next().unwrap().to_bits(),pcm[OFFSET].to_bits());
}
#[test]
fn a_second_actual_decoder_claim_refuses_capture_instead_of_fabricating_a_cursor(){
 let(mut f,p,_,_)=media_fixture(true,false);let pcm=decoded_pcm(&f);let audio=crate::native_video::start_current_for_test(f.app.world_mut()).unwrap();audio.0.lock().unwrap().running=true;let mut first=audio.decoder();advance_actual(&mut f,&audio,&mut first,&pcm);let mut duplicate=audio.decoder();assert_eq!(duplicate.next(),None);let before=atomic_world_before(&f);assert!(crate::native_video::capture_save15(f.app.world(),&p,None,1,"duplicate".into()).unwrap_err().contains("one actual reader"));assert_eq!(atomic_world_before(&f),before);drop(duplicate);assert!(crate::native_video::capture_save15(f.app.world(),&p,None,1,"single".into()).is_ok());
}
#[test]
fn cold_constructor_and_cold_native_preroll_restore_saved_right_bits_and_french_without_replay(){
 let(mut f,p,_,_)=media_fixture(true,false);let pcm=decoded_pcm(&f);f.app.world_mut().resource_mut::<VnEngine>().0.locale.as_mut().unwrap().set_language("fr").unwrap();f.app.world_mut().resource_mut::<crate::systems::settings_menu::Settings>().language="fr".into();
 let audio=crate::native_video::start_current_for_test(f.app.world_mut()).unwrap();audio.0.lock().unwrap().running=true;let mut source=audio.decoder();advance_actual(&mut f,&audio,&mut source,&pcm);let save=fenced(&mut f,&p);let vars=f.app.world().resource::<VnEngine>().0.state.vars.clone();let rng=f.app.world().resource::<VnEngine>().0.state.random;
 let encoded=save.encode().unwrap();let save=rvn_core::save::consumer_checkpoint::Save15::decode(&encoded).unwrap();assert_eq!(save.native_media_checkpoint.as_ref().unwrap()[0].audio.as_ref().unwrap().next_sample,OFFSET as u64);
 drop(source);drop(audio);crate::native_video::clear_current_for_cold_test(f.app.world_mut());
 let old=&f.app.world().resource::<VnEngine>().0;let mut english=old.locale.as_ref().unwrap().clone();english.set_language("en").unwrap();let cold=Engine::from_program_with_persistent_and_text_context(p.clone(),BevyRenderer::new(),8,HashMap::new(),rvn_core::TextRuntimeContext::new(old.text_resources().clone(),Some(english))).unwrap();assert!(!cold.state.vars.contains_key("roll"));f.app.world_mut().insert_resource(VnEngine(cold));f.app.world_mut().resource_mut::<crate::systems::settings_menu::Settings>().language="en".into();
 let mut expected=portable_receipt(f.app.world()).unwrap();expected.locales=save.locale.as_ref().unwrap().clone();let plan=ready_plan(&mut f,&p,&save,&expected);crate::atomic_text_restore::commit(f.app.world_mut(),plan).unwrap();let game=&f.app.world().resource::<VnEngine>().0;assert_eq!(game.state.vars,vars);assert_eq!(game.state.random,rng);assert_eq!(game.locale.as_ref().unwrap().current_lang(),"fr");assert!(game.renderer.native_admission.is_none());
 let restored=crate::native_video::restored_sources_for_test(f.app.world());let audio=restored[0].1.as_ref().unwrap();audio.0.lock().unwrap().running=true;let mut source=audio.decoder();assert_eq!(source.next(),Some(0.0),"Physical stereo output pads left before the saved right");for expected in &pcm[OFFSET..OFFSET+64]{assert_eq!(source.next().unwrap().to_bits(),expected.to_bits());}
}
fn rejected(mut f:Fixture,p:rvn_parser::ParsedProgram,save:rvn_core::save::consumer_checkpoint::Save15,expected:PortableTextHostReceipt){
 let before=atomic_world_before(&f);let entities=f.app.world().entities().len();let deadline=Instant::now()+Duration::from_secs(20);loop{match crate::atomic_text_restore::prepare(f.app.world(),&p,save.clone(),&expected){Ok(_)=>panic!("Forged cursor was admitted"),Err(e)if e.contains("still staging")||e.contains("still loading")=>{f.app.update();},Err(e)=>{assert!(!e.is_empty());break}}assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(2));}assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);
}
#[test]
fn changed_channel_source_stream_asset_and_ordinal_refuse_before_any_world_mutation(){
 for corrupt in 0..6{let(mut f,p,_,_)=media_fixture(true,false);let pcm=decoded_pcm(&f);let audio=crate::native_video::start_current_for_test(f.app.world_mut()).unwrap();audio.0.lock().unwrap().running=true;let mut source=audio.decoder();advance_actual(&mut f,&audio,&mut source,&pcm);let mut save=fenced(&mut f,&p);let row=&mut save.native_media_checkpoint.as_mut().unwrap()[0];match corrupt{0=>row.audio.as_mut().unwrap().next_channel=0,1=>row.source_locator="other.webm".into(),2=>row.profile.audio_stream=Some(99),3=>row.asset_sha256[0]^=1,4=>{row.audio.as_mut().unwrap().next_sample+=2;},_=>row.picture.ordinal+=1};let mut expected=portable_receipt(f.app.world()).unwrap();expected.locales=save.locale.as_ref().unwrap().clone();rejected(f,p,save,expected);}
}
#[test]
fn a_neutral_cursor_cannot_authorize_a_direct_core_commit_without_world_staging(){
 let(mut f,p,_,_)=media_fixture(true,false);let pcm=decoded_pcm(&f);let audio=crate::native_video::start_current_for_test(f.app.world_mut()).unwrap();audio.0.lock().unwrap().running=true;let mut source=audio.decoder();advance_actual(&mut f,&audio,&mut source,&pcm);let save=fenced(&mut f,&p);let game=&f.app.world().resource::<VnEngine>().0;let core=game.prepare_save15_with_locale(&p,save,game.locale.clone()).unwrap();let before=atomic_world_before(&f);let message=match f.app.world_mut().resource_mut::<VnEngine>().0.commit_prepared_save15(core){Err(e)=>e.to_string(),Ok(_)=>panic!("DTO alone authorized native publication")};assert!(message.contains("staged World admission"));assert_eq!(atomic_world_before(&f),before);
}
