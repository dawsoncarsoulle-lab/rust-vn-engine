//! Shared declarations are executable RVN initialization, not presentation-only
//! cache entries. Reconcile on a disposable graph set before source publication.
use crate::{GraphDocument, GraphKind, NodeKind, PropertyValue, VariableDefinition, VariableScope};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
enum BindingScope { Narrative, Calculation, Handler }

// Compare the already parsed original and probe ASTs in execution order. A
// local initializer reads the old environment before introducing its binding.
// Branches introduce a definite binding only when every branch does so.
fn uses_before_shadow(
    before: &[rvn_parser::Statement], after: &[rvn_parser::Statement], name: &str,
    shadowed: &mut bool, scope: BindingScope,
) -> bool {
    use rvn_parser::Statement;
    if before.len() != after.len() { return true; }
    for (old, new) in before.iter().zip(after) {
        match (old, new) {
            (Statement::FunctionLibrary{scope:old},Statement::FunctionLibrary{scope:new})=>{
                if old.library()!=new.library()||old.members().len()!=new.members().len(){return true;}
                for(before,after)in old.members().iter().zip(new.members()){
                    if before.graph_id()!=after.graph_id()||before.parameter_ids()!=after.parameter_ids(){return true;}
                    if uses_before_shadow(std::slice::from_ref(before.declaration()),std::slice::from_ref(after.declaration()),name,&mut false,BindingScope::Narrative){return true;}
                }
            },
            (Statement::LocalVar { name: binding, value }, Statement::LocalVar { value: probe, .. }) => {
                if !*shadowed && value != probe { return true; }
                if binding == name { *shadowed = true; }
            }
            (Statement::SetVar { name: binding, value }, Statement::SetVar { value: probe, .. })
                if matches!(scope, BindingScope::Calculation) => {
                if !*shadowed && value != probe { return true; }
                if binding == name { *shadowed = true; }
            }
            (Statement::Function { parameters, body, .. }, Statement::Function { body: probe, .. })
            | (Statement::Screen { parameters, body, .. }, Statement::Screen { body: probe, .. }) => {
                let mut bound = parameters.iter().any(|parameter| parameter == name);
                if uses_before_shadow(body, probe, name, &mut bound, BindingScope::Calculation) { return true; }
            }
            (Statement::Handler { parameters, body, .. }, Statement::Handler { body: probe, .. })
            | (Statement::UnitFunction { parameters, body, .. }, Statement::UnitFunction { body: probe, .. }) => {
                let mut bound = parameters.iter().any(|parameter| parameter == name);
                if uses_before_shadow(body, probe, name, &mut bound, BindingScope::Handler) { return true; }
            }
            (Statement::If { condition, then_branch, else_branch },
                Statement::If { condition: probe_condition, then_branch: probe_then, else_branch: probe_else }) => {
                if !*shadowed && condition != probe_condition { return true; }
                let mut first = *shadowed;
                let mut second = *shadowed;
                if uses_before_shadow(then_branch, probe_then, name, &mut first, scope)
                    || uses_before_shadow(else_branch, probe_else, name, &mut second, scope) { return true; }
                *shadowed = first && second;
            }
            (Statement::Sequence { branches }, Statement::Sequence { branches: probes }) => {
                if branches.len() != probes.len() { return true; }
                for (body, probe) in branches.iter().zip(probes) {
                    if uses_before_shadow(body, probe, name, shadowed, scope) { return true; }
                }
            }
            (Statement::While { condition, body }, Statement::While { condition: probe_condition, body: probe }) => {
                if !*shadowed && condition != probe_condition { return true; }
                let mut bound = *shadowed;
                if uses_before_shadow(body, probe, name, &mut bound, scope) { return true; }
                // A loop may execute zero times; retain its entry environment.
            }
            (Statement::ForEach { name: iterator, collection, body },
                Statement::ForEach { name: probe_iterator, collection: probe_collection, body: probe }) => {
                let local_iterator = !matches!(scope, BindingScope::Narrative);
                if !*shadowed && (collection != probe_collection || (!local_iterator && iterator != probe_iterator)) { return true; }
                let mut bound = *shadowed || (local_iterator && iterator == name);
                if uses_before_shadow(body, probe, name, &mut bound, scope) { return true; }
            }
            (Statement::ForLoopWithBreak { name: iterator, first, last, body },
                Statement::ForLoopWithBreak { name: probe_iterator, first: probe_first, last: probe_last, body: probe }) => {
                let local_iterator = !matches!(scope, BindingScope::Narrative);
                if !*shadowed && (first != probe_first || last != probe_last || (!local_iterator && iterator != probe_iterator)) { return true; }
                let mut bound = *shadowed || (local_iterator && iterator == name);
                if uses_before_shadow(body, probe, name, &mut bound, scope) { return true; }
            }
            (Statement::Choice { options }, Statement::Choice { options: probes }) => {
                if options.len() != probes.len() { return true; }
                let entry = *shadowed;
                let mut all_bound = !options.is_empty();
                for (option, probe) in options.iter().zip(probes) {
                    if !entry && (option.label != probe.label || option.condition != probe.condition) { return true; }
                    let mut bound = entry;
                    if uses_before_shadow(&option.body, &probe.body, name, &mut bound, scope) { return true; }
                    all_bound &= bound;
                }
                // Conditional choices may have no eligible option.
                *shadowed = entry || (all_bound && options.iter().all(|option| option.condition.is_none()));
            }
            (Statement::Imagemap { hotspots, .. }, Statement::Imagemap { hotspots: probes, .. }) => {
                if hotspots.len() != probes.len() { return true; }
                for (hotspot, probe) in hotspots.iter().zip(probes) {
                    let mut bound = *shadowed;
                    if uses_before_shadow(&hotspot.body, &probe.body, name, &mut bound, scope) { return true; }
                }
            }
            (Statement::Init { body }, Statement::Init { body: probe }) => {
                if uses_before_shadow(body, probe, name, shadowed, BindingScope::Narrative) { return true; }
            }
            (Statement::ScheduledConsumer { steps, body, .. },
                Statement::ScheduledConsumer { steps: probe_steps, body: probe, .. }) => {
                // Producers run before their consumer body. A Local in that
                // body cannot shadow a preceding capture's global read.
                if !*shadowed && steps != probe_steps { return true; }
                if uses_before_shadow(body, probe, name, shadowed, scope) { return true; }
            }
            _ if !*shadowed && old != new => return true,
            _ => {}
        }
    }
    false
}

