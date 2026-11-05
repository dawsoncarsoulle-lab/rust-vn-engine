//! The existing Init owner executes explicit demand-loop phases. Neither the
//! private expression bindings nor the serialized cursor grant ownership.
use super::*;
use rvn_parser::scheduled_source::native_loop_contract::{self as contract,Kind,Phase};
use crate::native_loop_cursor::DemandLoopCursor;

impl<R:Renderer> Engine<R>{
    pub(super) fn exec_startup_native_loop(&mut self,statement:Statement,header:Vec<wire::Route>)->Result<(),RuntimeError>{
        let binding=contract::binding(&statement).map_err(|e|self.scene_resource_error(e))?
            .ok_or_else(||self.scene_resource_error("Missing explicit Native source loop".into()))?;
        let existing=self.startup.as_ref().unwrap().loops.last().filter(|f|f.header==header).cloned();
        let (mut cursor,first_gather)=if let Some(frame)=existing {
            let PortableStartupLoopState::DemandV1{mut cursor}=frame.state else{return Err(self.scene_resource_error("Native header has another iterator contract".into()));};
            if frame.native_break.is_some(){return Err(self.scene_resource_error("A completed Native Break cannot request another header phase".into()));}
            cursor.after_body(false).map_err(|e|self.eval_err(e,"Native next comparison"))?;
            (cursor,if binding.kind==Kind::Range{vec![frame.gather[0].clone()]}else{vec![]})
        }else if binding.kind==Kind::Range{
            let(next,value)=self.gather_startup_native_phase(&statement,&binding,Phase::First,None)?;
            self.state=next;
            let first=vec![Probe::completed_native_first_record(self.functions.debug_probe.as_ref().unwrap(),&header)
                .map_err(|e|self.eval_err(e,"Native FirstIndex retained root"))?];
            (DemandLoopCursor::range(value).map_err(|e|self.eval_err(e,"Native FirstIndex"))?,first)
        }else{(DemandLoopCursor::array(),vec![])};
        let(next,input)=self.gather_startup_native_phase(&statement,&binding,Phase::Test,Some(cursor.index()))?;
        self.state=next;
        let mut gather=Probe::completed_loop_records(self.functions.debug_probe.as_ref().unwrap(),&header)
            .map_err(|e|self.eval_err(e,"Native header retained root"))?;
        // An initial range visit has First+Test in this transaction; subsequent
        // visits retain their earlier authentic First and the current Test.
        let test=gather.pop().ok_or_else(||self.scene_resource_error("Native comparison root absent".into()))?;
        let mut retained=first_gather;retained.push(test);
        let entered=cursor.enter(&input).map_err(|e|self.eval_err(e,"Native current loop input"))?;
        if self.startup.as_ref().unwrap().loops.last().is_some_and(|f|f.header==header){self.startup.as_mut().unwrap().loops.pop();}
        if !entered{
            return self.startup.as_mut().unwrap().advance_from(&self.script,header).map_err(|e|self.scene_resource_error(e));
        }
        let origin=self.startup_origin_for_route(&header)?;
        let visits=cursor.visits();
        self.startup.as_mut().unwrap().loops.push(PortableStartupLoop{header:header.clone(),header_origin:origin,gather:retained,visits,state:PortableStartupLoopState::DemandV1{cursor},native_break:None});
        crate::save::consumer_checkpoint::preflight_serialized(&self.startup.as_ref().unwrap().loops,crate::save::consumer_checkpoint::Save15::MAX_BYTES)
            .map_err(|e|self.scene_resource_error(e.to_string()))?;
        self.startup.as_mut().unwrap().enter_body(&self.script,&header).map_err(|e|self.scene_resource_error(e))
    }

    fn gather_startup_native_phase(&self,statement:&Statement,binding:&contract::Binding,phase:Phase,index:Option<i64>)->Result<(GameState,Value),RuntimeError>{
        let mut environment=self.vars_for_eval();
        environment.insert(binding.phase.clone(),Value::Int(if phase==Phase::First{0}else{1}));
        match index {Some(index)=>{environment.insert(binding.cursor.clone(),Value::Int(index));},None=>{environment.remove(&binding.cursor);}}
        self.gather_scheduled_action_inputs_in(statement,environment)
    }

    /// Overlay protected body projections from actual admitted source frames.
    /// A serialized/global value with this spelling is never the cursor.
    pub(in crate::engine) fn startup_native_bindings(&self,environment:&mut HashMap<String,Value>){
        let Some(startup)=&self.startup else{return;};
        for frame in &startup.loops {
            if let PortableStartupLoopState::DemandV1{cursor}=&frame.state {
                // Source and cursor are checked before admission/commit and on
                // cold restoration. A malformed frame produces no capability.
                if cursor.validate_body().is_err(){continue;}
                if let Ok(statement)=startup.statement_at(&self.script,&frame.header){
                    if let Ok(Some(binding))=contract::binding(statement){environment.insert(binding.index,Value::Int(cursor.index()));}
                }
            }
        }
    }
}
