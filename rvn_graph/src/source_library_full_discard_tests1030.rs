//! The actual existing Source journal/backup/writer. CPU definitions only.
use super::*;
use std::cell::Cell;
fn setup1030()->(Fixture,SourceWorkspace,SourceRecoveryCohortLibrary1027){
 let f=Fixture::new();let mut source=f.link();birth(&mut source,A);add(&mut source,A);let original=source.recovery_cohort_libraries1027().unwrap().unwrap();(f,source,original)
}
fn authored1030(f:&Fixture)->Vec<Vec<u8>>{["story.rvn","rvn.toml"].iter().map(|p|fs::read(f.root.join(p)).unwrap()).collect()}
#[test]
fn library_full_discard1030_archives_exact_cache_and_reserves_retired_ids_without_publishing_authored_assets(){
 let(f,mut source,original)=setup1030();let authored=authored1030(&f);let before_sidecar:serde_json::Value=serde_json::from_str(source.expected_sidecar.as_ref().unwrap()).unwrap();let cache=fs::read(f.root.join(RECOVERY)).unwrap();let installed=Cell::new(false);
 source.discard_library_recovery_with1030(&original,||Ok(()),||installed.set(true)).unwrap();assert!(installed.get());assert!(!f.root.join(RECOVERY).exists());assert_eq!(authored1030(&f),authored);assert!(!f.root.join(format!("assets/{A}.rvnlibrary")).exists());
 let mut old=before_sidecar;let mut after:serde_json::Value=serde_json::from_str(source.expected_sidecar.as_ref().unwrap()).unwrap();old.as_object_mut().unwrap().remove("graph_id_high_water");let high=after.as_object_mut().unwrap().remove("graph_id_high_water").unwrap().as_u64().unwrap();assert_eq!(old,after);assert!(high>=original.high_water1027());
 let backups=fs::read_dir(f.root.join(".rvn-backups")).unwrap().map(|e|e.unwrap().path().join("recovery.json")).collect::<Vec<_>>();assert!(backups.iter().any(|path|fs::read(path).ok().as_deref()==Some(cache.as_slice())));
 let cold=SourceWorkspace::open_scoped_libraries1027(&f.root,0).unwrap().unwrap();assert!(cold.recovery_cohort_libraries1027().unwrap().is_none());assert!(cold.graph_id_high_water()>=original.high_water1027());assert!(!f.root.join(refactor::REFACTOR_JOURNAL).exists());
}
#[test]
fn library_full_discard1030_busy_writer_changed_cache_or_preflight_refusal_never_reaches_install_or_changes_source(){
 for mode in 0..3{let(f,mut source,original)=setup1030();let authored=authored1030(&f);let sidecar=fs::read(f.root.join(SIDECAR)).unwrap();let installed=Cell::new(false);
  let hold=if mode==0{let file=fs::OpenOptions::new().read(true).write(true).open(f.root.join(".rvn-authoring.lock")).unwrap();file.try_lock().unwrap();Some(file)}else{None};
  if mode==1{let mut bytes=fs::read(f.root.join(RECOVERY)).unwrap();bytes.push(b'\n');fs::write(f.root.join(RECOVERY),bytes).unwrap();}
  let cache=fs::read(f.root.join(RECOVERY)).unwrap();let result=source.discard_library_recovery_with1030(&original,||if mode==2{Err("actual late model conflict".into())}else{Ok(())},||installed.set(true));assert!(result.is_err());assert!(!installed.get());assert_eq!(authored1030(&f),authored);assert_eq!(fs::read(f.root.join(SIDECAR)).unwrap(),sidecar);assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(),cache);assert!(!f.root.join(refactor::REFACTOR_JOURNAL).exists());drop(hold);
 }
}
// Snapshot every retained backup file without holding directory/file handles.
fn backup_bytes1030(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut result = std::collections::BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink());
            if kind.is_dir() { pending.push(entry.path()); }
            else {
                assert!(kind.is_file());
                result.insert(entry.path().strip_prefix(root).unwrap().to_path_buf(), fs::read(entry.path()).unwrap());
            }
        }
    }
    result
}

