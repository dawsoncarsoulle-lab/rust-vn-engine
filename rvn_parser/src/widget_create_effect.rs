//! Borrowed interpretation of one effectful assignment in the existing AST.
//! This is not a second AST or a pure callable producer. Only an Engine-owned
//! reached v2 Consumer may execute it; ordinary evaluation must reject it.
use crate::{Expr, Statement};
#[derive(Clone, Copy)]
pub enum ClassInput<'a> {
    Asset(&'a str),
    Null,
    Value(&'a Expr),
}
pub struct FactoryInput<'a> {
    pub binding: &'a str,
    pub local: bool,
    pub class: ClassInput<'a>,
    pub arguments: &'a Expr,
}
pub fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        "__rvn_widget_create_v1" | "__rvn_widget_class_v1" | "__rvn_widget_class_none_v1"
    )
}
pub fn parts(statement: &Statement) -> Result<Option<FactoryInput<'_>>, String> {
    let (binding, value, local) = match statement {
        Statement::SetVar { name, value } => (name, value, false),
        Statement::LocalVar { name, value } => (name, value, true),
        _ => return Ok(None),
    };
    let Expr::Call { name, args } = value else {
        return Ok(None);
    };
    if name != "__rvn_widget_create_v1" {
        return Ok(None);
    }
    if !crate::is_variable_binding_name(binding) || binding.starts_with("persistent.") {
        return Err("Widget factory result requires a real non-persistent variable binding".into());
    }
    let [class, arguments] = args.as_slice() else {
        return Err(
            "__rvn_widget_create_v1 requires a typed class input and an argument list".into(),
        );
    };
    let class = match class {
        Expr::Call { name, args } if name == "__rvn_widget_class_v1" => {
            let [Expr::Str(uuid)] = args.as_slice() else {
                return Err(
                    "__rvn_widget_class_v1 requires one literal indexed asset identity".into(),
                );
            };
            crate::localized_text::validate_text_asset_id(uuid)?;
            ClassInput::Asset(uuid)
        }
        Expr::Call { name, args } if name == "__rvn_widget_class_none_v1" => {
            if !args.is_empty() {
                return Err("__rvn_widget_class_none_v1 has no operands".into());
            }
            ClassInput::Null
        }
        Expr::Str(_) | Expr::ListLit(_) => {
            return Err("A string, list or dictionary cannot become a Widget class".into())
        }
        Expr::Call { name, .. } if name == "dict" => {
            return Err("A dictionary cannot become a Widget class".into())
        }
        value => ClassInput::Value(value),
    };
    Ok(Some(FactoryInput {
        binding,
        local,
        class,
        arguments,
    }))
}
pub fn direct_inputs<'a>(input: &FactoryInput<'a>) -> Vec<&'a Expr> {
    match input.class {
        ClassInput::Value(value) => vec![value, input.arguments],
        ClassInput::Asset(_) | ClassInput::Null => vec![input.arguments],
    }
}
/// Actual nested call argument ordinal, not a fabricated direct-command slot.
pub fn argument_ordinal(input: &FactoryInput<'_>, gathered_index: usize) -> usize {
    if matches!(input.class, ClassInput::Value(_)) {
        gathered_index
    } else {
        gathered_index + 1
    }
}
#[cfg(test)]
#[path = "widget_create_effect_tests_0978.rs"]
mod tests;
