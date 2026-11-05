//! Real public synchronous FunctionLibrary regressions. Debug/portable owners are separate work.
use super::*;
const SCOPE:&str="g764/n1:function_entry|exec_out=1@0";
const RANGE:&str="n2:for_loop_with_break|exec_in=2@1,first=3@0,last=4@0,break=5@0,body=6@0,index=7@0,completed=8@0";
const INDEX:&str="rvn_native_index_g764_n2";
fn source(defs:&str,first:&str,last:&str,body:&str)->String{
    format!("{defs}\nfunction probe(){{local trace=\"\" local seen=-1 local visits=0 flow.consume_v3(\"{SCOPE}\",\"{RANGE}\"){{flow.for_loop({INDEX},{first},{last}){{{body}}}}} set trace=trace+\"C\" return [trace,seen,visits]}}")
}
fn library(source:&str)->FunctionLibrary{FunctionLibrary::from_script(&rvn_parser::parse(source).unwrap()).unwrap()}
fn call(source:&str)->(Value,RandomState){
    let lib=library(source);let mut random=RandomState::seeded(764);
    let values=lib.execute_in_consumer_frame(&rvn_parser::parse("set answer=probe()").unwrap(),&HashMap::new(),&mut random,&mut crate::consumer_schedule::ConsumerFrame::default()).unwrap();
    assert_eq!(values.len(),1,"function temporaries and native index cannot escape their real caller");
    (values["answer"].clone(),random)
}
fn draws(count:usize)->RandomState{let mut r=RandomState::seeded(764);for _ in 0..count{r.integer(1,1).unwrap();}r}
const LAST:&str="function last(){local token=random(1,1) return 2}";

#[test]
fn native0764_public_function_break_preserves_pending_sequence_and_no_extra_comparison(){
    for first in ["0","9223372036854775807"]{
        let last=if first=="0"{"last()"}else{first};
        let body=format!("flow.sequence(){{{{flow.break()}}{{set trace=trace+\"T\" set seen={INDEX}}}}}");
        let (value,rng)=call(&source(LAST,first,last,&body));
        assert_eq!(value,Value::List(vec![Value::Str("TC".into()),Value::Int(first.parse().unwrap()),Value::Int(0)]));
        assert_eq!(rng,draws(usize::from(first=="0")));
    }
}

#[test]
fn native0764_public_function_body_break_tail_completed_has_measured_order(){
    let body="flow.sequence(){{set trace=trace+\"B\" set visits=visits+1 flow.break()}{set trace=trace+\"T\"}}";
    let (value,rng)=call(&source(LAST,"0","last()",body));
    assert_eq!(value,Value::List(vec![Value::Str("BTC".into()),Value::Int(-1),Value::Int(1)]));
    assert_eq!(rng,draws(1));
}

#[test]
fn native0764_public_function_inner_break_keeps_outer_tail_and_continuation(){
    let inner="n3:for_loop_with_break|exec_in=12@2,first=13@0,last=14@0,break=15@0,body=16@0,index=17@0,completed=18@0";
    let defs="function last(){local token=random(1,1) return 1} function inner_last(){local token=random(1,1) return 2}";
    let body=format!("flow.sequence(){{{{set trace=trace+\"O\" set visits=visits+1 flow.consume_v3(\"{SCOPE}\",\"{inner}\"){{flow.for_loop(rvn_native_index_g764_n3,0,inner_last()){{flow.sequence(){{{{set trace=trace+\"I\" flow.break()}}{{set trace=trace+\"T\" set seen=rvn_native_index_g764_n3}}}}}}}} set trace=trace+\"CA\"}}{{set trace=trace+\"X\"}}}}");
    let (value,rng)=call(&source(defs,"0","last()",&body));
    assert_eq!(value,Value::List(vec![Value::Str("OITCAXOITCAXC".into()),Value::Int(0),Value::Int(2)]));
    assert_eq!(rng,draws(5));
}

#[test]
fn native0764_public_function_interposed_legacy_range_masks_outer_native_owner(){
    for scheduled in [false,true]{
        let old="flow.for_loop(legacy_index,0,3){flow.sequence(){{flow.break()}{set trace=trace+\"forbidden\"}}}";
        let old=if scheduled{format!("flow.consume_v2(\"{SCOPE}\",\"n9:for_loop_with_break|exec_in=30@2,first=31@0,last=32@0,break=33@0,body=34@0,index=35@0,completed=36@0\"){{{old}}}")}else{old.into()};
        let body=format!("{old} set trace=trace+\"L\" set visits=visits+1");
        let (value,rng)=call(&source("function last(){local token=random(1,1) return 1}","0","last()",&body));
        assert_eq!(value,Value::List(vec![Value::Str("LLC".into()),Value::Int(-1),Value::Int(2)]));assert_eq!(rng,draws(3));
    }
}

#[test]
fn native0764_public_function_return_keeps_existing_early_return_contract(){
    let body="flow.sequence(){{return [\"return\",0,0]}{set trace=trace+\"forbidden\"}}";
    let (value,rng)=call(&source(LAST,"0","last()",body));
    assert_eq!(value,Value::List(vec![Value::Str("return".into()),Value::Int(0),Value::Int(0)]));assert_eq!(rng,draws(1));
}

#[test]
fn native0764_public_function_nested_call_restores_its_callers_already_latched_owner(){
    let helper=source("","0","last()","flow.sequence(){{set trace=trace+\"I\" flow.break()}{set trace=trace+\"H\"}}")
        .replace("function probe()","function helper()")
        .replace("g764/","g765/").replace("g764_n","g765_n")
        .replace("return [trace,seen,visits]","return trace");
    let body="flow.sequence(){{flow.break()}{set trace=trace+helper() set trace=trace+\"T\"}}";
    let (value,rng)=call(&source(&format!("{LAST} {helper}"),"0","last()",body));
    assert_eq!(value,Value::List(vec![Value::Str("IHCTC".into()),Value::Int(-1),Value::Int(0)]));assert_eq!(rng,draws(2));
}

#[test]
fn native0764_public_function_failed_tail_publishes_no_rng_or_consumer_frame(){
    let lib=library(&source(LAST,"0","last()","flow.sequence(){{flow.break()}{set seen=1/0}}"));
    let mut random=RandomState::seeded(764);let before=random;
    let mut frame=crate::consumer_schedule::ConsumerFrame::default();let before_frame=serde_json::to_value(&frame).unwrap();
    assert!(matches!(lib.execute_in_consumer_frame(&rvn_parser::parse("set answer=probe()").unwrap(),&HashMap::new(),&mut random,&mut frame),Err(EvalError::DivisionByZero)));
    assert_eq!(random,before);assert_eq!(serde_json::to_value(&frame).unwrap(),before_frame);
}
