use rvn_graph::*;
use rvn_parser::{Expr, Value};
use std::collections::HashMap;

#[test]
fn integer_boolean_strict_runtime_matches_zero_nonzero_across_i64_and_rejects_other_values() {
    let call = |args| Expr::Call { name: "__rvn_int_to_bool".into(), args };
    for value in [i64::MIN, -7, -1, 0, 1, 7, i64::MAX] {
        assert_eq!(rvn_core::eval::eval_expr(&call(vec![Expr::Int(value)]), &HashMap::new()),
            Ok(Value::Bool(value != 0)));
    }
    for args in [vec![], vec![Expr::Int(0), Expr::Int(1)], vec![Expr::Bool(true)],
        vec![Expr::Float(1.0)], vec![Expr::Str("1".into())], vec![Expr::ListLit(vec![])]] {
        assert!(rvn_core::eval::eval_expr(&call(args), &HashMap::new()).is_err());
    }
    for expression in ["__rvn_int_to_bool()", "__rvn_int_to_bool(0, 1)"] {
        let script=rvn_parser::parse(&format!("init {{set flag={expression}}}\nlabel start\nreturn\n")).unwrap();
        assert!(!rvn_parser::validate_logic(&script,false).is_empty());
    }
    assert!(!rvn_parser::is_binding_name("__rvn_int_to_bool"));
    assert!(rvn_core::eval::eval_expr(&Expr::Call {name:"to_int".into(),args:vec![Expr::Bool(true)]},&HashMap::new()).is_err());
}

#[test]
fn integer_boolean_explicit_source_roundtrip_preserves_identity_and_user_function() {
    let source="// Conserver été\nfunction int_to_bool(value){return value+9}\ninit {set flag=false set count=0}\nscene label start\nset flag=__rvn_int_to_bool(-7)\nset count=int_to_bool(1)\n\"Résultat [count]\"\nreturn\n";
    assert_eq!(rvn_parser::builtin_arity("int_to_bool"),None);
    let ast=rvn_parser::parse(source).unwrap();assert!(rvn_parser::validate_logic(&ast,false).is_empty());
    let graphs=import_script(&ast).unwrap();
    let graph=graphs.iter().find(|g|matches!(g.kind,GraphKind::Label {..})).unwrap();
    let cast=graph.nodes.values().find(|n|n.kind==NodeKind::ConvertIntToBool).unwrap();
    assert_eq!(graph.effective_pin_type(graph.pin_by_key(cast.id,"value").unwrap().id),Some(ValueType::Int));
    assert_eq!(graph.effective_pin_type(graph.pin_by_key(cast.id,"result").unwrap().id),Some(ValueType::Bool));
    assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),*graph);
    assert!(graph.nodes.values().any(|n|n.kind==NodeKind::FunctionCall&&n.properties.get("function")==Some(&PropertyValue::String("int_to_bool".into()))));
    let mut project=SourceProject::open(source,&graphs).unwrap();let before=project.graphs().to_vec();
    project.apply_visual(source,&before).unwrap();assert_eq!(project.source(),source);assert_eq!(project.graphs(),before);
    let cold=SourceProject::open(project.source(),project.graphs()).unwrap();assert_eq!(cold.graphs(),project.graphs());
    assert!(transpile_project(&graphs).unwrap().source.contains("__rvn_int_to_bool"));
    let mut engine=rvn_core::Engine::from_program(cold.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
    engine.enter_label("start").unwrap();assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue {character:None,text:"Résultat 10".into()}));
    assert_eq!(engine.state.vars.get("flag"),Some(&Value::Bool(true)));assert_eq!(engine.state.vars.get("count"),Some(&Value::Int(10)));
}

#[test]
fn integer_boolean_new_wire_is_atomic_and_legacy_type_changed_wire_is_never_repaired() {
    let mut graph=GraphDocument::new(GraphId(93),GraphKind::Init);
    let literal=graph.add_catalog_node(NodeKind::Literal,[0.0,0.0]).unwrap();
    graph.set_property(literal,"value",PropertyValue::Int(-7)).unwrap();
    let branch=graph.add_catalog_node(NodeKind::If,[500.0,0.0]).unwrap();
    let output=graph.pin_by_key(literal,"value").unwrap().id;let input=graph.pin_by_key(branch,"condition").unwrap().id;
    let before=graph.clone();assert!(graph.connect(output,input).is_err());assert_eq!(graph,before);
    let connection=graph.connect_with_conversions(output,input,[240.0,48.0]).unwrap();
    assert_eq!(connection.conversions.len(),1);let cast=connection.conversions[0];
    assert_eq!(graph.nodes[&cast].kind,NodeKind::ConvertIntToBool);assert_eq!(graph.nodes[&literal],before.nodes[&literal]);
    assert_eq!(graph.nodes[&branch],before.nodes[&branch]);
    for edge in graph.edges.values(){assert_eq!(graph.pins[&edge.output].value_type,graph.pins[&edge.input].value_type);}
    let after=graph.clone();assert!(graph.connect_with_conversions(output,input,[0.0,0.0]).is_err());assert_eq!(graph,after);
    assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),graph);
    let mut legacy=before;legacy.set_property(literal,"value",PropertyValue::Bool(true)).unwrap();
    let edge=legacy.connect(output,input).unwrap();legacy.set_property(literal,"value",PropertyValue::Int(7)).unwrap();
    let mismatch=legacy.clone();assert_eq!(legacy.materialize_implicit_conversions().unwrap(),0);assert_eq!(legacy,mismatch);
    assert!(legacy.validate().iter().any(|d|d.code=="incompatible_types"&&d.edge==Some(edge)));
    assert_eq!(GraphDocument::from_json(&legacy.to_pretty_json().unwrap()).unwrap(),mismatch);
}

