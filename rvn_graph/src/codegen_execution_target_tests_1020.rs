//! Reuses real Graph allocation/wiring and the existing lexical break checks.
use super::*;
#[test]
fn execution_target1020_real_edges_keep_the_entered_pin_and_existing_loop_break_admission() {
    let mut graph=GraphDocument::new(GraphId::new(1020),GraphKind::Init);
    let root=graph.add_catalog_node(NodeKind::Init,[0.0,0.0]).unwrap();
    let range=graph.add_catalog_node(NodeKind::ForLoopWithBreak,[200.0,0.0]).unwrap();
    let output=graph.pin_by_key(root,"exec_out").unwrap().id;
    let enter=graph.pin_by_key(range,"exec_in").unwrap().id;
    let stop=graph.pin_by_key(range,"break").unwrap().id;
    let edge=graph.connect(output,enter).unwrap();
    {
        let emitter=Emitter::new(&graph);
        let target=emitter.successor(root,"exec_out").unwrap().unwrap();
        assert_eq!(target,ExecutionTarget::PinTarget(enter));
        assert_eq!(target.node(&graph).unwrap(),range);
        assert_eq!(target.input(&graph).unwrap().unwrap().key,"exec_in");
        assert!(ExecutionTarget::RootEntry(root).input(&graph).unwrap().is_none());
        assert!(ExecutionTarget::PinTarget(output).node(&graph).is_err(),"output cannot masquerade as entered input");
    }
    graph.remove_edge(edge).unwrap();graph.connect(output,stop).unwrap();
    let emitter=Emitter::new(&graph);
    assert!(matches!(emitter.successor(root,"exec_out"),Err(TranspileError::ExecutionCycle(id)) if id==range),
        "existing lexical break refusal is unchanged outside that loop body");
    // This is the exact existing lexical emitter stack, not a runtime owner.
    emitter.range_owners.borrow_mut().push(range);
    let target=emitter.successor(root,"exec_out").unwrap().unwrap();
    assert_eq!(target,ExecutionTarget::PinTarget(stop));
    assert_eq!(target.node(&graph).unwrap(),range);
    assert_eq!(target.input(&graph).unwrap().unwrap().key,"break");
    assert_ne!(target,ExecutionTarget::PinTarget(enter),"same node preserves distinct true input roles");
}
