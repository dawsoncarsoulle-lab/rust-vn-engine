//! Explicit initial Link: original Source parameter IDs and existing allocator.
//! CPU definitions only; no typed-parameter D expansion or native activation.
use super::*;
use rvn_parser::function_library_asset1023::{
    FunctionLibraryAsset, FunctionLibraryId, FunctionLibraryMember, LibraryFunctionAccess,
};
const A: &str = "11111111111141118111111111111111";
const P: &str = "33333333333343338333333333333333";
fn asset() -> FunctionLibraryAsset {
    FunctionLibraryAsset::new(
        FunctionLibraryId::parse(A).unwrap(),
        vec![FunctionLibraryMember {
            graph_id: 90,
            access: LibraryFunctionAccess::Public,
            pure: true,
        }],
    )
    .unwrap()
}
fn source() -> String {
    format!("functions.library_v1(\"{A}\"){{functions.member_v1(\"90\",[\"{P}\"]){{function same(n){{return n+1}}}}}}\ninit {{set copied=0}}\nfunction apply(value) -> () {{set copied=functions.value_v1(\"{A}\",\"90\",{{\"{P}\":value}})}}\nscene label start\ncall apply(3)\n\"After\"\nreturn\n")
}
fn scopes(script: &Script) -> Vec<rvn_parser::function_library_program1023::FunctionLibraryScope> {
    script
        .iter()
        .filter_map(|s| {
            if let Statement::FunctionLibrary { scope } = s {
                Some(scope.clone())
            } else {
                None
            }
        })
        .collect()
}
fn ids(graphs: &[GraphDocument]) -> Vec<crate::ParameterId> {
    let unit = graphs
        .iter()
        .find(|g| matches!(&g.kind,GraphKind::UnitFunction{name}if name=="apply"))
        .unwrap();
    crate::unit_graph_source1003::parameters(unit)
        .unwrap()
        .into_iter()
        .map(|p| p.parameter_id.clone().unwrap())
        .collect()
}