// Literal children of this private Fixture only. Restore the original backup
// directory on an assertion panic as well as on the normal cold-recovery path.
struct BlockedBackupNamespace1030 { original: PathBuf, held: PathBuf, restore: bool }
impl BlockedBackupNamespace1030 {
    fn new(root: &Path) -> Self {
        let original = root.join(".rvn-backups");
        let held = root.join("held-backups1030");
        assert!(original.is_dir());
        assert!(!held.exists());
        fs::rename(&original, &held).unwrap();
        let guard = Self { original, held, restore: true };
        fs::write(&guard.original, b"occupied backup namespace").unwrap();
        guard
    }
    fn restore(&mut self) {
        fs::remove_file(&self.original).unwrap();
        fs::rename(&self.held, &self.original).unwrap();
        self.restore = false;
    }
}
impl Drop for BlockedBackupNamespace1030 {
    fn drop(&mut self) {
        if self.restore && self.held.is_dir() {
            if self.original.is_file() { let _ = fs::remove_file(&self.original); }
            if !self.original.exists() { let _ = fs::rename(&self.held, &self.original); }
        }
    }
}

#[test]
fn library_full_discard1030_archive_setup_failure_retains_journal_and_all_original_bytes_for_real_recovery() {
    let (f, mut source, original) = setup1030();
    let before = f.saved();
    let cache = fs::read(f.root.join(RECOVERY)).unwrap();
    let backups_before = backup_bytes1030(&f.root.join(".rvn-backups"));
    let expected_sidecar = source.expected_sidecar.clone();
    let expected_recovery = source.expected_recovery.clone();
    let graphs_before = source.project().graphs().to_vec();
    let high_before = source.graph_id_high_water();
    let mut blocked = BlockedBackupNamespace1030::new(&f.root);
    assert_eq!(backup_bytes1030(&blocked.held), backups_before);
    let preflight = Cell::new(false);
    let installed = Cell::new(false);
    let result = source.discard_library_recovery_with1030(&original, || { preflight.set(true); Ok(()) }, || installed.set(true));
    assert!(result.is_err());
    assert!(preflight.get(), "fixture must reach the actual discard action");
    assert!(!installed.get());
    assert_eq!(f.saved(), before);
    assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(), cache);
    assert_eq!(source.expected_sidecar, expected_sidecar);
    assert_eq!(source.expected_recovery, expected_recovery);
    assert_eq!(source.project().graphs(), graphs_before);
    assert_eq!(source.graph_id_high_water(), high_before);
    assert_eq!(fs::read(&blocked.original).unwrap(), b"occupied backup namespace");
    assert_eq!(backup_bytes1030(&blocked.held), backups_before);
    let journal_path = f.root.join(refactor::REFACTOR_JOURNAL);
    let journal = fs::read(&journal_path).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&journal).unwrap();
    assert_eq!(value["recovery"]["kind"], "source_library_recovery_transition_v5");
    assert_eq!(value["recovery"]["version"], 5);
    assert_eq!(value["recovery"]["mode"], "discard_all");
    assert_eq!(value["recovery"]["before_recovery"].as_str(), Some(std::str::from_utf8(&cache).unwrap()));
    assert!(value["recovery"].as_object().unwrap().contains_key("after_recovery"));
    assert!(value["recovery"]["after_recovery"].is_null());
    // The retained real journal also prevents a second, conflicting operation.
    assert!(source.discard_library_recovery_with1030(&original,
        || panic!("pending journal must refuse before preflight"),
        || panic!("pending journal must refuse before install")).is_err());
    assert_eq!(fs::read(&journal_path).unwrap(), journal);
    assert_eq!(f.saved(), before);
    assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(), cache);
    blocked.restore();
    assert_eq!(backup_bytes1030(&blocked.original), backups_before);
    let cold = SourceWorkspace::open_scoped_libraries1027(&f.root, 0).unwrap().unwrap();
    assert!(cold.recovery_cohort_libraries1027().unwrap().is_none());
    assert!(!journal_path.exists());
    assert!(!f.root.join(RECOVERY).exists());
    assert_eq!(authored1030(&f), before[..2]);
    assert!(cold.graph_id_high_water() >= original.high_water1027());
    let backups_after = backup_bytes1030(&blocked.original);
    for (path, bytes) in backups_before { assert_eq!(backups_after.get(&path), Some(&bytes)); }
    assert!(backups_after.values().any(|bytes| bytes == &cache));
}

