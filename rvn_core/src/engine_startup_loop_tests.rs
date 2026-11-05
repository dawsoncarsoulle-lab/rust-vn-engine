use super::*;
fn stop_in_loop(s:&mut EngineStartup<Host>,route:Vec<wire::Route>){
 let bp=s.site_page(0).unwrap().entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.route==route)).expect("genuine loop child site").breakpoint;
 s.configure(vec![bp],vec![]).unwrap();
 for _ in 0..64 {if s.snapshot().site.as_ref().is_some_and(|site|site.route==route)&&s.engine.startup.as_ref().is_some_and(|s|!s.loops.is_empty()){return;}s.step(StepKind::Continue).unwrap();}
 panic!("actual loop child was not reached");
}
#[test]
fn startup_loop_plain_nested_snapshot_and_nearest_break(){
 let mut s=startup("init {set items=[1,2,3] set total=0 for item in items {set items=[] flow.for_loop(index,2,5) {while true {set total=total+item if true {flow.break()} set unseen=99}}} set done=true}\nscene label start\n\"Done\"\n");
 finish(&mut s);assert_eq!(s.state().vars["total"],Value::Int(6));assert_eq!(s.state().vars["done"],Value::Bool(true));assert!(!s.state().vars.contains_key("unseen"));
}
#[test]
fn startup_loop_plain_while_rng_includes_terminal_condition(){
 let mut s=startup("init {set seen=0 while random(1,9)>0 and seen<3 {set seen=seen+1}}\nscene label start\n\"Done\"\n");
 let mut expected=s.state().random;let Statement::Init{body}=&s.program.script[0]else{unreachable!()};let Statement::While{condition,..}=&body[1]else{unreachable!()};
 let mut library=s.engine.functions.clone();library.debug_probe=None;
 for seen in 0..4 {let mut vars=HashMap::new();vars.insert("seen".into(),Value::Int(seen));assert_eq!(library.eval_bool_scoped_with_locale(condition,&vars,&HashMap::new(),&mut expected,None).unwrap(),seen<3);}
 finish(&mut s);assert_eq!(s.state().random,expected);assert_eq!(s.state().vars["seen"],Value::Int(3));
}
#[test]
fn startup_loop_cold_each_keeps_real_gather_and_rng_without_replay(){
 let raw="init {set total=0 for item in [random(1,9),random(1,9),random(1,9)] {set total=total+item} set done=true}\nscene label start\n\"Done\"\n";
 let mut s=startup(raw);let route=vec![wire::Route::Statement(1),wire::Route::Body,wire::Route::Statement(0)];stop_in_loop(&mut s,route.clone());
 let random=s.state().random;let save=s.capture_save15(1,"loop cold".into()).unwrap();let encoded=save.encode().unwrap();
 let mut cold=startup(raw);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&encoded).unwrap()).unwrap();
 assert_eq!(cold.snapshot().site.as_ref().unwrap().route,route);assert_eq!(cold.state().random,random);
 finish(&mut s);finish(&mut cold);assert_eq!(cold.state().random,random);assert_eq!(cold.state().vars,s.state().vars);
}
#[test]
fn startup_loop_cold_range_and_while_preserve_owned_headers(){
 for(raw,index)in [("init {set total=0 flow.for_loop(i,2,4) {set total=total+i} set done=true}\nscene label start\n\"Done\"\n",1),("init {set total=0 while total<3 {set total=total+1} set done=true}\nscene label start\n\"Done\"\n",1)]{
  let mut s=startup(raw);stop_in_loop(&mut s,vec![wire::Route::Statement(index),wire::Route::Body,wire::Route::Statement(0)]);
  let save=s.capture_save15(2,"range/while".into()).unwrap();let mut cold=startup(raw);cold.restore_save15(crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap();finish(&mut s);finish(&mut cold);assert_eq!(cold.state().vars,s.state().vars);
 }
}
#[test]
fn startup_loop_each_forged_items_next_gather_and_source_are_inert(){
 let raw="init {for item in [1,2,3] {set seen=item} set done=true}\nscene label start\n\"Done\"\n";
 let mut s=startup(raw);stop_in_loop(&mut s,vec![wire::Route::Statement(0),wire::Route::Body,wire::Route::Statement(0)]);let save=s.capture_save15(1,"forgery".into()).unwrap();
 for tamper in 0..7{
  let mut forged=save.clone();let frame=&mut forged.debugger.as_mut().unwrap().startup.as_mut().unwrap().loops[0];
  match tamper{
   0=>{let PortableStartupLoopState::ForEach{items,..}=&mut frame.state else{unreachable!()};items[0]=Value::Int(99);},
   1=>{let PortableStartupLoopState::ForEach{next,..}=&mut frame.state else{unreachable!()};*next+=1;},
   2=>frame.header=vec![wire::Route::Statement(1)],
   3=>frame.visits+=1,
   4=>frame.gather.clear(),
   6=>{frame.header_origin.route=vec![crate::program::AstRoutePart::Root{index:1}];},
   5=>{let mut row=serde_json::to_value(&frame.gather[0]).unwrap();row["request"]["route_prefix"]=serde_json::to_value(vec![wire::Route::Statement(1)]).unwrap();frame.gather[0]=serde_json::from_value(row).unwrap();},_=>unreachable!()
  }
  let mut target=startup(raw);let before=serde_json::to_value(target.state()).unwrap();assert!(target.restore_save15(forged).is_err(),"tamper {tamper}");assert_eq!(serde_json::to_value(target.state()).unwrap(),before);
 }
}
#[test]
fn startup_loop_range_forged_bounds_count_and_next_are_inert(){
 let raw="init {flow.for_loop(i,2,4) {set seen=i} set done=true}\nscene label start\n\"Done\"\n";let mut s=startup(raw);stop_in_loop(&mut s,vec![wire::Route::Statement(0),wire::Route::Body,wire::Route::Statement(0)]);let save=s.capture_save15(1,"range forgery".into()).unwrap();
 for tamper in 0..3{let mut forged=save.clone();let frame=&mut forged.debugger.as_mut().unwrap().startup.as_mut().unwrap().loops[0];let PortableStartupLoopState::ForRange{first,count,next}=&mut frame.state else{unreachable!()};match tamper{0=>*first+=1,1=>*count+=1,2=>*next+=1,_=>unreachable!()};let mut target=startup(raw);let before=serde_json::to_value(target.state()).unwrap();assert!(target.restore_save15(forged).is_err(),"tamper {tamper}");assert_eq!(serde_json::to_value(target.state()).unwrap(),before);}
}
#[test]
fn startup_loop_operation_stamp_binds_header_and_every_depth(){
 let mut s=startup("init {flow.for_loop(i,1,2) {for item in [1,2] {set seen=item}}}\nscene label start\n\"Done\"\n");stop_in_loop(&mut s,vec![wire::Route::Statement(0),wire::Route::Body,wire::Route::Statement(0),wire::Route::Body,wire::Route::Statement(0)]);
 let operation=s.engine.startup_operation().unwrap();assert_eq!(operation.loop_visits.len(),2);
 for tamper in 0..3{let mut forged=operation.clone();match tamper{0=>forged.loop_visits[0].header=forged.loop_visits[1].header.clone(),1=>forged.loop_visits.swap(0,1),2=>forged.loop_visits[1].visits+=1,_=>unreachable!()};assert!(s.engine.validate_startup_operation(&forged).is_err());}
}
#[test]
fn startup_loop_renderer_refusal_does_not_publish_body_or_advance(){
 let mut s=startup("init {for item in [1,2] {set seen=item} set done=true}\nscene label start\n\"Done\"\n");stop_in_loop(&mut s,vec![wire::Route::Statement(0),wire::Route::Body,wire::Route::Statement(0)]);
 for _ in 0..8{let plan=s.prepare_step(StepKind::Continue).unwrap();if plan.render_batch().is_some(){
 let before=serde_json::to_value(s.state()).unwrap();let cursor=format!("{:?}",s.engine.startup);let batches=s.engine.renderer.batches.len();s.engine.renderer.reject.store(true,Ordering::Relaxed);assert!(s.begin_prepared_step_commit(plan).is_err());assert_eq!(serde_json::to_value(s.state()).unwrap(),before);assert_eq!(format!("{:?}",s.engine.startup),cursor);assert_eq!(s.engine.renderer.batches.len(),batches);return;
 }s.begin_prepared_step_commit(plan).unwrap().commit();}panic!("real loop body never prepared a publication");
}
#[test]
fn startup_loop_legacy_portable_omits_new_empty_field_and_debug_is_exact(){
 let s=startup("init {set x=1}\nscene label start\n\"Done\"\n");let state=s.engine.startup.as_ref().unwrap();
 #[derive(Debug)]struct StartupState<'a>{roots:&'a Vec<InitRoot>,root:usize,body:usize,nested:&'a Vec<wire::Route>,exit_pc:usize,waiting_video:&'a Option<String>}
 let legacy=StartupState{roots:&state.roots,root:state.root,body:state.body,nested:&state.nested,exit_pc:state.exit_pc,waiting_video:&state.waiting_video};assert_eq!(format!("{state:?}"),format!("{legacy:?}"));
 let portable=state.portable().unwrap();let json=serde_json::to_value(portable).unwrap();assert!(json.get("loops").is_none());let decoded:PortableStartup=serde_json::from_value(json).unwrap();assert!(decoded.loops.is_empty());
}

#[test]
fn normal_core_arc_shared_by_steps_handoff_and_pending_fresh(){
 let mut s=startup("init {for item in [1,2] {set seen=item}}\nscene label start\n\"Ready\"\n");let frozen=std::sync::Arc::clone(&s.program);
 assert!(std::sync::Arc::ptr_eq(s.engine.frozen_startup_program.as_ref().unwrap(),&frozen));
 while !s.is_complete(){let plan=s.prepare_step(StepKind::Continue).unwrap();s.begin_prepared_step_commit(plan).unwrap().commit();assert!(std::sync::Arc::ptr_eq(&s.program,&frozen));assert!(std::sync::Arc::ptr_eq(s.engine.frozen_startup_program.as_ref().unwrap(),&frozen));}
 let engine=s.into_engine().unwrap();let fresh=engine.prepare_fresh_startup(Host::default(),8).unwrap();assert!(!fresh.is_complete());assert!(fresh.engine.renderer.batches.is_empty());assert!(std::sync::Arc::ptr_eq(&fresh.program,&frozen));assert!(std::sync::Arc::ptr_eq(fresh.engine.frozen_startup_program.as_ref().unwrap(),&frozen));
}
#[test]
fn normal_core_fresh_generations_are_set_before_any_session_or_publication(){
 for raw in ["init {set seen=0 while seen<3 {set seen=seen+1}}\nscene label start\n\"Done\"\n","init {for item in [1,2] {set seen=item}}\nscene label start\n\"Done\"\n","init {flow.for_loop(i,1,3) {set seen=i}}\nscene label start\n\"Done\"\n"]{
  let mut s=startup(raw);finish(&mut s);let mut engine=s.into_engine().unwrap();engine.runtime_epoch=7;engine.interface_epoch=8;engine.video_epoch=9;
  let mut fresh=engine.prepare_fresh_startup(Host::default(),8).unwrap();assert_eq!((fresh.engine.runtime_epoch,fresh.engine.interface_epoch,fresh.engine.video_epoch),(8,9,10));assert!(fresh.engine.renderer.batches.is_empty());let old=serde_json::to_vec(&engine.state).unwrap();
  finish(&mut fresh);assert!(fresh.engine.renderer.batches.iter().all(|batch|batch.runtime_epoch==8));assert_eq!(serde_json::to_vec(&engine.state).unwrap(),old);
 }
}
#[test]
fn normal_core_fresh_epoch_overflow_refuses_without_touching_old_owner(){
 for field in 0..3{let mut s=startup("init {set seen=1}\nscene label start\n\"Done\"\n");finish(&mut s);let mut engine=s.into_engine().unwrap();match field{0=>engine.runtime_epoch=u64::MAX,1=>engine.interface_epoch=u64::MAX,2=>engine.video_epoch=u64::MAX,_=>unreachable!()};let before=serde_json::to_vec(&engine.state).unwrap();let batches=engine.renderer.batches.len();assert!(engine.prepare_fresh_startup(Host::default(),8).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.renderer.batches.len(),batches);}
}
#[test]
fn normal_core_fresh_requires_actual_frozen_bytes_and_revalidates_emitted_source(){
 let mut s=startup("init {for item in [1] {set seen=item}}\nscene label start\n\"Done\"\n");finish(&mut s);let mut engine=s.into_engine().unwrap();let before=serde_json::to_vec(&engine.state).unwrap();let original=engine.frozen_startup_program.take().unwrap();assert!(engine.prepare_fresh_startup(Host::default(),8).is_err());engine.frozen_startup_program=Some(original);
 let mut forged=engine.frozen_startup_program.as_ref().unwrap().as_ref().clone();forged.sources[0].source.push_str("\n# changed source bytes\n");engine.frozen_startup_program=Some(std::sync::Arc::new(forged));assert!(engine.prepare_fresh_startup(Host::default(),8).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);
}
