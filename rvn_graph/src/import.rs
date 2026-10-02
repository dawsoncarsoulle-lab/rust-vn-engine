//! Deterministic, loss-checked import of the narrative RVN subset.
//! Imports are resolved by the caller; unsupported statements fail explicitly.
use crate::*;
use rvn_parser::{BinOpKind, Expr, Script, Statement, Transition};
use std::collections::BTreeMap;

/// Re-import all source documents while retaining unchanged graph/node/pin
/// identities and user layout. Never modifies the supplied prior documents.
pub fn reimport_script(
    script: &Script,
    previous: &[GraphDocument],
) -> Result<Vec<GraphDocument>, String> {
    // RVN stores Text payloads as strings. A prior explicit Text annotation is
    // presentation metadata unless source supplies a contrary typed value.
    let text_variables: std::collections::BTreeSet<_> = previous
        .iter()
        .flat_map(|graph| {
            graph
                .variables
                .values()
                .filter(|variable| {
                    variable.value_type == ValueType::InterpolatedText
                        && graph.variable_scope(&variable.name) == Some(VariableScope::Global)
                })
                .map(|variable| variable.name.clone())
        })
        .collect();
    let mut imported = import_script_with_text_variables(script, &text_variables)?;
    let mut next_graph = previous
        .iter()
        .map(|g| g.graph_id.get())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("Graph identity space exhausted")?;
    for graph in &mut imported {
        let matching: Vec<_> = previous
            .iter()
            .filter(|old| old.kind == graph.kind)
            .collect();
        match matching.as_slice() {
            [old] => {
                let mut previous = (*old).clone();
                // Source registries must refresh even when the label's AST has
                // not changed (for example an init default or type changed).
                previous.variables = graph.variables.clone();
                previous.characters = graph.characters.clone();
                let conflicts: BTreeMap<_, _> = graph
                    .nodes
                    .values()
                    .filter_map(|node| {
                        match (
                            node.properties.get("name"),
                            node.properties.get("inferred_conflict_types"),
                        ) {
                            (Some(PropertyValue::String(name)), Some(types)) => {
                                Some((name.clone(), types.clone()))
                            }
                            _ => None,
                        }
                    })
                    .collect();
                for node in previous
                    .nodes
                    .values_mut()
                    .filter(|node| node.kind == NodeKind::SetVariable)
                {
                    let name = match node.properties.get("name") {
                        Some(PropertyValue::String(name)) => Some(name.clone()),
                        _ => None,
                    };
                    if let Some(types) = name.and_then(|name| conflicts.get(&name)) {
                        node.properties
                            .insert("inferred_conflict_types".into(), types.clone());
                    } else {
                        node.properties.remove("inferred_conflict_types");
                    }
                }
                for node in previous.nodes.values() {
                    if node.kind == NodeKind::Index {
                        for id in &node.pins {
                            if previous.pins[id].key == "index" {
                                previous.pins.get_mut(id).unwrap().value_type = ValueType::IndexKey;
                            }
                        }
                    }
                    if matches!(
                        node.kind,
                        NodeKind::VariableGet | NodeKind::SetVariable | NodeKind::LocalVariable
                    ) {
                        if let Some(PropertyValue::String(name)) = node.properties.get("name") {
                            if let Some(variable) = previous.variables.get(name) {
                                for id in &node.pins {
                                    let pin = previous.pins.get_mut(id).unwrap();
                                    if matches!(pin.key.as_str(), "value" | "value_out") {
                                        pin.value_type = variable.value_type.clone();
                                    }
                                }
                            }
                        }
                    }
                }
                previous.materialize_implicit_conversions().map_err(err)?;
                previous.schema_version = GRAPH_SCHEMA_VERSION;
                // Type-only conversions may be presentation in dialogue RVN.
                // Keep the complete prior graph when its logic is unchanged,
                // instead of dropping visible casts and their layout on reopen.
                if transpile(&previous)
                    .is_ok_and(|before| transpile(graph).is_ok_and(|after| before.ast == after.ast))
                {
                    *graph = previous;
                } else {
                    graph.reconcile_import(&previous)?;
                }
            }
            [] => {
                graph.graph_id = GraphId::new(next_graph);
                next_graph = next_graph
                    .checked_add(1)
                    .ok_or("Graph identity space exhausted")?;
            }
            _ => return Err(format!("Ambiguous prior graph identity: {:?}", graph.kind)),
        }
        transpile(graph).map_err(err)?;
    }
    transpile_project(&imported)?;
    Ok(imported)
}

/// One init document followed by one document per source label, in source order.
pub fn import_script(script: &Script) -> Result<Vec<GraphDocument>, String> {
    import_script_with_text_variables(script, &std::collections::BTreeSet::new())
}

