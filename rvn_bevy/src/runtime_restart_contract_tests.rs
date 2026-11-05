//! Draft child module of runtime_restart: real normal loader + real schedules.
//! Prepared only. No fake owner, mailbox, clear_current or manual asset removal.
use super::*;
use crate::normal_runtime_startup_tests as normal;
use rvn_graph::NodeKind;
use rvn_core::editor_debugger::StepKind;

fn parked_real_restart(name:&str)->App {
    let (_,mut app)=normal::runtime_app(name,"init {set seen=0 for item in [1,2] {set seen=seen+item}}\n",NodeKind::ForEach);
    normal::pump_runtime(&mut app);
    normal::menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();
    let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());
    app.world_mut().get_mut::<Window>(window).unwrap().focused=false;
    let until=std::time::Instant::now()+std::time::Duration::from_secs(20);
    loop {
        app.update();
        assert!(app.world().contains_resource::<Anchor>(),"Real First must park the genuine old Engine");
        let startup=app.world().resource::<DebugBootstrap>();
        assert_eq!(startup.profile,BootstrapProfile::RuntimeRestart);
        assert!(startup.failed.is_none(),"Actual startup failed: {:?}",startup.failed);
        if crate::portable_text_resources::live_receipt(app.world()).is_ok(){break;}
        assert!(std::time::Instant::now()<until,"Actual pending text providers did not settle");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(!app.world().contains_resource::<VnEngine>());
    app
}
fn ready_step(app:&mut App)->crate::debugger_startup::PreparedBootstrapStep {
    let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());
    app.world_mut().get_mut::<Window>(window).unwrap().focused=true;
    let mut prepared=crate::debugger_startup::prepare_world_step(app.world(),StepKind::Continue)
        .unwrap_or_else(|error|panic!("Positive control: actual restart preparation must succeed before old source mutation: {error}"));
    let until=std::time::Instant::now()+std::time::Duration::from_secs(20);
    while !crate::debugger_startup::refresh_world_step(app.world(),&mut prepared).unwrap(){
        assert!(std::time::Instant::now()<until,"Actual assets did not become ready");
        app.world_mut().get_mut::<Window>(window).unwrap().focused=false;
        app.update();
        app.world_mut().get_mut::<Window>(window).unwrap().focused=true;
        assert!(app.world().contains_resource::<Anchor>());
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    prepared
}
fn pending_identity(app:&App)->(rvn_core::editor_debugger::Snapshot,serde_json::Value,serde_json::Value,usize){
    let startup=app.world().resource::<DebugBootstrap>();
    (startup.owner.snapshot(),serde_json::to_value(startup.owner.state()).unwrap(),
     serde_json::to_value(&app.world().resource::<crate::scene_publication::ScenePresentation>().batch.as_ref().unwrap().state).unwrap(),
     app.world().resource::<Events<crate::vn_command::VnCommand>>().len())
}
fn mutate_actual_retained_owner(app:&mut App){
    let source=app.world().resource::<Anchor>().engine.0.audio_request_source().unwrap();
    app.world_mut().resource_mut::<Anchor>().engine.0.state.vars.insert("retained_owner_changed_after_admission".into(),rvn_parser::Value::Int(907));
    assert!(!app.world().resource::<Anchor>().engine.0.accepts_audio_request_source(&source),"The test must really revoke the admitted old source");
}
#[test]
fn normal_runtime_cutover_direct_world_prepare_rejects_stale_retained_owner(){
    let mut app=parked_real_restart("direct-prepare-retirement");
    let positive=ready_step(&mut app);drop(positive);
    mutate_actual_retained_owner(&mut app);let before=pending_identity(&app);
    let error=crate::debugger_startup::prepare_world_step(app.world(),StepKind::Continue).err()
        .expect("A direct World call must refuse the genuinely changed retained source before Core preparation");
    assert!(error.contains("retirement"),"Wrong boundary refused the fixture: {error}");
    assert_eq!(pending_identity(&app),before,"Rejection must not change current VM/RNG, accepted publication or command queue");
    assert!(app.world().contains_resource::<Anchor>());assert!(!app.world().contains_resource::<VnEngine>());
}
#[test]
fn normal_runtime_cutover_direct_world_refresh_and_commit_reject_stale_retained_owner(){
    let mut app=parked_real_restart("direct-refresh-commit-retirement");
    let mut refresh_plan=ready_step(&mut app);let commit_plan=ready_step(&mut app);
    mutate_actual_retained_owner(&mut app);let before=pending_identity(&app);
    let refresh_error=crate::debugger_startup::refresh_world_step(app.world(),&mut refresh_plan).err()
        .expect("Direct refresh must revalidate the actual retained old source");
    assert!(refresh_error.contains("retirement"),"Wrong refresh boundary: {refresh_error}");assert_eq!(pending_identity(&app),before);
    let commit_error=crate::debugger_startup::commit_world_step(app.world_mut(),commit_plan).err()
        .expect("Direct commit must revalidate the actual retained old source independently of Native video");
    assert!(commit_error.contains("retirement"),"Wrong commit boundary: {commit_error}");assert_eq!(pending_identity(&app),before);
    assert!(app.world().contains_resource::<Anchor>());assert!(!app.world().contains_resource::<VnEngine>());
}

