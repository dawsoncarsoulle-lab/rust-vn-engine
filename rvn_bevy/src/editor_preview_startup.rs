//! The existing authenticated image/control lease with a genuine startup owner.
use super::*;
use crate::debugger_startup::DebugBootstrap;
#[path="editor_preview_startup_imagemap_0997.rs"]
mod imagemap;
pub(super) fn control(world:&mut World,header:&Header)->bool{
    if !world.contains_resource::<DebugBootstrap>(){return false;}
    if matches!(header,Header::Input{..}|Header::Release{..}){
        if let Some((sequence,result))=imagemap::input(world,header){
            if sequence>world.resource::<PreviewState>().received_sequence{acknowledge(world,sequence,result);}
        }
        return true;
    }
    if let Header::Control{instance,epoch,sequence,frame_control_sequence,position,kind,..}=header{
        if *sequence<=world.resource::<PreviewState>().received_sequence{return true;}
        let result=(||{
            let startup=world.resource::<DebugBootstrap>();let actual=startup.owner.snapshot();
            if *instance!=actual.instance||*epoch!=actual.epoch{return Err("The rendered startup owner changed".to_string());}
            if *kind==ControlKind::Stop{stop(world,None);return Ok(());}
            if *frame_control_sequence!=world.resource::<PreviewState>().sequence{return Err("The preceding startup control image expired".into());}
            match kind{
                ControlKind::Pause=>crate::debugger_startup::driver::pause(world),
                ControlKind::Resume=>crate::debugger_startup::driver::resume(world),
                ControlKind::Step=>{
                    if *position!=actual.position||!paused(world){return Err("The displayed paused Init instruction changed".into());}
                    if crate::debugger_startup::driver::pending(world){return Err("Startup source installation is still pending".into());}
                    crate::debugger_startup::driver::run(world,rvn_core::editor_debugger::StepKind::Into)
                },
                ControlKind::Advance=>Err("Initialization has no captured narrative Dialogue to advance".into()),
                ControlKind::Stop=>unreachable!(),
            }
        })();
        acknowledge(world,*sequence,result);return true;
    }
    let Header::Debugger{instance,epoch,sequence,frame_sequence,frame_control_sequence,command}=header else{return true;};
    if *sequence<=world.resource::<PreviewState>().received_sequence{return true;}
    let result=(||{
        let startup=world.resource::<DebugBootstrap>();let actual=startup.owner.snapshot();
        if startup.profile!=crate::debugger_startup::BootstrapProfile::Debug{return Err("This runtime bootstrap was not admitted for debugger commands".into());}
        if *instance!=actual.instance||*epoch!=actual.epoch{return Err("The rendered startup owner changed".to_string());}
        if startup.publication_pending||startup.failed.is_some(){return Err("Startup publication is not ready".into());}
        if *frame_control_sequence!=world.resource::<PreviewState>().sequence{return Err("The preceding startup control image expired".into());}
        let (frame,source)=world.resource::<Output>().completed_startup(*instance,*epoch,*frame_sequence).ok_or("The completed startup debugger image expired")?;
        if frame.control_sequence!=*frame_control_sequence||frame.position!=actual.position{return Err("The completed startup debugger source changed".into());}
        let source=source.ok_or("The completed image has no actual startup source observation")?;
        if !startup.owner.accepts_observation_source(&source){return Err("The completed startup continuation changed".into());}
        let command=rvn_core::editor_debugger::Command::decode(command)?;
        let sites=matches!(&command,rvn_core::editor_debugger::Command::Sites{..});
        if !sites&&crate::debugger_startup::driver::pending(world){return Err("The admitted Startup source installation must settle before another debugger command".into());}
        let next_intent=if sites{None}else{Some(startup.intent.checked_add(1).ok_or("Startup control generation exhausted")?)};
        match command{
            rvn_core::editor_debugger::Command::Configure{breakpoints,watches}=>world.resource_mut::<DebugBootstrap>().owner.configure(breakpoints,watches).map_err(|e|e.to_string())?,
            rvn_core::editor_debugger::Command::Sites{offset}=>{let page=world.resource::<DebugBootstrap>().owner.site_page(offset).map_err(|e|e.to_string())?;world.resource_mut::<DebugBootstrap>().sites=Some(page);},
            rvn_core::editor_debugger::Command::Run(kind)=>crate::debugger_startup::driver::run(world,kind)?,
            rvn_core::editor_debugger::Command::Disable=>return Err("Initialization must finish through its admitted debugger continuation".into()),
        }
        if let Some(next)=next_intent{world.resource_mut::<DebugBootstrap>().intent=next;}
        Ok(())
    })();
    acknowledge(world,*sequence,result);true
}
fn acknowledge(world:&mut World,sequence:u64,result:Result<(),String>){
    let mut state=world.resource_mut::<PreviewState>();state.received_sequence=sequence;
    state.control_error=result.as_ref().err().cloned();if result.is_ok(){state.sequence=sequence;}
}
pub(crate) fn finish_handoff(world:&mut World)->Result<(),String>{
    if let Some(mut output)=world.get_resource_mut::<Output>(){output.retire_startup_phase()?;}
    world.remove_resource::<RenderScope>();world.remove_resource::<RenderStartupSource>();world.remove_resource::<RenderInputFrame>();
    world.remove_resource::<RenderDebugger>();world.remove_resource::<RenderInspection>();world.remove_resource::<InstalledScope>();world.remove_resource::<InstalledInspection>();
    Ok(())
}
pub(super) fn capture(world:&mut World)->bool{
    let Some(startup)=world.get_resource::<DebugBootstrap>()else{return false;};
    let mut snapshot=startup.owner.snapshot();snapshot.running=startup.mode.is_some()&&!crate::debugger_startup::driver::suspended(world);snapshot.sites=startup.sites.clone();
    if let Some(output)=world.get_resource::<Output>(){output.discard_foreign(snapshot.instance,snapshot.epoch);}
    let pending=startup.publication_pending||crate::debugger_startup::driver::pending(world);let failed=startup.failed.clone();
    let Some(next)=world.resource::<PreviewState>().frame_sequence.checked_add(1)else{stop(world,Some("Editor frame sequence exhausted".into()));return true;};
    let mut scope=if pending{
        let Some(installed)=world.get_resource::<InstalledScope>()else{return true;};installed.0.clone()
    }else{wire::FrameScope{instance:snapshot.instance,epoch:snapshot.epoch,sequence:0,control_sequence:0,control_ack:0,control_error:None,
        paused:true,publication_pending:false,finished:Some(false),step_allowed:false,interaction:None,
        position:snapshot.position.clone(),active:None,parked:vec![]}}
    ;scope.sequence=next;scope.control_sequence=world.resource::<PreviewState>().sequence;scope.control_ack=world.resource::<PreviewState>().received_sequence;
    scope.control_error=world.resource::<PreviewState>().control_error.clone().or(failed);scope.publication_pending=pending;scope.paused=paused(world)||startup.mode.is_none();scope.step_allowed=!pending&&scope.control_error.is_none()&&scope.paused&&!startup.owner.waiting_for_video()&&!startup.owner.waiting_for_imagemap();
    let source=if pending{None}else{match startup.owner.observation_source(){Ok(source)=>Some(Arc::new(source)),Err(error)=>{stop(world,Some(error.to_string()));return true;}}};
    let observation=if pending{None}else{snapshot.encode().ok()};
    let inspection=if pending{None}else{let actual=world.resource::<DebugBootstrap>();let capture=rvn_core::editor_inspection::capture(actual.owner.state(),rvn_core::editor_inspection::FrameIdentity::of(&scope));capture.encode_for(rvn_core::editor_inspection::FrameIdentity::of(&scope)).ok().and_then(|bytes|String::from_utf8(bytes).ok()).map(Arc::from)};
    // The GPU readback carries precisely this physical geometry and the
    // observation above. Input may not recapture either from a later World.
    let physical=if !pending&&startup.owner.waiting_for_imagemap(){crate::editor_preview_input::capture(world,scope.clone())}else{None};
    world.resource_mut::<PreviewState>().frame_sequence=next;
    if !pending{world.insert_resource(InstalledScope(scope.clone()));}
    world.insert_resource(RenderScope(scope));world.insert_resource(RenderStartupSource(source));world.insert_resource(RenderDebugger(observation));world.insert_resource(RenderInspection(inspection));world.insert_resource(RenderInputFrame(physical));true
}
// CPU transport contract, not a GPU/readback or native-player qualification.
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn startup_real_socket_uses_actual_owner_completed_image_and_source_before_json(){
        use std::net::TcpListener;
        let(_,mut app)=crate::debugger_startup::tests::fixture();
        let world=app.world_mut();world.init_resource::<PreviewState>();world.init_resource::<PendingInput>();
        let actual=world.resource::<DebugBootstrap>().owner.snapshot();
        let listener=TcpListener::bind(("127.0.0.1",0)).unwrap();
        let options=Options{port:listener.local_addr().unwrap().port(),nonce:398,width:1,height:1};
        let(output,rx)=connect(&options,actual.instance).unwrap();
        let(mut peer,_)=listener.accept().unwrap();peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();peer.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
        let hello=Packet::read(&mut peer).unwrap();assert!(matches!(hello.header,Header::Hello{nonce:398,pid,instance,..}if pid==std::process::id()&&instance==actual.instance));
        world.insert_resource(output.clone());world.insert_resource(Controls(Mutex::new(rx)));sessions::attach(world,actual.instance,398);
        assert!(capture(world));let scope=world.resource::<RenderScope>().0.clone();assert_eq!(scope.position,actual.position);
        let observation=world.resource::<RenderDebugger>().0.clone();
        // The actual cache/writer path is driven with an explicit CPU pixel
        // fixture. No GPU completion is inferred from this contract test.
        output.frame_with_startup(Packet{header:Header::Frame{scope:scope.clone(),width:1,height:1,debugger:observation,inspection:None},pixels:vec![0,0,0,255]},None,world.resource::<RenderStartupSource>().0.clone());
        assert!(matches!(Packet::read(&mut peer).unwrap().header,Header::Frame{scope:seen,..}if seen.instance==actual.instance&&seen.position==actual.position));
        Packet::header(Header::FrameConsumed{instance:scope.instance,epoch:scope.epoch,frame_sequence:scope.sequence}).write(&mut peer).unwrap();
        let before=serde_json::to_value(world.resource::<DebugBootstrap>().owner.state()).unwrap();
        let command=rvn_core::editor_debugger::Command::Sites{offset:0}.encode().unwrap();
        Packet::header(Header::Debugger{instance:actual.instance,epoch:actual.epoch,sequence:1,frame_sequence:scope.sequence,frame_control_sequence:scope.control_sequence,command}).write(&mut peer).unwrap();
        let end=std::time::Instant::now()+Duration::from_secs(3);
        while world.resource::<PreviewState>().received_sequence<1{super::super::control(world);assert!(std::time::Instant::now()<end);std::thread::sleep(Duration::from_millis(5));}
        assert!(world.resource::<PreviewState>().control_error.is_none());assert!(!world.resource::<DebugBootstrap>().sites.as_ref().unwrap().entries.is_empty());
        assert_eq!(serde_json::to_value(world.resource::<DebugBootstrap>().owner.state()).unwrap(),before);
        drain_input(world);drain_consumed(world);
        assert!(capture(world));let scope=world.resource::<RenderScope>().0.clone();
        output.frame_with_startup(Packet{header:Header::Frame{scope:scope.clone(),width:1,height:1,debugger:world.resource::<RenderDebugger>().0.clone(),inspection:None},pixels:vec![0,0,0,255]},None,world.resource::<RenderStartupSource>().0.clone());
        assert!(matches!(Packet::read(&mut peer).unwrap().header,Header::Frame{..}));
        Packet::header(Header::FrameConsumed{instance:scope.instance,epoch:scope.epoch,frame_sequence:scope.sequence}).write(&mut peer).unwrap();
        let plan=crate::debugger_startup::prepare_world_step(world,rvn_core::editor_debugger::StepKind::Into).unwrap();
        let committed=crate::debugger_startup::commit_world_step(world,plan).unwrap();assert!(committed.publication.is_none());
        crate::debugger_startup::install_committed_world_step(world,committed);
        assert_eq!(world.resource::<DebugBootstrap>().owner.snapshot().position,scope.position);
        let stale=Header::Debugger{instance:actual.instance,epoch:actual.epoch,sequence:2,frame_sequence:scope.sequence,frame_control_sequence:scope.control_sequence,command:"invalid JSON must stay undecoded".into()};
        assert!(control(world,&stale));assert!(world.resource::<PreviewState>().control_error.as_ref().unwrap().contains("continuation changed"));
        assert_eq!(serde_json::to_value(world.resource::<DebugBootstrap>().owner.state()).unwrap(),before);
        let foreign=Header::Debugger{instance:actual.instance+1,epoch:actual.epoch,sequence:3,frame_sequence:scope.sequence,frame_control_sequence:1,command:"invalid JSON must stay undecoded".into()};
        assert!(control(world,&foreign));assert!(world.resource::<PreviewState>().control_error.as_ref().unwrap().contains("owner changed"));
        assert_eq!(serde_json::to_value(world.resource::<DebugBootstrap>().owner.state()).unwrap(),before);
        assert!(!world.contains_resource::<crate::resources::VnEngine>());output.failed();let _=peer.shutdown(std::net::Shutdown::Both);
    }
}

#[cfg(all(test,not(target_arch="wasm32")))]
#[path="editor_preview_startup_driver_tests.rs"]
mod actual_driver_tests;
