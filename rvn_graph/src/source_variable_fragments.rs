//! Narrative fragments inherit their caller's activation. A definition rename
//! must account for those dependencies before any model or source is changed.
use crate::{GraphDocument, GraphKind, NarrativeMetadata, NodeKind, PropertyValue};
use rvn_parser::Statement;
use std::collections::{BTreeMap, BTreeSet};

fn narrative_dependencies(script: &[Statement], targets: &mut BTreeSet<String>, locals: &mut BTreeSet<String>) {
    for statement in script {
        match statement {
            Statement::LocalVar { name, .. } => { locals.insert(name.clone()); }
            Statement::Call { target } | Statement::Jump { target } => {
                targets.insert(target.clone());
            }
            Statement::While { body, .. }
            | Statement::ForEach { body, .. }
            | Statement::ForLoopWithBreak { body, .. }
            | Statement::ScheduledConsumer { body, .. } => narrative_dependencies(body, targets, locals),
            Statement::If { then_branch, else_branch, .. } => {
                narrative_dependencies(then_branch, targets, locals);
                narrative_dependencies(else_branch, targets, locals);
            }
            Statement::Sequence { branches } => {
                for branch in branches { narrative_dependencies(branch, targets, locals); }
            }
            Statement::Choice { options } => {
                for option in options { narrative_dependencies(&option.body, targets, locals); }
            }
            Statement::Imagemap { hotspots, .. } => {
                for hotspot in hotspots { narrative_dependencies(&hotspot.body, targets, locals); }
            }
            // Functions, screens and handlers have lexical bindings. Their
            // expression calls do not inherit the narrative activation.
            _ => {}
        }
    }
}

fn refers_to(graph: &GraphDocument, name: &str, project: &[GraphDocument]) -> Result<bool, String> {
    let mut before = graph.clone();
    if !before.variables.contains_key(name) {
        before.add_variable(name, crate::ValueType::Any, PropertyValue::Int(0))
            .map_err(|error| error.to_string())?;
    }
    let mut probe = "RvNFragmentReferenceProbe".to_owned();
    while before.variables.contains_key(&probe) { probe.push('_'); }
    let mut after = before.clone();
    after.rename_variable_references(name, probe).map_err(|error| error.to_string())?;
    Ok(crate::unit_graph_source1003::compile_scope(&before,project).map_err(|error| error.to_string())?.ast
        != crate::unit_graph_source1003::compile_scope(&after,project).map_err(|error| error.to_string())?.ast)
}

// One dependency algorithm over owned graph views and actual read-only AST.
// An AST view has no graph/port identity and is never compiled or published.
#[derive(Clone, Copy)]
enum LabelView<'a> {
    Graph(&'a GraphDocument, bool, &'a [GraphDocument]),
    Ast(&'a super::variable_declarations::ReadonlyScope),
}
impl LabelView<'_> {
    fn owned_id(self)->Option<crate::GraphId> {match self {Self::Graph(g,false,_)=>Some(g.graph_id),_=>None}}
    fn imported(self)->bool { !matches!(self,Self::Graph(_,false,_)) }
    fn scene_entry(self)->bool {match self {
        Self::Graph(g,_,_)=>g.narrative==Some(NarrativeMetadata::SceneEntry),
        Self::Ast(scope)=>matches!(scope.ast.first(),Some(Statement::SceneLabel {..})),
    }}
    fn ast(self)->Result<rvn_parser::Script,String> {match self {
        Self::Graph(g,_,project)=>crate::unit_graph_source1003::compile_scope(g,project).map(|s|s.ast).map_err(|e|e.to_string()),
        Self::Ast(scope)=>Ok(scope.ast.clone()),
    }}
    fn refers_to(self,name:&str)->Result<bool,String> {match self {
        Self::Graph(g,_,project)=>refers_to(g,name,project),
        Self::Ast(scope)=>Ok(super::variable_declarations::readonly_scope_mentions(scope,name)),
    }}
}

