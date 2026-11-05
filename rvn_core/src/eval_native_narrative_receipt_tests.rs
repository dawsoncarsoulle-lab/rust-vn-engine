//! Evaluator-boundary tests only. Narrative ownership is exercised separately
//! by the Graph/file/Engine fixtures; equal values are never an owner proof.
use super::*;
const SCOPE:&str="g732/n1:label|name=1@0,exec_out=2@0";
const EACH:&str="n2:for_each|exec_in=3@1,collection=4@0,body=5@0,completed=6@0,array_index=7@0,array_element=8@0";
fn setup(failure:bool)->(FunctionLibrary,Statement,HashMap<String,Value>){
    let body=if failure{"local token=random(1,1) return 1/0"}else{"local token=random(1,1) return [11,22,33]"};let script=rvn_parser::parse(&format!("function supplier(){{{body}}}\n")).unwrap();let library=FunctionLibrary::from_script(&script).unwrap();let source=format!("flow.consume_v3(\"{SCOPE}\",\"{EACH}\"){{for rvn_native_index_g732_n2 in supplier(){{}}}}");
    let statement=rvn_parser::parse(&source).unwrap().remove(0);let binding=rvn_parser::scheduled_source::native_loop_contract::binding(&statement).unwrap().unwrap();let environment=HashMap::from([(binding.phase,Value::Int(1)),(binding.cursor,Value::Int(0))]);(library,statement,environment)
}
#[test]
fn narrative0732_actual_evaluation_receipt_matches_one_input_and_old_api(){
    let(library,statement,environment)=setup(false);let globals=HashMap::new();let mut random=RandomState::seeded(732);let mut frame=crate::consumer_schedule::ConsumerFrame::default();let mut old_random=random;let mut old_frame=frame.clone();let old=library.eval_engine_consumer(&statement,&environment,&globals,&mut old_random,&mut old_frame,"actual-bound-scope",None).unwrap();
    let(value,record)=library.eval_engine_consumer_record(&statement,&environment,&globals,&mut random,&mut frame,"actual-bound-scope",None).unwrap();assert_eq!(value,Value::List(vec![Value::Int(11),Value::Int(22),Value::Int(33)]));let mut expected=RandomState::seeded(732);expected.integer(1,1).unwrap();assert_eq!(random,expected);assert_eq!((value,random,frame),(old,old_random,old_frame));record.validate_loop_data().unwrap();assert_eq!(library.validate_retained_loop_inputs(&statement,Some("actual-bound-scope"),&[],&[record],None).unwrap(),Value::List(vec![Value::Int(11),Value::Int(22),Value::Int(33)]));
}
#[test]
fn narrative0732_actual_debugger_completion_issues_receipt_without_second_draw(){
    let(library,statement,environment)=setup(false);let probe=Probe::shared();let mut debug=library.clone();debug.debug_probe=Some(probe.clone());let globals=HashMap::new();let mut random=RandomState::seeded(732);let mut frame=crate::consumer_schedule::ConsumerFrame::default();assert!(debug.eval_engine_consumer_record(&statement,&environment,&globals,&mut random,&mut frame,"actual-bound-scope",None).is_err());assert_eq!(random,RandomState::seeded(732));assert_eq!(frame,Default::default());
    let mut continuation=Probe::pending(&probe,&library).unwrap().unwrap();for _ in 0..1000{if continuation.done(){break;}continuation.step().unwrap();}assert!(continuation.done());Probe::finish(&probe,&continuation).unwrap();Probe::rewind(&probe).unwrap();let(value,record)=debug.eval_engine_consumer_record(&statement,&environment,&globals,&mut random,&mut frame,"actual-bound-scope",None).unwrap();let mut expected=RandomState::seeded(732);expected.integer(1,1).unwrap();assert_eq!(random,expected);assert_eq!(Some(value),continuation.result);record.validate_loop_data().unwrap();assert!(Probe::complete(&probe));
}
#[test]
fn narrative0732_failed_actual_input_issues_no_receipt_or_candidate_publication(){
    let(library,statement,environment)=setup(true);let globals=HashMap::new();let mut random=RandomState::seeded(732);let mut frame=crate::consumer_schedule::ConsumerFrame::default();let before=frame.clone();assert!(library.eval_engine_consumer_record(&statement,&environment,&globals,&mut random,&mut frame,"actual-bound-scope",None).is_err());assert_eq!(random,RandomState::seeded(732));assert_eq!(frame,before);
}

#[test]
fn narrative0732_authentic_receipt_resource_accounting_is_aggregate_and_atomic(){
    let(library,statement,environment)=setup(false);let mut random=RandomState::seeded(732);let mut frame=Default::default();
    let(_,record)=library.eval_engine_consumer_record(&statement,&environment,&HashMap::new(),&mut random,&mut frame,"actual-bound-scope",None).unwrap();
    let(mut bytes,mut work)=(0usize,0usize);record.account_native_loop_storage(&mut bytes,&mut work).unwrap();assert!(bytes>0&&work>0);
    // These are private accounting counters, not saved owner/cursor fixtures.
    // The record itself comes from the actual evaluator and is not fabricated.
    let(mut near_bytes,mut near_work)=(TRACE_BYTES-bytes/2,0usize);let before=(near_bytes,near_work);assert!(record.account_native_loop_storage(&mut near_bytes,&mut near_work).is_err());assert_eq!((near_bytes,near_work),before);
    let(mut near_bytes,mut near_work)=(0usize,1_000_000usize);let before=(near_bytes,near_work);assert!(record.account_native_loop_storage(&mut near_bytes,&mut near_work).is_err());assert_eq!((near_bytes,near_work),before);
}
