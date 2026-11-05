use rvn_graph::*;
use rvn_parser::{Statement,Value};

fn native_init(body:&str)->String {
    let source=format!("init {{{body}}}\nlabel start\nreturn\n");
    let mut graphs=import_script(&rvn_parser::parse(&source).unwrap()).unwrap();
    let init=graphs.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();
    init.enable_native_frame_preparation().unwrap();
    prepare_native_frame_graph(init).unwrap().source
}
fn program(body:&str)->String {
    format!("{}\n{}\nfunction read_shared(){{return shared}}\nscene label start\n\"[read_shared()]\"\nreturn\n",
        native_init(body),include_str!("fixtures/native_source_single.rvn"))
}
#[test]
fn native_init_literal_global_is_shared_in_every_owned_scope_after_cold_reopen() {
    let source=program("set shared=\"été\"");
    let project=SourceProject::open(&source,&[]).unwrap();
    for graph in project.graphs() {
        assert_eq!(graph.variables["shared"].value_type,ValueType::String,"{:?}",graph.kind);
        assert_eq!(graph.variables["shared"].default_value,PropertyValue::String("été".into()));
        assert_eq!(graph.variable_scope("shared"),Some(VariableScope::Global));
    }
    let cold=SourceProject::open(project.source(),project.graphs()).unwrap();
    assert_eq!(cold.graphs(),project.graphs());
    let engine=rvn_core::Engine::from_program_with_debugger(rvn_parser::parse_program(&source).unwrap(),rvn_core::TerminalRenderer,4).unwrap();
    assert_eq!(engine.state.vars.get("shared"),Some(&Value::Str("été".into())));
}
#[test]
fn native_init_calculation_is_typed_but_not_presented_as_executed_literal() {
    let source=program("set shared=1+2");
    let project=SourceProject::open(&source,&[]).unwrap();
    for graph in project.graphs() {assert_eq!(graph.variables["shared"].value_type,ValueType::Int);}
    assert_eq!(project.global_declaration_values(project.graphs()).unwrap()["shared"],GlobalDeclarationValue::Runtime);
    let engine=rvn_core::Engine::from_program_with_debugger(rvn_parser::parse_program(&source).unwrap(),rvn_core::TerminalRenderer,4).unwrap();
    assert_eq!(engine.state.vars.get("shared"),Some(&Value::Int(3)));
}
#[test]
fn native_init_string_conversion_and_capture_types_reach_legacy_neighbours() {
    let source=program("set shared=string_to_text(\"left\"+\"right\")");
    let project=SourceProject::open(&source,&[]).unwrap();
    for graph in project.graphs() {assert_eq!(graph.variables["shared"].value_type,ValueType::InterpolatedText);}
    assert_eq!(project.global_declaration_values(project.graphs()).unwrap()["shared"],GlobalDeclarationValue::Runtime);
}
#[test]
fn native_init_order_multiple_writes_preserves_source_authority_and_runtime() {
    let source=program("set shared=1 set shared=shared+2");
    let project=SourceProject::open(&source,&[]).unwrap();let before=project.graphs().to_vec();
    let values=project.global_declaration_values(project.graphs()).unwrap();
    assert_eq!(values["shared"],GlobalDeclarationValue::Runtime);
    assert_eq!(project.graphs(),before);assert_eq!(project.source(),source);
    let engine=rvn_core::Engine::from_program_with_debugger(rvn_parser::parse_program(&source).unwrap(),rvn_core::TerminalRenderer,4).unwrap();
    assert_eq!(engine.state.vars.get("shared"),Some(&Value::Int(3)));
}
#[test]
fn native_global_name_discovery_does_not_turn_explicit_local_into_global() {
    let source=format!("{}\nfunction local_reader(){{local shared=2 return shared}}\nscene label start\nreturn\n",native_init("set shared=\"global\""));
    let project=SourceProject::open(&source,&[]).unwrap();
    let local=project.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="local_reader")).unwrap();
    assert_eq!(local.variable_scope("shared"),Some(VariableScope::Local));
    assert_eq!(local.variables["shared"].value_type,ValueType::Any,"Historical calculation-local type joining is unchanged");
    let init=project.graphs().iter().find(|g|g.kind==GraphKind::Init).unwrap();
    assert_eq!(init.variables["shared"].value_type,ValueType::String);
    assert_eq!(init.variable_scope("shared"),Some(VariableScope::Global));
}
#[test]
fn malformed_native_producer_identity_refuses_actual_import_without_cache_admission() {
    let source=program("set shared=1+2");
    let parsed=rvn_parser::parse_program(&source).unwrap();
    let [Statement::Init{body}, ..]=parsed.script.as_slice()else{panic!()};
    let [Statement::ScheduledConsumer{steps,..}]=body.as_slice()else{panic!()};
    let id=match &steps[0] {rvn_parser::ConsumerStep::Capture{producer,..}|rvn_parser::ConsumerStep::InspectTextTable{producer,..}=>producer};
    let forged=source.replace(id,"not_an_identity");
    assert!(SourceProject::open(&forged,&[]).is_err());
}

