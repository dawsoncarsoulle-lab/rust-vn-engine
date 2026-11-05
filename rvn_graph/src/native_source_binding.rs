//! PRIVATE writer/source boundary. All executable identities and plans are
//! reconstructed from actual .rvn; cached graph payloads are presentation only.
use crate::*;
use rvn_parser::{ParsedProgram,Statement,StatementSpan};
use std::{collections::{BTreeSet,BTreeMap},path::PathBuf};

pub(crate) fn contains_native(block:&[Statement])->bool {
    let mut pending=vec![block];
    while let Some(block)=pending.pop() {
        for statement in block {
            match statement {
                Statement::ScheduledConsumer{..}|Statement::StandardMacroDispatch{..}=>return true,
                Statement::Function{body,..}|Statement::UnitFunction{body,..}|Statement::Screen{body,..}|Statement::Handler{body,..}|Statement::Init{body}|Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>pending.push(body),
                Statement::If{then_branch,else_branch,..}=>{pending.push(then_branch);pending.push(else_branch);},
                Statement::Sequence{branches}=>pending.extend(branches.iter().map(Vec::as_slice)),
                Statement::Choice{options}=>pending.extend(options.iter().map(|option|option.body.as_slice())),
                Statement::Imagemap{hotspots,..}=>pending.extend(hotspots.iter().map(|hotspot|hotspot.body.as_slice())),
                _=>{},
            }
        }
    }
    false
}
pub(crate) fn compile_scope(graph:&GraphDocument,native:bool)->Result<TranspiledScript,TranspileError> {
    if native&&!graph.inspector_evaluation.is_legacy() {
        // Syntactic/structural preparation only. SourceProject and project
        // export reconstruct identities against the complete parsed candidate
        // before publication. No registry is expanded into an Init statement.
        let result=crate::native_frame_preparation::prepare_native_unchecked(graph)?;
        Ok(TranspiledScript{source:result.source,ast:result.ast})
    } else {transpile(graph)}
}

/// Exact source site of a real consumer, not an invented per-expression span.
/// `route` is [actual resolved root index, actual function-body index].
#[derive(Debug,Clone,PartialEq)]
pub struct NativeSourceSite {
    pub source:PathBuf,
    pub graph:GraphId,
    pub function:String,
    pub root:NodeId,
    pub consumer:NodeId,
    pub route:Vec<usize>,
    pub definition_route:Vec<usize>,
    pub span:StatementSpan,
    pub scope:String,
    pub plan:Statement,
    pub producer_nodes:Vec<NodeId>,
}

pub fn native_source_sites(program:&ParsedProgram)->Result<Vec<NativeSourceSite>,String> {
    validate_frozen_source_graphs(program)?;
    if program.script.len()!=program.origins.len() {return Err("Native program origin count mismatch".into());}
    fn walk(statement:&Statement,span:&StatementSpan,source:&PathBuf,name:&str,definition:usize,route:Vec<usize>,sites:&mut Vec<NativeSourceSite>)->Result<(),String> {
        if route.len()>128 || sites.len()>=65_536 {return Err("Native source site budget exceeded".into());}
        if let Statement::ScheduledConsumer{scope,consumer,steps,..}=statement {
            let (graph,root)=crate::native_frame_preparation::decode_scope(scope)?;
            let consumer=crate::native_frame_preparation::decode_node(consumer)?;
            let producer_nodes=steps.iter().map(|step|{
                let id=match step {rvn_parser::ConsumerStep::Capture{producer,..}|rvn_parser::ConsumerStep::InspectTextTable{producer,..}=>producer};
                crate::native_frame_preparation::decode_producer(id).map(|(node,_)|node.node)
            }).collect::<Result<Vec<_>,_>>()?;
            sites.push(NativeSourceSite{source:source.clone(),graph,function:name.into(),root:root.node,consumer:consumer.node,
                route:route.clone(),definition_route:vec![definition],span:span.clone(),scope:scope.clone(),plan:statement.clone(),producer_nodes});
        }
        let children:Vec<&Statement>=match statement {
            Statement::Function{body,..}|Statement::UnitFunction{body,..}|Statement::Screen{body,..}|Statement::Handler{body,..}|Statement::Init{body}|Statement::ScheduledConsumer{body,..}|Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>body.iter().collect(),
            Statement::If{then_branch,else_branch,..}=>then_branch.iter().chain(else_branch).collect(),
            Statement::StandardMacroDispatch{dispatch}=>dispatch.branches.iter().flat_map(|branch|branch.body.iter()).collect(),
            Statement::Sequence{branches}=>branches.iter().flatten().collect(),
            Statement::Choice{options}=>options.iter().flat_map(|option|&option.body).collect(),
            Statement::Imagemap{hotspots,..}=>hotspots.iter().flat_map(|hotspot|&hotspot.body).collect(),
            _=>Vec::new(),
        };
        if children.len()!=span.children.len() {return Err("Native nested statement/span count mismatch".into());}
        for (index,(child,span)) in children.into_iter().zip(&span.children).enumerate() {
            let mut route=route.clone();route.push(index);walk(child,span,source,name,definition,route,sites)?;
        }
        Ok(())
    }
    let mut sites=Vec::new();let mut label=String::new();let mut label_index=0;let mut owners=BTreeMap::new();
    for (root_index,(statement,origin)) in program.script.iter().zip(&program.origins).enumerate() {
        let source=origin.source.clone().ok_or("Native programme root lacks actual ProgramSource origin")?;
        if let Statement::Label{name}|Statement::SceneLabel{name}=statement {label=name.clone();label_index=root_index;}
        let name=match statement {Statement::Function{name,..}|Statement::UnitFunction{name,..}|Statement::Screen{name,..}|Statement::Handler{name,..}=>name.as_str(),Statement::Init{..}=>"<init>",_=>label.as_str()};
        let definition=if matches!(statement,Statement::Function{..}|Statement::UnitFunction{..}|Statement::Screen{..}|Statement::Handler{..}|Statement::Init{..}) {root_index}else{label_index};
        let begin=sites.len();walk(statement,&origin.span,&source,name,definition,vec![root_index],&mut sites)?;
        for site in &sites[begin..] {
            let key=(source.clone(),site.graph);let value=(name.to_owned(),site.root);
            if owners.get(&key).is_some_and(|old|old!=&value) {return Err("Native source/GraphId aliases a different actual definition".into());}
            owners.insert(key,value);
        }
    }
    Ok(sites)
}

