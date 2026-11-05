//! Real native Graph -> loader -> Save15 -> owned VM/driver regression cases.
//! Prepared only. No native screen/paint result is claimed by these CPU cases.
use super::*;
use rvn_core::engine::{DebugOperation,DebugSession,DebugProgress,DebugOutcome};
use rvn_core::editor_debugger::{StepKind,WatchValue};
type Game=rvn_core::Engine<crate::bevy_renderer::BevyRenderer>;
type Save=rvn_core::save::consumer_checkpoint::Save15;
const SOURCE:&str=r#"
handler initialized(event){set initialized_count=initialized_count+1}
handler pre(event){set pre_count=pre_count+1}
handler construct(event){local construct_draw=random(0,1000000) while false{set construct_draw=construct_draw+1} set construct_roll=construct_draw set constructed_count=constructed_count+1 set shared=shared+100}
handler destruct(event){set destructed_count=destructed_count+1}
handler click(event){local input_draw=random(0,1000000) while false{set input_draw=input_draw+1} set input_seen=shared+input_draw set input_count=input_count+1}
handler host_tick(event){local host_draw=random(0,1000000) while false{set host_draw=host_draw+1} set host_roll=host_draw set host_hits=host_hits+1}
function sample(){local story_draw=random(0,1000000) while false{set story_draw=story_draw+1} return shared+story_draw}
function paint(state,props,frame){return []}
screen panel(value){return {"host":{"events":{"initialized":"initialized","pre_construct":"pre","construct":"construct","destruct":"destruct"}},"root":component("real","button",{"events":{"click":"click"}},[])}}
screen legacy_host(context){return component("clock","canvas",{"draw":"paint","state":{},"events":{"tick":"host_tick"}},[])}
init{set initialized_count=0 set pre_count=0 set constructed_count=0 set destructed_count=0 set shared=0 set construct_roll=0 set input_seen=0 set input_count=0 set host_roll=0 set host_hits=0 set observed=0}
label start
"Ready"
set observed=sample()
"Observed"
return
label done
"Done"
"#;
fn native_project(source:&str)->Project{
 let project=Project::new(false);fs::remove_file(project.root.join(".rvn-authoring.json")).unwrap();
 let mut graphs=rvn_graph::import_script(&rvn_parser::parse(source).unwrap()).unwrap();for graph in &mut graphs{graph.enable_native_frame_preparation().unwrap();}
 let compiled=rvn_graph::transpile_project(&graphs).expect("real native Graph compiler");assert!(compiled.source.contains("flow.consume_v2("));fs::write(project.root.join("main.rvn"),compiled.source).unwrap();
 let workspace=rvn_graph::SourceWorkspace::link_native_preparation(&project.root,&project.root.join("main.rvn"),&graphs).unwrap();
 let screen=workspace.project().graphs().iter().find(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Screen{name}if name=="panel")).unwrap();let id=WidgetGraphRef16::from_u64(screen.graph_id.get());let members=workspace.project().graphs().iter().filter(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Screen{name}if name=="panel")||matches!(&graph.kind,rvn_graph::GraphKind::Handler{..}|rvn_graph::GraphKind::Function{..})).map(|graph|WidgetGraphRef16::from_u64(graph.graph_id.get())).collect();
 fs::write(project.root.join("assets/panel.rvnwidget"),WidgetBlueprintV1{uuid:UUID.into(),source:"main.rvn".into(),screen_graph:id,members}.encode().unwrap()).unwrap();project
}
fn warm(project:&Project)->(Game,rvn_core::engine::WidgetInstanceHandle){
 let mut game=load_game(&project.root).unwrap().engine;
 game.step_until_interaction().unwrap();
 let handle=game.create_widget(UUID,"one",vec![Value::Int(1)]).unwrap();
 game.add_widget_to_viewport(&handle,false,0).unwrap();(game,handle)
}
fn save(game:&mut Game,project:&Project,session:Option<&DebugSession>)->Save{
 handoff_zero_media_publication(game);
 let wire=game.capture_save15(&project.program(),session,16,"deferred actual Widget materialization".into()).unwrap().encode().unwrap();
 Save::decode(&wire).unwrap()
}
fn pause_after_binding(game:&mut Game,operation:DebugOperation,name:&str)->DebugSession{
 let mut session=game.debug_begin(operation,vec![]).unwrap();for _ in 0..128{let(progress,_)=game.debug_step(&mut session,StepKind::Into).unwrap();assert_ne!(progress,DebugProgress::Complete,"real requested local must be observed before operation commit");if session.snapshot().frames.iter().any(|frame|frame.bindings.iter().any(|(actual,value)|actual==name&&matches!(value,WatchValue::Value(Value::Int(_))))){return session;}}panic!("actual owned local {name} was never reached");
}
fn finish(game:&mut Game,session:&mut DebugSession)->DebugOutcome{for _ in 0..256{let(progress,outcome)=game.debug_step(session,StepKind::Continue).unwrap();if progress==DebugProgress::Complete{return outcome.unwrap();}}panic!("saved owned computation did not finish");}
// These Native Graph fixtures emit their Screen Return through consume_v1.
// It is genuine Screen VM work, but not a v2/v3 persistent instance projection.
// Authenticate the actual frozen source/class and paused computation separately.
fn assert_real_panel_computation(game:&Game,project:&Project,session:&DebugSession){
 let class=game.widget_classes().class(UUID).unwrap();
 let descriptor=WidgetBlueprintV1::decode(&fs::read(project.root.join("assets/panel.rvnwidget")).unwrap()).unwrap();
 assert!(class.descriptor()==&descriptor,"the pending work must use the actual admitted descriptor/Screen graph");
 assert_eq!(class.identity().uuid,UUID);assert_eq!(class.identity().definition,"panel");
 let instance=game.state.ui.screens.iter().find(|instance|instance.name=="one").unwrap();
 let binding=instance.widget_class.as_ref().unwrap();assert_eq!(&binding.class,class.identity());
 let program=project.program();
 let sources=rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&program).unwrap();
 let definition=sources.definition(&rvn_core::consumer_schedule::HostDefinitionKind::Screen,&class.identity().definition).unwrap();
 assert_eq!(definition.definition,class.identity().definition);assert_eq!(definition.source_ordinal,class.identity().source_ordinal);
 assert!(!definition.native_plans,"a v1 Return must not invent a v2/v3 resident projection");
 let rvn_parser::Statement::Screen{name,body,..}=&program.script[class.identity().root_index]else{panic!("actual admitted class root must remain its frozen Screen declaration");};
 assert_eq!(name,&class.identity().definition);
 assert!(body.iter().any(|statement|matches!(statement,rvn_parser::Statement::ScheduledConsumer{version:1,..})),"the actual Graph Screen Return keeps its v1 schedule: {body:?}");
 assert!(instance.consumer_projection.is_none(),"the actual v1 Screen has no resident v2/v3 Screen projection");
 // This instance may legitimately retain native lifecycle Handler frames.
 // Authenticate them against their own actual frozen definitions, rather
 // than inferring their absence from the Screen Return's v1 schedule.
 for(key,frame)in &instance.consumer_frames{
  assert_eq!(frame.owner.kind,rvn_core::consumer_schedule::HostDefinitionKind::Handler,"a v1 Screen cannot fabricate a native Screen frame");
  let handler=sources.definition(&frame.owner.kind,&frame.owner.definition).expect("resident Handler frame needs its actual frozen source definition");
  assert!(handler.native_plans,"only an actual native Handler owns this resident frame");
  assert_eq!(frame.owner.source_ordinal,handler.source_ordinal);
  assert_eq!(key,&frame.owner.storage_key());
  assert_eq!(frame.owner.target.kind,rvn_core::scene_resources::TargetKind::Interface);
  assert_eq!(frame.owner.target.alias,instance.name);
  frame.frame.validate().unwrap();
 }
 let snapshot=session.snapshot();assert!(snapshot.evaluating);assert!(!snapshot.running);assert!(snapshot.error.is_none());
 assert!(snapshot.frames.iter().any(|frame|frame.function.as_deref()==Some("screen:panel")),"the actual pending computation must belong to this admitted Screen, not a neighbouring Handler/function");
}
fn pending(game:&Game){assert!(game.has_pending_widget_materialization());assert!(game.capture_ui_input_source("one").is_none());let handle=game.widget_instance_handle("one").unwrap();let source=game.widget_backend_owner(&handle).unwrap();assert!(game.renderer.widget_backend.wrapper(&source).is_none());}
fn new_life(game:&mut Game){let operation=game.next_widget_materialization().unwrap().expect("completed saved boundary exposes actual new owner work");game.execute_widget_materialization(operation).unwrap();assert!(!game.has_pending_widget_materialization());assert!(game.capture_ui_input_source("one").is_some());assert_eq!(game.state.vars["initialized_count"],Value::Int(1));}
#[test]
fn cold_save15_reconstruction_is_explicit_and_two_reloads_do_not_replay_initialization_or_construct_before_boundary(){
 let project=native_project(SOURCE);let(mut warm,handle)=warm(&project);let saved=save(&mut warm,&project,None);let vars=warm.state.vars.clone();let rng=warm.state.random;
 let mut cold=load_game(&project.root).unwrap().engine;assert!(cold.restore_save15(&project.program(),saved).unwrap().is_none());assert_eq!(cold.state.vars,vars);assert_eq!(cold.state.random,rng);pending(&cold);assert!(!cold.widget_materialization_waits_for_saved_operation());
 let saved_again=save(&mut cold,&project,None);let mut colder=load_game(&project.root).unwrap().engine;colder.restore_save15(&project.program(),saved_again).unwrap();assert_eq!(colder.state.vars,vars);assert_eq!(colder.state.random,rng);pending(&colder);
 for game in [&mut cold,&mut colder]{assert!(game.add_widget_to_viewport(&handle,false,0).is_err());new_life(game);assert_eq!(game.state.vars["constructed_count"],Value::Int(2));assert_eq!(game.state.vars["pre_count"],Value::Int(2));assert_eq!(game.state.vars["shared"],Value::Int(200));let expected=game.state.vars.clone();assert!(game.next_widget_materialization().unwrap().is_none());assert_eq!(game.state.vars,expected);}
}
#[test]
fn stopped_story_keeps_saved_global_and_random_trace_then_construct_runs_after_its_true_commit(){
 let project=native_project(SOURCE);let(mut warm,_)=warm(&project);warm.advance_dialogue().unwrap();let session=pause_after_binding(&mut warm,DebugOperation::Statement,"story_draw");let saved=save(&mut warm,&project,Some(&session));let vars=warm.state.vars.clone();let rng=warm.state.random;
 let mut cold=load_game(&project.root).unwrap().engine;let mut restored=cold.restore_save15(&project.program(),saved).unwrap().unwrap();assert_eq!(cold.state.vars,vars);assert_eq!(cold.state.random,rng);pending(&cold);assert!(cold.widget_materialization_waits_for_saved_operation());assert!(cold.next_widget_materialization().unwrap().is_none());
 let saved_again=save(&mut cold,&project,Some(&restored));let mut colder=load_game(&project.root).unwrap().engine;let mut restored_again=colder.restore_save15(&project.program(),saved_again).unwrap().unwrap();assert_eq!(colder.state.vars,vars);assert_eq!(colder.state.random,rng);pending(&colder);
 for(game,session)in[(&mut cold,&mut restored),(&mut colder,&mut restored_again)]{let mut expected=rng;let roll=expected.integer(0,1000000).unwrap();assert!(matches!(finish(game,session),DebugOutcome::Interaction(_)));assert_eq!(game.state.vars["observed"],Value::Int(100+roll));assert_eq!(game.state.random,expected);assert_eq!(game.state.vars["constructed_count"],Value::Int(1));assert!(!game.widget_materialization_waits_for_saved_operation());new_life(game);assert_eq!(game.state.vars["shared"],Value::Int(200));assert_eq!(game.state.vars["observed"],Value::Int(100+roll));assert_eq!(game.state.vars["constructed_count"],Value::Int(2));}
}
#[test]
fn stopped_materialization_owned_callback_resaves_and_hydrates_without_requiring_a_ready_wrapper(){
 let project=native_project(SOURCE);let(mut warm,_)=warm(&project);let checkpoint=save(&mut warm,&project,None);let mut cold=load_game(&project.root).unwrap().engine;cold.restore_save15(&project.program(),checkpoint).unwrap();let operation=cold.next_widget_materialization().unwrap().unwrap();let prepared_generation=operation.object().unwrap().generation;let session=pause_after_binding(&mut cold,DebugOperation::MaterializeWidget(operation),"construct_draw");let vars=cold.state.vars.clone();let rng=cold.state.random;assert_eq!(vars["constructed_count"],Value::Int(1));pending(&cold);let checkpoint=save(&mut cold,&project,Some(&session));let wire:serde_json::Value=serde_json::from_str(&checkpoint.encode().unwrap()).unwrap();assert!(wire["debugger"]["widget_materialization"].is_object());assert!(wire["debugger"]["widget_materialization"].get("generation").is_none());assert!(wire["debugger"]["widget_materialization"].get("prepared").is_none());
 let mut reopened=load_game(&project.root).unwrap().engine;let mut restored=reopened.restore_save15(&project.program(),checkpoint).unwrap().unwrap();assert_eq!(reopened.state.vars,vars);assert_eq!(reopened.state.random,rng);pending(&reopened);let checkpoint=save(&mut reopened,&project,Some(&restored));let mut twice=load_game(&project.root).unwrap().engine;let mut twice_session=twice.restore_save15(&project.program(),checkpoint).unwrap().unwrap();
 for(game,session)in[(&mut reopened,&mut restored),(&mut twice,&mut twice_session)]{assert!(matches!(finish(game,session),DebugOutcome::WidgetMaterialized));assert_eq!(game.state.vars["initialized_count"],Value::Int(1));assert_eq!(game.state.vars["constructed_count"],Value::Int(2));assert_eq!(game.state.vars["pre_count"],Value::Int(2));assert_eq!(game.state.vars["shared"],Value::Int(200));assert!(!game.has_pending_widget_materialization());assert!(game.next_widget_materialization().unwrap().is_none());let handle=game.widget_instance_handle("one").unwrap();let source=game.widget_backend_owner(&handle).unwrap();assert_ne!(game.renderer.widget_backend.wrapper(&source).unwrap().upgrade().unwrap().generation(),prepared_generation);}
}
#[test]
fn stopped_widget_input_reloads_twice_with_exact_logical_authority_and_no_new_physical_capture(){
 let project=native_project(SOURCE);let(mut warm,_)=warm(&project);let source=warm.capture_ui_input_source("one").unwrap();let input=rvn_core::ui_input::ScopedUiInput{source:source.clone(),event:rvn_core::ui::UiInput{screen:"one".into(),element:"real".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}};let session=pause_after_binding(&mut warm,DebugOperation::Input(input),"input_draw");let checkpoint=save(&mut warm,&project,Some(&session));let vars=warm.state.vars.clone();let rng=warm.state.random;
 let mut cold=load_game(&project.root).unwrap().engine;let mut restored=cold.restore_save15(&project.program(),checkpoint).unwrap().unwrap();pending(&cold);assert!(!cold.ui_input_source_current(&source));let DebugOperation::Input(logical)=restored.operation()else{panic!("actual saved input was not restored");};assert!(cold.ui_input_source_current(&logical.source));assert!(cold.capture_ui_input_source("one").is_none());assert_eq!(cold.state.vars,vars);assert_eq!(cold.state.random,rng);
 let checkpoint=save(&mut cold,&project,Some(&restored));let mut twice=load_game(&project.root).unwrap().engine;let mut twice_session=twice.restore_save15(&project.program(),checkpoint).unwrap().unwrap();
 for(game,session)in[(&mut cold,&mut restored),(&mut twice,&mut twice_session)]{let mut expected=rng;let roll=expected.integer(0,1000000).unwrap();assert!(matches!(finish(game,session),DebugOutcome::Input(true)));assert_eq!(game.state.vars["input_count"],Value::Int(1));assert_eq!(game.state.vars["input_seen"],Value::Int(100+roll));assert_eq!(game.state.random,expected);assert_eq!(game.state.vars["constructed_count"],Value::Int(1));new_life(game);assert_eq!(game.state.vars["input_seen"],Value::Int(100+roll));assert_eq!(game.state.vars["constructed_count"],Value::Int(2));}
}
#[test]
fn stopped_game_frame_restores_its_clock_and_real_native_screen_trace_before_new_widget_life(){
 let source=SOURCE.replace("label start\n\"Ready\"","label start\ntimer 5 => jump done\n\"Ready\"");let project=native_project(&source);let(mut warm,_)=warm(&project);assert!(warm.state.game_tasks.is_some());let state_before=serde_json::to_value(&warm.state).unwrap();let saved_handler_frames=warm.state.ui.screens.iter().find(|instance|instance.name=="one").unwrap().consumer_frames.clone();assert!(!saved_handler_frames.is_empty(),"the real native lifecycle Handlers must already have resident frames before this saved frame operation");let session=warm.debug_begin(DebugOperation::GameFrame{seconds:0.25,reports:Default::default(),audio:vec![]},vec![]).unwrap();assert_real_panel_computation(&warm,&project,&session);assert_eq!(serde_json::to_value(&warm.state).unwrap(),state_before);let checkpoint=save(&mut warm,&project,Some(&session));let wire:serde_json::Value=serde_json::from_str(&checkpoint.encode().unwrap()).unwrap();assert!(wire["debugger"]["trace"].is_object(),"the attached Native Screen is real owned work, not a standalone scalar-clock entry");assert!(wire["debugger"]["frame_operation"].is_object());let rng=warm.state.random;
 let mut cold=load_game(&project.root).unwrap().engine;let mut restored=cold.restore_save15(&project.program(),checkpoint).unwrap().unwrap();pending(&cold);assert!(cold.next_widget_materialization().unwrap().is_none());let checkpoint=save(&mut cold,&project,Some(&restored));let mut twice=load_game(&project.root).unwrap().engine;let mut twice_session=twice.restore_save15(&project.program(),checkpoint).unwrap().unwrap();
 for(game,session)in[(&mut cold,&mut restored),(&mut twice,&mut twice_session)]{assert_eq!(game.state.ui.screens.iter().find(|instance|instance.name=="one").unwrap().consumer_frames,saved_handler_frames,"both actual Save15 restores retain the complete original Handler frames and their logical target identities");assert_real_panel_computation(game,&project,session);assert!(matches!(finish(game,session),DebugOutcome::Game(_)));assert_eq!(game.state.game_tasks.as_ref().unwrap().tasks.timer.as_ref().unwrap().elapsed,0.25);assert_eq!(game.state.random,rng);assert_eq!(game.state.vars["constructed_count"],Value::Int(1));new_life(game);assert_eq!(game.state.vars["constructed_count"],Value::Int(2));}
}
#[test]
fn stopped_real_legacy_host_frame_keeps_its_actual_lifecycle_context_while_neighbor_widget_is_deferred(){
 let project=native_project(SOURCE);let(mut warm,_)=warm(&project);warm.synchronize_source_menu(rvn_ui::PageRole::Pause,Some("legacy_host"),Value::Dict(Default::default()),false,-10).unwrap();let session=pause_after_binding(&mut warm,DebugOperation::HostFrame{seconds:0.1,reports:Default::default(),audio:vec![]},"host_draw");let checkpoint=save(&mut warm,&project,Some(&session));let rng=warm.state.random;let mut cold=load_game(&project.root).unwrap().engine;cold.step_until_interaction().unwrap();cold.synchronize_source_menu(rvn_ui::PageRole::Pause,Some("legacy_host"),Value::Dict(Default::default()),false,-10).unwrap();let context=cold.capture_host_lifecycle_restore_context(&project.program()).unwrap();let mut restored=cold.restore_save15_with_host_context(&project.program(),checkpoint,&context).unwrap().unwrap();pending(&cold);assert!(cold.next_widget_materialization().unwrap().is_none());assert_eq!(cold.state.random,rng);assert!(matches!(finish(&mut cold,&mut restored),DebugOutcome::Host(_)));assert_eq!(cold.state.vars["host_hits"],Value::Int(1));assert_eq!(cold.state.random,rng);assert_eq!(cold.state.vars["constructed_count"],Value::Int(1));new_life(&mut cold);assert_eq!(cold.state.vars["host_hits"],Value::Int(1));assert_eq!(cold.state.vars["constructed_count"],Value::Int(2));
}
#[test]
fn forged_saved_owner_method_or_class_is_refused_before_any_live_mutation(){
 let project=native_project(SOURCE);let(mut warm,_)=warm(&project);let checkpoint=save(&mut warm,&project,None);let mut pending_game=load_game(&project.root).unwrap().engine;pending_game.restore_save15(&project.program(),checkpoint).unwrap();let operation=pending_game.next_widget_materialization().unwrap().unwrap();let session=pause_after_binding(&mut pending_game,DebugOperation::MaterializeWidget(operation),"construct_draw");let checkpoint=save(&mut pending_game,&project,Some(&session));
 for foreign_class in[false,true]{let mut wire:serde_json::Value=serde_json::from_str(&checkpoint.encode().unwrap()).unwrap();if foreign_class{wire["debugger"]["widget_materialization"]["class"]["uuid"]=serde_json::json!("ffffffffffffffffffffffffffffffff");}else{wire["debugger"]["widget_materialization"]["methods"][0]["name"]=serde_json::json!("host_tick");}let forged=Save::decode(&serde_json::to_string(&wire).unwrap()).unwrap();let mut cold=load_game(&project.root).unwrap().engine;let before=serde_json::to_vec(&cold.state).unwrap();assert!(cold.restore_save15(&project.program(),forged).is_err());assert_eq!(serde_json::to_vec(&cold.state).unwrap(),before);assert!(!cold.has_pending_widget_materialization());}
}

