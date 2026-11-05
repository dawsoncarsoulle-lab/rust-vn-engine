use crate::{
    DiagnosticSeverity, GraphDiagnostic, GraphDocument, GraphKind, GraphNode, GraphPin, NodeId,
    NodeKind, PinDirection, PinId, PropertyValue, ValueType, NarrativeMetadata, GraphId,
};
#[path="codegen_standard_macros_1020.rs"]
mod standard_macros_1020;
use rvn_parser::{Script,scheduled_source::native_loop_contract::index_name as native_loop_index_name};
use std::{cell::{Cell, RefCell}, collections::{BTreeMap, BTreeSet}, ops::Range};

// Independent RVN expansion limits, not Unreal runtime limits.
const MAX_EXECUTION_EMISSIONS: usize = 65_536;
const MAX_EXECUTION_NESTING: usize = 128;
const MAX_GENERATED_SOURCE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub struct TranspiledScript {
    pub source: String,
    pub ast: Script,
}

/// A generated statement's genuine authoring node, or a source-span fallback
/// for generated project declarations and control-flow sentinels. Children
/// follow the parser's lexical AST order; no synthetic graph/node ID is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphStatementOrigin {
    pub graph: Option<GraphId>,
    pub node: Option<NodeId>,
    pub span: rvn_parser::StatementSpan,
    pub children: Vec<GraphStatementOrigin>,
}

#[derive(Debug)]
pub struct TranspiledScriptWithOrigins {
    pub source: String,
    pub ast: Script,
    pub context: rvn_parser::NarrativeContext,
    pub origins: Vec<GraphStatementOrigin>,
}

pub fn transpile(graph: &GraphDocument) -> Result<TranspiledScript, TranspileError> {
    let compiled = transpile_with_origins(graph)?;
    Ok(TranspiledScript { source: compiled.source, ast: compiled.ast })
}

pub fn transpile_with_origins(graph: &GraphDocument) -> Result<TranspiledScriptWithOrigins, TranspileError> {
    graph.require_native_loop_backend().map_err(TranspileError::GeneratedSourceInvalid)?;
    if !graph.inspector_evaluation.is_legacy() {
        return crate::prepare_native_frame_graph(graph);
    }
    transpile_origins(graph, false)
}

/// Editor analysis must remain available after a type edit retains an
/// incompatible wire. Only link type errors are tolerated here: structure,
/// authored expression checks, expansion limits and generated-source parsing
/// remain mandatory. This is not an executable/publication entry point.
pub(crate) fn transpile_for_binding_analysis(graph: &GraphDocument)
    -> Result<TranspiledScriptWithOrigins, TranspileError>
{
    if !graph.inspector_evaluation.is_legacy() {
        return crate::native_frame_preparation::prepare_native_binding_analysis(graph);
    }
    transpile_origins(graph, true)
}

// Same read-only binding analysis, with actual caller-owned declarations.
// Only incompatible link types are tolerated; structure, Unit ports, parsing,
// expansion limits and existing Native preparation gates still apply.
pub(crate) fn transpile_for_binding_analysis_in_project(graph: &GraphDocument, project: &[GraphDocument])
    -> Result<TranspiledScriptWithOrigins, TranspileError>
{
    graph.require_native_loop_backend().map_err(TranspileError::GeneratedSourceInvalid)?;
    if !graph.inspector_evaluation.is_legacy() {
        return crate::native_frame_preparation::prepare_native_binding_analysis(graph);
    }
    transpile_origins_in_project(graph, true, Some(project))
}

fn transpile_origins(graph: &GraphDocument, allow_incompatible_links: bool)
    -> Result<TranspiledScriptWithOrigins, TranspileError>
{ transpile_origins_in_project(graph,allow_incompatible_links,None) }

pub(crate) fn transpile_with_origins_in_project(graph: &GraphDocument, project: &[GraphDocument]) -> Result<TranspiledScriptWithOrigins, TranspileError> {
    graph.require_native_loop_backend().map_err(TranspileError::GeneratedSourceInvalid)?;
    if !graph.inspector_evaluation.is_legacy() { return crate::prepare_native_frame_graph(graph); }
    transpile_origins_in_project(graph, false, Some(project))
}

pub(crate) fn transpile_unit_in_project(graph:&GraphDocument, project:&[GraphDocument])->Result<TranspiledScriptWithOrigins,TranspileError>{
    transpile_origins_in_project(graph,false,Some(project))
}
fn transpile_origins_in_project<'a>(graph:&'a GraphDocument,allow_incompatible_links:bool,project:Option<&'a [GraphDocument]>)
    ->Result<TranspiledScriptWithOrigins,TranspileError> {
    if !graph.inspector_evaluation.is_legacy() {
        return Err(TranspileError::GeneratedSourceInvalid("NativeFrameV1 is private preparation only: source-authoritative import and activation provenance are not connected".into()));
    }
    if graph.library1027.is_some() { return Err(TranspileError::GeneratedSourceInvalid("Qualified Library graph requires its actual scoped resource compiler".into())); }
    transpile_origins_library1027(graph,allow_incompatible_links,project,None)
}
fn transpile_origins_library1027<'a>(graph:&'a GraphDocument,allow_incompatible_links:bool,project:Option<&'a [GraphDocument]>,library:Option<&'a crate::LibraryGraphResources1027<'a>>) -> Result<TranspiledScriptWithOrigins,TranspileError>{
    transpile_origins_current_context1027(graph,allow_incompatible_links,project,library,None)
}
fn transpile_origins_current_context1027<'a>(graph:&'a GraphDocument,allow_incompatible_links:bool,project:Option<&'a[GraphDocument]>,library:Option<&'a crate::LibraryGraphResources1027<'a>>,current:Option<&'a crate::library_current1027::LibraryCurrentMembers1027<'a>>)->Result<TranspiledScriptWithOrigins,TranspileError>{
    if let Some(resources)=library { resources.validate_graph(graph).map_err(TranspileError::GeneratedSourceInvalid)?; }
    if let Some(current)=current{current.validate_graph1027(graph).map_err(TranspileError::GeneratedSourceInvalid)?;}
    let diagnostics = graph.validate();
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error
            && !(allow_incompatible_links && diagnostic.code == "incompatible_types"))
    {
        return Err(TranspileError::InvalidGraph(diagnostics));
    }

    let mut emitter = Emitter::new(graph);
    emitter.unit_project=project;
    emitter.library1027=library;
    emitter.library_current1027=current;
    let source = emitter.emit_document()?;
    let parsed = rvn_parser::parse_with_origins(&source, graph.narrative_context())
        .map_err(|error| TranspileError::GeneratedSourceInvalid(error.to_string()))?;
    let ranges = emitter.emitted.borrow();
    let mappings: Vec<_> = ranges.iter().map(|(range, node)| (range.clone(), graph.graph_id, *node)).collect();
    let origins = parsed.spans.iter().map(|span| map_statement_origin(span, &mappings)).collect();
    Ok(TranspiledScriptWithOrigins { source, ast: parsed.script, context: graph.narrative_context(), origins })
}

pub(crate) fn map_statement_origin(span: &rvn_parser::StatementSpan, mappings: &[(Range<usize>, GraphId, NodeId)]) -> GraphStatementOrigin {
    let mapping = mappings.iter().filter(|(range, _, _)| range.start <= span.range.start && range.end >= span.range.end)
        .min_by_key(|(range, _, _)| range.end - range.start);
    GraphStatementOrigin { graph: mapping.map(|(_, graph, _)| *graph), node: mapping.map(|(_, _, node)| *node),
        span: span.clone(), children: span.children.iter().map(|child| map_statement_origin(child, mappings)).collect() }
}

/// An authored root has no incoming pin. A reached edge retains the exact
/// destination input; its node is resolved and validated from GraphDocument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExecutionTarget {
    RootEntry(NodeId),
    PinTarget(PinId),
}
impl ExecutionTarget {
    fn node(self, graph: &GraphDocument) -> Result<NodeId, TranspileError> {
        match self {
            Self::RootEntry(id) => {
                let node = graph.nodes.get(&id).ok_or(TranspileError::NodeNotFound(id))?;
                if !matches!(node.kind, NodeKind::Init | NodeKind::Label | NodeKind::FunctionEntry
                    | NodeKind::ScreenEntry | NodeKind::HandlerEntry) {
                    return Err(TranspileError::GeneratedSourceInvalid("RootEntry must be an actual graph entry node".into()));
                }
                Ok(id)
            }
            Self::PinTarget(id) => {
                let pin = graph.pins.get(&id).ok_or(TranspileError::PinNotFound(id))?;
                let node = graph.nodes.get(&pin.node).ok_or(TranspileError::NodeNotFound(pin.node))?;
                if pin.direction != PinDirection::Input || !graph.resolved_pin_is_execution(id)
                    || !node.pins.contains(&id) {
                    return Err(TranspileError::GeneratedSourceInvalid("Execution target must retain its actual owned input pin".into()));
                }
                Ok(node.id)
            }
        }
    }
    fn input<'a>(self, graph: &'a GraphDocument) -> Result<Option<&'a GraphPin>, TranspileError> {
        self.node(graph)?;
        Ok(match self {
            Self::RootEntry(_) => None,
            Self::PinTarget(id) => Some(&graph.pins[&id]),
        })
    }
}

struct Emitter<'a> {
    graph: &'a GraphDocument,
    unit_project: Option<&'a [GraphDocument]>,
    library1027:Option<&'a crate::LibraryGraphResources1027<'a>>,
    library_current1027:Option<&'a crate::library_current1027::LibraryCurrentMembers1027<'a>>,
    default_values: BTreeMap<PinId, PropertyValue>,
    pin_types: BTreeMap<PinId, ValueType>,
    emitted: RefCell<Vec<(Range<usize>, NodeId)>>,
    range_owners: RefCell<Vec<NodeId>>,
    native_loop_owners:RefCell<Vec<NodeId>>,
    execution_emissions: Cell<usize>,
    execution_nesting: Cell<usize>,
    scheduled: RefCell<Option<crate::native_frame_preparation::GatheredConsumer>>,
    native_scope: Option<String>,
}

impl<'a> Emitter<'a> {
    fn new(graph: &'a GraphDocument) -> Self {
        Self {
            graph,
            unit_project: None,
            library1027:None,
            library_current1027:None,
            default_values: graph.effective_pin_default_values(),
            pin_types: graph.effective_pin_types(),
            emitted: RefCell::new(Vec::new()),
            range_owners: RefCell::new(Vec::new()),
            native_loop_owners:RefCell::new(Vec::new()),
            execution_emissions: Cell::new(0),
            execution_nesting: Cell::new(0),
            scheduled: RefCell::new(None),
            native_scope: None,
        }
    }
}

/// Compile only a data subgraph for bounded previews. The same emitter is
/// used by export; disconnected/incomplete narrative nodes do not interfere.
pub fn transpile_value_output(
    graph: &GraphDocument,
    node: NodeId,
    key: &str,
) -> Result<String, TranspileError> {
    graph.require_native_loop_backend().map_err(TranspileError::GeneratedSourceInvalid)?;
    if !graph.inspector_evaluation.is_legacy() {
        return Err(TranspileError::GeneratedSourceInvalid("NativeFrameV1 terminals require a reached consumer and an owned frame; isolated preview refused".into()));
    }
    let emitter = Emitter::new(graph);
    let pin = emitter.pin(node, key)?;
    if pin.direction != PinDirection::Output || graph.resolved_pin_is_execution(pin.id) {
        return Err(TranspileError::ExpectedInput(pin.id));
    }
    let expression = emitter.expression_from_output(pin.id, &mut BTreeSet::new())?;
    validate_expression(&expression)?;
    Ok(expression)
}
pub fn transpile_value_input(
    graph: &GraphDocument,
    node: NodeId,
    key: &str,
) -> Result<String, TranspileError> {
    graph.require_native_loop_backend().map_err(TranspileError::GeneratedSourceInvalid)?;
    if !graph.inspector_evaluation.is_legacy() {
        return Err(TranspileError::GeneratedSourceInvalid("NativeFrameV1 isolated input preview refused".into()));
    }
    let expression = Emitter::new(graph).expression_from_input(node, key)?;
    validate_expression(&expression)?;
    Ok(expression)
}

/// Validate editor expressions without accepting extra injected statements.
pub fn validate_expression(expression: &str) -> Result<(), TranspileError> {
    let parsed = rvn_parser::parse(&format!("set __editor_check = {expression}\n"))
        .map_err(|error| TranspileError::GeneratedSourceInvalid(error.to_string()))?;
    if !matches!(parsed.as_slice(), [rvn_parser::Statement::SetVar { .. }]) {
        return Err(TranspileError::GeneratedSourceInvalid(
            "Une seule expression RVN est attendue".into(),
        ));
    }
    Ok(())
}