// Prefer the opaque live-loader capture. The None fallback proves only r3's
// explicit, lexically unambiguous routes from complete frozen bytes; it never
// infers wildcard membership/aliases from a known-source subset or reads FS.
pub(crate) fn prove_explicit_frozen_use_order(program: &ParsedProgram) -> Result<(), String> {
    use std::path::{Component, Path};
    if let Some(proof) = &program.resolution {
        // Loader-captured edges are replayed against the complete frozen input.
        // A client sidecar or a known-source subset cannot construct this proof.
        return proof.verify_for(program);
    }
    fn target(base: &Path, raw: &str) -> Result<PathBuf, String> {
        if raw.contains('*') || raw.contains('?') {
            return Err("Mixed Unit frozen Use requires a captured wildcard/alias resolution certificate".into());
        }
        let mut result = base.to_path_buf();
        let mut normal = false;
        for component in Path::new(raw).components() {
            match component {
                Component::CurDir => {},
                Component::ParentDir if !normal && result.file_name().is_some() => { result.pop(); },
                Component::Normal(name) => { normal = true; result.push(name); },
                _ => return Err("Mixed Unit frozen Use requires a captured wildcard/alias resolution certificate".into()),
            }
        }
        Ok(result)
    }
    if program.context != rvn_parser::NarrativeContext::from_script(&program.script)
        || program.script.len() != program.origins.len()
    { return Err("Mixed Unit frozen Use context/origin count differs".into()); }
    // A genuine in-memory monophysical Program has no Use resolution facts
    // to certify. Preserve its existing optional source identity, while proving
    // its entire parsed AST and every nested span against its sole byte buffer.
    if let [physical] = program.sources.as_slice() {
        let actual = rvn_parser::parse_with_origins(&physical.source,program.context).map_err(|e|e.to_string())?;
        if !actual.script.iter().any(|s|matches!(s,Statement::Use{..})) {
            if actual.script != program.script || actual.spans.len() != program.origins.len()
                || actual.spans.iter().zip(&program.origins).any(|(span,origin)|
                    span != &origin.span || origin.source != physical.path)
            { return Err("Mixed Unit frozen single-source AST/origin association differs".into()); }
            return Ok(());
        }
    }
    let mut parsed = BTreeMap::new();
    let mut source_order = Vec::new();
    let mut bytes = 0usize;
    for source in &program.sources {
        let path = source.path.as_ref().ok_or("Mixed Unit frozen Use lacks a real source path")?;
        if !path.is_absolute() || path.file_name().is_none()
            || path.components().any(|c|matches!(c,Component::ParentDir|Component::CurDir))
        { return Err("Mixed Unit frozen Use requires canonical absolute source identities".into()); }
        bytes = bytes.checked_add(source.source.len()).filter(|n|*n<=16*1024*1024)
            .ok_or("Mixed Unit frozen Use source budget exceeded")?;
        let value = rvn_parser::parse_with_origins(&source.source, program.context).map_err(|e|e.to_string())?;
        if parsed.insert(path.clone(), value).is_some() { return Err("Duplicate mixed Unit frozen source".into()); }
        source_order.push(path.clone());
    }
    enum Step { Enter(PathBuf), Root(PathBuf, usize), Leave(PathBuf) }
    let first = source_order.first().ok_or("Mixed Unit frozen Use has no entry source")?;
    let mut pending = vec![Step::Enter(first.clone())];
    let mut active = BTreeSet::new();
    let mut loaded = BTreeSet::new();
    let mut visited = Vec::new();
    let mut emitted = 0usize;
    while let Some(step) = pending.pop() {
        match step {
            Step::Leave(path) => { active.remove(&path); },
            Step::Root(path, index) => {
                let source = &parsed[&path];
                let origin = program.origins.get(emitted).ok_or("Mixed Unit frozen Use ordered roots differ")?;
                if program.script.get(emitted) != Some(&source.script[index])
                    || origin.source.as_ref() != Some(&path) || origin.span != source.spans[index]
                { return Err("Mixed Unit frozen Use ordered AST/origin stream differs".into()); }
                emitted += 1;
            },
            Step::Enter(path) => {
                // Like the real loader, a live stack hit is a cycle before dedup.
                if active.contains(&path) { return Err("Mixed Unit frozen Use cycle".into()); }
                if !loaded.insert(path.clone()) { continue; }
                let source = parsed.get(&path).ok_or("Mixed Unit frozen Use target lacks exact frozen canonical source; resolution certificate required")?;
                active.insert(path.clone()); visited.push(path.clone());
                pending.push(Step::Leave(path.clone()));
                let base = path.parent().ok_or("Mixed Unit frozen Use source parent absent")?;
                for (index, statement) in source.script.iter().enumerate().rev() {
                    if let Statement::Use { paths } = statement {
                        for raw in paths.iter().rev() { pending.push(Step::Enter(target(base, raw)?)); }
                    } else { pending.push(Step::Root(path.clone(), index)); }
                }
            },
        }
    }
    if emitted != program.script.len() || visited != source_order {
        return Err("Mixed Unit frozen Use source first-inclusion/root coverage differs".into());
    }
    Ok(())
}

