//! Typed Library calls enter the existing continuation frame/task/value stack.
use super::*;
use rvn_parser::function_library_asset1023::QualifiedLibraryFunction;
use rvn_parser::function_library_program1023::{QualifiedLibraryCall,LibraryCallMode};
impl Computation<'_>{
    pub(in super::super) fn debugger_library_call1023(&mut self,symbol:&FunctionSymbol,arguments:&[Value])->Option<EvalResult<Value>>{
        let FunctionSymbol::Library(reference)=symbol else{return Some(Err(invalid("Qualified debugger root requires Library symbol")));};
        let result=self.debugger_root(RootRef::LibraryCall(reference,arguments),&self.globals)?;
        Some(result.and_then(|result|{self.install_debugger_result(&result);if let Some(error)=result.error{return Err(error);}result.value.ok_or_else(||invalid("Qualified debugger value missing"))}))
    }
    pub(in super::super) fn debugger_library_unit1023(&mut self,call:&QualifiedLibraryCall,environment:&HashMap<String,Value>)->Option<EvalResult<()>>{
        // Same original frame, before arguments; no UI-supplied caller identity.
        if let Err(error)=self.library.resolve_library_call(call,LibraryCallMode::Completion,self.current_function1023.as_ref()){return Some(Err(error));}
        let result=self.debugger_root(RootRef::LibraryUnit(call),environment)?;
        Some(result.and_then(|result|{if let Some(error)=&result.error{return Err(error.clone());}if result.value.is_some()||!matches!(&result.unit_exit,Some(BlockExit::Continue|BlockExit::Unit)){return Err(invalid("Qualified Unit root did not complete"));}self.install_debugger_result(&result);Ok(())}))
    }
}
impl Continuation{
    pub(super) fn schedule_library_value1023(&mut self,call:QualifiedLibraryCall,ctx:Context)->EvalResult<()>{
        let frame=self.frames.get(ctx.frame).ok_or_else(||invalid("Library call actual frame absent"))?;
        let resolved=self.library.resolve_library_call(&call,LibraryCallMode::Value,frame.function1023.as_ref())?;
        let args=resolved.arguments.into_iter().cloned().collect::<Vec<_>>();
        self.tasks.push(Task::LibraryCall{reference:call.reference().clone(),count:args.len(),ctx:ctx.clone()});
        for(index,arg)in args.into_iter().enumerate().rev(){self.tasks.push(Task::Expr(arg,ctx.expression(Route::Argument(index))));}Ok(())
    }
    pub(super) fn enter_library_value1023(&mut self,reference:QualifiedLibraryFunction,arguments:Vec<Value>,ctx:Context)->EvalResult<()>{
        let frame=self.frames.get(ctx.frame).ok_or_else(||invalid("Library caller actual frame absent"))?;
        let symbol=self.library.resolve_library_symbol1023(&reference,LibraryCallMode::Value,frame.function1023.as_ref())?;
        let(parameters,body)=self.library.definitions.get(&symbol).cloned().ok_or_else(||invalid("Library canonical definition absent"))?;
        self.enter_value_definition1023(symbol,parameters,body,arguments,ctx)
    }
    pub(super) fn enter_value_definition1023(&mut self,symbol:FunctionSymbol,parameters:Vec<String>,body:Vec<Statement>,arguments:Vec<Value>,ctx:Context)->EvalResult<()>{
        let name=symbol.scope_key();
        rvn_parser::scheduled_source::native_loop_contract::validate_authored_bindings(&body).map_err(EvalError::InvalidFunction)?;
        if parameters.len()!=arguments.len(){return Err(invalid(&format!("{name} attend {} argument(s), reçu {}",parameters.len(),arguments.len())));}
        if self.state.calls>=MAX_FUNCTION_DEPTH{return Err(EvalError::ExecutionLimit{limit:"64 appels de fonction imbriqués"});}
        for work in self.state.globals.values().map(crate::value_limits::value_work).collect::<EvalResult<Vec<_>>>()?{
            self.state.value_work=self.state.value_work.checked_add(work).ok_or(EvalError::NumericOverflow)?;
            if self.state.value_work>1_000_000{return Err(EvalError::ExecutionLimit{limit:"1 000 000 copies ou caractères de valeurs par calcul"});}
        }
        let mut variables=self.state.globals.clone();variables.extend(parameters.into_iter().zip(arguments));
        self.state.calls+=1;let frame=self.frames.len();self.frames.push(Frame{function1023:Some(symbol),unit_locals:None,function:Some(name.clone()),variables,depth:self.state.calls,current_site:None,
            consumer_frame:Default::default(),consumer_storage:Default::default(),active_consumer:None});
        self.tasks.push(Task::FunctionEnd{frame,base:self.values.len(),name});
        self.tasks.push(Task::Block(body,0,Context{frame,route:vec![],depth:0}));Ok(())
    }
}

