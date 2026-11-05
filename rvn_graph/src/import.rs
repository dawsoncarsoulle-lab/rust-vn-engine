//! Deterministic, loss-checked import of the narrative RVN subset.
//! Imports are resolved by the caller; unsupported statements fail explicitly.
use crate::*;
use rvn_parser::{BinOpKind, Expr, Script, Statement, Transition};
use std::collections::BTreeMap;
#[path="import_native_loop_demand.rs"]
mod native_loop_demand_import;
#[path="import_standard_macros_1020.rs"]
mod standard_macros_1020_import;
use native_loop_demand_import::{NativeLoopDeclaration,NativeLoopBody,native_index_body};

/// Re-import all source documents while retaining unchanged graph/node/pin
/// identities and user layout. Never modifies the supplied prior documents.
pub fn reimport_script(
    script: &Script,
    previous: &[GraphDocument],
) -> Result<Vec<GraphDocument>, String> {
    reimport_scopes(script, previous, true, None)
}

/// The source transaction validates the complete, resolved project. Its
/// writable scopes alone may legitimately reference an imported definition.
pub(crate) fn reimport_source_scopes(
    script: &Script,
    resolved: &Script,
    previous: &[GraphDocument],
) -> Result<Vec<GraphDocument>, String> {
    reimport_scopes(script, previous, false, Some(resolved))
}

fn reimport_scopes(
    script: &Script,
    previous: &[GraphDocument],
    validate_whole_project: bool,
    resolved: Option<&Script>,
) -> Result<Vec<GraphDocument>, String> {
    crate::validate_text_owners(previous)?;
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
    let mut imported = import_script_with_text_variables(script, &text_variables, validate_whole_project, resolved, previous)?;
    reconcile_prepared_import(imported,previous,validate_whole_project)
}

pub(crate) fn reconcile_prepared_import(imported:Vec<GraphDocument>,previous:&[GraphDocument],validate_whole_project:bool)->Result<Vec<GraphDocument>,String>{
    let context = imported.clone();
    reconcile_prepared_import_in_context(imported, previous, validate_whole_project, &context)
}

