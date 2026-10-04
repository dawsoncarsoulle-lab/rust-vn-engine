use rvn_graph::{import_script, reimport_script, transpile, GraphDocument, GraphKind, NodeKind, PropertyValue, SourceProject, SourceWorkspace, ValueType, VariableScope};
use rvn_parser::{Statement, Value};
use std::{fs, path::PathBuf};

#[derive(Default)]
struct InterfaceRenderer { views: Vec<rvn_ui::programmable::ScreenView> }
impl rvn_core::Renderer for InterfaceRenderer {
    fn supports_programmable_ui(&self) -> bool { true }
    fn update_interfaces(&mut self, views: &[rvn_ui::programmable::ScreenView]) -> Result<(), String> {
        self.views = views.to_vec(); Ok(())
    }
    fn set_background(&mut self, _: &str, _: &rvn_parser::Transition) {}
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &rvn_parser::Position, _: &rvn_parser::Transition, _: Option<&rvn_core::SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &rvn_parser::Transition, _: &rvn_core::SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &rvn_parser::Position, _: &rvn_parser::Transition, _: &rvn_core::SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize { 0 }
    fn music_play(&mut self, _: &str, _: &rvn_parser::Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &rvn_parser::Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &rvn_parser::Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &rvn_parser::Transition) {}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[rvn_parser::Hotspot]) -> usize { 0 }
}

const MAIN: &str = concat!(
    "// Main source remains authored — été\r\nuse \"library.rvn\"\r\n",
    "init { set prefix = \"Local init\" }\r\n",
    "function caption(){return imported_caption_value}\r\n",
    "function amount(){return imported_score + 1}\r\n",
    "function shadow(imported_score){return imported_score + 1}\r\n",
    "screen form(){return component(\"root\",\"column\",{},[component(\"caption\",\"text\",{\"text\":caption()},[]),component(\"direct\",\"text\",{\"text\":imported_caption_value},[]),component(\"prefix\",\"text\",{\"text\":prefix},[])])}\r\n",
    "label start\r\nset imported_score = imported_score + 1\r\n",
    "ui.open(\"form\",[],false,1)\r\n\"[amount()]\"\r\nreturn\r\n"
);
const LIBRARY: &str = concat!(
    "// Exact imported CRLF and Unicode — 雨\r\n",
    "init { set prefix = \"Imported init\" set imported_caption_value = \"Imported value\" set imported_score = 4 }\r\n",
    "function imported_helper(value){return value}\r\n",
    "screen imported_form(){return component(\"imported\",\"text\",{\"text\":\"Read only\"},[])}\r\n"
);
const HOMONYM: &str = "init { set imported_caption_value = false set imported_score = \"Wrong root homonym\" }\n";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("rvn-imported-globals-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("scripts")).unwrap();
        fs::write(path.join("scripts/main.rvn"), MAIN).unwrap();
        fs::write(path.join("scripts/library.rvn"), LIBRARY).unwrap();
        fs::write(path.join("library.rvn"), HOMONYM).unwrap();
        Self(path)
    }
    fn main(&self) -> PathBuf { self.0.join("scripts/main.rvn") }
    fn assert_sources(&self, main: &str, library: &str) {
        assert_eq!(fs::read(self.main()).unwrap(), main.as_bytes());
        assert_eq!(fs::read(self.0.join("scripts/library.rvn")).unwrap(), library.as_bytes());
        assert_eq!(fs::read(self.0.join("library.rvn")).unwrap(), HOMONYM.as_bytes());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        assert_eq!(path.parent(), Some(std::env::temp_dir().canonicalize().unwrap().as_path()));
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("rvn-imported-globals-"));
        assert!(!fs::symlink_metadata(&self.0).unwrap().file_type().is_symlink());
        fs::remove_dir_all(path).unwrap();
    }
}

fn amount_edit(graphs: &mut [GraphDocument]) {
    let graph = graphs.iter_mut().find(|graph| matches!(&graph.kind, GraphKind::Function { name } if name == "amount")).unwrap();
    let literal = graph.nodes.values().find(|node| node.properties.get("value") == Some(&PropertyValue::Int(1))).unwrap().id;
    graph.set_property(literal, "value", PropertyValue::Int(2)).unwrap();
}

