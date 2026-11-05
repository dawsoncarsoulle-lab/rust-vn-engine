//! Engine::fresh keeps the owned player, but revokes its authenticated stream.
//! Only the editor may grant the next nonce over the preceding connection.
use super::*;
#[derive(Resource)]
struct Authenticated{instance:u64,nonce:u64,pending:Option<(u64,std::time::Instant)>}
pub(super) fn attach(world:&mut World,instance:u64,nonce:u64){world.insert_resource(Authenticated{instance,nonce,pending:None});}
pub(super) fn blocked(world:&World)->bool{world.get_resource::<Authenticated>().is_some_and(|auth|auth.pending.is_some())}
fn validate_grant(old:u64,nonce:u64,next:u64,current:u64,header:&Header)->Result<u64,String>{
    let Header::SessionGrant{instance,nonce:original,next_instance,next_nonce}=header else{return Err("Expected an authenticated session grant".into());};
    if *instance!=old||*original!=nonce||*next_instance!=next||current!=next||*next_nonce==0||*next_nonce==nonce{
        return Err("Foreign player session grant refused".into());}
    Ok(*next_nonce)
}
/// Called before image extraction. A fresh Engine never lends its pixels to an
/// old FrameScope, even when both start at runtime_epoch zero.
pub(super) fn observe(world:&mut World)->Result<bool,String>{
    let Some(auth)=world.get_resource::<Authenticated>()else{return Ok(false);};
    let current=crate::text_runtime_owner::current(world)?.instance;
    if auth.instance==current{return Ok(false);}
    if let Some((next,since))=auth.pending{
        if next!=current{return Err("The replacement Engine changed before its new session was authenticated".into());}
        if since.elapsed()>Duration::from_secs(5){return Err("The editor did not authenticate the replacement Engine".into());}
        return Ok(true);
    }
    let offer=Header::SessionOffer{nonce:auth.nonce,pid:std::process::id(),instance:auth.instance,next_instance:current};
    crate::editor_preview_input::release_held(world);world.resource::<Output>().release_held();
    world.remove_resource::<RenderScope>();world.remove_resource::<RenderInputFrame>();world.remove_resource::<RenderDebugger>();
    world.remove_resource::<InstalledScope>();
    world.remove_resource::<InstalledInspection>();world.remove_resource::<RenderInspection>();
    let output=world.resource::<Output>();let mut latest=output.queue.0.lock().map_err(|_|"Player image queue poisoned")?;
    latest.session_retired=true;latest.main_consumed=None;latest.awaiting_consumption=None;latest.writing=None;latest.writing_consumed=false;latest.queued_completion=None;latest.frame=Some(Packet::header(offer));output.queue.1.notify_one();drop(latest);
    world.resource_mut::<Authenticated>().pending=Some((current,std::time::Instant::now()));Ok(true)
}
/// Ordinary controls remain refused while the old stream is being retired.
/// Admission preserves old nonce/PID/instance; the new Hello is independent.
pub(super) fn controls(world:&mut World,headers:&[Header])->Result<bool,String>{
    if !observe(world)?{return Ok(false);}
    let auth=world.resource::<Authenticated>();let (next,_)=auth.pending.ok_or("Missing session transition")?;
    let old=auth.instance;let nonce=auth.nonce;
    for header in headers{
        if !matches!(header,Header::SessionGrant{..}){continue;}
        let current=crate::text_runtime_owner::current(world)?.instance;
        let next_nonce=validate_grant(old,nonce,next,current,header)?;
        let mut options=world.resource::<Options>().clone();options.nonce=next_nonce;
        let (output,receiver)=connect(&options,current)?;
        world.resource::<Output>().failed();world.insert_resource(output);
        world.insert_resource(Controls(Mutex::new(receiver)));world.insert_resource(options);
        let user_pause=world.resource::<PreviewState>().user_paused;
        {let mut state=world.resource_mut::<PreviewState>();state.sequence=0;state.received_sequence=0;state.frame_sequence=0;state.control_error=None;state.step_requested=false;}
        world.resource_mut::<PendingInput>().0.clear();
        if let Some(mut pending)=world.get_resource_mut::<PendingConsumed>(){pending.0.clear();}
        set_paused_for_debugger(world,user_pause)?;attach(world,current,next_nonce);return Ok(true);
    }
    Ok(true)
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn actual_fresh_engine_retires_the_old_gpu_writer_before_offering_its_new_session(){
        let original=rvn_core::Engine::from_program(rvn_parser::parse_program("init{set value=1}\nscene label start\n\"ready\"\n").unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
        let old=original.instance_id();let epoch=original.runtime_epoch();
        let fresh=original.fresh(crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
        let next=fresh.instance_id();assert_ne!(old,next);assert_eq!(fresh.runtime_epoch(),epoch.checked_add(1).unwrap());
        let scope=wire::FrameScope{instance:old,epoch,sequence:7,control_sequence:0,control_ack:0,
            position:rvn_core::scene::ResumePosition{pc:0,origin:None},interaction:None,
            paused:false,publication_pending:false, finished: None,control_error:None,step_allowed:false,active:None,parked:vec![]};
        let output=Output{phase:0,queue:Arc::new((Mutex::new(Latest::default()),Condvar::new())),alive:Arc::new(AtomicBool::new(true)),contexts:Default::default(),held:Default::default()};
        let mut world=World::new();world.insert_resource(crate::resources::VnEngine(fresh));world.insert_resource(output.clone());
        world.insert_resource(RenderScope(scope.clone()));attach(&mut world,old,19);
        assert!(observe(&mut world).unwrap());assert!(blocked(&world));assert!(!world.contains_resource::<RenderScope>());
        assert!(matches!(&output.queue.0.lock().unwrap().frame.as_ref().unwrap().header,
            Header::SessionOffer{nonce:19,instance,next_instance,..}if *instance==old&&*next_instance==next));
        // This is the same Output::frame path called by delayed GPU completion.
        // It cannot replace the authenticated offer with pixels from old slots.
        output.frame(Packet{header:Header::Frame{scope,width:1,height:1,debugger:None,inspection:None},pixels:vec![0;4]},None);
        assert!(matches!(&output.queue.0.lock().unwrap().frame.as_ref().unwrap().header,
            Header::SessionOffer{nonce:19,instance,next_instance,..}if *instance==old&&*next_instance==next));
        assert!(output.queue.0.lock().unwrap().completed.is_empty());
        assert!(observe(&mut world).unwrap());
        let before=format!("{:?}",world.resource::<crate::resources::VnEngine>().0.state);
        assert!(controls(&mut world,&[Header::Control{instance:old,epoch,sequence:1,frame_control_sequence:0,
            position:rvn_core::scene::ResumePosition{pc:0,origin:None},interaction:None,kind:ControlKind::Resume}]).unwrap());
        assert_eq!(format!("{:?}",world.resource::<crate::resources::VnEngine>().0.state),before);assert!(blocked(&world));
    }
    #[test]
    fn new_engine_session_grant_admits_only_its_exact_preceding_nonce_and_replacement(){
        let exact=Header::SessionGrant{instance:4,nonce:9,next_instance:5,next_nonce:10};
        assert_eq!(validate_grant(4,9,5,5,&exact).unwrap(),10);
        for bad in [Header::SessionGrant{instance:3,nonce:9,next_instance:5,next_nonce:10},Header::SessionGrant{instance:4,nonce:8,next_instance:5,next_nonce:10},Header::SessionGrant{instance:4,nonce:9,next_instance:6,next_nonce:10},Header::SessionGrant{instance:4,nonce:9,next_instance:5,next_nonce:9},Header::SessionGrant{instance:4,nonce:9,next_instance:5,next_nonce:0}]{assert!(validate_grant(4,9,5,5,&bad).is_err());}
        assert!(validate_grant(4,9,5,6,&exact).is_err());
    }
}
    #[test]
    fn actual_fresh_player_retires_logical_and_rendered_inspection_before_new_authentication() {
        use rvn_core::editor_inspection::{capture, FrameIdentity};
        let original = rvn_core::Engine::from_program(
            rvn_parser::parse_program("init{set value=1}\nscene label start\n\"ready\"\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),
            16,
        ).unwrap();
        let old = original.instance_id();
        let epoch = original.runtime_epoch();
        let logical = capture(&original.state, FrameIdentity { instance: old, epoch, sequence: 0 });
        assert!(logical.unavailable.is_none());
        let encoded = String::from_utf8(logical.encode_for(FrameIdentity { instance: old, epoch, sequence: 7 }).unwrap()).unwrap();
        let fresh = original.fresh(crate::bevy_renderer::BevyRenderer::new(), 16).unwrap();
        let next = fresh.instance_id();
        assert_ne!(old, next);
        assert_eq!(fresh.runtime_epoch(), epoch.checked_add(1).unwrap());
        let output = Output {
            phase:0,
            queue: Arc::new((Mutex::new(Latest::default()), Condvar::new())),
            alive: Arc::new(AtomicBool::new(true)), contexts: Default::default(), held: Default::default(),
        };
        let mut world = World::new();
        world.insert_resource(crate::resources::VnEngine(fresh));
        world.insert_resource(output.clone());
        world.insert_resource(InstalledInspection(Arc::new(logical)));
        world.insert_resource(RenderInspection(Some(Arc::from(encoded))));
        attach(&mut world, old, 19);
        let before = serde_json::to_value(&world.resource::<crate::resources::VnEngine>().0.state).unwrap();
        assert!(observe(&mut world).unwrap());
        assert!(blocked(&world));
        assert!(!world.contains_resource::<InstalledInspection>());
        assert!(!world.contains_resource::<RenderInspection>());
        assert!(matches!(&output.queue.0.lock().unwrap().frame.as_ref().unwrap().header,
            Header::SessionOffer { nonce: 19, instance, next_instance, .. } if *instance == old && *next_instance == next));
        assert_eq!(serde_json::to_value(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(), before);
    }

