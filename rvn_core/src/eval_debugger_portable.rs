use super::*;
#[path="eval_debugger_macro_portable_1020.rs"]
pub(super) mod standard_macros_1020;
use serde::{Serialize,Deserialize};
pub(super) fn empty_revision()->crate::string_table::StringTableRevision {crate::string_table::StringTableResources::default().revision_stamp()}
pub(super) fn empty_root()->Root {Root::Block(Vec::new())}
pub(super) mod commands {
use super::*;
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
enum Command {
    Menu {
        request: rvn_ui::source_menus::MenuRequest,
        source: Option<String>,
    },
    AccessibilityConfigure {
        settings: rvn_ui::accessibility::AccessibilitySettings,
    },
    AccessibilitySpeak {
        text: String,
    },
    AccessibilityStop,
    VideoPlay {
        name: String,
        definition: rvn_ui::video::VideoClip,
    },
    VideoPause {
        name: String,
    },
    VideoResume {
        name: String,
    },
    VideoStop {
        name: String,
    },
    VideoSkip {
        name: String,
    },
    VideoSeek {
        name: String,
        seconds: f64,
    },
    VideoVolume {
        name: String,
        volume: f64,
    },
    CharacterCompose {
        character: String,
        definition: rvn_ui::composition::Composition,
    },
    CharacterAttributes {
        character: String,
        attributes: rvn_ui::composition::Attributes,
    },
    MotionPlay {
        target: crate::motion::MotionTarget,
        definition: rvn_ui::motion::Motion,
    },
    MotionStop {
        target: crate::motion::MotionTarget,
    },
    Open {
        name: String,
        arguments: Vec<Value>,
        modal: bool,
        layer: i32,
        host_role: Option<rvn_ui::PageRole>,
        inherit_host: bool,
    },
    Close {
        name: String,
    },
    WidgetRemoveFromParent { object: Value },
    Focus {
        name: String,
        element: String,
    },
    FocusTarget{name:String,target:rvn_ui::programmable::HostFocusTarget,user:u32},
    SetState {
        name: String,
        element: String,
        state: Value,
    },
}

impl From<&UiCommand> for Command {fn from(v:&UiCommand)->Self {match v {
UiCommand::Menu{request,source}=>Self::Menu{request:request.clone(),source:source.clone()},
UiCommand::AccessibilityConfigure{settings}=>Self::AccessibilityConfigure{settings:settings.clone()},
UiCommand::AccessibilitySpeak{text}=>Self::AccessibilitySpeak{text:text.clone()},
UiCommand::AccessibilityStop=>Self::AccessibilityStop,
UiCommand::VideoPlay{name,definition}=>Self::VideoPlay{name:name.clone(),definition:definition.clone()},
UiCommand::VideoPause{name}=>Self::VideoPause{name:name.clone()},
UiCommand::VideoResume{name}=>Self::VideoResume{name:name.clone()},
UiCommand::VideoStop{name}=>Self::VideoStop{name:name.clone()},
UiCommand::VideoSkip{name}=>Self::VideoSkip{name:name.clone()},
UiCommand::VideoSeek{name,seconds}=>Self::VideoSeek{name:name.clone(),seconds:seconds.clone()},
UiCommand::VideoVolume{name,volume}=>Self::VideoVolume{name:name.clone(),volume:volume.clone()},
UiCommand::CharacterCompose{character,definition}=>Self::CharacterCompose{character:character.clone(),definition:definition.clone()},
UiCommand::CharacterAttributes{character,attributes}=>Self::CharacterAttributes{character:character.clone(),attributes:attributes.clone()},
UiCommand::MotionPlay{target,definition}=>Self::MotionPlay{target:target.clone(),definition:definition.clone()},
UiCommand::MotionStop{target}=>Self::MotionStop{target:target.clone()},
UiCommand::Open{name,arguments,modal,layer,host_role,inherit_host}=>Self::Open{name:name.clone(),arguments:arguments.clone(),modal:modal.clone(),layer:layer.clone(),host_role:host_role.clone(),inherit_host:inherit_host.clone()},
UiCommand::Close{name}=>Self::Close{name:name.clone()},
UiCommand::WidgetRemoveFromParent{object}=>Self::WidgetRemoveFromParent{object:object.clone()},
UiCommand::Focus{name,element}=>Self::Focus{name:name.clone(),element:element.clone()},
UiCommand::FocusTarget{name,target,user}=>Self::FocusTarget{name:name.clone(),target:target.clone(),user:*user},
UiCommand::SetState{name,element,state}=>Self::SetState{name:name.clone(),element:element.clone(),state:state.clone()},
}}}
impl From<Command> for UiCommand {fn from(v:Command)->Self {match v {
Command::Menu{request,source}=>Self::Menu{request,source},
Command::AccessibilityConfigure{settings}=>Self::AccessibilityConfigure{settings},
Command::AccessibilitySpeak{text}=>Self::AccessibilitySpeak{text},
Command::AccessibilityStop=>Self::AccessibilityStop,
Command::VideoPlay{name,definition}=>Self::VideoPlay{name,definition},
Command::VideoPause{name}=>Self::VideoPause{name},
Command::VideoResume{name}=>Self::VideoResume{name},
Command::VideoStop{name}=>Self::VideoStop{name},
Command::VideoSkip{name}=>Self::VideoSkip{name},
Command::VideoSeek{name,seconds}=>Self::VideoSeek{name,seconds},
Command::VideoVolume{name,volume}=>Self::VideoVolume{name,volume},
Command::CharacterCompose{character,definition}=>Self::CharacterCompose{character,definition},
Command::CharacterAttributes{character,attributes}=>Self::CharacterAttributes{character,attributes},
Command::MotionPlay{target,definition}=>Self::MotionPlay{target,definition},
Command::MotionStop{target}=>Self::MotionStop{target},
Command::Open{name,arguments,modal,layer,host_role,inherit_host}=>Self::Open{name,arguments,modal,layer,host_role,inherit_host},
Command::Close{name}=>Self::Close{name},
Command::WidgetRemoveFromParent{object}=>Self::WidgetRemoveFromParent{object},
Command::Focus{name,element}=>Self::Focus{name,element},
Command::FocusTarget{name,target,user}=>Self::FocusTarget{name,target,user},
Command::SetState{name,element,state}=>Self::SetState{name,element,state},
}}}
pub fn serialize<S:serde::Serializer>(v:&[UiCommand],s:S)->Result<S::Ok,S::Error>{v.iter().map(Command::from).collect::<Vec<_>>().serialize(s)}
pub fn deserialize<'de,D:serde::Deserializer<'de>>(d:D)->Result<Vec<UiCommand>,D::Error>{Ok(Vec::<Command>::deserialize(d)?.into_iter().map(UiCommand::from).collect())}
}

