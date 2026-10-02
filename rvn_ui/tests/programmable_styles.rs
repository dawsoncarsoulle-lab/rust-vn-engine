use rvn_ui::programmable::*;
use serde_json::json;

#[test]
fn cascades_are_local_bounded_and_explicit_properties_win() {
    let source = json!({"id":"root","kind":"column","style":[{"padding":12,"background":[0.1,0.2,0.3,1]},{"padding":24,"opacity":0.5}],"padding":32,"children":[{"id":"button","kind":"button","text":"OK","style":{"radius":18,"foreground":[1,0,0,1]},"opacity":0.5}]});
    let original = source.clone();
    let component = Component::parse(source).unwrap();
    assert_eq!(component.padding, 32.0);
    assert_eq!(component.background, [0.1, 0.2, 0.3, 1.0]);
    assert_eq!(component.children[0].radius, 18.0);
    assert_eq!(component.effective_opacity("button"), Some(0.25));
    assert!(original.get("style").is_some());
    for style in [
        json!({"events":{"click":"hidden_logic"}}),
        json!({"typo":5}),
        json!(vec![json!({}); 33]),
        json!(5),
    ] {
        assert!(Component::parse(json!({"id":"x","kind":"text","style":style})).is_err());
    }
    assert!(
        Component::parse(json!({"id":"x","kind":"button","event_data":{"null":null}})).is_err()
    );
}

#[test]
fn old_components_keep_original_defaults_and_new_geometry_is_checked() {
    let old: Component = serde_json::from_value(json!({"id":"x","kind":"text"})).unwrap();
    assert_eq!(old.opacity, 1.0);
    assert_eq!(old.radius, 6.0);
    assert!(old.auto_background);
    assert!(old.event_data.is_empty());
    for properties in [
        json!({"opacity":2}),
        json!({"min_width":200,"max_width":100}),
        json!({"radius":-1}),
        json!({"focus_color":[0,0,0,2]}),
    ] {
        let mut description = json!({"id":"x","kind":"button"});
        description
            .as_object_mut()
            .unwrap()
            .extend(properties.as_object().unwrap().clone());
        assert!(Component::parse(description).is_err());
    }
}

#[test]
fn reference_layout_shares_alignment_margin_constraints_and_nested_absolute_coordinates() {
    let root=Component::parse(json!({"id":"root","kind":"column","rect":[100,100,300,240],"padding":10,"align":"end","justify":"space_between","children":[
        {"id":"first","kind":"text","width":100,"height":40,"margin":[5,10,0,0]},
        {"id":"hidden","kind":"text","visible":false},
        {"id":"last","kind":"panel","width":80,"height":60,"children":[{"id":"absolute","kind":"text","rect":[4,6,20,10]}]}
    ]})).unwrap();
    let rects = layout_rects(&root, [1920.0, 1080.0]).unwrap();
    let rect = |id: &str| rects.iter().find(|item| item.id == id).unwrap().rect;
    assert_eq!(rect("root"), [100.0, 100.0, 300.0, 240.0]);
    assert_eq!(rect("first"), [280.0, 115.0, 100.0, 40.0]);
    assert_eq!(rect("last"), [310.0, 270.0, 80.0, 60.0]);
    assert_eq!(rect("absolute"), [314.0, 276.0, 20.0, 10.0]);
    assert!(!rects.iter().any(|item| item.id == "hidden"));
}

#[test]
fn wrapping_keeps_authored_order_and_absolute_screens_fill_the_viewport() {
    let root=Component::parse(json!({"id":"root","kind":"row","rect":[0,0,120,100],"wrap":true,"align":"start","spacing":4,"children":[{"id":"a","kind":"text","width":70,"height":20},{"id":"b","kind":"text","width":70,"height":20}]})).unwrap();
    let rects = layout_rects(&root, [1920.0, 1080.0]).unwrap();
    assert_eq!(rects[1].rect, [0.0, 0.0, 70.0, 20.0]);
    assert_eq!(rects[2].rect, [0.0, 24.0, 70.0, 20.0]);
    let root=Component::parse(json!({"id":"root","kind":"panel","children":[{"id":"a","kind":"text","rect":[40,50,200,20]}]})).unwrap();
    let rects = layout_rects(&root, [1920.0, 1080.0]).unwrap();
    assert_eq!(rects[0].rect, [0.0, 0.0, 1920.0, 1080.0]);
    assert_eq!(rects[1].rect, [40.0, 50.0, 200.0, 20.0]);
}

#[test]
fn custom_fonts_are_safe_project_resources_and_inherit_without_overriding_children() {
    let root=Component::parse(json!({"id":"root","kind":"column","style":{"font":"fonts/ui.ttf"},"children":[{"id":"title","kind":"text"},{"id":"other","kind":"text","font":"fonts/title.otf"}]})).unwrap();
    assert_eq!(root.effective_font("title"), Some("fonts/ui.ttf"));
    assert_eq!(root.effective_font("other"), Some("fonts/title.otf"));
    assert_eq!(root.effective_font("missing"), None);
    for font in [
        "/tmp/global.ttf",
        "../outside.ttf",
        "https://example.com/font.ttf",
        "font.png",
    ] {
        assert!(Component::parse(json!({"id":"x","kind":"text","font":font})).is_err());
    }
}
