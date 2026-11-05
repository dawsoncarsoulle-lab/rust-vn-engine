//! Actual source/recovery/group-journal CPU contracts; no UI acceptance claim.
use super::*;
use rvn_parser::widget_asset::{WidgetBlueprintV1, WidgetGraphRef16};
const UUID: &str = "17411a3e431e4c9388b78a2b9825b130";
#[test]
fn typed0935_admission_holds_existing_writer_lock_through_real_second_workspace_callback() {
    use std::sync::{mpsc, Arc, Barrier};
    use std::time::Duration;
    let mut f = Fixture::new();
    f.write();
    let bytes = f.recovery_bytes();
    let mut other = SourceWorkspace::open(&f.root).unwrap().unwrap();
    let mut changed = f.graphs.clone();
    changed[0].nodes.values_mut().next().unwrap().position[0] += 17.0;
    let widget = f.widget.clone();
    let barrier = Arc::new(Barrier::new(2));
    let worker_barrier = barrier.clone();
    let (result_tx, result_rx) = mpsc::channel();
    let (retry_tx, retry_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        worker_barrier.wait();
        result_tx
            .send(other.write_recovery_with_widgets(&changed, std::slice::from_ref(&widget)))
            .unwrap();
        retry_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        other.write_recovery_with_widgets(&changed, &[widget])
    });
    f.workspace
        .with_recovery_admission(|baseline, batch| {
            assert_eq!(baseline, f.workspace.project().graphs());
            assert_eq!(batch.graphs, f.graphs);
            assert_eq!(batch.widgets, vec![f.widget.clone()]);
            barrier.wait();
            let blocked = result_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            assert!(blocked.is_err());
            assert_eq!(f.recovery_bytes(), bytes);
            Ok(())
        })
        .unwrap();
    retry_tx.send(()).unwrap();
    worker.join().unwrap().unwrap();
    assert_ne!(f.recovery_bytes(), bytes);
    assert!(
        f.workspace.with_authoring_admission(|_| Ok(())).is_err(),
        "the original recovery owner must now refuse the foreign update"
    );
}
const BASE: &str = "init {set clicks=0}\nlabel start\n\"Ready\"\nreturn\n";
const ADDED:&str="function caption(value) {return value}\nhandler clicked(event) {set clicks=clicks+1}\nscreen panel() {return component(\"root\",\"text\",{\"text\":\"Panel\"},[])}\n";
struct Fixture {
    root: PathBuf,
    workspace: SourceWorkspace,
    graphs: Vec<GraphDocument>,
    widget: WidgetRecovery,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("rvn-widget-recovery-0935-{}", unique_name()));
        fs::create_dir_all(root.join("assets/widgets")).unwrap();
        fs::write(root.join("main.rvn"), BASE).unwrap();
        fs::write(root.join("rvn.toml"), "[project]\ntitle='Recovery'\n").unwrap();
        let workspace = SourceWorkspace::link(&root, &root.join("main.rvn"), &[]).unwrap();
        let graphs = SourceProject::open_at(
            &root.join("main.rvn"),
            format!("{ADDED}{BASE}"),
            workspace.project().graphs(),
        )
        .unwrap()
        .graphs()
        .to_vec();
        let members = graphs
            .iter()
            .filter(|g| {
                matches!(
                    g.kind,
                    crate::GraphKind::Screen { .. }
                        | crate::GraphKind::Handler { .. }
                        | crate::GraphKind::Function { .. }
                )
            })
            .map(|g| WidgetGraphRef16::from_u64(g.graph_id.0))
            .collect::<Vec<_>>();
        let screen_graph = graphs
            .iter()
            .find(|g| matches!(g.kind, crate::GraphKind::Screen { .. }))
            .unwrap()
            .graph_id;
        let blueprint = WidgetBlueprintV1 {
            uuid: UUID.into(),
            source: "main.rvn".into(),
            screen_graph: WidgetGraphRef16::from_u64(screen_graph.0),
            members,
        };
        let widget = WidgetRecovery {
            locator: "assets/widgets/panel.rvnwidget".into(),
            blueprint_json: blueprint.encode().unwrap(),
        };
        Self {
            root,
            workspace,
            graphs,
            widget,
        }
    }
    fn write(&mut self) {
        self.workspace
            .write_recovery_with_widgets(&self.graphs, std::slice::from_ref(&self.widget))
            .unwrap();
    }
    fn recovery_bytes(&self) -> Vec<u8> {
        fs::read(self.root.join(RECOVERY)).unwrap()
    }
    fn files(&self) -> Vec<SourceRefactorFile> {
        vec![
        SourceRefactorFile{path:"rvn.toml".into(),before:Some(fs::read_to_string(self.root.join("rvn.toml")).unwrap()),after:Some(format!("[project]\ntitle='Recovery'\n[widget_blueprints]\n{UUID}='widgets/panel.rvnwidget'\n"))},
        SourceRefactorFile{path:self.widget.locator.clone(),before:None,after:Some(self.widget.blueprint_json.clone())},
    ]
    }
    fn journal(&self) -> serde_json::Value {
        let mut next = self.workspace.project().clone();
        next.apply_visual(BASE, &self.graphs).unwrap();
        let sidecar = Snapshot {
            asset_placements:None,
            graph_id_high_water: None,
            version: 1,
            source: "main.rvn".into(),
            source_snapshot: next.source().into(),
            graphs: next.graphs().to_vec(),
            imports: next.imports_snapshot(),
        };
        let mut files = vec![
            SourceRefactorFile {
                path: "main.rvn".into(),
                before: Some(BASE.into()),
                after: Some(next.source().into()),
            },
            SourceRefactorFile {
                path: SIDECAR.into(),
                before: Some(fs::read_to_string(self.root.join(SIDECAR)).unwrap()),
                after: Some(serde_json::to_string_pretty(&sidecar).unwrap()),
            },
        ];
        files.extend(self.files());
        serde_json::json!({"source":"main.rvn","graphs":next.graphs(),"files":files,"recovery":String::from_utf8(self.recovery_bytes()).unwrap()})
    }
    fn install_journal(&self, journal: &serde_json::Value, published: usize) {
        fs::write(
            self.root.join(refactor::REFACTOR_JOURNAL),
            serde_json::to_vec(journal).unwrap(),
        )
        .unwrap();
        let files: Vec<SourceRefactorFile> =
            serde_json::from_value(journal["files"].clone()).unwrap();
        for file in files.iter().take(published) {
            fs::write(self.root.join(&file.path), file.after.as_ref().unwrap()).unwrap();
        }
    }
    fn archived(&self, bytes: &[u8]) -> bool {
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
        if let Ok(root) = self.root.canonicalize() {
            if root.parent() == std::env::temp_dir().canonicalize().ok().as_deref()
                && root
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("rvn-widget-recovery-0935-"))
            {
                let _ = fs::remove_dir_all(root);
            }
        }
    }
}

