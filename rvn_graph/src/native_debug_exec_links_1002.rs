//! Projection of an actual reached LoopBreak source position onto its one
//! reconstructed wire. No evaluator, first-incoming fallback or input grant.
use super::*;
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRangeBreakVisit {
    pub source: PathBuf,
    pub source_ordinal: usize,
    pub definition_root_ordinal: usize,
    pub graph: GraphId,
    pub range: NodeId,
    pub input: PinId,
    pub edge: EdgeId,
    pub predecessor: NodeId,
    pub predecessor_output: PinId,
    pub predecessor_role: String,
    /// Same flattened actual AST route used by native_source_sites.
    pub route: Vec<usize>,
    pub span: Range<usize>,
}

/// Frozen source/CFG presentation, not a runtime observation. `resolve_origin`
/// additionally requires the actual owner's current frozen programme. Its
/// source/span must come from that owner's real debugger observation; accepting
/// descriptive coordinates here cannot authenticate a client-supplied snapshot.
pub struct NativeRangeBreakSourceMap {
    program: ParsedProgram,
    visits: Vec<NativeRangeBreakVisit>,
}
impl NativeRangeBreakSourceMap {
    pub fn from_program(program: &ParsedProgram) -> Result<Self, String> {
        // Full reparsing, authoritative Graph reconstruction and exact source
        // roundtrip precede any CFG association. No standalone codec authority.
        let sites = native_source_sites(program)?;
        let mut definitions = BTreeMap::new();
        for site in &sites {
            if crate::native_frame_preparation::decode_scope(&site.scope)?
                .1
                .exec_links_v1
            {
                let definition = *site
                    .definition_route
                    .first()
                    .ok_or("Missing Range definition")?;
                definitions.insert((site.source.clone(), site.graph, definition), site.root);
            }
        }
        if definitions.is_empty() {
            return Err("Range debug requires actual ExecLinksV1 source".into());
        }
        let mut visits = Vec::new();
        for ((source, graph_id, definition), root) in definitions {
            let source_ordinal = program
                .sources
                .iter()
                .position(|s| s.path.as_ref() == Some(&source))
                .ok_or("Range debug source is absent")?;
            let raw = &program.sources[source_ordinal].source;
            let graphs = crate::empty_native_init_0984::import_frozen_in_context(raw,program)?;
            let graph = graphs
                .iter()
                .find(|g| g.graph_id == graph_id)
                .ok_or("Range debug Graph absent")?;
            // First actual owner adapter is Init. Scene/Game/lexical contexts
            // remain explicit gaps, never guessed from a neighbouring label.
            let Some(Statement::Init { body }) = program.script.get(definition) else {
                return Err(
                    "ExecLinksV1 Range debug currently requires its actual Init definition".into(),
                );
            };
            let origin = program
                .origins
                .get(definition)
                .ok_or("Range debug definition origin absent")?;
            if origin.source.as_ref() != Some(&source) || graph.kind != GraphKind::Init {
                return Err("Range debug definition/source/Graph owner differs".into());
            }
            let mut reader = Reader {
                graph,
                source: &source,
                source_ordinal,
                definition,
                sites: &sites,
                ranges: Vec::new(),
                visits: &mut visits,
            };
            let start = reader.output(root, "exec_out")?;
            reader.block(body, &origin.span.children, &[definition], start)?;
        }
        if visits.is_empty() {
            return Err("Range debug has no actual LoopBreak occurrence".into());
        }
        let mut seen = BTreeSet::new();
        for visit in &visits {
            if !seen.insert((visit.source.clone(), visit.span.start, visit.span.end)) {
                return Err("Range debug source span aliases two traversal occurrences".into());
            }
        }
        Ok(Self {
            program: program.clone(),
            visits,
        })
    }
    pub fn visits(&self) -> &[NativeRangeBreakVisit] {
        &self.visits
    }
    /// Exact positions only: containing the Range header/body is insufficient.
    /// This returns observation data; it never installs a breakpoint or steps
    /// a VM. Instance/epoch admission remains the existing Engine owner's job.
    pub fn resolve_origin(
        &self,
        actual_program: &ParsedProgram,
        source: &Path,
        span: &Range<usize>,
    ) -> Result<Option<&NativeRangeBreakVisit>, String> {
        if actual_program != &self.program {
            return Err("Range debug owner frozen programme changed".into());
        }
        if !actual_program
            .sources
            .iter()
            .any(|s| s.path.as_deref() == Some(source))
        {
            return Err("Range debug observation has a foreign source".into());
        }
        Ok(self
            .visits
            .iter()
            .find(|visit| visit.source == source && visit.span == *span))
    }
}

