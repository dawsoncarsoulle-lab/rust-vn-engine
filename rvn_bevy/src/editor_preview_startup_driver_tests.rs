//! Actual socket control and First/Update/Last driver. Explicit CPU frame
//! bytes exercise transport only; this is not a RenderDevice/GPU qualification.
use super::*;
use std::{net::{TcpListener,TcpStream,Shutdown},time::Instant};
fn capture_actual(world:&mut World){super::capture(world);}
fn reconcile_actual(world:&mut World){crate::scene_publication::consume(world);}
fn fixture(source:&str)->(App,TcpStream){
    let(_,mut app)=crate::debugger_startup::tests::fixture_source(source);
    app.init_resource::<crate::resources::VnRenderState>().init_resource::<PreviewState>().init_resource::<PendingInput>().init_resource::<crate::editor_preview_input::Receipts>();
    app.insert_resource(State::new(crate::resources::VnState::Stepping)).init_resource::<NextState<crate::resources::VnState>>();
    let actual=app.world().resource::<DebugBootstrap>().owner.snapshot();
    let listener=TcpListener::bind(("127.0.0.1",0)).unwrap();let options=Options{port:listener.local_addr().unwrap().port(),nonce:398,width:1,height:1};
    let(output,rx)=connect(&options,actual.instance).unwrap();let(mut peer,_)=listener.accept().unwrap();peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();peer.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    assert!(matches!(Packet::read(&mut peer).unwrap().header,Header::Hello{nonce:398,instance,..}if instance==actual.instance));
    app.insert_resource(options).insert_resource(output).insert_resource(Controls(Mutex::new(rx)));sessions::attach(app.world_mut(),actual.instance,398);
    app.add_systems(PreUpdate,(super::super::drain_input,super::super::drain_consumed).chain()).add_systems(First,(super::super::control,crate::editor_debugger::drain).chain()).add_systems(Update,reconcile_actual).add_systems(Last,capture_actual);
    (app,peer)
}
fn publish(app:&mut App,peer:&mut TcpStream)->wire::FrameScope{
    app.update();let world=app.world();let scope=world.resource::<RenderScope>().0.clone();
    world.resource::<Output>().frame_with_startup(Packet{header:Header::Frame{scope:scope.clone(),width:1,height:1,debugger:world.resource::<RenderDebugger>().0.clone(),inspection:None},pixels:vec![0,0,0,255]},None,world.resource::<RenderStartupSource>().0.clone());
    assert!(matches!(Packet::read(peer).unwrap().header,Header::Frame{scope:seen,..}if seen.sequence==scope.sequence));
    Packet::header(Header::FrameConsumed{instance:scope.instance,epoch:scope.epoch,frame_sequence:scope.sequence}).write(peer).unwrap();
    // The test peer consumes actual loopback bytes, then runs the genuine
    // First/PreUpdate barrier until that typed receipt has settled.
    let end=Instant::now()+Duration::from_secs(3);
    while app.world().resource::<Output>().queue.0.lock().unwrap().main_consumed.as_ref().is_none_or(|v|v.scope.sequence!=scope.sequence){
        app.update();assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(2));
    }
    scope
}
fn run_socket(app:&mut App,peer:&mut TcpStream,scope:&wire::FrameScope){
    Packet::header(Header::Debugger{instance:scope.instance,epoch:scope.epoch,sequence:scope.control_sequence+1,frame_sequence:scope.sequence,frame_control_sequence:scope.control_sequence,command:rvn_core::editor_debugger::Command::Run(rvn_core::editor_debugger::StepKind::Continue).encode().unwrap()}).write(peer).unwrap();
}
fn handoff(app:&mut App){
    let end=Instant::now()+Duration::from_secs(15);while app.world().contains_resource::<DebugBootstrap>(){
        app.update();if let Some(actual)=app.world().get_resource::<DebugBootstrap>(){assert!(actual.failed.is_none(),"Actual driver failed: {:?}",actual.failed);}
        assert!(Instant::now()<end,"Actual scheduled Startup handoff timed out");std::thread::sleep(Duration::from_millis(2));
    }
}
fn close(app:&mut App,peer:TcpStream){app.world().resource::<Output>().failed();let _=peer.shutdown(Shutdown::Both);}
#[test]
fn startup_driver_actual_socket_schedule_commits_then_hands_the_same_engine_to_gameplay(){
    let(mut app,mut peer)=fixture("init {set boot=random(1,9) set after=2}\nscene label start\n\"Done\"\n");
    let scope=publish(&mut app,&mut peer);let old_instance=scope.instance;let gate=app.world().resource::<DebugBootstrap>().broker.gate();let initial=app.world().resource::<DebugBootstrap>().owner.state().random;let mut expected=initial;let value=expected.integer(1,9).unwrap();
    run_socket(&mut app,&mut peer,&scope);handoff(&mut app);let world=app.world();let engine=&world.resource::<crate::resources::VnEngine>().0;
    assert_eq!(engine.instance_id(),old_instance);assert!(engine.renderer.audio_checkpoints.gate().same_gate(&gate));assert_eq!(engine.state.random,expected);assert_eq!(engine.state.vars["boot"],rvn_parser::Value::Int(value));assert_eq!(engine.state.vars["after"],rvn_parser::Value::Int(2));assert!(engine.state.scene_execution.is_some());
    assert_eq!(world.resource::<PreviewState>().received_sequence,1);assert_eq!(world.resource::<PreviewState>().sequence,1);assert!(world.resource::<PreviewState>().control_error.is_none());assert_eq!(world.resource::<Options>().nonce,398);assert!(!world.contains_resource::<RenderStartupSource>());close(&mut app,peer);
}
#[test]
fn startup_driver_loading_pause_retains_the_completed_candidate_and_real_random(){
    let(mut app,mut peer)=fixture("init {if random(1,9)>0 {sfx.play(\"delayed.wav\")} set after=2}\nscene label start\n\"Done\"\n");
    let scope=publish(&mut app,&mut peer);run_socket(&mut app,&mut peer,&scope);
    let end=Instant::now()+Duration::from_secs(3);while app.world().resource::<PreviewState>().received_sequence<1{app.update();assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(2));}
    assert!(app.world().contains_resource::<DebugBootstrap>());assert!(crate::debugger_startup::driver::pending(app.world()));
    let before=serde_json::to_value(app.world().resource::<DebugBootstrap>().owner.state()).unwrap();
    crate::debugger_startup::driver::pause(app.world_mut()).unwrap();for _ in 0..4{app.update();assert_eq!(serde_json::to_value(app.world().resource::<DebugBootstrap>().owner.state()).unwrap(),before);assert!(crate::debugger_startup::driver::pending(app.world()));}
    crate::debugger_startup::driver::resume(app.world_mut()).unwrap();handoff(&mut app);let engine=&app.world().resource::<crate::resources::VnEngine>().0;assert_eq!(engine.state.vars["after"],rvn_parser::Value::Int(2));assert_eq!(rvn_core::audio_playback::describe(&engine.state,engine.runtime_epoch()).unwrap().len(),1);close(&mut app,peer);
}
#[test]
fn startup_driver_refusals_are_acknowledged_and_old_phase_gpu_writers_cannot_publish_after_handoff(){
    let(mut app,mut peer)=fixture("init {set boot=1}\nscene label start\n\"Done\"\n");let scope=publish(&mut app,&mut peer);let delayed=app.world().resource::<Output>().clone();let source=app.world().resource::<RenderStartupSource>().0.clone();
    let bad=Header::Control{instance:scope.instance,epoch:scope.epoch,sequence:1,frame_control_sequence:scope.control_sequence,position:scope.position.clone(),interaction:None,kind:ControlKind::Advance};assert!(super::control(app.world_mut(),&bad));assert_eq!(app.world().resource::<PreviewState>().received_sequence,1);assert_eq!(app.world().resource::<PreviewState>().sequence,0);assert!(app.world().resource::<PreviewState>().control_error.as_ref().unwrap().contains("Dialogue"));
    let scope=publish(&mut app,&mut peer);Packet::header(Header::Debugger{instance:scope.instance,epoch:scope.epoch,sequence:2,frame_sequence:scope.sequence,frame_control_sequence:scope.control_sequence,command:rvn_core::editor_debugger::Command::Run(rvn_core::editor_debugger::StepKind::Continue).encode().unwrap()}).write(&mut peer).unwrap();handoff(&mut app);
    let before=app.world().resource::<Output>().queue.0.lock().unwrap().sequence;let mut old=scope;old.sequence=before+100;delayed.frame_with_startup(Packet{header:Header::Frame{scope:old.clone(),width:1,height:1,debugger:None,inspection:None},pixels:vec![0,0,0,255]},None,source);
    let output=app.world().resource::<Output>();assert_eq!(output.queue.0.lock().unwrap().sequence,before);assert!(output.completed(old.instance,old.epoch,old.sequence).is_none());assert_eq!(app.world().resource::<Options>().nonce,398);close(&mut app,peer);
}
