//! Live Unit roots use the existing Probe, continuation stack and candidate.
//! All fields below are process-local; portable paths explicitly reject them.
use super::*;

impl Computation<'_> {
    pub(in super::super) fn debugger_unit(&mut self,name:&str,arguments:&[Expr],environment:&HashMap<String,Value>)
        ->Option<EvalResult<()>> {
        let result=self.debugger_root(RootRef::Unit(name,arguments),environment)?;
        Some(result.and_then(|result|{
            if let Some(error)=&result.error{return Err(error.clone());}
            if result.value.is_some()||!matches!(&result.unit_exit,Some(BlockExit::Continue|BlockExit::Unit)) {
                return Err(invalid("Unit trace did not complete its actual resultless root"));
            }
            self.install_debugger_result(&result);Ok(())
        }))
    }
}

impl Continuation {
    fn unit_target(&self,name:&str,arity:usize)->EvalResult<()> {
        if !self.library.has_unit_function(name,arity){return Err(invalid("Unit target, mode or arity differs from its actual declaration"));}
        if self.state.pure||self.state.handler_locals.is_some(){return Err(invalid("Unit live debug requires a synchronous Scene owner"));}
        if self.state.calls>=MAX_FUNCTION_DEPTH{return Err(EvalError::ExecutionLimit{limit:"64 appels de fonction imbriqués"});}
        Ok(())
    }
    pub(super) fn schedule_unit(&mut self,name:String,arguments:Vec<Expr>,ctx:Context)->EvalResult<()> {
        // Exactly the normal Unit admission/tick order, before any argument RNG.
        self.unit_target(&name,arguments.len())?;self.tick()?;
        self.tasks.push(Task::UnitArguments{name,count:arguments.len(),ctx:ctx.clone()});
        for(index,argument)in arguments.into_iter().enumerate().rev(){
            self.tasks.push(Task::Expr(argument,Context{depth:0,..ctx.child(Route::Argument(index))}));
        }
        Ok(())
    }
    pub(super) fn enter_unit(&mut self,name:String,arguments:Vec<Value>,caller:Context)->EvalResult<()> {
        self.unit_target(&name,arguments.len())?;
        let (parameters,body)=self.library.legacy_definition(&name).cloned().ok_or_else(||invalid("Unit declaration disappeared"))?;
        self.enter_unit_definition1023(FunctionSymbol::legacy(&name),parameters,body,arguments,caller)
    }

    pub(super) fn finish_unit_frame(&mut self,frame:usize,base:usize,caller:Context,exit:BlockExit)->EvalResult<()> {
        if !matches!(exit,BlockExit::Continue|BlockExit::Unit)||frame+1!=self.frames.len()
            ||self.frames.get(frame).is_none_or(|frame|frame.unit_locals.is_none())||caller.frame>=frame {
            return Err(invalid("Unit completion differs from its actual invocation stack"));
        }
        self.frames.truncate(frame);self.state.calls=self.state.calls.checked_sub(1).ok_or_else(||invalid("Unit depth underflow"))?;
        if self.values.len()<base{return Err(invalid("Unit completion lost its caller value stack"));}self.values.truncate(base);
        if let Some(locals)=&self.frames[caller.frame].unit_locals {
            let mut charged=self.state.value_work;
            for(name,value)in &self.state.globals{if !locals.contains(name){
                charged=charged.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;
                if charged>1_000_000{return Err(EvalError::ExecutionLimit{limit:"1 000 000 copies ou caractères de valeurs par calcul"});}
            }}
            self.state.value_work=charged;
            let updates=self.state.globals.iter().filter(|(name,_)|!locals.contains(*name)).map(|(name,value)|(name.clone(),value.clone())).collect::<Vec<_>>();
            self.frames[caller.frame].variables.extend(updates);
        }else if caller.frame==0&&matches!(self.request.root,Root::Unit(..)|Root::LibraryUnit(..)){
            self.unit_exit=Some(exit);
        }else{return Err(invalid("Unit completion has no admitted Unit/Scene caller"));}
        Ok(())
    }
    pub(super) fn return_unit(&mut self,ctx:&Context)->EvalResult<()> {
        if self.frames.get(ctx.frame).is_none_or(|frame|frame.unit_locals.is_none()){return Err(invalid("Unit return outside its actual resultless invocation"));}
        let index=self.tasks.iter().rposition(|task|matches!(task,Task::UnitEnd{frame,..}if *frame==ctx.frame)).ok_or_else(||invalid("Unit return lost its actual invocation end"))?;
        let Task::UnitEnd{frame,base,caller}=self.tasks[index].clone()else{unreachable!()};
        self.cleanup_lexical_tasks(index);self.tasks.truncate(index);
        self.finish_unit_frame(frame,base,caller,BlockExit::Unit)
    }
    pub(super) fn reject_unit_portable(&self)->EvalResult<()> {
        if !self.library.library_metadata.is_empty()||!self.library.unit_functions.is_empty()||matches!(self.request.root,Root::Unit(..)|Root::LibraryUnit(..))||self.unit_exit.is_some()
            ||!self.state.unit_writes.is_empty()||self.frames.iter().any(|frame|frame.unit_locals.is_some()) {
            return Err(invalid("Live Unit roots/frames have no portable export or cold-import contract"));
        }Ok(())
    }
}

#[cfg(test)]
#[path="eval_unit_live_debug_tests1021.rs"]
mod tests;

#[cfg(test)]
#[path="eval_unit_macro_live_interop_tests1021.rs"]
mod macro_interop1021;
