use super::*;
use rvn_parser::enum_asset::{EnumAsset,EnumMemberId,EnumCommand};
const A:&str="329f41eae6ba4b4bb71dfaf60fb50001";
const M:&str="229f41eae6ba4b4bb71dfaf60fb50001";
fn resources()->(EnumAssets1025,BTreeMap<EnumAssetId,String>){
    let id=EnumAssetId::parse(A).unwrap();let asset=EnumAsset::new(id,"Current only".into()).unwrap().prepare(EnumCommand::Add{member:EnumMemberId::parse(M).unwrap(),display_name:"Unsaved member".into(),description:String::new()}).unwrap().after().clone();
    let path="draft/Current.rvnenum";let rows=[(A,Some(path))];let resources=EnumAssets1025::admit(EnumIndexInput::Table(&rows),&[(path.into(),asset.encode().unwrap().into_bytes())],true).unwrap();(resources,BTreeMap::from([(id,path.into())]))
}
fn program()->ParsedProgram{rvn_parser::parse_program(&format!("init{{set chosen=__rvn_enum_value_v1(\"{A}\",\"{M}\")}}\nscene label start\n\"Current\"\nreturn\n")).unwrap()}
#[test]
fn current_enum_transport1027_actual_admission_reaches_owned_init_without_a_saved_file(){
    let(resources,locators)=resources();let program=program();let captured=CapturedEnums1027::from_admitted(&resources,&locators).unwrap();let wire=captured.encode_for(&program,7).unwrap();let current=decode_for1027(&wire,&program,7).unwrap();
    let engine=crate::Engine::from_program_with_enum_assets1025(program,current,Default::default(),crate::TerminalRenderer,8,Default::default(),Default::default()).unwrap();
    assert_eq!(engine.state.vars.get("chosen"),Some(&rvn_parser::Value::Enum(resources.value(EnumAssetId::parse(A).unwrap(),EnumMemberId::parse(M).unwrap()).unwrap())));
}
#[test]
fn current_enum_transport1027_foreign_nonce_program_locator_and_uuid_refuse_before_engine(){
    let(resources,locators)=resources();let program=program();let captured=CapturedEnums1027::from_admitted(&resources,&locators).unwrap();let wire=captured.encode_for(&program,7).unwrap();assert!(decode_for1027(&wire,&program,8).is_err());
    let foreign=rvn_parser::parse_program("scene label start\nreturn\n").unwrap();assert!(decode_for1027(&wire,&foreign,7).is_err());
    for field in ["locator","uuid"]{let mut envelope:serde_json::Value=serde_json::from_slice(&wire).unwrap();envelope["current"][0][field]=if field=="locator"{"../Current.rvnenum"}else{"329f41eae6ba4b4bb71dfaf60fb50002"}.into();assert!(decode_for1027(&serde_json::to_vec(&envelope).unwrap(),&program,7).is_err());}
    assert!(CapturedEnums1027::from_admitted(&resources,&BTreeMap::new()).is_err());
}
#[test]
fn current_enum_transport1027_empty_projection_and_original_frozen_members_are_explicit(){
    let program=program();let plain=rvn_parser::parse_program("scene label start\nreturn\n").unwrap();let empty=CapturedEnums1027::from_admitted(&EnumAssets1025::default(),&BTreeMap::new()).unwrap();assert!(decode_for1027(&empty.encode_for(&plain,7).unwrap(),&plain,7).unwrap().is_empty());assert!(decode_for1027(&empty.encode_for(&program,7).unwrap(),&program,7).is_err());
    let(resources,locators)=resources();let original=CapturedEnums1027::from_admitted(&resources,&locators).unwrap();let id=EnumAssetId::parse(A).unwrap();let removed=resources.asset(id).unwrap().prepare(EnumCommand::Remove(EnumMemberId::parse(M).unwrap())).unwrap().after().clone();
    let rows=[(A,Some(locators[&id].as_str()))];let current=EnumAssets1025::admit(EnumIndexInput::Table(&rows),&[(locators[&id].clone(),removed.encode().unwrap().into_bytes())],true).unwrap();let latest=CapturedEnums1027::from_admitted(&current,&locators).unwrap();
    assert!(decode_for1027(&original.encode_for(&program,7).unwrap(),&program,7).unwrap().value(id,EnumMemberId::parse(M).unwrap()).is_ok());assert!(decode_for1027(&latest.encode_for(&program,8).unwrap(),&program,8).is_err());
    let bytes=original.encode_for(&program,7).unwrap();let seal=LaunchSeal1027::for_bytes(7,&bytes).unwrap();let argument=seal.argument();assert_eq!(launch_argument1027(vec![argument.clone()]).unwrap(),Some(seal.clone()));assert!(seal.validate_bytes(&bytes).is_ok());assert!(seal.validate_bytes(&latest.encode_for(&program,7).unwrap()).is_err());assert!(launch_argument1027(vec![argument.clone(),argument]).is_err());
    for args in [vec![ARGUMENT.trim_end_matches('=').into()],vec![format!("{ARGUMENT}0")],vec![format!("{ARGUMENT}01")],vec![format!("{ARGUMENT}1"),format!("{ARGUMENT}2")]]{assert!(nonce_argument1027(args).is_err());}
}
#[test]
fn current_enum_preflight1027_literal_missing_after_init_effect_refuses_before_startup_is_returned(){
    let source=format!("init{{set first_effect=1 set chosen=__rvn_enum_value_v1(\"{A}\",\"{M}\")}}\nscene label start\nreturn\n");let program=rvn_parser::parse_program(&source).unwrap();
    let refused=crate::Engine::prepare_enum_startup1025(program.clone(),EnumAssets1025::default(),Default::default(),crate::TerminalRenderer,8,Default::default(),Default::default(),None);
    let error=match refused{Err(error)=>error,Ok(_)=>panic!("missing literal must refuse before any pending Startup/Init execution is returned")};assert!(error.to_string().contains("Literal Enum reference refused before Init"));
    let legacy=crate::Engine::from_program(program.clone(),crate::TerminalRenderer,8);assert!(legacy.is_err(),"the legacy no-resources constructor cannot bypass literal admission");
    let(resources,_)=resources();let actual=crate::Engine::from_program_with_enum_assets1025(program,resources,Default::default(),crate::TerminalRenderer,8,Default::default(),Default::default()).unwrap();assert_eq!(actual.state.vars.get("first_effect"),Some(&rvn_parser::Value::Int(1)));assert!(matches!(actual.state.vars.get("chosen"),Some(rvn_parser::Value::Enum(_))));
}
#[test]
fn current_enum_preflight1027_dynamic_arguments_keep_actual_evaluator_admission(){
    let program=rvn_parser::parse_program(&format!("init{{set owner=\"{A}\" set member=\"{M}\" set chosen=__rvn_enum_value_v1(owner,member)}}\nscene label start\nreturn\n")).unwrap();
    assert!(crate::eval::validate_enum_program_literals1027(&program.script,&EnumAssets1025::default()).is_ok(),"static admission never predicts variables by executing Init");
    let pending=crate::Engine::prepare_enum_startup1025(program.clone(),EnumAssets1025::default(),Default::default(),crate::TerminalRenderer,8,Default::default(),Default::default(),None).unwrap();
    drop(pending); // Prepared Startup is admitted; use its existing public synchronous constructor for actual Init.
    let error=match crate::Engine::from_program_with_enum_assets1025(program.clone(),EnumAssets1025::default(),Default::default(),crate::TerminalRenderer,8,Default::default(),Default::default()){Err(error)=>error,Ok(_)=>panic!("dynamic evaluation must still enforce real resource admission")};
    assert!(error.to_string().contains("Enum asset is not admitted by this project's actual index"),"{error}");
    let(admitted,_)=resources();let expected=admitted.value(EnumAssetId::parse(A).unwrap(),EnumMemberId::parse(M).unwrap()).unwrap();
    let actual=crate::Engine::from_program_with_enum_assets1025(program,admitted,Default::default(),crate::TerminalRenderer,8,Default::default(),Default::default()).unwrap();
    assert_eq!(actual.state.vars.get("chosen"),Some(&rvn_parser::Value::Enum(expected)));
}

