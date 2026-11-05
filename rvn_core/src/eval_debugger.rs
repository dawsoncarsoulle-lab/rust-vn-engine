//! A bounded continuation for the existing computation semantics. This child
//! reuses builtin/operator/UI construction; it never executes a renderer.
#[cfg(test)]
#[path="eval_debugger_tests.rs"]
mod tests;
#[cfg(test)]
#[path="eval_native_narrative_receipt_tests.rs"]
mod narrative_receipt_tests;
use super::*;
use crate::editor_debugger::{self as wire, Route, Site};
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
#[path="eval_debugger_portable.rs"]
mod portable;
#[path="eval_debugger_standard_macros_1020.rs"]
mod standard_macros_1020;
#[path="eval_debugger_native_phase.rs"]
mod native_phase;
#[path="eval_debugger_loops.rs"]
mod loops;
#[path="eval_debugger_screen.rs"]
mod screen_construction;
#[path="eval_debugger_unit1003.rs"]
mod unit_live;
#[path="eval_debugger_function_libraries1023.rs"]
mod function_libraries1023;
use super::unit_sequenced::BlockExit;
use loops::{LexicalLoopState,LoopPhase};
pub(crate) use portable::{PortableTrace,PortableRecord};
#[path="eval_widget_reference_values_0974.rs"]
mod widget_values;

#[derive(Clone, Debug, PartialEq)]
enum Root { Expression(Expr, usize), Block(Vec<Statement>), Call(String, Vec<Value>), Consumer(Statement,String), Unit(String,Vec<Expr>), LibraryCall(rvn_parser::function_library_asset1023::QualifiedLibraryFunction,Vec<Value>), LibraryUnit(rvn_parser::function_library_program1023::QualifiedLibraryCall) }
#[derive(Debug)]
enum RootRef<'a>{Expression(&'a Expr,usize),Block(&'a [Statement]),Call(&'a str,&'a [Value]),Consumer(&'a Statement,&'a str),Unit(&'a str,&'a [Expr]),LibraryCall(&'a rvn_parser::function_library_asset1023::QualifiedLibraryFunction,&'a [Value]),LibraryUnit(&'a rvn_parser::function_library_program1023::QualifiedLibraryCall)}
impl RootRef<'_>{
    fn matches(&self,root:&Root)->bool{match(self,root){(Self::Expression(a,d),Root::Expression(b,e))=>*a==b&&d==e,(Self::Block(a),Root::Block(b))=>*a==b.as_slice(),(Self::Call(a,args),Root::Call(b,values))=>*a==b&&*args==values.as_slice(),(Self::Consumer(a,s),Root::Consumer(b,t))=>*a==b&&*s==t,(Self::Unit(a,args),Root::Unit(b,values))=>*a==b&&*args==values.as_slice(),(Self::LibraryCall(a,args),Root::LibraryCall(b,values))=>*a==b&&*args==values.as_slice(),(Self::LibraryUnit(a),Root::LibraryUnit(b))=>*a==b,_=>false}}
    fn owned(&self)->Root{match self{Self::Expression(expression,depth)=>Root::Expression((*expression).clone(),*depth),Self::Block(body)=>Root::Block(body.to_vec()),Self::Call(name,args)=>Root::Call((*name).into(),args.to_vec()),Self::Consumer(statement,scope)=>Root::Consumer((*statement).clone(),(*scope).into()),Self::Unit(name,args)=>Root::Unit((*name).into(),args.to_vec()),Self::LibraryCall(reference,args)=>Root::LibraryCall((*reference).clone(),args.to_vec()),Self::LibraryUnit(call)=>Root::LibraryUnit((*call).clone())}}
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    // Process-local true caller frame; portable Library continuations are gated.
    #[serde(skip)] function1023:Option<FunctionSymbol>,
    #[serde(skip)] unit_writes:HashSet<String>,
    globals: HashMap<String, Value>, random: RandomState, steps: usize,
    value_work: usize, calls: usize, handler_locals: Option<HashSet<String>>,
    #[serde(with="portable::commands")] commands: Vec<UiCommand>, pure: bool, range_break: bool,
    #[serde(skip)] locale: Option<crate::locale::LocaleManager>,
    #[serde(skip,default="portable::empty_revision")] text_resources: crate::string_table::StringTableRevision,
    // Process-local context only; Save15 validates the current loader receipt before rebind.
    #[serde(skip)] widget_classes: Option<crate::widget_class_source_0950::WidgetClassCatalogReceipt>,
    consumer_frame: crate::consumer_schedule::ConsumerFrame,
    #[serde(default,skip_serializing_if="Option::is_none")]
    widget_macro_frame:Option<crate::widget_class_source_0950::standard_macros::WidgetMacroFrame>,
    active_consumer: Option<ConsumerContext>,
}

#[derive(Clone,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsumerContext {
    scope:String,
    captures:BTreeMap<String,Value>,
    inspectors:std::collections::BTreeSet<String>,
}
impl ConsumerContext {
    fn from_active(active:&crate::consumer_schedule::ActiveConsumer)->Self {
        Self {scope:active.scope.clone(),captures:active.captures.clone(),inspectors:active.inspectors.iter().cloned().collect()}
    }
    fn into_active(self)->crate::consumer_schedule::ActiveConsumer {
        crate::consumer_schedule::ActiveConsumer{scope:self.scope,captures:self.captures,inspectors:self.inspectors.into_iter().collect()}
    }
}
#[derive(Clone, PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Request { #[serde(skip,default="portable::empty_root")] root: Root, variables: HashMap<String, Value>, state: State, scope:Option<String>,#[serde(default,skip_serializing_if="Vec::is_empty")] route_prefix:Vec<Route>,#[serde(skip)] screen:Option<Arc<screen_construction::ConstructionWitness>> }
#[derive(Clone, Debug,PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultState { #[serde(skip)] unit_exit:Option<BlockExit>, value: Option<Value>, variables: HashMap<String, Value>, state: State, #[serde(skip)] error: Option<EvalError> }
#[derive(Clone, Debug)]
struct Record { request: Request, result: ResultState }

