//! Real loader and existing CPU handoff/restart harness. Not executed for0670.
use super::*;
use crate::portable_text_resources::{locale_selection_mode, LocaleSelectionMode};

#[test]
fn normal_locale_mode_comes_from_real_profile_even_when_domains_are_identical() {
    let root = native_project("mode-profile", "init {set seen=0 while seen<1 {set seen=seen+1}}\n", NodeKind::While);
    let normal = load_runtime_game(&root).unwrap();
    let strict = load_runtime_game_text_profile(&root, true).unwrap();
    let (normal_root, normal_declarations) = normal.closed_locales.as_ref().unwrap();
    let (strict_root, strict_declarations) = strict.closed_locales.as_ref().unwrap();
    assert_eq!(normal_declarations, strict_declarations);
    assert_eq!(normal.locale_cfg.available, strict.locale_cfg.available);
    assert_eq!(normal_root.selection_mode(), LocaleSelectionMode::NormalCompatible);
    assert_eq!(strict_root.selection_mode(), LocaleSelectionMode::StrictDeclared);
    let RuntimeLaunchOwner::Startup(normal_owner) = &normal.engine else { panic!("actual normal owner required") };
    let RuntimeLaunchOwner::Startup(strict_owner) = &strict.engine else { panic!("actual closed owner required") };
    assert_eq!(normal_owner.profile, strict_owner.profile);
    assert_eq!(normal_owner.owner.locale().unwrap().portable_content_digest().unwrap(),
               strict_owner.owner.locale().unwrap().portable_content_digest().unwrap());
    assert!(!root.join(rvn_core::debugger_provenance::SIDECAR).exists());
}

#[test]
fn normal_locale_mode_survives_real_registration_handoff_and_async_restart() {
    let (_, mut app) = runtime_app("mode-restart", "init {set seen=0 while seen<2 {set seen=seen+1}}\n", NodeKind::While);
    assert!(app.world().contains_resource::<debugger_startup::DebugBootstrap>());
    assert_eq!(locale_selection_mode(app.world()).unwrap(), LocaleSelectionMode::NormalCompatible);
    pump_runtime(&mut app);
    assert!(!app.world().contains_resource::<debugger_startup::DebugBootstrap>());
    let old = app.world().resource::<VnEngine>().0.instance_id();
    let epoch = app.world().resource::<VnEngine>().0.runtime_epoch();
    assert_eq!(locale_selection_mode(app.world()).unwrap(), LocaleSelectionMode::NormalCompatible);
    menu_action(&mut app, rvn_ui::Action::StartScene("start".into())).unwrap();
    let window = app.world_mut().query_filtered::<Entity, With<bevy::window::PrimaryWindow>>().single(app.world());
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    app.update();
    assert!(!app.world().contains_resource::<VnEngine>());
    assert_eq!(app.world().resource::<debugger_startup::DebugBootstrap>().profile, debugger_startup::BootstrapProfile::RuntimeRestart);
    assert_eq!(locale_selection_mode(app.world()).unwrap(), LocaleSelectionMode::NormalCompatible);
    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    await_restarted(&mut app, old);
    let engine = app.world().resource::<VnEngine>();
    assert_eq!(engine.0.runtime_epoch(), epoch + 1);
    assert_eq!(engine.0.state.vars["seen"], rvn_parser::Value::Int(2));
    assert!(!editor_debugger::enabled(engine));
    assert_eq!(locale_selection_mode(app.world()).unwrap(), LocaleSelectionMode::NormalCompatible);
}
