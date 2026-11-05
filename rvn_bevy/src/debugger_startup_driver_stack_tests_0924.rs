//! Real owner/publication paths on the Windows player's 1 MiB stack budget.
//! Test-only threads; no production runner, stack setting or scheduler changes.
use super::*;
use crate::debugger_startup as startup;
use crate::resources::VnEngine;
use std::time::{Duration,Instant};

fn one_mib(name:&str,body:fn()){
    std::thread::Builder::new().name(name.into()).stack_size(1024*1024)
        .spawn(body).unwrap().join().unwrap();
}

#[test]
fn real_driver_heap_tokens_have_bounded_inline_transport_0924(){
    assert!(std::mem::size_of::<Driver>()<512);
    assert_eq!(std::mem::size_of::<Option<Box<PreparedBootstrapStep>>>(),std::mem::size_of::<usize>());
    assert_eq!(std::mem::size_of::<Box<startup::CommittedBootstrapStep>>(),std::mem::size_of::<usize>());
}

#[inline(never)]
fn continue_body(){
    let(_,mut app)=startup::tests::fixture_source("init {set boot=1 set other=2}\nscene label start\n\"Done\"\n");
    let before=app.world().resource::<DebugBootstrap>().owner.snapshot();
    let gate=app.world().resource::<DebugBootstrap>().broker.gate();
    run(app.world_mut(),StepKind::Continue).unwrap();
    let end=Instant::now()+Duration::from_secs(20);
    loop{
        drain(app.world_mut());
        if let Some(actual)=app.world().get_resource::<DebugBootstrap>(){
            assert_eq!(actual.owner.snapshot().instance,before.instance);
            assert_eq!(actual.owner.snapshot().epoch,before.epoch);
            assert!(actual.broker.gate().same_gate(&gate));
            assert!(actual.failed.is_none(),"Actual startup failed: {:?}",actual.failed);
            assert!(!actual.publication_pending,"Install must consume the committed publication before another step");
            assert!(!app.world().contains_resource::<VnEngine>());
        }else{break;}
        assert!(Instant::now()<end,"Actual Continue did not hand off");
        app.update();std::thread::sleep(Duration::from_millis(2));
    }
    let game=app.world().resource::<VnEngine>();
    assert_eq!(game.0.instance_id(),before.instance);assert_eq!(game.0.runtime_epoch(),before.epoch);
    assert!(game.0.renderer.audio_checkpoints.gate().same_gate(&gate));
    assert_eq!(game.0.state.vars["boot"],rvn_parser::Value::Int(1));
    assert_eq!(game.0.state.vars["other"],rvn_parser::Value::Int(2));
    let installed=app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().expect("Actual startup Source publication was installed");
    assert_eq!(installed.runtime_epoch,before.epoch);
    assert_eq!(installed.state.vars["other"],rvn_parser::Value::Int(2));
    assert!(!installed.state.vars.contains_key("narrative_started"));
    assert_eq!(crate::text_runtime_owner::current(app.world()).unwrap().instance,before.instance);
    assert!(crate::editor_debugger::enabled(game));
}
#[test]
fn real_startup_continue_publication_and_handoff_fit_one_mib_0924(){
    one_mib("startup-driver-one-mib",continue_body);
}

#[inline(never)]
fn rejection_body(){
    let(_,mut app)=startup::tests::fixture();
    let before=app.world().resource::<DebugBootstrap>().owner.snapshot();
    let state=serde_json::to_value(app.world().resource::<DebugBootstrap>().owner.state()).unwrap();
    let observed=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    let gate=app.world().resource::<DebugBootstrap>().broker.gate();
    let abandoned=startup::prepare_world_step_boxed(app.world(),StepKind::Continue).unwrap();
    assert!(abandoned.ready());drop(abandoned);
    assert_eq!(app.world().resource::<DebugBootstrap>().owner.snapshot(),before);
    assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&observed));
    let rejected=startup::prepare_world_step_boxed(app.world(),StepKind::Continue).unwrap();
    // This real World preference participates in the captured presentation
    // receipt. The exact candidate must fail before changing its owner/broker.
    app.insert_resource(crate::systems::settings_menu::Settings{music_volume:0.25,..default()});
    let error=startup::commit_world_step_boxed(app.world_mut(),rejected).err().expect("Changed actual presentation must reject");
    assert!(error.contains("presentation host changed"),"{error}");
    let actual=app.world().resource::<DebugBootstrap>();
    assert_eq!(actual.owner.snapshot(),before);
    assert_eq!(serde_json::to_value(actual.owner.state()).unwrap(),state);
    assert!(actual.owner.accepts_observation_source(&observed));assert!(actual.broker.gate().same_gate(&gate));
    assert!(!actual.publication_pending);assert!(actual.failed.is_none());
    let accepted=startup::prepare_world_step_boxed(app.world(),StepKind::Continue).unwrap();
    let committed=startup::commit_world_step_boxed(app.world_mut(),accepted).unwrap();
    assert!(committed.publication.is_some());
    assert!(app.world().resource::<DebugBootstrap>().publication_pending);
    startup::install_committed_world_step_boxed(app.world_mut(),committed);
    let actual=app.world().resource::<DebugBootstrap>();
    assert!(!actual.publication_pending);assert!(actual.broker.gate().same_gate(&gate));
    assert_eq!(actual.owner.snapshot().instance,before.instance);
    let installed=app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().unwrap();
    assert_eq!(serde_json::to_value(&installed.state).unwrap(),serde_json::to_value(actual.owner.state()).unwrap());
}
#[test]
fn real_startup_drop_reject_and_single_commit_preserve_owner_on_one_mib_0924(){
    one_mib("startup-reject-one-mib",rejection_body);
}

