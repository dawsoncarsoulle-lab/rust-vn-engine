//! Declaration values belong to explicit local nodes, not the last write in
//! the inferred variable registry. This query never evaluates an initializer.
use crate::{GraphDocument, GraphId, GraphStatementOrigin, NodeId, NodeKind, PropertyValue,
    TranspileError, VariableScope};
use rvn_parser::{Expr, Statement};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct LocalVariableInitializer {
    /// Node identities are scoped to their genuine document.
    pub graph: GraphId,
    /// Genuine authoring identity; no synthetic node or detached cache.
    pub node: NodeId,
    pub expression: Expr,
    /// Only authored primitive literals (including signed numbers). A
    /// connected expression remains a calculation even if it looks constant.
    pub literal_value: Option<PropertyValue>,
}

fn literal(expression: &Expr) -> Option<PropertyValue> {
    if let Ok(Some(value)) = crate::enum_graph1026::literal_expression1026(expression) { return Some(PropertyValue::Enum(value)); }
    match expression {
        Expr::Bool(value) => Some(PropertyValue::Bool(*value)),
        Expr::Int(value) => Some(PropertyValue::Int(*value)),
        Expr::Float(value) => Some(PropertyValue::Float(*value as f64)),
        Expr::Str(value) => Some(PropertyValue::String(value.clone())),
        Expr::Call { name, args } if name == "__rvn_literal_name" => {
            let [Expr::Str(value)] = args.as_slice() else { return None; };
            crate::NameDefault::new(value).ok().map(PropertyValue::Name)
        },
        Expr::Neg(value) => match literal(value)? {
            PropertyValue::Int(value) => value.checked_neg().map(PropertyValue::Int),
            PropertyValue::Float(value) => Some(PropertyValue::Float(-value)),
            _ => None,
        },
        _ => None,
    }
}

/// Declaration presentation, never the result of executing the bootstrap.
/// No scalar fallback is invented for a calculation, collection or object.
#[derive(Debug, Clone, PartialEq)]
pub enum GlobalDeclarationValue {
    Literal(PropertyValue),
    Runtime,
}

pub(crate) fn global_declaration_values(script: &[Statement])
    -> BTreeMap<String, GlobalDeclarationValue>
{
    let mut writes: BTreeMap<String, Vec<Option<PropertyValue>>> = BTreeMap::new();
    fn collect(body: &[Statement], unconditional: bool,
        writes: &mut BTreeMap<String, Vec<Option<PropertyValue>>>)
    {
        for statement in body {
            match statement {
                Statement::SetVar { name, value } => {
                    writes.entry(name.clone()).or_default()
                        .push(unconditional.then(|| literal(value)).flatten());
                }
                Statement::If { then_branch, else_branch, .. } => {
                    collect(then_branch, false, writes);
                    collect(else_branch, false, writes);
                }
                Statement::While { body, .. } | Statement::ForEach { body, .. }
                | Statement::ForLoopWithBreak { body, .. } => collect(body, false, writes),
                Statement::Sequence { branches } => {
                    for branch in branches { collect(branch, unconditional, writes); }
                }
                Statement::ScheduledConsumer { body, .. } => {
                    // Inspect the authored consumer declarations without
                    // evaluating preceding captures. A captured/calculated
                    // expression remains Runtime rather than an invented
                    // default, even when its producer happens to be constant.
                    collect(body, unconditional, writes);
                }
                Statement::Choice { options } => {
                    for option in options { collect(&option.body, false, writes); }
                }
                Statement::Imagemap { hotspots, .. } => {
                    for hotspot in hotspots { collect(&hotspot.body, false, writes); }
                }
                // Function/Screen/Handler/local/nested Init are not direct
                // project declarations. Do not execute calls or expressions.
                _ => {}
            }
        }
    }
    for statement in script {
        if let Statement::Init { body } = statement { collect(body, true, &mut writes); }
    }
    writes.into_iter().map(|(name, values)| {
        let value = match values.as_slice() {
            [Some(value)] => GlobalDeclarationValue::Literal(value.clone()),
            _ => GlobalDeclarationValue::Runtime,
        };
        (name, value)
    }).collect()
}

