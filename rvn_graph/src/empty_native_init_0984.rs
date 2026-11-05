//! Declarative identity of an actually empty Init. The source comment is not
//! an AST statement, execution plan, owner, input token or debugger consumer.
use crate::*;
use logos::Logos;
use rvn_parser::{
    lexer::Token, NarrativeContext, ParsedProgram, ParsedSource, Script, Statement, StatementSpan,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ops::Range};

const RESERVED: &str = "// @rvn-native-empty-init";
const V1: &str = "// @rvn-native-empty-init:v1 ";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    scope: String,
    schema: u32,
}
pub(crate) struct EmptyInit {
    pub scope: String,
    pub graph: GraphId,
    pub root: crate::native_frame_preparation::NativeNodeIdentity,
    pub schema: u32,
    pub span: StatementSpan,
}

/// Inspect lexer gaps, never string contents or a regular expression over RVN.
/// Logos skips only spaces and // comments; every other token keeps its range.
fn markers(source: &str) -> Result<Vec<(Range<usize>, &str)>, String> {
    if !source.contains(RESERVED) {
        return Ok(Vec::new());
    }
    fn gap<'a>(
        source: &'a str,
        range: Range<usize>,
        out: &mut Vec<(Range<usize>, &'a str)>,
    ) -> Result<(), String> {
        let text = &source[range.clone()];
        let comment = text.trim_start_matches([' ', '\t']);
        if comment.starts_with(RESERVED) {
            if !out.is_empty() {
                return Err("Repeated Empty Native Init declaration".into());
            }
            let start = range.start + text.len() - comment.len();
            out.push((start..range.end, comment));
        }
        Ok(())
    }
    let mut out = Vec::new();
    let mut end = 0;
    let mut count = 0usize;
    for (token, range) in Token::lexer(source).spanned() {
        token.map_err(|_| "Empty Native Init source has a lexer error")?;
        count = count
            .checked_add(1)
            .ok_or("Empty Native Init token budget")?;
        gap(source, end..range.start, &mut out)?;
        end = range.end;
    }
    gap(source, end..source.len(), &mut out)?;
    if !out.is_empty() && (source.len() > 16 * 1024 * 1024 || count > 262_144) {
        return Err("Empty Native Init source/token budget exceeded".into());
    }
    Ok(out)
}

pub(crate) fn declarations(source: &str, parsed: &ParsedSource) -> Result<Vec<EmptyInit>, String> {
    let comments = markers(source)?;
    if comments.is_empty() {
        return Ok(Vec::new());
    }
    if comments.len() != 1 || parsed.script.len() != parsed.spans.len() {
        return Err(
            "Empty Native Init declaration is repeated or its source origins differ".into(),
        );
    }
    let (range, comment) = &comments[0];
    let json = comment
        .strip_prefix(V1)
        .ok_or("Unknown or malformed Empty Native Init declaration version")?;
    if json.len() > 512 {
        return Err("Empty Native Init declaration budget".into());
    }
    let wire: Wire = serde_json::from_str(json)
        .map_err(|e| format!("Malformed Empty Native Init declaration: {e}"))?;
    if !matches!(
        wire.schema,
        GRAPH_NATIVE_FRAME_SCHEMA_VERSION
            | GRAPH_MIXED_INSPECTOR_SCHEMA_VERSION
            | GRAPH_NATIVE_LOOP_DEMAND_SCHEMA_VERSION
    ) {
        return Err("Unsupported Empty Native Init graph schema".into());
    }
    let (graph, root) = crate::native_frame_preparation::decode_scope(&wire.scope)?;
    if root.kind != NodeKind::Init
        || !root.static_inputs.is_empty()
        || !root.dynamic_inputs.is_empty()
        || root.legacy_string_sequence
    {
        return Err("Empty Native Init declaration does not identify its genuine Init root".into());
    }
    let mut catalog = GraphDocument::new(GraphId::new(1), GraphKind::Init);
    let id = catalog
        .add_catalog_node(NodeKind::Init, [0., 0.])
        .map_err(|e| e.to_string())?;
    let actual = &catalog.nodes[&id];
    if root.ports.len() != actual.pins.len()
        || root
            .ports
            .iter()
            .zip(&actual.pins)
            .any(|((key, _, incoming), pin)| {
                let pin = &catalog.pins[pin];
                key != &pin.key
                    || incoming.is_some()
                    || pin.direction != PinDirection::Output
                    || !pin.value_type.is_execution()
            })
    {
        return Err("Empty Native Init root ports differ from the actual catalogue".into());
    }
    let matches: Vec<_> = parsed
        .script
        .iter()
        .zip(&parsed.spans)
        .filter(|(_, span)| span.range.start <= range.start && range.end <= span.range.end)
        .collect();
    let [(Statement::Init { body }, span)] = matches.as_slice() else {
        return Err(
            "Empty Native Init declaration is not inside its real top-level Init span".into(),
        );
    };
    if !body.is_empty() || !span.children.is_empty() {
        return Err("Empty Native Init declaration cannot accompany executable children".into());
    }
    // The existing exporter can serialize the shared character registry in a
    // separate Init before this root. Preserve that actual declaration AST;
    // it is not the marked empty root, a consumer or an execution identity.
    // Other commands or another empty Init would be ambiguous in the existing
    // single-Init Graph model and are deliberately refused, never merged away.
    for (statement, other) in parsed.script.iter().zip(&parsed.spans) {
        if other == *span { continue; }
        if let Statement::Init { body } = statement {
            if body.is_empty() || !body.iter().all(|s|matches!(s,Statement::CharacterCreate{..})) {
                return Err("Empty Native Init has an ambiguous or executable second Init definition".into());
            }
        }
    }
    Ok(vec![EmptyInit {
        scope: wire.scope,
        graph,
        root,
        schema: wire.schema,
        span: (**span).clone(),
    }])
}

