//! Targeted contract tests for the existing Component tree extension, not a
//! substitute for the still-required Graph/Engine/Bevy/mixed-host F journey.
use rvn_ui::programmable::{
    layout_rects, layout_widget_rects, Alignment, BoxSlotSize, Component, ComponentKind,
    LayoutContract, ParentSlot, SlotEdges,
};
use serde_json::{json, Value};
fn source() -> Value {
    json!({"id":"root","kind":"column","spacing":0,"layout_contract":"widget_v1","children":[
     {"id":"button","kind":"button","spacing":0,"parent_slot":{"kind":"vertical_box","padding":{"left":8,"top":8,"right":8,"bottom":8}},"children":[
      {"id":"label","kind":"text","spacing":0,"text":"UserWidget 0875","parent_slot":{"kind":"button","padding":{"left":6,"top":6,"right":6,"bottom":6},"horizontal":"center","vertical":"center"}}
     ]}
    ]})
}
fn tree() -> Component {
    Component::parse(source()).unwrap()
}
fn text(id: &str) -> Component {
    Component::parse(json!({"id":id,"kind":"text","spacing":0,"text":"Content"})).unwrap()
}
fn twice() -> Component {
    let mut value = source();
    let mut other = value["children"][0].clone();
    other["id"] = json!("other_button");
    other["children"][0]["id"] = json!("other_label");
    value["children"].as_array_mut().unwrap().push(other);
    Component::parse(value).unwrap()
}
fn rects(root: &Component) -> Vec<rvn_ui::programmable::ComponentRect> {
    layout_widget_rects(root, [0., 0., 280., 100.], &mut |_| Ok([100., 20.])).unwrap()
}
fn at<'a>(rects: &'a [rvn_ui::programmable::ComponentRect], id: &str) -> &'a [f32; 4] {
    &rects.iter().find(|r| r.id == id).unwrap().rect
}

