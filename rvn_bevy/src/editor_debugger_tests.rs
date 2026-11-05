use super::*;
use rvn_parser::Value;

fn world(source:&str)->World{
    let mut engine=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
    engine.enter_label("start").unwrap();engine.renderer.scene_batch.take();engine.renderer.take_pending();
    let mut world=World::new();world.insert_resource(VnEngine(engine));
    world.insert_resource(State::new(VnState::Stepping));world.init_resource::<NextState<VnState>>();
    world.init_resource::<crate::resources::ScriptErrorMessage>();world.init_resource::<Time<bevy::time::Virtual>>();
    world.init_resource::<crate::editor_preview::PreviewState>();world.init_resource::<bevy::input::ButtonInput<bevy::input::mouse::MouseButton>>();
    configure(&mut world,vec![],vec![]).unwrap();world
}
struct Frozen { visible:String, source:rvn_core::engine::DebugStateSource }
fn frozen(world:&World)->Frozen{
    let engine=&world.resource::<VnEngine>().0;
    Frozen{visible:format!("{:?}|{:?}|{:?}",engine.state,engine.history.entries(),engine.renderer.scene_batch),source:engine.debug_state_source().unwrap()}
}
fn assert_frozen(world:&World,before:&Frozen){
    let engine=&world.resource::<VnEngine>().0;
    assert!(engine.accepts_debug_state_source(&before.source),"actual opaque source identity, epoch, state and history changed");
    assert_eq!(frozen(world).visible,before.visible);
}
fn finish(world:&mut World){for _ in 0..256{drain(world);if !pending(world.resource::<VnEngine>()){return;}}panic!("real callback did not complete");}

#[test]
fn bevy_debugger_pause_keeps_the_armed_over_target_and_one_actual_publication_across_drains(){
    let source=format!("function heavy(){{local sum=0\n{}return sum}}\ninit{{set score=0}}\nscene label start\nset score=heavy()\n\"done\"\n","set sum=sum+1\n".repeat(4500));
    let mut world=world(&source);
    let before=frozen(&world);run(&mut world,StepKind::Into).unwrap();drain(&mut world);
    let function=snapshot(world.resource::<VnEngine>()).unwrap().site.unwrap().function;assert_eq!(function.as_deref(),Some("heavy"));
    run(&mut world,StepKind::Out).unwrap();drain(&mut world);
    let observed=encoded_snapshot(world.resource::<VnEngine>()).unwrap().unwrap();
    let engine=&world.resource::<VnEngine>().0;let observed=Snapshot::decode(&observed,engine.instance_id(),engine.runtime_epoch()).unwrap();
    assert!(observed.running);assert_eq!(observed.armed,Some(StepKind::Out));assert_frozen(&world,&before);
    world.resource_mut::<bevy::input::ButtonInput<bevy::input::mouse::MouseButton>>().press(bevy::input::mouse::MouseButton::Left);
    pause(&mut world).unwrap();assert!(!world.resource::<bevy::input::ButtonInput<bevy::input::mouse::MouseButton>>().pressed(bevy::input::mouse::MouseButton::Left));
    let stopped=snapshot(world.resource::<VnEngine>()).unwrap();for _ in 0..3{drain(&mut world);}
    assert_eq!(snapshot(world.resource::<VnEngine>()).unwrap().steps,stopped.steps);assert_frozen(&world,&before);
    assert!(run(&mut world,StepKind::Into).is_err());resume(&mut world).unwrap();finish(&mut world);
    assert_eq!(world.resource::<VnEngine>().0.state.vars["score"],Value::Int(4500));
    assert!(world.resource::<VnEngine>().0.renderer.scene_batch_pending());assert!(crate::editor_preview::paused(&world));
    let after=frozen(&world);drain(&mut world);assert_frozen(&world,&after);
}

#[test]
fn bevy_debugger_frame_queue_refuses_aggregate_owned_payload_and_restores_the_preproducer_state(){
    let mut world=world("screen form(){return component(\"button\",\"button\",{},[])}\ninit{set score=0 ui.open(\"form\",[],false,0)}\nscene label start\n\"done\"\n");
    for i in 0..4{world.resource_mut::<VnEngine>().0.state.vars.insert(format!("safe_{i}"),Value::Str("x".repeat(900_000)));}
    let before=world.resource::<VnEngine>().0.state.clone();let history=format!("{:?}",world.resource::<VnEngine>().0.history.entries());
    let retained=retained_payload_bytes(&before).unwrap();assert!(retained<MAX_QUEUE_BYTES);
    world.resource_mut::<VnEngine>().0.state.vars.insert("score".into(),Value::Int(1));
    let source=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
    let operation=DebugOperation::Input(rvn_core::ui_input::ScopedUiInput{source,event:rvn_core::ui::UiInput{screen:"form".into(),element:"button".into(),kind:rvn_ui::programmable::ScreenEventKind::Key,value:Some(Value::Str("y".repeat(600_000))),key:None}});
    // A receipt closure may retain data independently of the operation. Both
    // individually fit; the aggregate is refused before any evaluator begins.
    let result=enqueue_frame(&mut world,operation,before.clone(),retained,Box::new(|_,_|panic!("refused callback executed")));
    assert!(result.is_err());assert_eq!(format!("{:?}",world.resource::<VnEngine>().0.state),format!("{before:?}"));
    assert_eq!(format!("{:?}",world.resource::<VnEngine>().0.history.entries()),history);
    let mailbox=world.resource::<VnEngine>().0.renderer.debugger.lock().unwrap();assert!(mailbox.queue.is_empty());assert_eq!(mailbox.bytes,0);assert!(mailbox.active.is_none());
}

