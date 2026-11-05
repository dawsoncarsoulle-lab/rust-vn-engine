//! Real Source lock/journal/CAS/cold-open paths. Definitions only in this
//! candidate; executing these tests supplies CPU evidence, never native parity.
use super::*;
use crate::source_function_library_recovery1023::{FunctionLibraryRecovery as Record, FunctionLibraryWire, LibraryRecoveryState};
use rvn_parser::function_library_asset1023::{FunctionLibraryAsset, FunctionLibraryId, FunctionLibraryMember, LibraryFunctionAccess};
use std::collections::BTreeMap;
const A: &str = "11111111111141118111111111111111";
const B: &str = "22222222222242228222222222222222";
fn id(value: &str) -> FunctionLibraryId { FunctionLibraryId::parse(value).unwrap() }
fn asset(owner: &str, members: &[u64]) -> FunctionLibraryAsset {
    FunctionLibraryAsset::new(id(owner), members.iter().map(|graph_id| FunctionLibraryMember { graph_id: *graph_id, access: LibraryFunctionAccess::Public, pure: false }).collect()).unwrap()
}
fn record(owner: &str, members: &[u64]) -> Record {
    Record::new(id(owner), None, Some(LibraryRecoveryState::new(format!("assets/{owner}.rvnlibrary"), FunctionLibraryWire::from_asset(&asset(owner, members)).unwrap()).unwrap())).unwrap()
}
struct Fixture { root: PathBuf }
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("rvn-complete-library1031-{}", unique_name()));
        fs::create_dir(&root).unwrap(); fs::create_dir(root.join("assets")).unwrap();
        fs::write(root.join("rvn.toml"), "[paths]\nassets=\"assets\"\n[project]\ntitle=\"Retain this exact unrelated setting\"\n").unwrap();
        fs::write(root.join("story.rvn"), "// genuine Saved baseline\nscene label start\n\"Before\"\n").unwrap();
        Self { root }
    }
    fn link(&self) -> SourceWorkspace { SourceWorkspace::link_scoped_libraries1027(&self.root, &self.root.join("story.rvn"), &[]).unwrap() }
    fn bytes(&self) -> BTreeMap<String, Vec<u8>> {
        fn visit(root: &Path, path: &Path, result: &mut BTreeMap<String, Vec<u8>>) {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap(); let path = entry.path();
                if entry.file_type().unwrap().is_dir() { visit(root, &path, result); }
                else { result.insert(path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"), fs::read(path).unwrap()); }
            }
        }
        let mut result = BTreeMap::new(); visit(&self.root, &self.root, &mut result); result
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(self.root.starts_with(std::env::temp_dir()));
        assert!(self.root.file_name().unwrap().to_string_lossy().starts_with("rvn-complete-library1031-"));
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn birth(workspace: &mut SourceWorkspace, owner: &str) {
    let expected = workspace.recovery_cohort_libraries1027().unwrap();
    let graphs = expected.as_ref().map(|c| c.batch.graphs.clone()).unwrap_or_else(|| workspace.project.graphs().to_vec());
    workspace.replace_recovery_with_library_changes1027(expected.as_ref(), &graphs, &[], &[], &[], &[LibraryRecoveryChange1027::Upsert { before: None, after: record(owner, &[]) }], &[], &[], &[], || ()).unwrap();
}
fn add(workspace: &mut SourceWorkspace, owner: &str, name: &str) -> GraphId {
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    let graph_id = workspace.next_library_member_graph_id1027(Some(&expected)).unwrap();
    let identity = workspace.prepare_library_member_identity1027(&id(owner), graph_id, Some(&expected)).unwrap();
    let mut graphs = expected.batch.graphs.clone(); graphs.push(crate::prepare_initial_library_member1027(id(owner), graph_id, name).unwrap());
    let before = expected.libraries.iter().find(|r| r.id() == &id(owner)).unwrap().clone();
    let mut members = before.after().unwrap().wire().asset().unwrap().members().iter().map(|m| m.graph_id).collect::<Vec<_>>(); members.push(graph_id.get());
    workspace.replace_recovery_with_library_changes1027(Some(&expected), &graphs, &[], &[], &[], &[LibraryRecoveryChange1027::Upsert { before: Some(before), after: record(owner, &members) }], &[graph_id], &[identity], &[], || ()).unwrap();
    graph_id
}
fn dirty_main(workspace: &mut SourceWorkspace) -> GraphId {
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    let mut graphs = expected.batch.graphs.clone();
    let graph = graphs.iter_mut().find(|g| matches!(g.kind, crate::GraphKind::Label { .. })).unwrap();
    // This is Legacy Dialogue/TextSegment::Lit, not FunctionReturn Expr::Str:
    // import.rs dialogue_text reconstructs the actual raw TextValue producer.
    let nodes = graph.nodes.values().filter(|n| n.kind == crate::NodeKind::TextValue && n.properties.get("value") == Some(&crate::PropertyValue::String("Before".into()))).map(|n| n.id).collect::<Vec<_>>();
    assert_eq!(nodes.len(), 1, "the original Dialogue must own one raw Before producer");
    let node = nodes[0]; let output = graph.pin_by_key(node, "value").unwrap();
    assert_eq!(output.value_type, crate::ValueType::InterpolatedText);
    assert!(output.text_localization.is_none() && output.text_table_reference.is_none() && output.default_value.is_none());
    let mut expected_graph = graph.clone();
    expected_graph.nodes.get_mut(&node).unwrap().properties.insert("value".into(), crate::PropertyValue::String("Current ordinary text".into()));
    expected_graph.nodes.get_mut(&node).unwrap().position = [319., 127.];
    graph.set_property(node, "value", crate::PropertyValue::String("Current ordinary text".into())).unwrap();
    graph.nodes.get_mut(&node).unwrap().position = [319., 127.];
    assert_eq!(*graph, expected_graph, "the actual typed raw Dialogue edit must retain every other graph/pin/Source identity");
    let graph_id = graph.graph_id;
    workspace.replace_recovery_with_library_changes1027(Some(&expected), &graphs, &[], &[], &[], &[], &[], &[], &[], || ()).unwrap();
    graph_id
}
fn candidate(workspace: &SourceWorkspace) -> (String, Vec<GraphDocument>, Vec<SourceRefactorFile>) {
    let current = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    let prepared = workspace.prepare_library_recovery_current1027().unwrap();
    let source = prepared.source().to_owned();
    let parsed = rvn_parser::parse(&source).unwrap();
    let resources = crate::LibraryGraphResources1027::prepare(prepared.scopes(), prepared.assets()).unwrap();
    let reconstructed = crate::import_library_program1027(&parsed, &resources, &current.batch.graphs).unwrap();
    assert!(same_images(&reconstructed, &current.batch.graphs), "strict Current emission must retain every actual full graph image");
    let before = fs::read_to_string(workspace.root.join("rvn.toml")).unwrap();
    let mut manifest: toml::Value = before.parse().unwrap();
    let assets = crate::source_function_library_recovery1023::validate_set(&library_recovery1027::saved_baselines_for_project1027(&workspace.project).unwrap(), &current.libraries).unwrap();
    let mut index = toml::map::Map::new();
    for row in assets {
        let locator = row.state().locator().strip_prefix("assets/").unwrap();
        index.insert(row.id().as_str().into(), toml::Value::String(locator.into()));
    }
    manifest.as_table_mut().unwrap().insert("function_libraries".into(), toml::Value::Table(index));
    let mut files = crate::source_function_library_recovery1023::publication_files(&current.libraries).unwrap();
    files.push(SourceRefactorFile { path: "rvn.toml".into(), before: Some(before), after: Some(toml::to_string_pretty(&manifest).unwrap()) });
    // SourceProject R2 owns presentation order. Its real read-only constructor
    // rebuilds all values from Source/resources, using actual Current templates
    // only for identities/presentation. No reordered clone is declared Saved.
    let strict = library_complete_save1031::prepare_candidate1031(workspace, &current, &source, &current.batch.graphs, &files, current.batch.asset_placements.clone()).unwrap();
    let graphs = strict.next.graphs().to_vec(); assert_eq!(graphs, current.batch.graphs);
    (source, graphs, files)
}
fn same_images(left: &[GraphDocument], right: &[GraphDocument]) -> bool {
    let a = left.iter().map(|g| (g.graph_id, g)).collect::<BTreeMap<_, _>>();
    let b = right.iter().map(|g| (g.graph_id, g)).collect::<BTreeMap<_, _>>();
    a.len() == left.len() && b.len() == right.len() && a == b
}
fn publish(workspace: &mut SourceWorkspace) -> SourceLibraryCompletePublication1031 {
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    let (source, graphs, files) = candidate(workspace);
    workspace.save_complete_library_recovery_receipt1031(&expected, &source, &graphs, &files, || Ok(()), || ()).unwrap().1
}

#[test]
fn library_complete1031_one_real_writer_saves_two_libraries_and_ordinary_current_then_cold_opens_exact_images() {
    let f = Fixture::new(); let mut workspace = f.link();
    birth(&mut workspace, A); let a1 = add(&mut workspace, A, "FirstA");
    birth(&mut workspace, B); let b1 = add(&mut workspace, B, "FirstB"); let a2 = add(&mut workspace, A, "SecondA");
    assert!(a1 < b1 && b1 < a2);
    let main = dirty_main(&mut workspace);
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap(); let before_saved = workspace.project.graphs().to_vec();
    let (source, graphs, files) = candidate(&workspace);
    assert!(source.contains("Current ordinary text"));
    let mut installed = 0;
    let (result, receipt) = workspace.save_complete_library_recovery_receipt1031(&expected, &source, &graphs, &files, || Ok(()), || { installed += 1; 73 }).unwrap();
    assert_eq!(result, 73); assert_eq!(installed, 1);
    assert_eq!(receipt.before_current1031(), &expected); assert_eq!(receipt.before_saved1031(), before_saved);
    assert_eq!(receipt.saved_libraries1031(), &[id(A), id(B)]);
    assert_eq!(receipt.after_saved1031(), graphs); assert!(receipt.after1031().cohort1029().is_none());
    assert_eq!(expected.batch.graphs, workspace.project.graphs());
    assert_ne!(workspace.project.graphs().iter().find(|g| g.graph_id == main), before_saved.iter().find(|g| g.graph_id == main));
    assert!(f.root.join(format!("assets/{A}.rvnlibrary")).exists()); assert!(f.root.join(format!("assets/{B}.rvnlibrary")).exists());
    assert!(workspace.recovery_cohort_libraries1027().unwrap().is_none()); assert!(!f.root.join(RECOVERY).exists()); assert!(!f.root.join(REFACTOR_JOURNAL).exists());
    let cold = SourceWorkspace::open_scoped_libraries1027(&f.root, 0).unwrap().unwrap();
    assert_eq!(cold.project.graphs(), workspace.project.graphs()); assert_eq!(cold.project.source(), source);
    assert!(cold.recovery_cohort_libraries1027().unwrap().is_none());
    assert_eq!(library_recovery1027::saved_baselines_for_project1027(&cold.project).unwrap().len(), 2);
}

#[test]
fn library_complete1031_dirty_empty_descriptor_and_full_current_placement_are_saved_and_high_water_never_rewinds() {
    let f = Fixture::new(); let mut workspace = f.link();
    // Use the real placement authoring contract before Library birth. Its
    // legacy recovery_batch guard must continue refusing typed Library edits.
    let graphs = workspace.project.graphs().to_vec(); let main = graphs[0].graph_id;
    let placements = SourceAssetPlacements::from_entries([SourceAssetPlacement { graph_id: main, namespace: SourcePlacementNamespace::SourceGraphs, directory: "CurrentFolder".into() }]).unwrap();
    workspace.commit_asset_placement_edit(&None, &graphs, placements.clone(), &[], &[], &[], &[], || ()).unwrap();
    birth(&mut workspace, A); birth(&mut workspace, B);
    let actual = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    let reserved = workspace.next_library_member_graph_id1027(Some(&actual)).unwrap();
    // Reserve through the real authoring path; the unused identity stays a
    // monotonic reservation, never a fictitious member or descriptor row.
    workspace.commit_member_graph_id(reserved).unwrap();
    // The reserve must use the real typed writer with its original Library
    // and placement cohort. The historical graph-only entry correctly refuses.
    assert!(workspace.write_recovery(&actual.batch.graphs).unwrap_err().contains("graph-only overwrite refused"));
    workspace.replace_recovery_with_library_changes1027(Some(&actual), &actual.batch.graphs,
        &actual.batch.widgets, &actual.batch.widget_memberships, &[], &[], &[], &[], &[], || ()).unwrap();
    let high = workspace.graph_id_high_water(); let receipt = publish(&mut workspace);
    assert_eq!(receipt.after1031().placements1029(), Some(&placements)); assert!(receipt.after1031().high_water1029() >= high);
    for owner in [A, B] { let descriptor = FunctionLibraryAsset::decode(&fs::read(f.root.join(format!("assets/{owner}.rvnlibrary"))).unwrap()).unwrap(); assert!(descriptor.members().is_empty()); }
    let cold = SourceWorkspace::open_scoped_libraries1027(&f.root, 0).unwrap().unwrap();
    assert_eq!(cold.published_asset_placements(), Some(&placements)); assert!(cold.graph_id_high_water() >= high); assert!(cold.recovery_cohort_libraries1027().unwrap().is_none());
}

#[test]
fn library_complete1031_body_only_saved_library_is_in_full_uuid_inventory_without_invented_records() {
    let f = Fixture::new(); let mut workspace = f.link(); birth(&mut workspace, A); let member = add(&mut workspace, A, "BodyOnly"); publish(&mut workspace);
    let mut graphs = workspace.project.graphs().to_vec(); graphs.iter_mut().find(|g| g.graph_id == member).unwrap().nodes.values_mut().next().unwrap().position = [137., 241.];
    workspace.write_recovery(&graphs).unwrap(); let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap(); assert!(expected.libraries.is_empty());
    assert!(workspace.recovery_batch().unwrap_err().contains("legacy flatten refused"));
    let receipt = publish(&mut workspace); assert_eq!(receipt.saved_libraries1031(), &[id(A)]); assert!(receipt.before_current1031().libraries.is_empty());
    assert!(same_images(&graphs, workspace.project.graphs())); assert!(workspace.recovery_cohort_libraries1027().unwrap().is_none());
}

#[test]
fn library_complete1031_saved_library_deletion_is_exact_tombstone_not_a_dropped_owner() {
    let f = Fixture::new(); let mut workspace = f.link(); birth(&mut workspace, A); let member = add(&mut workspace, A, "Retired"); publish(&mut workspace);
    let saved = library_recovery1027::saved_baselines_for_project1027(&workspace.project).unwrap().remove(0);
    let graphs = workspace.project.graphs().iter().filter(|g| g.graph_id != member).cloned().collect::<Vec<_>>();
    let deletion = Record::new(id(A), Some(saved.state().clone()), None).unwrap();
    workspace.replace_recovery_with_library_changes1027(None, &graphs, &[], &[], &[], &[LibraryRecoveryChange1027::Upsert { before: None, after: deletion }], &[], &[], &[], || ()).unwrap();
    let receipt = publish(&mut workspace); assert_eq!(receipt.saved_libraries1031(), &[id(A)]);
    assert!(!f.root.join(format!("assets/{A}.rvnlibrary")).exists()); assert!(library_recovery1027::saved_baselines_for_project1027(&workspace.project).unwrap().is_empty());
    assert!(same_images(&graphs, workspace.project.graphs())); assert!(workspace.recovery_cohort_libraries1027().unwrap().is_none());
}

#[test]
fn library_complete1031_prepublication_failure_is_zero_target_io_and_does_not_install() {
    let f = Fixture::new(); let mut workspace = f.link(); birth(&mut workspace, A); add(&mut workspace, A, "Unsaved"); birth(&mut workspace, B); dirty_main(&mut workspace);
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap(); let (source, graphs, files) = candidate(&workspace); let before = f.bytes();
    let result = workspace.save_complete_library_recovery_receipt1031(&expected, &source, &graphs, &files, || Err("injected before publisher".into()), || panic!("failed preflight installed models"));
    assert!(result.err().unwrap().contains("injected before publisher")); assert_eq!(f.bytes(), before);
    assert_eq!(workspace.recovery_cohort_libraries1027().unwrap(), Some(expected));
}

#[test]
fn library_complete1031_preflight_interleaving_rechecks_original_readset_before_any_publication() {
    for change_recovery in [false, true] {
        let f = Fixture::new(); let mut workspace = f.link(); birth(&mut workspace, A); add(&mut workspace, A, "Interleaving"); birth(&mut workspace, B);
        let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap(); let (source, graphs, files) = candidate(&workspace);
        let before = f.bytes(); let target = if change_recovery { RECOVERY } else { "rvn.toml" };
        let mut foreign = fs::read(f.root.join(target)).unwrap(); foreign.extend_from_slice(b" \n");
        let error = workspace.save_complete_library_recovery_receipt1031(&expected, &source, &graphs, &files, || { fs::write(f.root.join(target), &foreign).unwrap(); Ok(()) }, || panic!("stale readset installed")).err().unwrap();
        assert!(!error.is_empty()); let mut wanted = before; wanted.insert(target.into(), foreign); assert_eq!(f.bytes(), wanted);
    }
}

#[test]
fn library_complete1031_stale_cohort_missing_or_duplicate_graph_and_foreign_manifest_all_refuse_before_io() {
    let f = Fixture::new(); let mut workspace = f.link(); birth(&mut workspace, A); add(&mut workspace, A, "Frozen");
    let stale = workspace.recovery_cohort_libraries1027().unwrap().unwrap(); birth(&mut workspace, B); dirty_main(&mut workspace);
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap(); let (source, graphs, files) = candidate(&workspace); let before = f.bytes();
    assert!(workspace.save_complete_library_recovery_receipt1031(&stale, &source, &graphs, &files, || Ok(()), || panic!("stale installed")).is_err()); assert_eq!(f.bytes(), before);
    let mut sparse = graphs.clone(); sparse.pop();
    let mut duplicate = graphs.clone(); duplicate.push(graphs[0].clone());
    for invalid in [sparse, duplicate] {
        assert!(workspace.save_complete_library_recovery_receipt1031(&expected, &source, &invalid, &files, || Ok(()), || panic!("invalid images installed")).is_err()); assert_eq!(f.bytes(), before);
    }
    let mut reordered = graphs.clone(); reordered.rotate_left(1); assert_ne!(reordered, graphs); assert!(same_images(&reordered, &graphs));
    let error = workspace.save_complete_library_recovery_receipt1031(&expected, &source, &reordered, &files, || Ok(()), || panic!("different Current order installed")).err().unwrap();
    assert!(error.contains("exact original editor order"), "{error}"); assert_eq!(f.bytes(), before);
    // A coherent Source/import roundtrip is still insufficient: substituting
    // the ordinary Saved body would silently leave part of Current unsaved.
    let mixed_source = source.replace("Current ordinary text", "Before");
    let prepared = workspace.prepare_library_recovery_current1027().unwrap();
    let resources = crate::LibraryGraphResources1027::prepare(prepared.scopes(), prepared.assets()).unwrap();
    let mixed = crate::import_library_program1027(&rvn_parser::parse(&mixed_source).unwrap(), &resources, &expected.batch.graphs).unwrap();
    let error = workspace.save_complete_library_recovery_receipt1031(&expected, &mixed_source, &mixed, &files, || Ok(()), || panic!("mixed Current/Saved installed")).err().unwrap();
    assert!(error.contains("every complete Current graph image"), "{error}"); assert_eq!(f.bytes(), before);
    let mut foreign = files.clone(); foreign.iter_mut().find(|f| f.path == "rvn.toml").unwrap().after.as_mut().unwrap().push_str("\n[unjoined]\nsetting=\"foreign\"\n");
    let error = workspace.save_complete_library_recovery_receipt1031(&expected, &source, &graphs, &foreign, || Ok(()), || panic!("foreign manifest installed")).err().unwrap();
    assert!(error.contains("non-Library manifest"), "{error}"); assert_eq!(f.bytes(), before);
    let mut omitted = files.clone(); omitted.retain(|f| f.path != format!("assets/{B}.rvnlibrary"));
    assert!(workspace.save_complete_library_recovery_receipt1031(&expected, &source, &graphs, &omitted, || Ok(()), || panic!("missing empty descriptor installed")).is_err()); assert_eq!(f.bytes(), before);
    let mut table = files.clone(); table.push(SourceRefactorFile { path: "assets/unjoined.csv".into(), before: None, after: Some("Key,SourceString\nx,y\n".into()) });
    assert!(workspace.save_complete_library_recovery_receipt1031(&expected, &source, &graphs, &table, || Ok(()), || panic!("unjoined table installed")).is_err()); assert_eq!(f.bytes(), before);
}

#[test]
fn library_complete1031_busy_original_source_lock_refuses_without_io_or_install() {
    let f = Fixture::new(); let mut workspace = f.link(); birth(&mut workspace, A); let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap(); let (source, graphs, files) = candidate(&workspace); let before = f.bytes();
    let held = lock(&f.root).unwrap();
    assert!(workspace.save_complete_library_recovery_receipt1031(&expected, &source, &graphs, &files, || Ok(()), || panic!("busy installed")).is_err());
    // Windows blocks separate readers of the lock file while it is held.
    // The save call above still ran while the original lock was owned.
    drop(held);
    assert_eq!(f.bytes(), before);
}

#[test]
fn library_complete1031_unjoined_enum_descriptor_or_index_is_not_silently_acknowledged() {
    let f = Fixture::new(); let mut workspace = f.link(); birth(&mut workspace, A);
    let enumeration = rvn_parser::enum_recovery1025::EnumRecoveryRecord1025::unpublished(
        "assets/State.rvnenum".into(),
        rvn_parser::enum_asset::EnumAsset::new(rvn_parser::enum_asset::EnumAssetId::parse("99999999999949998999999999999999").unwrap(), "State".into()).unwrap(),
    ).unwrap();
    workspace.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Upsert { before: None, after: enumeration.clone() }], &[SourceRefactorFile { path: enumeration.locator().into(), before: None, after: None }], || ()).unwrap();
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap(); let (source, graphs, files) = candidate(&workspace); let before = f.bytes();
    assert_eq!(expected.batch.enums, vec![enumeration.clone()]);
    assert!(workspace.save_complete_library_recovery_receipt1031(&expected, &source, &graphs, &files, || Ok(()), || panic!("missing Enum descriptor installed")).is_err()); assert_eq!(f.bytes(), before);
    let mut joined = files.clone(); joined.push(SourceRefactorFile { path: enumeration.locator().into(), before: None, after: Some(enumeration.current().encode().unwrap()) });
    joined.iter_mut().find(|f| f.path == "rvn.toml").unwrap().after.as_mut().unwrap().push_str("\n[enum_assets]\n99999999999949998999999999999999='State.rvnenum'\n");
    let error = workspace.save_complete_library_recovery_receipt1031(&expected, &source, &graphs, &joined, || Ok(()), || panic!("unjoined Enum index installed")).err().unwrap();
    assert!(error.contains("non-Library manifest"), "{error}"); assert_eq!(f.bytes(), before);
    assert_eq!(workspace.recovery_cohort_libraries1027().unwrap(), Some(expected));
}

