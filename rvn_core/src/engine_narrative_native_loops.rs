//! Demand phases executed within the existing narrative transaction and real
//! Branch/Helper/SceneCall ownership. No nested Engine or alternative VM.
use super::*;
use crate::{consumer_schedule::{BoundConsumerFrame,ConsumerOwner},eval::debugger_eval::PortableRecord,native_loop_cursor::{DemandLoopCursor,AfterBody},scene::native_loops::{NativeLoopFrame,NativeBreakSite}};
use rvn_parser::scheduled_source::native_loop_contract::{self as contract,Kind,Phase};

impl<R:Renderer> Engine<R> {
    fn native_narrative_input(&self,next:&mut GameState,statement:&Statement,environment:&HashMap<String,Value>)->Result<(Value,PortableRecord),RuntimeError>{
        let Statement::ScheduledConsumer{scope,..}=statement else{unreachable!()};
        let owner=next.scene_execution.as_ref().and_then(|e|e.owner()).map(|activation|ConsumerOwner::Scene{activation}).unwrap_or(ConsumerOwner::Game);
        let scope=self.consumer_program.bound_scope(scope,self.state.pc).map_err(|error|self.eval_err(error,"Native narrative source namespace"))?;
        let identity=self.consumer_program.identity().map_err(|error|self.eval_err(error,"Native narrative program identity"))?;
        let widget_objects=self.widget_object_context(next)?;
        let slot=match next.scene_execution.as_mut().and_then(|e|e.active.as_mut()){Some(active)=>&mut active.consumer_frame,None=>&mut next.consumer_frame};
        let frame=slot.get_or_insert_with(||BoundConsumerFrame::new(identity,owner));frame.validate_binding(identity,owner).map_err(|error|self.eval_err(error,"Native narrative input owner"))?;
        self.functions.eval_engine_consumer_record_with_widget_objects(widget_objects.as_ref(), statement,environment,&self.lexical_vars_for_eval(),&mut next.random,&mut frame.frame,&scope,self.locale.as_ref()).map_err(|error|self.eval_err(error,"Native narrative phase input"))
    }

    fn native_narrative_environment(&self,binding:&contract::Binding,phase:Phase,cursor:Option<&DemandLoopCursor>)->Result<HashMap<String,Value>,RuntimeError>{
        let mut environment=self.vars_for_eval();
        if environment.insert(binding.phase.clone(),Value::Int(if phase==Phase::First{0}else{1})).is_some(){return Err(self.scene_resource_error("Native phase collides with an authored environment binding".into()));}
        if let Some(cursor)=cursor{if environment.insert(binding.cursor.clone(),Value::Int(cursor.index())).is_some(){return Err(self.scene_resource_error("Native cursor collides with an authored environment binding".into()));}}
        Ok(environment)
    }

    pub(super) fn exec_narrative_native_loop(&mut self,statement:Statement)->Result<(),RuntimeError>{
        self.validate_narrative_loop_gathers(&self.state,self.locale.as_ref())?;
        self.consumer_program.validate_plan_binding(&statement,self.state.pc,false).map_err(|error|self.eval_err(error,"Native narrative header binding"))?;
        if self.consumer_program.source.is_none(){return Err(self.scene_resource_error("Native narrative loops require their actual frozen source".into()));}
        let program=self.narrative.as_ref().ok_or_else(||self.scene_resource_error("Native narrative source has no explicit scene preparation".into()))?;
        let body=program.native_loops.body(self.state.pc).ok_or_else(||self.scene_resource_error("Native narrative header has no real compiled branch".into()))?;
        let binding=body.binding.clone();let body_label=body.body_label;let header=program.position(self.state.pc);
        let execution=self.state.scene_execution.as_ref().ok_or_else(||self.scene_resource_error("Native narrative activation is missing".into()))?;
        let current=execution.native_loop_frames().0.last().filter(|frame|frame.header==header);
        let existing=current.is_some();let mut next=self.state.clone();let mut gather=Vec::new();
        let mut cursor=if let Some(current)=current {
            let mut cursor=current.cursor.clone();
            if current.break_site.is_some(){return Err(self.scene_resource_error("Native Break must finish at the actual body Return, without another header operation".into()));}
            cursor.after_body(false).map_err(|error|self.eval_err(error,"Native narrative body continuation"))?;
            if binding.kind==Kind::Range{gather.push(current.gather[0].clone());}
            cursor
        }else if binding.kind==Kind::Range {
            let environment=self.native_narrative_environment(&binding,Phase::First,None)?;
            let(value,record)=self.native_narrative_input(&mut next,&statement,&environment)?;gather.push(record);
            DemandLoopCursor::range(value).map_err(|error|self.eval_err(error,"Native narrative FirstIndex"))?
        }else{DemandLoopCursor::array()};
        let environment=self.native_narrative_environment(&binding,Phase::Test,Some(&cursor))?;
        let(input,record)=self.native_narrative_input(&mut next,&statement,&environment)?;gather.push(record);
        let entered=cursor.enter(&input).map_err(|error|self.eval_err(error,"Native current narrative comparison"))?;
        if !entered {
            if existing{next.scene_execution.as_mut().unwrap().native_loop_frames_mut().0.pop();}
            next.pc=self.state.pc.checked_add(1).ok_or_else(||self.eval_err(EvalError::NumericOverflow,"Native Completed continuation"))?;
            return self.commit_scene_state(next);
        }
        let branch=next.scene_execution.as_ref().unwrap().frames.len();
        next=self.plan_narrative_transfer(next,body_label,Some(self.state.pc),true)?;
        let frame=NativeLoopFrame{header,cursor,gather,body_frame:branch,break_site:None};let loops=&mut next.scene_execution.as_mut().unwrap().native_loop_frames_mut().0;
        if existing{*loops.last_mut().unwrap()=frame;}else{loops.push(frame);}
        self.commit_scene_state(next)
    }

