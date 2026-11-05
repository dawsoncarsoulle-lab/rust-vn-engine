use super::*;
use rvn_parser::widget_asset::WidgetGraphRef16;
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};
const A: &str = "17411a3e431e4c9388b78a2b9825b130";
const B: &str = "797679920f7142be99d9d6fe5553ca34";
const SOURCE: &str = "screen panel_a(){return []}\nhandler panel_a_event(event){return 0}\nfunction panel_a_value(){return 1}\nscreen legacy_empty(){return []}\nscreen panel_b(){return component(\"root\",\"panel\",{},[])}\nhandler panel_b_event(event){return 0}\nlabel start\n\"Ready\"\nreturn\n";
struct Fixture { root: PathBuf, project: SourceProject, program: ParsedProgram }
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let name = format!("rvn-widget-class-admission-0950-{}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed));
        let path = std::env::temp_dir().join(name);
        fs::create_dir_all(&path).unwrap();
        let root = fs::canonicalize(path).unwrap();
        fs::write(root.join("main.rvn"), SOURCE).unwrap();
        let project = SourceProject::open_at(&root.join("main.rvn"), SOURCE, &[]).unwrap();
        let program = project.resolved_program().unwrap();
        Self { root, project, program }
    }
    fn blueprint(&self, uuid: &str, prefix: &str) -> WidgetBlueprintV1 {
        let members = self.project.graphs().iter().filter(|graph| definition(&graph.kind).is_some_and(|(_, name)| name.starts_with(prefix))).collect::<Vec<_>>();
        let screen = members.iter().find(|graph| matches!(graph.kind, GraphKind::Screen { .. })).unwrap();
        WidgetBlueprintV1 { uuid: uuid.into(), source: "main.rvn".into(), screen_graph: WidgetGraphRef16::from_u64(screen.graph_id.get()), members: members.iter().map(|graph| WidgetGraphRef16::from_u64(graph.graph_id.get())).collect() }
    }
    fn prepare(&self, blueprints: &[WidgetBlueprintV1]) -> Result<PreparedWidgetClasses, String> {
        let locators = (0..blueprints.len()).map(|i| format!("widgets/{i}.rvnwidget")).collect::<Vec<_>>();
        let entries = blueprints.iter().zip(&locators).map(|(blueprint, path)| (blueprint.uuid.as_str(), Some(path.as_str()))).collect::<Vec<_>>();
        let bytes = blueprints.iter().map(|blueprint| blueprint.encode().unwrap().into_bytes()).collect::<Vec<_>>();
        let assets = locators.iter().zip(&bytes).map(|(locator, bytes)| WidgetRuntimeAsset { locator, bytes }).collect::<Vec<_>>();
        prepare_widget_runtime_classes(&self.root, &self.program, WidgetIndexInput::Table(&entries), LocatorCase::Insensitive, &assets, &[WidgetRuntimeSource { source: "main.rvn", project: &self.project }])
    }
}
impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); } }

#[test]
fn actual_index_descriptor_source_and_members_admit_empty_class_without_tagging_legacy_empty() {
    let fixture = Fixture::new();
    let blueprint = fixture.blueprint(A, "panel_a");
    let admission = fixture.prepare(&[blueprint]).unwrap();
    assert_eq!(admission.classes().len(), 1);
    let class = admission.class(A).unwrap();
    assert_eq!(class.screen().definition(), "panel_a");
    assert_eq!(class.members().len(), 3);
    assert_eq!(class.source(), "main.rvn");
    assert_eq!(class.screen().origin().source.as_ref(), Some(&fixture.root.join("main.rvn")));
    assert!(admission.matches_program(&fixture.program));
    assert!(admission.classes().values().all(|class| class.screen().definition() != "legacy_empty"));
    let legacy = prepare_widget_runtime_classes(&fixture.root, &fixture.program, WidgetIndexInput::Absent, LocatorCase::Insensitive, &[], &[]).unwrap();
    assert!(legacy.classes().is_empty());
}

#[test]
fn two_actual_classes_are_atomic_and_second_invalid_uuid_or_shared_member_refuses_without_model_mutation() {
    let fixture = Fixture::new();
    let first = fixture.blueprint(A, "panel_a");
    let second = fixture.blueprint(B, "panel_b");
    assert_eq!(fixture.prepare(&[first.clone(), second.clone()]).unwrap().classes().len(), 2);
    let before = fixture.project.graphs().to_vec();
    let before_source = fixture.project.source().to_owned();
    let mut shared = second.clone();
    shared.members.push(first.members.iter().copied().find(|member| *member != first.screen_graph).unwrap());
    assert!(fixture.prepare(&[first.clone(), shared]).is_err());
    let mut missing = second;
    missing.screen_graph = WidgetGraphRef16::from_u64(u64::MAX);
    missing.members.push(missing.screen_graph);
    assert!(fixture.prepare(&[first, missing]).is_err());
    assert_eq!(fixture.project.graphs(), before);
    assert_eq!(fixture.project.source(), before_source);
    assert_eq!(fs::read_to_string(fixture.root.join("main.rvn")).unwrap(), SOURCE);
}

#[test]
fn descriptor_index_uuid_mismatch_and_unindexed_extra_asset_refuse_before_any_class_publication() {
    let fixture = Fixture::new();
    let bytes = fixture.blueprint(A, "panel_a").encode().unwrap();
    let entries = [(B, Some("widgets/panel.rvnwidget"))];
    let assets = [WidgetRuntimeAsset { locator: "widgets/panel.rvnwidget", bytes: bytes.as_bytes() }];
    let sources = [WidgetRuntimeSource { source: "main.rvn", project: &fixture.project }];
    assert!(prepare_widget_runtime_classes(&fixture.root, &fixture.program, WidgetIndexInput::Table(&entries), LocatorCase::Insensitive, &assets, &sources).is_err());
    assert!(prepare_widget_runtime_classes(&fixture.root, &fixture.program, WidgetIndexInput::Absent, LocatorCase::Insensitive, &assets, &sources).is_err());
}

#[test]
fn actual_definition_and_origin_change_in_frozen_program_refuses_even_with_same_empty_root_and_graph_ids() {
    let mut fixture = Fixture::new();
    let blueprint = fixture.blueprint(A, "panel_a");
    let admitted = fixture.prepare(&[blueprint.clone()]).unwrap();
    // Modify actual frozen bytes and AST together; the old authoritative model
    // cannot donate its graph identities to this new program incarnation.
    let changed = SOURCE.replace("function panel_a_value(){return 1}", "function panel_a_value(){return 2}");
    let changed_project = SourceProject::open_at(&fixture.root.join("main.rvn"), changed, &[]).unwrap();
    fixture.program = changed_project.resolved_program().unwrap();
    assert!(!admitted.matches_program(&fixture.program));
    assert!(fixture.prepare(&[blueprint]).is_err());
}
