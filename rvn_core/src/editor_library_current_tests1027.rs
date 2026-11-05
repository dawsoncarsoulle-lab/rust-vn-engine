//! Actual codec/program/Core consumers, with no host or native execution claim.
use super::*;
use rvn_parser::function_library_asset1023::{FunctionLibraryMember, LibraryFunctionAccess};
const A: &str = "11111111111141118111111111111111";
const B: &str = "22222222222242228222222222222222";
fn asset(owner: &str, graph: u64) -> FunctionLibraryAsset {
    FunctionLibraryAsset::new(FunctionLibraryId::parse(owner).unwrap(), vec![FunctionLibraryMember {
        graph_id: graph, access: LibraryFunctionAccess::Public, pure: true,
    }]).unwrap()
}
fn program(owner: &str, graph: u64, body: &str) -> ParsedProgram {
    rvn_parser::parse_program(&format!("functions.library_v1(\"{owner}\"){{functions.member_v1(\"{graph}\",[]){{function value(){{return {body}}}}}}}\nscene label start\nset actual=functions.value_v1(\"{owner}\",\"{graph}\",{{}})\n\"After\"\n")).unwrap()
}
fn captured(owner: &str, graph: u64) -> CapturedLibraries1027 {
    let asset = asset(owner, graph);
    CapturedLibraries1027::from_admitted(&[asset.clone()], &BTreeMap::from([(asset.id().clone(), "draft/Current.rvnlibrary".into())])).unwrap()
}
#[test]
fn current_library_transport1027_actual_current_codec_and_program_reach_the_same_core_without_saved_files() {
    let program = program(A, 11, "9");
    let captured = captured(A, 11);
    let bytes = captured.encode_for(&program, 7).unwrap();
    let assets = decode_for1027(&bytes, &program, 7).unwrap();
    assert_eq!(assets, captured.assets().unwrap());
    assert_eq!(assets[0].id().as_str(), A);
    assert_eq!(assets[0].members()[0].graph_id, 11);
    let prepared = crate::engine::PreparedFunctionLibraryProgram1023::prepare(program, &assets).unwrap();
    let mut engine = crate::Engine::from_function_library_program1023(prepared, crate::TerminalRenderer, 8).unwrap();
    engine.step_until_interaction().unwrap();
    assert_eq!(engine.state.vars.get("actual"), Some(&rvn_parser::Value::Int(9)));
}
#[test]
fn current_library_transport1027_original_byte_seal_refuses_a_valid_locator_replacement_and_nominal_corruption() {
    let program = program(A, 11, "9");
    let bytes = captured(A, 11).encode_for(&program, 7).unwrap();
    let original = LaunchSeal1027::for_bytes(7, &bytes).unwrap();
    let mut envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    envelope["current"][0]["locator"] = "other/Current.rvnlibrary".into();
    let changed = serde_json::to_vec(&envelope).unwrap();
    assert!(decode_for1027(&changed, &program, 7).is_ok(), "a locator is not a callable alias or Source authority");
    assert!(original.validate_bytes(&changed).is_err(), "the independently captured launch bytes remain authoritative");
    original.validate_bytes(&bytes).unwrap();
    for field in ["uuid", "locator", "wire"] {
        let mut bad: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        bad["current"][0][field] = match field { "uuid" => B.into(), "locator" => "../Current.rvnlibrary".into(), _ => "{}".into() };
        assert!(decode_for1027(&serde_json::to_vec(&bad).unwrap(), &program, 7).is_err());
    }
    let duplicate = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["current"][0].clone();
    let mut bad: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    bad["current"].as_array_mut().unwrap().push(duplicate);
    assert!(decode_for1027(&serde_json::to_vec(&bad).unwrap(), &program, 7).is_err());
    assert!(CapturedLibraries1027::from_admitted(&[asset(A, 11)], &BTreeMap::new()).is_err());
}
#[test]
fn current_library_transport1027_argument_kind_nonce_and_empty_current_are_explicit_without_fallback() {
    let plain = rvn_parser::parse_program("scene label start\nreturn\n").unwrap();
    let empty = CapturedLibraries1027::from_admitted(&[], &BTreeMap::new()).unwrap();
    let bytes = empty.encode_for(&plain, 7).unwrap();
    assert!(decode_for1027(&bytes, &plain, 7).unwrap().is_empty());
    let seal = LaunchSeal1027::for_bytes(7, &bytes).unwrap();
    let argument = seal.argument();
    assert_eq!(launch_argument1027(vec![argument.clone()]).unwrap(), Some(seal));
    assert!(launch_argument1027(Vec::new()).unwrap().is_none());
    assert!(launch_argument1027(vec![argument.clone(), argument]).is_err());
    for argument in [ARGUMENT.trim_end_matches('=').to_owned(), format!("{ARGUMENT}0:{}", "0".repeat(64)), format!("{ARGUMENT}07:{}", "0".repeat(64)), format!("{ARGUMENT}7:{}", "A".repeat(64)), format!("{ARGUMENT}7")] {
        assert!(launch_argument1027(vec![argument]).is_err());
    }
    assert!(decode_for1027(&bytes, &plain, 8).is_err());
    for (field, value) in [("kind", serde_json::json!("rvn.enum-current.v1")), ("version", serde_json::json!(2))] {
        let mut wrong: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        wrong[field] = value;
        assert!(decode_for1027(&serde_json::to_vec(&wrong).unwrap(), &plain, 7).is_err());
    }
    let scoped = program(A, 11, "9");
    assert!(empty.encode_for(&scoped, 7).is_err(), "explicit empty Current cannot acquire descriptors from declarations or Saved files");
}
#[test]
fn current_library_transport1027_exact_source_body_and_large_graph_identity_are_not_rewritten() {
    let graph = 9_007_199_254_740_993;
    let original = program(A, graph, "9");
    let captured = captured(A, graph);
    let bytes = captured.encode_for(&original, 7).unwrap();
    let admitted = decode_for1027(&bytes, &original, 7).unwrap();
    assert_eq!(admitted[0].members()[0].graph_id, graph);
    assert_eq!(admitted[0].encode().unwrap(), captured.assets().unwrap()[0].encode().unwrap());
    assert!(decode_for1027(&bytes, &program(A, graph, "10"), 7).is_err());
    assert!(decode_for1027(&bytes, &program(B, graph, "9"), 7).is_err());
    assert!(decode_for1027(&bytes, &program(A, graph + 1, "9"), 7).is_err());
}