/// The caller replays only transaction preparation against the same immutable
/// Engine anchor. Completed computations are consumed from this exact trace;
/// functions and RNG draws are not re-executed when another root suspends.
#[derive(Default, Clone)]
pub(crate) struct Probe {
    cursor: usize, records: Vec<Record>, requested: Option<Request>,
    requested_objects: Option<Arc<crate::widget_reference_values_0974::WidgetValueAdmission>>,
    stale: bool, scope:Option<String>, bytes:usize, sealed:bool,route_prefix:Vec<Route>,
    screen_operation:Option<Arc<crate::engine::DebugStateSource>>,screen_active:Option<Arc<screen_construction::ConstructionWitness>>,
}
pub(crate) type SharedProbe = Arc<Mutex<Probe>>;
impl Probe {
    /// Only an admitted owner forks a borrowed bounded trace. The candidate
    /// receives a new Arc; preparing/refusing a step cannot mutate live tasks.
    pub(crate) fn fork(probe:&SharedProbe)->EvalResult<SharedProbe>{
        let probe=probe.lock().map_err(|_|invalid("Debugger computation lock unavailable"))?;
        measured(&format_args!("{probe:?}"))?;
        Ok(Arc::new(Mutex::new(probe.clone())))
    }
    /// Issued by the admitted initializer owner, never by a client DTO.
    pub(crate) fn initializer_route<T>(probe:&SharedProbe,route:Vec<Route>,operation:impl FnOnce()->T)->EvalResult<T>{
        if route.len()>128{return Err(invalid("Initializer source route budget"));}
        struct Guard(SharedProbe,Vec<Route>);impl Drop for Guard{fn drop(&mut self){self.0.lock().unwrap_or_else(|p|p.into_inner()).route_prefix=std::mem::take(&mut self.1);}}
        let previous={let mut p=probe.lock().map_err(|_|invalid("Initializer trace lock unavailable"))?;std::mem::replace(&mut p.route_prefix,route)};
        let _guard=Guard(probe.clone(),previous);Ok(operation())
    }
    pub(crate) fn shared() -> SharedProbe { Arc::new(Mutex::new(Self::default())) }
    pub(crate) fn rewind(probe: &SharedProbe) -> EvalResult<()> {
        let mut probe = probe.lock().map_err(|_| invalid("Debugger computation lock unavailable"))?;
        probe.cursor = 0; probe.requested = None; probe.requested_objects=None; probe.stale = false; probe.screen_active=None; Ok(())
    }
    pub(crate) fn seal(probe:&SharedProbe)->EvalResult<()>{
        if !Self::complete(probe){return Err(invalid("Debugger trace cannot be sealed before preparation completes"));}
        probe.lock().map_err(|_|invalid("Debugger computation lock unavailable"))?.sealed=true;Ok(())
    }
    pub(crate) fn pending(probe: &SharedProbe, library: &FunctionLibrary) -> EvalResult<Option<Continuation>> {
        let probe = probe.lock().map_err(|_| invalid("Debugger computation lock unavailable"))?;
        if probe.stale { return Err(invalid("Debugger computation trace changed")); }
        let Some(request) = &probe.requested else { return Ok(None); };
        let mut library = library.clone(); library.debug_probe = None;
        let mut continuation=Continuation::new(library, request.clone())?;
        continuation.widget_objects=probe.requested_objects.clone();
        Ok(Some(continuation))
    }
    pub(crate) fn finish(probe: &SharedProbe, continuation: &Continuation) -> EvalResult<()> {
        let mut probe = probe.lock().map_err(|_| invalid("Debugger computation lock unavailable"))?;
        if probe.requested.as_ref() != Some(&continuation.request) || !continuation.done {
            return Err(invalid("Debugger computation completion is not the requested root"));
        }
        let cost=measured(&format_args!("{:?}{:?}{:?}{:?}",continuation.result,continuation.frames[0].variables,continuation.state,continuation.error))?;
        probe.bytes=probe.bytes.checked_add(cost).filter(|bytes|*bytes<=TRACE_BYTES).ok_or_else(||invalid("Debugger trace exceeds its bounded observation"))?;
        let mut state=continuation.state.clone();state.calls=continuation.request.state.calls;
        let result=ResultState { unit_exit:continuation.unit_exit.clone(),value:continuation.result.clone(),variables:continuation.frames[0].variables.clone(),
            state,error:continuation.error.clone() };
        probe.records.push(Record { request: continuation.request.clone(), result });
        probe.requested = None; probe.requested_objects=None; Ok(())
    }
    /// A real operation boundary with no evaluator root or completed records.
    /// This does not manufacture a pending computation for atomic clock work.
    pub(crate) fn empty_complete(probe: &SharedProbe) -> bool {
        probe.lock().is_ok_and(|p| !p.stale && p.requested.is_none()
            && p.records.is_empty() && p.cursor == 0)
    }
    pub(crate) fn complete(probe: &SharedProbe) -> bool {
        probe.lock().is_ok_and(|probe| !probe.stale && probe.requested.is_none() && probe.cursor == probe.records.len())
    }
}
fn invalid(message: &str) -> EvalError { EvalError::InvalidFunction(message.into()) }
const TRACE_BYTES:usize=16*1024*1024;
struct Measure(usize);
impl std::fmt::Write for Measure{fn write_str(&mut self,text:&str)->std::fmt::Result{self.0=self.0.checked_add(text.len()).filter(|bytes|*bytes<=TRACE_BYTES).ok_or(std::fmt::Error)?;Ok(())}}
fn measured(args:&std::fmt::Arguments<'_>)->EvalResult<usize>{let mut counter=Measure(0);std::fmt::write(&mut counter,*args).map_err(|_|invalid("Debugger computation exceeds its bounded observation"))?;Ok(counter.0)}

impl FunctionLibrary {
    // The real UI library supplies the declaration kind/name; anonymous public
    // computation remains anonymous. User binding names cannot contain ':'.
    pub(crate) fn debugger_scope<T>(&self,scope:String,operation:impl FnOnce()->EvalResult<T>)->EvalResult<T>{
        let Some(probe)=&self.debug_probe else{return operation();};
        struct Scope(SharedProbe,Option<String>,Vec<Route>);
        impl Drop for Scope{fn drop(&mut self){let mut probe=self.0.lock().unwrap_or_else(|poison|poison.into_inner());probe.scope=self.1.take();probe.route_prefix=std::mem::take(&mut self.2);}}
        let(previous,route)={let mut probe=probe.lock().map_err(|_|invalid("Debugger computation lock unavailable"))?;(probe.scope.replace(scope),std::mem::take(&mut probe.route_prefix))};
        let _guard=Scope(Arc::clone(probe),previous,route);operation()
    }
}

impl Computation<'_> {
    /// Called around the actual one-shot input evaluator. The request is
    /// captured before evaluation; no completed value can manufacture it.
    pub(super) fn retain_native_consumer_input(&mut self,statement:&Statement,scope:&str,variables:&mut HashMap<String,Value>,evaluate:impl FnOnce(&mut Self,&mut HashMap<String,Value>)->EvalResult<Value>)->EvalResult<(Value,PortableRecord)>{
        rvn_parser::scheduled_source::native_loop_contract::binding(statement).map_err(EvalError::InvalidFunction)?.ok_or_else(||invalid("Retained Engine input requires its actual Native loop header"))?;
        let (owner_scope,route_prefix)=match &self.library.debug_probe {
            Some(probe)=>{let probe=probe.lock().map_err(|_|invalid("Native input receipt cannot read its actual trace"))?;(probe.scope.clone(),probe.route_prefix.clone())},
            None=>(None,Vec::new()),
        };
        let request=Request{root:Root::Consumer(statement.clone(),scope.into()),variables:variables.clone(),state:self.debugger_state(),scope:owner_scope,route_prefix,screen:None};
        measured(&format_args!("{request:?}"))?;
        let value=evaluate(self,variables)?;
        let result=ResultState{unit_exit:None,value:Some(value.clone()),variables:variables.clone(),state:self.debugger_state(),error:None};
        let record=PortableRecord::from_completed_native_input(request,result)?;
        Ok((value,record))
    }

    fn debugger_state(&self) -> State {
        State { function1023:self.current_function1023.clone(), unit_writes:self.unit_global_writes.clone(),globals:self.globals.clone(), random:self.random, steps:self.steps,
            value_work:self.value_work, calls:self.calls, handler_locals:self.handler_locals.clone(),
            commands:self.ui_commands.clone(), pure:self.pure_canvas, range_break:self.range_break, locale:self.locale.cloned(),
            text_resources:self.library.text_resources().revision_stamp(),
            widget_classes:self.library.widget_classes.receipt().cloned(),
            widget_macro_frame:self.widget_macro_frame.clone(),consumer_frame:self.consumer_frame.clone(),active_consumer:self.active_consumer.as_ref().map(ConsumerContext::from_active) }
    }
    fn debugger_root(&self, root: RootRef<'_>, variables: &HashMap<String,Value>) -> Option<EvalResult<ResultState>> {
        let shared = self.library.debug_probe.as_ref()?;
        let mut probe = match shared.lock() { Ok(probe)=>probe, Err(_)=>return Some(Err(invalid("Debugger computation lock unavailable"))) };
        if let Some(record) = probe.records.get(probe.cursor) {
            let state=&record.request.state;
            if record.request.screen!=probe.screen_active||!root.matches(&record.request.root)||record.request.variables!=*variables||record.request.scope!=probe.scope||record.request.route_prefix!=probe.route_prefix
                ||state.globals!=self.globals||state.random!=self.random||state.steps!=self.steps||state.value_work!=self.value_work
                ||state.function1023!=self.current_function1023||state.unit_writes!=self.unit_global_writes||state.calls!=self.calls||state.handler_locals!=self.handler_locals||state.commands!=self.ui_commands||state.pure!=self.pure_canvas||state.range_break!=self.range_break||state.locale.as_ref()!=self.locale
                ||state.text_resources!=self.library.text_resources().revision_stamp()
                ||state.widget_classes.as_ref()!=self.library.widget_classes.receipt()
                ||state.widget_macro_frame!=self.widget_macro_frame||state.consumer_frame!=self.consumer_frame||state.active_consumer!=self.active_consumer.as_ref().map(ConsumerContext::from_active) {
                probe.stale=true; return Some(Err(invalid("Debugger computation trace changed"))); }
            // A retained class datum is revalidated against this caller's real
            // catalog. No catalog/instance authority is serialized in the trace.
            let checked = (|| -> Result<(), String> {
                portable::standard_macros_1020::validate_transition(self.library,&record.request.state,&record.result.state).map_err(|e|e.to_string())?;
                let mut visit = |value: &Value| crate::widget_class_getter_0981::validate_class_values(&self.library.widget_classes, value);
                widget_values::request_value_roots(&record.request, &mut visit)?;
                widget_values::state_value_roots(&record.result.state, &mut visit)?;
                for value in record.result.variables.values() { visit(value)?; }
                if let Some(value) = &record.result.value { visit(value)?; }
                Ok(())
            })();
            if let Err(message) = checked {
                probe.stale = true;
                return Some(Err(EvalError::InvalidFunction(message)));
            }
            let result=record.result.clone(); probe.cursor+=1; return Some(Ok(result));
        }
        if probe.sealed{probe.stale=true;return Some(Err(invalid("Sealed debugger trace requested an unprepared computation")));}
        let active=self.active_consumer.as_ref().map(|active|(&active.scope,&active.captures,&active.inspectors));
        let cost=match measured(&format_args!("{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}",root,variables,self.globals,self.handler_locals,self.ui_commands,probe.scope,self.locale,(&self.consumer_frame,&self.widget_macro_frame),active)){Ok(cost)=>cost,Err(error)=>return Some(Err(error))};
        let Some(bytes)=probe.bytes.checked_add(cost).filter(|bytes|*bytes<=TRACE_BYTES)else{return Some(Err(invalid("Debugger trace exceeds its bounded observation")));};
        probe.bytes=bytes;
        let request = Request { root:root.owned(), variables:variables.clone(), state:self.debugger_state(),scope:probe.scope.clone(),route_prefix:probe.route_prefix.clone(),screen:probe.screen_active.clone() };
        if request.screen.as_ref().is_some_and(|witness|!witness.matches(&request)){probe.stale=true;return Some(Err(invalid("Screen description differs from actual construction request")));}
        probe.requested_objects=self.widget_objects.map(|admission|Arc::new(admission.clone()));
        probe.requested=Some(request);
        Some(Err(invalid("Debugger computation suspended")))
    }
    fn install_debugger_result(&mut self, result: &ResultState) {
        self.current_function1023=result.state.function1023.clone();
        self.unit_global_writes=result.state.unit_writes.clone();
        self.globals=result.state.globals.clone(); self.random=result.state.random;
        self.steps=result.state.steps; self.value_work=result.state.value_work;
        self.calls=result.state.calls; self.handler_locals=result.state.handler_locals.clone();
        self.ui_commands=result.state.commands.clone(); self.range_break=result.state.range_break;
        self.widget_macro_frame=result.state.widget_macro_frame.clone();
        self.consumer_frame=result.state.consumer_frame.clone();
        self.consumer_storage=self.consumer_frame.storage().expect("validated debugger consumer frame");
        self.active_consumer=result.state.active_consumer.clone().map(ConsumerContext::into_active);
    }
    pub(super) fn debugger_expression(&mut self, expression:&Expr, variables:&HashMap<String,Value>, depth:usize) -> Option<EvalResult<Value>> {
        let result=self.debugger_root(RootRef::Expression(expression,depth),variables)?;
        Some(result.and_then(|result| { self.install_debugger_result(&result);if let Some(error)=result.error{return Err(error);}
            result.value.ok_or_else(||invalid("Debugger expression returned no value")) }))
    }
    pub(super) fn debugger_block(&mut self, block:&[Statement], variables:&mut HashMap<String,Value>) -> Option<EvalResult<Option<Value>>> {
        let result=self.debugger_root(RootRef::Block(block),variables)?;
        Some(result.and_then(|result| { self.install_debugger_result(&result); *variables=result.variables;
            if let Some(error)=result.error{Err(error)}else{Ok(result.value)} }))
    }
    pub(super) fn debugger_consumer(&mut self,statement:&Statement,scope:&str,variables:&HashMap<String,Value>)->Option<EvalResult<Value>> {
        let result=self.debugger_root(RootRef::Consumer(statement,scope),variables)?;
        Some(result.and_then(|result| {self.install_debugger_result(&result);if let Some(error)=result.error{return Err(error);}result.value.ok_or_else(||invalid("Engine consumer did not produce a value"))}))
    }
    pub(super) fn debugger_call(&mut self, name:&str, arguments:&[Value]) -> Option<EvalResult<Value>> {
        // No cloned global map is needed merely to compare a sealed trace.
        let result=self.debugger_root(RootRef::Call(name,arguments),&self.globals)?;
        Some(result.and_then(|result| { self.install_debugger_result(&result);if let Some(error)=result.error{return Err(error);}
            result.value.ok_or_else(||invalid("Debugger call returned no value")) }))
    }
}