// The wire contains cursors and data, never client Expr/Statement/ConsumerStep.
// All executable payloads are reconstructed from the current frozen root/library.
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskCursor {kind:String,ctx:Option<Context>,data:serde_json::Value}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableRecord {request:Request,result:ResultState}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableTrace {records:Vec<PortableRecord>,pending:PortableContinuation,#[serde(skip)] widget_values:Option<std::sync::Arc<crate::widget_reference_values_0974::WidgetValueAdmission>>}
#[path="eval_portable_widget_values_0974.rs"]
mod widget_values;
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableContinuation {request:Request,state:State,frames:Vec<Frame>,tasks:Vec<TaskCursor>,values:Vec<Value>}
#[path="eval_debugger_startup_input_0998.rs"]
mod startup_input;
#[path="eval_debugger_native_retained.rs"]
mod native_retained;

fn remap_portable_frame(value:&mut crate::consumer_schedule::ConsumerFrame,mapping:&BTreeMap<String,String>)->EvalResult<()> {
    let mut wire=serde_json::to_value(&*value).map_err(|_|invalid("Portable frame encoding failed"))?;
    let scopes=wire["scopes"].as_object_mut().ok_or_else(||invalid("Portable frame lost typed scopes"))?;
    let old=std::mem::take(scopes);
    for(scope,terminals)in old {
        let new=mapping.get(&scope).ok_or_else(||invalid("Portable root terminal has no authenticated source namespace"))?;
        if scopes.insert(new.clone(),terminals).is_some(){return Err(invalid("Portable namespace rebasing would merge distinct scopes"));}
    }
    *value=serde_json::from_value(wire).map_err(|_|invalid("Portable rebased frame is invalid"))?;Ok(())
}
fn remap_portable_active(context:&mut Option<ConsumerContext>,mapping:&BTreeMap<String,String>)->EvalResult<()> {
    if let Some(active)=context{active.scope=mapping.get(&active.scope).ok_or_else(||invalid("Portable root consumer has no authenticated source namespace"))?.clone();}
    Ok(())
}
fn remap_portable_state(value:&mut State,mapping:&BTreeMap<String,String>)->EvalResult<()> {
    remap_portable_frame(&mut value.consumer_frame,mapping)?;
    remap_portable_active(&mut value.active_consumer,mapping)
}
impl PortableRecord {
    // Private module boundary: callers supply the real before/after states
    // only from Computation::retain_native_consumer_input.
    pub(super) fn from_completed_native_input(request:Request,result:ResultState)->EvalResult<Self>{
        let value=Self{request,result};value.validate_loop_data()?;Ok(value)
    }

    /// Resource accounting only; this grants no source, owner or replay
    /// authority. Scene visits all active/parked records through one counter.
    pub(crate) fn account_native_loop_storage(&self,bytes:&mut usize,work:&mut usize)->EvalResult<()> {
        self.validate_loop_data()?;
        let cost=measured(&format_args!("{:?}{:?}",self.request,self.result))?;
        let next_bytes=bytes.checked_add(cost).filter(|size|*size<=TRACE_BYTES)
            .ok_or_else(||invalid("Retained narrative loop gathers exceed the existing trace storage budget"))?;
        let mut next_work=*work;
        for variables in [&self.request.variables,&self.result.variables]{
            for value in variables.values(){next_work=next_work.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;}
        }
        next_work=next_work.checked_add(crate::value_limits::value_work(self.result.value.as_ref().unwrap())?).ok_or(EvalError::NumericOverflow)?;
        if next_work>1_000_000{return Err(invalid("Retained narrative loop gathers exceed the existing continuation value budget"));}
        *bytes=next_bytes;*work=next_work;Ok(())
    }

    pub(crate) fn validate_loop_data(&self)->EvalResult<()> {
        if matches!(self.request.root,Root::Unit(..)|Root::LibraryCall(..)|Root::LibraryUnit(..))||self.result.unit_exit.is_some(){return Err(invalid("Unit records have no portable contract"));}
        measured(&format_args!("{:?}{:?}",self.request,self.result))?;
        validate_state(&self.request.state)?;validate_state(&self.result.state)?;
        if self.result.error.is_some()||self.result.value.is_none()||self.request.scope.is_some()
            ||self.request.state.active_consumer.is_some()||self.result.state.active_consumer.is_some()
            ||self.result.state.steps<self.request.state.steps||self.result.state.value_work<self.request.state.value_work
            ||self.result.state.calls!=self.request.state.calls||self.result.state.commands!=self.request.state.commands {
            return Err(invalid("Retained Init loop gather differs from a completed pure input root"));
        }
        let mut work=0usize;
        for variables in [&self.request.variables,&self.result.variables]{
            for(name,value)in variables{
                if !rvn_parser::is_variable_binding_name(name)&&!name.starts_with("persistent."){return Err(invalid("Retained Init loop gather has invalid variable data"));}
                work=work.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;
            }
        }
        work=work.checked_add(crate::value_limits::value_work(self.result.value.as_ref().unwrap())?).ok_or(EvalError::NumericOverflow)?;
        if work>1_000_000{return Err(invalid("Retained Init loop inputs exceed the existing continuation value budget"));}
        Ok(())
    }
    pub(crate) fn remap_loop_namespaces(&mut self,mapping:&BTreeMap<String,String>)->EvalResult<()> {
        self.validate_loop_data()?;
        remap_portable_state(&mut self.request.state,mapping)?;
        remap_portable_state(&mut self.result.state,mapping)
    }
    pub(crate) fn loop_ports_unbound(statement:&Statement,records:&[Self])->EvalResult<Value>{
        let scheduled=matches!(statement,Statement::ScheduledConsumer{..});
        let leaf=match statement{Statement::ScheduledConsumer{body,..}=>match body.as_slice(){[leaf]=>leaf,_=>return Err(invalid("Retained Init header has no unique frozen body"))},leaf=>leaf};
        let native=rvn_parser::scheduled_source::native_loop_contract::binding(statement).map_err(EvalError::InvalidFunction)?;
        let count=if let Some(binding)=&native{if binding.kind==rvn_parser::scheduled_source::native_loop_contract::Kind::Range{2}else{1}}else if scheduled{1}else if matches!(leaf,Statement::ForLoopWithBreak{..}){2}else{1};
        if records.len()!=count{return Err(invalid("Retained Init loop has a different number of input roots"));}
        for record in records{record.validate_loop_data()?;}
        if native.is_some(){return Ok(records.last().unwrap().result.value.as_ref().unwrap().clone());}
        if scheduled{return Ok(records[0].result.value.as_ref().unwrap().clone());}
        match leaf{
            Statement::While{..}=>Ok(records[0].result.value.as_ref().unwrap().clone()),
            Statement::ForEach{..}|Statement::ForLoopWithBreak{..}=>Ok(Value::List(records.iter().map(|record|record.result.value.as_ref().unwrap().clone()).collect())),
            _=>Err(invalid("Retained input data is not bound to an actual Init loop")),
        }
    }
}
impl Probe {
    /// Extract only the actual First root which debugger_root has just matched
    /// and consumed. Later retained Test roots need not have been replayed yet.
    /// This does not complete/seal the transaction or advance its trace cursor.
    pub(crate) fn completed_native_first_record(probe:&SharedProbe,route:&[Route])->EvalResult<PortableRecord>{
        use rvn_parser::scheduled_source::native_loop_contract::{self as contract,Kind,Phase};
        let probe=probe.lock().map_err(|_|invalid("Init gather trace lock unavailable"))?;
        if probe.stale||probe.requested.is_some(){return Err(invalid("Native First requires its consumed authentic root"));}
        let record=probe.cursor.checked_sub(1).and_then(|index|probe.records.get(index))
            .ok_or_else(||invalid("Native First has not been consumed"))?;
        if record.request.route_prefix!=route{return Err(invalid("Native First belongs to another source route"));}
        let Root::Consumer(statement,_) = &record.request.root else{return Err(invalid("Native First is not an actual consumer root"));};
        let binding=contract::binding(statement).map_err(EvalError::InvalidFunction)?
            .ok_or_else(||invalid("Native First has no explicit source binding"))?;
        if binding.kind!=Kind::Range||contract::owned_engine_phase(&binding,&record.request.variables).map_err(EvalError::InvalidFunction)?!=Phase::First{
            return Err(invalid("Consumed Native root is not FirstIndex"));
        }
        measured(&format_args!("{record:?}"))?;
        let row=PortableRecord{request:record.request.clone(),result:record.result.clone()};
        row.validate_loop_data()?;Ok(row)
    }
    pub(crate) fn completed_loop_records(probe:&SharedProbe,route:&[Route])->EvalResult<Vec<PortableRecord>>{
        let probe=probe.lock().map_err(|_|invalid("Init gather trace lock unavailable"))?;
        if probe.stale||probe.requested.is_some()||probe.cursor!=probe.records.len(){return Err(invalid("Init loop inputs require their actual completed trace"));}
        let records=probe.records.iter().filter(|record|record.request.route_prefix==route).collect::<Vec<_>>();
        if records.is_empty()||records.len()>2{return Err(invalid("Init loop gather lost its bounded actual input roots"));}
        measured(&format_args!("{records:?}"))?;
        let mut result=Vec::new();
        for record in records{
            let row=PortableRecord{request:record.request.clone(),result:record.result.clone()};row.validate_loop_data()?;result.push(row);
        }
        Ok(result)
    }
}
impl FunctionLibrary {
    pub(crate) fn validate_retained_loop_inputs(&self,statement:&Statement,scope:Option<&str>,route:&[Route],records:&[PortableRecord],locale:Option<&crate::locale::LocaleManager>)->EvalResult<Value>{
        let roots=match statement{
            Statement::ScheduledConsumer{version:3,..}=>{
                let binding=rvn_parser::scheduled_source::native_loop_contract::binding(statement).map_err(EvalError::InvalidFunction)?.ok_or_else(||invalid("Missing source v3 binding"))?;
                let root=Root::Consumer(statement.clone(),scope.ok_or_else(||invalid("Saved Native phase has no actual bound source scope"))?.into());
                vec![root;if binding.kind==rvn_parser::scheduled_source::native_loop_contract::Kind::Range{2}else{1}]
            },
            Statement::ScheduledConsumer{version:2,body,..}if matches!(body.as_slice(),[Statement::While{..}|Statement::ForEach{..}|Statement::ForLoopWithBreak{..}])=>{
                vec![Root::Consumer(statement.clone(),scope.ok_or_else(||invalid("Saved loop consumer has no actual bound source scope"))?.into())]
            },
            Statement::While{condition,..}=>vec![Root::Expression(condition.clone(),0)],
            Statement::ForEach{collection,..}=>vec![Root::Expression(collection.clone(),0)],
            Statement::ForLoopWithBreak{first,last,..}=>vec![Root::Expression(first.clone(),0),Root::Expression(last.clone(),0)],
            _=>return Err(invalid("Retained loop inputs have no actual frozen header")),
        };
        if roots.len()!=records.len(){return Err(invalid("Saved loop input root count differs from source"));}
        for(index,(root,record))in roots.into_iter().zip(records).enumerate(){
            record.validate_loop_data()?;
            if record.request.route_prefix!=route{return Err(invalid("Saved loop gather belongs to another actual source route"));}
            if !matches!(record.request.root,Root::Block(ref body)if body.is_empty())&&record.request.root!=root{
                return Err(invalid("Retained live loop root differs from its frozen header"));
            }
            if index>0 && !matches!(statement,Statement::ScheduledConsumer{version:3,..}){
                let prior=&records[index-1];
                if record.request.variables!=prior.request.variables||record.request.state.globals!=prior.request.state.globals
                    ||record.request.state.random!=prior.result.state.random{return Err(invalid("Saved range input roots lost their actual ordered context"));}
            }
            // Rebuild only the source payload. Variables/results remain inert
            // Save15 data, admitted with the same completed-record validator.
            let mut row=record.clone();row.request.root=root;
            rebind_state(&mut row.request.state,self,locale);rebind_state(&mut row.result.state,self,locale);
            let mut library=self.clone();library.debug_probe=None;
            let current=Continuation::new(library,row.request.clone())?;
            let probe=Probe::shared();
            probe.lock().map_err(|_|invalid("Loop record validation lock unavailable"))?.requested=Some(current.request.clone());
            Probe::restore_record_with_scope(&probe,&current,&row,locale,None)?;
        }
        PortableRecord::loop_ports_unbound(statement,records)
    }
}
impl PortableTrace {
    pub(crate) fn records(&self)->&[PortableRecord]{&self.records}
}
fn cursor(kind:&str,ctx:Option<&Context>,data:serde_json::Value)->TaskCursor {TaskCursor{kind:kind.into(),ctx:ctx.cloned(),data}}
fn task_cursor(task:&Task)->EvalResult<TaskCursor> {
    use serde_json::json;
    Ok(match task {
        Task::UnitStart{..}|Task::UnitArguments{..}|Task::UnitEnd{..}|Task::LibraryCall{..}|Task::LibraryUnitStart{..}|Task::LibraryUnitArguments{..}=>return Err(invalid("Unit tasks have no portable cursor contract")),
        Task::InitializerEntry(ctx)=>cursor("initializer_entry",Some(ctx),json!(null)),
        Task::Expr(_,ctx)=>cursor("expr",Some(ctx),json!(null)),
        Task::Statement(_,ctx)=>cursor("statement",Some(ctx),json!(null)),
        Task::MacroOperandNext{need,index,ctx,..}=>cursor("macro_operand_next_v1",Some(ctx),json!((need,index))),
        Task::MacroReady{need,ctx,..}=>cursor("macro_ready",Some(ctx),json!(need)),
        Task::Block(_,index,ctx)=>cursor("block",Some(ctx),json!(index)),
        Task::Charge(ctx)=>cursor("charge",Some(ctx),json!(null)),Task::Neg(ctx)=>cursor("neg",Some(ctx),json!(null)),
        Task::Not(ctx)=>cursor("not",Some(ctx),json!(null)),Task::Bool(ctx)=>cursor("bool",Some(ctx),json!(null)),
        Task::Binary(_,ctx)=>cursor("binary",Some(ctx),json!(null)),Task::Index(ctx)=>cursor("index",Some(ctx),json!(null)),Task::List(_,ctx)=>cursor("list",Some(ctx),json!(null)),
        Task::Short{ctx,..}=>cursor("short",Some(ctx),json!(null)),Task::Call{ctx,..}=>cursor("call",Some(ctx),json!(null)),
        Task::FunctionEnd{frame,base,..}=>cursor("function_end",None,json!([frame,base])),
        Task::Assign{ctx,..}=>cursor("assign",Some(ctx),json!(null)),Task::Return(ctx)=>cursor("return",Some(ctx),json!(null)),
        Task::Branch{ctx,..}=>cursor("branch",Some(ctx),json!(null)),Task::While{ctx,..}=>cursor("while",Some(ctx),json!(null)),Task::WhileReady{ctx,..}=>cursor("while_ready",Some(ctx),json!(null)),
        Task::EachReady{ctx,..}=>cursor("each_ready",Some(ctx),json!(null)),Task::EachNext{items,ctx,..}=>cursor("each_next",Some(ctx),json!(items.as_slice())),
        Task::RangeReady{ctx,..}=>cursor("range_ready",Some(ctx),json!(null)),Task::RangeExit(ctx)=>cursor("range_exit",Some(ctx),json!(null)),
        Task::RangeNext{first,count,cursor:c,ctx,..}=>cursor("range_next",Some(ctx),json!([first,count,c])),
        Task::Command{ctx,..}=>cursor("command",Some(ctx),json!(null)),
        Task::CurveNext{name,samples,cursor:c,values,random,ctx}=>cursor("curve_next",Some(ctx),json!([name,samples,c,values,random])),
        Task::CurveValue{name,samples,cursor:c,values,random,ctx}=>cursor("curve_value",Some(ctx),json!([name,samples,c,values,random])),
        Task::NativeLoopResume{state,ctx,..}=>cursor("native_loop_resume",Some(ctx),json!(state)),
        Task::NativeLoopHeader{phase,ctx}=>cursor("native_loop_header",Some(ctx),json!(phase)),
        Task::ConsumerNext{index,ctx,..}=>cursor("consumer_next",Some(ctx),json!(index)),
        Task::ConsumerCapture{ctx,..}=>cursor("consumer_capture",Some(ctx),json!(null)),Task::ConsumerInspect{ctx,..}=>cursor("consumer_inspect",Some(ctx),json!(null)),
        Task::ConsumerInputs{ctx,..}=>cursor("consumer_inputs",Some(ctx),serde_json::Value::Null),
        Task::ConsumerDialogueInput{ctx,..}=>cursor("consumer_dialogue_input",Some(ctx),serde_json::Value::Null),
        Task::ConsumerBranch{ctx,..}=>cursor("consumer_branch",Some(ctx),serde_json::Value::Null),
        Task::ConsumerLoop{ctx,..}=>cursor("consumer_loop",Some(ctx),serde_json::Value::Null),
        Task::ConsumerIteration{ctx,..}=>cursor("consumer_iteration",Some(ctx),serde_json::Value::Null),
        Task::ConsumerChoice{index,rows,ctx,..}=>cursor("consumer_choice",Some(ctx),json!([index,rows])),
        Task::ConsumerChoiceSelected{index,rows,ctx,..}=>cursor("consumer_choice_selected",Some(ctx),json!([index,rows])),
        Task::ConsumerChoiceLabel{index,rows,ctx,..}=>cursor("consumer_choice_label",Some(ctx),json!([index,rows])),
        Task::ConsumerEnd{ctx,..}=>cursor("consumer_end",Some(ctx),json!(null)),Task::RootEnd=>cursor("root_end",None,json!(null)),
    })
}
#[derive(Clone,Copy)]
enum Node<'a> {Block(&'a [Statement]),Statement(&'a Statement),Expr(&'a Expr),Call(&'a str,usize),Choice(&'a rvn_parser::ChoiceOption)}
impl<'a> Node<'a> {
    fn expr(self)->EvalResult<&'a Expr>{match self{Self::Expr(v)=>Ok(v),Self::Statement(Statement::SetVar{value,..}|Statement::LocalVar{value,..}|Statement::FunctionReturn{value})=>Ok(value),_=>Err(invalid("Continuation route is not an actual expression"))}}
    fn statement(self)->EvalResult<&'a Statement>{match self{Self::Statement(v)=>Ok(v),_=>Err(invalid("Continuation route is not an actual statement"))}}
    fn block(self)->EvalResult<&'a [Statement]>{match self{Self::Block(v)=>Ok(v),_=>Err(invalid("Continuation route is not an actual block"))}}
}
fn node_at<'a>(continuation:&'a Continuation,frames:&[Frame],ctx:&Context)->EvalResult<Node<'a>> {
    if ctx.depth>256||ctx.route.len()>512{return Err(invalid("Continuation route/depth exceeds its bound"));}
    let frame=frames.get(ctx.frame).ok_or_else(||invalid("Continuation has no actual lexical owner"))?;
    let mut node=if ctx.frame==0 {
        match &continuation.request.root {Root::Expression(v,_)=>Node::Expr(v),Root::Block(v)=>Node::Block(v),Root::Call(name,args)=>Node::Call(name,args.len()),Root::Consumer(statement,_)=>Node::Statement(statement),Root::Unit(..)|Root::LibraryCall(..)|Root::LibraryUnit(..)=>return Err(invalid("Unit root cannot be hydrated from portable cursors"))}
    }else {
        let name=frame.function.as_ref().ok_or_else(||invalid("Saved function frame has no definition"))?;
        Node::Block(&continuation.library.legacy_definition(name).ok_or_else(||invalid("Saved function definition no longer exists"))?.1)
    };
    let route=if ctx.frame==0{ctx.route.strip_prefix(continuation.request.route_prefix.as_slice()).ok_or_else(||invalid("Initializer cursor source prefix differs"))?}else{ctx.route.as_slice()};
    for route in route {
        node=match(node,route) {
            (Node::Block(body),Route::Statement(index))=>Node::Statement(body.get(*index).ok_or_else(||invalid("Continuation statement ordinal is outside actual block"))?),
            (Node::Statement(Statement::StandardMacroDispatch{dispatch}),Route::Producer(index))=>Node::Expr(match dispatch.operand_steps().get(*index).ok_or_else(||invalid("Macro producer ordinal differs from actual deferred plan"))?{rvn_parser::ConsumerStep::Capture{value,..}|rvn_parser::ConsumerStep::InspectTextTable{value,..}=>value}),
            (Node::Statement(Statement::ScheduledConsumer{steps,..}),Route::Producer(index))=>Node::Expr(match steps.get(*index).ok_or_else(||invalid("Continuation producer ordinal differs from actual plan"))? {rvn_parser::ConsumerStep::Capture{value,..}|rvn_parser::ConsumerStep::InspectTextTable{value,..}=>value}),
            (Node::Statement(Statement::ScheduledConsumer{body,..}|Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}),Route::Body)=>Node::Block(body),
            (Node::Statement(Statement::If{then_branch,..}),Route::Then)=>Node::Block(then_branch),
            (Node::Statement(Statement::If{else_branch,..}),Route::Else)=>Node::Block(else_branch),
            (Node::Statement(Statement::Sequence{branches}),Route::Branch(index))=>Node::Block(branches.get(*index).ok_or_else(||invalid("Continuation sequence ordinal differs"))?),
            (Node::Statement(Statement::StandardMacroDispatch{dispatch}),Route::Branch(index))=>Node::Block(&dispatch.branches.get(*index).ok_or_else(||invalid("Macro output ordinal differs from actual source"))?.body),
            (Node::Statement(Statement::StandardMacroDispatch{dispatch}),Route::Argument(0))=>Node::Expr(dispatch.operand.as_ref().ok_or_else(||invalid("Macro has no operand at this control"))?),
            (Node::Statement(Statement::Choice{options}),Route::Option(index))=>Node::Choice(options.get(*index).ok_or_else(||invalid("Choice option ordinal differs from frozen display"))?),
            (Node::Choice(option),Route::Condition)=>Node::Expr(option.condition.as_ref().ok_or_else(||invalid("Choice option has no authored condition"))?),
            (Node::Choice(option),Route::Argument(index))=>Node::Expr(option.label.0.iter().filter_map(|p|if let rvn_parser::TextSegment::Interp(e)=p{Some(e)}else{None}).nth(*index).ok_or_else(||invalid("Choice label port differs from actual interpolation"))?),
            (Node::Statement(Statement::If{condition,..}|Statement::While{condition,..}),Route::Condition)=>Node::Expr(condition),
            (Node::Statement(Statement::ForEach{collection,..}),Route::Collection)=>Node::Expr(collection),
            (Node::Statement(Statement::ForLoopWithBreak{first,..}),Route::First)=>Node::Expr(first),
            (Node::Statement(Statement::ForLoopWithBreak{last,..}),Route::Last)=>Node::Expr(last),
            (Node::Statement(command),Route::Argument(index)) if !matches!(command,Statement::SetVar{..}|Statement::LocalVar{..}|Statement::FunctionReturn{..})=>Node::Expr(command_expression_at(command,*index)?),
            (parent,route)=>{
                let expression=parent.expr()?;
                Node::Expr(match(expression,route) {
                    (Expr::Neg(v)|Expr::Not(v),Route::Unary)=>v,
                    (Expr::BinOp{left,..}|Expr::And(left,_)|Expr::Or(left,_),Route::Left)=>left,
                    (Expr::BinOp{right,..}|Expr::And(_,right)|Expr::Or(_,right),Route::Right)=>right,
                    (Expr::Call{args,..}|Expr::ListLit(args),Route::Argument(index))=>args.get(*index).ok_or_else(||invalid("Continuation argument is outside actual expression"))?,
                    (Expr::Index{target,..},Route::Collection)=>target,(Expr::Index{index,..},Route::Index)=>index,
                    _=>return Err(invalid("Continuation route differs from actual programme")),
                })
            }
        }
    }
    Ok(node)
}
fn command_expression_at(statement:&Statement,index:usize)->EvalResult<&Expr>{
    let expressions:Vec<&Expr>=match statement {
        Statement::Dialogue{text,..}=>text.0.iter().filter_map(|p|if let rvn_parser::TextSegment::Interp(expr)=p{Some(expr)}else{None}).collect(),
        Statement::MusicVolumeValue{level}=>vec![level],
        Statement::TypewriterSetValue{enabled}=>vec![enabled],
        Statement::TypewriterSpeedValue{chars_per_sec}=>vec![chars_per_sec],
        Statement::MotionWait{target}=>vec![target],Statement::VideoWait{name}=>vec![name],
        Statement::WidgetAddToViewport{object,z_order}=>vec![object,z_order],Statement::WidgetRemoveFromParent{object}=>vec![object],
        Statement::UiOpen{name,arguments,modal,layer,..}=>vec![name,arguments,modal,layer],
        Statement::MenuExecute{request}=>vec![request],Statement::AccessibilityConfigure{settings}=>vec![settings],Statement::AccessibilitySpeak{text}=>vec![text],Statement::AccessibilityStop=>vec![],
        Statement::UiClose{name}|Statement::VideoPause{name}|Statement::VideoResume{name}|Statement::VideoStop{name}|Statement::VideoSkip{name}=>vec![name],
        Statement::UiFocus{name,element}=>vec![name,element],Statement::UiSetState{name,element,state}=>vec![name,element,state],
        Statement::MotionPlay{target,definition}=>vec![target,definition],Statement::MotionStop{target}=>vec![target],
        Statement::VideoPlay{name,definition}=>vec![name,definition],Statement::VideoSeek{name,seconds}=>vec![name,seconds],Statement::VideoVolume{name,volume}=>vec![name,volume],
        Statement::CharacterCompose{character,definition}=>vec![character,definition],Statement::CharacterAttributes{character,attributes}=>vec![character,attributes],
        _=>return Err(invalid("Continuation command has no actual expression route")),
    };expressions.get(index).copied().ok_or_else(||invalid("Continuation command argument differs from source"))
}
fn runtime_scope(current:&Continuation,ctx:&Context,scope:String)->String {if ctx.frame==0&&ctx.route==current.request.route_prefix{if let Root::Consumer(_,bound)=&current.request.root{return bound.clone();}}scope}
fn read_data<T:serde::de::DeserializeOwned>(value:&serde_json::Value)->EvalResult<T>{serde_json::from_value(value.clone()).map_err(|_|invalid("Invalid continuation cursor data"))}
fn task_from_cursor(current:&Continuation,frames:&[Frame],wire:&TaskCursor)->EvalResult<Task> {
    let ctx=wire.ctx.clone();
    if let Some(ctx)=&ctx{current.validate_native_phase_task_route(&wire.kind,ctx)?;}
    let source=match &ctx{Some(ctx)=>Some(node_at(current,frames,ctx)?),None=>None};
    let context=||ctx.clone().ok_or_else(||invalid("Continuation task lost its actual source route"));
    let statement=||source.clone().ok_or_else(||invalid("Missing actual statement route"))?.statement();
    let expression=||source.clone().ok_or_else(||invalid("Missing actual expression route"))?.expr();
    let task=match wire.kind.as_str() {
        "expr"=>{
            if let Root::Consumer(authored,_)=&current.request.root {
                if rvn_parser::scheduled_source::dialogue_text_binding(authored).map_err(EvalError::InvalidFunction)?.is_some() {
                    let mut slot=current.request.route_prefix.clone();slot.extend([Route::Body,Route::Statement(0),Route::Argument(0)]);
                    if context()?.route.starts_with(&slot){return Err(invalid("Bound Dialogue input cannot resume as an authored expression evaluation"));}
                }
            }
            Task::Expr(expression()?.clone(),context()?)
        },"statement"=>Task::Statement(statement()?.clone(),context()?),
        "block"=>{let block=source.ok_or_else(||invalid("Missing actual block"))?.block()?;let index:usize=read_data(&wire.data)?;if index>block.len(){return Err(invalid("Continuation block cursor is outside actual block"));}Task::Block(block.to_vec(),index,context()?)},
        "initializer_entry"=>{let ctx=context()?;if ctx.frame!=0||current.request.route_prefix.is_empty()||ctx.route!=current.request.route_prefix||!current.library.debug_origins.iter().any(|origin|origin.scope.is_empty()&&origin.route==ctx.route&&origin.origin.scheduled_step.is_none()){return Err(invalid("Initializer entry cursor is not the current actual source statement"));}Task::InitializerEntry(ctx)},
        "charge"=>{expression()?;Task::Charge(context()?)},
        "neg"=>{if !matches!(expression()?,Expr::Neg(_)){return Err(invalid("Negation cursor differs from source"));}Task::Neg(context()?)},
        "not"=>{if !matches!(expression()?,Expr::Not(_)){return Err(invalid("Boolean cursor differs from source"));}Task::Not(context()?)},
        "bool"=>{if !matches!(expression()?,Expr::And(..)|Expr::Or(..)){return Err(invalid("Short circuit cursor differs from source"));}Task::Bool(context()?)},
        "binary"=>{let Expr::BinOp{op,..}=expression()?else{return Err(invalid("Binary cursor differs from source"));};Task::Binary(op.clone(),context()?)},
        "index"=>{if !matches!(expression()?,Expr::Index{..}){return Err(invalid("Index cursor differs from source"));}Task::Index(context()?)},
        "list"=>{let Expr::ListLit(items)=expression()?else{return Err(invalid("List cursor differs from source"));};Task::List(items.len(),context()?)},
        "short"=>{let(right,and)=match expression()?{Expr::And(_,right)=>(right.as_ref().clone(),true),Expr::Or(_,right)=>(right.as_ref().clone(),false),_=>return Err(invalid("Short circuit cursor differs from source"))};Task::Short{right,and,ctx:context()?}},
        "call"=>{let(name,count)=match source.ok_or_else(||invalid("Missing actual call"))? {Node::Call(name,count)=>(name.to_owned(),count),parent=>match parent.expr()?{Expr::Call{name,args}=>(name.clone(),args.len()),_=>return Err(invalid("Call cursor differs from source"))}};Task::Call{name,count,ctx:context()?}},
        "function_end"=>{let [frame,base]:[usize;2]=read_data(&wire.data)?;if frame==0||base>1_000_000{return Err(invalid("Invalid parked function cursor"));}let name=frames.get(frame).and_then(|frame|frame.function.clone()).ok_or_else(||invalid("Function end has no saved invocation"))?;Task::FunctionEnd{frame,base,name}},
        "assign"=>{let(name,local)=match statement()?{Statement::SetVar{name,..}=>(name.clone(),false),Statement::LocalVar{name,..}=>(name.clone(),true),_=>return Err(invalid("Assignment cursor differs from actual statement"))};let ctx=context()?;Task::Assign{name,local,frame:ctx.frame,ctx}},
        "return"=>{if !matches!(statement()?,Statement::FunctionReturn{..}){return Err(invalid("Return cursor differs from actual statement"));}Task::Return(context()?)},
        "branch"=>{let Statement::If{then_branch:yes,else_branch:no,..}=statement()?else{return Err(invalid("Branch cursor differs"));};Task::Branch{yes:yes.clone(),no:no.clone(),ctx:context()?}},
        "while"|"while_ready"=>{let Statement::While{condition,body}=statement()?else{return Err(invalid("Loop cursor differs"));};let ctx=context()?;if wire.kind=="while"{Task::While{condition:condition.clone(),body:body.clone(),ctx}}else{Task::WhileReady{condition:condition.clone(),body:body.clone(),ctx}}},
        "each_ready"|"each_next"=>{let Statement::ForEach{name,body,..}=statement()?else{return Err(invalid("Each cursor differs"));};let ctx=context()?;if wire.kind=="each_ready"{Task::EachReady{name:name.clone(),body:body.clone(),ctx}}else{let items:Vec<Value>=read_data(&wire.data)?;Task::EachNext{name:name.clone(),body:body.clone(),items:items.into_iter(),ctx}}},
        "range_ready"|"range_next"|"range_exit"=>{let Statement::ForLoopWithBreak{name,body,..}=statement()?else{return Err(invalid("Range cursor differs"));};let ctx=context()?;match wire.kind.as_str(){"range_ready"=>Task::RangeReady{name:name.clone(),body:body.clone(),ctx},"range_exit"=>Task::RangeExit(ctx),_=>{let[first,count,cursor]:[i64;3]=read_data(&wire.data)?;if count<0||cursor<0||cursor>count||count>100_000||first.checked_add(count.saturating_sub(1)).is_none(){return Err(invalid("Invalid bounded range continuation"));}Task::RangeNext{name:name.clone(),body:body.clone(),first,count,cursor,ctx}}}},
        "command"=>{let statement=statement()?;let count=command_expressions(&statement)?.len();Task::Command{statement:statement.clone(),count,ctx:context()?}},
        "curve_next"|"curve_value"=>{let(name,samples,cursor,values,random):(String,usize,usize,Vec<f32>,RandomState)=read_data(&wire.data)?;if !matches!(expression()?,Expr::Call{name,..}if name=="motion_curve")||!current.library.has_function(&name,1)||!(2..=257).contains(&samples)||cursor>samples||values.len()!=cursor||values.iter().any(|v|!v.is_finite()){return Err(invalid("Invalid saved curve candidate"));}let ctx=context()?;if wire.kind=="curve_next"{Task::CurveNext{name,samples,cursor,values,random,ctx}}else{Task::CurveValue{name,samples,cursor,values,random,ctx}}},
        "native_loop_resume"=>{let authored=statement()?.clone();let state:LexicalLoopState=read_data(&wire.data)?;state.validate(&authored)?;Task::NativeLoopResume{statement:authored,state,ctx:context()?}},
        "native_loop_header"=>{let authored=statement()?;if rvn_parser::scheduled_source::native_loop_contract::binding(authored).map_err(EvalError::InvalidFunction)?.is_none(){return Err(invalid("Native header lost its frozen statement"));}let phase:LoopPhase=read_data(&wire.data)?;if phase==LoopPhase::Body{return Err(invalid("Native body cannot own a header result"));}Task::NativeLoopHeader{phase,ctx:context()?}},
        "consumer_next"=>{let Statement::ScheduledConsumer{version,consumer,scope,steps,body}=statement()?else{return Err(invalid("Consumer cursor has no frozen plan"));};let index:usize=read_data(&wire.data)?;if index>steps.len(){return Err(invalid("Consumer producer cursor is outside frozen plan"));}Task::ConsumerNext{version:*version,consumer:consumer.clone(),scope:runtime_scope(current,&context()?,scope.clone()),steps:steps.clone(),index,body:body.clone(),value_only:matches!(current.request.root,Root::Consumer(..))&&context()?.frame==0&&context()?.route==current.request.route_prefix,ctx:context()?}},
        "consumer_capture"|"consumer_inspect"=>{let mut ctx=context()?;let Some(Route::Producer(index))=ctx.route.pop()else{return Err(invalid("Consumer terminal cursor lost its producer ordinal"));};let authored=node_at(current,frames,&ctx)?;let(scope,steps)=match authored.statement()?{Statement::ScheduledConsumer{scope,steps,..}=>(scope.clone(),steps.as_slice()),Statement::StandardMacroDispatch{dispatch}if dispatch.deferred_operand_v1.is_some()=>(dispatch.site.scope_source(),dispatch.operand_steps()),_=>return Err(invalid("Consumer terminal cursor has no frozen plan"))};let step=steps.get(index).ok_or_else(||invalid("Consumer producer ordinal differs"))?;let ctx=context()?;match(step,wire.kind.as_str()){(rvn_parser::ConsumerStep::Capture{producer,..},"consumer_capture")=>Task::ConsumerCapture{producer:producer.clone(),ctx},(rvn_parser::ConsumerStep::InspectTextTable{producer,..},"consumer_inspect")=>Task::ConsumerInspect{producer:producer.clone(),scope:runtime_scope(current,&Context{route:ctx.route[..ctx.route.len()-1].to_vec(),..ctx.clone()},scope.clone()),ctx},_=>return Err(invalid("Consumer terminal kind differs from frozen producer"))}},
        "consumer_dialogue_input"=>{
            let ctx=context()?;
            if !wire.data.is_null()||!matches!(current.request.root,Root::Consumer(..))||ctx.frame!=0||ctx.route!=current.request.route_prefix {
                return Err(invalid("Dynamic Dialogue cursor differs from its actual consumer root"));
            }
            let binding=rvn_parser::scheduled_source::dialogue_text_binding(statement()?).map_err(EvalError::InvalidFunction)?
                .ok_or_else(||invalid("Dynamic Dialogue cursor has no frozen bound source input"))?;
            Task::ConsumerDialogueInput{producer:binding.producer,ctx}
        },
        "macro_operand_next_v1"=>{
            let Statement::StandardMacroDispatch{dispatch}=statement()? else{return Err(invalid("Deferred macro cursor has no frozen source dispatch"));};
            dispatch.validate().map_err(EvalError::InvalidFunction)?;
            let(need,index):(crate::widget_class_source_0950::standard_macros::OperandNeed,usize)=read_data(&wire.data)?;
            if dispatch.deferred_operand_v1.is_none()||index>dispatch.operand_steps().len(){return Err(invalid("Deferred macro cursor exceeds actual source phase"));}
            Task::MacroOperandNext{dispatch:(**dispatch).clone(),need,index,ctx:context()?}
        },
        "macro_ready"=>{
            let Statement::StandardMacroDispatch{dispatch}=statement()? else{return Err(invalid("Macro operand cursor has no actual source dispatch"));};
            Task::MacroReady{dispatch:(**dispatch).clone(),need:read_data(&wire.data)?,ctx:context()?}
        },
        "consumer_inputs"=>{if matches!(statement()?,Statement::ScheduledConsumer{version:3,..}){return Err(invalid("Native phase cannot restore a snapshot tuple input"));}if rvn_parser::scheduled_source::dialogue_text_binding(statement()?).map_err(EvalError::InvalidFunction)?.is_some(){return Err(invalid("Bound Dialogue must restore its retained port transfer"));}let Statement::ScheduledConsumer{body,..}=statement()?else{return Err(invalid("Input cursor lost real consumer"));};if !matches!(current.request.root,Root::Consumer(..))||body.first().is_some_and(crate::consumer_schedule::scalar_engine_input){return Err(invalid("Input cursor differs from actual direct consumer"));}Task::ConsumerInputs{count:body.first().map(crate::consumer_schedule::direct_expressions).unwrap_or_default().len(),ctx:context()?}},
        "consumer_branch"=>{let Statement::ScheduledConsumer{body,..}=statement()?else{return Err(invalid("Branch cursor lost real consumer"));};let[Statement::If{then_branch,else_branch,..}]=body.as_slice()else{return Err(invalid("Branch cursor differs from actual consumer"));};if matches!(current.request.root,Root::Consumer(..))&&context()?.frame==0{return Err(invalid("Engine root selects its prepared branch at publication"));}Task::ConsumerBranch{yes:then_branch.clone(),no:else_branch.clone(),ctx:context()?}},
        "consumer_end"=>{if !match statement()?{Statement::ScheduledConsumer{..}=>true,Statement::StandardMacroDispatch{dispatch}=>dispatch.deferred_operand_v1.is_some(),_=>false}{return Err(invalid("Consumer end has no frozen plan"));}let ctx=context()?;Task::ConsumerEnd{frame:ctx.frame,ctx}},
        "consumer_loop"=>{let statement=statement()?;if !matches!(statement,Statement::ScheduledConsumer{version:2,body,..}if matches!(body.as_slice(),[Statement::While{..}]))||matches!(current.request.root,Root::Consumer(..))&&context()?.frame==0{return Err(invalid("Loop cursor differs from actual lexical consumer"));}Task::ConsumerLoop{statement:statement.clone(),ctx:context()?}},
        "consumer_iteration"=>{let Statement::ScheduledConsumer{version:2,body,..}=statement()?else{return Err(invalid("Iterator cursor lost its actual consumer"));};if matches!(current.request.root,Root::Consumer(..))&&context()?.frame==0{return Err(invalid("Narrative iterator requires its prepared bounds instruction"));}let(name,body,range)=match body.as_slice(){[Statement::ForEach{name,body,..}]=>(name,body,false),[Statement::ForLoopWithBreak{name,body,..}]=>(name,body,true),_=>return Err(invalid("Iterator cursor differs from frozen AST"))};Task::ConsumerIteration{name:name.clone(),body:body.clone(),range,ctx:context()?}},
        "consumer_choice"|"consumer_choice_selected"|"consumer_choice_label"=>{
            let Statement::ScheduledConsumer{version:2,body,..}=statement()?else{return Err(invalid("Choice cursor lost actual consumer"));};let[Statement::Choice{options}]=body.as_slice()else{return Err(invalid("Choice cursor differs from frozen AST"));};
            if !matches!(current.request.root,Root::Consumer(..))||context()?.frame!=0{return Err(invalid("Choice needs its real narrative input root"));}
            let(index,rows):(usize,Vec<Value>)=read_data(&wire.data)?;if index>options.len()||rows.len()!=index||wire.kind!="consumer_choice"&&index==options.len(){return Err(invalid("Choice cursor is outside actual option phase"));}
            for(row,option)in rows.iter().zip(options){let Value::List(fields)=row else{return Err(invalid("Choice cursor row shape differs"));};let[Value::Bool(visible),Value::List(values)]=fields.as_slice()else{return Err(invalid("Choice cursor row ports differ"));};let expected=if *visible{option.label.0.iter().filter(|p|matches!(p,rvn_parser::TextSegment::Interp(_))).count()}else{0};if values.len()!=expected{return Err(invalid("Choice retained label cursor differs from frozen ports"));}}
            let options=options.clone();let ctx=context()?;match wire.kind.as_str(){"consumer_choice"=>Task::ConsumerChoice{options,index,rows,ctx},"consumer_choice_selected"=>Task::ConsumerChoiceSelected{options,index,rows,ctx},_=>{let count=options[index].label.0.iter().filter(|p|matches!(p,rvn_parser::TextSegment::Interp(_))).count();Task::ConsumerChoiceLabel{options,index,rows,count,ctx}}}
        },
        "root_end"=>Task::RootEnd,
        _=>return Err(invalid("Unknown saved continuation task")),
    };
    // Reject ignored/extra data, contexts and forged redundant parameters.
    if task_cursor(&task)?!=*wire{return Err(invalid("Noncanonical saved continuation cursor"));}
    Ok(task)
}
fn rebind_state(state:&mut State,library:&FunctionLibrary,locale:Option<&crate::locale::LocaleManager>){state.text_resources=library.text_resources().revision_stamp();state.widget_classes=library.widget_classes.receipt().cloned();state.locale=locale.cloned();}
fn validate_state(state:&State)->EvalResult<()> {
    if !state.unit_writes.is_empty(){return Err(invalid("Unit write provenance has no portable contract"));}
    if state.steps>100_000||state.value_work>1_000_000||state.calls>64||state.commands.len()>128{return Err(invalid("Saved continuation budget exceeds runtime bounds"));}
    state.consumer_frame.validate()?;let mut work=0usize;
    for(name,value)in &state.globals {if !rvn_parser::is_variable_binding_name(name)&&!name.starts_with("persistent."){return Err(invalid("Invalid saved global binding"));}work=work.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;}
    if work>1_000_000{return Err(invalid("Saved continuation values exceed budget"));}Ok(())
}
impl Probe {
    pub(crate) fn portable_trace(probe:&SharedProbe,continuation:&Continuation)->EvalResult<PortableTrace>{
        Self::portable_trace_with_host(probe,continuation,None)
    }
    pub(crate) fn portable_trace_with_host(probe:&SharedProbe,continuation:&Continuation,permit:Option<&crate::ui_consumer::HostContinuationPermit>)->EvalResult<PortableTrace>{
        Self::portable_trace_with_scope(probe,continuation,permit.map(|permit|permit as &dyn crate::ui_consumer::PortableScopePermit))
    }
    pub(crate) fn portable_trace_with_scope(probe:&SharedProbe,continuation:&Continuation,permit:Option<&dyn crate::ui_consumer::PortableScopePermit>)->EvalResult<PortableTrace>{
        continuation.reject_unit_portable()?;
        let probe=probe.lock().map_err(|_|invalid("Debugger computation lock unavailable"))?;
        if std::iter::once(&continuation.request).chain(probe.records.iter().map(|r|&r.request)).any(|request|request.scope.as_ref().is_some_and(|scope|(scope.starts_with("screen:")||scope.starts_with("handler:"))&&!permit.is_some_and(|permit|permit.admits(scope))&&!probe.admits_screen_request(request))){return Err(invalid("Portable host/widget continuation requires its actual instance/lifecycle adapter"));}
        if probe.stale||probe.sealed||probe.requested.as_ref()!=Some(&continuation.request)||continuation.done||continuation.error.is_some()||probe.records.iter().any(|r|r.result.error.is_some()){return Err(invalid("Only a valid pending owned computation can be checkpointed"));}
        measured(&format_args!("{:?}",continuation.tasks))?;
        let value=PortableTrace{widget_values:None,records:probe.records.iter().map(|r|PortableRecord{request:r.request.clone(),result:r.result.clone()}).collect(),pending:PortableContinuation{request:continuation.request.clone(),state:continuation.state.clone(),frames:continuation.frames.clone(),tasks:continuation.tasks.iter().map(task_cursor).collect::<EvalResult<Vec<_>>>()?,values:continuation.values.clone()}};
        let bytes=serde_json::to_vec(&value).map_err(|_|invalid("Portable computation encoding failed"))?;if bytes.len()>TRACE_BYTES{return Err(invalid("Portable computation exceeds its bounded wire"));}Ok(value)
    }
    pub(crate) fn restore_record(probe:&SharedProbe,current:&Continuation,record:&PortableRecord,locale:Option<&crate::locale::LocaleManager>)->EvalResult<()> {
        Self::restore_record_with_host(probe,current,record,locale,None)
    }
    pub(crate) fn restore_record_with_host(probe:&SharedProbe,current:&Continuation,record:&PortableRecord,locale:Option<&crate::locale::LocaleManager>,permit:Option<&crate::ui_consumer::HostContinuationPermit>)->EvalResult<()> {
        Self::restore_record_with_scope(probe,current,record,locale,permit.map(|permit|permit as &dyn crate::ui_consumer::PortableScopePermit))
    }
    pub(crate) fn restore_record_with_scope(probe:&SharedProbe,current:&Continuation,record:&PortableRecord,locale:Option<&crate::locale::LocaleManager>,permit:Option<&dyn crate::ui_consumer::PortableScopePermit>)->EvalResult<()> {
        current.reject_unit_portable()?;
        if matches!(record.request.root,Root::Unit(..)|Root::LibraryCall(..)|Root::LibraryUnit(..))||record.result.unit_exit.is_some()||!record.request.state.unit_writes.is_empty()||!record.result.state.unit_writes.is_empty(){return Err(invalid("Unit completed roots have no portable import contract"));}
        let actual_screen=probe.lock().map_err(|_|invalid("Screen construction trace unavailable"))?.admits_screen_request(&current.request);
        if record.request.scope.as_ref().is_some_and(|scope|(scope.starts_with("screen:")||scope.starts_with("handler:"))&&!permit.is_some_and(|permit|permit.admits(scope))&&!actual_screen){return Err(invalid("Portable completed host root requires real instance/lifecycle adapter"));}
        let mut record=record.clone();record.request.root=current.request.root.clone();record.request.screen=current.request.screen.clone();rebind_state(&mut record.request.state,&current.library,locale);rebind_state(&mut record.result.state,&current.library,locale);
        if record.request!=current.request{return Err(invalid("Completed portable root differs from actual transaction preparation"));}validate_state(&record.result.state)?;
        standard_macros_1020::validate_transition(&current.library,&record.request.state,&record.result.state)?;
        if record.result.state.steps<record.request.state.steps||record.result.state.value_work<record.request.state.value_work||record.result.state.calls!=record.request.state.calls{return Err(invalid("Completed portable root budgets differ from caller"));}
        let mut data_work=0usize;
        for(name,value)in &record.result.variables{if !rvn_parser::is_variable_binding_name(name)&&!record.request.variables.contains_key(name){return Err(invalid("Completed portable root has invalid lexical binding"));}data_work=data_work.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;}
        if data_work>1_000_000{return Err(invalid("Completed portable lexical values exceed runtime bound"));}
        let cost=measured(&format_args!("{:?}{:?}",record.request,record.result))?;
        let mut probe=probe.lock().map_err(|_|invalid("Debugger computation lock unavailable"))?;if probe.requested.as_ref()!=Some(&current.request){return Err(invalid("Portable root was not actually requested"));}
        probe.bytes=probe.bytes.checked_add(cost).filter(|b|*b<=TRACE_BYTES).ok_or_else(||invalid("Portable trace exceeds cumulative budget"))?;
        probe.records.push(Record{request:record.request,result:record.result});probe.requested=None;Ok(())
    }
}
impl Continuation {
    pub(crate) fn restore_portable(&mut self,trace:&PortableTrace,locale:Option<&crate::locale::LocaleManager>)->EvalResult<()> {
        self.reject_unit_portable()?;
        let mut pending=trace.pending.clone();pending.request.root=self.request.root.clone();pending.request.screen=self.request.screen.clone();rebind_state(&mut pending.request.state,&self.library,locale);rebind_state(&mut pending.state,&self.library,locale);
        if pending.request!=self.request{return Err(invalid("Portable pending root differs from actual transaction preparation"));}
        validate_state(&pending.state)?;
        if pending.state.steps<self.state.steps||pending.state.value_work<self.state.value_work||pending.frames.is_empty()||pending.frames.len()>65||pending.tasks.is_empty()||pending.tasks.len()>100_000{return Err(invalid("Invalid saved continuation stack/budgets"));}
        let mut total=0usize;
        for(index,frame)in pending.frames.iter_mut().enumerate(){
            if frame.depth!=self.request.state.calls+index||frame.depth>64{return Err(invalid("Saved invocation depth differs from stack owner"));}
            if index==0&&frame.function!=self.frames[0].function{return Err(invalid("Saved root owner differs from actual declaration"));}
            if index>0&&!frame.function.as_ref().is_some_and(|name|(self.library.legacy_definition(name).is_some()&&!self.library.legacy_unit(name))){return Err(invalid("Saved invocation has no loaded definition"));}
            frame.consumer_storage=frame.consumer_frame.storage()?;
            for(name,value)in &frame.variables {if !rvn_parser::is_variable_binding_name(name)&&!self.request.variables.contains_key(name){return Err(invalid("Invalid saved lexical binding"));}total=total.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;}
        }
        for(index,frame)in pending.frames.iter().enumerate(){if let Some(site)=&frame.current_site{if site.function!=frame.function||site.depth!=frame.depth||!site.statement{return Err(invalid("Saved lexical observation differs from actual frame owner"));}node_at(self,&pending.frames,&Context{frame:index,route:site.route.clone(),depth:0})?.statement()?;}}
        if total>1_000_000||pending.state.calls!=self.request.state.calls+pending.frames.len()-1{return Err(invalid("Saved frame values/call count differ from actual stack"));}
        for value in &pending.values{total=total.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;}if total>1_000_000{return Err(invalid("Saved value stack exceeds budget"));}
        let mut clone_bytes=0usize;
        for task in &pending.tasks{if let Some(ctx)=&task.ctx {let node=node_at(self,&pending.frames,ctx)?;
            let bytes=match task.kind.as_str(){
                "expr"=>measured(&format_args!("{:?}",node.expr()?))?,"statement"|"command"=>measured(&format_args!("{:?}",node.statement()?))?,"block"=>measured(&format_args!("{:?}",node.block()?))?,
                "short"=>match node.expr()?{Expr::And(_,right)|Expr::Or(_,right)=>measured(&format_args!("{right:?}"))?,_=>0},
                "call"=>match node{Node::Call(name,_)=>name.len(),_=>match node.expr()?{Expr::Call{name,..}=>name.len(),_=>0}},
                "assign"=>match node.statement()?{Statement::SetVar{name,..}|Statement::LocalVar{name,..}=>name.len(),_=>0},
                "branch"=>match node.statement()?{Statement::If{then_branch,else_branch,..}=>measured(&format_args!("{then_branch:?}{else_branch:?}"))?,_=>0},
                "while"|"while_ready"=>match node.statement()?{Statement::While{condition,body}=>measured(&format_args!("{condition:?}{body:?}"))?,_=>0},
                "each_ready"|"each_next"|"range_ready"|"range_next"=>match node.statement()?{Statement::ForEach{name,body,..}|Statement::ForLoopWithBreak{name,body,..}=>name.len()+measured(&format_args!("{body:?}"))?,_=>0},
                "native_loop_resume"|"consumer_next"|"consumer_branch"|"consumer_loop"|"consumer_iteration"|"consumer_choice"|"consumer_choice_selected"|"consumer_choice_label"|"consumer_dialogue_input"=>match node.statement()?{Statement::ScheduledConsumer{scope,steps,body,..}=>scope.len()+measured(&format_args!("{steps:?}{body:?}"))?,_=>0},
                "macro_ready"|"macro_operand_next_v1"=>match node.statement()?{Statement::StandardMacroDispatch{dispatch}=>measured(&format_args!("{dispatch:?}"))?,_=>0},
                "consumer_capture"|"consumer_inspect"=>512,
                _=>0,
            };clone_bytes=clone_bytes.checked_add(bytes).filter(|b|*b<=TRACE_BYTES).ok_or_else(||invalid("Reconstructed continuation source payload exceeds bounded clone storage"))?;}}
        let mut tasks=pending.tasks.iter().map(|task|task_from_cursor(self,&pending.frames,task)).collect::<EvalResult<Vec<_>>>()?;
        if let Some(admission)=&trace.widget_values { super::widget_values::rebind_task_values(&mut tasks,admission)?; }
        // A root Dialogue transfers its completed captures in exactly one phase.
        // Equal values or a reused producer ordinal cannot create that owner.
        if tasks.iter().any(|task|matches!(task,Task::ConsumerDialogueInput{..})) {
            let exact=match tasks.as_slice(){
                [Task::RootEnd,Task::ConsumerEnd{frame:0,ctx:end},Task::ConsumerDialogueInput{ctx,..}]=>
                    end==ctx&&ctx.frame==0&&ctx.depth==0&&ctx.route==self.request.route_prefix,
                _=>false,
            };
            if !exact||pending.frames.len()!=1||!pending.values.is_empty(){return Err(invalid("Dynamic Dialogue transfer lost its exact root phase owner"));}
        }
        let ends=tasks.iter().filter_map(|task|if let Task::FunctionEnd{frame,base,..}=task{Some((*frame,*base))}else{None}).collect::<Vec<_>>();
        if ends.iter().map(|(f,_)|*f).collect::<Vec<_>>()!=(1..pending.frames.len()).collect::<Vec<_>>()||ends.iter().any(|(_,base)|*base>pending.values.len()){return Err(invalid("Saved parked callers do not match function-end stack"));}
        for (frame_index,_) in &ends {
            let end=tasks.iter().position(|t|matches!(t,Task::FunctionEnd{frame,..}if frame==frame_index)).unwrap();
            let caller=tasks[..end].iter().rev().find_map(|task|match task{Task::Charge(ctx)if ctx.frame+1==*frame_index=>Some(ctx),_=>None});
            let called=match caller{Some(ctx)=>match node_at(self,&pending.frames,ctx)?.expr()?{Expr::Call{name,..}=>name,_=>return Err(invalid("Parked invocation has no actual caller expression"))},None if *frame_index==1=>match &self.request.root{Root::Call(name,_)=>name,_=>return Err(invalid("Saved invocation has no actual call site"))},None=>return Err(invalid("Saved invocation lost parked caller route"))};
            let function=pending.frames[*frame_index].function.as_deref().unwrap();
            if called!=function && !(called=="motion_curve"&&tasks[..end].iter().any(|t|matches!(t,Task::CurveValue{name,ctx,..}if name==function&&ctx.frame+1==*frame_index))){return Err(invalid("Saved invocation definition differs from actual caller expression"));}
        }
        for(index,frame)in pending.frames.iter().enumerate(){
            let body:&[Statement]=if index==0{match &self.request.root{Root::Block(body)=>body,Root::Consumer(statement,_)=>std::slice::from_ref(statement),_=>&[]}}else{&self.library.legacy_definition(frame.function.as_ref().unwrap()).ok_or_else(||invalid("Saved Legacy definition absent"))?.1};
            let mut catalog:BTreeMap<String,HashSet<String>>=BTreeMap::new();
            crate::save::consumer_checkpoint::visit_statement_plans(body,&mut |plan|{if let Statement::ScheduledConsumer{scope,steps,..}=plan{let ids=catalog.entry(scope.clone()).or_default();ids.extend(steps.iter().filter_map(|step|match step{rvn_parser::ConsumerStep::InspectTextTable{producer,..}=>Some(producer.clone()),_=>None}));}Ok(())}).map_err(|e|invalid(&e.to_string()))?;
            standard_macros_1020::inspector_catalog(body,&mut catalog)?;
            if index==0 {
                let initial=serde_json::to_value(&self.request.state.consumer_frame).map_err(|_|invalid("Actual initial frame encoding failed"))?;
                for(scope,terminals)in initial["scopes"].as_object().unwrap(){catalog.entry(scope.clone()).or_default().extend(terminals.as_object().unwrap().keys().cloned());}
                if let Root::Consumer(Statement::ScheduledConsumer{scope,..},bound)=&self.request.root {if let Some(ids)=catalog.remove(scope){catalog.entry(bound.clone()).or_default().extend(ids);}}
            }
            let encoded=serde_json::to_value(&frame.consumer_frame).map_err(|_|invalid("Saved frame encoding failed"))?;
            for(scope,terminals)in encoded["scopes"].as_object().ok_or_else(||invalid("Saved frame lost its typed scopes"))?{let ids=catalog.get(scope).ok_or_else(||invalid("Saved terminal scope has no inspector in actual definition"))?;if terminals.as_object().ok_or_else(||invalid("Saved terminals lost typed storage"))?.keys().any(|id|!ids.contains(id)){return Err(invalid("Saved terminal producer has no actual definition"));}}
            let phases=tasks.iter().filter_map(|task|match task{Task::ConsumerNext{scope,steps,index:cursor,ctx,..}if ctx.frame==index=>Some((scope.clone(),steps.as_slice(),*cursor,ctx)),Task::MacroOperandNext{dispatch,index:cursor,ctx,..}if ctx.frame==index=>Some((dispatch.site.scope_source(),dispatch.operand_steps(),*cursor,ctx)),_=>None}).collect::<Vec<_>>();
            match &frame.active_consumer {
                Some(active)=>{
                    if phases.len()>1||!tasks.iter().any(|task|matches!(task,Task::ConsumerEnd{frame,..}if *frame==index)){return Err(invalid("Saved consumer has no coherent owning phase"));}
                    let end_ctx=tasks.iter().find_map(|t|match t{Task::ConsumerEnd{frame,ctx}if *frame==index=>Some(ctx),_=>None}).unwrap();
                    let authored=node_at(self,&pending.frames,end_ctx)?;
                    let(scope,steps)=match authored.statement()?{Statement::ScheduledConsumer{scope,steps,..}=>(runtime_scope(self,end_ctx,scope.clone()),steps.as_slice()),Statement::StandardMacroDispatch{dispatch}if dispatch.deferred_operand_v1.is_some()=>(dispatch.site.scope_source(),dispatch.operand_steps()),_=>return Err(invalid("Saved consumer end differs from actual plan"))};
                    if scope!=active.scope{return Err(invalid("Saved consumer scope differs from frozen plan"));}
                    if phases.first().is_some_and(|(phase_scope,phase_steps,_,phase_ctx)|phase_scope!=&scope||*phase_steps!=steps||*phase_ctx!=end_ctx){return Err(invalid("Saved phase cursor differs from its actual closing source owner"));}
                    let cursor=phases.first().map_or(steps.len(),|(_,_,cursor,_)|*cursor);
                    let stores=tasks.iter().filter_map(|t|match t{Task::ConsumerCapture{ctx,..}|Task::ConsumerInspect{ctx,..}if ctx.frame==index=>ctx.route.last().and_then(|r|if let Route::Producer(i)=r{Some(*i)}else{None}),_=>None}).collect::<Vec<_>>();
                    if stores.len()>1||stores.first().is_some_and(|i|cursor==0||*i+1!=cursor){return Err(invalid("Saved producer/store order differs from current plan cursor"));}
                    let completed=cursor-usize::from(!stores.is_empty());
                    let selected=if matches!(node_at(self,&pending.frames,end_ctx)?,Node::Statement(Statement::ScheduledConsumer{version:3,..})){match Continuation::lexical_phase_from_tasks(&tasks,end_ctx)?{Some((_,indices,_,_))=>Some(indices),None=>self.native_phase_details(end_ctx)?.map(|(_,indices,_,_)|indices)}}else{None};
                    let captures=steps[..completed].iter().enumerate().filter(|(i,_)|selected.as_ref().is_none_or(|s|s.contains(i))).map(|(_,step)|step).filter_map(|step|if let rvn_parser::ConsumerStep::Capture{producer,..}=step{Some(producer)}else{None}).collect::<HashSet<_>>();
                    let inspectors=steps[..completed].iter().enumerate().filter(|(i,_)|selected.as_ref().is_none_or(|s|s.contains(i))).map(|(_,step)|step).filter_map(|step|if let rvn_parser::ConsumerStep::InspectTextTable{producer,..}=step{Some(producer)}else{None}).collect::<HashSet<_>>();
                    if active.captures.keys().collect::<HashSet<_>>()!=captures||active.inspectors.iter().collect::<HashSet<_>>()!=inspectors{return Err(invalid("Saved consumer completion order differs from plan"));}
                },None if !phases.is_empty()=>return Err(invalid("Saved producer cursor has no actual consumer owner")),None=>{},
            }
        }
        validate_lexical_loops(self,&pending.frames,&pending.state,&tasks)?;
        standard_macros_1020::validate_tasks(self,&pending.state,&tasks)?;
        // The existing Save15 admission hydrates current object lineage/epoch.
        // No authority is decoded from the trace or inferred from class data.
        self.widget_objects=trace.widget_values.clone();
        self.state=pending.state;self.frames=pending.frames;self.tasks=tasks;self.values=pending.values;Ok(())
    }
}