#[test]
fn integer_boolean_import_refuses_wrong_payloads_and_preserves_authored_literal_condition() {
    for expression in ["__rvn_int_to_bool(true)","__rvn_int_to_bool(1.5)","__rvn_int_to_bool(\"1\")"] {
        let ast=rvn_parser::parse(&format!("init {{set flag=false}}\nlabel start\nset flag={expression}\nreturn\n")).unwrap();
        assert!(import_script(&ast).is_err(),"invalid input accepted: {expression}");
    }
    let ast=rvn_parser::parse("label start\nif 1 {\nreturn\n}\nreturn\n").unwrap();
    let graphs=import_script(&ast).unwrap();
    assert!(!graphs.iter().flat_map(|g|g.nodes.values()).any(|n|n.kind==NodeKind::ConvertIntToBool),
        "source import must not invent an integer→Boolean conversion");
    let graph=graphs.iter().find(|g|matches!(g.kind,GraphKind::Label {..})).unwrap();
    let branch=graph.nodes.values().find(|n|n.kind==NodeKind::If).unwrap();
    assert_eq!(graph.pin_by_key(branch.id,"condition").unwrap().default_value,Some(PropertyValue::Int(1)));
    assert_eq!(transpile(graph).unwrap().ast,ast);
    // Connected authored expressions follow the import type contract, too.
    let ast=rvn_parser::parse("init {set count=1}\nlabel start\nif count {\nreturn\n}\nreturn\n").unwrap();
    assert!(import_script(&ast).is_err());
}

#[test]
fn integer_boolean_visual_save_cold_executes_branch_from_modified_source_not_cached_default() {
    let root=std::env::temp_dir().join(format!("rvn-int-bool-{}-{}",std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&root).unwrap();let path=root.join("story.rvn");
    let source="// Préserver été\ninit {set flag=false}\nscene label start\nset flag=__rvn_int_to_bool(0)\nif flag {\n\"nonzero\"\n} else {\n\"zero\"\n}\nreturn\n";
    std::fs::write(&path,source).unwrap();let mut workspace=SourceWorkspace::link(&root,&path,&[]).unwrap();
    let mut graphs=workspace.project().graphs().to_vec();let graph=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Label {..})).unwrap();
    let cast=graph.nodes.values().find(|n|n.kind==NodeKind::ConvertIntToBool).unwrap().id;
    let input=graph.pin_by_key(cast,"value").unwrap().id;
    if let Some(edge)=graph.edges.values().find(|e|e.input==input).cloned(){let literal=graph.pins[&edge.output].node;
        assert_eq!(graph.nodes[&literal].kind,NodeKind::Literal);graph.set_property(literal,"value",PropertyValue::Int(-7)).unwrap();
    }else{graph.set_pin_default(cast,"value",PropertyValue::Int(-7)).unwrap();}
    let graph_id=graph.graph_id;workspace.save(&graphs).unwrap();let saved=std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("// Préserver été"));
    assert_eq!(rvn_parser::parse(&saved).unwrap(),rvn_parser::parse(&source.replace("__rvn_int_to_bool(0)","__rvn_int_to_bool(-7)")).unwrap(),
        "save must change only the authored cast argument; formatting is independent");
    let cold=SourceWorkspace::open(&root).unwrap().unwrap();assert_eq!(cold.project().graphs(),workspace.project().graphs());
    assert_eq!(cold.project().graphs().iter().find(|g|g.graph_id==graph_id).unwrap().nodes[&cast].id,cast);
    for (project,text,flag) in [(SourceProject::open(source,&[]).unwrap(),"zero",false),(cold.project().clone(),"nonzero",true)] {
        let mut engine=rvn_core::Engine::from_program(project.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
        engine.enter_label("start").unwrap();assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue {character:None,text:text.into()}));
        assert_eq!(engine.state.vars.get("flag"),Some(&Value::Bool(flag)));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn boolean_cast_executes_exact_values_and_rejects_non_booleans_and_wrong_arity() {
    let call = |args| Expr::Call { name: "__rvn_bool_to_int".into(), args };
    for (input, expected) in [(false, 0), (true, 1)] {
        assert_eq!(rvn_core::eval::eval_expr(&call(vec![Expr::Bool(input)]), &HashMap::new()), Ok(Value::Int(expected)));
    }
    for args in [vec![], vec![Expr::Bool(true), Expr::Bool(false)], vec![Expr::Int(1)],
        vec![Expr::Float(0.0)], vec![Expr::Str("true".into())], vec![Expr::ListLit(vec![])]] {
        assert!(rvn_core::eval::eval_expr(&call(args), &HashMap::new()).is_err());
    }
    // The established text/numeric conversion has not gained truthiness semantics.
    assert!(rvn_core::eval::eval_expr(&Expr::Call {name: "to_int".into(), args:vec![Expr::Bool(true)]}, &HashMap::new()).is_err());
    for expression in ["__rvn_bool_to_int()", "__rvn_bool_to_int(true, false)"] {
        let script=rvn_parser::parse(&format!("init {{ set result = {expression} }}\nlabel start\nreturn\n")).unwrap();
        assert!(!rvn_parser::validate_logic(&script, false).is_empty());
    }
}

