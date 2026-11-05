//! Actual BevyRenderer command queue, without a window or physical audio claim.
use super::*;
use rvn_core::{Engine,engine::{EngineStartup,TextRuntimeContext}};
use rvn_core::editor_debugger::StepKind;
use std::{collections::HashMap,sync::atomic::{AtomicUsize,Ordering}};
fn source()->rvn_parser::ParsedProgram{
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    let root=std::path::PathBuf::from(std::env::var("RVN_STARTUP_STEP_FIXTURES").expect("private fixture root")).join(format!("bevy-ending-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir(&root).unwrap();
    let path=root.join("main.rvn");std::fs::write(&path,"init{flow.consume_v2(\"boot\",\"ending\"){unlock_ending \"good\"}}\nscene label start\n\"Done\"").unwrap();rvn_parser::parse_file_program_with_uses(path).unwrap()
}
fn commit(s:&mut EngineStartup<BevyRenderer>)->StartupPublication{
    let p=s.prepare_step(StepKind::Continue).unwrap();let pcm=rvn_core::audio_checkpoint::AudioCheckpointSnapshot{instance_id:s.snapshot().instance,runtime_epoch:p.render_batch().unwrap().runtime_epoch,rows:vec![],music:vec![]};s.begin_prepared_host_step_commit(p).unwrap().commit_with_audio_checkpoint(pcm).unwrap().publication.unwrap()
}
#[test]
fn ending0904_bevy_actual_publication_delivers_one_real_command_after_replacement(){
    let mut s=Engine::prepare_debugger_startup(source(),BevyRenderer::new(),8,HashMap::new(),TextRuntimeContext::default(),None).unwrap();let publication=commit(&mut s);assert_eq!(publication.commands.iter().filter(|c|matches!(c,VnCommand::UnlockEnding{id}if id=="good")).count(),1);assert!(s.is_complete());let mut engine=s.into_engine().unwrap();assert!(engine.renderer.take_pending().is_empty());
    let old_epoch=publication.batch.runtime_epoch;let renderer=std::mem::replace(&mut engine.renderer,BevyRenderer::new());let mut fresh=engine.prepare_fresh_startup(renderer,8).unwrap();let publication=commit(&mut fresh);assert!(publication.batch.runtime_epoch>old_epoch);assert_eq!(publication.commands.iter().filter(|c|matches!(c,VnCommand::UnlockEnding{id}if id=="good")).count(),1,"real Fresh epoch clearing must precede effect delivery");assert!(fresh.is_complete());assert!(fresh.into_engine().unwrap().renderer.take_pending().is_empty());
}
