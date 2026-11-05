//! Actual Parser -> one computation/continuation -> same budgets and RNG.
use super::*;
use rvn_parser::function_library_asset1023::{FunctionLibraryAsset,FunctionLibraryId,FunctionLibraryMember,LibraryFunctionAccess as Access};
use rvn_parser::function_library_program1023::QualifiedLibraryCall;
const A:&str="11111111111141118111111111111111";
const B:&str="22222222222242228222222222222222";
const P:&str="33333333333343338333333333333333";
const Q:&str="44444444444444448444444444444444";
fn asset(id:&str,rows:&[(u64,Access,bool)])->FunctionLibraryAsset{FunctionLibraryAsset::new(FunctionLibraryId::parse(id).unwrap(),rows.iter().map(|(graph_id,access,pure)|FunctionLibraryMember{graph_id:*graph_id,access:*access,pure:*pure}).collect()).unwrap()}
fn expression(source:&str)->Expr{let parsed=rvn_parser::parse(&format!("set result={source}")).unwrap();let Statement::SetVar{value,..}=&parsed[0]else{panic!("actual expression")};value.clone()}
fn call(source:&str)->QualifiedLibraryCall{let parsed=rvn_parser::parse(source).unwrap();let Statement::LibraryUnitCall{call}=&parsed[0]else{panic!("actual qualified Unit")};call.clone()}
fn complete(pending:&mut Continuation)->Vec<String>{let mut scopes=Vec::new();for _ in 0..300_000{if pending.done(){return scopes;}for frame in &pending.frames{if let Some(scope)=&frame.function{if !scopes.contains(scope){scopes.push(scope.clone());}}}if pending.step().is_err(){return scopes;}}panic!("same bounded continuation did not complete")}
#[test]fn libraries_live1023_two_homonyms_private_actual_frame_and_rng_match_same_computation(){
 let source=format!(r#"functions.library_v1("{A}"){{functions.member_v1("11",["{P}"]){{function same(n){{return functions.value_v1("{A}","12",{{"{Q}":n}})}}}} functions.member_v1("12",["{Q}"]){{function private_value(n){{return n+random(1,9)}}}}}}
 functions.library_v1("{B}"){{functions.member_v1("22",[]){{function same(){{return legacy()}}}}}}
 function legacy(){{return random(10,19)}}"#);
 let program=rvn_parser::parse_program(&source).unwrap();let assets=[asset(A,&[(11,Access::Public,true),(12,Access::Private,true)]),asset(B,&[(22,Access::Public,true)])];let library=FunctionLibrary::from_script_with_library_assets(&program.script,&assets).unwrap();
 let expression=expression(&format!("functions.value_v1(\"{A}\",\"11\",{{\"{P}\":random(20,29)}})+functions.value_v1(\"{B}\",\"22\",{{}})"));let globals=HashMap::new();
 let mut normal=Computation::new(&library,&globals);normal.random=RandomState::seeded(771);let expected=normal.eval(&expression,&globals,0).unwrap();
 let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());let mut random=RandomState::seeded(771);
 assert!(debug.eval_with_random(&expression,&globals,&mut random).is_err());assert_eq!(random,RandomState::seeded(771));
 let mut pending=Probe::pending(&probe,&library).unwrap().unwrap();let scopes=complete(&mut pending);assert_eq!(pending.error,None);assert_eq!(pending.result,Some(expected.clone()));assert_eq!(pending.state.random,normal.random);assert_eq!(pending.work(),(normal.steps,normal.value_work));assert_eq!(pending.frames.len(),1);assert_eq!(pending.state.calls,0);
 for key in [format!("library:{A}:11"),format!("library:{A}:12"),format!("library:{B}:22"),"legacy".into()]{assert!(scopes.contains(&key),"actual frame missing {key}");}
 assert!(Probe::portable_trace(&probe,&pending).is_err(),"portable Library frames are not yet a qualified Save format");
 Probe::finish(&probe,&pending).unwrap();Probe::rewind(&probe).unwrap();assert_eq!(debug.eval_with_random(&expression,&globals,&mut random).unwrap(),expected);assert_eq!(random,normal.random);assert!(Probe::complete(&probe));
}
#[test]fn libraries_live1023_shared_unit_stack_depth_step_rng_and_failure_match_without_publication(){
 let source=format!(r#"functions.library_v1("{A}"){{functions.member_v1("11",["{P}"]){{function same(n)->(){{set total=total+random(1,9) if n>0{{functions.invoke_v1("{B}","22",{{"{Q}":n-1}})}}}}}}}}
 functions.library_v1("{B}"){{functions.member_v1("22",["{Q}"]){{function same(n)->(){{functions.invoke_v1("{A}","11",{{"{P}":n}})}}}}}}"#);
 let library=FunctionLibrary::from_script_with_library_assets(&rvn_parser::parse_program(&source).unwrap().script,&[asset(A,&[(11,Access::Public,false)]),asset(B,&[(22,Access::Public,false)])]).unwrap();let globals=HashMap::from([("total".into(),Value::Int(0))]);
 for depth in [0,31,32]{
  let invocation=call(&format!("functions.invoke_v1(\"{A}\",\"11\",{{\"{P}\":{depth}}})"));
  let mut normal=Computation::new(&library,&globals);normal.random=RandomState::seeded(72);let expected=normal.invoke_library_unit_expressions1023(&invocation,&globals);
  assert_eq!(expected.is_ok(),depth<32);
  let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());let mut random=RandomState::seeded(72);
  assert!(debug.invoke_library_unit_scoped1023(&invocation,&globals,&globals,&mut random,None).is_err());assert_eq!(random,RandomState::seeded(72));
  let mut pending=Probe::pending(&probe,&library).unwrap().unwrap();complete(&mut pending);assert_eq!(pending.error,expected.err());assert_eq!(pending.state.globals,normal.globals);assert_eq!(pending.state.random,normal.random);assert_eq!(pending.work(),(normal.steps,normal.value_work));
  Probe::finish(&probe,&pending).unwrap();Probe::rewind(&probe).unwrap();let outcome=debug.invoke_library_unit_scoped1023(&invocation,&globals,&globals,&mut random,None);
  if depth<32{let(values,_)=outcome.unwrap();assert_eq!(values,normal.globals);assert_eq!(random,normal.random);}else{assert!(outcome.is_err());assert_eq!(random,RandomState::seeded(72));}
 }
}
#[test]fn libraries_live1023_real_cross_owner_private_body_rejected_even_when_caller_and_target_exist(){
 let source=format!(r#"functions.library_v1("{A}"){{functions.member_v1("11",["{P}"]){{function secret(n){{return n}}}}}}
 functions.library_v1("{B}"){{functions.member_v1("22",[]){{function actual_other_library(){{return functions.value_v1("{A}","11",{{"{P}":random(1,9)}})}}}}}}"#);
 let script=rvn_parser::parse_program(&source).unwrap().script;
 assert!(FunctionLibrary::from_script_with_library_assets(&script,&[asset(A,&[(11,Access::Private,true)]),asset(B,&[(22,Access::Public,true)])]).is_err());
}
