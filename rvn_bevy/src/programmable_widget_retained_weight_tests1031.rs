//! Actual VM, Font, camera plan and ECS projection; no GPU/native claim.
use super::*;

fn retained_engine(
    kind: &str,
    rule: &str,
    weight: f32,
) -> rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
    let slot = if kind == "column" { "vertical_box" } else { "horizontal_box" };
    let button = |id: &str, size: serde_json::Value| serde_json::json!({
        "id": id, "kind": "button", "spacing": 0,
        "parent_slot": {"kind": slot, "size": size},
        "children": [{
            "id": format!("{id}_text"), "kind": "text", "text": "AVATAR",
            "font_size": 24, "spacing": 0,
            "parent_slot": {"kind": "button", "padding": {"left":4,"right":4,"top":2,"bottom":2},
                "horizontal":"center", "vertical":"center"}
        }]
    });
    let root = serde_json::json!({
        "id":"root", "kind":kind, "layout_contract":"widget_v1", "spacing":0,
        "children":[
            button("weighted", serde_json::json!({"rule":rule,"weight":weight})),
            button("peer", serde_json::json!({"rule":"fill","weight":1}))
        ]
    });
    let source = format!(
        "screen Retained1031() {{ return {root} }}\nlabel start\n ui.open(\"Retained1031\",[],false,0)\n \"Waiting\"\n"
    );
    let mut engine = rvn_core::Engine::new(
        rvn_parser::parse(&source).unwrap(), crate::bevy_renderer::BevyRenderer::new(), 32,
    ).unwrap();
    engine.step_until_interaction().unwrap();
    engine.renderer.scene_batch = None;
    engine
}

#[test]
fn retained1031_bevy_auto_ignores_dormant_weight_on_both_axes_without_changing_vm_description() {
    let (app, font) = layout_app();
    let assets = app.world().resource::<AssetServer>();
    let fonts = app.world().resource::<Assets<Font>>();
    for (kind, axis) in [("column", 1), ("row", 0)] {
        let mut resolved = Vec::new();
        for weight in [1.0, 5.0] {
            let engine = retained_engine(kind, "auto", weight);
            let view = engine.interface_views().unwrap().pop().unwrap();
            let original = serde_json::to_vec(&view.root).unwrap();
            let slot = view.root.find("weighted").unwrap().parent_slot.as_ref().unwrap();
            let size = match slot {
                rvn_ui::programmable::ParentSlot::VerticalBox { size, .. }
                | rvn_ui::programmable::ParentSlot::HorizontalBox { size, .. } => *size,
                _ => panic!("Actual authored Box slot required"),
            };
            assert_eq!(size, rvn_ui::programmable::BoxSlotSize::Auto { weight });
            let actual = widget_layout_view(
                &view, [0.0,0.0,280.0,100.0], 1.0, 1.5, assets, fonts, Some(&font),
            ).unwrap();
            let measured = widget_text_size(
                view.root.find("weighted_text").unwrap(), &font, fonts, 1.0, 1.5,
            ).unwrap();
            let main = actual.root.find("weighted").unwrap().rect.unwrap();
            near(main[axis + 2], measured[axis] + if axis == 0 { 8.0 } else { 4.0 });
            assert_eq!(serde_json::to_vec(&view.root).unwrap(), original);
            assert_eq!(serde_json::to_vec(&engine.interface_views().unwrap()[0].root).unwrap(), original);
            resolved.push(actual);
        }
        assert_eq!(resolved[0].root.find("weighted").unwrap().rect, resolved[1].root.find("weighted").unwrap().rect);
        assert_eq!(resolved[0].root.find("peer").unwrap().rect, resolved[1].root.find("peer").unwrap().rect);
    }
}

#[test]
fn retained1031_actual_bevy_camera_and_ecs_project_fill_five_to_one_on_both_axes() {
    use bevy::ecs::system::RunSystemOnce;
    for (kind, axis) in [("column", 1), ("row", 0)] {
        let (mut app, _) = layout_app();
        let engine = retained_engine(kind, "fill", 5.0);
        let views = engine.interface_views().unwrap();
        let original = serde_json::to_vec(&views[0].root).unwrap();
        let source = engine.capture_ui_input_source("Retained1031").unwrap();
        app.world_mut().resource_mut::<Screens>().views = views;
        app.insert_resource(VnEngine(engine));
        setup_camera(&mut app);
        app.world_mut().run_system_once(prepare_widget_layouts);
        assert!(widget_geometry_ready(app.world()));
        let layout = app.world().resource::<WidgetLayouts>();
        let bounds = layout.metrics.as_ref().unwrap().bounds;
        let plan = &layout.plans[&source.to_wire().unwrap()];
        assert_eq!(plan.source, source);
        let rects = plan.rects.clone();
        let main = rects.iter().find(|rect| rect.id == "weighted").unwrap().rect;
        let peer = rects.iter().find(|rect| rect.id == "peer").unwrap().rect;
        near(main[axis + 2], peer[axis + 2] * 5.0);
        near(main[axis + 2] + peer[axis + 2], bounds[axis + 2]);
        near(peer[axis], main[axis] + main[axis + 2]);
        app.world_mut().run_system_once(render);
        let controls = app.world_mut()
            .query::<(Entity, &Control, &ControlSource, &Style, &Parent)>()
            .iter(app.world())
            .filter(|(_, control, _, _, _)| control.screen == "Retained1031")
            .map(|(entity, control, actual, style, parent)| (
                control.element.clone(), (entity, actual.0.clone(), style.clone(), parent.get()),
            )).collect::<HashMap<_, _>>();
        assert_eq!(controls.len(), 2);
        assert_ne!(controls["weighted"].0, controls["peer"].0);
        assert_eq!(controls["weighted"].3, controls["peer"].3);
        assert!(app.world().get_entity(controls["weighted"].3).is_some());
        for name in ["weighted", "peer"] {
            let (_, actual_source, style, _) = &controls[name];
            let rect = rects.iter().find(|rect| rect.id == name).unwrap().rect;
            assert_eq!(actual_source, &source);
            assert_eq!(style.position_type, PositionType::Absolute);
            assert_eq!(style.left, Val::Px(rect[0]));
            assert_eq!(style.top, Val::Px(rect[1]));
            assert_eq!(style.width, Val::Px(rect[2]));
            assert_eq!(style.height, Val::Px(rect[3]));
        }
        assert_eq!(serde_json::to_vec(&app.world().resource::<Screens>().views[0].root).unwrap(), original);
        assert_eq!(serde_json::to_vec(&app.world().resource::<VnEngine>().0.interface_views().unwrap()[0].root).unwrap(), original);
    }
}
