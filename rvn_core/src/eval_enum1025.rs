//! Enum scalar contracts on the existing computation/evaluator, not a VM.
use super::*;
use rvn_parser::enum_asset::{EnumAssetId,EnumMemberId};
pub(super) const CONSTRUCTOR:&str="__rvn_enum_value_v1";
pub(super) fn construct(resources:&crate::enum_runtime_resources1025::EnumAssets1025,args:&[Value])->EvalResult<Value>{
 let [Value::Str(asset),Value::Str(member)]=args else{return Err(EvalError::InvalidFunction("Enum value requires its explicit canonical asset/member UUID strings".into()));};
 let asset=EnumAssetId::parse(asset).map_err(EvalError::InvalidFunction)?;
 let member=EnumMemberId::parse(member).map_err(EvalError::InvalidFunction)?;
 resources.value(asset,member).map(Value::Enum).map_err(EvalError::InvalidFunction)
}
pub(super) fn binary(op:&BinOpKind,left:&Value,right:&Value)->Option<EvalResult<Value>>{
 if !matches!(left,Value::Enum(_))&&!matches!(right,Value::Enum(_)){return None;}
 Some(match (op,left,right){
  (BinOpKind::Eq,Value::Enum(a),Value::Enum(b)) if a.asset==b.asset=>Ok(Value::Bool(a==b)),
  (BinOpKind::Ne,Value::Enum(a),Value::Enum(b)) if a.asset==b.asset=>Ok(Value::Bool(a!=b)),
  _=>Err(type_err(op,left,right)),
 })
}
pub(super) fn contains(value:&Value)->bool{match value{Value::Enum(_)=>true,Value::List(values)=>values.iter().any(contains),Value::Dict(values)=>values.values().any(contains),_=>false}}
#[cfg(test)]
#[path="eval_enum_tests1025.rs"]
mod tests;

#[path="enum_program_admission1027.rs"]
mod program_admission1027;
pub(super) fn validate_program_literals1027(script:&[rvn_parser::Statement],resources:&crate::enum_runtime_resources1025::EnumAssets1025)->EvalResult<()>{program_admission1027::validate(script,resources)}