#[test]
fn normal_runtime_cutover_direct_world_first_prepare_rejects_lost_retained_owner(){
    let mut app=parked_real_restart("direct-lost-anchor");
    // No prior prepare: real source readiness comes from AssetServer schedules.
    let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());
    app.world_mut().get_mut::<Window>(window).unwrap().focused=true;
    let retained=app.world_mut().remove_resource::<Anchor>().unwrap();
    assert!(retained.engine.0.audio_request_source().is_ok());
    let before=pending_identity(&app);
    let error=crate::debugger_startup::prepare_world_step(app.world(),StepKind::Continue).err()
        .expect("First direct prepare must not interpret loss of the real retained owner as an already published restart");
    assert!(error.contains("retirement"),"Wrong first-prepare boundary: {error}");
    assert_eq!(pending_identity(&app),before);assert!(!app.world().contains_resource::<VnEngine>());
    drop(retained);
}
#[test]
fn normal_runtime_cutover_generic_factory_cannot_create_restart_without_admitted_intent(){
    let(_,mut app)=normal::runtime_app("generic-restart-refusal","init {for item in [1,2] {set seen=item}}\n",NodeKind::ForEach);
    let actual=app.world_mut().remove_resource::<DebugBootstrap>().unwrap();
    let error=crate::debugger_startup::from_runtime_owner(actual.owner,actual.broker,None,BootstrapProfile::RuntimeRestart).err()
        .expect("A generic runtime factory must not create Restart without the real old-owner admission intent");
    assert!(error.contains("initial"),"Wrong factory boundary: {error}");
}