fn native_scope(source:&str,kind:GraphKind)->String {
    let mut graphs=import_script(&rvn_parser::parse_program(source).unwrap().script).unwrap();
    let graph=graphs.iter_mut().find(|g|g.kind==kind).unwrap();
    graph.enable_native_frame_preparation().unwrap();prepare_native_frame_graph(graph).unwrap().source
}
#[test]
fn native_fresh_scene_global_write_before_local_remains_global_and_runtime_local_is_distinct() {
    let scope=native_scope("scene label start\nset score=\"global\"\nlocal score=2\nset score=7\n",GraphKind::Label{name:"start".into()});
    let source=format!("function read_score(){{return score}}\n{scope}");
    let project=SourceProject::open(&source,&[]).unwrap();
    let reader=project.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="read_score")).unwrap();
    assert_eq!(reader.variables["score"].value_type,ValueType::String);
    assert_eq!(reader.variable_scope("score"),Some(VariableScope::Global));
    let mut engine=rvn_core::Engine::from_program_with_debugger(rvn_parser::parse_program(&source).unwrap(),rvn_core::TerminalRenderer,4).unwrap();
    assert_eq!(engine.step_until_interaction().unwrap(),None);
    assert_eq!(engine.state.vars.get("score"),Some(&Value::Str("global".into())));
    assert_eq!(engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals.get("score"),Some(&Value::Int(7)));
}
#[test]
fn native_handler_write_before_local_does_not_lose_project_global() {
    let scope=native_scope("handler event(evt){set score=\"global\" local score=2}\nlabel start\nreturn\n",GraphKind::Handler{name:"event".into()});
    let source=format!("{scope}\nfunction read_score(){{return score}}\nscene label start\nreturn\n");
    let project=SourceProject::open(&source,&[]).unwrap();
    let reader=project.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="read_score")).unwrap();
    assert_eq!(reader.variable_scope("score"),Some(VariableScope::Global));
    assert_eq!(reader.variables["score"].value_type,ValueType::String);
}
#[test]
fn native_handler_parameter_is_already_local_before_first_consumer() {
    let scope=native_scope("handler event(score){set score=\"local\"}\nlabel start\nreturn\n",GraphKind::Handler{name:"event".into()});
    let source=format!("{scope}\nfunction neighbour(){{return 0}}\nscene label start\nreturn\n");
    let project=SourceProject::open(&source,&[]).unwrap();
    let reader=project.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="neighbour")).unwrap();
    assert!(!reader.variables.contains_key("score"));
    let handler=project.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Handler{name}if name=="event")).unwrap();
    assert_eq!(handler.variable_scope("score"),Some(VariableScope::Parameter));
}
#[test]
fn native_definite_branches_keep_post_branch_set_local_but_optional_branch_keeps_global_possible() {
    for (body,global) in [
        ("if true {local score=2} else {local score=3}\nset score=4\nreturn",false),
        ("if false {local score=2}\nset score=4\nreturn",true),
        ("while false {local score=2}\nset score=4\nreturn",true),
    ] {
        let scope=native_scope(&format!("scene label start\n{body}\n"),GraphKind::Label{name:"start".into()});
        let source=format!("function neighbour(){{return 0}}\n{scope}");
        let project=SourceProject::open(&source,&[]).unwrap();
        let other=project.graphs().iter().find(|g|matches!(&g.kind,GraphKind::Function{name}if name=="neighbour")).unwrap();
        assert_eq!(other.variables.contains_key("score"),global,"{body}");
    }
}