#[test]
fn library_complete1031_v5_writer_does_not_become_generic_ordinary_only_admission() {
    let f = Fixture::new(); let mut workspace = f.link();
    let mut graphs = workspace.project.graphs().to_vec(); graphs[0].nodes.values_mut().next().unwrap().position = [193., 211.];
    workspace.write_recovery(&graphs).unwrap();
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap(); let (source, published, files) = candidate(&workspace); let before = f.bytes();
    let error = workspace.save_complete_library_recovery_receipt1031(&expected, &source, &published, &files, || Ok(()), || panic!("ordinary-only v5 installed")).err().unwrap();
    assert!(error.contains("generic v5 admission refused"), "{error}"); assert_eq!(f.bytes(), before);
}

#[test]
fn library_complete1031_unjoined_readonly_import_owner_mapping_is_not_flattened_or_acknowledged() {
    let f = Fixture::new();
    fs::write(f.root.join("shared.rvn"), "label imported\n\"Read only\"\n").unwrap();
    fs::write(f.root.join("story.rvn"), "use \"shared.rvn\"\nscene label start\n\"Before\"\n").unwrap();
    let mut workspace = f.link(); birth(&mut workspace, A);
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    let before = f.bytes();
    let source = workspace.project.source().to_owned();
    assert!(workspace.prepare_library_recovery_current1027().err().unwrap().contains("imported owner mapping"));
    let error = workspace.save_complete_library_recovery_receipt1031(&expected, &source, &expected.batch.graphs, &[], || Ok(()), || panic!("unjoined imported ownership installed")).err().unwrap();
    assert!(error.contains("imported owner mapping"), "{error}"); assert_eq!(f.bytes(), before);
    assert_eq!(workspace.recovery_cohort_libraries1027().unwrap(), Some(expected));
}

