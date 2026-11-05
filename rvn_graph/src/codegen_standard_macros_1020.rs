//! Child of the existing emitter. A reached input chooses the control; the
//! macro statement is independent of ScheduledConsumer v3 and owns no cells.
use super::*;
use rvn_parser::scheduled_source::standard_macro_contract::{Control, Kind};
impl Emitter<'_> {
    pub(super) fn emit_standard_macro(
        &self,
        target: ExecutionTarget,
        indent: usize,
        source: &mut String,
        visited: &mut BTreeSet<NodeId>,
    ) -> Result<(), TranspileError> {
        let node = target.node(self.graph)?;
        let input = target.input(self.graph)?.ok_or_else(|| {
            TranspileError::GeneratedSourceInvalid(
                "A macro is reached through its real control pin, never RootEntry".into(),
            )
        })?;
        let control = crate::standard_macros_1020::control(self.graph, node, input.id)
            .map_err(TranspileError::GeneratedSourceInvalid)?;
        let family = crate::standard_macros_1020::kind(self.graph.nodes[&node].kind).unwrap();
        // This first Graph slice deliberately requires the existing full source
        // identity path for ordinary downstream nodes. It does not infer a VM
        // owner or store macro cells in that consumer frame.
        if self.native_scope.is_none() {
            return Err(TranspileError::GeneratedSourceInvalid("Macro Graph first slice requires actual source-frame preparation for complete downstream identities".into()));
        }
        let (scope, identity) = crate::standard_macros_1020::source_identity(self.graph, node)
            .map_err(TranspileError::GeneratedSourceInvalid)?;
        if self.native_scope.as_ref() != Some(&scope) {
            return Err(TranspileError::GeneratedSourceInvalid(
                "Macro source root differs from the prepared Handler root".into(),
            ));
        }
        let key = match (family, control) {
            (Kind::DoOnce, Control::Enter) | (Kind::Gate, _) => Some("start_closed"),
            (Kind::DoN, Control::Enter) => Some("n"),
            _ => None,
        };
        let mut deferred = None;
        let operand = if let Some(key) = key {
            if self.input_is_connected(node, key)? {
                let prior = self.scheduled.borrow_mut().replace(Default::default());
                let value = self.expression_from_input(node, key);
                let gathered = self.scheduled.borrow_mut().take().unwrap();
                *self.scheduled.borrow_mut() = prior;
                deferred = Some(gathered);
                Some(value?)
            } else {
                Some(self.expression_from_input(node, key)?)
            }
        } else {
            None
        };
        push_indent(source, indent);
        source.push_str(&format!(
            "flow.macro_dispatch_v1({}, {}, {}",
            quote(&scope),
            quote(&identity),
            quote(crate::standard_macros_1020::control_source(control))
        ));
        if let Some(operand) = operand {
            if let Some(plan) = deferred {
                source.push_str(", operand_v1 {\n");
                for (inspect, id, expression, _) in plan.steps {
                    push_indent(source, indent + 1);
                    source.push_str(&format!("{} {} = {}\n", if inspect { "inspect" } else { "capture" }, quote(&id), expression));
                }
                push_indent(source, indent);
                source.push_str(&format!("}} => {operand}"));
            } else { source.push_str(&format!(", {operand}")); }
        }
        source.push_str(") {\n");
        let outputs: &[&str] = if control != Control::Enter {
            &[]
        } else {
            match family {
                Kind::DoOnce => &["completed"],
                Kind::DoN | Kind::Gate => &["exit"],
                Kind::FlipFlop => &["a", "b"],
            }
        };
        for output in outputs {
            push_indent(source, indent + 1);
            source.push_str("{\n");
            self.emit_sequence(
                self.successor(node, output)?,
                indent + 2,
                Some(node),
                source,
                visited,
            )?;
            push_indent(source, indent + 1);
            source.push_str("}\n");
        }
        push_indent(source, indent);
        source.push_str("}\n");
        Ok(())
    }
}
