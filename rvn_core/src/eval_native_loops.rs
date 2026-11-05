//! Explicit demand iteration on the existing owned computation. No renderer,
//! global cache, object registry or second VM is introduced here.
use super::*;
use rvn_parser::scheduled_source::native_loop_contract::{self as contract, Kind, Phase};
use crate::native_loop_cursor::DemandLoopCursor;

#[cfg(test)]
#[path="eval_native_loop_break_tests_0764.rs"]
mod break_tests;

impl Computation<'_> {
    pub(super) fn request_computation_range_break(&mut self) {
        match self.range_break_owners.last_mut() {
            Some(ComputationRangeBreak::Native { requested })=>*requested=true,
            Some(ComputationRangeBreak::Legacy)|None=>self.range_break=true,
        }
    }
    pub(super) fn legacy_range_body(&mut self,body:&[Statement],variables:&mut HashMap<String,Value>)->EvalResult<Option<Value>> {
        self.range_break_owners.push(ComputationRangeBreak::Legacy);
        let result=self.block(body,variables);
        let owner=self.range_break_owners.pop();
        debug_assert!(matches!(owner,Some(ComputationRangeBreak::Legacy)));
        result
    }
    fn native_range_body(&mut self,body:&[Statement],variables:&mut HashMap<String,Value>)->EvalResult<(Option<Value>,bool)> {
        self.range_break_owners.push(ComputationRangeBreak::Native{requested:false});
        let result=self.block(body,variables);
        let Some(ComputationRangeBreak::Native{requested})=self.range_break_owners.pop()else{
            return Err(EvalError::InvalidFunction("Native synchronous Range body lost its actual activation".into()));
        };
        result.map(|value|(value,requested))
    }
    pub(super) fn native_loop_input(&mut self, statement: &Statement, scope: &str, phase: Phase,
        variables: &mut HashMap<String,Value>) -> EvalResult<Value> {
        let Statement::ScheduledConsumer{steps,..}=statement else{return Err(EvalError::InvalidFunction("Native loop input lost its real consumer".into()));};
        let root=contract::phase_expression(statement,phase).map_err(EvalError::InvalidFunction)?;
        let indices=rvn_parser::loop_phase_dependencies::capture_step_indices(steps,root).map_err(EvalError::InvalidFunction)?;
        // This direct evaluator has no suspended Rust stack or portable task.
        // The debugger path keeps original producer ordinals when it schedules
        // the same selection; only this local slice is materialized here.
        let selected:Vec<_>=indices.iter().map(|index|steps[*index].clone()).collect();
        self.scheduled_inner(scope,&selected,ConsumerBody::Value(root),variables)?
            .ok_or_else(||EvalError::InvalidFunction("Native phase produced no input".into()))
    }

    pub(super) fn scheduled_native_loop(&mut self, statement: &Statement, variables: &mut HashMap<String,Value>)
        -> EvalResult<Option<Value>> {
        let binding=contract::binding(statement).map_err(EvalError::InvalidFunction)?
            .ok_or_else(||EvalError::InvalidFunction("Expected explicit Native loop v3".into()))?;
        let Statement::ScheduledConsumer{scope,body,..}=statement else{unreachable!()};
        let body=match &body[0]{Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>body,_=>unreachable!()};
        let prior=variables.get(&binding.index).cloned();
        let prior_locals=self.handler_locals.clone();
        if self.calls==0 {if let Some(locals)=&mut self.handler_locals{locals.insert(binding.index.clone());}}
        let result=(|| {
            let mut cursor=match binding.kind {
                Kind::Array=>DemandLoopCursor::array(),
                Kind::Range=>DemandLoopCursor::range(self.native_loop_input(statement,scope,Phase::First,variables)?)?,
            };
            loop {
                self.tick()?;
                let input=self.native_loop_input(statement,scope,Phase::Test,variables)?;
                if !cursor.enter(&input)? {return Ok(None);}
                // The actual loop activation supplies this private projection.
                // Authored source validation forbids writes/capture outside its
                // lexical owner; cleanup also prevents publishing it as global.
                variables.insert(binding.index.clone(),Value::Int(cursor.index()));
                let (value,native_break)=if binding.kind==Kind::Range {
                    self.native_range_body(body,variables)?
                }else{(self.block(body,variables)?,false)};
                if value.is_some(){return Ok(value);}
                if self.range_break {
                    if binding.kind==Kind::Range {self.range_break=false;}
                    return Ok(None);
                }
                if cursor.after_body(native_break)?==crate::native_loop_cursor::AfterBody::CompletedWithoutComparison {
                    return Ok(None);
                }
            }
        })();
        match prior {Some(value)=>{variables.insert(binding.index,value);},None=>{variables.remove(&binding.index);}}
        self.handler_locals=prior_locals;
        result
    }
}