#[test]
fn library_initial_unit1027_real_scoped_body_allocates_only_original_ordinary_unit_pins_and_cold_reopens(
) {
    let source = source();
    let script = rvn_parser::parse(&source).unwrap();
    let actual_scopes = scopes(&script);
    let assets = [asset()];
    let resources = crate::LibraryGraphResources1027::prepare(&actual_scopes, &assets).unwrap();
    let mut allocations = 0;
    let prepared = prepare_initial_unit_link_graphs_scoped1027(&script, &[], &resources, || {
        allocations += 1;
        Ok(format!("{:032x}", 0x9000 + allocations))
    })
    .unwrap();
    assert_eq!(allocations,1,"Library parameter P was already Source-owned; only the actual ordinary Unit parameter is fresh");
    let original_ids = ids(&prepared);
    assert_eq!(original_ids[0].as_str(), format!("{:032x}", 0x9001));
    let member = prepared.iter().find(|g| g.graph_id.get() == 90).unwrap();
    assert_eq!(
        member.library1027.as_ref().unwrap().entry_parameter_ids[0].as_str(),
        P
    );
    let unit = prepared
        .iter()
        .find(|g| matches!(g.kind, GraphKind::UnitFunction { .. }))
        .unwrap();
    assert_eq!(
        unit.schema_version,
        crate::GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION
    );
    assert!(crate::unit_graph_source1003::parameters(unit)
        .unwrap()
        .iter()
        .all(|pin| pin.value_type == crate::ValueType::Any && pin.default_value.is_none()));
    let compiled = crate::transpile_library_project1027(&prepared, &resources).unwrap();
    assert!(compiled.source.contains("functions.value_v1"));
    assert!(!compiled
        .ast
        .iter()
        .any(|s| matches!(s,Statement::Function{name,..}if name=="same")));
    assert_eq!(scopes(&compiled.ast), actual_scopes);
    let folder = std::env::temp_dir().join(format!(
        "rvn-library-unit-initial1027-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&folder).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(folder.clone());
    std::fs::create_dir(folder.join("assets")).unwrap();
    std::fs::write(
        folder.join("rvn.toml"),
        format!("[function_libraries]\n\"{A}\"='one.rvnlibrary'\n"),
    )
    .unwrap();
    std::fs::write(
        folder.join("assets/one.rvnlibrary"),
        asset().encode().unwrap(),
    )
    .unwrap();
    let path = folder.join("main.rvn");
    std::fs::write(&path, &source).unwrap();
    let workspace =
        crate::SourceWorkspace::link_scoped_libraries1027(&folder, &path, &prepared).unwrap();
    assert_eq!(ids(workspace.project().graphs()), original_ids);
    drop(workspace);
    let reopened = crate::SourceWorkspace::open_scoped_libraries1027(&folder, 0)
        .unwrap()
        .unwrap();
    assert_eq!(ids(reopened.project().graphs()), original_ids);
    assert_eq!(
        reopened
            .project()
            .graphs()
            .iter()
            .find(|g| g.graph_id.get() == 90)
            .unwrap()
            .library1027
            .as_ref()
            .unwrap()
            .entry_parameter_ids[0]
            .as_str(),
        P
    );
}
#[test]
fn library_initial_unit1027_existing_unit_owner_and_wrong_scopes_refuse_before_allocator_or_model_mutation(
) {
    let script = rvn_parser::parse(&source()).unwrap();
    let actual = scopes(&script);
    let assets = [asset()];
    let resources = crate::LibraryGraphResources1027::prepare(&actual, &assets).unwrap();
    let previous = prepare_initial_unit_link_graphs_scoped1027(&script, &[], &resources, || {
        Ok(format!("{:032x}", 0x9001))
    })
    .unwrap();
    let before = serde_json::to_vec(&previous).unwrap();
    let mut called = 0;
    let refusal =
        prepare_initial_unit_link_graphs_scoped1027(&script, &previous, &resources, || {
            called += 1;
            Ok(format!("{:032x}", 0x9002))
        })
        .unwrap_err();
    assert!(refusal.contains("owning sidecar"));
    assert_eq!(called, 0);
    assert_eq!(serde_json::to_vec(&previous).unwrap(), before);
    let changed = rvn_parser::parse(&source().replace("return n+1", "return n+2")).unwrap();
    let changed_scopes = scopes(&changed);
    let wrong = crate::LibraryGraphResources1027::prepare(&changed_scopes, &assets).unwrap();
    assert!(
        prepare_initial_unit_link_graphs_scoped1027(&script, &[], &wrong, || {
            called += 1;
            Ok(format!("{:032x}", 0x9003))
        })
        .unwrap_err()
        .contains("actual original Library Source scopes")
    );
    assert_eq!(called, 0);
}
#[test]
fn library_initial_unit1027_no_library_delegates_existing_unit14_route_without_schema_promotion() {
    for prefix in ["", "init {set held=__rvn_enum_value_v1(\"329f41eae6ba4b4bb71dfaf60fb50001\",\"229f41eae6ba4b4bb71dfaf60fb50001\")}\n"]{
        let script=rvn_parser::parse(&format!("{prefix}function apply(value) -> () {{set copied=value}}\nscene label start\ncall apply(3)\n\"After\"\nreturn\n")).unwrap();let empty=crate::LibraryGraphResources1027::prepare(&[],&[]).unwrap();let old=prepare_initial_unit_link_graphs(&script,&[],||Ok(format!("{:032x}",0x9001))).unwrap();let new=prepare_initial_unit_link_graphs_scoped1027(&script,&[],&empty,||Ok(format!("{:032x}",0x9001))).unwrap();assert_eq!(new,old);assert!(new.iter().all(|g|g.library1027.is_none()&&g.schema_version<=crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION));let unit=new.iter().find(|g|matches!(g.kind,GraphKind::UnitFunction{..})).unwrap();assert!([crate::GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION,crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION].contains(&unit.schema_version));if prefix.is_empty(){assert_eq!(unit.schema_version,crate::GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION);}else{assert!(new.iter().any(|g|g.schema_version==crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION));}
    }
}
#[test]
fn library_initial_unit1027_new_unit_with_prior_scene_keeps_original_scene_identity() {
    let script = rvn_parser::parse(&source()).unwrap();
    let actual = scopes(&script);
    let assets = [asset()];
    let resources = crate::LibraryGraphResources1027::prepare(&actual, &assets).unwrap();
    let mut prior =
        crate::import_script(&rvn_parser::parse("scene label start\n\"After\"\nreturn\n").unwrap())
            .unwrap();
    let scene = prior
        .iter_mut()
        .find(|g| matches!(g.kind, GraphKind::Label { .. }))
        .unwrap();
    scene.graph_id = GraphId::new(5);
    let before = serde_json::to_vec(&prior).unwrap();
    let mut called = 0;
    let prepared = prepare_initial_unit_link_graphs_scoped1027(&script, &prior, &resources, || {
        called += 1;
        Ok(format!("{:032x}", 0x9001))
    })
    .unwrap();
    assert_eq!(called, 1);
    assert_eq!(
        prepared
            .iter()
            .find(|g| matches!(&g.kind,GraphKind::Label{name}if name=="start"))
            .unwrap()
            .graph_id,
        GraphId::new(5)
    );
    assert_eq!(serde_json::to_vec(&prior).unwrap(), before);
    assert!(
        prepared
            .iter()
            .find(|g| matches!(g.kind, GraphKind::UnitFunction { .. }))
            .unwrap()
            .graph_id
            .get()
            > 90
    );
}
#[test]
fn library_initial_unit1027_readonly_import_and_nominal_graphid_collision_refuse_without_allocating(
) {
    let script = rvn_parser::parse(&source()).unwrap();
    let actual = scopes(&script);
    let assets = [asset()];
    let resources = crate::LibraryGraphResources1027::prepare(&actual, &assets).unwrap();
    let previous = vec![GraphDocument::new(
        GraphId::new(90),
        GraphKind::Label {
            name: "start".into(),
        },
    )];
    let before = serde_json::to_vec(&previous).unwrap();
    let mut called = 0;
    assert!(
        prepare_initial_unit_link_graphs_scoped1027(&script, &previous, &resources, || {
            called += 1;
            Err("allocator must not run".into())
        })
        .unwrap_err()
        .contains("collides")
    );
    assert_eq!(called, 0);
    assert_eq!(serde_json::to_vec(&previous).unwrap(), before);
    let mut imported = script;
    imported.insert(
        0,
        Statement::Use {
            paths: vec!["readonly.rvn".into()],
        },
    );
    assert!(
        prepare_initial_unit_link_graphs_scoped1027(&imported, &[], &resources, || {
            called += 1;
            Err("allocator must not run".into())
        })
        .is_err()
    );
    assert_eq!(called, 0);
}