pub(crate) fn validate_frozen_source_graphs(program:&ParsedProgram)->Result<(),String> {
    if program.script.len()!=program.origins.len() {return Err("Native program origin count mismatch".into());}
    crate::import::validate_frozen_program_operations(&program.script, false)?;
    let mut paths=BTreeSet::new();let mut parsed_sources=BTreeMap::new();let mut bytes=0usize;
    for source in &program.sources {
        bytes=bytes.checked_add(source.source.len()).filter(|count|*count<=16*1024*1024).ok_or("Native frozen source exceeds 16 MiB")?;
        let path=source.path.as_ref().ok_or("Native frozen ProgramSource requires real path")?;
        if !paths.insert(path.clone()) {return Err("Duplicate frozen ProgramSource path".into());}
        let parsed=rvn_parser::parse_with_origins(&source.source,program.context).map_err(|error|error.to_string())?;
        parsed_sources.insert(path.clone(),parsed);
    }
    let mut covered = BTreeSet::new();
    let mut lexical_ends = BTreeMap::new();
    for (statement,origin) in program.script.iter().zip(&program.origins) {
        let source=origin.source.as_ref().ok_or("Native origin has no source path")?;
        let parsed=parsed_sources.get(source).ok_or("Native origin has no actual frozen source")?;
        let exact=parsed.script.iter().zip(&parsed.spans).any(|(actual,span)|actual==statement&&*span==origin.span);
        if !exact {return Err("Native root AST/span differs from its frozen source bytes".into());}
        let key=(source.clone(),origin.span.range.start,origin.span.range.end);
        if !covered.insert(key) {return Err("Native frozen Program repeats a physical source root".into());}
        if lexical_ends.get(source).is_some_and(|end|*end>origin.span.range.start) {
            return Err("Native frozen Program changed the lexical root order within a source".into());
        }
        lexical_ends.insert(source.clone(),origin.span.range.end);
    }
    for (path,parsed) in &parsed_sources {
        for (statement,span) in parsed.script.iter().zip(&parsed.spans) {
            if !matches!(statement,Statement::Use{..})
                && !covered.contains(&(path.clone(),span.range.start,span.range.end))
            {
                return Err("Native frozen Program omits a physical source root".into());
            }
        }
    }
    // Every physical root is now bound exactly once to its actual AST/span.
    // Check Unit expression boundaries even when no source has a Native marker.
    crate::empty_native_init_0984::validate_frozen_unit_macro_data(&program.script)?;
    // Check the complete original Program before selecting any Native graph.
    // Legacy Unit calls are still checked for kind/arity and cannot disappear
    // from validation merely because their authoring pins are not reconstructed.
    crate::validate_project_script(&program.script, false)?;
    rvn_parser::scheduled_source::native_loop_contract::validate_authored_bindings(&program.script)?;
    for source in &program.sources {
        let parsed = &parsed_sources[source.path.as_ref().ok_or("Native frozen source path absent")?];
        let declared=crate::empty_native_init_0984::source_has_declarations(&source.source,program.context)?;
        if contains_native(&parsed.script)||declared {
            crate::empty_native_init_0984::import_frozen_in_context(&source.source,program)?;
        }
    }
    Ok(())
}
