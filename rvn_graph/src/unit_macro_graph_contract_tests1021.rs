//! Proposed cross-contract test for the future composed Graph, not817 credit.
use super::*;

#[test]
fn unit_macro1021_schema14_parameters_and_macro_contract_both_refuse_without_downgrade() {
    let script=rvn_parser::parse("init {set score=0}\nfunction sample(alpha)->(){set score=alpha}\nscene label start\nreturn").unwrap();
    let mut graphs=crate::import_script_with_parameter_allocator(&script,
        ||Ok("11111111111111111111111111111111".into())).unwrap();
    let graph=graphs.iter_mut().find(|graph|matches!(&graph.kind,GraphKind::UnitFunction{name}if name=="sample")).unwrap();
    let pin=crate::unit_graph_source1003::parameters(graph).unwrap()[0];
    let id=pin.id;let identity=pin.parameter_id.clone();
    assert_eq!(graph.schema_version,crate::GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION);
    graph.validate_narrative_metadata().unwrap();
    let node=graph.add_catalog_node(NodeKind::DoOnce,[300.0,0.0]).unwrap();
    assert!(graph.nodes[&node].standard_macro_contract.is_some());
    let snapshot=graph.clone();
    assert!(graph.validate_narrative_metadata().is_err());
    assert!(graph.enable_native_frame_preparation().is_err());
    assert_eq!(*graph,snapshot);
    assert_eq!(graph.schema_version,crate::GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION);
    assert_eq!(graph.pins[&id].parameter_id,identity);
    // Parameter metadata is still validated even when a second feature marker
    // is present. No macro contract or schema conversion may repair this pin.
    graph.pins.get_mut(&id).unwrap().parameter_id=None;
    let expected=crate::entry_parameter_identity1003::validate_graph_parameter_metadata(graph).unwrap_err();
    assert_eq!(graph.validate_narrative_metadata().unwrap_err(),expected);
    let invalid=graphs.clone();assert!(crate::transpile_project(&graphs).is_err());assert_eq!(graphs,invalid);
}