pub(crate) fn source_has_declarations(
    source: &str,
    context: NarrativeContext,
) -> Result<bool, String> {
    if !source.contains(RESERVED) {
        return Ok(false);
    }
    let parsed = rvn_parser::parse_with_origins(source, context).map_err(|e| e.to_string())?;
    Ok(!declarations(source, &parsed)?.is_empty())
}
// Frozen Program validation has source authority, not an authoring transaction's
// Unit parameter owners. Validate the declared empty root independently; full
// graph imports and Native binding retain their own strict entry points.
pub(crate) fn program_has_declarations(program: &ParsedProgram) -> Result<bool, String> {
    let mut found = false;
    for source in &program.sources {
        if !source.source.contains(RESERVED) {
            continue;
        }
        let parsed = rvn_parser::parse_with_origins(&source.source, program.context)
            .map_err(|e| e.to_string())?;
        let declarations = declarations(&source.source, &parsed)?;
        if let Some(declaration) = declarations.first() {
            let mut graph = GraphDocument::new(declaration.graph, GraphKind::Init);
            graph
                .add_catalog_node(NodeKind::Init, [0., 0.])
                .map_err(|e| e.to_string())?;
            apply(std::slice::from_mut(&mut graph), &declarations)?;
            found = true;
        }
    }
    Ok(found)
}

pub(crate) fn empty_graph(graph: &GraphDocument) -> bool {
    graph.kind == GraphKind::Init
        && graph.nodes.len() == 1
        && graph.edges.is_empty()
        && graph.nodes.values().all(|node| node.kind == NodeKind::Init)
}
pub(crate) fn emit(graph: &GraphDocument) -> Result<String, String> {
    if !empty_graph(graph) || graph.inspector_evaluation.is_legacy() {
        return Err(
            "Empty Native Init identity requires the explicit root-only Native graph".into(),
        );
    }
    graph.validate_inspector_evaluation_contract()?;
    let scope = crate::native_frame_preparation::native_scope(graph).map_err(|e| e.to_string())?;
    Ok(format!(
        "    {V1}{}\n",
        serde_json::to_string(&Wire {
            scope,
            schema: graph.schema_version
        })
        .map_err(|e| e.to_string())?
    ))
}
fn apply(result: &mut [GraphDocument], declarations: &[EmptyInit]) -> Result<(), String> {
    if declarations.is_empty() {
        return Ok(());
    }
    let declaration = &declarations[0];
    let init: Vec<_> = result
        .iter()
        .enumerate()
        .filter(|(_, graph)| graph.kind == GraphKind::Init)
        .map(|(i, _)| i)
        .collect();
    let [index] = init.as_slice() else {
        return Err("Empty Native Init source did not reconstruct one Init graph".into());
    };
    let graph = &mut result[*index];
    if !empty_graph(graph) || !graph.inspector_evaluation.is_legacy() {
        return Err("Empty Native Init declaration cannot replace an executable graph".into());
    }
    let previous = *graph.nodes.keys().next().ok_or("Empty Init root absent")?;
    graph.restore_native_source_identities(&BTreeMap::from([(
        previous,
        declaration.root.clone(),
    )]))?;
    graph.graph_id = declaration.graph;
    graph.schema_version = declaration.schema;
    graph.inspector_evaluation = InspectorEvaluationContract::NativeFrameV1;
    graph.validate_inspector_evaluation_contract()?;
    if graph
        .validate()
        .iter()
        .any(|d| d.severity == DiagnosticSeverity::Error)
    {
        return Err("Empty Native Init declaration did not reconstruct a valid graph".into());
    }
    if crate::native_frame_preparation::native_scope(graph).map_err(|e| e.to_string())?
        != declaration.scope
    {
        return Err("Empty Native Init identity is not the canonical root/pin encoding".into());
    }
    Ok(())
}

