//! Native demand loops inside the existing lexical continuation stack.
//! The owning Task is inert candidate data; source/frames remain authoritative.
use super::*;
use crate::native_loop_cursor::{AfterBody,DemandLoopCursor};
use rvn_parser::scheduled_source::native_loop_contract::{self as contract,Kind,Phase};

#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
#[serde(rename_all="snake_case")]
pub(super) enum LoopPhase { First, Test, Body }
#[derive(Clone,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LexicalLoopState {
    pub(super) cursor:Option<DemandLoopCursor>,
    pub(super) phase:LoopPhase,
    pub(super) break_requested:bool,
    pub(super) break_site:Option<Context>,
}
impl LexicalLoopState {
    pub(super) fn validate(&self,statement:&Statement)->EvalResult<()> {
        let binding=contract::binding(statement).map_err(EvalError::InvalidFunction)?.ok_or_else(||invalid("Lexical cursor has no frozen Native loop"))?;
        if self.break_requested!=self.break_site.is_some(){return Err(invalid("Native Break latch lost its actual source site"));}
        match(self.phase,binding.kind,&self.cursor){
            (LoopPhase::First,Kind::Range,None) if !self.break_requested=>return Ok(()),
            (LoopPhase::First,_,_)|(_,_,None)=>return Err(invalid("Native lexical phase lost its First/cursor contract")),
            (_,Kind::Array,Some(DemandLoopCursor::ArrayV1{..})) if !self.break_requested=>{},
            (_,Kind::Range,Some(DemandLoopCursor::RangeV1{..}))=>{},
            _=>return Err(invalid("Native lexical cursor kind differs from its frozen loop")),
        }
        let cursor=self.cursor.as_ref().unwrap();cursor.validate()?;
        if self.phase==LoopPhase::Body {cursor.validate_body()?;}
        else {
            let first=match cursor{DemandLoopCursor::ArrayV1{..}=>0,DemandLoopCursor::RangeV1{first,..}=>*first};
            if self.break_requested||i128::from(cursor.index())-i128::from(first)!=cursor.visits() as i128{return Err(invalid("Native lexical Test is not its next real comparison"));}
        }
        Ok(())
    }
}