#[test]
fn library_presence1027_exact_scopes_calls_hidden_expressions_and_macro_captures_require_resources(){
    let scoped=program(A,11,"9");assert!(program_requires_libraries1027(&scoped).unwrap());
    for source in [
        format!("scene label start\nset value=[false,functions.value_v1(\"{A}\",\"11\",{{}})]\nreturn\n"),
        format!("init{{if false{{functions.invoke_v1(\"{A}\",\"11\",{{}})}}}}\nscene label start\nreturn\n"),
        format!("init{{flow.consume_v1(\"scope\",\"consumer\"){{capture \"value\"=functions.value_v1(\"{A}\",\"11\",{{}}) set actual=__rvn_captured_v1(\"value\")}}}}\nscene label start\nreturn\n"),
    ]{let parsed=rvn_parser::parse_program(&source).unwrap();assert!(program_requires_libraries1027(&parsed).unwrap(),"nominal use cannot hide in a branch or operand");}
}
#[test]
fn library_presence1027_text_names_are_not_calls_and_empty_metadata_is_not_an_ast_claim(){
    let program=rvn_parser::parse_program("init{set value=\"functions.library_v1 functions.value_v1 LibraryUnitCall\"}\nscene label start\nreturn\n").unwrap();assert!(!program_requires_libraries1027(&program).unwrap());
    let empty=FunctionLibraryAsset::new(FunctionLibraryId::parse(A).unwrap(),vec![]).unwrap();assert!(empty.members().is_empty());
    assert!(!program_requires_libraries1027(&program).unwrap(),"the AST result deliberately says nothing about the separate actual empty metadata owner");
    let scoped=rvn_parser::parse_program(&format!("functions.library_v1(\"{A}\"){{}}\nscene label start\nreturn\n")).unwrap();assert!(program_requires_libraries1027(&scoped).unwrap());
}