#[test]
fn linked_local_functions_and_screens_use_real_imported_global_types_without_owning_their_init() {
    let fixture = Fixture::new();
    let mut workspace = SourceWorkspace::link(&fixture.0, &fixture.main(), &[]).unwrap();
    let project = workspace.project();
    assert!(project.is_imported_global("imported_caption_value"));
    assert!(project.is_imported_global("imported_score"), "A runtime Set must not transfer definition ownership");
    assert!(!project.is_imported_global("prefix"), "The authored Init owns its override");
    for graph in project.graphs() {
        assert!(match &graph.kind {
            GraphKind::Init => true,
            GraphKind::Label { name } => name == "start",
            GraphKind::Function { name } => ["caption", "amount", "shadow"].contains(&name.as_str()),
            GraphKind::Screen { name } => name == "form",
            _ => false,
        });
        assert_eq!(graph.variables["imported_caption_value"].value_type, ValueType::String);
        if matches!(&graph.kind, GraphKind::Function { name } if name == "shadow") {
            assert_eq!(graph.variable_scope("imported_score"), Some(VariableScope::Parameter));
            assert_eq!(graph.variables["imported_score"].value_type, ValueType::Any);
        } else {
            assert_eq!(graph.variables["imported_score"].value_type, ValueType::Int);
        }
        if graph.kind == GraphKind::Init {
            let ast = transpile(graph).unwrap().ast;
            assert!(matches!(ast.as_slice(), [Statement::Init { body }] if body.len() == 1 && matches!(&body[0], Statement::SetVar { name, .. } if name == "prefix")), "Imported definitions must never appear in the authored Init: {ast:?}");
        }
    }
    let script = project.resolved_script().unwrap();
    assert_eq!(script.iter().filter(|statement| matches!(statement, Statement::Init { .. })).count(), 2);
    let mut engine = rvn_core::Engine::new(script, InterfaceRenderer::default(), 16).unwrap();
    assert!(matches!(engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "6"));
    assert_eq!(engine.state.vars["imported_score"], Value::Int(5), "Runtime Set on an imported global remains executable");
    let views = engine.interface_views().unwrap();
    assert_eq!(views[0].root.find("caption").unwrap().text, "Imported value");
    assert_eq!(views[0].root.find("direct").unwrap().text, "Imported value");
    assert_eq!(views[0].root.find("prefix").unwrap().text, "Local init");
    assert_eq!(engine.renderer.views[0].root.find("direct").unwrap().text, "Imported value");
    fixture.assert_sources(MAIN, LIBRARY);
    workspace.save(&workspace.project().graphs().to_vec()).unwrap();
    fixture.assert_sources(MAIN, LIBRARY);
    let mut graphs = workspace.project().graphs().to_vec();
    amount_edit(&mut graphs);
    let start = graphs.iter_mut().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap();
    let literal = start.nodes.values().find(|node| node.properties.get("value") == Some(&PropertyValue::Int(1))).unwrap().id;
    start.set_property(literal, "value", PropertyValue::Int(2)).unwrap();
    workspace.save(&graphs).unwrap();
    let saved = workspace.project().source().to_owned();
    assert_eq!(saved.matches("init {").count(), 1, "Saving a local function must never copy imported Init into main");
    assert!(saved.contains("// Main source remains authored — été"));
    fixture.assert_sources(&saved, LIBRARY);
    let reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
    assert!(reopened.source_error.is_none());
    assert_eq!(reopened.project().graphs(), workspace.project().graphs());
    assert!(reopened.project().is_imported_global("imported_score"));
    let mut engine = rvn_core::Engine::new(reopened.project().resolved_script().unwrap(), InterfaceRenderer::default(), 16).unwrap();
    assert!(matches!(engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "8"));
    assert_eq!(engine.state.vars["imported_score"], Value::Int(6), "Editing a runtime Set is allowed without editing the imported definition");
}

