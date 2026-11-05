//! Internal frozen-program proof for the dedicated owned Startup input path.
//! This is not serialized, not an input event, and not a publication receipt.
use super::*;
#[derive(Clone,Debug)]
pub(crate) struct StartupInputProgram {
    program:std::sync::Arc<rvn_parser::ParsedProgram>,
    binding:ConsumerSourceBinding,
}
impl StartupInputProgram {
    /// Called by the dedicated StartupInputBatchSink constructor before an
    /// Engine exists or any Init producer runs. The generic constructors do
    /// not create this proof and keep their historical Init input refusal.
    pub(crate) fn from_frozen(program:std::sync::Arc<rvn_parser::ParsedProgram>)->EvalResult<Self>{
        crate::save::consumer_checkpoint::preflight_program(&program).map_err(|e|invalid(&e.to_string()))?;
        let binding=ConsumerSourceBinding::from_program(&program)?;
        Ok(Self{program,binding})
    }
    pub(crate) fn validate(&self,script:&[Statement],source:Option<&ConsumerSourceBinding>)->EvalResult<()>{
        if source!=Some(&self.binding){return Err(invalid("Startup input programme has no exact frozen source binding"));}
        // Both validation passes are real: the authored AST is checked before
        // lowering; the executable pass must have its exact frozen emitted IR.
        if script!=self.program.script.as_slice()
            &&crate::debugger_provenance::SourceMap::emitted_identity(script).map_err(|e|invalid(&e))?!=self.binding.emitted{
            return Err(invalid("Startup input programme differs from its actual authored/emitted source"));
        }
        Ok(())
    }
    pub(crate) fn program(&self)->&std::sync::Arc<rvn_parser::ParsedProgram>{&self.program}
}
