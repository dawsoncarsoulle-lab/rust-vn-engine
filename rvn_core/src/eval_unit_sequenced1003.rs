//! Explicit unit-result calls share Computation, its budgets and candidate RNG.
//! Computation publishes no state; the actual Engine adapter owns its candidate.
//! Actual Scene live roots reuse the debugger; Widget/persistent/Save remain unadmitted.
use super::*;

#[derive(Clone,Debug,PartialEq)]
pub(super) enum BlockExit { Continue, Value(Value), Unit }
impl BlockExit {
    pub(super) fn from_legacy(value:Option<Value>)->Self {
        match value {Some(value)=>Self::Value(value),None=>Self::Continue}
    }
}

pub(super) fn validate_body(body:&[Statement],depth:usize)->EvalResult<()> {
    if depth>128{return Err(EvalError::ExecutionLimit{limit:"128 blocs de contrôle imbriqués"});}
    for s in body {match s {
        Statement::SetVar{name,..} if rvn_parser::is_variable_binding_name(name)=>{},
        Statement::LocalVar{name,..} if rvn_parser::is_variable_binding_name(name)=>{},
        Statement::UnitReturn|Statement::UnitCall{..}|Statement::LibraryUnitCall{..}|Statement::LoopBreak=>{},
        Statement::If{then_branch,else_branch,..}=>{validate_body(then_branch,depth+1)?;validate_body(else_branch,depth+1)?;},
        Statement::Sequence{branches}=>for body in branches{validate_body(body,depth+1)?;},
        Statement::While{body,..}=>validate_body(body,depth+1)?,
        Statement::ForEach{name,body,..}|Statement::ForLoopWithBreak{name,body,..}
            if rvn_parser::is_variable_binding_name(name)=>validate_body(body,depth+1)?,
        _=>return Err(EvalError::InvalidFunction("Unit function currently admits synchronous variable effects only; no value return, latent, scheduled, renderer or event authority".into())),
    }}Ok(())
}
fn calls(body:&[Statement],visit:&mut impl FnMut(&str,usize)->EvalResult<()>)->EvalResult<()> {
    for s in body {match s {
        Statement::UnitCall{name,arguments}=>visit(name,arguments.len())?,
        Statement::If{then_branch,else_branch,..}=>{calls(then_branch,visit)?;calls(else_branch,visit)?;},
        Statement::Sequence{branches}=>for body in branches{calls(body,visit)?;},
        Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>calls(body,visit)?,
        _=>{}
    }}Ok(())
}
pub(super) fn contains_unit_runtime(body:&[Statement])->bool {
    let mut pending=vec![body];
    while let Some(body)=pending.pop(){for s in body {match s {
        Statement::UnitCall{..}|Statement::LibraryUnitCall{..}|Statement::UnitReturn|Statement::UnitFunction{..}=>return true,
        Statement::FunctionLibrary{scope}=>for member in scope.members(){match member.declaration(){Statement::UnitFunction{..}=>return true,Statement::Function{body,..}=>pending.push(body),_=>{}}},
        Statement::If{then_branch,else_branch,..}=>{pending.push(then_branch);pending.push(else_branch);},
        Statement::Sequence{branches}=>pending.extend(branches.iter().map(Vec::as_slice)),
        Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}|Statement::ScheduledConsumer{body,..}|Statement::Init{body}|Statement::Function{body,..}|Statement::Screen{body,..}|Statement::Handler{body,..}=>pending.push(body),
        Statement::StandardMacroDispatch{dispatch}=>pending.extend(dispatch.branches.iter().map(|branch|branch.body.as_slice())),
        Statement::Choice{options}=>pending.extend(options.iter().map(|o|o.body.as_slice())),
        Statement::Imagemap{hotspots,..}=>pending.extend(hotspots.iter().map(|h|h.body.as_slice())),
        _=>{},
    }}}false
}

