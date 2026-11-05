//! Parsed-source validator contracts only; no runtime/Graph/native acceptance.
use super::*;
const SCOPE:&str="g772/n1:init|exec_out=1@0";
const EACH:&str="n2:for_each|exec_in=2@1,collection=3@0,body=4@0,completed=5@0,array_index=6@0,array_element=7@0";
fn native(body:&str)->String{format!("flow.consume_v3(\"{SCOPE}\",\"{EACH}\"){{for {} in [11]{{{body}}}}}",index_name(772,2))}
fn actual_binding()->Binding{
    let script=crate::parse(&native("")).unwrap();validate_authored_bindings(&script).unwrap();binding(&script[0]).unwrap().unwrap()
}
fn legacy(name:&str,range:bool,body:&str)->String{if range{format!("flow.for_loop({name},0,0){{{body}}}")}else{format!("for {name} in [9]{{{body}}}")}}
#[test]
fn native0772_legacy_iterator_and_range_targets_cannot_assign_any_actual_native_alias(){
    let owner=actual_binding();for name in [&owner.index,&owner.cursor,&owner.phase]{for range in [false,true]{
        for inside in [false,true]{
            let old=legacy(name,range,"");let source=if inside{format!("init{{{}}}",native(&old))}else{format!("init{{{} {old}}}",native(""))};
            let script=crate::parse(&source).expect("a legal historical variable spelling must reach the structural validator");
            let error=validate_authored_bindings(&script).expect_err("Legacy loop target is an authored write, not Native owner authority");assert!(error.contains("assignment collides"),"{error}: {source}");
        }
    }}
}
#[test]
fn native0772_legacy_only_alias_spellings_remain_admissible_without_a_declared_native_binding(){
    let owner=actual_binding();for name in [&owner.index,&owner.cursor,&owner.phase]{for range in [false,true]{
        let source=format!("init{{{} }}",legacy(name,range,&format!("set seen={name}")));let script=crate::parse(&source).unwrap();validate_authored_bindings(&script).unwrap();
    }}
}
#[test]
fn native0772_actual_v3_structural_index_and_different_legacy_targets_keep_their_body_scope(){
    let owner=actual_binding();for range in [false,true]{
        let body=legacy("ordinary_item",range,&format!("set seen={}",owner.index));let source=format!("init{{{}}}",native(&body));validate_authored_bindings(&crate::parse(&source).unwrap()).unwrap();
    }
    let source=format!("init{{{}}}",native(&format!("set seen={}",owner.index)));validate_authored_bindings(&crate::parse(&source).unwrap()).unwrap();
}
