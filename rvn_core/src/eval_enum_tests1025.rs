use super::*;
use rvn_parser::{enum_asset::{EnumAsset,EnumCommand},enum_asset_json::EnumIndexInput,enum_asset_admission1025::EnumAssets1025};
const A:&str="329f41eae6ba4b4bb71dfaf60fb50001";const B:&str="329f41eae6ba4b4bb71dfaf60fb50002";const M:&str="229f41eae6ba4b4bb71dfaf60fb50001";
fn resources()->EnumAssets1025{let mut files=Vec::new();let rows=[(A,Some("Alpha.rvnenum")),(B,Some("Beta.rvnenum"))];for (id,path) in rows{let asset=EnumAsset::new(EnumAssetId::parse(id).unwrap(),"Alpha".into()).unwrap().prepare(EnumCommand::Add{member:EnumMemberId::parse(M).unwrap(),display_name:"First".into(),description:String::new()}).unwrap().after().clone();files.push((path.unwrap().into(),asset.encode().unwrap().into_bytes()));}EnumAssets1025::admit(EnumIndexInput::Table(&rows),&files,true).unwrap()}
fn expression(asset:&str,member:&str)->Expr{Expr::Call{name:CONSTRUCTOR.into(),args:vec![Expr::Str(asset.into()),Expr::Str(member.into())]}}
#[test] fn enum_eval1025_constructor_uses_actual_admitted_asset_and_member(){let library=FunctionLibrary::default().with_enum_assets1025(std::sync::Arc::new(resources()));assert_eq!(library.eval(&expression(A,M),&HashMap::new()).unwrap(),Value::Enum(resources().value(EnumAssetId::parse(A).unwrap(),EnumMemberId::parse(M).unwrap()).unwrap()));}
#[test] fn enum_eval1025_missing_resource_and_unknown_member_refuse(){assert!(FunctionLibrary::default().eval(&expression(A,M),&HashMap::new()).is_err());let library=FunctionLibrary::default().with_enum_assets1025(std::sync::Arc::new(resources()));assert!(library.eval(&expression(A,"229f41eae6ba4b4bb71dfaf60fb50099"),&HashMap::new()).is_err());}
#[test] fn enum_eval1025_equality_same_asset_and_cross_asset_type_error(){let library=FunctionLibrary::default().with_enum_assets1025(std::sync::Arc::new(resources()));let expr=|a,b|Expr::BinOp{op:BinOpKind::Eq,left:Box::new(expression(a,M)),right:Box::new(expression(b,M))};assert_eq!(library.eval(&expr(A,A),&HashMap::new()).unwrap(),Value::Bool(true));assert!(library.eval(&expr(A,B),&HashMap::new()).is_err());}
#[test] fn enum_eval1025_boolean_numeric_name_and_string_coercions_refuse(){let library=FunctionLibrary::default().with_enum_assets1025(std::sync::Arc::new(resources()));assert!(library.eval(&Expr::Not(Box::new(expression(A,M))),&HashMap::new()).is_err());for right in [Expr::Int(0),Expr::Str("First".into()),Expr::Bool(true)]{for op in [BinOpKind::Add,BinOpKind::Eq]{assert!(library.eval(&Expr::BinOp{op,left:Box::new(expression(A,M)),right:Box::new(right.clone())},&HashMap::new()).is_err());}}}
#[test] fn enum_eval1025_variable_and_collection_values_require_current_admission(){let value=resources().value(EnumAssetId::parse(A).unwrap(),EnumMemberId::parse(M).unwrap()).unwrap();let globals=HashMap::from([("held".into(),Value::List(vec![Value::Enum(value)]))]);assert!(FunctionLibrary::default().eval(&Expr::Var("held".into()),&globals).is_err());assert!(FunctionLibrary::default().with_enum_assets1025(std::sync::Arc::new(resources())).eval(&Expr::Var("held".into()),&globals).is_ok());}
#[test] fn enum_eval1025_reserved_constructor_cannot_be_shadowed_by_script_function(){
 // The source parser and public-AST library boundary must both reject this binding.
 assert!(rvn_parser::parse("function __rvn_enum_value_v1(a,b) { return 0 }").is_err());
 let mut source=rvn_parser::parse("function enum_shadow_probe1025(a,b) { return 0 }").unwrap();
 match source.as_mut_slice(){
  [rvn_parser::Statement::Function{name,..}]=>*name=CONSTRUCTOR.into(),
  _=>panic!("expected one real parsed function"),
 }
 assert!(FunctionLibrary::from_script(&source).is_err());
}
