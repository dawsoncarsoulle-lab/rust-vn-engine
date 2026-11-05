//! Borrowed grammar contract for actual Widget object viewport effects.
//! These are statements, never pure functions or named legacy Screens.
use crate::{Expr, Statement};

pub enum ViewportInput<'a> {
    Add { object: &'a Expr, z_order: &'a Expr },
    Remove { object: &'a Expr },
}
pub fn parts(statement: &Statement) -> Option<ViewportInput<'_>> {
    match statement {
        Statement::WidgetAddToViewport { object, z_order } => Some(ViewportInput::Add { object, z_order }),
        Statement::WidgetRemoveFromParent { object } => Some(ViewportInput::Remove { object }),
        _ => None,
    }
}
pub fn direct_inputs(statement: &Statement) -> Option<Vec<&Expr>> {
    Some(match parts(statement)? {
        ViewportInput::Add { object, z_order } => vec![object, z_order],
        ViewportInput::Remove { object } => vec![object],
    })
}
pub fn from_call(method: &str, arguments: &[Expr]) -> Result<Statement, String> {
    match (method, arguments) {
        ("add_to_viewport", [object]) => Ok(Statement::WidgetAddToViewport { object: object.clone(), z_order: Expr::Int(0) }),
        ("add_to_viewport", [object, z_order]) => Ok(Statement::WidgetAddToViewport { object: object.clone(), z_order: z_order.clone() }),
        ("remove_from_parent", [object]) => Ok(Statement::WidgetRemoveFromParent { object: object.clone() }),
        _ => Err("widget.add_to_viewport(objet, ordreZ=0) ou widget.remove_from_parent(objet) attendu".into()),
    }
}
pub fn identity_contract(statement: &Statement) -> Option<(&'static str, &'static [&'static str])> {
    match statement {
        Statement::WidgetAddToViewport { .. } => Some(("widget_add_to_viewport", &["exec_in", "object", "z_order", "exec_out"])),
        Statement::WidgetRemoveFromParent { .. } => Some(("widget_remove_from_parent", &["exec_in", "object", "exec_out"])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handler_remove1003_parser_admits_only_removal_as_a_handler_effect() {
        let ast = crate::parse("handler click(event){widget.remove_from_parent(value)}").unwrap();
        let [Statement::Handler { body, .. }] = ast.as_slice() else { panic!("real Handler declaration"); };
        assert!(matches!(body.as_slice(), [Statement::WidgetRemoveFromParent { object: Expr::Var(name) }] if name == "value"));
        for source in ["handler click(e){widget.add_to_viewport(value)}", "function pure(){widget.remove_from_parent(value)}", "screen panel(){widget.remove_from_parent(value)}"] {
            assert!(crate::parse(source).is_err(), "{source}");
        }
    }
    #[test]
    fn viewport1003_effect_grammar_defaults_and_json_preserve_typed_operands() {
        let parsed = crate::parse("widget.add_to_viewport(value)\nwidget.remove_from_parent(value)\n").unwrap();
        assert!(matches!(&parsed[0], Statement::WidgetAddToViewport { object: Expr::Var(name), z_order: Expr::Int(0) } if name == "value"));
        assert_eq!(direct_inputs(&parsed[0]).unwrap().len(), 2);
        assert_eq!(direct_inputs(&parsed[1]).unwrap().len(), 1);
        let wire = serde_json::to_string(&parsed).unwrap();
        assert_eq!(serde_json::from_str::<Vec<Statement>>(&wire).unwrap(), parsed);
        assert!(from_call("unknown", &[Expr::Int(0)]).is_err());
        for source in ["widget.add_to_viewport()", "widget.add_to_viewport(x,0,1)", "widget.remove_from_parent()", "function pure(){widget.add_to_viewport(x)}", "handler event(e){widget.add_to_viewport(x)}", "function pure(){widget.remove_from_parent(x)}", "screen panel(){widget.remove_from_parent(x)}"] {
            assert!(crate::parse(source).is_err(), "{source}");
        }
        assert!(matches!(crate::parse("widget.effect(tint:\"#ffffffff\")").unwrap().as_slice(), [Statement::SpriteEffect { character_id, .. }] if character_id == "widget"), "the historical Sprite named widget keeps its methods");
    }
    #[test]
    fn handler_remove1003_scheduled_sources_authenticate_actual_native_handler_root() {
        // Exact Handler authored by the real SourceProject/Graph fixture that
        // exposed the lexical-root mismatch; no AST or endpoint is fabricated.
        const SOURCE: &str = r###"handler allowed(event){ flow.consume_v1("g2/n1:handler_entry|exec_out=1@0", "n2:local_variable|exec_in=2@1,name=3@0,value=4@2,exec_out=5@0,value_out=6@0") {
        capture "n3:function_call|result=7@0,item_0=8@0,item_1=9@0"  =random(1,100) local roll = __rvn_captured_v1("n3:function_call|result=7@0,item_0=8@0,item_1=9@0")
    }
    flow.consume_v2("g2/n1:handler_entry|exec_out=1@0", "n9:widget_remove_from_parent|exec_in=25@8,exec_out=26@0,object=27@10") {
        capture "n10:variable_get|value=28@0" = saved_object
        widget.remove_from_parent(__rvn_captured_v1("n10:variable_get|value=28@0"))
    }
    flow.consume_v1("g2/n1:handler_entry|exec_out=1@0", "n4:local_variable|exec_in=10@9,name=11@0,value=12@0,exec_out=13@0,value_out=14@0") {
        local  removed_marker=1 }
    flow.consume_v1("g2/n1:handler_entry|exec_out=1@0", "n5:set_variable|exec_in=15@4,name=16@0,value=17@7,exec_out=18@0,value_out=19@0") {
        capture "n7:variable_get|value=23@0"   =hits capture "n8:literal|value=24@0" = 1 capture "n6:math_add|left=20@5,right=21@6,value=22@0" = (__rvn_captured_v1("n7:variable_get|value=23@0") + __rvn_captured_v1("n8:literal|value=24@0"))
        set hits = __rvn_captured_v1("n6:math_add|left=20@5,right=21@6,value=22@0")
    }  }
"###;
        let program = crate::parse_program(SOURCE).unwrap();
        let sites = crate::scheduled_source::scheduled_step_sources(&program).unwrap();
        let removal = sites.iter().filter(|site| site.consumer_identity.starts_with("n9:widget_remove_from_parent|")).collect::<Vec<_>>();
        let [actual] = removal.as_slice() else { panic!("one actual scheduled Remove object producer"); };
        assert_eq!(actual.definition_root_ordinal, 0);
        assert_eq!(actual.source_ordinal, 0);
        assert_eq!(actual.scope.as_deref(), Some("handler:allowed"));
        assert!(actual.plan_scope.contains("/n1:handler_entry|"));
        assert_eq!(&SOURCE[actual.expression.clone()], "saved_object");
        let endpoint = actual.endpoint.as_ref().expect("actual variable getter identity");
        assert_eq!(endpoint.graph, 2); assert_eq!(endpoint.node, 10); assert_eq!(endpoint.kind, "variable_get");
        assert!(crate::parse_program(&SOURCE.replacen("flow.consume_v2", "flow.consume_v1", 1)).is_err(), "v1 grammar cannot hold a viewport effect");
        for refused in [
            SOURCE.replace("n1:handler_entry|", "n1:function_entry|"),
            SOURCE.replace("n9:widget_remove_from_parent|exec_in=25@8,exec_out=26@0,object=27@10", "n9:widget_remove_from_parent|exec_in=25@8,exec_out=26@0"),
        ] {
            let parsed = crate::parse_program(&refused).unwrap();
            assert!(crate::scheduled_source::scheduled_step_sources(&parsed).is_err(), "wrong lexical root or ports must not be admitted: {refused}");
        }
    }

}
