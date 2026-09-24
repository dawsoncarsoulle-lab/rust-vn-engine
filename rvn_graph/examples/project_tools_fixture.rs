//! Disposable project for native editor organization/reference tests.
use rvn_graph::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("new output folder required")?,
    );
    std::fs::create_dir(&root)?;
    std::fs::create_dir_all(root.join("graphs/01_intro"))?;
    std::fs::create_dir(root.join("graphs/02_suite"))?;
    std::fs::create_dir(root.join("assets"))?;
    let mut start = GraphDocument::new(
        GraphId::new(1),
        GraphKind::Label {
            name: "intro".into(),
        },
    );
    let entry = start.add_catalog_node(NodeKind::Label, [0.0, 0.0])?;
    let jump = start.add_catalog_node(NodeKind::Jump, [480.0, 0.0])?;
    let target = start.add_catalog_node(NodeKind::LabelValue, [220.0, 150.0])?;
    start.set_property(target, "label", PropertyValue::String("suite".into()))?;
    start.connect(
        start.pin_by_key(entry, "exec_out").unwrap().id,
        start.pin_by_key(jump, "exec_in").unwrap().id,
    )?;
    start.connect(
        start.pin_by_key(target, "value").unwrap().id,
        start.pin_by_key(jump, "target").unwrap().id,
    )?;
    let mut suite = GraphDocument::new(
        GraphId::new(2),
        GraphKind::Label {
            name: "suite".into(),
        },
    );
    let entry = suite.add_catalog_node(NodeKind::Label, [0.0, 0.0])?;
    let text = suite.add_catalog_node(NodeKind::Dialogue, [300.0, 0.0])?;
    suite.set_pin_default(
        text,
        "text",
        PropertyValue::String("La référence de chapitre fonctionne.".into()),
    )?;
    suite.connect(
        suite.pin_by_key(entry, "exec_out").unwrap().id,
        suite.pin_by_key(text, "exec_in").unwrap().id,
    )?;
    suite.materialize_visible_defaults()?;
    std::fs::write(
        root.join("graphs/01_intro/intro.rvngraph"),
        start.to_pretty_json()?,
    )?;
    std::fs::write(
        root.join("graphs/02_suite/suite.rvngraph"),
        suite.to_pretty_json()?,
    )?;
    std::fs::write(
        root.join("project.generated.rvn"),
        transpile_project(&[start, suite])?.source,
    )?;
    std::fs::write(root.join("rvn.toml"),"[project]\ntitle = \"Organisation QA\"\nmain_script = \"project.generated.rvn\"\nstart_label = \"intro\"\n[paths]\nassets = \"assets\"\nsaves = \"saves\"\n")?;
    Ok(())
}
