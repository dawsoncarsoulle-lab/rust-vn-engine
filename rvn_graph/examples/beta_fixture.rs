//! Generate an isolated manual-QA graph, never the user's working graph.
use rvn_graph::*;
fn main() {
    let path = std::env::args().nth(1).expect("fixture output path");
    let mut g = GraphDocument::new(
        GraphId::new(1),
        GraphKind::Label {
            name: "start".into(),
        },
    );
    let root = g.add_catalog_node(NodeKind::Label, [0.0, 0.0]).unwrap();
    let choice = g.add_catalog_node(NodeKind::Choice, [260.0, 0.0]).unwrap();
    for text in ["Partir", "Ouvrir la porte", "Révéler le secret"] {
        g.add_choice_option(choice, text).unwrap();
    }
    g.set_property(
        choice,
        "option_1_condition",
        PropertyValue::String("possede_cle".into()),
    )
    .unwrap();
    g.set_property(
        choice,
        "option_2_condition",
        PropertyValue::String("confiance >= 50".into()),
    )
    .unwrap();
    g.connect(
        g.pin_by_key(root, "exec_out").unwrap().id,
        g.pin_by_key(choice, "exec_in").unwrap().id,
    )
    .unwrap();
    let function = g
        .add_catalog_node(NodeKind::FunctionCall, [0.0, 170.0])
        .unwrap();
    g.set_property(function, "function", PropertyValue::String("random".into()))
        .unwrap();
    g.resize_value_inputs(function, 2).unwrap();
    let list = g
        .add_catalog_node(NodeKind::ListLiteral, [260.0, 210.0])
        .unwrap();
    g.resize_value_inputs(list, 3).unwrap();
    g.connect(
        g.pin_by_key(function, "result").unwrap().id,
        g.pin_by_key(list, "item_0").unwrap().id,
    )
    .unwrap();
    let map = g
        .add_catalog_node(NodeKind::Imagemap, [550.0, 0.0])
        .unwrap();
    let pin = g.pin_by_key(map, "background").unwrap().id;
    g.pins.get_mut(&pin).unwrap().default_value = Some(PropertyValue::String("preview.png".into()));
    g.set_imagemap_hotspots(map, vec!["door:0:0:100:100".into()])
        .unwrap();
    g.add_catalog_node(NodeKind::SpriteEffect, [550.0, 210.0])
        .unwrap();
    std::fs::write(path, g.to_pretty_json().unwrap()).unwrap();
}