fn uses(graph: &GraphDocument, definition: &VariableDefinition, project: &[GraphDocument]) -> Result<bool, String> {
    if matches!(
        graph.variable_scope(&definition.name),
        Some(VariableScope::Parameter)
    ) {
        return Ok(false);
    }
    let mut before = graph.clone();
    before
        .variables
        .entry(definition.name.clone())
        .or_insert_with(|| definition.clone());
    let mut after = before.clone();
    let mut probe = "RvNDeclarationReferenceProbe".to_owned();
    while after.variables.contains_key(&probe) {
        probe.push('_');
    }
    after
        .rename_variable_references(&definition.name, probe)
        .map_err(|error| error.to_string())?;
    let original = crate::unit_graph_source1003::compile_scope(&before,project).map_err(|error| error.to_string())?.ast;
    let probe = crate::unit_graph_source1003::compile_scope(&after,project).map_err(|error| error.to_string())?.ast;
    if graph.variable_scope(&definition.name) == Some(VariableScope::Local) {
        Ok(uses_before_shadow(&original, &probe, &definition.name, &mut false, BindingScope::Narrative))
    } else {
        Ok(original != probe)
    }
}

// Immutable analysis views carry only actual parsed source, never GraphIds or
// parameter/port owners. Publication continues to use the original source tree.
pub(super) struct ReadonlyScope {
    pub(super) ast: rvn_parser::Script,
}
fn unit_targets(body: &[rvn_parser::Statement]) -> BTreeSet<String> {
    use rvn_parser::Statement as S;
    let mut targets=BTreeSet::new();let mut pending=vec![body];
    while let Some(body)=pending.pop() { for s in body { match s {
        S::UnitCall {name,..}=>{targets.insert(name.clone());},
        S::Init {body}|S::Function {body,..}|S::UnitFunction {body,..}|S::Screen {body,..}|S::Handler {body,..}
        |S::ScheduledConsumer {body,..}|S::While {body,..}|S::ForEach {body,..}|S::ForLoopWithBreak {body,..}=>pending.push(body),
        S::If {then_branch,else_branch,..}=>{pending.push(then_branch);pending.push(else_branch);},
        S::Sequence {branches}=>pending.extend(branches.iter().map(Vec::as_slice)),
        S::StandardMacroDispatch {dispatch}=>pending.extend(dispatch.branches.iter().map(|b|b.body.as_slice())),
        S::Choice {options}=>pending.extend(options.iter().map(|o|o.body.as_slice())),
        S::Imagemap {hotspots,..}=>pending.extend(hotspots.iter().map(|h|h.body.as_slice())),
        _=>{},
    }}}
    targets
}
fn source_scope_groups(script:&rvn_parser::Script)->Vec<Vec<usize>> {
    use rvn_parser::Statement as S;
    let mut groups: Vec<Vec<usize>>=Vec::new();let mut label=None;
    for (i,s) in script.iter().enumerate() {match s {
        S::Use {..}=>{}, // Already resolved and validated through the complete Context.
        S::Label {..}|S::SceneLabel {..}=>{label=Some(groups.len());groups.push(vec![i]);},
        S::Init {..}|S::UnitFunction {..}|S::Function {..}|S::Screen {..}|S::Handler {..}=>groups.push(vec![i]),
        _=>if let Some(label)=label {groups[label].push(i);} else {groups.push(vec![i]);},
    }}
    groups
}
pub(super) fn readonly_dependencies(script: &rvn_parser::Script, complete:&rvn_parser::Script) -> Result<(rvn_parser::Script, Vec<ReadonlyScope>),String> {
    use rvn_parser::Statement as S;
    let mut units:BTreeSet<String>=script.iter().filter_map(|s|match s {S::UnitFunction {name,parameters,..}if !parameters.is_empty()=>Some(name.clone()),_=>None}).collect();
    // Extend only the affected callee component. Unrelated arity-zero Units
    // and callers retain their historical graph validation/dependency policy.
    loop {
        let before=units.len();
        for s in script {if let S::UnitFunction {name,body,..}=s {
            if !unit_targets(body).is_disjoint(&units) {units.insert(name.clone());}
        }}
        if units.len()==before {break;}
    }
    let groups=source_scope_groups(script);
    let actual_scopes:Vec<rvn_parser::Script>=source_scope_groups(complete).iter().map(|indices|indices.iter().map(|i|complete[*i].clone()).collect()).collect();
    let mut taken=BTreeSet::new();let mut readonly=Vec::new();
    for indices in groups {
        let ast:rvn_parser::Script=indices.iter().map(|i|script[*i].clone()).collect();
        // Keep the existing graph import for Native and Init/host containers.
        // Only true Legacy narrative callers gain this new read-only port.
        if matches!(ast.first(),Some(S::UnitFunction {name,..})if units.contains(name))
            || matches!(ast.first(),Some(S::Label {..}|S::SceneLabel {..}))
                && !unit_targets(&ast).is_disjoint(&units)&&!crate::native_source_binding::contains_native(&ast) {
            let actual=actual_scopes.iter().find(|scope|match (ast.first(),scope.first()) {
                (Some(S::UnitFunction {name,..}),Some(S::UnitFunction {name:other,..}))=>name==other,
                (Some(S::Label {name}|S::SceneLabel {name}),Some(S::Label {name:other}|S::SceneLabel {name:other}))=>name==other,
                _=>false,
            }).ok_or("Read-only caller has no actual resolved source scope")?;
            if actual!=&ast {return Err("Read-only Unit caller crosses resolved scope boundaries/order; an exact dependency view is required before saving".into());}
            taken.extend(indices);readonly.push(ReadonlyScope {ast});
        }
    }
    let retained=script.iter().enumerate().filter(|(i,s)|!taken.contains(i)&&!matches!(s,S::Use {..})).map(|(_,s)|s.clone()).collect();
    Ok((retained,readonly))
}