#[derive(Clone, Debug,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    #[serde(skip)] function1023:Option<FunctionSymbol>,
    #[serde(skip)] unit_locals:Option<HashSet<String>>,
    function:Option<String>, variables:HashMap<String,Value>, depth:usize, current_site:Option<Site>,
    consumer_frame:crate::consumer_schedule::ConsumerFrame,
    #[serde(skip)] consumer_storage:crate::consumer_schedule::FrameStorage,
    active_consumer:Option<ConsumerContext>,
}
# [derive(Clone, Debug,PartialEq,serde::Serialize,serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Context { frame:usize, route:Vec<Route>, depth:usize }
impl Context {
    fn child(&self, part:Route) -> Self { let mut next=self.clone();next.route.push(part);next }
    fn expression(&self, part:Route) -> Self { let mut next=self.child(part);next.depth+=1;next }
}
#[derive(Clone, Debug)]
enum Task {
    InitializerEntry(Context),
    Expr(Expr,Context), Statement(Statement,Context), Block(Vec<Statement>,usize,Context),
    Charge(Context), Neg(Context), Not(Context), Bool(Context), Binary(BinOpKind,Context), Index(Context), List(usize,Context),
    Short { right:Expr, ctx:Context, and:bool },
    Call { name:String, count:usize, ctx:Context },
    LibraryCall { reference:rvn_parser::function_library_asset1023::QualifiedLibraryFunction, count:usize, ctx:Context },
    LibraryUnitStart { call:rvn_parser::function_library_program1023::QualifiedLibraryCall, ctx:Context },
    LibraryUnitArguments { reference:rvn_parser::function_library_asset1023::QualifiedLibraryFunction, count:usize, ctx:Context },
    FunctionEnd { frame:usize, base:usize, name:String },
    Assign { name:String, local:bool, frame:usize, ctx:Context }, Return(Context),
    Branch { yes:Vec<Statement>, no:Vec<Statement>, ctx:Context },
    While { condition:Expr, body:Vec<Statement>, ctx:Context },
    WhileReady { condition:Expr, body:Vec<Statement>, ctx:Context },
    EachReady { name:String, body:Vec<Statement>, ctx:Context },
    EachNext { name:String, body:Vec<Statement>, items:std::vec::IntoIter<Value>, ctx:Context },
    RangeReady { name:String, body:Vec<Statement>, ctx:Context }, RangeExit(Context),
    RangeNext { name:String, body:Vec<Statement>, first:i64, count:i64, cursor:i64, ctx:Context },
    Command { statement:Statement, count:usize, ctx:Context },
    CurveNext { name:String, samples:usize, cursor:usize, values:Vec<f32>, random:RandomState, ctx:Context },
    CurveValue { name:String, samples:usize, cursor:usize, values:Vec<f32>, random:RandomState, ctx:Context },
    NativeLoopResume {statement:Statement,state:LexicalLoopState,ctx:Context},
    MacroOperandNext {dispatch:rvn_parser::scheduled_source::standard_macro_contract::Dispatch,need:crate::widget_class_source_0950::standard_macros::OperandNeed,index:usize,ctx:Context},
    MacroReady {dispatch:rvn_parser::scheduled_source::standard_macro_contract::Dispatch,need:crate::widget_class_source_0950::standard_macros::OperandNeed,ctx:Context},
    NativeLoopHeader {phase:LoopPhase,ctx:Context},
    ConsumerNext {version:u32,consumer:String,scope:String,steps:Vec<rvn_parser::ConsumerStep>,index:usize,body:Vec<Statement>,value_only:bool,ctx:Context},
    ConsumerCapture {producer:String,ctx:Context},
    ConsumerInspect {producer:String,scope:String,ctx:Context},
    ConsumerInputs {count:usize,ctx:Context},
    ConsumerDialogueInput {producer:String,ctx:Context},
    ConsumerBranch {yes:Vec<Statement>,no:Vec<Statement>,ctx:Context},
    ConsumerLoop {statement:Statement,ctx:Context},
    ConsumerIteration {name:String,body:Vec<Statement>,range:bool,ctx:Context},
    ConsumerChoice {options:Vec<rvn_parser::ChoiceOption>,index:usize,rows:Vec<Value>,ctx:Context},
    ConsumerChoiceSelected {options:Vec<rvn_parser::ChoiceOption>,index:usize,rows:Vec<Value>,ctx:Context},
    ConsumerChoiceLabel {options:Vec<rvn_parser::ChoiceOption>,index:usize,rows:Vec<Value>,count:usize,ctx:Context},
    ConsumerEnd {frame:usize,ctx:Context},
    UnitStart{name:String,arguments:Vec<Expr>,ctx:Context},
    UnitArguments{name:String,count:usize,ctx:Context},
    UnitEnd{frame:usize,base:usize,caller:Context},
    RootEnd,
}

