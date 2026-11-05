use rvn_graph::*;
use rvn_parser::localized_text::StringTableReference;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const OWNER: &str = "b9ab9b6d1c9448ad8dcd799c23b4b550";
const TABLE: &str = "17411a3e431e4c9388b78a2b9825b130";
fn script() -> rvn_parser::Script {
    rvn_parser::parse(&format!("init {{set first=__rvn_string_table_text(\"{TABLE}\",\"hello\") set second=__rvn_string_table_text(\"{TABLE}\",\"name\") set message=__rvn_format_text(__rvn_string_table_text(\"{TABLE}\",\"hello\"),{{\"Name\":second,\"0\":2}})}}\nscene label start\n\"[message]\"\nreturn\n")).unwrap()
}
fn seed() -> GraphDocument {
    let mut graph = GraphDocument::new(GraphId(1), GraphKind::Init);
    graph
        .enable_text_owner(TextOwnerId::parse(OWNER).unwrap())
        .unwrap();
    graph.schema_version = GRAPH_TABLE_TEXT_SCHEMA_VERSION;
    graph.add_catalog_node(NodeKind::Init, [0., 0.]).unwrap();
    graph
}
fn table_pin(graph: &GraphDocument, key: &str) -> PinId {
    graph
        .pins
        .values()
        .find(|pin| {
            pin.direction == PinDirection::Output
                && pin.text_table_reference.as_ref()
                    == Some(&StringTableReference {
                        table: TABLE.into(),
                        key: key.into(),
                    })
        })
        .unwrap()
        .id
}

#[test]
fn explicit_source_scope_retains_table_defaults_argument_names_and_node_identities() {
    let script = script();
    let graphs = reimport_script(&script, &[seed()]).unwrap();
    let init = graphs
        .iter()
        .find(|graph| graph.kind == GraphKind::Init)
        .unwrap();
    assert_eq!(init.schema_version, 8);
    let first = table_pin(init, "hello");
    let second = table_pin(init, "name");
    assert_ne!(first, second);
    let format = init
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::FormatText)
        .unwrap()
        .id;
    let pattern = init.pin_by_key(format, "format").unwrap().id;
    assert_eq!(
        init.pin_text_table_reference(pattern).unwrap(),
        Some(&StringTableReference {
            table: TABLE.into(),
            key: "hello".into()
        })
    );
    assert!(init.pin_by_key(format, "argument_Name").is_some());
    assert!(init.pin_by_key(format, "argument_0").is_some());
    let generated = transpile_project(&graphs).unwrap();
    assert_eq!(reimport_script(&script, &graphs).unwrap(), graphs);
    let source = generated
        .source
        .replacen("init {", "init {\n    set extra = 3", 1);
    let changed = reimport_script(&rvn_parser::parse(&source).unwrap(), &graphs).unwrap();
    let current = changed.iter().find(|g| g.kind == GraphKind::Init).unwrap();
    assert_eq!(table_pin(current, "hello"), first);
    assert_eq!(table_pin(current, "name"), second);
    assert_eq!(current.pin_by_key(format, "format").unwrap().id, pattern);
    assert_eq!(current.text_owner, init.text_owner);
    current.validate_narrative_metadata().unwrap();
}

#[test]
fn source_reordering_matches_references_by_semantic_pair_and_legacy_import_stays_generic() {
    let before = script();
    let graphs = reimport_script(&before, &[seed()]).unwrap();
    let init = graphs.iter().find(|g| g.kind == GraphKind::Init).unwrap();
    let first = table_pin(init, "hello");
    let second = table_pin(init, "name");
    let source = transpile_project(&graphs).unwrap().source;
    let lines: Vec<_> = source.lines().collect();
    let a = lines
        .iter()
        .position(|line| line.contains("set first ="))
        .unwrap();
    let b = lines
        .iter()
        .position(|line| line.contains("set second ="))
        .unwrap();
    let mut swapped = lines.clone();
    swapped.swap(a, b);
    let refreshed =
        reimport_script(&rvn_parser::parse(&swapped.join("\n")).unwrap(), &graphs).unwrap();
    let init = refreshed
        .iter()
        .find(|g| g.kind == GraphKind::Init)
        .unwrap();
    assert_eq!(table_pin(init, "hello"), first);
    assert_eq!(table_pin(init, "name"), second);
    let legacy = import_script(&before).unwrap();
    assert!(legacy
        .iter()
        .all(|g| g.pins.values().all(|p| p.text_table_reference.is_none())));
    assert_eq!(
        &transpile_project(&legacy).unwrap().ast[..before.len()],
        before.as_slice()
    );
    let mut owner = seed();
    owner.schema_version = 7;
    let old_owned = reimport_script(&before, &[owner]).unwrap();
    assert!(old_owned
        .iter()
        .all(|g| g.pins.values().all(|p| p.text_table_reference.is_none())));
    assert!(old_owned
        .iter()
        .find(|g| g.kind == GraphKind::Init)
        .unwrap()
        .nodes
        .values()
        .any(|n| n.properties.get("function")
            == Some(&PropertyValue::String("__rvn_string_table_text".into()))));
}

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn repeated_same_table_pair_keeps_each_producer_with_its_named_consumer_after_reorder() {
    let source = transpile_project(&reimport_script(&script(), &[seed()]).unwrap())
        .unwrap()
        .source;
    let source = source.replace(
        &format!("\"{TABLE}\", \"name\""),
        &format!("\"{TABLE}\", \"hello\""),
    );
    let graphs = reimport_script(&rvn_parser::parse(&source).unwrap(), &[seed()]).unwrap();
    let assigned = |graph: &GraphDocument, name: &str| {
        let node = graph
            .nodes
            .values()
            .find(|node| {
                node.kind == NodeKind::SetVariable
                    && node.properties.get("name") == Some(&PropertyValue::String(name.into()))
            })
            .unwrap()
            .id;
        let input = graph.pin_by_key(node, "value").unwrap().id;
        graph
            .edges
            .values()
            .find(|edge| edge.input == input)
            .unwrap()
            .output
    };
    let init = graphs.iter().find(|g| g.kind == GraphKind::Init).unwrap();
    let first = assigned(init, "first");
    let second = assigned(init, "second");
    assert_ne!(first, second);
    assert_eq!(
        init.pins[&first].text_table_reference,
        init.pins[&second].text_table_reference
    );
    let generated = transpile_project(&graphs).unwrap().source;
    let mut lines: Vec<_> = generated.lines().collect();
    let a = lines
        .iter()
        .position(|line| line.contains("set first ="))
        .unwrap();
    let b = lines
        .iter()
        .position(|line| line.contains("set second ="))
        .unwrap();
    lines.swap(a, b);
    let refreshed =
        reimport_script(&rvn_parser::parse(&lines.join("\n")).unwrap(), &graphs).unwrap();
    let init = refreshed
        .iter()
        .find(|g| g.kind == GraphKind::Init)
        .unwrap();
    assert_eq!(assigned(init, "first"), first);
    assert_eq!(assigned(init, "second"), second);
}