/// Legacy project documents have no source owner. Strictly validate the
/// complete supplied project, then inspect its emitted declarations only.
pub fn global_declaration_values_for_graphs(graphs: &[GraphDocument])
    -> Result<BTreeMap<String, GlobalDeclarationValue>, String>
{
    Ok(global_declaration_values(&crate::transpile_project(graphs)?.ast))
}

/// Inspect a resolved project script after strict structural validation.
/// Callers resolve imports through their existing project authority first;
/// this query does not load files, execute expressions or publish a result.
pub fn global_declaration_values_for_script(script: &[Statement])
    -> Result<BTreeMap<String, GlobalDeclarationValue>, String>
{
    crate::validate_project_script(&script.to_vec(), false)?;
    Ok(global_declaration_values(script))
}

impl GraphDocument {
    /// Exact authoring ownership, not name/GraphId alone. No callee identities
    /// or source declarations are allocated by this read-only helper.
    pub(crate) fn binding_project_owner_index(&self, project: &[GraphDocument]) -> Result<usize, String> {
        let owners: Vec<_> = project.iter().enumerate().filter(|(_, graph)| graph.graph_id == self.graph_id).collect();
        let [(index, owner)] = owners.as_slice() else {
            return Err("Binding operation requires one actual project graph owner".into());
        };
        if *owner != self { return Err("Binding project owner is stale or belongs to another document".into()); }
        crate::unit_graph_source1003::validate_project(project)?;
        Ok(*index)
    }

    pub(crate) fn binding_project_replacing(&self, replacement: &GraphDocument, project: &[GraphDocument]) -> Result<Vec<GraphDocument>, String> {
        let index = self.binding_project_owner_index(project)?;
        if replacement.graph_id != self.graph_id || replacement.kind != self.kind {
            return Err("Binding edit changed its actual project owner".into());
        }
        let mut edited = project.to_vec();
        edited[index] = replacement.clone();
        crate::unit_graph_source1003::validate_project(&edited)?;
        Ok(edited)
    }

    /// Explicit locals emitted from this graph. Later Set, call arguments,
    /// implicit calculation assignments and disconnected nodes are not local
    /// declarations. Repeated Local declarations retain separate identities.
    /// A fragment's inherited activation bindings require a project query.
    pub fn local_variable_initializers(&self, name: &str)
        -> Result<Vec<LocalVariableInitializer>, TranspileError>
    {
        if self.variable_scope(name) != Some(VariableScope::Local) {
            return Ok(Vec::new());
        }
        Ok(self.local_declaration_initializers()?.remove(name).unwrap_or_default())
    }

    /// Batch projection for an editor view rebuild. Rendering can reuse this
    /// transient result without transpiling the graph once per name/frame.
    pub fn local_declaration_initializers(&self)
        -> Result<BTreeMap<String, Vec<LocalVariableInitializer>>, TranspileError>
    {
        self.collect_local_declaration_initializers(false, None)
    }

    /// Declaration inspection during a retained-link type edit. Only the
    /// existing binding-analysis route can tolerate incompatible_types;
    /// structure, source parsing and expansion limits remain strict. This
    /// read-only query cannot compile or publish an executable program.
    pub fn local_declaration_initializers_for_binding_analysis(&self)
        -> Result<BTreeMap<String, Vec<LocalVariableInitializer>>, TranspileError>
    {
        self.collect_local_declaration_initializers(true, None)
    }

