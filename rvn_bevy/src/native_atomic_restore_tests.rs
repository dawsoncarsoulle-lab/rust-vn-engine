//! Real packaged WebM/VP8/Vorbis decoding + CPU Assets/ECS contracts.
//! No device, GPU upload, or visible/audible acceptance is inferred.
use super::*;
use bevy::audio::Decodable;
const CLIP:&[u8]=include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../fixtures/clip.webm"));
fn media_fixture(current:bool,mask:bool)->(Fixture,rvn_parser::ParsedProgram,rvn_core::save::consumer_checkpoint::Save15,PortableTextHostReceipt){media_fixture_options(current,mask,None)}
fn media_fixture_options(current:bool,mask:bool,terminal:Option<bool>)->(Fixture,rvn_parser::ParsedProgram,rvn_core::save::consumer_checkpoint::Save15,PortableTextHostReceipt){
    let(mut f,_,_,_)=atomic_save_fixture();fs::write(f.root.join("assets/clip.webm"),CLIP).unwrap();if mask{fs::write(f.root.join("assets/mask.webm"),CLIP).unwrap();}
    crate::native_video::init_restore_runtime_for_test(&mut f.app);
    f.app.insert_resource(crate::project_paths::ProjectPaths::new(f.root.clone(),f.root.join("assets"),f.root.join("locales"),f.root.join("theme.toml"),f.root.join("saves")));
    let mut props=serde_json::Map::new();if mask{props.insert("mask".into(),serde_json::json!("mask.webm"));}if let Some(keep)=terminal{props.insert("keep_last_frame".into(),serde_json::json!(keep));props.insert("on_end".into(),serde_json::json!("done"));}let options=serde_json::Value::Object(props).to_string();
    let prefix=if terminal.is_some(){"init {set ends=0}\nhandler done(event){set ends=ends+1}\n"}else{""};
    let text=format!("{prefix}label start\nflow.consume_v2(\"legacy\",\"rng\"){{set roll=random(1,9)}}\nflow.consume_v2(\"legacy\",\"movie\"){{video.play(\"intro\",video_clip(\"clip.webm\",{options}))}}\n\"First\"\n\"after\"\n");
    let path=f.root.join("movie.rvn");fs::write(&path,&text).unwrap();let mut p=rvn_parser::parse_program(&fs::read_to_string(&path).unwrap()).unwrap();p.sources[0].path=Some(path.clone());for origin in &mut p.origins{origin.source=Some(path.clone());}
    let existing=&f.app.world().resource::<VnEngine>().0;let resources=existing.text_resources().clone();let locale=existing.locale.as_ref().unwrap().clone();let mut french=locale.clone();french.set_language("fr").unwrap();
    let mut saved=Engine::from_program_with_persistent_and_text_context(p.clone(),BevyRenderer::new(),8,HashMap::new(),rvn_core::TextRuntimeContext::new(resources.clone(),Some(french))).unwrap();saved.step_silent().unwrap();assert!(saved.state.videos.tracks.contains_key("intro"));
    let save=saved.capture_save15(&p,None,1,"actual cold native movie".into()).unwrap();let save=rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap();
    let mut live=Engine::from_program_with_persistent_and_text_context(p.clone(),BevyRenderer::new(),8,HashMap::new(),rvn_core::TextRuntimeContext::new(resources,Some(locale))).unwrap();if current{live.step_silent().unwrap();}
    f.app.world_mut().insert_resource(VnEngine(live));if current{crate::native_video::consume_current_commands_for_test(f.app.world_mut());}let mut expected=portable_receipt(f.app.world()).unwrap();expected.locales=save.locale.as_ref().unwrap().clone();(f,p,save,expected)
}
fn ready_plan(f:&mut Fixture,p:&rvn_parser::ParsedProgram,save:&rvn_core::save::consumer_checkpoint::Save15,expected:&PortableTextHostReceipt)->crate::atomic_text_restore::PreparedTextRestore{
    let before=atomic_world_before(f);let entities=f.app.world().entities().len();let deadline=Instant::now()+Duration::from_secs(20);
    loop{match crate::atomic_text_restore::prepare(f.app.world(),p,save.clone(),expected){Ok(plan)=>{assert_eq!(atomic_world_before(f),before);assert_eq!(f.app.world().entities().len(),entities);return plan;},Err(message)if message.contains("still staging")||message.contains("still loading")=>{},Err(message)=>panic!("Real media preparation refused: {message}")};assert!(Instant::now()<deadline,"Real native readiness timed out");f.app.update();std::thread::sleep(Duration::from_millis(2));}
}
#[test]
fn decoded_candidate_commits_french_rng_once_and_installs_real_image_pcm_before_return(){
    let(mut f,p,save,expected)=media_fixture(false,false);let actual_saved=f.app.world().resource::<VnEngine>().0.save15_state_for_audio_warming(&p,&save).unwrap();let vars=actual_saved.vars;let random=actual_saved.random;let before=atomic_world_before(&f);let plan=ready_plan(&mut f,&p,&save,&expected);
    crate::atomic_text_restore::commit(f.app.world_mut(),plan).unwrap();let game=&f.app.world().resource::<VnEngine>().0;assert_eq!(game.state.vars,vars);assert_eq!(game.state.random,random);assert_eq!(game.runtime_epoch(),before.3+1);assert_eq!(game.locale.as_ref().unwrap().current_lang(),"fr");assert!(!game.renderer.scene_batch_pending());
    let rows=crate::native_video::restored_sources_for_test(f.app.world());assert_eq!(rows.len(),1);assert_eq!((rows[0].2,rows[0].3),(320,180));assert_eq!(rows[0].0,game.video_views().unwrap()[0]);let audio=rows[0].1.as_ref().unwrap();let mut buffer=audio.0.lock().unwrap();assert!(buffer.len()>0);assert!(!buffer.running);assert!(buffer.verifies_owner(game.instance_id(),&game.renderer.audio_checkpoints.gate()));buffer.running=true;drop(buffer);let mut source=audio.decoder();assert!(source.next().is_some());assert!(source.next().is_some());
}
#[test]
fn decoded_mask_is_ready_before_atomic_installation(){let(mut f,p,save,expected)=media_fixture(false,true);let plan=ready_plan(&mut f,&p,&save,&expected);crate::atomic_text_restore::commit(f.app.world_mut(),plan).unwrap();assert_eq!(crate::native_video::restored_sources_for_test(f.app.world()).len(),1);}
#[test]
fn ordinary_owner_binds_real_buffers_and_retired_cached_right_cannot_escape_commit(){
    let(mut f,p,save,expected)=media_fixture(true,false);let old=crate::native_video::start_current_for_test(f.app.world_mut()).unwrap();old.0.lock().unwrap().running=true;let mut source=old.decoder();assert!(source.next().is_some());let position=old.0.lock().unwrap().position;
    let plan=ready_plan(&mut f,&p,&save,&expected);crate::atomic_text_restore::commit(f.app.world_mut(),plan).unwrap();assert_eq!(source.next(),None,"Cached old right must be revoked under the same publication gate");assert_eq!(old.0.lock().unwrap().position,position);assert!(old.0.lock().unwrap().revoked);
}
#[test]
fn actual_shared_gate_and_preview_pause_preserve_cached_right_without_source_eof(){
    let(mut f,_,_,_)=media_fixture(true,false);let audio=crate::native_video::start_current_for_test(f.app.world_mut()).unwrap();audio.0.lock().unwrap().running=true;let game=&f.app.world().resource::<VnEngine>().0;let gate=game.renderer.audio_checkpoints.gate();let mut source=audio.decoder();let _left=source.next().unwrap();let before=audio.0.lock().unwrap().position;
    gate.publish(&[],|_|{assert_eq!(source.next(),Some(0.0));assert_eq!(source.next(),Some(0.0));assert_eq!(audio.0.lock().unwrap().position,before);Ok(())}).unwrap();
    gate.set_preview_paused(true).unwrap();assert_eq!(source.next(),Some(0.0));assert_eq!(source.next(),Some(0.0));assert_eq!(audio.0.lock().unwrap().position,before);gate.set_preview_paused(false).unwrap();assert!(source.next().is_some());assert_eq!(audio.0.lock().unwrap().position,before);
}
#[test]
fn changed_real_file_refuses_before_core_settings_or_entities_and_keeps_old_pcm(){
    let(mut f,p,save,expected)=media_fixture(true,false);let old=crate::native_video::start_current_for_test(f.app.world_mut()).unwrap();old.0.lock().unwrap().running=true;let mut source=old.decoder();assert!(source.next().is_some());let plan=ready_plan(&mut f,&p,&save,&expected);let before=atomic_world_before(&f);let entities=f.app.world().entities().len();let mut bytes=CLIP.to_vec();let last=bytes.len()-1;bytes[last]^=1;fs::write(f.root.join("assets/clip.webm"),bytes).unwrap();assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err());assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);assert!(!old.0.lock().unwrap().revoked);assert!(source.next().is_some());
}
#[test]
fn changed_asset_runtime_refuses_before_core_publication(){let(mut f,p,save,expected)=media_fixture(false,false);let plan=ready_plan(&mut f,&p,&save,&expected);f.app.world_mut().remove_resource::<Assets<Image>>();let before=atomic_world_before(&f);let entities=f.app.world().entities().len();assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err());assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);}
#[test]
fn replaced_core_instance_refuses_decoded_staging_without_replaying_movie(){let(mut f,p,save,expected)=media_fixture(false,false);let plan=ready_plan(&mut f,&p,&save,&expected);let old=&f.app.world().resource::<VnEngine>().0;let replacement=Engine::from_program_with_persistent_and_text_context(p.clone(),BevyRenderer::new(),8,HashMap::new(),rvn_core::TextRuntimeContext::new(old.text_resources().clone(),old.locale.clone())).unwrap();f.app.world_mut().insert_resource(VnEngine(replacement));let before=atomic_world_before(&f);let entities=f.app.world().entities().len();assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err());assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);}
#[test]
fn invalid_media_error_arrives_before_any_authoritative_mutation(){let(mut f,p,save,expected)=media_fixture(false,false);fs::write(f.root.join("assets/clip.webm"),b"malformed actual WebM").unwrap();let before=atomic_world_before(&f);let entities=f.app.world().entities().len();let deadline=Instant::now()+Duration::from_secs(10);loop{match crate::atomic_text_restore::prepare(f.app.world(),&p,save.clone(),&expected){Err(message)if message.contains("still staging")=>{},Err(_)=>break,Ok(_)=>panic!("Malformed media was admitted")};assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(2));}assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);}
fn private_inputs()->std::collections::BTreeSet<PathBuf>{fs::read_dir(std::env::temp_dir()).unwrap().filter_map(Result::ok).map(|e|e.path()).filter(|p|p.file_name().is_some_and(|n|n.to_string_lossy().starts_with(&format!("rvn-video-stage-0376-{}-",std::process::id())))).collect()}
#[test]
fn dropping_ready_plan_releases_real_decoder_snapshots_without_touching_world(){
    let(mut f,p,save,expected)=media_fixture(false,true);let old=private_inputs();let before=atomic_world_before(&f);let entities=f.app.world().entities().len();let plan=ready_plan(&mut f,&p,&save,&expected);let paths:Vec<_>=private_inputs().difference(&old).cloned().collect();assert_eq!(paths.len(),2);assert!(paths.iter().all(|p|p.is_file()));drop(plan);
    let deadline=Instant::now()+Duration::from_secs(5);while paths.iter().any(|p|p.exists()){assert!(Instant::now()<deadline,"Actual decoder did not close its owned snapshots after drop");std::thread::sleep(Duration::from_millis(2));}assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);
}
#[test]
fn stale_and_fabricated_view_cannot_bind_a_source_from_actual_current_or_prepared_engine(){
    let(f,p,save,expected)=media_fixture(true,false);let game=&f.app.world().resource::<VnEngine>().0;let mut stale=game.video_views().unwrap()[0].clone();stale.epoch+=1;assert!(crate::video_audio::source_for_current(f.app.world(),&stale,0.0).is_err());
    let host=prepare_selected_language(f.app.world(),&expected).unwrap();let core=game.prepare_save15_with_locale(&p,save,Some(host.locale().clone())).unwrap();let mut fabricated=core.render_batch().videos[0].clone();fabricated.id="unowned".into();assert!(crate::video_audio::source_for_prepared(f.app.world(),&core,&fabricated).is_err());
}
#[test]
fn added_bound_source_after_preparation_is_rejected_before_core_or_retirement(){
    let(mut f,p,save,expected)=media_fixture(true,false);let old=crate::native_video::start_current_for_test(f.app.world_mut()).unwrap();let plan=ready_plan(&mut f,&p,&save,&expected);let view=f.app.world().resource::<VnEngine>().0.video_views().unwrap()[0].clone();let extra=crate::video_audio::source_for_current(f.app.world(),&view,view.track.position).unwrap();let _handle=f.app.world_mut().resource_mut::<Assets<crate::video_audio::VideoAudio>>().add(extra);let before=atomic_world_before(&f);let entities=f.app.world().entities().len();assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err());assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);assert!(!old.0.lock().unwrap().revoked);
}
#[test]
fn replaced_native_owner_refuses_before_core_or_source_retirement(){let(mut f,p,save,expected)=media_fixture(false,false);let plan=ready_plan(&mut f,&p,&save,&expected);crate::native_video::replace_runtime_for_test(f.app.world_mut());let before=atomic_world_before(&f);let entities=f.app.world().entities().len();assert!(crate::atomic_text_restore::commit(f.app.world_mut(),plan).is_err());assert_eq!(atomic_world_before(&f),before);assert_eq!(f.app.world().entities().len(),entities);}
#[test]
fn removing_actual_pending_owner_cancels_its_job_and_releases_private_inputs(){
    let(f,p,save,expected)=media_fixture(false,false);let mut f=f;let old=private_inputs();let before=atomic_world_before(&f);let result=crate::atomic_text_restore::prepare(f.app.world(),&p,save,&expected);assert!(matches!(result,Err(ref e)if e.contains("still staging")));let cancel=crate::native_video::cancel_runtime_for_test(f.app.world_mut()).expect("Actual Pending job owns its cancellation signal");
    let deadline=Instant::now()+Duration::from_secs(5);while cancel.upgrade().is_some()||!private_inputs().is_subset(&old){assert!(Instant::now()<deadline,"Pending job or decoder kept a cancelled private input");std::thread::sleep(Duration::from_millis(2));}assert_eq!(atomic_world_before(&f),before);
}
#[test]
fn legitimate_live_video_projection_is_warmed_before_capturing_full_core_source(){
    let(f,p,save,expected)=atomic_save_fixture();let game=&f.app.world().resource::<VnEngine>().0;let first=game.debug_state_source().unwrap();assert!(game.accepts_debug_state_source(&first));game.video_views().unwrap();assert!(!game.accepts_debug_state_source(&first),"First actual projection changes the cache included in authority");let warmed=game.debug_state_source().unwrap();game.video_views().unwrap();assert!(game.accepts_debug_state_source(&warmed));let host=prepare_selected_language(f.app.world(),&expected).unwrap();let core=game.prepare_save15_with_locale(&p,save,Some(host.locale().clone())).unwrap();assert!(game.accepts_prepared_save15(&core));assert!(crate::native_video::prepare_restore(f.app.world(),&core).unwrap().is_some());assert!(game.accepts_prepared_save15(&core));
}

#[path="native_exact_cursor_tests.rs"]
mod exact_cursor;

#[path="native_mask_terminal_tests.rs"]
mod mask_terminal;