#[test]
fn explicit_boolean_cast_reopens_with_identity_and_preserves_authored_source() {
    let source="// Booléen indépendant\ninit { set flag = true set count = 0 }\nlabel start\nset count = __rvn_bool_to_int(flag)\nreturn\n";
    let ast=rvn_parser::parse(source).unwrap();
    assert!(rvn_parser::validate_logic(&ast,false).is_empty());
    let graphs=import_script(&ast).unwrap();
    let graph=graphs.iter().find(|g|matches!(g.kind,GraphKind::Label {..})).unwrap();
    let cast=graph.nodes.values().find(|n|n.kind==NodeKind::ConvertBoolToInt).unwrap();
    assert_eq!(graph.effective_pin_type(graph.pin_by_key(cast.id,"value").unwrap().id),Some(ValueType::Bool));
    assert_eq!(graph.effective_pin_type(graph.pin_by_key(cast.id,"result").unwrap().id),Some(ValueType::Int));
    assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),*graph);
    let mut project=SourceProject::open(source,&graphs).unwrap();
    let before=project.graphs().to_vec();
    project.apply_visual(source,&before).unwrap();
    assert_eq!(project.source(),source);
    assert_eq!(project.graphs(),before);
    assert_eq!(SourceProject::open(project.source(),project.graphs()).unwrap().graphs(),project.graphs());
    assert!(transpile_project(&graphs).unwrap().source.contains("__rvn_bool_to_int(flag)"));
}

#[test]
fn boolean_visual_connection_inserts_one_typed_cast_and_failed_retry_is_atomic() {
    let mut graph=GraphDocument::new(GraphId(91),GraphKind::Init);
    let literal=graph.add_catalog_node(NodeKind::Literal,[0.0,0.0]).unwrap();
    graph.set_property(literal,"value",PropertyValue::Bool(true)).unwrap();
    let integer=graph.add_catalog_node(NodeKind::ConvertIntToFloat,[500.0,0.0]).unwrap();
    let output=graph.pin_by_key(literal,"value").unwrap().id;
    let input=graph.pin_by_key(integer,"value").unwrap().id;
    let before=graph.clone();
    assert!(graph.connect(output,input).is_err());
    assert_eq!(graph,before);
    let inserted=graph.connect_with_conversions(output,input,[240.0,48.0]).unwrap();
    assert_eq!(inserted.conversions.len(),1);
    let cast=inserted.conversions[0];
    assert_eq!(graph.nodes[&cast].kind,NodeKind::ConvertBoolToInt);
    assert_eq!(graph.nodes[&literal],before.nodes[&literal]);
    assert_eq!(graph.nodes[&integer],before.nodes[&integer]);
    for edge in graph.edges.values() {assert_eq!(graph.pins[&edge.output].value_type,graph.pins[&edge.input].value_type);}
    let after=graph.clone();
    assert!(graph.connect_with_conversions(output,input,[240.0,48.0]).is_err());
    assert_eq!(graph,after);
    assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),graph);
}

#[test]
fn source_import_does_not_coerce_authored_boolean_assignments_or_bad_cast_payloads() {
    let authored=rvn_parser::parse("init {set count=0}\nlabel start\nset count=true\nreturn\n").unwrap();
    let graphs=import_script(&authored).unwrap();
    assert!(!graphs.iter().flat_map(|g|g.nodes.values()).any(|n|n.kind==NodeKind::ConvertBoolToInt));
    let label=graphs.iter().find(|g|matches!(g.kind,GraphKind::Label {..})).unwrap();
    assert_eq!(transpile(label).unwrap().ast,authored[1..]);
    for expression in ["__rvn_bool_to_int(7)", "__rvn_bool_to_int(\"true\")"] {
        let script=rvn_parser::parse(&format!("init {{ set count = 0 }}\nlabel start\nset count = {expression}\nreturn\n")).unwrap();
        assert!(import_script(&script).is_err(),"unexpected implicit cast for {expression}");
    }
}

#[test]
fn existing_boolean_integer_wire_is_not_repaired_by_legacy_upgrade_or_reopen() {
    let mut graph=GraphDocument::new(GraphId(92),GraphKind::Init);
    let boolean=graph.add_catalog_node(NodeKind::Literal,[0.0,0.0]).unwrap();
    graph.set_property(boolean,"value",PropertyValue::Bool(true)).unwrap();
    let integer=graph.add_catalog_node(NodeKind::ConvertIntToFloat,[500.0,0.0]).unwrap();
    let output=graph.pin_by_key(boolean,"value").unwrap().id;
    let input=graph.pin_by_key(integer,"value").unwrap().id;
    graph.pins.get_mut(&input).unwrap().value_type=ValueType::Bool;
    let id=graph.connect(output,input).unwrap();
    // Reconstruct the input's type without discarding its existing edge.
    graph.pins.get_mut(&input).unwrap().value_type=ValueType::Int;
    let before=graph.clone();
    assert_eq!(graph.materialize_implicit_conversions().unwrap(),0);
    assert_eq!(graph,before);
    assert!(graph.validate().iter().any(|d|d.code=="incompatible_types" && d.edge==Some(id)));
    assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),before);
}

