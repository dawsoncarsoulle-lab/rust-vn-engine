//! Causal parsed-source/Engine checks. No editor or native activation is asserted.
use super::*;
use rvn_parser::function_library_asset1023::{FunctionLibraryAsset,FunctionLibraryId,FunctionLibraryMember,LibraryFunctionAccess as Access};
const A:&str="11111111111141118111111111111111";
const B:&str="22222222222242228222222222222222";
const P:&str="33333333333343338333333333333333";
const Q:&str="44444444444444448444444444444444";
#[derive(Clone,Default)]struct Observe{
 publications:std::rc::Rc<std::cell::Cell<usize>>,
 scene:std::rc::Rc<std::cell::RefCell<crate::engine::debugger_commands::LibrarySceneTestHost1027>>,
}
impl Renderer for Observe{
 fn validate_scene_batch(&self,batch:&crate::renderer::SceneRenderBatch)->Result<(),String>{self.scene.borrow().validate_scene_batch(batch)}
 fn publish_scene_batch(&mut self,batch:crate::renderer::SceneRenderBatch){self.scene.borrow_mut().publish_scene_batch(batch);self.publications.set(self.publications.get()+1);}
 fn set_background(&mut self,_:&str,_:&Transition){}
 fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
 fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
 fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
 fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
 fn show_choice(&mut self,_:&[String])->usize{0}
 fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{0}
 fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
 fn music_stop(&mut self,_:&Transition){}
 fn music_set_volume(&mut self,_:f32){}
 fn sfx_play(&mut self,_:&str,_:&Transition){}
 fn sfx_stop(&mut self,_:&str,_:&Transition){}
}
fn asset(id:&str,rows:&[(u64,Access,bool)])->FunctionLibraryAsset{
 FunctionLibraryAsset::new(FunctionLibraryId::parse(id).unwrap(),rows.iter().map(|(graph_id,access,pure)|FunctionLibraryMember{graph_id:*graph_id,access:*access,pure:*pure}).collect()).unwrap()
}
fn scope(id:&str,members:&str)->String{format!("functions.library_v1(\"{id}\"){{{members}}}\n")}
fn member(id:u64,parameter:Option<&str>,body:&str)->String{
 let parameters=parameter.map(|p|format!("\"{p}\"")).unwrap_or_default();
 format!("functions.member_v1(\"{id}\",[{parameters}]){{{body}}}")
}
fn value(owner:&str,id:u64,arg:Option<(&str,&str)>)->String{
 let arguments=arg.map(|(id,v)|format!("\"{id}\":{v}")).unwrap_or_default();
 format!("functions.value_v1(\"{owner}\",\"{id}\",{{{arguments}}})")
}
fn invoke(owner:&str,id:u64,arg:Option<(&str,&str)>)->String{value(owner,id,arg).replace("functions.value_v1","functions.invoke_v1")}
fn prepare(source:&str,assets:&[FunctionLibraryAsset])->Result<Engine<Observe>,RuntimeError>{
 let parsed=rvn_parser::parse_program(source).unwrap();
 let prepared=PreparedFunctionLibraryProgram1023::prepare(parsed,assets)?;
 Engine::from_function_library_program1023(prepared,Observe::default(),8)
}
#[test]fn libraries1023_actual_parser_two_homonyms_are_distinct_before_init_and_after_fresh(){
 let declarations=scope(A,&member(11,Some(P),"function same(n){return n+1}"))+&scope(B,&member(22,Some(Q),"function same(n){return n+2}"));
 let source=format!("{declarations}init{{set initialized={}+{}}}\nscene label start\nset first={}\nset second={}\n\"After\"",value(A,11,Some((P,"10"))),value(B,22,Some((Q,"20"))),value(A,11,Some((P,"3"))),value(B,22,Some((Q,"3"))));
 let assets=[asset(A,&[(11,Access::Public,true)]),asset(B,&[(22,Access::Public,true)])];
 let mut game=prepare(&source,&assets).unwrap();assert_eq!(game.state.vars["initialized"],Value::Int(33));
 game.step_until_interaction().unwrap();assert_eq!(game.state.vars["first"],Value::Int(4));assert_eq!(game.state.vars["second"],Value::Int(5));
 assert_eq!(game.functions.library_assets1023(),&assets);
 let mut fresh=game.fresh(Observe::default(),8).unwrap();assert_eq!(fresh.state.vars["initialized"],Value::Int(33));fresh.step_until_interaction().unwrap();assert_eq!(fresh.state.vars["first"],Value::Int(4));assert_eq!(fresh.state.vars["second"],Value::Int(5));assert_eq!(fresh.functions.library_assets1023(),&assets);
}
#[test]fn libraries1023_real_unit_publication_keeps_scene_local_legacy_function_and_parameter_ids(){
 let declarations=scope(A,&member(11,Some(P),"function same(n)->(){local own=900 set result=legacy(n)}"))+&scope(B,&member(22,Some(Q),"function same(n)->(){set result=result+n}"));
 let source=format!("{declarations}function legacy(n){{return n+1}}\ninit{{set result=0}}\nscene label start\nlocal result=50\n{}\n{}\n\"After\"",invoke(A,11,Some((P,"result"))),invoke(B,22,Some((Q,"2"))));
 let assets=[asset(A,&[(11,Access::Public,false)]),asset(B,&[(22,Access::Public,false)])];let mut game=prepare(&source,&assets).unwrap();
 game.step_until_interaction().unwrap();assert_eq!(game.state.vars["result"],Value::Int(53));
 let active=game.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap();assert_eq!(active.locals["result"],Value::Int(50));
 assert!(!game.state.vars.contains_key("n"));assert!(!game.state.vars.contains_key("own"));assert!(game.state.call_stack.is_empty());assert!(game.renderer.publications.get()>=2);
}
#[test]fn libraries1023_private_member_uses_actual_same_library_frame_and_cross_library_body_is_refused(){
 let private=value(A,12,Some((P,"n")));
 let declarations=scope(A,&(member(11,Some(Q),&format!("function public_entry(n){{return {private}}}"))+&member(12,Some(P),"function secret(n){return n+7}")));
 let assets=[asset(A,&[(11,Access::Public,true),(12,Access::Private,true)])];
 let source=format!("{declarations}scene label start\nset result={}\n\"After\"",value(A,11,Some((Q,"5"))));
 let mut game=prepare(&source,&assets).unwrap();game.step_until_interaction().unwrap();assert_eq!(game.state.vars["result"],Value::Int(12));
 // Same real target exists. A different real Library frame cannot inherit its private scope.
 let foreign=scope(B,&member(22,None,&format!("function other(){{return {}}}",value(A,12,Some((P,"random(1,9)"))))));
 let source=format!("{declarations}{foreign}scene label start\n\"After\"");let assets=[assets[0].clone(),asset(B,&[(22,Access::Public,true)])];
 assert!(PreparedFunctionLibraryProgram1023::prepare(rvn_parser::parse_program(&source).unwrap(),&assets).is_err());
}
#[test]fn libraries1023_unscoped_valid_private_target_and_stale_parameter_refuse_before_rng_and_publish(){
 let declarations=scope(A,&member(11,Some(P),"function secret(n){return n}"));
 for (access,parameter) in [(Access::Private,P),(Access::Public,Q)]{
  let source=format!("{declarations}scene label start\nset result={}\n\"After\"",value(A,11,Some((parameter,"random(1,9)"))));
  let mut game=prepare(&source,&[asset(A,&[(11,access,true)])]).unwrap();
  for _ in 0..100{if matches!(game.peek_statement(),Some(Statement::SetVar{name,..})if name=="result"){break;}game.exec_silent(game.peek_statement().unwrap().clone()).unwrap();}
  let before=serde_json::to_vec(&game.state).unwrap();let publications=game.renderer.publications.get();
  assert!(game.exec_silent(game.peek_statement().unwrap().clone()).is_err());assert_eq!(serde_json::to_vec(&game.state).unwrap(),before);assert_eq!(game.renderer.publications.get(),publications);
 }
}
#[test]fn libraries1023_debugger_sites_retain_each_actual_member_declaration_and_utf8_body(){
 let source=scope(A,&member(11,Some(P),"function same(n){local `é`=n+1 return `é`}"))+&scope(B,&member(22,Some(Q),"function same(n){return n+2}"))+"scene label start\nreturn";
 let program=rvn_parser::parse_program(&source).unwrap();let map=crate::debugger_provenance::SourceMap::from_program(&program).unwrap();
 for(owner,graph,needle)in[(A,11,"local `é`=n+1"),(B,22,"return n+2")]{
  let key=format!("library:{owner}:{graph}");let site=map.sites.iter().find(|site|site.scope==key&&site.origin.span.as_ref().is_some_and(|range|source.get(range.clone())==Some(needle))).unwrap();
  assert_eq!(site.declaration.graph.as_ref().unwrap().graph,graph);assert_eq!(site.declaration.route.len(),2);
  let root=program.script.iter().find(|s|matches!(s,Statement::FunctionLibrary{scope}if scope.library().as_str()==owner)).unwrap();
  assert!(crate::editor_debugger::validate_site(root,&crate::editor_debugger::Site{function:Some(key),route:site.route.clone(),depth:1,statement:true}));
 }
}