/// Source-aware import is the only new public entry point. AST-only import
/// deliberately cannot restore an identity which the AST does not contain.
pub fn import_native_frame_source(
    source: &str,
    context: NarrativeContext,
) -> Result<Vec<GraphDocument>, String> {
    let parsed = rvn_parser::parse_with_origins(source, context).map_err(|e| e.to_string())?;
    let script = parsed
        .script
        .iter()
        .filter(|s| !matches!(s, Statement::Use { .. }))
        .cloned()
        .collect();
    import_in_context(source, &script, None)
}
pub(crate) fn import_in_context(
    source: &str,
    script: &Script,
    context: Option<&Script>,
) -> Result<Vec<GraphDocument>, String> {
    import_in_context_with_owners(source,script,context,&[])
}
fn import_in_context_with_owners(
    source:&str,script:&Script,context:Option<&Script>,owners:&[GraphDocument],
)->Result<Vec<GraphDocument>,String>{
    let parsed = rvn_parser::parse_with_origins(
        source,
        NarrativeContext::from_script(context.unwrap_or(script)),
    )
    .map_err(|e| e.to_string())?;
    let authored: Script = parsed
        .script
        .iter()
        .filter(|s| !matches!(s, Statement::Use { .. }))
        .cloned()
        .collect();
    let supplied: Script = script
        .iter()
        .filter(|s| !matches!(s, Statement::Use { .. }))
        .cloned()
        .collect();
    if authored != supplied {
        return Err("Empty Native Init source/AST association differs".into());
    }
    let declarations = declarations(source, &parsed)?;
    if declarations.is_empty() {
        return crate::native_frame_preparation::import_native_frame_script_with_owners(
            script, context, owners,
        );
    }
    let mut result = crate::import::import_native_preparation_with_owners(script,context,owners)?;
    apply(&mut result, &declarations)?;
    crate::native_frame_preparation::finish_native_import(script, result)
}
// Inspect actual AST values in every physical Unit definition, independently
// of whether this source needs authoring-owner-free reconstruction. No text
// spelling, captured reference resolution, expression evaluation or owner mint.
use rvn_parser::Expr;
fn text_expression(segment:&rvn_parser::TextSegment)->Option<&Expr>{match segment{rvn_parser::TextSegment::Lit(_)=>None,rvn_parser::TextSegment::Interp(value)=>Some(value)}}
/// Every Statement field is named deliberately: adding an AST variant/field
/// must re-audit this walk at compilation, rather than silently skip an Expr.
/// Opaque historical asset/method strings are not parsed as expressions.
fn frozen_unit_expressions(statement: &Statement)->Vec<&Expr> {
    match statement {
        Statement::LibraryUnitCall{call}=>call.arguments().iter().map(|(_,value)|value).collect(),
        Statement::StandardMacroDispatch{dispatch}=>dispatch.operand_expressions(),
        Statement::ScheduledConsumer{version:_,scope:_,consumer:_,steps,body:_}=>steps.iter().map(|step|match step {rvn_parser::ConsumerStep::Capture{producer:_,value}|rvn_parser::ConsumerStep::InspectTextTable{producer:_,value}=>value}).collect(),
        Statement::SetVar{name:_,value}|Statement::LocalVar{name:_,value}|Statement::FunctionReturn{value}=>vec![value],
        Statement::UnitCall{name:_,arguments}=>arguments.iter().collect(),
        Statement::ForEach{name:_,collection,body:_}=>vec![collection],Statement::ForLoopWithBreak{name:_,first,last,body:_}=>vec![first,last],
        Statement::If{condition,then_branch:_,else_branch:_}|Statement::While{condition,body:_}=>vec![condition],
        Statement::Dialogue{character_id:_,text}=>text.0.iter().filter_map(text_expression).collect(),
        Statement::Choice{options}=>options.iter().flat_map(|o|{let rvn_parser::ChoiceOption{label,condition,body:_}=o;condition.iter().chain(label.0.iter().filter_map(text_expression))}).collect(),
        Statement::WidgetAddToViewport{object,z_order}=>vec![object,z_order],
        Statement::WidgetRemoveFromParent{object}=>vec![object],
        Statement::UiOpen{name,arguments,modal,layer,story:_}=>vec![name,arguments,modal,layer],
        Statement::UiClose{name}|Statement::VideoPause{name}|Statement::VideoResume{name}|Statement::VideoStop{name}|Statement::VideoSkip{name}|Statement::VideoWait{name}=>vec![name],
        Statement::UiFocus{name,element}=>vec![name,element],Statement::UiSetState{name,element,state}=>vec![name,element,state],
        Statement::MenuExecute{request}=>vec![request],Statement::MotionPlay{target,definition}=>vec![target,definition],
        Statement::MotionStop{target}|Statement::MotionWait{target}=>vec![target],
        Statement::CharacterCompose{character,definition}=>vec![character,definition],Statement::CharacterAttributes{character,attributes}=>vec![character,attributes],
        Statement::VideoPlay{name,definition}=>vec![name,definition],Statement::VideoSeek{name,seconds}=>vec![name,seconds],Statement::VideoVolume{name,volume}=>vec![name,volume],
        Statement::AccessibilityConfigure{settings}=>vec![settings],Statement::AccessibilitySpeak{text}=>vec![text],
        Statement::MusicVolumeValue{level}=>vec![level],
        Statement::TypewriterSetValue{enabled}=>vec![enabled],
        Statement::TypewriterSpeedValue{chars_per_sec}=>vec![chars_per_sec],
        Statement::FunctionLibrary{scope:_}|Statement::Use{paths:_}|Statement::Init{body:_}
        |Statement::Function{name:_,parameters:_,body:_}|Statement::UnitFunction{name:_,parameters:_,body:_}|Statement::UnitReturn|Statement::Screen{name:_,parameters:_,body:_}|Statement::Handler{name:_,parameters:_,body:_}
        |Statement::AccessibilityStop|Statement::Sequence{branches:_}|Statement::LoopBreak
        |Statement::Config{key:_,value:_}|Statement::CharacterCreate{id:_,display_name:_}
        |Statement::Label{name:_}|Statement::SceneLabel{name:_}|Statement::Jump{target:_}|Statement::Call{target:_}|Statement::Return
        |Statement::Scene{background:_,transition:_}|Statement::CinematicShow{id:_,transition:_}|Statement::CinematicHide{transition:_}|Statement::UnlockEnding{id:_}
        |Statement::ShowSprite{character_id:_,emotion:_,position:_,transition:_}|Statement::HideSprite{character_id:_,transition:_}
        |Statement::MoveSprite{character_id:_,position:_,transition:_}|Statement::SpriteAnimate{character_id:_,animation:_,params:_}
        |Statement::SpriteStopAnimation{character_id:_}|Statement::SpriteEffect{character_id:_,flip_x:_,flip_y:_,scale:_,rotation:_,tint:_}
        |Statement::Timer{duration_secs:_,action:_}|Statement::TimerCancel|Statement::MethodCall{target:_,method:_,arg:_,transition:_}
        |Statement::MusicPlay{file:_,transition:_}|Statement::MusicStop{transition:_}|Statement::MusicVolume{level:_}
        |Statement::SfxPlay{file:_,transition:_}|Statement::SfxStop{file:_,transition:_}|Statement::VoicePlay{file:_}|Statement::VoiceStop
        |Statement::Imagemap{background:_,hover_image:_,hotspots:_}|Statement::TypewriterSet{enabled:_}|Statement::TypewriterSpeed{chars_per_sec:_}=>vec![],
    }
}
fn frozen_unit_children(statement: &Statement)->Vec<&[Statement]> {
    match statement {
        Statement::FunctionLibrary{scope}=>scope.members().iter().map(|member|std::slice::from_ref(member.declaration())).collect(),
        Statement::Init{body}|Statement::Function{body,..}|Statement::UnitFunction{body,..}|Statement::Handler{body,..}|Statement::Screen{body,..}
        |Statement::ScheduledConsumer{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}|Statement::While{body,..}=>vec![body],
        Statement::If{then_branch,else_branch,..}=>vec![then_branch,else_branch],
        Statement::Sequence{branches}=>branches.iter().map(Vec::as_slice).collect(),
        Statement::StandardMacroDispatch{dispatch}=>dispatch.branches.iter().map(|branch|branch.body.as_slice()).collect(),
        Statement::Choice{options}=>options.iter().map(|o|o.body.as_slice()).collect(),
        Statement::Imagemap{hotspots,..}=>hotspots.iter().map(|h|h.body.as_slice()).collect(),_=>vec![],
    }
}
pub(crate) fn validate_frozen_unit_macro_data(script: &Script) -> Result<(), String> {
    let mut blocks = vec![(script.as_slice(), false)];
    while let Some((body, inside_unit)) = blocks.pop() {
        for statement in body {
            let inside_unit = inside_unit || matches!(statement, Statement::UnitFunction { .. });
            if inside_unit {
                let mut expressions = frozen_unit_expressions(statement);
                while let Some(expression) = expressions.pop() {
                    match expression {
                        Expr::StandardMacroData { .. } => return Err("Frozen Native source cannot place Native macro data in a schema14 Unit definition".into()),
                        Expr::LibraryCall{call}=>expressions.extend(call.arguments().iter().map(|(_,value)|value)),
                        Expr::Call { args, .. } | Expr::ListLit(args) => expressions.extend(args),
                        Expr::BinOp { left, right, .. } | Expr::Index { target: left, index: right }
                        | Expr::And(left, right) | Expr::Or(left, right) => {
                            expressions.push(left); expressions.push(right);
                        }
                        Expr::Neg(value) | Expr::Not(value) => expressions.push(value),
                        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) | Expr::Var(_) => {},
                    }
                }
            }
            blocks.extend(frozen_unit_children(statement).into_iter().map(|body| (body, inside_unit)));
        }
    }
    Ok(())
}