impl FunctionLibrary {
    pub(crate) fn reject_unadmitted_unit_owner(script:&[Statement])->EvalResult<()> {
        if contains_unit_runtime(script){return Err(EvalError::InvalidFunction("Unit Engine/Scene/Init/debug/save source owner is not connected in this private layer".into()));}
        Ok(())
    }
    pub(crate) fn contains_unit_source(script:&[Statement])->bool { contains_unit_runtime(script) }
    pub(crate) fn validate_unit_definition_body(body:&[Statement])->EvalResult<()> {
        crate::flow_control::validate(body,false,0)?; validate_body(body,0)
    }
    pub(crate) fn reject_unit_portable_owner(&self,script:&[Statement])->EvalResult<()> {
        if !self.unit_functions.is_empty(){return Err(EvalError::InvalidFunction("Unit debug/Save activation and source identity are not yet admitted".into()));}
        Self::reject_unadmitted_unit_owner(script)
    }
    /// Admit only synchronous statement calls owned by the actual Engine.
    /// Lexical definitions stay in the one FunctionLibrary; Host/producer
    /// contexts never acquire statement-call authority through this walk.
    pub(crate) fn validate_engine_unit_owner(&self,script:&[Statement])->EvalResult<()> {
        if !contains_unit_runtime(script) {
            if self.unit_functions.is_empty(){return Ok(());}
            return Err(EvalError::InvalidFunction("Unit declarations disappeared from their actual Engine source".into()));
        }
        let declarations=script.iter().filter_map(|s|match s {
            Statement::Function{name,parameters,body}|Statement::UnitFunction{name,parameters,body}=>Some((FunctionSymbol::legacy(name),(parameters.clone(),body.clone()))),_=>None,
        }).collect::<HashMap<_,_>>();
        let units=script.iter().filter_map(|s|if let Statement::UnitFunction{name,..}=s{Some(FunctionSymbol::legacy(name))}else{None}).collect::<std::collections::HashSet<_>>();
        let (declarations,units)=self.extend_library_source_declarations1023(script,declarations,units)?;
        if declarations!=self.definitions || units!=self.unit_functions {
            return Err(EvalError::InvalidFunction("Engine function declarations differ from the admitted lexical library".into()));
        }
        fn walk(library:&FunctionLibrary,body:&[Statement],root:bool,allowed:bool,init:bool,depth:usize)->EvalResult<()> {
            if depth>128{return Err(EvalError::ExecutionLimit{limit:"128 blocs de contrôle imbriqués"});}
            for s in body {match s {
                Statement::UnitFunction{..}|Statement::FunctionLibrary{..} if root=>{},
                Statement::LibraryUnitCall{call}=>{
                    if !allowed{return Err(EvalError::InvalidFunction("Library Unit call has no synchronous Engine root".into()));}
                    library.resolve_library_call(call,rvn_parser::function_library_program1023::LibraryCallMode::Completion,None)?;
                },
                Statement::UnitFunction{..}|Statement::UnitReturn=>return Err(EvalError::InvalidFunction("Unit declaration or return has no actual root/invocation owner".into())),
                Statement::UnitCall{name,arguments}=>{
                    if !allowed || !library.has_unit_function(name,arguments.len()) {
                        return Err(EvalError::InvalidFunction("Unit statement has no admitted synchronous Engine target, arity or scope".into()));
                    }
                },
                Statement::Function{body,..}|Statement::Screen{body,..}|Statement::Handler{body,..}|Statement::ScheduledConsumer{body,..}=>walk(library,body,false,false,init,depth+1)?,
                Statement::StandardMacroDispatch{dispatch}=>for branch in &dispatch.branches{walk(library,&branch.body,false,false,init,depth+1)?;},
                Statement::Init{body}=>walk(library,body,false,allowed,true,depth+1)?,
                Statement::If{then_branch,else_branch,..}=>{walk(library,then_branch,false,allowed,init,depth+1)?;walk(library,else_branch,false,allowed,init,depth+1)?;},
                // These ordinary Init controls currently delegate to the legacy
                // computation wrapper. They have no scoped Unit root adapter.
                Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>walk(library,body,false,allowed&&!init,init,depth+1)?,
                Statement::Sequence{branches}=>for branch in branches{walk(library,branch,false,allowed&&!init,init,depth+1)?;},
                Statement::Choice{options}=>for option in options{walk(library,&option.body,false,allowed&&!init,init,depth+1)?;},
                Statement::Imagemap{hotspots,..}=>for hotspot in hotspots{walk(library,&hotspot.body,false,allowed&&!init,init,depth+1)?;},
                _=>{},
            }}Ok(())
        }
        walk(self,script,true,true,false,0)
    }
    /// Caller expressions see their genuine Scene/Init environment, while
    /// called bodies close over game globals. Both use one Computation/RNG and
    /// one depth/step/value-work budget; publication belongs to the Engine.
    pub(crate) fn invoke_unit_scoped_with_locale(&self,name:&str,arguments:&[Expr],environment:&HashMap<String,Value>,globals:&HashMap<String,Value>,random:&mut RandomState,locale:Option<&crate::locale::LocaleManager>)
        ->EvalResult<(HashMap<String,Value>,std::collections::HashSet<String>)> {
        if !self.has_unit_function(name,arguments.len()){return Err(EvalError::InvalidFunction(format!("Unit function or arity is not admitted: {name}")));}
        let mut computation=Computation::new(self,globals);computation.random=*random;computation.locale=locale;
        if let Some(result)=computation.debugger_unit(name,arguments,environment){result?;}
        else{computation.invoke_unit_expressions(name,arguments,environment)?;}
        *random=computation.random;Ok((computation.globals,computation.unit_global_writes))
    }
    pub fn has_unit_function(&self,name:&str,arity:usize)->bool {
        self.legacy_unit(name)&&self.legacy_definition(name).is_some_and(|(p,_)|p.len()==arity)
    }
    pub(super) fn validate_unit_calls(&self)->EvalResult<()> {
        for name in &self.unit_functions {
            self.validate_library_body_calls1023(&self.definitions[name].1,Some(name))?;
            calls(&self.definitions[name].1,&mut |target,arity| {
                if !self.has_unit_function(target,arity){return Err(EvalError::InvalidFunction(format!("Unit target or arity does not match its explicit declaration: {target}")));}
                Ok(())
            })?;
        }Ok(())
    }
    /// Candidate-only shared executor. A future actual Engine/Scene/Widget
    /// owner must validate/publish the returned variables at its own boundary.
    /// Refusal leaves the caller's source variables and RNG unchanged.
    pub fn invoke_unit_with_random(&self,name:&str,arguments:&[Expr],globals:&HashMap<String,Value>,random:&mut RandomState)
        ->EvalResult<HashMap<String,Value>> {
        if self.debug_probe.is_some(){return Err(EvalError::InvalidFunction("Unit debugger/save activation is not yet source-admitted".into()));}
        let mut computation=Computation::new(self,globals);computation.random=*random;
        computation.invoke_unit_expressions(name,arguments,globals)?;
        *random=computation.random;
        Ok(computation.globals)
    }
}
impl Computation<'_> {
    pub(super) fn invoke_unit_expressions(&mut self,name:&str,arguments:&[Expr],environment:&HashMap<String,Value>)->EvalResult<()> {
        let first = self.enter_unit_frame(name, arguments, environment)?;
        let mut frames = vec![first];
        let result = self.drain_unit_frames(&mut frames);
        // A failed argument, expression or budget restores every lexical
        // caller, while effects remain only in this uncommitted candidate.
        while let Some(frame) = frames.pop() { self.leave_unit_frame(frame); }
        result
    }
    pub(super) fn invoke_library_unit_expressions1023(&mut self,call:&rvn_parser::function_library_program1023::QualifiedLibraryCall,environment:&HashMap<String,Value>)->EvalResult<()>{
        let first=self.enter_library_unit_frame1023(call,environment)?;let mut frames=vec![first];
        let result=self.drain_unit_frames(&mut frames);
        while let Some(frame)=frames.pop(){self.leave_unit_frame(frame);}
        result
    }
    pub(super) fn refresh_unit_globals(&mut self,variables:&mut HashMap<String,Value>)->EvalResult<()> {
        // Account for exactly the return copies before materializing any of
        // them. Masked parameters/locals retain their own binding and do not
        // receive (or pay for) a candidate-global clone.
        let mut charged=self.value_work;
        for (name,value) in &self.globals {
            if !self.unit_locals.as_ref().is_some_and(|locals|locals.contains(name)) {
                charged=charged.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;
                if charged>1_000_000{return Err(EvalError::ExecutionLimit{limit:"1 000 000 copies ou caractères de valeurs par calcul"});}
            }
        }
        self.value_work=charged;
        for (name,value) in &self.globals {
            if !self.unit_locals.as_ref().is_some_and(|locals|locals.contains(name)){variables.insert(name.clone(),value.clone());}
        }
        Ok(())
    }

    pub(super) fn range_outcome(&mut self,body:&[Statement],variables:&mut HashMap<String,Value>)->EvalResult<BlockExit>{
        self.range_break_owners.push(ComputationRangeBreak::Legacy);
        let result=self.block_outcome(body,variables);
        let owner=self.range_break_owners.pop();debug_assert!(matches!(owner,Some(ComputationRangeBreak::Legacy)));
        result
    }
}