impl Continuation {
    fn lexical_owner_index(&self,ctx:&Context)->EvalResult<usize> {
        let mut matching=self.tasks.iter().enumerate().filter_map(|(i,t)|matches!(t,Task::NativeLoopResume{ctx:owner,..}if owner==ctx).then_some(i));
        let index=matching.next().ok_or_else(||invalid("Native lexical phase lost its actual stack owner"))?;
        if matching.next().is_some(){return Err(invalid("Native lexical phase has duplicate stack owners"));}Ok(index)
    }
    pub(super) fn lexical_phase_from_tasks(tasks:&[Task],ctx:&Context)->EvalResult<Option<(Phase,Vec<usize>,Expr,Context)>> {
        let mut matching=tasks.iter().filter_map(|task|match task{Task::NativeLoopResume{statement,state,ctx:owner}if owner==ctx=>Some((statement,state)),_=>None});
        let Some((statement,state))=matching.next()else{return Ok(None);};
        if matching.next().is_some(){return Err(invalid("Duplicate Native lexical source owners"));}
        state.validate(statement)?;
        let phase=match state.phase{LoopPhase::First=>Phase::First,LoopPhase::Test=>Phase::Test,LoopPhase::Body=>return Err(invalid("Native body cannot gather another header phase"))};
        let Statement::ScheduledConsumer{steps,..}=statement else{unreachable!()};
        let root=contract::phase_expression(statement,phase).map_err(EvalError::InvalidFunction)?;
        let selected=rvn_parser::loop_phase_dependencies::capture_step_indices(steps,root).map_err(EvalError::InvalidFunction)?;
        let binding=contract::binding(statement).map_err(EvalError::InvalidFunction)?.unwrap();
        let route=match(binding.kind,phase){(Kind::Array,Phase::Test)=>Route::Collection,(Kind::Range,Phase::First)=>Route::First,(Kind::Range,Phase::Test)=>Route::Last,_=>return Err(invalid("Array has no First request"))};
        Ok(Some((phase,selected,root.clone(),Context{depth:0,..ctx.child(Route::Body).child(Route::Statement(0)).child(route)})))
    }
    pub(super) fn loop_phase_details(&self,ctx:&Context)->EvalResult<Option<(Phase,Vec<usize>,Expr,Context)>> {
        if let Some(details)=Self::lexical_phase_from_tasks(&self.tasks,ctx)?{return Ok(Some(details));}
        self.native_phase_details(ctx)
    }
    pub(super) fn begin_lexical_loop(&mut self,statement:Statement,ctx:Context)->EvalResult<()> {
        let binding=contract::binding(&statement).map_err(EvalError::InvalidFunction)?.ok_or_else(||invalid("Expected actual Native lexical loop"))?;
        if self.frames[ctx.frame].variables.contains_key(&binding.index){return Err(invalid("Native lexical index collides with an existing binding"));}
        if self.frames[ctx.frame].active_consumer.is_some(){return Err(invalid("Native lexical loop cannot borrow an active consumer"));}
        let state=match binding.kind{Kind::Array=>LexicalLoopState{cursor:Some(DemandLoopCursor::array()),phase:LoopPhase::Test,break_requested:false,break_site:None},Kind::Range=>LexicalLoopState{cursor:None,phase:LoopPhase::First,break_requested:false,break_site:None}};
        self.tasks.push(Task::NativeLoopResume{statement,state,ctx:ctx.clone()});
        self.begin_lexical_phase(&ctx)
    }
    fn begin_lexical_phase(&mut self,ctx:&Context)->EvalResult<()> {
        let index=self.lexical_owner_index(ctx)?;
        let Task::NativeLoopResume{statement,state,..}=&self.tasks[index]else{unreachable!()};
        state.validate(statement)?;
        let phase=state.phase;
        let Statement::ScheduledConsumer{version,scope,consumer,steps,body}=statement else{unreachable!()};
        let(version,scope,consumer,steps,body)=(*version,scope.clone(),consumer.clone(),steps.clone(),body.clone());
        if phase==LoopPhase::Body{return Err(invalid("Body is not a Native input phase"));}
        if self.frames[ctx.frame].active_consumer.is_some(){return Err(invalid("Native phase cannot replace an active consumer"));}
        if phase==LoopPhase::Test{self.tick()?;}
        self.frames[ctx.frame].active_consumer=Some(ConsumerContext{scope:scope.clone(),captures:Default::default(),inspectors:Default::default()});
        self.tasks.push(Task::NativeLoopHeader{phase,ctx:ctx.clone()});
        self.tasks.push(Task::ConsumerEnd{frame:ctx.frame,ctx:ctx.clone()});
        self.tasks.push(Task::ConsumerNext{version,consumer,scope,steps,index:0,body,value_only:false,ctx:ctx.clone()});
        Ok(())
    }
    pub(super) fn lexical_header_ready(&mut self,phase:LoopPhase,ctx:Context)->EvalResult<()> {
        let input=self.pop()?;
        if self.frames[ctx.frame].active_consumer.is_some(){return Err(invalid("Native header consumer must close before its body"));}
        let index=self.lexical_owner_index(&ctx)?;
        let Task::NativeLoopResume{statement,mut state,ctx:owner}=self.tasks.remove(index)else{unreachable!()};
        state.validate(&statement)?;
        if phase!=state.phase{return Err(invalid("Native header result belongs to another phase"));}
        if phase==LoopPhase::First{
            state.cursor=Some(DemandLoopCursor::range(input)?);state.phase=LoopPhase::Test;
            self.tasks.insert(index,Task::NativeLoopResume{statement,state,ctx:owner});return self.begin_lexical_phase(&ctx);
        }
        if phase!=LoopPhase::Test{return Err(invalid("Native body has no header result"));}
        let binding=contract::binding(&statement).map_err(EvalError::InvalidFunction)?.unwrap();
        let entered=state.cursor.as_mut().unwrap().enter(&input)?;
        if !entered{self.frames[ctx.frame].variables.remove(&binding.index);return Ok(());}
        self.frames[ctx.frame].variables.insert(binding.index,Value::Int(state.cursor.as_ref().unwrap().index()));
        let Statement::ScheduledConsumer{body,..}=&statement else{unreachable!()};
        let body=match &body[0]{Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>body.clone(),_=>unreachable!()};
        state.phase=LoopPhase::Body;
        self.tasks.insert(index,Task::NativeLoopResume{statement,state,ctx:owner});
        self.tasks.push(Task::Block(body,0,ctx.child(Route::Body).child(Route::Statement(0)).child(Route::Body)));Ok(())
    }
    pub(super) fn lexical_body_complete(&mut self,statement:Statement,mut state:LexicalLoopState,ctx:Context)->EvalResult<()> {
        state.validate(&statement)?;
        if state.phase!=LoopPhase::Body{return Err(invalid("Native resume reached before its real input completed"));}
        let binding=contract::binding(&statement).map_err(EvalError::InvalidFunction)?.unwrap();
        // This owner Task was popped by execute; remove its private projection
        // before fallible advancement so an overflow/error cannot orphan it.
        self.frames[ctx.frame].variables.remove(&binding.index);
        if state.cursor.as_mut().unwrap().after_body(state.break_requested)?==AfterBody::CompletedWithoutComparison{return Ok(());}
        state.phase=LoopPhase::Test;
        self.tasks.push(Task::NativeLoopResume{statement,state,ctx:ctx.clone()});self.begin_lexical_phase(&ctx)
    }
    pub(super) fn lexical_break(&mut self,ctx:&Context)->EvalResult<bool> {
        for task in self.tasks.iter_mut().rev(){match task{
            Task::FunctionEnd{..}|Task::UnitEnd{..}=>break,
            Task::RangeExit(owner) if owner.frame==ctx.frame=>return Ok(false),
            Task::NativeLoopResume{statement,state,ctx:owner} if owner.frame==ctx.frame=>{
                let binding=contract::binding(statement).map_err(EvalError::InvalidFunction)?.unwrap();
                if binding.kind==Kind::Range{
                    state.validate(statement)?;if state.phase!=LoopPhase::Body{return Err(invalid("Break outside the entered Native body"));}
                    state.break_requested=true;state.break_site=Some(ctx.clone());return Ok(true);
                }
            },_=>{},
        }}Ok(false)
    }
    pub(super) fn cleanup_lexical_tasks(&mut self,start:usize) {
        for task in self.tasks[start..].iter().rev(){if let Task::NativeLoopResume{statement,ctx,..}=task{
            if let Ok(Some(binding))=contract::binding(statement){if let Some(frame)=self.frames.get_mut(ctx.frame){frame.variables.remove(&binding.index);}}
        }}
    }
}
