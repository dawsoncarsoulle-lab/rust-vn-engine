//! Proposed tests for the future actual Unit+F composition, never source817
//! acceptance. Macro declarations come from real Graph allocation/lowering.
use super::*;
use rvn_parser::scheduled_source::standard_macro_contract::{DataOutput, Dispatch};

fn actual_macro_dispatch(kind: rvn_graph::NodeKind) -> Box<Dispatch> {
    let script = rvn_parser::parse("handler unit_macro_probe(event){}").unwrap();
    let mut graph = rvn_graph::import_script(&script).unwrap().into_iter()
        .find(|graph| matches!(&graph.kind,rvn_graph::GraphKind::Handler{name} if name=="unit_macro_probe")).unwrap();
    let root = graph.nodes.values().find(|node|node.kind==rvn_graph::NodeKind::HandlerEntry).unwrap().id;
    let node = graph.add_catalog_node(kind,[300.0,0.0]).unwrap();
    let output = graph.pin_by_key(root,"exec_out").unwrap().id;
    let input = graph.pin_by_key(node,"exec_in").unwrap().id;
    graph.connect(output,input).unwrap();
    graph.enable_native_frame_preparation().unwrap();
    let generated = rvn_graph::prepare_native_frame_graph(&graph).unwrap();
    let script = rvn_parser::parse(&generated.source).unwrap();
    let body = script.iter().find_map(|statement|match statement {
        Statement::Handler{name,body,..} if name=="unit_macro_probe" => Some(body),_=>None,
    }).unwrap();
    let [Statement::StandardMacroDispatch{dispatch}] = body.as_slice() else {
        panic!("actual defaults-only Handler lowering must contain one macro dispatch");
    };
    dispatch.validate().unwrap();
    dispatch.clone()
}

#[test]
fn unit_macro1021_hidden_branches_preserve_engine_handler_and_portable_unit_refusal() {
    let declaration = rvn_parser::parse("function effect(n)->(){set score=n}").unwrap();
    let library = FunctionLibrary::from_script(&declaration).unwrap();
    let empty_library = FunctionLibrary::from_script(&[]).unwrap();
    for hidden in [
        Statement::UnitCall{name:"effect".into(),arguments:vec![Expr::Int(7)]},
        Statement::UnitReturn,
        Statement::UnitFunction{name:"nested".into(),parameters:vec![],body:vec![]},
    ] {
        let mut dispatch = actual_macro_dispatch(rvn_graph::NodeKind::DoOnce);
        // Deliberately forge only the branch body of an otherwise real lowered
        // declaration. These are negative admission tests, not Widget grants.
        dispatch.branches[0].body = vec![hidden];
        dispatch.validate().unwrap();
        let wrapped = Statement::StandardMacroDispatch{dispatch};
        assert!(contains_unit_runtime(std::slice::from_ref(&wrapped)));
        assert!(FunctionLibrary::reject_unadmitted_unit_owner(std::slice::from_ref(&wrapped)).is_err());
        assert!(empty_library.reject_unit_portable_owner(std::slice::from_ref(&wrapped)).is_err());
        assert!(super::super::validate_handler(std::slice::from_ref(&wrapped)).is_err());
        let mut candidate = declaration.clone();candidate.push(wrapped);
        assert!(library.validate_engine_unit_owner(&candidate).is_err());
    }
}

#[test]
fn unit_macro1021_each_actual_macro_remains_unadmitted_in_a_unit_definition() {
    for kind in [rvn_graph::NodeKind::DoOnce,rvn_graph::NodeKind::DoN,
        rvn_graph::NodeKind::FlipFlop,rvn_graph::NodeKind::Gate] {
        let dispatch = actual_macro_dispatch(kind);
        let script = vec![Statement::UnitFunction{name:"effect".into(),parameters:vec![],
            body:vec![Statement::StandardMacroDispatch{dispatch}]}];
        assert!(FunctionLibrary::from_script(&script).is_err());
    }
}

#[test]
fn unit_macro1021_live_macro_argument_failure_preserves_unit_rng_and_globals() {
    let dispatch = actual_macro_dispatch(rvn_graph::NodeKind::FlipFlop);
    let pin = dispatch.site.data_pin(DataOutput::IsA).unwrap();
    let live = Expr::StandardMacroData{site:Box::new(dispatch.site.clone()),output:DataOutput::IsA,pin};
    let script = rvn_parser::parse("function effect(noise,value)->(){set score=value}").unwrap();
    let library = FunctionLibrary::from_script(&script).unwrap();
    let globals = HashMap::from([("score".into(),Value::Int(9))]);let before=globals.clone();
    let seed = RandomState::seeded(775);let mut random=seed;
    let noise = Expr::Call{name:"random".into(),args:vec![Expr::Int(1),Expr::Int(100)]};
    assert!(library.invoke_unit_with_random("effect",&[noise.clone(),live.clone()],&globals,&mut random).is_err());
    assert_eq!(random,seed);assert_eq!(globals,before);
    // Mode/arity admission still precedes evaluating either candidate argument.
    assert!(library.invoke_unit_with_random("effect",&[live.clone()],&globals,&mut random).is_err());
    assert!(library.invoke_unit_with_random("missing",&[noise,live],&globals,&mut random).is_err());
    assert_eq!(random,seed);assert_eq!(globals,before);
    let accepted=library.invoke_unit_with_random("effect",&[Expr::Int(0),Expr::Int(7)],&globals,&mut random).unwrap();
    assert_eq!(accepted["score"],Value::Int(7));assert_eq!(random,seed);assert_eq!(globals,before);
}