impl Continuation{
    pub(super) fn schedule_library_unit1023(&mut self,call:QualifiedLibraryCall,ctx:Context)->EvalResult<()>{
        let frame=self.frames.get(ctx.frame).ok_or_else(||invalid("Library Unit caller actual frame absent"))?;
        let resolved=self.library.resolve_library_call(&call,LibraryCallMode::Completion,frame.function1023.as_ref())?;
        let args=resolved.arguments.into_iter().cloned().collect::<Vec<_>>();
        self.unit_symbol_target1023(&resolved.symbol,args.len())?;self.tick()?;
        self.tasks.push(Task::LibraryUnitArguments{reference:call.reference().clone(),count:args.len(),ctx:ctx.clone()});
        for(index,arg)in args.into_iter().enumerate().rev(){self.tasks.push(Task::Expr(arg,Context{depth:0,..ctx.child(Route::Argument(index))}));}Ok(())
    }
    fn unit_symbol_target1023(&self,symbol:&FunctionSymbol,arity:usize)->EvalResult<()>{
        if !self.library.unit_functions.contains(symbol)||self.library.definitions.get(symbol).is_none_or(|(parameters,_)|parameters.len()!=arity){return Err(invalid("Unit actual symbol/mode/arity differs"));}
        if self.state.pure||self.state.handler_locals.is_some(){return Err(invalid("Unit live debug requires an admitted synchronous Scene owner"));}
        if self.state.calls>=MAX_FUNCTION_DEPTH{return Err(EvalError::ExecutionLimit{limit:"64 appels de fonction imbriqués"});}Ok(())
    }
    pub(super) fn enter_library_unit1023(&mut self,reference:QualifiedLibraryFunction,arguments:Vec<Value>,ctx:Context)->EvalResult<()>{
        let frame=self.frames.get(ctx.frame).ok_or_else(||invalid("Library Unit caller actual frame absent"))?;
        let symbol=self.library.resolve_library_symbol1023(&reference,LibraryCallMode::Completion,frame.function1023.as_ref())?;
        self.unit_symbol_target1023(&symbol,arguments.len())?;
        let(parameters,body)=self.library.definitions.get(&symbol).cloned().ok_or_else(||invalid("Unit canonical definition absent"))?;
        self.enter_unit_definition1023(symbol,parameters,body,arguments,ctx)
    }
    pub(super) fn enter_unit_definition1023(&mut self,symbol:FunctionSymbol,parameters:Vec<String>,body:Vec<Statement>,arguments:Vec<Value>,caller:Context)->EvalResult<()>{
        let name=symbol.scope_key();
        for value in self.state.globals.values(){
            self.state.value_work=self.state.value_work.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;
            if self.state.value_work>1_000_000{return Err(EvalError::ExecutionLimit{limit:"1 000 000 copies ou caractères de valeurs par calcul"});}
        }
        let mut variables=self.state.globals.clone();variables.extend(parameters.iter().cloned().zip(arguments));
        self.state.calls+=1;let frame=self.frames.len();
        self.frames.push(Frame{function1023:Some(symbol),function:Some(name),variables,depth:self.state.calls,current_site:None,
            unit_locals:Some(parameters.into_iter().collect()),consumer_frame:Default::default(),consumer_storage:Default::default(),active_consumer:None});
        self.tasks.push(Task::UnitEnd{frame,base:self.values.len(),caller});
        self.tasks.push(Task::Block(body,0,Context{frame,route:vec![],depth:0}));Ok(())
    }
}
