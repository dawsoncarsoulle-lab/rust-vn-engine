//! Existing Bevy Font/layout/entity pipeline, CPU only. No GPU/hit/native claim.
use super::*;
const ROW_SOURCE: &str = r#"
screen Row0954() {return component("root","row",{"layout_contract":"widget_v1"},[
 component("left","button",{"parent_slot":{"kind":"horizontal_box"}},[component("label","text",{"text":"AVATAR","font_size":24,"parent_slot":{"kind":"button","padding":{"left":4,"top":2,"right":4,"bottom":2},"horizontal":"center","vertical":"center"}},[])]),
 component("right","button",{"parent_slot":{"kind":"horizontal_box","size":{"rule":"fill","weight":1}}},[component("other","text",{"text":"Right","font_size":24,"parent_slot":{"kind":"button","padding":{"left":4,"top":2,"right":4,"bottom":2},"horizontal":"center","vertical":"center"}},[])])
])}
label start
 ui.open("Row0954",[],false,0)
 "Waiting"
"#;
fn row_engine() -> rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
    let mut e = rvn_core::Engine::new(
        rvn_parser::parse(ROW_SOURCE).unwrap(),
        crate::bevy_renderer::BevyRenderer::new(),
        32,
    )
    .unwrap();
    e.step_until_interaction().unwrap();
    e.renderer.scene_batch = None;
    e
}
#[test]
fn horizontal0954_bevy_real_font_resolves_shared_axis_rects_without_mutating_vm_description() {
    let mut app = assets();
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap());
    let engine = row_engine();
    let view = engine.interface_views().unwrap().pop().unwrap();
    let before = serde_json::to_vec(&view.root).unwrap();
    let assets = app.world().resource::<AssetServer>();
    let fonts = app.world().resource::<Assets<Font>>();
    let resolved = widget_layout_view(
        &view,
        [0.0, 0.0, 500.0, 100.0],
        1.0,
        1.5,
        assets,
        fonts,
        Some(&font),
    )
    .unwrap();
    let measured =
        widget_text_size(view.root.find("label").unwrap(), &font, fonts, 1.0, 1.5).unwrap();
    let left = resolved.root.find("left").unwrap().rect.unwrap();
    let right = resolved.root.find("right").unwrap().rect.unwrap();
    near(left[2], measured[0] + 8.0);
    near(right[0], left[2]);
    near(left[2] + right[2], 500.0);
    assert_eq!(resolved.root.kind, Kind::Row);
    assert_eq!(resolved.root.find("left").unwrap().children.len(), 1);
    assert_eq!(serde_json::to_vec(&view.root).unwrap(), before);
}
#[test]
fn horizontal0954_actual_vm_camera_plan_spawns_two_existing_button_controls_with_shared_source() {
    use bevy::ecs::system::RunSystemOnce;
    let (mut app, _) = layout_app();
    let engine = row_engine();
    let views = engine.interface_views().unwrap();
    let source = engine.capture_ui_input_source("Row0954").unwrap();
    app.world_mut().resource_mut::<Screens>().views = views;
    app.insert_resource(VnEngine(engine));
    setup_camera(&mut app);
    app.world_mut().run_system_once(prepare_widget_layouts);
    assert!(widget_geometry_ready(app.world()));
    let rects = app.world().resource::<WidgetLayouts>().plans[&source.to_wire().unwrap()]
        .rects
        .clone();
    app.world_mut().run_system_once(render);
    let rows = app
        .world_mut()
        .query::<(Entity, &Control, &ControlSource, &Style, &Parent)>()
        .iter(app.world())
        .filter(|(_, control, _, _, _)| control.screen == "Row0954")
        .map(|(entity, control, actual, style, parent)| {
            (
                control.element.clone(),
                (entity, actual.0.clone(), style.clone(), parent.get()),
            )
        })
        .collect::<HashMap<_, _>>();
    assert_eq!(rows.len(), 2);
    let mut parent = None;
    for name in ["left", "right"] {
        let (_, actual, style, host) = &rows[name];
        assert_eq!(actual, &source);
        assert_eq!(style.position_type, PositionType::Absolute);
        let rect = rects.iter().find(|rect| rect.id == name).unwrap();
        assert_eq!(style.left, Val::Px(rect.rect[0]));
        assert_eq!(style.width, Val::Px(rect.rect[2]));
        assert!(app.world().get_entity(*host).is_some());
        if let Some(parent) = parent {
            assert_eq!(*host, parent);
        } else {
            parent = Some(*host);
        }
    }
    assert_ne!(rows["left"].0, rows["right"].0);
}
