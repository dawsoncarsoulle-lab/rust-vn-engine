//! Normal/fresh glue for the same source-owned Startup.
use super::*;
impl<R:Renderer> Engine<R>{
    /// Observation only. Real preparation still validates bytes/AST/ports.
    pub fn program_requires_owned_startup(program:&rvn_parser::ParsedProgram)->bool{
        crate::consumer_schedule::init_requires_owned_continuation(&program.script)
    }
    pub fn has_frozen_startup_program(&self)->bool{self.frozen_startup_program.is_some()}
    pub fn requires_owned_startup_for_fresh(&self)->bool{self.frozen_startup_program.as_ref().is_some_and(|program|Self::program_requires_owned_startup(program)||!self.widget_classes.is_empty()||!self.functions.enum_assets1025.is_empty()||!self.functions.library_assets1023().is_empty())}
    fn validate_frozen_library_fresh1027(&self,program:&rvn_parser::ParsedProgram)->Result<(),RuntimeError>{
        if self.functions.library_assets1023().is_empty(){return Err(self.scene_resource_error("Fresh Library requires its actual retained resources".into()))}
        if !self.widget_classes.matches_program(program).map_err(|e|self.scene_resource_error(e))?{return Err(self.scene_resource_error("Widget classes differ from the original Fresh Library program".into()))}
        crate::save::consumer_checkpoint::preflight_program(program).map_err(|e|self.scene_resource_error(e.to_string()))?;
        crate::save::consumer_checkpoint::preflight_serialized(&self.script,16*1024*1024).map_err(|e|self.scene_resource_error(e.to_string()))?;
        self.consumer_program.validate_emitted(&self.script).map_err(|e|self.eval_err(e,"Fresh original Library executable"))?;
        let binding=crate::consumer_schedule::ConsumerSourceBinding::from_program(program).map_err(|e|self.eval_err(e,"Fresh original Library source"))?;
        if self.consumer_program.source.as_ref()!=Some(&binding){return Err(self.scene_resource_error("Fresh Library program differs from the actual retained Source binding".into()))}
        // validate_emitted above is intentionally conditional on scheduled
        // plans. Fresh also retains ordinary/pure Library programs with no
        // plan: bind their actual lowered instructions to this SAME frozen
        // SourceMap, as StartupInputProgram::validate already does.
        let emitted=crate::debugger_provenance::SourceMap::emitted_identity(&self.script).map_err(|e|self.scene_resource_error(e))?;
        if emitted!=binding.emitted{return Err(self.scene_resource_error("Fresh Library executable differs from its actual frozen Source program".into()))}
        self.functions.validate_engine_unit_owner(&self.script).map_err(|e|self.eval_err(e,"Fresh retained Library definitions"))?;
        Ok(())
    }
    /// Re-admit the original immutable source, never print an AST or reread disk.
    /// This returns a pending owner; it performs no Init and clears no old UI.
    pub fn prepare_fresh_startup(&self,renderer:R,rollback_depth:usize)->Result<EngineStartup<R>,RuntimeError>{
        let runtime=self.runtime_epoch.checked_add(1).ok_or_else(||self.eval_err(EvalError::NumericOverflow,"Fresh runtime epoch"))?;
        let interface=self.interface_epoch.checked_add(1).ok_or_else(||self.eval_err(EvalError::NumericOverflow,"Fresh interface epoch"))?;
        let video=self.video_epoch.checked_add(1).ok_or_else(||self.eval_err(EvalError::NumericOverflow,"Fresh video epoch"))?;
        let program=self.frozen_startup_program.as_ref().ok_or_else(||self.scene_resource_error("Fresh Startup requires the actual retained frozen ParsedProgram".into()))?;
        if self.functions.library_assets1023().is_empty(){
            // Keep the exact legacy/Enum/Widget admission, including portable
            // Unit refusals. A missing Library resource never gains a new owner.
            self.validate_save15_program(program).map_err(|error|self.scene_resource_error(error.to_string()))?;
            return Self::prepare_owned_startup_enum_in1025(std::sync::Arc::clone(program),renderer,rollback_depth,self.persistent_vars.clone(),TextRuntimeContext{resources:self.functions.text_resources.clone(),locale:self.locale.clone()},None,(runtime,interface,video),self.consumer_program.startup_input.clone(),self.functions.enum_assets1025.clone())?.with_widget_classes(self.widget_classes.clone());
        }
        // Warm Fresh is an original-engine transition, not a portable Save15
        // restore. Keep its source/consumer/budget fences without granting the
        // unrelated portable Unit serializer any new admission.
        self.validate_frozen_library_fresh1027(program)?;
        let functions=super::super::function_libraries1023::startup_joint1027::prepare_fresh_functions1027(program.as_ref().clone(),self.functions.library_assets1023())?.with_enum_assets1025(self.functions.enum_assets1025.clone());
        Self::prepare_owned_startup_library_enum_in1027(std::sync::Arc::clone(program),renderer,rollback_depth,self.persistent_vars.clone(),TextRuntimeContext{resources:self.functions.text_resources.clone(),locale:self.locale.clone()},None,(runtime,interface,video),self.consumer_program.startup_input.clone(),self.functions.enum_assets1025.clone(),Some(functions))?.with_widget_classes(self.widget_classes.clone())
    }
}
impl<R:Renderer> EngineStartup<R>{
    pub(in crate::engine) fn finish_synchronous(mut self)->Result<Engine<R>,RuntimeError>{
        while !self.is_complete(){
            if self.waiting_for_imagemap(){return Err(self.engine.scene_resource_error("This synchronous constructor reached an actual Init input wait; its capable host must retain EngineStartup".into()));}
            if self.waiting_for_video(){return Err(self.engine.scene_resource_error("This synchronous constructor reached a real media wait; the host must retain and drive EngineStartup".into()));}
            self.step(StepKind::Continue)?;
        }
        self.into_engine()
    }
}
