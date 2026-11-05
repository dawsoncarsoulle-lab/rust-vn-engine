//! Actual Source Recovery/lifecycle protocol. Defined CPU tests, not native ACK.
use super::*;
use std::fs;
const A: &str = "11111111111141118111111111111111";
fn id() -> FunctionLibraryId {
    FunctionLibraryId::parse(A).unwrap()
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "rvn-library-lifecycle1027-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("assets")).unwrap();
        fs::write(root.join("rvn.toml"), "[paths]\nassets=\"assets\"\n").unwrap();
        fs::write(
            root.join("story.rvn"),
            "// retain actual source\nscene label start\n\"Before\"\n",
        )
        .unwrap();
        Self(root)
    }
    fn link(&self) -> SourceWorkspace {
        SourceWorkspace::link_scoped_libraries1027(&self.0, &self.0.join("story.rvn"), &[]).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn prepare(workspace: &SourceWorkspace) -> LibraryBirth1026 {
    prepare_library_birth1026(workspace, &[], id(), "assets/new.rvnlibrary").unwrap()
}
fn install(workspace: &mut SourceWorkspace, birth: &LibraryBirth1026) {
    birth.validate_original_source(workspace).unwrap();
    workspace
        .replace_recovery_with_library_changes1027(
            birth.expected_cohort1027(),
            birth.graphs(),
            &[],
            &[],
            &[],
            &[birth.recovery_change1027().unwrap()],
            birth.fresh_graphs(),
            birth.identity_plans1027(),
            &[],
            || {
                if birth.first_open {
                    assert!(birth.consume_first_editor_open1027());
                }
            },
        )
        .unwrap();
}
#[test]
fn library_lifecycle1027_first_open_requires_actual_birth_and_commits_real_member_before_preparing_save(
) {
    let fixture = Fixture::new();
    let mut workspace = fixture.link();
    let source = fs::read(fixture.0.join("story.rvn")).unwrap();
    let high = workspace.graph_id_high_water();
    assert!(prepare(&workspace)
        .prepare_first_editor_open(&workspace, "NewFunction")
        .is_err());
    assert_eq!(workspace.graph_id_high_water(), high);
    let birth = prepare(&workspace);
    assert!(birth.asset().members().is_empty());
    assert!(birth.fresh_graphs().is_empty());
    install(&mut workspace, &birth);
    let initialized = birth
        .recapture_first_editor_open_intent1027(&workspace)
        .unwrap()
        .prepare_first_editor_open(&workspace, "NewFunction")
        .unwrap();
    assert_eq!(
        workspace.graph_id_high_water(),
        high,
        "preparation alone reserves no ID"
    );
    assert_eq!(initialized.fresh_graphs().len(), 1);
    let member = initialized
        .graphs()
        .iter()
        .find(|g| g.graph_id == initialized.fresh_graphs()[0])
        .unwrap();
    assert!(matches!(member.kind, crate::GraphKind::UnitFunction { .. }));
    assert_eq!(member.nodes.len(), 1);
    assert!(member
        .nodes
        .values()
        .all(|n| n.kind == crate::NodeKind::FunctionEntry));
    assert_eq!(
        member.library1027.as_ref().unwrap().owner.as_ref(),
        Some(&id())
    );
    assert!(
        initialized.prepare_save(&workspace).is_err(),
        "proposal is not an installed Current cohort"
    );
    assert!(
        !initialized.consume_first_editor_open1027(),
        "an uncommitted proposal cannot consume the newborn token"
    );
    assert!(birth
        .recapture_first_editor_open_intent1027(&workspace)
        .is_ok());
    install(&mut workspace, &initialized);
    let save = initialized.prepare_save(&workspace).unwrap();
    assert!(
        matches!(save.scope().members()[0].declaration(),rvn_parser::Statement::UnitFunction{body,..}if body.is_empty())
    );
    assert!(!save.scope_source().contains("return 0"));
    save.validate_original_source(&workspace).unwrap();
    assert!(initialized
        .prepare_first_editor_open(&workspace, "Again")
        .is_err());
    assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(), source);
    assert!(!fixture.0.join("assets/new.rvnlibrary").exists());
}
#[test]
fn library_lifecycle1027_saved_empty_and_cold_workspace_never_receive_newborn_seed_intent() {
    let fixture = Fixture::new();
    let mut workspace = fixture.link();
    let high = workspace.graph_id_high_water();
    let birth = prepare(&workspace);
    install(&mut workspace, &birth);
    let plan = birth.prepare_save(&workspace).unwrap();
    let source = plan.scope_source().to_owned() + workspace.project().source();
    let script = rvn_parser::parse(&source).unwrap();
    let assets = [plan.asset().clone()];
    let scopes = [plan.scope().clone()];
    let resources = LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graphs =
        crate::import_library_program1027(&script, &resources, workspace.project().graphs())
            .unwrap();
    let before = fs::read_to_string(fixture.0.join("rvn.toml")).unwrap();
    let after = format!("{before}\n[function_libraries]\n\"{A}\"=\"new.rvnlibrary\"\n");
    let mut files = plan.files().to_vec();
    files.push(SourceRefactorFile {
        path: "rvn.toml".into(),
        before: Some(before),
        after: Some(after),
    });
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    workspace
        .save_library_recovery_with1027(&expected, &[id()], &source, &graphs, &files, || ())
        .unwrap();
    let cold = SourceWorkspace::open_scoped_libraries1027(&fixture.0, 0)
        .unwrap()
        .unwrap();
    let baseline = [FunctionLibraryBaseline::new(
        id(),
        LibraryRecoveryState::new(
            "assets/new.rvnlibrary".into(),
            FunctionLibraryWire::from_asset(plan.asset()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()];
    assert!(prepare_library_birth1026(&cold, &baseline, id(), "assets/new.rvnlibrary").is_err());
    let loaded = admit_saved_library1026(
        &id(),
        "assets/new.rvnlibrary",
        plan.files()[0].after.as_deref().unwrap(),
        plan.scope_source(),
        cold.project().graphs(),
    )
    .unwrap();
    assert!(loaded.asset().members().is_empty());
    assert!(loaded.scope().members().is_empty());
    assert_eq!(cold.graph_id_high_water(), high);
    assert_eq!(cold.project().graphs(), workspace.project().graphs());
    assert!(birth.recapture_first_editor_open_intent1027(&cold).is_err());
    let warm_open = birth
        .recapture_first_editor_open_intent1027(&workspace)
        .unwrap();
    let initialized = warm_open
        .prepare_first_editor_open(&workspace, "NewFunction")
        .unwrap();
    install(&mut workspace, &initialized);
    assert_eq!(workspace.graph_id_high_water(), high + 1);
    assert!(birth
        .recapture_first_editor_open_intent1027(&workspace)
        .is_err());
}
#[test]
fn library_lifecycle1027_cold_incarnation_refuses_first_open_without_consuming_identity() {
    let fixture = Fixture::new();
    let mut workspace = fixture.link();
    let birth = prepare(&workspace);
    install(&mut workspace, &birth);
    let cold = SourceWorkspace::open_scoped_libraries1027(&fixture.0, 0)
        .unwrap()
        .unwrap();
    let cache = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    let high = workspace.graph_id_high_water();
    assert!(birth
        .prepare_first_editor_open(&cold, "NotTransferred")
        .is_err());
    assert_eq!(workspace.graph_id_high_water(), high);
    assert_eq!(
        workspace.recovery_cohort_libraries1027().unwrap().unwrap(),
        cache
    );
    assert!(cache.libraries[0]
        .after()
        .unwrap()
        .wire()
        .asset()
        .unwrap()
        .members()
        .is_empty());
}
#[test]
fn library_lifecycle1027_changed_current_refuses_stale_first_open_without_consuming_identity() {
    let fixture = Fixture::new();
    let mut workspace = fixture.link();
    let birth = prepare(&workspace);
    install(&mut workspace, &birth);
    let open = birth
        .recapture_first_editor_open_intent1027(&workspace)
        .unwrap();
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    let mut graphs = expected.batch.graphs.clone();
    graphs[0].nodes.values_mut().next().unwrap().position = [313., 217.];
    workspace
        .replace_recovery_with_library_changes1027(
            Some(&expected),
            &graphs,
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            || (),
        )
        .unwrap();
    let after = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    let high = workspace.graph_id_high_water();
    let file = fs::read(fixture.0.join("story.rvn")).unwrap();
    assert!(open
        .prepare_first_editor_open(&workspace, "StaleIntent")
        .is_err());
    assert_eq!(workspace.graph_id_high_water(), high);
    assert_eq!(
        workspace.recovery_cohort_libraries1027().unwrap().unwrap(),
        after
    );
    assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(), file);
    assert!(after.libraries[0]
        .after()
        .unwrap()
        .wire()
        .asset()
        .unwrap()
        .members()
        .is_empty());
    let retry = open
        .recapture_first_editor_open_intent1027(&workspace)
        .unwrap();
    let initialized = retry
        .prepare_first_editor_open(&workspace, "NewFunction")
        .unwrap();
    install(&mut workspace, &initialized);
    assert_eq!(workspace.graph_id_high_water(), high + 1);
    assert!(open
        .recapture_first_editor_open_intent1027(&workspace)
        .is_err());
}
#[test]
fn library_lifecycle1027_new_open_after_admitted_rename_keeps_original_uuid_without_reusing_old_callback(
) {
    let fixture = Fixture::new();
    let mut workspace = fixture.link();
    let birth = prepare(&workspace);
    install(&mut workspace, &birth);
    let old_open = birth
        .recapture_first_editor_open_intent1027(&workspace)
        .unwrap();
    let expected = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    let before = expected.libraries[0].clone();
    let after = FunctionLibraryRecovery::new(
        id(),
        before.before().cloned(),
        Some(
            LibraryRecoveryState::new(
                "assets/renamed.rvnlibrary".into(),
                before.after().unwrap().wire().clone(),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    workspace
        .replace_recovery_with_library_changes1027(
            Some(&expected),
            &expected.batch.graphs,
            &[],
            &[],
            &[],
            &[LibraryRecoveryChange1027::Upsert {
                before: Some(before),
                after,
            }],
            &[],
            &[],
            &[],
            || (),
        )
        .unwrap();
    assert!(old_open
        .prepare_first_editor_open(&workspace, "Stale")
        .is_err());
    let fresh = birth
        .recapture_first_editor_open_intent1027(&workspace)
        .unwrap();
    assert_eq!(fresh.locator(), "assets/renamed.rvnlibrary");
    assert_eq!(fresh.asset().id(), &id());
    let initialized = fresh
        .prepare_first_editor_open(&workspace, "NewFunction")
        .unwrap();
    install(&mut workspace, &initialized);
    let current = workspace.recovery_cohort_libraries1027().unwrap().unwrap();
    assert_eq!(
        current.libraries[0].after().unwrap().locator(),
        "assets/renamed.rvnlibrary"
    );
    assert_eq!(
        current.libraries[0]
            .after()
            .unwrap()
            .wire()
            .asset()
            .unwrap()
            .id(),
        &id()
    );
    assert_eq!(
        initialized
            .graphs()
            .iter()
            .filter(|g| g.library1027.as_ref().and_then(|m| m.owner.as_ref()) == Some(&id()))
            .count(),
        1
    );
    assert!(birth
        .recapture_first_editor_open_intent1027(&workspace)
        .is_err());
}


#[test]
fn library_lifecycle1031_first_editor_birth_with_project_globals_and_characters_saves_and_cold_reopens_exact_full_members(){
 let fixture=Fixture::new();fs::write(fixture.0.join("story.rvn"),"init{set amount=19\ncharacter.create(\"iris\",\"Current Iris\")}\nscene label start\n\"Before\"\n").unwrap();let mut workspace=fixture.link();
 let birth=prepare(&workspace);install(&mut workspace,&birth);let initialized=birth.recapture_first_editor_open_intent1027(&workspace).unwrap().prepare_first_editor_open(&workspace,"NewFunction").unwrap();
 let member=initialized.graphs().iter().find(|graph|graph.graph_id==initialized.fresh_graphs()[0]).unwrap();let init=workspace.project().graphs().iter().find(|graph|graph.kind==crate::GraphKind::Init).unwrap();assert_eq!(member.variables,init.variables);assert_eq!(member.characters,init.characters);let expected_member=member.clone();install(&mut workspace,&initialized);
 let plan=initialized.prepare_save(&workspace).unwrap();let source=plan.scope_source().to_owned()+workspace.project().source();let script=rvn_parser::parse(&source).unwrap();let assets=[plan.asset().clone()];let scopes=[plan.scope().clone()];let resources=LibraryGraphResources1027::prepare(&scopes,&assets).unwrap();let expected=workspace.recovery_cohort_libraries1027().unwrap().unwrap();let graphs=crate::import_library_program1027(&script,&resources,&expected.batch.graphs).unwrap();assert_eq!(graphs.iter().find(|graph|graph.graph_id==expected_member.graph_id),Some(&expected_member));
 let before=fs::read_to_string(fixture.0.join("rvn.toml")).unwrap();let mut files=plan.files().to_vec();files.push(SourceRefactorFile{path:"rvn.toml".into(),before:Some(before.clone()),after:Some(format!("{before}\n[function_libraries]\n\"{A}\"=\"new.rvnlibrary\"\n"))});
 workspace.save_library_recovery_with1027(&expected,&[id()],&source,&graphs,&files,||()).unwrap();let cold=SourceWorkspace::open_scoped_libraries1027(&fixture.0,0).unwrap().unwrap();assert_eq!(cold.project().graphs(),graphs);assert_eq!(cold.project().graphs().iter().find(|graph|graph.graph_id==expected_member.graph_id),Some(&expected_member));
}