impl PortableTrace {
    pub(crate) fn remap_engine_namespaces(&mut self,mapping:&BTreeMap<String,String>)->EvalResult<()> {
        self.remap_owned_namespaces(mapping,&BTreeMap::new())
    }
    pub(crate) fn remap_owned_namespaces(&mut self,mapping:&BTreeMap<String,String>,hosts:&BTreeMap<String,BTreeMap<String,String>>)->EvalResult<()> {
        for record in &mut self.records {
            let actual=record.request.scope.as_ref().and_then(|scope|hosts.get(scope)).unwrap_or(mapping);
            remap_portable_state(&mut record.request.state,actual)?;
            remap_portable_state(&mut record.result.state,actual)?;
        }
        let actual=self.pending.request.scope.as_ref().and_then(|scope|hosts.get(scope)).unwrap_or(mapping);
        remap_portable_state(&mut self.pending.request.state,actual)?;remap_portable_state(&mut self.pending.state,actual)?;
        let root=self.pending.frames.get_mut(0).ok_or_else(||invalid("Portable continuation has no root owner"))?;remap_portable_frame(&mut root.consumer_frame,actual)?;remap_portable_active(&mut root.active_consumer,actual)?;Ok(())
    }
}

/// Re-admit every lexical owner from the frozen definition, then relate the
/// inert saved cursor to its actual task phase and private local projection.
/// Saved values remain Save15 data; routes/names never mint host authority.
fn validate_lexical_loops(current:&Continuation,frames:&[Frame],state:&State,tasks:&[Task])->EvalResult<()> {
    use rvn_parser::scheduled_source::native_loop_contract::{self as contract,Kind};
    let mut owners=std::collections::BTreeSet::new();
    for (position,task) in tasks.iter().enumerate(){
        let Task::NativeLoopResume{statement, state:loop_state,ctx}=task else{continue;};
        if ctx.depth!=0||!owners.insert((ctx.frame,format!("{:?}",ctx.route))){return Err(invalid("Duplicate/deep Native lexical owner"));}
        if statement!=node_at(current,frames,ctx)?.statement()?{return Err(invalid("Native lexical owner differs from its actual definition route"));}
        loop_state.validate(statement)?;
        let binding=contract::binding(statement).map_err(EvalError::InvalidFunction)?.unwrap();
        let frame=frames.get(ctx.frame).ok_or_else(||invalid("Native lexical owner frame absent"))?;
        if let Some(site)=&loop_state.break_site{
            if site.frame!=ctx.frame||site.depth!=0||!matches!(node_at(current,frames,site)?,Node::Statement(Statement::LoopBreak)){return Err(invalid("Saved Native Break has no actual lexical source site"));}
            let expected=ctx.child(Route::Body).child(Route::Statement(0)).route;
            let mut nearest=None;
            for length in 0..site.route.len(){let at=Context{frame:site.frame,route:site.route[..length].to_vec(),depth:0};if matches!(node_at(current,frames,&at),Ok(Node::Statement(Statement::ForLoopWithBreak{..}))){nearest=Some(at.route);}}
            if nearest.as_ref()!=Some(&expected){return Err(invalid("Saved Break targets another nearer Range/function activation"));}
        }
        let projection=frame.variables.get(&binding.index);
        match loop_state.phase{
            LoopPhase::Body if projection==Some(&Value::Int(loop_state.cursor.as_ref().unwrap().index()))=>{},
            LoopPhase::First|LoopPhase::Test if projection.is_none()=>{},
            _=>return Err(invalid("Saved Native index is not its actual phase/cursor projection")),
        }
        let headers=tasks.iter().enumerate().filter_map(|(i,t)|match t{Task::NativeLoopHeader{phase,ctx:owner}if owner==ctx=>Some((i,*phase)),_=>None}).collect::<Vec<_>>();
        if loop_state.phase==LoopPhase::Body{
            if !headers.is_empty(){return Err(invalid("Entered Native body retains a header result"));}
        }else if !matches!(headers.as_slice(),[(i,phase)]if *i>position&&*phase==loop_state.phase){return Err(invalid("Native input has no matching parked result continuation"));}
        let details=if loop_state.phase==LoopPhase::Body{None}else{Continuation::lexical_phase_from_tasks(tasks,ctx)?};
        let body_route=ctx.child(Route::Body).child(Route::Statement(0)).child(Route::Body).route;
        for (i,other) in tasks.iter().enumerate(){
            let wire=task_cursor(other)?;let Some(source)=wire.ctx else{continue;};
            if source.frame!=ctx.frame||!source.route.starts_with(&ctx.route){continue;}
            if i<position{return Err(invalid("Native lexical work precedes its owning stack marker"));}
            if source.route==ctx.route&&source.depth==0{
                let allowed=if loop_state.phase==LoopPhase::Body{wire.kind=="native_loop_resume"}
                    else{matches!(wire.kind.as_str(),"native_loop_resume"|"native_loop_header"|"consumer_next"|"consumer_end")};
                if allowed{continue;}
            }
            if loop_state.phase==LoopPhase::Body{
                if source.route.starts_with(&body_route){continue;}
                return Err(invalid("Saved Native body contains work from a discarded header"));
            }
            let Some((_,selected,_,root))=&details else{return Err(invalid("Native header phase lost its demanded slice"));};
            let expression_kind=matches!(wire.kind.as_str(),"expr"|"charge"|"neg"|"not"|"bool"|"binary"|"index"|"list"|"short"|"call"|"curve_next"|"curve_value"|"consumer_capture"|"consumer_inspect");
            if expression_kind&&(source.route.starts_with(&root.route)||selected.iter().any(|index|source.route.starts_with(&ctx.child(Route::Producer(*index)).route))){continue;}
            return Err(invalid("Saved Native lexical task is outside its actual demanded phase"));
        }
        if binding.kind==Kind::Array&&loop_state.break_requested{return Err(invalid("Array cannot carry a Range Break latch"));}
    }
    // The inverse relation matters too: a header/producer cannot manufacture
    // an owner merely by naming an existing frozen Native source route.
    for task in tasks{
        match task{
            Task::NativeLoopHeader{ctx,..}=>{if !owners.contains(&(ctx.frame,format!("{:?}",ctx.route))){return Err(invalid("Native result continuation has no owner"));}},
            Task::ConsumerNext{version:3,value_only:false,ctx,..}=>{if !owners.contains(&(ctx.frame,format!("{:?}",ctx.route))){return Err(invalid("Native producer cursor has no lexical owner"));}},
            _=>{},
        }
    }
    // Only an entered body in THIS frame may retain the lexical projection.
    // No index/phase/cursor is exported as an ordinary global or orphan local.
    for (frame_index,frame) in frames.iter().enumerate(){
        let body:&[Statement]=if frame_index==0{match &current.request.root{Root::Block(body)=>body,Root::Consumer(statement,_)=>std::slice::from_ref(statement),_=>&[]}}
            else{&current.library.legacy_definition(frame.function.as_ref().unwrap()).ok_or_else(||invalid("Saved Legacy definition absent"))?.1};
        crate::save::consumer_checkpoint::visit_statement_plans(body,&mut |statement|{
            if let Some(binding)=contract::binding(statement).map_err(|e|crate::save::SaveError::Incompatible(e))?{
                // The Engine input-root has its independently authenticated
                // projection/phase in Request, never a lexical loop marker.
                let engine_root=frame_index==0&&matches!(&current.request.root,Root::Consumer(root,_)if root==statement);
                if !engine_root{
                    let active=tasks.iter().find_map(|task|match task{Task::NativeLoopResume{statement:owner,state,ctx}if ctx.frame==frame_index&&owner==statement&&state.phase==LoopPhase::Body=>Some(state),_=>None});
                    if frame.variables.contains_key(&binding.index)!=active.is_some()||frame.variables.contains_key(&binding.phase)||frame.variables.contains_key(&binding.cursor)
                        ||state.globals.contains_key(&binding.index)||state.globals.contains_key(&binding.phase)||state.globals.contains_key(&binding.cursor){return Err(crate::save::SaveError::Incompatible("Saved Native lexical projection escaped its actual body".into()));}
                }
            }Ok(())
        }).map_err(|e|invalid(&e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod owner_focus_commands_0947_tests{
    use super::*;
    #[derive(Debug,PartialEq,Serialize,Deserialize)]
    struct Payload{#[serde(with="super::commands")]commands:Vec<UiCommand>}
    #[test]
    fn portable_focus_keeps_exact_legacy_shape_and_round_trips_typed_owner_intent(){
        let legacy=Payload{commands:vec![UiCommand::Focus{name:"owner".into(),element:"actual".into()}]};
        assert_eq!(serde_json::to_value(&legacy).unwrap(),serde_json::json!({"commands":[{"Focus":{"name":"owner","element":"actual"}}]}));
        let typed=Payload{commands:vec![UiCommand::FocusTarget{name:"owner".into(),target:rvn_ui::programmable::HostFocusTarget::Owner,user:2},UiCommand::FocusTarget{name:"child".into(),target:rvn_ui::programmable::HostFocusTarget::Element("actual".into()),user:0}]};let encoded=serde_json::to_string(&typed).unwrap();assert_eq!(serde_json::from_str::<Payload>(&encoded).unwrap(),typed);
        assert!(!encoded.contains("wrapper_generation"));assert!(!encoded.contains("FrameScope"));assert!(!encoded.contains("lineage"));
    }
}

#[cfg(test)]
#[path="eval_enum_portable_cursor_tests1025.rs"]
mod enum_cursor_tests1025;