#[test]
fn library_full_discard1030_unknown_recovery_version_is_rejected_without_a_legacy_flatten_or_archive() {
    // Causal control: the existing explicit v1 recovery remains admitted through
    // the real writer/readers, while an absent version has never been legacy v1.
    let legacy_fixture = Fixture::new();
    let mut legacy = legacy_fixture.link();
    let mut graphs = legacy.project().graphs().to_vec();
    graphs[0].add_node(crate::NodeKind::Dialogue, [123.0, 456.0]);
    legacy.write_recovery(&graphs).unwrap();
    let legacy_bytes = fs::read(legacy_fixture.root.join(RECOVERY)).unwrap();
    let mut legacy_value: serde_json::Value = serde_json::from_slice(&legacy_bytes).unwrap();
    assert_eq!(legacy_value["version"], 1);
    assert_eq!(legacy.recovery_batch().unwrap().unwrap().graphs, graphs);
    legacy_value.as_object_mut().unwrap().remove("version");
    let absent_bytes = serde_json::to_vec(&legacy_value).unwrap();
    fs::write(legacy_fixture.root.join(RECOVERY), &absent_bytes).unwrap();
    let absent = SourceWorkspace::open_scoped_libraries1027(&legacy_fixture.root, 0).unwrap().unwrap();
    let error = absent.recovery_cohort_libraries1027().unwrap_err();
    assert!(error.contains("Unreadable") && error.contains("version"), "{error}");
    assert_eq!(fs::read(legacy_fixture.root.join(RECOVERY)).unwrap(), absent_bytes);
    assert!(!legacy_fixture.root.join(refactor::REFACTOR_JOURNAL).exists());

    let (f, _source, original) = setup1030();
    let original_cache = fs::read(f.root.join(RECOVERY)).unwrap();
    let before = f.saved();
    let backups_before = backup_bytes1030(&f.root.join(".rvn-backups"));
    for version in [999, 0, 6] {
        let mut value: serde_json::Value = serde_json::from_slice(&original_cache).unwrap();
        value["version"] = serde_json::json!(version);
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(f.root.join(RECOVERY), &bytes).unwrap();
        let mut cold = SourceWorkspace::open_scoped_libraries1027(&f.root, 0).unwrap().unwrap();
        let error = cold.recovery_cohort_libraries1027().unwrap_err();
        assert!(error.contains("Unsupported") && error.contains(&version.to_string()), "{error}");
        let legacy_error = cold.recovery_batch().unwrap_err();
        assert!(legacy_error.contains("Unsupported"), "{legacy_error}");
        let archive_error = cold.archive_recovery().unwrap_err();
        assert!(archive_error.contains("Unsupported"), "{archive_error}");
        let discard_error = cold.discard_library_recovery_with1030(&original,
            || panic!("unknown version must refuse before preflight"),
            || panic!("unknown version must refuse before install")).unwrap_err();
        assert!(discard_error.contains("Unsupported"), "{discard_error}");
        assert_eq!(f.saved(), before);
        assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(), bytes);
        assert_eq!(backup_bytes1030(&f.root.join(".rvn-backups")), backups_before);
        assert!(!f.root.join(refactor::REFACTOR_JOURNAL).exists());
    }
}
