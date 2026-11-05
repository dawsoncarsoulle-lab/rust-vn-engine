//! CPU transport/publication contracts. The socket case uses real loopback,
//! explicit CPU pixels and a controlled writer cadence; no GPU/native credit.
use super::*;
use std::io::{self,Write};

fn output()->Output{
    Output{phase:0,queue:Arc::new((Mutex::new(Latest::default()),Condvar::new())),alive:Arc::new(AtomicBool::new(true)),contexts:Default::default(),held:Default::default()}
}
fn scope(sequence:u64)->wire::FrameScope{
    wire::FrameScope{instance:7,epoch:0,sequence,control_sequence:0,control_ack:0,control_error:None,
        paused:true,publication_pending:false,finished:Some(false),step_allowed:true,interaction:None,
        position:rvn_core::scene::ResumePosition{pc:0,origin:None},active:None,parked:vec![]}
}
fn packet(scope:&wire::FrameScope)->Packet{
    Packet{header:Header::Frame{scope:scope.clone(),width:1,height:1,debugger:None,inspection:None},pixels:vec![31,63,127,255]}
}
fn physical(scope:wire::FrameScope)->Arc<crate::editor_preview_input::PhysicalFrame>{
    let game=rvn_core::Engine::new(rvn_parser::parse("set value=1\n\"Ready\"\n").unwrap(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();
    let mut world=World::new();world.insert_resource(crate::resources::VnEngine(game));
    world.init_resource::<crate::programmable_ui::Screens>();world.init_resource::<bevy::ui::UiStack>();
    crate::editor_preview_input::capture(&world,scope).unwrap()
}
fn startup_fixture()->(App,Output,wire::FrameScope,Arc<rvn_core::engine::StartupObservationSource>){
    let(_,mut app)=crate::debugger_startup::tests::fixture();
    app.init_resource::<PreviewState>();
    assert!(startup::capture(app.world_mut()));
    let scope=app.world().resource::<RenderScope>().0.clone();
    let source=app.world().resource::<RenderStartupSource>().0.clone().unwrap();
    let out=output();app.insert_resource(out.clone());(app,out,scope,source)
}
fn take(out:&Output)->(Packet,Option<WriteToken>){
    let mut q=out.queue.0.lock().unwrap();out.take_for_write(&mut q).unwrap()
}

#[test]
fn unpublished_gpu_burst_never_evicts_real_published_scope_source_or_physical_context(){
    let(_,out,first,source)=startup_fixture();let context=physical(first.clone());
    out.frame_with_startup(packet(&first),Some(context.clone()),Some(source.clone()));out.publish_and_consume_cpu_fixture();
    for n in 1..=100{let mut next=first.clone();next.sequence+=n;out.frame_with_startup(packet(&next),Some(physical(next)),Some(source.clone()));}
    assert_eq!(out.completed(first.instance,first.epoch,first.sequence),Some(first.clone()));
    assert!(Arc::ptr_eq(&out.completed_startup_source(first.instance,first.epoch,first.sequence).unwrap(),&source));
    assert!(Arc::ptr_eq(&out.context(first.instance,first.epoch,first.sequence).unwrap(),&context));
    assert!(out.completed(first.instance,first.epoch,first.sequence+100).is_none());
    assert_eq!(out.queue.0.lock().unwrap().completed.len(),1);
}

#[test]
fn writing_pin_survives_burst_and_only_exact_success_moves_it_to_published(){
    let out=output();let first=scope(1);let context=physical(first.clone());
    out.frame(packet(&first),Some(context.clone()));let(p,token)=take(&out);
    for n in 2..=40{out.frame(packet(&scope(n)),None);}
    assert_eq!(out.completed(7,0,1),Some(first));assert!(Arc::ptr_eq(&out.context(7,0,1).unwrap(),&context));
    assert!(out.queue.0.lock().unwrap().completed.is_empty());
    // A foreign finish cannot release or publish the current writer lease.
    out.finish_write(Some(WriteToken{sequence:99,..token.unwrap()}),true);
    assert!(out.queue.0.lock().unwrap().writing.is_some());
    p.write(&mut Vec::new()).unwrap();out.finish_write(token,true);
    let q=out.queue.0.lock().unwrap();assert!(q.writing.is_none());assert_eq!(q.completed.len(),1);assert_eq!(q.completed[0].sequence,1);
}

struct PartialFailure{remaining:usize}
impl Write for PartialFailure{
    fn write(&mut self,bytes:&[u8])->io::Result<usize>{if self.remaining==0{return Err(io::Error::new(io::ErrorKind::BrokenPipe,"intentional partial socket fixture"));}let n=bytes.len().min(self.remaining);self.remaining-=n;Ok(n)}
    fn flush(&mut self)->io::Result<()>{Ok(())}
}
#[test]
fn partial_write_error_never_enters_published_or_retains_source_and_geometry(){
    let(_,out,first,source)=startup_fixture();let context=physical(first.clone());let weak=Arc::downgrade(&context);let weak_source=Arc::downgrade(&source);
    out.frame_with_startup(packet(&first),Some(context),Some(source));let(p,token)=take(&out);
    let result=p.write(&mut PartialFailure{remaining:5});assert!(result.is_err());out.finish_write(token,result.is_ok());
    assert!(out.completed(first.instance,first.epoch,first.sequence).is_none());assert!(out.context(first.instance,first.epoch,first.sequence).is_none());
    assert!(weak.upgrade().is_none());
    // Other genuine startup owners may still own source internals; this Arc is
    // retained only by Output during the exact queued/writing lifetime.
    assert!(weak_source.upgrade().is_none());
}

#[test]
fn eight_successful_publications_share_one_scope_and_startup_source_frontier(){
    let(_,out,first,source)=startup_fixture();
    for n in 0..=8{let mut next=first.clone();next.sequence+=n;out.frame_with_startup(packet(&next),None,Some(source.clone()));out.publish_and_consume_cpu_fixture();}
    assert!(out.completed(first.instance,first.epoch,first.sequence).is_none());assert!(out.completed_startup_source(first.instance,first.epoch,first.sequence).is_none());
    for n in 1..=8{assert!(out.completed(first.instance,first.epoch,first.sequence+n).is_some());assert!(out.completed_startup_source(first.instance,first.epoch,first.sequence+n).is_some());}
    let q=out.queue.0.lock().unwrap();assert_eq!(q.completed.len(),8);assert_eq!(q.startup_sources.len(),8);
}

#[test]
fn phase_retirement_revokes_inflight_source_and_delayed_gpu_and_finish(){
    let(_,mut out,first,source)=startup_fixture();let old=out.clone();
    out.frame_with_startup(packet(&first),Some(physical(first.clone())),Some(source.clone()));let(_,token)=take(&out);
    out.retire_startup_phase().unwrap();assert!(out.completed(first.instance,first.epoch,first.sequence).is_none());
    old.finish_write(token,true);let mut late=first.clone();late.sequence+=100;old.frame_with_startup(packet(&late),None,Some(source));
    let q=out.queue.0.lock().unwrap();assert!(q.frame.is_none());assert!(q.completed.is_empty());assert!(q.startup_sources.is_empty());assert!(q.writing.is_none());
    drop(q);
    // The SAME authenticated socket writer continues after handoff; its
    // extraction clone has the old phase but a newly taken lease is current.
    late.sequence+=1;out.frame(packet(&late),None);let(p,token)=take(&old);p.write(&mut Vec::new()).unwrap();old.finish_write(token,true);
    assert!(out.completed(late.instance,late.epoch,late.sequence).is_some());
}

#[test]
fn session_retirement_revokes_writer_while_preserving_the_header_only_offer(){
    let out=output();out.frame(packet(&scope(1)),Some(physical(scope(1))));let(_,token)=take(&out);
    {let mut q=out.queue.0.lock().unwrap();q.session_retired=true;q.queued_completion=None;q.frame=Some(Packet::header(Header::SessionOffer{nonce:19,pid:std::process::id(),instance:7,next_instance:8}));}
    assert!(out.completed(7,0,1).is_none());assert!(out.context(7,0,1).is_none());out.finish_write(token,true);
    let(offer,token)=take(&out);assert!(token.is_none());assert!(matches!(offer.header,Header::SessionOffer{instance:7,next_instance:8,..}));assert!(out.queue.0.lock().unwrap().completed.is_empty());
}

#[test]
fn restored_epoch_revokes_all_metadata_and_delayed_same_phase_gpu_completion(){
    let(_,out,first,source)=startup_fixture();let old_context=physical(first.clone());let weak=Arc::downgrade(&old_context);
    out.frame_with_startup(packet(&first),Some(old_context),Some(source.clone()));let(_,token)=take(&out);
    let mut queued=first.clone();queued.sequence+=1;out.frame_with_startup(packet(&queued),Some(physical(queued.clone())),Some(source));
    out.discard_foreign(first.instance,first.epoch+1);assert!(weak.upgrade().is_none());
    assert!(out.completed(first.instance,first.epoch,first.sequence).is_none());out.finish_write(token,true);
    queued.sequence+=50;out.frame(packet(&queued),Some(physical(queued.clone())));
    let q=out.queue.0.lock().unwrap();assert!(q.frame.is_none());assert!(q.writing.is_none());assert!(q.completed.is_empty());assert!(q.startup_sources.is_empty());drop(q);
    queued.epoch+=1;out.frame(packet(&queued),Some(physical(queued.clone())));out.publish_and_consume_cpu_fixture();assert!(out.completed(queued.instance,queued.epoch,queued.sequence).is_some());
}

#[test]
fn physical_union_budget_counts_allocations_overflow_and_failed_hold_without_mutation(){
    let out=output();
    for n in 1..=8{let mut frame=physical(scope(n));Arc::get_mut(&mut frame).unwrap().cost=2*1024*1024;out.hold(frame,&wire::ImageInput::Key{key:format!("Key{n}"),logical:None,pressed:true,modifiers:[false;4]}).unwrap();}
    // A positive conservative fixture cost must exceed the already-held exact16MiB.
    let mut frame=physical(scope(9));Arc::get_mut(&mut frame).unwrap().cost=1;out.frame(packet(&frame.scope),Some(frame.clone()));assert!(out.queue.0.lock().unwrap().frame.is_none());
    let before=out.held.lock().unwrap().len();assert!(out.hold(frame,&wire::ImageInput::Key{key:"ninth".into(),logical:None,pressed:true,modifiers:[false;4]}).is_err());assert_eq!(out.held.lock().unwrap().len(),before);
    assert!(!out.alive.load(Ordering::Acquire));
    assert!(out.queue.0.lock().unwrap().failure_reason.as_deref().unwrap().contains("16 MiB"));
    // Fatal overload cannot be reused. A fresh Output with the same exact
    // held16MiB exercises the zero-cost boundary independently.
    let boundary=output();
    for n in 1..=8{let mut frame=physical(scope(n));Arc::get_mut(&mut frame).unwrap().cost=2*1024*1024;boundary.hold(frame,&wire::ImageInput::Key{key:format!("Key{n}"),logical:None,pressed:true,modifiers:[false;4]}).unwrap();}
    let mut zero=physical(scope(10));Arc::get_mut(&mut zero).unwrap().cost=0;boundary.frame(packet(&zero.scope),Some(zero));
    assert!(boundary.alive.load(Ordering::Acquire));
    assert!(matches!(&boundary.queue.0.lock().unwrap().frame.as_ref().unwrap().header,Header::Frame{scope,..}if scope.sequence==10));
    let mut a=physical(scope(50));Arc::get_mut(&mut a).unwrap().cost=usize::MAX;let mut b=physical(scope(50));Arc::get_mut(&mut b).unwrap().cost=1;
    assert_eq!(Output::context_cost_all(&Default::default(),&Default::default(),Some(&a),Some(&a),None,None),usize::MAX);
    assert_eq!(Output::context_cost_all(&Default::default(),&Default::default(),Some(&a),Some(&b),None,None),usize::MAX);
    Arc::get_mut(&mut a).unwrap().cost=40;Arc::get_mut(&mut b).unwrap().cost=60;
    assert_eq!(Output::context_cost_all(&Default::default(),&Default::default(),Some(&a),Some(&a),None,None),40);
    assert_eq!(Output::context_cost_all(&Default::default(),&Default::default(),Some(&a),Some(&b),None,None),100);
}

#[test]
fn publications_without_geometry_retire_recent_contexts_but_keep_genuine_held_gesture(){
    let out=output();let context=physical(scope(1));out.frame(packet(&context.scope),Some(context.clone()));out.publish_and_consume_cpu_fixture();
    let input=wire::ImageInput::Key{key:"KeyA".into(),logical:None,pressed:true,modifiers:[false;4]};out.hold(context.clone(),&input).unwrap();
    for n in 2..=10{out.frame(packet(&scope(n)),None);out.publish_and_consume_cpu_fixture();}
    assert!(out.completed(7,0,1).is_none());assert!(out.contexts.lock().unwrap().is_empty());assert!(out.context(7,0,1).is_some());
    out.hold(context,&wire::ImageInput::Key{key:"KeyA".into(),logical:None,pressed:false,modifiers:[false;4]}).unwrap();assert!(out.context(7,0,1).is_none());
}

struct PausedSocket{stream:TcpStream,entered:Option<mpsc::SyncSender<()>>,proceed:mpsc::Receiver<()>}
impl Write for PausedSocket{
    fn write(&mut self,bytes:&[u8])->io::Result<usize>{if let Some(entered)=self.entered.take(){entered.send(()).unwrap();self.proceed.recv_timeout(Duration::from_secs(3)).unwrap();}self.stream.write(bytes)}
    fn flush(&mut self)->io::Result<()>{self.stream.flush()}
}
#[test]
fn real_socket_slow_writer_preserves_the_exact_completed_startup_source_during_gpu_burst(){
    use std::net::TcpListener;
    let(_,out,first,source)=startup_fixture();let listener=TcpListener::bind(("127.0.0.1",0)).unwrap();let client=TcpStream::connect(listener.local_addr().unwrap()).unwrap();let(mut peer,_)=listener.accept().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();client.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    out.frame_with_startup(packet(&first),None,Some(source.clone()));let(p,token)=take(&out);
    let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(proceed_tx,proceed_rx)=mpsc::sync_channel(1);let writer=out.clone();
    let thread=std::thread::spawn(move||{let mut stream=PausedSocket{stream:client,entered:Some(entered_tx),proceed:proceed_rx};let result=p.write(&mut stream);writer.finish_write(token,result.is_ok());result});
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    for n in 1..=64{let mut next=first.clone();next.sequence+=n;out.frame_with_startup(packet(&next),None,Some(source.clone()));}
    assert!(Arc::ptr_eq(&out.completed_startup_source(first.instance,first.epoch,first.sequence).unwrap(),&source));proceed_tx.send(()).unwrap();
    let delivered=Packet::read(&mut peer).unwrap();assert!(matches!(delivered.header,Header::Frame{scope:seen,..}if seen==first));assert_eq!(delivered.pixels,vec![31,63,127,255]);thread.join().unwrap().unwrap();
    assert!(Arc::ptr_eq(&out.completed_startup_source(first.instance,first.epoch,first.sequence).unwrap(),&source));assert_eq!(out.queue.0.lock().unwrap().completed.len(),1);
}

#[test]
fn retained_actual_startup_image_accepts_sites_but_changed_continuation_precedes_json(){
    let(mut app,out,first,source)=startup_fixture();out.frame_with_startup(packet(&first),None,Some(source));out.publish_and_consume_cpu_fixture();
    for n in 1..=40{let mut next=first.clone();next.sequence+=n;out.frame(packet(&next),None);}
    let command=rvn_core::editor_debugger::Command::Sites{offset:0}.encode().unwrap();
    let header=|sequence,command|Header::Debugger{instance:first.instance,epoch:first.epoch,sequence,frame_sequence:first.sequence,frame_control_sequence:first.control_sequence,command};
    assert!(startup::control(app.world_mut(),&header(1,command)));assert!(app.world().resource::<PreviewState>().control_error.is_none());
    // Sites is read-only, but accepting the command advances the transport
    // ordinal. Publish the actual next image before testing continuation CAS.
    assert!(startup::capture(app.world_mut()));let mut next=app.world().resource::<RenderScope>().0.clone();next.sequence+=100;
    out.frame_with_startup(packet(&next),None,app.world().resource::<RenderStartupSource>().0.clone());out.publish_and_consume_cpu_fixture();
    let plan=crate::debugger_startup::prepare_world_step(app.world_mut(),rvn_core::editor_debugger::StepKind::Into).unwrap();let committed=crate::debugger_startup::commit_world_step(app.world_mut(),plan).unwrap();crate::debugger_startup::install_committed_world_step(app.world_mut(),committed);
    let stale=Header::Debugger{instance:next.instance,epoch:next.epoch,sequence:2,frame_sequence:next.sequence,frame_control_sequence:next.control_sequence,command:"{must remain undecoded".into()};
    assert!(startup::control(app.world_mut(),&stale));assert!(app.world().resource::<PreviewState>().control_error.as_ref().unwrap().contains("continuation changed"));
}
