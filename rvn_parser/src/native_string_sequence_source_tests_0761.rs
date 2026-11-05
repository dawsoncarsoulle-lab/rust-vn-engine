use super::*;
const SCOPE:&str="g761/n1:function_entry|exec_out=1@0";
const CONSUMER:&str="n3:function_return|exec_in=3@1,value=4@2";
fn source(producer:&str,value:&str)->String{format!("// été 🌿\r\nfunction probe(){{\r\nflow.consume_v1(\"{SCOPE}\",\"{CONSUMER}\"){{\r\ncapture \"{producer}\"={value}\r\nreturn __rvn_captured_v1(\"{producer}\")\r\n}}\r\n}}\r\n")}
#[test]
fn sequence0761_source_metadata_preserves_real_ranges_ordinals_and_old_omission(){
    for(kind,port,value)in [("list_literal","list","[]"),("list_literal","list","[\"11\",\"été 🌿\"]"),("function_call","result","collect0()"),("function_call","result","collect2(\"11\",\"été 🌿\")")]{
        let id=format!("l:string_list_v1/n2:{kind}|{port}=2@0");let raw=source(&id,value);let parsed=crate::parse_program(&raw).unwrap();assert_eq!(crate::parse(&raw).unwrap(),parsed.script);let sites=scheduled_step_sources(&parsed).unwrap();assert_eq!(sites.len(),1);assert_eq!(sites[0].source_ordinal,0);assert_eq!(sites[0].definition_root_ordinal,0);assert_eq!(&raw[sites[0].expression.clone()],value);let endpoint=sites[0].endpoint.as_ref().unwrap();assert!(endpoint.legacy_string_sequence);assert_eq!(endpoint.node,2);assert_eq!(endpoint.ports.len(),1);assert_eq!(endpoint.ports[0].pin,2);
        let plain=id.strip_prefix("l:string_list_v1/").unwrap();let old=source(plain,value);let sites=scheduled_step_sources(&crate::parse_program(&old).unwrap()).unwrap();let wire=serde_json::to_value(sites[0].endpoint.as_ref().unwrap()).unwrap();assert!(wire.get("legacy_string_sequence").is_none());let restored:DeclaredEndpoint=serde_json::from_value(wire.clone()).unwrap();assert!(!restored.legacy_string_sequence);assert_eq!(serde_json::to_value(restored).unwrap(),wire);
    }
}
#[test]
fn sequence0761_source_metadata_refuses_unknown_modes_roles_and_nonstring_payloads(){
    for(id,value)in [
        ("l:string_list_v9/n2:list_literal|list=2@0","[]"),
        ("l:string_list_v1/n2:literal|value=2@0","\"x\""),
        ("l:string_list_v1/n2:list_literal|list=2@0,item_0=9@0","[\"x\"]"),
        ("l:string_list_v1/n2:list_literal|list=2@0,item_0=9@0,item_0=10@0","[\"x\"]"),
        ("l:string_list_v1/n2:list_literal|list=2@7","[]"),
        ("l:string_list_v1/n2:list_literal|list=2@0","[11]"),
        ("l:string_list_v1/n2:function_call|result=2@0","collect2(11,\"x\")"),
        ("l:string_list_v1/n2:list_literal|list=2@0","collect0()"),
    ]{let raw=source(id,value);let parsed=crate::parse_program(&raw).unwrap();assert!(scheduled_step_sources(&parsed).is_err(),"accepted {id}={value}");}
}
#[test]
fn sequence0761_conflicting_representation_for_one_actual_node_is_refused(){
    let marked="l:string_list_v1/n2:list_literal|list=2@0";let plain="n2:list_literal|list=2@0";
    let raw=format!("function probe(){{flow.consume_v1(\"{SCOPE}\",\"{CONSUMER}\"){{capture \"{marked}\"=[] capture \"{plain}\"=[] return __rvn_captured_v1(\"{marked}\")}}}}");
    assert!(scheduled_step_sources(&crate::parse_program(&raw).unwrap()).is_err());
}