// A disposable reference probe, not a compiler/interpreter or source rewrite.
// The exhaustive statement match forces new expression fields to be reviewed.
fn probe_source(body: &mut [rvn_parser::Statement], name: &str, probe: &str, declarations: bool) {
    use rvn_parser::{Statement as S, Expr, TextSegment, ConsumerStep};
    fn expr(value:&mut Expr,name:&str,probe:&str) {
        if crate::text_table_nodes::source_binding_names(value).contains(name) {*value=Expr::Var(probe.into());}
    }
    fn text(value:&mut rvn_parser::InterpolatedText,name:&str,probe:&str) {
        for segment in &mut value.0 {if let TextSegment::Interp(value)=segment {expr(value,name,probe);}}
    }
    fn steps(values:&mut [ConsumerStep],name:&str,probe:&str) {
        for step in values {match step {ConsumerStep::Capture {value,..}|ConsumerStep::InspectTextTable {value,..}=>expr(value,name,probe)}}
    }
    for statement in body {match statement {
        S::FunctionLibrary{scope}=>{
            let members=scope.members().iter().map(|member|{
                let mut declaration=member.declaration().clone();
                probe_source(std::slice::from_mut(&mut declaration),name,probe,declarations);
                // This is an inert clone for the existing read-only difference probe.
                // Nominal IDs/signature are unchanged and remain validated.
                rvn_parser::function_library_program1023::LibraryMemberSource::new(member.graph_id(),member.parameter_ids().to_vec(),declaration)
                    .expect("readonly variable probe preserves validated member identity/signature")
            }).collect();
            *scope=rvn_parser::function_library_program1023::FunctionLibraryScope::new(scope.library().clone(),members)
                .expect("readonly variable probe preserves validated Library membership");
        },
        S::LibraryUnitCall{call}=>{
            let arguments=call.arguments().iter().map(|(id,value)|{let mut value=value.clone();expr(&mut value,name,probe);(id.clone(),value)}).collect();
            *call=rvn_parser::function_library_program1023::QualifiedLibraryCall::new(call.reference().clone(),arguments)
                .expect("readonly variable probe preserves validated argument identities");
        },
        S::SetVar {name:binding,value}=>{expr(value,name,probe);if binding==name {*binding=probe.into();}},
        S::LocalVar {name:binding,value}=>{expr(value,name,probe);if declarations&&binding==name {*binding=probe.into();}},
        S::UnitCall {arguments,..}=>for value in arguments {expr(value,name,probe);},
        S::Init {body}|S::Function {body,..}|S::UnitFunction {body,..}|S::Screen {body,..}|S::Handler {body,..}=>probe_source(body,name,probe,declarations),
        S::If {condition,then_branch,else_branch}=>{expr(condition,name,probe);probe_source(then_branch,name,probe,declarations);probe_source(else_branch,name,probe,declarations);},
        S::While {condition,body}=>{expr(condition,name,probe);probe_source(body,name,probe,declarations);},
        S::ForEach {name:binding,collection,body}=>{expr(collection,name,probe);if binding==name {*binding=probe.into();}probe_source(body,name,probe,declarations);},
        S::ForLoopWithBreak {name:binding,first,last,body}=>{expr(first,name,probe);expr(last,name,probe);if binding==name {*binding=probe.into();}probe_source(body,name,probe,declarations);},
        S::Sequence {branches}=>for branch in branches {probe_source(branch,name,probe,declarations);},
        S::ScheduledConsumer {steps:values,body,..}=>{steps(values,name,probe);probe_source(body,name,probe,declarations);},
        S::StandardMacroDispatch {dispatch}=>{
            if let Some(value)=&mut dispatch.operand {expr(value,name,probe);}
            if let Some(plan)=&mut dispatch.deferred_operand_v1 {steps(&mut plan.steps,name,probe);}
            for branch in &mut dispatch.branches {probe_source(&mut branch.body,name,probe,declarations);}
        },
        S::Dialogue {text:value,..}=>text(value,name,probe),
        S::Choice {options}=>for option in options {text(&mut option.label,name,probe);if let Some(value)=&mut option.condition {expr(value,name,probe);}probe_source(&mut option.body,name,probe,declarations);},
        S::Imagemap {hotspots,..}=>for hotspot in hotspots {probe_source(&mut hotspot.body,name,probe,declarations);},
        S::WidgetAddToViewport {object,z_order}=>{expr(object,name,probe);expr(z_order,name,probe);},
        S::WidgetRemoveFromParent {object}=>expr(object,name,probe),
        S::UiOpen {name:value,arguments,modal,layer,..}=>{expr(value,name,probe);expr(arguments,name,probe);expr(modal,name,probe);expr(layer,name,probe);},
        S::UiFocus {name:value,element}=>{expr(value,name,probe);expr(element,name,probe);},
        S::UiSetState {name:value,element,state}=>{expr(value,name,probe);expr(element,name,probe);expr(state,name,probe);},
        S::UiClose {name:value}|S::VideoPause {name:value}|S::VideoResume {name:value}|S::VideoStop {name:value}|S::VideoSkip {name:value}|S::VideoWait {name:value}=>expr(value,name,probe),
        S::MenuExecute {request:value}|S::AccessibilityConfigure {settings:value}|S::AccessibilitySpeak {text:value}
        |S::FunctionReturn {value}|S::MotionStop {target:value}|S::MotionWait {target:value}
        |S::MusicVolumeValue {level:value}|S::TypewriterSetValue {enabled:value}|S::TypewriterSpeedValue {chars_per_sec:value}=>expr(value,name,probe),
        S::MotionPlay {target,definition}=>{expr(target,name,probe);expr(definition,name,probe);},
        S::CharacterCompose {character,definition}=>{expr(character,name,probe);expr(definition,name,probe);},
        S::CharacterAttributes {character,attributes}=>{expr(character,name,probe);expr(attributes,name,probe);},
        S::VideoPlay {name:value,definition}=>{expr(value,name,probe);expr(definition,name,probe);},
        S::VideoSeek {name:value,seconds}=>{expr(value,name,probe);expr(seconds,name,probe);},
        S::VideoVolume {name:value,volume}=>{expr(value,name,probe);expr(volume,name,probe);},
        S::Use {..}|S::Label {..}|S::SceneLabel {..}|S::Config {..}|S::CharacterCreate {..}
        |S::Jump {..}|S::Call {..}|S::Return|S::UnitReturn|S::LoopBreak|S::AccessibilityStop
        |S::Scene {..}|S::CinematicShow {..}|S::CinematicHide {..}|S::UnlockEnding {..}
        |S::ShowSprite {..}|S::HideSprite {..}|S::MoveSprite {..}|S::SpriteAnimate {..}|S::SpriteStopAnimation {..}|S::SpriteEffect {..}
        |S::Timer {..}|S::TimerCancel|S::MethodCall {..}|S::MusicPlay {..}|S::MusicStop {..}|S::MusicVolume {..}
        |S::SfxPlay {..}|S::SfxStop {..}|S::VoicePlay {..}|S::VoiceStop|S::TypewriterSet {..}|S::TypewriterSpeed {..}=>{},
    }}
}
pub(super) fn readonly_scope_mentions(scope: &ReadonlyScope, name:&str) -> bool {
    let mut after=scope.ast.clone();let mut probe="RvNReadOnlyBindingProbe".to_owned();if probe==name {probe.push('_');}
    probe_source(&mut after,name,&probe,true);scope.ast!=after
}
fn readonly_scope_uses(scope:&ReadonlyScope,name:&str)->bool {
    let mut after=scope.ast.clone();let mut probe="RvNReadOnlyGlobalReferenceProbe".to_owned();if probe==name {probe.push('_');}
    probe_source(&mut after,name,&probe,false);
    uses_before_shadow(&scope.ast,&after,name,&mut false,BindingScope::Narrative)
}