impl Emitter<'_> {
    fn emit_document(&self) -> Result<String, TranspileError> {
        let (root_kind, header) = match &self.graph.kind {
            GraphKind::Label { name } => (
                NodeKind::Label,
                format!("{}label {}\n", if self.graph.narrative == Some(NarrativeMetadata::SceneEntry) { "scene " } else { "" }, validate_identifier(name)?),
            ),
            GraphKind::Init => (NodeKind::Init, "init {\n".to_owned()),
            GraphKind::UnitFunction { name } => (NodeKind::FunctionEntry,format!("function {}",validate_identifier(name)?)),
            GraphKind::Function { name } => (
                NodeKind::FunctionEntry,
                format!("function {}", validate_identifier(name)?),
            ),
            GraphKind::Screen { name } => (
                NodeKind::ScreenEntry,
                format!("screen {}", validate_identifier(name)?),
            ),
            GraphKind::Handler { name } => (
                NodeKind::HandlerEntry,
                format!("handler {}", validate_identifier(name)?),
            ),
            kind => return Err(TranspileError::UnsupportedGraphKind(kind.clone())),
        };
        let roots: Vec<_> = self
            .graph
            .nodes
            .values()
            .filter(|node| node.kind == root_kind && (root_kind != NodeKind::Label
                || !matches!(node.properties.get("label"), Some(PropertyValue::String(name)) if !name.is_empty())))
            .map(|node| node.id)
            .collect();
        let [root] = roots.as_slice() else {
            return Err(TranspileError::ExpectedSingleRoot {
                kind: root_kind,
                found: roots.len(),
            });
        };

        // Root selection above remains authoritative; it deliberately carries
        // no made-up execution pin. Edge traversal begins after exec_out.
        let root_entry = ExecutionTarget::RootEntry(*root);
        root_entry.node(self.graph)?;
        let mut source = String::new();
        for node in self
            .graph
            .nodes
            .values()
            .filter(|node| node.kind == NodeKind::Use)
        {
            let start = source.len();
            let paths = self.pin_string_list(node.id, "paths")?;
            if paths.len() == 1 {
                source.push_str("use ");
                source.push_str(&quote(&paths[0]));
                source.push('\n');
            } else if !paths.is_empty() {
                source.push_str("use { ");
                source.push_str(
                    &paths
                        .iter()
                        .map(|path| quote(path))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                source.push_str(" }\n");
            }
            if source.len() > start { self.emitted.borrow_mut().push((start..source.len(), node.id)); }
        }
        // The complete registry still participates in validation for every
        // scope. Only a project-owned scope emits project declarations:
        // nominal Library members own one callable declaration, not an Init.
        for id in self.graph.characters.keys() { validate_identifier(id)?; }
        let owns_project_declarations=self.graph.library1027.as_ref()
            .and_then(|metadata|metadata.owner.as_ref()).is_none();
        if owns_project_declarations && !self.graph.characters.is_empty() {
            source.push_str("init {\n");
            for (id, name) in &self.graph.characters {
                source.push_str(&format!(
                    "    character.create({}, {})\n",
                    quote(id),
                    quote(name)
                ));
            }
            source.push_str("}\n");
        }
        let root_start = source.len();
        source.push_str(&header);
        if matches!(
            self.graph.kind,
            GraphKind::Function { .. } | GraphKind::UnitFunction { .. } | GraphKind::Screen { .. } | GraphKind::Handler { .. }
        ) {
            if matches!(self.graph.kind,GraphKind::UnitFunction{..}) {
                let parameters=crate::unit_graph_source1003::parameters(self.graph).map_err(TranspileError::GeneratedSourceInvalid)?;
                let names=parameters.iter().map(|pin|validate_variable_binding_name(&pin.label)).collect::<Result<Vec<_>,_>>()?;
                source.push_str(&format!("({}) -> () {{\n",names.join(", ")));
            } else {
            let parameters = match self.graph.nodes[root].properties.get("parameters") {
                Some(PropertyValue::StringList(parameters)) => parameters,
                _ => {
                    return Err(TranspileError::GeneratedSourceInvalid(
                        "Paramètres de fonction absents".into(),
                    ))
                }
            };
            let parameter_source = parameters.iter()
                .map(|parameter| validate_variable_binding_name(parameter))
                .collect::<Result<Vec<_>,_>>()?;
            if parameters.len() > 128
                || parameters.iter().collect::<BTreeSet<_>>().len() != parameters.len()
            {
                return Err(TranspileError::GeneratedSourceInvalid(
                    "Paramètres de fonction dupliqués ou trop nombreux".into(),
                ));
            }
            source.push_str(&format!("({}) {{\n", parameter_source.join(", ")));
            }
        }
        if self.native_scope.is_some() && crate::empty_native_init_0984::empty_graph(self.graph) {
            source.push_str(&crate::empty_native_init_0984::emit(self.graph).map_err(TranspileError::GeneratedSourceInvalid)?);
        }
        self.emit_sequence(
            self.successor(*root, "exec_out")?,
            1,
            None,
            &mut source,
            &mut BTreeSet::new(),
        )?;
        if matches!(
            self.graph.kind,
            GraphKind::Init
                | GraphKind::Function { .. }
                | GraphKind::UnitFunction { .. }
                | GraphKind::Screen { .. }
                | GraphKind::Handler { .. }
        ) {
            source.push_str("}\n");
        }
        self.emitted.borrow_mut().push((root_start..source.len(), *root));
        if let GraphKind::Label { name } = &self.graph.kind {
            let mut names = BTreeSet::from([name.clone()]);
            for node in self
                .graph
                .nodes
                .values()
                .filter(|n| n.kind == NodeKind::Label && n.id != *root)
            {
                let label = property_string(node.id, &node.properties, "label")?;
                validate_identifier(&label)?;
                if !names.insert(label.clone()) {
                    return Err(TranspileError::GeneratedSourceInvalid(format!(
                        "Label dupliqué : {label}"
                    )));
                }
                let start = source.len();
                source.push_str(&format!("\nlabel {label}\n"));
                self.emit_sequence(
                    self.successor(node.id, "exec_out")?,
                    1,
                    None,
                    &mut source,
                    &mut BTreeSet::new(),
                )?;
                self.emitted.borrow_mut().push((start..source.len(), node.id));
            }
        }
        if source.len() > MAX_GENERATED_SOURCE_BYTES {
            return Err(TranspileError::ExecutionExpansionLimit {
                node: *root, resource: "source_bytes", limit: MAX_GENERATED_SOURCE_BYTES,
            });
        }
        Ok(source)
    }

    fn emit_sequence(
        &self,
        current: Option<ExecutionTarget>,
        indent: usize,
        branch_owner: Option<NodeId>,
        source: &mut String,
        ancestors: &mut BTreeSet<NodeId>,
    ) -> Result<(), TranspileError> {
        let Some(target) = current else { return Ok(()); };
        let node = target.node(self.graph)?;
        let nesting = self.execution_nesting.get();
        if nesting >= MAX_EXECUTION_NESTING {
            return Err(TranspileError::ExecutionExpansionLimit {
                node, resource: "nesting", limit: MAX_EXECUTION_NESTING,
            });
        }
        // Each branch inherits its ancestors, but does not consume its
        // siblings or the parent continuation. A convergent node is a
        // separate lexical visit; a return to an ancestor remains a cycle.
        let mut active_path = ancestors.clone();
        self.execution_nesting.set(nesting + 1);
        // Nested execution branches own their own consumer gather. Preserve
        // the real header plan while its child statements are emitted.
        let parent_plan = self.scheduled.borrow_mut().take();
        let result = self.emit_sequence_inner(current, indent, branch_owner, source, &mut active_path);
        *self.scheduled.borrow_mut() = parent_plan;
        self.execution_nesting.set(nesting);
        result?;
        if source.len() > MAX_GENERATED_SOURCE_BYTES {
            return Err(TranspileError::ExecutionExpansionLimit {
                node, resource: "source_bytes", limit: MAX_GENERATED_SOURCE_BYTES,
            });
        }
        Ok(())
    }

    fn emit_sequence_inner(
        &self,
        mut current: Option<ExecutionTarget>,
        indent: usize,
        branch_owner: Option<NodeId>,
        source: &mut String,
        visited: &mut BTreeSet<NodeId>,
    ) -> Result<(), TranspileError> {
        while let Some(target) = current {
            let node_id = target.node(self.graph)?;
            // Macro/control lowering can inspect this exact input. Existing
            // node lowering keeps its established semantics in this tranche.
            let _reached_input = target.input(self.graph)?;
            if source.len() > MAX_GENERATED_SOURCE_BYTES {
                return Err(TranspileError::ExecutionExpansionLimit {
                    node: node_id, resource: "source_bytes", limit: MAX_GENERATED_SOURCE_BYTES,
                });
            }
            let emissions = self.execution_emissions.get();
            if emissions >= MAX_EXECUTION_EMISSIONS {
                return Err(TranspileError::ExecutionExpansionLimit {
                    node: node_id, resource: "execution_visits", limit: MAX_EXECUTION_EMISSIONS,
                });
            }
            self.execution_emissions.set(emissions + 1);
            if self.range_owners.borrow().last()==Some(&node_id) {
                let start=source.len();push_indent(source,indent);source.push_str("flow.break()\n");
                self.emitted.borrow_mut().push((start..source.len(),node_id));return Ok(());
            }
            // Mutating control inputs terminate without an execution output.
            // Reaching Reset/Close on an active macro is not Enter reentry.
            let terminal_macro=if crate::standard_macros_1020::kind(self.graph.nodes[&node_id].kind).is_some(){
                let input=target.input(self.graph)?.ok_or_else(||TranspileError::GeneratedSourceInvalid("Macro cannot be RootEntry".into()))?;
                crate::standard_macros_1020::control(self.graph,node_id,input.id).map_err(TranspileError::GeneratedSourceInvalid)?
                    !=rvn_parser::scheduled_source::standard_macro_contract::Control::Enter
            }else{false};
            if !visited.insert(node_id) && !terminal_macro {
                return Err(TranspileError::ExecutionCycle(node_id));
            }
            let node = self
                .graph
                .nodes
                .get(&node_id)
                .ok_or(TranspileError::NodeNotFound(node_id))?;
            if node.kind == NodeKind::BranchEnd {
                let owner = property_int(node_id, &node.properties, "owner")? as u64;
                if branch_owner != Some(NodeId::new(owner)) {
                    return Err(TranspileError::WrongBranchEnd {
                        node: node_id,
                        expected_owner: branch_owner,
                        found_owner: NodeId::new(owner),
                    });
                }
                if self.native_scope.is_some() {
                    let start=source.len();
                    self.open_consumer(node_id,indent,source,&Some(Default::default()))?;
                    push_indent(source,indent);source.push_str("}\n");
                    self.emitted.borrow_mut().push((start..source.len(),node_id));
                }
                return Ok(());
            }

            let start = source.len();
            let generic=self.native_scope.is_some()&&!matches!(node.kind,NodeKind::SetVariable|NodeKind::LocalVariable|NodeKind::FunctionReturn|NodeKind::DoOnce|NodeKind::DoN|NodeKind::FlipFlop|NodeKind::Gate);
            if generic {*self.scheduled.borrow_mut()=Some(Default::default());}
            if generic {
                for key in node.pins.iter().filter_map(|id|self.graph.pins.get(id)).map(|pin|pin.key.as_str()) {
                    if crate::native_frame_preparation::is_static_audio_input(node.kind,key)
                        && self.graph.pin_by_key(node_id,key).is_some() && self.input_is_connected(node_id,key)? {
                        self.expression_from_input(node_id,key)?;
                    }
                }
            }
            let original_indent=indent;
            let indent=indent+usize::from(generic);
            current = match node.kind {
                NodeKind::DoOnce|NodeKind::DoN|NodeKind::FlipFlop|NodeKind::Gate=>{
                    self.emit_standard_macro(target,indent,source,visited)?;None
                },
                NodeKind::Reroute => self.successor(node_id, "value_out")?,
                NodeKind::Dialogue => {
                    let character = self.pin_string(node_id, "character")?;
                    let text = self.pin_interpolated_text(node_id, "text")?;
                    if !character.is_empty() {
                        self.emit_character_appearance(node_id, &character, indent, source)?;
                    }
                    push_indent(source, indent);
                    if !character.is_empty() {
                        source.push_str(validate_identifier(&character)?);
                        source.push(' ');
                    }
                    source.push_str(&quote(&text));
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::WidgetCreate => {
                    if self.native_scope.is_none() { return Err(TranspileError::GeneratedSourceInvalid("Create Widget requires its source-owned Native Consumer".into())); }
                    let name=crate::widget_factory_graph_1001::result_name(node).map_err(TranspileError::GeneratedSourceInvalid)?;
                    let local=crate::widget_factory_graph_1001::local(node).map_err(TranspileError::GeneratedSourceInvalid)?;
                    let class=self.expression_from_input(node_id,"class")?;
                    let arguments=self.expression_from_input(node_id,"arguments")?;
                    push_indent(source,indent);
                    source.push_str(&format!("{} {} = __rvn_widget_create_v1({}, {})\n",if local{"local"}else{"set"},validate_variable_name(name)?,class,arguments));
                    self.successor(node_id,"exec_out")?
                }
                NodeKind::SetVariable | NodeKind::LocalVariable => {
                    let name = match node.properties.get("name") {
                        Some(PropertyValue::String(name)) => name.clone(),
                        _ => self.pin_string(node_id, "name")?,
                    };
                    let (value, plan) = self.consumer_value(node_id)?;
                    self.open_consumer(node_id, indent, source, &plan)?;
                    let indent = indent + usize::from(plan.is_some());
                    push_indent(source, indent);
                    source.push_str(if node.kind == NodeKind::LocalVariable {
                        "local "
                    } else {
                        "set "
                    });
                    source.push_str(&validate_variable_name(&name)?);
                    source.push_str(" = ");
                    source.push_str(&value);
                    source.push('\n');
                    if plan.is_some() { push_indent(source, indent-1); source.push_str("}\n"); }
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::Scene => {
                    let background = self.pin_string(node_id, "background")?;
                    let transition = self.pin_string(node_id, "transition")?;
                    push_indent(source, indent);
                    source.push_str("scene ");
                    source.push_str(&quote(&background));
                    if !transition.is_empty() && transition != "none" {
                        source.push_str(" with ");
                        source.push_str(&transition);
                    }
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::Config => {
                    let key = self.pin_string(node_id, "key")?;
                    let value = self.pin_string(node_id, "value")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&key)?);
                    source.push_str(": ");
                    source.push_str(&quote(&value));
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::CharacterCreate => {
                    let id = self.pin_string(node_id, "id")?;
                    let display_name = self.pin_string(node_id, "display_name")?;
                    push_indent(source, indent);
                    source.push_str("character.create(");
                    source.push_str(&quote(&id));
                    source.push_str(", ");
                    source.push_str(&quote(&display_name));
                    source.push_str(")\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::CinematicShow => {
                    let cinematic = self.pin_string(node_id, "cinematic")?;
                    push_indent(source, indent);
                    source.push_str("cinematic ");
                    source.push_str(&quote(&cinematic));
                    self.push_cinematic_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::CinematicHide => {
                    push_indent(source, indent);
                    source.push_str("cinematic hide");
                    self.push_cinematic_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::UnlockEnding => {
                    let id = self.pin_string(node_id, "id")?;
                    push_indent(source, indent);
                    source.push_str("unlock_ending ");
                    source.push_str(&quote(&id));
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteShow => {
                    let character = self.pin_string(node_id, "character")?;
                    let emotion = self.connected_sprite(node_id)?;
                    if self.native_scope.is_some() && emotion.is_some() {
                        let legacy=self.pin(node_id,"emotion")?;
                        if self.input_is_connected(node_id,"emotion")? || matches!(&legacy.default_value,Some(PropertyValue::String(value))if !value.is_empty()) {
                            return Err(TranspileError::GeneratedSourceInvalid("A native connected Character Sprite cannot also claim an unused legacy emotion operand".into()));
                        }
                    }
                    if emotion.is_none()
                        && node.properties.get("use_default_sprite")
                            != Some(&PropertyValue::Bool(true))
                    {
                        return Err(TranspileError::MissingInputValue {
                            node: node_id,
                            key:
                                "Sprite : reliez une image au personnage ou utilisez sa composition"
                                    .into(),
                        });
                    }
                    let position = self.pin_string(node_id, "position")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(".show(");
                    if let Some(emotion) = emotion {
                        source.push_str(&quote(&emotion));
                    }
                    source.push(')');
                    if !position.is_empty() {
                        source.push_str(" at ");
                        source.push_str(&position);
                    }
                    self.push_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteHide => {
                    let character = self.pin_string(node_id, "character")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(".hide()");
                    self.push_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteMove => {
                    let character = self.pin_string(node_id, "character")?;
                    let position = self.pin_string(node_id, "position")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(".move() at ");
                    source.push_str(&position);
                    self.push_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteAnimate => {
                    let character = self.pin_string(node_id, "character")?;
                    let animation = self.pin_string(node_id, "animation")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(".animate(");
                    source.push_str(&quote(&animation));
                    if let Some(PropertyValue::StringList(params)) = node.properties.get("params") {
                        for param in params {
                            let (name, value) = param.split_once('=').ok_or_else(|| {
                                TranspileError::InvalidDynamicParameter(param.clone())
                            })?;
                            if node
                                .properties
                                .contains_key(&format!("animation_param_{}", name.trim()))
                            {
                                continue;
                            }
                            source.push_str(", ");
                            source.push_str(validate_identifier(name.trim())?);
                            source.push_str(": ");
                            source.push_str(value.trim());
                        }
                    }
                    for (key, value) in &node.properties {
                        if let Some(name) = key.strip_prefix("animation_param_") {
                            source.push_str(", ");
                            source.push_str(validate_identifier(name)?);
                            source.push_str(": ");
                            source.push_str(&emit_property_expression(value)?);
                        }
                    }
                    source.push_str(")\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteStopAnimation => {
                    let character = self.pin_string(node_id, "character")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(".stop_animation()\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SpriteEffect => {
                    let character = self.pin_string(node_id, "character")?;
                    let mut effects = Vec::new();
                    for key in ["flip_x", "flip_y", "scale", "rotation", "tint"] {
                        // Missing flags mean legacy behavior: all effects were applied.
                        if node.properties.get(&format!("apply_{key}"))
                            == Some(&PropertyValue::Bool(false))
                        {
                            continue;
                        }
                        let value = match key {
                            "flip_x" | "flip_y" => self.pin_bool(node_id, key)?.to_string(),
                            "tint" => match self.pin_string(node_id, key) {
                                // tint is a historical raw source-token interior;
                                // unlike expression strings it has not been decoded.
                                // Preserve its exact bytes, including interpolation's
                                // outer escaping, instead of escaping them twice.
                                Ok(color) => rvn_parser::parser::raw_string_token_source(&color)
                                    .map_err(TranspileError::GeneratedSourceInvalid)?,
                                Err(_) => quote(&format!(
                                    "[{}]",
                                    self.expression_from_input(node_id, key)?
                                )),
                            },
                            _ => self.pin_float(node_id, key)?.to_string(),
                        };
                        effects.push(format!("{key}: {value}"));
                    }
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&character)?);
                    source.push_str(&format!(".effect({})\n", effects.join(", ")));
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::Timer => {
                    let duration = self.pin_float(node_id, "duration")?;
                    let target = self.pin_string(node_id, "target")?;
                    push_indent(source, indent);
                    source.push_str(&format!(
                        "timer {duration} => jump {}\n",
                        validate_identifier(&target)?
                    ));
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::TimerCancel => {
                    push_indent(source, indent);
                    source.push_str("timer cancel\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::MethodCall => {
                    let target = self.pin_string(node_id, "target")?;
                    let method = self.pin_string(node_id, "method")?;
                    push_indent(source, indent);
                    source.push_str(validate_identifier(&target)?);
                    source.push('.');
                    source.push_str(validate_identifier(&method)?);
                    source.push('(');
                    let arg = self.pin_string(node_id, "arg")?;
                    if !arg.is_empty() {
                        source.push_str(&quote(&arg));
                    }
                    source.push(')');
                    self.push_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::MusicPlay => {
                    let file = self.pin_string(node_id, "file")?;
                    push_indent(source, indent);
                    source.push_str("music.play(");
                    source.push_str(&quote(&file));
                    source.push(')');
                    self.push_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::MusicStop => {
                    push_indent(source, indent);
                    source.push_str("music.stop()");
                    self.push_optional_transition(node_id, source)?;
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::MusicVolume => {
                    if crate::native_frame_preparation::music_volume_uses_expression(self.graph,node_id) {
                        let level=self.expression_from_input(node_id,"level")?;
                        push_indent(source,indent);source.push_str(&format!("music.volume({level})\n"));
                    } else {
                    let level = self.pin_float(node_id, "level")?;
                    // Le parseur historique des appels de méthode reçoit ses
                    // arguments sous forme de chaînes avant de convertir le volume.
                    push_indent(source, indent);
                    source.push_str(&format!("music.volume(\"{level}\")\n"));
                    }
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::SfxPlay | NodeKind::SfxStop => {
                    let file = self.pin_string(node_id, "file")?;
                    push_indent(source, indent);
                    source.push_str(if node.kind == NodeKind::SfxPlay {
                        "sfx.play("
                    } else {
                        "sfx.stop("
                    });
                    source.push_str(&quote(&file));
                    source.push_str(")\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::VoicePlay => {
                    let file = self.pin_string(node_id, "file")?;
                    push_indent(source, indent);
                    source.push_str("voice ");
                    source.push_str(&quote(&file));
                    source.push('\n');
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::VoiceStop => {
                    push_indent(source, indent);
                    source.push_str("voice stop\n");
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::Imagemap => {
                    let background = self.pin_string(node_id, "background")?;
                    let hover = self.pin_string(node_id, "hover")?;
                    push_indent(source, indent);
                    source.push_str("imagemap {\n");
                    push_indent(source, indent + 1);
                    source.push_str("background: ");
                    source.push_str(&quote(&background));
                    source.push('\n');
                    if !hover.is_empty() {
                        push_indent(source, indent + 1);
                        source.push_str("hover: ");
                        source.push_str(&quote(&hover));
                        source.push('\n');
                    }
                    // Child consumers own separate gather plans. Preserve the
                    // reached Imagemap header (including literal asset sources).
                    let header_plan=self.scheduled.borrow_mut().take();
                    if let Some(PropertyValue::StringList(hotspots)) =
                        node.properties.get("hotspots")
                    {
                        for (index, hotspot) in hotspots.iter().enumerate() {
                            let spec=crate::imagemap_spec::HotspotSpec::parse(hotspot).map_err(|_|TranspileError::InvalidHotspot(hotspot.clone()))?;
                            push_indent(source, indent + 1);
                            source.push_str("hotspot {");
                            if let Some(name)=&spec.name{source.push_str(" name: ");source.push_str(&quote(name));}
                            source.push_str(&format!(" area: ({}, {}, {}, {})",spec.area[0],spec.area[1],spec.area[2],spec.area[3]));
                            if let Some(area)=spec.hover_area{source.push_str(&format!(" hover_area: ({}, {}, {}, {})",area[0],area[1],area[2],area[3]));}
                            source.push_str(" } => {\n");
                            self.emit_sequence(
                                self.successor(node_id, &format!("hotspot_{index}"))?,
                                indent + 2,
                                Some(node_id),
                                source,
                                visited,
                            )?;
                            push_indent(source, indent + 1);
                            source.push_str("}\n");
                        }
                    }
                    push_indent(source, indent);
                    source.push_str("}\n");
                    *self.scheduled.borrow_mut()=header_plan;
                    self.successor(node_id, "completed")?
                }
                NodeKind::TypewriterSet => {
                    let enabled = if self.native_scope.is_some()
                        && crate::native_frame_preparation::typewriter_set_uses_expression(self.graph,node_id) {
                        self.expression_from_input(node_id,"enabled")?
                    } else { self.pin_bool(node_id,"enabled")?.to_string() };
                    push_indent(source, indent);
                    source.push_str(&format!("typewriter = {enabled}\n"));
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::TypewriterSpeed => {
                    let speed = if self.native_scope.is_some()
                        && crate::native_frame_preparation::typewriter_speed_uses_expression(self.graph,node_id) {
                        self.expression_from_input(node_id,"speed")?
                    } else { self.pin_int(node_id,"speed")?.to_string() };
                    push_indent(source, indent);
                    source.push_str(&format!("typewriter.speed({speed})\n"));
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::If => {
                    self.emit_if(node_id, indent, source, visited)?;
                    self.successor(node_id, "completed")?
                }
                NodeKind::Sequence => {
                    let count=property_int(node_id,&node.properties,"output_count")?;
                    if !(1..=32).contains(&count){return Err(TranspileError::MissingInputValue{node:node_id,key:"output_count: 1..32".into()});}
                    push_indent(source,indent);source.push_str("flow.sequence() {\n");
                    for index in 0..count {
                        push_indent(source,indent+1);source.push_str("{\n");
                        self.emit_sequence(self.successor(node_id,&format!("then_{index}"))?,indent+2,Some(node_id),source,visited)?;
                        push_indent(source,indent+1);source.push_str("}\n");
                    }
                    push_indent(source,indent);source.push_str("}\n");self.successor(node_id,"completed")?
                }
                NodeKind::ForLoopWithBreak => {
                    let name=if node.native_loop_contract.is_some(){native_loop_index_name(self.graph.graph_id.get(),node_id.get())}else{property_string(node_id,&node.properties,"name")?};
                    push_indent(source,indent);source.push_str(&format!("flow.for_loop({}, {}, {}) {{\n",validate_variable_binding_name(&name)?,self.expression_from_input(node_id,"first")?,self.expression_from_input(node_id,"last")?));
                    self.range_owners.borrow_mut().push(node_id);
                    if node.native_loop_contract.is_some(){self.native_loop_owners.borrow_mut().push(node_id);}
                    let result=self.emit_sequence(self.successor(node_id,"body")?,indent+1,Some(node_id),source,visited);
                    if node.native_loop_contract.is_some(){self.native_loop_owners.borrow_mut().pop();}
                    self.range_owners.borrow_mut().pop();result?;
                    push_indent(source,indent);source.push_str("}\n");self.successor(node_id,"completed")?
                }
                NodeKind::While | NodeKind::ForEach => {
                    push_indent(source, indent);
                    if node.kind == NodeKind::While {
                        source.push_str(&format!(
                            "while {} {{\n",
                            self.expression_from_input(node_id, "condition")?
                        ));
                    } else {
                        let name = if node.native_loop_contract.is_some(){native_loop_index_name(self.graph.graph_id.get(),node_id.get())}else{property_string(node_id, &node.properties, "name")?};
                        source.push_str(&format!(
                            "for {} in {} {{\n",
                            validate_variable_binding_name(&name)?,
                            self.expression_from_input(node_id, "collection")?
                        ));
                    }
                    if node.native_loop_contract.is_some(){self.native_loop_owners.borrow_mut().push(node_id);}
                    let body_result=self.emit_sequence(
                        self.successor(node_id, "body")?,
                        indent + 1,
                        Some(node_id),
                        source,
                        visited,
                    );
                    if node.native_loop_contract.is_some(){self.native_loop_owners.borrow_mut().pop();}
                    body_result?;
                    push_indent(source, indent);
                    source.push_str("}\n");
                    self.successor(node_id, "completed")?
                }
                NodeKind::UnitCall => {
                    if self.graph.library_call1027(node_id).is_some(){let text=self.qualified_call1027(node_id,rvn_parser::function_library_program1023::LibraryCallMode::Completion,&mut BTreeSet::new())?;push_indent(source,indent);source.push_str(&text);source.push('\n');self.successor(node_id,"exec_out")?}else{
                    let project=self.unit_project.ok_or_else(||TranspileError::GeneratedSourceInvalid("UnitCall requires its actual resolved project/source context".into()))?;
                    let (_,inputs)=crate::unit_graph_source1003::resolve_call(project,self.graph,node_id).map_err(TranspileError::GeneratedSourceInvalid)?;
                    let arguments=inputs.iter().map(|pin|self.expression_from_input(node_id,&pin.key)).collect::<Result<Vec<_>,_>>()?;
                    let name=property_string(node_id,&node.properties,"function")?;
                    push_indent(source,indent);source.push_str(&format!("call {}({})\n",validate_identifier(&name)?,arguments.join(", ")));
                    self.successor(node_id,"exec_out")?
                    }
                }
                NodeKind::UnitReturn => {
                    crate::unit_graph_source1003::entry(self.graph).map_err(TranspileError::GeneratedSourceInvalid)?;
                    push_indent(source,indent);source.push_str("return\n");None
                }
                NodeKind::FunctionReturn => {
                    let (value, plan) = self.consumer_value(node_id)?;
                    self.open_consumer(node_id, indent, source, &plan)?;
                    push_indent(source, indent + usize::from(plan.is_some()));
                    source.push_str(&format!("return {value}\n"));
                    if plan.is_some() { push_indent(source,indent); source.push_str("}\n"); }
                    None
                }
                NodeKind::MenuExecute => {
                    push_indent(source, indent);
                    source.push_str(&format!("menu.execute({})\n", self.expression_from_input(node_id, "request")?));
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::WidgetAddToViewport | NodeKind::WidgetRemoveFromParent => {
                    crate::widget_viewport_graph1003::validate_node(self.graph,node).map_err(TranspileError::GeneratedSourceInvalid)?;
                    let object=self.expression_from_input(node_id,"object")?;
                    push_indent(source,indent);
                    if node.kind==NodeKind::WidgetAddToViewport {let z_order=self.expression_from_input(node_id,"z_order")?;source.push_str(&format!("widget.add_to_viewport({object}, {z_order})\n"));}
                    else {source.push_str(&format!("widget.remove_from_parent({object})\n"));}
                    self.successor(node_id,"exec_out")?
                }
                NodeKind::UiOpen
                | NodeKind::UiClose
                | NodeKind::UiFocus
                | NodeKind::UiSetState
                | NodeKind::MotionPlay
                | NodeKind::MotionStop
                | NodeKind::MotionWait
                | NodeKind::CharacterCompose
                | NodeKind::CharacterAttributes
                | NodeKind::VideoPlay
                | NodeKind::VideoPause
                | NodeKind::VideoResume
                | NodeKind::VideoStop
                | NodeKind::VideoSkip
                | NodeKind::VideoSeek
                | NodeKind::VideoVolume
                | NodeKind::VideoWait
                | NodeKind::AccessibilityConfigure
                | NodeKind::AccessibilitySpeak
                | NodeKind::AccessibilityStop => {
                    let (method, inputs): (&str, &[&str]) = match node.kind {
                        NodeKind::UiOpen => {
                            let method=match node.properties.get("story") {
                                None|Some(PropertyValue::Bool(false))=>"open",
                                Some(PropertyValue::Bool(true))=>"open_story",
                                _=>return Err(TranspileError::ExpectedBool { node: node_id, key: "story".into() }),
                            };
                            (method,&["name", "arguments", "modal", "layer"])
                        },
                        NodeKind::UiClose => ("close", &["name"]),
                        NodeKind::UiSetState => ("set_state", &["name", "element", "state"]),
                        NodeKind::MotionPlay => ("play", &["target", "definition"]),
                        NodeKind::MotionStop => ("stop", &["target"]),
                        NodeKind::MotionWait => ("wait", &["target"]),
                        NodeKind::CharacterCompose => ("compose", &["character", "definition"]),
                        NodeKind::CharacterAttributes => {
                            ("attributes", &["character", "attributes"])
                        }
                        NodeKind::VideoPlay => ("play", &["name", "definition"]),
                        NodeKind::VideoPause => ("pause", &["name"]),
                        NodeKind::VideoResume => ("resume", &["name"]),
                        NodeKind::VideoStop => ("stop", &["name"]),
                        NodeKind::VideoSkip => ("skip", &["name"]),
                        NodeKind::VideoWait => ("wait", &["name"]),
                        NodeKind::VideoSeek => ("seek", &["name", "seconds"]),
                        NodeKind::VideoVolume => ("volume", &["name", "volume"]),
                        NodeKind::AccessibilityConfigure => ("configure", &["settings"]),
                        NodeKind::AccessibilitySpeak => ("speak", &["text"]),
                        NodeKind::AccessibilityStop => ("stop", &[]),
                        _ => ("focus", &["name", "element"]),
                    };
                    let arguments = inputs
                        .iter()
                        .map(|input| self.expression_from_input(node_id, input))
                        .collect::<Result<Vec<_>, _>>()?;
                    push_indent(source, indent);
                    let namespace = if matches!(
                        node.kind,
                        NodeKind::MotionPlay | NodeKind::MotionStop | NodeKind::MotionWait
                    ) {
                        "motion"
                    } else if matches!(
                        node.kind,
                        NodeKind::CharacterCompose | NodeKind::CharacterAttributes
                    ) {
                        "character"
                    } else if matches!(
                        node.kind,
                        NodeKind::VideoPlay
                            | NodeKind::VideoPause
                            | NodeKind::VideoResume
                            | NodeKind::VideoStop
                            | NodeKind::VideoSkip
                            | NodeKind::VideoWait
                            | NodeKind::VideoSeek
                            | NodeKind::VideoVolume
                    ) {
                        "video"
                    } else {
                        "ui"
                    };
                    let namespace = if matches!(
                        node.kind,
                        NodeKind::AccessibilityConfigure
                            | NodeKind::AccessibilitySpeak
                            | NodeKind::AccessibilityStop
                    ) {
                        "accessibility"
                    } else {
                        namespace
                    };
                    source.push_str(&format!("{namespace}.{method}({})\n", arguments.join(", ")));
                    self.successor(node_id, "exec_out")?
                }
                NodeKind::Choice => {
                    self.emit_choice(node_id, indent, source, visited)?;
                    self.successor(node_id, "completed")?
                }
                NodeKind::Jump | NodeKind::Call => {
                    let keyword = if node.kind == NodeKind::Jump {
                        "jump"
                    } else {
                        "call"
                    };
                    let target = self.pin_string(node_id, "target")?;
                    push_indent(source, indent);
                    source.push_str(keyword);
                    source.push(' ');
                    source.push_str(validate_identifier(&target)?);
                    source.push('\n');
                    if node.kind == NodeKind::Call {
                        self.successor(node_id, "exec_out")?
                    } else {
                        None
                    }
                }
                NodeKind::Return => {
                    push_indent(source, indent);
                    source.push_str("return\n");
                    None
                }
                kind => {
                    return Err(TranspileError::UnsupportedStatement {
                        node: node_id,
                        kind,
                    })
                }
            };
            if generic {
                let plan=self.scheduled.borrow_mut().take().unwrap_or_default();
                let body=source.split_off(start);
                self.open_consumer(node_id,original_indent,source,&Some(plan))?;
                let shift=source.len()-start;
                for (range,_) in self.emitted.borrow_mut().iter_mut().filter(|(range,_)|range.start>=start) {
                    range.start+=shift;range.end+=shift;
                }
                source.push_str(&body);push_indent(source,original_indent);source.push_str("}\n");
            }
            if source.len() > start { self.emitted.borrow_mut().push((start..source.len(), node_id)); }
        }

        Ok(())
    }

    fn emit_if(
        &self,
        node: NodeId,
        indent: usize,
        source: &mut String,
        visited: &mut BTreeSet<NodeId>,
    ) -> Result<(), TranspileError> {
        let condition = self.expression_from_input(node, "condition")?;
        let outer=self.scheduled.borrow_mut().take();
        push_indent(source, indent);
        source.push_str("if ");
        source.push_str(&condition);
        source.push_str(" {\n");
        self.emit_sequence(
            self.successor(node, "then")?,
            indent + 1,
            Some(node),
            source,
            visited,
        )?;
        push_indent(source, indent);
        source.push('}');
        if let Some(else_start) = self.successor(node, "else")? {
            source.push_str(" else {\n");
            self.emit_sequence(Some(else_start), indent + 1, Some(node), source, visited)?;
            push_indent(source, indent);
            source.push('}');
        }
        source.push('\n');
        *self.scheduled.borrow_mut()=outer;
        Ok(())
    }

    fn emit_choice(
        &self,
        node: NodeId,
        indent: usize,
        source: &mut String,
        visited: &mut BTreeSet<NodeId>,
    ) -> Result<(), TranspileError> {
        let graph_node = self.graph.nodes.get(&node).unwrap();
        let labels = match graph_node.properties.get("options") {
            Some(PropertyValue::StringList(labels)) if !labels.is_empty() => labels,
            _ => return Err(TranspileError::ChoiceWithoutOptions(node)),
        };
        push_indent(source, indent);
        source.push_str("choice {\n");
        for (index, label) in labels.iter().enumerate() {
            push_indent(source, indent + 1);
            source.push_str(&quote(label));
            let condition_key = format!("option_{index}_condition");
            if self.input_is_connected(node, &condition_key)? {
                source.push_str(" if ");
                source.push_str(&self.expression_from_input(node, &condition_key)?);
            } else if let Some(PropertyValue::String(condition)) =
                graph_node.properties.get(&condition_key)
            {
                if !condition.trim().is_empty() {
                    validate_expression(condition)?;
                    source.push_str(" if ");
                    source.push_str(condition);
                } else if self.graph.pin_by_key(node, &condition_key)
                    .and_then(|pin| self.graph.choice_condition_default(pin.id)) == Some(false)
                {
                    source.push_str(" if false");
                }
            } else if self.graph.pin_by_key(node, &condition_key)
                .and_then(|pin| self.graph.choice_condition_default(pin.id)) == Some(false)
            {
                source.push_str(" if false");
            }
            source.push_str(" => {\n");
            self.emit_sequence(
                self.successor(node, &format!("option_{index}"))?,
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

    fn consumer_value(&self, node: NodeId) -> Result<(String, Option<crate::native_frame_preparation::GatheredConsumer>),TranspileError> {
        if self.native_scope.is_none() { return self.expression_from_input(node,"value").map(|value|(value,None)); }
        *self.scheduled.borrow_mut() = Some(Default::default());
        let result = self.expression_from_input(node,"value");
        let plan = self.scheduled.borrow_mut().take();
        result.map(|value|(value,plan))
    }
    fn open_consumer(&self,node:NodeId,indent:usize,source:&mut String,plan:&Option<crate::native_frame_preparation::GatheredConsumer>) -> Result<(),TranspileError> {
        if let Some(plan)=plan {
            push_indent(source,indent);
            let version=if self.graph.nodes[&node].native_loop_contract.is_some(){3}else if matches!(self.graph.nodes[&node].kind,NodeKind::SetVariable|NodeKind::LocalVariable|NodeKind::FunctionReturn) {1}else{2};
            source.push_str(&format!("flow.consume_v{version}({}, {}) {{\n",quote(self.native_scope.as_ref().unwrap()),quote(&crate::native_frame_preparation::encode_node(self.graph,node).map_err(TranspileError::GeneratedSourceInvalid)?)));
            for (inspection,id,value,_) in &plan.steps {
                push_indent(source,indent+1);
                source.push_str(&format!("{} {} = {}\n",if *inspection {"inspect"} else {"capture"},quote(id),value));
            }
        }
        Ok(())
    }
    fn expression_from_input(&self, node: NodeId, key: &str) -> Result<String, TranspileError> {
        self.expression_from_input_with_stack(node, key, &mut BTreeSet::new())
    }

    fn expression_from_input_with_stack(
        &self,
        node: NodeId,
        key: &str,
        active: &mut BTreeSet<NodeId>,
    ) -> Result<String, TranspileError> {
        let input = self.pin(node, key)?;
        if input.direction != PinDirection::Input {
            return Err(TranspileError::ExpectedInput(input.id));
        }
        let connections: Vec<_> = self
            .graph
            .edges
            .values()
            .filter(|edge| edge.input == input.id)
            .collect();
        match connections.as_slice() {
            [] => match self.default_values.get(&input.id) {
                // The dynamic String Table Key admits empty text; it is a value, not a missing operand.
                Some(PropertyValue::String(value)) if key == "key" && self.graph.nodes[&node].kind == NodeKind::TextFromStringTable => Ok(quote(value)),
                Some(PropertyValue::String(table)) if input.value_type == ValueType::Asset(crate::AssetKind::StringTable) =>
                    Ok(format!("__rvn_string_table_asset({})", quote(table))),
                // Une chaîne vide provenant du catalogue est un placeholder
                // d'édition, pas une expression câblée par l'utilisateur.
                // Elle ne doit donc jamais produire `to_int("")` ou un SET
                // silencieux dans le script généré.
                Some(PropertyValue::String(value))
                    if value.is_empty()
                        && !self
                            .graph
                            .nodes
                            .get(&node)
                            .is_some_and(crate::type_inference::allows_empty_string_operand) =>
                {
                    Err(TranspileError::MissingInputValue {
                        node,
                        key: key.to_owned(),
                    })
                }
                Some(value)
                    if crate::blueprint_policy::authored_default_type(self.graph, input, value)
                        == ValueType::InterpolatedText
                        || (self
                            .graph
                            .nodes
                            .get(&node)
                            .is_some_and(GraphNode::uses_blueprint_operator_policy)
                            && self.pin_types.get(&input.id)
                                == Some(&ValueType::InterpolatedText)) =>
                {
                    Ok(format!(
                        "string_to_text({})",
                        emit_property_expression(value)?
                    ))
                }
                Some(value) => emit_property_expression(value),
                None => Err(TranspileError::MissingInputValue {
                    node,
                    key: key.to_owned(),
                }),
            },
            [edge] => self.expression_from_output(edge.output, active),
            _ => Err(TranspileError::MultipleInputConnections(input.id)),
        }
    }

    fn expression_from_output(
        &self, output: PinId, active: &mut BTreeSet<NodeId>,
    ) -> Result<String, TranspileError> {
        if self.graph.pins.get(&output).and_then(|pin|self.graph.nodes.get(&pin.node))
            .is_some_and(|node|crate::standard_macros_1020::kind(node.kind).is_some()) {
            return crate::standard_macros_1020::data_source(self.graph,output).map_err(TranspileError::GeneratedSourceInvalid)?
                .ok_or_else(||TranspileError::GeneratedSourceInvalid("Missing typed macro live output".into()));
        }
        if self.scheduled.borrow().is_none() {
            return self.expression_from_output_raw(output, active);
        }
        if active.len()>=128 {return Err(TranspileError::GeneratedSourceInvalid("Native dependency gather exceeds 128 levels".into()));}
        let pin = self.graph.pins.get(&output).ok_or(TranspileError::PinNotFound(output))?;
        let node = &self.graph.nodes[&pin.node];
        if node.native_loop_contract.is_some(){
            if !self.native_loop_owners.borrow().contains(&node.id){return Err(TranspileError::GeneratedSourceInvalid("Native loop output is outside its actual lexical body".into()));}
            if matches!((node.kind,pin.key.as_str()),(NodeKind::ForEach,"array_index")|(NodeKind::ForLoopWithBreak,"index")){
                return Ok(native_loop_index_name(self.graph.graph_id.get(),node.id.get()));
            }
        }
        let id = crate::native_frame_preparation::encode_node(self.graph,node.id).map_err(TranspileError::GeneratedSourceInvalid)?;
        let legacy=node.kind==NodeKind::GetStringTableIdAndKey && crate::native_frame_preparation::inspector_contract(self.graph,node.id).is_legacy();
        if legacy {
            let id=format!("p:{}/{}",pin.key,id);
            if id.len()>256 {return Err(TranspileError::GeneratedSourceInvalid("Legacy terminal source identity exceeds 256 UTF-8 bytes".into()));}
            let reference=format!("__rvn_captured_v1({})",quote(&id));
            if self.scheduled.borrow().as_ref().unwrap().legacy_outputs.contains(&output) {return Ok(reference);}
            let value=self.expression_from_output_raw(output,active)?;
            let mut schedule=self.scheduled.borrow_mut();let plan=schedule.as_mut().unwrap();
            if plan.steps.len()>=1024 {return Err(TranspileError::GeneratedSourceInvalid("NativeFrameV1 exceeds 1024 producers".into()));}
            plan.legacy_outputs.insert(output);plan.steps.push((false,id,value,node.id));return Ok(reference);
        }
        let inspection = node.kind == NodeKind::GetStringTableIdAndKey;
        let reference = if inspection {
            if !matches!(pin.key.as_str(), "valid"|"table_id"|"key") {
                return Err(TranspileError::GeneratedSourceInvalid("Unknown typed inspector terminal".into()));
            }
            format!("__rvn_terminal_v1({}, {})", quote(&id), quote(&pin.key))
        } else {
            // No speculative representation of multi-result producers.
            let outputs = node.pins.iter().filter(|id| {
                let pin = &self.graph.pins[id];
                pin.direction == PinDirection::Output && !self.graph.resolved_pin_is_execution(pin.id)
            }).count();
            if outputs != 1 && !(node.native_loop_contract.is_some()&&node.kind==NodeKind::ForEach&&pin.key=="array_element") {
                return Err(TranspileError::GeneratedSourceInvalid("NativeFrameV1 producer requires one representable value output (except the real typed inspector)".into()));
            }
            format!("__rvn_captured_v1({})", quote(&id))
        };
        if self.scheduled.borrow().as_ref().unwrap().seen.contains(&node.id) { return Ok(reference); }
        let value = if inspection {
            if !active.insert(node.id) { return Err(TranspileError::ExpressionCycle(node.id)); }
            let result = if self.input_is_connected(node.id,"text")? {
                self.expression_from_input_with_stack(node.id,"text",active)
            } else { Ok("__rvn_empty_text()".into()) };
            active.remove(&node.id);
            result?
        } else { self.expression_from_output_raw(output,active)? };
        let mut schedule = self.scheduled.borrow_mut();
        let plan = schedule.as_mut().unwrap();
        if plan.steps.len() >= 1024 { return Err(TranspileError::GeneratedSourceInvalid("NativeFrameV1 exceeds 1024 producers".into())); }
        plan.seen.insert(node.id);
        plan.steps.push((inspection,id,value,node.id));
        Ok(reference)
    }

    fn expression_from_output_raw(
        &self,
        output: PinId,
        active: &mut BTreeSet<NodeId>,
    ) -> Result<String, TranspileError> {
        let pin = self
            .graph
            .pins
            .get(&output)
            .ok_or(TranspileError::PinNotFound(output))?;
        if !active.insert(pin.node) {
            return Err(TranspileError::ExpressionCycle(pin.node));
        }
        let node = self.graph.nodes.get(&pin.node).unwrap();
        let expression = match node.kind {
            NodeKind::DoOnce|NodeKind::DoN|NodeKind::FlipFlop|NodeKind::Gate=>crate::standard_macros_1020::data_source(self.graph,output)
                .map_err(TranspileError::GeneratedSourceInvalid)?.ok_or_else(||TranspileError::GeneratedSourceInvalid("Missing typed macro live output".into()))?,
            NodeKind::WidgetClassValue => crate::widget_factory_graph_1001::class_expression(node).map_err(TranspileError::GeneratedSourceInvalid)?,
            NodeKind::WidgetObjectClass => {
                crate::widget_object_class_graph1002::validate_node(self.graph,node).map_err(TranspileError::GeneratedSourceInvalid)?;
                format!("__rvn_widget_object_class_v1({})",self.expression_from_input_with_stack(node.id,"object",active)?)
            },
            NodeKind::WidgetCreate => crate::widget_factory_graph_1001::result_name(node).map_err(TranspileError::GeneratedSourceInvalid)?.to_owned(),
            NodeKind::UiComponent => {
                let arguments = ["id", "kind", "properties", "children"]
                    .iter()
                    .map(|key| {
                        let pin = self.graph.pin_by_key(node.id, key).ok_or_else(|| {
                            TranspileError::MissingProperty {
                                node: node.id,
                                key: (*key).into(),
                            }
                        })?;
                        if matches!(*key, "properties" | "children")
                            && pin.default_value.is_none()
                            && !self.graph.edges.values().any(|edge| edge.input == pin.id)
                        {
                            Ok(if *key == "properties" { "dict()" } else { "[]" }.into())
                        } else {
                            self.expression_from_input_with_stack(node.id, key, active)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                format!("component({})", arguments.join(", "))
            }
            kind if kind.menu_constructor().is_some() => {
                let (name, keys) = kind.menu_constructor().unwrap();
                let args = keys.iter().map(|key| self.expression_from_input_with_stack(node.id, key, active)).collect::<Result<Vec<_>, _>>()?;
                format!("{name}({})", args.join(", "))
            }
            NodeKind::CanvasRect
            | NodeKind::CanvasEllipse
            | NodeKind::CanvasLine
            | NodeKind::CanvasPolygon
            | NodeKind::CanvasText
            | NodeKind::CanvasImage
            | NodeKind::CanvasGroup
            | NodeKind::CanvasHit
            | NodeKind::MotionTween
            | NodeKind::MotionSpline
            | NodeKind::MotionBezier
            | NodeKind::MotionCurve
            | NodeKind::MotionPause
            | NodeKind::MotionSequence
            | NodeKind::MotionParallel
            | NodeKind::MotionRepeat
            | NodeKind::MotionFrames
            | NodeKind::LayeredImage
            | NodeKind::ImageLayer
            | NodeKind::ImageLayers
            | NodeKind::VideoClip => {
                let (name, keys): (&str, &[&str]) = match node.kind {
                    NodeKind::CanvasRect => ("canvas_rect", &["rect", "color", "radius"]),
                    NodeKind::CanvasEllipse => ("canvas_ellipse", &["rect", "color"]),
                    NodeKind::CanvasLine => ("canvas_line", &["points", "color", "width"]),
                    NodeKind::CanvasPolygon => ("canvas_polygon", &["points", "color"]),
                    NodeKind::CanvasText => ("canvas_text", &["text", "position", "color", "size"]),
                    NodeKind::CanvasImage => ("canvas_image", &["image", "rect"]),
                    NodeKind::CanvasGroup => ("canvas_group", &["transform", "clip", "children"]),
                    NodeKind::CanvasHit => ("canvas_hit", &["id", "rect"]),
                    NodeKind::MotionTween => ("motion_tween", &["seconds", "from", "to", "curve"]),
                    NodeKind::MotionSpline => ("motion_spline", &["seconds", "points", "curve"]),
                    NodeKind::MotionBezier => ("motion_bezier", &["x1", "y1", "x2", "y2"]),
                    NodeKind::MotionCurve => ("motion_curve", &["function", "samples"]),
                    NodeKind::MotionPause => ("motion_pause", &["seconds"]),
                    NodeKind::MotionSequence => ("motion_sequence", &["steps"]),
                    NodeKind::MotionParallel => ("motion_parallel", &["steps"]),
                    NodeKind::MotionRepeat => ("motion_repeat", &["times", "motion"]),
                    NodeKind::LayeredImage => (
                        "layered_image",
                        if self
                            .graph
                            .pin_by_key(node.id, "options")
                            .is_some_and(|pin| {
                                pin.default_value.is_some()
                                    || self.graph.edges.values().any(|edge| edge.input == pin.id)
                            })
                        {
                            &["size", "defaults", "layers", "options"][..]
                        } else {
                            &["size", "defaults", "layers"][..]
                        },
                    ),
                    NodeKind::ImageLayer => ("image_layer", &["id", "image", "properties"]),
                    NodeKind::ImageLayers => ("image_layers", &["prefix", "images"]),
                    NodeKind::VideoClip => ("video_clip", &["source", "properties"]),
                    _ => ("motion_frames", &["images", "fps"]),
                };
                let args = keys
                    .iter()
                    .map(|key| {
                        if node.kind == NodeKind::MotionTween
                            && matches!(*key, "from" | "to")
                            && !self.input_is_connected(node.id, key)?
                            && self
                                .graph
                                .pin_by_key(node.id, key)
                                .is_some_and(|pin| pin.default_value.is_none())
                        {
                            return Ok("{}".into());
                        }
                        if matches!(
                            (node.kind, *key),
                            (NodeKind::LayeredImage, "defaults")
                                | (NodeKind::ImageLayer, "properties")
                                | (NodeKind::VideoClip, "properties")
                        ) && !self.input_is_connected(node.id, key)?
                            && self
                                .graph
                                .pin_by_key(node.id, key)
                                .is_some_and(|pin| pin.default_value.is_none())
                        {
                            return Ok("{}".into());
                        }
                        self.expression_from_input_with_stack(node.id, key, active)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                format!("{name}({})", args.join(", "))
            }
            NodeKind::Literal => {
                let value = node
                    .properties
                    .get("value")
                    .or(pin.default_value.as_ref())
                    .ok_or_else(|| TranspileError::MissingProperty {
                        node: node.id,
                        key: "value".into(),
                    })?;
                emit_property_expression(value)?
            }
            NodeKind::TextValue if pin.text_table_reference.is_some() =>
                crate::text_pin::emit_table_text(pin.text_table_reference.as_ref().unwrap()).map_err(TranspileError::GeneratedSourceInvalid)?,
            NodeKind::TextValue => match self.graph.pin_text(pin.id)
                .map_err(TranspileError::GeneratedSourceInvalid)? {
                Some(text) => crate::text_pin::emit_authored_text(&text,
                    node.properties.get("@text_explicit_notes") != Some(&PropertyValue::Bool(false))),
                None => quote(&property_string(node.id, &node.properties, "value")?),
            },
            NodeKind::MakeColor => {
                let channels = ["r", "g", "b", "a"]
                    .into_iter()
                    .map(|key| self.expression_from_input_with_stack(node.id, key, active))
                    .collect::<Result<Vec<_>, _>>()?;
                let function = if node.properties.get("rgb_max") == Some(&PropertyValue::Int(255)) {
                    "make_color_rgb"
                } else {
                    "make_color"
                };
                format!("{function}({})", channels.join(", "))
            }
            NodeKind::FormatText if node.properties.get("deferred_text") == Some(&PropertyValue::Bool(true)) =>
                self.deferred_format_text_output(node.id, active)?,
            NodeKind::FormatText => quote(&self.format_text_output(node.id, active)?),
            NodeKind::Reroute => self.expression_from_input_with_stack(node.id, "value", active)?,
            NodeKind::CharacterValue => {
                if self.native_scope.is_some() && self.input_is_connected(node.id,"sprite")? {
                    // Capture the actual image source before its Character value;
                    // the Character remains the existing string identity at runtime.
                    self.expression_from_input_with_stack(node.id,"sprite",active)?;
                }
                quote(&property_string(node.id, &node.properties, "character")?)
            }
            NodeKind::LabelValue => quote(&property_string(node.id, &node.properties, "label")?),
            NodeKind::PositionValue => {
                quote(&property_string(node.id, &node.properties, "position")?)
            }
            NodeKind::TextFromStringTable => format!("__rvn_string_table_text_name({}, {})",
                self.expression_from_input_with_stack(node.id, "table", active)?,
                self.expression_from_input_with_stack(node.id, "key", active)?),
            NodeKind::GetStringTableIdAndKey => {
                let function = crate::text_table_nodes::inspector_function(&pin.key)
                    .ok_or_else(|| TranspileError::GeneratedSourceInvalid("Unknown Text inspector output".into()))?;
                let argument = if self.input_is_connected(node.id, "text")? {
                    if self.native_scope.is_none() {self.graph.validate_inspector_input_is_reproducible(node.id)
                        .map_err(TranspileError::GeneratedSourceInvalid)?;}
                    self.expression_from_input_with_stack(node.id, "text", active)?
                } else { "__rvn_empty_text()".into() };
                format!("{function}({argument})")
            },
            NodeKind::StringTableAsset => {
                let table = property_string(node.id, &node.properties, "table")?;
                if !table.is_empty() {
                    rvn_parser::localized_text::validate_string_table_id(&table).map_err(TranspileError::InvalidIdentifier)?;
                }
                format!("__rvn_string_table_asset({})", quote(&table))
            }
            NodeKind::StringTableId => format!("{}({})", node.table_query_contract().id_function(), self.expression_from_input_with_stack(node.id, "asset", active)?),
            NodeKind::SceneAsset
            | NodeKind::SpriteAsset
            | NodeKind::MusicAsset
            | NodeKind::SoundEffectAsset
            | NodeKind::VoiceAsset
            | NodeKind::CinematicAsset
            | NodeKind::HoverImageAsset
            | NodeKind::ScriptAsset => quote(&property_string(node.id, &node.properties, "path")?),
            NodeKind::TransitionNone
            | NodeKind::TransitionFade
            | NodeKind::TransitionDissolve
            | NodeKind::TransitionSlideLeft
            | NodeKind::TransitionSlideRight
            | NodeKind::TransitionSlideUp
            | NodeKind::TransitionSlideDown
            | NodeKind::TransitionZoomIn
            | NodeKind::TransitionZoomOut
            | NodeKind::TransitionWipe
            | NodeKind::TransitionBlur => quote(&transition_value(node)?),
            NodeKind::ForEach if node.native_loop_contract.is_some()&&pin.key=="array_element"=>{
                if !self.native_loop_owners.borrow().contains(&node.id){return Err(TranspileError::GeneratedSourceInvalid("ArrayElement has no actual reached loop body".into()));}
                let collection=self.expression_from_input_with_stack(node.id,"collection",active)?;
                format!("({collection})[{}]",native_loop_index_name(self.graph.graph_id.get(),node.id.get()))
            },
            NodeKind::ForEach if node.native_loop_contract.is_some()&&pin.key=="array_index"=>native_loop_index_name(self.graph.graph_id.get(),node.id.get()),
            NodeKind::ForLoopWithBreak if node.native_loop_contract.is_some()&&pin.key=="index"=>native_loop_index_name(self.graph.graph_id.get(),node.id.get()),
            NodeKind::ForLoopWithBreak if pin.key=="index" => {
                validate_variable_name(&property_string(node.id,&node.properties,"name")?)?.to_owned()
            }
            NodeKind::FunctionEntry if matches!(self.graph.kind,GraphKind::UnitFunction{..}) => {
                if pin.parameter_id.is_none(){return Err(TranspileError::GeneratedSourceInvalid("Execution/unknown Unit entry pin is not a value".into()));}
                validate_variable_binding_name(&pin.label)?
            }
            NodeKind::VariableGet => {
                validate_variable_name(&property_string(node.id, &node.properties, "name")?)?
                    .to_owned()
            }
            // SET exposes the already assigned value, not the expression that
            // produced it. Re-evaluating a function/random expression here
            // would give consumers a different value and repeat side effects.
            NodeKind::SetVariable | NodeKind::LocalVariable => {
                let name = match node.properties.get("name") {
                    Some(PropertyValue::String(name)) => name.clone(),
                    _ => self.pin_string(node.id, "name")?,
                };
                validate_variable_name(&name)?.to_owned()
            }
            NodeKind::ConvertBoolToFloat => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                // One real Graph producer; the existing strict Bool intrinsic
                // and existing f32 Add0 define its value, never a truthy cast.
                format!("(__rvn_bool_to_int({value}) + 0.0)")
            }
            NodeKind::ConvertBoolToInt => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                format!("__rvn_bool_to_int({value})")
            }
            NodeKind::StringTableRegistered | NodeKind::StringTableEntryRegistered
            | NodeKind::StringTableNamespace | NodeKind::StringTableSource | NodeKind::StringTableMetadata
            | NodeKind::RegisteredStringTables | NodeKind::StringTableKeys | NodeKind::StringTableMetadataIds => {
                let (function, keys, _) = node.string_table_query().unwrap();
                let args = keys.iter().map(|key| self.expression_from_input_with_stack(node.id, key, active))
                    .collect::<Result<Vec<_>, _>>()?;
                format!("{function}({})", args.join(", "))
            }
            NodeKind::ConvertIntToFloat => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                format!("({value} + 0.0)")
            }
            NodeKind::ConvertIntToBool => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                format!("__rvn_int_to_bool({value})")
            }
            NodeKind::ConvertNumberToText => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                format!("(\"\" + {value})")
            }
            NodeKind::ConvertTextToInt => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                format!("to_int({value})")
            }
            NodeKind::ConvertStringToName | NodeKind::ConvertNameToString => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                let function = if node.kind == NodeKind::ConvertStringToName {
                    "__rvn_string_to_name"
                } else {
                    "__rvn_name_to_string"
                };
                format!("{function}({value})")
            }
            NodeKind::ConvertStringToText | NodeKind::ConvertTextToString => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                if node.properties.get("legacy_passthrough") == Some(&PropertyValue::Bool(true)) {
                    value
                } else {
                    let function = if node.kind == NodeKind::ConvertStringToText {
                        "string_to_text"
                    } else {
                        "text_to_string"
                    };
                    format!("{function}({value})")
                }
            }
            NodeKind::VariableReference => {
                quote(&property_string(node.id, &node.properties, "name")?)
            }
            NodeKind::BinaryOperator => {
                let operator = property_string(node.id, &node.properties, "operator")?;
                validate_operator(&operator, false)?;
                let left = self.expression_from_input_with_stack(node.id, "left", active)?;
                let right = self.expression_from_input_with_stack(node.id, "right", active)?;
                format!("({left} {operator} {right})")
            }
            NodeKind::UnaryOperator => {
                let operator = property_string(node.id, &node.properties, "operator")?;
                validate_operator(&operator, true)?;
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                if operator == "not" {
                    format!("(not {value})")
                } else {
                    format!("(-{value})")
                }
            }
            NodeKind::MathAdd
            | NodeKind::StringAppend
            | NodeKind::MathSubtract
            | NodeKind::MathMultiply
            | NodeKind::MathDivide
            | NodeKind::MathEqual
            | NodeKind::MathNotEqual
            | NodeKind::MathLess
            | NodeKind::MathLessEqual
            | NodeKind::MathGreater
            | NodeKind::MathGreaterEqual
            | NodeKind::LogicAnd
            | NodeKind::LogicOr => {
                let operator = explicit_binary_operator(node.kind);
                let input_keys: Vec<_> = node
                    .pins
                    .iter()
                    .map(|pin| &self.graph.pins[pin])
                    .filter(|pin| pin.direction == PinDirection::Input)
                    .map(|pin| pin.key.clone())
                    .collect();
                let mut values = input_keys
                    .iter()
                    .map(|key| self.expression_from_input_with_stack(node.id, key, active));
                let first = values
                    .next()
                    .ok_or_else(|| TranspileError::MissingInputValue {
                        node: node.id,
                        key: "left".into(),
                    })??;
                values.try_fold(first, |left, right| {
                    let right = right?;
                    Ok(format!("({left} {operator} {right})"))
                })?
            }
            NodeKind::LogicNot | NodeKind::MathNegate => {
                let value = self.expression_from_input_with_stack(node.id, "value", active)?;
                if node.kind == NodeKind::LogicNot {
                    format!("(not {value})")
                } else {
                    format!("(-{value})")
                }
            }
            NodeKind::Index => {
                let target = self.expression_from_input_with_stack(node.id, "target", active)?;
                let index = self.expression_from_input_with_stack(node.id, "index", active)?;
                format!("{target}[{index}]")
            }
            NodeKind::FunctionCall => {
                if self.graph.library_call1027(node.id).is_some(){let value=self.qualified_call1027(node.id,rvn_parser::function_library_program1023::LibraryCallMode::Value,active)?;active.remove(&node.id);return Ok(value);}
                let function = property_string(node.id, &node.properties, "function")?;
                if let Some(PropertyValue::Int(count)) = node.properties.get("input_count") {
                    let args = (0..*count)
                        .map(|i| {
                            self.expression_from_input_with_stack(
                                node.id,
                                &format!("item_{i}"),
                                active,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    active.remove(&node.id);
                    return Ok(format!(
                        "{}({})",
                        validate_identifier(&function)?,
                        args.join(", ")
                    ));
                }
                let args = property_string_list(node.id, &node.properties, "args")?;
                format!(
                    "{}({})",
                    validate_identifier(&function)?,
                    args.iter()
                        .map(|value| quote(value))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            NodeKind::ListLiteral => {
                if let Some(PropertyValue::Int(count)) = node.properties.get("input_count") {
                    let items = (0..*count)
                        .map(|i| {
                            self.expression_from_input_with_stack(
                                node.id,
                                &format!("item_{i}"),
                                active,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    active.remove(&node.id);
                    return Ok(format!("[{}]", items.join(", ")));
                }
                let items = property_string_list(node.id, &node.properties, "items")?;
                format!(
                    "[{}]",
                    items
                        .iter()
                        .map(|value| quote(value))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            kind => {
                return Err(TranspileError::UnsupportedExpression {
                    node: node.id,
                    kind,
                })
            }
        };
        active.remove(&pin.node);
        Ok(expression)
    }

    fn connected_character(
        &self,
        node: NodeId,
        key: &str,
        visited: &mut BTreeSet<NodeId>,
    ) -> Option<NodeId> {
        if !visited.insert(node) {
            return None;
        }
        let pin = self.pin(node, key).ok()?;
        let edge = self
            .graph
            .edges
            .values()
            .find(|edge| edge.input == pin.id)?;
        let source = &self.graph.nodes[&self.graph.pins[&edge.output].node];
        match source.kind {
            NodeKind::CharacterValue => Some(source.id),
            NodeKind::Reroute => self.connected_character(source.id, "value", visited),
            _ => None,
        }
    }

    fn connected_sprite(&self, consumer: NodeId) -> Result<Option<String>, TranspileError> {
        let Some(character) = self.connected_character(consumer, "character", &mut BTreeSet::new())
        else {
            return Ok(None);
        };
        let Some(pin) = self.graph.pin_by_key(character, "sprite") else {
            return Ok(None);
        };
        if !self.graph.edges.values().any(|e| e.input == pin.id) {
            return Ok(None);
        }
        let path = self.pin_string(character, "sprite")?;
        if path.trim().is_empty() {
            return Err(TranspileError::MissingInputValue {
                node: character,
                key: "Sprite : chemin d’image vide".into(),
            });
        }
        // A file at the asset root is still a file, never an emotion name.
        Ok(Some(if path.contains('/') {
            path
        } else {
            format!("./{path}")
        }))
    }

    fn emit_character_appearance(
        &self,
        consumer: NodeId,
        character: &str,
        indent: usize,
        source: &mut String,
    ) -> Result<(), TranspileError> {
        let character = validate_identifier(character)?;
        if let Some(path) = self.connected_sprite(consumer)? {
            push_indent(source, indent);
            source.push_str(&format!("{character}.show({})\n", quote(&path)));
        }
        Ok(())
    }

    fn pin_string(&self, node: NodeId, key: &str) -> Result<String, TranspileError> {
        let pin = self.pin(node, key)?;
        if let Some(edge) = self.graph.edges.values().find(|edge| edge.input == pin.id) {
            let source_pin = self
                .graph
                .pins
                .get(&edge.output)
                .ok_or(TranspileError::PinNotFound(edge.output))?;
            let source = self.graph.nodes.get(&source_pin.node).unwrap();
            return match source.kind {
                NodeKind::TextValue => property_string(source.id, &source.properties, "value"),
                NodeKind::LabelValue => property_string(source.id, &source.properties, "label"),
                NodeKind::PositionValue => {
                    property_string(source.id, &source.properties, "position")
                }
                NodeKind::Reroute => self.pin_string(source.id, "value"),
                NodeKind::ConvertStringToText | NodeKind::ConvertTextToString => {
                    self.pin_interpolated_text(source.id, "value")
                }
                NodeKind::CharacterValue => {
                    property_string(source.id, &source.properties, "character")
                }
                NodeKind::SceneAsset
                | NodeKind::SpriteAsset
                | NodeKind::MusicAsset
                | NodeKind::SoundEffectAsset
                | NodeKind::VoiceAsset
                | NodeKind::CinematicAsset
                | NodeKind::HoverImageAsset
                | NodeKind::ScriptAsset => property_string(source.id, &source.properties, "path"),
                NodeKind::VariableReference => {
                    property_string(source.id, &source.properties, "name")
                }
                NodeKind::Literal => property_string(source.id, &source.properties, "value"),
                NodeKind::TransitionNone
                | NodeKind::TransitionFade
                | NodeKind::TransitionDissolve
                | NodeKind::TransitionSlideLeft
                | NodeKind::TransitionSlideRight
                | NodeKind::TransitionSlideUp
                | NodeKind::TransitionSlideDown
                | NodeKind::TransitionZoomIn
                | NodeKind::TransitionZoomOut
                | NodeKind::TransitionWipe
                | NodeKind::TransitionBlur => Ok(transition_value(source)?),
                _ => Err(TranspileError::ExpectedString {
                    node,
                    key: key.into(),
                }),
            };
        }
        match &pin.default_value {
            Some(PropertyValue::String(value)) => Ok(value.clone()),
            Some(_) => Err(TranspileError::ExpectedString {
                node,
                key: key.into(),
            }),
            None => Err(TranspileError::MissingInputValue {
                node,
                key: key.into(),
            }),
        }
    }

    fn pin_interpolated_text(&self, node: NodeId, key: &str) -> Result<String, TranspileError> {
        let pin = self.pin(node, key)?;
        let Some(edge) = self.graph.edges.values().find(|edge| edge.input == pin.id) else {
            // Le texte d'un dialogue est une donnée métier obligatoire. Une
            // ancienne valeur par défaut cachée ne doit jamais être émise si
            // aucune source visible n'est branchée dans le graphe.
            return Err(TranspileError::MissingInputValue {
                node,
                key: key.into(),
            });
        };
        let source_pin = self
            .graph
            .pins
            .get(&edge.output)
            .ok_or(TranspileError::PinNotFound(edge.output))?;
        let source = self.graph.nodes.get(&source_pin.node).unwrap();
        if self.native_scope.is_some() {
            if let Some(name)=crate::native_frame_preparation::dialogue_variable_name(self.graph,node,key) {
                // Gather the complete real chain, but keep the authored locale
                // key and placeholder expression independent of capture IDs.
                self.expression_from_output(source_pin.id,&mut BTreeSet::new())?;
                return Ok(format!("[{}]",validate_variable_name(&name)?));
            }
        }
        if source_pin.text_localization.is_some() || source_pin.text_table_reference.is_some()
            || (source.kind == NodeKind::FormatText
                && source.properties.get("deferred_text") == Some(&PropertyValue::Bool(true))) {
            return Ok(format!("[{}]", self.expression_from_output(source_pin.id, &mut BTreeSet::new())?));
        }
        if matches!(source.kind, NodeKind::VariableGet | NodeKind::SetVariable | NodeKind::LocalVariable) {
            // A SET value output is a typed variable value too. Read the
            // assigned value at runtime, rather than treating it as a literal.
            let name = match source.properties.get("name") {
                Some(PropertyValue::String(name)) => name.clone(),
                _ if matches!(source.kind, NodeKind::SetVariable | NodeKind::LocalVariable) => self.pin_string(source.id, "name")?,
                _ => property_string(source.id, &source.properties, "name")?,
            };
            return Ok(format!("[{}]", validate_variable_name(&name)?));
        }
        if source.kind == NodeKind::FormatText {
            return self.format_text_output(source.id, &mut BTreeSet::new());
        }
        if source.kind == NodeKind::ConvertNumberToText {
            // Interpolation already renders numbers as text; avoid embedding
            // a quoted empty string inside the dialogue's quoted payload.
            return Ok(format!(
                "[{}]",
                self.expression_from_input(source.id, "value")?
            ));
        }
        if source.kind == NodeKind::Reroute {
            return self.pin_interpolated_text(source.id, "value");
        }
        if matches!(
            source.kind,
            NodeKind::ConvertStringToText | NodeKind::ConvertTextToString | NodeKind::ConvertNameToString
        ) {
            if source.properties.get("legacy_passthrough") == Some(&PropertyValue::Bool(true)) {
                // Historical alias wires keep their original template and key.
                return self.pin_interpolated_text(source.id, "value");
            }
            // A new String -> Text cast does not reinterpret string contents as
            // another RVN template. Keep the same strict call as script mode.
            return Ok(format!(
                "[{}]",
                self.expression_from_output(source_pin.id, &mut BTreeSet::new())?
            ));
        }
        if matches!(
            source.kind,
            NodeKind::FunctionCall | NodeKind::MathAdd | NodeKind::StringAppend | NodeKind::Index
        ) || source.kind.is_string_table_query() {
            return Ok(format!(
                "[{}]",
                self.expression_from_output(source_pin.id, &mut BTreeSet::new())?
            ));
        }
        if self.native_scope.is_some() && source.kind==NodeKind::TextValue {
            // Retain the real raw-template node identity. Its constant capture
            // never evaluates interpolation; the actual Dialogue body below
            // remains the original translation key and sole template consumer.
            self.expression_from_output(source_pin.id,&mut BTreeSet::new())?;
        }
        self.pin_string(node, key)
    }

    fn deferred_format_text_output(&self, node: NodeId, active: &mut BTreeSet<NodeId>) -> Result<String, TranspileError> {
        let pattern_pin = self.pin(node, "format")?;
        let pattern = if self.graph.edges.values().any(|e| e.input == pattern_pin.id) {
            self.expression_from_input_with_stack(node, "format", active)?
        } else if let Some(reference) = &pattern_pin.text_table_reference {
            crate::text_pin::emit_table_text(reference).map_err(TranspileError::GeneratedSourceInvalid)?
        } else {
            let text = self.graph.pin_text(pattern_pin.id).map_err(TranspileError::GeneratedSourceInvalid)?
                .ok_or_else(|| TranspileError::MissingInputValue {node, key: "format".into()})?;
            crate::text_pin::emit_authored_text(&text,
                self.graph.nodes[&node].properties.get("@text_explicit_notes") != Some(&PropertyValue::Bool(false)))
        };
        let mut arguments = Vec::new();
        for pin in self.graph.nodes[&node].pins.iter().filter_map(|p| self.graph.pins.get(p))
            .filter(|p| p.direction == PinDirection::Input && p.key.starts_with("argument_")) {
            let value = if pin.default_value.is_none() && !self.graph.edges.values().any(|e| e.input == pin.id) {
                "__rvn_empty_text()".into()
            } else { self.expression_from_input_with_stack(node, &pin.key, active)? };
            arguments.push(format!("{}: {value}", quote(&pin.key["argument_".len()..])));
        }
        Ok(format!("__rvn_format_text({pattern}, {{{}}})", arguments.join(", ")))
    }

    fn format_text_output(
        &self,
        node: NodeId,
        active: &mut BTreeSet<NodeId>,
    ) -> Result<String, TranspileError> {
        let pattern = self.pin_string(node, "format")?;
        let mut result = String::new();
        let mut chars = pattern.chars().peekable();
        while let Some(character) = chars.next() {
            if character == '`' {
                if let Some(escaped) = chars.next() {
                    result.push(escaped);
                }
                continue;
            }
            if character != '{' {
                result.push(character);
                continue;
            }

            let mut marker = String::new();
            let mut closed = false;
            while let Some(next) = chars.next() {
                if next == '}' {
                    closed = true;
                    break;
                }
                marker.push(next);
            }
            if !closed {
                result.push('{');
                result.push_str(&marker);
                break;
            }
            let argument = marker.trim();
            let key = format!("argument_{argument}");
            if self.graph.pin_by_key(node, &key).is_none() {
                result.push('{');
                result.push_str(&marker);
                result.push('}');
                continue;
            }
            let expression = self.expression_from_input_with_stack(node, &key, active)?;
            result.push('[');
            result.push_str(&expression);
            result.push(']');
        }
        Ok(result)
    }

    fn pin_string_list(&self, node: NodeId, key: &str) -> Result<Vec<String>, TranspileError> {
        let pin = self.pin(node, key)?;
        match &pin.default_value {
            Some(PropertyValue::StringList(values)) => Ok(values.clone()),
            Some(_) => Err(TranspileError::ExpectedStringList {
                node,
                key: key.into(),
            }),
            None => Ok(Vec::new()),
        }
    }

    fn constant_input(
        &self,
        node: NodeId,
        key: &str,
        active: &mut BTreeSet<NodeId>,
    ) -> Result<Option<PropertyValue>, TranspileError> {
        if !active.insert(node) {
            return Err(TranspileError::ExpressionCycle(node));
        }
        let pin = self.pin(node, key)?;
        let result = if let Some(edge) = self.graph.edges.values().find(|e| e.input == pin.id) {
            let source = &self.graph.nodes[&self.graph.pins[&edge.output].node];
            match source.kind {
                NodeKind::Literal => source.properties.get("value").cloned(),
                NodeKind::Reroute => self.constant_input(source.id, "value", active)?,
                NodeKind::ConvertBoolToFloat if self.native_scope.is_some() => {
                    match self.constant_input(source.id, "value", active)? {
                        Some(PropertyValue::Bool(value)) => Some(PropertyValue::Float(if value { 1.0 } else { 0.0 })),
                        _ => return Err(TranspileError::GeneratedSourceInvalid("Static BoolToFloat requires its actual boolean constant input".into())),
                    }
                },
                NodeKind::ConvertIntToFloat if self.native_scope.is_some() => {
                    // Match this existing RVN conversion's emitted i64/f32 Add0,
                    // never evaluate arbitrary arithmetic, functions or RNG here.
                    match self.constant_input(source.id, "value", active)? {
                        Some(PropertyValue::Int(value)) => Some(PropertyValue::Float(f64::from(value as f32))),
                        _ => return Err(TranspileError::GeneratedSourceInvalid("Static IntToFloat requires its actual integer constant input".into())),
                    }
                },
                _ => return Err(TranspileError::GeneratedSourceInvalid(format!("La broche {node}.{key} requiert une constante RVN, pas une expression dynamique"))),
            }
        } else {
            pin.default_value.clone()
        };
        active.remove(&node);
        Ok(result)
    }

    fn pin_bool(&self, node: NodeId, key: &str) -> Result<bool, TranspileError> {
        match self.constant_input(node, key, &mut BTreeSet::new())? {
            Some(PropertyValue::Bool(value)) => Ok(value),
            _ => Err(TranspileError::ExpectedBool {
                node,
                key: key.into(),
            }),
        }
    }

    fn pin_int(&self, node: NodeId, key: &str) -> Result<i64, TranspileError> {
        match self.constant_input(node, key, &mut BTreeSet::new())? {
            Some(PropertyValue::Int(value)) => Ok(value),
            _ => Err(TranspileError::ExpectedInt {
                node,
                key: key.into(),
            }),
        }
    }

    fn pin_float(&self, node: NodeId, key: &str) -> Result<f64, TranspileError> {
        match self.constant_input(node, key, &mut BTreeSet::new())? {
            Some(PropertyValue::Float(value)) => Ok(value),
            Some(PropertyValue::Int(value)) => Ok(value as f64),
            _ => Err(TranspileError::ExpectedFloat {
                node,
                key: key.into(),
            }),
        }
    }

    fn push_cinematic_transition(
        &self,
        node: NodeId,
        source: &mut String,
    ) -> Result<(), TranspileError> {
        let value = self.pin_string(node, "transition")?;
        let name = match value.as_str() {
            "" | "none" => return Ok(()),
            "fade" | "fade(500)" => "fade",
            "dissolve" | "dissolve(300)" => "dissolve",
            _ => return Err(TranspileError::GeneratedSourceInvalid("Les illustrations cinématiques RVN acceptent fade/dissolve sans durée personnalisée".into())),
        };
        source.push_str(&format!(" with {name}"));
        Ok(())
    }

    fn push_transition(&self, node: NodeId, source: &mut String) -> Result<(), TranspileError> {
        let transition = self.pin_string(node, "transition")?;
        if !transition.is_empty() && transition != "none" {
            source.push_str(" with ");
            source.push_str(&transition);
        }
        Ok(())
    }

    fn push_optional_transition(
        &self,
        node: NodeId,
        source: &mut String,
    ) -> Result<(), TranspileError> {
        if self.graph.pin_by_key(node, "transition").is_some() {
            self.push_transition(node, source)?;
        }
        Ok(())
    }

    fn input_is_connected(&self, node: NodeId, key: &str) -> Result<bool, TranspileError> {
        let pin = self.pin(node, key)?;
        Ok(self.graph.edges.values().any(|edge| edge.input == pin.id))
    }

    fn successor(&self, node: NodeId, key: &str) -> Result<Option<ExecutionTarget>, TranspileError> {
        let output = self.pin(node, key)?;
        let edges: Vec<_> = self
            .graph
            .edges
            .values()
            .filter(|edge| edge.output == output.id)
            .collect();
        match edges.as_slice() {
            [] => Ok(None),
            [edge] => {
                let input=self.graph.pins.get(&edge.input).ok_or(TranspileError::PinNotFound(edge.input))?;
                if self.graph.nodes[&input.node].kind==NodeKind::ForLoopWithBreak {
                    if input.key=="break" {
                        if self.range_owners.borrow().last()!=Some(&input.node){return Err(TranspileError::ExecutionCycle(input.node));}
                    } else if self.range_owners.borrow().contains(&input.node) {return Err(TranspileError::ExecutionCycle(input.node));}
                }
                let target = ExecutionTarget::PinTarget(edge.input);
                target.node(self.graph)?;
                Ok(Some(target))
            },
            _ => Err(TranspileError::AmbiguousExecutionOutput(output.id)),
        }
    }

    fn pin(&self, node: NodeId, key: &str) -> Result<&GraphPin, TranspileError> {
        self.graph
            .pin_by_key(node, key)
            .ok_or_else(|| TranspileError::MissingPin {
                node,
                key: key.into(),
            })
    }
}

fn property_string(
    node: NodeId,
    properties: &BTreeMap<String, PropertyValue>,
    key: &str,
) -> Result<String, TranspileError> {
    match properties.get(key) {
        Some(PropertyValue::String(value)) => Ok(value.clone()),
        Some(_) => Err(TranspileError::ExpectedString {
            node,
            key: key.into(),
        }),
        None => Err(TranspileError::MissingProperty {
            node,
            key: key.into(),
        }),
    }
}

fn property_int(
    node: NodeId,
    properties: &BTreeMap<String, PropertyValue>,
    key: &str,
) -> Result<i64, TranspileError> {
    match properties.get(key) {
        Some(PropertyValue::Int(value)) => Ok(*value),
        Some(_) => Err(TranspileError::ExpectedInt {
            node,
            key: key.into(),
        }),
        None => Err(TranspileError::MissingProperty {
            node,
            key: key.into(),
        }),
    }
}

fn property_string_list(
    node: NodeId,
    properties: &BTreeMap<String, PropertyValue>,
    key: &str,
) -> Result<Vec<String>, TranspileError> {
    match properties.get(key) {
        Some(PropertyValue::StringList(values)) => Ok(values.clone()),
        Some(_) => Err(TranspileError::ExpectedStringList {
            node,
            key: key.into(),
        }),
        None => Err(TranspileError::MissingProperty {
            node,
            key: key.into(),
        }),
    }
}

fn emit_property_expression(value: &PropertyValue) -> Result<String, TranspileError> {
    match value {
        PropertyValue::Bool(value) => Ok(value.to_string()),
        PropertyValue::Int(value) => Ok(value.to_string()),
        PropertyValue::Float(value) => {
            if !value.is_finite() {
                return Err(TranspileError::NonFiniteFloat(*value));
            }
            let mut text = value.to_string();
            if !text.contains('.') {
                text.push_str(".0");
            }
            Ok(text)
        }
        PropertyValue::String(value) => Ok(quote(value)),
        PropertyValue::Name(value) => Ok(format!("__rvn_literal_name({})", quote(value.as_str()))),
        PropertyValue::Enum(value) => Ok(crate::enum_graph1026::source1026(*value)),
        PropertyValue::StringList(values) => Ok(format!(
            "[{}]",
            values
                .iter()
                .map(|value| quote(value))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn validate_identifier(value: &str) -> Result<&str, TranspileError> {
    let mut chars = value.chars();
    let valid = chars
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    valid
        .then_some(value)
        .ok_or_else(|| TranspileError::InvalidIdentifier(value.into()))
}

fn validate_variable_name(value: &str) -> Result<String, TranspileError> {
    rvn_parser::variable_source_name(value)
        .ok_or_else(|| TranspileError::InvalidIdentifier(value.into()))
}

fn validate_variable_binding_name(value: &str) -> Result<String, TranspileError> {
    rvn_parser::variable_binding_source_name(value)
        .ok_or_else(|| TranspileError::InvalidIdentifier(value.into()))
}

fn validate_operator(operator: &str, unary: bool) -> Result<(), TranspileError> {
    let valid = if unary {
        matches!(operator, "not" | "-")
    } else {
        matches!(
            operator,
            "+" | "-" | "*" | "/" | "==" | "!=" | "<" | "<=" | ">" | ">=" | "and" | "or"
        )
    };
    valid
        .then_some(())
        .ok_or_else(|| TranspileError::InvalidOperator(operator.into()))
}

fn explicit_binary_operator(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::MathAdd | NodeKind::StringAppend => "+",
        NodeKind::MathSubtract => "-",
        NodeKind::MathMultiply => "*",
        NodeKind::MathDivide => "/",
        NodeKind::MathEqual => "==",
        NodeKind::MathNotEqual => "!=",
        NodeKind::MathLess => "<",
        NodeKind::MathLessEqual => "<=",
        NodeKind::MathGreater => ">",
        NodeKind::MathGreaterEqual => ">=",
        NodeKind::LogicAnd => "and",
        NodeKind::LogicOr => "or",
        _ => unreachable!("not an explicit binary operator"),
    }
}

fn transition_value(node: &GraphNode) -> Result<String, TranspileError> {
    let name = match node.kind {
        NodeKind::TransitionNone => return Ok("none".into()),
        NodeKind::TransitionFade => "fade",
        NodeKind::TransitionDissolve => "dissolve",
        NodeKind::TransitionSlideLeft => "slideleft",
        NodeKind::TransitionSlideRight => "slideright",
        NodeKind::TransitionSlideUp => "slideup",
        NodeKind::TransitionSlideDown => "slidedown",
        NodeKind::TransitionZoomIn => "zoomin",
        NodeKind::TransitionZoomOut => "zoomout",
        NodeKind::TransitionWipe => "wipe",
        NodeKind::TransitionBlur => "blur",
        _ => unreachable!("not a transition value node"),
    };
    let duration = match node.properties.get("duration_ms") {
        Some(PropertyValue::Int(value)) if *value >= 0 => *value,
        _ => {
            return Err(TranspileError::MissingProperty {
                node: node.id,
                key: "duration_ms".into(),
            })
        }
    };
    Ok(format!("{name}({duration})"))
}

fn quote(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

fn push_indent(source: &mut String, level: usize) {
    for _ in 0..level {
        source.push_str("    ");
    }
}

#[derive(Debug)]
pub enum TranspileError {
    InvalidGraph(Vec<GraphDiagnostic>),
    UnsupportedGraphKind(GraphKind),
    ExpectedSingleRoot {
        kind: NodeKind,
        found: usize,
    },
    NodeNotFound(NodeId),
    PinNotFound(PinId),
    MissingPin {
        node: NodeId,
        key: String,
    },
    MissingInputValue {
        node: NodeId,
        key: String,
    },
    MissingProperty {
        node: NodeId,
        key: String,
    },
    ExpectedString {
        node: NodeId,
        key: String,
    },
    ExpectedStringList {
        node: NodeId,
        key: String,
    },
    ExpectedBool {
        node: NodeId,
        key: String,
    },
    ExpectedFloat {
        node: NodeId,
        key: String,
    },
    ExpectedInt {
        node: NodeId,
        key: String,
    },
    ExpectedInput(PinId),
    MultipleInputConnections(PinId),
    AmbiguousExecutionOutput(PinId),
    ExecutionCycle(NodeId),
    ExecutionExpansionLimit { node: NodeId, resource: &'static str, limit: usize },
    ExpressionCycle(NodeId),
    MissingBranchEnd(NodeId),
    WrongBranchEnd {
        node: NodeId,
        expected_owner: Option<NodeId>,
        found_owner: NodeId,
    },
    ChoiceWithoutOptions(NodeId),
    UnsupportedStatement {
        node: NodeId,
        kind: NodeKind,
    },
    UnsupportedExpression {
        node: NodeId,
        kind: NodeKind,
    },
    InvalidIdentifier(String),
    InvalidOperator(String),
    InvalidDynamicParameter(String),
    InvalidHotspot(String),
    NonFiniteFloat(f64),
    GeneratedSourceInvalid(String),
}

impl TranspileError {
    /// Élément de graphe à sélectionner lorsqu'une erreur est affichée dans
    /// l'éditeur. Les erreurs globales n'ont volontairement pas de cible.
    pub fn node(&self) -> Option<NodeId> {
        match self {
            Self::NodeNotFound(node)
            | Self::ExecutionCycle(node)
            | Self::ExpressionCycle(node)
            | Self::MissingBranchEnd(node)
            | Self::ChoiceWithoutOptions(node) => Some(*node),
            Self::ExecutionExpansionLimit { node, .. }
            | Self::MissingPin { node, .. }
            | Self::MissingInputValue { node, .. }
            | Self::MissingProperty { node, .. }
            | Self::ExpectedString { node, .. }
            | Self::ExpectedStringList { node, .. }
            | Self::ExpectedBool { node, .. }
            | Self::ExpectedFloat { node, .. }
            | Self::ExpectedInt { node, .. }
            | Self::WrongBranchEnd { node, .. }
            | Self::UnsupportedStatement { node, .. }
            | Self::UnsupportedExpression { node, .. } => Some(*node),
            Self::PinNotFound(_)
            | Self::ExpectedInput(_)
            | Self::MultipleInputConnections(_)
            | Self::AmbiguousExecutionOutput(_)
            | Self::InvalidGraph(_)
            | Self::UnsupportedGraphKind(_)
            | Self::ExpectedSingleRoot { .. }
            | Self::InvalidIdentifier(_)
            | Self::InvalidOperator(_)
            | Self::InvalidDynamicParameter(_)
            | Self::InvalidHotspot(_)
            | Self::NonFiniteFloat(_)
            | Self::GeneratedSourceInvalid(_) => None,
        }
    }

    pub fn pin(&self) -> Option<PinId> {
        match self {
            Self::PinNotFound(pin)
            | Self::ExpectedInput(pin)
            | Self::MultipleInputConnections(pin)
            | Self::AmbiguousExecutionOutput(pin) => Some(*pin),
            _ => None,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidGraph(_) => "invalid_graph",
            Self::UnsupportedGraphKind(_) => "unsupported_graph_kind",
            Self::ExpectedSingleRoot { .. } => "expected_single_root",
            Self::NodeNotFound(_) => "node_not_found",
            Self::PinNotFound(_) => "pin_not_found",
            Self::MissingPin { .. } => "missing_pin",
            Self::MissingInputValue { .. } => "missing_input_value",
            Self::MissingProperty { .. } => "missing_property",
            Self::ExpectedString { .. } => "expected_string",
            Self::ExpectedStringList { .. } => "expected_string_list",
            Self::ExpectedBool { .. } => "expected_bool",
            Self::ExpectedFloat { .. } => "expected_float",
            Self::ExpectedInt { .. } => "expected_int",
            Self::ExpectedInput(_) => "expected_input",
            Self::MultipleInputConnections(_) => "multiple_input_connections",
            Self::AmbiguousExecutionOutput(_) => "ambiguous_execution_output",
            Self::ExecutionCycle(_) => "execution_cycle",
            Self::ExecutionExpansionLimit { .. } => "execution_expansion_limit",
            Self::ExpressionCycle(_) => "expression_cycle",
            Self::MissingBranchEnd(_) => "missing_branch_end",
            Self::WrongBranchEnd { .. } => "wrong_branch_end",
            Self::ChoiceWithoutOptions(_) => "choice_without_options",
            Self::UnsupportedStatement { .. } => "unsupported_statement",
            Self::UnsupportedExpression { .. } => "unsupported_expression",
            Self::InvalidIdentifier(_) => "invalid_identifier",
            Self::InvalidOperator(_) => "invalid_operator",
            Self::InvalidDynamicParameter(_) => "invalid_dynamic_parameter",
            Self::InvalidHotspot(_) => "invalid_hotspot",
            Self::NonFiniteFloat(_) => "non_finite_float",
            Self::GeneratedSourceInvalid(_) => "generated_source_invalid",
        }
    }
}

impl std::fmt::Display for TranspileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if matches!(self, Self::ExecutionExpansionLimit { .. }) {
            formatter.write_str("Ce graphe dépasse la limite de génération du script. Réduisez ses convergences répétées ou sa profondeur.")
        } else {
            write!(formatter, "{self:?}")
        }
    }
}

impl std::error::Error for TranspileError {}

/// PRIVATE: this is a preparation boundary, never an executable publication.
pub(crate) fn prepare_native_emission(graph:&GraphDocument,scope:String) -> Result<TranspiledScriptWithOrigins,TranspileError> {
    prepare_native_emission_impl(graph,scope,false)
}
pub(crate) fn prepare_native_binding_analysis(graph:&GraphDocument,scope:String) -> Result<TranspiledScriptWithOrigins,TranspileError> {
    prepare_native_emission_impl(graph,scope,true)
}
fn prepare_native_emission_impl(graph:&GraphDocument,scope:String,allow_incompatible_links:bool) -> Result<TranspiledScriptWithOrigins,TranspileError> {
    graph.require_native_loop_backend().map_err(TranspileError::GeneratedSourceInvalid)?;
    graph.validate_inspector_evaluation_contract().map_err(TranspileError::GeneratedSourceInvalid)?;
    let diagnostics=graph.validate();
    if diagnostics.iter().any(|d|d.severity==DiagnosticSeverity::Error
        && !(allow_incompatible_links && d.code=="incompatible_types")) {return Err(TranspileError::InvalidGraph(diagnostics));}
    let mut emitter=Emitter::new(graph);emitter.native_scope=Some(scope);
    let source=emitter.emit_document()?;
    let parsed=rvn_parser::parse_with_origins(&source,graph.narrative_context())
        .map_err(|error|TranspileError::GeneratedSourceInvalid(error.to_string()))?;
    let mappings:Vec<_>=emitter.emitted.borrow().iter().map(|(range,node)|(range.clone(),graph.graph_id,*node)).collect();
    let origins=parsed.spans.iter().map(|span|map_statement_origin(span,&mappings)).collect();
    Ok(TranspiledScriptWithOrigins{source,ast:parsed.script,context:graph.narrative_context(),origins})
}


#[cfg(test)]
mod raw_sprite_tint_roundtrip_tests1002 {
    use super::*;
    fn source(raw: &str) -> String {
        format!("scene label start\nhero.effect(tint:{})\n\"Ready\"\nreturn\n",
            rvn_parser::parser::raw_string_token_source(raw).unwrap())
    }
    fn graph(source: &str) -> GraphDocument {
        let script = rvn_parser::parse(source).unwrap();
        crate::import_script(&script).unwrap().into_iter()
            .find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap()
    }
    fn tints(script: &Script) -> Vec<String> {
        let mut values = Vec::new();
        for statement in script {
            match statement {
                rvn_parser::Statement::SpriteEffect { tint: Some(tint), .. } => values.push(tint.clone()),
                rvn_parser::Statement::ScheduledConsumer { body, .. } => values.extend(tints(body)),
                _ => {}
            }
        }
        values
    }
    #[test]
    fn sprite_tint1002_legacy_graph_roundtrip_keeps_raw_ast_and_json_bytes() {
        for raw in [r##"#ffffffff"##, r##"[( [1, \"#ffffffff\"] )[1]]"##, r##"literal\\path\n\t\q"##] {
            let original = source(raw); let ast = rvn_parser::parse(&original).unwrap();
            let authored = graph(&original); let before = serde_json::to_vec(&authored).unwrap();
            let compiled = transpile(&authored).unwrap();
            assert_eq!(tints(&compiled.ast), vec![raw.to_owned()]);
            assert_eq!(compiled.ast, ast);
            assert_eq!(serde_json::to_vec(&authored).unwrap(), before, "serialization never edits the real graph");
            let again = transpile(&graph(&compiled.source)).unwrap();
            assert_eq!(again.ast, ast, "a second save/reopen must not add another escape layer");
        }
    }
    #[test]
    fn sprite_tint1002_native_source_retains_static_capture_and_effect_operand_exactly() {
        let raw = r##"[( [1, \"#ffffffff\"] )[1]]"##;
        let mut authored = graph(&source(raw)); authored.enable_native_frame_preparation().unwrap();
        let before = serde_json::to_vec(&authored).unwrap();
        let compiled = crate::prepare_native_frame_graph(&authored).unwrap();
        assert_eq!(tints(&compiled.ast), vec![raw.to_owned()]);
        let captured = compiled.ast.iter().filter_map(|statement| match statement {
            rvn_parser::Statement::ScheduledConsumer { steps, .. } => Some(steps), _ => None,
        }).flatten().filter_map(|step| match step {
            rvn_parser::ConsumerStep::Capture { value: rvn_parser::Expr::Str(value), .. } if value == raw => Some(value),
            _ => None,
        }).count();
        assert_eq!(captured, 1, "the real static producer captures exactly the raw command operand");
        assert_eq!(rvn_parser::parse_interpolated_str(&tints(&compiled.ast)[0]).unwrap(),
            rvn_parser::parse_interpolated_str(raw).unwrap());
        assert_eq!(serde_json::to_vec(&authored).unwrap(), before);
    }
    #[test]
    fn sprite_tint1002_invalid_raw_pin_data_refuses_without_mutation_or_partial_source() {
        let mut authored = graph(&source("#ffffffff"));
        let literal = authored.nodes.values().find(|node| node.kind == NodeKind::Literal
            && node.properties.get("value") == Some(&PropertyValue::String("#ffffffff".into()))).unwrap().id;
        authored.set_property(literal, "value", PropertyValue::String("unescaped\"quote".into())).unwrap();
        let before = serde_json::to_vec(&authored).unwrap();
        assert!(transpile(&authored).is_err());
        assert_eq!(serde_json::to_vec(&authored).unwrap(), before);
        authored.enable_native_frame_preparation().unwrap();
        let before = serde_json::to_vec(&authored).unwrap();
        assert!(crate::prepare_native_frame_graph(&authored).is_err());
        assert_eq!(serde_json::to_vec(&authored).unwrap(), before);
    }
}

#[cfg(test)]
#[path="codegen_execution_target_tests_1020.rs"]
mod execution_target_tests_1020;

/// Original scoped inputs for readonly binding analysis only. Retained invalid
/// wire types keep the old analysis policy; structural/reference guards and
/// qualified identity checks remain mandatory, and export remains strict.
pub(crate) fn transpile_library_binding_analysis1027<'a>(graph:&'a GraphDocument,project:&'a[GraphDocument],resources:&'a crate::LibraryGraphResources1027<'a>)->Result<TranspiledScriptWithOrigins,TranspileError>{
 if graph.library1027.is_none(){return transpile_for_binding_analysis_in_project(graph,project)}
 graph.require_native_loop_backend().map_err(TranspileError::GeneratedSourceInvalid)?;
 if !graph.inspector_evaluation.is_legacy(){return Err(TranspileError::GeneratedSourceInvalid("Qualified Library binding analysis cannot grant owned Native continuation admission".into()))}
 transpile_origins_library1027(graph,true,Some(project),Some(resources))
}
pub(crate) fn transpile_library_graph1027<'a>(graph:&'a GraphDocument,project:&'a[GraphDocument],resources:&'a crate::LibraryGraphResources1027<'a>)->Result<TranspiledScriptWithOrigins,TranspileError>{
 graph.require_native_loop_backend().map_err(TranspileError::GeneratedSourceInvalid)?;
 if graph.library1027.is_none(){
  // Exactly the existing project-export branch. This is syntactic Source
  // preparation; SourceProject retains its full reconstructed identity/context
  // validation before any publication. No Library metadata grants Native use.
  return if graph.inspector_evaluation.is_legacy(){transpile_with_origins_in_project(graph,project)}else{crate::native_frame_preparation::prepare_native_unchecked(graph)};
 }
 if !graph.inspector_evaluation.is_legacy(){return Err(TranspileError::GeneratedSourceInvalid("Qualified Library NativeFrame/owned continuation admission is not supplied by a source scope".into()))}
 transpile_origins_library1027(graph,false,Some(project),Some(resources))
}
impl Emitter<'_>{
 fn qualified_call1027(&self,node:NodeId,mode:rvn_parser::function_library_program1023::LibraryCallMode,active:&mut BTreeSet<NodeId>)->Result<String,TranspileError>{
  if let Some(current)=self.library_current1027{
   let call=self.graph.library_call1027(node).ok_or_else(||TranspileError::GeneratedSourceInvalid("Current qualified target is absent".into()))?;current.validate_call1027(self.graph,&call.target,mode).map_err(TranspileError::GeneratedSourceInvalid)?;let parameters=current.parameter_ids(&call.target).map_err(TranspileError::GeneratedSourceInvalid)?;
   if parameters.len()!=call.arguments.len(){return Err(TranspileError::GeneratedSourceInvalid("Current Library call has stale argument identity cardinality".into()))}
   let mut args=Vec::new();for id in parameters{let(_,pin)=call.arguments.iter().find(|(p,_)|p==id).ok_or_else(||TranspileError::GeneratedSourceInvalid("Current Library argument has no original target ParameterId".into()))?;let input=&self.graph.pins[pin];let expression=self.expression_from_input_with_stack(node,&input.key,active)?;args.push(format!("{}:{expression}",quote(id.as_str())));}
   let function=match mode{rvn_parser::function_library_program1023::LibraryCallMode::Value=>"functions.value_v1",rvn_parser::function_library_program1023::LibraryCallMode::Completion=>"functions.invoke_v1"};return Ok(format!("{function}({}, {}, {{{}}})",quote(call.target.library.as_str()),quote(&call.target.graph_id.to_string()),args.join(", ")))
  }
  let resources=self.library1027.ok_or_else(||TranspileError::GeneratedSourceInvalid("Original qualified Library resources missing".into()))?;
  let call=self.graph.library_call1027(node).ok_or_else(||TranspileError::GeneratedSourceInvalid("Original qualified Library target absent".into()))?;
  resources.validate_graph(self.graph).map_err(TranspileError::GeneratedSourceInvalid)?;
  let member=resources.member(&call.target).map_err(TranspileError::GeneratedSourceInvalid)?;
  let mut args=Vec::new();for id in member.parameter_ids(){let(_,pin)=call.arguments.iter().find(|(p,_)|p==id).ok_or_else(||TranspileError::GeneratedSourceInvalid("Stale Library argument identity".into()))?;let input=&self.graph.pins[pin];let value=self.expression_from_input_with_stack(node,&input.key,active)?;args.push(format!("{}:{value}",quote(id.as_str())));}
  let function=match mode{rvn_parser::function_library_program1023::LibraryCallMode::Value=>"functions.value_v1",rvn_parser::function_library_program1023::LibraryCallMode::Completion=>"functions.invoke_v1"};
  Ok(format!("{function}({}, {}, {{{}}})",quote(call.target.library.as_str()),quote(&call.target.graph_id.to_string()),args.join(", ")))
 }
}
/// Private authoring stage. The complete-cohort compiler or Current signature
/// projection must revalidate its actual emitted members before exposing data.
/// It is never an executable or publication entry point.
pub(crate) fn emit_library_current_member_candidate1027<'a>(graph:&'a GraphDocument,project:&'a[GraphDocument],current:&'a crate::library_current1027::LibraryCurrentMembers1027<'a>)->Result<TranspiledScriptWithOrigins,TranspileError>{
 graph.require_native_loop_backend().map_err(TranspileError::GeneratedSourceInvalid)?;if !graph.inspector_evaluation.is_legacy(){return Err(TranspileError::GeneratedSourceInvalid("Current Library authoring cannot grant owned Native continuation admission".into()))}
 transpile_origins_current_context1027(graph,false,Some(project),None,Some(current))
}