#[test]
fn widget0894_legacy_button_leaf_and_serialized_fields_remain_unchanged() {
    let old = Component::parse(json!({"id":"old","kind":"button","text":"Old Button"})).unwrap();
    let wire = serde_json::to_value(&old).unwrap();
    assert!(wire.get("layout_contract").is_none() && wire.get("parent_slot").is_none());
    assert_eq!(Component::parse(wire).unwrap(), old);
    assert!(Component::parse(
        json!({"id":"old","kind":"button","children":[{"id":"text","kind":"text"}]})
    )
    .is_err());
    let layout = layout_rects(&old, [400., 200.]).unwrap();
    assert_eq!(layout.len(), 1);
    assert_eq!(layout[0].rect[2], 180.);
}
#[test]
fn widget0894_real_button_child_and_typed_slots_roundtrip_without_a_second_tree() {
    let old = tree();
    let saved = serde_json::to_string(&old).unwrap();
    let loaded: Component = serde_json::from_str(&saved).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded, old);
    assert_eq!(loaded.layout_contract, Some(LayoutContract::WidgetV1));
    assert_eq!(loaded.children[0].children[0].id, "label");
    assert!(matches!(
        loaded.children[0].parent_slot,
        Some(ParentSlot::VerticalBox { .. })
    ));
    assert!(matches!(
        loaded.children[0].children[0].parent_slot,
        Some(ParentSlot::Button { .. })
    ));
}
#[test]
fn widget0894_nested_eight_and_six_padding_have_distinct_real_layout_boundaries() {
    let layout = rects(&tree());
    assert_eq!(*at(&layout, "root"), [0., 0., 280., 100.]);
    assert_eq!(*at(&layout, "button"), [8., 8., 264., 32.]);
    assert_eq!(*at(&layout, "label"), [90., 14., 100., 20.]);
}
#[test]
fn widget0894_parent_padding_edit_does_not_become_button_content_style() {
    let mut definition = tree();
    if let Some(ParentSlot::VerticalBox { padding, .. }) =
        &mut definition.find_mut("button").unwrap().parent_slot
    {
        *padding = SlotEdges::uniform(16.);
    }
    let layout = rects(&definition);
    assert_eq!(*at(&layout, "button"), [16., 16., 248., 32.]);
    assert_eq!(*at(&layout, "label"), [90., 22., 100., 20.]);
    assert!(matches!(
        definition.find("label").unwrap().parent_slot,
        Some(ParentSlot::Button {
            padding: SlotEdges { left: 6., .. },
            ..
        })
    ));
}
#[test]
fn widget0894_button_content_padding_changes_desired_height_and_text_inset() {
    let mut definition = tree();
    if let Some(ParentSlot::Button { padding, .. }) =
        &mut definition.find_mut("label").unwrap().parent_slot
    {
        *padding = SlotEdges::uniform(12.);
    }
    let layout = rects(&definition);
    assert_eq!(*at(&layout, "button"), [8., 8., 264., 44.]);
    assert_eq!(*at(&layout, "label"), [90., 20., 100., 20.]);
}
#[test]
fn widget0894_occupied_button_add_refuses_before_detaching_another_parents_child() {
    let mut definition = twice();
    let before = definition.clone();
    let other = definition.find("other_label").unwrap().clone();
    assert!(definition.add_widget_child("button", &other).is_err());
    assert_eq!(definition, before);
    assert!(definition
        .reparent_widget_child("other_label", "button")
        .is_err());
    assert_eq!(definition, before);
    assert_eq!(
        definition.find("other_button").unwrap().children[0].id,
        "other_label"
    );
}
#[test]
fn widget0894_reparent_changes_only_actual_relation_and_receiver_slot_type() {
    let mut definition = twice();
    let label = definition.find("other_label").unwrap().clone();
    definition
        .reparent_widget_child("other_label", "root")
        .unwrap();
    definition.validate().unwrap();
    assert!(definition.find("other_button").unwrap().children.is_empty());
    let moved = definition.find("other_label").unwrap();
    assert_eq!(moved.text, label.text);
    assert_eq!(moved.id, label.id);
    assert_eq!(
        moved.parent_slot,
        Some(ParentSlot::for_parent(ComponentKind::Column).unwrap())
    );
    assert!(matches!(label.parent_slot, Some(ParentSlot::Button { .. })));
}
#[test]
fn widget0894_same_parent_reparent_is_a_true_noop_preserving_slot_properties() {
    let mut definition = tree();
    let before = serde_json::to_string(&definition).unwrap();
    definition.reparent_widget_child("label", "button").unwrap();
    assert_eq!(serde_json::to_string(&definition).unwrap(), before);
}
#[test]
fn widget0894_set_content_is_distinct_from_occupied_add_and_returns_retained_detached_child() {
    let mut definition = tree();
    let child = text("replacement");
    assert!(definition.add_widget_child("button", &child).is_err());
    let removed = definition
        .set_widget_button_content("button", Some(&child))
        .unwrap()
        .unwrap();
    assert_eq!(removed.id, "label");
    assert!(removed.parent_slot.is_none());
    assert_eq!(removed.text, "UserWidget 0875");
    assert_eq!(
        definition.find("button").unwrap().children[0].id,
        "replacement"
    );
    assert_eq!(
        definition.find("replacement").unwrap().parent_slot,
        Some(ParentSlot::for_parent(ComponentKind::Button).unwrap())
    );
    assert!(child.parent_slot.is_none());
}
#[test]
fn widget0894_invalid_identity_cycle_and_slot_edits_leave_whole_definition_unchanged() {
    let mut definition = twice();
    let before = definition.clone();
    let duplicate = definition.find("label").unwrap().clone();
    assert!(definition.add_widget_child("root", &duplicate).is_err());
    assert_eq!(definition, before);
    assert!(definition.reparent_widget_child("button", "label").is_err());
    assert_eq!(definition, before);
    let mut bad = source();
    bad["children"][0]["parent_slot"] =
        json!({"kind":"button","padding":{},"horizontal":"center","vertical":"center"});
    assert!(Component::parse(bad).is_err());
}
#[test]
fn widget0894_fill_allocation_keeps_auto_padding_separate_and_uses_explicit_weights() {
    let mut definition = twice();
    for (id, weight) in [("button", 1.), ("other_button", 3.)] {
        definition.find_mut(id).unwrap().parent_slot = Some(ParentSlot::VerticalBox {
            padding: SlotEdges::default(),
            horizontal: Alignment::Stretch,
            vertical: Alignment::Stretch,
            size: BoxSlotSize::Fill { weight },
        });
    }
    let layout = rects(&definition);
    assert_eq!(*at(&layout, "button"), [0., 0., 280., 25.]);
    assert_eq!(*at(&layout, "other_button"), [0., 25., 280., 75.]);
}
#[test]
fn widget0894_alignment_uses_asymmetric_slot_edges_and_clamped_child_size() {
    let mut value = source()["children"][0].clone();
    value["layout_contract"] = json!("widget_v1");
    value.as_object_mut().unwrap().remove("parent_slot");
    value["children"][0]["parent_slot"] = json!({"kind":"button","padding":{"left":1,"top":2,"right":3,"bottom":4},"horizontal":"center","vertical":"center"});
    let definition = Component::parse(value).unwrap();
    let layout =
        layout_widget_rects(&definition, [10., 20., 200., 50.], &mut |_| Ok([100., 20.])).unwrap();
    assert_eq!(*at(&layout, "label"), [58., 33., 100., 20.]);
}
#[test]
fn widget0894_measuring_is_pure_and_refusal_or_nonfinite_metrics_cannot_change_definition() {
    let definition = tree();
    let before = definition.clone();
    assert!(
        layout_widget_rects(&definition, [0., 0., 280., 100.], &mut |_| Err(
            "actual text service refused".into()
        ))
        .is_err()
    );
    assert_eq!(definition, before);
    assert!(
        layout_widget_rects(&definition, [0., 0., 280., 100.], &mut |_| Ok([
            f32::NAN,
            20.
        ]))
        .is_err()
    );
    assert_eq!(definition, before);
}
#[test]
fn widget0894_default_slots_are_actual_parent_defaults_not_fixture_six_or_eight() {
    assert_eq!(
        ParentSlot::for_parent(ComponentKind::Column).unwrap(),
        ParentSlot::VerticalBox {
            padding: SlotEdges::default(),
            horizontal: Alignment::Stretch,
            vertical: Alignment::Stretch,
            size: BoxSlotSize::Auto { weight: 1.0 }
        }
    );
    assert_eq!(
        ParentSlot::for_parent(ComponentKind::Button).unwrap(),
        ParentSlot::Button {
            padding: SlotEdges {
                left: 4.,
                right: 4.,
                top: 2.,
                bottom: 2.
            },
            horizontal: Alignment::Center,
            vertical: Alignment::Center
        }
    );
}
#[test]
fn widget0894_wrong_or_nested_contract_and_multiple_button_children_are_refused() {
    let mut value = source();
    value["layout_contract"] = json!("widget_v2");
    assert!(Component::parse(value).is_err());
    let mut value = source();
    value["children"][0]["layout_contract"] = json!("widget_v1");
    assert!(Component::parse(value).is_err());
    let mut value = source();
    let mut copy = value["children"][0]["children"][0].clone();
    copy["id"] = json!("second");
    value["children"][0]["children"]
        .as_array_mut()
        .unwrap()
        .push(copy);
    assert!(Component::parse(value).is_err());
}