#[test]
fn boolean_cast_visual_edit_saves_reopens_and_runs_from_the_authored_program() {
    let root=std::env::temp_dir().join(format!("rvn-bool-cast-{}-{}",std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&root).unwrap();
    let path=root.join("story.rvn");
    let source="// Conserver été\ninit {set count=0}\nscene label start\nset count=__rvn_bool_to_int(true)\n\"Résultat [count]\"\nreturn\n";
    std::fs::write(&path,source).unwrap();
    let mut workspace=SourceWorkspace::link(&root,&path,&[]).unwrap();
    let mut graphs=workspace.project().graphs().to_vec();
    let graph=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Label {..})).unwrap();
    let cast=graph.nodes.values().find(|n|n.kind==NodeKind::ConvertBoolToInt).unwrap().id;
    let input=graph.pin_by_key(cast,"value").unwrap().id;
    if let Some(edge)=graph.edges.values().find(|e|e.input==input).cloned() {
        let literal=graph.pins[&edge.output].node;
        assert_eq!(graph.nodes[&literal].kind,NodeKind::Literal);
        graph.set_property(literal,"value",PropertyValue::Bool(false)).unwrap();
    } else {
        assert_eq!(graph.pins[&input].default_value,Some(PropertyValue::Bool(true)));
        graph.pins.get_mut(&input).unwrap().default_value=Some(PropertyValue::Bool(false));
    }
    let graph_id=graph.graph_id;
    workspace.save(&graphs).unwrap();
    let saved=std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("// Conserver été") && saved.contains("__rvn_bool_to_int(false)"));
    let cold=SourceWorkspace::open(&root).unwrap().unwrap();
    assert_eq!(cold.project().graphs(),workspace.project().graphs());
    assert_eq!(cold.project().graphs().iter().find(|g|g.graph_id==graph_id).unwrap().nodes[&cast].id,cast);
    let mut engine=rvn_core::Engine::from_program(cold.project().resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
    engine.enter_label("start").unwrap();
    assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue {character:None,text:"Résultat 0".into()}));
    assert_eq!(engine.state.vars.get("count"),Some(&Value::Int(0)));
    // Run the original True program separately; it must execute, not use the cached default0.
    let original=SourceProject::open(source,&[]).unwrap();
    let mut engine=rvn_core::Engine::from_program(original.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
    engine.enter_label("start").unwrap();
    assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue {character:None,text:"Résultat 1".into()}));
    assert_eq!(engine.state.vars.get("count"),Some(&Value::Int(1)));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn internal_boolean_intrinsic_preserves_an_existing_user_function_with_the_old_public_name() {
    let source="// Fonction utilisateur à conserver\nfunction bool_to_int(value){return value+9}\ninit {set count=0}\nscene label start\nset count=bool_to_int(1)\n\"Résultat [count]\"\nreturn\n";
    assert_eq!(rvn_parser::builtin_arity("bool_to_int"), None);
    assert!(!rvn_parser::is_binding_name("__rvn_bool_to_int"));
    let ast=rvn_parser::parse(source).unwrap();
    assert!(rvn_parser::validate_logic(&ast,false).is_empty());
    let graphs=import_script(&ast).unwrap();
    assert!(!graphs.iter().any(|g|g.nodes.values().any(|n|n.kind==NodeKind::ConvertBoolToInt)));
    assert!(graphs.iter().any(|g|g.nodes.values().any(|n|n.kind==NodeKind::FunctionCall
        && n.properties.get("function")==Some(&PropertyValue::String("bool_to_int".into())))));
    let mut project=SourceProject::open(source,&graphs).unwrap();
    let before=project.graphs().to_vec();
    project.apply_visual(source,&before).unwrap();
    assert_eq!(project.source(),source);
    assert_eq!(project.graphs(),before);
    let cold=SourceProject::open(project.source(),project.graphs()).unwrap();
    assert_eq!(cold.graphs(),project.graphs());
    let mut engine=rvn_core::Engine::from_program(cold.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
    engine.enter_label("start").unwrap();
    assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue {character:None,text:"Résultat 10".into()}));
    assert_eq!(engine.state.vars.get("count"),Some(&Value::Int(10)));
}


#[path="../../rvn_core/tests/support/strict_native_consumer.rs"]
mod bool_float_native1003;

