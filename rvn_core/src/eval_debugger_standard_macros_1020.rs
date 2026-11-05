//! Child of the existing continuation, using its Task/Context/budgets. The
//! shared owner cells live in State, not a new frame per pure-function call.
use super::*;
use crate::widget_class_source_0950::standard_macros::{Operand, OperandNeed};
use rvn_parser::scheduled_source::standard_macro_contract::{
    DataOutput, Dispatch, Site as MacroSite,
};
impl Continuation {
    pub(super) fn start_macro(&mut self, dispatch: Dispatch, ctx: Context) -> EvalResult<()> {
        dispatch.validate().map_err(EvalError::InvalidFunction)?;
        if self.state.calls != 0
            || self.state.handler_locals.is_none()
            || self.state.pure
            || ctx.frame != 0
        {
            return Err(invalid(
                "Stateful macro requires its actual Widget Handler continuation",
            ));
        }
        let frame = self
            .state
            .widget_macro_frame
            .as_ref()
            .ok_or_else(|| invalid("Macro continuation lost its actual Widget owner"))?;
        let site = self
            .library
            .widget_macro_sites
            .site(frame.class(), &dispatch.site)
            .map_err(EvalError::InvalidFunction)?;
        let need = frame
            .operand_need(site, dispatch.control)
            .map_err(EvalError::InvalidFunction)?;
        if need == OperandNeed::None {
            return self.finish_macro(dispatch, Operand::None, ctx);
        }
        let operand = dispatch
            .operand
            .clone()
            .ok_or_else(|| invalid("Macro operand lost its actual source expression"))?;
        if dispatch.deferred_operand_v1.is_some() {
            if self.frames[ctx.frame].active_consumer.is_some(){return Err(invalid("Deferred macro requires its own actual demand phase"));}
            self.frames[ctx.frame].active_consumer=Some(ConsumerContext{scope:dispatch.site.scope_source(),captures:Default::default(),inspectors:Default::default()});
            self.tasks.push(Task::MacroReady{dispatch:dispatch.clone(),need,ctx:ctx.clone()});
            self.tasks.push(Task::ConsumerEnd{frame:ctx.frame,ctx:ctx.clone()});
            self.tasks.push(Task::MacroOperandNext{dispatch,need,index:0,ctx});
            return Ok(());
        }
        self.tasks.push(Task::MacroReady {
            dispatch,
            need,
            ctx: ctx.clone(),
        });
        self.tasks.push(Task::Expr(
            operand,
            Context {
                depth: 0,
                ..ctx.child(Route::Argument(0))
            },
        ));
        Ok(())
    }
    pub(super) fn macro_operand_next(&mut self, dispatch:Dispatch, need:OperandNeed, index:usize, ctx:Context)->EvalResult<()> {
        if let Some(step)=dispatch.operand_steps().get(index).cloned(){
            self.tick()?;
            let scope=dispatch.site.scope_source();
            self.tasks.push(Task::MacroOperandNext{dispatch,need,index:index+1,ctx:ctx.clone()});
            let(producer,value,inspect)=match step{rvn_parser::ConsumerStep::Capture{producer,value}=>(producer,value,false),rvn_parser::ConsumerStep::InspectTextTable{producer,value}=>(producer,value,true)};
            let child=ctx.child(Route::Producer(index));
            self.tasks.push(if inspect{Task::ConsumerInspect{producer,scope,ctx:child.clone()}}else{Task::ConsumerCapture{producer,ctx:child.clone()}});
            self.tasks.push(Task::Expr(value,Context{depth:0,..child}));
        }else{
            if index!=dispatch.operand_steps().len(){return Err(invalid("Deferred macro cursor exceeds actual source plan"));}
            let value=dispatch.operand.ok_or_else(||invalid("Deferred macro terminal expression missing"))?;
            self.tasks.push(Task::Expr(value,Context{depth:0,..ctx.child(Route::Argument(0))}));
        }
        Ok(())
    }
    pub(super) fn macro_ready(
        &mut self,
        dispatch: Dispatch,
        need: OperandNeed,
        ctx: Context,
    ) -> EvalResult<()> {
        let value = self.pop()?;
        let operand = match (need, value) {
            (OperandNeed::StartClosed, Value::Bool(value)) => Operand::StartClosed(value),
            (OperandNeed::Limit, Value::Int(value)) => Operand::Limit(
                crate::standard_macro_state::DoNLimit::try_from(value)
                    .map_err(|_| invalid("DoN N is outside signed int32"))?,
            ),
            _ => return Err(invalid("Macro continuation operand phase/type differs")),
        };
        self.finish_macro(dispatch, operand, ctx)
    }
    fn finish_macro(
        &mut self,
        dispatch: Dispatch,
        operand: Operand,
        ctx: Context,
    ) -> EvalResult<()> {
        let frame = self
            .state
            .widget_macro_frame
            .as_mut()
            .ok_or_else(|| invalid("Macro continuation owner disappeared"))?;
        let site = self
            .library
            .widget_macro_sites
            .site(frame.class(), &dispatch.site)
            .map_err(EvalError::InvalidFunction)?;
        let before = frame.retained_bytes().map_err(EvalError::InvalidFunction)?;
        let emitted = frame
            .step(site, dispatch.control, operand)
            .map_err(EvalError::InvalidFunction)?;
        let after = frame.retained_bytes().map_err(EvalError::InvalidFunction)?;
        self.state.value_work = self
            .state
            .value_work
            .checked_add(
                after
                    .checked_sub(before)
                    .ok_or(EvalError::NumericOverflow)?,
            )
            .ok_or(EvalError::NumericOverflow)?;
        if self.state.value_work > 1_000_000 {
            return Err(EvalError::ExecutionLimit {
                limit: "1 000 000 copies ou caractères de valeurs par calcul",
            });
        }
        if let Some(output) = emitted {
            let (index, branch) = dispatch
                .branches
                .into_iter()
                .enumerate()
                .find(|(_, branch)| branch.output == output)
                .ok_or_else(|| invalid("Macro emitted output lost its real source branch"))?;
            self.tasks
                .push(Task::Block(branch.body, 0, ctx.child(Route::Branch(index))));
        }
        Ok(())
    }
    pub(super) fn macro_data(
        &self,
        declaration: &MacroSite,
        output: DataOutput,
        pin: u64,
    ) -> EvalResult<Value> {
        if self.state.pure {
            return Err(invalid(
                "Pure projection cannot borrow Widget Handler macro cells",
            ));
        }
        let frame = self
            .state
            .widget_macro_frame
            .as_ref()
            .ok_or_else(|| invalid("Macro data has no actual Widget owner"))?;
        let site = self
            .library
            .widget_macro_sites
            .site(frame.class(), declaration)
            .map_err(EvalError::InvalidFunction)?;
        frame
            .data(site, output, pin)
            .map_err(EvalError::InvalidFunction)
    }
}
