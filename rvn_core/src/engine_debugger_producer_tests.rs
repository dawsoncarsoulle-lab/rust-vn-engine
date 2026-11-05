use super::*;
#[test]
fn producer_debugger_choice_child_stops_on_real_wrapper_before_its_draw() {
    for legacy in [false, true] {
        let source = format!("{} label start\nflow.consume_v2(\"root\",\"display\") {{choice {{\"first\"=>{{flow.consume_v2(\"root\",\"child\") {{capture \"draw\"=random(1,9) set result=__rvn_captured_v1(\"draw\")}}}}}}}}\n\"Done\"\n", if legacy { "" } else { "scene" });
        let mut engine = consumer_engine(actual_program(&source, "C:/actual/choice-producer.rvn"));
        assert!(matches!(engine.step_until_interaction().unwrap(), Some(crate::Interaction::Choice { .. })));
        engine.submit_choice(0).unwrap();
        let mut offset=0;
        let bp=loop {
            let page=engine.debug_site_page(offset).unwrap();
            if let Some(entry)=page.entries.iter().find(|entry|entry.origin.as_ref().and_then(|o|o.scheduled_step.as_ref()).is_some_and(|step|step.consumer_identity=="child")) { break entry.breakpoint.clone(); }
            offset+=page.entries.len();assert!(offset<page.total,"actual child producer not offered");
        };
        // The selected branch may first enter a real generated label. Drive
        // its actual debug operation; never assign a fabricated producer PC.
        for _ in 0..8 {
            if engine.state.pc==bp.position.pc { break; }
            let before=engine.state.random;
            assert!(matches!(engine.script[engine.state.pc],Statement::Label{..}));
            let mut entry=engine.debug_begin(DebugOperation::Statement,vec![]).unwrap();
            finish(&mut engine,&mut entry);
            assert_eq!(engine.state.random,before);
        }
        assert_eq!(bp.position.pc, engine.state.pc);
        assert!(matches!(engine.script[bp.position.pc], Statement::ScheduledConsumer { .. }));
        let before=serde_json::to_value(&engine.state).unwrap();
        let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp]).unwrap();
        assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);
        assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);
        let origin=session.snapshot().origin.unwrap();
        let step=origin.scheduled_step.unwrap();
        assert_eq!(&source[step.expression],"random(1,9)");
        assert!(origin.route.iter().any(|part|matches!(part,crate::program::AstRoutePart::Choice { option:0,.. })));
        finish(&mut engine,&mut session);
        let mut expected=crate::random::RandomState::seeded(4);expected.integer(1,9).unwrap();
        assert_eq!(engine.state.random,expected);
        assert!(matches!(engine.state.vars["result"],Value::Int(_)));
    }
}
fn native_source()->String{
    "// été 🍃\r\nfunction producer() {local draw=random(1,9) return __rvn_invariant_text(\"literal\")}\r\nfunction consumer() {\r\nflow.consume_v1(\"g9/n1:function_entry|exec=11@0\",\"n2:function_return|exec=12@0,result=13@0\") {\r\ncapture \"n3:function_call|result=16@0\" = producer()\r\ninspect \"n4:get_string_table_id_and_key|text=17@0,valid=18@0,table_id=19@0,key=20@0\" = __rvn_captured_v1(\"n3:function_call|result=16@0\")\r\nreturn __rvn_terminal_v1(\"n4:get_string_table_id_and_key|text=17@0,valid=18@0,table_id=19@0,key=20@0\",\"valid\")\r\n}\r\n}\r\nscene label start\r\nset result=consumer()\r\n\"Done\"\r\n".into()
}
fn actual_program(raw:&str,path:&str)->rvn_parser::ParsedProgram{
    let mut p=rvn_parser::parse_program(raw).unwrap();let path=Some(path.into());p.sources[0].path=path.clone();for o in &mut p.origins{o.source=path.clone();}p
}
fn producer_bp(engine:&Engine<Host>,index:usize)->wire::Breakpoint{
    let mut offset=0;
    loop{let page=engine.debug_site_page(offset).unwrap();for entry in &page.entries{
        if entry.origin.as_ref().and_then(|o|o.scheduled_step.as_ref()).is_some_and(|s|s.scope.as_deref()==Some("consumer")&&s.index==index){return entry.breakpoint.clone();}
    }offset+=page.entries.len();assert!(offset<page.total,"actual producer not offered");}
}
#[test]
fn producer_debugger_actual_stop_before_rng_then_one_step_and_one_inspector_three_ports(){
    let raw=native_source();let p=actual_program(&raw,"C:/actual/source.rvn");let mut engine=consumer_engine(p);let before=serde_json::to_value(&engine.state).unwrap();
    let bp0=producer_bp(&engine,0);let bp1=producer_bp(&engine,1);assert!(!bp0.site.as_ref().unwrap().statement);
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp0.clone(),bp1.clone()]).unwrap();
    assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);
    let snap=session.snapshot();assert_eq!(snap.site.as_ref().unwrap().route,bp0.site.as_ref().unwrap().route);let origin=snap.origin.unwrap();
    assert_eq!(&raw[origin.scheduled_step.as_ref().unwrap().expression.clone()],"producer()");assert_eq!(origin.graph.as_ref().unwrap().node,Some(3));
    assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);assert!(engine.renderer.batches.is_empty());
    assert_eq!(engine.debug_step(&mut session,StepKind::Over).unwrap().0,DebugProgress::Stopped);
    let snap=session.snapshot();assert_eq!(snap.site.as_ref().unwrap().route,bp1.site.as_ref().unwrap().route);
    let endpoint=snap.origin.unwrap().scheduled_step.unwrap().endpoint.unwrap();assert_eq!(endpoint.ports.iter().filter(|p|matches!(p.role.as_str(),"valid"|"table_id"|"key")).count(),3);
    finish(&mut engine,&mut session);let mut expected=crate::random::RandomState::seeded(4);expected.integer(1,9).unwrap();assert_eq!(engine.state.random,expected);assert_eq!(engine.state.vars["result"],Value::Bool(false));assert_eq!(engine.renderer.batches.len(),1);
}
#[test]
fn producer_debugger_into_real_function_and_out_returns_to_inspector(){
    let p=actual_program(&native_source(),"C:/actual/source.rvn");let mut engine=consumer_engine(p);let bp=producer_bp(&engine,0);let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp]).unwrap();
    engine.debug_step(&mut session,StepKind::Continue).unwrap();engine.debug_step(&mut session,StepKind::Into).unwrap();assert_eq!(session.snapshot().site.unwrap().function.as_deref(),Some("producer"));
    engine.debug_step(&mut session,StepKind::Out).unwrap();let snap=session.snapshot();assert_eq!(snap.site.unwrap().function.as_deref(),Some("consumer"));assert_eq!(snap.origin.unwrap().scheduled_step.unwrap().index,1);
    finish(&mut engine,&mut session);
}
#[test]
fn producer_debugger_indices_wrong_kind_expression_routes_and_ast_only_engine_refused(){
    let p=actual_program(&native_source(),"C:/actual/source.rvn");let engine=consumer_engine(p.clone());let bp=producer_bp(&engine,0);
    for mutation in 0..4{let mut bad=bp.clone();let site=bad.site.as_mut().unwrap();match mutation{0=>*site.route.last_mut().unwrap()=wire::Route::Producer(99),1=>site.statement=true,2=>site.route.push(wire::Route::Argument(0)),_=>site.function=Some("producer".into())};assert!(engine.validate_debug_breakpoints(&[bad]).is_err());}
    let plain=Engine::from_program(p,Host::default(),16).unwrap();assert!(plain.validate_debug_breakpoints(&[bp]).is_err());
}
#[test]
fn producer_debugger_cold_relocation_preserves_ids_and_source_bytes_without_overlay_authority(){
    let raw=native_source();let p=actual_program(&raw,"C:/original/source.rvn");let moved=actual_program(&raw,"D:/cold/source.rvn");let original=consumer_engine(p);let cold=consumer_engine(moved);
    let bp0=producer_bp(&original,0);let bp1=producer_bp(&cold,0);assert_eq!(bp0.site,bp1.site);
    let origin=cold.functions.debug_origins.iter().find(|o|o.origin.scheduled_step.as_ref().is_some_and(|s|s.scope.as_deref()==Some("consumer")&&s.index==0)).unwrap();assert_eq!(origin.origin.source.as_ref().unwrap().to_string_lossy(),"D:/cold/source.rvn");assert_eq!(origin.origin.graph.as_ref().unwrap().node,Some(3));
    let mut bad=actual_program(&raw,"C:/actual/source.rvn");bad.sources[0].source.push_str("// changed frozen source\n");assert!(crate::debugger_provenance::SourceMap::from_program(&bad).is_ok()); // same AST, new binding
    assert_ne!(crate::debugger_provenance::SourceMap::from_program(&bad).unwrap().content,original.functions.debug_source_map.as_ref().unwrap().content);
}
#[test]
fn producer_debugger_session_owner_and_source_resource_changes_refused_before_effect(){
    let p=actual_program(&native_source(),"C:/actual/source.rvn");let mut engine=consumer_engine(p.clone());let bp=producer_bp(&engine,0);let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp]).unwrap();engine.debug_step(&mut session,StepKind::Continue).unwrap();
    let mut other=consumer_engine(p);let before=serde_json::to_value(&other.state).unwrap();assert!(other.debug_step(&mut session,StepKind::Continue).is_err());assert_eq!(serde_json::to_value(&other.state).unwrap(),before);
    engine.state.vars.insert("external_changed".into(),Value::Int(7));let before=serde_json::to_value(&engine.state).unwrap();assert!(engine.debug_step(&mut session,StepKind::Continue).is_err());assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);assert!(engine.renderer.batches.is_empty());
}
#[test]
fn producer_debugger_narrative_wrapper_site_uses_actual_pc_and_original_producer_span(){
    let raw="scene label start\nflow.consume_v1(\"scope\",\"c\") {capture \"p\"=random(1,9) inspect \"i\"=__rvn_invariant_text(\"x\") set result=__rvn_terminal_v1(\"i\",\"valid\")}\n\"Done\"\n";
    let mut engine=consumer_engine(actual_program(raw,"C:/actual/narrative.rvn"));let page=engine.debug_site_page(0).unwrap();let bp=page.entries.iter().find(|e|e.origin.as_ref().and_then(|o|o.scheduled_step.as_ref()).is_some_and(|s|s.index==0)).unwrap().breakpoint.clone();
    assert!(matches!(engine.script[bp.position.pc],Statement::ScheduledConsumer{..}));assert_eq!(bp.site.as_ref().unwrap().route,vec![wire::Route::Producer(0)]);
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp]).unwrap();assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);assert_eq!(&raw[session.snapshot().origin.unwrap().scheduled_step.unwrap().expression],"random(1,9)");finish(&mut engine,&mut session);
}