#[test]
fn bool_float1003_single_node_roundtrips_native_source_and_applies_exact_music_gain_once() {
    for flag in [false,true] {for reroute in [false,true] {
        let root=std::env::temp_dir().join(format!("rvn-bool-float1003-{}-{}-{flag}-{reroute}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();let path=root.join("authored.rvn");
        let source=format!("scene label start\nset sampled=random(1,9)\nset flag={flag}\nset level=0.0\n\"ready\"\nmusic.volume(level)\n\"applied\"\nreturn\n");std::fs::write(&path,source).unwrap();
        let original=rvn_parser::parse_file_program_with_uses(&path).unwrap();let mut graphs=import_script(&original.script).unwrap();
        let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();graph.enable_native_frame_preparation().unwrap();graph.refresh_variable_usage_bindings().unwrap();
        let command=graph.nodes.values().find(|n|n.kind==NodeKind::MusicVolume).unwrap().id;let input=graph.pin_by_key(command,"level").unwrap().id;
        let old=graph.edges.values().find(|e|e.input==input).unwrap().clone();let old_node=graph.pins[&old.output].node;assert_eq!(graph.nodes[&old_node].kind,NodeKind::VariableGet);assert_eq!(graph.edges.values().filter(|e|e.output==old.output).count(),1);graph.remove_node(old_node).unwrap();graph.set_pin_default(command,"level",PropertyValue::Float(0.0)).unwrap();
        let get=graph.add_catalog_node(NodeKind::VariableGet,[100.,300.]).unwrap();graph.set_property(get,"name",PropertyValue::String("flag".into())).unwrap();graph.refresh_variable_usage_bindings().unwrap();let mut output=graph.pin_by_key(get,"value").unwrap().id;assert_eq!(graph.effective_pin_type(output),Some(ValueType::Bool));
        if reroute {let r=graph.add_catalog_node(NodeKind::Reroute,[200.,300.]).unwrap();graph.set_reroute_type(r,ValueType::Bool).unwrap();graph.connect(output,graph.pin_by_key(r,"value").unwrap().id).unwrap();output=graph.pin_by_key(r,"value_out").unwrap().id;}
        let before=graph.clone();assert!(graph.connect(output,input).is_err());assert_eq!(*graph,before,"Never make Bool a directly accepted Float wire");
        let chain=graph.connect_with_conversions(output,input,[400.,300.]).unwrap();assert_eq!(chain.conversions.iter().map(|n|graph.nodes[n].kind).collect::<Vec<_>>(),[NodeKind::ConvertBoolToFloat]);
        assert_eq!(ValueType::Float.conversion_from(&ValueType::Bool),Some(NodeKind::ConvertBoolToFloat));assert!(!ValueType::Float.accepts(&ValueType::Bool));
        let frozen=bool_float_native1003::finish_program(root.clone(),original,graphs,&[NodeKind::MusicVolume]);let binding=rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&frozen).unwrap();let mut engine=rvn_core::Engine::from_program_with_consumer_binding(frozen.clone(),binding,rvn_core::TerminalRenderer,16).unwrap();
        assert!(matches!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text=="ready"));let rng=engine.state.random;let sampled=engine.state.vars["sampled"].clone();engine.advance_dialogue().unwrap();assert!(matches!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text=="applied"));
        let expected=if flag{1.0f32}else{0.0f32};assert_eq!(engine.state.music.volume.to_bits(),expected.to_bits());assert_eq!(engine.state.random,rng);assert_eq!(engine.state.vars["sampled"],sampled);assert_eq!(engine.state.vars["flag"],Value::Bool(flag));
        let saved=engine.capture_save15(&frozen,None,1,"bool-float1003".into()).unwrap().encode().unwrap();let mut cold=rvn_core::Engine::from_program(frozen.clone(),rvn_core::TerminalRenderer,16).unwrap();cold.restore_save15(&frozen,rvn_core::save::consumer_checkpoint::Save15::decode(&saved).unwrap()).unwrap();assert_eq!(cold.state.music.volume.to_bits(),expected.to_bits());assert_eq!(cold.state.random,rng);assert_eq!(cold.state.vars["sampled"],sampled);
        std::fs::remove_dir_all(root).unwrap();
    }}
}

#[test]
fn bool_float1003_wrong_direction_occupied_input_and_collection_refuse_atomically() {
    let mut graph=GraphDocument::new(GraphId(1003),GraphKind::Init);let boolean=graph.add_catalog_node(NodeKind::Literal,[0.,0.]).unwrap();graph.set_property(boolean,"value",PropertyValue::Bool(true)).unwrap();let color=graph.add_catalog_node(NodeKind::MakeColor,[600.,0.]).unwrap();let output=graph.pin_by_key(boolean,"value").unwrap().id;let input=graph.pin_by_key(color,"r").unwrap().id;
    let before=graph.clone();assert!(graph.connect_with_conversions(input,output,[200.,0.]).is_err());assert_eq!(graph,before);
    let float=graph.add_catalog_node(NodeKind::Literal,[0.,100.]).unwrap();graph.set_property(float,"value",PropertyValue::Float(0.5)).unwrap();graph.connect(graph.pin_by_key(float,"value").unwrap().id,input).unwrap();let occupied=graph.clone();assert!(graph.connect_with_conversions(output,input,[200.,0.]).is_err());assert_eq!(graph,occupied,"Failed chain must preserve old edge and all counters/IDs");
    assert_eq!(ValueType::List(Box::new(ValueType::Float)).conversion_path_from(&ValueType::List(Box::new(ValueType::Bool))),None,"Scalar composition must not grant elementwise container conversion");
    let chain=ValueType::Float.conversion_path_from(&ValueType::Bool).unwrap();assert_eq!(chain,[NodeKind::ConvertBoolToFloat]);
}

#[test]
fn bool_float1003_existing_invalid_wire_is_not_upgraded_or_repaired_on_cold_reopen() {
    let mut graph=GraphDocument::new(GraphId(1004),GraphKind::Init);let literal=graph.add_catalog_node(NodeKind::Literal,[0.,0.]).unwrap();graph.set_property(literal,"value",PropertyValue::Float(1.0)).unwrap();let color=graph.add_catalog_node(NodeKind::MakeColor,[600.,0.]).unwrap();let output=graph.pin_by_key(literal,"value").unwrap().id;let input=graph.pin_by_key(color,"r").unwrap().id;let edge=graph.connect(output,input).unwrap();
    graph.set_property(literal,"value",PropertyValue::Bool(true)).unwrap();let before=graph.clone();assert_eq!(graph.materialize_implicit_conversions().unwrap(),0);assert_eq!(graph,before);assert!(graph.validate().iter().any(|d|d.code=="incompatible_types"&&d.edge==Some(edge)));
    let cold=GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();assert_eq!(cold,before);assert!(cold.validate().iter().any(|d|d.code=="incompatible_types"&&d.edge==Some(edge)));assert!(!cold.nodes.values().any(|n|matches!(n.kind,NodeKind::ConvertBoolToInt|NodeKind::ConvertIntToFloat|NodeKind::ConvertBoolToFloat)));
}


