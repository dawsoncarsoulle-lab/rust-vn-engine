//! Typed structured flow lowers into the existing saved branch/While machinery.
use rvn_parser::{Statement as S, Expr as E, BinOpKind as B, Script};
use crate::eval::EvalError;
pub(crate) const MAX_ITERATIONS: i64 = 100_000;
pub(crate) fn bounds(first: &rvn_parser::Value, last: &rvn_parser::Value) -> Result<(i64,i64),EvalError> {
    let (rvn_parser::Value::Int(first),rvn_parser::Value::Int(last))=(first,last) else {
        return Err(EvalError::InvalidFunction("ForLoopWithBreak attend deux entiers".into()));
    };
    let count=if first>last {0} else {i128::from(*last)-i128::from(*first)+1};
    if count>i128::from(MAX_ITERATIONS) { return Err(EvalError::ExecutionLimit { limit:"100 000 indices de ForLoopWithBreak" }); }
    Ok((*first,count as i64))
}
pub(crate) fn validate(block:&[S], range:bool, depth:usize)->Result<(),EvalError> {
    // Old scripts without this additive construct keep their former validation.
    if !contains_flow(block) { return Ok(()); }
    if depth>128 {return Err(EvalError::ExecutionLimit {limit:"128 blocs de contrôle imbriqués"});}
    for s in block {match s {
        S::LoopBreak if !range=>return Err(EvalError::InvalidFunction("break hors ForLoopWithBreak".into())),
        S::ForLoopWithBreak{name,body,..}=> {if !rvn_parser::is_variable_binding_name(name){return Err(EvalError::InvalidFunction("index de boucle invalide".into()));} validate(body,true,depth+1)?;},
        S::StandardMacroDispatch{dispatch}=>{dispatch.validate().map_err(EvalError::InvalidFunction)?;for branch in &dispatch.branches {validate(&branch.body,range,depth+1)?;}},
        S::Sequence{branches}=>{if branches.is_empty()||branches.len()>32{return Err(EvalError::InvalidFunction("Sequence attend 1 à 32 sorties".into()));} for b in branches {validate(b,range,depth+1)?;}},
        S::If{then_branch,else_branch,..}=>{validate(then_branch,range,depth+1)?;validate(else_branch,range,depth+1)?;},
        S::Choice{options}=>for option in options {validate(&option.body,range,depth+1)?;},
        S::Imagemap{hotspots,..}=>for h in hotspots {validate(&h.body,range,depth+1)?;},
        S::While{body,..}|S::ForEach{body,..}|S::ScheduledConsumer{body,..}=>validate(body,range,depth+1)?,
        S::Init{body}|S::Function{body,..}|S::UnitFunction{body,..}|S::Screen{body,..}|S::Handler{body,..}=>validate(body,false,depth+1)?,
        _=>{}
    }} Ok(())
}
fn contains_flow(block:&[S])->bool {
    let mut pending=vec![block];
    while let Some(block)=pending.pop(){for statement in block{match statement{
        S::Sequence{..}|S::ForLoopWithBreak{..}|S::LoopBreak=>return true,
        S::If{then_branch,else_branch,..}=>{pending.push(then_branch);pending.push(else_branch);},
        S::StandardMacroDispatch{dispatch}=>for branch in &dispatch.branches {pending.push(&branch.body);},
        S::Choice{options}=>for option in options{pending.push(&option.body);},
        S::Imagemap{hotspots,..}=>for h in hotspots{pending.push(&h.body);},
        S::While{body,..}|S::ForEach{body,..}|S::ScheduledConsumer{body,..}|S::Init{body}|S::Function{body,..}|S::UnitFunction{body,..}|S::Screen{body,..}|S::Handler{body,..}=>pending.push(body),
        _=>{}
    }}}false
}
pub(crate) fn extended_schema(execution:&crate::scene::SceneExecution)->bool {
    use crate::program::{AstRoutePart,GeneratedRole};
    let new_position=|p:&crate::scene::ResumePosition|p.origin.as_ref().is_some_and(|o|
        o.route.iter().any(|r|matches!(r,AstRoutePart::Sequence{..})) || matches!(o.generated,Some(GeneratedRole::ForBreak|GeneratedRole::ForBreakGuard)));
    let range_temporaries=|t:&std::collections::HashMap<String,rvn_parser::Value>|t.keys().any(|name|name.starts_with("__rvn_scene_range_"));
    new_position(&execution.live_position)||execution.display.as_ref().is_some_and(|d|new_position(&d.position))
        ||range_temporaries(&execution.pre_scene_temporaries)
        ||execution.active.as_ref().is_some_and(|a|range_temporaries(&a.temporaries))
        ||execution.frames.iter().any(|f|new_position(&f.continuation)
            ||f.parked.as_ref().is_some_and(|a|range_temporaries(&a.temporaries))
            ||f.pre_scene_temporaries.as_ref().is_some_and(range_temporaries))
}
pub(crate) fn active(flag:&str)->E {E::Not(Box::new(E::Var(flag.into())))}
fn guard(block:Script,flag:&str,counter:&mut usize)->Script {block.into_iter().map(|s|{
    // Source-map labels are stripped before execution and must not create stop guards.
    if matches!(&s,S::Label{name}if name.starts_with("@editor-debug-origin-")){return s;}
    let s=match s {
        S::LoopBreak=>S::SetVar{name:flag.into(),value:E::Bool(true)},
        S::If{condition,then_branch,else_branch}=>S::If{condition,then_branch:guard(then_branch,flag,counter),else_branch:guard(else_branch,flag,counter)},
        S::Sequence{branches}=>S::Sequence{branches:branches.into_iter().map(|b|guard(b,flag,counter)).collect()},
        S::Choice{mut options}=>{for o in &mut options{o.body=guard(std::mem::take(&mut o.body),flag,counter);}S::Choice{options}},
        S::Imagemap{background,hover_image,mut hotspots}=>{for h in &mut hotspots{h.body=guard(std::mem::take(&mut h.body),flag,counter);}S::Imagemap{background,hover_image,hotspots}},
        S::While{condition,body}=>S::While{condition:E::And(Box::new(active(flag)),Box::new(condition)),body:guard(body,flag,counter)},
        S::ScheduledConsumer{version,scope,consumer,steps,body}=>{
            match body.as_slice(){
                [S::While{condition,body}]=>{
                    *counter+=1;let run=format!("__rvn_range_{}_consumer_while",counter);let header=S::ScheduledConsumer{version,scope,consumer,steps,body:vec![S::If{condition:condition.clone(),then_branch:guard(body.clone(),flag,counter),else_branch:vec![S::SetVar{name:run.clone(),value:E::Bool(false)}]}]};
                    S::Sequence{branches:vec![vec![S::SetVar{name:run.clone(),value:E::Bool(true)},S::While{condition:E::And(Box::new(active(flag)),Box::new(E::Var(run))),body:vec![header]}]]}
                },
                [S::If{condition,then_branch,else_branch}]=>S::ScheduledConsumer{version,scope,consumer,steps,body:vec![S::If{condition:condition.clone(),then_branch:guard(then_branch.clone(),flag,counter),else_branch:guard(else_branch.clone(),flag,counter)}]},
                [S::Choice{options}]=>{let mut options=options.clone();for option in &mut options{option.body=guard(std::mem::take(&mut option.body),flag,counter);}S::ScheduledConsumer{version,scope,consumer,steps,body:vec![S::Choice{options}]}},
                [S::ForEach{..}]=>{
                    let mut guarded=guard(body,flag,counter);let S::If{mut then_branch,..}=guarded.remove(0)else{unreachable!()};let S::Sequence{mut branches}=then_branch.remove(0)else{unreachable!()};let mut lowered=branches.remove(0);lower_legacy(&mut lowered,counter);
                    let first=lowered.remove(0);let header=S::ScheduledConsumer{version,scope,consumer,steps,body:vec![first]};let mut children=vec![header];children.extend(lowered);S::Sequence{branches:vec![children]}
                },
                // Scalar consumers already receive the surrounding stop guard;
                // their captures must remain available to the direct RHS only.
                _=>S::ScheduledConsumer{version,scope,consumer,steps,body},
            }
        },
        S::ForEach{name,collection,body}=>{
            *counter+=1;let items=format!("__rvn_range_{}_items",counter);let cursor=format!("__rvn_range_{}_each",counter);
            let mut iteration=vec![S::SetVar{name,value:E::Index{target:Box::new(E::Var(items.clone())),index:Box::new(E::Var(cursor.clone()))}},S::SetVar{name:cursor.clone(),value:E::BinOp{op:B::Add,left:Box::new(E::Var(cursor.clone())),right:Box::new(E::Int(1))}}];
            iteration.extend(guard(body,flag,counter));
            S::Sequence{branches:vec![vec![S::SetVar{name:items.clone(),value:E::Call{name:"__rvn_iterable".into(),args:vec![collection]}},S::SetVar{name:cursor.clone(),value:E::Int(0)},S::While{condition:E::And(Box::new(active(flag)),Box::new(E::BinOp{op:B::Lt,left:Box::new(E::Var(cursor)),right:Box::new(E::Call{name:"len".into(),args:vec![E::Var(items)]})})),body:iteration}]]}
        },
        other=>other,
    }; S::If{condition:active(flag),then_branch:vec![s],else_branch:vec![]}
}).collect()}
pub(crate) fn lower_legacy(script:&mut Script,counter:&mut usize) {
    let mut out=Vec::new();
    for s in std::mem::take(script) {match s {
        S::Sequence{branches}=>for mut b in branches {lower_legacy(&mut b,counter);out.extend(b);},
        S::ForLoopWithBreak{name,first,last,body}=>{
            *counter+=1; let prefix=format!("__rvn_range_{}",counter);
            let bounds=format!("{prefix}_bounds"); let cursor=format!("{prefix}_cursor"); let stop=format!("{prefix}_stop");
            out.push(S::SetVar{name:bounds.clone(),value:E::Call{name:"__rvn_for_bounds".into(),args:vec![first,last]}});
            out.push(S::SetVar{name:cursor.clone(),value:E::Int(0)});
            out.push(S::SetVar{name:stop.clone(),value:E::Bool(false)});
            let at=|index|E::Index{target:Box::new(E::Var(bounds.clone())),index:Box::new(E::Int(index))};
            let mut iteration=vec![S::SetVar{name,value:E::BinOp{op:B::Add,left:Box::new(at(0)),right:Box::new(E::Var(cursor.clone()))}},S::SetVar{name:cursor.clone(),value:E::BinOp{op:B::Add,left:Box::new(E::Var(cursor.clone())),right:Box::new(E::Int(1))}}];
            iteration.extend(guard(body,&stop,counter)); lower_legacy(&mut iteration,counter);
            out.push(S::While{condition:E::And(Box::new(active(&stop)),Box::new(E::BinOp{op:B::Lt,left:Box::new(E::Var(cursor)),right:Box::new(at(1))})),body:iteration});
        },
        other=>out.push(other)
    }} *script=out;
}