// Existing complete imports need no new resolution certificate. Classify only
// missing authoring owners, never an error string from a failed import attempt.
fn needs_frozen_unit_projection(script: &Script, context: &Script) -> bool {
    let parameters: std::collections::BTreeSet<_> = context.iter().filter_map(|s|match s {
        Statement::UnitFunction { name, parameters, .. } if !parameters.is_empty() => Some(name.as_str()), _=>None,
    }).collect();
    if script.iter().any(|s|matches!(s,Statement::UnitFunction{parameters,..}if !parameters.is_empty())) { return true; }
    let mut pending = vec![script.as_slice()];
    while let Some(body) = pending.pop() {
        for statement in body {
            match statement {
                Statement::UnitCall { name, .. } if parameters.contains(name.as_str()) => return true,
                Statement::Init{body}|Statement::Function{body,..}|Statement::UnitFunction{body,..}
                |Statement::Screen{body,..}|Statement::Handler{body,..}|Statement::ScheduledConsumer{body,..}
                |Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>pending.push(body),
                Statement::If{then_branch,else_branch,..}=>{pending.push(then_branch);pending.push(else_branch);},
                Statement::Sequence{branches}=>pending.extend(branches.iter().map(Vec::as_slice)),
                Statement::StandardMacroDispatch{dispatch}=>pending.extend(dispatch.branches.iter().map(|b|b.body.as_slice())),
                Statement::Choice{options}=>pending.extend(options.iter().map(|o|o.body.as_slice())),
                Statement::Imagemap{hotspots,..}=>pending.extend(hotspots.iter().map(|h|h.body.as_slice())),
                _=>{},
            }
        }
    }
    false
}

/// Runtime/source projection carries actual frozen bytes, never authoring IDs.
/// Newly admitted explicit imports must prove their complete resolution order;
/// old complete imports retain their existing wildcard/alias behavior.
pub(crate) fn import_frozen_in_context(
    source: &str, program: &ParsedProgram,
) -> Result<Vec<GraphDocument>,String> {
    let context = &program.script;
    // Complete imports and skipped foreign Unit roots obey the same data gate.
    // The resolved AST is separately bound to every physical source/origin by
    // validate_frozen_source_graphs (or the certified projection path below).
    validate_frozen_unit_macro_data(context)?;
    let parsed = rvn_parser::parse_with_origins(source, NarrativeContext::from_script(context))
        .map_err(|e|e.to_string())?;
    let script: Script = parsed.script.iter()
        .filter(|statement| !matches!(statement, Statement::Use { .. })).cloned().collect();
    if !needs_frozen_unit_projection(&script, context) {
        return import_in_context(source, &script, Some(context));
    }
    if !program.sources.iter().any(|physical|physical.source==source) {
        return Err("Mixed Unit frozen reconstruction requires the exact supplied physical source bytes".into());
    }
    crate::native_source_binding::prove_explicit_frozen_use_order(program)?;
    let declarations = declarations(source, &parsed)?;
    let mut result = crate::import::import_frozen_native_scopes(&script, context, !declarations.is_empty())?;
    apply(&mut result, &declarations)?;
    crate::native_frame_preparation::finish_native_import(&script, result)
}