#[test]
fn imported_types_do_not_relax_unknown_variables_or_strict_public_import() {
    let fixture = Fixture::new();
    let unknown = MAIN.replace("return imported_caption_value", "return absent_global");
    assert!(SourceProject::open_at(&fixture.main(), &unknown, &[]).unwrap_err().contains("absent_global"));
    let local = rvn_parser::parse(MAIN).unwrap().into_iter().filter(|statement| !matches!(statement, Statement::Use { .. })).collect();
    assert!(import_script(&local).is_err());
    assert!(reimport_script(&local, &[]).is_err());
    let typed_source = MAIN.replace("set imported_score = imported_score + 1", "set imported_score = imported_score - 1");
    let project = SourceProject::open_at(&fixture.main(), &typed_source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let graph = graphs.iter_mut().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap();
    let add = graph.nodes.values().find(|node| node.kind == NodeKind::MathSubtract).unwrap().id;
    let value = graph.pin_by_key(add, "right").unwrap().id;
    let producer = graph.pins[&graph.edges.values().find(|edge| edge.input == value).unwrap().output].node;
    graph.set_property(producer, "value", PropertyValue::String("An incompatible typed operand".into())).unwrap();
    let mut project = project;
    assert!(project.apply_visual(&typed_source, &graphs).is_err(), "Imported Int type must still reject an incompatible wired operand");
    assert_eq!(project.source(), typed_source);
    fixture.assert_sources(MAIN, LIBRARY);
}

#[test]
fn imported_definition_edits_and_changed_imports_preserve_main_presentation_and_recovery() {
    let fixture = Fixture::new();
    let mut workspace = SourceWorkspace::link(&fixture.0, &fixture.main(), &[]).unwrap();
    let baseline = workspace.project().graphs().to_vec();
    let mut invalid = baseline.clone();
    let graph = invalid.iter_mut().find(|graph| matches!(&graph.kind, GraphKind::Function { name } if name == "caption")).unwrap();
    graph.rename_variable("imported_caption_value", "renamed_import").unwrap();
    assert!(workspace.save(&invalid).unwrap_err().contains("read-only definition"));
    assert_eq!(workspace.project().graphs(), baseline);
    let mut graphs = baseline.clone();
    amount_edit(&mut graphs);
    workspace.write_recovery(&graphs).unwrap();
    let sidecar = fs::read(fixture.0.join(".rvn-authoring.json")).unwrap();
    let recovery = fs::read(fixture.0.join(".rvn-authoring.recovery.json")).unwrap();
    let external = format!("{LIBRARY}// External writer\r\n");
    fs::write(fixture.0.join("scripts/library.rvn"), &external).unwrap();
    assert!(workspace.save(&graphs).unwrap_err().contains("changed externally"));
    assert_eq!(workspace.project().graphs(), baseline);
    assert_eq!(fs::read(fixture.0.join(".rvn-authoring.json")).unwrap(), sidecar);
    assert_eq!(fs::read(fixture.0.join(".rvn-authoring.recovery.json")).unwrap(), recovery);
    fixture.assert_sources(MAIN, &external);
    fs::remove_file(fixture.0.join("scripts/library.rvn")).unwrap();
    let reopened = SourceWorkspace::open(&fixture.0).unwrap().unwrap();
    assert!(reopened.source_error.is_some());
    assert_eq!(reopened.project().graphs(), baseline, "Cached typed context must retain the last valid graph when a dependency disappears");
    assert!(reopened.project().is_imported_global("imported_caption_value"));
    assert_eq!(fs::read(fixture.main()).unwrap(), MAIN.as_bytes());
    assert_eq!(fs::read(fixture.0.join(".rvn-authoring.json")).unwrap(), sidecar);
    assert_eq!(fs::read(fixture.0.join(".rvn-authoring.recovery.json")).unwrap(), recovery);
    assert_eq!(fs::read(fixture.0.join("library.rvn")).unwrap(), HOMONYM.as_bytes());
}