#[test]fn libraries1023_declared_rename_keeps_target_and_parameter_ids_in_actual_cold_parse(){
 let assets=[asset(A,&[(11,Access::Public,true)])];
 for name in ["original_name","renamed_function"]{
  let body=format!("function {name}(left,right){{return left*100+right}}");
  let declaration=format!("functions.library_v1(\"{A}\"){{functions.member_v1(\"11\",[\"{P}\",\"{Q}\"]){{{body}}}}}");
  // Reversed wire dictionary still binds/evaluates in original signature order.
  let source=format!("{declaration}\nscene label start\nset actual=functions.value_v1(\"{A}\",\"11\",{{\"{Q}\":random(1,9),\"{P}\":random(1,9)}})\n\"After\"");
  let mut game=prepare(&source,&assets).unwrap();
  // Reach the authored call itself, before interaction presentation takes its
  // separate display seed. Never seed or overwrite the actual Engine RNG.
  for _ in 0..100{if matches!(game.peek_statement(),Some(Statement::SetVar{name,..})if name=="actual"){break;}game.exec_silent(game.peek_statement().unwrap().clone()).unwrap();}
  assert!(matches!(game.peek_statement(),Some(Statement::SetVar{name,..})if name=="actual"));
  let mut expected_random=game.state.random;let globals=game.state.vars.clone();
  let random=rvn_parser::Expr::Call{name:"random".into(),args:vec![rvn_parser::Expr::Int(1),rvn_parser::Expr::Int(9)]};
  let left=game.functions.eval_with_random(&random,&globals,&mut expected_random).unwrap();let right=game.functions.eval_with_random(&random,&globals,&mut expected_random).unwrap();
  let(Value::Int(left),Value::Int(right))=(left,right)else{panic!("integer RNG")};
  game.exec_silent(game.peek_statement().unwrap().clone()).unwrap();
  assert_eq!(game.state.vars["actual"],Value::Int(left*100+right));assert_eq!(game.state.random,expected_random);
  // The real dialogue publication intentionally draws exactly one seed from
  // story randomness. Repainting the same interaction must not draw another.
  let display=crate::random::RandomState::seeded(expected_random.next_u64());
  assert!(game.step_until_interaction().unwrap().is_some());
  assert_eq!(game.state.vars["actual"],Value::Int(left*100+right));assert_eq!(game.state.random,expected_random);assert_eq!(game.state.display_random,display);
  assert!(game.step_until_interaction().unwrap().is_some());assert_eq!(game.state.random,expected_random);assert_eq!(game.state.display_random,display);
 }
}

