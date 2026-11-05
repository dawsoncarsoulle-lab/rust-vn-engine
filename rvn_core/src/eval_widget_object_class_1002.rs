//! Inspect the real executable slots once, without cloning FunctionLibrary.
//! Tint strings are parsed by the same interpolation parser as the runtime;
//! their short-lived expression tree is not a second stored program/model.
use super::*;
fn expression_uses_getter(root:&Expr)->bool{
    let mut expressions=vec![root];
    while let Some(expr)=expressions.pop(){match expr{
        Expr::LibraryCall{call}=>expressions.extend(call.arguments().iter().map(|(_,value)|value)),
        Expr::Call{name,args}=>{if name=="__rvn_widget_object_class_v1"{return true;}expressions.extend(args);},
        Expr::Neg(value)|Expr::Not(value)=>expressions.push(value),
        Expr::BinOp{left,right,..}|Expr::And(left,right)|Expr::Or(left,right)=>expressions.extend([left.as_ref(),right.as_ref()]),
        Expr::Index{target,index}=>expressions.extend([target.as_ref(),index.as_ref()]),
        Expr::ListLit(items)=>expressions.extend(items),_=>{}
    }}false
}
pub(super) fn script_uses_getter(script:&[Statement])->bool {
    let mut blocks=vec![script];
    while let Some(block)=blocks.pop(){for statement in block{
        if let Statement::FunctionLibrary{scope}=statement{for member in scope.members(){blocks.push(std::slice::from_ref(member.declaration()));}}
        let mut expressions=crate::consumer_schedule::direct_expressions(statement);
        if let Statement::ScheduledConsumer{steps,..}=statement {for step in steps{match step{
            rvn_parser::ConsumerStep::Capture{value,..}|rvn_parser::ConsumerStep::InspectTextTable{value,..}=>expressions.push(value),
        }}}
        if expressions.into_iter().any(expression_uses_getter){return true;}
        if let Statement::SpriteEffect{tint:Some(tint),..}=statement{
            // Malformed tint still receives its existing error when reached;
            // this capability scan neither admits nor repairs the expression.
            if rvn_parser::parse_interpolated_str(tint).is_ok_and(|text|text.0.iter().any(|segment|matches!(segment,TextSegment::Interp(expression) if expression_uses_getter(expression)))){return true;}
        }
        blocks.extend(crate::consumer_schedule::statement_children(statement));
    }}false
}