/// Owns continuations/lexical bindings/candidate RNG and the caller's cumulative
/// budget. No Rust stack frame or mutable Engine/World borrow survives a stop.
#[derive(Debug, Clone)]
pub(crate) struct Continuation {
    request:Request, library:FunctionLibrary, state:State,
    widget_objects: Option<Arc<crate::widget_reference_values_0974::WidgetValueAdmission>>,
    frames:Vec<Frame>, tasks:Vec<Task>, values:Vec<Value>,
    result:Option<Value>, unit_exit:Option<BlockExit>, error:Option<EvalError>, done:bool,
}
impl Continuation {
    pub(crate) fn fork(&self)->EvalResult<Self>{
        measured(&format_args!("{self:?}"))?;
        let mut candidate=self.clone();
        candidate.library.debug_probe=None;
        Ok(candidate)
    }
    fn new(library:FunctionLibrary, request:Request) -> EvalResult<Self> {
        match &request.root{Root::Block(body) if request.route_prefix.is_empty()=>rvn_parser::scheduled_source::native_loop_contract::validate_authored_bindings(body),_=>Ok(())}.map_err(EvalError::InvalidFunction)?;
        let context=Context{frame:0,route:request.route_prefix.clone(),depth:0};
        let first=match &request.root {
            Root::Expression(expression,depth)=>Task::Expr(expression.clone(),Context{depth:*depth,..context.clone()}),
            Root::Block(block)=>Task::Block(block.clone(),0,context.clone()),
            Root::Call(name,args)=>Task::Call{name:name.clone(),count:args.len(),ctx:context.clone()},
            Root::LibraryCall(reference,args)=>Task::LibraryCall{reference:reference.clone(),count:args.len(),ctx:context.clone()},
            Root::LibraryUnit(call)=>Task::LibraryUnitStart{call:call.clone(),ctx:context.clone()},
            Root::Consumer(Statement::ScheduledConsumer{version,consumer,steps,body,..},scope)=>Task::ConsumerNext{version:*version,consumer:consumer.clone(),scope:scope.clone(),steps:steps.clone(),index:0,body:body.clone(),value_only:true,ctx:context.clone()},
            Root::Consumer(..)=>unreachable!("validated original Engine consumer"),
            Root::Unit(name,arguments)=>Task::UnitStart{name:name.clone(),arguments:arguments.clone(),ctx:context.clone()},
        };
        let values=match &request.root {Root::Call(_,args)|Root::LibraryCall(_,args)=>args.clone(),_=>vec![]};
        let consumer=match &request.root{Root::Consumer(_,scope)=>Some(ConsumerContext{scope:scope.clone(),captures:Default::default(),inspectors:Default::default()}),_=>request.state.active_consumer.clone()};
        let mut tasks=vec![Task::RootEnd];if consumer.is_some()&&matches!(request.root,Root::Consumer(..)){tasks.push(Task::ConsumerEnd{frame:0,ctx:context.clone()});}tasks.push(first);
        if !request.route_prefix.is_empty(){tasks.push(Task::InitializerEntry(context.clone()));}
        Ok(Self{library,widget_objects:None,state:request.state.clone(),frames:vec![Frame{function1023:request.state.function1023.clone(),unit_locals:None,function:request.scope.clone(),variables:request.variables.clone(),depth:request.state.calls,current_site:None,
            consumer_frame:request.state.consumer_frame.clone(),consumer_storage:request.state.consumer_frame.storage().expect("validated request consumer frame"),active_consumer:consumer}],
            tasks,values,result:None,unit_exit:None,error:None,done:false,request})
    }
    pub(crate) fn widget_macro_frame(&self)->Option<&crate::widget_class_source_0950::standard_macros::WidgetMacroFrame>{self.state.widget_macro_frame.as_ref()}
    pub(crate) fn done(&self)->bool { self.done }
    pub(crate) fn site(&self)->Option<Site> {
        let (ctx,statement)=match self.tasks.last()? {
            // Pure value expressions have no independent emitted debug site.
            // Their execution still consumes the retained budget and may enter
            // a function whose actual statements become visible below.
            Task::Statement(_,ctx)=>(ctx.clone(),true),
            Task::InitializerEntry(ctx) if self.library.debug_origins.iter().any(|origin|origin.scope.is_empty()&&origin.route==ctx.route&&origin.origin.scheduled_step.is_none())=>(ctx.clone(),true),
            Task::MacroOperandNext{dispatch,index,ctx,..} if *index<dispatch.operand_steps().len()=>{
                let producer=match &dispatch.operand_steps()[*index]{rvn_parser::ConsumerStep::Capture{producer,..}|rvn_parser::ConsumerStep::InspectTextTable{producer,..}=>producer};
                let consumer=dispatch.site.node_source();let ctx=ctx.child(Route::Producer(*index));let frame=self.frames.get(ctx.frame)?;
                if !self.library.debug_origins.iter().any(|o|o.origin.scheduled_step.as_ref().is_some_and(|step|step.consumer_identity==consumer&&step.producer_identity==*producer)&&o.route==ctx.route&&if let Some(scope)=&frame.function{o.scope==*scope}else{o.scope.is_empty()}){return None;}
                (ctx,false)
            },
            Task::ConsumerNext{version,consumer,steps,index,ctx,..} if *index<steps.len()=>{
                if *version==3 {
                    // A skipped producer has no reached source observation.
                    // Its original ordinal is retained only for later slices.
                    let (_,selected,_,_)=self.loop_phase_details(ctx).ok()??;
                    if !selected.contains(index){return None;}
                }
                let ctx=ctx.child(Route::Producer(*index));let frame=self.frames.get(ctx.frame)?;
                let producer=match &steps[*index]{rvn_parser::ConsumerStep::Capture{producer,..}|rvn_parser::ConsumerStep::InspectTextTable{producer,..}=>producer};
                if !self.library.debug_origins.iter().any(|o|o.origin.scheduled_step.as_ref().is_some_and(|step|step.consumer_identity==*consumer&&step.producer_identity==*producer)&&o.route==ctx.route&&if let Some(scope)=&frame.function{o.scope==*scope}else{o.scope.is_empty()}){return None;}
                (ctx,false)
            },_=>return None,
        };
        let frame=self.frames.get(ctx.frame)?;
        Some(Site{function:frame.function.clone(),route:ctx.route.clone(),depth:frame.depth,statement})
    }
    #[cfg(test)]
    pub(crate) fn dialogue_transfer_pending(&self)->bool {
        matches!(self.tasks.last(),Some(Task::ConsumerDialogueInput{..}))
    }
    pub(crate) fn depth(&self)->usize { self.frames.last().map_or(0,|frame|frame.depth) }
    pub(crate) fn work(&self)->(usize,usize){(self.state.steps,self.state.value_work)}
    pub(crate) fn site_origin(&self,site:&Site,position:&crate::scene::ResumePosition)->Option<crate::program::InstructionOrigin>{
        self.library.debug_origins.iter().find(|origin|origin.route==site.route&&if let Some(scope)=&site.function{origin.scope==*scope}else{
            origin.scope.is_empty()&&self.library.debug_source_map.as_ref().and_then(|map|map.origins.get(position.pc)).is_some_and(|actual|actual.source==origin.declaration.source&&actual.span==origin.declaration.span)
        }).map(|site|site.origin.clone())
            .or_else(||(site.statement&&site.function.is_none()).then(||position.origin.clone()).flatten())
    }
    pub(crate) fn snapshot(&self, instance:u64, epoch:u64, position:crate::scene::ResumePosition, watches:&[wire::Watch])->wire::Snapshot {
        let mut remaining=wire::MAX_SNAPSHOT_BYTES;
        let active=self.site();
        let frames=self.frames.iter().enumerate().map(|(index,frame)|{
            let mut names=frame.variables.keys().collect::<Vec<_>>();names.sort();names.truncate(wire::MAX_WATCHES);
            let bindings=names.into_iter().filter_map(|name|{
                let bytes=name.len().checked_add(64)?;if bytes>remaining{return None;}remaining-=bytes;
                Some((name.clone(),wire::bounded_value_with_budget(&frame.variables[name],&mut remaining)))
            }).collect();
            let site=if index+1==self.frames.len(){active.clone().or_else(||frame.current_site.clone())}else{frame.current_site.clone()};
            let origin=site.as_ref().and_then(|site|self.site_origin(site,&position));
            wire::LexicalFrame{function:frame.function.clone(),depth:frame.depth,site,origin,binding_total:frame.variables.len(),bindings}
        }).collect();
        let watches=watches.iter().take(wire::MAX_WATCHES).map(|watch|{
            let map=match watch.scope {wire::WatchScope::Globals=>Some(&self.state.globals),wire::WatchScope::Frame(index)=>self.frames.get(index).map(|frame|&frame.variables)};
            remaining=remaining.saturating_sub(watch.name.len()+64);
            let value=match map {None=>wire::WatchValue::NotInScope,Some(map)=>map.get(&watch.name).map(|value|wire::bounded_value_with_budget(value,&mut remaining)).unwrap_or(wire::WatchValue::NoProperty)};
            wire::ObservedWatch{watch:watch.clone(),value}
        }).collect();
        let site=self.site();
        let origin=site.as_ref().and_then(|site|self.site_origin(site,&position));
        wire::Snapshot{instance,epoch,position,site,origin,frames,watches,evaluating:!self.done,running:false,armed:None,candidate:true,
            steps:self.state.steps,value_work:self.state.value_work,error:self.error.as_ref().map(ToString::to_string),sites:None,breakpoints:Vec::new()}
    }
    /// Executes one visible AST operation plus its administrative continuations.
    /// Administrative operations are bounded by the same retained step budget.
    pub(crate) fn step(&mut self)->EvalResult<()> { self.step_with_budget(4096) }
    pub(crate) fn step_with_budget(&mut self,administrative_limit:usize)->EvalResult<()> {
        let administrative_limit=administrative_limit.clamp(1,4096);
        if self.done{return Ok(());}
        let result:EvalResult<()>=(||{
            let started_visible=self.site().is_some();let mut visible=false;let mut administrative=0;
            while !self.done {
                if self.site().is_some()&&(visible||!started_visible){break;}
                visible|=self.site().is_some();
                let task=self.tasks.pop().ok_or_else(||invalid("Debugger continuation underflow"))?;
                administrative+=1;if administrative>administrative_limit{self.tasks.push(task);break;}
                self.execute(task)?;
            }
            Ok(())
        })();
        if let Err(error)=&result {self.cleanup_lexical_tasks(0);self.error=Some(error.clone());self.done=true;self.tasks.clear();}
        result
    }
    fn tick(&mut self)->EvalResult<()> {
        self.state.steps=self.state.steps.checked_add(1).ok_or(EvalError::NumericOverflow)?;
        if self.state.steps>MAX_COMPUTATION_STEPS{return Err(EvalError::ExecutionLimit{limit:"100 000 opérations"});}Ok(())
    }
    fn charge(&mut self,value:&Value)->EvalResult<()> {
        self.state.value_work=self.state.value_work.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;
        if self.state.value_work>1_000_000{return Err(EvalError::ExecutionLimit{limit:"1 000 000 copies ou caractères de valeurs par calcul"});}Ok(())
    }
    fn pop(&mut self)->EvalResult<Value>{self.values.pop().ok_or_else(||invalid("Debugger value stack underflow"))}
    fn arguments(&mut self,count:usize)->EvalResult<Vec<Value>> {
        if self.values.len()<count{return Err(invalid("Debugger argument stack underflow"));}
        Ok(self.values.split_off(self.values.len()-count))
    }
    fn assign(&mut self,frame:usize,name:String,value:Value,local:bool)->EvalResult<()> {
        if let Some(locals)=&mut self.frames[frame].unit_locals {
            if local{locals.insert(name.clone());}
            if !locals.contains(&name){self.state.globals.insert(name.clone(),value.clone());self.state.unit_writes.insert(name.clone());}
        }else if self.frames[frame].depth==0 {
            if local {match self.state.handler_locals.as_mut(){Some(locals)=>{locals.insert(name.clone());},None=>return Err(invalid("local hors d’un calcul ou gestionnaire"))}}
            else if !self.state.handler_locals.as_ref().is_some_and(|locals|locals.contains(&name)){self.state.globals.insert(name.clone(),value.clone());}
        }
        self.frames[frame].variables.insert(name,value);Ok(())
    }
    fn execute(&mut self,task:Task)->EvalResult<()> {
        match task {
            Task::Expr(expression,ctx)=>{
                if let Expr::Call{name,..}=&expression{if self.library.legacy_unit(name){return Err(invalid("Unit function has no expression value"));}}
                self.tick()?;if ctx.depth>256{return Err(EvalError::ExecutionLimit{limit:"256 niveaux d’expression"});}
                self.tasks.push(Task::Charge(ctx.clone()));
                match expression {
                    Expr::StandardMacroData{site,output,pin}=>self.values.push(self.macro_data(&site,output,pin)?),
                    Expr::Int(v)=>self.values.push(Value::Int(v)),Expr::Float(v)=>self.values.push(Value::Float(v)),Expr::Bool(v)=>self.values.push(Value::Bool(v)),Expr::Str(v)=>self.values.push(Value::Str(v)),
                    Expr::Var(name)=>self.values.push(self.frames[ctx.frame].variables.get(&name).cloned().ok_or(EvalError::UndefinedVar(name))?),
                    Expr::Neg(inner)=>{self.tasks.push(Task::Neg(ctx.clone()));self.tasks.push(Task::Expr(*inner,ctx.expression(Route::Unary)));},
                    Expr::Not(inner)=>{self.tasks.push(Task::Not(ctx.clone()));self.tasks.push(Task::Expr(*inner,ctx.expression(Route::Unary)));},
                    Expr::BinOp{op,left,right}=>{self.tasks.push(Task::Binary(op,ctx.clone()));self.tasks.push(Task::Expr(*right,ctx.expression(Route::Right)));self.tasks.push(Task::Expr(*left,ctx.expression(Route::Left)));},
                    Expr::And(left,right)=>{self.tasks.push(Task::Short{right:*right,ctx:ctx.clone(),and:true});self.tasks.push(Task::Expr(*left,ctx.expression(Route::Left)));},
                    Expr::Or(left,right)=>{self.tasks.push(Task::Short{right:*right,ctx:ctx.clone(),and:false});self.tasks.push(Task::Expr(*left,ctx.expression(Route::Left)));},
                    Expr::Call{name,args}=>{self.tasks.push(Task::Call{name,count:args.len(),ctx:ctx.clone()});for (index,arg) in args.into_iter().enumerate().rev(){self.tasks.push(Task::Expr(arg,ctx.expression(Route::Argument(index))));}},
                    Expr::LibraryCall{call}=>self.schedule_library_value1023(call,ctx)?,
                    Expr::ListLit(items)=>{self.tasks.push(Task::List(items.len(),ctx.clone()));for(index,item)in items.into_iter().enumerate().rev(){self.tasks.push(Task::Expr(item,ctx.expression(Route::Argument(index))));}},
                    Expr::Index{target,index}=>{self.tasks.push(Task::Index(ctx.clone()));self.tasks.push(Task::Expr(*index,ctx.expression(Route::Index)));self.tasks.push(Task::Expr(*target,ctx.expression(Route::Collection)));},
                }
            },
            Task::MacroOperandNext{dispatch,need,index,ctx}=>self.macro_operand_next(dispatch,need,index,ctx)?,
            Task::MacroReady{dispatch,need,ctx}=>self.macro_ready(dispatch,need,ctx)?,
            Task::InitializerEntry(_)=>{self.tick()?;},
            Task::Charge(_)=>{let value=self.values.last().cloned().ok_or_else(||invalid("Debugger result missing"))?;crate::enum_runtime_resources1025::validate_value(&self.library.enum_assets1025,&value)?;self.charge(&value)?;},
            Task::Neg(_)=>{let value=self.pop()?;let result=match value{Value::Int(v)=>Value::Int(v.checked_neg().ok_or(EvalError::NumericOverflow)?),Value::Float(v)=>Value::Float(-v),v=>return Err(EvalError::TypeMismatch{op:"-".into(),left:type_name(&v).into(),right:String::new()})};self.values.push(result);},
            Task::Not(_)=>{let value=self.pop()?;self.values.push(Value::Bool(!truthy_bounded(&value,Some(self.library.text_resources()),&mut self.state.value_work)?));},
            Task::Bool(_)=>{let value=self.pop()?;self.values.push(Value::Bool(truthy_bounded(&value,Some(self.library.text_resources()),&mut self.state.value_work)?));},
            Task::Binary(op,_)=>{let right=self.pop()?;let left=self.pop()?;self.values.push(eval_binop(&op,left,right)?);},
            Task::Short{right,ctx,and}=>{
                let value=self.pop()?;let left=truthy_bounded(&value,Some(self.library.text_resources()),&mut self.state.value_work)?;
                if left==and{self.tasks.push(Task::Bool(ctx.clone()));self.tasks.push(Task::Expr(right,ctx.expression(Route::Right)));}
                else{self.values.push(Value::Bool(left));let charge=self.tasks.pop();if !matches!(charge,Some(Task::Charge(_))){return Err(invalid("Debugger short circuit continuation changed"));}}
            },
            Task::List(count,_)=>{let values=self.arguments(count)?;self.values.push(Value::List(values));},
            Task::Index(_)=>{let index=self.pop()?;let target=self.pop()?;let value=match (&target,&index){
                (Value::Dict(items),Value::Str(key))=>items.get(key).cloned().ok_or_else(||EvalError::UndefinedVar(format!("dictionary key {key}")))?,
                (Value::List(items),Value::Int(i))=>items.get(*i as usize).cloned().ok_or_else(||index_error(*i as usize,items.len()))?,
                (Value::Str(text),Value::Int(i))=>text.chars().nth(*i as usize).map(|v|Value::Str(v.to_string())).ok_or_else(||index_error(*i as usize,text.chars().count()))?,
                _=>return Err(EvalError::TypeMismatch{op:"index".into(),left:"list or string".into(),right:"int".into()}),
            };self.values.push(value);},
            Task::Call{name,count,ctx}=>{let arguments=self.arguments(count)?;self.call(name,arguments,ctx)?;},
            Task::LibraryCall{reference,count,ctx}=>{let arguments=self.arguments(count)?;self.enter_library_value1023(reference,arguments,ctx)?;},
            Task::LibraryUnitStart{call,ctx}=>self.schedule_library_unit1023(call,ctx)?,
            Task::LibraryUnitArguments{reference,count,ctx}=>{let arguments=self.arguments(count)?;self.enter_library_unit1023(reference,arguments,ctx)?;},
            Task::UnitStart{name,arguments,ctx}=>self.schedule_unit(name,arguments,ctx)?,
            Task::UnitArguments{name,count,ctx}=>{let arguments=self.arguments(count)?;self.enter_unit(name,arguments,ctx)?;},
            Task::UnitEnd{frame,base,caller}=>self.finish_unit_frame(frame,base,caller,BlockExit::Continue)?,
            Task::FunctionEnd{frame,base,name}=>{self.frames.truncate(frame);self.state.calls-=1;self.values.truncate(base);return Err(invalid(&format!("{name} n’a retourné aucune valeur")));},
            Task::Block(block,index,ctx)=>{
                if let Some(statement)=block.get(index).cloned(){self.tasks.push(Task::Block(block,index+1,ctx.clone()));self.tasks.push(Task::Statement(statement,ctx.child(Route::Statement(index))));}
            },
            Task::Statement(statement,ctx)=>{
                let frame=&mut self.frames[ctx.frame];frame.current_site=Some(Site{function:frame.function.clone(),route:ctx.route.clone(),depth:frame.depth,statement:true});
                self.tick()?;self.statement(statement,ctx)?;
            },
            Task::Assign{name,local,frame,..}=>{let value=self.pop()?;self.assign(frame,name,value,local)?;},
            Task::Return(_)=>{
                let value=self.pop()?;
                if let Some(index)=self.tasks.iter().rposition(|task|matches!(task,Task::FunctionEnd{..})){
                    let Task::FunctionEnd{frame,base,..}=self.tasks[index].clone()else{unreachable!()};
                    self.cleanup_lexical_tasks(index);self.tasks.truncate(index);self.frames.truncate(frame);self.state.calls-=1;self.values.truncate(base);self.values.push(value);
                }else{self.cleanup_lexical_tasks(0);self.tasks.clear();self.frames[0].active_consumer=None;self.result=Some(value);self.done=true;}
            },
            Task::Branch{yes,no,ctx}=>{let value=self.pop()?;let yes_selected=truthy_bounded(&value,Some(self.library.text_resources()),&mut self.state.value_work)?;self.tasks.push(Task::Block(if yes_selected{yes}else{no},0,ctx.child(if yes_selected{Route::Then}else{Route::Else})));},
            Task::While{condition,body,ctx}=>{self.tasks.push(Task::WhileReady{condition:condition.clone(),body,ctx:ctx.clone()});self.tasks.push(Task::Expr(condition,Context{depth:0,..ctx.child(Route::Condition)}));},
            Task::WhileReady{condition,body,ctx}=>{let value=self.pop()?;if truthy_bounded(&value,Some(self.library.text_resources()),&mut self.state.value_work)?{self.tick()?;self.tasks.push(Task::While{condition,body:body.clone(),ctx:ctx.clone()});self.tasks.push(Task::Block(body,0,ctx.child(Route::Body)));}},
            Task::EachReady{name,body,ctx}=>{let Value::List(items)=self.pop()?else{return Err(invalid("for attend une liste"));};self.loop_binding(&name,ctx.frame);self.tasks.push(Task::EachNext{name,body,items:items.into_iter(),ctx});},
            Task::EachNext{name,body,mut items,ctx}=>{if let Some(value)=items.next(){self.tick()?;self.assign(ctx.frame,name.clone(),value,false)?;self.tasks.push(Task::EachNext{name,body:body.clone(),items,ctx:ctx.clone()});self.tasks.push(Task::Block(body,0,ctx.child(Route::Body)));}},
            Task::RangeReady{name,body,ctx}=>{let last=self.pop()?;let first=self.pop()?;let(first,count)=crate::flow_control::bounds(&first,&last)?;self.loop_binding(&name,ctx.frame);self.tasks.push(Task::RangeExit(ctx.clone()));self.tasks.push(Task::RangeNext{name,body,first,count,cursor:0,ctx});},
            Task::RangeExit(_)=>{},
            Task::RangeNext{name,body,first,count,cursor,ctx}=>{if cursor<count{self.tick()?;self.assign(ctx.frame,name.clone(),Value::Int(first.checked_add(cursor).ok_or(EvalError::NumericOverflow)?),false)?;self.tasks.push(Task::RangeNext{name,body:body.clone(),first,count,cursor:cursor.checked_add(1).ok_or(EvalError::NumericOverflow)?,ctx:ctx.clone()});self.tasks.push(Task::Block(body,0,ctx.child(Route::Body)));}},
            Task::Command{statement,count,ctx}=>{
                let values=self.arguments(count)?;let (statement,bindings)=replace_command_values(statement,values)?;
                let mut scratch=Computation::new(&self.library,&self.state.globals);scratch.widget_objects=self.widget_objects.as_deref();scratch.random=self.state.random;scratch.locale=self.state.locale.as_ref();
                let command=scratch.ui_command(&statement,&bindings)?;
                if self.state.commands.len()>=128{return Err(EvalError::ExecutionLimit{limit:"128 commandes d’interface par événement"});}
                self.state.commands.push(command);
            },
            Task::CurveNext{name,samples,cursor,values,random,ctx}=>{
                if cursor==samples{
                    if self.state.random!=random{self.state.random=random;return Err(invalid("Une courbe d’animation doit être déterministe : random est interdit"));}
                    rvn_ui::motion::Easing::Samples{values:values.clone()}.validate().map_err(EvalError::InvalidFunction)?;
                    let mut values=values;values[0]=0.0;values[samples-1]=1.0;
                    self.values.push(Value::Dict(BTreeMap::from([("kind".into(),Value::Str("samples".into())),("values".into(),Value::List(values.into_iter().map(Value::Float).collect()))])));
                }else{
                    self.tasks.push(Task::CurveValue{name:name.clone(),samples,cursor,values,random,ctx:ctx.clone()});
                    self.call(name,vec![Value::Float(cursor as f32/(samples-1)as f32)],ctx)?;
                }
            },
            Task::CurveValue{name,samples,cursor,mut values,random,ctx}=>{let value=match self.pop()?{Value::Int(v)=>v as f32,Value::Float(v)=>v,_=>return Err(invalid("A custom animation curve must return a number"))};values.push(value);self.tasks.push(Task::CurveNext{name,samples,cursor:cursor+1,values,random,ctx});},
            Task::NativeLoopResume{statement,state,ctx}=>self.lexical_body_complete(statement,state,ctx)?,
            Task::NativeLoopHeader{phase,ctx}=>self.lexical_header_ready(phase,ctx)?,
            Task::ConsumerNext{version,consumer,scope,steps,index,body,value_only,ctx}=>{
                if version==3 {
                    let Some((_,selected,root,root_ctx))=self.loop_phase_details(&ctx)? else{return Err(invalid("Native phase lost its actual Engine or lexical request"));};
                    if index<steps.len() && !selected.contains(&index){
                        self.tasks.push(Task::ConsumerNext{version,consumer,scope,steps,index:index+1,body,value_only,ctx});return Ok(());
                    }
                    if index==steps.len(){self.tasks.push(Task::Expr(root,root_ctx));return Ok(());}
                }
                if let Some(step)=steps.get(index).cloned(){
                    self.tick()?;
                    self.tasks.push(Task::ConsumerNext{version,consumer,scope:scope.clone(),steps,index:index+1,body,value_only,ctx:ctx.clone()});
                    let (producer,value,inspect)=match step{
                        rvn_parser::ConsumerStep::Capture{producer,value}=>(producer,value,false),
                        rvn_parser::ConsumerStep::InspectTextTable{producer,value}=>(producer,value,true),
                    };
                    self.tasks.push(if inspect{Task::ConsumerInspect{producer,scope,ctx:ctx.child(Route::Producer(index))}}else{Task::ConsumerCapture{producer,ctx:ctx.child(Route::Producer(index))}});
                    self.tasks.push(Task::Expr(value,Context{depth:0,..ctx.child(Route::Producer(index))}));
                }else if value_only {
                    let binding=match &self.request.root {
                        Root::Consumer(authored,_)=>rvn_parser::scheduled_source::dialogue_text_binding(authored).map_err(EvalError::InvalidFunction)?,
                        _=>None,
                    };
                    if let Some(binding)=binding {
                        self.tasks.push(Task::ConsumerDialogueInput{producer:binding.producer,ctx});return Ok(());
                    }
                    if let[Statement::Choice{options}]=body.as_slice(){self.tasks.push(Task::ConsumerChoice{options:options.clone(),index:0,rows:vec![],ctx});return Ok(());}
                    let inputs=body.first().map(crate::consumer_schedule::direct_expressions).unwrap_or_default();let scalar=body.first().is_some_and(crate::consumer_schedule::scalar_engine_input);
                    if !scalar{self.tasks.push(Task::ConsumerInputs{count:inputs.len(),ctx:ctx.clone()});}
                    for (index,value) in inputs.into_iter().enumerate().rev(){let child=ctx.child(Route::Body).child(Route::Statement(0));let child=if scalar{if matches!(body.first(),Some(Statement::If{..}|Statement::While{..})){child.child(Route::Condition)}else if matches!(body.first(),Some(Statement::MotionWait{..}|Statement::VideoWait{..})){child.child(Route::Argument(0))}else{child}}else{let ordinal=body.first().and_then(|statement|rvn_parser::widget_create_effect::parts(statement).ok().flatten()).map(|input|rvn_parser::widget_create_effect::argument_ordinal(&input,index)).unwrap_or(index);child.child(Route::Argument(ordinal))};self.tasks.push(Task::Expr(value.clone(),Context{depth:0,..child}));}
                }else{match body.as_slice(){
                    [Statement::ForEach{name,collection,body}]=>{self.tasks.push(Task::ConsumerIteration{name:name.clone(),body:body.clone(),range:false,ctx:ctx.clone()});self.tasks.push(Task::Expr(collection.clone(),Context{depth:0,..ctx.child(Route::Body).child(Route::Statement(0)).child(Route::Collection)}));},
                    [Statement::ForLoopWithBreak{name,first,last,body}]=>{self.tasks.push(Task::ConsumerIteration{name:name.clone(),body:body.clone(),range:true,ctx:ctx.clone()});let inner=ctx.child(Route::Body).child(Route::Statement(0));self.tasks.push(Task::Expr(last.clone(),Context{depth:0,..inner.child(Route::Last)}));self.tasks.push(Task::Expr(first.clone(),Context{depth:0,..inner.child(Route::First)}));},
                    [Statement::While{condition,..}]=>{let statement=Statement::ScheduledConsumer{version,scope,consumer,steps,body:body.clone()};self.tasks.push(Task::ConsumerLoop{statement,ctx:ctx.clone()});self.tasks.push(Task::Expr(condition.clone(),Context{depth:0,..ctx.child(Route::Body).child(Route::Statement(0)).child(Route::Condition)}));},
                    [Statement::If{condition,then_branch,else_branch}]=>{self.tasks.push(Task::ConsumerBranch{yes:then_branch.clone(),no:else_branch.clone(),ctx:ctx.clone()});self.tasks.push(Task::Expr(condition.clone(),Context{depth:0,..ctx.child(Route::Body).child(Route::Statement(0)).child(Route::Condition)}));},
                    [Statement::Sequence{branches}]=>{self.close_consumer(&ctx)?;let inner=ctx.child(Route::Body).child(Route::Statement(0));for(index,branch)in branches.iter().enumerate().rev(){self.tasks.push(Task::Block(branch.clone(),0,inner.child(Route::Branch(index))));}},
                    _=>self.tasks.push(Task::Block(body,0,ctx.child(Route::Body))),
                }}
            },
            Task::ConsumerInputs{count,..}=>{let values=self.arguments(count)?;self.values.push(Value::List(values));},
            Task::ConsumerDialogueInput{producer,ctx}=>{
                let value=self.frames[ctx.frame].active_consumer.as_ref().and_then(|a|a.captures.get(&producer))
                    .ok_or_else(||invalid("Dynamic Dialogue lost its actual captured port"))?;
                let value=crate::consumer_schedule::retained_dialogue_port(value,&mut self.state.value_work)?;
                self.values.push(Value::List(vec![value]));
            },
            Task::ConsumerBranch{yes,no,ctx}=>{let value=self.pop()?;let selected=truthy_bounded(&value,Some(self.library.text_resources()),&mut self.state.value_work)?;self.close_consumer(&ctx)?;self.tasks.push(Task::Block(if selected{yes}else{no},0,ctx.child(Route::Body).child(Route::Statement(0)).child(if selected{Route::Then}else{Route::Else})));},
            Task::ConsumerLoop{statement,ctx}=>{
                let value=self.pop()?;let entered=truthy_bounded(&value,Some(self.library.text_resources()),&mut self.state.value_work)?;self.close_consumer(&ctx)?;
                if entered {
                    let Statement::ScheduledConsumer{body,..}=&statement else{unreachable!()};let[Statement::While{body,..}]=body.as_slice()else{unreachable!()};let body=body.clone();
                    self.tasks.push(Task::Statement(statement,ctx.clone()));self.tasks.push(Task::Block(body,0,ctx.child(Route::Body).child(Route::Statement(0)).child(Route::Body)));
                }
            },
            Task::ConsumerIteration{name,body,range,ctx}=>{
                let last=if range{Some(self.pop()?)}else{None};let first=self.pop()?;self.close_consumer(&ctx)?;self.loop_binding(&name,ctx.frame);let inner=ctx.child(Route::Body).child(Route::Statement(0));
                if let Some(last)=last{let(first,count)=crate::flow_control::bounds(&first,&last)?;self.tasks.push(Task::RangeExit(inner.clone()));self.tasks.push(Task::RangeNext{name,body,first,count,cursor:0,ctx:inner});}
                else{let Value::List(items)=first else{return Err(invalid("for attend une liste"));};self.tasks.push(Task::EachNext{name,body,items:items.into_iter(),ctx:inner});}
            },
            Task::ConsumerChoice{options,index,rows,ctx}=>{
                if let Some(option)=options.get(index){self.tick()?;let condition=option.condition.clone();self.tasks.push(Task::ConsumerChoiceSelected{options,index,rows,ctx:ctx.clone()});if let Some(condition)=condition{self.tasks.push(Task::Expr(condition,Context{depth:0,..ctx.child(Route::Body).child(Route::Statement(0)).child(Route::Option(index)).child(Route::Condition)}));}else{self.values.push(Value::Bool(true));}}
                else{self.values.push(Value::List(rows));}
            },
            Task::ConsumerChoiceSelected{options,index,mut rows,ctx}=>{
                let value=self.pop()?;let visible=truthy_bounded(&value,Some(self.library.text_resources()),&mut self.state.value_work)?;
                if visible{let expressions=options[index].label.0.iter().filter_map(|p|if let rvn_parser::TextSegment::Interp(e)=p{Some(e.clone())}else{None}).collect::<Vec<_>>();self.tasks.push(Task::ConsumerChoiceLabel{options,index,rows,count:expressions.len(),ctx:ctx.clone()});for(i,expr)in expressions.into_iter().enumerate().rev(){self.tasks.push(Task::Expr(expr,Context{depth:0,..ctx.child(Route::Body).child(Route::Statement(0)).child(Route::Option(index)).child(Route::Argument(i))}));}}
                else{rows.push(Value::List(vec![Value::Bool(false),Value::List(vec![])]));self.tasks.push(Task::ConsumerChoice{options,index:index+1,rows,ctx});}
            },
            Task::ConsumerChoiceLabel{options,index,mut rows,count,ctx}=>{let values=self.arguments(count)?;rows.push(Value::List(vec![Value::Bool(true),Value::List(values)]));self.tasks.push(Task::ConsumerChoice{options,index:index+1,rows,ctx});},
            Task::ConsumerCapture{producer,ctx}=>{
                let value=self.pop()?;
                self.frames[ctx.frame].active_consumer.as_mut().ok_or_else(||invalid("Consumer capture lost its owning phase"))?.captures.insert(producer,value);
            },
            Task::ConsumerInspect{producer,scope,ctx}=>{
                let value=self.pop()?;let frame=&mut self.frames[ctx.frame];
                let copied=frame.consumer_frame.inspect(&mut frame.consumer_storage,&scope,&producer,value)?;
                self.state.value_work=self.state.value_work.checked_add(copied).ok_or(EvalError::NumericOverflow)?;
                if self.state.value_work>1_000_000{return Err(EvalError::ExecutionLimit{limit:"1 000 000 copies ou caractères de valeurs par calcul"});}
                frame.active_consumer.as_mut().ok_or_else(||invalid("Consumer inspector lost its owning phase"))?.inspectors.insert(producer);
            },
            Task::ConsumerEnd{frame,..}=>{self.frames[frame].active_consumer=None;},
            Task::RootEnd=>{if matches!(self.request.root,Root::Unit(..)|Root::LibraryUnit(..)){
                if !matches!(self.unit_exit,Some(BlockExit::Continue|BlockExit::Unit))||!self.values.is_empty(){return Err(invalid("Unit root lost its typed completion"));}
            }else{self.result=self.values.pop();}self.done=true;},
        }
        if self.done {
            self.state.consumer_frame=self.frames[0].consumer_frame.clone();
            self.state.active_consumer=self.frames[0].active_consumer.clone();
        }
        Ok(())
    }
    fn close_consumer(&mut self,ctx:&Context)->EvalResult<()>{match self.tasks.pop(){Some(Task::ConsumerEnd{frame,ctx:owner})if frame==ctx.frame&&owner==*ctx=>{self.frames[frame].active_consumer=None;Ok(())},_=>Err(invalid("Structured consumer lost its exact phase boundary"))}}
    fn loop_binding(&mut self,name:&str,frame:usize){if let Some(locals)=&mut self.frames[frame].unit_locals{locals.insert(name.into());}else if self.frames[frame].depth==0{if let Some(locals)=&mut self.state.handler_locals{locals.insert(name.into());}}}
    fn call(&mut self,name:String,arguments:Vec<Value>,ctx:Context)->EvalResult<()> {
        for value in &arguments{crate::enum_runtime_resources1025::validate_value(&self.library.enum_assets1025,value)?;}
        if name==enum_values1025::CONSTRUCTOR{self.values.push(enum_values1025::construct(&self.library.enum_assets1025,&arguments)?);return Ok(());}
        if self.state.pure&&matches!(name.as_str(),"random"|"rand"|"mouse_clicked"|"mouse_x"|"mouse_y"|"key_pressed"){
            return Err(invalid(&format!("Pure canvas drawing cannot query '{name}'; use saved state and event handlers")));
        }
        if name=="motion_curve"{
            let[Value::Str(function),Value::Int(samples)]=arguments.as_slice()else{return Err(invalid("motion_curve(nom de fonction, 2–257 échantillons) attendu"));};
            if !(2..=257).contains(samples)||!self.library.has_function(function,1){return Err(invalid("La courbe attend une fonction RVN à un paramètre et 2–257 échantillons"));}
            self.tasks.push(Task::CurveNext{name:function.clone(),samples:*samples as usize,cursor:0,values:Vec::with_capacity(*samples as usize),random:self.state.random,ctx});return Ok(());
        }
        if matches!(name.as_str(),"__rvn_captured_v1"|"__rvn_terminal_v1") {
            let frame=&self.frames[ctx.frame];
            let active=frame.active_consumer.as_ref().ok_or_else(||invalid("Scheduled port access outside its consumer activation"))?;
            let value=match(name.as_str(),arguments.as_slice()) {
                ("__rvn_captured_v1",[Value::Str(id)])=>active.captures.get(id).cloned().ok_or_else(||invalid("Unknown scheduled capture"))?,
                ("__rvn_terminal_v1",[Value::Str(id),Value::Str(port)]) if active.inspectors.contains(id)=>{
                    let terminal=frame.consumer_frame.terminals(&active.scope,id).ok_or_else(||invalid("Unknown scheduled terminal"))?;
                    match port.as_str(){"valid"=>Value::Bool(terminal.valid),"table_id"=>Value::Name(terminal.table_id.clone()),"key"=>Value::Str(terminal.key.clone()),_=>return Err(invalid("Unknown typed scheduled port"))}
                },
                _=>return Err(invalid("Invalid scheduled port arguments")),
            };
            self.values.push(value);return Ok(());
        }
        if self.library.legacy_unit(&name){return Err(invalid("Unit function cannot be an old value-call debugger activation"));}
        let Some((parameters,body))=self.library.legacy_definition(&name).cloned()else{
            self.values.push(eval_call(&name,arguments,&self.state.globals,&mut self.state.random,self.state.locale.as_ref(),Some(self.library.text_resources()),&self.library.widget_classes,self.widget_objects.as_deref(),&mut self.state.value_work)?);return Ok(());
        };
        self.enter_value_definition1023(FunctionSymbol::legacy(&name),parameters,body,arguments,ctx)
    }

