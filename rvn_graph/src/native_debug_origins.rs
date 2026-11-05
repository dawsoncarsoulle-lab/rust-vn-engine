//! Source-authoritative Graph endpoint provenance. This does not invent Core
//! instructions or authorize client-supplied breakpoint/cursor objects.
use crate::*;
#[path="native_debug_exec_links_1002.rs"]
mod range_break_debug1002;
pub use range_break_debug1002::{NativeRangeBreakSourceMap, NativeRangeBreakVisit};
use logos::Logos;
use rvn_parser::{lexer::Token, ConsumerStep, Expr, ParsedProgram, Statement, StatementSpan};
use std::{collections::BTreeSet, ops::Range, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeOriginRole {
    Declaration,
    Consumer,
    ConsumerBody,
    Producer {
        step: usize,
        inspector: bool,
        legacy_terminal: Option<String>,
        native_loop_output:Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativePortOrigin {
    pub role: String,
    pub pin: PinId,
    pub incoming: Option<EdgeId>,
    pub direction: PinDirection,
    pub value_type: ValueType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeDebugOrigin {
    pub source: PathBuf,
    pub source_ordinal: usize,
    pub definition_root_ordinal: usize,
    pub graph: GraphId,
    pub node: NodeId,
    pub kind: NodeKind,
    pub ports: Vec<NativePortOrigin>,
    pub role: NativeOriginRole,
    /// Original UTF-8 bytes, including CRLF and comments between tokens.
    pub span: Range<usize>,
    /// Producer value only; never a copied enclosing statement range.
    pub expression: Option<Range<usize>>,
    pub route: Vec<usize>,
    pub scope: String,
}

/// Construction is deliberately restricted to verified source bytes. A caller
/// may inspect records, but cannot deserialize a replacement authority.
pub struct NativeDebugSourceMap {
    origins: Vec<NativeDebugOrigin>,
}
impl NativeDebugSourceMap {
    pub fn from_workspace(workspace: &SourceWorkspace) -> Result<Self, String> {
        Self::from_program(&workspace.freeze_native_program()?)
    }
    pub fn from_program(program: &ParsedProgram) -> Result<Self, String> {
        let sites = native_source_sites(program)?;
        // The source graph may now preserve several legitimate Break ingress
        // edges. Until a real predecessor/visit trace is wired here, refuse
        // this debug boundary explicitly instead of reporting the first edge.
        for site in &sites {
            if crate::native_frame_preparation::decode_scope(&site.scope)?.1.exec_links_v1 {
                return Err("ExecLinksV1 debug traversal requires actual predecessor/visit evidence; source preparation alone grants no debugger authority".into());
            }
        }
        let mut graphs = Vec::new();
        for source in &program.sources {
            let parsed = rvn_parser::parse_with_origins(&source.source, program.context)
                .map_err(|e| e.to_string())?;
            let script: Vec<_> = parsed
                .script
                .into_iter()
                .filter(|s| !matches!(s, Statement::Use { .. }))
                .collect();
            if crate::native_source_binding::contains_native(&script)
                || crate::empty_native_init_0984::source_has_declarations(&source.source,program.context)? {
                graphs.push((
                    source
                        .path
                        .as_ref()
                        .ok_or("Native debug source requires actual path")?,
                    crate::empty_native_init_0984::import_frozen_in_context(&source.source,program)?,
                ));
            }
        }
        let mut origins = Vec::new();
        let mut declarations = BTreeSet::new();
        for site in &sites {
            let source_ordinal = program
                .sources
                .iter()
                .position(|s| s.path.as_ref() == Some(&site.source))
                .ok_or("Missing native source ordinal")?;
            let raw = &program.sources[source_ordinal].source;
            let graph = graphs
                .iter()
                .filter(|(path, _)| **path == site.source)
                .flat_map(|(_, graphs)| graphs)
                .find(|g| g.graph_id == site.graph)
                .ok_or("Missing reconstructed native graph")?;
            let definition = site.definition_route[0];
            let make = |node: NodeId,
                        role,
                        span,
                        expression,
                        route|
             -> Result<NativeDebugOrigin, String> {
                let actual = graph
                    .nodes
                    .get(&node)
                    .ok_or("Source origin references absent real node")?;
                let ports = actual
                    .pins
                    .iter()
                    .map(|id| {
                        let pin = graph
                            .pins
                            .get(id)
                            .ok_or("Source origin references absent real pin")?;
                        Ok(NativePortOrigin {
                            role: pin.key.clone(),
                            pin: *id,
                            incoming: graph.edges.values().find(|e| e.input == *id).map(|e| e.id),
                            direction: pin.direction,
                            value_type: pin.value_type.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(NativeDebugOrigin {
                    source: site.source.clone(),
                    source_ordinal,
                    definition_root_ordinal: definition,
                    graph: site.graph,
                    node,
                    kind: actual.kind,
                    ports,
                    role,
                    span,
                    expression,
                    route,
                    scope: site.scope.clone(),
                })
            };
            if declarations.insert((source_ordinal, site.graph, definition)) {
                let span = program
                    .origins
                    .get(definition)
                    .ok_or("Absent real definition origin")?;
                if span.source.as_ref() != Some(&site.source) {
                    return Err("Definition/source association differs".into());
                }
                origins.push(make(
                    site.root,
                    NativeOriginRole::Declaration,
                    span.span.range.clone(),
                    None,
                    vec![definition],
                )?);
            }
            origins.push(make(
                site.consumer,
                NativeOriginRole::Consumer,
                site.span.range.clone(),
                None,
                site.route.clone(),
            )?);
            let Statement::ScheduledConsumer { steps, body, .. } = &site.plan else {
                return Err("Expected actual native plan".into());
            };
            for (index, (step, (span, expression))) in steps
                .iter()
                .zip(producer_ranges(raw, &site.span, steps)?)
                .enumerate()
            {
                let (id, inspector) = match step {
                    ConsumerStep::Capture { producer, .. } => (producer, false),
                    ConsumerStep::InspectTextTable { producer, .. } => (producer, true),
                };
                let (identity, legacy_terminal) =
                    crate::native_frame_preparation::decode_producer(id)?;
                let role = NativeOriginRole::Producer {
                    step: index,
                    inspector,
                    legacy_terminal,
                    native_loop_output:if identity.kind==NodeKind::ForEach&&graph.nodes.get(&identity.node).is_some_and(|n|n.native_loop_contract.is_some()){Some("array_element".into())}else{None},
                };
                origins.push(make(
                    identity.node,
                    role,
                    span,
                    Some(expression),
                    site.route.clone(),
                )?);
            }
            fn body_spans<'a>(
                statements: &[&Statement],
                spans: &'a [StatementSpan],
                route: &[usize],
                result: &mut Vec<(&'a StatementSpan, Vec<usize>)>,
            ) -> Result<(), String> {
                if statements.len() != spans.len() {
                    return Err("Native body/span count mismatch".into());
                }
                if route.len() > 128 {
                    return Err("Native debug route budget exceeded".into());
                }
                for (index, (statement, span)) in statements.iter().zip(spans).enumerate() {
                    // Nested plans have their own authoritative consumer node.
                    if matches!(statement, Statement::ScheduledConsumer { .. }) {
                        continue;
                    }
                    let mut next = route.to_vec();
                    next.push(index);
                    if result.len() >= 262_144 {
                        return Err("Native body origin budget exceeded".into());
                    }
                    result.push((span, next.clone()));
                    let children: Vec<&Statement> = match statement {
                        Statement::If {
                            then_branch,
                            else_branch,
                            ..
                        } => then_branch.iter().chain(else_branch).collect(),
                        Statement::Sequence { branches } => branches.iter().flatten().collect(),
                        Statement::Choice { options } => options.iter().flat_map(|option| &option.body).collect(),
                        Statement::Imagemap { hotspots, .. } => hotspots.iter().flat_map(|hotspot| &hotspot.body).collect(),
                        Statement::While { body, .. }
                        | Statement::ForEach { body, .. }
                        | Statement::ForLoopWithBreak { body, .. } => body.iter().collect(),
                        _ => Vec::new(),
                    };
                    body_spans(&children, &span.children, &next, result)?;
                }
                Ok(())
            }
            let mut spans = Vec::new();
            body_spans(
                &body.iter().collect::<Vec<_>>(),
                &site.span.children,
                &site.route,
                &mut spans,
            )?;
            for (span, route) in spans {
                origins.push(make(
                    site.consumer,
                    NativeOriginRole::ConsumerBody,
                    span.range.clone(),
                    None,
                    route,
                )?);
            }
            if origins.len() > 262_144 {
                return Err("Native debug origin budget exceeded".into());
            }
        }
        crate::empty_native_init_0984::append_debug_origins(program,&mut origins)?;
        Ok(Self { origins })
    }
    pub fn origins(&self) -> &[NativeDebugOrigin] {
        &self.origins
    }
    pub fn at_byte(&self, source: &std::path::Path, byte: usize) -> Vec<&NativeDebugOrigin> {
        let contained: Vec<_> = self
            .origins
            .iter()
            .filter(|origin| origin.source == source && origin.span.contains(&byte))
            .collect();
        let minimum = contained.iter().map(|origin| origin.span.len()).min();
        contained
            .into_iter()
            .filter(|origin| Some(origin.span.len()) == minimum)
            .collect()
    }
    /// A producer's multi-output node has one genuine step; each terminal is
    /// selected by its real role and PinId, not by three artificial nodes.
    pub fn endpoint(
        &self,
        source: &std::path::Path,
        graph: GraphId,
        node: NodeId,
        pin: PinId,
    ) -> Vec<&NativeDebugOrigin> {
        self.origins
            .iter()
            .filter(|o| {
                o.source == source
                    && o.graph == graph
                    && o.node == node
                    && o.ports.iter().any(|p| {
                        p.pin == pin
                            && match &o.role {
                                NativeOriginRole::Producer{native_loop_output:Some(role),..}=> &p.role==role,
                                NativeOriginRole::Producer {
                                    legacy_terminal: Some(role),
                                    ..
                                } => &p.role == role,
                                _ => true,
                            }
                    })
            })
            .collect()
    }
}

fn expression(raw: &str) -> Result<Expr, String> {
    let parsed =
        rvn_parser::parse(&format!("set __origin_probe = {raw}\n")).map_err(|e| e.to_string())?;
    match parsed.as_slice() {
        [Statement::SetVar { value, .. }] => Ok(value.clone()),
        _ => Err("Producer source probe was not exactly one expression".into()),
    }
}

/// Locate through the very lexer used by the parser, then reparse each located
/// literal/value and compare with the actual frozen step. No emitted offsets,
/// stale cache ranges, textual label trust or AST from a client is accepted.
fn producer_ranges(
    raw: &str,
    span: &StatementSpan,
    steps: &[ConsumerStep],
) -> Result<Vec<(Range<usize>, Range<usize>)>, String> {
    if steps.is_empty() {
        return Ok(Vec::new());
    }
    let end = span
        .children
        .first()
        .ok_or("Producer plan has no genuine body span")?
        .range
        .start;
    let prefix = raw
        .get(span.range.start..end)
        .ok_or("Producer source range invalid UTF-8")?;
    let mut tokens = Vec::new();
    for (token, range) in Token::lexer(prefix).spanned() {
        if tokens.len() >= 262_144 {
            return Err("Native producer lexer token budget exceeded".into());
        }
        tokens.push((token.map_err(|_| "Producer source lexical error")?, range));
    }
    let mut depth = 0usize;
    let mut markers = Vec::new();
    for (i, (token, _)) in tokens.iter().enumerate() {
        if depth == 1
            && matches!(token, Token::Ident("capture" | "inspect"))
            && matches!(tokens.get(i + 1).map(|t| &t.0), Some(Token::String(_)))
            && matches!(tokens.get(i + 2).map(|t| &t.0), Some(Token::Assign))
        {
            if !matches!(tokens.get(i + 1).map(|t| &t.0), Some(Token::String(_)))
                || !matches!(tokens.get(i + 2).map(|t| &t.0), Some(Token::Assign))
            {
                return Err("Malformed actual producer header".into());
            }
            markers.push(i);
        }
        match token {
            Token::BraceOpen | Token::ParenOpen | Token::BracketOpen => depth += 1,
            Token::BraceClose | Token::ParenClose | Token::BracketClose => {
                depth = depth.checked_sub(1).ok_or("Producer delimiter mismatch")?
            }
            _ => {}
        }
    }
    if markers.len() != steps.len() {
        return Err("Actual producer lexer/step count mismatch".into());
    }
    let offset = span.range.start;
    let mut ranges = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        let begin = markers[index];
        let next = markers.get(index + 1).copied().unwrap_or(tokens.len());
        let (id, value, inspector) = match step {
            ConsumerStep::Capture { producer, value } => (producer, value, false),
            ConsumerStep::InspectTextTable { producer, value } => (producer, value, true),
        };
        let literal = raw
            .get(offset + tokens[begin + 1].1.start..offset + tokens[begin + 1].1.end)
            .ok_or("Actual producer literal UTF-8 range invalid")?;
        if expression(literal)? != Expr::Str(id.clone())
            || matches!(tokens[begin].0, Token::Ident("inspect")) != inspector
        {
            return Err("Actual producer identity/kind differs from frozen plan".into());
        }
        let last = (begin + 3..next)
            .rev()
            .find(|i| !matches!(tokens[*i].0, Token::Newline))
            .ok_or("Empty actual producer expression")?;
        let expression_span = offset + tokens[begin + 3].1.start..offset + tokens[last].1.end;
        if expression(
            raw.get(expression_span.clone())
                .ok_or("Actual producer expression UTF-8 range invalid")?,
        )? != *value
        {
            return Err("Actual producer expression differs from frozen plan".into());
        }
        ranges.push((
            offset + tokens[begin].1.start..expression_span.end,
            expression_span,
        ));
    }
    Ok(ranges)
}
