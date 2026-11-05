use super::*;
use rvn_parser::enum_asset::{EnumAsset, EnumAssetId, EnumCommand};
const UUID: &str = "17411a3e431e4c9388b78a2b9825b130";
struct Fixture { root: PathBuf, source: SourceWorkspace, before: String, after: String }
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("rvn-enum-metadata1025-{}", unique_name()));
        fs::create_dir(&root).unwrap(); fs::create_dir(root.join("assets")).unwrap();
        fs::write(root.join("story.rvn"), "label start\n\"Hello\"\n").unwrap();
        let value = EnumAsset::new(EnumAssetId::parse(UUID).unwrap(), "Alpha".into()).unwrap();
        let before = value.encode().unwrap();
        let after = value.prepare(EnumCommand::DescribeAsset("Saved metadata only".into())).unwrap().after().encode().unwrap();
        fs::write(root.join("assets/Alpha.rvnenum"), &before).unwrap();
        fs::write(root.join("rvn.toml"), format!("[enum_assets]\n{UUID}='Alpha.rvnenum'\n")).unwrap();
        let mut source = SourceWorkspace::link(&root, &root.join("story.rvn"), &[]).unwrap();
        let mut dirty = source.project().graphs().to_vec();
        dirty[0].nodes.values_mut().next().unwrap().title_override = Some("Unrelated unsaved graph".into());
        source.write_recovery(&dirty).unwrap();
        Self { root, source, before, after }
    }
    fn files(&self) -> Vec<SourceRefactorFile> {
        let manifest = fs::read_to_string(self.root.join("rvn.toml")).unwrap();
        vec![SourceRefactorFile { path: "rvn.toml".into(), before: Some(manifest.clone()), after: Some(manifest) },
            SourceRefactorFile { path: "assets/Alpha.rvnenum".into(), before: Some(self.before.clone()), after: Some(self.after.clone()) }]
    }
    fn observers(&self) -> Vec<Vec<u8>> { ["story.rvn", SIDECAR, RECOVERY].iter().map(|path| fs::read(self.root.join(path)).unwrap()).collect() }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(self.root.starts_with(std::env::temp_dir()));
        assert!(self.root.file_name().unwrap().to_string_lossy().starts_with("rvn-enum-metadata1025-"));
        fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn source_metadata1025_real_enum_save_preserves_exact_source_sidecar_recovery_and_graphs() {
    let mut f = Fixture::new(); let observers = f.observers(); let baseline = f.source.project().graphs().to_vec();
    let recovery = f.source.recovery_batch().unwrap(); let expected_recovery = f.source.expected_recovery.clone();
    f.source.save_metadata_files_preserving_authoring1025(&baseline, &f.files()).unwrap();
    assert_eq!(f.observers(), observers); assert_eq!(f.source.project().graphs(), baseline);
    assert_eq!(f.source.recovery_batch().unwrap(), recovery); assert_eq!(f.source.expected_recovery, expected_recovery);
    assert_eq!(fs::read_to_string(f.root.join("assets/Alpha.rvnenum")).unwrap(), f.after);
    assert!(!f.root.join(refactor::REFACTOR_JOURNAL).exists());
    assert!(!fs::read_dir(f.root.join(".rvn-backups")).unwrap().any(|entry| { let path = entry.unwrap().path(); path.join(RECOVERY).exists() || path.join("recovery.json").exists() }));
    let reopened = SourceWorkspace::open(&f.root).unwrap().unwrap();
    assert_eq!(reopened.recovery_batch().unwrap(), recovery);
}
#[test]
fn source_metadata1025_external_enum_conflict_never_archives_recovery_or_overwrites_files() {
    let mut f = Fixture::new(); let observers = f.observers(); let files = f.files();
    fs::write(f.root.join("assets/Alpha.rvnenum"), "external edit").unwrap();
    let baseline = f.source.project().graphs().to_vec();
    assert!(f.source.save_metadata_files_preserving_authoring1025(&baseline, &files).is_err());
    assert_eq!(f.observers(), observers); assert_eq!(fs::read(f.root.join("assets/Alpha.rvnenum")).unwrap(), b"external edit");
    assert!(!f.root.join(refactor::REFACTOR_JOURNAL).exists());
}
#[test]
fn source_metadata1025_rejects_submitted_graph_changes_and_reserved_recovery_targets() {
    let mut f = Fixture::new(); let observers = f.observers(); let mut changed = f.source.project().graphs().to_vec();
    changed[0].nodes.values_mut().next().unwrap().title_override = Some("Not metadata".into());
    assert!(f.source.save_metadata_files_preserving_authoring1025(&changed, &f.files()).is_err());
    let baseline = f.source.project().graphs().to_vec();
    let mut files = f.files(); files.push(SourceRefactorFile { path: RECOVERY.into(), before: f.source.expected_recovery.clone(), after: None });
    assert!(f.source.save_metadata_files_preserving_authoring1025(&baseline, &files).is_err());
    assert_eq!(f.observers(), observers); assert_eq!(fs::read_to_string(f.root.join("assets/Alpha.rvnenum")).unwrap(), f.before);
}
#[test]
fn source_metadata1025_external_recovery_owner_is_not_rebaselined_or_archived() {
    let mut f = Fixture::new(); let expected = f.source.expected_recovery.clone(); let baseline = f.source.project().graphs().to_vec();
    let files = f.files(); let mut other = f.source.clone(); let mut changed = baseline.clone();
    changed[0].nodes.values_mut().next().unwrap().title_override = Some("New external recovery".into());
    other.write_recovery(&changed).unwrap(); let external = fs::read(f.root.join(RECOVERY)).unwrap();
    assert!(f.source.save_metadata_files_preserving_authoring1025(&baseline, &files).is_err());
    assert_eq!(fs::read(f.root.join(RECOVERY)).unwrap(), external); assert_eq!(f.source.expected_recovery, expected);
    assert_eq!(fs::read_to_string(f.root.join("assets/Alpha.rvnenum")).unwrap(), f.before);
}