fn staged_full(f: &Fixture, workspace: &mut SourceWorkspace) -> (RefactorJournal, SourceLibraryCompletePublication1031) {
    birth(workspace, A); add(workspace, A, "JournalA"); birth(workspace, B); dirty_main(workspace);
    let current = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    let unused = workspace.next_library_member_graph_id1027(Some(&current)).unwrap();
    workspace.commit_member_graph_id(unused).unwrap(); workspace.write_recovery(&current.batch.graphs).unwrap();
    let before_recovery = workspace.expected_recovery.clone(); let before_resources = workspace.library_journal_readset1027();
    let receipt = publish(workspace);
    let changes = receipt.before_current1031().libraries.iter().cloned().map(|before| LibraryRecoveryChange1027::Remove { before }).collect();
    let transition = RecoveryTransition1025 { kind: "source_library_recovery_transition_v5".into(), version: 5, mode: TransitionMode1025::CompleteSource, before_recovery: before_recovery.clone(), after_recovery: None, changes: Vec::new(), library1027: Some(LibraryTransition1027 { changes, saved: receipt.saved_libraries1031().to_vec() }) };
    let journal = RefactorJournal { library_before1027: before_resources, library_after1027: workspace.library_journal_readset1027(), source: "story.rvn".into(), graphs: receipt.after_saved1031().to_vec(), files: receipt.files1031().to_vec(), recovery: Some(JournalRecovery1025::Transition(transition)) };
    // Reproduce interruption using the real successful publication's exact
    // images, never guessed metadata, synthetic Saved graphs or fresh owners.
    for file in &journal.files { replace(&f.root.join(&file.path), &file.before).unwrap(); }
    replace(&f.root.join(RECOVERY), &before_recovery).unwrap();
    (journal, receipt)
}
#[test]
fn library_complete1031_exact_half_publication_replays_cold_as_one_complete_cohort() {
    let f = Fixture::new(); let mut workspace = f.link(); let (journal, receipt) = staged_full(&f, &mut workspace);
    replace(&f.root.join("story.rvn"), &Some(receipt.after_source1031().into())).unwrap();
    fs::write(f.root.join(REFACTOR_JOURNAL), serde_json::to_string(&journal).unwrap()).unwrap();
    let cold = SourceWorkspace::open_scoped_libraries1027(&f.root, 0).unwrap().unwrap();
    assert_eq!(cold.project.graphs(), receipt.after_saved1031()); assert!(cold.recovery_cohort_libraries1027().unwrap().is_none());
    for file in &journal.files { assert_eq!(read_optional(&f.root.join(&file.path)).unwrap(), file.after); }
    assert!(!f.root.join(REFACTOR_JOURNAL).exists());
}
#[test]
fn library_complete1031_tampered_saved_inventory_and_recovery_ahead_of_files_are_not_replayed() {
    for tamper in 0..3 {
        let f = Fixture::new(); let mut workspace = f.link(); let (mut journal, _) = staged_full(&f, &mut workspace);
        if tamper == 1 { fs::remove_file(f.root.join(RECOVERY)).unwrap(); }
        else if tamper == 2 {
            let sidecar = journal.files.iter_mut().find(|f| f.path == SIDECAR).unwrap();
            let mut value: serde_json::Value = serde_json::from_str(sidecar.after.as_deref().unwrap()).unwrap();
            let high = value["graph_id_high_water"].as_u64().unwrap(); value["graph_id_high_water"] = serde_json::json!(high - 1);
            sidecar.after = Some(serde_json::to_string(&value).unwrap());
        } else if let Some(JournalRecovery1025::Transition(transition)) = &mut journal.recovery { transition.library1027.as_mut().unwrap().saved.pop(); }
        fs::write(f.root.join(REFACTOR_JOURNAL), serde_json::to_string(&journal).unwrap()).unwrap(); let before = f.bytes();
        assert!(SourceWorkspace::open_scoped_libraries1027(&f.root, 0).is_err()); assert_eq!(f.bytes(), before);
    }
}