pub(super) fn reconcile(
    baseline: &[GraphDocument],
    edited: &mut [GraphDocument],
    imported: &[GraphDocument],
    readonly_scopes: &[ReadonlyScope],
) -> Result<(), String> {
    let mut globals = BTreeMap::<String, VariableDefinition>::new();
    for graph in baseline {
        for (name, definition) in &graph.variables {
            if graph.variable_scope(name) == Some(VariableScope::Global) {
                globals
                    .entry(name.clone())
                    .or_insert_with(|| definition.clone());
            }
        }
    }
    let mut added = BTreeMap::<String, VariableDefinition>::new();
    let mut removed = BTreeSet::new();
    for graph in edited.iter() {
        let old = baseline.iter().find(|old| old.graph_id == graph.graph_id);
        for (name, definition) in &graph.variables {
            if graph.variable_scope(name) != Some(VariableScope::Global)
                || globals.contains_key(name)
            {
                continue;
            }
            if added
                .get(name)
                .is_some_and(|previous| previous != definition)
            {
                return Err(format!(
                    "Conflicting variable declaration: {name}. No source was changed."
                ));
            }
            added.insert(name.clone(), definition.clone());
        }
        if let Some(old) = old {
            for name in old.variables.keys() {
                if old.variable_scope(name) == Some(VariableScope::Global)
                    && !graph.variables.contains_key(name)
                {
                    removed.insert(name.clone());
                }
            }
        }
    }
    if added.is_empty() && removed.is_empty() {
        return Ok(());
    }
    let init_index = edited
        .iter()
        .position(|graph| graph.kind == GraphKind::Init)
        .ok_or(
            "Variable declarations require the shared initialization graph; no source was changed.",
        )?;

    for name in &removed {
        let definition = &globals[name];
        let init = &mut edited[init_index];
        let setters: Vec<_> = init
            .nodes
            .values()
            .filter(|node| {
                node.kind == NodeKind::SetVariable
                    && init.variable_node_name(node.id).as_deref() == Some(name)
            })
            .map(|node| node.id)
            .collect();
        if setters.len() > 1
            || if let Some(node) = setters.first() {
                init.literal_input_value(*node, "value")?.is_none()
            } else {
                false
            }
        {
            return Err(format!("Variable '{name}' has executable initialization. Edit its bindings explicitly; no source was changed."));
        }
        // An explicit project refactor may already have removed or renamed
        // this binding. Only generate an implicit deletion if it still exists.
        if let Some(node) = setters.first().copied() {
            let input = init
                .pin_by_key(node, "exec_in")
                .ok_or("Missing declaration execution input")?
                .id;
            let output = init
                .pin_by_key(node, "exec_out")
                .ok_or("Missing declaration execution output")?
                .id;
            let predecessor: Vec<_> = init
                .edges
                .values()
                .filter(|edge| edge.input == input)
                .map(|edge| edge.output)
                .collect();
            if predecessor.len() != 1
                || init
                    .edges
                    .values()
                    .any(|edge| init.pins[&edge.output].node == node && edge.output != output)
            {
                return Err(format!(
                    "Variable '{name}' has connected uses; no source was changed."
                ));
            }
            for edge in init.edges.values_mut().filter(|edge| edge.output == output) {
                edge.output = predecessor[0];
            }
            init.remove_node(node);
        }
        // Real UnitCall ports require their actual callee owners. Imported
        // graph identities are a separate namespace, never merged with ours.
        for project in [&*edited, imported] {
            for graph in project {
                if uses(graph, definition, project)? {
                    return Err(format!("Variable '{name}' is still referenced. Remove its uses before deleting the declaration; no source was changed."));
                }
            }
        }
        for scope in readonly_scopes {
            if readonly_scope_uses(scope, name) {
                let kind=if matches!(scope.ast.first(),Some(rvn_parser::Statement::UnitFunction {..})) {"Unit"} else {"scope"};
                return Err(format!("Variable '{name}' is still referenced by a read-only imported {kind}. Remove its uses in its owning source before deleting the declaration; no source was changed."));
            }
        }
        for graph in edited.iter_mut() {
            if !matches!(
                graph.variable_scope(name),
                Some(VariableScope::Local | VariableScope::Parameter)
            ) {
                graph.variables.remove(name);
            }
        }
    }
    for (name, definition) in &added {
        if rvn_parser::variable_source_name(name).is_none() {
            return Err(format!(
                "Invalid RVN variable declaration: {name}. No source was changed."
            ));
        }
        for graph in edited.iter_mut() {
            if !matches!(
                graph.variable_scope(name),
                Some(VariableScope::Local | VariableScope::Parameter)
            ) {
                graph.variables.insert(name.clone(), definition.clone());
            }
        }
        // Existing authored bindings own execution and its ordering. An
        // explicit rename is not a request to add another initialization Set.
        fn writes_name(body:&[rvn_parser::Statement],name:&str)->bool {
            use rvn_parser::Statement as S;
            body.iter().any(|s|match s {
                S::SetVar{name:target,..}=>target==name,
                S::Init{body}|S::ScheduledConsumer{body,..}|S::While{body,..}
                    |S::ForEach{body,..}|S::ForLoopWithBreak{body,..}=>writes_name(body,name),
                S::If{then_branch,else_branch,..}=>writes_name(then_branch,name)||writes_name(else_branch,name),
                S::Sequence{branches}=>branches.iter().any(|body|writes_name(body,name)),
                S::Choice{options}=>options.iter().any(|option|writes_name(&option.body,name)),
                S::Imagemap{hotspots,..}=>hotspots.iter().any(|hotspot|writes_name(&hotspot.body,name)),
                S::Handler{body,..}=>writes_name(body,name),
                _=>false,
            })
        }
        let mut authored=false;
        for graph in edited.iter().filter(|graph|graph.variable_scope(name)==Some(VariableScope::Global)) {
            let compiled=crate::native_source_binding::compile_scope(graph,true).map_err(|e|e.to_string())?;
            if writes_name(&compiled.ast,name){authored=true;break;}
        }
        // A disconnected Legacy setter is presentation, not an executable
        // declaration. Only its genuinely emitted binding can own this write.
        if authored {
            continue;
        }
        let init = &mut edited[init_index];
        let root = init
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::Init)
            .ok_or("Missing shared initialization entry")?
            .id;
        let root_output = init
            .pin_by_key(root, "exec_out")
            .ok_or("Missing initialization execution output")?
            .id;
        let next: Vec<_> = init
            .edges
            .values()
            .filter(|edge| edge.output == root_output)
            .map(|edge| edge.id)
            .collect();
        let node = init
            .add_catalog_node(
                NodeKind::SetVariable,
                [
                    320.0,
                    -96.0 * (added.keys().position(|key| key == name).unwrap() + 1) as f64,
                ],
            )
            .map_err(|error| error.to_string())?;
        init.set_property(node, "name", PropertyValue::String(name.clone()))
            .map_err(|error| error.to_string())?;
        let pins = init.nodes[&node].pins.clone();
        for pin in pins {
            if matches!(init.pins[&pin].key.as_str(), "value" | "value_out") {
                init.pins.get_mut(&pin).unwrap().value_type = definition.value_type.clone();
            }
        }
        init.set_pin_default(node, "value", definition.default_value.clone())
            .map_err(|error| error.to_string())?;
        let input = init
            .pin_by_key(node, "exec_in")
            .ok_or("Missing declaration execution input")?
            .id;
        let output = init
            .pin_by_key(node, "exec_out")
            .ok_or("Missing declaration execution output")?
            .id;
        for edge in next {
            init.edges.get_mut(&edge).unwrap().output = output;
        }
        init.connect(root_output, input)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
