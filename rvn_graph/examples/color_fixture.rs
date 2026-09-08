use rvn_graph::*;
fn main() {
    let path = std::env::args().nth(1).expect("isolated fixture output path");
    let mut g = GraphDocument::new(GraphId::new(1), GraphKind::Label { name: "start".into() });
    g.add_catalog_node(NodeKind::Label, [0.0, -110.0]).unwrap();
    let color = g.add_catalog_node(NodeKind::MakeColor, [0.0, 100.0]).unwrap();
    let red = g.pin_by_key(color, "r").unwrap().id;
    g.pins.get_mut(&red).unwrap().default_value = Some(PropertyValue::Float(255.0));
    for (i, (name, kind, value)) in [("Exemple Num", ValueType::Int, PropertyValue::Int(0)),
        ("Exemple Bool", ValueType::Bool, PropertyValue::Bool(false)),
        ("Var", ValueType::String, PropertyValue::String("texte ici".into()))].into_iter().enumerate() {
        let name = name.replace(' ', "_");
        g.add_variable(&name, kind, value).unwrap();
        let set = g.add_catalog_node(NodeKind::SetVariable, [240.0 + (i % 2) as f64 * 280.0, (i / 2) as f64 * 150.0]).unwrap();
        let get = g.add_catalog_node(NodeKind::VariableGet, [285.0 + (i % 2) as f64 * 280.0, 95.0 + (i / 2) as f64 * 150.0]).unwrap();
        for node in [set, get] { g.set_property(node, "name", PropertyValue::String(name.clone())).unwrap(); }
        if name == "Exemple_Bool" {
            let pin = g.pin_by_key(set, "value").unwrap().id;
            g.pins.get_mut(&pin).unwrap().value_type = ValueType::Bool;
            g.pins.get_mut(&pin).unwrap().default_value = Some(PropertyValue::Bool(true));
        }
    }
    std::fs::write(path, g.to_pretty_json().unwrap()).unwrap();
}
