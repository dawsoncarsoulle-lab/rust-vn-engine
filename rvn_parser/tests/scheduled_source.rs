use rvn_parser::{parse_program,scheduled_source::scheduled_step_sources};
#[test]
fn scheduled_source_choice_children_keep_actual_option_ranges() {
    use rvn_parser::scheduled_source::ScheduledSourceRoute;
    let choice = "// été 🍃\r\nscene label start\r\nflow.consume_v2(\"root\",\"choice\") {choice {\"one\"=>{flow.consume_v2(\"root\",\"first\") {capture \"p\"=random(1,9) set result=__rvn_captured_v1(\"p\")}} \"two\"=>{flow.consume_v2(\"root\",\"second\") {capture \"q\"=random(2,8) set result=__rvn_captured_v1(\"q\")}}}}\r\n";
    let sites = scheduled_step_sources(&parse_program(choice).unwrap()).unwrap();
    assert_eq!(sites.len(), 2);
    for (index, expression) in ["random(1,9)", "random(2,8)"].iter().enumerate() {
        assert!(sites[index].route.contains(&ScheduledSourceRoute::Choice(index)));
        assert_eq!(&choice[sites[index].expression.clone()], *expression);
        assert_eq!(sites[index].definition_root_ordinal, 0);
    }
}
#[test]
fn scheduled_source_imagemap_children_keep_actual_hotspot_ranges() {
    use rvn_parser::scheduled_source::ScheduledSourceRoute;
    let source="scene label start\nimagemap { background: \"map.png\" hotspot { area: (0,0,20,20) } => { flow.consume_v2(\"root\",\"first\") { capture \"p\"=random(1,9) set result=__rvn_captured_v1(\"p\") } } hotspot { area: (20,0,40,20) } => { flow.consume_v2(\"root\",\"second\") { capture \"q\"=random(2,8) set result=__rvn_captured_v1(\"q\") } } }\n";
    let sites=scheduled_step_sources(&parse_program(source).unwrap()).unwrap();
    assert_eq!(sites.len(),2);
    for (index,expression) in ["random(1,9)","random(2,8)"].iter().enumerate() {
        assert!(sites[index].route.contains(&ScheduledSourceRoute::Hotspot(index)));
        assert_eq!(&source[sites[index].expression.clone()],*expression);
    }
}
fn source()->String{
    "// été 🍃\r\nfunction probe() {\r\nflow.consume_v1(\"g9/n1:function_entry|exec=11@0\",\"n2:function_return|exec=12@0,result=13@0\") {\r\ncapture \"n3:random|a=14@0,b=15@0,result=16@0\" = random(1,9) // first actual value\r\ninspect \"n4:get_string_table_id_and_key|text=17@0,valid=18@0,table_id=19@0,key=20@0\" = __rvn_invariant_text(\"capture \\\"x\\\" = 🍃\")\r\nreturn __rvn_terminal_v1(\"n4:get_string_table_id_and_key|text=17@0,valid=18@0,table_id=19@0,key=20@0\",\"valid\")\r\n}\r\n}\r\nscene label start\r\nset r=probe()\r\n".into()
}
#[test]
fn scheduled_source_utf8_crlf_exact_producer_and_expression_ranges(){
    let raw=source();let program=parse_program(&raw).unwrap();let sites=scheduled_step_sources(&program).unwrap();assert_eq!(sites.len(),2);
    assert_eq!(&raw[sites[0].expression.clone()],"random(1,9)");
    assert!(raw[sites[0].span.clone()].starts_with("capture \"n3:"));
    assert!(raw[sites[1].span.clone()].starts_with("inspect \"n4:"));
    assert_eq!(sites[0].definition_root_ordinal,0);assert_eq!(sites[0].source_ordinal,0);
    assert_eq!(sites[1].endpoint.as_ref().unwrap().ports.iter().find(|p|p.role=="key").unwrap().pin,20);
    assert_eq!(sites[1].endpoint.as_ref().unwrap().node,4);
    assert!(sites[1].span.len()<sites[1].consumer.len());
}
#[test]
fn scheduled_source_inline_steps_and_opaque_labels_have_no_invented_identity(){
    let raw="function f() {flow.consume_v1(\"scope\",\"c\") {capture \"p\"=random(1,2) inspect \"i\"=__rvn_invariant_text(\"x\") return __rvn_terminal_v1(\"i\",\"valid\")}}";
    let sites=scheduled_step_sources(&parse_program(raw).unwrap()).unwrap();assert_eq!(sites.len(),2);assert!(sites.iter().all(|s|s.endpoint.is_none()));
    assert_eq!(&raw[sites[0].expression.clone()],"random(1,2)");
}
#[test]
fn scheduled_source_changed_bytes_or_ast_and_duplicate_root_are_refused(){
    let mut program=parse_program(&source()).unwrap();program.sources[0].source=program.sources[0].source.replace("random(1,9)","random(1,8)");assert!(scheduled_step_sources(&program).is_err());
    let mut program=parse_program(&source()).unwrap();program.script.push(program.script[0].clone());program.origins.push(program.origins[0].clone());assert!(scheduled_step_sources(&program).is_err());
    let mut program=parse_program(&source()).unwrap();program.origins[0].span.children[0].range.start+=1;assert!(scheduled_step_sources(&program).is_err());
}
#[test]
fn scheduled_source_declared_pins_roles_nodes_edges_collisions_are_refused(){
    for(from,to)in [("key=20@0","key=19@0"),("key=20@0","table_id=20@0"),("n4:get_string_table_id_and_key","n3:get_string_table_id_and_key"),("text=17@0","text=14@0"),("n3:random","n03:random"),("g9/","g09/"),("result=13@0","result=13@31"),("text=17@0","text=17@31")]{
        let mut raw=source().replace(from,to);
        if from=="result=13@0"{raw=raw.replace("text=17@0","text=17@31");}
        if from=="text=17@0"&&to=="text=17@31"{raw=raw.replace("result=13@0","result=13@31");}
        assert!(scheduled_step_sources(&parse_program(&raw).unwrap()).is_err(),"{from} -> {to}");
    }
}
#[test]
fn scheduled_source_relocation_content_order_and_nonempty_qualified_definitions(){
    let p=parse_program(&source()).unwrap();let mut moved=p.clone();let path=Some("D:/actual/cold.rvn".into());moved.sources[0].path=path.clone();for o in &mut moved.origins{o.source=path.clone();}
    assert_eq!(scheduled_step_sources(&p).unwrap(),scheduled_step_sources(&moved).unwrap());
    for(kind,expected)in [("screen","screen:f"),("handler","handler:f")]{
        let raw=format!("{kind} f() {{flow.consume_v1(\"scope\",\"c\") {{inspect \"i\"=__rvn_invariant_text(\"x\") local result=__rvn_terminal_v1(\"i\",\"valid\")}}}}");
        let p=parse_program(&raw).unwrap();let s=scheduled_step_sources(&p).unwrap();assert_eq!(s[0].scope.as_deref(),Some(expected));
    }
}
#[test]
fn scheduled_source_budget_is_checked_before_reparse(){
    let mut p=parse_program(&source()).unwrap();p.sources[0].source=" ".repeat(16*1024*1024+1);assert!(scheduled_step_sources(&p).unwrap_err().contains("byte budget"));
}
#[test]
fn scheduled_source_legacy_intrinsic_has_no_scheduled_site(){
    let p=parse_program("function f() {return __rvn_text_table_valid(__rvn_invariant_text(\"x\"))}").unwrap();assert!(scheduled_step_sources(&p).unwrap().is_empty());
}
#[test]
fn scheduled_source_narrative_consumers_share_the_actual_label_definition(){
    let scope="g22/n1:label|exec_out=1@0";
    let raw=format!("scene label start\nflow.consume_v1(\"{scope}\",\"n2:set_variable|exec_in=2@0,exec_out=3@0,value=4@0\") {{capture \"n4:int_literal|value=8@0\"=1 set a=__rvn_captured_v1(\"n4:int_literal|value=8@0\")}}\nflow.consume_v1(\"{scope}\",\"n3:set_variable|exec_in=5@0,exec_out=6@0,value=7@0\") {{capture \"n4:int_literal|value=8@0\"=1 set b=__rvn_captured_v1(\"n4:int_literal|value=8@0\")}}\n");
    let sites=scheduled_step_sources(&parse_program(&raw).unwrap()).unwrap();assert_eq!(sites.len(),2);assert_eq!(sites[0].definition_root_ordinal,0);assert_eq!(sites[1].definition_root_ordinal,0);assert_eq!(sites[0].endpoint,sites[1].endpoint);
}
#[test]
fn scheduled_source_declared_root_kind_must_be_the_actual_definition(){
    let raw=source().replace("n1:function_entry","n1:label");assert!(scheduled_step_sources(&parse_program(&raw).unwrap()).unwrap_err().contains("actual lexical definition"));
}
#[test]
fn scheduled_source_actual_source_ordinals_namespace_declared_graph_ids(){
    let mut p=parse_program(&source()).unwrap();let path=Some("C:/actual/a.rvn".into());p.sources[0].path=path.clone();for o in &mut p.origins{o.source=path.clone();}
    let mut q=parse_program(&source().replace("probe","other")).unwrap();let path=Some("C:/actual/b.rvn".into());q.sources[0].path=path.clone();for o in &mut q.origins{o.source=path.clone();}
    let root_offset=p.script.len();p.script.extend(q.script);p.origins.extend(q.origins);p.sources.extend(q.sources);
    let sites=scheduled_step_sources(&p).unwrap();assert_eq!(sites.len(),4);assert_eq!(sites[2].source_ordinal,1);assert_eq!(sites[2].definition_root_ordinal,root_offset);assert_eq!(sites[0].endpoint,sites[2].endpoint);
}
