//! Real Legacy Screen[] materialization entry. Empty child/callbacks are not
//! an absent evaluator: the existing description path requests screen:panel.
//! CPU/Same VM only; no Native metadata, paint, physical frame or Ready setter.
use super::*;
use rvn_core::engine::{DebugOperation,DebugOutcome,DebugProgress,DebugSession};
use rvn_core::editor_debugger::StepKind;
type Game=rvn_core::Engine<crate::bevy_renderer::BevyRenderer>;
type Save=rvn_core::save::consumer_checkpoint::Save15;
const EMPTY_LEGACY:&str=r#"
screen panel(value){return []}
init{set empty_entry_draw=random(0,1000000)}
label start
"Ready"
return
"#;
#[inline(never)]
fn game(project:&Project)->Box<Game>{
    // Real production class admission/StartupOwner and same Init/Engine. Keep
    // big transports boxed as the loader does; never enlarge the 1MiB stack.
    let launch=crate::load_runtime_game(&project.root).unwrap();
    let crate::RuntimeLaunchOwner::Startup(mut startup)=launch.engine else{
        panic!("actual admitted Widget class requires its real startup owner");
    };
    let lineage=startup.owner.snapshot().instance;let epoch=startup.owner.snapshot().epoch;
    let gate=startup.broker.gate();
    assert!(!startup.owner.state().vars.contains_key("empty_entry_draw"));
    for _ in 0..256{if startup.owner.is_complete(){break;}startup.owner.step(StepKind::Continue).unwrap();}
    assert!(startup.owner.is_complete());
    let label=startup.start_label.take();let mut game=Box::new(startup.owner.into_engine().unwrap());
    assert_eq!(game.instance_id(),lineage);assert_eq!(game.runtime_epoch(),epoch);
    assert!(game.renderer.audio_checkpoints.gate().same_gate(&gate));
    if game.renderer.scene_batch_pending(){handoff_zero_media_publication(&mut game);}
    if let Some(label)=label{game.enter_label(&label).unwrap();}
    game
}
fn save(game:&mut Game,project:&Project,session:Option<&DebugSession>)->Save{
    handoff_zero_media_publication(game);
    let wire=game.capture_save15(&project.program(),session,16,"real Legacy empty owner phase entry".into()).unwrap().encode().unwrap();
    Save::decode(&wire).unwrap()
}
fn actual_legacy_owner(game:&Game,project:&Project)->rvn_core::widget_backend_0956::WidgetBackendOwner{
    let descriptor=WidgetBlueprintV1::decode(&fs::read(project.root.join("assets/panel.rvnwidget")).unwrap()).unwrap();
    let class=game.widget_classes().class(UUID).unwrap();assert_eq!(class.descriptor(),&descriptor);
    assert_eq!(descriptor.members,vec![descriptor.screen_graph]);
    assert_eq!(class.identity().uuid,UUID);assert_eq!(class.identity().source,"main.rvn");assert_eq!(class.identity().definition,"panel");
    let instance=game.state.ui.screens.iter().find(|instance|instance.name=="one").unwrap();
    assert_eq!(&instance.widget_class.as_ref().unwrap().class,class.identity());
    assert!(instance.owner_properties.events.is_empty(),"there are no Initialized/PreConstruct/Construct/Destruct callbacks");
    assert!(instance.consumer_projection.is_none()&&instance.consumer_frames.is_empty(),"ordinary Legacy Screen keeps no invented Native projection/frame");
    let handle=game.widget_instance_handle("one").unwrap();game.widget_backend_owner(&handle).unwrap()
}
fn pending(game:&Game,project:&Project){
    let owner=actual_legacy_owner(game,project);
    assert!(game.has_pending_widget_materialization());
    assert!(game.capture_ui_input_source("one").is_none());
    assert!(game.renderer.widget_backend.wrapper(&owner).is_none());
}
fn actual_entry(game:&Game,project:&Project,session:&DebugSession){
    pending(game,project);assert!(game.accepts_debug_session(session));
    let snapshot=session.snapshot();assert!(snapshot.evaluating&&!snapshot.running);assert!(snapshot.error.is_none());
    // Even without Native plans/callbacks, prepare_owner_properties evaluates
    // the real Legacy return[] under this actual scope; do not fabricate None.
    assert!(snapshot.frames.iter().any(|frame|frame.function.as_deref()==Some("screen:panel")));
    assert_eq!((snapshot.steps,snapshot.value_work),(0,0),"this is before executing the actual pending Return expression");
    let DebugOperation::MaterializeWidget(operation)=session.operation()else{panic!("wrong actual operation");};
    assert_eq!(operation.owner(),&actual_legacy_owner(game,project));
}
fn actual_wire(save:&Save){
    let wire:serde_json::Value=serde_json::from_str(&save.encode().unwrap()).unwrap();
    assert!(wire["debugger"]["trace"].is_object(),"Legacy return[] is a real pending computation too");
    let owner=&wire["debugger"]["widget_materialization"];assert!(owner.is_object());
    assert_eq!(owner["class"]["uuid"].as_str(),Some(UUID));assert_eq!(owner["class"]["definition"].as_str(),Some("panel"));
    assert!(owner["methods"].as_array().unwrap().is_empty());
    for physical in ["generation","prepared","object","ready"]{assert!(owner.get(physical).is_none());}
}
#[test]
fn real_legacy_empty_owner_entry_keeps_its_actual_screen_computation_and_two_save15_reloads_without_ready_or_rng_replay_one_mib(){
    let project=project_with_source(EMPTY_LEGACY);
    let temp = std::env::temp_dir().canonicalize().unwrap();
    let project_root = project.root.canonicalize().unwrap();
    assert!(matches!(temp.components().next(), Some(std::path::Component::Prefix(prefix))
        if matches!(prefix.kind(), std::path::Prefix::Disk(drive) | std::path::Prefix::VerbatimDisk(drive)
            if drive.to_ascii_uppercase() == b'D')), "runner must provide real isolated D TEMP");
    assert!(project_root != temp && project_root.starts_with(&temp),
        "actual project must remain a descendant of the runner's canonical isolated TEMP");
    assert_eq!(fs::read_to_string(project.root.join("main.rvn")).unwrap(),EMPTY_LEGACY);
    let parsed=project.program();
    assert!(!parsed.script.iter().any(|statement|matches!(statement,rvn_parser::Statement::Handler{..}|rvn_parser::Statement::Function{..}|rvn_parser::Statement::ScheduledConsumer{..})));
    let body=parsed.script.iter().find_map(|statement|if let rvn_parser::Statement::Screen{name,body,..}=statement{(name=="panel").then_some(body)}else{None}).unwrap();
    assert!(matches!(body.as_slice(),[rvn_parser::Statement::FunctionReturn{value:rvn_parser::Expr::ListLit(items)}]if items.is_empty()));
    let workspace=rvn_graph::SourceWorkspace::open(&project.root).unwrap().unwrap();
    assert!(workspace.source_error.is_none());assert!(workspace.project().graphs().iter().all(|graph|graph.inspector_evaluation.is_legacy()));
    let mut warm=game(&project);warm.step_until_interaction().unwrap();
    let handle=warm.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();
    let owner=actual_legacy_owner(&warm,&project);assert!(warm.renderer.widget_backend.wrapper(&owner).is_none());
    warm.add_widget_to_viewport(&handle,false,0).unwrap();
    let original=warm.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();
    assert!(original.attached());assert_eq!(original.owner(),&owner);assert!(warm.interface_views().unwrap().is_empty());
    let vars=warm.state.vars.clone();let rng=warm.state.random;
    let screen_rng=warm.state.ui.screens.iter().find(|instance|instance.name=="one").unwrap().random;
    let checkpoint=save(&mut warm,&project,None);
    let mut cold=game(&project);assert!(cold.restore_save15(&project.program(),checkpoint).unwrap().is_none());
    pending(&cold,&project);assert_eq!(cold.state.vars,vars);assert_eq!(cold.state.random,rng);
    let operation=cold.next_widget_materialization().unwrap().unwrap();let prepared=operation.object().unwrap().clone();
    let before=serde_json::to_value(&cold.state).unwrap();
    let session=cold.debug_begin(DebugOperation::MaterializeWidget(operation),vec![]).unwrap();
    actual_entry(&cold,&project,&session);assert_eq!(serde_json::to_value(&cold.state).unwrap(),before);
    let checkpoint=save(&mut cold,&project,Some(&session));actual_wire(&checkpoint);
    let mut reopened=game(&project);let mut restored=reopened.restore_save15(&project.program(),checkpoint).unwrap().unwrap();
    actual_entry(&reopened,&project,&restored);assert!(reopened.widget_materialization_waits_for_saved_operation());assert!(reopened.next_widget_materialization().unwrap().is_none());
    let checkpoint=save(&mut reopened,&project,Some(&restored));actual_wire(&checkpoint);
    let mut twice=game(&project);let mut restored_twice=twice.restore_save15(&project.program(),checkpoint).unwrap().unwrap();
    actual_entry(&twice,&project,&restored_twice);
    for (game,session) in [(&mut reopened,&mut restored),(&mut twice,&mut restored_twice)]{
        pending(game,&project);assert_eq!(game.state.vars,vars);assert_eq!(game.state.random,rng);
        assert_eq!(game.state.ui.screens.iter().find(|instance|instance.name=="one").unwrap().random,screen_rng);
        let current=actual_legacy_owner(game,&project);assert_eq!(current.class(),owner.class());assert_eq!(current.order(),owner.order());assert_eq!(current.target(),owner.target());
        assert_ne!(current,owner);assert!(game.widget_backend_owner(&handle).is_err(),"old actual Engine handle never becomes a cold input grant");
        let mut complete=false;
        for _ in 0..256{
            let (progress,outcome)=game.debug_step(session,StepKind::Continue).unwrap();
            if progress==DebugProgress::Complete{assert!(matches!(outcome,Some(DebugOutcome::WidgetMaterialized)));complete=true;break;}
        }
        assert!(complete);assert!(!game.has_pending_widget_materialization());assert!(!game.widget_materialization_waits_for_saved_operation());
        let exact=game.renderer.widget_backend.wrapper(&current).unwrap().upgrade().unwrap();
        assert!(exact.attached());assert_eq!(exact.owner(),&current);assert_ne!(exact.generation(),original.generation());assert_ne!(exact.generation(),prepared.generation);
        assert!(game.capture_ui_input_source("one").is_some());assert!(game.interface_views().unwrap().is_empty());
        assert_eq!(game.state.vars,vars);assert_eq!(game.state.random,rng);
        assert_eq!(game.state.ui.screens.iter().find(|instance|instance.name=="one").unwrap().random,screen_rng);
        let committed=serde_json::to_value(&game.state).unwrap();assert!(game.debug_step(session,StepKind::Continue).is_err());assert_eq!(serde_json::to_value(&game.state).unwrap(),committed);
        assert!(game.next_widget_materialization().unwrap().is_none());assert!(std::sync::Arc::ptr_eq(&exact,&game.renderer.widget_backend.wrapper(&current).unwrap().upgrade().unwrap()));
    }
    assert!(original.attached(),"cold restore does not retire another actual Engine's wrapper");
}