// Selection and compilation context are different at the mixed Native/Legacy
// boundary. Only scopes with retained owners are reconciled, but their calls
// must resolve against the complete source-reconstructed candidate, including
// a newly authored zero-parameter Unit declaration. Context remains read-only.
pub(crate) fn reconcile_prepared_import_in_context(imported:Vec<GraphDocument>,previous:&[GraphDocument],validate_whole_project:bool,imported_context:&[GraphDocument])->Result<Vec<GraphDocument>,String>{
    reconcile_prepared_import_context_library1027(imported,previous,validate_whole_project,imported_context,None,&Default::default())
}
fn reconcile_prepared_import_context_library1027(mut imported:Vec<GraphDocument>,previous:&[GraphDocument],validate_whole_project:bool,imported_context:&[GraphDocument],library:Option<&crate::LibraryGraphResources1027<'_>>,reserved:&std::collections::BTreeSet<GraphId>)->Result<Vec<GraphDocument>,String>{
    for graph in &imported {
        let owners:Vec<_> = imported_context.iter().filter(|owner|owner.graph_id==graph.graph_id).collect();
        let [owner] = owners.as_slice() else { return Err("Reconciled scope requires one actual graph in its complete imported context".into()); };
        if *owner != graph { return Err("Reconciled scope differs from its actual complete imported context".into()); }
    }
    crate::validate_text_owners(previous)?;
    if previous.iter().any(|graph|reserved.contains(&graph.graph_id)){return Err("Initial source presentation collides with an original nominal Library member identity".into())}
    let mut next_graph = previous
        .iter()
        .map(|g| g.graph_id.get())
        .chain(reserved.iter().map(|id|id.get())).max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("Graph identity space exhausted")?;
    let prior_project=previous;
    for graph in &mut imported {
        let matching: Vec<_> = previous
            .iter()
            .filter(|old| old.kind == graph.kind)
            .collect();
        match matching.as_slice() {
            [old] => {
                let mut previous = (*old).clone();
                // AST-only/Legacy import cannot recover a declarative Native
                // Init identity from a prior presentation with equal body[].
                // Source-aware declarations use the separate verified port.
                if crate::empty_native_init_0984::empty_graph(&previous) && !previous.inspector_evaluation.is_legacy() {
                    previous.inspector_evaluation=crate::InspectorEvaluationContract::LegacyStateless;
                    previous.schema_version=crate::GRAPH_SCHEMA_VERSION;
                }
                // Source registries must refresh even when the label's AST has
                // not changed (for example an init default or type changed).
                previous.variables = graph.variables.clone();
                previous.characters = graph.characters.clone();
                previous.normalize_assignment_value_outputs().map_err(err)?;
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
                previous.materialize_choice_conditions().map_err(err)?;
                previous.materialize_implicit_conversions().map_err(err)?;
                previous.set_narrative_metadata(graph.narrative)?;
                if let Some(resources)=library{previous.refresh_narrative_binding_types_library1027(&graph.source_global_binding_types()?,prior_project,resources)?;}
                else{previous.refresh_narrative_binding_types_in_project(&graph.source_global_binding_types()?, prior_project)?;}
                // Type-only conversions may be presentation in dialogue RVN.
                // Keep the complete prior graph when its logic is unchanged,
                // instead of dropping visible casts and their layout on reopen.
                let same_scoped=if let Some(resources)=library{
                    crate::codegen::transpile_library_graph1027(&previous,prior_project,resources).is_ok_and(|before|crate::codegen::transpile_library_graph1027(graph,imported_context,resources).is_ok_and(|after|before.ast==after.ast))
                }else{false};
                if same_scoped || (library.is_none() && ((crate::uses_unit_context1026(&previous)
                    && crate::codegen::transpile_unit_in_project(&previous,prior_project).is_ok_and(|before|
                        crate::codegen::transpile_unit_in_project(graph,imported_context).is_ok_and(|after|before.ast==after.ast)))
                    || transpile(&previous).is_ok_and(|before|transpile(graph).is_ok_and(|after|before.ast==after.ast))))
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
        if let Some(resources)=library{graph.refresh_narrative_binding_types_library1027(&graph.source_global_binding_types()?,imported_context,resources)?;crate::codegen::transpile_library_graph1027(graph,imported_context,resources).map_err(err)?;}
        else{graph.refresh_narrative_binding_types_in_project(&graph.source_global_binding_types()?, imported_context)?;
        if !crate::uses_unit_context1026(graph) { transpile(graph).map_err(err)?; }}
    }
    if validate_whole_project {
        transpile_project(&imported)?;
    }
    Ok(imported)
}

/// One init document followed by one document per source label, in source order.
pub fn import_script(script: &Script) -> Result<Vec<GraphDocument>, String> {
    import_script_with_text_variables(script, &std::collections::BTreeSet::new(), true, None, &[])
}

pub(crate) fn import_source_scopes(script: &Script, resolved: &Script) -> Result<Vec<GraphDocument>, String> {
    import_script_with_text_variables(script, &std::collections::BTreeSet::new(), false, Some(resolved), &[])
}

fn import_script_with_text_variables(
    script:&Script,text_variables:&std::collections::BTreeSet<String>,validate_whole_project:bool,
    resolved:Option<&Script>,owners:&[GraphDocument])->Result<Vec<GraphDocument>,String>{
    import_script_allocated(script,text_variables,validate_whole_project,resolved,owners,&mut||Err("Fresh Unit parameter import requires the owning transaction's injected OS identity allocator".into()))
}
pub fn import_script_with_parameter_allocator(script:&Script,mut allocate:impl FnMut()->Result<String,String>)->Result<Vec<GraphDocument>,String>{
    import_script_allocated(script,&Default::default(),true,None,&[],&mut allocate)
}
fn import_script_allocated(
    script: &Script,
    text_variables: &std::collections::BTreeSet<String>,
    validate_whole_project: bool,
    resolved: Option<&Script>,
    owners: &[GraphDocument],
    allocate:&mut dyn FnMut()->Result<String,String>,
) -> Result<Vec<GraphDocument>, String> {
    import_script_selected(script, text_variables, validate_whole_project, resolved, owners, allocate, None)
}

// A frozen Native projection is not an editable authoring project. Inventory,
// source semantics and environments remain complete; only graph construction
// is selective, and no Unit parameter allocator participates in this purpose.
fn import_script_selected(
    script: &Script,
    text_variables: &std::collections::BTreeSet<String>,
    validate_whole_project: bool,
    resolved: Option<&Script>,
    owners: &[GraphDocument],
    allocate: &mut dyn FnMut()->Result<String,String>,
    frozen_empty_init: Option<bool>,
) -> Result<Vec<GraphDocument>, String> {
    import_script_selected_library1027(script,text_variables,validate_whole_project,resolved,owners,allocate,frozen_empty_init,None,None)
}
fn import_script_selected_library1027<'a>(script:&Script,text_variables:&std::collections::BTreeSet<String>,validate_whole_project:bool,resolved:Option<&Script>,owners:&[GraphDocument],allocate:&mut dyn FnMut()->Result<String,String>,frozen_empty_init:Option<bool>,library:Option<&'a crate::LibraryGraphResources1027<'a>>,library_member:Option<&rvn_parser::function_library_asset1023::QualifiedLibraryFunction>)->Result<Vec<GraphDocument>,String>{
    if script.iter().any(|statement|matches!(statement,Statement::FunctionLibrary{..})){
        return Err("Qualified Function Library Graph import requires its original membership/signature context; legacy name-only import cannot consume it".into());
    }
    rvn_parser::scheduled_source::native_loop_contract::validate_authored_bindings(script)?;
    let resolved_scene_entries: std::collections::BTreeSet<_> = resolved.unwrap_or(script).iter().filter_map(|statement| {
        if let Statement::SceneLabel { name } = statement { Some(name.clone()) } else { None }
    }).collect();
    // The caller's executable AST is immutable. A canonical project export can
    // append one unowned transport jump after an identity-bearing Native body.
    // Remove only that footer from the derived Graph body, then require exact
    // full-project re-export below. All authored/scheduled jumps remain nodes.
    let transport_end = crate::project::project_transport_end(script);
    let mut adapted_transport = false;
    let mut init = Vec::new();
    let mut functions = Vec::new();
    let mut labels: Vec<(String, Script)> = Vec::new();
    let narrative_context = rvn_parser::NarrativeContext::from_script(resolved.unwrap_or(script));
    let scene_entries: std::collections::BTreeSet<_> = script.iter().filter_map(|statement| {
        if let Statement::SceneLabel { name } = statement { Some(name.as_str()) } else { None }
    }).collect();
    for statement in script {
        match statement {
            Statement::Init { body } => init.extend(body.clone()),
            Statement::UnitFunction { name,parameters,body }
            | Statement::Function {
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
                    Statement::UnitFunction { .. } => GraphKind::UnitFunction { name:name.clone() },
                    Statement::Screen { .. } => GraphKind::Screen { name: name.clone() },
                    Statement::Handler { .. } => GraphKind::Handler { name: name.clone() },
                    _ => GraphKind::Function { name: name.clone() },
                };
                if functions.iter().any(|(existing, _, _)| existing == &kind) {
                    return Err(format!("Déclaration dupliquée : {name}"));
                }
                functions.push((kind, parameters.clone(), body.clone()));
            }
            Statement::Label { name } | Statement::SceneLabel { name } => {
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
    let mut variables = initialized_variables_library1027(&init, text_variables, library)?;
    // Every contextual scope receives the complete source-owned registry,
    // including ordinary authoring reopen. Foreign Init commands remain
    // read-only context, never nodes in the locally authored Init graph.
    let character_init: Vec<_> = resolved.unwrap_or(script).iter().filter_map(|s|match s {
        Statement::Init{body}=>Some(body.as_slice()),_=>None,
    }).flatten().collect();
    for statement in character_init {
        match statement {
            Statement::CharacterCreate { id, display_name } => {
                if characters
                    .insert(id.clone(), display_name.clone())
                    .is_some()
                {
                    return Err(format!("Personnage déclaré deux fois : {id}"));
                }
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
        let project_footer = transport_end.is_some_and(|end|
            body.iter().any(|statement| matches!(statement, Statement::ScheduledConsumer { .. }))
            && matches!(body.last(), Some(Statement::Jump { target }) if target == end));
        if project_footer {
            body.pop();
            adapted_transport = true;
        }
        if !project_footer && !body
            .last()
            .is_some_and(crate::project::is_unconditional_graph_exit)
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
    let conflicts = if let Some(resolved) = resolved {
        // Use the complete validated project for types, while the blocks below
        // still contain only scopes physically authored in this source file.
        // Imported initialization never becomes an editable Init node.
        let context = global_context_blocks(resolved);
        variables = initialized_variables_library1027(&context[0].1, text_variables, library)?;
        infer_project_globals_library1027(&context, &mut variables, narrative_context, &resolved_scene_entries, library)?
    } else {
        infer_project_globals_library1027(&blocks, &mut variables, narrative_context, &resolved_scene_entries, library)?
    };
    if frozen_empty_init.is_some() {
        // The existing adapted-transport proof reexports the complete project.
        // A Native-only subset must never pretend to satisfy that equality.
        if adapted_transport {
            return Err("Mixed Unit frozen Native project transport requires the existing complete project proof; Native-only reconstruction cannot bypass it".into());
        }
        for (kind, body, _) in &blocks {
            if matches!(kind, GraphKind::UnitFunction { .. })
                && crate::native_source_binding::contains_native(body)
            {
                return Err("Frozen Native source cannot promote a schema14 Unit definition".into());
            }
        }
    }
    let unit_declarations = std::sync::Arc::new(if frozen_empty_init.is_some() {
        BTreeMap::new()
    } else {
        crate::unit_graph_source1003::declarations(&blocks, owners, allocate)?
    });
    let mut result = Vec::new();
    for (index, (kind, body, parameters)) in blocks.into_iter().enumerate() {
        if let Some(declared_empty_init) = frozen_empty_init {
            let native = crate::native_source_binding::contains_native(&body);
            if !native && !(declared_empty_init && kind == GraphKind::Init) {
                continue;
            }
            validate_frozen_program_operations(&body, true)?;
        }
        let mut graph = if let GraphKind::UnitFunction{name}=&kind { unit_declarations.get(name).ok_or("Prepared actual Unit entry missing")?.clone() }
            else { GraphDocument::new(GraphId::new(index as u64 + 1), kind.clone()) };
        if let Some(previous) = owners.iter().find(|g| g.kind == kind) {
            if let Some(owner) = previous.text_owner.clone() { graph.enable_text_owner(owner)?; }
            if previous.schema_version >= crate::GRAPH_TABLE_TEXT_SCHEMA_VERSION {
                // A cache may retain Text/Name presentation capabilities, but
                // Native schema/loop opt-in must come from versioned source.
                // Copying schema12 without its source contract makes even an
                // unmarked empty Init fail before it can replace that cache.
                if !matches!(kind,GraphKind::UnitFunction{..}) { graph.schema_version = previous.schema_version.min(crate::GRAPH_NAME_TEXT_SCHEMA_VERSION); }
            }
        }
        if let GraphKind::Label { name } = &kind {
            if narrative_context == rvn_parser::NarrativeContext::ExplicitScenes {
                graph.set_narrative_metadata(Some(if scene_entries.contains(name.as_str()) {
                    crate::NarrativeMetadata::SceneEntry
                } else { crate::NarrativeMetadata::SceneFragment }))?;
            }
        }
        if let (Some(resources),Some(owner))=(library,library_member) {
            if matches!(kind,GraphKind::Function{..}|GraphKind::UnitFunction{..}){
            graph.schema_version=crate::GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION;
            let mut metadata=crate::LibraryGraphMetadata1027::default();
            let member=resources.member(owner)?;graph.graph_id=GraphId::new(owner.graph_id);metadata.owner=Some(owner.library.clone());metadata.entry_parameter_ids=member.parameter_ids().to_vec();
            graph.library1027=Some(metadata);
            }
        }
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
                    statement @ Statement::ScheduledConsumer{body,..}=>{
                        if let Some((index,children))=native_index_body(statement){names.insert(index.into());collect_bindings(children,names,calculation,handler);}
                        else{collect_bindings(body,names,calculation,handler);}
                    },
                    Statement::LocalVar { name, .. } => {
                        names.insert(name.clone());
                    }
                    Statement::SetVar { name, .. } if calculation => {
                        names.insert(name.clone());
                    }
                    Statement::ForEach { name, body, .. } | Statement::ForLoopWithBreak { name, body, .. } => {
                        if calculation || handler {
                            names.insert(name.clone());
                        }
                        collect_bindings(body, names, calculation, handler);
                    }
                    Statement::Sequence{branches}=>for b in branches{collect_bindings(b,names,calculation,handler);},
                    Statement::StandardMacroDispatch{dispatch}=>for branch in &dispatch.branches{collect_bindings(&branch.body,names,calculation,handler);},
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
            matches!(kind, GraphKind::Handler { .. } | GraphKind::UnitFunction { .. }),
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
            narrative_shadow: bool,
            captures: &ScheduledBindingTypes,
        ) {
            fn binding(
                name: &str,
                ty: ValueType,
                default: Option<PropertyValue>,
                locals: &std::collections::BTreeSet<String>,
                parameters: &[String],
                vars: &mut BTreeMap<String, VariableDefinition>,
                written: &mut std::collections::BTreeSet<String>,
                narrative_shadow: bool,
            ) {
                if parameters.iter().any(|parameter| parameter == name) {
                    return;
                }
                if !locals.contains(name) && vars.contains_key(name) {
                    return;
                }
                // A new scene local shadows the global declaration instead
                // of becoming a heterogeneous write to that global. Keep the
                // prior legacy lexical inference unchanged.
                let previous = if narrative_shadow && locals.contains(name) && !written.contains(name) { None }
                    else { vars.get(name).map(|variable| variable.value_type.clone()) };
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
                    statement @ Statement::ScheduledConsumer { steps, body, .. } => {
                        let captures = ScheduledBindingTypes::from_steps(steps, vars);
                        if let Some((index,children))=native_index_body(statement){
                            binding(index,ValueType::Int,Some(PropertyValue::Int(0)),locals,parameters,vars,written,narrative_shadow);
                            infer_bindings(children,locals,parameters,vars,written,narrative_shadow,&captures);
                        }else{infer_bindings(body, locals, parameters, vars, written, narrative_shadow, &captures);}
                    },
                    Statement::SetVar { name, value } | Statement::LocalVar { name, value } => {
                        binding(
                            name,
                            captures.expression_type(value, vars),
                            literal(value),
                            locals,
                            parameters,
                            vars,
                            written,
                            narrative_shadow,
                        )
                    }
                    Statement::ForEach {
                        name,
                        collection,
                        body,
                    } => {
                        let ty = match captures.expression_type(collection, vars) {
                            ValueType::List(inner) => *inner,
                            _ => ValueType::Any,
                        };
                        binding(name, ty, None, locals, parameters, vars, written, narrative_shadow);
                        infer_bindings(body, locals, parameters, vars, written, narrative_shadow, captures);
                    }
                    Statement::ForLoopWithBreak{name,body,..}=>{binding(name,ValueType::Int,None,locals,parameters,vars,written,narrative_shadow);infer_bindings(body,locals,parameters,vars,written,narrative_shadow,captures);},
                    Statement::Sequence{branches}=>for b in branches{infer_bindings(b,locals,parameters,vars,written,narrative_shadow,captures);},
                    Statement::StandardMacroDispatch{dispatch}=>for branch in &dispatch.branches{infer_bindings(&branch.body,locals,parameters,vars,written,narrative_shadow,captures);},
                    Statement::While { body, .. } | Statement::Init { body } => {
                        infer_bindings(body, locals, parameters, vars, written, narrative_shadow, captures)
                    }
                    Statement::If {
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        infer_bindings(then_branch, locals, parameters, vars, written, narrative_shadow, captures);
                        infer_bindings(else_branch, locals, parameters, vars, written, narrative_shadow, captures);
                    }
                    Statement::Choice { options } => {
                        for option in options {
                            infer_bindings(&option.body, locals, parameters, vars, written, narrative_shadow, captures);
                        }
                    }
                    Statement::Imagemap { hotspots, .. } => {
                        for hotspot in hotspots {
                            infer_bindings(&hotspot.body, locals, parameters, vars, written, narrative_shadow, captures);
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
            graph.narrative.is_some(),
            &ScheduledBindingTypes::default(),
        );
        let root = if matches!(kind,GraphKind::UnitFunction{..}) { crate::unit_graph_source1003::entry(&graph)? } else { graph
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
            .map_err(err)? };
        if matches!(
            kind,
            GraphKind::Function { .. } | GraphKind::Screen { .. } | GraphKind::Handler { .. }
        ) {
            graph
                .set_property(root, "parameters", PropertyValue::StringList(parameters))
                .map_err(err)?;
        }
        let globals = graph.narrative.map(|_| variables.iter()
            .map(|(name, definition)|(name.clone(),definition.value_type.clone())).collect());
        let binding_states = if graph.narrative == Some(crate::NarrativeMetadata::SceneFragment) {
            bindings.iter().map(|name|(name.clone(),3)).collect()
        } else { BTreeMap::new() };
        let mut builder = Builder { graph, library1027:library, unit_declarations:unit_declarations.clone(), lane: 0.0, range_owners:Vec::new(),
            narrative_globals:globals, narrative_locals:bindings.clone(), binding_states, inspectors:Default::default(), native_producers:Default::default(),native_active:Default::default(),native_nodes:Default::default(),native_assignment_pending:Default::default(),native_scope:None,native_constant_consumer:None,native_loop_pending:None,native_loop_declarations:Default::default(),native_loop_bodies:vec![],macro_definitions:Default::default() };
        builder.predeclare_assignment_outputs(&body)?;
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
        if !builder.native_assignment_pending.is_empty(){return Err("Native assignment output has no actual execution declaration".into());}
        if builder.native_scope.is_some() {
            builder.graph.restore_native_source_identities(&builder.native_nodes)?;
            let generated=crate::native_frame_preparation::prepare_native_unchecked(&builder.graph).map_err(err)?;
            // The shared character registry emits its own canonical Init
            // before this graph's actual definition. It has no authored node
            // identity and is not part of `body` (see registry extraction
            // above). Admit only that exact generated prelude, not an
            // arbitrary first Init or any executable prefix.
            let scope=if builder.graph.characters.is_empty(){generated.ast.as_slice()}else{
                let prelude=Statement::Init{body:builder.graph.characters.iter().map(|(id,display_name)|Statement::CharacterCreate{id:id.clone(),display_name:display_name.clone()}).collect()};
                generated.ast.as_slice().strip_prefix(std::slice::from_ref(&prelude))
                    .ok_or("Native preparation changed its canonical shared character prelude")?
            };
            let roundtrip=match scope {
                [Statement::Function{body,..}|Statement::Screen{body,..}|Statement::Handler{body,..}|Statement::Init{body}]=>body.clone(),
                [Statement::Label{..}|Statement::SceneLabel{..},body @ ..]=>body.to_vec(),
                _=>return Err("Native preparation did not preserve its actual definition scope".into()),
            };
            if roundtrip!=body {return Err("Native source cannot be represented without semantic rewriting; round-trip refused".into());}
            layout_imported_values(&mut builder.graph);
            result.push(builder.graph);
            continue;
        }
        builder.graph.materialize_visible_defaults().map_err(err)?;
        layout_imported_values(&mut builder.graph);
        if let Some(globals)=builder.narrative_globals.clone() {
            // These are the real entry-pin declarations already prepared
            // from this source's blocks and retained authoring owners above.
            // They are analysis context only; no caller/callee is fabricated,
            // and the complete result still takes the original export gates.
            let declarations = builder.unit_declarations.values().cloned().collect::<Vec<_>>();
            if let Some(resources)=library {builder.graph.refresh_narrative_binding_types_library1027(&globals,&declarations,resources)?;}
            else{builder.graph.refresh_narrative_binding_types_in_project(&globals, &declarations)?;}
        }
        builder.graph.adopt_nominal_enum_schema1026();
        // Catch omitted/incorrect pins immediately, before any files are written.
        if library.is_none() && !crate::uses_unit_context1026(&builder.graph) { transpile(&builder.graph).map_err(err)?; }
        result.push(builder.graph);
    }
    if adapted_transport {
        // Native IDs came from the actual source. Legacy root/footer IDs are
        // provisional allocations; reconcile them before project validation,
        // which must not mistake a same-numbered Native graph for its footer.
        crate::native_frame_preparation::reconcile_imported_graph_ids(&mut result)?;
    }
    if library.is_none() && (validate_whole_project || adapted_transport) {
        let generated = transpile_project(&result)?;
        if adapted_transport && generated.ast != *script {
            return Err("Native project transport must preserve the complete executable source exactly".into());
        }
    }
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

// Transient type evidence from the actual preceding steps of one consumer.
// No source declarations, values or producer identities are fabricated. This
// table is discarded at the consumer boundary and is never serialized.
#[derive(Default)]
struct ScheduledBindingTypes {
    values: BTreeMap<String, ScheduledBindingType>,
}
enum ScheduledBindingType { Capture(ValueType), Inspector }
impl ScheduledBindingTypes {
    fn resolve(&self, expression: &Expr) -> Option<ValueType> {
        let Expr::Call { name, args } = expression else { return None; };
        match name.as_str() {
            "__rvn_captured_v1" => Some(match args.as_slice() {
                [Expr::Str(id)] => match self.values.get(id) {
                    Some(ScheduledBindingType::Capture(ty)) => ty.clone(),
                    _ => ValueType::Any,
                },
                _ => ValueType::Any,
            }),
            "__rvn_terminal_v1" => Some(match args.as_slice() {
                [Expr::Str(id), Expr::Str(port)]
                    if matches!(self.values.get(id), Some(ScheduledBindingType::Inspector)) => match port.as_str() {
                        "valid" => ValueType::Bool,
                        "table_id" => ValueType::Name,
                        "key" => ValueType::String,
                        _ => ValueType::Any,
                    },
                _ => ValueType::Any,
            }),
            _ => None,
        }
    }
    fn expression_type(&self, value: &Expr, variables: &BTreeMap<String, VariableDefinition>) -> ValueType {
        if self.values.is_empty() { return crate::type_inference::expression_type(value, variables); }
        crate::type_inference::expression_type_with_resolver(value, variables, &|expr| self.resolve(expr))
    }
    fn expression_type_library1027(&self, value: &Expr, variables: &BTreeMap<String, VariableDefinition>,
        library: Option<&crate::LibraryGraphResources1027<'_>>) -> Result<ValueType, String> {
        match library {
            Some(resources) => resources.source_expression_type_with_resolver1027(value, variables, &|expr| self.resolve(expr)),
            None => Ok(self.expression_type(value, variables)),
        }
    }
    fn from_steps(steps: &[rvn_parser::ConsumerStep], variables: &BTreeMap<String, VariableDefinition>) -> Self {
        Self::from_steps_library1027(steps, variables, None)
            .expect("Ordinary capture type inference has no qualified resolver failure")
    }
    fn from_steps_library1027(steps: &[rvn_parser::ConsumerStep], variables: &BTreeMap<String, VariableDefinition>,
        library: Option<&crate::LibraryGraphResources1027<'_>>) -> Result<Self, String> {
        let mut found = Self::default();
        // Parser/graph admission has its own structural limits. This analysis
        // also stays bounded and never makes a bad identity into type evidence.
        for step in steps.iter().take(65_536) {
            let (id, value, inspection) = match step {
                rvn_parser::ConsumerStep::Capture { producer, value } => (producer, value, false),
                rvn_parser::ConsumerStep::InspectTextTable { producer, value } => (producer, value, true),
            };
            let Ok((identity, legacy_port)) = crate::native_frame_preparation::decode_producer(id) else { continue; };
            let ty = if inspection {
                if identity.kind != NodeKind::GetStringTableIdAndKey || legacy_port.is_some() { continue; }
                ScheduledBindingType::Inspector
            } else { ScheduledBindingType::Capture(found.expression_type_library1027(value, variables, library)?) };
            if found.values.contains_key(id) {
                // Invalid duplicates cannot narrow a type even in a read-only
                // query. Full source import still rejects the malformed plan.
                found.values.insert(id.clone(), ScheduledBindingType::Capture(ValueType::Any));
            } else { found.values.insert(id.clone(), ty); }
        }
        Ok(found)
    }
}

fn initialized_variables(
    init: &[Statement],
    text_variables: &std::collections::BTreeSet<String>,
) -> BTreeMap<String, VariableDefinition> {
    initialized_variables_library1027(init, text_variables, None)
        .expect("Ordinary initial type inference has no qualified resolver failure")
}
fn initialized_variables_library1027(
    init: &[Statement], text_variables: &std::collections::BTreeSet<String>,
    library: Option<&crate::LibraryGraphResources1027<'_>>,
) -> Result<BTreeMap<String, VariableDefinition>, String> {
    let mut variables = BTreeMap::new();
    fn collect(init: &[Statement], variables: &mut BTreeMap<String, VariableDefinition>,
        text_variables: &std::collections::BTreeSet<String>, captures: &ScheduledBindingTypes, library: Option<&crate::LibraryGraphResources1027<'_>>) -> Result<(), String> {
    for statement in init {
        if let Statement::ScheduledConsumer { steps, body, .. } = statement {
            // Captures read the entry environment, before the consumer can
            // introduce a same-named Local or overwrite a global.
            let captures = ScheduledBindingTypes::from_steps_library1027(steps, variables, library)?;
            collect(body, variables, text_variables, &captures, library)?;
            continue;
        }
        let Statement::SetVar { name, value } = statement else { continue; };
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
            if cast.is_some() { PropertyValue::String(String::new()) }
            else { PropertyValue::Int(0) }
        });
        let value_type = match &default_value {
            _ if cast.is_some_and(|(kind, _)| kind == "string_to_text") => ValueType::InterpolatedText,
            _ if cast.is_some_and(|(kind, _)| kind == "text_to_string") => ValueType::String,
            _ if initial_literal.is_none() => captures.expression_type_library1027(value, variables, library)?,
            PropertyValue::Bool(_) => ValueType::Bool,
            PropertyValue::Int(_) => ValueType::Int,
            PropertyValue::Float(_) => ValueType::Float,
            PropertyValue::Name(_) => ValueType::Name,
            PropertyValue::Enum(value) => ValueType::Enum(value.asset),
            PropertyValue::StringList(items) => ValueType::List(Box::new(
                if items.is_empty() { ValueType::Any } else { ValueType::String }
            )),
            _ if text_variables.contains(name) => ValueType::InterpolatedText,
            _ => ValueType::String,
        };
        variables.insert(name.clone(), VariableDefinition {
            name: name.clone(), value_type, default_value,
        });
    }
    Ok(())
    }
    collect(init, &mut variables, text_variables, &ScheduledBindingTypes::default(), library)?;
    Ok(variables)
}

// Build only the environments used by global inference. This never imports
// nodes from dependencies, whose runtime statements need not be editable.
fn global_context_blocks(script: &Script) -> Vec<(GraphKind, Script, Vec<String>)> {
    let mut init = Vec::new();
    let mut blocks = Vec::new();
    let mut narrative = None;
    for statement in script {
        match statement {
            Statement::Init { body } => init.extend(body.clone()),
            Statement::Handler { name, parameters, body } | Statement::UnitFunction { name, parameters, body } => {
                // Global inference inspects the actual Unit body as a lexical
                // activation. It creates no graph, entry pin or parameter ID.
                let kind = if matches!(statement, Statement::UnitFunction {..}) {
                    GraphKind::UnitFunction {name:name.clone()}
                } else {GraphKind::Handler {name:name.clone()}};
                blocks.push((kind, body.clone(), parameters.clone()));
            }
            Statement::Label { name } | Statement::SceneLabel { name } => {
                narrative = Some(blocks.len());
                blocks.push((GraphKind::Label { name: name.clone() }, Vec::new(), Vec::new()));
            }
            Statement::Function { .. } | Statement::Screen { .. } => {},
            other => {
                if let Some(index) = narrative { blocks[index].1.push(other.clone()); }
            }
        }
    }
    blocks.insert(0, (GraphKind::Init, init, Vec::new()));
    blocks
}

pub(crate) fn source_global_names(script: &Script) -> std::collections::BTreeSet<String> {
    source_global_names_with_context(script, rvn_parser::NarrativeContext::from_script(script))
}

pub(crate) fn source_global_names_with_context(script: &Script, context: rvn_parser::NarrativeContext) -> std::collections::BTreeSet<String> {
    let blocks = global_context_blocks(script);
    let mut variables = initialized_variables(&blocks[0].1, &std::collections::BTreeSet::new());
    let scene_entries=script.iter().filter_map(|statement| match statement {
        Statement::SceneLabel {name}=>Some(name.clone()), _=>None,
    }).collect();
    infer_project_globals(&blocks, &mut variables, context, &scene_entries);
    variables.into_keys().collect()
}

fn infer_project_globals(
    blocks: &[(GraphKind, Script, Vec<String>)],
    variables: &mut BTreeMap<String, VariableDefinition>,
    context: rvn_parser::NarrativeContext,
    scene_entries: &std::collections::BTreeSet<String>,
) -> BTreeMap<String, Vec<String>> {
    infer_project_globals_library1027(blocks, variables, context, scene_entries, None)
        .expect("Ordinary global type inference has no qualified resolver failure")
}
fn infer_project_globals_library1027(
    blocks: &[(GraphKind, Script, Vec<String>)],
    variables: &mut BTreeMap<String, VariableDefinition>,
    context: rvn_parser::NarrativeContext,
    scene_entries: &std::collections::BTreeSet<String>,
    library: Option<&crate::LibraryGraphResources1027<'_>>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    use std::collections::BTreeSet;
    fn shadowed(body: &[Statement], locals: &mut BTreeSet<String>, foreach_local: bool) {
        for statement in body {
            match statement {
                statement @ Statement::ScheduledConsumer { body, .. } => {
                    if let Some((index,children))=native_index_body(statement){locals.insert(index.into());shadowed(children,locals,foreach_local);}
                    else{shadowed(body, locals, foreach_local);}
                },
                Statement::LocalVar { name, .. } => {
                    locals.insert(name.clone());
                }
                Statement::ForEach { name, body, .. } | Statement::ForLoopWithBreak { name, body, .. } => {
                    if foreach_local { locals.insert(name.clone()); }
                    shadowed(body, locals, foreach_local);
                }
                Statement::Sequence{branches}=>for b in branches{shadowed(b,locals,foreach_local);},
                Statement::StandardMacroDispatch{dispatch}=>for branch in &dispatch.branches{shadowed(&branch.body,locals,foreach_local);},
                Statement::While { body, .. } | Statement::Init { body } => shadowed(body, locals, foreach_local),
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    shadowed(then_branch, locals, foreach_local);
                    shadowed(else_branch, locals, foreach_local);
                }
                Statement::Choice { options } => {
                    for option in options { shadowed(&option.body, locals, foreach_local); }
                }
                Statement::Imagemap { hotspots, .. } => {
                    for hotspot in hotspots { shadowed(&hotspot.body, locals, foreach_local); }
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
        captures: &ScheduledBindingTypes,
        library: Option<&crate::LibraryGraphResources1027<'_>>,
    ) -> Result<(), String> {
        let mut ty = captures.expression_type_library1027(value, env, library)?;
        if locals.contains(name) {
            env.insert(
                name.into(),
                VariableDefinition {
                    name: name.into(),
                    value_type: ty,
                    default_value: literal(value).unwrap_or(PropertyValue::Int(0)),
                },
            );
            return Ok(());
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
        Ok(())
    }
    fn walk(
        body: &[Statement],
        locals: &BTreeSet<String>,
        env: &mut BTreeMap<String, VariableDefinition>,
        globals: &mut BTreeMap<String, VariableDefinition>,
        observed: &mut BTreeMap<String, Vec<ValueType>>,
        captures: &ScheduledBindingTypes,
        library: Option<&crate::LibraryGraphResources1027<'_>>,
    ) -> Result<(), String> {
        for statement in body {
            match statement {
                statement @ Statement::ScheduledConsumer { steps, body, .. } => {
                    let captures = ScheduledBindingTypes::from_steps_library1027(steps, env, library)?;
                    if let Some((index,children))=native_index_body(statement){
                        record(index,&Expr::Int(0),locals,env,globals,observed,&captures, library)?;
                        walk(children,locals,env,globals,observed,&captures, library)?;
                    }else{walk(body, locals, env, globals, observed, &captures, library)?;}
                },
                Statement::SetVar { name, value } | Statement::LocalVar { name, value } => {
                    record(name, value, locals, env, globals, observed, captures, library)?
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
                    record(name, &value, locals, env, globals, observed, captures, library)?;
                    walk(body, locals, env, globals, observed, captures, library)?;
                }
                Statement::ForLoopWithBreak{name,body,..}=>{record(name,&Expr::Int(0),locals,env,globals,observed,captures, library)?;walk(body,locals,env,globals,observed,captures, library)?;},
                Statement::Sequence{branches}=>for b in branches{walk(b,locals,env,globals,observed,captures, library)?;},
                Statement::StandardMacroDispatch{dispatch}=>for branch in &dispatch.branches{walk(&branch.body,locals,env,globals,observed,captures, library)?;},
                Statement::While { body, .. } | Statement::Init { body } => {
                    walk(body, locals, env, globals, observed, captures, library)?
                }
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    let original = env.clone();
                    walk(then_branch, locals, env, globals, observed, captures, library)?;
                    let mut alternate = original.clone();
                    walk(else_branch, locals, &mut alternate, globals, observed, captures, library)?;
                    merge_env(env, &alternate, &original);
                }
                Statement::Choice { options } => {
                    let original = env.clone();
                    for option in options {
                        let mut branch = original.clone();
                        walk(&option.body, locals, &mut branch, globals, observed, captures, library)?;
                        merge_env(env, &branch, &original);
                    }
                }
                Statement::Imagemap { hotspots, .. } => {
                    let original = env.clone();
                    for hotspot in hotspots {
                        let mut branch = original.clone();
                        walk(&hotspot.body, locals, &mut branch, globals, observed, captures, library)?;
                        merge_env(env, &branch, &original);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    // Only scopes with a proved fresh activation use definite-local analysis.
    // Fragments can inherit caller bindings and keep the existing conservative
    // analysis until the project provenance supplies that entry environment.
    fn ordered_walk(
        body: &[Statement], bound: &mut BTreeSet<String>,
        env: &mut BTreeMap<String, VariableDefinition>, globals: &mut BTreeMap<String, VariableDefinition>,
        observed: &mut BTreeMap<String, Vec<ValueType>>, captures: &ScheduledBindingTypes,
        handler: bool,
        library: Option<&crate::LibraryGraphResources1027<'_>>,
    ) -> Result<(), String> {
        for statement in body {
            match statement {
                statement @ Statement::ScheduledConsumer { steps, body, .. } => {
                    let captures=ScheduledBindingTypes::from_steps_library1027(steps,env,library)?;
                    if let Some((index,children))=native_index_body(statement){
                        let original=env.clone();let mut branch=env.clone();let mut local=bound.clone();local.insert(index.into());
                        record(index,&Expr::Int(0),&local,&mut branch,globals,observed,&captures, library)?;
                        ordered_walk(children,&mut local,&mut branch,globals,observed,&captures,handler, library)?;
                        merge_env(env,&branch,&original);
                    }else{ordered_walk(body,bound,env,globals,observed,&captures,handler, library)?;}
                }
                Statement::LocalVar { name,value } => {
                    // RHS/captures read the entry environment. Marking this
                    // write local must not change that environment beforehand.
                    let mut write_bound=bound.clone();write_bound.insert(name.clone());
                    record(name,value,&write_bound,env,globals,observed,captures, library)?;
                    bound.insert(name.clone());
                }
                Statement::SetVar {name,value} => record(name,value,bound,env,globals,observed,captures, library)?,
                Statement::Sequence {branches} => for branch in branches {ordered_walk(branch,bound,env,globals,observed,captures,handler, library)?;},
                Statement::StandardMacroDispatch{dispatch}=>for branch in &dispatch.branches{ordered_walk(&branch.body,bound,env,globals,observed,captures,handler, library)?;},
                Statement::If {then_branch,else_branch,..} => {
                    let original=env.clone();let entry=bound.clone();let mut alternate=original.clone();let mut other=entry;
                    ordered_walk(then_branch,bound,env,globals,observed,captures,handler, library)?;
                    ordered_walk(else_branch,&mut other,&mut alternate,globals,observed,captures,handler, library)?;
                    merge_env(env,&alternate,&original);
                    *bound=bound.intersection(&other).cloned().collect();
                }
                Statement::While {body,..} => {
                    let original=env.clone();let mut branch=env.clone();let mut local=bound.clone();
                    ordered_walk(body,&mut local,&mut branch,globals,observed,captures,handler, library)?;
                    merge_env(env,&branch,&original); // zero iterations remain possible
                }
                Statement::ForEach {name,collection,body} => {
                    let original=env.clone();let mut branch=env.clone();let mut local=bound.clone();
                    if handler {local.insert(name.clone());}
                    let value=Expr::Index{target:Box::new(collection.clone()),index:Box::new(Expr::Int(0))};
                    record(name,&value,&local,&mut branch,globals,observed,captures, library)?;
                    ordered_walk(body,&mut local,&mut branch,globals,observed,captures,handler, library)?;
                    merge_env(env,&branch,&original);
                }
                Statement::ForLoopWithBreak {name,body,..} => {
                    let original=env.clone();let mut branch=env.clone();let mut local=bound.clone();
                    if handler {local.insert(name.clone());}
                    record(name,&Expr::Int(0),&local,&mut branch,globals,observed,captures, library)?;
                    ordered_walk(body,&mut local,&mut branch,globals,observed,captures,handler, library)?;
                    merge_env(env,&branch,&original);
                }
                Statement::Choice {options} => {
                    let original=env.clone();let entry=bound.clone();let mut merged:Option<BTreeMap<String,VariableDefinition>>=None;let mut definite:Option<BTreeSet<String>>=None;
                    for option in options {
                        let mut branch=original.clone();let mut local=entry.clone();
                        ordered_walk(&option.body,&mut local,&mut branch,globals,observed,captures,handler, library)?;
                        if let Some(first)=&mut merged {merge_env(first,&branch,&original);} else {merged=Some(branch);}
                        definite=Some(match definite {None=>local,Some(first)=>first.intersection(&local).cloned().collect()});
                    }
                    if let Some(mut merged)=merged {
                        if options.iter().any(|option|option.condition.is_some()) {merge_env(&mut merged,&original,&original);} else if let Some(local)=definite {*bound=local;}
                        *env=merged;
                    }
                }
                Statement::Imagemap {hotspots,..} => {
                    let original=env.clone();
                    for hotspot in hotspots {let mut branch=original.clone();let mut local=bound.clone();ordered_walk(&hotspot.body,&mut local,&mut branch,globals,observed,captures,handler, library)?;merge_env(env,&branch,&original);}
                }
                // Nested definitions have their own activation. The caller's
                // binding state does not enter those definitions.
                _=>{},
            }
        }
        Ok(())
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
        // Unit is always a fresh lexical activation, including its ordinary
        // (non-Native) source form. Local introductions must not become global
        // merely because cold import uses blocks rather than resolved input.
        let fresh_activation = matches!(kind, GraphKind::UnitFunction {..})
            || crate::native_source_binding::contains_native(body) && match kind {
            GraphKind::Init | GraphKind::Handler {..}=>true,
            GraphKind::Label {name}=>scene_entries.contains(name),
            _=>false,
        };
        let mut locals = parameters.iter().cloned().collect();
        if !fresh_activation && (matches!(kind, GraphKind::Handler { .. }) || (matches!(kind, GraphKind::Label { .. })
            && context == rvn_parser::NarrativeContext::ExplicitScenes)) {
            shadowed(body, &mut locals, matches!(kind, GraphKind::Handler { .. }));
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
        if fresh_activation {
            ordered_walk(body, &mut locals, &mut env, variables, &mut observed, &ScheduledBindingTypes::default(), matches!(kind,GraphKind::Handler{..}|GraphKind::UnitFunction{..}), library)?;
        } else { walk(body, &locals, &mut env, variables, &mut observed, &ScheduledBindingTypes::default(), library)?; }
    }
    let mut conflicts = BTreeMap::new();
    for (name, types) in observed {
        if types.len() > 1 {
            variables.get_mut(&name).unwrap().value_type = ValueType::Any;
            conflicts.insert(name, types.iter().map(|ty| format!("{ty:?}")).collect());
        }
    }
    Ok(conflicts)
}
pub(crate) fn expression_source(expr: &Expr) -> String {
    match expr {
        Expr::LibraryCall{call}=>format!("functions.value_v1({:?}, {:?}, {{{}}})",call.reference().library.as_str(),call.reference().graph_id.to_string(),call.arguments().iter().map(|(id,value)|format!("{:?}:{}",id.as_str(),expression_source(value))).collect::<Vec<_>>().join(",")),
        Expr::StandardMacroData{site,output,..}=>{
            let method=match output {rvn_parser::scheduled_source::standard_macro_contract::DataOutput::Counter=>"flow.macro_counter_v1",
                rvn_parser::scheduled_source::standard_macro_contract::DataOutput::IsA=>"flow.macro_is_a_v1"};
            format!("{method}({}, {})",serde_json::to_string(&site.scope_source()).unwrap(),serde_json::to_string(&site.node_source()).unwrap())
        },
        Expr::Int(v) => v.to_string(),
        Expr::Float(v) => format!("{v:?}"),
        Expr::Bool(v) => v.to_string(),
        Expr::Str(v) => serde_json::to_string(v).unwrap(),
        Expr::Var(v) => rvn_parser::variable_source_name(v).unwrap_or_else(|| v.clone()),
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
pub(crate) fn text_source(text: &rvn_parser::InterpolatedText) -> String {
    text.0
        .iter()
        .map(|s| match s {
            rvn_parser::TextSegment::Lit(v) => v.replace('[', "[[").replace(']', "]]"),
            rvn_parser::TextSegment::Interp(v) => format!("[{}]", expression_source(v)),
        })
        .collect()
}
fn literal(value: &Expr) -> Option<PropertyValue> {
    if let Ok(Some(value)) = crate::enum_graph1026::literal_expression1026(value) { return Some(PropertyValue::Enum(value)); }
    Some(match value {
        Expr::Int(v) => PropertyValue::Int(*v),
        Expr::Float(v) => PropertyValue::Float(*v as f64),
        Expr::Neg(value) => match value.as_ref(){
            Expr::Int(value)=>PropertyValue::Int(value.checked_neg()?),
            Expr::Float(value)=>PropertyValue::Float(f64::from(-*value)),
            _=>return None,
        },
        Expr::Bool(v) => PropertyValue::Bool(*v),
        Expr::Str(v) => PropertyValue::String(v.clone()),
        Expr::Call { name, args } if name == "__rvn_literal_name" => {
            let [Expr::Str(value)] = args.as_slice() else { return None; };
            PropertyValue::Name(crate::NameDefault::new(value).ok()?)
        },
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
#[derive(Clone, Copy)]
struct ExpressionRef { node: NodeId, output: PinId }
struct Builder<'a> {
    library1027:Option<&'a crate::LibraryGraphResources1027<'a>>,
    graph: GraphDocument,
    unit_declarations:std::sync::Arc<BTreeMap<String,GraphDocument>>,
    lane: f64,
    range_owners: Vec<NodeId>,
    narrative_globals: Option<BTreeMap<String, ValueType>>,
    narrative_locals: std::collections::BTreeSet<String>,
    binding_states: BTreeMap<String, u8>,
    inspectors: BTreeMap<String, NodeId>,
    native_producers:BTreeMap<String,(Expr,bool,ExpressionRef)>,
    native_active:std::collections::BTreeSet<String>,
    native_nodes:BTreeMap<NodeId,crate::native_frame_preparation::NativeNodeIdentity>,
    native_assignment_pending:std::collections::BTreeSet<NodeId>,
    native_scope:Option<String>,
    native_constant_consumer:Option<crate::native_frame_preparation::NativeNodeIdentity>,
    native_loop_pending:Option<NativeLoopDeclaration>,
    native_loop_declarations:BTreeMap<NodeId,NativeLoopDeclaration>,
    native_loop_bodies:Vec<NativeLoopBody>,
    macro_definitions:BTreeMap<NodeId,standard_macros_1020_import::MacroDefinition>,
}

impl GraphDocument {
    /// Replace a real assignment with a source-owned factory transactionally.
    /// Only existing node/pin/edge allocators produce identities.
    pub fn replace_assignment_with_widget_factory(&mut self,assignment:NodeId,class:&Expr,arguments:&Expr)->Result<NodeId,String>{
        let changes=[(assignment,class.clone(),arguments.clone())];
        Ok(self.replace_assignments_with_widget_factories(&changes)?.remove(0))
    }
    /// Validate the coherent typed batch before a single document commit.
    pub fn replace_assignments_with_widget_factories(&mut self,changes:&[(NodeId,Expr,Expr)])->Result<Vec<NodeId>,String>{
        if changes.len()>65_536{return Err("Widget factory edit batch budget".into());}
        let mut next=self.clone();let mut created=Vec::new();let mut seen=std::collections::BTreeSet::new();
        for(assignment,class,arguments)in changes{if !seen.insert(*assignment){return Err("Repeated Widget factory assignment in one edit batch".into());}created.push(next.replace_assignment_with_widget_factory_in(*assignment,class,arguments)?);}
        next.refresh_variable_usage_bindings()?;
        if let Some(error)=next.validate().iter().find(|d|d.severity==DiagnosticSeverity::Error){return Err(error.message.clone());}
        *self=next;Ok(created)
    }
    fn replace_assignment_with_widget_factory_in(&mut self,assignment:NodeId,class:&Expr,arguments:&Expr)->Result<NodeId,String>{
        let old=self.nodes.get(&assignment).ok_or("Missing actual assignment")?.clone();
        if !matches!(old.kind,NodeKind::SetVariable|NodeKind::LocalVariable){return Err("Widget factory replacement requires an actual assignment".into());}
        self.widget_factory_authoring_context()?;
        let Some(PropertyValue::String(name))=old.properties.get("name") else{return Err("Actual assignment name missing".into());};
        let mut builder = Builder { graph: self.clone(), library1027:None, unit_declarations:Default::default(), lane: 0.0, range_owners:Vec::new(),
            narrative_globals:None, narrative_locals:Default::default(), binding_states:Default::default(), inspectors:Default::default(), native_producers:Default::default(),native_active:Default::default(),native_nodes:Default::default(),native_assignment_pending:Default::default(),native_scope:None,native_constant_consumer:None,native_loop_pending:None,native_loop_declarations:Default::default(),native_loop_bodies:vec![],macro_definitions:Default::default() };
        let factory=builder.node(NodeKind::WidgetCreate,old.position)?;
        builder.property(factory,"name",PropertyValue::String(name.clone()))?;
        builder.property(factory,"local",PropertyValue::Bool(old.kind==NodeKind::LocalVariable))?;
        builder.input_expr(factory,"class",class)?;
        builder.input_expr(factory,"arguments",arguments)?;
        let mut connections=Vec::new();let mut old_literals=Vec::new();
        for edge in self.edges.values(){
            let a=&self.pins[&edge.output];let z=&self.pins[&edge.input];
            if z.node==assignment&&z.key=="value"&&self.nodes[&a.node].kind==NodeKind::Literal&&self.edges.values().filter(|e|self.pins[&e.output].node==a.node).count()==1{old_literals.push(a.node);}
            if a.node==assignment&&matches!(a.key.as_str(),"exec_out"|"value_out"){connections.push((builder.graph.pin_by_key(factory,&a.key).ok_or("Factory output missing")?.id,z.id));}
            if z.node==assignment&&z.key=="exec_in"{connections.push((a.id,builder.graph.pin_by_key(factory,"exec_in").ok_or("Factory input missing")?.id));}
        }
        builder.graph.remove_node(assignment);
        for node in old_literals{builder.graph.remove_node(node);}
        for(a,z)in connections{builder.graph.connect(a,z).map_err(err)?;}
        if let Some(variable)=builder.graph.variables.get_mut(name){variable.value_type=ValueType::WidgetObject;}
        let reads:Vec<_>=builder.graph.nodes.values().filter(|n|n.kind==NodeKind::VariableGet&&n.properties.get("name")==Some(&PropertyValue::String(name.clone()))).flat_map(|n|n.pins.clone()).collect();
        for pin in reads{if let Some(pin)=builder.graph.pins.get_mut(&pin){if !pin.value_type.is_execution(){pin.value_type=ValueType::WidgetObject;}}}
        *self=builder.graph;Ok(factory)
    }
    /// Convert old per-answer RVN guards to ordinary Boolean producer wires.
    /// The update is additive: existing node/pin/edge identities and positions
    /// stay intact. Older truthy non-Boolean RVN guards are retained verbatim
    /// rather than silently coercing them or changing their source AST.
    pub fn materialize_choice_conditions(&mut self) -> Result<usize, GraphEditError> {
        let guards: Vec<_> = self.nodes.values().filter(|node| node.kind == NodeKind::Choice)
            .flat_map(|node| node.properties.iter().filter_map(|(key, value)| {
                let PropertyValue::String(source) = value else { return None; };
                if !key.starts_with("option_") || !key.ends_with("_condition")
                    || source.trim().is_empty() { return None; }
                let pin = self.pin_by_key(node.id, key)?;
                if self.edges.values().any(|edge| edge.input == pin.id) { return None; }
                Some((node.id, key.clone(), source.clone()))
            })).collect();
        if guards.is_empty() { return Ok(0); }
        let mut builder = Builder { graph: self.clone(), library1027:None, unit_declarations:Default::default(), lane: 0.0, range_owners:Vec::new(),
            narrative_globals:None, narrative_locals:Default::default(), binding_states:Default::default(), inspectors:Default::default(), native_producers:Default::default(),native_active:Default::default(),native_nodes:Default::default(),native_assignment_pending:Default::default(),native_scope:None,native_constant_consumer:None,native_loop_pending:None,native_loop_declarations:Default::default(),native_loop_bodies:vec![],macro_definitions:Default::default() };
        let mut created = 0;
        for (node, key, source) in guards {
            let parsed = rvn_parser::parse(&format!("set __choice_guard = {source}\n"))
                .map_err(|_| GraphEditError::InvalidPropertyType { node, key: key.clone() })?;
            let [Statement::SetVar { value, .. }] = parsed.as_slice() else {
                return Err(GraphEditError::InvalidPropertyType { node, key });
            };
            let before = builder.graph.clone();
            let before_inspectors = builder.inspectors.clone();
            if builder.choice_condition(node, &key, value).is_ok() {
                builder.graph.set_property(node, key, PropertyValue::String(String::new()))?;
                created += 1;
            } else {
                // Valid old RVN may use truthy numbers/strings. A new Boolean
                // socket must not pretend those expressions are Boolean.
                builder.graph = before;
                builder.inspectors = before_inspectors;
            }
        }
        *self = builder.graph;
        Ok(created)
    }
}

impl Builder<'_> {
    /// Allocate only real assignment outputs referenced by data captures.
    /// Their actual RHS and execution edges are populated in the normal source
    /// pass; this never creates a GET carrying a SET identity.
    fn predeclare_assignment_outputs(&mut self,body:&[Statement])->Result<(),String>{
        let mut pending=vec![body];let mut plans=Vec::new();let mut wanted=BTreeMap::new();let mut work=0usize;
        while let Some(block)=pending.pop(){for statement in block{
            work=work.checked_add(1).ok_or("Assignment import count overflow")?;if work>100_000{return Err("Assignment import budget".into());}
            if let Statement::ScheduledConsumer{scope,steps,..}=statement {
                if scope.starts_with('g')&&scope.contains("/n") {
                    plans.push(statement);
                    for step in steps{if let rvn_parser::ConsumerStep::Capture{producer,value}=step {
                        let (identity,terminal)=crate::native_frame_preparation::decode_producer(producer)?;
                        if terminal.is_none()&&matches!(identity.kind,NodeKind::SetVariable|NodeKind::LocalVariable|NodeKind::WidgetCreate) {
                            let Expr::Var(name)=value else{return Err("Assignment capture must read its current variable".into());};
                            if wanted.get(&identity.node).is_some_and(|(old,old_name)|old!=&identity||old_name!=name){return Err("Assignment output source identity/name conflict".into());}
                            wanted.insert(identity.node,(identity,name.clone()));
                        }
                    }}
                }
            }
            match statement{
                Statement::ScheduledConsumer{body,..}|Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>pending.push(body),
                Statement::If{then_branch,else_branch,..}=>{pending.push(then_branch);pending.push(else_branch);},
                Statement::Sequence{branches}=>pending.extend(branches.iter().map(Vec::as_slice)),
                Statement::StandardMacroDispatch{dispatch}=>pending.extend(dispatch.branches.iter().map(|branch|branch.body.as_slice())),
                Statement::Choice{options}=>pending.extend(options.iter().map(|o|o.body.as_slice())),
                Statement::Imagemap{hotspots,..}=>pending.extend(hotspots.iter().map(|h|h.body.as_slice())),_=>{},
            }
        }}
        for (_, (identity,name)) in wanted {
            let declarations=plans.iter().filter(|plan|match plan {
                Statement::ScheduledConsumer{consumer,body,..}=>crate::native_frame_preparation::decode_node(consumer).is_ok_and(|actual|actual==identity)
                    && matches!(body.as_slice(),[Statement::SetVar{name:actual,..}|Statement::LocalVar{name:actual,..}]if actual==&name),_=>false,
            }).count();
            if declarations!=1{return Err("Assignment data output requires its one actual source execution declaration".into());}
            let node=self.node(identity.kind,[0.0,0.0])?;
            self.property(node,"name",PropertyValue::String(name))?;
            self.native_nodes.insert(node,identity);self.native_assignment_pending.insert(node);
        }
        Ok(())
    }
    fn merge_incoming_bindings(&mut self, incoming: &BTreeMap<String,u8>) {
        for name in &self.narrative_locals {
            let before=incoming.get(name).copied().unwrap_or(1);
            *self.binding_states.entry(name.clone()).or_insert(1) |= before;
        }
    }

    fn accumulate_branch_bindings(&self, merged: &mut BTreeMap<String,u8>) {
        for name in &self.narrative_locals {
            *merged.entry(name.clone()).or_default() |= self.binding_states.get(name).copied().unwrap_or(1);
        }
    }

    fn node(&mut self, kind: NodeKind, pos: [f64; 2]) -> Result<NodeId, String> {
        if matches!(kind,NodeKind::SetVariable|NodeKind::LocalVariable|NodeKind::WidgetCreate) {
            if let Some(identity)=self.native_constant_consumer.as_ref().filter(|i|i.kind==kind) {
                if let Some((node,prior))=self.native_nodes.iter().find(|(_,i)|i.node==identity.node) {
                    let node=*node;
                    if prior!=identity||!self.native_assignment_pending.remove(&node){return Err("Assignment source cannot declare its execution node twice".into());}
                    self.graph.nodes.get_mut(&node).unwrap().position=pos;return Ok(node);
                }
            }
        }
        let schema=self.graph.schema_version;let contract=self.graph.inspector_evaluation;
        let node=self.graph.add_catalog_node(kind,pos).map_err(err)?;
        if self.native_loop_pending.is_some(){self.install_native_loop_declaration(node)?;}
        if kind==NodeKind::GetStringTableIdAndKey {
            self.graph.nodes.get_mut(&node).unwrap().inspector_contract=Some(crate::InspectorEvaluationContract::LegacyStateless);
            self.graph.schema_version=schema.max(crate::GRAPH_NAME_TEXT_SCHEMA_VERSION);self.graph.inspector_evaluation=contract;
        }
        Ok(node)
    }
    fn property(&mut self, n: NodeId, key: &str, value: PropertyValue) -> Result<(), String> {
        self.graph.set_property(n, key, value).map_err(err)
    }
    fn native_constant_value(&self,value:&Expr,kind:NodeKind,active:&mut std::collections::BTreeSet<String>)->Option<PropertyValue> {
        if kind==NodeKind::ConvertBoolToFloat {
            if let Expr::BinOp{op:BinOpKind::Add,left,right}=value {
                if !matches!(right.as_ref(),Expr::Float(zero)if zero.to_bits()==0.0f32.to_bits()){return None;}
                let Expr::Call{name,args}=left.as_ref() else{return None;};
                let [operand]=args.as_slice() else{return None;};
                if name!="__rvn_bool_to_int"{return None;}
                let PropertyValue::Bool(value)=self.native_constant_value(operand,NodeKind::Literal,active)? else{return None;};
                return Some(PropertyValue::Float(if value{1.0}else{0.0}));
            }
        }
        if kind==NodeKind::ConvertIntToFloat {
            if let Expr::BinOp{op:BinOpKind::Add,left,right}=value {
                if !matches!(right.as_ref(),Expr::Float(zero)if zero.to_bits()==0.0f32.to_bits()){return None;}
                let PropertyValue::Int(integer)=self.native_constant_value(left,NodeKind::Literal,active)? else{return None;};
                return Some(PropertyValue::Float(f64::from(integer as f32)));
            }
        }
        if let Some(value)=literal(value) {return Some(value);}
        let Expr::Call{name,args}=value else {return None;};
        if name!="__rvn_captured_v1" {return None;}
        let [Expr::Str(id)]=args.as_slice() else {return None;};
        if !self.native_active.contains(id)||active.len()>=128||!active.insert(id.clone()) {return None;}
        let (value,inspection,source)=self.native_producers.get(id)?;
        if *inspection {return None;}
        let result=self.native_constant_value(value,self.graph.nodes.get(&source.node)?.kind,active);active.remove(id);result
    }
    fn default(&mut self, n: NodeId, key: &str, value: PropertyValue) -> Result<(), String> {
        if let Some(identity)=&self.native_constant_consumer {
            if identity.kind==self.graph.nodes[&n].kind {
                if let Some(output)=identity.static_inputs.get(key).filter(|_|crate::native_frame_preparation::is_static_audio_input(identity.kind,key)) {
                    let matches:Vec<_>=self.native_producers.iter().filter(|(id,(_,inspection,source))|
                        !inspection && self.native_active.contains(*id)
                        && self.native_nodes.get(&source.node).is_some_and(|identity|
                            identity.ports.iter().any(|(key,pin,_)|pin==output && self.graph.pin_by_key(source.node,key).is_some_and(|pin|pin.id==source.output))))
                        .map(|(_, (expr,_,source))|(expr.clone(),*source)).collect();
                    let [(expr,source)]=matches.as_slice() else {return Err("Static audio input requires one active actual source output".into());};
                    if self.native_constant_value(expr,self.graph.nodes[&source.node].kind,&mut Default::default())!=Some(value.clone()) {
                        return Err("Static audio captured constant differs from actual command operand".into());
                    }
                    let source=*source;self.connect_expression(source,n,key)?;
                    let input=self.graph.pin_by_key(n,key).unwrap().id;self.graph.pins.get_mut(&input).unwrap().default_value=None;
                    return Ok(());
                }
            }
        }
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
        // A visual Bool -> Int connection may request a visible cast. Import
        // must preserve authored expressions and never introduce that coercion.
        if source == ValueType::Bool && target == ValueType::Int {
            return Err("Boolean input requires an explicit __rvn_bool_to_int(...) conversion".into());
        }
        if source == ValueType::Int && target == ValueType::Bool {
            return Err("Integer input requires an explicit __rvn_int_to_bool(...) conversion".into());
        }
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
        if self.graph.nodes[&node].kind!=NodeKind::LocalVariable && self.narrative_locals.contains(name) {
            if let Some(globals)=&self.narrative_globals {
                let global=globals.get(name).cloned().unwrap_or(ValueType::Any);
                let state=self.binding_states.get(name).copied().unwrap_or(1);
                let scoped=match state {
                    1=>Some(global),
                    3=>Some(if global==ty {ty.clone()} else {ValueType::Any}),
                    _=>None,
                };
                if let Some(scoped)=scoped {
                    self.graph.cache_source_binding_types(node,name,
                        [("value".into(),scoped.clone()),("value_out".into(),scoped)].into())?;
                }
            }
        }
        for key in ["value", "value_out"] {
            if let Some(id) = self.graph.pin_by_key(node, key).map(|p| p.id) {
                self.graph.pins.get_mut(&id).unwrap().value_type = ty.clone();
            }
        }
        Ok(())
    }
    fn value_output(&self, node: NodeId) -> Result<ExpressionRef, String> {
        let output = self.graph.nodes[&node].pins.iter().copied()
            .find(|pin| self.graph.pins[pin].direction == PinDirection::Output && !self.graph.pins[pin].value_type.is_execution())
            .ok_or_else(|| format!("Expression without output: {node}"))?;
        Ok(ExpressionRef { node, output })
    }
    fn expression(&mut self, expr: &Expr, pos: [f64; 2]) -> Result<ExpressionRef, String> {
        if let Expr::LibraryCall{call}=expr {let node=self.node(NodeKind::FunctionCall,pos)?;self.import_qualified_call1027(node,call,rvn_parser::function_library_program1023::LibraryCallMode::Value)?;return self.value_output(node);}
        if let Expr::Var(name)=expr {
            if matches!(self.graph.kind,GraphKind::UnitFunction{..}) {
                if let Some(pin)=crate::unit_graph_source1003::parameters(&self.graph)?.into_iter().find(|pin|&pin.label==name) {
                    return Ok(ExpressionRef{node:pin.node,output:pin.id});
                }
            }
        }
        if let Expr::StandardMacroData{site,output,pin}=expr{return self.macro_data(site,*output,*pin,pos);}
        if let Some(source)=self.native_loop_index_source(expr){return source;}
        if let Expr::Call{name,args}=expr {
            if matches!(name.as_str(),"__rvn_captured_v1"|"__rvn_terminal_v1") {
                let Some(Expr::Str(id))=args.first() else {return Err("Native reference requires literal source identity".into());};
                if !self.native_active.contains(id) {return Err("Native reference is forward, cross-consumer, or outside a source plan".into());}
                let (_,inspection,source)=self.native_producers.get(id).ok_or("Missing native source producer")?;
                return match (name.as_str(),*inspection,args.as_slice()) {
                    ("__rvn_captured_v1",false,[Expr::Str(_)])=>Ok(*source),
                    ("__rvn_terminal_v1",true,[Expr::Str(_),Expr::Str(key)]) if matches!(key.as_str(),"valid"|"table_id"|"key")=>{
                        let output=self.graph.pin_by_key(source.node,key).ok_or("Missing typed native output role")?.id;
                        Ok(ExpressionRef{node:source.node,output})
                    },
                    _=>Err("Native reference kind/arity/typed port mismatch".into()),
                };
            }
        }
        if let Expr::Call { name, args } = expr {
            if name=="__rvn_widget_object_class_v1" {
                let [object]=args.as_slice() else{return Err("Get Class expects exactly one typed Widget object argument".into());};
                let node=self.node(NodeKind::WidgetObjectClass,pos)?;
                self.input_expr(node,"object",object)?;
                return self.value_output(node);
            }
            if matches!(name.as_str(),"__rvn_widget_class_v1"|"__rvn_widget_class_none_v1") {
                let uuid=match(name.as_str(),args.as_slice()) {
                    ("__rvn_widget_class_v1",[Expr::Str(uuid)])=>{rvn_parser::localized_text::validate_text_asset_id(uuid)?;uuid.clone()},
                    ("__rvn_widget_class_none_v1",[])=>String::new(),
                    _=>return Err("Widget Class requires a literal UUID or explicit empty class".into()),
                };
                let node=self.node(NodeKind::WidgetClassValue,pos)?;
                self.property(node,"uuid",PropertyValue::String(uuid))?;
                return self.value_output(node);
            }
            if let Some(key) = crate::text_table_nodes::inspector_output(name) {
                let [argument] = args.as_slice() else { return Err(format!("{name} expects one argument")); };
                if !crate::text_table_nodes::reproducible_source(argument) {
                    return Err("Capture an effectful Text expression in SET/Local before using the shared inspector".into());
                }
                let bindings: BTreeMap<_,_> = crate::text_table_nodes::source_binding_names(argument).into_iter()
                    .map(|name| { let state=self.binding_states.get(&name).copied().unwrap_or(1); (name,state) }).collect();
                let identity = format!("{}|{:?}|{:?}", serde_json::to_string(argument).map_err(|error|error.to_string())?, bindings, self.range_owners);
                let node = if let Some(node) = self.inspectors.get(&identity).copied() { node } else {
                    let node = self.node(NodeKind::GetStringTableIdAndKey, pos)?;
                    self.strict_input(node, "text", argument, ValueType::InterpolatedText)?;
                    self.inspectors.insert(identity, node); node
                };
                let output = self.graph.pin_by_key(node, key).unwrap().id;
                return Ok(ExpressionRef { node, output });
            }
        }
        let node = self.expression_node(expr, pos)?; self.value_output(node)
    }
    fn strict_input(&mut self, node: NodeId, key: &str, argument: &Expr, expected: ValueType) -> Result<(), String> {
        let pos = self.graph.nodes[&node].position;
        let source = self.expression(argument, [pos[0]-260.0, pos[1]+160.0])?;
        let actual = self.graph.effective_pin_type(source.output).unwrap_or(ValueType::Any);
        if actual != expected && actual != ValueType::Any {
            return Err(format!("Text boundary input {key} expects {expected:?}, not {actual:?}; use an explicit conversion"));
        }
        self.connect_expression(source,node,key)
    }
    fn expression_node(&mut self, expr: &Expr, pos: [f64; 2]) -> Result<NodeId, String> {
        // Canonical legacy spelling for one BoolToFloat auto-cast. Native
        // producers are reconstructed by their declared kind before this path,
        // preserving an authored pair of conversions as two distinct nodes.
        if let Expr::BinOp{op:BinOpKind::Add,left,right}=expr {
            if matches!(right.as_ref(),Expr::Float(zero)if zero.to_bits()==0.0f32.to_bits()) {
                if let Expr::Call{name,args}=left.as_ref() {
                    if name=="__rvn_bool_to_int" {
                        if let [operand]=args.as_slice() {
                            if literal(operand).is_some_and(|value|!matches!(value,PropertyValue::Bool(_))){return Err("BoolToFloat expects its actual Bool operand".into());}
                            let node=self.node(NodeKind::ConvertBoolToFloat,pos)?;
                            self.input_expr(node,"value",operand)?;return Ok(node);
                        }
                    }
                }
            }
        }

        if let Some(owner) = self.graph.text_owner.clone() {
            if self.graph.schema_version >= crate::GRAPH_TABLE_TEXT_SCHEMA_VERSION {
                if let Some(reference) = crate::text_pin::table_text_expression(expr)? {
                    let node = self.node(NodeKind::TextValue, pos)?;
                    let pin = self.graph.pin_by_key(node, "value").unwrap().id;
                    self.graph.set_pin_text_table_reference(pin, reference, &[])?;
                    return Ok(node);
                }
            }
            if let Some(text) = crate::text_pin::authored_text_expression(expr, &owner)? {
                let node = self.node(NodeKind::TextValue, pos)?;
                let pin = self.graph.pin_by_key(node, "value").unwrap().id;
                self.graph.set_pin_text(pin, text)?;
                if matches!(expr, Expr::Call {name, args} if name == "__rvn_localizable_text" && args.len() == 4) {
                    self.property(node, "@text_explicit_notes", PropertyValue::Bool(false))?;
                }
                return Ok(node);
            }
            if let Expr::Call {name, args} = expr {
                if name == "__rvn_format_text" && args.len() == 2 {
                    if let Expr::Call {name: dictionary, args: entries} = &args[1] {
                        let keys: Option<Vec<_>> = entries.chunks(2).map(|entry| match entry {
                            [Expr::Str(key), _] => Some(key.clone()), _ => None,
                        }).collect();
                        if let Some(keys) = keys.filter(|keys| dictionary == "dict"
                            && keys.iter().collect::<std::collections::BTreeSet<_>>().len() == keys.len()) {
                            let node = self.node(NodeKind::FormatText, pos)?;
                            let reference = if self.graph.schema_version >= crate::GRAPH_TABLE_TEXT_SCHEMA_VERSION {
                                crate::text_pin::table_text_expression(&args[0])?
                            } else { None };
                            if let Some(reference) = reference {
                                let pin = self.graph.pin_by_key(node, "format").unwrap().id;
                                self.graph.set_pin_text_table_reference(pin, reference, &keys)?;
                            } else if let Some(text) = crate::text_pin::authored_text_expression(&args[0], &owner)? {
                                let pin = self.graph.pin_by_key(node, "format").unwrap().id;
                                self.graph.set_pin_text(pin, text)?;
                                if matches!(&args[0], Expr::Call {name, args} if name == "__rvn_localizable_text" && args.len() == 4) {
                                    self.property(node, "@text_explicit_notes", PropertyValue::Bool(false))?;
                                }
                            } else {
                                self.property(node, "deferred_text", PropertyValue::Bool(true))?;
                                self.input_expr(node, "format", &args[0])?;
                            }
                            // Reproduce the captured dictionary exactly, including
                            // omitted or extra arguments. Pattern edits synchronize
                            // the pins later in an explicit authoring transaction.
                            let removed: std::collections::BTreeSet<_> = self.graph.nodes[&node].pins.iter().copied()
                                .filter(|p| self.graph.pins[p].key.starts_with("argument_")
                                    && !keys.contains(&self.graph.pins[p].key["argument_".len()..].to_string())).collect();
                            self.graph.nodes.get_mut(&node).unwrap().pins.retain(|p| !removed.contains(p));
                            self.graph.pins.retain(|p, _| !removed.contains(p));
                            for (index, key) in keys.iter().enumerate() {
                                let pin_key = format!("argument_{key}");
                                if self.graph.pin_by_key(node, &pin_key).is_none() {
                                    self.graph.add_pin(node, &pin_key, key, PinDirection::Input, ValueType::Any, PinCardinality::One).map_err(err)?;
                                }
                                self.input_expr(node, &pin_key, &entries[index*2+1])?;
                            }
                            return Ok(node);
                        }
                    }
                }
            }
        }
        // Collections and calls must remain editable nodes, not opaque text.
        if let Expr::Call { name, args } = expr {
            if name == "__rvn_string_table_asset" {
                let [argument] = args.as_slice() else { return Err(format!("{name} expects one argument")); };
                if let Expr::Str(id) = argument {
                    if !id.is_empty() { rvn_parser::localized_text::validate_string_table_id(id)?; }
                    let node = self.node(NodeKind::StringTableAsset, pos)?;
                    self.property(node, "table", PropertyValue::String(id.clone()))?;
                    return Ok(node);
                }
                let source = self.expression(argument, [pos[0] - 260.0, pos[1] + 160.0])?;
                let output = source.output;
                if !matches!(self.graph.effective_pin_type(output).unwrap_or(ValueType::Any), ValueType::String | ValueType::Any) {
                    return Err(format!("{name} expects a string asset identifier"));
                }
                let node = self.node(NodeKind::FunctionCall, pos)?;
                self.property(node, "function", PropertyValue::String(name.clone()))?;
                self.graph.resize_value_inputs(node, 1).map_err(err)?;
                self.connect_expression(source, node, "item_0")?;
                return Ok(node);
            }
            if matches!(name.as_str(), "__rvn_string_table_id" | "__rvn_string_table_id_name") {
                let [argument] = args.as_slice() else { return Err(format!("{name} expects one argument")); };
                let contract = if name.ends_with("_name") { crate::StringTableQueryContract::NameV2 } else { crate::StringTableQueryContract::LegacyStringV1 };
                let node = self.graph.add_table_query_node(NodeKind::StringTableId, pos, contract).map_err(err)?;
                // An empty object field is the explicit null reference.
                if matches!(argument, Expr::Str(id) if id.is_empty()) { return Ok(node); }
                let source = self.expression(argument, [pos[0] - 260.0, pos[1] + 160.0])?;
                let output = source.output;
                let value_type = self.graph.effective_pin_type(output).unwrap_or(ValueType::Any);
                if !matches!(value_type, ValueType::Asset(crate::AssetKind::StringTable) | ValueType::Any) {
                    return Err(format!("{name} expects a String Table asset reference, not {value_type:?}"));
                }
                self.connect_expression(source, node, "asset")?;
                return Ok(node);
            }
            if let Some((kind, keys, contract)) = crate::ALL_NODE_KINDS.iter().copied().find_map(|kind| {
                [crate::StringTableQueryContract::LegacyStringV1, crate::StringTableQueryContract::NameV2].into_iter().find_map(|contract| {
                    kind.string_table_query_with_contract(contract).and_then(|(function, keys, _)| (function == name).then_some((kind, keys, contract)))
                })
            }) {
                if keys.len() != args.len() { return Err(format!("{name} expects {} arguments", keys.len())); }
                let node = self.graph.add_table_query_node(kind, pos, contract).map_err(err)?;
                for (key, argument) in keys.iter().zip(args) {
                    let expected = contract.input_type(key);
                    if expected == ValueType::String && matches!(argument, Expr::Str(_)) {
                        self.input_expr(node, key, argument)?;
                        continue;
                    }
                    let source = self.expression(argument, [pos[0] - 260.0, pos[1] + 160.0])?;
                    let output = source.output;
                    let value_type = self.graph.effective_pin_type(output).unwrap_or(ValueType::Any);
                    if value_type != expected && value_type != ValueType::Any {
                        return Err(format!("{name} input {key} expects {expected:?}, not {value_type:?}; use an explicit conversion"));
                    }
                    self.connect_expression(source, node, key)?;
                }
                return Ok(node);
            }
            if name == "__rvn_string_table_text_name" {
                let [table,key] = args.as_slice() else { return Err(format!("{name} expects two arguments")); };
                let node = self.node(NodeKind::TextFromStringTable, pos)?;
                // Retain literal Name as an editable authored default, including blank/None spellings.
                if let Expr::Call {name: literal, args} = table {
                    if literal == "__rvn_literal_name" {
                        let [Expr::Str(value)] = args.as_slice() else { return Err("Literal Name expects one string".into()); };
                        self.default(node,"table",PropertyValue::Name(NameDefault::new(value).map_err(|error|error.to_string())?))?;
                    } else { self.strict_input(node,"table",table,ValueType::Name)?; }
                } else { self.strict_input(node,"table",table,ValueType::Name)?; }
                if let Expr::Str(value) = key { self.text(node,"key",value)?; }
                else { self.strict_input(node,"key",key,ValueType::String)?; }
                return Ok(node);
            }
            if name == crate::ENUM_VALUE_CONSTRUCTOR1026 {
                let value = crate::enum_graph1026::literal_expression1026(expr)?.ok_or("Enum literal constructor recognition failed")?;
                let node = self.node(NodeKind::Literal, pos)?;
                self.property(node, "value", PropertyValue::Enum(value))?;
                return Ok(node);
            }
            if name == "__rvn_literal_name" {
                let [Expr::Str(value)] = args.as_slice() else {
                    return Err("A literal Name requires one authored string".into());
                };
                let value = crate::NameDefault::new(value).map_err(|error| error.to_string())?;
                let node = self.node(NodeKind::Literal, pos)?;
                self.property(node, "value", PropertyValue::Name(value))?;
                return Ok(node);
            }
            if args.len() == 1 && matches!(name.as_str(), "__rvn_string_to_name" | "__rvn_name_to_string") {
                let (kind, expected) = if name == "__rvn_string_to_name" {
                    (NodeKind::ConvertStringToName, ValueType::String)
                } else {
                    (NodeKind::ConvertNameToString, ValueType::Name)
                };
                let node = self.node(kind, pos)?;
                let argument = self.expression(&args[0], [pos[0] - 260.0, pos[1] + 160.0])?;
                let output = argument.output;
                let actual = self.graph.effective_pin_type(output).unwrap_or(ValueType::Any);
                if actual != expected && actual != ValueType::Any {
                    return Err(format!("{name} expects {expected:?}, not {actual:?}"));
                }
                self.connect_expression(argument, node, "value")?;
                return Ok(node);
            }
            if args.len() == 1 && matches!(name.as_str(), "__rvn_bool_to_int" | "__rvn_int_to_bool") {
                let (kind, expected) = if name == "__rvn_bool_to_int" {
                    (NodeKind::ConvertBoolToInt, ValueType::Bool)
                } else {
                    (NodeKind::ConvertIntToBool, ValueType::Int)
                };
                let node = self.node(kind, pos)?;
                let argument = self.expression(&args[0], [pos[0] - 260.0, pos[1] + 160.0])?;
                let output = argument.output;
                let argument_type = self.graph.effective_pin_type(output).unwrap_or(ValueType::Any);
                if argument_type != expected && argument_type != ValueType::Any {
                    return Err(format!("{name} expects {expected:?}, not {argument_type:?}"));
                }
                self.connect_expression(argument, node, "value")?;
                return Ok(node);
            }
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
                    self.value_output(text)?
                } else {
                    self.expression(&args[0], [pos[0] - 260.0, pos[1] + 160.0])?
                };
                let argument_type = self.graph.effective_pin_type(argument.output).unwrap_or(ValueType::Any);
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
                _ => NodeKind::menu_constructor_named(name),
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
            if self.native_scope.is_some() {
                // Native producer identities distinguish an inline operand
                // from a captured source. Preserve that distinction: making
                // a fresh Literal for an inline false/number invents a node
                // that the authoritative source deliberately does not name.
                // Captured expressions still resolve to their actual prior
                // producers, and identity/wiring checks remain mandatory.
                self.input_expr(n, key, expr)?;
            } else {
                let source =
                    self.expression(expr, [pos[0] - 260.0, pos[1] + 120.0 + i as f64 * 180.0])?;
                self.connect_expression(source, n, key)?;
            }
        }
        Ok(n)
    }
    /// Restore the declared sequence representation instead of inferring it
    /// from equal values or promoting historical property strings to pins.
    fn native_value_sequence(&mut self,identity:&crate::native_frame_preparation::NativeNodeIdentity,value:&Expr,pos:[f64;2])->Result<ExpressionRef,String>{
        let(function,items,property,output)=match(identity.kind,value){
            (NodeKind::ListLiteral,Expr::ListLit(items))=>(None,items.as_slice(),"items","list"),
            (NodeKind::FunctionCall,Expr::Call{name,args})=>(Some(name.as_str()),args.as_slice(),"args","result"),
            _=>return Err("Native sequence kind differs from its actual expression".into()),
        };
        let property_only=matches!(identity.ports.as_slice(),[(role,_,None)]if role==output);
        let strings=items.iter().map(|item|match item{Expr::Str(value)=>Some(value.clone()),_=>None}).collect::<Option<Vec<_>>>();
        // Before the explicit annotation existed, only nonempty one-output
        // string sources prove this property representation. Empty unmarked
        // sources retain the previous typed-zero interpretation, not a guess.
        let legacy=identity.legacy_string_sequence||property_only&&!items.is_empty()&&strings.is_some();
        if legacy{
            if !property_only{return Err("Native property sequence cannot acquire typed input pins".into());}
            let values=strings.ok_or("Native property StringList requires actual string expressions")?;
            let node=self.node(identity.kind,pos)?;
            if let Some(name)=function{self.property(node,"function",PropertyValue::String(name.into()))?;}
            self.property(node,property,PropertyValue::StringList(values))?;
            if !identity.legacy_string_sequence{self.property(node,"@native_string_sequence_source",PropertyValue::String("unmarked_v0".into()))?;}
            return self.value_output(node);
        }
        let [(role,_,None),inputs @ ..]=identity.ports.as_slice()else{return Err("Native typed sequence lost its output/arity".into());};
        if role!=output||inputs.len()!=items.len()||inputs.iter().enumerate().any(|(index,(role,_,_))|*role!=format!("item_{index}")){
            return Err("Native typed sequence input roles/arity differ from its actual expression".into());
        }
        let node=self.value_sequence(identity.kind,function,items,pos)?;self.value_output(node)
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
    fn connect_expression(&mut self, source: ExpressionRef, target: NodeId, key: &str) -> Result<(), String> {
        let output_key = self.graph.pins[&source.output].key.clone();
        self.connect(source.node, &output_key, target, key)
    }
    fn input_expr(&mut self, n: NodeId, key: &str, value: &Expr) -> Result<(), String> {
        // An authored empty string is a real value, unlike a catalogue's
        // empty placeholder. Keep that distinction explicit in the graph.
        if self.native_scope.is_some() && matches!(value,Expr::Str(text) if text.is_empty()) {
            return self.default(n,key,PropertyValue::String(String::new()));
        }
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
            return self.connect_expression(self.value_output(source)?, n, key);
        }
        if let Some(value) = literal(value) {
            return self.default(n, key, value);
        }
        let p = self.graph.nodes[&n].position;
        let expr = self.expression(value, [p[0] - 260.0, p[1] + 160.0])?;
        self.connect_expression(expr, n, key)
    }
    fn choice_condition(&mut self, n: NodeId, key: &str, value: &Expr) -> Result<(), String> {
        let position = self.graph.nodes[&n].position;
        let index = crate::choice_option_index(key).unwrap_or(0);
        // Even `if true` is a real authored expression. Keep it connected so
        // source round-trip distinguishes it from an absent (always) guard.
        let source = self.expression(value,
            [position[0] - 260.0, position[1] + 80.0 + index as f64 * 180.0])?;
        self.connect_expression(source, n, key)
    }
    fn cinematic_transition(&mut self,n:NodeId,value:Option<&str>)->Result<(),String>{
        let value=match value{None=>Transition::None,Some("fade")=>Transition::Fade{duration_ms:Transition::DEFAULT_FADE_MS},Some("dissolve")=>Transition::Dissolve{duration_ms:Transition::DEFAULT_DISSOLVE_MS},_=>return Err("Cinematic transition has no exact existing graph preset".into())};
        self.transition(n,&value)
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
        if let Some(output)=self.native_constant_consumer.as_ref()
            .filter(|identity|identity.kind==NodeKind::Dialogue)
            .and_then(|identity|identity.dynamic_inputs.get("text")).copied() {
            let candidates:Vec<_>=self.native_producers.iter()
                .filter(|(id,(_,inspection,_))|!inspection&&self.native_active.contains(*id))
                .filter_map(|(_,(_,_,source))|self.native_nodes.get(&source.node)
                    .filter(|identity|identity.ports.iter().any(|(role,pin,incoming)|
                        *pin==output&&incoming.is_none()&&self.graph.pin_by_key(source.node,role).is_some_and(|pin|pin.id==source.output)))
                    .map(|_|*source)).collect();
            let [source]=candidates.as_slice() else{return Err("Dynamic Dialogue text requires its one actual active source output".into());};
            return self.connect_expression(*source,dialogue,"text");
        }
        // A raw-template capture carries identity only: the real body still
        // performs interpolation once, with its original translation key.
        if self.native_scope.is_some() {
            let template=text_source(text);
            let candidates:std::collections::BTreeSet<_>=self.native_producers.iter()
                .filter(|(id,_)|self.native_active.contains(*id))
                .filter_map(|(_,(_,inspection,source))|(!inspection
                    && self.graph.nodes[&source.node].kind==NodeKind::TextValue
                    && self.graph.nodes[&source.node].properties.get("value")==Some(&PropertyValue::String(template.clone())))
                    .then_some(source.node)).collect();
            if candidates.len()>1 {return Err("Ambiguous native raw Dialogue template producer".into());}
            if let Some(value)=candidates.first(){return self.connect(*value,"value",dialogue,"text");}
        }
        // Blank dialogue is intentional in credits; never trim it.
        let value = self.node(NodeKind::TextValue, position)?;
        self.property(value, "value", PropertyValue::String(text_source(text)))?;
        self.connect(value, "value", dialogue, "text")
    }
    fn sequence(&mut self,body:&[Statement],prev:(NodeId,String),x:f64,y:f64,owner:Option<NodeId>)->Result<f64,String>{
        self.sequence_impl(body,prev,x,y,owner,true)
    }
    fn sequence_impl(
        &mut self,
        body: &[Statement],
        mut prev: (NodeId, String),
        mut x: f64,
        y: f64,
        owner: Option<NodeId>,
        synthesize_end:bool,
    ) -> Result<f64, String> {
        self.lane = self.lane.max(y);
        for (index, stmt) in body.iter().enumerate() {
            use Statement as S;
            if let S::StandardMacroDispatch{dispatch}=stmt {
                if index+1!=body.len(){return Err("A macro's continuation belongs to its actual execution outputs; sequential source tail is not losslessly representable".into());}
                return self.macro_dispatch(dispatch,prev,x,y);
            }
            if let S::ScheduledConsumer{version,scope,consumer,steps,body:consumer_body}=stmt {
                rvn_parser::scheduled_source::dialogue_text_binding(stmt)?;
                if !matches!(version,1|2|3) {return Err("Native source contract/version/scope kind refused".into());}
                if *version==3{rvn_parser::scheduled_source::native_loop_contract::binding(stmt)?;}
                if self.native_scope.as_ref().is_some_and(|old|old!=scope) {return Err("Native graph scope changed inside one function".into());}
                if self.native_scope.is_none() {
                    let (graph,identity)=crate::native_frame_preparation::decode_scope(scope)?;
                    let root=self.graph.nodes.values().find(|node|node.kind==identity.kind&&matches!(node.kind,NodeKind::Init|NodeKind::Label|NodeKind::FunctionEntry|NodeKind::ScreenEntry|NodeKind::HandlerEntry)).ok_or("Missing actual definition root of matching kind")?.id;
                    if identity.exec_links_v1{self.graph.nodes.get_mut(&root).unwrap().properties.insert("@source_exec_links_v1".into(),PropertyValue::Bool(true));}
                    self.native_nodes.insert(root,identity);self.graph.graph_id=graph;
                    self.graph.enable_native_frame_preparation()?;
                    self.native_scope=Some(scope.clone());
                }
                self.native_active.clear();
                self.native_operand_steps(steps,x,y)?;
                let identity=crate::native_frame_preparation::decode_node(consumer)?;
                if identity.exec_links_v1!=crate::native_frame_preparation::uses_exec_links(&self.graph)?{return Err("ExecLinksV1 actual consumer format differs from its source root".into());}
                if consumer_body.is_empty() {
                    if identity.kind!=NodeKind::BranchEnd || !steps.is_empty() || index+1!=body.len() {return Err("Empty Native source marker requires the actual final BranchEnd and no producers".into());}
                    let owner=owner.ok_or("Native BranchEnd has no actual control owner")?;
                    let node=self.graph.add_branch_end(owner,[x,y]).map_err(err)?;
                    self.connect(prev.0,&prev.1,node,"exec_in")?;
                    self.native_nodes.insert(node,identity);self.native_active.clear();return Ok(x);
                }
                if consumer_body.len()!=1 {return Err("Native requires exactly one actual execution consumer".into());}
                let output=self.graph.pin_by_key(prev.0,&prev.1).ok_or("Actual prior execution output missing")?.id;
                let before:std::collections::BTreeSet<_>=self.graph.edges.keys().copied().collect();
                let predeclared=self.native_assignment_pending.iter().any(|node|self.native_nodes.get(node)==Some(&identity));
                let outer_loop=self.native_loop_pending.take();
                if *version==3{self.native_loop_pending=Some(NativeLoopDeclaration{identity:identity.clone(),binding:rvn_parser::scheduled_source::native_loop_contract::binding(stmt)?.unwrap()});}
                let outer=self.native_constant_consumer.replace(identity.clone());
                let next=self.sequence(consumer_body,prev.clone(),x,y,None);
                self.native_constant_consumer=outer;self.native_loop_pending=outer_loop;x=next?;
                let consumers:Vec<_>=self.graph.edges.values().filter(|edge|edge.output==output&&!before.contains(&edge.id)).filter_map(|edge|self.graph.pins.get(&edge.input)).map(|pin|pin.node).collect();
                let [node]=consumers.as_slice() else {return Err("Native consumer reconstruction is ambiguous at its real incoming execution wire".into());};
                if self.graph.nodes[node].kind!=identity.kind {return Err("Native consumer role/kind mismatch".into());}
                if let Some(prior)=self.native_nodes.insert(*node,identity.clone()) {if prior!=identity||!predeclared&& !(*version==3&&self.graph.nodes[node].native_loop_contract.is_some()){return Err("Duplicate native consumer identity".into());}}
                self.native_active.clear();
                if matches!(consumer_body[0],S::FunctionReturn{..}|S::Return|S::Jump{..}) {
                    if index+1!=body.len() {return Err("Unreachable source after native return".into());}
                    return Ok(x);
                }
                let continuation=if matches!(self.graph.nodes[node].kind,NodeKind::If|NodeKind::While|NodeKind::ForEach|NodeKind::ForLoopWithBreak|NodeKind::Sequence|NodeKind::Choice|NodeKind::Imagemap) {"completed"}else{"exec_out"};
                prev=(*node,continuation.into());continue;
            }
            if matches!(stmt,S::LoopBreak) {
                let target=*self.range_owners.last().ok_or("break hors ForLoopWithBreak")?;
                self.connect(prev.0,&prev.1,target,"break")?;
                if index+1!=body.len(){return Err("Instructions inaccessibles après break".into());}
                return Ok(x);
            }
            let kind = match stmt {
                S::FunctionLibrary{..}=>return Err("Nested Library scope is not an executable node".into()),
                S::LibraryUnitCall{..}=>NodeKind::UnitCall,
                S::SetVar { .. } | S::LocalVar { .. } if rvn_parser::widget_create_effect::parts(stmt)?.is_some() => NodeKind::WidgetCreate,
                S::SetVar { .. } => NodeKind::SetVariable,
                S::LocalVar { .. } => NodeKind::LocalVariable,
                S::WidgetAddToViewport { .. } => NodeKind::WidgetAddToViewport,
                S::WidgetRemoveFromParent { .. } => NodeKind::WidgetRemoveFromParent,
                S::UiOpen { .. } => NodeKind::UiOpen,
                S::UiClose { .. } => NodeKind::UiClose,
                S::UiFocus { .. } => NodeKind::UiFocus,
                S::UiSetState { .. } => NodeKind::UiSetState,
                S::MenuExecute { .. } => NodeKind::MenuExecute,
                S::MotionPlay { .. } => NodeKind::MotionPlay,
                S::MotionStop { .. } => NodeKind::MotionStop,
                S::MotionWait { .. } => NodeKind::MotionWait,
                S::CharacterCreate { .. } => NodeKind::CharacterCreate,
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
                S::UnitCall { .. } => NodeKind::UnitCall,
                S::UnitReturn => NodeKind::UnitReturn,
                S::FunctionReturn { .. } => NodeKind::FunctionReturn,
                S::Sequence {..}=>NodeKind::Sequence,
                S::ForLoopWithBreak {..}=>NodeKind::ForLoopWithBreak,
                S::While { .. } => NodeKind::While,
                S::ForEach { .. } => NodeKind::ForEach,
                S::Jump { .. } => NodeKind::Jump,
                S::Call { .. } => NodeKind::Call,
                S::Return => NodeKind::Return,
                S::Scene { .. } => NodeKind::Scene,
                S::ShowSprite { .. } => NodeKind::SpriteShow,
                S::HideSprite { .. } => NodeKind::SpriteHide,
                S::MoveSprite { .. } => NodeKind::SpriteMove,
                S::SpriteAnimate { .. } => NodeKind::SpriteAnimate,
                S::SpriteStopAnimation { .. } => NodeKind::SpriteStopAnimation,
                S::SpriteEffect { .. } => NodeKind::SpriteEffect,
                S::MusicPlay { .. } => NodeKind::MusicPlay,
                S::Timer { .. } => NodeKind::Timer,
                S::TimerCancel => NodeKind::TimerCancel,
                S::MusicStop { .. } => NodeKind::MusicStop,
                S::MusicVolume { .. } | S::MusicVolumeValue { .. } => NodeKind::MusicVolume,
                S::SfxPlay { .. } => NodeKind::SfxPlay,
                S::SfxStop { .. } => NodeKind::SfxStop,
                S::VoicePlay { .. } => NodeKind::VoicePlay,
                S::VoiceStop => NodeKind::VoiceStop,
                S::CinematicShow { .. } => NodeKind::CinematicShow,
                S::CinematicHide { .. } => NodeKind::CinematicHide,
                S::UnlockEnding { .. } => NodeKind::UnlockEnding,
                S::Imagemap { .. } => NodeKind::Imagemap,
                S::TypewriterSet { .. } | S::TypewriterSetValue { .. } => NodeKind::TypewriterSet,
                S::TypewriterSpeed { .. } | S::TypewriterSpeedValue { .. } => NodeKind::TypewriterSpeed,
                S::Config { .. } => NodeKind::Config,
                S::MethodCall { .. } => NodeKind::MethodCall,
                other => return Err(format!("Instruction non prise en charge : {other:?}")),
            };
            let n = self.node(kind, [x, y])?;
            self.connect(prev.0, &prev.1, n, "exec_in")?;
            let mut continuation = "exec_out";
            match stmt {
                S::CharacterCreate { id, display_name } => {
                    self.text(n, "id", id)?;
                    self.text(n, "display_name", display_name)?;
                }
                S::MethodCall { target, method, arg, transition } => {
                    if arg.as_ref().is_some_and(String::is_empty) {
                        return Err("MethodCall explicit empty argument is not representable by the historical empty argument pin".into());
                    }
                    self.text(n, "target", target)?;
                    self.text(n, "method", method)?;
                    self.text(n, "arg", arg.as_deref().unwrap_or(""))?;
                    self.transition(n, transition)?;
                }
                S::Config { key, value } => {
                    self.text(n, "key", key)?;
                    self.text(n, "value", value)?;
                }
                S::SetVar { .. } | S::LocalVar { .. } if kind==NodeKind::WidgetCreate => {
                    let input=rvn_parser::widget_create_effect::parts(stmt)?.ok_or("Actual Widget factory instruction missing")?;
                    self.property(n,"name",PropertyValue::String(input.binding.into()))?;
                    self.property(n,"local",PropertyValue::Bool(input.local))?;
                    match input.class {
                        rvn_parser::widget_create_effect::ClassInput::Asset(uuid)=>self.input_expr(n,"class",&Expr::Call{name:"__rvn_widget_class_v1".into(),args:vec![Expr::Str(uuid.into())]})?,
                        rvn_parser::widget_create_effect::ClassInput::Null=>self.input_expr(n,"class",&Expr::Call{name:"__rvn_widget_class_none_v1".into(),args:vec![]})?,
                        rvn_parser::widget_create_effect::ClassInput::Value(value)=>self.input_expr(n,"class",value)?,
                    }
                    self.input_expr(n,"arguments",input.arguments)?;
                    if input.local&&self.narrative_globals.is_some(){self.binding_states.insert(input.binding.into(),2);}
                }
                S::SetVar { name, value } | S::LocalVar { name, value } => {
                    self.variable(n, name)?;
                    self.text(n, "name", name)?;
                    self.input_expr(n, "value", value)?;
                    if self.narrative_globals.is_some() && matches!(stmt,S::LocalVar{..}) {
                        self.binding_states.insert(name.clone(),2);
                    }
                }
                S::LibraryUnitCall{call}=>self.import_qualified_call1027(n,call,rvn_parser::function_library_program1023::LibraryCallMode::Completion)?,
                S::UnitCall{name,arguments} => {
                    let callee=self.unit_declarations.get(name).ok_or("UnitCall callee is missing or not a Unit declaration in this project/source owner")?.clone();
                    let parameters=crate::unit_graph_source1003::parameters(&callee)?;
                    if parameters.len()!=arguments.len(){return Err("UnitCall arity differs from its actual declaration".into());}
                    self.property(n,"function",PropertyValue::String(name.clone()))?;
                    for(parameter,argument)in parameters.into_iter().zip(arguments){
                        let identity=parameter.parameter_id.clone().ok_or("Unit parameter has no identity")?;
                        let key=format!("argument_{}",identity.as_str());
                        let input=self.graph.add_pin(n,&key,&parameter.label,PinDirection::Input,ValueType::Any,PinCardinality::One).map_err(err)?;
                        self.graph.pins.get_mut(&input).ok_or("Argument allocation missing")?.parameter_id=Some(identity);
                        self.input_expr(n,&key,argument)?;
                    }
                }
                S::UnitReturn=>{crate::unit_graph_source1003::entry(&self.graph)?;}
                S::FunctionReturn { value } => self.input_expr(n, "value", value)?,
                S::UiOpen {
                    name,
                    arguments,
                    modal,
                    layer,
                    story,
                } => {
                    self.graph.nodes.get_mut(&n).unwrap().properties.insert("story".into(),PropertyValue::Bool(*story));
                    self.input_expr(n, "name", name)?;
                    self.input_expr(n, "arguments", arguments)?;
                    self.input_expr(n, "modal", modal)?;
                    self.input_expr(n, "layer", layer)?;
                }
                S::WidgetAddToViewport { object, z_order } => {self.input_expr(n,"object",object)?;self.input_expr(n,"z_order",z_order)?;},
                S::WidgetRemoveFromParent { object } => self.input_expr(n,"object",object)?,
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
                S::MenuExecute { request } => self.input_expr(n, "request", request)?,
                S::Sequence {branches}=>{
                    continuation="completed";self.graph.resize_sequence_outputs(n,branches.len()).map_err(err)?;
                    for (index,branch) in branches.iter().enumerate(){self.lane+=960.0;x=x.max(self.sequence(branch,(n,format!("then_{index}")),x+960.0,self.lane,Some(n))?);}
                }
                S::ForLoopWithBreak {name,first,last,body}=>{
                    continuation="completed";self.property(n,"name",PropertyValue::String(name.clone()))?;
                    self.input_expr(n,"first",first)?;self.input_expr(n,"last",last)?;
                    let incoming=self.binding_states.clone();
                    let native_body=self.enter_native_loop_source_body(n,None);
                    self.range_owners.push(n);self.lane+=960.0;
                    let result=self.sequence(body,(n,"body".into()),x+960.0,self.lane,Some(n));
                    self.range_owners.pop();if native_body{self.native_loop_bodies.pop();}x=x.max(result?);
                    self.merge_incoming_bindings(&incoming);
                }
                S::While { condition, body } => {
                    continuation = "completed";
                    self.input_expr(n, "condition", condition)?;
                    let incoming=self.binding_states.clone();
                    self.lane += 960.0;
                    x = x.max(self.sequence(
                        body,
                        (n, "body".into()),
                        x + 960.0,
                        self.lane,
                        Some(n),
                    )?);
                    self.merge_incoming_bindings(&incoming);
                }
                S::ForEach {
                    name,
                    collection,
                    body,
                } => {
                    continuation = "completed";
                    self.property(n, "name", PropertyValue::String(name.clone()))?;
                    self.input_expr(n, "collection", collection)?;
                    let incoming=self.binding_states.clone();
                    self.lane += 960.0;
                    let native_body=self.enter_native_loop_source_body(n,Some(collection));
                    let result=self.sequence(body,(n,"body".into()),x+960.0,self.lane,Some(n));
                    if native_body{self.native_loop_bodies.pop();}
                    x=x.max(result?);
                    self.merge_incoming_bindings(&incoming);
                }
                S::Dialogue { character_id, text } => {
                    self.text(n, "character", character_id.as_deref().unwrap_or(""))?;
                    if self.native_constant_consumer.as_ref().is_some_and(|identity|identity.kind==NodeKind::Dialogue&&identity.dynamic_inputs.contains_key("text")) {
                        self.dialogue_text(n,text,[x-260.0,y+340.0])?;
                    } else if let [rvn_parser::TextSegment::Interp(expression @ Expr::Call { name, .. })] =
                        text.0.as_slice()
                    {
                        // A Native Dialogue consumes its genuine already
                        // reconstructed producer/typed terminal. Rewrapping
                        // that reference in TextValue would invent a node and
                        // destroy strict source identities on round-trip.
                        if (self.native_scope.is_some() && matches!(name.as_str(),"__rvn_captured_v1"|"__rvn_terminal_v1"))
                            || name == "string_to_text" || (self.graph.text_owner.is_some()
                            && matches!(name.as_str(), "__rvn_localizable_text" | "__rvn_invariant_text" | "__rvn_empty_text" | "__rvn_format_text")) {
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
                S::Jump { target } | S::Call { target } => {
                    self.text(n, "target", target)?;
                    if self.narrative_globals.is_some() && matches!(stmt,S::Call{..}) {
                        for name in &self.narrative_locals {
                            let state=self.binding_states.entry(name.clone()).or_insert(1);
                            if *state!=2 {*state=3;}
                        }
                    }
                },
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
                    let native_sprite_path=if self.native_constant_consumer.as_ref().is_some_and(|identity|identity.kind==NodeKind::SpriteShow&&!identity.static_inputs.contains_key("emotion")) {
                        let mut input=self.graph.pin_by_key(n,"character").unwrap().id;let mut seen=std::collections::BTreeSet::new();
                        loop {
                            let Some(edge)=self.graph.edges.values().find(|edge|edge.input==input)else{break false;};
                            let source=&self.graph.nodes[&self.graph.pins[&edge.output].node];
                            if seen.len()>=128||!seen.insert(source.id){break false;}
                            match source.kind {
                                NodeKind::CharacterValue=>break self.graph.pin_by_key(source.id,"sprite").is_some_and(|pin|self.graph.edges.values().any(|edge|edge.input==pin.id)),
                                NodeKind::Reroute=>input=self.graph.pin_by_key(source.id,"value").unwrap().id,
                                _=>break false,
                            }
                        }
                    }else{false};
                    self.text(n, "emotion", if native_sprite_path {""} else {emotion.as_deref().unwrap_or("")})?;
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
                S::SpriteAnimate { character_id, animation, params } => {
                    self.text(n,"character",character_id)?;self.text(n,"animation",animation)?;
                    for param in params {
                        let value=match &param.value {rvn_parser::AnimationValue::Bool(v)=>PropertyValue::Bool(*v),rvn_parser::AnimationValue::Int(v)=>PropertyValue::Int(*v),rvn_parser::AnimationValue::Float(v)=>PropertyValue::Float(f64::from(*v)),rvn_parser::AnimationValue::Str(v)=>PropertyValue::String(v.clone())};
                        self.property(n,&format!("animation_param_{}",param.name),value)?;
                    }
                },
                S::SpriteStopAnimation { character_id } => self.text(n,"character",character_id)?,
                S::SpriteEffect { character_id,flip_x,flip_y,scale,rotation,tint } => {
                    self.text(n,"character",character_id)?;
                    for(key,value)in[("flip_x",flip_x.map(PropertyValue::Bool)),("flip_y",flip_y.map(PropertyValue::Bool)),("scale",scale.map(|value|PropertyValue::Float(f64::from(value)))),("rotation",rotation.map(|value|PropertyValue::Float(f64::from(value)))),("tint",tint.clone().map(PropertyValue::String))]{
                        self.property(n,&format!("apply_{key}"),PropertyValue::Bool(value.is_some()))?;
                        if let Some(value)=value{self.default(n,key,value)?;}
                    }
                },
                S::Timer { duration_secs, action } => {
                    let mut parts = action.split_whitespace();
                    let (Some("jump"), Some(target), None) = (parts.next(), parts.next(), parts.next()) else {
                        return Err("Timer action cannot be represented losslessly by the existing jump Timer node".into());
                    };
                    self.default(n, "duration", PropertyValue::Float(f64::from(*duration_secs)))?;
                    self.text(n, "target", target)?;
                }
                S::TimerCancel => {}
                S::MusicStop { transition } => self.transition(n, transition)?,
                S::MusicVolumeValue { level } => self.input_expr(n,"level",level)?,
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
                    self.cinematic_transition(n,transition.as_deref())?;
                }
                S::CinematicHide { transition } => {
                    self.cinematic_transition(n,transition.as_deref())?
                }
                S::UnlockEnding { id } => self.text(n, "id", id)?,
                S::TypewriterSetValue { enabled } => self.input_expr(n,"enabled",enabled)?,
                S::TypewriterSet { enabled } => {
                    self.default(n, "enabled", PropertyValue::Bool(*enabled))?
                }
                S::TypewriterSpeedValue { chars_per_sec } => self.input_expr(n,"speed",chars_per_sec)?,
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
                    let incoming=self.binding_states.clone();
                    let mut merged=BTreeMap::<String,u8>::new();
                    for (key, branch) in [("then", then_branch), ("else", else_branch)] {
                        self.binding_states=incoming.clone();
                        self.lane += 960.0;
                        x = x.max(self.sequence(
                            branch,
                            (n, key.into()),
                            self.graph.nodes[&n].position[0] + 960.0,
                            self.lane,
                            Some(n),
                        )?);
                        self.accumulate_branch_bindings(&mut merged);
                    }
                    self.binding_states=merged;
                }
                S::Choice { options } => {
                    continuation = "completed";
                    let incoming=self.binding_states.clone();
                    let mut merged=incoming.clone();
                    for option in options {
                        self.binding_states=incoming.clone();
                        let pins = self
                            .graph
                            .add_choice_option(n, text_source(&option.label))
                            .map_err(err)?;
                        if let Some(condition) = &option.condition {
                            let key = format!("option_{}_condition", pins.index);
                            let before = self.graph.clone();
                            let before_inspectors = self.inspectors.clone();
                            if self.choice_condition(n, &key, condition).is_err() {
                                self.graph = before;
                                self.inspectors = before_inspectors;
                                self.property(n, &key, PropertyValue::String(expression_source(condition)))?;
                            }
                        }
                        self.lane += 960.0;
                        x = x.max(self.sequence(
                            &option.body,
                            (n, format!("option_{}", pins.index)),
                            self.graph.nodes[&n].position[0] + 960.0,
                            self.lane,
                            Some(n),
                        )?);
                        self.accumulate_branch_bindings(&mut merged);
                    }
                    self.binding_states=merged;
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
                        specs.push(crate::imagemap_spec::HotspotSpec::from_hotspot(h)?.encode()?);
                    }
                    self.graph.set_imagemap_hotspots(n, specs).map_err(err)?;
                    let incoming=self.binding_states.clone();
                    let mut merged=incoming.clone();
                    for (i, h) in hotspots.iter().enumerate() {
                        self.binding_states=incoming.clone();
                        self.lane += 960.0;
                        x = x.max(self.sequence(
                            &h.body,
                            (n, format!("hotspot_{i}")),
                            self.graph.nodes[&n].position[0] + 960.0,
                            self.lane,
                            Some(n),
                        )?);
                        self.accumulate_branch_bindings(&mut merged);
                    }
                    self.binding_states=merged;
                }
                S::Return | S::VoiceStop => {}
                _ => unreachable!(),
            }
            x += 960.0;
            if matches!(stmt, S::Jump { .. } | S::Return | S::FunctionReturn { .. } | S::UnitReturn) {
                if index + 1 != body.len() {
                    return Err(
                        "Instructions inaccessibles après jump/return : conversion refusée".into(),
                    );
                }
                return Ok(x);
            }
            prev = (n, continuation.into());
        }
        // Native source records a real BranchEnd with its own empty consumer
        // marker above. Absence of that marker must remain absence of a node;
        // only Legacy import synthesizes a visual closure for implicit ends.
        if let Some(owner) = owner.filter(|_|synthesize_end && self.native_scope.is_none()) {
            let end = self.graph.add_branch_end(owner, [x, y]).map_err(err)?;
            self.connect(prev.0, &prev.1, end, "exec_in")?;
        }
        Ok(x)
    }
}

// The complete Program has already been reparsed against its frozen origins.
// Validate its semantics before choosing any graph, including Legacy Unit
// declarations/calls which have no authoring ParameterIds in ParsedProgram.
pub(crate) fn import_frozen_native_scopes(
    script: &Script, resolved: &Script, declared_empty_init: bool,
) -> Result<Vec<GraphDocument>,String> {
    validate_frozen_program_operations(resolved, false)?;
    crate::validate_project_script(resolved, false)?;
    rvn_parser::scheduled_source::native_loop_contract::validate_authored_bindings(resolved)?;
    import_script_selected(script, &Default::default(), false, Some(resolved), &[],
        &mut || Err("Frozen Native reconstruction cannot allocate authoring parameter identities".into()),
        Some(declared_empty_init))
}

pub(crate) fn validate_frozen_program_operations(body: &[Statement], native: bool) -> Result<(),String> {
    let mut pending = vec![body];
    while let Some(block) = pending.pop() {
        for statement in block {
            match statement {
                Statement::Use { .. } =>
                    return Err("Frozen Native validation requires a fully resolved Program; residual Use is refused".into()),
                Statement::UnitCall { .. } | Statement::UnitReturn if native =>
                    return Err("Frozen Native definitions cannot contain UnitCall or UnitReturn without an admitted Native/Unit capability".into()),
                Statement::Init { body } | Statement::Function { body, .. }
                | Statement::UnitFunction { body, .. } | Statement::Screen { body, .. }
                | Statement::Handler { body, .. } | Statement::ScheduledConsumer { body, .. }
                | Statement::While { body, .. } | Statement::ForEach { body, .. }
                | Statement::ForLoopWithBreak { body, .. } => pending.push(body),
                Statement::If { then_branch, else_branch, .. } => {
                    pending.push(then_branch); pending.push(else_branch);
                }
                Statement::Sequence { branches } => pending.extend(branches.iter().map(Vec::as_slice)),
                Statement::StandardMacroDispatch { dispatch } =>
                    pending.extend(dispatch.branches.iter().map(|branch| branch.body.as_slice())),
                Statement::Choice { options } => pending.extend(options.iter().map(|option|option.body.as_slice())),
                Statement::Imagemap { hotspots, .. } => pending.extend(hotspots.iter().map(|hotspot|hotspot.body.as_slice())),
                _ => {}
            }
        }
    }
    Ok(())
}

pub(crate) fn import_native_preparation(script: &Script) -> Result<Vec<GraphDocument>,String> {
    import_native_preparation_with_owners(script,None,&[])
}

/// Authored scopes only, typed against the caller's genuine resolved closure.
/// No presentation registry/cache is expanded into a declaration or statement.
pub(crate) fn import_native_preparation_with_context(script: &Script, resolved: &Script)
    -> Result<Vec<GraphDocument>,String>
{
    import_native_preparation_with_owners(script,Some(resolved),&[])
}


// Only a real reimport supplies retained Unit parameter owners. Fresh imports
// keep an empty list and the existing refusing allocator; no IDs are minted.
pub(crate) fn import_native_preparation_with_owners(
    script:&Script,context:Option<&Script>,owners:&[GraphDocument],
)->Result<Vec<GraphDocument>,String>{
    import_script_with_text_variables(script,&Default::default(),false,context,owners)
}


#[cfg(test)]
mod scheduled_binding_type_tests_0422 {
    use super::*;
    use rvn_parser::ConsumerStep;
    fn producer(n: u64) -> String { format!("n{n}:variable_get|value={n}@0") }
    fn captured(id: &str) -> Expr { Expr::Call { name: "__rvn_captured_v1".into(), args: vec![Expr::Str(id.into())] } }
    fn terminal(id: &str, role: &str) -> Expr { Expr::Call { name: "__rvn_terminal_v1".into(), args: vec![Expr::Str(id.into()), Expr::Str(role.into())] } }
    fn globals() -> BTreeMap<String, VariableDefinition> {
        let script=rvn_parser::parse("init {set score=\"global\"}").unwrap();
        let [Statement::Init { body }]=script.as_slice() else {panic!()};
        initialized_variables(body, &Default::default())
    }
    #[test]
    fn captures_freeze_global_string_before_local_int_without_evaluation() {
        let id=producer(2); let mut vars=globals();
        let captured_types=ScheduledBindingTypes::from_steps(&[ConsumerStep::Capture {producer:id.clone(), value:Expr::Var("score".into())}], &vars);
        // This environment change is the consumer's actual Local binding, not
        // an execution or replacement of the preceding capture.
        vars.get_mut("score").unwrap().value_type=ValueType::Int;
        assert_eq!(captured_types.expression_type(&captured(&id), &vars),ValueType::String);
        assert_eq!(captured_types.expression_type(&Expr::Var("score".into()), &vars),ValueType::Int);
        assert_eq!(vars["score"].default_value,PropertyValue::String("global".into()));
    }
    #[test]
    fn shared_capture_conditions_and_conversions_reuse_source_types_only() {
        let id=producer(2);let next=producer(3);let vars=globals();
        let table=ScheduledBindingTypes::from_steps(&[
            ConsumerStep::Capture { producer:id.clone(),value:Expr::Var("score".into()) },
            ConsumerStep::Capture { producer:next.clone(),value:Expr::Call { name:"split".into(),args:vec![captured(&id),Expr::Str(",".into())] } },
        ],&vars);
        let expr=Expr::Call{name:"list_slice".into(),args:vec![captured(&next),Expr::Int(0),Expr::Int(1)]};
        assert_eq!(table.expression_type(&expr,&vars),ValueType::List(Box::new(ValueType::String)));
        let cmp=Expr::BinOp{op:BinOpKind::Eq,left:Box::new(captured(&id)),right:Box::new(captured(&id))};
        assert_eq!(table.expression_type(&cmp,&vars),ValueType::Bool);
        let text=Expr::Call{name:"string_to_text".into(),args:vec![captured(&id)]};
        assert_eq!(table.expression_type(&text,&vars),ValueType::InterpolatedText);
        assert_eq!(table.expression_type(&captured(&id),&vars),ValueType::String);
    }
    #[test]
    fn inspector_roles_are_typed_only_after_real_inspect_identity() {
        let id="n3:get_string_table_id_and_key|text=5@0,valid=6@0,table_id=7@0,key=8@0";
        let table=ScheduledBindingTypes::from_steps(&[ConsumerStep::InspectTextTable { producer:id.into(),value:Expr::Call{name:"__rvn_empty_text".into(),args:vec![]} }],&Default::default());
        for (role,ty) in [("valid",ValueType::Bool),("table_id",ValueType::Name),("key",ValueType::String)] {
            assert_eq!(table.expression_type(&terminal(id,role),&Default::default()),ty);
        }
        assert_eq!(table.expression_type(&terminal(id,"fake"),&Default::default()),ValueType::Any);
        assert_eq!(table.expression_type(&captured(id),&Default::default()),ValueType::Any);
    }
    #[test]
    fn absent_future_duplicate_and_wrong_kind_identity_never_narrow_types() {
        let a=producer(2);let b=producer(3);
        let table=ScheduledBindingTypes::from_steps(&[
            ConsumerStep::Capture {producer:a.clone(),value:captured(&b)},
            ConsumerStep::Capture {producer:b.clone(),value:Expr::Int(9)},
        ],&Default::default());
        assert_eq!(table.expression_type(&captured(&a),&Default::default()),ValueType::Any);
        let dup=ScheduledBindingTypes::from_steps(&[
            ConsumerStep::Capture {producer:a.clone(),value:Expr::Int(9)},
            ConsumerStep::Capture {producer:a.clone(),value:Expr::Str("changed".into())},
        ],&Default::default());
        assert_eq!(dup.expression_type(&captured(&a),&Default::default()),ValueType::Any);
        for id in ["fake",a.as_str()] {
            let wrong=ScheduledBindingTypes::from_steps(&[ConsumerStep::InspectTextTable {producer:id.into(),value:Expr::Int(9)}],&Default::default());
            assert_eq!(wrong.expression_type(&terminal(id,"valid"),&Default::default()),ValueType::Any);
        }
        assert_eq!(ScheduledBindingTypes::default().expression_type(&captured(&a),&Default::default()),ValueType::Any);
    }
    #[test]
    fn consumer_tables_end_at_scope_boundary() {
        let id=producer(2);
        let first=ScheduledBindingTypes::from_steps(&[ConsumerStep::Capture{producer:id.clone(),value:Expr::Int(7)}],&Default::default());
        assert_eq!(first.expression_type(&captured(&id),&Default::default()),ValueType::Int);
        assert_eq!(ScheduledBindingTypes::from_steps(&[],&Default::default()).expression_type(&captured(&id),&Default::default()),ValueType::Any);
    }
    #[test]
    fn native_context_uses_real_imported_init_and_keeps_it_readonly() {
        let own=format!("{}\nfunction imported_reader(){{return borrowed}}\nscene label start\nreturn\n",
            include_str!("../tests/fixtures/native_source_single.rvn"));
        let authored=rvn_parser::parse_program(&own).unwrap().script;
        let mut resolved=rvn_parser::parse("init {set borrowed=4}").unwrap();resolved.extend(authored.clone());
        let guard=resolved.clone();let original=authored.clone();
        let graphs=import_native_preparation_with_context(&authored,&resolved).unwrap();
        for graph in &graphs {assert_eq!(graph.variables["borrowed"].value_type,ValueType::Int);}
        let empty_init=graphs.iter().find(|g|g.kind==GraphKind::Init).unwrap();
        assert!(empty_init.edges.is_empty());assert!(empty_init.nodes.values().all(|n|n.kind==NodeKind::Init));
        assert!(graphs.iter().any(|g|g.graph_id==GraphId::new(900)));
        assert_eq!(resolved,guard);assert_eq!(authored,original);
        assert!(import_native_preparation(&authored).is_err(),"A missing real dependency must not be fabricated from a cached owner");
    }

}

/// Explicit initial source linking only. A prior Unit signature requires its
/// real sidecar/refactor owner, never new IDs guessed from source names.
pub fn prepare_initial_unit_link_graphs(script:&Script,previous:&[GraphDocument],allocate:impl FnMut()->Result<String,String>)->Result<Vec<GraphDocument>,String>{
    if script.iter().any(|statement|matches!(statement,Statement::Use{..})) {
        return Err("Unit initial linking with imported owners requires explicit source-owner/refactor mapping".into());
    }
    if previous.iter().any(|graph|matches!(graph.kind,GraphKind::UnitFunction{..}) || graph.pins.values().any(|pin|pin.parameter_id.is_some())) {
        return Err("Existing Unit identities require reopening their owning sidecar or an explicit source refactor; initial linking never replaces them".into());
    }
    let prepared=import_script_with_parameter_allocator(script,allocate)?;
    // Reuse the genuine existing old/new node+pin mapping and graph-ID floor.
    reconcile_prepared_import(prepared,previous,true)
}

/// The same explicit initial Link producer with actual nominal Source resources.
/// Existing ordinary Unit owners still require their own sidecar/refactor path.
/// Library member parameter IDs come from Source scopes, never this allocator.
pub fn prepare_initial_unit_link_graphs_scoped1027(script:&Script,previous:&[GraphDocument],resources:&crate::LibraryGraphResources1027<'_>,mut allocate:impl FnMut()->Result<String,String>)->Result<Vec<GraphDocument>,String>{
    let actual=script.iter().filter_map(|s|if let Statement::FunctionLibrary{scope}=s{Some(scope.clone())}else{None}).collect::<Vec<_>>();
    if actual.as_slice()!=resources.scopes(){return Err("Initial Link resources differ from the actual original Library Source scopes".into())}
    if actual.is_empty(){return prepare_initial_unit_link_graphs(script,previous,allocate)}
    if script.iter().any(|s|matches!(s,Statement::Use{..})){return Err("Unit initial linking with imported owners requires explicit source-owner/refactor mapping".into())}
    let ordinary_previous=previous.iter().filter(|g|g.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_none()).cloned().collect::<Vec<_>>();
    if ordinary_previous.iter().any(|g|matches!(g.kind,GraphKind::UnitFunction{..})||g.pins.values().any(|p|p.parameter_id.is_some())){return Err("Existing Unit identities require reopening their owning sidecar or an explicit source refactor; initial linking never replaces them".into())}
    let reserved=actual.iter().flat_map(|scope|scope.members()).map(|member|GraphId::new(member.graph_id())).collect::<std::collections::BTreeSet<_>>();
    if reserved.len()!=actual.iter().map(|scope|scope.members().len()).sum::<usize>(){return Err("Initial Source has two nominal Library owners for one project GraphId".into())}
    if ordinary_previous.iter().any(|g|reserved.contains(&g.graph_id)){return Err("Initial source presentation collides with an original nominal Library member identity".into())}
    let ordinary=script.iter().filter(|s|!matches!(s,Statement::FunctionLibrary{..})).cloned().collect::<Script>();
    let prepared=import_script_selected_library1027(&ordinary,&Default::default(),false,Some(script),&[],&mut allocate,None,Some(resources),None)?;
    let context=prepared.clone();let mut retained=reconcile_prepared_import_context_library1027(prepared,&ordinary_previous,false,&context,Some(resources),&reserved)?;
    retained.extend(previous.iter().filter(|g|g.library1027.as_ref().and_then(|m|m.owner.as_ref()).is_some()).cloned());
    // The normal scoped importer owns the final actual member + ordinary
    // cohort, preserving the real prepared Unit pins rather than allocating.
    crate::import_library_program1027(script,resources,&retained)
}

#[cfg(test)]
#[path="library_initial_unit_tests1027.rs"]
mod library_initial_unit_tests1027;

impl Builder<'_> {
    /// Shared actual producer reconstruction; native_active belongs to caller phase.
    fn native_operand_steps(&mut self, steps: &[rvn_parser::ConsumerStep], x:f64, y:f64)->Result<(),String>{
                for step in steps {
                    let (inspection,id,value)=match step {
                        rvn_parser::ConsumerStep::Capture{producer,value}=>(false,producer,value),
                        rvn_parser::ConsumerStep::InspectTextTable{producer,value}=>(true,producer,value),
                    };
                    if !self.native_active.insert(id.clone()) {return Err("Duplicate producer in native consumer".into());}
                    // Do not expose the current identity to its own expression.
                    self.native_active.remove(id);
                    let (identity,legacy_port)=crate::native_frame_preparation::decode_producer(id)?;
                    if identity.exec_links_v1!=crate::native_frame_preparation::uses_exec_links(&self.graph)?{return Err("ExecLinksV1 actual producer format differs from its source root".into());}
                    let source=if let Some((prior,kind,source))=self.native_producers.get(id) {
                        if prior!=value||*kind!=inspection {return Err("Shared native producer changed its source semantics".into());}
                        *source
                    } else {
                        let source=if let Some(port)=&legacy_port {
                            if inspection {return Err("Legacy terminal cannot be an inspect step".into());}
                            let Expr::Call{name,args}=value else {return Err("Legacy terminal requires its original stateless intrinsic".into());};
                            if crate::text_table_nodes::inspector_output(name)!=Some(port.as_str()) || args.len()!=1 {return Err("Legacy terminal role/intrinsic mismatch".into());}
                            let matching=self.native_nodes.iter().find(|(_,prior)|prior.node==identity.node);
                            let node=if let Some((old,prior))=matching {
                                if prior!=&identity || !crate::native_frame_preparation::inspector_contract(&self.graph,*old).is_legacy() {return Err("Legacy terminal aliases a different source node/contract".into());}
                                let prior=self.native_producers.values().find(|(_,_,source)|source.node==*old).ok_or("Legacy source producer missing")?;
                                if !matches!(&prior.0,Expr::Call{args:old_args,..} if old_args==args) {return Err("Legacy inspector ports changed their shared Text source".into());}
                                *old
                            } else {
                                let node=self.node(NodeKind::GetStringTableIdAndKey,[x-260.0,y+160.0])?;
                                let connected=identity.ports.iter().find(|(key,_,_)|key=="text").ok_or("Legacy inspector Text role missing")?.2.is_some();
                                if connected {self.strict_input(node,"text",&args[0],ValueType::InterpolatedText)?;}
                                else if !matches!(&args[0],Expr::Call{name,args} if name=="__rvn_empty_text"&&args.is_empty()) {return Err("Unconnected Legacy inspector requires empty Text source".into());}
                                node
                            };
                            ExpressionRef{node,output:self.graph.pin_by_key(node,port).ok_or("Legacy output port missing")?.id}
                        } else if identity.kind==NodeKind::ForEach&&identity.ports.iter().any(|(role,_,_)|role=="array_element"){
                            if inspection{return Err("ArrayElement cannot be an inspect step".into());}
                            self.native_array_element_source(&identity,value,[x-260.0,y+160.0])?
                        } else if inspection {
                            if identity.kind!=NodeKind::GetStringTableIdAndKey {return Err("Inspect step must be one real three-output inspector".into());}
                            let node=self.node(NodeKind::GetStringTableIdAndKey,[x-260.0,y+160.0])?;
                            self.graph.nodes.get_mut(&node).unwrap().inspector_contract=Some(crate::InspectorEvaluationContract::NativeFrameV1);
                            let connected=identity.ports.iter().find(|(key,_,_)|key=="text").ok_or("Inspector Text role missing")?.2.is_some();
                            if connected {self.strict_input(node,"text",value,ValueType::InterpolatedText)?;}
                            else if !matches!(value,Expr::Call{name,args} if name=="__rvn_empty_text"&&args.is_empty()) {return Err("Unconnected inspector requires empty Text source".into());}
                            ExpressionRef{node,output:self.graph.pin_by_key(node,"valid").unwrap().id}
                        } else if matches!(identity.kind,NodeKind::SetVariable|NodeKind::LocalVariable|NodeKind::WidgetCreate) {
                            let Expr::Var(name)=value else{return Err("Assignment output must read its current variable, not replay the RHS".into());};
                            let (node,prior)=self.native_nodes.iter().find(|(_,prior)|prior.node==identity.node)
                                .ok_or("Assignment output requires its actual already reconstructed assignment node")?;
                            if prior!=&identity||self.graph.variable_node_name(*node).as_deref()!=Some(name.as_str()) {
                                return Err("Assignment output differs from its actual source variable/ports".into());
                            }
                            ExpressionRef{node:*node,output:self.graph.pin_by_key(*node,"value_out").ok_or("Assignment value_out missing")?.id}
                        } else if identity.kind==NodeKind::MathNegate {
                            // The authenticated producer identity selects an
                            // operator, even when its actual operand is inline.
                            // Signed Literal producers keep their distinct path.
                            let Expr::Neg(operand)=value else{return Err("Native MathNegate requires its actual negation expression".into());};
                            let node=self.node(NodeKind::MathNegate,[x-260.0,y+160.0])?;
                            self.input_expr(node,"value",operand)?;self.value_output(node)?
                        } else if identity.kind==NodeKind::ConvertBoolToInt {
                            // Its authenticated capture may use a real inline
                            // Bool. Reconstruct that pin default without adding
                            // an undeclared provisional Literal producer.
                            let Expr::Call{name,args}=value else{return Err("Native BoolToInt requires its actual strict Bool intrinsic".into());};
                            let [operand]=args.as_slice() else{return Err("Native BoolToInt strict Bool intrinsic arity".into());};
                            if name!="__rvn_bool_to_int"{return Err("Native BoolToInt cannot replace its strict Bool intrinsic".into());}
                            if literal(operand).is_some_and(|value|!matches!(value,PropertyValue::Bool(_))){return Err("Native BoolToInt expects its actual Bool operand".into());}
                            let node=self.node(NodeKind::ConvertBoolToInt,[x-260.0,y+160.0])?;
                            self.input_expr(node,"value",operand)?;self.value_output(node)?
                        } else if identity.kind==NodeKind::ConvertBoolToFloat {
                            let Expr::BinOp{op:BinOpKind::Add,left,right}=value else{return Err("Native BoolToFloat requires its actual strict Bool intrinsic plus float zero".into());};
                            if !matches!(right.as_ref(),Expr::Float(zero)if zero.to_bits()==0.0f32.to_bits()){return Err("Native BoolToFloat cannot change its actual float-zero operand".into());}
                            let Expr::Call{name,args}=left.as_ref() else{return Err("Native BoolToFloat requires its actual strict Bool intrinsic".into());};
                            let [operand]=args.as_slice() else{return Err("Native BoolToFloat strict Bool intrinsic arity".into());};
                            if name!="__rvn_bool_to_int"{return Err("Native BoolToFloat cannot replace its strict Bool intrinsic".into());}
                            if literal(operand).is_some_and(|value|!matches!(value,PropertyValue::Bool(_))){return Err("Native BoolToFloat expects its actual Bool operand".into());}
                            let node=self.node(NodeKind::ConvertBoolToFloat,[x-260.0,y+160.0])?;
                            self.input_expr(node,"value",operand)?;self.value_output(node)?
                        } else if identity.kind==NodeKind::ConvertIntToFloat {
                            let Expr::BinOp{op:BinOpKind::Add,left,right}=value else{return Err("Native IntToFloat requires its actual integer addition with float zero".into());};
                            if !matches!(right.as_ref(),Expr::Float(zero)if zero.to_bits()==0.0f32.to_bits()){return Err("Native IntToFloat cannot change its actual float-zero operand".into());}
                            let node=self.node(NodeKind::ConvertIntToFloat,[x-260.0,y+160.0])?;
                            self.input_expr(node,"value",left)?;self.value_output(node)?
                        } else if identity.kind==NodeKind::ConvertNumberToText {
                            let Expr::BinOp{op:BinOpKind::Add,left,right}=value else{return Err("Native NumberToText requires its actual emitted empty-string addition".into());};
                            if !matches!(left.as_ref(),Expr::Str(text)if text.is_empty()){return Err("Native NumberToText cannot change its emitted numeric formatting expression".into());}
                            let node=self.node(NodeKind::ConvertNumberToText,[x-260.0,y+160.0])?;
                            self.input_expr(node,"value",right)?;self.value_output(node)?
                        } else if matches!(identity.kind,NodeKind::ConvertStringToText|NodeKind::ConvertTextToString|NodeKind::ConvertNameToString) && matches!(value,Expr::Call{name,args}if name=="__rvn_captured_v1"&&matches!(args.as_slice(),[Expr::Str(_)])) {
                            let source=self.expression(value,[x-520.0,y+160.0])?;
                            let node=self.node(identity.kind,[x-260.0,y+160.0])?;
                            self.property(node,"legacy_passthrough",PropertyValue::Bool(true))?;
                            self.connect_expression(source,node,"value")?;self.value_output(node)?
                        } else if identity.kind==NodeKind::TextValue && matches!(value,Expr::Str(_)) {
                            let Expr::Str(template)=value else {unreachable!()};
                            let node=self.node(NodeKind::TextValue,[x-260.0,y+160.0])?;
                            self.property(node,"value",PropertyValue::String(template.clone()))?;
                            self.value_output(node)?
                        } else if identity.kind==NodeKind::CharacterValue {
                            let Expr::Str(id)=value else {return Err("Character capture must retain its actual literal identity".into());};
                            let node=self.node(identity.kind,[x-260.0,y+160.0])?;
                            self.property(node,"character",PropertyValue::String(id.clone()))?;
                            if let Some(output)=identity.static_inputs.get("sprite") {
                                let sources:Vec<_>=self.native_producers.iter().filter(|(id,(_,inspection,source))|
                                    !inspection&&self.native_active.contains(*id)&&self.native_nodes.get(&source.node).is_some_and(|actual|
                                        actual.ports.iter().any(|(key,pin,_)|pin==output&&self.graph.pin_by_key(source.node,key).is_some_and(|pin|pin.id==source.output))))
                                    .map(|(_,(_,_,source))|*source).collect();
                                let [source]=sources.as_slice()else{return Err("Character Sprite requires one active actual source output".into());};
                                self.connect_expression(*source,node,"sprite")?;
                            }
                            self.value_output(node)?
                        } else if identity.kind==NodeKind::PositionValue {
                            let Expr::Str(position)=value else{return Err("Position capture requires its actual literal position".into());};
                            let node=self.node(identity.kind,[x-260.0,y+160.0])?;self.property(node,"position",PropertyValue::String(position.clone()))?;self.value_output(node)?
                        } else if identity.kind==NodeKind::LabelValue {
                            let Expr::Str(label)=value else {return Err("Label capture must retain its actual literal target".into());};
                            let node=self.node(identity.kind,[x-260.0,y+160.0])?;
                            self.property(node,"label",PropertyValue::String(label.clone()))?;self.value_output(node)?
                        } else if matches!(identity.kind,NodeKind::MusicAsset|NodeKind::SoundEffectAsset|NodeKind::VoiceAsset|NodeKind::SceneAsset|NodeKind::SpriteAsset|NodeKind::CinematicAsset|NodeKind::HoverImageAsset) {
                            let Expr::Str(path)=value else {return Err("Audio asset capture must retain its actual literal path".into());};
                            let node=self.node(identity.kind,[x-260.0,y+160.0])?;self.property(node,"path",PropertyValue::String(path.clone()))?;self.value_output(node)?
                        } else if matches!(identity.kind,NodeKind::TransitionNone|NodeKind::TransitionFade|NodeKind::TransitionDissolve|NodeKind::TransitionSlideLeft|NodeKind::TransitionSlideRight|NodeKind::TransitionSlideUp|NodeKind::TransitionSlideDown|NodeKind::TransitionZoomIn|NodeKind::TransitionZoomOut|NodeKind::TransitionWipe|NodeKind::TransitionBlur) {
                            let Expr::Str(raw)=value else {return Err("Transition capture must retain its actual literal".into());};
                            let value=PropertyValue::String(raw.clone());
                            if crate::document::transition_kind(&value)!=identity.kind || (identity.kind==NodeKind::TransitionNone && raw!="none") {
                                return Err("Transition capture kind/value mismatch".into());
                            }
                            let node=self.node(identity.kind,[x-260.0,y+160.0])?;
                            if identity.kind!=NodeKind::TransitionNone {
                                let duration=crate::document::transition_duration(&value).ok_or("Transition capture duration missing")?;
                                self.property(node,"duration_ms",PropertyValue::Int(duration))?;
                            }
                            self.value_output(node)?
                        } else if identity.kind==NodeKind::Literal {
                            // The declared node, not value equality, selects the
                            // editable representation. Legacy materialization can
                            // produce a real Literal containing a StringList;
                            // expression_node would otherwise make a ListLiteral.
                            let value=literal(value).ok_or("Native Literal requires its actual literal value, not a dynamic expression")?;
                            let node=self.node(NodeKind::Literal,[x-260.0,y+160.0])?;
                            self.property(node,"value",value)?;self.value_output(node)?
                        } else if matches!(identity.kind,NodeKind::ListLiteral|NodeKind::FunctionCall){
                            self.native_value_sequence(&identity,value,[x-260.0,y+160.0])?
                        } else if identity.kind==NodeKind::Reroute {
                            let node=self.node(NodeKind::Reroute,[x-260.0,y+160.0])?;
                            self.input_expr(node,"value",value)?;self.value_output(node)?
                        } else {self.expression(value,[x-260.0,y+160.0])?};
                        if self.graph.nodes[&source.node].kind!=identity.kind {return Err("Native producer kind cannot be reconstructed from this expression; source refused".into());}
                        if self.native_nodes.get(&source.node).is_some_and(|prior|prior!=&identity) {return Err("Two native source identities alias one reconstructed node".into());}
                        self.native_nodes.insert(source.node,identity);
                        self.native_producers.insert(id.clone(),(value.clone(),inspection,source));source
                    };
                    let _=source;self.native_active.insert(id.clone());
                }
        Ok(())
    }
}

pub(crate) fn import_library_scope1027<'a>(script:&Script,resolved:&Script,resources:&'a crate::LibraryGraphResources1027<'a>,member:Option<&rvn_parser::function_library_asset1023::QualifiedLibraryFunction>,previous:&[GraphDocument])->Result<Vec<GraphDocument>,String>{
 let ids=member.map(|m|resources.member(m).map(|m|m.parameter_ids().to_vec())).transpose()?.unwrap_or_default();let mut ids=ids.into_iter();
 // These IDs already belong to the actual scoped Source declaration. This is
 // an injected immutable source sequence, not a random identity allocator.
 let mut original=||ids.next().map(|p|p.as_str().to_owned()).ok_or_else(||"Scoped import cannot allocate or guess a missing parameter identity".into());
 let mut seeds=Vec::new();
 if let Some(target)=member{let source=resources.member(target)?;if let Statement::UnitFunction{name,parameters,..}=source.declaration(){
  // The Source scope supplies this exact ordered ID/name relation, including
  // an explicit rename/reorder. It is never recovered from old pin positions.
  let mut seed=GraphDocument::new(GraphId::new(target.graph_id),GraphKind::UnitFunction{name:name.clone()});let entry=seed.add_catalog_node(NodeKind::FunctionEntry,[0.,0.]).map_err(err)?;
  for(id,name)in source.parameter_ids().iter().zip(parameters){let identity=crate::ParameterId::parse(id.as_str())?;let pin=seed.add_pin(entry,format!("parameter_{}",identity.as_str()),name,PinDirection::Output,ValueType::Any,PinCardinality::Many).map_err(err)?;seed.pins.get_mut(&pin).unwrap().parameter_id=Some(identity);}seeds.push(seed);
 }}
 let owners=if seeds.is_empty(){previous}else{&seeds};
 import_script_selected_library1027(script,&Default::default(),false,Some(resolved),owners,&mut original,None,Some(resources),member)
}
impl Builder<'_>{
 fn import_qualified_call1027(&mut self,node:NodeId,call:&rvn_parser::function_library_program1023::QualifiedLibraryCall,mode:rvn_parser::function_library_program1023::LibraryCallMode)->Result<(),String>{
  let resources=self.library1027.ok_or("Qualified call import requires original scoped source/resources; no legacy lowering")?;
  resources.resolve(&self.graph,call.reference(),mode,call.arguments())?;
  let member=resources.member(call.reference())?;let ordered=member.parameter_ids().iter().map(|id|call.arguments().iter().find(|(p,_)|p==id).cloned().ok_or_else(||"Stale/foreign original Library ParameterId".to_owned())).collect::<Result<Vec<_>,_>>()?;
  let name=member.name().to_owned();self.property(node,"function",PropertyValue::String(name))?;
  if mode==rvn_parser::function_library_program1023::LibraryCallMode::Value{self.graph.resize_value_inputs(node,ordered.len()).map_err(err)?;}
  let mut arguments=Vec::new();
  for(index,(parameter,expression))in ordered.iter().enumerate(){
   let(key,pin)=if mode==rvn_parser::function_library_program1023::LibraryCallMode::Value{let key=format!("item_{index}");let pin=self.graph.pin_by_key(node,&key).ok_or("Actual qualified value input missing")?.id;(key,pin)}else{
    let key=format!("argument_{}",parameter.as_str());let pin=self.graph.add_pin(node,&key,&member.parameters()[index],PinDirection::Input,ValueType::Any,PinCardinality::One).map_err(err)?;self.graph.pins.get_mut(&pin).ok_or("Real qualified Unit input disappeared")?.parameter_id=Some(crate::ParameterId::parse(parameter.as_str())?);(key,pin)
   };
    // Both callable families expose the current nominal parameter name, not
    // the generic value-sequence catalogue label. Identity remains ParameterId.
    self.graph.pins.get_mut(&pin).ok_or("Actual qualified argument pin disappeared")?.label=member.parameters()[index].clone();
    self.input_expr(node,&key,expression)?;arguments.push((parameter.clone(),pin));
  }
  self.graph.schema_version=crate::GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION;
  self.graph.library1027.get_or_insert_with(Default::default).calls.insert(node,crate::QualifiedLibraryCallBinding1027{target:call.reference().clone(),arguments});
  if mode==rvn_parser::function_library_program1023::LibraryCallMode::Value{self.graph.refresh_library_result_type1027(node,resources)?;}Ok(())
 }
}
