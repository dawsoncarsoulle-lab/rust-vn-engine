//! Source-aware Widget class preparation. This is neither a runtime instance
//! nor an input/lifecycle grant. A class is never inferred from its child tree.
use crate::{GraphId, GraphKind, SourceProject};
use rvn_parser::{widget_asset::{self, LocatorCase, WidgetBlueprintV1, WidgetIndexInput}, ParsedProgram, Statement, StatementOrigin};
use std::{collections::{BTreeMap, BTreeSet}, path::{Component, Path}, sync::Arc};

/// Already-read project metadata. Physical-root, junction, CAS and writer
/// admission remain at the existing loader boundary; this query writes nothing.
pub struct WidgetRuntimeAsset<'a> { pub locator: &'a str, pub bytes: &'a [u8] }
/// The real, immutable source-first model for this project-relative source.
/// A caller cannot substitute a loose GraphDocument carrying invented IDs.
pub struct WidgetRuntimeSource<'a> { pub source: &'a str, pub project: &'a SourceProject }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WidgetDefinitionKind { Screen, Handler, Function }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedWidgetMember {
    graph: GraphId,
    kind: WidgetDefinitionKind,
    definition: String,
    root_index: usize,
    origin: StatementOrigin,
}
impl PreparedWidgetMember {
    pub fn graph(&self) -> GraphId { self.graph }
    pub fn kind(&self) -> &WidgetDefinitionKind { &self.kind }
    pub fn definition(&self) -> &str { &self.definition }
    pub fn root_index(&self) -> usize { self.root_index }
    pub fn origin(&self) -> &StatementOrigin { &self.origin }
}

/// Opaque admission result, not deserializable and not constructible from a
/// UUID, alias, filename, [] return value or cached editor presentation alone.
#[derive(Clone, Debug)]
pub struct PreparedWidgetClass {
    blueprint: WidgetBlueprintV1,
    locator: String,
    descriptor_bytes: Arc<[u8]>,
    members: Vec<PreparedWidgetMember>,
}
impl PreparedWidgetClass {
    pub fn uuid(&self) -> &str { &self.blueprint.uuid }
    pub fn source(&self) -> &str { &self.blueprint.source }
    pub fn locator(&self) -> &str { &self.locator }
    pub fn descriptor_bytes(&self) -> &[u8] { &self.descriptor_bytes }
    pub fn members(&self) -> &[PreparedWidgetMember] { &self.members }
    pub fn screen(&self) -> &PreparedWidgetMember {
        // Construction validates exactly one real member matching screen_graph.
        self.members.iter().find(|member| member.kind == WidgetDefinitionKind::Screen).expect("admitted Widget class Screen")
    }
}

/// One frozen program and its admitted class descriptors. Instances and saved
/// acquired focus never live here. No generated code or additional VM exists.
#[derive(Clone, Debug)]
pub struct PreparedWidgetClasses {
    program: Arc<ParsedProgram>,
    index: BTreeMap<String, String>,
    classes: BTreeMap<String, PreparedWidgetClass>,
}
impl PreparedWidgetClasses {
    pub fn program(&self) -> &ParsedProgram { &self.program }
    pub fn index(&self) -> &BTreeMap<String, String> { &self.index }
    pub fn classes(&self) -> &BTreeMap<String, PreparedWidgetClass> { &self.classes }
    pub fn class(&self, uuid: &str) -> Option<&PreparedWidgetClass> { self.classes.get(uuid) }
    pub fn matches_program(&self, actual: &ParsedProgram) -> bool { self.program.as_ref() == actual }
}
fn locator_key(value: &str, case: LocatorCase) -> String {
    match case { LocatorCase::Sensitive => value.to_owned(), LocatorCase::Insensitive => value.to_lowercase() }
}
fn definition(kind: &GraphKind) -> Option<(WidgetDefinitionKind, &str)> {
    match kind {
        GraphKind::Screen { name } => Some((WidgetDefinitionKind::Screen, name)),
        GraphKind::Handler { name } => Some((WidgetDefinitionKind::Handler, name)),
        GraphKind::Function { name } => Some((WidgetDefinitionKind::Function, name)),
        _ => None,
    }
}
fn is_definition(statement: &Statement, kind: &WidgetDefinitionKind, name: &str) -> bool {
    match (kind, statement) {
        (WidgetDefinitionKind::Screen, Statement::Screen { name: actual, .. })
        | (WidgetDefinitionKind::Handler, Statement::Handler { name: actual, .. })
        | (WidgetDefinitionKind::Function, Statement::Function { name: actual, .. }) => actual == name,
        _ => false,
    }
}

