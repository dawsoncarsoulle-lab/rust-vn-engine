use super::*;
struct Fixture{root:std::path::PathBuf,workspace:SourceWorkspace,graphs:Vec<GraphDocument>,declaration:GraphId,identity:ParameterId}
impl Drop for Fixture{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.root);}}
impl Fixture{
    fn new(source:&str)->Self{
        let root=std::env::temp_dir().join(format!("rvn-parameter-refs-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();let path=root.join("main.rvn");std::fs::write(&path,source).unwrap();
        let mut next=0u64;let prepared=crate::import_script_with_parameter_allocator(&rvn_parser::parse(source).unwrap(),||{next+=1;Ok(format!("{next:032x}"))}).unwrap();
        let workspace=SourceWorkspace::link(&root,&path,&prepared).unwrap();let graphs=workspace.project().graphs().to_vec();
        let graph=graphs.iter().find(|graph|matches!(&graph.kind,GraphKind::UnitFunction{name}if name=="sample")).unwrap();
        let declaration=graph.graph_id;let identity=crate::unit_graph_source1003::parameters(graph).unwrap()[0].parameter_id.clone().unwrap();
        Self{root,workspace,graphs,declaration,identity}
    }
    fn bytes(&self)->(Vec<u8>,Vec<u8>){(std::fs::read(self.root.join("main.rvn")).unwrap(),std::fs::read(self.root.join(".rvn-authoring.json")).unwrap())}
    fn used(&self,graphs:&[GraphDocument])->Result<bool,String>{self.workspace.unit_parameter_is_referenced(graphs,self.declaration,&self.identity)}
}
#[test]
fn parameter_refs1003_real_owner_ignores_homonymous_function_handler_and_global_bindings(){
    let fixture=Fixture::new("init {set alpha=99 set result=0 set outside=0}\nfunction echo(alpha){return alpha}\nhandler observer(alpha){set outside=alpha}\nfunction sample(alpha) -> () {set result=1}\nscene label start\nreturn");
    let before=fixture.bytes();let snapshot=fixture.graphs.clone();assert!(!fixture.used(&fixture.graphs).unwrap());
    let mut without_cache=fixture.graphs.clone();without_cache.iter_mut().find(|graph|graph.graph_id==fixture.declaration).unwrap().variables.remove("alpha");
    assert!(!fixture.used(&without_cache).unwrap());assert_eq!(fixture.graphs,snapshot);assert_eq!(fixture.bytes(),before);
}
#[test]
fn parameter_refs1003_quoted_parameter_reads_conditions_and_unit_call_arguments_resolve_in_owner(){
    let fixture=Fixture::new("init {set result=0}\nfunction sink(value) -> () {set result=value}\nfunction sample(`not a name`) -> () {if `not a name`>0 {call sink(`not a name`)}}\nscene label start\nreturn");
    let before=fixture.bytes();assert!(fixture.used(&fixture.graphs).unwrap());assert_eq!(fixture.bytes(),before);
}
#[test]
fn parameter_refs1003_callee_names_and_bracketed_string_data_do_not_read_parameter(){
    let fixture=Fixture::new("init {set result=0}\nfunction alpha(){return 7}\nfunction sample(alpha) -> () {set result=alpha() local literal=\"[alpha]\"}\nscene label start\nreturn");
    assert!(!fixture.used(&fixture.graphs).unwrap());
}
#[test]
fn parameter_refs1003_unit_rename_preserves_raw_text_payloads_and_unfinished_brackets(){
    let fixture=Fixture::new("init {set result=0}\nfunction sample(alpha) -> () {set result=1}\nscene label start\nreturn");
    let mut graphs=fixture.graphs.clone();let graph=graphs.iter_mut().find(|graph|graph.graph_id==fixture.declaration).unwrap();
    let first=graph.add_catalog_node(NodeKind::TextValue,[200.,200.]).unwrap();let second=graph.add_catalog_node(NodeKind::TextValue,[200.,400.]).unwrap();
    graph.set_property(first,"value",PropertyValue::String("[alpha]".into())).unwrap();
    graph.set_property(second,"value",PropertyValue::String("a valid string with [unfinished".into())).unwrap();
    let before=fixture.bytes();let snapshot=graphs.clone();assert!(!fixture.used(&graphs).unwrap());
    let renamed=fixture.workspace.prepare_unit_parameter_rename(&graphs,fixture.declaration,&fixture.identity,"quoted parameter").unwrap();
    let graph=renamed.iter().find(|graph|graph.graph_id==fixture.declaration).unwrap();
    assert_eq!(graph.nodes[&first],snapshot.iter().find(|graph|graph.graph_id==fixture.declaration).unwrap().nodes[&first]);
    assert_eq!(graph.nodes[&second],snapshot.iter().find(|graph|graph.graph_id==fixture.declaration).unwrap().nodes[&second]);
    assert_eq!(crate::unit_graph_source1003::parameters(graph).unwrap()[0].label,"quoted parameter");
    assert_eq!(graphs,snapshot);assert_eq!(fixture.bytes(),before);
}
#[test]
fn parameter_refs1003_parameter_slot_writes_shadow_global_and_rename_keeps_actual_engine_effect(){
    let fixture=Fixture::new("init {set alpha=99 set result=0}\nfunction sample(alpha) -> () {local alpha=alpha+1 set result=alpha}\nscene label start\ncall sample(3)\n\"After\"\nreturn");
    let before=fixture.bytes();let snapshot=fixture.graphs.clone();
    let effects=|graphs:&[GraphDocument]|{
        let compiled=crate::transpile_project(graphs).unwrap();
        let mut engine=rvn_core::Engine::from_program(rvn_parser::parse_program(&compiled.source).unwrap(),rvn_core::TerminalRenderer,16).unwrap();
        engine.state.random=rvn_core::random::RandomState::seeded(775);engine.enter_label("start").unwrap();engine.step_until_interaction().unwrap();
        (engine.state.vars["result"].clone(),engine.state.vars["alpha"].clone())
    };
    for keep_cache in [true,false]{
        let mut graphs=fixture.graphs.clone();let graph=graphs.iter_mut().find(|graph|graph.graph_id==fixture.declaration).unwrap();
        if !keep_cache{assert!(graph.variables.remove("alpha").is_some());}
        assert!(graph.nodes.values().any(|node|graph.variable_node_name(node.id).as_deref()==Some("alpha")));
        assert!(fixture.used(&graphs).unwrap());
        let expected=(rvn_parser::Value::Int(4),rvn_parser::Value::Int(99));assert_eq!(effects(&graphs),expected);
        let renamed=fixture.workspace.prepare_unit_parameter_rename(&graphs,fixture.declaration,&fixture.identity,"renamed alpha").unwrap();
        let graph=renamed.iter().find(|graph|graph.graph_id==fixture.declaration).unwrap();
        let pin=crate::unit_graph_source1003::parameters(graph).unwrap()[0];
        assert_eq!(pin.label,"renamed alpha");assert_eq!(pin.parameter_id.as_ref(),Some(&fixture.identity));
        assert!(!graph.nodes.values().any(|node|graph.variable_node_name(node.id).as_deref()==Some("alpha")));
        assert_eq!(graph.variables.contains_key("renamed alpha"),keep_cache);
        assert_eq!(effects(&renamed),expected);
    }
    // Missing cache plus a name without any real entry pin remains malformed.
    let mut malformed=fixture.graphs.clone();let graph=malformed.iter_mut().find(|graph|graph.graph_id==fixture.declaration).unwrap();
    graph.variables.remove("alpha");
    // Lowering may feed real parameter pins directly and represents the
    // authored local assignment as LocalVariable. Create the unknown read
    // explicitly instead of assuming a Get/Set shape for that valid body.
    let node=graph.add_catalog_node(NodeKind::VariableGet,[800.,600.]).unwrap();
    graph.set_property(node,"name",PropertyValue::String("not_a_parameter".into())).unwrap();
    assert_eq!(graph.variable_scope("not_a_parameter"),None);
    assert!(graph.validate().iter().any(|diagnostic|diagnostic.code=="unknown_variable"&&diagnostic.node==Some(node)));
    let mut missing_pin=fixture.graphs.clone();let graph=missing_pin.iter_mut().find(|graph|graph.graph_id==fixture.declaration).unwrap();
    let parameter=crate::unit_graph_source1003::parameters(graph).unwrap()[0].id;graph.pins.remove(&parameter);
    assert!(fixture.used(&missing_pin).is_err());
    assert!(fixture.workspace.prepare_unit_parameter_rename(&missing_pin,fixture.declaration,&fixture.identity,"renamed alpha").is_err());
    assert_eq!(fixture.graphs,snapshot);assert_eq!(fixture.bytes(),before);
}
#[test]
fn parameter_refs1003_typed_text_choice_reader_distinguishes_literal_interpolation_and_condition(){
    let quoted=rvn_parser::parse_interpolated_str("Hello [`not a name`]").unwrap();
    let literal=rvn_parser::parse_interpolated_str("not a name and [[not a name]").unwrap();
    assert!(text_uses(&quoted,"not a name"));assert!(!text_uses(&literal,"not a name"));
    let parsed=rvn_parser::parse("set value=`not a name`>0").unwrap();
    let Statement::SetVar{value:condition,..}=&parsed[0]else{panic!("actual condition AST")};
    let choice=Statement::Choice{options:vec![rvn_parser::ChoiceOption{label:literal,condition:Some(condition.clone()),body:vec![]}]};
    assert!(body_uses(&[choice],"not a name").unwrap());
    // Typed reading is not new Unit interaction authority. The actual current
    // source validator continues refusing Dialogue/Choice Unit bodies.
    let invalid=vec![Statement::UnitFunction{name:"sample".into(),parameters:vec!["not a name".into()],body:vec![Statement::Dialogue{character_id:None,text:quoted}]}];
    assert!(crate::import_script_with_parameter_allocator(&invalid,||Ok("11111111111111111111111111111111".into())).is_err());
}
#[test]
fn parameter_refs1003_external_source_and_foreign_identity_refuse_without_writes(){
    let fixture=Fixture::new("function sample(alpha) -> () {}\nscene label start\nreturn");
    let before=fixture.bytes();let foreign=ParameterId::parse("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap();
    assert!(fixture.workspace.unit_parameter_is_referenced(&fixture.graphs,fixture.declaration,&foreign).is_err());assert_eq!(fixture.bytes(),before);
    let path=fixture.root.join("main.rvn");let external=format!("{}\n// changed externally\n",std::fs::read_to_string(&path).unwrap());std::fs::write(&path,&external).unwrap();
    assert!(fixture.used(&fixture.graphs).is_err());assert_eq!(std::fs::read_to_string(&path).unwrap(),external);
    assert_eq!(std::fs::read(fixture.root.join(".rvn-authoring.json")).unwrap(),before.1);
}
