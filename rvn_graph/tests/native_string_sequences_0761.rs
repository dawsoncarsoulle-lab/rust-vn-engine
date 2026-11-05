//! Actual Graph -> source -> parse/import contracts, not native/pixel proofs.
use rvn_graph::*;
use rvn_parser::{ConsumerStep,Expr,Statement,Value};
use std::collections::HashMap;
const MARK:&str="l:string_list_v1/";
const HELPERS:&str="function collect0(){return []}\nfunction collect2(a,b){return [a,b]}\n";
fn wire(g:&mut GraphDocument,a:NodeId,ak:&str,b:NodeId,bk:&str){g.connect(g.pin_by_key(a,ak).unwrap().id,g.pin_by_key(b,bk).unwrap().id).unwrap();}
fn graph(kind:NodeKind,values:&[&str],typed:bool)->(GraphDocument,NodeId){
    let mut g=GraphDocument::new(GraphId::new(761),GraphKind::Function{name:"probe".into()});
    let root=g.add_catalog_node(NodeKind::FunctionEntry,[0.,0.]).unwrap();let n=g.add_catalog_node(kind,[100.,0.]).unwrap();let ret=g.add_catalog_node(NodeKind::FunctionReturn,[200.,0.]).unwrap();
    if kind==NodeKind::FunctionCall{g.set_property(n,"function",PropertyValue::String(if values.is_empty(){"collect0"}else{"collect2"}.into())).unwrap();}
    let property=if kind==NodeKind::ListLiteral{"items"}else{"args"};
    g.set_property(n,property,PropertyValue::StringList(values.iter().map(|v|(*v).into()).collect())).unwrap();
    if typed{g.resize_value_inputs(n,values.len()).unwrap();}
    wire(&mut g,root,"exec_out",ret,"exec_in");wire(&mut g,n,if kind==NodeKind::ListLiteral{"list"}else{"result"},ret,"value");g.enable_native_frame_preparation().unwrap();(g,n)
}
fn roundtrip(g:&GraphDocument)->(TranspiledScriptWithOrigins,GraphDocument){
    let compiled=prepare_native_frame_graph(g).unwrap();assert_eq!(rvn_parser::parse(&compiled.source).unwrap(),compiled.ast);
    let full=format!("{HELPERS}{}",compiled.source);let program=rvn_parser::parse_program(&full).unwrap();let sites=rvn_parser::scheduled_source::scheduled_step_sources(&program).unwrap();assert!(!sites.is_empty());assert!(sites.iter().all(|site|site.source_ordinal==0));
    let imported=import_native_frame_script(&compiled.ast).unwrap();let restored=imported.into_iter().find(|candidate|candidate.graph_id==g.graph_id).unwrap();
    assert_eq!(restored.nodes.keys().collect::<Vec<_>>(),g.nodes.keys().collect::<Vec<_>>());assert_eq!(restored.pins.keys().collect::<Vec<_>>(),g.pins.keys().collect::<Vec<_>>());assert_eq!(restored.edges,g.edges);
    assert_eq!(prepare_native_frame_graph(&restored).unwrap().ast,compiled.ast);(compiled,restored)
}
fn value(compiled:&TranspiledScriptWithOrigins)->Value{
    let mut script=rvn_parser::parse(HELPERS).unwrap();script.extend(compiled.ast.clone());let library=rvn_core::eval::FunctionLibrary::from_script(&script).unwrap();
    let Statement::SetVar{value,..}=rvn_parser::parse("set result=probe()").unwrap().remove(0)else{unreachable!()};library.eval(&value,&HashMap::new()).unwrap()
}
fn property_case(kind:NodeKind,values:&[&str]){
    let(g,n)=graph(kind,values,false);let(compiled,cold)=roundtrip(&g);let property=if kind==NodeKind::ListLiteral{"items"}else{"args"};
    assert!(compiled.source.contains(MARK));assert!(!cold.nodes[&n].properties.contains_key("input_count"));assert_eq!(cold.nodes[&n].pins,g.nodes[&n].pins);assert_eq!(cold.nodes[&n].properties.get(property),g.nodes[&n].properties.get(property));
    if kind==NodeKind::FunctionCall{assert_eq!(cold.nodes[&n].properties.get("function"),g.nodes[&n].properties.get("function"));}
    assert_eq!(value(&compiled),Value::List(values.iter().map(|s|Value::Str((*s).into())).collect()));
}
#[test]fn sequence0761_property_list_empty_preserves_explicit_storage(){property_case(NodeKind::ListLiteral,&[]);}
#[test]fn sequence0761_property_list_strings_preserve_exact_values(){property_case(NodeKind::ListLiteral,&["11","été 🌿"]);}
#[test]fn sequence0761_property_call_empty_preserves_explicit_storage(){property_case(NodeKind::FunctionCall,&[]);}
#[test]fn sequence0761_property_call_strings_preserve_exact_arguments(){property_case(NodeKind::FunctionCall,&["11","été 🌿"]);}
#[test]
fn sequence0761_typed_zero_never_collapses_into_property_mode(){
    for kind in [NodeKind::ListLiteral,NodeKind::FunctionCall]{let(g,n)=graph(kind,&[],true);let(compiled,cold)=roundtrip(&g);assert!(!compiled.source.contains(MARK));assert_eq!(cold.nodes[&n].properties.get("input_count"),Some(&PropertyValue::Int(0)));assert_eq!(cold.nodes[&n].pins,g.nodes[&n].pins);assert_eq!(value(&compiled),Value::List(vec![]));}
}
#[test]
fn sequence0761_typed_defaults_and_connected_inputs_keep_their_exact_ports(){
    for kind in [NodeKind::ListLiteral,NodeKind::FunctionCall]{
        let(mut g,n)=graph(kind,&["old unused","old unused"],true);g.set_pin_default(n,"item_0",PropertyValue::Int(7)).unwrap();
        let literal=g.add_catalog_node(NodeKind::Literal,[100.,100.]).unwrap();g.set_property(literal,"value",PropertyValue::String("connected".into())).unwrap();wire(&mut g,literal,"value",n,"item_1");
        let(compiled,cold)=roundtrip(&g);assert!(!compiled.source.contains(MARK));assert_eq!(cold.nodes[&n].properties.get("input_count"),Some(&PropertyValue::Int(2)));assert_eq!(cold.nodes[&n].pins,g.nodes[&n].pins);assert_eq!(cold.pin_by_key(n,"item_0").unwrap().default_value,Some(PropertyValue::Int(7)));assert_eq!(cold.edges,g.edges);assert_eq!(value(&compiled),Value::List(vec![Value::Int(7),Value::Str("connected".into())]));
    }
}
fn rewrite(script:&rvn_parser::Script,old:&str,new:&str)->rvn_parser::Script{
    fn visit(value:&mut serde_json::Value,old:&str,new:&str){match value{serde_json::Value::String(s)if s==old=>*s=new.into(),serde_json::Value::Array(a)=>for v in a{visit(v,old,new)},serde_json::Value::Object(o)=>for v in o.values_mut(){visit(v,old,new)},_=>{}}}
    let mut data=serde_json::to_value(script).unwrap();visit(&mut data,old,new);serde_json::from_value(data).unwrap()
}
fn producer(script:&rvn_parser::Script)->String{let Statement::Function{body,..}=&script[0]else{unreachable!()};let Statement::ScheduledConsumer{steps,..}=&body[0]else{unreachable!()};let ConsumerStep::Capture{producer,..}=&steps[0]else{unreachable!()};producer.clone()}
#[test]
fn sequence0761_old_unmarked_nonempty_properties_keep_their_actual_old_source(){
    for kind in [NodeKind::ListLiteral,NodeKind::FunctionCall]{
        let(g,n)=graph(kind,&["11","été"],false);let(compiled,_)=roundtrip(&g);let id=producer(&compiled.ast);let old_id=id.strip_prefix(MARK).unwrap();let old=rewrite(&compiled.ast,&id,old_id);
        let graphs=import_native_frame_script(&old).unwrap();let restored=graphs.iter().find(|g|g.graph_id==GraphId::new(761)).unwrap();assert!(!restored.nodes[&n].properties.contains_key("input_count"));assert_eq!(prepare_native_frame_graph(restored).unwrap().ast,old);
    }
    // The old empty bytes cannot reveal their author's former storage mode.
    // They preserve the existing typed-zero interpretation, without claiming
    // that property-only empty provenance has been recovered from equal values.
    let(g,n)=graph(NodeKind::ListLiteral,&[],true);let(compiled,_)=roundtrip(&g);let restored=import_native_frame_script(&compiled.ast).unwrap();assert_eq!(restored.iter().find(|g|g.graph_id==GraphId::new(761)).unwrap().nodes[&n].properties.get("input_count"),Some(&PropertyValue::Int(0)));
}
#[test]
fn sequence0761_forged_mode_ports_kind_and_nonstring_values_are_refused(){
    for kind in [NodeKind::ListLiteral,NodeKind::FunctionCall]{
        let(g,_)=graph(kind,&["a","b"],false);let(compiled,_)=roundtrip(&g);let id=producer(&compiled.ast);let plain=id.strip_prefix(MARK).unwrap();let output=if kind==NodeKind::ListLiteral{"list"}else{"result"};
        let forged=[id.replace("string_list_v1","string_list_v9"),format!("{id},item_0=90@0"),format!("{id},item_0=90@0,item_0=91@0"),id.replace(&format!("|{output}="),"|wrong_output="),id.replace(if kind==NodeKind::ListLiteral{":list_literal|"}else{":function_call|"},":literal|")];
        for bad in forged{let source=rewrite(&compiled.ast,&id,&bad);assert!(import_native_frame_script(&source).is_err(),"accepted forged identity {bad}");}
        let mut bad=compiled.ast.clone();let Statement::Function{body,..}=&mut bad[0]else{unreachable!()};let Statement::ScheduledConsumer{steps,..}=&mut body[0]else{unreachable!()};let ConsumerStep::Capture{value,..}=&mut steps[0]else{unreachable!()};
        *value=if kind==NodeKind::ListLiteral{Expr::ListLit(vec![Expr::Int(11)])}else{Expr::Call{name:"collect2".into(),args:vec![Expr::Int(11),Expr::Str("b".into())]}};assert!(import_native_frame_script(&bad).is_err());
        let(typed,_)=graph(kind,&["a","b"],true);let(compiled,_)=roundtrip(&typed);let id=producer(&compiled.ast);let bad=rewrite(&compiled.ast,&id,&format!("{MARK}{id}"));assert!(import_native_frame_script(&bad).is_err(),"typed inputs cannot be declared property-only");
        let mut bad=compiled.ast.clone();let Statement::Function{body,..}=&mut bad[0]else{unreachable!()};let Statement::ScheduledConsumer{steps,..}=&mut body[0]else{unreachable!()};let ConsumerStep::Capture{value,..}=&mut steps[0]else{unreachable!()};
        match value{Expr::ListLit(items)|Expr::Call{args:items,..}=>{items.pop();},_=>unreachable!()};assert!(import_native_frame_script(&bad).is_err(),"typed declared arity cannot differ from actual expression arity");
        let _=plain;
    }
}
