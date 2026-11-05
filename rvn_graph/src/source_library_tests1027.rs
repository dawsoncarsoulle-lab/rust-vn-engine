//! Actual Source IO/Parser/Graph transactions. CPU definitions, not native ACK.
use super::*;
use rvn_parser::function_library_asset1023::{
    FunctionLibraryAsset, FunctionLibraryId, FunctionLibraryMember, LibraryFunctionAccess,
};
const A: &str = "11111111111141118111111111111111";
const B: &str = "22222222222242228222222222222222";
fn asset(id: &str, graph: u64) -> FunctionLibraryAsset {
    FunctionLibraryAsset::new(
        FunctionLibraryId::parse(id).unwrap(),
        vec![FunctionLibraryMember {
            graph_id: graph,
            access: LibraryFunctionAccess::Public,
            pure: true,
        }],
    )
    .unwrap()
}
fn descriptor(id: &str, graph: u64) -> String {
    String::from_utf8(asset(id, graph).encode().unwrap()).unwrap()
}
fn scope(id: &str, graph: u64, n: i32) -> String {
    format!("functions.library_v1(\"{id}\"){{functions.member_v1(\"{graph}\",[]){{function same(){{return {n}}}}}}}\n")
}
fn manifest(second: bool) -> String {
    format!(
        "[paths]\nassets=\"assets\"\n[function_libraries]\n\"{A}\"=\"one.rvnlibrary\"\n{}",
        if second {
            format!("\"{B}\"=\"two.rvnlibrary\"\n")
        } else {
            String::new()
        }
    )
}
fn source(second: bool) -> String {
    format!("// keep original source note\n{}{}scene label start\nset result=functions.value_v1(\"{A}\",\"11\",{{}})\n\"Untouched tail\"\n",scope(A,11,1),if second{scope(B,22,2)}else{String::new()})
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("rvn-library-source1027-{}", unique_name()));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("assets")).unwrap();
        fs::write(root.join("rvn.toml"), manifest(false)).unwrap();
        fs::write(root.join("assets/one.rvnlibrary"), descriptor(A, 11)).unwrap();
        fs::write(root.join("story.rvn"), source(false)).unwrap();
        // Public SourceWorkspace entry points canonicalize before recovery.
        // Direct private recover probes must use that same Windows root form.
        Self(root.canonicalize().unwrap())
    }
    fn linked(&self) -> SourceWorkspace {
        SourceWorkspace::link_scoped_libraries1027(&self.0, &self.0.join("story.rvn"), &[]).unwrap()
    }
    fn candidate(
        &self,
        workspace: &SourceWorkspace,
    ) -> (String, Vec<GraphDocument>, Vec<SourceRefactorFile>) {
        let source = source(true);
        let parsed = rvn_parser::parse(&source).unwrap();
        let scopes = parsed
            .iter()
            .filter_map(|s| {
                if let rvn_parser::Statement::FunctionLibrary { scope } = s {
                    Some(scope.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let assets = [asset(A, 11), asset(B, 22)];
        let resources = crate::LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
        let graphs =
            crate::import_library_program1027(&parsed, &resources, workspace.project().graphs())
                .unwrap();
        (
            source,
            graphs,
            vec![
                SourceRefactorFile {
                    path: "rvn.toml".into(),
                    before: Some(manifest(false)),
                    after: Some(manifest(true)),
                },
                SourceRefactorFile {
                    path: "assets/two.rvnlibrary".into(),
                    before: None,
                    after: Some(descriptor(B, 22)),
                },
            ],
        )
    }
    fn staged(&self, workspace: &SourceWorkspace) -> String {
        let (source, graphs, additional) = self.candidate(workspace);
        let mut snapshot: Snapshot =
            serde_json::from_str(workspace.expected_sidecar.as_deref().unwrap()).unwrap();
        snapshot.graphs = graphs.clone();
        snapshot.source_snapshot = source.clone();
        snapshot.graph_id_high_water = Some(workspace.graph_id_high_water().max(22));
        let mut files = vec![
            SourceRefactorFile {
                path: "story.rvn".into(),
                before: Some(workspace.project.source().into()),
                after: Some(source),
            },
            SourceRefactorFile {
                path: SIDECAR.into(),
                before: workspace.expected_sidecar.clone(),
                after: Some(serde_json::to_string_pretty(&snapshot).unwrap()),
            },
        ];
        workspace
            .append_publication_files(&mut files, &additional)
            .unwrap();
        serde_json::to_string(&serde_json::json!({"source":"story.rvn","graphs":graphs,"files":files,"recovery":null})).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn changed(workspace: &SourceWorkspace) -> Vec<GraphDocument> {
    let mut graphs = workspace.project().graphs().to_vec();
    let member = graphs.iter_mut().find(|g| g.graph_id.get() == 11).unwrap();
    let literal = member
        .nodes
        .values()
        .find(|n| {
            n.kind == crate::NodeKind::Literal
                && n.properties.get("value") == Some(&crate::PropertyValue::Int(1))
        })
        .unwrap()
        .id;
    member
        .set_property(literal, "value", crate::PropertyValue::Int(9))
        .unwrap();
    graphs
}

#[test]
fn library_source1027_cold_link_visual_save_and_reopen_preserve_real_member_ids_and_trivia() {
    let fixture = Fixture::new();
    let mut workspace = fixture.linked();
    let graphs = changed(&workspace);
    workspace.save(&graphs).unwrap();
    assert!(workspace
        .project()
        .source()
        .starts_with("// keep original source note\n"));
    assert!(workspace.project().source().contains("Untouched tail"));
    assert!(workspace.project().source().contains("return 9"));
    let saved = workspace.project().graphs().to_vec();
    drop(workspace);
    let reopened = SourceWorkspace::open_scoped_libraries1027(&fixture.0, 0)
        .unwrap()
        .unwrap();
    assert_eq!(reopened.project().graphs(), saved);
    assert_eq!(
        reopened.source_library_inputs1027().unwrap().assets(),
        &[asset(A, 11)]
    );
}
#[test]
fn library_source1027_stale_original_descriptor_refuses_save_before_source_or_sidecar_mutation() {
    let fixture = Fixture::new();
    let mut workspace = fixture.linked();
    let graphs = changed(&workspace);
    let before = fs::read(fixture.0.join("story.rvn")).unwrap();
    let sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
    fs::write(
        fixture.0.join("assets/one.rvnlibrary"),
        format!("{}\n", descriptor(A, 11)),
    )
    .unwrap();
    assert!(workspace.save(&graphs).is_err());
    assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(), before);
    assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), sidecar);
    assert!(!fixture.0.join(JOURNAL).exists());
}
#[test]
fn library_source1027_second_owner_refactor_and_replay_use_original_before_after_resources() {
    let fixture = Fixture::new();
    let mut workspace = fixture.linked();
    let (source, graphs, files) = fixture.candidate(&workspace);
    let receipt = workspace
        .refactor_library_source1027(&source, &graphs, &files)
        .unwrap();
    assert_eq!(
        workspace
            .source_library_inputs1027()
            .unwrap()
            .assets()
            .len(),
        2
    );
    workspace.replay_refactor(&receipt, false).unwrap();
    assert_eq!(
        workspace.source_library_inputs1027().unwrap().assets(),
        &[asset(A, 11)]
    );
    assert!(!fixture.0.join("assets/two.rvnlibrary").exists());
    drop(workspace);
    let mut reopened = SourceWorkspace::open_scoped_libraries1027(&fixture.0, 0)
        .unwrap()
        .unwrap();
    reopened.replay_refactor(&receipt, true).unwrap();
    assert_eq!(
        reopened.source_library_inputs1027().unwrap().assets().len(),
        2
    );
    assert_eq!(reopened.project().source(), source);
    assert_eq!(reopened.project().graphs(), graphs);
    assert!(reopened.graph_id_high_water() >= 22);
}
#[test]
fn library_source1027_original_journal_readset_refuses_changed_metadata_before_recovery_writes() {
    let fixture = Fixture::new();
    let workspace = fixture.linked();
    let before_source = fs::read_to_string(fixture.0.join("story.rvn")).unwrap();
    let before_sidecar = fs::read_to_string(fixture.0.join(SIDECAR)).unwrap();
    let journal = Transaction {
        library_readset1027: workspace.library_journal_readset1027(),
        reservation_archive: None,
        editorial_publication: None,
        source: "story.rvn".into(),
        before_source: before_source.clone(),
        after_source: before_source.clone(),
        before_sidecar: Some(before_sidecar.clone()),
        after_sidecar: before_sidecar.clone(),
    };
    fs::write(
        fixture.0.join(JOURNAL),
        serde_json::to_string(&journal).unwrap(),
    )
    .unwrap();
    fs::write(
        fixture.0.join("assets/one.rvnlibrary"),
        format!("{}\n", descriptor(A, 11)),
    )
    .unwrap();
    assert!(recover(&fixture.0).is_err());
    assert_eq!(
        fs::read_to_string(fixture.0.join("story.rvn")).unwrap(),
        before_source
    );
    assert_eq!(
        fs::read_to_string(fixture.0.join(SIDECAR)).unwrap(),
        before_sidecar
    );
    assert!(fixture.0.join(JOURNAL).exists());
}
#[test]
fn library_source1027_absent_index_keeps_legacy_absolute_assets_and_observes_manifest_without_mkdir(
) {
    let root = std::env::temp_dir().join(format!("rvn-library-legacy1027-{}", unique_name()));
    fs::create_dir(&root).unwrap();
    let fixture = Fixture(root);
    fs::write(
        fixture.0.join("rvn.toml"),
        "[paths]\nassets='D:/original-assets'\n",
    )
    .unwrap();
    let captured = SourceLibraryInputs1027::read(&fixture.0).unwrap().unwrap();
    assert!(captured.assets().is_empty());
    assert_eq!(captured.source_references().len(), 1);
    captured.validate_original().unwrap();
    assert!(!fixture.0.join("assets").exists());
    fs::write(
        fixture.0.join("rvn.toml"),
        "[paths]\nassets='D:/changed-assets'\n",
    )
    .unwrap();
    assert!(captured.validate_original().is_err());
}
#[test]
fn library_source1027_indexed_absolute_namespace_and_missing_original_descriptor_are_refused() {
    assert!(crate::parse_library_manifest1027(
        "[paths]\nassets='D:/assets'\n[function_libraries]\n"
    )
    .is_err());
    let fixture = Fixture::new();
    fs::remove_file(fixture.0.join("assets/one.rvnlibrary")).unwrap();
    assert!(SourceLibraryInputs1027::read(&fixture.0).is_err());
    assert!(!fixture.0.join(SIDECAR).exists());
}
#[test]
fn library_source1027_partial_refactor_recovers_from_exact_after_descriptors_not_mixed_disk() {
    let fixture = Fixture::new();
    let workspace = fixture.linked();
    let journal = fixture.staged(&workspace);
    fs::write(fixture.0.join(refactor::REFACTOR_JOURNAL), journal).unwrap();
    // Real interrupted order: manifest is AFTER, descriptor2/source/sidecar BEFORE.
    fs::write(fixture.0.join("rvn.toml"), manifest(true)).unwrap();
    assert!(!fixture.0.join("assets/two.rvnlibrary").exists());
    recover(&fixture.0).unwrap();
    assert_eq!(
        fs::read_to_string(fixture.0.join("story.rvn")).unwrap(),
        source(true)
    );
    assert_eq!(
        fs::read_to_string(fixture.0.join("assets/two.rvnlibrary")).unwrap(),
        descriptor(B, 22)
    );
    assert!(!fixture.0.join(refactor::REFACTOR_JOURNAL).exists());
    let reopened = SourceWorkspace::open_scoped_libraries1027(&fixture.0, 0)
        .unwrap()
        .unwrap();
    assert_eq!(
        reopened.source_library_inputs1027().unwrap().assets().len(),
        2
    );
}
#[test]
fn library_source1027_partial_refactor_foreign_descriptor_refuses_before_any_remaining_publication()
{
    let fixture = Fixture::new();
    let workspace = fixture.linked();
    let journal = fixture.staged(&workspace);
    let before_source = fs::read(fixture.0.join("story.rvn")).unwrap();
    let before_sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
    fs::write(fixture.0.join(refactor::REFACTOR_JOURNAL), journal).unwrap();
    fs::write(fixture.0.join("rvn.toml"), manifest(true)).unwrap();
    fs::write(
        fixture.0.join("assets/one.rvnlibrary"),
        format!("{}\n", descriptor(A, 11)),
    )
    .unwrap();
    assert!(recover(&fixture.0).is_err());
    assert_eq!(
        fs::read(fixture.0.join("story.rvn")).unwrap(),
        before_source
    );
    assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), before_sidecar);
    assert!(!fixture.0.join("assets/two.rvnlibrary").exists());
    assert!(fixture.0.join(refactor::REFACTOR_JOURNAL).exists());
}
#[test]
fn library_source1027_unrelated_native_uses_existing_compiler_and_qualified_native_still_refuses() {
    let mut native = crate::import_script(
        &rvn_parser::parse("scene label start\nset number=1\n\"After\"").unwrap(),
    )
    .unwrap()
    .remove(0);
    native.enable_native_frame_preparation().unwrap();
    let empty = crate::LibraryGraphResources1027::prepare(&[], &[]).unwrap();
    let cohort = vec![native.clone()];
    let old = crate::native_frame_preparation::prepare_native_unchecked(&native).unwrap();
    let scoped = crate::codegen::transpile_library_graph1027(&native, &cohort, &empty).unwrap();
    assert_eq!(scoped.source, old.source);
    assert_eq!(scoped.ast, old.ast);
    let fixture = Fixture::new();
    let workspace = fixture.linked();
    let script = rvn_parser::parse(workspace.project().source()).unwrap();
    let scopes = script
        .iter()
        .filter_map(|s| {
            if let rvn_parser::Statement::FunctionLibrary { scope } = s {
                Some(scope.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let assets = [asset(A, 11)];
    let resources = crate::LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let mut caller = workspace
        .project()
        .graphs()
        .iter()
        .find(|g| g.library1027.as_ref().is_some_and(|m| !m.calls.is_empty()))
        .unwrap()
        .clone();
    let before = caller.clone();
    assert!(caller.enable_native_frame_preparation().is_err());
    assert_eq!(caller, before);
    // An explicitly malformed requested combination also cannot use the helper.
    caller.inspector_evaluation = crate::InspectorEvaluationContract::NativeFrameV1;
    let result = crate::codegen::transpile_library_graph1027(
        &caller,
        workspace.project().graphs(),
        &resources,
    )
    .unwrap_err();
    assert!(result.to_string().contains("owned continuation"));
}
#[test]
fn library_source1027_changed_member_result_requires_actual_coherent_caller_before_publication() {
    let fixture = Fixture::new();
    let mut workspace = fixture.linked();
    let before_source = fs::read(fixture.0.join("story.rvn")).unwrap();
    let before_sidecar = fs::read(fixture.0.join(SIDECAR)).unwrap();
    let candidate = source(false).replace("return 1", "return \"changed\"");
    let stale = workspace.project().graphs().to_vec();
    assert!(workspace
        .refactor_library_source1027(&candidate, &stale, &[])
        .is_err());
    assert_eq!(
        fs::read(fixture.0.join("story.rvn")).unwrap(),
        before_source
    );
    assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(), before_sidecar);
    assert_eq!(workspace.project().graphs(), stale);
    let parsed = rvn_parser::parse(&candidate).unwrap();
    let scopes = parsed
        .iter()
        .filter_map(|s| {
            if let rvn_parser::Statement::FunctionLibrary { scope } = s {
                Some(scope.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let assets = [asset(A, 11)];
    let resources = crate::LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
    let graphs =
        crate::import_library_program1027(&parsed, &resources, workspace.project().graphs())
            .unwrap();
    let receipt = workspace
        .refactor_library_source1027(&candidate, &graphs, &[])
        .unwrap();
    assert_eq!(workspace.project().source(), candidate);
    workspace.replay_refactor(&receipt, false).unwrap();
    assert_eq!(workspace.project().graphs(), stale);
}

#[test]
fn library_source1027_actual_scoped_binding_flow_requires_original_resources_without_legacy_alias() {
    let fixture=Fixture::new();let workspace=fixture.linked();
    let graphs=workspace.project().graphs();let caller=graphs.iter().find(|g|g.library1027.as_ref().is_some_and(|m|!m.calls.is_empty())).unwrap();
    let parsed=rvn_parser::parse(workspace.project().source()).unwrap();let scopes=parsed.iter().filter_map(|s|if let rvn_parser::Statement::FunctionLibrary{scope}=s{Some(scope.clone())}else{None}).collect::<Vec<_>>();let assets=[asset(A,11)];let resources=crate::LibraryGraphResources1027::prepare(&scopes,&assets).unwrap();
    let before=caller.to_pretty_json().unwrap();let source=fs::read(fixture.0.join("story.rvn")).unwrap();let sidecar=fs::read(fixture.0.join(SIDECAR)).unwrap();
    assert!(caller.variable_binding_projection_in_project("result",graphs).is_err(),"no fallback compiler may turn a nominal reference into a global function name");
    let projection=caller.variable_binding_projection_library1027("result",graphs,&resources).unwrap();assert_eq!(projection.graph,caller.graph_id);
    let assignment=caller.nodes.values().find(|n|n.kind==crate::NodeKind::SetVariable&&caller.variable_node_name(n.id).as_deref()==Some("result")).unwrap().id;
    assert_eq!(projection.write_contexts.get(&assignment),Some(&crate::VariableBindingContext::ProjectGlobal));
    let compiled=crate::codegen::transpile_library_binding_analysis1027(caller,graphs,&resources).unwrap();assert!(compiled.ast.iter().any(|s|matches!(s,rvn_parser::Statement::SetVar{value:rvn_parser::Expr::LibraryCall{..},..})));assert!(compiled.source.contains("functions.value_v1"));
    let wrong_script=rvn_parser::parse(&scope(B,11,1)).unwrap();let wrong_scopes=wrong_script.iter().filter_map(|s|if let rvn_parser::Statement::FunctionLibrary{scope}=s{Some(scope.clone())}else{None}).collect::<Vec<_>>();let wrong_assets=[asset(B,11)];let wrong=crate::LibraryGraphResources1027::prepare(&wrong_scopes,&wrong_assets).unwrap();assert!(caller.variable_binding_projection_library1027("result",graphs,&wrong).is_err());
    assert_eq!(caller.to_pretty_json().unwrap(),before);assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(),source);assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(),sidecar);
}

#[test]
fn library_source1027_large_no_index_manifest_keeps_legacy_read_but_present_empty_index_is_bounded(){
    let root=std::env::temp_dir().join(format!("rvn-library-large-legacy1027-{}",unique_name()));fs::create_dir(&root).unwrap();let fixture=Fixture(root);
    let mut legacy=String::from("#");legacy.push_str(&" ".repeat(crate::MAX_LIBRARY_SOURCE_INPUT_BYTES1027));legacy.push_str("\n[paths]\nassets='D:/original-assets'\n");
    fs::write(fixture.0.join("rvn.toml"),&legacy).unwrap();let captured=SourceLibraryInputs1027::read(&fixture.0).unwrap().unwrap();assert!(!captured.manifest().has_library_index);assert!(captured.assets().is_empty());assert_eq!(captured.source_references()[0].before.as_deref(),Some(legacy.as_str()));captured.validate_original().unwrap();
    fs::write(fixture.0.join("rvn.toml"),format!("{legacy}\n# changed original\n")).unwrap();assert!(captured.validate_original().is_err());
    legacy.push_str("\n[function_libraries]\n");fs::write(fixture.0.join("rvn.toml"),&legacy).unwrap();assert!(SourceLibraryInputs1027::read(&fixture.0).unwrap_err().contains("64 MiB"));assert!(!fixture.0.join("assets").exists());
}

fn unpublished_enum1027()->rvn_parser::enum_recovery1025::EnumRecoveryRecord1025{
    let value=rvn_parser::enum_asset::EnumAsset::new(rvn_parser::enum_asset::EnumAssetId::parse("329f41eae6ba4b4bb71dfaf60fb50001").unwrap(),"Flag".into()).unwrap();rvn_parser::enum_recovery1025::EnumRecoveryRecord1025::unpublished("assets/Flag.rvnenum".into(),value).unwrap()
}
#[test]
fn library_source1027_real_enum_transition_save_advances_only_actual_manifest_frontier_and_keeps_library_source(){
    let fixture=Fixture::new();let mut workspace=fixture.linked();let source=fs::read(fixture.0.join("story.rvn")).unwrap();let sidecar=fs::read(fixture.0.join(SIDECAR)).unwrap();let record=unpublished_enum1027();let mut installed=0;workspace.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Upsert{before:None,after:record.clone()}],&[],||{installed+=1}).unwrap();assert_eq!(installed,1);assert_eq!(workspace.recovery_cohort_libraries1027().unwrap().unwrap().batch.enums,std::slice::from_ref(&record));
    let wire=record.current().encode().unwrap();let next_manifest=format!("{}\n[enum_assets]\n'{}'='Flag.rvnenum'\n",manifest(false),record.asset_id().to_hex());let files=[SourceRefactorFile{path:"rvn.toml".into(),before:Some(manifest(false)),after:Some(next_manifest.clone())},SourceRefactorFile{path:"assets/Flag.rvnenum".into(),before:None,after:Some(wire.clone())}];let graphs=workspace.project().graphs().to_vec();workspace.save_metadata_files_with_enum_recovery1025(&graphs,&files,&[EnumRecoveryChange1025::Remove{before:record}]).unwrap();workspace.validate_source_library_inputs1027().unwrap();assert_eq!(workspace.source_library_inputs1027().unwrap().assets(),&[asset(A,11)]);assert!(workspace.recovery_cohort_libraries1027().unwrap().is_none());assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(),source);assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(),sidecar);assert_eq!(fs::read_to_string(fixture.0.join("rvn.toml")).unwrap(),next_manifest);assert_eq!(fs::read_to_string(fixture.0.join("assets/Flag.rvnenum")).unwrap(),wire);drop(workspace);let reopened=SourceWorkspace::open_scoped_libraries1027(&fixture.0,0).unwrap().unwrap();reopened.validate_source_library_inputs1027().unwrap();
}
#[test]
fn library_source1027_stale_library_during_enum_transition_keeps_recovery_and_authored_files(){
    let fixture=Fixture::new();let mut workspace=fixture.linked();let record=unpublished_enum1027();workspace.transition_enum_recovery_with1025(&[EnumRecoveryChange1025::Upsert{before:None,after:record.clone()}],&[],||()).unwrap();let cache=fs::read(fixture.0.join(RECOVERY)).unwrap();let source=fs::read(fixture.0.join("story.rvn")).unwrap();let sidecar=fs::read(fixture.0.join(SIDECAR)).unwrap();let old_manifest=fs::read(fixture.0.join("rvn.toml")).unwrap();fs::write(fixture.0.join("assets/one.rvnlibrary"),format!("{}\n",descriptor(A,11))).unwrap();let graphs=workspace.project().graphs().to_vec();let files=[SourceRefactorFile{path:"assets/Flag.rvnenum".into(),before:None,after:Some(record.current().encode().unwrap())}];assert!(workspace.save_metadata_files_with_enum_recovery1025(&graphs,&files,&[EnumRecoveryChange1025::Remove{before:record}]).is_err());assert_eq!(fs::read(fixture.0.join(RECOVERY)).unwrap(),cache);assert_eq!(fs::read(fixture.0.join("story.rvn")).unwrap(),source);assert_eq!(fs::read(fixture.0.join(SIDECAR)).unwrap(),sidecar);assert_eq!(fs::read(fixture.0.join("rvn.toml")).unwrap(),old_manifest);assert!(!fixture.0.join("assets/Flag.rvnenum").exists());assert!(!fixture.0.join(refactor::REFACTOR_JOURNAL).exists());
}
#[cfg(windows)]
#[test]
fn library_source1027_windows_manifest_case_alias_still_publishes_one_coherent_nominal_frontier(){
    let fixture=Fixture::new();let mut workspace=fixture.linked();let(source,graphs,mut files)=fixture.candidate(&workspace);files.iter_mut().find(|f|f.path=="rvn.toml").unwrap().path="RVN.TOML".into();let receipt=workspace.refactor_library_source1027(&source,&graphs,&files).unwrap();assert_eq!(workspace.source_library_inputs1027().unwrap().assets().len(),2);assert_eq!(SourceLibraryInputs1027::read(&fixture.0).unwrap().unwrap().assets().len(),2);workspace.replay_refactor(&receipt,false).unwrap();assert_eq!(workspace.source_library_inputs1027().unwrap().assets(),&[asset(A,11)]);
}

#[cfg(unix)]
fn redirected_legacy1027()->(Fixture,PathBuf,PathBuf){
    let root=std::env::temp_dir().join(format!("rvn-library-redirect-legacy1027-{}",unique_name()));fs::create_dir(&root).unwrap();let fixture=Fixture(root);let project=fixture.0.join("project");fs::create_dir(&project).unwrap();let target=fixture.0.join("original-manifest.toml");fs::write(&target,"[paths]\nassets='/original/legacy-assets'\n").unwrap();std::os::unix::fs::symlink(&target,project.join("rvn.toml")).unwrap();fs::write(project.join("story.rvn"),"function caption(){return 1}\nscene label start\nset number=caption()\n\"After\"\n").unwrap();(fixture,project,target)
}
#[cfg(unix)]
#[test]
fn library_source1027_redirected_no_index_manifest_save_replay_and_recovery_never_write_alias(){
    let(_fixture,project,target)=redirected_legacy1027();let original=fs::read(&target).unwrap();let mut workspace=SourceWorkspace::link_scoped_libraries1027(&project,&project.join("story.rvn"),&[]).unwrap();let mut graphs=workspace.project().graphs().to_vec();let caption=graphs.iter_mut().find(|g|matches!(&g.kind,crate::GraphKind::Function{name}if name=="caption")).unwrap();let literal=caption.nodes.values().find(|n|n.kind==crate::NodeKind::Literal&&n.properties.get("value")==Some(&crate::PropertyValue::Int(1))).unwrap().id;caption.set_property(literal,"value",crate::PropertyValue::Int(9)).unwrap();
    let receipt=workspace.refactor(&graphs,&[]).unwrap();assert!(receipt.files.iter().all(|f|f.path!="rvn.toml"));assert!(receipt.library_before1027.is_some());workspace.replay_refactor(&receipt,false).unwrap();
    let journal=serde_json::json!({"source":"story.rvn","graphs":receipt.after_graphs,"files":receipt.files,"recovery":null,"library_before1027":receipt.library_before1027,"library_after1027":receipt.library_after1027});fs::write(project.join(refactor::REFACTOR_JOURNAL),serde_json::to_string(&journal).unwrap()).unwrap();drop(workspace);recover(&project).unwrap();let mut reopened=SourceWorkspace::open_scoped_libraries1027(&project,0).unwrap().unwrap();assert!(reopened.project().source().contains("return 9"));reopened.save(&reopened.project().graphs().to_vec()).unwrap();assert_eq!(fs::read(&target).unwrap(),original);assert!(fs::symlink_metadata(project.join("rvn.toml")).unwrap().file_type().is_symlink());assert!(!project.join("assets").exists());
}
#[cfg(unix)]
#[test]
fn library_source1027_redirected_manifest_cannot_gain_index_or_bypass_original_journal_observer(){
    let(_fixture,project,target)=redirected_legacy1027();let workspace=SourceWorkspace::link_scoped_libraries1027(&project,&project.join("story.rvn"),&[]).unwrap();let source=fs::read_to_string(project.join("story.rvn")).unwrap();let sidecar=fs::read_to_string(project.join(SIDECAR)).unwrap();let transaction=Transaction{library_readset1027:workspace.library_journal_readset1027(),reservation_archive:None,editorial_publication:None,source:"story.rvn".into(),before_source:source.clone(),after_source:source.clone(),before_sidecar:Some(sidecar.clone()),after_sidecar:sidecar.clone()};fs::write(project.join(JOURNAL),serde_json::to_string(&transaction).unwrap()).unwrap();
    fs::write(&target,"[paths]\nassets='assets'\n[function_libraries]\n").unwrap();assert!(SourceLibraryInputs1027::read(&project).is_err());assert!(recover(&project).is_err());assert_eq!(fs::read_to_string(project.join("story.rvn")).unwrap(),source);assert_eq!(fs::read_to_string(project.join(SIDECAR)).unwrap(),sidecar);assert!(project.join(JOURNAL).exists());
}
