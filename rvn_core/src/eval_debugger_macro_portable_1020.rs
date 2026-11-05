//! Child of the existing portable continuation codec. Rebind never rewrites
//! macro target/class/cells; immutable source data is not a new owner grant.
use super::*;
fn frame_context<'a>(
    library: &FunctionLibrary,
    request: &'a State,
    result: &'a State,
) -> EvalResult<()> {
    match (&request.widget_macro_frame, &result.widget_macro_frame) {
        (None, None) => Ok(()),
        (Some(before), Some(after)) => {
            if before.lineage() != after.lineage()
                || before.target() != after.target()
                || before.class() != after.class()
            {
                return Err(invalid(
                    "Portable macro trace changed its actual Engine/owner/source generation",
                ));
            }
            library
                .widget_macro_sites
                .validate_frame(before)
                .map_err(EvalError::InvalidFunction)?;
            library
                .widget_macro_sites
                .validate_frame(after)
                .map_err(EvalError::InvalidFunction)
        }
        _ => Err(invalid(
            "Portable macro trace invented or discarded an owner",
        )),
    }
}
pub(in crate::eval::debugger_eval) fn validate_transition(
    library: &FunctionLibrary,
    request: &State,
    result: &State,
) -> EvalResult<()> {
    frame_context(library, request, result)
}
pub(in crate::eval::debugger_eval) fn validate_tasks(
    current: &Continuation,
    state: &State,
    tasks: &[Task],
) -> EvalResult<()> {
    frame_context(&current.library, &current.request.state, state)?;
    for task in tasks {
        if let Task::MacroReady { dispatch, need, ctx }
            | Task::MacroOperandNext { dispatch, need, ctx, .. } = task
        {
            if ctx.frame != 0 || ctx.depth != 0 || state.handler_locals.is_none() || state.pure {
                return Err(invalid(
                    "Portable macro operand phase lost its actual Handler owner",
                ));
            }
            dispatch.validate().map_err(EvalError::InvalidFunction)?;
            if let Task::MacroOperandNext{index,..}=task {
                if dispatch.deferred_operand_v1.is_none()||*index>dispatch.operand_steps().len()
                    || !tasks.iter().any(|t|matches!(t,Task::ConsumerEnd{ctx:end,..}if end==ctx))
                    || !tasks.iter().any(|t|matches!(t,Task::MacroReady{ctx:ready,need:expected,dispatch:actual}if ready==ctx&&expected==need&&actual==dispatch)) {
                    return Err(invalid("Deferred macro phase lost its actual source/closing/result owner"));
                }
            }
            let frame = state
                .widget_macro_frame
                .as_ref()
                .ok_or_else(|| invalid("Portable macro phase has no Widget owner"))?;
            let site = current
                .library
                .widget_macro_sites
                .site(frame.class(), &dispatch.site)
                .map_err(EvalError::InvalidFunction)?;
            if *need == crate::widget_class_source_0950::standard_macros::OperandNeed::None
                || frame
                    .operand_need(site, dispatch.control)
                    .map_err(EvalError::InvalidFunction)?
                    != *need
            {
                return Err(invalid(
                    "Portable macro operand phase differs from its retained cell/source",
                ));
            }
        }
    }
    Ok(())
}

// Test-only consumer of the real in-memory Engine continuation. It mutates a decoded DTO copy,
// never the live owner, a Context, an AST declaration or a production capability.
#[cfg(test)]
impl FunctionLibrary {
    pub(crate) fn assert_actual_macro_trace_lineage_for_test(
        &self,
        trace: &PortableTrace,
        actual_lineage: u64,
        foreign_lineage: u64,
    ) {
        let before = &trace.pending.request.state;
        let after = &trace.pending.state;
        let before_frame = before
            .widget_macro_frame
            .as_ref()
            .expect("real stopped Handler request has its actual owner");
        let after_frame = after
            .widget_macro_frame
            .as_ref()
            .expect("real stopped macro candidate has retained cells");
        assert_eq!(before_frame.lineage(), actual_lineage);
        assert_eq!(after_frame.lineage(), actual_lineage);
        assert_ne!(actual_lineage, foreign_lineage);
        validate_transition(self, before, after).expect("the exact real transition is valid");
        let original = serde_json::to_value(after_frame).unwrap();
        let mut tampered = original.clone();
        tampered["lineage"] = serde_json::json!(foreign_lineage);
        let decoded: crate::widget_class_source_0950::standard_macros::WidgetMacroFrame =
            serde_json::from_value(tampered).unwrap();
        assert_eq!(decoded.target(), after_frame.target());
        assert_eq!(decoded.class(), after_frame.class());
        assert_eq!(
            serde_json::to_value(&decoded).unwrap()["cells"],
            original["cells"]
        );
        let mut result = after.clone();
        result.widget_macro_frame = Some(decoded);
        let error = validate_transition(self, before, &result).unwrap_err();
        assert!(
            error.to_string().contains("Engine/owner/source"),
            "reject changed lineage at the transition boundary: {error}"
        );
        validate_transition(self, before, after)
            .expect("DTO mutation grants no change to the real captured transition");
    }
}