// PREPARED ONLY: real UiFocus/layout/physical event path, no manual Interaction.
fn actual_settings_target(app:&mut App)->Entity{
    let target=app.world_mut().query::<(Entity,&crate::systems::title::TitleButton)>()
        .iter(app.world()).find_map(|(entity,button)|matches!(button,crate::systems::title::TitleButton::Settings).then_some(entity))
        .expect("The genuine published title has a Settings button");
    assert!(app.world().resource::<bevy::ui::UiStack>().uinodes.contains(&target));
    assert!(app.world().get::<bevy::render::view::ViewVisibility>(target).unwrap().get());
    assert!(app.world().get::<Node>(target).unwrap().size().cmpgt(Vec2::ZERO).all());
    target
}
fn actual_pointer(app:&mut App,target:Entity,state:bevy::input::ButtonState){
    let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());
    let point=app.world().get::<Node>(target).unwrap().logical_rect(app.world().get::<GlobalTransform>(target).unwrap()).center();
    let scale=app.world().get::<Window>(window).unwrap().scale_factor()*app.world().resource::<bevy::ui::UiScale>().0;
    app.world_mut().get_mut::<Window>(window).unwrap().set_physical_cursor_position(Some((point*scale).as_dvec2()));
    app.world_mut().send_event(bevy::input::mouse::MouseButtonInput{button:MouseButton::Left,state,window});
}
#[test]
fn normal_runtime_cutover_held_actual_ui_press_is_not_replayed_after_refusal(){
    use bevy::input::ButtonState;
    let mut app=parked_real_restart("pointer-held-refusal");
    let target=actual_settings_target(&mut app);
    let focus=app.world().resource::<bevy::a11y::Focus>().0;
    let color=app.world().get::<BackgroundColor>(target).unwrap().0;
    actual_pointer(&mut app,target,ButtonState::Pressed);app.update();
    assert!(app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
    assert_ne!(*app.world().get::<Interaction>(target).unwrap(),Interaction::Pressed,"The actual UiFocus press must be quarantined while retained");
    assert_eq!(app.world().get::<BackgroundColor>(target).unwrap().0,color,"Quarantine changes no retained presenter colour");
    assert_eq!(app.world().resource::<bevy::a11y::Focus>().0,focus);
    assert!(!app.world().resource::<crate::systems::settings_menu::SettingsMenuState>().active);
    let old=app.world().resource::<Anchor>().engine.0.instance_id();
    app.world_mut().resource_mut::<DebugBootstrap>().broker=crate::checkpoint_broker::AudioCheckpointBroker::default();
    for _ in 0..3{app.update();}
    assert_eq!(app.world().resource::<VnEngine>().0.instance_id(),old);
    assert!(app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left),"There was no fresh release or press during restoration");
    assert_eq!(app.world().resource::<State<VnState>>().get(),&VnState::TitleScreen);
    assert!(!app.world().resource::<crate::systems::settings_menu::SettingsMenuState>().active,"A held old press must not open Settings after failure");
    assert_eq!(*app.world().get::<Interaction>(target).unwrap(),Interaction::Hovered,"Real UiFocus resumes current hover, without a click");
    actual_pointer(&mut app,target,ButtonState::Released);app.update();
    actual_pointer(&mut app,target,ButtonState::Pressed);app.update();app.update();
    assert!(app.world().resource::<crate::systems::settings_menu::SettingsMenuState>().active,"A genuinely new press must still work after refusal");
    assert_eq!(app.world().resource::<State<VnState>>().get(),&VnState::Menu);
}
#[test]
fn normal_runtime_cutover_direct_restore_quarantines_real_ui_focus_press(){
    use bevy::ecs::system::RunSystemOnce;
    let mut app=parked_real_restart("pointer-direct-restore");let target=actual_settings_target(&mut app);
    actual_pointer(&mut app,target,bevy::input::ButtonState::Pressed);
    // Exercise the actual input producer without the schedule quarantine, as
    // may happen before a direct failure/installer API is called this frame.
    app.world_mut().run_system_once(bevy::input::mouse::mouse_button_input_system);
    app.world_mut().run_system_once(bevy::ui::ui_focus_system);
    assert_eq!(*app.world().get::<Interaction>(target).unwrap(),Interaction::Pressed,"Positive control must hit the actual laid out title button");
    assert!(restore_before_publication(app.world_mut(),"Explicit direct failure boundary"));
    assert_ne!(*app.world().get::<Interaction>(target).unwrap(),Interaction::Pressed);
    for _ in 0..3{app.update();}
    assert!(!app.world().resource::<crate::systems::settings_menu::SettingsMenuState>().active);
}
#[test]
fn normal_runtime_cutover_restore_before_input_system_drops_queued_old_pointer_press(){
    let mut app=parked_real_restart("pointer-queued-before-restore");let target=actual_settings_target(&mut app);
    actual_pointer(&mut app,target,bevy::input::ButtonState::Pressed);
    assert!(!app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left),"The real physical event has not reached InputSystem yet");
    assert!(restore_before_publication(app.world_mut(),"First/direct failure before InputSystem"));
    // First-style restoration precedes InputSystem and UiFocus. The real frame
    // stamp must keep their eventual old click from reaching the restored UI.
    for _ in 0..3{app.update();}
    assert!(app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
    assert!(!app.world().resource::<crate::systems::settings_menu::SettingsMenuState>().active);
    assert_eq!(app.world().resource::<State<VnState>>().get(),&VnState::TitleScreen);
}
#[test]
fn normal_runtime_cutover_pointer_quarantine_preserves_actual_host_panel_buttons(){
    use bevy::input::{ButtonState,keyboard::{KeyboardInput,Key}};
    let mut app=parked_real_restart("pointer-host-panel");
    let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());
    app.world_mut().send_event(KeyboardInput{key_code:KeyCode::F8,logical_key:Key::F8,state:ButtonState::Pressed,window});
    for _ in 0..3{app.update();}
    assert!(app.world().resource::<crate::accessibility::Accessibility>().open);
    assert!(app.world().contains_resource::<Anchor>());
    let panel=app.world_mut().query_filtered::<Entity,With<crate::accessibility::PanelButton>>().iter(app.world())
        .find(|entity|app.world().get::<bevy::render::view::ViewVisibility>(*entity).is_some_and(|v|v.get())&&app.world().get::<Node>(*entity).is_some_and(|node|node.size().cmpgt(Vec2::ZERO).all())).expect("The actual host panel must create a visible, laid out button");
    actual_pointer(&mut app,panel,ButtonState::Pressed);
    // Inspect after the real Focus + quarantine schedule, before host Update
    // may redraw its controls in response to this genuine press.
    app.world_mut().run_schedule(PreUpdate);
    assert_eq!(*app.world().get::<Interaction>(panel).unwrap(),Interaction::Pressed,"Actual host pointer controls must remain enabled under Anchor");
    assert!(app.world().contains_resource::<Anchor>());
}