#[path="widget_materialization_driver_tests_0964.rs"]
mod driver_0964;

#[test]
fn actual_empty_owner_materialization_resaves_its_real_native_screen_trace_without_a_ready_grant(){
 let source=SOURCE.replace("screen panel(value){return {\"host\":{\"events\":{\"initialized\":\"initialized\",\"pre_construct\":\"pre\",\"construct\":\"construct\",\"destruct\":\"destruct\"}},\"root\":component(\"real\",\"button\",{\"events\":{\"click\":\"click\"}},[])}}","screen panel(value){return []}");assert_ne!(source,SOURCE);let project=native_project(&source);let(mut game,_)=warm(&project);let checkpoint=save(&mut game,&project,None);let mut cold=load_game(&project.root).unwrap().engine;cold.restore_save15(&project.program(),checkpoint).unwrap();let operation=cold.next_widget_materialization().unwrap().unwrap();let state_before=serde_json::to_value(&cold.state).unwrap();let session=cold.debug_begin(DebugOperation::MaterializeWidget(operation),vec![]).unwrap();let entry=session.snapshot();assert!(entry.evaluating,"the actual operation is pending until its one commit");assert_real_panel_computation(&cold,&project,&session);assert_eq!((entry.steps,entry.value_work),(0,0));assert_eq!(serde_json::to_value(&cold.state).unwrap(),state_before);let checkpoint=save(&mut cold,&project,Some(&session));let wire:serde_json::Value=serde_json::from_str(&checkpoint.encode().unwrap()).unwrap();assert!(wire["debugger"]["trace"].is_object(),"return [] is still a real Native Screen computation and its trace must survive Save15");assert!(wire["debugger"]["widget_materialization"].is_object());let mut reopened=load_game(&project.root).unwrap().engine;let mut restored=reopened.restore_save15(&project.program(),checkpoint).unwrap().unwrap();pending(&reopened);assert!(reopened.next_widget_materialization().unwrap().is_none());let checkpoint=save(&mut reopened,&project,Some(&restored));let mut twice=load_game(&project.root).unwrap().engine;let mut twice_session=twice.restore_save15(&project.program(),checkpoint).unwrap().unwrap();for(game,session)in[(&mut reopened,&mut restored),(&mut twice,&mut twice_session)]{assert_real_panel_computation(game,&project,session);assert!(matches!(finish(game,session),DebugOutcome::WidgetMaterialized));assert!(!game.has_pending_widget_materialization());assert!(game.interface_views().unwrap().is_empty());assert_eq!(game.state.vars["constructed_count"],Value::Int(0));assert_eq!(game.state.vars["initialized_count"],Value::Int(0));assert!(game.capture_ui_input_source("one").is_some());}
}

