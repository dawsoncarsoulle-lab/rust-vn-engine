//! Admission for the first real live Unit operation, not a portable DTO grant.
use super::*;
impl<R:Renderer> Engine<R> {
    pub(in super::super) fn validate_live_unit_debug(&self,operation:Option<&DebugOperation>)->Result<(),RuntimeError> {
        // Actual non-Unit Init work must not acquire a Unit continuation merely
        // because an unused qualified declaration exists elsewhere. Keep the
        // original source/cursor/instance proof and all other operation gates.
        if let Some(DebugOperation::Startup(operation))=operation{
            if self.startup.is_some()&&!self.functions.library_assets1023().is_empty(){
                self.validate_startup_operation(operation)?;
                let program=self.frozen_startup_program.as_ref().ok_or_else(||self.eval_err(EvalError::InvalidFunction("Joint startup lost its original frozen program".into()),"Library Startup source"))?;
                crate::engine::function_libraries1023::startup_joint1027::validate_retained_library_units1027(&self.functions,&program.script).map_err(|error|self.eval_err(error,"Library Startup source"))?;
                return Ok(());
            }
        }
        if !FunctionLibrary::contains_unit_source(&self.script){
            // Preserve refusal when a stale library still owns a removed Unit
            // declaration; absence from the current script is no grant.
            return self.functions.reject_unit_portable_owner(&self.script).map_err(|error|self.eval_err(error,"Unit live declaration owner"));
        }
        let fail=|message:&str|self.eval_err(EvalError::InvalidFunction(message.into()),"Unit live debugger owner");
        if self.startup.is_some()||self.state.scene_execution.as_ref().and_then(|execution|execution.active.as_ref()).is_none(){return Err(fail("Unit live debugging requires the actual reached Scene owner; Init/Startup is not admitted"));}
        if operation.is_some_and(|operation|!matches!(operation,DebugOperation::Statement))
            ||operation.is_some()&&!matches!(self.script.get(self.state.pc),Some(Statement::UnitCall{..}|Statement::LibraryUnitCall{..})) {
            return Err(fail("Only the actual current synchronous UnitCall statement is admitted for this live operation"));
        }
        self.functions.validate_engine_unit_owner(&self.script).map_err(|error|self.eval_err(error,"Unit live lexical source"))?;
        self.consumer_program.validate_unit_emitted(&self.script).map_err(|error|self.eval_err(error,"Unit live frozen executable"))?;
        let map=self.functions.debug_source_map.as_ref().ok_or_else(||fail("Unit live debugging needs the actual frozen source map"))?;
        if map.emitted!=crate::debugger_provenance::SourceMap::emitted_identity(&self.script).map_err(|message|fail(&message))? {
            return Err(fail("Unit live executable differs from its frozen source map"));
        }
        let mut pending=vec![self.script.as_slice()];
        while let Some(body)=pending.pop(){for statement in body{
            if matches!(statement,Statement::ScheduledConsumer{..}){return Err(fail("Unit live NativeFrame/source-plan combination is not admitted"));}
            pending.extend(crate::consumer_schedule::statement_children(statement));
        }}
        Ok(())
    }
}