fn actual_assistive_event(app:&mut App,target:Entity,action:bevy::a11y::accesskit::Action){
 app.world_mut().send_event(bevy::a11y::ActionRequest(bevy::a11y::accesskit::ActionRequest{action,target:bevy::a11y::accesskit::NodeId(target.to_bits()),data:None}));
}
#[test]
fn normal_runtime_cutover_buffered_assistive_action_does_not_replay_on_restore(){
 use bevy::a11y::accesskit::Action;
 let mut app=parked_real_restart("raw-assistive-restore");let target=actual_settings_target(&mut app);
 let focus=app.world().resource::<bevy::a11y::Focus>().0;
 actual_assistive_event(&mut app,target,Action::Focus);actual_assistive_event(&mut app,target,Action::Default);
 assert!(restore_before_publication(app.world_mut(),"Direct failure before assistive consumer"));
 app.world_mut().run_schedule(PreUpdate);
 assert_eq!(app.world().resource::<bevy::a11y::Focus>().0,focus,"An event buffered under Anchor must not focus the restored story");
 assert_ne!(*app.world().get::<Interaction>(target).unwrap(),Interaction::Pressed);
 for _ in 0..3{app.update();}
 assert!(!app.world().resource::<crate::systems::settings_menu::SettingsMenuState>().active);
 assert_eq!(app.world().resource::<State<VnState>>().get(),&VnState::TitleScreen);
 actual_assistive_event(&mut app,target,Action::Focus);app.world_mut().run_schedule(PreUpdate);
 assert_eq!(app.world().resource::<bevy::a11y::Focus>().0,Some(target),"A genuinely new assistive request remains usable");
 actual_assistive_event(&mut app,target,Action::Default);app.update();app.update();
 assert!(app.world().resource::<crate::systems::settings_menu::SettingsMenuState>().active);
}
#[test]
fn normal_runtime_cutover_buffered_keyboard_chord_does_not_activate_restored_title(){
 use bevy::input::{ButtonState,keyboard::{KeyboardInput,Key}};
 let(_,mut app)=normal::runtime_app("raw-keyboard-restore","init {while false {set unreachable=1}}\n",NodeKind::While);
 normal::pump_runtime(&mut app);let target=actual_settings_target(&mut app);
 assert!(app.world().get::<crate::menu_documents::MenuFocus>(target).is_some(),"Positive control must use the actual legacy keyboard projection");
 actual_assistive_event(&mut app,target,bevy::a11y::accesskit::Action::Focus);app.world_mut().run_schedule(PreUpdate);
 assert_eq!(app.world().resource::<crate::menu_documents::Menus>().keyboard_focus(),Some(target));
 normal::menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();
 let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());
 app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();assert!(app.world().contains_resource::<Anchor>());
 app.world_mut().send_event(KeyboardInput{key_code:KeyCode::Enter,logical_key:Key::Enter,state:ButtonState::Pressed,window});
 assert!(restore_before_publication(app.world_mut(),"Direct failure before keyboard consumer"));
 for _ in 0..3{app.update();}
 assert!(!app.world().resource::<crate::systems::settings_menu::SettingsMenuState>().active);
 assert_eq!(app.world().resource::<State<VnState>>().get(),&VnState::TitleScreen);
 app.world_mut().send_event(KeyboardInput{key_code:KeyCode::Enter,logical_key:Key::Enter,state:ButtonState::Released,window});app.update();
 app.world_mut().send_event(KeyboardInput{key_code:KeyCode::Enter,logical_key:Key::Enter,state:ButtonState::Pressed,window});app.update();app.update();
 assert!(app.world().resource::<crate::systems::settings_menu::SettingsMenuState>().active,"A fresh keyboard activation must still reach the genuine Settings button");
}

