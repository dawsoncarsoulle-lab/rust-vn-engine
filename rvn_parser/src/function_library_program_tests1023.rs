use super::*;
use crate::function_library_asset1023::{FunctionLibraryMember,LibraryFunctionAccess};
use crate::parser::function_libraries1023::{parse_library_scope,parse_library_call};
fn id(value:u64)->FunctionLibraryId{FunctionLibraryId::parse(format!("{value:032x}")).unwrap()}
fn parameter(value:u64)->LibraryParameterId{LibraryParameterId::parse(format!("{value:032x}")).unwrap()}
fn unit_scope(library:u64,graph:u64,param:u64,name:&str,increment:i64)->String{
    format!("functions.library_v1(\"{library:032x}\") {{\nfunctions.member_v1(\"{graph}\",[\"{param:032x}\"]) {{\nfunction {name}(value) -> () {{set total=total+value+{increment} return}}\n}}\n}}\n")
}
fn asset(library:u64,graph:u64,access:LibraryFunctionAccess,pure:bool)->FunctionLibraryAsset{
    FunctionLibraryAsset::new(id(library),vec![FunctionLibraryMember{graph_id:graph,access,pure}]).unwrap()
}
fn call(library:u64,graph:u64,param:u64,value:&str)->String{
    format!("functions.invoke_v1(\"{library:032x}\",\"{graph}\",{{\"{param:032x}\":{value}}})")
}
#[test]
fn library_source1023_real_parser_scope_roundtrip_keeps_uuid_graph_parameter_ids_and_actual_unit_body(){
    let source=unit_scope(1,41,101,"NewFunction",1);let scope=parse_library_scope(&source).unwrap();
    assert_eq!(scope.library(),&id(1));let member=&scope.members()[0];assert_eq!(member.graph_id(),41);assert_eq!(member.parameter_ids(),[parameter(101)]);
    let declaration="function NewFunction(value) -> () {set total=total+value+1 return}";
    assert_eq!(member.declaration(),&crate::parse(declaration).unwrap()[0]);
    let encoded=emit_library_scope(&scope,|_|Ok(declaration.into())).unwrap();
    assert_eq!(parse_library_scope(&encoded).unwrap(),scope);
    assert!(emit_library_scope(&scope,|_|Ok("function NewFunction(value) -> () {set total=999 return}".into())).is_err(),"an equal display name cannot substitute another source body");
    assert_eq!(parse_library_scope(&source).unwrap(),scope,"emission cannot mutate original Source");
}
#[test]
fn library_source1023_two_library_owners_can_have_same_display_name_and_calls_bind_only_original_graph_and_scope(){
    let scopes=[parse_library_scope(&unit_scope(1,41,101,"shared",1)).unwrap(),parse_library_scope(&unit_scope(2,42,102,"shared",2)).unwrap()];
    let assets=[asset(1,41,LibraryFunctionAccess::Public,false),asset(2,42,LibraryFunctionAccess::Public,false)];
    let bindings=ScopedLibraryBindings::prepare(&scopes,&assets).unwrap();
    let(mode,a)=parse_library_call(&call(1,41,101,"random(1,9)")).unwrap();
    let(_,b)=parse_library_call(&call(2,42,102,"random(1,9)")).unwrap();
    let first=bindings.resolve(&a,mode,None).unwrap();let second=bindings.resolve(&b,mode,None).unwrap();
    assert_eq!(first.member.name(),second.member.name());assert_ne!(first.member.declaration(),second.member.declaration());
    assert!(std::ptr::eq(first.member,&scopes[0].members()[0]));assert!(std::ptr::eq(second.member,&scopes[1].members()[0]));
    assert!(std::ptr::eq(first.arguments[0],&a.arguments()[0].1),"resolution borrows the original expression; no argument evaluation/copy");
    let(_,wrong)=parse_library_call(&call(2,41,101,"random(1,9)")).unwrap();
    assert!(bindings.resolve(&wrong,mode,None).is_err(),"graph41 cannot borrow same-name graph42 in another owner");
}
#[test]
fn library_source1023_parameter_mapping_is_by_id_and_real_member_rename_does_not_retarget_qualified_call(){
    let source=format!("functions.library_v1(\"{:032x}\"){{functions.member_v1(\"41\",[\"{:032x}\",\"{:032x}\"]){{function original(first,second)->(){{set total=first*10+second return}}}}}}",1,101,102);
    let scope=parse_library_scope(&source).unwrap();let renamed=parse_library_scope(&source.replace("original(","renamed(")).unwrap();
    let assets=[asset(1,41,LibraryFunctionAccess::Public,false)];
    let wire=format!("functions.invoke_v1(\"{:032x}\",\"41\",{{\"{:032x}\":7,\"{:032x}\":4}})",1,102,101);
    let(mode,call)=parse_library_call(&wire).unwrap();let old_scopes=[scope];let new_scopes=[renamed];
    let old=ScopedLibraryBindings::prepare(&old_scopes,&assets).unwrap();let new=ScopedLibraryBindings::prepare(&new_scopes,&assets).unwrap();
    let before=old.resolve(&call,mode,None).unwrap();let after=new.resolve(&call,mode,None).unwrap();
    assert_eq!(before.arguments,[&Expr::Int(4),&Expr::Int(7)]);assert_eq!(before.arguments,after.arguments);
    assert_eq!(before.member.graph_id(),after.member.graph_id());assert_eq!(after.member.name(),"renamed");
    let(_,foreign)=parse_library_call(&wire.replace(&format!("{:032x}",102),&format!("{:032x}",103))).unwrap();
    assert!(new.resolve(&foreign,mode,None).is_err(),"same arity never retargets a parameter identity");
}
#[test]
fn library_source1023_private_visibility_and_result_modes_keep_actual_caller_scope_distinct_from_name(){
    let scopes=[parse_library_scope(&unit_scope(1,41,101,"same",1)).unwrap(),parse_library_scope(&unit_scope(2,42,102,"same",2)).unwrap()];
    let assets=[asset(1,41,LibraryFunctionAccess::Private,false),asset(2,42,LibraryFunctionAccess::Public,false)];let bindings=ScopedLibraryBindings::prepare(&scopes,&assets).unwrap();
    let(mode,call)=parse_library_call(&call(1,41,101,"4")).unwrap();
    assert!(bindings.resolve(&call,mode,None).is_err());
    let own=QualifiedLibraryFunction{library:id(1),graph_id:41};let foreign=QualifiedLibraryFunction{library:id(2),graph_id:42};
    assert!(bindings.resolve(&call,mode,Some(&own)).is_ok());assert!(bindings.resolve(&call,mode,Some(&foreign)).is_err());
    assert!(bindings.resolve(&call,LibraryCallMode::Value,Some(&own)).is_err(),"Unit completion cannot masquerade as a value");
    let missing=QualifiedLibraryFunction{library:id(1),graph_id:99};assert!(bindings.resolve(&call,mode,Some(&missing)).is_err(),"caller Library alone is not a real member");
}
#[test]
fn library_source1023_malformed_members_duplicates_nested_narrative_and_asset_mismatch_are_refused(){
    let good=unit_scope(1,41,101,"function_one",1);
    for source in [good.replace("\"41\"","\"0\""),good.replace("\"41\"","\"041\""),good.replace("function function_one(value) -> () {set total=total+value+1 return}","handler foreign(event) {local accepted=true}"),good.replace("function function_one(value) -> () {set total=total+value+1 return}","scene label foreign\nreturn"),good.replace("[\"00000000000000000000000000000065\"]","[]"),good.replace("library_v1","library_v2")]{assert!(parse_library_scope(&source).is_err(),"{source}");}
    let scope=parse_library_scope(&good).unwrap();
    assert!(ScopedLibraryBindings::prepare(&[scope.clone()],&[asset(1,42,LibraryFunctionAccess::Public,false)]).is_err());
    assert!(ScopedLibraryBindings::prepare(&[scope.clone(),scope.clone()],&[asset(1,41,LibraryFunctionAccess::Public,false)]).is_err());
    let other=parse_library_scope(&unit_scope(2,41,102,"other",2)).unwrap();
    assert!(ScopedLibraryBindings::prepare(&[scope,other],&[asset(1,41,LibraryFunctionAccess::Public,false),asset(2,41,LibraryFunctionAccess::Public,false)]).is_err(),"Source GraphId cannot belong to two Library owners");
    let empty=FunctionLibraryAsset::new(id(3),Vec::new()).unwrap();assert!(ScopedLibraryBindings::prepare(&[],&[empty]).is_ok(),"empty metadata owner is valid before first editor open");
}
#[test]
fn library_source1023_dynamic_identity_duplicate_arguments_and_residual_input_never_form_typed_calls(){
    let good=call(1,41,101,"4");
    let bad=[good.replace(&format!("\"{:032x}\"",1),"selected_library"),good.replace("\"41\"","41"),good.replace("\"41\"","\"041\""),good.replace("invoke_v1","invoke_v2"),good.replace(&format!("\"{:032x}\":4",101),&format!("\"{:032x}\":4,\"{:032x}\":7",101,101)),format!("{good}\nset other=7")];
    for source in bad{assert!(parse_library_call(&source).is_err(),"{source}");}
    let (_,parsed)=parse_library_call(&good).unwrap();let mut value=serde_json::to_value(&parsed).unwrap();value["reference"]["graph_id"]=serde_json::json!(0);
    assert!(serde_json::from_value::<QualifiedLibraryCall>(value).is_err(),"raw AST DTO cannot bypass nonzero identity validation");
}
#[test]
fn library_source1023_real_lexer_handles_comments_unicode_crlf_and_braces_in_strings_without_textual_scope_search(){
    let source=format!("// functions.library_v1(\"fake\") {{\r\nfunctions.library_v1(\"{:032x}\"){{\r\nfunctions.member_v1(\"41\",[]){{\r\nfunction display(){{return \"é {{ }} functions.member_v1\"}}\r\n}}\r\n}}\r\n",1);
    let scope=parse_library_scope(&source).unwrap();assert_eq!(scope.members()[0].name(),"display");
    assert!(matches!(scope.members()[0].declaration(),Statement::Function{body,..}if matches!(body.as_slice(),[Statement::FunctionReturn{value:Expr::Str(text)}]if text=="é { } functions.member_v1")));
    let assets=[asset(1,41,LibraryFunctionAccess::Public,true)];let scopes=[scope];let bindings=ScopedLibraryBindings::prepare(&scopes,&assets).unwrap();
    let(mode,call)=parse_library_call(&format!("functions.value_v1(\"{:032x}\",\"41\",{{}})",1)).unwrap();assert!(bindings.resolve(&call,mode,None).is_ok());
    assert!(bindings.resolve(&call,LibraryCallMode::Completion,None).is_err());
}

