//! Phase selection for a real Engine consumer request. Original producer
//! ordinals/routes are preserved; a skipped producer consumes no RNG or tick.
use super::*;
use rvn_parser::scheduled_source::native_loop_contract::{self as contract,Kind,Phase};

impl Continuation {
    /// All task contexts at this Engine input root must remain inside the
    /// reached phase. Checking Expr alone would leave parked Call/Charge and
    /// terminal-store cursors able to name a real but unrequested producer.
    pub(super) fn validate_native_phase_task_route(&self,kind:&str,ctx:&Context)->EvalResult<()> {
        if ctx.frame!=0||!matches!(self.request.root,Root::Consumer(Statement::ScheduledConsumer{version:3,..},_)){return Ok(());}
        match kind{
            "initializer_entry"|"consumer_next"|"consumer_end" if ctx.route==self.request.route_prefix&&ctx.depth==0=>Ok(()),
            "expr"|"charge"|"neg"|"not"|"bool"|"binary"|"index"|"list"|"short"|"call"|"curve_next"|"curve_value"|"consumer_capture"|"consumer_inspect"=>self.validate_native_phase_expression_route(ctx),
            _=>Err(invalid("Native Engine phase task is outside its actual input owner")),
        }
    }
    pub(super) fn native_phase_details(&self,ctx:&Context)->EvalResult<Option<(Phase,Vec<usize>,Expr,Context)>>{
        let Root::Consumer(statement @ Statement::ScheduledConsumer{version:3,steps,..},_)=&self.request.root else{return Ok(None);};
        if ctx.frame!=0 || ctx.route!=self.request.route_prefix{return Err(invalid("Native Engine phase belongs to another lexical/source context"));}
        let binding=contract::binding(statement).map_err(EvalError::InvalidFunction)?.ok_or_else(||invalid("Native request has no actual loop binding"))?;
        let phase=contract::owned_engine_phase(&binding,&self.request.variables).map_err(EvalError::InvalidFunction)?;
        let root=contract::phase_expression(statement,phase).map_err(EvalError::InvalidFunction)?;
        let indices=rvn_parser::loop_phase_dependencies::capture_step_indices(steps,root).map_err(EvalError::InvalidFunction)?;
        let route=match(binding.kind,phase){(Kind::Array,Phase::Test)=>Route::Collection,(Kind::Range,Phase::First)=>Route::First,(Kind::Range,Phase::Test)=>Route::Last,_=>return Err(invalid("Array cannot own a FirstIndex request"))};
        Ok(Some((phase,indices,root.clone(),Context{depth:0,..ctx.child(Route::Body).child(Route::Statement(0)).child(route)})))
    }
    pub(super) fn validate_native_phase_expression_route(&self,ctx:&Context)->EvalResult<()> {
        if ctx.frame!=0{return Ok(());}
        let owner=Context{frame:0,route:self.request.route_prefix.clone(),depth:0};
        let Some((_,indices,_,root))=self.native_phase_details(&owner)? else{return Ok(());};
        if ctx.route.starts_with(&root.route)||indices.into_iter().any(|i|ctx.route.starts_with(&owner.child(Route::Producer(i)).route)){return Ok(());}
        Err(invalid("Native phase expression is outside its actual demanded source slice"))
    }
}