struct Reader<'a> {
    graph: &'a GraphDocument,
    source: &'a PathBuf,
    source_ordinal: usize,
    definition: usize,
    sites: &'a [NativeSourceSite],
    ranges: Vec<NodeId>,
    visits: &'a mut Vec<NativeRangeBreakVisit>,
}
impl Reader<'_> {
    fn output(&self, node: NodeId, role: &str) -> Result<Option<PinId>, String> {
        let pin = self
            .graph
            .pin_by_key(node, role)
            .ok_or_else(|| format!("Range debug output {node}.{role} absent"))?;
        if pin.node != node
            || pin.direction != PinDirection::Output
            || pin.value_type != ValueType::Execution
        {
            return Err("Range debug predecessor is not an actual execution output".into());
        }
        Ok(Some(pin.id))
    }
    fn wire(&self, output: Option<PinId>, node: NodeId, role: &str) -> Result<GraphEdge, String> {
        let output = output.ok_or("Range debug path has no actual predecessor")?;
        let input = self
            .graph
            .pin_by_key(node, role)
            .ok_or("Range debug target pin absent")?;
        if input.direction != PinDirection::Input || input.value_type != ValueType::Execution {
            return Err("Range debug target is not an actual execution input".into());
        }
        let wires: Vec<_> = self
            .graph
            .edges
            .values()
            .filter(|e| e.output == output)
            .collect();
        match wires.as_slice() {
            [edge] if edge.input == input.id => Ok((**edge).clone()),
            _ => Err(
                "Range debug lexical predecessor differs from the actual sole successor wire"
                    .into(),
            ),
        }
    }
    fn block(
        &mut self,
        body: &[Statement],
        spans: &[StatementSpan],
        base: &[usize],
        previous: Option<PinId>,
    ) -> Result<Option<PinId>, String> {
        self.block_offset(body, spans, base, previous, 0)
    }
    fn block_offset(
        &mut self,
        body: &[Statement],
        spans: &[StatementSpan],
        base: &[usize],
        mut previous: Option<PinId>,
        offset: usize,
    ) -> Result<Option<PinId>, String> {
        if body.len() != spans.len() || base.len() > 128 {
            return Err("Range debug actual block/span/depth mismatch".into());
        }
        for (index, (statement, span)) in body.iter().zip(spans).enumerate() {
            let mut route = base.to_vec();
            route.push(offset + index);
            previous = self.statement(statement, span, &route, previous)?;
        }
        Ok(previous)
    }
    fn statement(
        &mut self,
        statement: &Statement,
        span: &StatementSpan,
        route: &[usize],
        previous: Option<PinId>,
    ) -> Result<Option<PinId>, String> {
        if route.len() > 128 || self.visits.len() >= 65_536 {
            return Err("Range debug CFG budget exceeded".into());
        }
        if matches!(statement, Statement::LoopBreak) {
            let range = *self
                .ranges
                .last()
                .ok_or("Range debug Break lacks its nearest actual Range")?;
            let edge = self.wire(previous, range, "break")?;
            let pin = self
                .graph
                .pins
                .get(&edge.output)
                .ok_or("Range debug predecessor pin absent")?;
            self.visits.push(NativeRangeBreakVisit {
                source: self.source.clone(),
                source_ordinal: self.source_ordinal,
                definition_root_ordinal: self.definition,
                graph: self.graph.graph_id,
                range,
                input: edge.input,
                edge: edge.id,
                predecessor: pin.node,
                predecessor_output: edge.output,
                predecessor_role: pin.key.clone(),
                route: route.to_vec(),
                span: span.range.clone(),
            });
            // The graph writer ends this lexical emission at the Break wire.
            // Existing engaged If/loop continuations still run in the real VM;
            // this source projection neither truncates them nor gathers inputs.
            return Ok(None);
        }
        let Statement::ScheduledConsumer { body, .. } = statement else {
            return Err("Range debug path contains an unsupported unowned statement".into());
        };
        let site = self
            .sites
            .iter()
            .find(|s| {
                s.source == *self.source
                    && s.graph == self.graph.graph_id
                    && s.route.as_slice() == route
            })
            .ok_or("Range debug consumer lacks its exact frozen source site")?;
        let node_id = site.consumer;
        let node = self
            .graph
            .nodes
            .get(&node_id)
            .ok_or("Range debug consumer node absent")?;
        let kind = node.kind;
        let demand = node.native_loop_contract.is_some();
        self.wire(previous, node_id, "exec_in")?;
        if kind == NodeKind::BranchEnd {
            if !body.is_empty() || !span.children.is_empty() {
                return Err("Range debug BranchEnd contains commands".into());
            }
            return Ok(None);
        }
        let [command] = body.as_slice() else {
            return Err("Range debug consumer requires its one actual command".into());
        };
        let [inner] = span.children.as_slice() else {
            return Err("Range debug consumer command span mismatch".into());
        };
        let mut base = route.to_vec();
        base.push(0);
        match command {
            Statement::If {
                then_branch,
                else_branch,
                ..
            } if kind == NodeKind::If => {
                let split = then_branch.len();
                let (then_spans, else_spans) = inner
                    .children
                    .split_at_checked(split)
                    .ok_or("Range debug If span split mismatch")?;
                self.block(
                    then_branch,
                    then_spans,
                    &base,
                    self.output(node_id, "then")?,
                )?;
                // Flattened origin indices retain the actual then offset.
                self.block_offset(
                    else_branch,
                    else_spans,
                    &base,
                    self.output(node_id, "else")?,
                    split,
                )?;
                self.output(node_id, "completed")
            }
            Statement::ForLoopWithBreak { body, .. } if kind == NodeKind::ForLoopWithBreak => {
                if !demand {
                    return Err("Range debug requires actual demand Range contract".into());
                }
                self.ranges.push(node_id);
                let result =
                    self.block(body, &inner.children, &base, self.output(node_id, "body")?);
                self.ranges.pop();
                result?;
                self.output(node_id, "completed")
            }
            Statement::While { body, .. } | Statement::ForEach { body, .. } => {
                if !matches!(kind, NodeKind::While | NodeKind::ForEach) {
                    return Err("Range debug loop kind differs".into());
                }
                self.block(body, &inner.children, &base, self.output(node_id, "body")?)?;
                self.output(node_id, "completed")
            }
            Statement::Sequence { .. } | Statement::Choice { .. } | Statement::Imagemap { .. } => {
                Err("Range debug Sequence/Choice/Imagemap continuation adapter pending".into())
            }
            Statement::Return | Statement::FunctionReturn { .. } | Statement::Jump { .. } => {
                Ok(None)
            }
            _ => self.output(node_id, "exec_out"),
        }
    }
}