    fn statement(&mut self,statement:Statement,ctx:Context)->EvalResult<()> {
        match statement {
            Statement::ScheduledConsumer {version,consumer,scope,steps,body}=>{
                if version==3{return self.begin_lexical_loop(Statement::ScheduledConsumer{version,consumer,scope,steps,body},ctx);}
                // The immutable library and public block entry validated this
                // plan before effects. Do not rescan skipped RHSs per visit.
                if self.frames[ctx.frame].active_consumer.is_some(){return Err(invalid("Nested consumer must use its own activation"));}
                self.frames[ctx.frame].active_consumer=Some(ConsumerContext{scope:scope.clone(),captures:BTreeMap::new(),inspectors:Default::default()});
                self.tasks.push(Task::ConsumerEnd{frame:ctx.frame,ctx:ctx.clone()});
                self.tasks.push(Task::ConsumerNext{version,consumer,scope,steps,index:0,body,value_only:false,ctx});
            },
            Statement::StandardMacroDispatch{dispatch}=>self.start_macro(*dispatch,ctx)?,
            Statement::SetVar{name,value}=>{self.tasks.push(Task::Assign{name,local:false,frame:ctx.frame,ctx:ctx.clone()});self.tasks.push(Task::Expr(value,Context{depth:0,..ctx}));},
            Statement::LocalVar{name,value}=>{self.tasks.push(Task::Assign{name,local:true,frame:ctx.frame,ctx:ctx.clone()});self.tasks.push(Task::Expr(value,Context{depth:0,..ctx}));},
            Statement::LibraryUnitCall{call}=>self.schedule_library_unit1023(call,ctx)?,
            Statement::UnitCall{name,arguments}=>{if self.frames[ctx.frame].unit_locals.is_none(){return Err(invalid("Unit call outside actual Unit root"));}self.schedule_unit(name,arguments,ctx)?;},
            Statement::UnitReturn=>self.return_unit(&ctx)?,
            Statement::FunctionReturn{value}=>{if self.frames[ctx.frame].unit_locals.is_some(){return Err(invalid("Value return inside Unit activation"));}self.tasks.push(Task::Return(ctx.clone()));self.tasks.push(Task::Expr(value,Context{depth:0,..ctx}));},
            Statement::If{condition,then_branch,else_branch}=>{self.tasks.push(Task::Branch{yes:then_branch,no:else_branch,ctx:ctx.clone()});self.tasks.push(Task::Expr(condition,Context{depth:0,..ctx.child(Route::Condition)}));},
            Statement::While{condition,body}=>self.tasks.push(Task::While{condition,body,ctx}),
            Statement::ForEach{name,collection,body}=>{self.tasks.push(Task::EachReady{name,body,ctx:ctx.clone()});self.tasks.push(Task::Expr(collection,Context{depth:0,..ctx.child(Route::Collection)}));},
            Statement::ForLoopWithBreak{name,first,last,body}=>{self.tasks.push(Task::RangeReady{name,body,ctx:ctx.clone()});self.tasks.push(Task::Expr(last,Context{depth:0,..ctx.child(Route::Last)}));self.tasks.push(Task::Expr(first,Context{depth:0,..ctx.child(Route::First)}));},
            Statement::Sequence{branches}=>{for(index,branch)in branches.into_iter().enumerate().rev(){self.tasks.push(Task::Block(branch,0,ctx.child(Route::Branch(index))));}},
            Statement::LoopBreak=>{
                if self.lexical_break(&ctx)?{return Ok(());}
                let index=self.tasks.iter().rposition(|task|matches!(task,Task::RangeExit(_))).ok_or_else(||invalid("Break hors de ForLoopWithBreak"))?;
                if self.tasks[index..].iter().any(|task|matches!(task,Task::FunctionEnd{..}|Task::UnitEnd{..})){return Err(invalid("Break ne peut pas sortir d’un appel de fonction"));}
                self.cleanup_lexical_tasks(index);self.tasks.truncate(index);
            },
            command if self.state.calls==0&&self.state.handler_locals.is_some()=>{
                let expressions=command_expressions(&command)?;self.tasks.push(Task::Command{statement:command,count:expressions.len(),ctx:ctx.clone()});
                for(index,expression)in expressions.into_iter().enumerate().rev(){self.tasks.push(Task::Expr(expression,Context{depth:0,..ctx.child(Route::Argument(index))}));}
            },
            _=>return Err(invalid("instruction non calculable")),
        }
        Ok(())
    }
}

