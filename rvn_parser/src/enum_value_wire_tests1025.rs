use super::*;
const A:&str="329f41eae6ba4b4bb71dfaf60fb50001";
const M:&str="229f41eae6ba4b4bb71dfaf60fb50001";
#[test] fn enum_wire1025_roundtrip_keeps_both_opaque_identities(){
 let value=EnumValue{asset:EnumAssetId::parse(A).unwrap(),member:EnumMemberId::parse(M).unwrap()};
 let wire=serde_json::to_string(&crate::Value::Enum(value)).unwrap();
 assert_eq!(wire,format!("{{\"Enum\":{{\"asset\":\"{A}\",\"member\":\"{M}\"}}}}"));
 assert_eq!(serde_json::from_str::<crate::Value>(&wire).unwrap(),crate::Value::Enum(value));
}
#[test] fn enum_wire1025_rejects_zero_noncanonical_unknown_and_numeric_payloads(){
 for payload in [format!("{{\"asset\":\"{A}\",\"member\":\"{}\"}}","0".repeat(32)),format!("{{\"asset\":\"{}\",\"member\":\"{M}\"}}",A.to_uppercase()),format!("{{\"asset\":\"{A}\",\"member\":1}}"),format!("{{\"asset\":\"{A}\",\"member\":\"{M}\",\"ordinal\":0}}")]{assert!(serde_json::from_str::<EnumValue>(&payload).is_err(),"{payload}");}
}
#[test] fn enum_wire1025_same_member_in_different_assets_is_not_the_same_value(){
 let member=EnumMemberId::parse(M).unwrap();let a=EnumValue{asset:EnumAssetId::parse(A).unwrap(),member};
 let b=EnumValue{asset:EnumAssetId::parse("329f41eae6ba4b4bb71dfaf60fb50002").unwrap(),member};assert_ne!(a,b);
}