#[test]
fn current_enum_preflight1027_library_scope_and_both_qualified_argument_routes_visit_real_ast(){
    use rvn_parser::{Expr,Statement};
    use rvn_parser::function_library_asset1023::{FunctionLibraryId,QualifiedLibraryFunction};
    use rvn_parser::function_library_program1023::{FunctionLibraryScope,LibraryMemberSource,LibraryParameterId,QualifiedLibraryCall};
    let library=FunctionLibraryId::parse(A).unwrap();
    let literal=Expr::Call{name:"__rvn_enum_value_v1".into(),args:vec![Expr::Str(A.into()),Expr::Str(M.into())]};
    let scope=FunctionLibraryScope::new(library.clone(),vec![LibraryMemberSource::new(7,vec![],Statement::Function{name:"choose".into(),parameters:vec![],body:vec![Statement::FunctionReturn{value:literal.clone()}]}).unwrap()]).unwrap();
    let call=QualifiedLibraryCall::new(QualifiedLibraryFunction{library,graph_id:9},vec![(LibraryParameterId::parse(M).unwrap(),literal)]).unwrap();
    let (admitted,_)=resources();
    for statement in [Statement::FunctionLibrary{scope},Statement::SetVar{name:"chosen".into(),value:Expr::LibraryCall{call:call.clone()}},Statement::LibraryUnitCall{call}]{
        let script=[statement];let error=crate::eval::validate_enum_program_literals1027(&script,&EnumAssets1025::default()).unwrap_err();
        assert!(error.to_string().contains("Literal Enum reference refused before Init"));
        assert!(crate::eval::validate_enum_program_literals1027(&script,&admitted).is_ok());
    }
}
