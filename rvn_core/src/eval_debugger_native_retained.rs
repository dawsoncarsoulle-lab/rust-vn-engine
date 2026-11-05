//! Inert retained phase data is rebound to the same frozen source and real
//! startup owner by the existing completed-record admission path.
use super::*;
use rvn_parser::scheduled_source::native_loop_contract::{self as contract,Kind,Phase};
use crate::native_loop_cursor::DemandLoopCursor;

impl PortableRecord {
    pub(crate) fn validate_native_loop_cursor(statement:&Statement,records:&[Self],cursor:&DemandLoopCursor)->EvalResult<()> {
        let binding=contract::binding(statement).map_err(EvalError::InvalidFunction)?.ok_or_else(||invalid("Retained Native data has no actual v3 source"))?;
        cursor.validate_body()?;
        let count=if binding.kind==Kind::Range{2}else{1};
        if records.len()!=count{return Err(invalid("Native retained roots differ from First/Test contract"));}
        for (i,record) in records.iter().enumerate(){
            record.validate_loop_data()?;
            let expected=if count==2 && i==0{Phase::First}else{Phase::Test};
            if contract::owned_engine_phase(&binding,&record.request.variables).map_err(EvalError::InvalidFunction)?!=expected
                ||record.request.variables!=record.result.variables{
                return Err(invalid("Native retained phase differs from its real private Engine environment"));
            }
            if [binding.index.as_str(),binding.cursor.as_str(),binding.phase.as_str()].iter().any(|name|record.request.state.globals.contains_key(*name)||record.result.state.globals.contains_key(*name)){
                return Err(invalid("Native retained phase cannot obtain authority from a global projection"));
            }
            match expected {
                Phase::First=>{
                    let DemandLoopCursor::RangeV1{first,..}=cursor else{return Err(invalid("Array cannot borrow a FirstIndex root"));};
                    if record.request.variables.contains_key(&binding.cursor)||record.result.value!=Some(Value::Int(*first)){
                        return Err(invalid("Native retained FirstIndex differs from its actual initialized cursor"));
                    }
                },
                Phase::Test=>{
                    if record.request.variables.get(&binding.cursor)!=Some(&Value::Int(cursor.index())){
                        return Err(invalid("Native retained Test belongs to another current cursor"));
                    }
                },
            }
        }
        Ok(())
    }
}