#[test]
fn save15_preserves_pending_publication_refusal_until_this_zero_media_host_receives_the_actual_output(){
 let project=native_project(SOURCE);let(mut game,_)=warm(&project);assert!(game.renderer.scene_batch_pending());let before=serde_json::to_value(&game.state).unwrap();let source=game.debug_state_source().unwrap();
 let refused=game.capture_save15(&project.program(),None,16,"must remain pending".into());assert!(refused.is_err());assert!(refused.err().unwrap().to_string().contains("settled successful publication"));assert!(game.renderer.scene_batch_pending());assert!(game.accepts_debug_state_source(&source));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
 handoff_zero_media_publication(&mut game);assert!(!game.renderer.scene_batch_pending());let checkpoint=game.capture_save15(&project.program(),None,16,"actual zero-media CPU boundary".into()).unwrap();assert!(checkpoint.encode().is_ok());assert!(game.accepts_debug_state_source(&source));assert_eq!(serde_json::to_value(&game.state).unwrap(),before);
}

#[path="widget_load_retirement_tests_0965.rs"]
mod load_retirement_0965;
// Macro warm/cold materialization belongs to the existing real Bevy wrapper
// fixture, not the Core logical-only Host. No World/paint claim is made here.
fn macro_materialization_project() -> (Project, (u64, u64)) { macro_materialization_project_with_connected(false) }
fn macro_materialization_project_with_connected(connected:bool) -> (Project, (u64, u64)) {
    let raw = r#"
handler initialized(event){set initialized_count=initialized_count+1}
handler pre(event){set pre_count=pre_count+1}
handler construct(event){set constructed_count=constructed_count+1}
handler destruct(event){set destructed_count=destructed_count+1}
handler click(event){set input_count=input_count+1}
screen panel(value){return {"host":{"events":{"initialized":"initialized","pre_construct":"pre","construct":"construct","destruct":"destruct"}},"root":component("real","button",{"events":{"click":"click"}},[])}}
init{set initialized_count=0 set pre_count=0 set constructed_count=0 set destructed_count=0 set input_count=0}
label start
"Ready"
"Later"
return
"#;
    let raw=if connected{raw.replace("handler click(event){", "handler click(event){set operand_probe=random(1,1)>1 ")}else{raw.to_owned()};
    let project = Project::new(false);
    fs::remove_file(project.root.join(".rvn-authoring.json")).unwrap();
    let mut graphs = rvn_graph::import_script(&rvn_parser::parse(&raw).unwrap()).unwrap();
    let graph = graphs
        .iter_mut()
        .find(|g| matches!(&g.kind,rvn_graph::GraphKind::Handler{name}if name=="click"))
        .unwrap();
    let operand=if connected{
        let probe=graph.nodes.values().find(|n|n.kind==rvn_graph::NodeKind::SetVariable&&n.properties.get("name")==Some(&rvn_graph::PropertyValue::String("operand_probe".into()))).unwrap().id;
        let value=graph.pin_by_key(probe,"value").unwrap().id;let producer=graph.edges.values().find(|e|e.input==value).unwrap().output;
        let input=graph.pin_by_key(probe,"exec_in").unwrap().id;let output=graph.pin_by_key(probe,"exec_out").unwrap().id;
        let prior=graph.edges.values().find(|e|e.input==input).unwrap().output;let next=graph.edges.values().find(|e|e.output==output).unwrap().input;
        graph.remove_node(probe).unwrap();graph.connect(prior,next).unwrap();Some(producer)
    }else{None};
    let root = graph
        .nodes
        .values()
        .find(|n| n.kind == rvn_graph::NodeKind::HandlerEntry)
        .unwrap()
        .id;
    let enter = graph.pin_by_key(root, "exec_out").unwrap().id;
    let edge = graph
        .edges
        .values()
        .find(|e| e.output == enter)
        .unwrap()
        .clone();
    let once = graph
        .add_catalog_node(rvn_graph::NodeKind::DoOnce, [400.0, 0.0])
        .unwrap();
    graph.remove_edge(edge.id).unwrap();
    graph
        .connect(enter, graph.pin_by_key(once, "exec_in").unwrap().id)
        .unwrap();
    graph
        .connect(graph.pin_by_key(once, "completed").unwrap().id, edge.input)
        .unwrap();
    if let Some(producer)=operand{
        let reroute=graph.add_catalog_node(rvn_graph::NodeKind::Reroute,[200.0,100.0]).unwrap();
        graph.connect(producer,graph.pin_by_key(reroute,"value").unwrap().id).unwrap();
        graph.connect(graph.pin_by_key(reroute,"value_out").unwrap().id,graph.pin_by_key(once,"start_closed").unwrap().id).unwrap();
    }
    let site = (graph.graph_id.get(), once.get());
    for graph in &mut graphs {
        graph.enable_native_frame_preparation().unwrap();
    }
    let macro_graph = graphs.iter().find(|g| g.graph_id.get() == site.0).unwrap();
    rvn_graph::prepare_native_frame_graph(macro_graph).expect("actual macro pins/source roundtrip");
    let compiled = rvn_graph::transpile_project(&graphs).unwrap();
    assert!(compiled.source.contains("flow.macro_dispatch_v1("));
    fs::write(project.root.join("main.rvn"), compiled.source).unwrap();
    let workspace = rvn_graph::SourceWorkspace::link_native_preparation(
        &project.root,
        &project.root.join("main.rvn"),
        &graphs,
    )
    .unwrap();
    let screen = workspace
        .project()
        .graphs()
        .iter()
        .find(|g| matches!(&g.kind,rvn_graph::GraphKind::Screen{name}if name=="panel"))
        .unwrap();
    let id = WidgetGraphRef16::from_u64(screen.graph_id.get());
    let members = workspace
        .project()
        .graphs()
        .iter()
        .filter(|g| {
            matches!(&g.kind,rvn_graph::GraphKind::Screen{name}if name=="panel")
                || matches!(&g.kind, rvn_graph::GraphKind::Handler { .. })
        })
        .map(|g| WidgetGraphRef16::from_u64(g.graph_id.get()))
        .collect();
    fs::write(
        project.root.join("assets/panel.rvnwidget"),
        WidgetBlueprintV1 {
            uuid: UUID.into(),
            source: "main.rvn".into(),
            screen_graph: id,
            members,
        }
        .encode()
        .unwrap(),
    )
    .unwrap();
    (project, site)
}
fn macro_materialization_click(game: &mut Game, alias: &str) {
    let input = rvn_core::ui_input::ScopedUiInput {
        source: game.capture_ui_input_source(alias).unwrap(),
        event: rvn_core::ui::UiInput {
            screen: alias.into(),
            element: "real".into(),
            kind: rvn_ui::programmable::ScreenEventKind::Click,
            value: None,
            key: None,
        },
    };
    assert!(game.interface_event_scoped(input).unwrap());
    handoff_zero_media_publication(game);
}
fn macro_materialize_all(game: &mut Game) {
    for _ in 0..32 {
        let Some(operation) = game.next_widget_materialization().unwrap() else {
            break;
        };
        let object = operation
            .object()
            .expect("actual pure prepared Bevy wrapper")
            .clone();
        let backend = game.renderer.widget_backend.clone();
        assert!(
            backend.wrapper(&object.owner).is_none(),
            "pure preparation grants no viewport parent"
        );
        game.execute_widget_materialization(operation).unwrap();
        let published = backend.wrapper(&object.owner).unwrap().upgrade().unwrap();
        assert_eq!(published.generation(), object.generation);
        assert!(published.attached());
        assert!(published.prepared_node().is_ok());
        handoff_zero_media_publication(game);
    }
    assert!(!game.has_pending_widget_materialization());
}
fn macro_materialization_cell(
    game: &Game,
    alias: &str,
    site: (u64, u64),
) -> rvn_core::widget_class_source_0950::standard_macros::CellInspection {
    let handle = game.widget_instance_handle(alias).unwrap();
    let (origin, cell) = game
        .inspect_widget_macro_cell(&handle, site.0, site.1)
        .unwrap();
    assert_eq!((origin.graph, origin.node), (site.0, Some(site.1)));
    cell
}
#[test]
fn standard_macro1021_real_bevy_warm_rollback_materializes_new_wrappers_preserving_exact_owner_cells(
) {
    use rvn_core::widget_class_source_0950::standard_macros::CellInspection as C;
    let (project, site) = macro_materialization_project();
    let (mut game, one) = warm(&project);
    let two = game
        .create_widget(UUID, "two", vec![Value::Int(2)])
        .unwrap();
    game.add_widget_to_viewport(&two, false, 0).unwrap();
    handoff_zero_media_publication(&mut game);
    macro_materialization_click(&mut game, "one");
    macro_materialization_click(&mut game, "one");
    assert_eq!(game.state.vars["input_count"], Value::Int(1));
    macro_materialization_click(&mut game, "two");
    assert_eq!(game.state.vars["input_count"], Value::Int(2));
    let backend = game.renderer.widget_backend.clone();
    let one_owner = game.widget_backend_owner(&one).unwrap();
    let two_owner = game.widget_backend_owner(&two).unwrap();
    let old_one = backend.wrapper(&one_owner).unwrap().upgrade().unwrap();
    let old_two = backend.wrapper(&two_owner).unwrap().upgrade().unwrap();
    let retained = game
        .state
        .ui
        .screens
        .iter()
        .find(|i| i.name == "one")
        .unwrap()
        .standard_macro_frame
        .clone();
    assert!(retained.is_some());
    let saved_rng = game.state.random;
    let saved_ui_rng = game
        .state
        .ui
        .screens
        .iter()
        .filter(|i| i.name == "one" || i.name == "two")
        .map(|i| (i.name.clone(), i.random))
        .collect::<std::collections::BTreeMap<_, _>>();
    let old_input = game.capture_ui_input_source("one").unwrap();
    let lineage = game.instance_id();
    let epoch = game.runtime_epoch();
    assert!(game.rollback());
    assert_eq!(game.instance_id(), lineage);
    assert_ne!(game.runtime_epoch(), epoch);
    assert_eq!(game.state.random, saved_rng);
    assert_eq!(game.state.vars["input_count"], Value::Int(1));
    assert_eq!(
        game.state
            .ui
            .screens
            .iter()
            .find(|i| i.name == "one")
            .unwrap()
            .standard_macro_frame,
        retained,
        "actual history clone retained the consumed frame exactly"
    );
    assert!(!game.ui_input_source_current(&old_input));
    assert!(!old_one.attached() && !old_two.attached());
    assert!(backend.wrapper(&one_owner).is_none() && backend.wrapper(&two_owner).is_none());
    assert!(game.capture_ui_input_source("one").is_none());
    assert!(game.capture_ui_input_source("two").is_none());
    assert_eq!(
        macro_materialization_cell(&game, "one", site),
        C::DoOnce {
            initialized: true,
            completed: true
        }
    );
    assert_eq!(
        macro_materialization_cell(&game, "two", site),
        C::DoOnce {
            initialized: false,
            completed: false
        }
    );
    handoff_zero_media_publication(&mut game);
    macro_materialize_all(&mut game);
    let current_one = game.widget_instance_handle("one").unwrap();
    let current_two = game.widget_instance_handle("two").unwrap();
    let current_one_owner = game.widget_backend_owner(&current_one).unwrap();
    let current_two_owner = game.widget_backend_owner(&current_two).unwrap();
    assert_eq!(current_one_owner.lineage(), one_owner.lineage());
    assert_eq!(
        current_one_owner.target(),
        one_owner.target(),
        "real warm transfer preserves logical incarnation; do not rebind by alias"
    );
    assert_eq!(current_two_owner.target(), two_owner.target());
    assert_ne!(
        backend
            .wrapper(&current_one_owner)
            .unwrap()
            .upgrade()
            .unwrap()
            .generation(),
        old_one.generation()
    );
    assert_ne!(
        backend
            .wrapper(&current_two_owner)
            .unwrap()
            .upgrade()
            .unwrap()
            .generation(),
        old_two.generation()
    );
    assert_eq!(
        game.state
            .ui
            .screens
            .iter()
            .find(|i| i.name == "one")
            .unwrap()
            .standard_macro_frame,
        retained
    );
    assert_eq!(game.state.random, saved_rng);
    for instance in game
        .state
        .ui
        .screens
        .iter()
        .filter(|i| i.name == "one" || i.name == "two")
    {
        assert_eq!(instance.random, saved_ui_rng[&instance.name]);
    }
    macro_materialization_click(&mut game, "one");
    assert_eq!(game.state.vars["input_count"], Value::Int(1));
    macro_materialization_click(&mut game, "two");
    assert_eq!(game.state.vars["input_count"], Value::Int(2));
    assert_eq!(
        game.state.vars["initialized_count"],
        Value::Int(2),
        "new wrapper life does not replay logical initialization"
    );
}
#[test]
fn standard_macro1021_real_bevy_completed_cold_save_direct_and_json_reset_cells_then_materialize_fresh_owner(
) {
    use rvn_core::widget_class_source_0950::standard_macros::CellInspection as C;
    let (project, site) = macro_materialization_project();
    let (mut live, handle) = warm(&project);
    handoff_zero_media_publication(&mut live);
    macro_materialization_click(&mut live, "one");
    macro_materialization_click(&mut live, "one");
    assert_eq!(live.state.vars["input_count"], Value::Int(1));
    let saved_vars = live.state.vars.clone();
    let saved_rng = live.state.random;
    let saved_ui_rng = live
        .state
        .ui
        .screens
        .iter()
        .find(|i| i.name == "one")
        .unwrap()
        .random;
    let live_owner = live.widget_backend_owner(&handle).unwrap();
    let old = live
        .renderer
        .widget_backend
        .wrapper(&live_owner)
        .unwrap()
        .upgrade()
        .unwrap();
    let saved = live
        .capture_save15(
            &project.program(),
            None,
            17,
            "actual cold macro reset".into(),
        )
        .unwrap();
    let json = saved.encode().unwrap();
    assert!(!json.contains("standard_macro_frame"));
    let decoded = Save::decode(&json).unwrap();
    for save in [saved, decoded] {
        let mut cold = load_game(&project.root).unwrap().engine;
        assert_ne!(cold.instance_id(), live.instance_id());
        assert!(cold
            .restore_save15(&project.program(), save)
            .unwrap()
            .is_none());
        assert_eq!(cold.state.vars, saved_vars);
        assert_eq!(cold.state.random, saved_rng);
        assert!(
            cold.state
                .ui
                .screens
                .iter()
                .all(|i| i.standard_macro_frame.is_none()),
            "both in-memory Save15 and JSON cold hydration reset transient cells"
        );
        assert!(
            cold.add_widget_to_viewport(&handle, false, 0).is_err(),
            "old Engine handle grants no owner"
        );
        pending(&cold);
        handoff_zero_media_publication(&mut cold);
        macro_materialize_all(&mut cold);
        let current = cold.widget_instance_handle("one").unwrap();
        let owner = cold.widget_backend_owner(&current).unwrap();
        assert_ne!(owner.lineage(), live_owner.lineage());
        let fresh = cold
            .renderer
            .widget_backend
            .wrapper(&owner)
            .unwrap()
            .upgrade()
            .unwrap();
        assert!(fresh.attached());
        assert_ne!(fresh.generation(), old.generation());
        assert_eq!(
            macro_materialization_cell(&cold, "one", site),
            C::DoOnce {
                initialized: false,
                completed: false
            }
        );
        assert_eq!(cold.state.vars["initialized_count"], Value::Int(1));
        assert_eq!(cold.state.vars["constructed_count"], Value::Int(2));
        assert_eq!(cold.state.random, saved_rng);
        assert_eq!(
            cold.state
                .ui
                .screens
                .iter()
                .find(|i| i.name == "one")
                .unwrap()
                .random,
            saved_ui_rng
        );
        macro_materialization_click(&mut cold, "one");
        assert_eq!(
            cold.state.vars["input_count"],
            Value::Int(2),
            "first cold Enter consumes a fresh cell"
        );
        macro_materialization_click(&mut cold, "one");
        assert_eq!(cold.state.vars["input_count"], Value::Int(2));
        assert!(
            old.attached(),
            "independent cold Engine never steals the original backend parent"
        );
        assert_eq!(live.state.vars, saved_vars);
        assert_eq!(
            macro_materialization_cell(&live, "one", site),
            C::DoOnce {
                initialized: true,
                completed: true
            }
        );
    }
}