#[test]
fn bevy_debugger_host_callback_failure_keeps_the_original_warning_policy_and_no_story_mutation(){
    let source="handler failed(event){local missing=unknown_variable}\nscreen host(){return component(\"button\",\"button\",{\"events\":{\"click\":\"failed\"}},[])}\ninit{set score=0}\nscene label start\n\"done\"\n";
    let mut world=world(source);world.resource_mut::<VnEngine>().0.synchronize_source_menu(rvn_ui::PageRole::QuickActions,Some("host"),Value::Dict(Default::default()),false,100).unwrap();
    let engine=world.resource::<VnEngine>();let source=engine.0.capture_ui_input_source("host").unwrap();assert!(source.host_role.is_some());
    let input=rvn_core::ui_input::ScopedUiInput{source,event:rvn_core::ui::UiInput{screen:"host".into(),element:"button".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}};
    let before=frozen(&world);assert!(input_engine(&mut world,input));finish(&mut world);
    assert_frozen(&world,&before);assert!(!world.resource::<crate::resources::ScriptErrorMessage>().0.is_empty());
    assert!(!matches!(*world.resource::<NextState<VnState>>(),NextState::Pending(VnState::Error)));
    assert!(world.resource::<VnEngine>().0.renderer.debugger.lock().unwrap().error.is_none());
}
fn input_engine(world:&mut World,event:rvn_core::ui_input::ScopedUiInput)->bool{input(world.resource::<VnEngine>(),event).unwrap()}

#[test]
fn bevy_debugger_replaced_engine_drops_the_old_callback_without_acknowledging_its_source(){
    let mut world=world("init{set score=0}\nscene label start\nset score=score+1\n\"done\"\n");
    let completed=std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));let count=completed.clone();
    enqueue(&mut world,DebugOperation::Statement,Box::new(move|_,_|{count.fetch_add(1,std::sync::atomic::Ordering::SeqCst);Ok(())})).unwrap();
    let shared=world.resource::<VnEngine>().0.renderer.debugger.clone();let old=world.resource::<VnEngine>().0.instance_id();
    let mut replacement=rvn_core::Engine::from_program(rvn_parser::parse_program("init{set score=99}\nscene label start\n\"new\"\n").unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
    replacement.enter_label("start").unwrap();replacement.renderer.scene_batch.take();replacement.renderer.debugger=shared;
    assert_ne!(replacement.instance_id(),old);world.insert_resource(VnEngine(replacement));let before=frozen(&world);
    drain(&mut world);assert_frozen(&world,&before);assert_eq!(completed.load(std::sync::atomic::Ordering::SeqCst),0);
    assert!(!pending(world.resource::<VnEngine>()));
}

