//! Regression fixtures from11 measured UE cases, adapted explicitly to source3.
//! Prepared only:0 executions. The immutable0689 packet remains unchanged.
//! Only Native Each/Range is explicitly migrated; While retains version2 and
//! Legacy retains its historical source, outputs, snapshots and oracles.
use rvn_core::{Engine, TerminalRenderer};
use rvn_core::engine::EngineStartup;
use rvn_core::editor_debugger::StepKind;
use rvn_graph::*;
use rvn_parser::Value;
use std::{collections::HashMap, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

fn owner(name:&str, authored:&str, native:bool, kind:NodeKind)->EngineStartup<TerminalRenderer>{
    let mut graphs=import_script(&rvn_parser::parse(authored).unwrap()).unwrap();
    assert!(graphs.iter().any(|g|matches!(g.kind,GraphKind::Init)
        &&g.nodes.values().any(|n|n.kind==kind)),"must use the actual imported control node");
    for graph in &mut graphs{
        *graph=GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
        if native{graph.enable_native_frame_preparation().unwrap();}
    }
    if native && matches!(kind,NodeKind::ForEach|NodeKind::ForLoopWithBreak){
        let graph=graphs.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();
        let loop_node=graph.nodes.values().find(|n|n.kind==kind).unwrap().id;
        let old_name=graph.variable_node_name(loop_node).unwrap();
        *graph=graph.prepare_native_loop_demand(loop_node).unwrap().graph;
        let output=graph.pin_by_key(loop_node,if kind==NodeKind::ForEach{"array_element"}else{"index"}).unwrap().id;
        // The fixture itself declares these selected body reads as loop outputs.
        // This is an explicit test graph edit, never a production name heuristic.
        let reads:Vec<_>=graph.nodes.values().filter(|n|n.kind==NodeKind::VariableGet&&graph.variable_node_name(n.id).as_deref()==Some(old_name.as_str())).map(|n|n.id).collect();
        for read in reads{
            let output_pins=graph.nodes[&read].pins.clone();
            let wires:Vec<_>=graph.edges.values().filter(|e|output_pins.contains(&e.output)).map(|e|(e.id,e.input)).collect();
            for(edge,input)in wires{graph.edges.remove(&edge);graph.connect(output,input).unwrap();}
            graph.remove_node(read).unwrap();
        }
    }
    let compiled=transpile_project(&graphs).expect("actual project typing/source lowering must succeed first");
    let demand=native&&matches!(kind,NodeKind::ForEach|NodeKind::ForLoopWithBreak);
    assert_eq!(compiled.source.contains("flow.consume_v3("),demand,"the selected loop requires its explicit versioned owner");
    if native&&!demand{assert!(compiled.source.contains("flow.consume_v2("),"While retains its already-owned native contract");}
    if !native{assert!(!compiled.source.contains("flow.consume_v2("),"historical Legacy fixture remains unchanged");}
    assert_eq!(rvn_parser::parse(&compiled.source).unwrap(),compiled.ast);
    static NEXT:AtomicU64=AtomicU64::new(0);
    let root=PathBuf::from(std::env::var("RVN_LOOP0689_FIXTURES").expect("isolated D fixture root required"))
        .join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    std::fs::create_dir_all(&root).unwrap();
    for (i,graph) in graphs.iter().enumerate(){std::fs::write(root.join(format!("{i}.graph.json")),graph.to_pretty_json().unwrap()).unwrap();}
    let file=root.join("actual.native.rvn");std::fs::write(&file,&compiled.source).unwrap();
    let program=rvn_parser::parse_file_program_with_uses(&file).unwrap();
    let owner=Engine::prepare_debugger_startup(program,TerminalRenderer,16,HashMap::new(),Default::default(),None)
        .expect("a real source-bound Startup owner is required");
    assert!(!owner.is_complete());
    assert!(!owner.state().vars.contains_key("completed"),"admission must not run Init");
    owner
}

fn run(name:&str,definitions:&str,body:&str,native:bool,kind:NodeKind,bodies:i64,sum:Option<i64>,draws:Option<usize>){
    let source=format!("{definitions}\ninit {{set bodies=0 set sum=0 set completed=0 {body} set completed=completed+1}}\n");
    let mut owner=owner(name,&source,native,kind);
    let mut random=owner.state().random;
    if let Some(draws)=draws{for _ in 0..draws{random.integer(1,1).unwrap();}}
    for _ in 0..256{if owner.is_complete(){break;}owner.step(StepKind::Continue).unwrap();}
    assert!(owner.is_complete(),"bounded fixture must complete without a fabricated owner");
    assert_eq!(owner.state().vars["bodies"],Value::Int(bodies),"UE case {name}: body count");
    assert_eq!(owner.state().vars["completed"],Value::Int(1));
    if let Some(sum)=sum{assert_eq!(owner.state().vars["sum"],Value::Int(sum));}
    // The Rust supplier performs exactly one real RNG draw per invocation.
    // This checks invocation demand, not a forged global ProducerCalls value.
    if draws.is_some(){assert_eq!(owner.state().random,random,"UE case {name}: real supplier invocation count");}
}

const VALUES:&str="function source() {local token=random(1,1) return [11,22,33]}";
const LAST:&str="function last() {local token=random(1,1) return 2}";
const CONDITION:&str="function condition(seen,limit) {local token=random(1,1) return seen<limit}";

#[test]
fn ue0689_foreach_index_only_supplier_is_demanded_for_each_length_check(){
    run("FE_IndexOnly0653V3",VALUES,"for item in source() {set bodies=bodies+1}",true,NodeKind::ForEach,3,None,Some(4));
}
#[test]
fn ue0689_foreach_element_supplier_has_separate_consumer_demand(){
    run("FE_ConsumeElement0653V3",VALUES,"for item in source() {set bodies=bodies+1 set sum=sum+item}",true,NodeKind::ForEach,3,Some(66),Some(7));
}
#[test]
fn ue0689_foreach_shrink_changes_the_next_length_check(){
    run("FE_ShrinkArray0653V3","","set items=[11,22,33] for item in items {set bodies=bodies+1 set sum=sum+item set items=[11]}",true,NodeKind::ForEach,1,Some(11),None);
}
#[test]
fn ue0689_foreach_grow_exposes_new_zero_elements_without_snapshotting(){
    run("FE_GrowBounded0653V3","","set items=[11,22,33] for item in items {set bodies=bodies+1 set sum=sum+item set items=[11,22,33,0,0]}",true,NodeKind::ForEach,5,Some(66),None);
}
#[test]
fn ue0689_range_last_supplier_is_demanded_including_terminal_check(){
    run("FL_FixedLast0653V3",LAST,"flow.for_loop(index,0,last()) {set bodies=bodies+1 set sum=sum+index}",true,NodeKind::ForLoopWithBreak,3,Some(3),Some(4));
}
#[test]
fn ue0689_range_shrink_changes_the_next_comparison(){
    run("FL_ShrinkLast0653V3","","set last=2 flow.for_loop(index,0,last) {set bodies=bodies+1 set sum=sum+index set last=0}",true,NodeKind::ForLoopWithBreak,1,Some(0),None);
}
#[test]
fn ue0689_range_grow_changes_the_next_comparison(){
    run("FL_GrowLast0653V3","","set last=2 flow.for_loop(index,0,last) {set bodies=bodies+1 set sum=sum+index set last=4}",true,NodeKind::ForLoopWithBreak,5,Some(10),None);
}
#[test]
fn ue0689_range_break_has_no_extra_terminal_supplier_demand(){
    run("FL_BreakFirst0653V3",LAST,"flow.for_loop(index,0,last()) {set bodies=bodies+1 set sum=sum+index flow.break()}",true,NodeKind::ForLoopWithBreak,1,Some(0),Some(1));
}
#[test]
fn ue0689_while_fixed_supplier_includes_false_terminal_condition(){
    run("WH_FixedLimit0653V3",CONDITION,"set limit=3 while condition(bodies,limit) {set bodies=bodies+1}",true,NodeKind::While,3,None,Some(4));
}
#[test]
fn ue0689_while_shrink_uses_current_inputs(){
    run("WH_ShrinkLimit0653V3",CONDITION,"set limit=3 while condition(bodies,limit) {set bodies=bodies+1 set limit=1}",true,NodeKind::While,1,None,Some(2));
}
#[test]
fn ue0689_while_grow_uses_current_inputs(){
    run("WH_GrowLimit0653V3",CONDITION,"set limit=3 while condition(bodies,limit) {set bodies=bodies+1 set limit=4}",true,NodeKind::While,4,None,Some(5));
}

#[test]
fn legacy0689_foreach_preserves_one_source_invocation(){
    run("legacy-one-source",VALUES,"for item in source() {set bodies=bodies+1 set sum=sum+item}",false,NodeKind::ForEach,3,Some(66),Some(1));
}
#[test]
fn legacy0689_foreach_preserves_snapshot_during_source_reassignment(){
    run("legacy-each-snapshot","","set items=[11,22,33] for item in items {set bodies=bodies+1 set sum=sum+item set items=[11]}",false,NodeKind::ForEach,3,Some(66),None);
}
#[test]
fn legacy0689_range_preserves_admitted_bounds_during_source_reassignment(){
    run("legacy-range-snapshot","","set last=2 flow.for_loop(index,0,last) {set bodies=bodies+1 set sum=sum+index set last=0}",false,NodeKind::ForLoopWithBreak,3,Some(3),None);
}