#[test]
fn library_complete1031_partial_save_still_keeps_unselected_current_and_saved_separate() {
    let f = Fixture::new(); let mut workspace = f.link(); birth(&mut workspace, A); add(&mut workspace, A, "Selected"); birth(&mut workspace, B); add(&mut workspace, B, "Unselected"); dirty_main(&mut workspace);
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap(); let saved = workspace.project.graphs().to_vec();
    let selected = expected.libraries.iter().find(|r| r.id() == &id(A)).unwrap(); let asset = selected.after().unwrap().wire().asset().unwrap();
    let members = expected.batch.graphs.iter().filter(|g| g.library1027.as_ref().and_then(|m| m.owner.as_ref()) == Some(&id(A))).cloned().collect::<Vec<_>>();
    let prepared = crate::prepare_library_current_cohort1027(&members, std::slice::from_ref(&asset)).unwrap(); let source = prepared.source().to_owned() + workspace.project.source();
    let complete_asset_inputs1031 = [asset]; let resources = crate::LibraryGraphResources1027::prepare(prepared.scopes(), &complete_asset_inputs1031).unwrap(); let mut templates = saved.clone(); templates.extend(members);
    let graphs = crate::import_library_program1027(&rvn_parser::parse(&source).unwrap(), &resources, &templates).unwrap();
    let before = fs::read_to_string(f.root.join("rvn.toml")).unwrap(); let mut files = crate::source_function_library_recovery1023::publication_files(std::slice::from_ref(selected)).unwrap();
    files.push(SourceRefactorFile { path: "rvn.toml".into(), before: Some(before.clone()), after: Some(format!("{before}\n[function_libraries]\n\"{A}\"=\"{A}.rvnlibrary\"\n")) });
    workspace.save_library_recovery_receipt1030(&expected, &[id(A)], &source, &graphs, &files, || ()).unwrap();
    let residual = workspace.recovery_cohort_libraries1027().unwrap().unwrap(); assert_eq!(residual.batch.graphs, expected.batch.graphs); assert_eq!(residual.libraries, expected.libraries.iter().filter(|r| r.id() != &id(A)).cloned().collect::<Vec<_>>());
    let saved_main = saved.iter().find(|g| matches!(g.kind, crate::GraphKind::Label { .. })).unwrap(); assert!(workspace.project.graphs().contains(saved_main));
    assert!(!f.root.join(format!("assets/{B}.rvnlibrary")).exists()); assert!(!workspace.project.source().contains("Current ordinary text"));
    let cold = SourceWorkspace::open_scoped_libraries1027(&f.root, 0).unwrap().unwrap(); assert_eq!(cold.recovery_cohort_libraries1027().unwrap().unwrap(), residual);
}