struct Fixture(PathBuf);

#[test]
fn inserting_an_identical_reference_before_existing_statements_does_not_take_their_identity() {
    let graphs = reimport_script(&script(), &[seed()]).unwrap();
    let assigned = |graph: &GraphDocument, name: &str| {
        let node = graph
            .nodes
            .values()
            .find(|node| {
                node.kind == NodeKind::SetVariable
                    && node.properties.get("name") == Some(&PropertyValue::String(name.into()))
            })
            .unwrap()
            .id;
        let input = graph.pin_by_key(node, "value").unwrap().id;
        graph
            .edges
            .values()
            .find(|edge| edge.input == input)
            .unwrap()
            .output
    };
    let init = graphs.iter().find(|g| g.kind == GraphKind::Init).unwrap();
    let first = assigned(init, "first");
    let second = assigned(init, "second");
    let source = transpile_project(&graphs).unwrap().source.replacen(
        "init {",
        &format!("init {{\n    set inserted = __rvn_string_table_text(\"{TABLE}\", \"hello\")"),
        1,
    );
    let refreshed = reimport_script(&rvn_parser::parse(&source).unwrap(), &graphs).unwrap();
    let init = refreshed
        .iter()
        .find(|g| g.kind == GraphKind::Init)
        .unwrap();
    assert_eq!(assigned(init, "first"), first);
    assert_eq!(assigned(init, "second"), second);
    assert_ne!(assigned(init, "inserted"), first);
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "rvn-table-pin-source-0116-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let root = self.0.canonicalize().unwrap();
        let temp = std::env::temp_dir().canonicalize().unwrap();
        assert_eq!(root.parent(), Some(temp.as_path()));
        assert!(root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("rvn-table-pin-source-0116-"));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn real_source_save_recovery_cold_open_and_failed_source_refresh_preserve_table_pin_data() {
    let fixture = Fixture::new();
    let path = fixture.0.join("main.rvn");
    let initial = "// authored comment kept\ninit {}\nscene label start\n\"Waiting\"\nreturn\n";
    std::fs::write(&path, initial).unwrap();
    let mut workspace = SourceWorkspace::link(&fixture.0, &path, &[]).unwrap();
    let mut previous = workspace.project().graphs().to_vec();
    let init = previous
        .iter_mut()
        .find(|g| g.kind == GraphKind::Init)
        .unwrap();
    init.enable_text_owner(TextOwnerId::parse(OWNER).unwrap())
        .unwrap();
    init.schema_version = 8;
    let graphs = reimport_script(&script(), &previous).unwrap();
    let table = table_pin(
        graphs.iter().find(|g| g.kind == GraphKind::Init).unwrap(),
        "hello",
    );
    workspace.write_recovery(&graphs).unwrap();
    assert_eq!(workspace.recovery().unwrap().unwrap(), graphs);
    workspace.save(&graphs).unwrap();
    assert!(workspace.recovery().unwrap().is_none());
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.starts_with("// authored comment kept"));
    assert!(saved.contains("__rvn_string_table_text"));
    let published = workspace.project().graphs().to_vec();
    drop(workspace);
    let cold = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
    assert_eq!(cold.project().graphs(), published);
    let source = saved.replacen("init {", "init {\n    set extra = 3", 1);
    std::fs::write(&path, &source).unwrap();
    drop(cold);
    let cold = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
    assert_eq!(
        table_pin(
            cold.project()
                .graphs()
                .iter()
                .find(|g| g.kind == GraphKind::Init)
                .unwrap(),
            "hello"
        ),
        table
    );
    let good = cold.project().graphs().to_vec();
    let current = cold.project().source().to_string();
    let mut checked = cold.project().clone();
    assert!(checked.refresh("init { broken (").is_err());
    assert_eq!(checked.graphs(), good);
    assert_eq!(checked.source(), current);
}
