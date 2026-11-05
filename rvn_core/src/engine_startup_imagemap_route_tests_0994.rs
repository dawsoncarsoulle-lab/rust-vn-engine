//! Routing/source evidence only. No Engine/input owner is fabricated here.
use super::*;
use std::{path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};
fn directory(name:&str)->PathBuf {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    let root=PathBuf::from(std::env::var("RVN_NATIVE_IMAGEMAP_FIXTURES").expect("isolated private D fixtures"));
    let path=root.join(format!("startup-route-{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    std::fs::create_dir_all(&path).unwrap();path
}
fn project(name:&str,source:&str)->(ParsedProgram,SourceMap) {
    let path=directory(name).join("main.rvn");std::fs::write(&path,source).unwrap();
    let program=rvn_parser::parse_file_program_with_uses(&path).unwrap();
    let map=SourceMap::from_program(&program).unwrap();(program,map)
}
fn source(scope:&str)->String {
    format!(r#"init{{flow.consume_v2("{scope}","map"){{capture "sample"=random(1,9) imagemap{{background:"garden.png" hotspot{{name:"left" area:(0,0,50,100)}}=>{{set chosen=1}} hotspot{{name:"empty" area:(50,0,100,100)}}=>{{}}}}}}}}"#)
}
fn root(program:&ParsedProgram)->usize {
    program.script.iter().position(|node|matches!(node,Statement::Init{..})).expect("actual authored Init")
}
#[test]
fn startup_imagemap0994_frozen_file_and_source_map_resolve_the_real_child_and_return_boundary_without_execution() {
    let(program,map)=project("source",&source("actual"));let definition=root(&program);
    let cursor=prepare(&program,&map,definition,&[Route::Statement(0)],0).unwrap();
    cursor.validate_for(&program,&map).unwrap();
    assert_eq!(cursor.return_route(),[Route::Statement(0)]);
    let(route,origin)=cursor.continuation().expect("the real nonempty selected branch");
    assert_eq!(route,[Route::Statement(0),Route::Body,Route::Statement(0),Route::Option(0),Route::Statement(0)]);
    assert!(origin.route.iter().any(|part|matches!(part,crate::program::AstRoutePart::Hotspot{hotspot:0,index:0})));
    assert_eq!(origin.source,program.origins[definition].source);
    let file=program.sources.iter().find(|source|source.path==origin.source).unwrap();
    assert_eq!(&file.source[origin.span.clone().unwrap()],"set chosen=1");
    assert!(matches!(statement_at(&program.script[definition],route).unwrap(),Statement::SetVar{name,..}if name=="chosen"));
    // The plan retains a source range, never an evaluated capture/response.
    assert!(file.source.contains("capture \"sample\"=random(1,9)"));
    assert_ne!(cursor.parent_origin().span,origin.span);
}
#[test]
fn startup_imagemap0994_condition_routes_and_empty_branch_stay_distinct_without_deciding_the_condition() {
    let inside=source("then_scope");let then_body=inside.strip_prefix("init{").unwrap().strip_suffix('}').unwrap();
    let inside=source("else_scope");let else_body=inside.strip_prefix("init{").unwrap().strip_suffix('}').unwrap();
    let(program,map)=project("condition",&format!("init{{if false{{{then_body}}}else{{{else_body}}}}}"));let definition=root(&program);
    let then=prepare(&program,&map,definition,&[Route::Statement(0),Route::Then,Route::Statement(0)],0).unwrap();
    let otherwise=prepare(&program,&map,definition,&[Route::Statement(0),Route::Else,Route::Statement(0)],0).unwrap();
    assert_ne!(then.parent_origin().span,otherwise.parent_origin().span);
    assert_ne!(then.continuation().unwrap().1.span,otherwise.continuation().unwrap().1.span);
    let empty=prepare(&program,&map,definition,&[Route::Statement(0),Route::Else,Route::Statement(0)],1).unwrap();
    assert!(empty.continuation().is_none());
    assert_eq!(empty.return_route(),[Route::Statement(0),Route::Else,Route::Statement(0)]);
    let Statement::Init{body}=&program.script[definition]else{panic!()};
    assert!(matches!(&body[0],Statement::If{condition:rvn_parser::Expr::Bool(false),..}));
    // Both structural paths can be inspected. Only the future real waiting
    // owner may select the current path; this plan grants neither branch.
}
#[test]
fn startup_imagemap0994_changed_source_ambiguous_sites_wrong_kind_and_bounds_refuse_without_modifying_frozen_inputs() {
    let(program,map)=project("guards",&source("guarded"));let definition=root(&program);let before=program.clone();let map_before=serde_json::to_vec(&map).unwrap();
    assert!(prepare(&program,&map,definition,&[Route::Statement(0)],2).is_err());
    assert!(prepare(&program,&map,definition,&[Route::Statement(0),Route::Body],0).is_err());
    assert!(prepare(&program,&map,definition,&vec![Route::Statement(0);131],0).is_err());
    assert!(prepare(&program,&map,program.script.len(),&[Route::Statement(0)],0).is_err());
    let cursor=prepare(&program,&map,definition,&[Route::Statement(0)],0).unwrap();
    let mut changed=program.clone();changed.sources[0].source=changed.sources[0].source.replace("chosen=1","chosen=2");assert!(cursor.validate_for(&changed,&map).is_err());
    let mut wrong=map.clone();let site=wrong.sites.iter().find(|site|site.route==[Route::Statement(0)]&&site.scope.is_empty()&&site.origin.scheduled_step.is_none()).unwrap().clone();wrong.sites.push(site);assert!(cursor.validate_for(&program,&wrong).is_err());
    let mut wrong=map.clone();let site=wrong.sites.iter_mut().find(|site|site.route==[Route::Statement(0)]&&site.scope.is_empty()&&site.origin.scheduled_step.is_none()).unwrap();site.origin.span=Some(0..1);assert!(cursor.validate_for(&program,&wrong).is_err());
    let mut wrong_cursor=cursor.clone();wrong_cursor.index=1;assert!(wrong_cursor.validate_for(&program,&map).is_err());
    assert_eq!(program,before);assert_eq!(serde_json::to_vec(&map).unwrap(),map_before);
    let(program,map)=project("wrong-kind","init{set actual=1}");assert!(prepare(&program,&map,root(&program),&[Route::Statement(0)],0).is_err());
}
#[test]
fn startup_imagemap0994_imported_equal_children_keep_original_source_and_definition_ordinals() {
    let dir=directory("imports");std::fs::write(dir.join("left.rvn"),source("left")).unwrap();std::fs::write(dir.join("right.rvn"),source("right")).unwrap();std::fs::write(dir.join("main.rvn"),"use \"left.rvn\"\nuse \"right.rvn\"\n").unwrap();
    let program=rvn_parser::parse_file_program_with_uses(&dir.join("main.rvn")).unwrap();let map=SourceMap::from_program(&program).unwrap();
    let definitions=program.script.iter().enumerate().filter_map(|(index,node)|matches!(node,Statement::Init{..}).then_some(index)).collect::<Vec<_>>();assert_eq!(definitions.len(),2);
    let a=prepare(&program,&map,definitions[0],&[Route::Statement(0)],0).unwrap();let b=prepare(&program,&map,definitions[1],&[Route::Statement(0)],0).unwrap();
    assert_ne!(a.definition,b.definition);assert_ne!(a.parent_origin().source,b.parent_origin().source);assert_ne!(a.continuation().unwrap().1.source,b.continuation().unwrap().1.source);
    assert_eq!(statement_at(&program.script[definitions[0]],a.continuation().unwrap().0).unwrap(),statement_at(&program.script[definitions[1]],b.continuation().unwrap().0).unwrap());
    let mut forged=a.clone();forged.definition=definitions[1];assert!(forged.validate_for(&program,&map).is_err());a.validate_for(&program,&map).unwrap();b.validate_for(&program,&map).unwrap();
}