pub(super) fn check(
    baseline: &[GraphDocument], edited: &[GraphDocument], readonly: &[GraphDocument],
    readonly_scopes: &[super::variable_declarations::ReadonlyScope],
) -> Result<(), String> {
    let mut renames = Vec::new();
    for before in baseline.iter().filter(|graph| matches!(graph.kind, GraphKind::Label { .. })) {
        let Some(after) = edited.iter().find(|graph| graph.graph_id == before.graph_id) else { continue; };
        for node in before.nodes.values().filter(|node| node.kind == NodeKind::LocalVariable) {
            let Some(current) = after.nodes.get(&node.id).filter(|node| node.kind == NodeKind::LocalVariable) else { continue; };
            let (Some(PropertyValue::String(old)), Some(PropertyValue::String(new))) =
                (node.properties.get("name"), current.properties.get("name")) else { continue; };
            if old != new { renames.push((after.graph_id, old.clone(), new.clone())); }
        }
    }
    if renames.is_empty() { return Ok(()); }

    // Imported GraphIds have a separate namespace. Resolve narrative targets
    // by their validated labels, never by an imported numeric graph identity.
    let graphs: Vec<_> = edited.iter().filter(|g|matches!(g.kind,GraphKind::Label {..})).map(|g|LabelView::Graph(g,false,edited))
        .chain(readonly.iter().filter(|g|matches!(g.kind,GraphKind::Label {..})).map(|g|LabelView::Graph(g,true,readonly)))
        .chain(readonly_scopes.iter().filter(|s|matches!(s.ast.first(),Some(Statement::Label {..}|Statement::SceneLabel {..}))).map(LabelView::Ast)).collect();
    let mut labels = BTreeMap::new();
    let mut dependencies = Vec::new();
    let mut local_declarations = Vec::new();
    for (index, view) in graphs.iter().enumerate() {
        let ast = view.ast()?;
        for statement in &ast {
            if let Statement::Label { name } | Statement::SceneLabel { name } = statement {
                if labels.insert(name.clone(), index).is_some() { return Err(format!("Duplicate narrative dependency label: {name}")); }
            }
        }
        let mut targets = BTreeSet::new();
        let mut locals = BTreeSet::new();
        narrative_dependencies(&ast, &mut targets, &mut locals);
        dependencies.push(targets);
        local_declarations.push(locals);
    }
    for (owner, old, new) in renames {
        let root = graphs.iter().position(|view| view.owned_id() == Some(owner))
            .ok_or("Missing variable declaration owner")?;
        // Analyse the original declaration name and execution order. In the
        // edited graph the Local already has its new name, so it cannot tell
        // us when the old binding became visible to the caller's fragments.
        let before = baseline.iter().find(|graph| graph.graph_id == owner)
            .ok_or("Missing original variable declaration owner")?;
        let targets = crate::variable_rename::narrative_calls_with_local_binding(before, &old, baseline)
            .map_err(|error| error.to_string())?;
        let mut pending: Vec<_> = match targets {
            Some(targets) => targets.into_iter().collect(),
            None => dependencies[root].iter().cloned().map(|target| (target, true)).collect(),
        };
        // The same fragment may be reached before and after Local. Visiting it
        // with a global binding must not hide a later local dependency.
        let mut visited = BTreeSet::from([(root, false), (root, true)]);
        while let Some((target, inherited_local)) = pending.pop() {
            let Some(index) = labels.get(&target).copied() else { continue; };
            let view = graphs[index];
            // A child scene receives its own locals; calls returning from it
            // restore the parent's activation. Ordinary labels share locals.
            // A callee can create this activation slot before the caller's
            // declaration. Its Local survives Return and must migrate too.
            let local = inherited_local || local_declarations[index].contains(&old);
            if view.scene_entry() || !visited.insert((index, local)) { continue; }
            if local && view.refers_to(&old)? {
                let ownership = if view.imported() { "read-only imported" } else { "owned" };
                return Err(format!("Cannot rename local '{old}' to '{new}': {ownership} fragment '{target}' still uses the caller's binding. Migrate its affected dependencies together before renaming. No source or document was changed."));
            }
            pending.extend(dependencies[index].iter().cloned().map(|target| (target, local)));
        }
    }
    Ok(())
}