fn index_error(index:usize,length:usize)->EvalError{EvalError::TypeMismatch{op:"index".into(),left:format!("index {index} out of bounds (len {length})"),right:String::new()}}
fn command_expressions(statement:&Statement)->EvalResult<Vec<Expr>> {
    Ok(match statement {
        Statement::WidgetAddToViewport{object,z_order}=>vec![object.clone(),z_order.clone()],Statement::WidgetRemoveFromParent{object}=>vec![object.clone()],
        Statement::UiOpen{name,arguments,modal,layer,..}=>vec![name.clone(),arguments.clone(),modal.clone(),layer.clone()],
        Statement::MenuExecute{request}=>vec![request.clone()],Statement::AccessibilityConfigure{settings}=>vec![settings.clone()],Statement::AccessibilitySpeak{text}=>vec![text.clone()],Statement::AccessibilityStop=>vec![],
        Statement::UiClose{name}|Statement::VideoPause{name}|Statement::VideoResume{name}|Statement::VideoStop{name}|Statement::VideoSkip{name}=>vec![name.clone()],
        Statement::UiFocus{name,element}=>vec![name.clone(),element.clone()],Statement::UiSetState{name,element,state}=>vec![name.clone(),element.clone(),state.clone()],
        Statement::MotionPlay{target,definition}=>vec![target.clone(),definition.clone()],Statement::MotionStop{target}=>vec![target.clone()],
        Statement::CharacterCompose{character,definition}=>vec![character.clone(),definition.clone()],Statement::CharacterAttributes{character,attributes}=>vec![character.clone(),attributes.clone()],
        Statement::VideoPlay{name,definition}=>vec![name.clone(),definition.clone()],Statement::VideoSeek{name,seconds}=>vec![name.clone(),seconds.clone()],Statement::VideoVolume{name,volume}=>vec![name.clone(),volume.clone()],
        _=>return Err(invalid("commande d’interface attendue")),
    })
}
pub(crate) fn replace_command_values(mut statement:Statement,values:Vec<Value>)->EvalResult<(Statement,HashMap<String,Value>)> {
    // Preserve the exact values, without reconstructing dict/list expressions
    // or re-running a selector. The scratch validator reads private bindings.
    let mut values=values.into_iter();let mut bindings=HashMap::new();let mut index=0;
    let mut next=||{let value=values.next().ok_or_else(||invalid("Debugger command value missing"))?;let name=format!("@debug-value-{index}");index+=1;bindings.insert(name.clone(),value);Ok(Expr::Var(name))};
    match &mut statement {
        Statement::WidgetAddToViewport{object,z_order}=>{*object=next()?;*z_order=next()?;},Statement::WidgetRemoveFromParent{object}=>*object=next()?,
        Statement::UiOpen{name,arguments,modal,layer,..}=>{*name=next()?;*arguments=next()?;*modal=next()?;*layer=next()?;},
        Statement::MenuExecute{request}=>*request=next()?,Statement::AccessibilityConfigure{settings}=>*settings=next()?,Statement::AccessibilitySpeak{text}=>*text=next()?,Statement::AccessibilityStop=>{},
        Statement::UiClose{name}|Statement::VideoPause{name}|Statement::VideoResume{name}|Statement::VideoStop{name}|Statement::VideoSkip{name}=>*name=next()?,
        Statement::UiFocus{name,element}=>{*name=next()?;*element=next()?;},Statement::UiSetState{name,element,state}=>{*name=next()?;*element=next()?;*state=next()?;},
        Statement::MotionPlay{target,definition}|Statement::CharacterCompose{character:target,definition}|Statement::VideoPlay{name:target,definition}=>{*target=next()?;*definition=next()?;},
        Statement::MotionStop{target}=>*target=next()?,Statement::CharacterAttributes{character,attributes}=>{*character=next()?;*attributes=next()?;},
        Statement::VideoSeek{name,seconds}=>{*name=next()?;*seconds=next()?;},Statement::VideoVolume{name,volume}=>{*name=next()?;*volume=next()?;},
        _=>return Err(invalid("commande d’interface attendue")),
    }
    if values.next().is_some(){return Err(invalid("Debugger command has excess values"));}Ok((statement,bindings))
}

#[cfg(test)]
#[path="eval_debugger_loop_tests_0770.rs"]
mod lexical_loop_tests0770;

#[cfg(test)]
#[path="eval_function_library_live_tests1023.rs"]
mod function_library_live1023;