#[test]
fn library_source1023_source_bytes_and_existing_signature_bound_precede_unbounded_collection(){
    let oversized=" ".repeat(MAX_FUNCTION_LIBRARY_BYTES+1);
    assert!(parse_library_scope(&oversized).is_err());
    assert!(parse_library_call(&oversized).is_err());
    let arguments=(1..=MAX_FUNCTION_LIBRARY_PARAMETERS).map(|n|format!("\"{n:032x}\":{n}")).collect::<Vec<_>>().join(",");
    let exact=format!("functions.invoke_v1(\"{:032x}\",\"41\",{{{arguments}}})",1);
    assert_eq!(parse_library_call(&exact).unwrap().1.arguments().len(),MAX_FUNCTION_LIBRARY_PARAMETERS);
    let excessive=exact.replace("})",&format!(",\"{:032x}\":129}})",MAX_FUNCTION_LIBRARY_PARAMETERS+1));
    assert!(parse_library_call(&excessive).is_err());
    let scope=parse_library_scope(&unit_scope(1,41,101,"unit",1)).unwrap();
    assert!(ScopedLibraryBindings::prepare(&[scope],&[asset(1,41,LibraryFunctionAccess::Public,true)]).is_err(),"Pure metadata cannot contradict a completion-only declaration before any caller resolves");
}
