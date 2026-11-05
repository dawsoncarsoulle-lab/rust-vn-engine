//! Durable two-UUID restart frontiers and existing admission guard refusal.
//! CPU filesystem tests only; no product hook, extra writer or UI acceptance.
use super::*;
use rvn_parser::widget_asset::{WidgetBlueprintV1, WidgetGraphRef16};
use std::collections::BTreeSet;
const A: &str = "17411a3e431e4c9388b78a2b9825b130";
const B: &str = "797679920f7142be99d9d6fe5553ca34";
const BASE: &str = "init {set clicks=0}\nlabel start\n\"Ready\"\nreturn\n";
const ADDED: &str = "screen panel_a() {return component(\"root\",\"panel\",{},[])}\nhandler panel_a_event(event) {return 0}\nfunction panel_a_value() {return 0}\nscreen panel_b() {return component(\"root\",\"panel\",{},[])}\nhandler panel_b_event(event) {return 0}\nfunction panel_b_value() {return 0}\n";
struct Fixture {
    root: PathBuf,
    workspace: SourceWorkspace,
    graphs: Vec<GraphDocument>,
    widgets: Vec<WidgetRecovery>,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("rvn-widget-batch-failure-0938-{}", unique_name()));
        fs::create_dir_all(root.join("assets/widgets")).unwrap();
        fs::write(root.join("main.rvn"), BASE).unwrap();
        fs::write(
            root.join("rvn.toml"),
            "[project]\ntitle='Two Widget frontiers'\n",
        )
        .unwrap();
        let workspace = SourceWorkspace::link(&root, &root.join("main.rvn"), &[]).unwrap();
        let graphs = SourceProject::open_at(
            &root.join("main.rvn"),
            format!("{ADDED}{BASE}"),
            workspace.project().graphs(),
        )
        .unwrap()
        .graphs()
        .to_vec();
        let mut widgets = Vec::new();
        let mut identities = BTreeSet::new();
        for (uuid, prefix) in [(A, "panel_a"), (B, "panel_b")] {
            let members = graphs
                .iter()
                .filter(|g| match &g.kind {
                    crate::GraphKind::Screen { name }
                    | crate::GraphKind::Handler { name }
                    | crate::GraphKind::Function { name, .. } => name.starts_with(prefix),
                    _ => false,
                })
                .collect::<Vec<_>>();
            assert_eq!(members.len(), 3);
            let screen = members
                .iter()
                .find(|g| matches!(g.kind, crate::GraphKind::Screen { .. }))
                .unwrap()
                .graph_id;
            let blueprint = WidgetBlueprintV1 {
                uuid: uuid.into(),
                source: "main.rvn".into(),
                screen_graph: WidgetGraphRef16::from_u64(screen.get()),
                members: members
                    .iter()
                    .map(|g| {
                        assert!(identities.insert(g.graph_id));
                        WidgetGraphRef16::from_u64(g.graph_id.get())
                    })
                    .collect(),
            };
            widgets.push(WidgetRecovery {
                locator: format!("assets/widgets/{prefix}.rvnwidget"),
                blueprint_json: blueprint.encode().unwrap(),
            });
        }
        assert_eq!(identities.len(), 6);
        Self {
            root,
            workspace,
            graphs,
            widgets,
        }
    }
    fn write(&mut self) {
        self.workspace
            .write_recovery_with_widgets(&self.graphs, &self.widgets)
            .unwrap();
    }
    fn bytes(&self) -> Vec<u8> {
        fs::read(self.root.join(RECOVERY)).unwrap()
    }
    fn journal(&self) -> serde_json::Value {
        let mut next = self.workspace.project().clone();
        next.apply_visual(BASE, &self.graphs).unwrap();
        let snapshot = Snapshot {
            asset_placements:None,
            graph_id_high_water: None,
            version: 1,
            source: "main.rvn".into(),
            source_snapshot: next.source().into(),
            graphs: next.graphs().to_vec(),
            imports: next.imports_snapshot(),
        };
        let manifest=format!("[project]\ntitle='Two Widget frontiers'\n[widget_blueprints]\n{A}='widgets/panel_a.rvnwidget'\n{B}='widgets/panel_b.rvnwidget'\n");
        let mut files = vec![
            SourceRefactorFile {
                path: "main.rvn".into(),
                before: Some(BASE.into()),
                after: Some(next.source().into()),
            },
            SourceRefactorFile {
                path: SIDECAR.into(),
                before: Some(fs::read_to_string(self.root.join(SIDECAR)).unwrap()),
                after: Some(serde_json::to_string_pretty(&snapshot).unwrap()),
            },
            SourceRefactorFile {
                path: "rvn.toml".into(),
                before: Some(fs::read_to_string(self.root.join("rvn.toml")).unwrap()),
                after: Some(manifest),
            },
        ];
        files.extend(self.widgets.iter().map(|widget| SourceRefactorFile {
            path: widget.locator.clone(),
            before: None,
            after: Some(widget.blueprint_json.clone()),
        }));
        assert_eq!(files.len(), 5);
        serde_json::json!({"source":"main.rvn","graphs":next.graphs(),"files":files,"recovery":String::from_utf8(self.bytes()).unwrap()})
    }
    fn archive_contains(&self, bytes: &[u8]) -> bool {
        fs::read_dir(self.root.join(".rvn-backups"))
            .unwrap()
            .flatten()
            .any(|entry| {
                fs::read(entry.path().join("recovery.json")).is_ok_and(|actual| actual == bytes)
            })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let (Ok(root), Ok(temp)) = (
            self.root.canonicalize(),
            std::env::temp_dir().canonicalize(),
        ) else {
            return;
        };
        if root.parent() == Some(temp.as_path())
            && root.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .starts_with("rvn-widget-batch-failure-0938-")
            })
        {
            let _ = fs::remove_dir_all(root);
        }
    }
}
#[test]
fn widget_batch0938_two_uuid_all_five_durable_file_prefixes_recover_and_reopen_idempotently() {
    // These are explicit durable-crash snapshots, never forged save receipts.
    for prefix in 0..=5 {
        let mut f = Fixture::new();
        f.write();
        let cache = f.bytes();
        let journal = f.journal();
        let files: Vec<SourceRefactorFile> =
            serde_json::from_value(journal["files"].clone()).unwrap();
        let expected: Vec<GraphDocument> =
            serde_json::from_value(journal["graphs"].clone()).unwrap();
        fs::write(
            f.root.join(refactor::REFACTOR_JOURNAL),
            serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        for file in files.iter().take(prefix) {
            fs::write(f.root.join(&file.path), file.after.as_ref().unwrap()).unwrap();
        }
        let reopened = SourceWorkspace::open(&f.root).unwrap().unwrap();
        assert_eq!(
            reopened.project().graphs(),
            expected.as_slice(),
            "durable prefix {prefix}"
        );
        for file in &files {
            assert_eq!(
                read_optional(&f.root.join(&file.path)).unwrap(),
                file.after,
                "durable prefix {prefix}, {}",
                file.path
            );
        }
        assert!(reopened.recovery_batch().unwrap().is_none());
        assert!(f.archive_contains(&cache));
        assert!(!f.root.join(refactor::REFACTOR_JOURNAL).exists());
        let mut ids = BTreeSet::new();
        for (widget, uuid) in f.widgets.iter().zip([A, B]) {
            let bp = WidgetBlueprintV1::decode(&fs::read(f.root.join(&widget.locator)).unwrap())
                .unwrap();
            assert_eq!(bp.uuid, uuid);
            assert_eq!(bp.members.len(), 3);
            for member in bp.members {
                assert!(ids.insert(member.get()));
            }
        }
        assert_eq!(ids.len(), 6);
        let backups = fs::read_dir(f.root.join(".rvn-backups")).unwrap().count();
        let again = SourceWorkspace::open(&f.root).unwrap().unwrap();
        assert_eq!(again.project().graphs(), expected.as_slice());
        assert!(again.recovery_batch().unwrap().is_none());
        assert_eq!(
            fs::read_dir(f.root.join(".rvn-backups")).unwrap().count(),
            backups
        );
    }
}
#[test]
fn widget_batch0938_refused_admission_releases_lock_and_preserves_owned_source_and_cache() {
    let mut f = Fixture::new();
    f.write();
    let cache = f.bytes();
    let baseline = f.workspace.project().graphs().to_vec();
    let mut other = SourceWorkspace::open(&f.root).unwrap().unwrap();
    let refused: Result<(), String> = f
        .workspace
        .with_recovery_admission(|_, _| Err("intentional consumer refusal".into()));
    assert_eq!(refused, Err("intentional consumer refusal".into()));
    assert_eq!(f.bytes(), cache);
    assert_eq!(f.workspace.project().graphs(), baseline.as_slice());
    assert!(!f.root.join(refactor::REFACTOR_JOURNAL).exists());
    // This is the real cooperative writer lock, not a test boolean guard.
    other
        .write_recovery_with_widgets(&f.graphs, &f.widgets)
        .unwrap();
    assert_eq!(f.bytes(), cache);
    f.workspace.with_authoring_admission(|_| Ok(())).unwrap();
}