// Only Unit definitions use these transient heap frames. Expression/value
// calls, scalar leaves and shared budgets remain owned by Computation.
struct UnitCaller {
    function1023: Option<FunctionSymbol>,
    locals: Option<std::collections::HashSet<String>>,
    frame: crate::consumer_schedule::ConsumerFrame,
    storage: crate::consumer_schedule::FrameStorage,
    consumer: Option<crate::consumer_schedule::ActiveConsumer>,
    break_owners: Vec<ComputationRangeBreak>,
    range_break: bool,
}
struct UnitFrame<'a> {
    variables: HashMap<String, Value>,
    tasks: Vec<UnitTask<'a>>,
    caller: UnitCaller,
}
enum UnitTask<'a> {
    Block(&'a [Statement], usize),
    Sequence(&'a [Vec<Statement>], usize),
    While(&'a Expr, &'a [Statement]),
    Each(&'a str, std::vec::IntoIter<Value>, &'a [Statement]),
    Range { name: &'a str, first: i64, next: i64, count: i64, body: &'a [Statement] },
    RangeEnd,
}
enum UnitStep<'a> {
    Continue,
    Call(&'a str, &'a [Expr]),
    LibraryCall(&'a rvn_parser::function_library_program1023::QualifiedLibraryCall),
    Done,
}

impl<'a> Computation<'a> {
    fn enter_unit_frame(&mut self, name: &str, arguments: &[Expr], environment: &HashMap<String, Value>) -> EvalResult<UnitFrame<'a>> {
        if !self.library.has_unit_function(name,arguments.len()){return Err(EvalError::InvalidFunction("Legacy Unit target/arity absent".into()));}
        self.enter_unit_definition1023(FunctionSymbol::legacy(name), arguments.iter().collect(), environment)
    }
    fn enter_library_unit_frame1023(&mut self, call: &rvn_parser::function_library_program1023::QualifiedLibraryCall, environment: &HashMap<String, Value>) -> EvalResult<UnitFrame<'a>> {
        let library=self.library;
        let resolved=library.resolve_library_call(call,rvn_parser::function_library_program1023::LibraryCallMode::Completion,self.current_function1023.as_ref())?;
        self.enter_unit_definition1023(resolved.symbol,resolved.arguments,environment)
    }
    fn enter_unit_definition1023(&mut self,symbol:FunctionSymbol,arguments:Vec<&Expr>,environment:&HashMap<String,Value>)->EvalResult<UnitFrame<'a>>{
        let library: &'a FunctionLibrary=self.library;
        let (parameters,body)=library.definitions.get(&symbol).ok_or_else(||EvalError::InvalidFunction("Unit definition disappeared".into()))?;
        if !library.unit_functions.contains(&symbol)||parameters.len()!=arguments.len(){return Err(EvalError::InvalidFunction("Unit function identity/mode/arity is not admitted".into()));}
        if library.debug_probe.is_some()||self.pure_canvas{return Err(EvalError::InvalidFunction("Unit call has no admitted debugger/pure-canvas owner".into()));}
        if self.calls>=MAX_FUNCTION_DEPTH{return Err(EvalError::ExecutionLimit{limit:"64 appels de fonction imbriqués"});}
        self.tick()?;
        let mut values=Vec::new();values.try_reserve(arguments.len()).map_err(|_|EvalError::InvalidFunction("Unit argument allocation".into()))?;
        for expression in arguments{values.push(self.eval(expression,environment,0)?);}
        for value in self.globals.values(){
            self.value_work=self.value_work.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;
            if self.value_work>1_000_000{return Err(EvalError::ExecutionLimit{limit:"1 000 000 copies ou caractères de valeurs par calcul"});}
        }
        let mut variables=self.globals.clone();variables.extend(parameters.iter().cloned().zip(values));
        let caller=UnitCaller{
            function1023:self.current_function1023.replace(symbol),
            locals:self.unit_locals.replace(parameters.iter().cloned().collect()),
            frame:std::mem::take(&mut self.consumer_frame),storage:std::mem::take(&mut self.consumer_storage),consumer:self.active_consumer.take(),
            break_owners:std::mem::take(&mut self.range_break_owners),range_break:std::mem::replace(&mut self.range_break,false),
        };
        self.calls+=1;Ok(UnitFrame{variables,tasks:vec![UnitTask::Block(body,0)],caller})
    }

    fn leave_unit_frame(&mut self, frame: UnitFrame<'_>) {
        debug_assert!(self.calls > 0);
        self.calls -= 1;
        let caller = frame.caller;
        self.current_function1023 = caller.function1023;
        self.unit_locals = caller.locals;
        self.consumer_frame = caller.frame;
        self.consumer_storage = caller.storage;
        self.active_consumer = caller.consumer;
        self.range_break_owners = caller.break_owners;
        self.range_break = caller.range_break;
    }

    fn drain_unit_frames(&mut self, frames: &mut Vec<UnitFrame<'a>>) -> EvalResult<()> {
        while !frames.is_empty() {
            let step = self.step_unit_frame(frames.last_mut().expect("active Unit frame"))?;
            match step {
                UnitStep::Continue => {},
                UnitStep::Call(name, arguments) => {
                    let child = self.enter_unit_frame(name, arguments,
                        &frames.last().expect("Unit caller frame").variables)?;
                    frames.push(child);
                },
                UnitStep::LibraryCall(call) => {
                    let child=self.enter_library_unit_frame1023(call,&frames.last().expect("Unit caller frame").variables)?;
                    frames.push(child);
                },
                UnitStep::Done => {
                    self.leave_unit_frame(frames.pop().expect("completed Unit frame"));
                    if let Some(caller) = frames.last_mut() {
                        self.refresh_unit_globals(&mut caller.variables)?;
                    }
                },
            }
        }
        Ok(())
    }

    fn step_unit_frame(&mut self, frame: &mut UnitFrame<'a>) -> EvalResult<UnitStep<'a>> {
        let Some(task) = frame.tasks.pop() else { return Ok(UnitStep::Done); };
        // A legacy Break crosses If/Sequence/While/ForEach but is consumed
        // only by its actual nearest range owner, exactly as block_outcome.
        if self.range_break && !matches!(&task, UnitTask::RangeEnd) {
            return Ok(UnitStep::Continue);
        }
        match task {
            UnitTask::Block(body, next) => {
                let Some(statement) = body.get(next) else { return Ok(UnitStep::Continue); };
                frame.tasks.push(UnitTask::Block(body, next + 1));
                if matches!(statement, Statement::SetVar { .. } | Statement::LocalVar { .. }) {
                    // Reuse the existing scalar implementation and its one
                    // statement tick; this frame is never retained per call.
                    let exit = self.block_outcome(std::slice::from_ref(statement), &mut frame.variables)?;
                    debug_assert!(matches!(exit, BlockExit::Continue));
                    return Ok(UnitStep::Continue);
                }
                self.tick()?;
                match statement {
                    Statement::UnitReturn => return Ok(UnitStep::Done),
                    Statement::UnitCall { name, arguments } => return Ok(UnitStep::Call(name, arguments)),
                    Statement::LibraryUnitCall { call } => return Ok(UnitStep::LibraryCall(call)),
                    Statement::If { condition, then_branch, else_branch } => {
                        let branch = if truthy_bounded(&self.eval(condition, &frame.variables, 0)?,
                            Some(self.library.text_resources()), &mut self.value_work)? {
                            then_branch
                        } else { else_branch };
                        frame.tasks.push(UnitTask::Block(branch, 0));
                    },
                    Statement::Sequence { branches } => frame.tasks.push(UnitTask::Sequence(branches, 0)),
                    Statement::While { condition, body } => frame.tasks.push(UnitTask::While(condition, body)),
                    Statement::ForEach { name, collection, body } => {
                        let Value::List(items) = self.eval(collection, &frame.variables, 0)? else {
                            return Err(EvalError::InvalidFunction("for attend une liste".into()));
                        };
                        self.unit_locals.as_mut().expect("active Unit locals").insert(name.clone());
                        frame.tasks.push(UnitTask::Each(name, items.into_iter(), body));
                    },
                    Statement::ForLoopWithBreak { name, first, last, body } => {
                        let first = self.eval(first, &frame.variables, 0)?;
                        let last = self.eval(last, &frame.variables, 0)?;
                        let (first, count) = crate::flow_control::bounds(&first, &last)?;
                        self.unit_locals.as_mut().expect("active Unit locals").insert(name.clone());
                        frame.tasks.push(UnitTask::Range { name, first, next: 0, count, body });
                    },
                    Statement::LoopBreak => self.request_computation_range_break(),
                    _ => return Err(EvalError::InvalidFunction("Statement has no admitted synchronous Unit body owner".into())),
                }
            },
            UnitTask::Sequence(branches, next) => {
                if let Some(branch) = branches.get(next) {
                    frame.tasks.push(UnitTask::Sequence(branches, next + 1));
                    frame.tasks.push(UnitTask::Block(branch, 0));
                }
            },
            UnitTask::While(condition, body) => {
                if truthy_bounded(&self.eval(condition, &frame.variables, 0)?,
                    Some(self.library.text_resources()), &mut self.value_work)? {
                    self.tick()?;
                    frame.tasks.push(UnitTask::While(condition, body));
                    frame.tasks.push(UnitTask::Block(body, 0));
                }
            },
            UnitTask::Each(name, mut items, body) => {
                if let Some(item) = items.next() {
                    self.tick()?;
                    frame.variables.insert(name.into(), item);
                    frame.tasks.push(UnitTask::Each(name, items, body));
                    frame.tasks.push(UnitTask::Block(body, 0));
                }
            },
            UnitTask::Range { name, first, next, count, body } => {
                if next < count {
                    self.tick()?;
                    let value = first.checked_add(next).ok_or(EvalError::NumericOverflow)?;
                    frame.variables.insert(name.into(), Value::Int(value));
                    frame.tasks.push(UnitTask::Range { name, first, next: next + 1, count, body });
                    frame.tasks.push(UnitTask::RangeEnd);
                    self.range_break_owners.push(ComputationRangeBreak::Legacy);
                    frame.tasks.push(UnitTask::Block(body, 0));
                }
            },
            UnitTask::RangeEnd => {
                let owner = self.range_break_owners.pop();
                debug_assert!(matches!(owner, Some(ComputationRangeBreak::Legacy)));
                if self.range_break {
                    self.range_break = false;
                    let next = frame.tasks.pop();
                    debug_assert!(matches!(next, Some(UnitTask::Range { .. })));
                }
            },
        }
        Ok(UnitStep::Continue)
    }
}

#[cfg(test)]
#[path="eval_unit_sequenced_tests1003.rs"]
mod tests;

#[cfg(test)]
#[path="eval_unit_macro_interop_tests1021.rs"]
mod unit_macro_interop1021;