    /// The complete caller-owned project is required when a scope calls Unit.
    pub fn local_declaration_initializers_in_project(&self, project: &[GraphDocument])
        -> Result<BTreeMap<String, Vec<LocalVariableInitializer>>, TranspileError> {
        self.binding_project_owner_index(project).map_err(TranspileError::GeneratedSourceInvalid)?;
        self.collect_local_declaration_initializers(false, Some(project))
    }

    pub fn local_declaration_initializers_for_binding_analysis_in_project(&self, project: &[GraphDocument])
        -> Result<BTreeMap<String, Vec<LocalVariableInitializer>>, TranspileError> {
        self.binding_project_owner_index(project).map_err(TranspileError::GeneratedSourceInvalid)?;
        self.collect_local_declaration_initializers(true, Some(project))
    }

    fn collect_local_declaration_initializers(&self, allow_incompatible_links: bool, project: Option<&[GraphDocument]>)
        -> Result<BTreeMap<String, Vec<LocalVariableInitializer>>, TranspileError>
    {
        if !self.nodes.values().any(|node| crate::widget_factory_graph_1001::is_local_assignment(node)) {
            // A retained-link analysis promises structural validation even
            // when this particular graph has no declaration to collect.
            if allow_incompatible_links {
                match project { Some(project) => crate::codegen::transpile_for_binding_analysis_in_project(self, project)?, None => crate::codegen::transpile_for_binding_analysis(self)? };
            }
            return Ok(BTreeMap::new());
        }
        let compiled = if allow_incompatible_links {
            match project { Some(project) => crate::codegen::transpile_for_binding_analysis_in_project(self, project)?, None => crate::codegen::transpile_for_binding_analysis(self)? }
        } else {
            match project { Some(project) => crate::codegen::transpile_with_origins_in_project(self, project)?, None => crate::transpile_with_origins(self)? }
        };
        let mut found = BTreeMap::new();
        let mut seen = BTreeSet::new();
        fn collect(graph: &GraphDocument, statements: &[Statement],
            origins: &[GraphStatementOrigin], seen: &mut BTreeSet<NodeId>,
            found: &mut BTreeMap<String, Vec<LocalVariableInitializer>>)
        {
            for (statement, origin) in statements.iter().zip(origins) {
                if let Statement::LocalVar { name: binding, value } = statement {
                    if graph.variable_scope(binding) == Some(VariableScope::Local) {
                        if let Some(node) = origin.node.filter(|id| graph.nodes.get(id)
                            .is_some_and(|node| crate::widget_factory_graph_1001::is_local_assignment(node))) {
                            if seen.insert(node) {
                                found.entry(binding.clone()).or_default().push(LocalVariableInitializer { graph:graph.graph_id, node,
                                    expression: value.clone(), literal_value: literal(value) });
                            }
                        }
                    }
                }
                let children: Vec<&[Statement]> = match statement {
                    Statement::Function { body, .. } | Statement::UnitFunction { body, .. } | Statement::Screen { body, .. }
                    | Statement::Handler { body, .. } | Statement::Init { body }
                    | Statement::While { body, .. } | Statement::ForEach { body, .. }
                    | Statement::ForLoopWithBreak { body, .. }
                    | Statement::ScheduledConsumer { body, .. } => vec![body],
                    Statement::If { then_branch, else_branch, .. } => vec![then_branch, else_branch],
                    Statement::Sequence { branches } => branches.iter().map(Vec::as_slice).collect(),
                    Statement::Choice { options } => options.iter().map(|option| option.body.as_slice()).collect(),
                    Statement::Imagemap { hotspots, .. } => hotspots.iter().map(|hotspot| hotspot.body.as_slice()).collect(),
                    _ => Vec::new(),
                };
                let mut offset = 0;
                for body in children {
                    let next = offset + body.len();
                    if let Some(nested) = origin.children.get(offset..next) {
                        collect(graph, body, nested, seen, found);
                    }
                    offset = next;
                }
            }
        }
        collect(self, &compiled.ast, &compiled.origins, &mut seen, &mut found);
        Ok(found)
    }
}
