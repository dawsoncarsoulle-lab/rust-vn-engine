//! Explicit source contract for demand loops. These names are private lexical
//! projections of real graph nodes, never runtime ownership capabilities.
use super::{id, node};
use crate::{Expr, Statement};
use std::collections::BTreeSet;
#[cfg(test)]
#[path="native_loop_contract_tests.rs"]
mod tests;
#[cfg(test)]
#[path="native_loop_legacy_alias_tests_0772.rs"]
mod legacy_alias0772;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Array, Range }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase { First, Test }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub graph: u64,
    pub node: u64,
    pub kind: Kind,
    pub index: String,
    pub cursor: String,
    pub phase: String,
}
/// A source spelling only, never an ownership capability. The existing RVN
/// grammar keeps __rvn_ reserved. Exact names are reserved structurally only
/// when binding()/validate_authored_bindings admit an actual source3 loop.
pub fn index_name(graph:u64,node:u64)->String {
    format!("rvn_native_index_g{graph}_n{node}")
}
pub fn binding_for_identity(scope: &str, consumer: &str) -> Result<Binding,String> {
    let (graph,root)=scope.strip_prefix('g').ok_or("Native loop requires its declared graph scope")?.split_once('/').ok_or("Native loop root missing")?;
    let graph=id(graph)?;
    let root=node(root,graph)?;
    if !matches!(root.kind.as_str(),"init"|"label"|"function_entry"|"handler_entry"|"screen_entry") {
        return Err("Native loop has no actual supported graph root role".into());
    }
    let owner=node(consumer,graph)?;
    if owner.exec_links_v1!=root.exec_links_v1{return Err("Native loop execution topology format differs from actual root".into());}
    let (kind,roles):(_, &[&str])=match owner.kind.as_str() {
        "for_each" => (Kind::Array,&["exec_in","collection","body","completed","array_index","array_element"]),
        "for_loop_with_break" => (Kind::Range,&["exec_in","first","last","break","body","index","completed"]),
        _=>return Err("Native loop consumer identity is not its actual loop kind".into()),
    };
    if owner.node==root.node || owner.ports.iter().map(|p|p.role.as_str()).collect::<BTreeSet<_>>() != roles.iter().copied().collect() {
        return Err("Native loop ports/root identity differ from its explicit contract".into());
    }
    if owner.ports.iter().any(|port|port.source_pin.is_some() || !matches!(port.role.as_str(),"exec_in"|"collection"|"first"|"last"|"break") && port.incoming.is_some()){
        return Err("Native loop output direction/source suffix differs".into());
    }
    let base=format!("g{graph}_n{}",owner.node);
    Ok(Binding {graph,node:owner.node,kind,
        index:index_name(graph,owner.node),cursor:format!("rvn_native_cursor_{base}"),phase:format!("rvn_native_phase_{base}")})
}
pub fn binding(statement: &Statement) -> Result<Option<Binding>,String> {
    let Statement::ScheduledConsumer{version:3,scope,consumer,body,..}=statement else{return Ok(None);};
    let value=binding_for_identity(scope,consumer)?;
    if !matches!((value.kind,body.as_slice()),(Kind::Array,[Statement::ForEach{..}])|(Kind::Range,[Statement::ForLoopWithBreak{..}])) {
        return Err("Authored consume_v3 must contain exactly its declared demand loop".into());
    }
    let name=match &body[0]{Statement::ForEach{name,..}|Statement::ForLoopWithBreak{name,..}=>name,_=>unreachable!()};
    if *name!=value.index{return Err("Native body index must project its actual loop identity".into());}
    Ok(Some(value))
}
pub fn phase_expression(statement: &Statement, phase: Phase) -> Result<&Expr,String> {
    binding(statement)?.ok_or("Native phase has no explicit loop statement")?;
    let Statement::ScheduledConsumer{body,..}=statement else{unreachable!()};
    match (body.as_slice(),phase) {
        ([Statement::ForEach{collection,..}],Phase::Test)=>Ok(collection),
        ([Statement::ForLoopWithBreak{first,..}],Phase::First)=>Ok(first),
        ([Statement::ForLoopWithBreak{last,..}],Phase::Test)=>Ok(last),
        _=>Err("Native array has no evaluated FirstIndex phase".into()),
    }
}