#[test]
fn bool_float1003_strict_existing_expression_rejects_non_bool_arity_and_forged_float_zero() {
    for flag in [false,true] {
        let value=Expr::BinOp{op:rvn_parser::BinOpKind::Add,left:Box::new(Expr::Call{name:"__rvn_bool_to_int".into(),args:vec![Expr::Bool(flag)]}),right:Box::new(Expr::Float(0.0))};
        assert_eq!(rvn_core::eval::eval_expr(&value,&HashMap::new()),Ok(Value::Float(if flag{1.0}else{0.0})));
    }
    for args in [vec![],vec![Expr::Bool(true),Expr::Bool(false)],vec![Expr::Int(1)],vec![Expr::Float(0.0)],vec![Expr::Str("true".into())],vec![Expr::ListLit(vec![])]] {
        let value=Expr::BinOp{op:rvn_parser::BinOpKind::Add,left:Box::new(Expr::Call{name:"__rvn_bool_to_int".into(),args}),right:Box::new(Expr::Float(0.0))};
        assert!(rvn_core::eval::eval_expr(&value,&HashMap::new()).is_err());
    }
    let source="scene label start\nset result=0.0\n\"ready\"\nreturn\n";
    let mut graphs=import_script(&rvn_parser::parse(source).unwrap()).unwrap();let graph=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();graph.enable_native_frame_preparation().unwrap();
    let set=graph.nodes.values().find(|n|n.kind==NodeKind::SetVariable).unwrap().id;let input=graph.pin_by_key(set,"value").unwrap().id;
    if let Some(edge)=graph.edges.values().find(|e|e.input==input).cloned(){let node=graph.pins[&edge.output].node;graph.remove_node(node).unwrap();}
    let cast=graph.add_catalog_node(NodeKind::ConvertBoolToFloat,[240.,200.]).unwrap();graph.connect(graph.pin_by_key(cast,"result").unwrap().id,input).unwrap();
    let before=graph.clone();let strict=prepare_native_frame_graph(graph).unwrap();let raw=&strict.source;
    let capture=raw.lines().find(|line|line.trim_start().starts_with("capture ")&&line.contains(":convert_bool_to_float|")).unwrap();assert!(capture.ends_with("+ 0.0)"));
    for expression in [capture.replace("+ 0.0)","+ 1.0)"),capture.replace("__rvn_bool_to_int(false)","__rvn_bool_to_int(1)"),capture.replace("__rvn_bool_to_int(false)","random(1,9)")] {
        assert_ne!(&expression,capture);let forged=raw.replacen(capture,&expression,1);let ast=rvn_parser::parse(&forged).unwrap();assert!(import_native_frame_script(&ast).is_err());assert_eq!(*graph,before);
    }
}

