//! Pure shared layout tests, not font/native/pixel acceptance.
use super::*;
use serde_json::json;
fn tree() -> Component {
    Component::parse(json!({"id":"root","kind":"row","layout_contract":"widget_v1","children":[
        {"id":"a","kind":"text","text":"A","parent_slot":{"kind":"horizontal_box","padding":{"left":2,"top":3,"right":4,"bottom":5},"vertical":"center"}},
        {"id":"b","kind":"text","text":"B","parent_slot":{"kind":"horizontal_box","padding":{"left":1,"top":2,"right":3,"bottom":4},"size":{"rule":"fill","weight":1}}},
        {"id":"c","kind":"text","text":"C","parent_slot":{"kind":"horizontal_box","size":{"rule":"fill","weight":3}}}
    ]})).unwrap()
}
fn rects(root: &Component, bounds: [f32; 4]) -> Vec<ComponentRect> {
    layout_widget_rects(root, bounds, &mut |_| Ok([40.0, 10.0])).unwrap()
}
fn at(items: &[ComponentRect], id: &str) -> [f32; 4] {
    items.iter().find(|item| item.id == id).unwrap().rect
}
fn close(a: [f32; 4], b: [f32; 4]) {
    for (a, b) in a.into_iter().zip(b) {
        assert!((a - b).abs() < 0.0001, "{a} != {b}");
    }
}
#[test]
fn horizontal0954_default_slot_is_typed_for_actual_row_and_roundtrips() {
    let slot = ParentSlot::for_parent(Kind::Row).unwrap();
    assert_eq!(
        slot,
        ParentSlot::HorizontalBox {
            padding: SlotEdges::default(),
            horizontal: Alignment::Stretch,
            vertical: Alignment::Stretch,
            size: BoxSlotSize::Auto { weight: 1.0 }
        }
    );
    let wire = serde_json::to_value(&slot).unwrap();
    assert_eq!(wire["kind"], "horizontal_box");
    assert_eq!(serde_json::from_value::<ParentSlot>(wire).unwrap(), slot);
    let root = tree();
    let reopened = Component::parse(serde_json::to_value(&root).unwrap()).unwrap();
    assert_eq!(reopened, root);
    assert_eq!(root.spacing, 0.0);
}
#[test]
fn horizontal0954_auto_fill_weights_and_asymmetric_alignment_use_horizontal_axis() {
    let r = rects(&tree(), [10.0, 20.0, 300.0, 90.0]);
    close(at(&r, "a"), [12.0, 58.0, 40.0, 10.0]);
    close(at(&r, "b"), [57.0, 22.0, 62.5, 84.0]);
    close(at(&r, "c"), [122.5, 20.0, 187.5, 90.0]);
}
#[test]
fn horizontal0954_axis_transpose_matches_existing_vertical_box_for_all_alignments() {
    for horizontal in [
        Alignment::Start,
        Alignment::Center,
        Alignment::End,
        Alignment::Stretch,
    ] {
        for vertical in [
            Alignment::Start,
            Alignment::Center,
            Alignment::End,
            Alignment::Stretch,
        ] {
            let mut row = tree();
            for child in &mut row.children {
                if let Some(ParentSlot::HorizontalBox {
                    horizontal: h,
                    vertical: v,
                    ..
                }) = &mut child.parent_slot
                {
                    *h = horizontal;
                    *v = vertical;
                }
            }
            let mut column = row.clone();
            column.kind = Kind::Column;
            for child in &mut column.children {
                let Some(ParentSlot::HorizontalBox {
                    padding,
                    horizontal,
                    vertical,
                    size,
                }) = child.parent_slot.take()
                else {
                    panic!("row slot")
                };
                child.parent_slot = Some(ParentSlot::VerticalBox {
                    padding: SlotEdges {
                        left: padding.top,
                        top: padding.left,
                        right: padding.bottom,
                        bottom: padding.right,
                    },
                    horizontal: vertical,
                    vertical: horizontal,
                    size,
                });
            }
            let a = rects(&row, [10.0, 20.0, 300.0, 90.0]);
            let b = layout_widget_rects(&column, [20.0, 10.0, 90.0, 300.0], &mut |_| {
                Ok([10.0, 40.0])
            })
            .unwrap();
            for item in &a {
                let swapped = at(&b, &item.id);
                close(item.rect, [swapped[1], swapped[0], swapped[3], swapped[2]]);
            }
        }
    }
}
#[test]
fn horizontal0954_nested_row_desired_size_flows_through_column_and_real_button_child() {
    let root=Component::parse(json!({"id":"outer","kind":"column","layout_contract":"widget_v1","children":[
        {"id":"row","kind":"row","parent_slot":{"kind":"vertical_box"},"children":[
            {"id":"button","kind":"button","parent_slot":{"kind":"horizontal_box"},"children":[{"id":"label","kind":"text","text":"Child","parent_slot":{"kind":"button","padding":{"left":4,"top":2,"right":4,"bottom":2},"horizontal":"center","vertical":"center"}}]},
            {"id":"other","kind":"text","text":"Other","parent_slot":{"kind":"horizontal_box"}}
        ]}
    ]})).unwrap();
    let r = rects(&root, [0.0, 0.0, 280.0, 100.0]);
    close(at(&r, "row"), [0.0, 0.0, 280.0, 14.0]);
    close(at(&r, "button"), [0.0, 0.0, 48.0, 14.0]);
    close(at(&r, "label"), [4.0, 2.0, 40.0, 10.0]);
    close(at(&r, "other"), [48.0, 0.0, 40.0, 14.0]);
}
#[test]
fn horizontal0954_invisible_child_and_zero_fill_do_not_allocate_main_axis_space() {
    let mut root = tree();
    root.children[0].visible = false;
    for child in &mut root.children[1..] {
        if let Some(ParentSlot::HorizontalBox { padding, size, .. }) = &mut child.parent_slot {
            *padding = SlotEdges::default();
            *size = BoxSlotSize::Fill { weight: 0.0 };
        }
    }
    let mut measured = Vec::new();
    let r = layout_widget_rects(&root, [0.0, 0.0, 100.0, 50.0], &mut |node| {
        measured.push(node.id.clone());
        Ok([40.0, 10.0])
    })
    .unwrap();
    assert!(!measured.iter().any(|id| id == "a"));
    assert!(!r.iter().any(|item| item.id == "a"));
    close(at(&r, "b"), [0.0, 0.0, 0.0, 50.0]);
    close(at(&r, "c"), [0.0, 0.0, 0.0, 50.0]);
}
#[test]
fn horizontal0954_wrong_parent_slot_invalid_fill_and_nonfinite_bounds_refuse() {
    let root = tree();
    for bad in [f32::NAN, f32::INFINITY, -1.0, 16385.0] {
        let mut copy = root.clone();
        copy.children[0].parent_slot = Some(ParentSlot::HorizontalBox {
            padding: SlotEdges::default(),
            horizontal: Alignment::Stretch,
            vertical: Alignment::Stretch,
            size: BoxSlotSize::Fill { weight: bad },
        });
        assert!(copy.validate().is_err());
    }
    let mut copy = root.clone();
    copy.children[0].parent_slot = Some(ParentSlot::for_parent(Kind::Column).unwrap());
    assert!(copy.validate().is_err());
    for bounds in [
        [0.0, 0.0, -1.0, 10.0],
        [f32::NAN, 0.0, 100.0, 10.0],
        [0.0, 0.0, 100.0, f32::INFINITY],
    ] {
        assert!(layout_widget_rects(&root, bounds, &mut |_| Ok([10.0, 10.0])).is_err());
    }
}
#[test]
fn horizontal0954_reparent_uses_receiving_slot_and_keeps_identity_or_refuses_atomically() {
    let mut root=Component::parse(json!({"id":"outer","kind":"column","layout_contract":"widget_v1","children":[{"id":"row","kind":"row","parent_slot":{"kind":"vertical_box"}},{"id":"text","kind":"text","text":"Keep","parent_slot":{"kind":"vertical_box"}}]})).unwrap();
    root.reparent_widget_child("text", "row").unwrap();
    assert_eq!(
        root.find("text").unwrap().parent_slot,
        Some(ParentSlot::for_parent(Kind::Row).unwrap())
    );
    let before = root.clone();
    root.reparent_widget_child("text", "row").unwrap();
    assert_eq!(root, before);
    assert!(root.reparent_widget_child("row", "text").is_err());
    assert_eq!(root, before);
    root.reparent_widget_child("text", "outer").unwrap();
    assert_eq!(root.find("text").unwrap().text, "Keep");
    assert_eq!(
        root.find("text").unwrap().parent_slot,
        Some(ParentSlot::for_parent(Kind::Column).unwrap())
    );
}
#[test]
fn horizontal0954_legacy_row_keeps_reference_layout_and_rejects_typed_slot_without_opt_in() {
    let root=Component::parse(json!({"id":"old","kind":"row","spacing":7,"children":[{"id":"a","kind":"text","text":"A"},{"id":"b","kind":"text","text":"B"}]})).unwrap();
    let wire = serde_json::to_value(&root).unwrap();
    assert!(wire.get("layout_contract").is_none());
    let old = super::super::layout_rects(&root, [300.0, 100.0]).unwrap();
    let reopened = Component::parse(wire).unwrap();
    assert_eq!(root, reopened);
    assert_eq!(
        super::super::layout_rects(&reopened, [300.0, 100.0]).unwrap(),
        old
    );
    assert!(
        layout_widget_rects(&root, [0.0, 0.0, 300.0, 100.0], &mut |_| Ok([40.0, 10.0])).is_err()
    );
    let mut copy = root.clone();
    copy.children[0].parent_slot = Some(ParentSlot::for_parent(Kind::Row).unwrap());
    assert!(copy.validate().is_err());
}
