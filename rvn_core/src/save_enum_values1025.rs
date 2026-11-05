//! Typed Serde walk of the existing checkpoint object, including all saved
//! continuations. Object keys/labels are NEVER interpreted as nominal values.
//! Only the real Rust Value::Enum newtype variant contributes a reference.
use rvn_parser::enum_asset::EnumValue;
use serde::{Serialize,ser::{self,Serializer}};
use std::collections::BTreeSet;
type Error=serde_json::Error;
type Result<T>=std::result::Result<T,Error>;
struct Scan { values:BTreeSet<EnumValue> }
struct Compound<'a>{scan:&'a mut Scan,cursor:bool,kind:Option<String>}
impl<'a> Compound<'a>{
 fn plain(scan:&'a mut Scan)->Self{Self{scan,cursor:false,kind:None}}
 fn structure(scan:&'a mut Scan,name:&str)->Self{Self{scan,cursor:name=="TaskCursor",kind:None}}
 fn field<T:Serialize+?Sized>(&mut self,key:&str,value:&T)->Result<()>{
  if self.cursor&&key=="kind"{self.kind=Some(serde_json::from_value(serde_json::to_value(value)?)?);return Ok(());}
  if self.cursor&&key=="data"{
   let kind=self.kind.as_deref().ok_or_else(||<Error as ser::Error>::custom("Typed TaskCursor lacks its actual kind before data"))?;
   match kind{
    "each_next"=>{let raw=serde_json::to_value(value)?;
     if raw.as_array().is_none_or(|items|items.len()>crate::value_limits::MAX_COLLECTION_ITEMS){return Err(<Error as ser::Error>::custom("Typed Each cursor exceeds its existing collection budget or shape"));}
     let values:Vec<rvn_parser::Value>=serde_json::from_value(raw)?;
     return values.serialize(&mut *self.scan);
    },
    "consumer_choice"|"consumer_choice_selected"|"consumer_choice_label"=>{let raw=serde_json::to_value(value)?;
     if raw.as_array().filter(|tuple|tuple.len()==2).and_then(|tuple|tuple[1].as_array()).is_none_or(|items|items.len()>crate::value_limits::MAX_COLLECTION_ITEMS){return Err(<Error as ser::Error>::custom("Typed Choice cursor exceeds its existing collection budget or tuple shape"));}
     let (index,values):(usize,Vec<rvn_parser::Value>)=serde_json::from_value(raw)?;
     return (index,values).serialize(&mut *self.scan);
    },
    "assign"|"binary"|"block"|"bool"|"branch"|"call"|"charge"|"command"|"consumer_branch"|"consumer_capture"|"consumer_dialogue_input"|"consumer_end"|"consumer_inputs"|"consumer_inspect"|"consumer_iteration"|"consumer_loop"|"consumer_next"|"curve_next"|"curve_value"|"each_ready"|"expr"|"function_end"|"index"|"initializer_entry"|"list"|"macro_operand_next_v1"|"macro_ready"|"native_loop_header"|"native_loop_resume"|"neg"|"not"|"range_exit"|"range_next"|"range_ready"|"return"|"root_end"|"short"|"statement"|"while"|"while_ready"=>{},
    _=>return Err(<Error as ser::Error>::custom("Unknown actual TaskCursor kind requires an explicit nominal inventory contract")),
   }
  }
  value.serialize(&mut *self.scan)
 }
}
pub(crate) fn collect(value:&(impl Serialize+?Sized))->Result<Vec<EnumValue>>{
 // Borrowed existing byte/depth/node count precedes any cursor JSON clone or
 // typed allocation; no source/player/continuation DTO is rebaselined here.
 crate::save::consumer_checkpoint::preflight_serialized(value,crate::save::consumer_checkpoint::Save15::MAX_BYTES).map_err(<Error as ser::Error>::custom)?;
 let mut scan=Scan{values:BTreeSet::new()};value.serialize(&mut scan)?;Ok(scan.values.into_iter().collect())
}
macro_rules! scalar {($($name:ident($ty:ty)),*)=>{$(fn $name(self,_: $ty)->Result<()>{Ok(())})*};}
impl<'a> Serializer for &'a mut Scan {
 type Ok=();type Error=Error;
 type SerializeSeq=Compound<'a>;type SerializeTuple=Compound<'a>;type SerializeTupleStruct=Compound<'a>;type SerializeTupleVariant=Compound<'a>;type SerializeMap=Compound<'a>;type SerializeStruct=Compound<'a>;type SerializeStructVariant=Compound<'a>;
 scalar!(serialize_bool(bool),serialize_i8(i8),serialize_i16(i16),serialize_i32(i32),serialize_i64(i64),serialize_i128(i128),serialize_u8(u8),serialize_u16(u16),serialize_u32(u32),serialize_u64(u64),serialize_u128(u128),serialize_f32(f32),serialize_f64(f64),serialize_char(char),serialize_str(&str),serialize_bytes(&[u8]));
 fn serialize_none(self)->Result<()>{Ok(())}
 fn serialize_some<T:Serialize+?Sized>(self,value:&T)->Result<()>{value.serialize(self)}
 fn serialize_unit(self)->Result<()>{Ok(())}
 fn serialize_unit_struct(self,_:&'static str)->Result<()>{Ok(())}
 fn serialize_unit_variant(self,_:&'static str,_:u32,_:&'static str)->Result<()>{Ok(())}
 fn serialize_newtype_struct<T:Serialize+?Sized>(self,_:&'static str,value:&T)->Result<()>{value.serialize(self)}
 fn serialize_newtype_variant<T:Serialize+?Sized>(self,name:&'static str,_:u32,variant:&'static str,value:&T)->Result<()>{
  if name=="Value"&&variant=="Enum"{self.values.insert(serde_json::from_value(serde_json::to_value(value)?)?);Ok(())}else{value.serialize(self)}
 }
 fn serialize_seq(self,_:Option<usize>)->Result<Self::SerializeSeq>{Ok(Compound::plain(self))}
 fn serialize_tuple(self,_:usize)->Result<Self::SerializeTuple>{Ok(Compound::plain(self))}
 fn serialize_tuple_struct(self,_:&'static str,_:usize)->Result<Self::SerializeTupleStruct>{Ok(Compound::plain(self))}
 fn serialize_tuple_variant(self,_:&'static str,_:u32,_:&'static str,_:usize)->Result<Self::SerializeTupleVariant>{Ok(Compound::plain(self))}
 fn serialize_map(self,_:Option<usize>)->Result<Self::SerializeMap>{Ok(Compound::plain(self))}
 fn serialize_struct(self,name:&'static str,_:usize)->Result<Self::SerializeStruct>{Ok(Compound::structure(self,name))}
 fn serialize_struct_variant(self,_:&'static str,_:u32,_:&'static str,_:usize)->Result<Self::SerializeStructVariant>{Ok(Compound::plain(self))}
 fn is_human_readable(&self)->bool{true}
}
macro_rules! sequence {($($trait:ident),*)=>{$(impl ser::$trait for Compound<'_>{type Ok=();type Error=Error;fn serialize_element<T:Serialize+?Sized>(&mut self,value:&T)->Result<()>{value.serialize(&mut *self.scan)}fn end(self)->Result<()>{Ok(())}})*};}
sequence!(SerializeSeq,SerializeTuple);
macro_rules! tuple_fields {($($trait:ident),*)=>{$(impl ser::$trait for Compound<'_>{type Ok=();type Error=Error;fn serialize_field<T:Serialize+?Sized>(&mut self,value:&T)->Result<()>{value.serialize(&mut *self.scan)}fn end(self)->Result<()>{Ok(())}})*};}
tuple_fields!(SerializeTupleStruct,SerializeTupleVariant);
impl ser::SerializeMap for Compound<'_>{type Ok=();type Error=Error;fn serialize_key<T:Serialize+?Sized>(&mut self,value:&T)->Result<()>{value.serialize(&mut *self.scan)}fn serialize_value<T:Serialize+?Sized>(&mut self,value:&T)->Result<()>{value.serialize(&mut *self.scan)}fn end(self)->Result<()>{Ok(())}}
macro_rules! struct_fields {($($trait:ident),*)=>{$(impl ser::$trait for Compound<'_>{type Ok=();type Error=Error;fn serialize_field<T:Serialize+?Sized>(&mut self,_:&'static str,value:&T)->Result<()>{value.serialize(&mut *self.scan)}fn end(self)->Result<()>{Ok(())}})*};}
struct_fields!(SerializeStructVariant);
impl ser::SerializeStruct for Compound<'_>{type Ok=();type Error=Error;fn serialize_field<T:Serialize+?Sized>(&mut self,key:&'static str,value:&T)->Result<()>{self.field(key,value)}fn end(self)->Result<()>{Ok(())}}
#[cfg(test)]
#[path="save_enum_values_tests1025.rs"]
mod tests;