    pub(super) fn exec_narrative_native_break(&mut self)->Result<(),RuntimeError>{
        self.validate_narrative_loop_gathers(&self.state,self.locale.as_ref())?;
        let program=self.narrative.as_ref().ok_or_else(||self.scene_resource_error("Native Break requires its real narrative preparation".into()))?;
        let position=program.position(self.state.pc);
        let body=program.native_loops.break_owner(program,&position).map_err(|error|self.eval_err(error,"Native Break source owner"))?;
        let mut next=self.state.clone();let execution=next.scene_execution.as_mut().ok_or_else(||self.scene_resource_error("Native Break has no actual narrative continuation".into()))?;
        let frame=execution.native_loop_frames_mut().0.iter_mut().rev().find(|frame|frame.header.origin.as_ref()==Some(&body.header))
            .ok_or_else(||self.scene_resource_error("Native Break source has no entered actual Range frame".into()))?;
        frame.cursor.validate_body().map_err(|error|self.eval_err(error,"Native Break entered visit"))?;
        if frame.break_site.is_none(){frame.break_site=Some(NativeBreakSite{position,visit:frame.cursor.visits()});}
        // Keep all already engaged Branch/Sequence continuations. The latch is
        // consumed only when this Range's own body really returns to its header.
        next.pc=self.state.pc.checked_add(1).ok_or_else(||self.eval_err(EvalError::NumericOverflow,"Native Break instruction continuation"))?;
        self.commit_scene_state(next)
    }

    /// Called within the existing Return candidate after its real Branch has
    /// popped. Do not publish a fictitious header visit after a latched Break.
    pub(super) fn finish_native_narrative_body_return(&self,next:&mut GameState,returned:&crate::scene::ResumePosition,synthetic:bool)->Result<(),RuntimeError>{
        if !synthetic{return Ok(());}
        let program=self.narrative.as_ref().unwrap();let execution=next.scene_execution.as_mut().unwrap();
        let Some(frame)=execution.native_loop_frames().0.last()else{return Ok(());};
        if frame.break_site.is_none()||frame.header!=*returned{return Ok(());}
        let body=program.native_loops.body(frame.header.pc).ok_or_else(||self.scene_resource_error("Native Return lost its actual header preparation".into()))?;
        if body.body_return!=self.state.pc||execution.frames.len()!=frame.body_frame{
            return Err(self.scene_resource_error("Native Break completion is not its actual generated Branch Return".into()));
        }
        let mut cursor=frame.cursor.clone();
        if cursor.after_body(true).map_err(|error|self.eval_err(error,"Native Break body completion"))?!=AfterBody::CompletedWithoutComparison{return Err(self.scene_resource_error("Native Break lost its completion decision".into()));}
        next.pc=frame.header.pc.checked_add(1).ok_or_else(||self.eval_err(EvalError::NumericOverflow,"Native Completed continuation"))?;
        execution.native_loop_frames_mut().0.pop();Ok(())
    }

    /// Rebind every live/parked data record to its actual current immutable
    /// source, namespace and FunctionLibrary before save/restore publication.
    pub(super) fn validate_narrative_loop_gathers(&self,state:&GameState,locale:Option<&LocaleManager>)->Result<(),RuntimeError>{
        let Some(execution)=&state.scene_execution else{return Ok(());};if !execution.has_native_loop_frames(){return Ok(());}
        let program=self.narrative.as_ref().ok_or_else(||self.scene_resource_error("Saved Native narrative loops have no actual prepared source".into()))?;
        self.consumer_program.validate_state_binding(state).map_err(|error|self.eval_err(error,"Native narrative consumer ownership"))?;
        execution.validate(program,&state.vars,&state.call_stack).map_err(|message|self.scene_resource_error(message))?;
        // validate_state_binding above already excludes stored aliases from
        // every real variable owner; only disposable projections are needed.
        let mut projected=HashMap::new();
        execution.native_loop_frames().project_body_indices(program,&execution.live_position,&mut projected)
            .map_err(|error|self.eval_err(error,"Native narrative fallible body projection"))?;
        execution.visit_native_loop_frames(|frame|{
            let statement=&program.script[frame.header.pc];self.consumer_program.validate_plan_binding(statement,frame.header.pc,false).map_err(|error|self.eval_err(error,"Retained Native narrative header"))?;
            let Statement::ScheduledConsumer{scope,..}=statement else{unreachable!()};let scope=self.consumer_program.bound_scope(scope,frame.header.pc).map_err(|error|self.eval_err(error,"Retained Native narrative namespace"))?;
            self.functions.validate_retained_loop_inputs(statement,Some(&scope),&[],&frame.gather,locale).map_err(|error|self.eval_err(error,"Retained actual Native narrative calculation"))?;
            Ok(())
        })
    }

    pub(super) fn narrative_native_bindings(&self,variables:&mut HashMap<String,Value>){
        let(Some(program),Some(execution))=(&self.narrative,&self.state.scene_execution)else{return;};
        if execution.native_loop_frames().is_empty(){return;}
        // A read helper cannot grant an invalid frame access to a projection.
        // The fallible step/commit/restore boundaries report its precise error.
        let position=program.position(self.state.pc);
        if execution.native_loop_frames().validate_chain(program,&execution.frames,execution.owner(),&position).is_ok(){
            let _=execution.native_loop_frames().project_body_indices(program,&position,variables);
        }
    }
}