#[test]
fn producer_debugger_real_graph0350_source_inspector_has_three_actual_output_pin_ids(){
    let raw=format!("{}\nscene label start\nset result=probe()\n\"Done\"\n",include_str!("../tests/fixtures/producer_origin_graph0350.rvn"));
    let p=actual_program(&raw,"C:/real_graph/main.rvn");let source_map=crate::debugger_provenance::SourceMap::from_program(&p).unwrap();
    for pin in [6,7,8]{assert_eq!(source_map.producer_sites_for_endpoint(0,900,3,pin).count(),1);}assert_eq!(source_map.producer_sites_for_endpoint(0,900,3,999).count(),0);
    let mut context=closed_nonempty_text_context();
    std::sync::Arc::make_mut(&mut context.resources).publish(crate::string_table::StringTableAsset{id:"asset:TableCase".into(),namespace:"VN".into(),entries:[("Hello\n世界".into(),crate::string_table::StringTableEntry{source:"Actual graph text".into(),developer_notes:String::new(),metadata:Default::default()})].into()}).unwrap();
    let mut engine=Engine::from_program_with_debugger_and_text_context(p,Host::default(),16,HashMap::new(),context).unwrap();engine.enter_label("start").unwrap();engine.renderer.batches.clear();
    let entries=engine.debug_site_page(0).unwrap().entries;let producers:Vec<_>=entries.iter().filter(|e|e.origin.as_ref().is_some_and(|o|o.scheduled_step.is_some())).collect();assert_eq!(producers.len(),3);
    let inspector=producers.iter().find(|e|e.origin.as_ref().unwrap().scheduled_step.as_ref().unwrap().index==1).unwrap();let origin=inspector.origin.as_ref().unwrap();assert_eq!(origin.graph.as_ref().unwrap().graph,900);assert_eq!(origin.graph.as_ref().unwrap().node,Some(3));
    let ports=&origin.scheduled_step.as_ref().unwrap().endpoint.as_ref().unwrap().ports;for(role,pin)in[("valid",6),("table_id",7),("key",8)]{assert_eq!(ports.iter().find(|p|p.role==role).unwrap().pin,pin);}
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![inspector.breakpoint.clone()]).unwrap();assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);assert_eq!(session.snapshot().origin.unwrap().scheduled_step.unwrap().index,1);
    finish(&mut engine,&mut session);assert_eq!(engine.state.vars["result"],Value::List(vec![Value::Bool(true),Value::Name(rvn_parser::Name::new("asset:TableCase").unwrap()),Value::Str("Hello\n世界".into())]));
}
#[test]
fn producer_debugger_overlay_pin_and_node_forgery_and_changed_source_refused_atomically(){
    let p=actual_program(&native_source(),"C:/actual/source.rvn");let map=crate::debugger_provenance::SourceMap::from_program(&p).unwrap();let overlay=map.clone().staged_overlay(&map,77).unwrap();let encoded=overlay.encode().unwrap();
    for mutation in 0..3{
        let mut json:serde_json::Value=serde_json::from_str(&encoded).unwrap();let sites=json["map"]["sites"].as_array_mut().unwrap();let site=sites.iter_mut().find(|s|s["origin"]["scheduled_step"].is_object()).unwrap();
        match mutation{0=>site["origin"]["scheduled_step"]["endpoint"]["ports"][0]["pin"]=serde_json::json!(999),1=>site["origin"]["graph"]["node"]=serde_json::json!(999),_=>site["origin"]["scheduled_step"]["source_ordinal"]=serde_json::json!(9)}
        let overlay=crate::debugger_provenance::Overlay::decode(json.to_string().as_bytes()).unwrap();let mut engine=consumer_engine(p.clone());let before=serde_json::to_value(&engine.state).unwrap();assert!(engine.install_debug_source_map(overlay,77).is_err());assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);
    }
    let mut changed=p.clone();changed.sources[0].source.push_str("// AST-identical changed bytes\n");let mut engine=consumer_engine(changed);let before=serde_json::to_value(&engine.state).unwrap();assert!(engine.install_debug_source_map(crate::debugger_provenance::Overlay::decode(encoded.as_bytes()).unwrap(),77).is_err());assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);
}
#[test]
fn producer_debugger_qualified_screen_handler_source_sites_keep_real_definition_ordinals(){
    let raw="function unrelated() {return 1}\nscreen f() {flow.consume_v1(\"scope\",\"c\") {inspect \"i\"=__rvn_invariant_text(\"x\") local result=__rvn_terminal_v1(\"i\",\"valid\")}}\nhandler h() {flow.consume_v1(\"scope2\",\"c\") {inspect \"i\"=__rvn_invariant_text(\"x\") local result=__rvn_terminal_v1(\"i\",\"valid\")}}\nscene label start\n\"Done\"\n";
    let map=crate::debugger_provenance::SourceMap::from_program(&actual_program(raw,"C:/actual/definitions.rvn")).unwrap();
    for(scope,ordinal)in [("screen:f",1),("handler:h",2)]{let origin=map.sites.iter().find(|o|o.scope==scope&&o.origin.scheduled_step.is_some()).unwrap();assert_eq!(origin.origin.scheduled_step.as_ref().unwrap().definition_root_ordinal,ordinal);assert!(raw[origin.declaration.span.clone().unwrap()].starts_with(if ordinal==1{"screen"}else{"handler"}));}
}
#[test]
fn producer_debugger_nested_branch_actual_route_is_reached_without_a_synthetic_statement(){
    let raw="function consumer() {flow.consume_v2(\"scope\",\"branch\") {if true {flow.consume_v1(\"scope\",\"return\") {capture \"p\"=random(1,1) return __rvn_captured_v1(\"p\")}} else {return 0}} return 0}\nscene label start\nset result=consumer()\n\"Done\"\n";
    let mut engine=consumer_engine(actual_program(raw,"C:/actual/branch.rvn"));let bp=producer_bp(&engine,0);assert!(bp.site.as_ref().unwrap().route.contains(&wire::Route::Then));
    let before=serde_json::to_value(&engine.state).unwrap();let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp.clone()]).unwrap();assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);assert_eq!(session.snapshot().site.unwrap().route,bp.site.unwrap().route);assert_eq!(serde_json::to_value(&engine.state).unwrap(),before);
    finish(&mut engine,&mut session);assert_eq!(engine.state.vars["result"],Value::Int(1));let mut expected=crate::random::RandomState::seeded(4);expected.integer(1,1).unwrap();assert_eq!(engine.state.random,expected);
}
#[test]
fn producer_debugger_legacy_terminal_role_selects_only_its_real_pin(){
    let identity="n3:get_string_table_id_and_key|text=5@2,valid=6@0,table_id=7@0,key=8@0";
    let captured="n2:text_from_string_table|table=2@0,key=3@0,result=4@0";
    let mut raw=include_str!("../tests/fixtures/producer_origin_graph0350.rvn").to_owned();
    let original=format!("inspect \"{identity}\" = __rvn_captured_v1(\"{captured}\")");
    let replacement=[("valid","__rvn_text_table_valid"),("table_id","__rvn_text_table_id"),("key","__rvn_text_table_key")].into_iter().map(|(role,builtin)|format!("capture \"p:{role}/{identity}\" = {builtin}(__rvn_captured_v1(\"{captured}\"))")).collect::<Vec<_>>().join("\n        ");
    assert!(raw.contains(&original));raw=raw.replace(&original,&replacement);
    for role in ["valid","table_id","key"]{raw=raw.replace(&format!("__rvn_terminal_v1(\"{identity}\", \"{role}\")"),&format!("__rvn_captured_v1(\"p:{role}/{identity}\")"));}
    let map=crate::debugger_provenance::SourceMap::from_program(&actual_program(&raw,"C:/actual/legacy_mixed.rvn")).unwrap();
    for(role,pin)in [("valid",6),("table_id",7),("key",8)]{let sites:Vec<_>=map.producer_sites_for_endpoint(0,900,3,pin).collect();assert_eq!(sites.len(),1);assert_eq!(sites[0].origin.scheduled_step.as_ref().unwrap().endpoint.as_ref().unwrap().legacy_terminal.as_deref(),Some(role));}
    assert_eq!(map.producer_sites_for_endpoint(0,900,3,5).count(),0,"input pin is not a Legacy output terminal");
    let ordinary=actual_program("function f() {return __rvn_text_table_valid(__rvn_invariant_text(\"x\"))}","C:/actual/stateless.rvn");assert!(crate::debugger_provenance::SourceMap::from_program(&ordinary).unwrap().sites.iter().all(|s|s.origin.scheduled_step.is_none()));
}
#[test]
fn producer_debugger_real_file_inclusion_and_cold_reparse_preserve_sources_and_mapping(){
    let base=std::path::PathBuf::from(std::env::var("RVN_PRODUCER_FIXTURE_DIR").expect("private fixture directory required"));
    let root=base.join(format!("loader-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));let original=root.join("original");let cold=root.join("cold");
    std::fs::create_dir_all(&original).unwrap();std::fs::create_dir_all(&cold).unwrap();
    let lib=format!("// source inclus été 🍃\r\n{}",include_str!("../tests/fixtures/producer_origin_graph0350.rvn").replace('\n',"\r\n"));let main="use \"lib.rvn\"\r\nscene label start\r\nset result=probe()\r\n\"Done\"\r\n";
    for directory in [&original,&cold]{std::fs::write(directory.join("lib.rvn"),&lib).unwrap();std::fs::write(directory.join("main.rvn"),main).unwrap();}
    let p=rvn_parser::parse_file_program_with_uses(original.join("main.rvn")).unwrap();let q=rvn_parser::parse_file_program_with_uses(cold.join("main.rvn")).unwrap();let a=crate::debugger_provenance::SourceMap::from_program(&p).unwrap();let b=crate::debugger_provenance::SourceMap::from_program(&q).unwrap();assert_eq!(a.content,b.content);assert_ne!(a.closure,b.closure);
    let site=b.producer_sites_for_endpoint(1,900,3,8).next().unwrap();assert_eq!(site.origin.scheduled_step.as_ref().unwrap().source_ordinal,1);assert_eq!(&lib[site.origin.scheduled_step.as_ref().unwrap().expression.clone()],"__rvn_captured_v1(\"n2:text_from_string_table|table=2@0,key=3@0,result=4@0\")");assert_eq!(site.origin.source.as_ref().unwrap(),&cold.join("lib.rvn").canonicalize().unwrap());
    let mut engine=consumer_engine(q);let page=engine.debug_site_page(0).unwrap();let bp=page.entries.iter().find(|e|e.origin.as_ref().and_then(|o|o.scheduled_step.as_ref()).is_some_and(|s|s.index==1&&s.scope.as_deref()==Some("probe"))).unwrap().breakpoint.clone();
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![bp]).unwrap();assert_eq!(engine.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);assert_eq!(session.snapshot().origin.unwrap().source.as_ref().unwrap(),&cold.join("lib.rvn").canonicalize().unwrap());finish(&mut engine,&mut session);
    std::fs::write(cold.join("lib.rvn"),format!("{lib}// changed actual source\r\n")).unwrap();let changed=rvn_parser::parse_file_program_with_uses(cold.join("main.rvn")).unwrap();assert_ne!(crate::debugger_provenance::SourceMap::from_program(&changed).unwrap().content,b.content);
}
#[test]
fn producer_debugger_real_graph_source_reroute_recalculates_pin_edge_and_rejects_old_binding(){
    let original=format!("{}\nscene label start\nset result=probe()\n\"Done\"\n",include_str!("../tests/fixtures/producer_origin_graph0350.rvn"));
    let rerouted=original.replace("key=8@0","key=15@0").replace("text=5@2","text=5@222");
    let p=actual_program(&original,"C:/actual/reroute.rvn");let q=actual_program(&rerouted,"C:/actual/reroute.rvn");let old=crate::debugger_provenance::SourceMap::from_program(&p).unwrap();let new=crate::debugger_provenance::SourceMap::from_program(&q).unwrap();assert_ne!(old.content,new.content);
    assert_eq!(new.producer_sites_for_endpoint(0,900,3,8).count(),0);let site=new.producer_sites_for_endpoint(0,900,3,15).next().unwrap();let endpoint=site.origin.scheduled_step.as_ref().unwrap().endpoint.as_ref().unwrap();assert_eq!(endpoint.ports.iter().find(|p|p.role=="text").unwrap().incoming,Some(222));
    let overlay=old.clone().staged_overlay(&old,93).unwrap();let mut cold=consumer_engine(q);let before=serde_json::to_value(&cold.state).unwrap();assert!(cold.install_debug_source_map(overlay,93).is_err());assert_eq!(serde_json::to_value(&cold.state).unwrap(),before);assert!(cold.renderer.batches.is_empty());
}