// The existing fourteen legacy Complete/Partial cases above remain unchanged.
// These cases exercise the callback-free producer, not App/native completion.
fn verified_business_images1031(f:&Fixture)->BTreeMap<String,Vec<u8>> {
    f.bytes().into_iter().filter(|(path,_)|!path.starts_with(".rvn-backups/")&&path!=REFACTOR_JOURNAL).collect()
}
#[test]
fn library_complete_verified1031_real_all_owner_publication_retains_empty_library_and_cold_order(){
    let f=Fixture::new();let mut workspace=f.link();birth(&mut workspace,A);add(&mut workspace,A,"ActualMember");birth(&mut workspace,B);dirty_main(&mut workspace);
    let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let(source,graphs,files)=candidate(&workspace);
    let result=workspace.save_complete_library_recovery_verified1031(&expected,&source,&graphs,&files).unwrap();
    assert_eq!(result.committed1031().cleanup1031(),CleanupStatus1031::Removed);
    assert_eq!(result.committed1031().root1031(),workspace.root.as_path());
    assert_eq!(result.publication1031().before_current1031(),&expected);
    assert_eq!(result.publication1031().saved_libraries1031(),&[id(A),id(B)]);
    assert_eq!(result.publication1031().after_saved1031(),graphs);
    let empty=FunctionLibraryAsset::decode(&fs::read(f.root.join(format!("assets/{B}.rvnlibrary"))).unwrap()).unwrap();assert!(empty.members().is_empty());
    let journal:RefactorJournal=serde_json::from_str(result.committed1031().original_journal1031()).unwrap();
    let Some(JournalRecovery1025::Transition(transition))=journal.recovery else{panic!("actual typed Complete journal")};
    assert_eq!(transition.mode,TransitionMode1025::CompleteSource);assert_eq!(transition.library1027.unwrap().saved,vec![id(A),id(B)]);
    assert_eq!(workspace.project.graphs(),graphs);assert!(workspace.recovery_cohort_libraries1027().unwrap().is_none());
    let cold=SourceWorkspace::open_scoped_libraries1027(&f.root,0).unwrap().unwrap();
    assert_eq!(cold.project.graphs(),graphs);assert_eq!(cold.project.source(),source);assert!(cold.recovery_cohort_libraries1027().unwrap().is_none());
}
#[test]
fn library_complete_verified1031_cleanup_warning_survives_fallible_caller_and_retry_is_zero_rewrite(){
    use publication_outcome1031::{injected1031,Fault1031};
    let f=Fixture::new();let mut workspace=f.link();birth(&mut workspace,A);birth(&mut workspace,B);dirty_main(&mut workspace);
    let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let(source,graphs,files)=candidate(&workspace);let(ex,probe)=injected1031(Fault1031::Cleanup);
    // Retain the genuine result BEFORE the subsequent fallible caller. The
    // producer receives no callback; an App consumer must retain it likewise.
    let retained=workspace.save_complete_library_verified_inner1031(&expected,&source,&graphs,&files,&ex).unwrap();
    let caller_result:Result<(),&str>=Err("independent notification refused");assert!(caller_result.is_err());
    let CleanupStatus1031::Pending(warning)=retained.committed1031().cleanup1031()else{panic!("actual cleanup failure")};
    assert_eq!(warning.kind1031(),std::io::ErrorKind::PermissionDenied);
    assert_eq!(workspace.project.graphs(),graphs);assert!(workspace.expected_recovery.is_none());
    let journal:RefactorJournal=serde_json::from_str(retained.committed1031().original_journal1031()).unwrap();
    for file in &journal.files{assert_eq!(read_optional(&f.root.join(&file.path)).unwrap(),file.after);}
    let Some(JournalRecovery1025::Transition(t))=&journal.recovery else{panic!()};assert_eq!(read_optional(&f.root.join(RECOVERY)).unwrap(),t.after_recovery);
    let bytes=f.bytes();let counts=probe.borrow().stats1031();assert_eq!((counts.0,counts.4,counts.5),(1,1,1));
    let refused=workspace.save_complete_library_verified_inner1031(&expected,&source,&graphs,&files,&ex);
    assert!(refused.err().unwrap().contains("maintenance"));assert_eq!(f.bytes(),bytes);assert_eq!(probe.borrow().stats1031(),counts);
    let(retry,retry_probe)=injected1031(Fault1031::None);
    let cleanup={let _lock=lock(&workspace.root).unwrap();super::super::cleanup_typed_journal1031(&workspace.root,retained.committed1031(),&retry).unwrap()};
    assert_eq!(cleanup.cleanup1031(),CleanupStatus1031::Removed);assert_eq!(cleanup.original_journal1031(),retained.committed1031().original_journal1031());
    let mut after=f.bytes();let original=after.insert(REFACTOR_JOURNAL.into(),bytes[REFACTOR_JOURNAL].clone());assert!(original.is_none());assert_eq!(after,bytes);
    assert_eq!(retry_probe.borrow().stats1031(),(0,0,0,0,1,1));
    assert_eq!(workspace.project.graphs(),graphs);assert_eq!(retained.publication1031().saved_libraries1031(),&[id(A),id(B)]);
}
#[test]
fn library_complete_verified1031_precommit_failures_preserve_original_current_and_never_mint(){
    use publication_outcome1031::{injected1031,Fault1031};
    for fault in [Fault1031::BeforeJournal,Fault1031::Target(1),Fault1031::Recovery,Fault1031::Readback]{
        let f=Fixture::new();let mut workspace=f.link();birth(&mut workspace,A);birth(&mut workspace,B);dirty_main(&mut workspace);
        let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let(source,graphs,files)=candidate(&workspace);
        let before=verified_business_images1031(&f);let saved=workspace.project.graphs().to_vec();let saved_source=workspace.project.source().to_owned();let sidecar=workspace.expected_sidecar.clone();let recovery=workspace.expected_recovery.clone();let high=workspace.graph_id_high_water();
        let(ex,probe)=injected1031(fault);let result=workspace.save_complete_library_verified_inner1031(&expected,&source,&graphs,&files,&ex);
        assert!(result.is_err());assert_eq!(probe.borrow().stats1031().4,0);assert_eq!(verified_business_images1031(&f),before);
        assert_eq!(workspace.project.graphs(),saved);assert_eq!(workspace.project.source(),saved_source);assert_eq!(workspace.expected_sidecar,sidecar);assert_eq!(workspace.expected_recovery,recovery);assert_eq!(workspace.graph_id_high_water(),high);
        assert_eq!(workspace.recovery_cohort_libraries1027().unwrap(),Some(expected));assert!(!f.root.join(REFACTOR_JOURNAL).exists());
        let counts=probe.borrow().stats1031();assert_eq!(counts.0,1);assert_eq!(counts.5,0);
        if fault==Fault1031::BeforeJournal{assert_eq!(counts,(1,0,0,0,0,0));}
        if fault==Fault1031::Target(1){assert_eq!(counts.2,1);}
        if fault==Fault1031::Recovery{assert!(counts.2>1);assert_eq!(counts.3,0);}
        if fault==Fault1031::Readback{assert!(counts.2>1);assert_eq!(counts.3,1);}
    }
}
