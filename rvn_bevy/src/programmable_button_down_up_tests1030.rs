//! CPU schedule/real Core class + publication tests. No WindowCreated, input
//! permission or accepted renderer state is forged. Geometry is a CPU fixture.
use super::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::input::mouse::MouseButtonInput;
use bevy::winit::WinitEvent;
use rvn_core::widget_class_source_0950::{AdmittedWidgetClasses,CompilerWidgetClass,CompilerWidgetMember,WidgetMemberKind};
use rvn_parser::widget_asset::{WidgetBlueprintV1,WidgetGraphRef16,WidgetIndexInput,LocatorCase};
const UUID:&str="a26f04b5ebd141299253e20416566023";
const SOURCE:&str=r#"handler clicked(event){set clicks=clicks+1}
handler focused(event){set focuses=focuses+1}
screen form(){return component("root","column",{"layout_contract":"widget_v1"},[component("button","button",{"parent_slot":{"kind":"vertical_box","padding":{"left":8,"top":8,"right":8,"bottom":8}},"events":{"click":"clicked","focus":"focused"}},[component("button-label","text",{"parent_slot":{"kind":"button","padding":{"left":6,"top":4,"right":6,"bottom":4},"horizontal":"center","vertical":"center"},"text":"Press","font_size":18},[])])])}
init{set clicks=0 set focuses=0 set foreign=0}
scene label start
"Waiting"
return
"#;
struct Fixture {app:App,window:Entity,button:Entity,_root:std::path::PathBuf}
fn fixture()->Fixture{fixture_source(SOURCE)}
fn fixture_source(input_source:&str)->Fixture{
    let path=std::env::temp_dir().join(format!("rvn-button-down-up1030-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&path).unwrap();let root=std::fs::canonicalize(path).unwrap();let source=root.join("main.rvn");std::fs::write(&source,input_source).unwrap();
    let mut project=rvn_graph::SourceProject::open_at(&source,input_source,&[]).unwrap();
    let mut graphs=project.graphs().to_vec();for graph in &mut graphs{graph.enable_native_frame_preparation().unwrap();}
    project.apply_visual(input_source,&graphs).unwrap();std::fs::write(&source,project.source()).unwrap();
    let program=project.resolved_program().unwrap();
    let members=project.graphs().iter().filter(|g|matches!(&g.kind,rvn_graph::GraphKind::Screen{name}if name=="form")||matches!(&g.kind,rvn_graph::GraphKind::Handler{name}if name=="clicked"||name=="focused")).map(|g|WidgetGraphRef16::from_u64(g.graph_id.get())).collect::<Vec<_>>();
    let screen=project.graphs().iter().find(|g|matches!(&g.kind,rvn_graph::GraphKind::Screen{name}if name=="form")).unwrap();
    let bytes=WidgetBlueprintV1{uuid:UUID.into(),source:"main.rvn".into(),screen_graph:WidgetGraphRef16::from_u64(screen.graph_id.get()),members}.encode().unwrap();
    let entries=[(UUID,Some("form.rvnwidget"))];
    let prepared=rvn_graph::prepare_widget_runtime_classes(&root,&program,WidgetIndexInput::Table(&entries),LocatorCase::Insensitive,&[rvn_graph::WidgetRuntimeAsset{locator:"form.rvnwidget",bytes:bytes.as_bytes()}],&[rvn_graph::WidgetRuntimeSource{source:"main.rvn",project:&project}]).unwrap();
    let classes=AdmittedWidgetClasses::admit(&program,prepared.classes().values().map(|class|CompilerWidgetClass{index_uuid:class.uuid().into(),locator:class.locator().into(),descriptor_bytes:class.descriptor_bytes().to_vec(),members:class.members().iter().map(|m|CompilerWidgetMember{graph:m.graph().get(),kind:match m.kind(){rvn_graph::WidgetDefinitionKind::Screen=>WidgetMemberKind::Screen,rvn_graph::WidgetDefinitionKind::Handler=>WidgetMemberKind::Handler,rvn_graph::WidgetDefinitionKind::Function=>WidgetMemberKind::Function},definition:m.definition().into(),root_index:m.root_index(),origin:m.origin().clone()}).collect()}).collect()).unwrap();
    let mut engine=rvn_core::Engine::from_program_with_widget_classes(program,classes,crate::bevy_renderer::BevyRenderer::new(),32).unwrap();engine.step_until_interaction().unwrap();
    let handle=engine.create_widget(UUID,"form",vec![]).unwrap();engine.add_widget_to_viewport(&handle,false,0).unwrap();
    // Reuse the existing schedule/resources, not its legacy definition/owner.
    let (mut app,window,_)=super::super::scoped_ui_input_contracts::fixture("screen form(){return component(\"root\",\"panel\",{},[component(\"button\",\"button\",{},[])])}\ninit{set clicks=0}\nlabel start\nui.open(\"form\",[],false,0)\n\"Waiting\"\n");
    app.insert_resource(VnEngine(engine)).init_resource::<crate::scene_publication::ScenePresentation>().init_resource::<crate::resources::VnRenderState>().init_resource::<crate::resources::ChoiceFocus>().init_resource::<crate::resources::ImagemapState>().init_resource::<crate::resources::TypewriterState>();
    crate::scene_publication::consume(app.world_mut());
    super::super::scoped_ui_input_contracts::install_completed_owner_geometry_for_test(app.world_mut());
    assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
    let camera=app.world().iter_entities().find(|e|e.contains::<Camera>()).unwrap().id();
    let button=super::super::scoped_ui_input_contracts::pointer_node(app.world_mut(),Some("button"),camera,Vec2::new(100.0,80.0),Vec2::new(120.0,40.0));
    let actual=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();app.world_mut().entity_mut(button).insert(ControlSource(actual.clone()));
    app.world_mut().resource_mut::<bevy::ui::UiStack>().uinodes=vec![button];
    {let mut s=app.world_mut().resource_mut::<Screens>();s.rendered_sources.insert("form".into(),actual);s.rendered=s.views.clone();s.keyboard_focus=None;}
    app.add_event::<WinitEvent>().add_systems(PreUpdate,super::capture.after(capture_input_sources));
    Fixture{app,window,button,_root:root}
}
fn cursor(f:&mut Fixture,point:Vec2){f.app.world_mut().get_mut::<Window>(f.window).unwrap().set_cursor_position(Some(point));f.app.world_mut().send_event(WinitEvent::CursorMoved(bevy::window::CursorMoved{window:f.window,position:point,delta:None}));}
fn edge(f:&mut Fixture,state:ButtonState){let e=MouseButtonInput{button:MouseButton::Left,state,window:f.window};f.app.world_mut().send_event(e);f.app.world_mut().send_event(WinitEvent::MouseButtonInput(e));}
fn hits(f:&Fixture)->Value{f.app.world().resource::<VnEngine>().0.state.vars["clicks"].clone()}
fn consume(f:&mut Fixture){super::super::scoped_ui_input_contracts::consume_completed_owner_publication_for_test(&mut f.app);}
fn down(f:&mut Fixture){cursor(f,Vec2::new(100.0,80.0));edge(f,ButtonState::Pressed);f.app.update();assert_eq!(hits(f),Value::Int(0));assert!(f.app.world().resource::<Screens>().widget_buttons1030.press.is_some());}
fn focus_ready(f:&mut Fixture){consume(f);f.app.update();assert!(f.app.world().resource::<Screens>().widget_buttons1030.press.as_ref().unwrap().focus.is_none());}
// A real installed Scene timer advances a foreign Core clock without firing
// jump/call. execute_timer_action accepts no arbitrary RVN assignment.
fn fixture_with_foreign_clock()->Fixture{
    fixture_source(&SOURCE.replace("scene label start\n","scene label start\ntimer 3600 => jump start\n"))
}
fn advance_foreign_clock(f:&mut Fixture){
    let definition=widget_definition_digest(&f.app.world().resource::<Screens>().views[0]).unwrap();
    let mut game=f.app.world_mut().resource_mut::<VnEngine>();
    let engine=&mut game.0;
    let source=engine.capture_ui_input_source("form").unwrap();
    let full=engine.debug_state_source().unwrap();
    let token=engine.timer_token().expect("actual installed Scene timer");
    let active=engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap();
    let activation=active.id;let task=active.tasks.timer.clone().unwrap();
    assert_eq!(task.action,"jump start");assert_eq!(task.duration,3600.0);
    let position=engine.state.pc;let vars=engine.state.vars.clone();
    let ui=serde_json::to_vec(&engine.state.ui).unwrap();
    let queued=engine.renderer.scene_batch.as_ref().map(|batch|serde_json::to_vec(&batch.state).unwrap());
    assert!(engine.tick_scene_tasks(0.125).unwrap());
    let active=engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap();
    let next=active.tasks.timer.as_ref().unwrap();
    assert_eq!(active.id,activation);assert_eq!(engine.timer_token(),Some(token));
    assert_eq!(next.action,task.action);assert_eq!(next.duration,task.duration);assert_eq!(next.revision,task.revision);
    assert_eq!(next.elapsed,task.elapsed+0.125);assert!(next.elapsed<next.duration);
    assert_eq!(engine.state.pc,position);assert_eq!(engine.state.vars,vars);
    assert_eq!(serde_json::to_vec(&engine.state.ui).unwrap(),ui);
    assert_eq!(engine.capture_ui_input_source("form").unwrap(),source);
    assert!(!engine.accepts_debug_state_source(&full),"the genuine full Core state changed");
    assert_eq!(engine.renderer.scene_batch.as_ref().map(|batch|serde_json::to_vec(&batch.state).unwrap()),queued,"clock progress publishes no UI batch");
    drop(game);
    assert_eq!(widget_definition_digest(&f.app.world().resource::<Screens>().views[0]).unwrap(),definition);
}
fn republish_foreign_clock(f:&mut Fixture){
    let mut game=f.app.world_mut().resource_mut::<VnEngine>();let engine=&mut game.0;
    let original=serde_json::to_vec(&engine.renderer.scene_batch.as_ref().expect("actual own-Focus batch remains pending").state).unwrap();
    let current=serde_json::to_vec(&engine.state).unwrap();
    assert_ne!(original,current,"the real pending Focus predates the foreign clock progress");
    let full=engine.debug_state_source().unwrap();
    engine.publish_current_scene_interaction().unwrap();
    assert!(engine.accepts_debug_state_source(&full),"republication executes no statement or input");
    assert!(engine.renderer.scene_batch_pending());
    assert_eq!(serde_json::to_vec(&engine.renderer.scene_batch.as_ref().unwrap().state).unwrap(),current,"the newly built real batch carries the foreign state");
}

#[test]
fn widget_button1030_hold_has_no_click_and_inside_release_fires_once(){let mut f=fixture();down(&mut f);focus_ready(&mut f);for _ in 0..3{f.app.update();assert_eq!(hits(&f),Value::Int(0));}edge(&mut f,ButtonState::Released);f.app.update();assert_eq!(hits(&f),Value::Int(1));consume(&mut f);f.app.update();assert_eq!(hits(&f),Value::Int(1));}
#[test]
fn widget_button1030_release_outside_and_down_outside_up_inside_do_not_click(){for began_inside in [false,true]{let mut f=fixture();if began_inside{down(&mut f);focus_ready(&mut f);cursor(&mut f,Vec2::new(500.0,500.0));}else{cursor(&mut f,Vec2::new(500.0,500.0));edge(&mut f,ButtonState::Pressed);f.app.update();cursor(&mut f,Vec2::new(100.0,80.0));}edge(&mut f,ButtonState::Released);f.app.update();assert_eq!(hits(&f),Value::Int(0));}}
#[test]
fn widget_button1030_ordered_short_click_and_outside_origin_same_frame(){for inside in [false,true]{let mut f=fixture();cursor(&mut f,if inside{Vec2::new(100.0,80.0)}else{Vec2::new(500.0,500.0)});edge(&mut f,ButtonState::Pressed);cursor(&mut f,Vec2::new(100.0,80.0));edge(&mut f,ButtonState::Released);f.app.update();if inside{consume(&mut f);f.app.update();}assert_eq!(hits(&f),Value::Int(if inside{1}else{0}));}}
#[test]
fn widget_button1030_pending_release_keeps_frozen_outside_verdict_after_later_hover(){let mut f=fixture();down(&mut f);cursor(&mut f,Vec2::new(500.0,500.0));edge(&mut f,ButtonState::Released);f.app.update();cursor(&mut f,Vec2::new(100.0,80.0));consume(&mut f);f.app.update();assert_eq!(hits(&f),Value::Int(0));}
#[test]
fn widget_button1030_focus_lost_disabled_hidden_and_entity_replacement_cancel(){for mode in 0..4{let mut f=fixture();down(&mut f);focus_ready(&mut f);match mode{0=>{f.app.world_mut().send_event(WinitEvent::WindowFocused(bevy::window::WindowFocused{window:f.window,focused:false}));},1|2=>{let mut s=f.app.world_mut().resource_mut::<Screens>();s.views[0].root.visit_mut(&mut |c|if c.id=="button"{if mode==1{c.enabled=false}else{c.visible=false}});},_=>{f.app.world_mut().entity_mut(f.button).despawn();}}edge(&mut f,ButtonState::Released);f.app.update();assert_eq!(hits(&f),Value::Int(0));}}
#[test]
fn widget_button1030_removed_readded_same_alias_cannot_inherit_press(){let mut f=fixture();down(&mut f);focus_ready(&mut f);let original=f.app.world().resource::<Screens>().widget_buttons1030.press.as_ref().unwrap().original.source.clone();{let mut e=f.app.world_mut().resource_mut::<VnEngine>();let h=e.0.widget_instance_handle("form").unwrap();e.0.remove_widget_from_viewport(&h).unwrap();}consume(&mut f);{let mut e=f.app.world_mut().resource_mut::<VnEngine>();let h=e.0.widget_instance_handle("form").unwrap();e.0.add_widget_to_viewport(&h,false,0).unwrap();}consume(&mut f);assert!(!f.app.world().resource::<VnEngine>().0.ui_input_source_current(&original));edge(&mut f,ButtonState::Released);f.app.update();assert_eq!(hits(&f),Value::Int(0));}
#[test]
fn widget_button1030_no_focus_completion_is_not_an_installed_receipt(){let mut f=fixture();down(&mut f);edge(&mut f,ButtonState::Released);f.app.update();assert_eq!(hits(&f),Value::Int(0));assert!(f.app.world().resource::<Screens>().widget_buttons1030.press.as_ref().unwrap().focus.is_some());}
#[test]
fn widget_button1030_own_focus_source_revalidated_after_preupdate_before_send(){for foreign in [false,true]{let mut f=fixture_with_foreign_clock();down(&mut f);cursor(&mut f,Vec2::new(100.0,80.0));edge(&mut f,ButtonState::Released);f.app.update();consume(&mut f);super::capture(f.app.world_mut());assert!(f.app.world().resource::<Screens>().widget_buttons1030.press.as_ref().unwrap().focus.is_some());if foreign{advance_foreign_clock(&mut f);consume(&mut f);}f.app.world_mut().run_system_once(input);assert_eq!(hits(&f),Value::Int(if foreign{0}else{1}));}}
#[test]
fn widget_button1030_completed_focus_allows_unrelated_progress_during_hold(){let mut f=fixture_with_foreign_clock();down(&mut f);focus_ready(&mut f);advance_foreign_clock(&mut f);consume(&mut f);f.app.update();edge(&mut f,ButtonState::Released);f.app.update();assert_eq!(hits(&f),Value::Int(1));}
#[test]
fn widget_button1030_focus_only_keeps_definition_but_root_order_modality_layer_change_it(){let f=fixture();let original=f.app.world().resource::<Screens>().views[0].clone();let digest=widget_definition_digest(&original).unwrap();let mut focus=original.clone();focus.focus=Some("button".into());assert_eq!(widget_definition_digest(&focus).unwrap(),digest);for mode in 0..4{let mut changed=original.clone();match mode{0=>changed.order+=1,1=>changed.modal=!changed.modal,2=>changed.layer+=1,_=>changed.root.enabled=false}assert_ne!(widget_definition_digest(&changed).unwrap(),digest);}}
#[test]
fn widget_button1030_real_focus_publication_keeps_original_entity_and_source(){let mut f=fixture();let source=f.app.world().get::<ControlSource>(f.button).unwrap().0.clone();down(&mut f);focus_ready(&mut f);assert_eq!(f.app.world().get::<ControlSource>(f.button).unwrap().0,source);assert_eq!(f.app.world().resource::<VnEngine>().0.state.vars["focuses"],Value::Int(1));assert_eq!(hits(&f),Value::Int(0));}
#[test]
fn widget_button1030_cancelled_press_and_release_without_down_never_call_handler(){let mut f=fixture();cursor(&mut f,Vec2::new(100.0,80.0));edge(&mut f,ButtonState::Released);f.app.update();assert_eq!(hits(&f),Value::Int(0));down(&mut f);cancel_runtime_input(f.app.world_mut());consume(&mut f);edge(&mut f,ButtonState::Released);f.app.update();assert_eq!(hits(&f),Value::Int(0));}

#[test]
fn widget_button1030_admitted_image_release_during_own_focus_retains_hit_not_future_hover(){
    use rvn_core::editor_preview::{ImageInput,PointerKind};
    for inside in [false,true]{
        let mut f=fixture();let frame=super::super::scoped_ui_input_contracts::completed_native_image(f.app.world(),1);
        crate::editor_preview_input::inject(f.app.world_mut(),frame.clone(),ImageInput::Pointer{kind:PointerKind::Down,button:0,point:[100.0,80.0],wheel:[0.0;2]}).unwrap();f.app.update();
        assert!(f.app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());assert_eq!(hits(&f),Value::Int(0));
        let up=ImageInput::Pointer{kind:PointerKind::Up,button:0,point:if inside{[100.0,80.0]}else{[500.0,500.0]},wheel:[0.0;2]};
        crate::editor_preview_input::capture_widget_button_neutral_release1030(f.app.world_mut(),&frame,&up);
        crate::editor_preview_input::neutralize_passive(f.app.world_mut(),&up).unwrap();
        assert_eq!(hits(&f),Value::Int(0));cursor(&mut f,if inside{Vec2::new(500.0,500.0)}else{Vec2::new(100.0,80.0)});
        consume(&mut f);f.app.update();assert_eq!(hits(&f),Value::Int(if inside{1}else{0}));
    }
}
#[test]
fn widget_button1030_image_focusloss_and_foreign_pending_do_not_acquire_a_release(){
    use rvn_core::editor_preview::{ImageInput,PointerKind};
    for foreign in [false,true]{
        let mut f=fixture_with_foreign_clock();let frame=super::super::scoped_ui_input_contracts::completed_native_image(f.app.world(),1);
        crate::editor_preview_input::inject(f.app.world_mut(),frame.clone(),ImageInput::Pointer{kind:PointerKind::Down,button:0,point:[100.0,80.0],wheel:[0.0;2]}).unwrap();f.app.update();
        if foreign{advance_foreign_clock(&mut f);republish_foreign_clock(&mut f);}
        else{crate::editor_preview_input::neutralize_passive(f.app.world_mut(),&ImageInput::FocusLost).unwrap();}
        let up=ImageInput::Pointer{kind:PointerKind::Up,button:0,point:[100.0,80.0],wheel:[0.0;2]};
        crate::editor_preview_input::capture_widget_button_neutral_release1030(f.app.world_mut(),&frame,&up);crate::editor_preview_input::neutralize_passive(f.app.world_mut(),&up).unwrap();
        consume(&mut f);f.app.update();assert_eq!(hits(&f),Value::Int(0));assert!(f.app.world().resource::<Screens>().widget_buttons1030.press.is_none());
    }
}

fn rendered_fixture(input_source:&str)->Fixture{
    let mut f=fixture_source(input_source);
    let dummy=f.app.world().iter_entities().filter(|entity|entity.contains::<Control>()).map(|entity|entity.id()).collect::<Vec<_>>();
    for entity in dummy{f.app.world_mut().entity_mut(entity).despawn();}
    let root=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
    f.app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{file_path:root.to_str().unwrap().into(),watch_for_changes_override:Some(false),..default()}))
        .init_asset::<Font>().init_asset::<Image>().init_asset::<Shader>().init_asset::<Mesh>().init_asset::<bevy::sprite::TextureAtlasLayout>()
        .init_resource::<WidgetLayouts>()
        .add_plugins(bevy::transform::TransformPlugin).add_plugins(bevy::render::camera::CameraPlugin)
        .add_plugins(bevy::render::view::VisibilityPlugin).add_plugins(bevy::text::TextPlugin).add_plugins(bevy::ui::UiPlugin)
        .add_systems(PostUpdate,owner_hosts_0943::parent_actual_children.before(bevy::ui::UiSystem::Layout));
    let font=f.app.world_mut().resource_mut::<Assets<Font>>().add(Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap());
    f.app.insert_resource(crate::menu_documents::MenuFont(font));
    f.app.finish();f.app.cleanup();
    f.app.world_mut().run_system_once(prepare_widget_layouts);f.app.world_mut().run_system_once(render);f.app.update();
    assert!(f.app.world().resource::<WidgetLayouts>().problem.is_none());
    assert!(!f.app.world().resource::<Screens>().widget_plans.is_empty(),"plan must come from actual preparation, never a test stub");
    let button=f.app.world().iter_entities().find(|entity|entity.contains::<Parent>()&&entity.get::<Control>().is_some_and(|control|control.screen=="form"&&control.element=="button")).unwrap().id();
    f.button=button;
    assert!(f.app.world().get::<Node>(button).unwrap().size().cmpgt(Vec2::ZERO).all());
    assert!(f.app.world().get::<ViewVisibility>(button).unwrap().get());
    assert!(f.app.world().resource::<bevy::ui::UiStack>().uinodes.contains(&button));
    // The intrinsic label uses the real renderer's decorative Pass policy.
    // Its actual hit must reach the original Button without test overrides.
    let label=f.app.world().iter_entities().find(|entity|entity.get::<crate::composed_motion::InterfaceTarget>().is_some_and(|target|target.screen=="form"&&target.element=="button-label")).unwrap().id();
    assert_eq!(f.app.world().get::<FocusPolicy>(label),Some(&FocusPolicy::Pass));
    assert!(f.app.world().get::<Control>(label).is_none());
    let label_center=f.app.world().get::<GlobalTransform>(label).unwrap().translation().truncate();
    assert_eq!(pick(f.app.world(),label_center),Some(button));
    f
}
fn rendered_down(f:&mut Fixture){
    let center=f.app.world().get::<GlobalTransform>(f.button).unwrap().translation().truncate();
    assert_eq!(pick(f.app.world(),center),Some(f.button));cursor(f,center);edge(f,ButtonState::Pressed);f.app.update();
    assert_eq!(hits(f),Value::Int(0));assert!(f.app.world().resource::<Screens>().widget_buttons1030.press.as_ref().unwrap().focus.is_some());
}
#[test]
fn widget_button1030_actual_prepare_render_focus_preserves_entity_and_camera_change_refuses_geometry(){
    let mut f=rendered_fixture(SOURCE);let button=f.button;
    let control=f.app.world().get::<Control>(button).unwrap().clone();let source=f.app.world().get::<ControlSource>(button).unwrap().clone();
    let proof=rendered_geometry(f.app.world(),&control,&source).unwrap();
    rendered_down(&mut f);
    f.app.world_mut().run_system_once(prepare_widget_layouts);f.app.world_mut().run_system_once(render);
    assert!(f.app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
    assert_eq!(f.app.world().get::<ControlSource>(button).unwrap().0,source.0);
    assert!(f.app.world().resource::<Screens>().widget_buttons1030.press.as_ref().unwrap().focus.is_some());
    consume(&mut f);f.app.world_mut().run_system_once(prepare_widget_layouts);f.app.world_mut().run_system_once(render);
    assert_eq!(f.app.world().get::<ControlSource>(button).unwrap().0,source.0);
    assert!(geometry_current(f.app.world(),&control,&source,proof));
    f.app.update();assert!(f.app.world().resource::<Screens>().widget_buttons1030.press.as_ref().unwrap().focus.is_none());
    edge(&mut f,ButtonState::Released);f.app.update();assert_eq!(hits(&f),Value::Int(1));
    consume(&mut f);
    let height=f.app.world().get::<Window>(f.window).unwrap().height();let width=f.app.world().get::<Window>(f.window).unwrap().width();
    f.app.world_mut().get_mut::<Window>(f.window).unwrap().resolution.set(width+100.0,height);
    f.app.world_mut().run_system_once(bevy::render::camera::camera_system::<OrthographicProjection>);
    f.app.world_mut().run_system_once(prepare_widget_layouts);
    assert!(!geometry_current(f.app.world(),&control,&source,proof),"real recomputed camera/layout must reject the old geometry before render");
}
#[test]
fn widget_button1030_camera_change_during_own_focus_cancels_original_physical_hold(){
    let mut f=rendered_fixture(SOURCE);rendered_down(&mut f);
    let width=f.app.world().get::<Window>(f.window).unwrap().width();let height=f.app.world().get::<Window>(f.window).unwrap().height();
    f.app.world_mut().get_mut::<Window>(f.window).unwrap().resolution.set(width+100.0,height);
    f.app.world_mut().run_system_once(bevy::render::camera::camera_system::<OrthographicProjection>);
    f.app.world_mut().run_system_once(prepare_widget_layouts);f.app.world_mut().run_system_once(render);
    assert!(f.app.world().resource::<Screens>().widget_buttons1030.press.is_none());
    consume(&mut f);edge(&mut f,ButtonState::Released);f.app.update();assert_eq!(hits(&f),Value::Int(0));
}
#[test]
fn widget_button1030_focus_handler_layout_change_cannot_preserve_a_pending_physical_press(){
    let source=SOURCE.replace("set focuses=focuses+1","set focuses=focuses+1 set slot_inset=48")
        .replace("\"left\":8,\"top\":8,\"right\":8,\"bottom\":8","\"left\":slot_inset,\"top\":8,\"right\":8,\"bottom\":8")
        .replace("set foreign=0","set foreign=0 set slot_inset=8");
    let mut f=rendered_fixture(&source);
    let before=f.app.world().resource::<VnEngine>().0.interface_views().unwrap().into_iter().find(|view|view.name=="form").unwrap();
    let inset=|view:&ScreenView|match view.root.find("button").unwrap().parent_slot.as_ref().unwrap(){rvn_ui::programmable::ParentSlot::VerticalBox{padding,..}=>padding.left,_=>panic!("the actual parent is a VerticalBox")};
    assert_eq!(inset(&before),8.0);
    let definition=widget_definition_digest(&before).unwrap();
    rendered_down(&mut f);
    let after=f.app.world().resource::<VnEngine>().0.interface_views().unwrap().into_iter().find(|view|view.name=="form").unwrap();
    assert_eq!(f.app.world().resource::<VnEngine>().0.state.vars["focuses"],Value::Int(1));
    assert_eq!(inset(&after),48.0,"the actual Focus handler must reevaluate its dynamic parent-slot padding");
    assert!(after.root.find("button").unwrap().enabled,"layout cancellation must not be a disabled-control refusal");
    assert_ne!(widget_definition_digest(&after).unwrap(),definition);
    after.root.validate().unwrap();
    f.app.world_mut().run_system_once(prepare_widget_layouts);f.app.world_mut().run_system_once(render);
    assert!(f.app.world().resource::<Screens>().widget_buttons1030.press.is_none());
    consume(&mut f);edge(&mut f,ButtonState::Released);f.app.update();assert_eq!(hits(&f),Value::Int(0));
}