#[test]fn libraries1027_rebased_qualified_calls_preserve_nominal_enum_admission_and_cross_asset_refusal(){
 use rvn_parser::{enum_asset::{EnumAsset,EnumAssetId,EnumMemberId,EnumCommand},enum_asset_json::EnumIndexInput,enum_asset_admission1025::EnumAssets1025};
 const EA:&str="329f41eae6ba4b4bb71dfaf60fb50001";const EB:&str="329f41eae6ba4b4bb71dfaf60fb50002";const EM:&str="229f41eae6ba4b4bb71dfaf60fb50001";
 let rows=[(EA,Some("Alpha.rvnenum")),(EB,Some("Beta.rvnenum"))];let mut files=Vec::new();
 for(id,path)in rows{let asset=EnumAsset::new(EnumAssetId::parse(id).unwrap(),"Same visible label".into()).unwrap().prepare(EnumCommand::Add{member:EnumMemberId::parse(EM).unwrap(),display_name:"First".into(),description:String::new()}).unwrap().after().clone();files.push((path.unwrap().to_owned(),asset.encode().unwrap().into_bytes()));}
 let resources=std::sync::Arc::new(EnumAssets1025::admit(EnumIndexInput::Table(&rows),&files,true).unwrap());
 let assets=[asset(A,&[(11,Access::Public,true)])];
 let declaration=scope(A,&member(11,Some(P),"function identity(n){return n}"));
 let expression=|owner:&str|{let argument=format!("__rvn_enum_value_v1(\"{owner}\",\"{EM}\")");let source=format!("{declaration}scene label start\nset actual={}\n\"After\"",value(A,11,Some((P,&argument))));let parsed=rvn_parser::parse_program(&source).unwrap();let expression=parsed.script.iter().find_map(|s|if let Statement::SetVar{name,value}=s{(name=="actual").then(||value.clone())}else{None}).unwrap();(parsed.script,expression)};
 let(script,left)=expression(EA);let(_,right)=expression(EB);
 let library=FunctionLibrary::from_script_with_library_assets(&script,&assets).unwrap().with_enum_assets1025(resources.clone());
 let original=resources.value(EnumAssetId::parse(EA).unwrap(),EnumMemberId::parse(EM).unwrap()).unwrap();
 let passed=library.eval(&left,&HashMap::new()).unwrap();assert_eq!(passed,Value::Enum(original));
 assert_eq!(serde_json::from_slice::<Value>(&serde_json::to_vec(&passed).unwrap()).unwrap(),passed);
 let equality=|a:rvn_parser::Expr,b:rvn_parser::Expr|rvn_parser::Expr::BinOp{op:rvn_parser::BinOpKind::Eq,left:Box::new(a),right:Box::new(b)};
 assert_eq!(library.eval(&equality(left.clone(),left.clone()),&HashMap::new()).unwrap(),Value::Bool(true));
 assert!(library.eval(&equality(left.clone(),right),&HashMap::new()).is_err(),"same display labels and member UUID cannot collapse two nominal asset identities");
 let missing=FunctionLibrary::from_script_with_library_assets(&script,&assets).unwrap();assert!(missing.eval(&left,&HashMap::new()).is_err(),"qualified scope is not an Enum resource grant");
 for scalar in [rvn_parser::Expr::Int(0),rvn_parser::Expr::Str("First".into())]{assert!(library.eval(&equality(left.clone(),scalar),&HashMap::new()).is_err(),"no ordinal/string masquerade");}
}