#[test]
fn widget0894_detached_nested_content_remains_a_valid_serializable_definition_subtree() {
    let mut value = source();
    let original = value["children"][0]["children"][0].clone();
    value["children"][0]["children"][0] = json!({"id":"inner_column","kind":"column","spacing":0,"parent_slot":{"kind":"button","padding":{},"horizontal":"center","vertical":"center"},"children":[]});
    let mut inner = original;
    inner["parent_slot"] = json!({"kind":"vertical_box"});
    value["children"][0]["children"][0]["children"]
        .as_array_mut()
        .unwrap()
        .push(inner);
    let mut definition = Component::parse(value).unwrap();
    let old = definition
        .set_widget_button_content("button", None)
        .unwrap()
        .unwrap();
    assert_eq!(old.layout_contract, Some(LayoutContract::WidgetV1));
    assert!(old.parent_slot.is_none());
    old.validate().unwrap();
    let decoded = Component::parse(serde_json::to_value(&old).unwrap()).unwrap();
    assert_eq!(decoded, old);
    assert_eq!(decoded.children[0].id, "label");
    definition.add_widget_child("button", &decoded).unwrap();
    assert_eq!(
        definition.find("inner_column").unwrap().layout_contract,
        None
    );
    definition.validate().unwrap();
}

#[test]
fn widget0894_negative_padding_preserves_signed_desired_size_before_auto_arrangement_clamps_content(
) {
    let mut definition = tree();
    definition.find_mut("label").unwrap().parent_slot = Some(ParentSlot::Button {
        padding: SlotEdges {
            left: 0.,
            right: 0.,
            top: -20.,
            bottom: -20.,
        },
        horizontal: Alignment::Center,
        vertical: Alignment::Center,
    });
    let layout = rects(&definition);
    assert_eq!(*at(&layout, "button"), [8., 8., 264., 0.]);
    assert_eq!(*at(&layout, "label"), [90., -2., 100., 20.]);
}
#[test]
fn widget0894_zero_fill_is_finite_and_collapsed_children_do_not_dilute_visible_weights() {
    let mut definition = twice();
    for id in ["button", "other_button"] {
        definition.find_mut(id).unwrap().parent_slot = Some(ParentSlot::VerticalBox {
            padding: SlotEdges::default(),
            horizontal: Alignment::Stretch,
            vertical: Alignment::Stretch,
            size: BoxSlotSize::Fill { weight: 0. },
        });
    }
    let layout = rects(&definition);
    assert_eq!(*at(&layout, "button"), [0., 0., 280., 0.]);
    assert_eq!(*at(&layout, "other_button"), [0., 0., 280., 0.]);
    assert!(layout.iter().flat_map(|r| r.rect).all(f32::is_finite));
    definition.find_mut("button").unwrap().parent_slot = Some(ParentSlot::VerticalBox {
        padding: SlotEdges::default(),
        horizontal: Alignment::Stretch,
        vertical: Alignment::Stretch,
        size: BoxSlotSize::Fill { weight: 1. },
    });
    definition.find_mut("other_button").unwrap().parent_slot = Some(ParentSlot::VerticalBox {
        padding: SlotEdges::uniform(200.),
        horizontal: Alignment::Stretch,
        vertical: Alignment::Stretch,
        size: BoxSlotSize::Fill { weight: 100. },
    });
    definition.find_mut("other_button").unwrap().visible = false;
    let layout = rects(&definition);
    assert_eq!(*at(&layout, "button"), [0., 0., 280., 100.]);
    assert!(layout
        .iter()
        .all(|r| r.id != "other_button" && r.id != "other_label"));
    definition.find_mut("button").unwrap().visible = false;
    let layout = rects(&definition);
    assert_eq!(layout.len(), 1);
    assert_eq!(layout[0].id, "root");
}
#[test]
fn widget0894_slot_coefficients_and_actual_text_metrics_reject_invalid_numbers_without_editing() {
    let definition = tree();
    let before = definition.clone();
    assert!(
        layout_widget_rects(&definition, [0., 0., 280., 100.], &mut |_| Ok([-1., 20.])).is_err()
    );
    assert_eq!(definition, before);
    for weight in [-1., f32::NAN, f32::INFINITY] {
        let mut bad = definition.clone();
        bad.find_mut("button").unwrap().parent_slot = Some(ParentSlot::VerticalBox {
            padding: SlotEdges::default(),
            horizontal: Alignment::Stretch,
            vertical: Alignment::Stretch,
            size: BoxSlotSize::Fill { weight },
        });
        assert!(bad.validate().is_err());
        assert_eq!(definition, before);
    }
}