fn import_script_with_text_variables(
    script: &Script,
    text_variables: &std::collections::BTreeSet<String>,
) -> Result<Vec<GraphDocument>, String> {
    let mut init = Vec::new();
    let mut functions = Vec::new();
    let mut labels: Vec<(String, Script)> = Vec::new();
    for statement in script {
        match statement {
            Statement::Init { body } => init.extend(body.clone()),
            Statement::Function {
                name,
                parameters,
                body,
            }
            | Statement::Screen {
                name,
                parameters,
                body,
            }
            | Statement::Handler {
                name,
                parameters,
                body,
            } => {
                let kind = match statement {
                    Statement::Screen { .. } => GraphKind::Screen { name: name.clone() },
                    Statement::Handler { .. } => GraphKind::Handler { name: name.clone() },
                    _ => GraphKind::Function { name: name.clone() },
                };
                if functions.iter().any(|(existing, _, _)| existing == &kind) {
                    return Err(format!("Déclaration dupliquée : {name}"));
                }
                functions.push((kind, parameters.clone(), body.clone()));
            }
            Statement::Label { name } => {
                if labels.iter().any(|(n, _)| n == name) {
                    return Err(format!("Label dupliqué : {name}"));
                }
                labels.push((name.clone(), Vec::new()));
            }
            Statement::Use { .. } => {
                return Err("Résoudre les imports RVN avant la conversion".into())
            }
            other => labels
                .last_mut()
                .ok_or("Instruction hors init et label")?
                .1
                .push(other.clone()),
        }
    }
    let mut characters = BTreeMap::new();
    let mut variables = BTreeMap::new();
    for statement in &init {
        match statement {
            Statement::CharacterCreate { id, display_name } => {
                if characters
                    .insert(id.clone(), display_name.clone())
                    .is_some()
                {
                    return Err(format!("Personnage déclaré deux fois : {id}"));
                }
            }
            Statement::SetVar { name, value } => {
                let cast = match value {
                    Expr::Call { name, args }
                        if args.len() == 1
                            && matches!(name.as_str(), "string_to_text" | "text_to_string") =>
                    {
                        Some((name.as_str(), &args[0]))
                    }
                    _ => None,
                };
                let initial_literal = cast
                    .and_then(|(_, value)| literal(value))
                    .or_else(|| literal(value));
                let default_value = initial_literal.clone().unwrap_or_else(|| {
                    if cast.is_some() {
                        PropertyValue::String(String::new())
                    } else {
                        PropertyValue::Int(0)
                    }
                });
                let value_type = match &default_value {
                    _ if cast.is_some_and(|(kind, _)| kind == "string_to_text") => {
                        ValueType::InterpolatedText
                    }
                    _ if cast.is_some_and(|(kind, _)| kind == "text_to_string") => {
                        ValueType::String
                    }
                    _ if initial_literal.is_none() => {
                        crate::type_inference::expression_type(value, &variables)
                    }
                    PropertyValue::Bool(_) => ValueType::Bool,
                    PropertyValue::Int(_) => ValueType::Int,
                    PropertyValue::Float(_) => ValueType::Float,
                    PropertyValue::StringList(items) => {
                        ValueType::List(Box::new(if items.is_empty() {
                            ValueType::Any
                        } else {
                            ValueType::String
                        }))
                    }
                    _ if text_variables.contains(name) => ValueType::InterpolatedText,
                    _ => ValueType::String,
                };
                variables.insert(
                    name.clone(),
                    VariableDefinition {
                        name: name.clone(),
                        value_type,
                        default_value,
                    },
                );
            }
            _ => {}
        }
    }
    // Characters are serialized once through the shared character registry.
    init.retain(|s| !matches!(s, Statement::CharacterCreate { .. }));
    let mut blocks = vec![(GraphKind::Init, init, Vec::new())];
    for (kind, parameters, body) in functions {
        blocks.push((kind, body, parameters));
    }
    for (index, (name, body)) in labels.iter().enumerate() {
        let mut body = body.clone();
        if !body
            .last()
            .is_some_and(|s| matches!(s, Statement::Jump { .. } | Statement::Return))
        {
            if let Some((next, _)) = labels.get(index + 1) {
                body.push(Statement::Jump {
                    target: next.clone(),
                });
            }
        }
        blocks.push((GraphKind::Label { name: name.clone() }, body, Vec::new()));
    }
    // The runtime permits a global to change type. Keep compatible/unknown
    // assignments faithful to the initialized type, but expose proven concrete
    // conflicts as Any with a diagnostic, never a forced conversion. Discover
    // globals declared outside init once so every graph shares that registry.
    let conflicts = infer_project_globals(&blocks, &mut variables);
    let mut result = Vec::new();
    for (index, (kind, body, parameters)) in blocks.into_iter().enumerate() {
        let mut graph = GraphDocument::new(GraphId::new(index as u64 + 1), kind.clone());
        graph.characters = characters.clone();
        graph.variables = variables.clone();
        // Set in a handler writes a global unless a parameter/local/for shadows
        // it. It must not erase that global's known type on every import.
        // Calculation Set is local. Unknown/heterogeneous local writes remain
        // Any; they are never mislabeled as a global solely by matching its name.
        fn collect_bindings(
            body: &[Statement],
            names: &mut std::collections::BTreeSet<String>,
            calculation: bool,
            handler: bool,
        ) {
            for statement in body {
                match statement {
                    Statement::LocalVar { name, .. } => {
                        names.insert(name.clone());
                    }
                    Statement::SetVar { name, .. } if calculation => {
                        names.insert(name.clone());
                    }
                    Statement::ForEach { name, body, .. } => {
                        if calculation || handler {
                            names.insert(name.clone());
                        }
                        collect_bindings(body, names, calculation, handler);
                    }
                    Statement::While { body, .. } | Statement::Init { body } => {
                        collect_bindings(body, names, calculation, handler)
                    }
                    Statement::If {
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        collect_bindings(then_branch, names, calculation, handler);
                        collect_bindings(else_branch, names, calculation, handler);
                    }
                    Statement::Choice { options } => {
                        for option in options {
                            collect_bindings(&option.body, names, calculation, handler);
                        }
                    }
                    Statement::Imagemap { hotspots, .. } => {
                        for hotspot in hotspots {
                            collect_bindings(&hotspot.body, names, calculation, handler);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut bindings = parameters.iter().cloned().collect();
        collect_bindings(
            &body,
            &mut bindings,
            matches!(kind, GraphKind::Function { .. } | GraphKind::Screen { .. }),
            matches!(kind, GraphKind::Handler { .. }),
        );
        // Sequential inference accumulates known local values, but a different
        // write type (including a shadowed global's type) conservatively joins
        // to Any. Parameters are untyped and never narrowed from assignment.
        fn infer_bindings(
            body: &[Statement],
            locals: &std::collections::BTreeSet<String>,
            parameters: &[String],
            vars: &mut BTreeMap<String, VariableDefinition>,
            written: &mut std::collections::BTreeSet<String>,
        ) {
            fn binding(
                name: &str,
                ty: ValueType,
                default: Option<PropertyValue>,
                locals: &std::collections::BTreeSet<String>,
                parameters: &[String],
                vars: &mut BTreeMap<String, VariableDefinition>,
                written: &mut std::collections::BTreeSet<String>,
            ) {
                if parameters.iter().any(|parameter| parameter == name) {
                    return;
                }
                if !locals.contains(name) && vars.contains_key(name) {
                    return;
                }
                let previous = vars.get(name).map(|variable| variable.value_type.clone());
                let ty = if previous.is_some()
                    && (!locals.contains(name)
                        || written.contains(name)
                        || previous.as_ref() != Some(&ty))
                {
                    if previous.as_ref() == Some(&ty) {
                        ty
                    } else {
                        ValueType::Any
                    }
                } else {
                    ty
                };
                vars.insert(
                    name.into(),
                    VariableDefinition {
                        name: name.into(),
                        value_type: ty,
                        default_value: default.unwrap_or(PropertyValue::Int(0)),
                    },
                );
                written.insert(name.into());
            }
            for statement in body {
                match statement {
                    Statement::SetVar { name, value } | Statement::LocalVar { name, value } => {
                        binding(
                            name,
                            crate::type_inference::expression_type(value, vars),
                            literal(value),
                            locals,
                            parameters,
                            vars,
                            written,
                        )
                    }
                    Statement::ForEach {
                        name,
                        collection,
                        body,
                    } => {
                        let ty = match crate::type_inference::expression_type(collection, vars) {
                            ValueType::List(inner) => *inner,
                            _ => ValueType::Any,
                        };
                        binding(name, ty, None, locals, parameters, vars, written);
                        infer_bindings(body, locals, parameters, vars, written);
                    }
                    Statement::While { body, .. } | Statement::Init { body } => {
                        infer_bindings(body, locals, parameters, vars, written)
                    }
                    Statement::If {
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        infer_bindings(then_branch, locals, parameters, vars, written);
                        infer_bindings(else_branch, locals, parameters, vars, written);
                    }
                    Statement::Choice { options } => {
                        for option in options {
                            infer_bindings(&option.body, locals, parameters, vars, written);
                        }
                    }
                    Statement::Imagemap { hotspots, .. } => {
                        for hotspot in hotspots {
                            infer_bindings(&hotspot.body, locals, parameters, vars, written);
                        }
                    }
                    _ => {}
                }
            }
        }
        for name in &parameters {
            graph.variables.insert(
                name.clone(),
                VariableDefinition {
                    name: name.clone(),
                    value_type: ValueType::Any,
                    default_value: PropertyValue::Int(0),
                },
            );
        }
        infer_bindings(
            &body,
            &bindings,
            &parameters,
            &mut graph.variables,
            &mut std::collections::BTreeSet::new(),
        );
        let root = graph
            .add_catalog_node(
                if kind == GraphKind::Init {
                    NodeKind::Init
                } else if matches!(kind, GraphKind::Function { .. }) {
                    NodeKind::FunctionEntry
                } else if matches!(kind, GraphKind::Screen { .. }) {
                    NodeKind::ScreenEntry
                } else if matches!(kind, GraphKind::Handler { .. }) {
                    NodeKind::HandlerEntry
                } else {
                    NodeKind::Label
                },
                [0.0, 0.0],
            )
            .map_err(err)?;
        if matches!(
            kind,
            GraphKind::Function { .. } | GraphKind::Screen { .. } | GraphKind::Handler { .. }
        ) {
            graph
                .set_property(root, "parameters", PropertyValue::StringList(parameters))
                .map_err(err)?;
        }
        let mut builder = Builder { graph, lane: 0.0 };
        builder
            .sequence(&body, (root, "exec_out".into()), 960.0, 0.0, None)
            .map_err(|e| format!("{kind:?} : {e}"))?;
        for node in builder
            .graph
            .nodes
            .values_mut()
            .filter(|node| node.kind == NodeKind::SetVariable)
        {
            if let Some(PropertyValue::String(name)) = node.properties.get("name") {
                if !bindings.contains(name) {
                    if let Some(types) = conflicts.get(name) {
                        node.properties.insert(
                            "inferred_conflict_types".into(),
                            PropertyValue::StringList(types.clone()),
                        );
                    }
                }
            }
        }
        builder.graph.materialize_visible_defaults().map_err(err)?;
        layout_imported_values(&mut builder.graph);
        // Catch omitted/incorrect pins immediately, before any files are written.
        transpile(&builder.graph).map_err(err)?;
        result.push(builder.graph);
    }
    transpile_project(&result)?;
    Ok(result)
}

/// Arrange pure inputs below their action using actual asset-card clearances.
/// Execution positions and all graph semantics remain untouched.
pub fn layout_imported_values(graph: &mut GraphDocument) {
    fn inputs(graph: &GraphDocument, node: NodeId) -> Vec<NodeId> {
        graph.nodes[&node]
            .pins
            .iter()
            .filter_map(|id| {
                let pin = &graph.pins[id];
                if pin.direction != PinDirection::Input || pin.value_type.is_execution() {
                    return None;
                }
                graph
                    .edges
                    .values()
                    .find(|e| e.input == *id)
                    .map(|e| graph.pins[&e.output].node)
            })
            .collect()
    }
    fn place(
        graph: &mut GraphDocument,
        node: NodeId,
        pos: [f64; 2],
        visited: &mut std::collections::BTreeSet<NodeId>,
    ) {
        if !visited.insert(node) {
            return;
        }
        graph.nodes.get_mut(&node).unwrap().position = pos;
        for (i, child) in inputs(graph, node).into_iter().enumerate() {
            place(
                graph,
                child,
                [pos[0] - 300.0, pos[1] + 180.0 + i as f64 * 240.0],
                visited,
            );
        }
    }
    let actions: Vec<_> = graph
        .nodes
        .values()
        .filter(|n| {
            n.pins
                .iter()
                .any(|id| graph.pins[id].value_type.is_execution())
        })
        .map(|n| n.id)
        .collect();
    let mut visited: std::collections::BTreeSet<_> = actions.iter().copied().collect();
    for action in actions {
        let pos = graph.nodes[&action].position;
        for (i, child) in inputs(graph, action).into_iter().enumerate() {
            place(
                graph,
                child,
                [pos[0] - 320.0, pos[1] + 180.0 + i as f64 * 240.0],
                &mut visited,
            );
        }
    }
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn key_is_variable_assignment(graph: &GraphDocument, node: NodeId, key: &str) -> bool {
    key == "value"
        && matches!(
            graph.nodes[&node].kind,
            NodeKind::SetVariable | NodeKind::LocalVariable
        )
}

fn infer_project_globals(
    blocks: &[(GraphKind, Script, Vec<String>)],
    variables: &mut BTreeMap<String, VariableDefinition>,
) -> BTreeMap<String, Vec<String>> {
    use std::collections::BTreeSet;
    fn shadowed(body: &[Statement], locals: &mut BTreeSet<String>) {
        for statement in body {
            match statement {
                Statement::LocalVar { name, .. } => {
                    locals.insert(name.clone());
                }
                Statement::ForEach { name, body, .. } => {
                    locals.insert(name.clone());
                    shadowed(body, locals);
                }
                Statement::While { body, .. } | Statement::Init { body } => shadowed(body, locals),
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    shadowed(then_branch, locals);
                    shadowed(else_branch, locals);
                }
                _ => {}
            }
        }
    }
    fn concrete(ty: &ValueType) -> bool {
        match ty {
            ValueType::Any | ValueType::IndexKey => false,
            ValueType::List(inner) => concrete(inner),
            _ => true,
        }
    }
    fn record(
        name: &str,
        value: &Expr,
        locals: &BTreeSet<String>,
        env: &mut BTreeMap<String, VariableDefinition>,
        globals: &mut BTreeMap<String, VariableDefinition>,
        observed: &mut BTreeMap<String, Vec<ValueType>>,
    ) {
        let mut ty = crate::type_inference::expression_type(value, env);
        if locals.contains(name) {
            env.insert(
                name.into(),
                VariableDefinition {
                    name: name.into(),
                    value_type: ty,
                    default_value: literal(value).unwrap_or(PropertyValue::Int(0)),
                },
            );
            return;
        }
        // A literal string has no runtime Text distinction. Preserve a prior
        // deliberate localizable Text annotation unless source requests a
        // contrary typed expression, such as text_to_string(...).
        if matches!(value, Expr::Str(_))
            && globals
                .get(name)
                .is_some_and(|variable| variable.value_type == ValueType::InterpolatedText)
        {
            ty = ValueType::InterpolatedText;
        }
        if concrete(&ty) {
            let types = observed.entry(name.into()).or_default();
            if !types.contains(&ty) {
                types.push(ty.clone());
            }
        }
        globals
            .entry(name.into())
            .or_insert_with(|| VariableDefinition {
                name: name.into(),
                value_type: ty.clone(),
                default_value: literal(value).unwrap_or(PropertyValue::Int(0)),
            });
        env.insert(
            name.into(),
            VariableDefinition {
                name: name.into(),
                value_type: ty,
                default_value: literal(value).unwrap_or(PropertyValue::Int(0)),
            },
        );
    }
    fn walk(
        body: &[Statement],
        locals: &BTreeSet<String>,
        env: &mut BTreeMap<String, VariableDefinition>,
        globals: &mut BTreeMap<String, VariableDefinition>,
        observed: &mut BTreeMap<String, Vec<ValueType>>,
    ) {
        for statement in body {
            match statement {
                Statement::SetVar { name, value } | Statement::LocalVar { name, value } => {
                    record(name, value, locals, env, globals, observed)
                }
                Statement::ForEach {
                    name,
                    collection,
                    body,
                } => {
                    let value = Expr::Index {
                        target: Box::new(collection.clone()),
                        index: Box::new(Expr::Int(0)),
                    };
                    record(name, &value, locals, env, globals, observed);
                    walk(body, locals, env, globals, observed);
                }
                Statement::While { body, .. } | Statement::Init { body } => {
                    walk(body, locals, env, globals, observed)
                }
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    let original = env.clone();
                    walk(then_branch, locals, env, globals, observed);
                    let mut alternate = original.clone();
                    walk(else_branch, locals, &mut alternate, globals, observed);
                    merge_env(env, &alternate, &original);
                }
                Statement::Choice { options } => {
                    let original = env.clone();
                    for option in options {
                        let mut branch = original.clone();
                        walk(&option.body, locals, &mut branch, globals, observed);
                        merge_env(env, &branch, &original);
                    }
                }
                Statement::Imagemap { hotspots, .. } => {
                    let original = env.clone();
                    for hotspot in hotspots {
                        let mut branch = original.clone();
                        walk(&hotspot.body, locals, &mut branch, globals, observed);
                        merge_env(env, &branch, &original);
                    }
                }
                _ => {}
            }
        }
    }
    fn merge_env(
        env: &mut BTreeMap<String, VariableDefinition>,
        other: &BTreeMap<String, VariableDefinition>,
        original: &BTreeMap<String, VariableDefinition>,
    ) {
        let names: BTreeSet<_> = env.keys().chain(other.keys()).cloned().collect();
        for name in names {
            let first = env.get(&name).or_else(|| original.get(&name));
            let second = other.get(&name).or_else(|| original.get(&name));
            if first.map(|value| &value.value_type) != second.map(|value| &value.value_type) {
                if let Some(value) = first.or(second) {
                    let mut value = value.clone();
                    value.value_type = ValueType::Any;
                    env.insert(name, value);
                }
            }
        }
    }
    let mut observed: BTreeMap<String, Vec<ValueType>> = variables
        .iter()
        .filter(|(_, variable)| concrete(&variable.value_type))
        .map(|(name, variable)| (name.clone(), vec![variable.value_type.clone()]))
        .collect();
    for (kind, body, parameters) in blocks {
        if matches!(kind, GraphKind::Function { .. } | GraphKind::Screen { .. }) {
            continue;
        }
        let mut locals = parameters.iter().cloned().collect();
        if matches!(kind, GraphKind::Handler { .. }) {
            shadowed(body, &mut locals);
        }
        let mut env = variables.clone();
        for name in parameters {
            env.insert(
                name.clone(),
                VariableDefinition {
                    name: name.clone(),
                    value_type: ValueType::Any,
                    default_value: PropertyValue::Int(0),
                },
            );
        }
        walk(body, &locals, &mut env, variables, &mut observed);
    }
    let mut conflicts = BTreeMap::new();
    for (name, types) in observed {
        if types.len() > 1 {
            variables.get_mut(&name).unwrap().value_type = ValueType::Any;
            conflicts.insert(name, types.iter().map(|ty| format!("{ty:?}")).collect());
        }
    }
    conflicts
}
fn expression_source(expr: &Expr) -> String {
    match expr {
        Expr::Int(v) => v.to_string(),
        Expr::Float(v) => format!("{v:?}"),
        Expr::Bool(v) => v.to_string(),
        Expr::Str(v) => serde_json::to_string(v).unwrap(),
        Expr::Var(v) => v.clone(),
        Expr::BinOp { op, left, right } => format!(
            "({} {op} {})",
            expression_source(left),
            expression_source(right)
        ),
        Expr::And(a, b) => format!("({} and {})", expression_source(a), expression_source(b)),
        Expr::Or(a, b) => format!("({} or {})", expression_source(a), expression_source(b)),
        Expr::Not(v) => format!("(not {})", expression_source(v)),
        Expr::Neg(v) => format!("(-{})", expression_source(v)),
        Expr::Call { name, args } => format!(
            "{name}({})",
            args.iter()
                .map(expression_source)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::ListLit(items) => format!(
            "[{}]",
            items
                .iter()
                .map(expression_source)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Index { target, index } => format!(
            "{}[{}]",
            expression_source(target),
            expression_source(index)
        ),
    }
}
fn text_source(text: &rvn_parser::InterpolatedText) -> String {
    text.0
        .iter()
        .map(|s| match s {
            rvn_parser::TextSegment::Lit(v) => v.clone(),
            rvn_parser::TextSegment::Interp(v) => format!("[{}]", expression_source(v)),
        })
        .collect()
}
fn literal(value: &Expr) -> Option<PropertyValue> {
    Some(match value {
        Expr::Int(v) => PropertyValue::Int(*v),
        Expr::Float(v) => PropertyValue::Float(*v as f64),
        Expr::Bool(v) => PropertyValue::Bool(*v),
        Expr::Str(v) => PropertyValue::String(v.clone()),
        Expr::ListLit(items) => PropertyValue::StringList(
            items
                .iter()
                .map(|item| match item {
                    Expr::Str(value) => Some(value.clone()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?,
        ),
        _ => return None,
    })
}
struct Builder {
    graph: GraphDocument,
    lane: f64,
}
impl Builder {
    fn node(&mut self, kind: NodeKind, pos: [f64; 2]) -> Result<NodeId, String> {
        self.graph.add_catalog_node(kind, pos).map_err(err)
    }
    fn property(&mut self, n: NodeId, key: &str, value: PropertyValue) -> Result<(), String> {
        self.graph.set_property(n, key, value).map_err(err)
    }
    fn default(&mut self, n: NodeId, key: &str, value: PropertyValue) -> Result<(), String> {
        let id = self
            .graph
            .pin_by_key(n, key)
            .ok_or_else(|| format!("Broche absente : {n}.{key}"))?
            .id;
        self.graph.pins.get_mut(&id).unwrap().default_value = Some(value);
        Ok(())
    }
    fn text(&mut self, n: NodeId, key: &str, value: &str) -> Result<(), String> {
        self.default(n, key, PropertyValue::String(value.into()))
    }
    fn connect(&mut self, a: NodeId, ak: &str, b: NodeId, bk: &str) -> Result<(), String> {
        let output = self
            .graph
            .pin_by_key(a, ak)
            .ok_or_else(|| format!("Sortie absente : {a}.{ak}"))?
            .id;
        let input = self
            .graph
            .pin_by_key(b, bk)
            .ok_or_else(|| format!("Entrée absente : {b}.{bk}"))?
            .id;
        let from = self.graph.nodes[&a].position;
        let to = self.graph.nodes[&b].position;
        if key_is_variable_assignment(&self.graph, b, bk) {
            // Existing RVN did not request a cast. A textual assignment to a
            // known number cannot become to_int(...) merely through import.
            let source = self
                .graph
                .effective_pin_type(output)
                .unwrap_or(ValueType::Any);
            let target = self
                .graph
                .pin_constraint_type(input)
                .unwrap_or(ValueType::Any);
            if !target.accepts(&source)
                && !matches!(
                    (&target, &source),
                    (ValueType::String, ValueType::InterpolatedText)
                        | (ValueType::InterpolatedText, ValueType::String)
                )
            {
                return Err(format!("Variable assignment {target:?} cannot accept {source:?}; write an explicit conversion or resolve the type conflict"));
            }
        }
        let source = self
            .graph
            .effective_pin_type(output)
            .unwrap_or(ValueType::Any);
        let target = self
            .graph
            .pin_constraint_type(input)
            .unwrap_or(ValueType::Any);
        // Existing RVN arithmetic widening is valid without rewriting an
        // authored expression as (... + 0.0). New visual authoring, unlike
        // source import, shows that precision boundary as an explicit node.
        if target.accepts(&source) {
            self.graph.connect(output, input).map_err(err)?;
        } else {
            self.graph
                .connect_with_conversions(
                    output,
                    input,
                    [(from[0] + to[0]) * 0.5, (from[1] + to[1]) * 0.5],
                )
                .map_err(err)?;
        }
        Ok(())
    }
    fn variable(&mut self, node: NodeId, name: &str) -> Result<(), String> {
        let ty = self
            .graph
            .variables
            .get(name)
            .ok_or_else(|| format!("Variable sans déclaration typée : {name}"))?
            .value_type
            .clone();
        self.property(node, "name", PropertyValue::String(name.into()))?;
        for key in ["value", "value_out"] {
            if let Some(id) = self.graph.pin_by_key(node, key).map(|p| p.id) {
                self.graph.pins.get_mut(&id).unwrap().value_type = ty.clone();
            }
        }
        Ok(())
    }
    fn expression(&mut self, expr: &Expr, pos: [f64; 2]) -> Result<NodeId, String> {
        // Collections and calls must remain editable nodes, not opaque text.
        if let Expr::Call { name, args } = expr {
            if args.len() == 1 && matches!(name.as_str(), "string_to_text" | "text_to_string") {
                let kind = if name == "string_to_text" {
                    NodeKind::ConvertStringToText
                } else {
                    NodeKind::ConvertTextToString
                };
                let node = self.node(kind, pos)?;
                let argument = if kind == NodeKind::ConvertTextToString
                    && matches!(&args[0], Expr::Str(_))
                {
                    let Expr::Str(value) = &args[0] else {
                        unreachable!()
                    };
                    let text = self.node(NodeKind::TextValue, [pos[0] - 260.0, pos[1] + 160.0])?;
                    self.property(text, "value", PropertyValue::String(value.clone()))?;
                    text
                } else {
                    self.expression(&args[0], [pos[0] - 260.0, pos[1] + 160.0])?
                };
                let argument_type = self.graph.nodes[&argument]
                    .pins
                    .iter()
                    .map(|id| &self.graph.pins[id])
                    .find(|pin| {
                        pin.direction == PinDirection::Output && !pin.value_type.is_execution()
                    })
                    .unwrap()
                    .value_type
                    .clone();
                if !matches!(
                    argument_type,
                    ValueType::String | ValueType::InterpolatedText | ValueType::Any
                ) {
                    return Err(format!(
                        "{name} expects a string payload, not {argument_type:?}"
                    ));
                }
                self.connect_expression(argument, node, "value")?;
                return Ok(node);
            }
            let motion = match name.as_str() {
                "canvas_rect" => Some((NodeKind::CanvasRect, &["rect", "color", "radius"][..])),
                "canvas_ellipse" => Some((NodeKind::CanvasEllipse, &["rect", "color"][..])),
                "canvas_line" => Some((NodeKind::CanvasLine, &["points", "color", "width"][..])),
                "canvas_polygon" => Some((NodeKind::CanvasPolygon, &["points", "color"][..])),
                "canvas_text" => Some((
                    NodeKind::CanvasText,
                    &["text", "position", "color", "size"][..],
                )),
                "canvas_image" => Some((NodeKind::CanvasImage, &["image", "rect"][..])),
                "canvas_group" => Some((
                    NodeKind::CanvasGroup,
                    &["transform", "clip", "children"][..],
                )),
                "canvas_hit" => Some((NodeKind::CanvasHit, &["id", "rect"][..])),
                "motion_tween" => Some((
                    NodeKind::MotionTween,
                    &["seconds", "from", "to", "curve"][..],
                )),
                "motion_spline" => {
                    Some((NodeKind::MotionSpline, &["seconds", "points", "curve"][..]))
                }
                "motion_bezier" => Some((NodeKind::MotionBezier, &["x1", "y1", "x2", "y2"][..])),
                "motion_curve" => Some((NodeKind::MotionCurve, &["function", "samples"][..])),
                "motion_pause" => Some((NodeKind::MotionPause, &["seconds"][..])),
                "motion_sequence" => Some((NodeKind::MotionSequence, &["steps"][..])),
                "motion_parallel" => Some((NodeKind::MotionParallel, &["steps"][..])),
                "motion_repeat" => Some((NodeKind::MotionRepeat, &["times", "motion"][..])),
                "motion_frames" => Some((NodeKind::MotionFrames, &["images", "fps"][..])),
                "layered_image" => Some((
                    NodeKind::LayeredImage,
                    if args.len() == 4 {
                        &["size", "defaults", "layers", "options"][..]
                    } else {
                        &["size", "defaults", "layers"][..]
                    },
                )),
                "image_layer" => Some((NodeKind::ImageLayer, &["id", "image", "properties"][..])),
                "image_layers" => Some((NodeKind::ImageLayers, &["prefix", "images"][..])),
                "video_clip" => Some((NodeKind::VideoClip, &["source", "properties"][..])),
                _ => None,
            };
            if let Some((kind, keys)) = motion.filter(|(_, keys)| keys.len() == args.len()) {
                let node = self.node(kind, pos)?;
                for (key, argument) in keys.iter().zip(args) {
                    self.input_expr(node, key, argument)?;
                }
                return Ok(node);
            }
            if name == "component" && args.len() == 4 {
                let node = self.node(NodeKind::UiComponent, pos)?;
                for (key, argument) in ["id", "kind", "properties", "children"].iter().zip(args) {
                    self.input_expr(node, key, argument)?;
                }
                return Ok(node);
            }
            return self.value_sequence(NodeKind::FunctionCall, Some(name), args, pos);
        }
        if let Expr::ListLit(items) = expr {
            return self.value_sequence(NodeKind::ListLiteral, None, items, pos);
        }
        if let Some(value) = literal(expr) {
            let n = self.node(NodeKind::Literal, pos)?;
            self.property(n, "value", value)?;
            return Ok(n);
        }
        if let Expr::Var(name) = expr {
            let n = self.node(NodeKind::VariableGet, pos)?;
            self.variable(n, name)?;
            return Ok(n);
        }
        let (kind, inputs): (NodeKind, Vec<(&str, &Expr)>) = match expr {
            Expr::BinOp { op, left, right } => (
                match op {
                    BinOpKind::Add => NodeKind::MathAdd,
                    BinOpKind::Sub => NodeKind::MathSubtract,
                    BinOpKind::Mul => NodeKind::MathMultiply,
                    BinOpKind::Div => NodeKind::MathDivide,
                    BinOpKind::Eq => NodeKind::MathEqual,
                    BinOpKind::Ne => NodeKind::MathNotEqual,
                    BinOpKind::Lt => NodeKind::MathLess,
                    BinOpKind::Le => NodeKind::MathLessEqual,
                    BinOpKind::Gt => NodeKind::MathGreater,
                    BinOpKind::Ge => NodeKind::MathGreaterEqual,
                },
                vec![("left", left), ("right", right)],
            ),
            Expr::And(a, b) => (NodeKind::LogicAnd, vec![("left", a), ("right", b)]),
            Expr::Or(a, b) => (NodeKind::LogicOr, vec![("left", a), ("right", b)]),
            Expr::Not(v) => (NodeKind::LogicNot, vec![("value", v)]),
            Expr::Neg(v) => (NodeKind::MathNegate, vec![("value", v)]),
            Expr::Index { target, index } => {
                (NodeKind::Index, vec![("target", target), ("index", index)])
            }
            _ => {
                return Err(format!(
                    "Expression non prise en charge par l'importeur : {expr:?}"
                ))
            }
        };
        let n = self.node(kind, pos)?;
        for (i, (key, expr)) in inputs.iter().enumerate() {
            let source =
                self.expression(expr, [pos[0] - 260.0, pos[1] + 120.0 + i as f64 * 180.0])?;
            self.connect_expression(source, n, key)?;
        }
        Ok(n)
    }
    fn value_sequence(
        &mut self,
        kind: NodeKind,
        function: Option<&str>,
        items: &[Expr],
        pos: [f64; 2],
    ) -> Result<NodeId, String> {
        if items.len() > 128 {
            return Err("Une expression Blueprint accepte au maximum 128 arguments".into());
        }
        let node = self.node(kind, pos)?;
        if let Some(name) = function {
            self.property(node, "function", PropertyValue::String(name.into()))?;
        }
        self.graph
            .resize_value_inputs(node, items.len())
            .map_err(err)?;
        for (index, item) in items.iter().enumerate() {
            self.input_expr(node, &format!("item_{index}"), item)?;
        }
        Ok(node)
    }
    fn connect_expression(
        &mut self,
        source: NodeId,
        target: NodeId,
        key: &str,
    ) -> Result<(), String> {
        let output = self.graph.nodes[&source]
            .pins
            .iter()
            .map(|id| &self.graph.pins[id])
            .find(|pin| pin.direction == PinDirection::Output && !pin.value_type.is_execution())
            .ok_or_else(|| format!("Expression sans sortie : {source}"))?
            .key
            .clone();
        self.connect(source, &output, target, key)
    }
    fn input_expr(&mut self, n: NodeId, key: &str, value: &Expr) -> Result<(), String> {
        // An authored empty string is a real value, unlike a catalogue's
        // empty placeholder. Keep that distinction explicit in the graph.
        if matches!(value, Expr::Str(text) if text.is_empty()) {
            let position = self.graph.nodes[&n].position;
            let kind = if self
                .graph
                .pin_by_key(n, key)
                .is_some_and(|pin| pin.value_type == ValueType::InterpolatedText)
            {
                NodeKind::TextValue
            } else {
                NodeKind::Literal
            };
            let source = self.node(kind, [position[0] - 260.0, position[1] + 160.0])?;
            self.property(source, "value", PropertyValue::String(String::new()))?;
            return self.connect_expression(source, n, key);
        }
        if let Some(value) = literal(value) {
            return self.default(n, key, value);
        }
        let p = self.graph.nodes[&n].position;
        let expr = self.expression(value, [p[0] - 260.0, p[1] + 160.0])?;
        self.connect_expression(expr, n, key)
    }
    fn transition(&mut self, n: NodeId, value: &Transition) -> Result<(), String> {
        self.text(n, "transition", &value.to_string())
    }
    fn dialogue_text(
        &mut self,
        dialogue: NodeId,
        text: &rvn_parser::InterpolatedText,
        position: [f64; 2],
    ) -> Result<(), String> {
        // Blank dialogue is intentional in credits; never trim it.
        let value = self.node(NodeKind::TextValue, position)?;
        self.property(value, "value", PropertyValue::String(text_source(text)))?;
        self.connect(value, "value", dialogue, "text")
    }
    fn sequence(
        &mut self,
        body: &[Statement],
        mut prev: (NodeId, String),
        mut x: f64,
        y: f64,
        owner: Option<NodeId>,
    ) -> Result<f64, String> {
        self.lane = self.lane.max(y);
        for (index, stmt) in body.iter().enumerate() {
            use Statement as S;
            let kind = match stmt {
                S::SetVar { .. } => NodeKind::SetVariable,
                S::LocalVar { .. } => NodeKind::LocalVariable,
                S::UiOpen { .. } => NodeKind::UiOpen,
                S::UiClose { .. } => NodeKind::UiClose,
                S::UiFocus { .. } => NodeKind::UiFocus,
                S::UiSetState { .. } => NodeKind::UiSetState,
                S::MotionPlay { .. } => NodeKind::MotionPlay,
                S::MotionStop { .. } => NodeKind::MotionStop,
                S::MotionWait { .. } => NodeKind::MotionWait,
                S::CharacterCompose { .. } => NodeKind::CharacterCompose,
                S::CharacterAttributes { .. } => NodeKind::CharacterAttributes,
                S::VideoPlay { .. } => NodeKind::VideoPlay,
                S::VideoPause { .. } => NodeKind::VideoPause,
                S::VideoResume { .. } => NodeKind::VideoResume,
                S::VideoStop { .. } => NodeKind::VideoStop,
                S::VideoSkip { .. } => NodeKind::VideoSkip,
                S::VideoWait { .. } => NodeKind::VideoWait,
                S::VideoSeek { .. } => NodeKind::VideoSeek,
                S::VideoVolume { .. } => NodeKind::VideoVolume,
                S::AccessibilityConfigure { .. } => NodeKind::AccessibilityConfigure,
                S::AccessibilitySpeak { .. } => NodeKind::AccessibilitySpeak,
                S::AccessibilityStop => NodeKind::AccessibilityStop,
                S::Dialogue { .. } => NodeKind::Dialogue,
                S::Choice { .. } => NodeKind::Choice,
                S::If { .. } => NodeKind::If,
                S::FunctionReturn { .. } => NodeKind::FunctionReturn,
                S::While { .. } => NodeKind::While,
                S::ForEach { .. } => NodeKind::ForEach,
                S::Jump { .. } => NodeKind::Jump,
                S::Call { .. } => NodeKind::Call,
                S::Return => NodeKind::Return,
                S::Scene { .. } => NodeKind::Scene,
                S::ShowSprite { .. } => NodeKind::SpriteShow,
                S::HideSprite { .. } => NodeKind::SpriteHide,
                S::MoveSprite { .. } => NodeKind::SpriteMove,
                S::MusicPlay { .. } => NodeKind::MusicPlay,
                S::MusicStop { .. } => NodeKind::MusicStop,
                S::MusicVolume { .. } => NodeKind::MusicVolume,
                S::SfxPlay { .. } => NodeKind::SfxPlay,
                S::SfxStop { .. } => NodeKind::SfxStop,
                S::VoicePlay { .. } => NodeKind::VoicePlay,
                S::VoiceStop => NodeKind::VoiceStop,
                S::CinematicShow { .. } => NodeKind::CinematicShow,
                S::CinematicHide { .. } => NodeKind::CinematicHide,
                S::UnlockEnding { .. } => NodeKind::UnlockEnding,
                S::Imagemap { .. } => NodeKind::Imagemap,
                S::TypewriterSet { .. } => NodeKind::TypewriterSet,
                S::TypewriterSpeed { .. } => NodeKind::TypewriterSpeed,
                S::Config { .. } => NodeKind::Config,
                other => return Err(format!("Instruction non prise en charge : {other:?}")),
            };
            let n = self.node(kind, [x, y])?;
            self.connect(prev.0, &prev.1, n, "exec_in")?;
            let mut continuation = "exec_out";
            match stmt {
                S::Config { key, value } => {
                    self.text(n, "key", key)?;
                    self.text(n, "value", value)?;
                }
                S::SetVar { name, value } | S::LocalVar { name, value } => {
                    self.variable(n, name)?;
                    self.text(n, "name", name)?;
                    self.input_expr(n, "value", value)?;
                }
                S::FunctionReturn { value } => self.input_expr(n, "value", value)?,
                S::UiOpen {
                    name,
                    arguments,
                    modal,
                    layer,
                } => {
                    self.input_expr(n, "name", name)?;
                    self.input_expr(n, "arguments", arguments)?;
                    self.input_expr(n, "modal", modal)?;
                    self.input_expr(n, "layer", layer)?;
                }
                S::UiClose { name } => self.input_expr(n, "name", name)?,
                S::MotionPlay { target, definition } => {
                    self.input_expr(n, "target", target)?;
                    self.input_expr(n, "definition", definition)?;
                }
                S::CharacterCompose {
                    character,
                    definition,
                } => {
                    self.input_expr(n, "character", character)?;
                    self.input_expr(n, "definition", definition)?;
                }
                S::CharacterAttributes {
                    character,
                    attributes,
                } => {
                    self.input_expr(n, "character", character)?;
                    self.input_expr(n, "attributes", attributes)?;
                }
                S::MotionStop { target } | S::MotionWait { target } => {
                    self.input_expr(n, "target", target)?
                }
                S::VideoPlay { name, definition } => {
                    self.input_expr(n, "name", name)?;
                    self.input_expr(n, "definition", definition)?;
                }
                S::VideoPause { name }
                | S::VideoResume { name }
                | S::VideoStop { name }
                | S::VideoSkip { name }
                | S::VideoWait { name } => self.input_expr(n, "name", name)?,
                S::VideoSeek { name, seconds } => {
                    self.input_expr(n, "name", name)?;
                    self.input_expr(n, "seconds", seconds)?;
                }
                S::VideoVolume { name, volume } => {
                    self.input_expr(n, "name", name)?;
                    self.input_expr(n, "volume", volume)?;
                }
                S::AccessibilityConfigure { settings } => {
                    self.input_expr(n, "settings", settings)?
                }
                S::AccessibilitySpeak { text } => self.input_expr(n, "text", text)?,
                S::AccessibilityStop => {}
                S::UiFocus { name, element } => {
                    self.input_expr(n, "name", name)?;
                    self.input_expr(n, "element", element)?;
                }
                S::UiSetState {
                    name,
                    element,
                    state,
                } => {
                    self.input_expr(n, "name", name)?;
                    self.input_expr(n, "element", element)?;
                    self.input_expr(n, "state", state)?;
                }
                S::While { condition, body } => {
                    continuation = "completed";
                    self.input_expr(n, "condition", condition)?;
                    self.lane += 960.0;
                    x = x.max(self.sequence(
                        body,
                        (n, "body".into()),
                        x + 960.0,
                        self.lane,
                        Some(n),
                    )?);
                }
                S::ForEach {
                    name,
                    collection,
                    body,
                } => {
                    continuation = "completed";
                    self.property(n, "name", PropertyValue::String(name.clone()))?;
                    self.input_expr(n, "collection", collection)?;
                    self.lane += 960.0;
                    x = x.max(self.sequence(
                        body,
                        (n, "body".into()),
                        x + 960.0,
                        self.lane,
                        Some(n),
                    )?);
                }
                S::Dialogue { character_id, text } => {
                    self.text(n, "character", character_id.as_deref().unwrap_or(""))?;
                    if let [rvn_parser::TextSegment::Interp(expression @ Expr::Call { name, .. })] =
                        text.0.as_slice()
                    {
                        if name == "string_to_text" {
                            let value = self.expression(expression, [x - 260.0, y + 340.0])?;
                            self.connect_expression(value, n, "text")?;
                        } else {
                            self.dialogue_text(n, text, [x - 260.0, y + 340.0])?;
                        }
                    } else if let [rvn_parser::TextSegment::Interp(expression @ Expr::Var(name))] =
                        text.0.as_slice()
                    {
                        if self.graph.variables.contains_key(name) {
                            let value = self.expression(expression, [x - 260.0, y + 340.0])?;
                            let before: std::collections::BTreeSet<_> =
                                self.graph.nodes.keys().copied().collect();
                            self.connect_expression(value, n, "text")?;
                            // This represents the existing [variable] dialogue,
                            // not a newly authored explicit strict cast.
                            for id in self
                                .graph
                                .nodes
                                .keys()
                                .copied()
                                .filter(|id| !before.contains(id))
                                .collect::<Vec<_>>()
                            {
                                if matches!(
                                    self.graph.nodes[&id].kind,
                                    NodeKind::ConvertStringToText | NodeKind::ConvertTextToString
                                ) {
                                    self.property(
                                        id,
                                        "legacy_passthrough",
                                        PropertyValue::Bool(true),
                                    )?;
                                }
                            }
                        } else {
                            self.dialogue_text(n, text, [x - 260.0, y + 340.0])?;
                        }
                    } else {
                        self.dialogue_text(n, text, [x - 260.0, y + 340.0])?;
                    }
                }
                S::Jump { target } | S::Call { target } => self.text(n, "target", target)?,
                S::Scene {
                    background,
                    transition,
                } => {
                    self.text(n, "background", background)?;
                    self.transition(n, transition)?;
                }
                S::ShowSprite {
                    character_id,
                    emotion,
                    position,
                    transition,
                } => {
                    self.text(n, "character", character_id)?;
                    self.text(n, "emotion", emotion.as_deref().unwrap_or(""))?;
                    if emotion.is_none() {
                        self.property(n, "use_default_sprite", PropertyValue::Bool(true))?;
                    }
                    self.text(
                        n,
                        "position",
                        &position
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_default(),
                    )?;
                    self.transition(n, transition)?;
                }
                S::HideSprite {
                    character_id,
                    transition,
                } => {
                    self.text(n, "character", character_id)?;
                    self.transition(n, transition)?;
                }
                S::MoveSprite {
                    character_id,
                    position,
                    transition,
                } => {
                    self.text(n, "character", character_id)?;
                    self.text(n, "position", &position.to_string())?;
                    self.transition(n, transition)?;
                }
                S::MusicPlay { file, transition } => {
                    self.text(n, "file", file)?;
                    self.transition(n, transition)?;
                }
                S::MusicStop { transition } => self.transition(n, transition)?,
                S::MusicVolume { level } => {
                    self.default(n, "level", PropertyValue::Float(f64::from(*level)))?
                }
                S::SfxPlay { file, transition } | S::SfxStop { file, transition } => {
                    if *transition != Transition::None {
                        return Err("Transition SFX non représentable actuellement".into());
                    }
                    self.text(n, "file", file)?;
                }
                S::VoicePlay { file } => self.text(n, "file", file)?,
                S::CinematicShow { id, transition } => {
                    self.text(n, "cinematic", id)?;
                    self.text(n, "transition", transition.as_deref().unwrap_or("none"))?;
                }
                S::CinematicHide { transition } => {
                    self.text(n, "transition", transition.as_deref().unwrap_or("none"))?
                }
                S::UnlockEnding { id } => self.text(n, "id", id)?,
                S::TypewriterSet { enabled } => {
                    self.default(n, "enabled", PropertyValue::Bool(*enabled))?
                }
                S::TypewriterSpeed { chars_per_sec } => {
                    self.default(n, "speed", PropertyValue::Int(*chars_per_sec as i64))?
                }
                S::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    continuation = "completed";
                    self.input_expr(n, "condition", condition)?;
                    for (key, branch) in [("then", then_branch), ("else", else_branch)] {
                        self.lane += 960.0;
                        x = x.max(self.sequence(
                            branch,
                            (n, key.into()),
                            self.graph.nodes[&n].position[0] + 960.0,
                            self.lane,
                            Some(n),
                        )?);
                    }
                }
                S::Choice { options } => {
                    continuation = "completed";
                    for option in options {
                        let pins = self
                            .graph
                            .add_choice_option(n, text_source(&option.label))
                            .map_err(err)?;
                        if let Some(condition) = &option.condition {
                            self.property(
                                n,
                                &format!("option_{}_condition", pins.index),
                                PropertyValue::String(expression_source(condition)),
                            )?;
                        }
                        self.lane += 960.0;
                        x = x.max(self.sequence(
                            &option.body,
                            (n, format!("option_{}", pins.index)),
                            self.graph.nodes[&n].position[0] + 960.0,
                            self.lane,
                            Some(n),
                        )?);
                    }
                }
                S::Imagemap {
                    background,
                    hover_image,
                    hotspots,
                } => {
                    continuation = "completed";
                    self.text(n, "background", background)?;
                    self.text(n, "hover", hover_image.as_deref().unwrap_or(""))?;
                    let mut specs = Vec::new();
                    for h in hotspots {
                        let name = h
                            .name
                            .as_deref()
                            .ok_or("Les zones sans nom ne sont pas prises en charge")?;
                        if name.contains(':') {
                            return Err("Nom de zone contenant ':' non représentable".into());
                        }
                        let a = &h.area;
                        let mut s = format!("{name}:{}:{}:{}:{}", a.x1, a.y1, a.x2, a.y2);
                        if let Some(a) = &h.hover_area {
                            s.push_str(&format!(":{}:{}:{}:{}", a.x1, a.y1, a.x2, a.y2));
                        }
                        specs.push(s);
                    }
                    self.graph.set_imagemap_hotspots(n, specs).map_err(err)?;
                    for (i, h) in hotspots.iter().enumerate() {
                        self.lane += 960.0;
                        x = x.max(self.sequence(
                            &h.body,
                            (n, format!("hotspot_{i}")),
                            self.graph.nodes[&n].position[0] + 960.0,
                            self.lane,
                            Some(n),
                        )?);
                    }
                }
                S::Return | S::VoiceStop => {}
                _ => unreachable!(),
            }
            x += 960.0;
            if matches!(stmt, S::Jump { .. } | S::Return | S::FunctionReturn { .. }) {
                if index + 1 != body.len() {
                    return Err(
                        "Instructions inaccessibles après jump/return : conversion refusée".into(),
                    );
                }
                return Ok(x);
            }
            prev = (n, continuation.into());
        }
        if let Some(owner) = owner {
            let end = self.graph.add_branch_end(owner, [x, y]).map_err(err)?;
            self.connect(prev.0, &prev.1, end, "exec_in")?;
        }
        Ok(x)
    }
}
