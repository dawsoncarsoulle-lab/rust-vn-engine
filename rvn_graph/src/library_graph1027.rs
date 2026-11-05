//! Source-scoped Library projection over the existing graph, pins and emitter.
//! Immutable borrowed resources are no Source/Registry/Journal permission.
use crate::*;
use rvn_parser::function_library_asset1023::{
    FunctionLibraryAsset, FunctionLibraryId, QualifiedLibraryFunction,
};
use rvn_parser::function_library_program1023::{
    FunctionLibraryScope, LibraryCallMode, LibraryMemberSource, LibraryParameterId,
    QualifiedLibraryCall, ScopedLibraryBindings,
};
use rvn_parser::{Expr, Statement};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualifiedLibraryCallBinding1027 {
    pub target: QualifiedLibraryFunction,
    /// Actual input PinIds, keyed by original declaration ParameterId. No name or ordinal resolution.
    pub arguments: Vec<(LibraryParameterId, PinId)>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryGraphMetadata1027 {
    pub owner: Option<FunctionLibraryId>,
    /// Value functions need this explicit source identity; Unit entries retain their actual pin identities too.
    pub entry_parameter_ids: Vec<LibraryParameterId>,
    pub calls: BTreeMap<NodeId, QualifiedLibraryCallBinding1027>,
}

pub struct LibraryGraphResources1027<'a> {
    scopes: &'a [FunctionLibraryScope],
    assets: &'a [FunctionLibraryAsset],
}
impl<'a> LibraryGraphResources1027<'a> {
    pub fn prepare(
        scopes: &'a [FunctionLibraryScope],
        assets: &'a [FunctionLibraryAsset],
    ) -> Result<Self, String> {
        ScopedLibraryBindings::prepare(scopes, assets)?;
        Ok(Self { scopes, assets })
    }
    pub fn scopes(&self) -> &'a [FunctionLibraryScope] {
        self.scopes
    }
    pub fn assets(&self) -> &'a [FunctionLibraryAsset] {
        self.assets
    }
    /// Actual dynamic RVN parameters stay generic when unknown. A missing target
    /// never reaches this analysis and never acquires Any as an admission fallback.
    pub(crate) fn result_type1027(
        &self,
        target: &QualifiedLibraryFunction,
        arguments: &[ValueType],
    ) -> Result<ValueType, String> {
        self.result_type_depth1027(target, arguments, 0)
    }
    fn source_expression_type1027(
        &self,
        expr: &Expr,
        vars: &BTreeMap<String, VariableDefinition>,
        depth: usize,
    ) -> Result<ValueType, String> {
        self.source_expression_type_resolved1027(expr, vars, depth, &|_| None)
    }
    /// Qualified source type evidence plus the caller's existing lexical
    /// capture evidence. No expression is evaluated and no value is supplied.
    pub(crate) fn source_expression_type_with_resolver1027(
        &self, expr: &Expr, vars: &BTreeMap<String, VariableDefinition>,
        resolver: &impl Fn(&Expr) -> Option<ValueType>,
    ) -> Result<ValueType, String> {
        self.source_expression_type_resolved1027(expr, vars, 0, resolver)
    }
    fn source_expression_type_resolved1027(
        &self, expr: &Expr, vars: &BTreeMap<String, VariableDefinition>, depth: usize,
        resolver: &impl Fn(&Expr) -> Option<ValueType>,
    ) -> Result<ValueType, String> {
        if depth > 128 {
            return Err(
                "Qualified result analysis is recursive or exceeds the existing nesting bound"
                    .into(),
            );
        }
        let failure = std::cell::RefCell::new(None);
        let value = crate::type_inference::expression_type_with_resolver(expr, vars, &|e| {
            if let Some(ty) = resolver(e) { return Some(ty); }
            let Expr::LibraryCall { call } = e else {
                return None;
            };
            let result = (|| {
                let member = self.member(call.reference())?;
                let args = member
                    .parameter_ids()
                    .iter()
                    .map(|id| {
                        let (_, arg) = call
                            .arguments()
                            .iter()
                            .find(|(p, _)| p == id)
                            .ok_or("Original result argument identity missing")?;
                        self.source_expression_type_resolved1027(arg, vars, depth + 1, resolver)
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                self.result_type_depth1027(call.reference(), &args, depth + 1)
            })();
            Some(match result {
                Ok(v) => v,
                Err(e) => {
                    *failure.borrow_mut() = Some(e);
                    ValueType::Any
                }
            })
        });
        if let Some(error) = failure.into_inner() {
            return Err(error);
        }
        Ok(value)
    }
    fn result_type_depth1027(
        &self,
        target: &QualifiedLibraryFunction,
        arguments: &[ValueType],
        depth: usize,
    ) -> Result<ValueType, String> {
        if depth > 128 {
            return Err(
                "Qualified result analysis is recursive or exceeds the existing nesting bound"
                    .into(),
            );
        }
        let member = self.member(target)?;
        if member.parameters().len() != arguments.len() {
            return Err("Actual Library type analysis arity differs".into());
        }
        let Statement::Function { body, .. } = member.declaration() else {
            return Err("Completion-only Library has no data result".into());
        };
        let mut vars = member
            .parameters()
            .iter()
            .zip(arguments)
            .map(|(name, ty)| {
                (
                    name.clone(),
                    VariableDefinition {
                        name: name.clone(),
                        value_type: ty.clone(),
                        default_value: PropertyValue::Int(0),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        fn walk(
            resources: &LibraryGraphResources1027<'_>,
            body: &[Statement],
            vars: &mut BTreeMap<String, VariableDefinition>,
            out: &mut Vec<ValueType>,
            depth: usize,
        ) -> Result<(), String> {
            if depth > 128 {
                return Err(
                    "Library result analysis nesting exceeds existing bounded graph contract"
                        .into(),
                );
            }
            for statement in body {
                match statement {
                    Statement::SetVar { name, value } | Statement::LocalVar { name, value } => {
                        let ty = resources.source_expression_type1027(value, vars, depth + 1)?;
                        vars.insert(
                            name.clone(),
                            VariableDefinition {
                                name: name.clone(),
                                value_type: ty,
                                default_value: PropertyValue::Int(0),
                            },
                        );
                    }
                    Statement::FunctionReturn { value } => {
                        out.push(resources.source_expression_type1027(value, vars, depth + 1)?)
                    }
                    Statement::If {
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        walk(resources, then_branch, &mut vars.clone(), out, depth + 1)?;
                        walk(resources, else_branch, &mut vars.clone(), out, depth + 1)?;
                    }
                    Statement::Sequence { branches } => {
                        for branch in branches {
                            walk(resources, branch, &mut vars.clone(), out, depth + 1)?;
                        }
                    }
                    Statement::While { body, .. }
                    | Statement::ForEach { body, .. }
                    | Statement::ForLoopWithBreak { body, .. }
                    | Statement::ScheduledConsumer { body, .. } => {
                        walk(resources, body, &mut vars.clone(), out, depth + 1)?
                    }
                    _ => {}
                }
            }
            Ok(())
        }
        let mut values = Vec::new();
        walk(self, body, &mut vars, &mut values, depth + 1)?;
        let first = values
            .first()
            .ok_or("Actual Library value declaration has no return")?
            .clone();
        Ok(if values.iter().all(|v| *v == first) {
            first
        } else {
            ValueType::Any
        })
    }
    pub fn member(
        &self,
        target: &QualifiedLibraryFunction,
    ) -> Result<&'a LibraryMemberSource, String> {
        target.validate()?;
        self.scopes
            .iter()
            .find(|s| s.library() == &target.library)
            .and_then(|s| s.members().iter().find(|m| m.graph_id() == target.graph_id))
            .ok_or_else(|| {
                "Original Library UUID/member GraphId is missing; no name fallback".into()
            })
    }
    pub(crate) fn caller(
        &self,
        graph: &GraphDocument,
    ) -> Result<Option<QualifiedLibraryFunction>, String> {
        let Some(owner) = graph.library1027.as_ref().and_then(|m| m.owner.as_ref()) else {
            return Ok(None);
        };
        let target = QualifiedLibraryFunction {
            library: owner.clone(),
            graph_id: graph.graph_id.get(),
        };
        let member = self.member(&target)?;
        let kind = match member.declaration() {
            Statement::Function { name, .. } => GraphKind::Function { name: name.clone() },
            Statement::UnitFunction { name, .. } => GraphKind::UnitFunction { name: name.clone() },
            _ => return Err("Original Library source member is not a Function/Unit".into()),
        };
        if graph.kind != kind
            || graph.library1027.as_ref().unwrap().entry_parameter_ids != member.parameter_ids()
        {
            return Err("Library graph owner/name/actual parameter identity differs from the source candidate".into());
        }
        Ok(Some(target))
    }
    pub(crate) fn resolve<'b>(
        &'b self,
        graph: &GraphDocument,
        target: &QualifiedLibraryFunction,
        mode: LibraryCallMode,
        arguments: &'b [(LibraryParameterId, Expr)],
    ) -> Result<(), String> {
        let caller = self.caller(graph)?;
        let call = QualifiedLibraryCall::new(target.clone(), arguments.to_vec())?;
        ScopedLibraryBindings::prepare(self.scopes, self.assets)?.resolve(
            &call,
            mode,
            caller.as_ref(),
        )?;
        Ok(())
    }
    pub(crate) fn validate_graph(&self, graph: &GraphDocument) -> Result<(), String> {
        graph.validate_library_metadata1027()?;
        self.caller(graph)?;
        let Some(meta) = &graph.library1027 else {
            return Ok(());
        };
        if let Some(owner) = &meta.owner {
            let member = self.member(&QualifiedLibraryFunction {
                library: owner.clone(),
                graph_id: graph.graph_id.get(),
            })?;
            let entry = graph
                .nodes
                .values()
                .find(|n| n.kind == NodeKind::FunctionEntry)
                .ok_or("Library entry missing")?;
            let names = if matches!(graph.kind, GraphKind::UnitFunction { .. }) {
                crate::unit_graph_source1003::parameters(graph)?
                    .iter()
                    .map(|p| p.label.clone())
                    .collect::<Vec<_>>()
            } else {
                match entry.properties.get("parameters") {
                    Some(PropertyValue::StringList(v)) => v.clone(),
                    _ => return Err("Actual value entry parameter names missing".into()),
                }
            };
            if names != member.parameters() {
                return Err(
                    "Library declaration parameter names/order differ from source candidate".into(),
                );
            }
        }
        for (node, call) in &meta.calls {
            let mode = match graph.nodes[node].kind {
                NodeKind::FunctionCall => LibraryCallMode::Value,
                NodeKind::UnitCall => LibraryCallMode::Completion,
                _ => return Err("Qualified call metadata is attached to a noncall node".into()),
            };
            let args = call
                .arguments
                .iter()
                .map(|(p, _)| (p.clone(), Expr::Int(0)))
                .collect::<Vec<_>>();
            self.resolve(graph, &call.target, mode, &args)?;
            if mode == LibraryCallMode::Value {
                let types = graph.effective_pin_types();
                let member = self.member(&call.target)?;
                let ordered = member
                    .parameter_ids()
                    .iter()
                    .map(|id| {
                        call.arguments
                            .iter()
                            .find(|(p, _)| p == id)
                            .and_then(|(_, pin)| types.get(pin))
                            .cloned()
                            .ok_or_else(|| "Original argument type slot missing".to_owned())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let expected = self.result_type1027(&call.target, &ordered)?;
                let output = graph.nodes[node]
                    .pins
                    .iter()
                    .filter_map(|p| graph.pins.get(p))
                    .find(|p| p.direction == PinDirection::Output && !p.value_type.is_execution())
                    .ok_or("Real Library value output missing")?;
                if output.value_type != expected {
                    return Err("Library value output is stale or differs from actual scoped source analysis".into());
                }
            }
        }
        Ok(())
    }
    pub fn validate_project(&self, graphs: &[GraphDocument]) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        for graph in graphs {
            if !ids.insert(graph.graph_id) {
                return Err("Duplicate actual graph identity in Library source cohort".into());
            }
            self.validate_graph(graph)?;
        }
        let mut parameter_owners = BTreeMap::new();
        for graph in graphs {
            let identities = graph
                .library1027
                .as_ref()
                .into_iter()
                .flat_map(|m| m.entry_parameter_ids.iter().map(|p| p.as_str()))
                .chain(
                    graph
                        .pins
                        .values()
                        .filter(|p| p.direction == PinDirection::Output)
                        .filter_map(|p| p.parameter_id.as_ref().map(|id| id.as_str())),
                );
            for id in identities {
                if parameter_owners
                    .insert(id, graph.graph_id)
                    .is_some_and(|old| old != graph.graph_id)
                {
                    return Err("Actual Library and legacy declarations cannot share one opaque parameter identity".into());
                }
            }
        }
        for scope in self.scopes {
            for member in scope.members() {
                let matches = graphs
                    .iter()
                    .filter(|g| g.graph_id.get() == member.graph_id())
                    .collect::<Vec<_>>();
                let [g] = matches.as_slice() else {
                    return Err("Original Library member graph is absent or ambiguous in the actual source cohort".into());
                };
                if g.library1027.as_ref().and_then(|m| m.owner.as_ref()) != Some(scope.library()) {
                    return Err("Library member cannot become a legacy global declaration".into());
                }
            }
        }
        Ok(())
    }
}

impl GraphDocument {
    /// Explicit membership adoption by the real source candidate; loading legacy
    /// graphs allocates/promotes nothing. Descriptor + source signature must agree.
    pub fn set_library_member1027(
        &mut self,
        target: QualifiedLibraryFunction,
        resources: &LibraryGraphResources1027<'_>,
    ) -> Result<(), String> {
        if self.graph_id.get() != target.graph_id {
            return Err("Membership cannot retarget the actual Source GraphId".into());
        }
        if self
            .library1027
            .as_ref()
            .and_then(|m| m.owner.as_ref())
            .is_some_and(|owner| *owner != target.library)
        {
            return Err("Membership transfer requires the original Source transaction, never a label-based retarget".into());
        }
        let member = resources.member(&target)?;
        let mut next = self.clone();
        next.schema_version = GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION;
        let metadata = next.library1027.get_or_insert_with(Default::default);
        metadata.owner = Some(target.library);
        metadata.entry_parameter_ids = member.parameter_ids().to_vec();
        resources.validate_graph(&next)?;
        if let Some(d) = next
            .validate()
            .iter()
            .find(|d| d.severity == DiagnosticSeverity::Error)
        {
            return Err(d.message.clone());
        }
        *self = next;
        Ok(())
    }
    pub fn validate_library_metadata1027(&self) -> Result<(), String> {
        let Some(meta) = &self.library1027 else {
            if self.schema_version == GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION {
                return Err("Schema16 requires its explicit Library metadata, including ordinary caller ownership".into());
            }
            return Ok(());
        };
        if self.schema_version != GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION {
            return Err("Nominal Library metadata requires explicit graph schema16; old documents are never promoted during decode".into());
        }
        if meta.owner.is_none() && !meta.entry_parameter_ids.is_empty() {
            return Err("A nonmember cannot claim Library parameter identities".into());
        }
        if meta.owner.is_some()
            && !matches!(
                self.kind,
                GraphKind::Function { .. } | GraphKind::UnitFunction { .. }
            )
        {
            return Err("Library owner is valid only on a real Function/Unit graph".into());
        }
        if meta.entry_parameter_ids.len() > 128
            || meta
                .entry_parameter_ids
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != meta.entry_parameter_ids.len()
        {
            return Err("Library entry identity count or uniqueness is invalid".into());
        }
        if meta.owner.is_some() && matches!(self.kind, GraphKind::UnitFunction { .. }) {
            let ids = crate::unit_graph_source1003::parameters(self)?
                .iter()
                .map(|p| {
                    LibraryParameterId::parse(
                        p.parameter_id
                            .as_ref()
                            .ok_or("Original Unit pin ParameterId absent")?
                            .as_str(),
                    )
                })
                .collect::<Result<Vec<_>, String>>()?;
            if ids != meta.entry_parameter_ids {
                return Err(
                    "Library Unit parameter IDs must be exactly the actual ordered entry pin IDs"
                        .into(),
                );
            }
        }
        for (node, call) in &meta.calls {
            call.target.validate()?;
            let owner = self
                .nodes
                .get(node)
                .ok_or("Qualified target has no actual call node")?;
            if !matches!(owner.kind, NodeKind::FunctionCall | NodeKind::UnitCall) {
                return Err("Qualified target requires actual existing call-node kind".into());
            }
            let mut seen = BTreeSet::new();
            let mut params = BTreeSet::new();
            for (parameter, id) in &call.arguments {
                let pin = self
                    .pins
                    .get(id)
                    .ok_or("Qualified argument PinId is absent")?;
                if !seen.insert(*id)
                    || !params.insert(parameter)
                    || pin.node != *node
                    || pin.direction != PinDirection::Input
                    || pin.value_type.is_execution()
                    || !owner.pins.contains(id)
                {
                    return Err("Qualified parameter-to-pin mapping has duplicate/stale/foreign input ownership".into());
                }
                if owner.kind == NodeKind::UnitCall
                    && pin.parameter_id.as_ref().map(|v| v.as_str()) != Some(parameter.as_str())
                {
                    return Err("Qualified Unit input lost its actual parameter identity".into());
                }
            }
            let actual = owner
                .pins
                .iter()
                .filter(|id| {
                    self.pins.get(id).is_some_and(|p| {
                        p.direction == PinDirection::Input && !p.value_type.is_execution()
                    })
                })
                .copied()
                .collect::<BTreeSet<_>>();
            if actual != seen {
                return Err(
                    "Every real data input must retain one exact original Library ParameterId"
                        .into(),
                );
            }
        }
        Ok(())
    }
    /// Explicit caller intent only. The original Source/Journal transaction still owns publication.
    pub fn set_library_call_target1027(
        &mut self,
        node: NodeId,
        target: QualifiedLibraryFunction,
        resources: &LibraryGraphResources1027<'_>,
    ) -> Result<(), String> {
        let mut next = self.clone();
        let member = resources.member(&target)?;
        let owner = next.nodes.get(&node).ok_or("Actual call node absent")?;
        let mode = match owner.kind {
            NodeKind::FunctionCall => LibraryCallMode::Value,
            NodeKind::UnitCall => LibraryCallMode::Completion,
            _ => return Err("Actual FunctionCall/UnitCall required".into()),
        };
        let actual = owner
            .pins
            .iter()
            .filter_map(|id| next.pins.get(id))
            .filter(|p| p.direction == PinDirection::Input && !p.value_type.is_execution())
            .collect::<Vec<_>>();
        // Never guess a retarget mapping from argument position. Existing binding or
        // Unit opaque pin IDs must already identify each original parameter.
        let old = next.library1027.as_ref().and_then(|m| m.calls.get(&node));
        let arguments=member.parameter_ids().iter().map(|parameter|{
   let matches=actual.iter().filter(|p|p.parameter_id.as_ref().is_some_and(|id|id.as_str()==parameter.as_str())||old.is_some_and(|old|old.arguments.iter().any(|(id,pin)|id==parameter&&*pin==p.id))).collect::<Vec<_>>();
   let[pin]=matches.as_slice()else{return Err("Retarget requires explicit original parameter-to-pin identity mapping, not positional reconstruction".into())};Ok((parameter.clone(),pin.id))
  }).collect::<Result<Vec<_>,String>>()?;
        if actual.len() != arguments.len() {
            return Err("Retarget would discard/add actual data inputs".into());
        }
        let dummy = arguments
            .iter()
            .map(|(p, _)| (p.clone(), Expr::Int(0)))
            .collect::<Vec<_>>();
        resources.resolve(&next, &target, mode, &dummy)?;
        next.nodes.get_mut(&node).unwrap().properties.insert(
            "function".into(),
            PropertyValue::String(member.name().to_owned()),
        );
        next.schema_version = GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION;
        next.library1027
            .get_or_insert_with(Default::default)
            .calls
            .insert(node, QualifiedLibraryCallBinding1027 { target, arguments });
        if mode == LibraryCallMode::Value {
            next.refresh_library_result_type1027(node, resources)?;
        }
        resources.validate_graph(&next)?;
        if let Some(d) = next
            .validate()
            .iter()
            .find(|d| d.severity == DiagnosticSeverity::Error)
        {
            return Err(d.message.clone());
        }
        *self = next;
        Ok(())
    }
    /// Editor/Source producers supply the exact mapping selected from original
    /// parameter identities and actual pins. Missing or extra inputs fail atomically.
    pub fn set_library_call_arguments1027(
        &mut self,
        node: NodeId,
        target: QualifiedLibraryFunction,
        arguments: Vec<(LibraryParameterId, PinId)>,
        resources: &LibraryGraphResources1027<'_>,
    ) -> Result<(), String> {
        let mut next = self.clone();
        let member = resources.member(&target)?;
        next.nodes
            .get_mut(&node)
            .ok_or("Actual call node missing")?
            .properties
            .insert(
                "function".into(),
                PropertyValue::String(member.name().to_owned()),
            );
        next.schema_version = GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION;
        next.library1027
            .get_or_insert_with(Default::default)
            .calls
            .insert(node, QualifiedLibraryCallBinding1027 { target, arguments });
        next.validate_library_metadata1027()?;
        if next.nodes[&node].kind == NodeKind::FunctionCall {
            next.refresh_library_result_type1027(node, resources)?;
        }
        resources.validate_graph(&next)?;
        if let Some(d) = next
            .validate()
            .iter()
            .find(|d| d.severity == DiagnosticSeverity::Error)
        {
            return Err(d.message.clone());
        }
        *self = next;
        Ok(())
    }
    pub(crate) fn library_call1027(
        &self,
        node: NodeId,
    ) -> Option<&QualifiedLibraryCallBinding1027> {
        self.library1027.as_ref()?.calls.get(&node)
    }
    pub(crate) fn refresh_library_result_type1027(
        &mut self,
        node: NodeId,
        resources: &LibraryGraphResources1027<'_>,
    ) -> Result<(), String> {
        let call = self
            .library_call1027(node)
            .ok_or("Original target missing")?
            .clone();
        let member = resources.member(&call.target)?;
        let types = self.effective_pin_types();
        let args = member
            .parameter_ids()
            .iter()
            .map(|id| {
                call.arguments
                    .iter()
                    .find(|(p, _)| p == id)
                    .and_then(|(_, pin)| types.get(pin))
                    .cloned()
                    .ok_or_else(|| "Original argument type slot missing".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let value = resources.result_type1027(&call.target, &args)?;
        let output = self.nodes[&node]
            .pins
            .iter()
            .filter_map(|p| self.pins.get(p))
            .find(|p| p.direction == PinDirection::Output && !p.value_type.is_execution())
            .ok_or("Actual value output missing")?
            .id;
        self.pins.get_mut(&output).unwrap().value_type = value;
        Ok(())
    }
}

/// Compile real graphs with their original nominal resources. No flattening or
/// global callable alias is emitted. Ordinary exports keep their old refusal.
pub fn transpile_library_project1027(
    graphs: &[GraphDocument],
    resources: &LibraryGraphResources1027<'_>,
) -> Result<TranspiledScriptWithOrigins, String> {
    resources.validate_project(graphs)?;
    crate::validate_text_owners(graphs)?;
    crate::unit_graph_source1003::validate_project(graphs)?;
    let mut source = String::new();
    let mut mappings = Vec::new();
    fn record(
        origin: &GraphStatementOrigin,
        offset: usize,
        out: &mut Vec<(std::ops::Range<usize>, GraphId, NodeId)>,
    ) {
        if let (Some(g), Some(n)) = (origin.graph, origin.node) {
            out.push((
                origin.span.range.start + offset..origin.span.range.end + offset,
                g,
                n,
            ));
        }
        for c in &origin.children {
            record(c, offset, out)
        }
    }
    for original in resources.scopes {
        let mut compiled = BTreeMap::new();
        let mut members = Vec::new();
        for member in original.members() {
            let graph = graphs
                .iter()
                .find(|g| g.graph_id.get() == member.graph_id())
                .ok_or("Original member graph absent")?;
            let emitted = crate::codegen::transpile_library_graph1027(graph, graphs, resources)
                .map_err(|e| e.to_string())?;
            let [declaration] = emitted.ast.as_slice() else {
                return Err("Actual Library member emitted extra top-level declarations".into());
            };
            members.push(LibraryMemberSource::new(
                graph.graph_id.get(),
                member.parameter_ids().to_vec(),
                declaration.clone(),
            )?);
            compiled.insert(member.graph_id(), emitted);
        }
        let scope = FunctionLibraryScope::new(original.library().clone(), members)?;
        let text = rvn_parser::function_library_program1023::emit_library_scope(&scope, |m| {
            Ok(compiled[&m.graph_id()].source.clone())
        })?;
        let mut cursor = 0;
        for member in scope.members() {
            let part = &compiled[&member.graph_id()];
            let at = text[cursor..]
                .find(&part.source)
                .ok_or("Original member source range missing")?
                + cursor;
            for origin in &part.origins {
                record(origin, source.len() + at, &mut mappings)
            }
            cursor = at + part.source.len();
        }
        source.push_str(&text);
    }
    let ordinary = graphs
        .iter()
        .filter(|g| {
            g.library1027
                .as_ref()
                .and_then(|m| m.owner.as_ref())
                .is_none()
        })
        .cloned()
        .collect::<Vec<_>>();
    let context = if graphs.iter().any(|g| g.narrative.is_some()) {
        rvn_parser::NarrativeContext::ExplicitScenes
    } else {
        rvn_parser::NarrativeContext::Legacy
    };
    if !ordinary.is_empty() {
        let emitted = crate::project::transpile_project_library1027(&ordinary, graphs, resources)?;
        for origin in &emitted.origins {
            record(origin, source.len(), &mut mappings)
        }
        source.push_str(&emitted.source);
    }
    let parsed = rvn_parser::parse_with_origins(&source, context).map_err(|e| e.to_string())?;
    // The real member bodies may have been edited since the borrowed Source
    // frontier. Re-admit the scopes actually emitted by those graphs and check
    // every retained caller again before returning executable source. No old
    // output-type cache can authorize a changed nominal return through Any.
    let emitted_scopes = parsed
        .script
        .iter()
        .filter_map(|statement| {
            if let Statement::FunctionLibrary { scope } = statement {
                Some(scope.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    LibraryGraphResources1027::prepare(&emitted_scopes, resources.assets)?
        .validate_project(graphs)?;
    let origins = parsed
        .spans
        .iter()
        .map(|span| crate::codegen::map_statement_origin(span, &mappings))
        .collect();
    Ok(TranspiledScriptWithOrigins {
        source,
        ast: parsed.script,
        context,
        origins,
    })
}

/// Source-provided Library scopes supply all member GraphIds and ParameterIds;
/// ordinary Source graph IDs remain the existing caller-owned import projection.
pub fn import_library_program1027(
    script: &rvn_parser::Script,
    resources: &LibraryGraphResources1027<'_>,
    previous: &[GraphDocument],
) -> Result<Vec<GraphDocument>, String> {
    let actual = script
        .iter()
        .filter_map(|s| {
            if let Statement::FunctionLibrary { scope } = s {
                Some(scope.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if actual.as_slice() != resources.scopes {
        return Err("Import resources differ from the actual frozen Source scopes".into());
    }
    let mut graphs = Vec::new();
    for scope in resources.scopes {
        for member in scope.members() {
            let owner = QualifiedLibraryFunction {
                library: scope.library().clone(),
                graph_id: member.graph_id(),
            };
            let retained = previous
                .iter()
                .filter(|g| g.graph_id.get() == member.graph_id())
                .cloned()
                .collect::<Vec<_>>();
            let imported = crate::import::import_library_scope1027(
                &vec![member.declaration().clone()],
                script,
                resources,
                Some(&owner),
                &retained,
            )?;
            let mut matched = imported
                .into_iter()
                .filter(|g| {
                    matches!(
                        g.kind,
                        GraphKind::Function { .. } | GraphKind::UnitFunction { .. }
                    )
                })
                .collect::<Vec<_>>();
            if matched.len() != 1 {
                return Err("One actual member must import exactly one Function/Unit graph".into());
            }
            let mut graph = matched.remove(0);
            if let [old] = retained.as_slice() {
                if old.library1027.as_ref().and_then(|m| m.owner.as_ref()) != Some(scope.library())
                {
                    return Err("Prior graph belongs to another original Library owner".into());
                }
                let mut exact_owner = old.clone();
                exact_owner.kind = graph.kind.clone();
                graph.reconcile_import(&exact_owner)?;
            } else if !retained.is_empty() {
                return Err("Original Library graph identity is ambiguous".into());
            }
            graphs.push(graph);
        }
    }
    let ordinary = script
        .iter()
        .filter(|s| !matches!(s, Statement::FunctionLibrary { .. }))
        .cloned()
        .collect::<Vec<_>>();
    if !ordinary.is_empty() {
        let owners = previous
            .iter()
            .filter(|g| {
                g.library1027
                    .as_ref()
                    .and_then(|m| m.owner.as_ref())
                    .is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut imported =
            crate::import::import_library_scope1027(&ordinary, script, resources, None, &owners)?;
        let reserved = graphs.iter().map(|g| g.graph_id).collect::<BTreeSet<_>>();
        let mut high = reserved
            .iter()
            .map(|id| id.get())
            .chain(previous.iter().map(|g| g.graph_id.get()))
            .max()
            .unwrap_or(0);
        for graph in &mut imported {
            if let Some(old) = owners.iter().find(|old| old.kind == graph.kind) {
                graph.graph_id = old.graph_id;
                graph.reconcile_import(old)?;
            } else {
                high = high.checked_add(1).ok_or("Graph identity exhausted")?;
                graph.graph_id = GraphId::new(high);
            }
            if reserved.contains(&graph.graph_id) {
                return Err("Actual Source graph identity collides with a Library member".into());
            }
        }
        graphs.extend(imported);
    }
    resources.validate_project(&graphs)?;
    transpile_library_project1027(&graphs, resources)?;
    Ok(graphs)
}

#[cfg(test)]
#[path = "library_graph_tests1027.rs"]
mod tests;

#[cfg(all(test,not(target_arch="wasm32")))]
#[path="library_cold_call_authoring_tests1030.rs"]
mod cold_call_authoring1030;