#[inline(never)]
fn runtime_restart_body(){
    use crate::normal_runtime_startup_tests as normal;
    let(_,mut app)=normal::runtime_app("box-driver-runtime-restart-0924","init {set seen=0 for item in [1,2,3] {set seen=seen+item}}\n",rvn_graph::NodeKind::ForEach);
    let initial=app.world().resource::<DebugBootstrap>().owner.snapshot();
    normal::pump_runtime(&mut app);
    let game=app.world().resource::<VnEngine>();
    assert_eq!(game.0.instance_id(),initial.instance);assert_eq!(game.0.runtime_epoch(),initial.epoch);
    assert_eq!(game.0.state.vars["seen"],rvn_parser::Value::Int(6));
    assert!(!game.0.state.vars.contains_key("narrative_started"));
    let gate=game.0.renderer.audio_checkpoints.gate();
    normal::menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();
    assert_eq!(app.world().resource::<VnEngine>().0.instance_id(),initial.instance,"Admission must retain the actual old owner");
    let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());
    app.world_mut().get_mut::<Window>(window).unwrap().focused=false;
    app.update();
    assert!(!app.world().contains_resource::<VnEngine>());
    let incoming=app.world().resource::<DebugBootstrap>();
    assert_eq!(incoming.profile,startup::BootstrapProfile::RuntimeRestart);
    let incoming_id=incoming.owner.snapshot().instance;
    assert_ne!(incoming_id,initial.instance);assert_eq!(incoming.owner.snapshot().epoch,initial.epoch+1);
    assert!(incoming.broker.gate().same_gate(&gate));assert!(!incoming.owner.state().vars.contains_key("seen"));
    app.world_mut().get_mut::<Window>(window).unwrap().focused=true;
    let end=Instant::now()+Duration::from_secs(20);
    loop{
        app.update();
        if let Some(actual)=app.world().get_resource::<DebugBootstrap>(){
            assert_eq!(actual.owner.snapshot().instance,incoming_id);
            assert!(actual.failed.is_none(),"Actual restart failed: {:?}",actual.failed);
        }else if app.world().get_resource::<VnEngine>().is_some_and(|engine|engine.0.instance_id()==incoming_id){break;}
        assert!(Instant::now()<end,"Actual restart did not publish and hand off");
        std::thread::sleep(Duration::from_millis(2));
    }
    let game=app.world().resource::<VnEngine>();
    assert_eq!(game.0.instance_id(),incoming_id);assert_eq!(game.0.runtime_epoch(),initial.epoch+1);
    assert_eq!(game.0.state.vars["seen"],rvn_parser::Value::Int(6));
    assert!(game.0.renderer.audio_checkpoints.gate().same_gate(&gate));
    assert!(!crate::editor_debugger::enabled(game));
    assert_eq!(app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().unwrap().runtime_epoch,initial.epoch+1);
}
#[test]
fn real_runtime_init_publication_and_async_restart_fit_one_mib_0924(){
    one_mib("runtime-restart-one-mib",runtime_restart_body);
}


/// Exact native156 baseline shape: no Init statement, two legacy labels. The
/// real completed Startup must prepare entry, publish once and transfer the
/// SAME owner through the actual World driver on the Windows 1 MiB budget.
#[inline(never)]
fn empty_init_legacy_entry_body1027(){
    assert!(std::mem::size_of::<rvn_core::engine::StartupStepCommitGuard<'static,crate::bevy_renderer::BevyRenderer>>()<=2*std::mem::size_of::<usize>(),"Core retained-step guard must carry a pointer, never inline candidate states");
    let(_,mut app)=startup::tests::fixture_source("label start\n\"Première scène\"\njump other\nlabel other\n\"Autre scène\"\n");
    let original=app.world().resource::<DebugBootstrap>().owner.snapshot();
    let observed=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
    let gate=app.world().resource::<DebugBootstrap>().broker.gate();
    assert!(app.world().resource::<DebugBootstrap>().owner.is_complete());
    assert!(!app.world().contains_resource::<VnEngine>());
    assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&observed));
    run(app.world_mut(),StepKind::Continue).unwrap();
    let end=Instant::now()+Duration::from_secs(20);
    while app.world().contains_resource::<DebugBootstrap>(){
        drain(app.world_mut());
        if let Some(actual)=app.world().get_resource::<DebugBootstrap>(){
            assert!(actual.failed.is_none(),"{:?}",actual.failed);
            assert_eq!(actual.owner.snapshot().instance,original.instance);
            assert!(actual.broker.gate().same_gate(&gate));
        }
        assert!(Instant::now()<end,"Real empty Init entry did not complete");
        app.update();std::thread::sleep(Duration::from_millis(2));
    }
    let game=app.world().resource::<VnEngine>();
    assert_eq!(game.0.instance_id(),original.instance);assert_eq!(game.0.runtime_epoch(),original.epoch);
    assert!(game.0.renderer.audio_checkpoints.gate().same_gate(&gate));
    assert!(crate::editor_debugger::enabled(game));
    let installed=app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().expect("Actual gameplay entry publication");
    assert_eq!(installed.runtime_epoch,original.epoch);
    assert_eq!(crate::text_runtime_owner::current(app.world()).unwrap().instance,original.instance);
}
#[test]
fn real_startup_empty_init_legacy_entry_full_driver_handoff_fits_one_mib1027(){
    one_mib("empty-init-entry-one-mib",empty_init_legacy_entry_body1027);
}
