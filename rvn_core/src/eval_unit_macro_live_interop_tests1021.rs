//! One additional continuation test for the future actual Unit+F+live merge.
//! The Macro site/data pin comes from real Graph allocation and lowering.
//! This private evaluator test grants no Engine/Widget/NativeFrame authority.
use super::*;

fn actual_flip_flop_data()->Expr{
    let source=rvn_parser::parse("handler unit_live_macro_probe(event){}").unwrap();
    let mut graph=rvn_graph::import_script(&source).unwrap().into_iter()
        .find(|graph|matches!(&graph.kind,rvn_graph::GraphKind::Handler{name}if name=="unit_live_macro_probe")).unwrap();
    let entry=graph.nodes.values().find(|node|node.kind==rvn_graph::NodeKind::HandlerEntry).unwrap().id;
    let node=graph.add_catalog_node(rvn_graph::NodeKind::FlipFlop,[300.,0.]).unwrap();
    graph.connect(graph.pin_by_key(entry,"exec_out").unwrap().id,graph.pin_by_key(node,"exec_in").unwrap().id).unwrap();
    graph.enable_native_frame_preparation().unwrap();
    let generated=rvn_graph::prepare_native_frame_graph(&graph).unwrap();
    let source=rvn_parser::parse(&generated.source).unwrap();
    let body=source.iter().find_map(|statement|if let Statement::Handler{name,body,..}=statement{(name=="unit_live_macro_probe").then_some(body)}else{None}).unwrap();
    let [Statement::StandardMacroDispatch{dispatch}]=body.as_slice()else{panic!("actual lowered Macro dispatch")};
    dispatch.validate().unwrap();
    use rvn_parser::scheduled_source::standard_macro_contract::DataOutput;
    let pin=dispatch.site.data_pin(DataOutput::IsA).unwrap();
    Expr::StandardMacroData{site:Box::new(dispatch.site.clone()),output:DataOutput::IsA,pin}
}

#[test]
fn unit_macro_live1021_real_macro_data_argument_after_rng_refuses_without_owner_or_unit_publication(){
    let source=rvn_parser::parse("function effect(noise,value)->(){set result=value}").unwrap();
    let library=FunctionLibrary::from_script(&source).unwrap();
    let globals=HashMap::from([("result".into(),Value::Int(9))]);let original=globals.clone();
    let arguments=[Expr::Call{name:"random".into(),args:vec![Expr::Int(1),Expr::Int(100)]},actual_flip_flop_data()];
    let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());
    let seed=RandomState::seeded(775);let mut random=seed;
    assert!(debug.invoke_unit_scoped_with_locale("effect",&arguments,&globals,&globals,&mut random,None).is_err());
    let mut continuation=Probe::pending(&probe,&library).unwrap().unwrap();
    assert!(continuation.state.widget_macro_frame.is_none());
    assert!(Probe::portable_trace(&probe,&continuation).is_err(),"Macro None is not a portable Unit owner");
    for _ in 0..128{if continuation.done(){break;}if continuation.step().is_err(){break;}}
    assert!(continuation.done());
    let error=continuation.error.as_ref().expect("actual Macro data read without Widget owner must fail");
    assert!(error.to_string().contains("Macro data has no actual Widget owner"),"{error}");
    let mut expected=seed;expected.integer(1,100).unwrap();
    assert_eq!(continuation.state.random,expected,"ordered earlier argument ran only in the candidate");
    assert!(continuation.state.widget_macro_frame.is_none());assert!(continuation.state.unit_writes.is_empty());
    assert_eq!(continuation.state.globals,original);assert_eq!(continuation.frames.len(),1);
    assert_eq!(continuation.unit_exit,None);assert_eq!(continuation.result,None);
    assert_eq!(random,seed);assert_eq!(globals,original);
    Probe::finish(&probe,&continuation).unwrap();Probe::rewind(&probe).unwrap();
    assert!(debug.invoke_unit_scoped_with_locale("effect",&arguments,&globals,&globals,&mut random,None).is_err());
    assert_eq!(random,seed);assert_eq!(globals,original);assert!(Probe::complete(&probe));
}
