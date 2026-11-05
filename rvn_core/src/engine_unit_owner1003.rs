//! Synchronous Unit calls use the actual Engine state/publication owner.
//! Live debugging retains the actual computation; publication still belongs here.
use super::*;
enum UnitInvocation1023<'a>{Legacy{name:&'a str,arguments:&'a[rvn_parser::Expr]},Library(&'a rvn_parser::function_library_program1023::QualifiedLibraryCall)}

impl<R:Renderer> Engine<R> {
    pub(super) fn exec_unit_call(&mut self,name:&str,arguments:&[rvn_parser::Expr])->Result<(),RuntimeError> {
        self.exec_unit_invocation1023(UnitInvocation1023::Legacy{name,arguments})
    }
    pub(super) fn exec_library_unit_call1023(&mut self,call:&rvn_parser::function_library_program1023::QualifiedLibraryCall)->Result<(),RuntimeError>{
        self.exec_unit_invocation1023(UnitInvocation1023::Library(call))
    }
    fn exec_unit_invocation1023(&mut self,invocation:UnitInvocation1023<'_>)->Result<(),RuntimeError>{
        if self.startup.is_some() {
            return Err(self.eval_err(EvalError::InvalidFunction("Unit retained Startup continuation is not admitted".into()),"Unit owner"));
        }
        if self.functions.debug_probe.is_some(){self.validate_live_unit_debug(Some(&DebugOperation::Statement))?;}
        self.functions.validate_engine_unit_owner(&self.script).map_err(|e|self.eval_err(e,"Unit lexical source"))?;
        self.consumer_program.validate_unit_emitted(&self.script).map_err(|e|self.eval_err(e,"Unit frozen executable"))?;
        let environment=self.vars_for_eval();
        let globals=self.lexical_vars_for_eval();
        // This bounded adapter has no Widget-reference owner. Reject before
        // evaluating arguments rather than treating a reference as a value
        // which could silently acquire a new global alias or Host authority.
        for value in environment.values().chain(globals.values()) {
            crate::widget_reference_values_0974::reject_widget_values(value).map_err(|_|self.eval_err(EvalError::InvalidFunction("Unit Widget-reference argument/global adapter is not admitted".into()),"Unit value owner"))?;
        }
        let mut next=self.state.clone();
        let result=match invocation{
            UnitInvocation1023::Legacy{name,arguments}=>self.functions.invoke_unit_scoped_with_locale(name,arguments,&environment,&globals,&mut next.random,self.locale.as_ref()),
            UnitInvocation1023::Library(call)=>self.functions.invoke_library_unit_scoped1023(call,&environment,&globals,&mut next.random,self.locale.as_ref()),
        };
        let(computed,written)=result.map_err(|e|self.eval_err(e,"Unit synchronous call"))?;
        // These are exactly the current lexical_vars_for_eval injections.
        // The key snapshot exists only when this actual input owner injects
        // a key; no prefix is reserved and locals/parameters do not write here.
        let writes_input=["__input_mouse_clicked","__input_mouse_x","__input_mouse_y"].iter().any(|name|written.contains(*name))
            || self.input_state.last_key_pressed.is_some()&&written.contains("__input_key");
        if writes_input {
            return Err(self.eval_err(EvalError::InvalidFunction("Unit cannot write the actual Engine input snapshots".into()),"Unit input snapshot publication"));
        }
        for(name,value) in computed {
            if globals.get(&name)==Some(&value){continue;}
            if !rvn_parser::is_variable_binding_name(&name) {
                return Err(self.eval_err(EvalError::InvalidFunction("Unit result attempted to write an unowned variable namespace".into()),"Unit global writeback"));
            }
            crate::widget_reference_values_0974::reject_widget_values(&value).map_err(|_|self.eval_err(EvalError::InvalidFunction("Unit Widget-reference result adapter is not admitted".into()),"Unit value owner"))?;
            // Functions close over globals even when the caller has a local
            // or a loop index of the same name. Caller scopes stay untouched.
            next.vars.insert(name,value);
        }
        next.pc=next.pc.checked_add(1).ok_or_else(||self.eval_err(EvalError::NumericOverflow,"Unit continuation"))?;
        // These are the existing fallible preflight/publication transactions.
        // Neither authoritative RNG nor PC is installed before they accept.
        self.commit_scheduled_state(next)
    }
}

#[cfg(test)]
#[path="engine_unit_owner_tests1003.rs"]
mod tests;