#[test]
fn standard_macro1021_connected_real_bevy_warm_keeps_cells_and_cold_resamples_producer(){
    use rvn_core::widget_class_source_0950::standard_macros::CellInspection as C;
    let(project,site)=macro_materialization_project_with_connected(true);
    let(mut live,handle)=warm(&project);
    let two=live.create_widget(UUID,"two",vec![Value::Int(2)]).unwrap();
    live.add_widget_to_viewport(&two,false,0).unwrap();handoff_zero_media_publication(&mut live);
    macro_materialization_click(&mut live,"one");
    let consumed_history=live.history.len();
    assert_eq!(live.history.entries().last().unwrap().state.vars["input_count"],Value::Int(0));
    assert!(live.history.entries().last().unwrap().state.ui.screens.iter().find(|i|i.name=="one").unwrap().standard_macro_frame.is_none(),"the first changed input records its actual pre-consumption state");
    macro_materialization_click(&mut live,"one");
    assert_eq!(live.history.len(),consumed_history,"a completed DoOnce is a real no-op and adds no rollback checkpoint");
    assert_eq!(live.state.vars["input_count"],Value::Int(1));
    let frame=live.state.ui.screens.iter().find(|i|i.name=="one").unwrap().standard_macro_frame.clone();
    let owner=live.widget_backend_owner(&handle).unwrap();let old=live.renderer.widget_backend.wrapper(&owner).unwrap().upgrade().unwrap();
    let story=live.state.random;let ui=live.state.ui.screens.iter().find(|i|i.name=="one").unwrap().random;
    let ui_random=live.state.ui.screens.iter().map(|i|(i.name.clone(),i.random)).collect::<std::collections::BTreeMap<_,_>>();
    // Undo a distinct, real input after one has consumed its cell. Rolling
    // back immediately after its no-op click would correctly undo the first
    // consumption itself, whose actual historical frame is None.
    macro_materialization_click(&mut live,"two");
    assert_eq!(live.state.vars["input_count"],Value::Int(2));
    assert_eq!(live.history.len(),consumed_history+1);
    {
        let checkpoint=&live.history.entries().last().unwrap().state;
        assert_eq!(checkpoint.vars["input_count"],Value::Int(1));
        assert_eq!(checkpoint.random,story);
        assert_eq!(checkpoint.ui.screens.iter().find(|i|i.name=="one").unwrap().standard_macro_frame,frame,"the chosen rollback entry must already own one's exact consumed cell");
        assert_eq!(checkpoint.ui.screens.iter().map(|i|(i.name.clone(),i.random)).collect::<std::collections::BTreeMap<_,_>>(),ui_random);
    }
    let lineage=live.instance_id();let epoch=live.runtime_epoch();assert!(live.rollback());assert_eq!(live.instance_id(),lineage);assert_ne!(live.runtime_epoch(),epoch);
    assert_eq!(live.state.vars["input_count"],Value::Int(1));assert_eq!(live.state.random,story);
    assert_eq!(live.state.ui.screens.iter().map(|i|(i.name.clone(),i.random)).collect::<std::collections::BTreeMap<_,_>>(),ui_random);
    assert_eq!(live.state.ui.screens.iter().find(|i|i.name=="one").unwrap().standard_macro_frame,frame);
    assert!(!old.attached());
    handoff_zero_media_publication(&mut live);macro_materialize_all(&mut live);
    let current=live.widget_instance_handle("one").unwrap();let new_owner=live.widget_backend_owner(&current).unwrap();
    assert_eq!(new_owner.lineage(),owner.lineage());assert_eq!(new_owner.target(),owner.target());
    let wrapper=live.renderer.widget_backend.wrapper(&new_owner).unwrap().upgrade().unwrap();assert!(wrapper.attached());assert_ne!(wrapper.generation(),old.generation());
    macro_materialization_click(&mut live,"one");assert_eq!(live.state.vars["input_count"],Value::Int(1));assert_eq!(live.state.random,story);assert_eq!(live.state.ui.screens.iter().find(|i|i.name=="one").unwrap().random,ui);
    assert_eq!(macro_materialization_cell(&live,"one",site),C::DoOnce{initialized:true,completed:true});
    let saved=live.capture_save15(&project.program(),None,18,"connected completed cold".into()).unwrap();let encoded=saved.encode().unwrap();let decoded=Save::decode(&encoded).unwrap();
    for save in[saved,decoded]{
        let mut cold=load_game(&project.root).unwrap().engine;assert_ne!(cold.instance_id(),lineage);cold.restore_save15(&project.program(),save).unwrap();
        assert!(cold.state.ui.screens.iter().all(|i|i.standard_macro_frame.is_none()));assert!(cold.add_widget_to_viewport(&handle,false,0).is_err());pending(&cold);
        handoff_zero_media_publication(&mut cold);macro_materialize_all(&mut cold);assert_eq!(macro_materialization_cell(&cold,"one",site),C::DoOnce{initialized:false,completed:false});
        let before_story=cold.state.random;let before_ui=cold.state.ui.screens.iter().find(|i|i.name=="one").unwrap().random;
        let mut expected_story=before_story;expected_story.integer(1,1).unwrap();let mut expected_ui=before_ui;expected_ui.integer(1,1).unwrap();
        macro_materialization_click(&mut cold,"one");let after=(cold.state.random,cold.state.ui.screens.iter().find(|i|i.name=="one").unwrap().random);
        assert!(after==(expected_story,before_ui)||after==(before_story,expected_ui),"fresh real owner samples exactly one connected RNG producer");assert_eq!(cold.state.vars["input_count"],Value::Int(2));
        macro_materialization_click(&mut cold,"one");assert_eq!((cold.state.random,cold.state.ui.screens.iter().find(|i|i.name=="one").unwrap().random),after);assert_eq!(cold.state.vars["input_count"],Value::Int(2));
    }
    assert_eq!(live.state.ui.screens.iter().find(|i|i.name=="one").unwrap().standard_macro_frame,frame);assert_eq!(live.state.vars["input_count"],Value::Int(1));
}
