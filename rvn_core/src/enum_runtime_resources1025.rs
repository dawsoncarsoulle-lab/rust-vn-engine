//! Playback consumes the immutable projection already admitted from project
//! index/files. No disk IO, mutable registry, alias/ordinal resolution or UMG.
pub use rvn_parser::enum_asset_admission1025::EnumAssets1025;
use rvn_parser::Value;
use crate::eval::{EvalResult,EvalError};
pub fn validate_value(resources:&EnumAssets1025,value:&Value)->EvalResult<()> {
 crate::value_limits::value_work(value)?;
 fn visit(resources:&EnumAssets1025,value:&Value)->EvalResult<()>{
  match value{
   Value::Enum(value)=>resources.validate_value(*value).map_err(EvalError::InvalidFunction)?,
   Value::List(values)=>for value in values{visit(resources,value)?;},
   Value::Dict(values)=>for value in values.values(){visit(resources,value)?;},
   _=>{},
  }Ok(())
 }visit(resources,value)
}
