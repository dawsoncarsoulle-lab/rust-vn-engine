use super::*;
use std::{path::PathBuf,sync::atomic::{AtomicU64,Ordering}};
const SCOPE:&str="g732/n1:label|name=1@0,exec_out=2@0";
const EACH:&str="n2:for_each|exec_in=3@1,collection=4@0,body=5@0,completed=6@0,array_index=7@0,array_element=8@0";
const INNER:&str="n3:for_each|exec_in=9@1,collection=10@0,body=11@0,completed=12@0,array_index=13@0,array_element=14@0";
fn parsed(source:&str)->rvn_parser::ParsedProgram{
    static NEXT:AtomicU64=AtomicU64::new(0);
    let base=PathBuf::from(std::env::var("RVN_NATIVE_NARRATIVE0732_FIXTURES").expect("isolated D fixture root required"));
    let root=base.join(format!("metadata-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir_all(&root).unwrap();let path=root.join("source.rvn");std::fs::write(&path,source).unwrap();rvn_parser::parse_file_program_with_uses(path).unwrap()
}
fn each(identity:&str,index:&str,body:&str)->String{format!("flow.consume_v3(\"{SCOPE}\",\"{identity}\"){{for {index} in [11,22]{{{body}}}}}")}
#[test]
fn narrative0732_preparation_retains_authored_header_and_real_generated_body(){
    let source=format!("scene label start\n{}\n\"Done\"\n",each(EACH,"rvn_native_index_g732_n2","set seen=rvn_native_index_g732_n2 \"[seen]\""));let program=parsed(&source);let authored=program.script.clone();let source_binding=crate::consumer_schedule::ConsumerSourceBinding::from_program(&program).unwrap();let prepared=PreparedNarrative::from_parsed(program).unwrap();
    assert_eq!(prepared.native_loops.authored(),Some(authored.as_slice()));let (pc,header)=prepared.script.iter().enumerate().find(|(_,s)|matches!(s,Statement::ScheduledConsumer{version:3,..})).unwrap();assert_eq!(header,&authored[1]);let body=prepared.native_loops.body(pc).unwrap();assert_eq!(body.binding.index,"rvn_native_index_g732_n2");assert_eq!(body.header,prepared.origins[pc]);assert_eq!(prepared.label_table[&body.body_target],body.body_label);assert!(prepared.branch_returns.contains(&body.body_return));assert!(matches!(&prepared.script[body.body_label],Statement::Label{name}if name==&body.body_target));assert!(matches!(prepared.script[body.body_return],Statement::Return));
    assert!(prepared.origins[body.body_label+1].route.starts_with(&body.body_route));assert!(prepared.temporary_writes.is_empty());assert!(!prepared.origins.iter().any(|o|matches!(o.generated,Some(GeneratedRole::ForCollection|GeneratedRole::ForItem|GeneratedRole::ForAdvance))));
    native_loop_contract::validate_authored_bindings(prepared.native_loops.authored().unwrap()).unwrap();assert!(native_loop_contract::validate_authored_bindings(&prepared.script).is_err(),"flat compiled body is not a substitute for the authenticated original lexical AST");
    assert!(crate::consumer_schedule::ConsumerProgram::new_with_source(&prepared.script,Some(&source_binding)).is_err(),"raw compiled AST cannot skip the original authored walk");
    assert!(crate::consumer_schedule::ConsumerProgram::new_with_prepared_native(&prepared,None).is_err(),"public no-source preparation lends no frozen source authority");
    let admitted=crate::consumer_schedule::ConsumerProgram::new_with_prepared_native(&prepared,Some(&source_binding)).unwrap();admitted.validate_emitted(&prepared.script).unwrap();
    let mut changed=prepared.clone();changed.script[body.body_label+1]=Statement::SetVar{name:"changed".into(),value:rvn_parser::Expr::Int(1)};
    assert!(crate::consumer_schedule::ConsumerProgram::new_with_prepared_native(&changed,Some(&source_binding)).is_err(),"detached body changes cannot borrow an equal header or its frozen source digest");
}
#[test]
fn narrative0732_preparation_preserves_historical_lowering_without_new_ast_copy(){
    let prepared=PreparedNarrative::from_parsed(parsed("scene label start\nfor item in [1,2]{set seen=item}\n\"Done\"\n")).unwrap();assert!(prepared.native_loops.authored().is_none());assert!(prepared.native_loops.is_empty());assert!(prepared.origins.iter().any(|o|matches!(o.generated,Some(GeneratedRole::ForCollection))));assert!(prepared.origins.iter().any(|o|matches!(o.generated,Some(GeneratedRole::ForAdvance))));assert!(!prepared.temporary_writes.is_empty());
}
#[test]
fn narrative0732_preparation_nested_headers_keep_actual_distinct_source_ancestry(){
    let inner=each(INNER,"rvn_native_index_g732_n3","set outer_seen=rvn_native_index_g732_n2 set inner_seen=rvn_native_index_g732_n3");let source=format!("scene label start\n{}\n\"Done\"\n",each(EACH,"rvn_native_index_g732_n2",&inner));let prepared=PreparedNarrative::from_parsed(parsed(&source)).unwrap();let headers:Vec<_>=prepared.script.iter().enumerate().filter_map(|(pc,_)|prepared.native_loops.body(pc)).collect();assert_eq!(headers.len(),2);let outer=headers.iter().find(|b|b.binding.node==2).unwrap();let inner=headers.iter().find(|b|b.binding.node==3).unwrap();assert!(inner.header.route.starts_with(&outer.body_route));assert_ne!(inner.body_label,outer.body_label);assert_ne!(inner.body_return,outer.body_return);assert_ne!(inner.header,outer.header);
    let repeated=each(EACH,"rvn_native_index_g732_n2","set seen=rvn_native_index_g732_n2");
    let sequential=rvn_parser::parse(&format!("scene label start\n{repeated}\n{repeated}\n")).unwrap();native_loop_contract::validate_authored_bindings(&sequential).unwrap();
    let another_scope=repeated.replace(SCOPE,"g732/n9:label|name=30@0,exec_out=31@0");
    let ambiguous=rvn_parser::parse(&format!("scene label start\n{}\n",each(EACH,"rvn_native_index_g732_n2",&another_scope))).unwrap();
    let error=native_loop_contract::validate_authored_bindings(&ambiguous).unwrap_err();assert!(error.contains("same private index"),"{error}");
}

#[test]
fn narrative0732_break_resolves_nearest_actual_range_and_cannot_borrow_through_legacy(){
    const RANGE:&str="n2:for_loop_with_break|exec_in=3@1,first=4@0,last=5@0,break=6@0,body=7@0,index=8@0,completed=9@0";
    const INNER_RANGE:&str="n3:for_loop_with_break|exec_in=10@2,first=11@0,last=12@0,break=13@0,body=14@0,index=15@0,completed=16@0";
    let range=|identity:&str,index:&str,body:&str|format!("flow.consume_v3(\"{SCOPE}\",\"{identity}\"){{flow.for_loop({index},0,2){{{body}}}}}");
    let sequence="flow.sequence() {{flow.break()} {set tail=1}}";
    let inner=range(INNER_RANGE,"rvn_native_index_g732_n3",sequence);
    let prepared=PreparedNarrative::from_parsed(parsed(&format!("scene label start\n{}\n\"Done\"\n",range(RANGE,"rvn_native_index_g732_n2",&inner)))).unwrap();
    let pc=prepared.script.iter().position(|statement|matches!(statement,Statement::LoopBreak)).unwrap();
    assert_eq!(prepared.native_loops.break_owner(&prepared,&prepared.position(pc)).unwrap().binding.node,3);
    assert!(prepared.script.iter().any(|statement|matches!(statement,Statement::SetVar{name,..}if name=="tail")),"Then1 remains a real compiled instruction");
    assert!(prepared.native_loops.break_owner(&prepared,&prepared.position(0)).is_err());
    let array=each(INNER,"rvn_native_index_g732_n3","flow.break()");
    let legacy=format!("flow.for_loop(old_index,0,2){{{array}}}");
    let prepared=PreparedNarrative::from_parsed(parsed(&format!("scene label start\n{}\n\"Done\"\n",range(RANGE,"rvn_native_index_g732_n2",&legacy)))).unwrap();
    let pc=prepared.script.iter().position(|statement|matches!(statement,Statement::LoopBreak)).unwrap();
    let error=prepared.native_loops.break_owner(&prepared,&prepared.position(pc)).unwrap_err().to_string();
    assert!(error.contains("nearest actual Range adapter"),"{error}");
}
