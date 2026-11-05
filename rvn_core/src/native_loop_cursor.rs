//! Narrow arithmetic/state for the explicit demand-loop contract. A cursor is
//! inert data: only the existing source/Engine/continuation owner may use it.
use crate::eval::{EvalError, EvalResult};
use rvn_parser::Value;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag="kind", deny_unknown_fields)]
pub(crate) enum DemandLoopCursor {
    ArrayV1 { index: i64, visits: usize },
    RangeV1 { first: i64, index: i64, visits: usize },
}
/// The real owner chooses its existing continuation after a completed body.
/// This arithmetic result neither owns nor abandons Sequence/Branch frames.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum AfterBody { CompareNext, CompletedWithoutComparison }

impl DemandLoopCursor {
    pub(crate) fn array() -> Self { Self::ArrayV1 { index: 0, visits: 0 } }
    pub(crate) fn range(first: Value) -> EvalResult<Self> {
        let Value::Int(first)=first else { return Err(EvalError::InvalidFunction("Native FirstIndex requires an integer".into())); };
        Ok(Self::RangeV1 { first, index: first, visits: 0 })
    }
    pub(crate) fn index(&self) -> i64 { match self { Self::ArrayV1 {index,..}|Self::RangeV1 {index,..} => *index } }
    pub(crate) fn visits(&self) -> usize { match self { Self::ArrayV1 {visits,..}|Self::RangeV1 {visits,..} => *visits } }
    /// Called only at the NEXT comparison after a completed body. Break does
    /// not call this and does not request another header input.
    pub(crate) fn advance(&mut self) -> EvalResult<()> {
        self.validate_body()?;
        let next=self.index().checked_add(1).ok_or(EvalError::NumericOverflow)?;
        match self { Self::ArrayV1 {index,..}|Self::RangeV1 {index,..} => *index=next }
        Ok(())
    }
    /// Used only by an explicit Native v3 owner after its actual body returns.
    /// A latched range Break retains the entered index, even at i64::MAX, and
    /// requests no new bound. Legacy loop callers keep their existing paths.
    pub(crate) fn after_body(&mut self,break_requested:bool)->EvalResult<AfterBody>{
        self.validate_body()?;
        if break_requested{
            if !matches!(self,Self::RangeV1{..}){return Err(EvalError::InvalidFunction("Native array loop has no Break input".into()));}
            return Ok(AfterBody::CompletedWithoutComparison);
        }
        self.advance()?;Ok(AfterBody::CompareNext)
    }
    /// A current header value is consumed once; no collection/bound is cached
    /// by this arithmetic kernel and no producer is called here.
    pub(crate) fn enter(&mut self, input: &Value) -> EvalResult<bool> {
        self.validate()?;
        let first=match self { Self::ArrayV1 {..}=>0,Self::RangeV1 {first,..}=>*first };
        if i128::from(self.index())-i128::from(first)!=self.visits() as i128 {
            return Err(EvalError::InvalidFunction("Native comparison has not advanced from its prior body".into()));
        }
        let entered=match (self.index(), &*self, input) {
            (index,Self::ArrayV1 {..},Value::List(items)) => usize::try_from(index).ok().is_some_and(|index| index<items.len()),
            (index,Self::RangeV1 {..},Value::Int(last)) => index<=*last,
            (_,Self::ArrayV1 {..},_) => return Err(EvalError::InvalidFunction("Native ArrayLength requires its actual list".into())),
            _ => return Err(EvalError::InvalidFunction("Native LastIndex requires an integer".into())),
        };
        if entered {
            let visits=self.visits().checked_add(1).ok_or(EvalError::NumericOverflow)?;
            if visits>crate::eval::MAX_COMPUTATION_STEPS { return Err(EvalError::ExecutionLimit {limit:"existing computation-step budget for Native loop visits"}); }
            match self { Self::ArrayV1 {visits:old,..}|Self::RangeV1 {visits:old,..} => *old=visits }
        }
        Ok(entered)
    }
    /// Both before-comparison and entered-body states are valid arithmetic
    /// states. The real source/task owner separately checks the saved phase.
    pub(crate) fn validate(&self) -> EvalResult<()> {
        if self.visits()>crate::eval::MAX_COMPUTATION_STEPS { return Err(EvalError::InvalidFunction("Saved Native visit budget differs".into())); }
        let first=match self { Self::ArrayV1 {..}=>0,Self::RangeV1 {first,..}=>*first };
        let delta=i128::from(self.index())-i128::from(first);
        let visits=self.visits() as i128;
        if delta<0 || !(delta==visits || visits>0 && delta==visits-1) {
            return Err(EvalError::InvalidFunction("Saved Native cursor/visit arithmetic is incoherent".into()));
        }
        Ok(())
    }
    pub(crate) fn validate_body(&self) -> EvalResult<()> {
        self.validate()?;
        let first=match self { Self::ArrayV1 {..}=>0,Self::RangeV1 {first,..}=>*first };
        if self.visits()==0 || i128::from(self.index())-i128::from(first)!=self.visits() as i128-1 {
            return Err(EvalError::InvalidFunction("Native body cursor is not the entered visit".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_range_bound_can_shrink_or_grow_without_snapshot_count() {
        let mut cursor=DemandLoopCursor::range(Value::Int(0)).unwrap();
        assert!(cursor.enter(&Value::Int(2)).unwrap());
        assert_eq!(cursor.after_body(false).unwrap(),AfterBody::CompareNext);
        assert!(!cursor.enter(&Value::Int(0)).unwrap());
        assert!(cursor.enter(&Value::Int(4)).unwrap());
        assert_eq!((cursor.index(),cursor.visits()),(1,2));
    }
    #[test]
    fn array_length_is_current_and_element_value_is_not_retained_here() {
        let mut cursor=DemandLoopCursor::array();
        assert!(!cursor.enter(&Value::List(vec![])).unwrap());
        assert!(cursor.enter(&Value::List(vec![Value::Int(11),Value::Int(22)])).unwrap());
        cursor.advance().unwrap();
        assert!(!cursor.enter(&Value::List(vec![Value::Int(99)])).unwrap());
    }
    #[test]
    fn wrong_types_overflow_and_incoherent_saved_phase_are_atomic_refusals() {
        let mut cursor=DemandLoopCursor::range(Value::Int(i64::MAX)).unwrap();
        assert!(cursor.enter(&Value::Int(i64::MAX)).unwrap());
        let before=cursor.clone();
        assert!(cursor.advance().is_err());
        assert_eq!(cursor,before);
        assert_eq!(cursor.after_body(true).unwrap(),AfterBody::CompletedWithoutComparison);
        assert_eq!(cursor,before,"Break cannot advance the entered maximum index");
        assert!(cursor.after_body(false).is_err());assert_eq!(cursor,before);
        let mut array=DemandLoopCursor::array();
        assert!(array.after_body(true).is_err(),"unentered body is never a valid continuation");
        assert!(array.enter(&Value::List(vec![Value::Int(11)])).unwrap());let before_array=array.clone();
        assert!(array.after_body(true).is_err());assert_eq!(array,before_array);
        assert_eq!(array.after_body(false).unwrap(),AfterBody::CompareNext);assert_eq!((array.index(),array.visits()),(1,1));
        assert!(DemandLoopCursor::range(Value::Float(0.)).is_err());
        assert!(DemandLoopCursor::ArrayV1{index:9,visits:1}.validate().is_err());
        assert!(DemandLoopCursor::RangeV1{first:4,index:4,visits:0}.validate_body().is_err());
    }
}
