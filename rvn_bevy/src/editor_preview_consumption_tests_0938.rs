//! Explicit CPU transport credit fixtures; no RenderDevice/native acceptance.
use super::*;
fn output()->Output {Output{phase:0,queue:Arc::new((Mutex::new(Latest::default()),Condvar::new())),alive:Arc::new(AtomicBool::new(true)),contexts:Default::default(),held:Default::default()}}
fn scope(sequence:u64)->wire::FrameScope {
    wire::FrameScope{instance:7,epoch:0,sequence,control_sequence:0,control_ack:0,control_error:None,paused:true,publication_pending:false,finished:Some(false),step_allowed:true,interaction:None,position:rvn_core::scene::ResumePosition{pc:0,origin:None},active:None,parked:vec![]}
}
fn packet(scope:&wire::FrameScope)->Packet {Packet{header:Header::Frame{scope:scope.clone(),width:1,height:1,debugger:None,inspection:None},pixels:vec![1,2,3,255]}}
fn take(out:&Output)->(Packet,Option<WriteToken>) {out.take_for_write(&mut out.queue.0.lock().unwrap()).expect("available actual transport credit")}
fn write_cpu(out:&Output) {let(p,t)=take(out);p.write(&mut Vec::new()).unwrap();out.finish_write(t,true);}
fn physical(scope:wire::FrameScope,cost:usize)->Arc<crate::editor_preview_input::PhysicalFrame> {
    let game=rvn_core::Engine::new(rvn_parser::parse("\"Ready\"\n").unwrap(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();
    let mut world=World::new();world.insert_resource(crate::resources::VnEngine(game));world.init_resource::<crate::programmable_ui::Screens>();world.init_resource::<bevy::ui::UiStack>();
    let mut frame=crate::editor_preview_input::capture(&world,scope).unwrap();Arc::get_mut(&mut frame).unwrap().cost=cost;frame
}
#[test]
fn slow_main_keeps_exact_pin_and_one_successor_while_gpu_latest_coalesces() {
    let out=output();let first=scope(1);out.frame(packet(&first),Some(physical(first.clone(),3)));write_cpu(&out);assert!(out.consume_frame(7,0,1).unwrap());
    out.frame(packet(&scope(2)),Some(physical(scope(2),5)));write_cpu(&out);
    for n in 3..=1000 {out.frame(packet(&scope(n)),None);}
    let mut q=out.queue.0.lock().unwrap();assert!(!q.can_write());assert!(out.take_for_write(&mut q).is_none());
    assert_eq!(q.main_consumed.as_ref().unwrap().scope,first);assert_eq!(q.awaiting_consumption.as_ref().unwrap().scope.sequence,2);
    assert!(matches!(&q.frame.as_ref().unwrap().header,Header::Frame{scope,..} if scope.sequence==1000));assert_eq!(q.completed.len(),2);drop(q);
    assert!(out.completed(7,0,1).is_some());assert!(out.context(7,0,1).is_some());
    assert!(out.consume_frame(7,0,2).unwrap());let(p,t)=take(&out);assert!(matches!(&p.header,Header::Frame{scope,..} if scope.sequence==1000));p.write(&mut Vec::new()).unwrap();out.finish_write(t,true);
}
#[test]
fn queued_unpublished_or_foreign_ack_never_opens_credit() {
    let out=output();out.frame(packet(&scope(1)),None);
    assert!(out.consume_frame(7,0,1).is_err());assert!(out.consume_frame(8,0,1).is_err());assert!(out.consume_frame(7,1,1).is_err());assert!(out.consume_frame(7,0,0).is_err());
    write_cpu(&out);out.frame(packet(&scope(2)),None);assert!(out.consume_frame(7,0,2).is_err());assert!(!out.queue.0.lock().unwrap().can_write());
    assert!(out.consume_frame(7,0,1).unwrap());assert!(!out.consume_frame(7,0,1).unwrap());assert!(out.queue.0.lock().unwrap().can_write());
}
#[test]
fn consumption_before_write_return_promotes_only_on_exact_success() {
    let out=output();out.frame(packet(&scope(1)),None);let(p,t)=take(&out);
    assert!(!out.consume_frame(7,0,1).unwrap());assert!(out.queue.0.lock().unwrap().main_consumed.is_none());
    p.write(&mut Vec::new()).unwrap();out.finish_write(t,true);
    let q=out.queue.0.lock().unwrap();assert_eq!(q.main_consumed.as_ref().unwrap().scope.sequence,1);assert!(q.awaiting_consumption.is_none());assert!(!q.writing_consumed);
}
#[test]
fn failed_write_cannot_promote_ack_and_keeps_previous_main_pin() {
    let out=output();out.frame(packet(&scope(1)),None);write_cpu(&out);out.consume_frame(7,0,1).unwrap();
    out.frame(packet(&scope(2)),None);let(_,t)=take(&out);assert!(!out.consume_frame(7,0,2).unwrap());out.finish_write(t,false);
    let q=out.queue.0.lock().unwrap();assert_eq!(q.main_consumed.as_ref().unwrap().scope.sequence,1);assert!(q.awaiting_consumption.is_none());assert!(!q.writing_consumed);drop(q);
    assert!(out.consume_frame(7,0,2).is_err());assert!(out.completed(7,0,2).is_none());
}
#[test]
fn main_and_successor_geometry_are_bounded_pins_not_budget_eviction_candidates() {
    let out=output();let main=physical(scope(1),8*1024*1024);let next=physical(scope(2),8*1024*1024);
    out.frame(packet(&main.scope),Some(main.clone()));write_cpu(&out);out.consume_frame(7,0,1).unwrap();
    out.frame(packet(&next.scope),Some(next.clone()));write_cpu(&out);
    let forbidden=physical(scope(3),1);out.frame(packet(&forbidden.scope),Some(forbidden));
    assert!(out.queue.0.lock().unwrap().frame.is_none());assert!(Arc::ptr_eq(&out.context(7,0,1).unwrap(),&main));assert!(Arc::ptr_eq(&out.context(7,0,2).unwrap(),&next));
    assert_eq!(Output::context_cost_all(&Default::default(),&Default::default(),None,None,Some(&main),Some(&main)),8*1024*1024);
    assert_eq!(Output::context_cost_all(&Default::default(),&Default::default(),None,None,Some(&main),Some(&next)),16*1024*1024);
    assert!(!out.alive.load(Ordering::Acquire));
    {let q=out.queue.0.lock().unwrap();assert!(!q.can_write());assert!(q.failure_reason.as_deref().unwrap().contains("16 MiB"));}
    let(mut world,_)=control_contracts::fixture();world.insert_resource(out.clone());
    world.init_resource::<bevy::ecs::event::Events<bevy::app::AppExit>>();
    control(&mut world);
    let exits=world.resource_mut::<bevy::ecs::event::Events<bevy::app::AppExit>>().drain().collect::<Vec<_>>();
    assert!(matches!(exits.as_slice(),[bevy::app::AppExit::Error(_)]));
    assert!(Arc::ptr_eq(&out.context(7,0,1).unwrap(),&main));assert!(Arc::ptr_eq(&out.context(7,0,2).unwrap(),&next));
}
#[test]
fn phase_retirement_revokes_pins_and_inflight_ack_without_sequence_reuse() {
    let mut out=output();out.frame(packet(&scope(1)),None);write_cpu(&out);out.consume_frame(7,0,1).unwrap();
    out.frame(packet(&scope(2)),None);let(_,t)=take(&out);out.consume_frame(7,0,2).unwrap();let old=out.clone();
    out.retire_startup_phase().unwrap();old.finish_write(t,true);assert!(!out.consume_frame(7,0,2).unwrap());
    assert!(out.completed(7,0,1).is_none());assert!(out.queue.0.lock().unwrap().main_consumed.is_none());
    out.frame(packet(&scope(3)),None);write_cpu(&out);assert!(out.consume_frame(7,0,3).unwrap());
}
#[test]
fn epoch_retirement_keeps_only_bounded_neutral_release_tombstones() {
    let out=output();let frame=physical(scope(1),4);out.frame(packet(&frame.scope),Some(frame));write_cpu(&out);out.consume_frame(7,0,1).unwrap();
    out.contexts.lock().unwrap().clear();out.discard_foreign(7,1);
    assert!(out.context(7,0,1).is_none());assert!(out.retired(7,0,1));assert!(!out.consume_frame(7,0,1).unwrap());
    assert!(out.consume_frame(8,1,1).is_err());
}
#[test]
fn authenticated_session_offer_bypasses_exhausted_image_credit() {
    let out=output();out.frame(packet(&scope(1)),None);write_cpu(&out);
    {let mut q=out.queue.0.lock().unwrap();q.session_retired=true;q.main_consumed=None;q.awaiting_consumption=None;q.frame=Some(Packet::header(Header::SessionOffer{nonce:77,pid:std::process::id(),instance:7,next_instance:8}));}
    let(p,t)=take(&out);assert!(t.is_none());assert!(matches!(p.header,Header::SessionOffer{instance:7,next_instance:8,..}));
}
#[test]
fn actual_input_drain_precedes_ack_pin_retirement_and_does_not_forge_control_ack() {
    let(mut world,tx)=control_contracts::fixture();let first=control_contracts::image_context(&mut world,1);let second=control_contracts::image_context(&mut world,2);let out=world.resource::<Output>().clone();
    out.frame(packet(&first.scope),Some(first.clone()));write_cpu(&out);out.consume_frame(first.scope.instance,first.scope.epoch,1).unwrap();
    out.frame(packet(&second.scope),Some(second.clone()));write_cpu(&out);out.contexts.lock().unwrap().clear();
    tx.send(Header::Release{instance:first.scope.instance,epoch:first.scope.epoch,sequence:1,frame_sequence:1,release:wire::ImageRelease::FocusLost}).unwrap();
    tx.send(Header::FrameConsumed{instance:second.scope.instance,epoch:second.scope.epoch,frame_sequence:2}).unwrap();
    control(&mut world);assert_eq!(world.resource::<PreviewState>().received_sequence,0);assert!(out.context(first.scope.instance,first.scope.epoch,1).is_some());
    drain_input(&mut world);assert_eq!(world.resource::<PreviewState>().received_sequence,1);assert!(world.resource::<PreviewState>().control_error.is_none());
    drain_consumed(&mut world);assert!(out.context(first.scope.instance,first.scope.epoch,1).is_none());assert_eq!(world.resource::<PreviewState>().received_sequence,1);assert_eq!(world.resource::<PreviewState>().sequence,1);
}
#[test]
fn debugger_before_consumption_receipt_uses_exact_main_pin_before_opaque_json() {
    let(mut world,tx)=control_contracts::fixture();world.init_resource::<PendingInput>();let first=control_contracts::image_context(&mut world,1);let second=control_contracts::image_context(&mut world,2);let out=world.resource::<Output>().clone();
    out.frame(packet(&first.scope),Some(first.clone()));write_cpu(&out);out.consume_frame(first.scope.instance,first.scope.epoch,1).unwrap();
    out.frame(packet(&second.scope),Some(second.clone()));write_cpu(&out);
    // A bounded recent frontier is not the authority for the visible Main pin.
    out.queue.0.lock().unwrap().completed.clear();
    tx.send(Header::Debugger{instance:first.scope.instance,epoch:first.scope.epoch,sequence:1,frame_sequence:1,frame_control_sequence:first.scope.control_sequence,command:"invalid JSON stays opaque until its source is admitted".into()}).unwrap();
    tx.send(Header::FrameConsumed{instance:second.scope.instance,epoch:second.scope.epoch,frame_sequence:2}).unwrap();
    control(&mut world);assert_eq!(world.resource::<PreviewState>().received_sequence,1);assert_eq!(world.resource::<PreviewState>().sequence,0);
    let error=world.resource::<PreviewState>().control_error.clone().expect("the actual admitted JSON is invalid");assert!(!error.contains("expired"));
    assert_eq!(out.queue.0.lock().unwrap().main_consumed.as_ref().unwrap().scope.sequence,1);
    drain_input(&mut world);drain_consumed(&mut world);
    assert_eq!(out.queue.0.lock().unwrap().main_consumed.as_ref().unwrap().scope.sequence,2);assert_eq!(world.resource::<PreviewState>().control_error.as_ref(),Some(&error));assert_eq!(world.resource::<PreviewState>().received_sequence,1);
}
#[test]
fn actual_loopback_writer_waits_for_world_consumed_receipt_then_sends_only_latest_gpu_fixture() {
    use std::net::{TcpListener,Shutdown};use std::time::Instant;
    let(mut world,_tx)=control_contracts::fixture();let first=control_contracts::image_context(&mut world,1);let original=first.scope.clone();
    let listener=TcpListener::bind(("127.0.0.1",0)).unwrap();let options=Options{port:listener.local_addr().unwrap().port(),nonce:714,width:1,height:1};
    let(out,rx)=connect(&options,original.instance).unwrap();let(mut peer,_)=listener.accept().unwrap();peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    assert!(matches!(Packet::read(&mut peer).unwrap().header,Header::Hello{version:6,nonce:714,pid,instance} if pid==std::process::id()&&instance==original.instance));
    world.insert_resource(out.clone());world.insert_resource(Controls(Mutex::new(rx)));world.init_resource::<PendingInput>();
    out.frame(packet(&original),Some(first));assert!(matches!(Packet::read(&mut peer).unwrap().header,Header::Frame{scope,..} if scope==original));
    for n in 2..=1000{let mut next=original.clone();next.sequence=n;out.frame(packet(&next),None);}
    peer.set_read_timeout(Some(Duration::from_millis(50))).unwrap();let mut probe=[0u8;1];
    assert!(matches!(peer.peek(&mut probe),Err(e) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)),"socket receipt alone must not grant more image credit");
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    Packet::header(Header::FrameConsumed{instance:original.instance,epoch:original.epoch,frame_sequence:1}).write(&mut peer).unwrap();
    let end=Instant::now()+Duration::from_secs(3);
    while out.queue.0.lock().unwrap().main_consumed.is_none(){control(&mut world);drain_input(&mut world);drain_consumed(&mut world);assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(2));}
    assert!(matches!(Packet::read(&mut peer).unwrap().header,Header::Frame{scope,..} if scope.sequence==1000));
    assert_eq!(world.resource::<PreviewState>().received_sequence,0);assert_eq!(world.resource::<PreviewState>().sequence,0);
    out.failed();let _=peer.shutdown(Shutdown::Both);
}