/// This value is inserted only by the real Engine loop owner into its private
/// evaluation environment, then bound by the existing debugger Request. It is
/// not a source instruction or an authority read from GameState::vars.
pub fn owned_engine_phase(binding:&Binding,variables:&std::collections::HashMap<String,crate::Value>)->Result<Phase,String>{
    match variables.get(&binding.phase) {
        Some(crate::Value::Int(0)) if binding.kind==Kind::Range=>Ok(Phase::First),
        Some(crate::Value::Int(1))=>Ok(Phase::Test),
        _=>Err("Native phase has no actual owning Engine environment".into()),
    }
}

fn children(statement: &Statement)->Vec<&[Statement]> {
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
/// Detect this explicit new source contract without imposing another statement
/// budget on historical scripts. Reuses the same complete child-body walk.
pub fn contains_explicit_loop(script:&[Statement])->bool {
    let mut blocks=vec![script];
    while let Some(block)=blocks.pop(){
        for statement in block {
            if matches!(statement,Statement::ScheduledConsumer{version:3,..}){return true;}
            blocks.extend(children(statement));
        }
    }
    false
}
fn text_expression(segment:&crate::TextSegment)->Option<&Expr>{match segment{crate::TextSegment::Lit(_)=>None,crate::TextSegment::Interp(value)=>Some(value)}}
/// Every Statement field is named deliberately: adding an AST variant/field
/// must re-audit this walk at compilation, rather than silently skip an Expr.
/// Opaque historical asset/method strings are not parsed as expressions.
fn direct(statement: &Statement)->Vec<&Expr> {
    match statement {
        Statement::LibraryUnitCall{call}=>call.arguments().iter().map(|(_,value)|value).collect(),
        Statement::StandardMacroDispatch{dispatch}=>dispatch.operand_expressions(),
        Statement::ScheduledConsumer{version:_,scope:_,consumer:_,steps,body:_}=>steps.iter().map(|step|match step {crate::ConsumerStep::Capture{producer:_,value}|crate::ConsumerStep::InspectTextTable{producer:_,value}=>value}).collect(),
        Statement::SetVar{name:_,value}|Statement::LocalVar{name:_,value}|Statement::FunctionReturn{value}=>vec![value],
        Statement::UnitCall{name:_,arguments}=>arguments.iter().collect(),
        Statement::ForEach{name:_,collection,body:_}=>vec![collection],Statement::ForLoopWithBreak{name:_,first,last,body:_}=>vec![first,last],
        Statement::If{condition,then_branch:_,else_branch:_}|Statement::While{condition,body:_}=>vec![condition],
        Statement::Dialogue{character_id:_,text}=>text.0.iter().filter_map(text_expression).collect(),
        Statement::Choice{options}=>options.iter().flat_map(|o|{let crate::ChoiceOption{label,condition,body:_}=o;condition.iter().chain(label.0.iter().filter_map(text_expression))}).collect(),
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
fn names(value: &Expr, mut visit: impl FnMut(&str)->Result<(),String>)->Result<(),String> {
    let mut stack=vec![value];
    while let Some(value)=stack.pop() {match value {
        Expr::Var(name)=>visit(name)?,
        Expr::LibraryCall{call}=>stack.extend(call.arguments().iter().map(|(_,value)|value)),
        Expr::StandardMacroData{site,output,pin}=>{if site.data_pin(*output)?!=*pin{return Err("Macro live output pin differs".into());}},
        Expr::Call{name:_,args}|Expr::ListLit(args)=>stack.extend(args),
        Expr::BinOp{op:_,left,right}|Expr::Index{target:left,index:right}|Expr::And(left,right)|Expr::Or(left,right)=>{stack.push(left);stack.push(right);},
        Expr::Not(v)|Expr::Neg(v)=>stack.push(v),Expr::Int(_)|Expr::Float(_)|Expr::Bool(_)|Expr::Str(_)=>{},
    }}Ok(())
}
/// Run on ORIGINAL authored source, before compiler-generated temporary writes
/// exist. Only aliases of actual explicitly declared loops are reserved; an
/// unrelated old script using a similar prefix acquires no new restriction.
pub fn validate_authored_bindings(script: &[Statement])->Result<(),String> {
    let mut all=BTreeSet::new();let mut stack=vec![script];
    while let Some(block)=stack.pop(){for statement in block{
        if let Some(b)=binding(statement)?{
            all.extend([b.index,b.cursor,b.phase]);
        }
        stack.extend(children(statement));
    }}
    if all.is_empty(){return Ok(());}
    // An authored node identity may occur in repeated source expansions. Only
    // the actual ancestor occurrence can lend its output to this body. A global
    // map keyed by node IDs would conflate distinct activations/source roots.
    let mut pending=vec![(script,Vec::<(&str,&str,Binding,Option<&Expr>)>::new())];
    while let Some((block,active))=pending.pop(){for statement in block{
        let has_index=|name:&str|active.iter().any(|(_,_,owner,_)|owner.index==name);
        if let Statement::ScheduledConsumer{scope,steps,..}=statement{
            for step in steps{if let crate::ConsumerStep::Capture{producer,value}=step{
                if let Some((_,_,owner,Some(collection)))=active.iter().rev().find(|(source_scope,source_consumer,_,_)|*source_scope==scope.as_str()&&*source_consumer==producer.as_str()){
                    let Expr::Index{target,index}=value else{return Err("Native ArrayElement capture lost its actual lazy array access".into());};
                    if **index!=Expr::Var(owner.index.clone())||target.as_ref()!=*collection{
                        return Err("Native ArrayElement capture differs from its actual source loop/body/collection expression".into());
                    }
                }else if let Some((graph,_))=scope.strip_prefix('g').and_then(|v|v.split_once('/')){
                    if let Ok(graph)=id(graph){if let Ok(endpoint)=node(producer,graph){
                        if endpoint.kind=="for_each"&&endpoint.ports.iter().any(|p|p.role=="array_element"){
                            return Err("Native ArrayElement has no actual declared source loop owner".into());
                        }
                    }}
                }
            }}
        }
        if let Statement::Function{parameters,..}|Statement::UnitFunction{parameters,..}|Statement::Handler{parameters,..}|Statement::Screen{parameters,..}=statement {
            if parameters.iter().any(|name|all.contains(name)){return Err("Authored parameter collides with an actual native loop private binding".into());}
        }
        if let Statement::SetVar{name,..}|Statement::LocalVar{name,..}
            |Statement::ForEach{name,..}|Statement::ForLoopWithBreak{name,..}=statement {
            if all.contains(name){return Err("Authored assignment collides with an actual native loop private binding".into());}
        }
        for expression in direct(statement){names(expression,|name| {
            if all.contains(name)&&!has_index(name){return Err("Native loop projection is outside its actual lexical body".into());}Ok(())
        })?;}
        if let Some(b)=binding(statement)?{
            // Header inputs/steps use the outer context; this loop's index
            // becomes available only for its actual body, never the header.
            let Statement::ScheduledConsumer{scope,consumer,body,steps,..}=statement else{unreachable!()};
            if active.iter().any(|(s,c,_,_)|*s==scope.as_str()&&*c==consumer.as_str()){return Err("Nested native outputs have ambiguous active source ownership".into());}
            if active.iter().any(|(_,_,owner,_)|owner.index==b.index){return Err("Nested native source declarations project the same private index from distinct active occurrences".into());}
            let check_header=|name:&str|if name==b.index||name==b.cursor||name==b.phase||all.contains(name)&&!has_index(name){Err("Loop header cannot borrow its own body index".into())}else{Ok(())};
            for step in steps{let value=match step{crate::ConsumerStep::Capture{value,..}|crate::ConsumerStep::InspectTextTable{value,..}=>value};names(value,&check_header)?;}
            let (body,collection)=match &body[0]{Statement::ForEach{collection,body,..}=>{names(collection,&check_header)?;(body,Some(collection))},
                Statement::ForLoopWithBreak{first,last,body,..}=>{for e in [first,last]{names(e,&check_header)?;}(body,None)},_=>unreachable!()};
            let mut nested=active.clone();nested.push((scope,consumer,b,collection));pending.push((body,nested));
        }else{
            let reset=matches!(statement,Statement::Function{..}|Statement::UnitFunction{..}|Statement::Screen{..}|Statement::Handler{..});
            for body in children(statement){pending.push((body,if reset{Vec::new()}else{active.clone()}));}
        }
    }}Ok(())
}