#[test]
fn typed0935_recovery_preserves_uuid_three_real_members_without_publishing() {
    let mut f = Fixture::new();
    let source = fs::read(f.root.join("main.rvn")).unwrap();
    let presentation = fs::read(f.root.join(SIDECAR)).unwrap();
    f.write();
    let reopened = SourceWorkspace::open(&f.root).unwrap().unwrap();
    let batch = reopened.recovery_batch().unwrap().unwrap();
    assert_eq!(batch.graphs, f.graphs);
    assert_eq!(batch.widgets, vec![f.widget.clone()]);
    assert_eq!(batch.widgets[0].blueprint().unwrap().members.len(), 3);
    assert!(reopened.recovery().is_err());
    assert_eq!(fs::read(f.root.join("main.rvn")).unwrap(), source);
    assert_eq!(fs::read(f.root.join(SIDECAR)).unwrap(), presentation);
    assert!(!f.root.join(&f.widget.locator).exists());
}
#[test]
fn typed0935_empty_metadata_keeps_legacy_wire_and_read_contract() {
    let mut f = Fixture::new();
    let graphs = f.workspace.project().graphs().to_vec();
    f.workspace.write_recovery(&graphs).unwrap();
    let bytes = f.recovery_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json.get("widgets").is_none());
    assert_eq!(f.workspace.recovery().unwrap(), Some(graphs.clone()));
    assert_eq!(
        f.workspace.recovery_batch().unwrap().unwrap().widgets,
        Vec::<WidgetRecovery>::new()
    );
    f.workspace
        .write_recovery_with_widgets(&graphs, &[])
        .unwrap();
    assert_eq!(f.recovery_bytes(), bytes);
}
#[test]
fn typed0935_legacy_overwrite_and_partial_identity_removal_refuse_before_mutation() {
    let mut f = Fixture::new();
    f.write();
    let bytes = f.recovery_bytes();
    assert!(f.workspace.write_recovery(&f.graphs).is_err());
    assert!(f
        .workspace
        .write_recovery_with_widgets(&f.graphs, &[])
        .is_err());
    assert_eq!(f.recovery_bytes(), bytes);
    let baseline = f.workspace.project().graphs().to_vec();
    f.workspace
        .write_recovery_with_widgets(&baseline, &[])
        .unwrap();
    assert_eq!(f.workspace.recovery().unwrap(), Some(baseline));
}
#[test]
fn typed0935_foreign_duplicate_missing_or_wrong_role_members_refuse_atomically() {
    let mut f = Fixture::new();
    f.write();
    let bytes = f.recovery_bytes();
    let mut bad = f.widget.clone();
    let mut bp = bad.blueprint().unwrap();
    bp.source = "foreign.rvn".into();
    bad.blueprint_json = bp.encode().unwrap();
    assert!(f
        .workspace
        .write_recovery_with_widgets(&f.graphs, &[bad])
        .is_err());
    assert!(f
        .workspace
        .write_recovery_with_widgets(&f.graphs, &[f.widget.clone(), f.widget.clone()])
        .is_err());
    let mut missing = f.graphs.clone();
    missing.retain(|g| !matches!(g.kind, crate::GraphKind::Handler { .. }));
    assert!(f
        .workspace
        .write_recovery_with_widgets(&missing, std::slice::from_ref(&f.widget))
        .is_err());
    let mut wrong = f.graphs.clone();
    let graph = wrong
        .iter_mut()
        .find(|g| matches!(g.kind, crate::GraphKind::Handler { .. }))
        .unwrap();
    graph.kind = crate::GraphKind::Label {
        name: "wrong".into(),
    };
    assert!(f
        .workspace
        .write_recovery_with_widgets(&wrong, std::slice::from_ref(&f.widget))
        .is_err());
    assert_eq!(f.recovery_bytes(), bytes);
}
#[test]
fn typed0935_codec_paths_versions_canonical_form_and_budget_refuse() {
    let f = Fixture::new();
    for locator in [
        "../panel.rvnwidget",
        "C:/panel.rvnwidget",
        "assets//panel.rvnwidget",
        "assets/panel.rvnwidget.",
    ] {
        let mut w = f.widget.clone();
        w.locator = locator.into();
        assert!(w.blueprint().is_err());
    }
    let mut w = f.widget.clone();
    w.blueprint_json.push(' ');
    assert!(w.blueprint().is_err());
    let mut value: serde_json::Value = serde_json::from_str(&f.widget.blueprint_json).unwrap();
    value["version"] = 2.into();
    w.blueprint_json = serde_json::to_string(&value).unwrap();
    assert!(w.blueprint().is_err());
    assert!(widget_recovery::validate(
        &vec![f.widget.clone(); 257],
        &f.graphs,
        f.workspace.project().graphs(),
        "main.rvn"
    )
    .is_err());
    let mut over = f.widget.clone();
    over.blueprint_json = " ".repeat(4 * 1024 * 1024 + 1);
    assert!(widget_recovery::validate(
        &[over],
        &f.graphs,
        f.workspace.project().graphs(),
        "main.rvn"
    )
    .is_err());
}
#[test]
fn typed0935_generic_or_incomplete_first_save_retains_exact_recovery() {
    let mut f = Fixture::new();
    f.write();
    let bytes = f.recovery_bytes();
    let source = fs::read(f.root.join("main.rvn")).unwrap();
    assert!(f.workspace.save(&f.graphs).is_err());
    let files = f.files();
    for partial in [vec![files[0].clone()], vec![files[1].clone()]] {
        assert!(f.workspace.save_with_files(&f.graphs, &partial).is_err());
    }
    let mut incorrect = files.clone();
    incorrect[1].after = Some("{}".into());
    assert!(f.workspace.save_with_files(&f.graphs, &incorrect).is_err());
    assert_eq!(f.recovery_bytes(), bytes);
    assert_eq!(fs::read(f.root.join("main.rvn")).unwrap(), source);
}
#[test]
fn typed0935_first_save_cas_failure_retains_all_models_and_can_retry() {
    let mut f = Fixture::new();
    f.write();
    let bytes = f.recovery_bytes();
    let files = f.files();
    let source = fs::read(f.root.join("main.rvn")).unwrap();
    fs::write(f.root.join("rvn.toml"), "external").unwrap();
    assert!(f.workspace.save_with_files(&f.graphs, &files).is_err());
    assert_eq!(f.recovery_bytes(), bytes);
    assert_eq!(fs::read(f.root.join("main.rvn")).unwrap(), source);
    assert!(!f.root.join(&f.widget.locator).exists());
    fs::write(f.root.join("rvn.toml"), files[0].before.as_ref().unwrap()).unwrap();
    f.workspace.save_with_files(&f.graphs, &files).unwrap();
    assert!(f.workspace.recovery_batch().unwrap().is_none());
    assert!(f.archived(&bytes));
    assert_eq!(
        fs::read_to_string(f.root.join(&f.widget.locator)).unwrap(),
        f.widget.blueprint_json
    );
    assert_eq!(
        SourceWorkspace::open(&f.root)
            .unwrap()
            .unwrap()
            .project()
            .graphs(),
        f.workspace.project().graphs()
    );
}
#[test]
fn typed0935_interrupted_group_every_file_frontier_recovers_uuid_and_archive() {
    for count in 0..=4 {
        let mut f = Fixture::new();
        f.write();
        let bytes = f.recovery_bytes();
        let journal = f.journal();
        f.install_journal(&journal, count);
        let reopened = SourceWorkspace::open(&f.root).unwrap().unwrap();
        assert!(reopened.recovery_batch().unwrap().is_none());
        assert!(f.archived(&bytes));
        assert!(!f.root.join(refactor::REFACTOR_JOURNAL).exists());
        assert_eq!(
            fs::read_to_string(f.root.join(&f.widget.locator)).unwrap(),
            f.widget.blueprint_json
        );
    }
}
#[test]
fn typed0935_archive_done_journal_cleanup_interruption_is_idempotent() {
    let mut f = Fixture::new();
    f.write();
    let journal = f.journal();
    f.install_journal(&journal, 4);
    f.workspace.archive_recovery().unwrap();
    let reopened = SourceWorkspace::open(&f.root).unwrap().unwrap();
    assert!(reopened.recovery_batch().unwrap().is_none());
    assert!(!f.root.join(refactor::REFACTOR_JOURNAL).exists());
}
#[test]
fn typed0935_missing_recovery_before_complete_publication_refuses_without_writes() {
    let mut f = Fixture::new();
    f.write();
    let journal = f.journal();
    f.install_journal(&journal, 1);
    fs::remove_file(f.root.join(RECOVERY)).unwrap();
    let sidecar = fs::read(f.root.join(SIDECAR)).unwrap();
    assert!(SourceWorkspace::open(&f.root).is_err());
    assert_eq!(fs::read(f.root.join(SIDECAR)).unwrap(), sidecar);
    assert!(f.root.join(refactor::REFACTOR_JOURNAL).exists());
    assert!(!f.root.join(&f.widget.locator).exists());
}
#[test]
fn typed0935_foreign_recovery_refuses_interrupted_publication() {
    let mut f = Fixture::new();
    f.write();
    let journal = f.journal();
    f.install_journal(&journal, 0);
    let source = fs::read(f.root.join("main.rvn")).unwrap();
    fs::write(f.root.join(RECOVERY), "another session").unwrap();
    assert!(SourceWorkspace::open(&f.root).is_err());
    assert_eq!(fs::read(f.root.join("main.rvn")).unwrap(), source);
    assert!(f.root.join(refactor::REFACTOR_JOURNAL).exists());
}
#[test]
fn typed0935_corrupt_embedded_recovery_never_archives_or_publishes() {
    let mut f = Fixture::new();
    f.write();
    let bytes = f.recovery_bytes();
    let mut journal = f.journal();
    journal["recovery"] = "{}".into();
    f.install_journal(&journal, 0);
    let source = fs::read(f.root.join("main.rvn")).unwrap();
    assert!(SourceWorkspace::open(&f.root).is_err());
    assert_eq!(f.recovery_bytes(), bytes);
    assert_eq!(fs::read(f.root.join("main.rvn")).unwrap(), source);
    assert!(!f.root.join(&f.widget.locator).exists());
}