pub(crate) fn reimport_in_context(
    source: &str,
    script: &Script,
    previous: &[GraphDocument],
    context: Option<&Script>,
) -> Result<Vec<GraphDocument>, String> {
    let result = import_in_context_with_owners(source, script, context, previous)?;
    crate::native_frame_preparation::reconcile_native_presentation(
        result, script, previous, context,
    )
}

pub(crate) fn append_debug_origins(
    program: &ParsedProgram,
    origins: &mut Vec<NativeDebugOrigin>,
) -> Result<(), String> {
    for (source_ordinal, source) in program.sources.iter().enumerate() {
        let parsed = rvn_parser::parse_with_origins(&source.source, program.context)
            .map_err(|e| e.to_string())?;
        for declaration in declarations(&source.source, &parsed)? {
            let path = source
                .path
                .as_ref()
                .ok_or("Empty Native Init debug origin requires the actual source path")?;
            let matching: Vec<_> = program
                .script
                .iter()
                .zip(&program.origins)
                .enumerate()
                .filter(|(_, (statement, origin))| {
                    origin.source.as_ref() == Some(path)
                        && origin.span == declaration.span
                        && matches!(statement, Statement::Init { body } if body.is_empty())
                })
                .collect();
            let [(definition_root_ordinal, _)] = matching.as_slice() else {
                return Err("Empty Native Init definition/source origin is not unique".into());
            };
            let graphs = import_frozen_in_context(&source.source, program)?;
            let graph = graphs
                .iter()
                .find(|g| g.graph_id == declaration.graph)
                .ok_or("Missing Empty Native Init reconstructed graph")?;
            let node = &graph.nodes[&declaration.root.node];
            let ports = node
                .pins
                .iter()
                .map(|id| {
                    let pin = &graph.pins[id];
                    NativePortOrigin {
                        role: pin.key.clone(),
                        pin: *id,
                        incoming: None,
                        direction: pin.direction,
                        value_type: pin.value_type.clone(),
                    }
                })
                .collect();
            origins.push(NativeDebugOrigin {
                source: path.clone(),
                source_ordinal,
                definition_root_ordinal: *definition_root_ordinal,
                graph: declaration.graph,
                node: node.id,
                kind: NodeKind::Init,
                ports,
                role: NativeOriginRole::Declaration,
                span: declaration.span.range,
                expression: None,
                route: vec![*definition_root_ordinal],
                scope: declaration.scope,
            });
            if origins.len() > 262_144 {
                return Err("Native debug origin budget exceeded".into());
            }
        }
    }
    Ok(())
}

/// SourceDocument deliberately skips AST-identical rewrites and preserves all
/// ordinary comments. This reserved, validated declarative contract is the sole
/// exception owned here: remove its exact lexer gap, delegate actual AST edits
/// to the existing writer, then insert the new declaration at the genuine brace.
/// No executable statement or character registry is synthesized by this port.
pub(crate) fn replace_definition(
    document: &mut rvn_parser::SourceDocument,
    range: Range<usize>,
    baseline: Option<&str>,
    replacement: &str,
) -> Result<bool, String> {
    let context = document.narrative_context();
    let original = document.source();
    if !original.contains(RESERVED) && !replacement.contains(RESERVED) { return Ok(false); }
    let old_parsed = rvn_parser::parse_with_origins(original, context).map_err(|e| e.to_string())?;
    let old_declarations = declarations(original, &old_parsed)?;
    let new_parsed = rvn_parser::parse_with_origins(replacement, context).map_err(|e| e.to_string())?;
    let new_declarations = declarations(replacement, &new_parsed)?;
    let old = old_declarations.first().filter(|declaration| {
        range.clone().any(|index| document.statements().get(index)
            .is_some_and(|statement| statement.range == declaration.span.range))
    });
    if old.is_none() && new_declarations.is_empty() { return Ok(false); }
    if range.len() > 1 || (old.is_some() && range.len() != 1)
        || (!new_declarations.is_empty() && new_parsed.script.len() != 1) {
        return Err("Empty Native Init rewrite does not identify one actual definition".into());
    }
    fn strip(source: &str) -> Result<String, String> {
        let mut result = source.to_owned();
        for (range, _) in markers(source)?.into_iter().rev() { result.replace_range(range, ""); }
        Ok(result)
    }
    let source = if old.is_some() { strip(original)? } else { original.to_owned() };
    let mut candidate = rvn_parser::SourceDocument::parse_with_context(source, context)
        .map_err(|e| e.to_string())?;
    let replacement_plain = strip(replacement)?;
    let baseline_plain = baseline.map(strip).transpose()?;
    let current = candidate.source().to_owned();
    candidate.replace_statement_range_from_baseline(range.clone(), &current,
        baseline_plain.as_deref(), &replacement_plain).map_err(|e| e.to_string())?;
    if !new_declarations.is_empty() {
        let statement = candidate.statements().get(range.start).ok_or("Empty Init insertion disappeared")?;
        if !matches!(&statement.statement, Statement::Init { body } if body.is_empty()) {
            return Err("Empty Native Init rewrite acquired an executable child".into());
        }
        let fragment = &candidate.source()[statement.range.clone()];
        let brace = Token::lexer(fragment).spanned().find_map(|(token, range)|
            matches!(token, Ok(Token::BraceOpen)).then_some(range.end))
            .ok_or("Empty Init lexical opening brace missing")?;
        let comment = markers(replacement)?.into_iter().next().ok_or("Empty Init declaration missing")?.1;
        let mut source = candidate.source().to_owned();
        source.insert_str(statement.range.start + brace, &format!("\n{comment}\n"));
        let rebuilt = rvn_parser::SourceDocument::parse_with_context(source, context).map_err(|e|e.to_string())?;
        if rebuilt.statements().iter().map(|s| &s.statement).ne(candidate.statements().iter().map(|s| &s.statement)) {
            return Err("Empty Init declarative edit changed its AST".into());
        }
        let parsed = rvn_parser::parse_with_origins(rebuilt.source(), context).map_err(|e|e.to_string())?;
        let actual = declarations(rebuilt.source(), &parsed)?;
        if actual.len() != 1 || actual[0].scope != new_declarations[0].scope
            || actual[0].schema != new_declarations[0].schema {
            return Err("Empty Init declaration did not retain its exact source contract".into());
        }
        candidate = rebuilt;
    }
    *document = candidate;
    Ok(true)
}


