//! Read parameter uses from the admitted source projection, never from a cache
//! name search or a UUID without its actual owner/declaration/entry pin.
use crate::*;
use rvn_parser::{Expr,Statement,TextSegment};

fn expression_uses(root:&Expr,name:&str)->bool{
    let mut pending=vec![root];
    while let Some(value)=pending.pop(){match value{
        Expr::Var(found)if found==name=>return true,
        Expr::Call{args,..}|Expr::ListLit(args)=>pending.extend(args),
        Expr::BinOp{left,right,..}|Expr::And(left,right)|Expr::Or(left,right)
            |Expr::Index{target:left,index:right}=>{pending.push(left);pending.push(right);},
        Expr::Neg(value)|Expr::Not(value)=>pending.push(value),
        // A function's callee name and a string/template payload do not read
        // a variable. A real interpolation is a TextSegment::Interp instead.
        _=>{},
    }}false
}
fn text_uses(text:&rvn_parser::InterpolatedText,name:&str)->bool{
    text.0.iter().any(|segment|matches!(segment,TextSegment::Interp(value)if expression_uses(value,name)))
}
fn body_uses(body:&[Statement],name:&str)->Result<bool,String>{
    let mut pending=body.iter().collect::<Vec<_>>();
    while let Some(statement)=pending.pop(){match statement{
        Statement::SetVar{name:target,value}|Statement::LocalVar{name:target,value}=>{
            if target==name||expression_uses(value,name){return Ok(true);}
        },
        Statement::UnitCall{arguments,..}=>if arguments.iter().any(|value|expression_uses(value,name)){return Ok(true);},
        Statement::If{condition,then_branch,else_branch}=>{
            if expression_uses(condition,name){return Ok(true);}pending.extend(then_branch);pending.extend(else_branch);
        },
        Statement::Sequence{branches}=>for branch in branches{pending.extend(branch);},
        Statement::While{condition,body}=>{if expression_uses(condition,name){return Ok(true);}pending.extend(body);},
        Statement::ForEach{name:target,collection,body}=>{
            if target==name||expression_uses(collection,name){return Ok(true);}pending.extend(body);
        },
        Statement::ForLoopWithBreak{name:target,first,last,body}=>{
            if target==name||expression_uses(first,name)||expression_uses(last,name){return Ok(true);}pending.extend(body);
        },
        Statement::Dialogue{text,..}=>if text_uses(text,name){return Ok(true);},
        Statement::Choice{options}=>for option in options{
            if text_uses(&option.label,name)||option.condition.as_ref().is_some_and(|value|expression_uses(value,name)){return Ok(true);}
            pending.extend(&option.body);
        },
        Statement::FunctionReturn{value}=>if expression_uses(value,name){return Ok(true);},
        Statement::UnitReturn|Statement::LoopBreak|Statement::Return=>{},
        // These are independent lexical activations, not nested access to the
        // selected parameter. Current source validation rejects nested defs.
        Statement::Function{..}|Statement::UnitFunction{..}|Statement::Screen{..}|Statement::Handler{..}=>{},
        _=>return Err("The parameter reference reader has no admitted contract for this statement; no edit was published".into()),
    }}Ok(false)
}

pub(crate) fn source_uses_parameter(workspace:&SourceWorkspace,graphs:&[GraphDocument],
    declaration:GraphId,pin:&GraphPin)->Result<bool,String>{
    // Caller retains the existing with_authoring_admission guard; acquiring a
    // second workspace writer lock here would deadlock the real transaction.
        let graph=graphs.iter().find(|graph|graph.graph_id==declaration).ok_or("Parameter declaration is outside this owner")?;
        let GraphKind::UnitFunction{name}=&graph.kind else{return Err("An actual Unit declaration is required".into());};
        let parameters=crate::unit_graph_source1003::parameters(graph)?;
        if !parameters.iter().any(|actual|*actual==pin){return Err("Parameter reference query does not identify a current actual entry pin".into());}
        // Reuse the real loss-checked owner projection and import/arity/mode
        // validation. The candidate is disposable and writes no source/sidecar.
        let mut candidate=workspace.project().clone();
        candidate.apply_visual(workspace.project().source(),graphs)?;
        let program=candidate.resolved_program()?;
        let declarations=program.script.iter().filter_map(|statement|match statement{
            Statement::UnitFunction{name:actual,parameters,body}if actual==name=>Some((parameters,body)),_=>None,
        }).collect::<Vec<_>>();
        let [(names,body)]=declarations.as_slice()else{return Err("Parameter declaration must resolve uniquely in its actual source owner".into());};
        if names.iter().map(String::as_str).collect::<Vec<_>>()!=parameters.iter().map(|pin|pin.label.as_str()).collect::<Vec<_>>(){
            return Err("Resolved source parameters differ from the admitted actual entry pins".into());
        }
        body_uses(body,&pin.label)
}

pub(crate) fn rename_unit_slot_references(graph:&mut GraphDocument,old:&str,new:&str)->Result<(),String>{
    // Unit currently admits synchronous computation only. Its TextValue data
    // outputs are literal strings/DeferredText payloads, not Dialogue/Choice
    // interpolation consumers. Keep those exact bytes, including unfinished
    // bracket spellings that are perfectly valid string data.
    let mut payloads=Vec::new();
    for node in graph.nodes.values_mut(){
        let keys=node.properties.keys().filter(|key|
            (node.kind==NodeKind::TextValue && key.as_str()=="value")
            ||(node.kind==NodeKind::Choice && (key.as_str()=="options"||key.starts_with("option_")&&key.ends_with("_condition"))))
            .cloned().collect::<Vec<_>>();
        for key in keys{payloads.push((node.id,key.clone(),node.properties.remove(&key).unwrap()));}
    }
    // The entry pin has already admitted the lexical slot. Always rewrite
    // its actual consumers; retain a cache only when one existed beforehand.
    let result=graph.rename_variable_reference_payloads(old,new).map_err(|error|error.to_string());
    if result.is_ok(){if let Some(mut definition)=graph.variables.remove(old){
        definition.name=new.into();graph.variables.insert(new.into(),definition);
    }}
    for(node,key,value)in payloads{graph.nodes.get_mut(&node).ok_or("Unit literal producer disappeared")?.properties.insert(key,value);}
    result
}

#[cfg(test)]
#[path="unit_parameter_reference_tests1003.rs"]
mod tests;
