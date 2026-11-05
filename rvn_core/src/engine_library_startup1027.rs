//! One actual pending Engine owner for Library + Enum + Widget resources.
//! This prepares no new VM, Source/Registry grant or Unit Startup continuation.
use super::*;
use crate::enum_runtime_resources1025::EnumAssets1025;
use crate::widget_class_source_0950::AdmittedWidgetClasses;
use std::sync::Arc;
fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::no_stmt(
        crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(message.into())),
        0,
    )
}
/// Only Library Unit declarations and later synchronous scene calls are newly
/// distinguishable here. Legacy Unit14 retained guards remain unchanged. All
/// REAL Init roots are examined, including controls/scheduled/macro branches;
/// an actual Unit call there still has no admitted retained continuation.
pub(in crate::engine) fn validate_retained_library_units1027(
    functions: &FunctionLibrary,
    script: &[Statement],
) -> Result<(), EvalError> {
    if functions.library_assets1023().is_empty() {
        return Err(EvalError::InvalidFunction(
            "Unit calls have no admitted retained Startup/debug/save continuation".into(),
        ));
    }
    for root in script {
        if let Statement::Init { body } = root {
            if FunctionLibrary::contains_unit_source(body) {
                return Err(EvalError::InvalidFunction(
                    "Actual Init Unit call has no admitted retained Startup continuation".into(),
                ));
            }
        }
    }
    functions.validate_engine_unit_owner(script)?;
    let mut pending = script.iter().collect::<Vec<_>>();
    while let Some(statement) = pending.pop() {
        match statement {
            Statement::FunctionLibrary { .. } => continue,
            Statement::UnitFunction { .. } | Statement::UnitCall { .. } | Statement::UnitReturn => {
                return Err(EvalError::InvalidFunction(
                    "Legacy Unit14 retained Startup admission is unchanged".into(),
                ))
            }
            _ => {
                for child in crate::consumer_schedule::statement_children(statement) {
                    pending.extend(child.iter());
                }
            }
        }
    }
    Ok(())
}
fn prepare_joint(
    program: rvn_parser::ParsedProgram,
    libraries: &[FunctionLibraryAsset],
    enums: EnumAssets1025,
    classes: &AdmittedWidgetClasses,
) -> Result<
    (
        Arc<rvn_parser::ParsedProgram>,
        FunctionLibrary,
        Arc<EnumAssets1025>,
    ),
    RuntimeError,
> {
    if !classes.matches_program(&program).map_err(invalid)? {
        return Err(invalid(
            "Widget classes differ from the actual joint startup program",
        ));
    }
    let prepared = PreparedFunctionLibraryProgram1023::prepare(program, libraries)?;
    let enums = Arc::new(enums);
    crate::eval::validate_enum_program_literals1027(&prepared.program.script, &enums)
        .map_err(|e| RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(e), 0))?;
    // From-script admission validates mode/private/signature/body identities;
    // Enum resources are installed before any Init expression can execute.
    let functions = prepared.functions.with_enum_assets1025(enums.clone());
    if FunctionLibrary::contains_unit_source(&prepared.program.script) {
        validate_retained_library_units1027(&functions, &prepared.program.script)
            .map_err(|e| RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(e), 0))?;
    }
    Ok((Arc::new(prepared.program), functions, enums))
}
/// Warm original-engine preparation only. The sibling runtime module cannot
/// extract private Prepared fields or manufacture a portable owner grant.
pub(in crate::engine) fn prepare_fresh_functions1027(program:rvn_parser::ParsedProgram,libraries:&[FunctionLibraryAsset])->Result<FunctionLibrary,RuntimeError>{
    let prepared=PreparedFunctionLibraryProgram1023::prepare(program,libraries)?;
    if FunctionLibrary::contains_unit_source(&prepared.program.script){validate_retained_library_units1027(&prepared.functions,&prepared.program.script).map_err(|e|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(e),0))?;}
    Ok(prepared.functions)
}
impl<R: Renderer> Engine<R> {
    /// Synchronous use of the same admitted pending owner. No public bypass of
    /// the retained startup/input gates or private completion API is introduced.
    pub fn from_library_enum_program1027(
        program: rvn_parser::ParsedProgram,
        libraries: &[FunctionLibraryAsset],
        enums: EnumAssets1025,
        classes: AdmittedWidgetClasses,
        renderer: R,
        rollback_depth: usize,
        persistent: HashMap<String, Value>,
        text: TextRuntimeContext,
    ) -> Result<Self, RuntimeError> {
        Self::prepare_library_enum_startup1027(program,libraries,enums,classes,renderer,rollback_depth,persistent,text,None)?.finish_synchronous()
    }

    pub fn prepare_library_enum_startup1027(
        program: rvn_parser::ParsedProgram,
        libraries: &[FunctionLibraryAsset],
        enums: EnumAssets1025,
        classes: AdmittedWidgetClasses,
        renderer: R,
        rollback_depth: usize,
        persistent: HashMap<String, Value>,
        text: TextRuntimeContext,
        overlay: Option<(crate::debugger_provenance::Overlay, u64)>,
    ) -> Result<EngineStartup<R>, RuntimeError> {
        let (program, functions, enums) = prepare_joint(program, libraries, enums, &classes)?;
        Self::prepare_owned_startup_library_enum_in1027(
            program,
            renderer,
            rollback_depth,
            persistent,
            text,
            overlay,
            (0, 0, 0),
            None,
            enums,
            Some(functions),
        )?
        .with_widget_classes(Arc::new(classes))
    }
}
impl<R: StartupInputBatchSink> Engine<R> {
    pub fn prepare_library_enum_input_startup1027(
        program: rvn_parser::ParsedProgram,
        libraries: &[FunctionLibraryAsset],
        enums: EnumAssets1025,
        classes: AdmittedWidgetClasses,
        renderer: R,
        rollback_depth: usize,
        persistent: HashMap<String, Value>,
        text: TextRuntimeContext,
        overlay: Option<(crate::debugger_provenance::Overlay, u64)>,
    ) -> Result<EngineStartup<R>, RuntimeError> {
        let (program, functions, enums) = prepare_joint(program, libraries, enums, &classes)?;
        renderer.validate_startup_input_host().map_err(invalid)?;
        let input = crate::consumer_schedule::StartupInputProgram::from_frozen(program.clone())
            .map_err(|e| RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(e), 0))?;
        Self::prepare_owned_startup_library_enum_in1027(
            program,
            renderer,
            rollback_depth,
            persistent,
            text,
            overlay,
            (0, 0, 0),
            Some(Arc::new(input)),
            enums,
            Some(functions),
        )?
        .with_widget_classes(Arc::new(classes))
    }
}
#[cfg(test)]
#[path = "engine_library_startup_tests1027.rs"]
mod tests;