/// Validate a complete frozen admission set before returning any class. The
/// `program` is the loader's existing resolved narrative, not a new evaluator.
/// All member IDs are resolved only through the matching real SourceProject.
/// Exact frozen paths, bytes, AST and spans are checked; equivalent source at a
/// different locator is not accepted. This function performs no filesystem write.
pub fn prepare_widget_runtime_classes(
    project_root: &Path,
    program: &ParsedProgram,
    index: WidgetIndexInput<'_>,
    case: LocatorCase,
    assets: &[WidgetRuntimeAsset<'_>],
    sources: &[WidgetRuntimeSource<'_>],
) -> Result<PreparedWidgetClasses, String> {
    if !project_root.is_absolute() || project_root.components().any(|part| matches!(part, Component::CurDir | Component::ParentDir)) {
        return Err("Widget class admission requires the actual normalized absolute project root".into());
    }
    if assets.len() > 256 || sources.len() > 256 { return Err("Widget class admission exceeds its source/asset count budget".into()); }
    let index = widget_asset::validate_index(index, case)?;
    if index.len() > 256 || index.len() != assets.len() { return Err("Widget class assets must match the complete bounded index".into()); }
    // Reuses the actual source/origin validator, including its 16 MiB bound.
    // Legacy [] remains a legacy Screen unless a validated descriptor admits it.
    crate::native_source_binding::validate_frozen_source_graphs(program)?;
    let mut asset_map = BTreeMap::new();
    let mut bytes = 0usize;
    for asset in assets {
        widget_asset::validate_relative_locator(asset.locator, Some("rvnwidget"))?;
        bytes = bytes.checked_add(asset.bytes.len()).and_then(|value| value.checked_add(asset.locator.len())).filter(|value| *value <= 4 * 1024 * 1024).ok_or("Widget class descriptor budget exceeded")?;
        if asset_map.insert(locator_key(asset.locator, case), asset).is_some() { return Err("Widget class asset locator is ambiguous".into()); }
    }
    let mut source_map = BTreeMap::new();
    for source in sources {
        widget_asset::validate_relative_locator(source.source, Some("rvn"))?;
        if source_map.insert(source.source, source.project).is_some() { return Err("Widget source model is ambiguous".into()); }
    }
    let mut checked_sources = BTreeSet::new();
    let mut exact_definitions = BTreeSet::new();
    let mut claimed_graphs = BTreeSet::new();
    let mut claimed_definitions = BTreeSet::new();
    let mut member_count = 0usize;
    let mut classes = BTreeMap::new();
    for (uuid, locator) in &index {
        let asset = asset_map.get(&locator_key(locator, case)).ok_or("Indexed Widget descriptor was not frozen")?;
        let blueprint = WidgetBlueprintV1::decode(asset.bytes)?;
        if &blueprint.uuid != uuid { return Err("Widget descriptor UUID differs from its actual index entry".into()); }
        let project = source_map.get(blueprint.source.as_str()).ok_or("Widget source has no actual source-first project")?;
        let path = project_root.join(&blueprint.source);
        let actual_source = program.sources.iter().find(|source| source.path.as_ref() == Some(&path)).ok_or("Widget source is absent from the actual frozen program closure")?;
        if project.source() != actual_source.source { return Err("Widget authoring model differs from the frozen source bytes".into()); }
        if checked_sources.insert(blueprint.source.clone()) {
            // A dirty/import-stale model cannot donate graph identities. Its
            // actual closure must be an exact subset of the one frozen above.
            let current = project.resolved_program()?;
            crate::native_source_binding::validate_frozen_source_graphs(&current)?;
            if current.sources.iter().any(|source| !program.sources.contains(source)) {
                return Err("Widget source imports differ from the frozen program closure".into());
            }
            // Frozen text alone cannot silently change parsing context. Prove
            // each used definition's actual AST and origin, not just its name.
            let current_roots = current.script.iter().zip(&current.origins).map(|(statement, origin)| ((origin.source.as_ref(), origin.span.range.start, origin.span.range.end), (statement, origin))).collect::<BTreeMap<_, _>>();
            if current_roots.len() != current.script.len() { return Err("Widget source repeats a frozen root origin".into()); }
            for (index, (statement, origin)) in program.script.iter().zip(&program.origins).enumerate() {
                if origin.source.as_ref() == Some(&path) && current_roots.get(&(origin.source.as_ref(), origin.span.range.start, origin.span.range.end)).is_some_and(|(actual, site)| *actual == statement && *site == origin) {
                    exact_definitions.insert(index);
                }
            }
        }
        let graph_map = project.graphs().iter().map(|graph| (graph.graph_id, graph)).collect::<BTreeMap<_, _>>();
        if graph_map.len() != project.graphs().len() { return Err("Widget source repeats a real graph identity".into()); }
        member_count = member_count.checked_add(blueprint.members.len()).filter(|count| *count <= 4096).ok_or("Widget class membership budget exceeded")?;
        let mut members = Vec::with_capacity(blueprint.members.len());
        for reference in &blueprint.members {
            let id = GraphId::new(reference.get());
            let graph = graph_map.get(&id).ok_or("Widget descriptor references a missing actual source graph")?;
            let (kind, name) = definition(&graph.kind).ok_or("Widget class members must be Screen/Handler/Function definitions")?;
            if (kind == WidgetDefinitionKind::Screen) != (*reference == blueprint.screen_graph) { return Err("Widget class needs its one declared real Screen and only Handler/Function siblings".into()); }
            if !claimed_graphs.insert((blueprint.source.clone(), id)) { return Err("Two Widget classes claim the same source graph".into()); }
            let matching = program.script.iter().zip(&program.origins).enumerate().filter(|(_, (statement, origin))| origin.source.as_ref() == Some(&path) && is_definition(statement, &kind, name)).collect::<Vec<_>>();
            let [(root_index, (_, origin))] = matching.as_slice() else { return Err("Widget member has no unique definition in its actual frozen source".into()); };
            if !exact_definitions.contains(root_index) { return Err("Widget definition AST/origin differs from its real source-first model".into()); }
            if !claimed_definitions.insert(*root_index) { return Err("Two Widget classes claim the same actual source definition".into()); }
            members.push(PreparedWidgetMember { graph: id, kind, definition: name.to_owned(), root_index: *root_index, origin: (**origin).clone() });
        }
        classes.insert(uuid.clone(), PreparedWidgetClass { blueprint, locator: locator.clone(), descriptor_bytes: Arc::from(asset.bytes), members });
    }
    Ok(PreparedWidgetClasses { program: Arc::new(program.clone()), index, classes })
}

#[cfg(test)]
#[path = "widget_runtime_binding_tests_0950.rs"]
mod tests;