#[cfg(test)]
mod explicit_frozen_use_tests1021 {
    use super::*;
    use std::{fs,path::PathBuf};

    #[test]
    fn frozen_native1021_explicit_use_keeps_foreign_registry_and_unit_data_boundary_without_owners() {
        let root=std::env::var("RVN_NATIVE_SOURCE_TEST_DIR").map(PathBuf::from).unwrap_or_else(|_|std::env::temp_dir())
            .join(format!("rvn-frozen-use-registry-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(root.join("deps")).unwrap();let root=fs::canonicalize(root).unwrap();
        let path=root.join("main.rvn");let imported=root.join("deps/characters.rvn");
        let raw=r#"init {character.create("guide","Guide世界")}
handler probe(event) {local marker=1}
"#;
        let mut handler=crate::import_script(&rvn_parser::parse(raw).unwrap()).unwrap().into_iter()
            .find(|g|matches!(&g.kind,GraphKind::Handler{name}if name=="probe")).unwrap();
        handler.enable_native_frame_preparation().unwrap();
        let emitted=crate::prepare_native_frame_graph(&handler).unwrap();
        let memory_source=format!("function unit_shadow(alpha) -> () {{return}}\n{}",emitted.source);
        let memory=rvn_parser::parse_program(&memory_source).unwrap();
        assert_eq!(memory.sources.len(),1);assert!(memory.sources[0].path.is_none());
        assert!(memory.origins.iter().all(|origin|origin.source.is_none()));
        let restored=import_frozen_in_context(&memory_source,&memory).unwrap();
        let actual=restored.iter().find(|g|g.graph_id==handler.graph_id).unwrap();
        assert_eq!(actual.characters,handler.characters);assert_eq!(actual.edges,handler.edges);
        assert!(actual.nodes.keys().eq(handler.nodes.keys()));assert!(actual.pins.keys().eq(handler.pins.keys()));
        assert!(import_native_frame_source(&memory_source,memory.context).unwrap_err().contains("allocator"));
        // Optional memory identity is not an actual file/debug source path.
        // Existing public Graph source-site admission must remain unchanged.
        assert!(crate::native_source_sites(&memory).unwrap_err().contains("requires real path"));
        let mut forged=memory.clone();forged.origins[0].span.range.end+=1;
        assert!(import_frozen_in_context(&memory_source,&forged).unwrap_err().contains("single-source AST/origin"));
        let parsed=rvn_parser::parse_with_origins(&emitted.source,NarrativeContext::Legacy).unwrap();
        let init=parsed.script.iter().position(|s|matches!(s,Statement::Init{body}if body.iter().all(|s|matches!(s,Statement::CharacterCreate{..})))).unwrap();
        let span=parsed.spans[init].range.clone();let characters=emitted.source[span.clone()].to_owned();
        let mut body=emitted.source;body.replace_range(span,"");
        let main=format!("use \"deps/characters.rvn\"\nfunction unit_shadow(alpha) -> () {{return}}\n{body}");
        fs::write(&path,&main).unwrap();fs::write(&imported,&characters).unwrap();
        let program=rvn_parser::parse_file_program_with_uses(&path).unwrap();
        let actual=import_frozen_in_context(&main,&program).unwrap();
        let graph=actual.iter().find(|g|g.graph_id==handler.graph_id).unwrap();
        assert_eq!(graph.characters,handler.characters,"the complete real imported registry reaches Native reconstruction");
        assert!(graph.nodes.keys().eq(handler.nodes.keys()));assert!(graph.pins.keys().eq(handler.pins.keys()));assert_eq!(graph.edges,handler.edges);
        assert!(!actual.iter().any(|g|matches!(g.kind,GraphKind::UnitFunction{..})),"no authoring Unit or ParameterId is invented");
        assert!(import_native_frame_source(&main,program.context).unwrap_err().contains("allocator"),"fresh authoring remains separate from frozen reconstruction");
        assert!(!crate::native_source_sites(&program).unwrap().is_empty());
        assert!(import_frozen_in_context(&format!("{main}\n// detached source bytes\n"),&program)
            .unwrap_err().contains("exact supplied physical source bytes"));
        let duplicate=format!("{characters}\ninit {{character.create(\"guide\",\"Duplicate\")}}\n");
        fs::write(&imported,&duplicate).unwrap();let parsed=rvn_parser::parse_file_program_with_uses(&path).unwrap();
        assert!(crate::native_source_sites(&parsed).is_err(),"foreign duplicate characters remain a real source error");
        // The skipped foreign Unit must not hide actual Native macro data.
        // Both sites come from real catalogue nodes and Native lowering.
        for (kind,method) in [(NodeKind::DoN,"flow.macro_counter_v1"),(NodeKind::FlipFlop,"flow.macro_is_a_v1")] {
            let mut graph=crate::import_script(&rvn_parser::parse("handler probe(event) {local marker=1}\nhandler data_probe(event){}\n").unwrap()).unwrap().into_iter().find(|g|matches!(&g.kind,GraphKind::Handler{name}if name=="data_probe")).unwrap();
            let entry=graph.nodes.values().find(|n|n.kind==NodeKind::HandlerEntry).unwrap().id;
            let node=graph.add_catalog_node(kind,[300.,0.]).unwrap();
            graph.connect(graph.pin_by_key(entry,"exec_out").unwrap().id,graph.pin_by_key(node,"exec_in").unwrap().id).unwrap();
            graph.enable_native_frame_preparation().unwrap();let generated=crate::prepare_native_frame_graph(&graph).unwrap();
            let parsed=rvn_parser::parse(&generated.source).unwrap();
            let dispatch=parsed.iter().find_map(|s|match s{Statement::Handler{body,..}=>body.iter().find_map(|s|match s{Statement::StandardMacroDispatch{dispatch}=>Some(dispatch),_=>None}),_=>None}).unwrap();
            dispatch.validate().unwrap();
            let data=format!("{method}({}, {})",serde_json::to_string(&dispatch.site.scope_source()).unwrap(),serde_json::to_string(&dispatch.site.node_source()).unwrap());
            for spelling in [data.clone(),data.replacen('.',"\n.\n",1),data.replacen('(',"\n(",1)] {
                let bad=format!("{characters}\nfunction foreign(alpha) -> () {{set value={spelling} return}}\n");
                fs::write(&imported,&bad).unwrap();let parsed=rvn_parser::parse_file_program_with_uses(&path).unwrap();
                let unit=parsed.script.iter().find_map(|s|match s{Statement::UnitFunction{name,body,..}if name=="foreign"=>Some(body),_=>None}).unwrap();
                assert!(matches!(&unit[0],Statement::SetVar{value:rvn_parser::Expr::StandardMacroData{site,..},..}if site.as_ref()==&dispatch.site));
                assert!(crate::native_source_sites(&parsed).unwrap_err().contains("Native macro data in a schema14 Unit"));
                // Native A neither defines nor invokes a parameterized Unit.
                // Foreign B has no dispatch/consumer: only the real data Expr.
                // This takes the historical complete-import path for A.
                let complete_main = main.replace("function unit_shadow(alpha) -> () {return}\n", "");
                fs::write(&path, &complete_main).unwrap();
                let complete = rvn_parser::parse_file_program_with_uses(&path).unwrap();
                let local = rvn_parser::parse_with_origins(&complete_main, complete.context).unwrap();
                assert!(!needs_frozen_unit_projection(&local.script, &complete.script));
                assert!(!crate::native_source_binding::contains_native(&rvn_parser::parse_with_origins(&bad, complete.context).unwrap().script));
                assert!(import_frozen_in_context(&complete_main, &complete).unwrap_err().contains("Native macro data in a schema14 Unit"));
                assert!(crate::native_source_sites(&complete).unwrap_err().contains("Native macro data in a schema14 Unit"));
                assert_eq!(fs::read_to_string(&path).unwrap(), complete_main);
                assert_eq!(fs::read_to_string(&imported).unwrap(), bad);
                // No Native marker/dispatch anywhere: the whole-program gate
                // must still reject data reads in both Unit signature forms.
                for parameters in ["", "alpha"] {
                    let only=format!("function solo({parameters}) -> () {{set value={spelling} return}}\n");
                    fs::write(&path,&only).unwrap();
                    let unit_only=rvn_parser::parse_file_program_with_uses(&path).unwrap();
                    assert!(!crate::native_source_binding::contains_native(&unit_only.script));
                    assert!(crate::native_source_sites(&unit_only).unwrap_err().contains("Native macro data in a schema14 Unit"));
                    assert!(import_frozen_in_context(&only,&unit_only).unwrap_err().contains("Native macro data in a schema14 Unit"));
                    assert_eq!(fs::read_to_string(&path).unwrap(),only);
                }
                fs::write(&path, &main).unwrap();
                let comment=spelling.replace('\n',"\n// ");
                let inert=format!("{characters}\nfunction foreign(alpha) -> () {{set value={} // {comment}\nreturn}}\n",serde_json::to_string(&spelling).unwrap());
                fs::write(&imported,&inert).unwrap();let parsed=rvn_parser::parse_file_program_with_uses(&path).unwrap();
                crate::native_source_sites(&parsed).unwrap();
            }
        }
        fs::write(&imported,format!("{characters}\nfunction foreign(alpha) -> () {{return}}\n")).unwrap();
        // Actual live capture admits these routes; the None fallback must still refuse them.
        // Older Native A + unused Unit B retains the exact old import route.
        let old_main=format!("use \"deps/characters.rvn\"\n{body}");
        for route in ["deps/*.rvn","deps/../deps/characters.rvn"] {
            let unsupported=main.replacen("deps/characters.rvn",route,1);fs::write(&path,&unsupported).unwrap();
            let parsed=rvn_parser::parse_file_program_with_uses(&path).unwrap();
            assert!(!crate::native_source_sites(&parsed).unwrap().is_empty());
            let mut uncertified=parsed.clone();uncertified.resolution=None;
            assert!(crate::native_source_sites(&uncertified).unwrap_err().contains("resolution certificate"));
            let old=old_main.replacen("deps/characters.rvn",route,1);fs::write(&path,&old).unwrap();
            let parsed=rvn_parser::parse_file_program_with_uses(&path).unwrap();
            assert!(!crate::native_source_sites(&parsed).unwrap().is_empty(),"an older complete-import path must not acquire the new mixed certificate requirement");
        }
        fs::write(&path,&main).unwrap();fs::write(&imported,&characters).unwrap();
        assert_eq!(rvn_parser::parse_file_program_with_uses(&path).unwrap(),program);
    }
    #[test]
    fn frozen_native1021_unit_macro_data_visits_real_deferred_operand_ast() {
        // Real lowering creates the site, pure DAG, captures and operand_v1.
        // No hand-built Site/Dispatch/Consumer AST is used by this regression.
        let mut graph=crate::import_script(&rvn_parser::parse("init {set source_value=1}\nhandler data_probe(event){set probe=source_value+2}").unwrap()).unwrap()
            .into_iter().find(|g|matches!(&g.kind,GraphKind::Handler{name}if name=="data_probe")).unwrap();
        let set=graph.nodes.values().find(|n|n.kind==NodeKind::SetVariable).unwrap().id;
        let value=graph.pin_by_key(set,"value").unwrap().id;
        let sum=graph.pins[&graph.edges.values().find(|e|e.input==value).unwrap().output].node;
        let left=graph.pin_by_key(sum,"left").unwrap().id;
        let variable=graph.pins[&graph.edges.values().find(|e|e.input==left).unwrap().output].node;
        assert_eq!(graph.nodes[&variable].kind,NodeKind::VariableGet);
        let execution=graph.pin_by_key(set,"exec_in").unwrap().id;
        assert_eq!(graph.disconnect_pin(execution),1);
        graph.remove_node(variable).unwrap();
        let entry=graph.nodes.values().find(|n|n.kind==NodeKind::HandlerEntry).unwrap().id;
        let first=graph.add_catalog_node(NodeKind::DoN,[300.,0.]).unwrap();
        let second=graph.add_catalog_node(NodeKind::DoN,[600.,0.]).unwrap();
        let wire=|graph:&mut GraphDocument,a:NodeId,output:&str,b:NodeId,input:&str|{
            graph.connect(graph.pin_by_key(a,output).unwrap().id,graph.pin_by_key(b,input).unwrap().id).unwrap();
        };
        wire(&mut graph,entry,"exec_out",first,"exec_in");
        wire(&mut graph,first,"exit",second,"exec_in");
        wire(&mut graph,second,"exit",set,"exec_in");
        wire(&mut graph,first,"counter",sum,"left");
        wire(&mut graph,sum,"value",second,"n");
        graph.enable_native_frame_preparation().unwrap();
        let before=graph.clone();let generated=crate::prepare_native_frame_graph(&graph).unwrap();
        let parsed=rvn_parser::parse_with_origins(&generated.source,NarrativeContext::Legacy).unwrap();
        let (body,span)=parsed.script.iter().zip(&parsed.spans).find_map(|(s,span)|match s{
            Statement::Handler{name,body,..}if name=="data_probe"=>Some((body,span)),_=>None,
        }).unwrap();
        validate_frozen_unit_macro_data(&parsed.script).unwrap(); // Actual Handler ownership is preserved.
        let dispatch=body.iter().find_map(|s|match s{
            Statement::StandardMacroDispatch{dispatch}=>dispatch.branches.iter().flat_map(|b|&b.body).find_map(|s|match s{
                Statement::StandardMacroDispatch{dispatch}if dispatch.site.node_source().starts_with(&format!("n{}:",second.0))=>Some(dispatch),_=>None,
            }),_=>None,
        }).expect("real second dispatch in the first DoN Exit branch");
        assert!(dispatch.deferred_operand_v1.is_some());
        // The source getter is nested in a real captured arithmetic expression.
        assert!(dispatch.operand_steps().iter().any(|step| {
            let value=match step{rvn_parser::ConsumerStep::Capture{value,..}|rvn_parser::ConsumerStep::InspectTextTable{value,..}=>value};
            let mut stack=vec![value];
            while let Some(e)=stack.pop(){match e{
                Expr::StandardMacroData{..}=>return true,
                Expr::BinOp{left,right,..}|Expr::Index{target:left,index:right}|Expr::And(left,right)|Expr::Or(left,right)=>{stack.push(left);stack.push(right);},
                Expr::LibraryCall{call}=>stack.extend(call.arguments().iter().map(|(_,value)|value)),
                Expr::Neg(v)|Expr::Not(v)=>stack.push(v),Expr::Call{args,..}|Expr::ListLit(args)=>stack.extend(args),
                Expr::Int(_)|Expr::Float(_)|Expr::Bool(_)|Expr::Str(_)|Expr::Var(_)=>{},
            }}false
        }),"actual deferred capture must carry MacroData, not an unrelated producer");
        let body_source=&generated.source[span.children.first().unwrap().range.start..span.children.last().unwrap().range.end];
        let unit=format!("function forbidden(alpha) -> () {{{body_source}}}\n");
        // The real Parser preserves its Handler-only dispatch boundary.
        let error=rvn_parser::parse_program(&unit).unwrap_err();
        assert!(error.contains("un véritable Handler Widget (première tranche)"),"{error}");
        // Deliberately unadmitted AST: clone the verified actual Handler body,
        // never forge a Site/Dispatch/Consumer or claim matching Unit source.
        let mut program=rvn_parser::parse_program("function forbidden(alpha) -> () {return}\n").unwrap();
        assert!(program.resolution.is_none());
        program.script=vec![Statement::UnitFunction {
            name:"forbidden".into(),parameters:vec!["alpha".into()],body:body.clone(),
        }];
        assert!(validate_frozen_unit_macro_data(&program.script).unwrap_err().contains("Native macro data in a schema14 Unit"));
        // This exact AST refusal must precede parsing the invalid text above,
        // source association, graph selection and any authoring allocation.
        assert!(import_frozen_in_context(&unit,&program).unwrap_err().contains("Native macro data in a schema14 Unit"));
        assert_eq!(graph,before,"validation and refusal never mutate the actual Graph");
    }

}
