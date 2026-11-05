//! A new nominal call's structural authoring, not strict Compile or publication.
//! Real pins/opaque parameters are created empty; existing diagnostics survive.
use crate::*;
use rvn_parser::{
    function_library_asset1023::QualifiedLibraryFunction,
    function_library_program1023::LibraryCallMode, Expr, Statement,
};
impl GraphDocument {
    pub fn prepare_library_call_node1027(
        &self,
        target: QualifiedLibraryFunction,
        position: [f64; 2],
        resources: &LibraryGraphResources1027<'_>,
    ) -> Result<(GraphDocument, NodeId), String> {
        if self.schema_version < GRAPH_SCHEMA_VERSION
            || self.schema_version > GRAPH_MAX_SUPPORTED_SCHEMA_VERSION
            || position.iter().any(|n| !n.is_finite())
        {
            return Err(
                "Library call authoring needs a supported original graph and finite position"
                    .into(),
            );
        }
        self.validate_library_metadata1027()?;
        self.validate_nominal_enum_shape1026()?;
        let member = resources.member(&target)?;
        let (kind, mode) = match member.declaration() {
            Statement::Function { .. } => (NodeKind::FunctionCall, LibraryCallMode::Value),
            Statement::UnitFunction { .. } => (NodeKind::UnitCall, LibraryCallMode::Completion),
            _ => return Err("Qualified target has no actual callable family".into()),
        };
        // Source parameter names here are structural witnesses only. They are
        // never evaluated, emitted or installed as a pin default/value.
        let signature = member
            .parameter_ids()
            .iter()
            .zip(member.parameters())
            .map(|(id, name)| (id.clone(), Expr::Var(name.clone())))
            .collect::<Vec<_>>();
        resources.resolve(self, &target, mode, &signature)?;
        self.check_allocation_capacity(
            1,
            member.parameter_ids().len() + if kind == NodeKind::UnitCall { 2 } else { 1 },
            0,
        )
        .map_err(|e| e.to_string())?;
        let mut next = self.clone();
        let node = next
            .add_catalog_node(kind, position)
            .map_err(|e| e.to_string())?;
        let mut arguments = Vec::new();
        for (index, (parameter, name)) in member
            .parameter_ids()
            .iter()
            .zip(member.parameters())
            .enumerate()
        {
            let key = if kind == NodeKind::UnitCall {
                format!("argument_{}", parameter.as_str())
            } else {
                format!("item_{index}")
            };
            let pin = next
                .add_pin(
                    node,
                    key,
                    name.clone(),
                    PinDirection::Input,
                    ValueType::Any,
                    PinCardinality::One,
                )
                .map_err(|e| e.to_string())?;
            if kind == NodeKind::UnitCall {
                next.pins.get_mut(&pin).unwrap().parameter_id =
                    Some(ParameterId::parse(parameter.as_str())?);
            }
            arguments.push((parameter.clone(), pin));
        }
        next.nodes.get_mut(&node).unwrap().properties.insert(
            "function".into(),
            PropertyValue::String(member.name().to_owned()),
        );
        if kind == NodeKind::FunctionCall {
            next.nodes.get_mut(&node).unwrap().properties.insert(
                "input_count".into(),
                PropertyValue::Int(arguments.len() as i64),
            );
        }
        next.schema_version = GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION;
        next.library1027
            .get_or_insert_with(Default::default)
            .calls
            .insert(node, QualifiedLibraryCallBinding1027 { target, arguments });
        if kind == NodeKind::FunctionCall {
            next.refresh_library_result_type1027(node, resources)?;
        }
        // Structural identity/pin checks only. Calling validate_graph or the
        // complete compiler here would erase the ability to author empty args
        // in an already diagnosed graph. Those gates remain on Compile/Save.
        next.validate_library_metadata1027()?;
        next.validate_nominal_enum_shape1026()?;
        Ok((next, node))
    }
}
#[cfg(test)]
#[path = "library_call_authoring_tests1027.rs"]
mod tests;