#[test]
fn bool_float1003_static_timer_authenticates_literal_default_reroutes_and_refuses_dynamic_source() {
    for flag in [false,true] {for style in 0..3 {
        let root=std::env::temp_dir().join(format!("rvn-static-bool-float1003-{}-{flag}-{style}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir(&root).unwrap();let file=root.join("authored.rvn");
        let source="scene label start\nset sampled=random(1,9)\n\"ready\"\ntimer 0.25 => jump start\n\"applied\"\nreturn\n";std::fs::write(&file,source).unwrap();let original=rvn_parser::parse_file_program_with_uses(&file).unwrap();let mut graphs=import_script(&original.script).unwrap();
        let graph=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();graph.enable_native_frame_preparation().unwrap();let timer=graph.nodes.values().find(|n|n.kind==NodeKind::Timer).unwrap().id;let input=graph.pin_by_key(timer,"duration").unwrap().id;
        if let Some(edge)=graph.edges.values().find(|e|e.input==input).cloned(){graph.remove_node(graph.pins[&edge.output].node).unwrap();}
        let cast=graph.add_catalog_node(NodeKind::ConvertBoolToFloat,[400.,300.]).unwrap();
        if style==0{graph.set_pin_default(cast,"value",PropertyValue::Bool(flag)).unwrap();}
        else{let literal=graph.add_catalog_node(NodeKind::Literal,[100.,300.]).unwrap();graph.set_property(literal,"value",PropertyValue::Bool(flag)).unwrap();let mut output=graph.pin_by_key(literal,"value").unwrap().id;
            if style==2{let route=graph.add_catalog_node(NodeKind::Reroute,[240.,300.]).unwrap();graph.set_reroute_type(route,ValueType::Bool).unwrap();graph.connect(output,graph.pin_by_key(route,"value").unwrap().id).unwrap();output=graph.pin_by_key(route,"value_out").unwrap().id;}
            graph.connect(output,graph.pin_by_key(cast,"value").unwrap().id).unwrap();}
        graph.connect(graph.pin_by_key(cast,"result").unwrap().id,input).unwrap();let frozen=bool_float_native1003::finish_program(root.clone(),original,graphs,&[NodeKind::Timer]);
        let mut engine=rvn_core::Engine::from_program(frozen.clone(),rvn_core::TerminalRenderer,16).unwrap();engine.step_until_interaction().unwrap();let rng=engine.state.random;engine.advance_dialogue().unwrap();assert!(matches!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text=="applied"));
        let tasks=&engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().tasks;assert_eq!(tasks.timer.as_ref().unwrap().duration,if flag{1.0}else{0.0});assert_eq!(engine.state.random,rng);
        let raw=&frozen.sources[0].source;let capture=raw.lines().find(|l|l.trim_start().starts_with("capture ")&&l.contains(":convert_bool_to_float|")).unwrap();
        let changed=raw.replacen(capture,&capture.replace("+ 0.0)","+ 1.0)"),1);let bad=root.join("forged.rvn");std::fs::write(&bad,changed).unwrap();
        let error=match rvn_parser::parse_file_program_with_uses(&bad){Err(error)=>error,Ok(program)=>rvn_core::Engine::from_program(program,rvn_core::TerminalRenderer,16).err().expect("Static source forgery must refuse").to_string()};assert!(error.contains("Static BoolToFloat cannot alter its actual float-zero operand"),"{error}");
        for operand in ["flag", "__rvn_int_to_bool(1)", "random(1,9)"] {
            let (header,_) = capture.split_once(" = ").expect("Actual capture assignment");
            let forged_capture = format!("{header} = (__rvn_bool_to_int({operand}) + 0.0)");
            let bad = root.join("forged-computed.rvn"); std::fs::write(&bad,raw.replacen(capture,&forged_capture,1)).unwrap();
            let refused = match rvn_parser::parse_file_program_with_uses(&bad) {
                Err(_) => true,
                Ok(program) => rvn_core::Engine::from_program(program,rvn_core::TerminalRenderer,16).is_err(),
            };
            assert!(refused,"Static BoolToFloat must not admit GET, function or RNG: {operand}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }}
    let ast=rvn_parser::parse("scene label start\nset flag=true\ntimer 1 => jump start\nreturn\n").unwrap();let mut graphs=import_script(&ast).unwrap();let graph=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();graph.enable_native_frame_preparation().unwrap();let timer=graph.nodes.values().find(|n|n.kind==NodeKind::Timer).unwrap().id;let input=graph.pin_by_key(timer,"duration").unwrap().id;if let Some(edge)=graph.edges.values().find(|e|e.input==input).cloned(){graph.remove_node(graph.pins[&edge.output].node).unwrap();}graph.set_pin_default(timer,"duration",PropertyValue::Float(1.0)).unwrap();
    let get=graph.add_catalog_node(NodeKind::VariableGet,[100.,300.]).unwrap();graph.set_property(get,"name",PropertyValue::String("flag".into())).unwrap();graph.refresh_variable_usage_bindings().unwrap();let cast=graph.add_catalog_node(NodeKind::ConvertBoolToFloat,[300.,300.]).unwrap();graph.connect(graph.pin_by_key(get,"value").unwrap().id,graph.pin_by_key(cast,"value").unwrap().id).unwrap();graph.connect(graph.pin_by_key(cast,"result").unwrap().id,input).unwrap();let before=graph.clone();let error=prepare_native_frame_graph(graph).unwrap_err().to_string();assert!(error.contains("requiert une constante RVN, pas une expression dynamique"),"{error}");assert_eq!(*graph,before);
}

#[test]
fn branch1003_fresh_catalogue_default_true_executes_and_explicit_or_serialized_false_stays_false() {
    let source="scene label start\nif false {\n\"true branch\"\n} else {\n\"false branch\"\n}\nreturn\n";
    let ast=rvn_parser::parse(source).unwrap();let graphs=import_script(&ast).unwrap();let graph=graphs.iter().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();let branch=graph.nodes.values().find(|n|n.kind==NodeKind::If).unwrap().id;
    assert_eq!(graph.pin_by_key(branch,"condition").unwrap().default_value,Some(PropertyValue::Bool(false)));let json=graph.to_pretty_json().unwrap();assert_eq!(GraphDocument::from_json(&json).unwrap(),*graph);
    let mut unchanged=SourceProject::open(source,&graphs).unwrap();unchanged.apply_visual(source,&graphs).unwrap();assert_eq!(unchanged.source(),source);assert_eq!(unchanged.graphs(),graphs);
    let mut fresh=GraphDocument::new(GraphId::new(1003002),GraphKind::Label{name:"start".into()});let entry=fresh.add_catalog_node(NodeKind::Label,[0.,0.]).unwrap();let branch=fresh.add_catalog_node(NodeKind::If,[240.,0.]).unwrap();assert_eq!(fresh.pin_by_key(branch,"condition").unwrap().default_value,Some(PropertyValue::Bool(true)));
    let yes=fresh.add_catalog_node(NodeKind::Dialogue,[480.,0.]).unwrap();let no=fresh.add_catalog_node(NodeKind::Dialogue,[480.,240.]).unwrap();
    // Dialogue consumes a visible Text producer; its hidden legacy default
    // never substitutes for the required real business-data connection.
    for (dialogue,text,y) in [(yes,"true branch",0.),(no,"false branch",240.)] {
        let value=fresh.add_catalog_node(NodeKind::TextValue,[240.,y+160.]).unwrap();fresh.set_property(value,"value",PropertyValue::String(text.into())).unwrap();
        let input=fresh.pin_by_key(dialogue,"text").unwrap().id;let output=fresh.pin_by_key(value,"value").unwrap().id;fresh.connect(output,input).unwrap();
        assert_eq!(fresh.edges.values().filter(|edge|edge.input==input&&edge.output==output).count(),1);
    }
    assert_eq!(fresh.nodes.values().filter(|node|node.kind==NodeKind::TextValue).count(),2);
    for(a,out,b)in[(entry,"exec_out",branch),(branch,"then",yes),(branch,"else",no)]{fresh.connect(fresh.pin_by_key(a,out).unwrap().id,fresh.pin_by_key(b,"exec_in").unwrap().id).unwrap();}
    let mut old_false=fresh.clone();old_false.set_pin_default(branch,"condition",PropertyValue::Bool(false)).unwrap();
    for (g,expected) in [(graph.clone(),"false branch"),(fresh,"true branch"),(old_false,"false branch")] {
        let cold=GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();assert_eq!(cold,g);let compiled=transpile(&cold).unwrap();let restored=import_script(&compiled.ast).unwrap();assert!(restored.iter().any(|g|g.nodes.values().any(|n|n.kind==NodeKind::If)));
        let mut engine=rvn_core::Engine::new(compiled.ast,rvn_core::TerminalRenderer,16).unwrap();assert!(matches!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text==expected));
    }
}


#[test]
fn bool_float1003_legacy_canonical_source_stays_one_node_and_native_pair_keeps_both_identities() {
    let source="// Preserve canonical authored value\ninit {set flag=true}\nscene label start\nset result=(__rvn_bool_to_int(flag)+0.0)\n\"[result]\"\nreturn\n";
    let graphs=import_script(&rvn_parser::parse(source).unwrap()).unwrap();let label=graphs.iter().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();assert_eq!(label.nodes.values().filter(|n|n.kind==NodeKind::ConvertBoolToFloat).count(),1);assert!(!label.nodes.values().any(|n|n.kind==NodeKind::ConvertBoolToInt));
    let mut project=SourceProject::open(source,&graphs).unwrap();project.apply_visual(source,&graphs).unwrap();assert_eq!(project.source(),source);let cold=SourceProject::open(project.source(),project.graphs()).unwrap();assert_eq!(cold.graphs(),project.graphs());let mut engine=rvn_core::Engine::from_program(cold.resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();engine.enter_label("start").unwrap();engine.step_until_interaction().unwrap();assert_eq!(engine.state.vars["result"],Value::Float(1.0));
    for flag in [false,true] {for style in 0..3 {
    let mut docs=import_script(&rvn_parser::parse("scene label start\nset result=0.0\nreturn\n").unwrap()).unwrap();let graph=docs.iter_mut().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();graph.enable_native_frame_preparation().unwrap();let set=graph.nodes.values().find(|n|n.kind==NodeKind::SetVariable).unwrap().id;let input=graph.pin_by_key(set,"value").unwrap().id;if let Some(edge)=graph.edges.values().find(|e|e.input==input).cloned(){graph.remove_node(graph.pins[&edge.output].node).unwrap();}
    let integer=graph.add_catalog_node(NodeKind::ConvertBoolToInt,[240.,200.]).unwrap();graph.set_pin_default(integer,"value",PropertyValue::Bool(flag)).unwrap();
    if style>0 {let literal=graph.add_catalog_node(NodeKind::Literal,[0.,200.]).unwrap();graph.set_property(literal,"value",PropertyValue::Bool(flag)).unwrap();let mut output=graph.pin_by_key(literal,"value").unwrap().id;if style==2 {let reroute=graph.add_catalog_node(NodeKind::Reroute,[120.,200.]).unwrap();graph.set_reroute_type(reroute,ValueType::Bool).unwrap();graph.connect(output,graph.pin_by_key(reroute,"value").unwrap().id).unwrap();output=graph.pin_by_key(reroute,"value_out").unwrap().id;}graph.connect(output,graph.pin_by_key(integer,"value").unwrap().id).unwrap();}
    let float=graph.add_catalog_node(NodeKind::ConvertIntToFloat,[480.,200.]).unwrap();graph.connect(graph.pin_by_key(integer,"result").unwrap().id,graph.pin_by_key(float,"value").unwrap().id).unwrap();graph.connect(graph.pin_by_key(float,"result").unwrap().id,input).unwrap();let before=graph.clone();let compiled=prepare_native_frame_graph(graph).unwrap();let restored=import_native_frame_script(&compiled.ast).unwrap();let restored=restored.iter().find(|g|g.graph_id==graph.graph_id).unwrap();assert_eq!(restored.edges,before.edges);assert_eq!(restored.nodes[&integer].kind,NodeKind::ConvertBoolToInt);assert_eq!(restored.nodes.keys().collect::<Vec<_>>(),before.nodes.keys().collect::<Vec<_>>());assert_eq!(restored.pins.keys().collect::<Vec<_>>(),before.pins.keys().collect::<Vec<_>>());if style==0 {assert_eq!(restored.pin_by_key(integer,"value").unwrap().default_value,Some(PropertyValue::Bool(flag)));assert!(!restored.edges.values().any(|edge|edge.input==restored.pin_by_key(integer,"value").unwrap().id));}assert_eq!(restored.nodes[&float].kind,NodeKind::ConvertIntToFloat);assert!(!restored.nodes.values().any(|n|n.kind==NodeKind::ConvertBoolToFloat));assert_eq!(prepare_native_frame_graph(restored).unwrap().ast,compiled.ast);
    }}
}