#[test]
fn libraries_scene_fixture1027_reuses_actual_retained_cpu_batches_and_refusal_keeps_core_unpublished(){
 let source=scope(A,&member(11,None,"function same(){return 7}"))+"scene label start\n\"Ready\"\nscene label other\nset actual=1\n\"Other\"\n";
 let mut game=prepare(&source,&[asset(A,&[(11,Access::Public,true)])]).unwrap();
 game.enter_label("start").unwrap(); // actual source-owned initial publication before the baseline
 let before=serde_json::to_vec(&game.state).unwrap();let count=game.renderer.publications.get();
 assert_eq!(game.renderer.scene.borrow().retained_count1027(),count);
 assert!(count>0,"actual initial scene batch was retained, not just a capability boolean");
 let retained=serde_json::to_vec(game.renderer.scene.borrow().retained_state1027().unwrap()).unwrap();assert_eq!(retained,before);
 game.renderer.scene.borrow_mut().reject_scene1027(true);
 assert!(game.enter_label("other").is_err());assert_eq!(serde_json::to_vec(&game.state).unwrap(),before);
 assert_eq!(game.renderer.publications.get(),count);assert_eq!(game.renderer.scene.borrow().retained_count1027(),count);
 assert_eq!(serde_json::to_vec(game.renderer.scene.borrow().retained_state1027().unwrap()).unwrap(),retained);
 game.renderer.scene.borrow_mut().reject_scene1027(false);game.enter_label("other").unwrap();
 assert!(matches!(game.peek_statement(),Some(Statement::SetVar{name,..})if name=="actual"));
 assert!(game.renderer.scene.borrow().retained_count1027()>count);
 assert_eq!(serde_json::to_vec(game.renderer.scene.borrow().retained_state1027().unwrap()).unwrap(),serde_json::to_vec(&game.state).unwrap());
}