#[cfg(all(feature="video",not(target_arch="wasm32")))]
#[test]
fn bevy_debugger_running_and_stopped_pause_actual_pcm_and_video_cursors_without_overwriting_residence(){
    use bevy::audio::Decodable;
    let script=format!("function heavy(){{local sum=0\n{}return sum}}\ninit{{set score=0}}\nscene label start\nset score=heavy()\n\"done\"\n","set sum=sum+1\n".repeat(4500));
    let mut world=world(&script);
    let gate=world.resource::<VnEngine>().0.renderer.audio_checkpoints.gate();
    let clip=crate::checkpoint_pcm::PcmClip::decode("sfx/exact.wav",&crate::checkpoint_sources::fixture_wave_for_test()).unwrap();
    let current=clip.start_looped(gate.clone(),false);let parked=clip.start(gate.clone(),true);
    let mut current_output=current.decoder();let mut parked_output=parked.decoder();
    assert_eq!(current_output.next(),Some(100));
    let video=crate::video_audio::VideoAudio(std::sync::Arc::new(std::sync::Mutex::new(crate::video_audio::Buffer::new(0.5))));
    {let mut buffer=video.0.lock().unwrap();buffer.push(0.5,vec![0.2,0.4,0.3,0.5]).unwrap();buffer.running=true;}
    let mut video_output=video.decoder();assert_eq!(video_output.next(),Some(0.2));assert_eq!(video_output.next(),Some(0.4));
    let video_position=video.0.lock().unwrap().position;
    world.init_resource::<Assets<crate::video_audio::VideoAudio>>();
    let _handle=world.resource_mut::<Assets<crate::video_audio::VideoAudio>>().add(video.clone());
    let before=frozen(&world);let audio_before=gate.snapshot_with_status(&[&current,&parked],Ok).unwrap();
    run(&mut world,StepKind::Into).unwrap();drain(&mut world);
    run(&mut world,StepKind::Out).unwrap();drain(&mut world);
    assert!(snapshot(world.resource::<VnEngine>()).unwrap().running);
    assert!(!crate::scene_publication::game_active(&world));assert!(!crate::scene_publication::host_active(&world));
    assert!(world.resource::<Time<bevy::time::Virtual>>().is_paused());
    assert!(!video.0.lock().unwrap().running);
    for _ in 0..101 {assert_eq!(current_output.next(),Some(0));assert_eq!(parked_output.next(),Some(0));assert_eq!(video_output.next(),Some(0.0));}
    assert_eq!(gate.snapshot_with_status(&[&current,&parked],Ok).unwrap(),audio_before);
    assert_eq!(video.0.lock().unwrap().position,video_position);assert_frozen(&world,&before);
    // A new streaming buffer starts stopped, and a newly published exact PCM
    // reader sees the existing global gate without changing its own residence.
    let new=clip.start(gate.clone(),false);let mut new_output=new.decoder();assert_eq!(new_output.next(),Some(0));
    assert!(!crate::video_audio::Buffer::new(2.0).running);
    pause(&mut world).unwrap();for _ in 0..3{drain(&mut world);}
    assert_frozen(&world,&before);assert_eq!(gate.snapshot_with_status(&[&current,&parked],Ok).unwrap(),audio_before);
    resume(&mut world).unwrap();finish(&mut world);
    assert!(crate::editor_preview::paused(&world)); // Out commits once and remains stopped.
    run(&mut world,StepKind::Continue).unwrap();finish(&mut world);
    assert!(!crate::editor_preview::paused(&world));assert!(!world.resource::<Time<bevy::time::Virtual>>().is_paused());
    // An odd silent pull count needs one explicit left alignment sample.
    assert_eq!(current_output.next(),Some(0));assert_eq!(current_output.next(),Some(200));assert_eq!(parked_output.next(),Some(0));
    assert!(gate.snapshot_with_status(&[&parked],Ok).unwrap()[0].1);
    assert!(!video.0.lock().unwrap().running,"Only the actual owner-aware video tick may resume playback");
    assert_eq!(video.0.lock().unwrap().position,video_position);
}

#[test]
fn bevy_debugger_failed_receipt_is_visible_terminal_and_never_acknowledged_twice(){
    let mut world=world("init{set score=0}\nscene label start\nset score=score+1\n\"done\"\n");
    let called=std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));let count=called.clone();
    enqueue(&mut world,DebugOperation::Statement,Box::new(move|_,_|{
        count.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
        Err("injected receipt publication failure".into())
    })).unwrap();
    run(&mut world,StepKind::Continue).unwrap();finish(&mut world);
    assert_eq!(called.load(std::sync::atomic::Ordering::SeqCst),1);
    assert_eq!(world.resource::<VnEngine>().0.state.vars["score"],Value::Int(1));
    assert!(matches!(*world.resource::<NextState<VnState>>(),NextState::Pending(VnState::Error)));
    let engine=world.resource::<VnEngine>();
    let encoded=encoded_snapshot(engine).unwrap().unwrap();
    let observed=Snapshot::decode(&encoded,engine.0.instance_id(),engine.0.runtime_epoch()).unwrap();
    assert_eq!(observed.error.as_deref(),Some("injected receipt publication failure"));
    assert!(!observed.running);assert!(crate::editor_preview::paused(&world));
    let committed=frozen(&world);
    assert!(run(&mut world,StepKind::Continue).is_err());
    assert!(configure(&mut world,vec![],vec![]).is_err());
    assert!(enqueue(&mut world,DebugOperation::Statement,Box::new(|_,_|panic!("terminal operation accepted"))).is_err());
    cancel(&mut world).unwrap(); // Same path as Command::Disable.
    assert!(crate::editor_preview::paused(&world));
    assert!(configure(&mut world,vec![],vec![]).is_err());
    assert!(run(&mut world,StepKind::Continue).is_err());
    let engine=world.resource::<VnEngine>();
    let observed=Snapshot::decode(&encoded_snapshot(engine).unwrap().unwrap(),engine.0.instance_id(),engine.0.runtime_epoch()).unwrap();
    assert_eq!(observed.error.as_deref(),Some("injected receipt publication failure"));
    assert!(!observed.running);
    for _ in 0..3{drain(&mut world);}
    assert_frozen(&world,&committed);assert_eq!(called.load(std::sync::atomic::Ordering::SeqCst),1);
}