// Cold GameSave cannot retain a process-local owner witness. This observes all
// replay roots, including completed callbacks preceding the current root.
impl Probe {
    pub(crate) fn has_widget_macro_owner(
        probe: &SharedProbe,
        continuation: Option<&Continuation>,
    ) -> EvalResult<bool> {
        let probe = probe
            .lock()
            .map_err(|_| invalid("Macro checkpoint trace lock unavailable"))?;
        Ok(continuation.is_some_and(|c| {
            c.request.state.widget_macro_frame.is_some() || c.state.widget_macro_frame.is_some()
        }) || probe
            .requested
            .as_ref()
            .is_some_and(|r| r.state.widget_macro_frame.is_some())
            || probe.records.iter().any(|r| {
                r.request.state.widget_macro_frame.is_some()
                    || r.result.state.widget_macro_frame.is_some()
            }))
    }
}
impl PortableTrace {
    pub(crate) fn has_widget_macro_owner(&self) -> bool {
        self.pending.request.state.widget_macro_frame.is_some()
            || self.pending.state.widget_macro_frame.is_some()
            || self.records.iter().any(|r| {
                r.request.state.widget_macro_frame.is_some()
                    || r.result.state.widget_macro_frame.is_some()
            })
    }
}

pub(in crate::eval::debugger_eval) fn inspector_catalog(body:&[Statement], catalog:&mut BTreeMap<String,HashSet<String>>)->EvalResult<()> {
    let mut pending=vec![body];let mut work=0usize;
    while let Some(body)=pending.pop(){for statement in body{
        work+=1;if work>100_000{return Err(invalid("Macro inspector source traversal budget"));}
        if let Statement::StandardMacroDispatch{dispatch}=statement{
            dispatch.validate().map_err(EvalError::InvalidFunction)?;
            let ids=catalog.entry(dispatch.site.scope_source()).or_default();
            ids.extend(dispatch.operand_steps().iter().filter_map(|step|match step{rvn_parser::ConsumerStep::InspectTextTable{producer,..}=>Some(producer.clone()),_=>None}));
        }
        pending.extend(crate::consumer_schedule::statement_children(statement));
    }}Ok(())
}

#[cfg(test)]
impl PortableTrace {
    /// Read only the current phase of the Engine-provided trace. This predicate
    /// grants no owner, source, replay or Save authority to an inert DTO.
    pub(crate) fn at_deferred_prefix_for_test(&self)->bool {
        self.pending.tasks.last().is_some_and(|cursor|
            cursor.kind=="macro_operand_next_v1"
            &&cursor.data.as_array().and_then(|data|data.get(1))
                .and_then(serde_json::Value::as_u64).is_some_and(|index|index>=1))
    }
    /// Observe the actual Engine-stopped continuation. Clones retain its real
    /// owner/admission; the inert modified cursor below grants no authority.
    pub(crate) fn assert_deferred_replay_for_test(&self, actual:&Continuation) {
        assert!(self.at_deferred_prefix_for_test(),"real deferred producer phase");
        let cursor=self.pending.tasks.last().unwrap();
        let index=cursor.data.as_array().unwrap()[1].as_u64().unwrap();assert!(index>=1,"completed prefix must be nonempty");
        let mut restored=actual.clone();restored.restore_portable(self,None).expect("same actual owner/source accepts hot snapshot");
        let mut control=actual.clone();
        for _ in 0..100_000 {
            assert_eq!(restored.done(),control.done());
            if control.done(){break;}
            restored.step().unwrap();control.step().unwrap();
            assert_eq!(restored.state,control.state);
            assert_eq!(restored.values,control.values);
        }
        assert!(restored.done()&&control.done());assert_eq!(restored.result,control.result);assert_eq!(restored.state.widget_macro_frame,control.state.widget_macro_frame);
        let mut tampered=self.clone();let cursor=tampered.pending.tasks.iter_mut().find(|task|task.kind=="macro_operand_next_v1").unwrap();cursor.data.as_array_mut().unwrap()[1]=serde_json::json!(0);
        let mut refused=actual.clone();let before=refused.state.clone();
        assert!(refused.restore_portable(&tampered,None).is_err(),"cursor cannot rewind around the actually completed capture prefix");assert_eq!(refused.state,before);
    }
}