#[test]
fn normal_runtime_cutover_held_modifier_then_fresh_tab_release_keeps_ordered_chord(){
 use bevy::input::{ButtonState,keyboard::{KeyboardInput,Key}};
 let(_,mut app)=normal::runtime_app("raw-held-modifier","init {while false {set unreachable=1}}\n",NodeKind::While);
 normal::pump_runtime(&mut app);let target=actual_settings_target(&mut app);
 actual_assistive_event(&mut app,target,bevy::a11y::accesskit::Action::Focus);app.world_mut().run_schedule(PreUpdate);
 assert_eq!(app.world().resource::<crate::menu_documents::Menus>().keyboard_focus(),Some(target));
 normal::menu_action(&mut app,rvn_ui::Action::NewGame).unwrap();let window=app.world_mut().query_filtered::<Entity,With<bevy::window::PrimaryWindow>>().single(app.world());
 app.world_mut().get_mut::<Window>(window).unwrap().focused=false;app.update();assert!(app.world().contains_resource::<Anchor>());
 app.world_mut().send_event(KeyboardInput{key_code:KeyCode::ShiftLeft,logical_key:Key::Shift,state:ButtonState::Pressed,window});app.update();
 assert!(app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::ShiftLeft));
 assert!(restore_before_publication(app.world_mut(),"Restore while real Shift remains held"));app.update();
 let mut items=app.world_mut().query::<(Entity,&crate::menu_documents::MenuFocus)>().iter(app.world()).map(|(entity,focus)|(focus.0,entity,focus.1.clone())).collect::<Vec<_>>();items.sort_by_key(|(order,_,key)|(*order,key.clone()));
 let current=items.iter().position(|(_,entity,_)|*entity==target).unwrap();assert!(items.len()>2);
 let expected=items[(current+items.len()-1)%items.len()].1;
 app.world_mut().send_event(KeyboardInput{key_code:KeyCode::Tab,logical_key:Key::Tab,state:ButtonState::Pressed,window});
 app.world_mut().send_event(KeyboardInput{key_code:KeyCode::ShiftLeft,logical_key:Key::Shift,state:ButtonState::Released,window});app.update();
 assert!(!app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::ShiftLeft),"The physical final state differs from Shift at Tab's ordered keydown");
 assert_eq!(app.world().resource::<crate::menu_documents::Menus>().keyboard_focus(),Some(expected),"Suspended Ctrl/Shift edges must not destroy the baseline for a genuinely new ordered chord");
}
