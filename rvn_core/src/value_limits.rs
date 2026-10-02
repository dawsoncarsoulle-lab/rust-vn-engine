//! Finite collection and text budgets, shared by evaluation and save loading.
use crate::eval::EvalError;
use rvn_parser::Value;

pub const MAX_COLLECTION_ITEMS: usize = 4096;
pub const MAX_VALUE_BYTES: usize = 1_048_576;

pub fn value_work(value: &Value) -> Result<usize, EvalError> {
    fn walk(
        value: &Value,
        depth: usize,
        nodes: &mut usize,
        bytes: &mut usize,
    ) -> Result<(), EvalError> {
        *nodes += 1;
        if depth > 64 || *nodes > 65_536 {
            return Err(EvalError::ExecutionLimit {
                limit: "64 niveaux et 65 536 valeurs par collection",
            });
        }
        let text = |text: &str, bytes: &mut usize| -> Result<(), EvalError> {
            *bytes = bytes
                .checked_add(text.len())
                .ok_or(EvalError::NumericOverflow)?;
            if *bytes > MAX_VALUE_BYTES {
                Err(EvalError::ExecutionLimit {
                    limit: "1 Mio de texte par valeur",
                })
            } else {
                Ok(())
            }
        };
        match value {
            Value::Str(string) => text(string, bytes)?,
            Value::Float(number) if !number.is_finite() => return Err(EvalError::NumericOverflow),
            Value::List(items) => {
                if items.len() > MAX_COLLECTION_ITEMS {
                    return Err(EvalError::ExecutionLimit {
                        limit: "4 096 éléments par collection",
                    });
                }
                for item in items {
                    walk(item, depth + 1, nodes, bytes)?;
                }
            }
            Value::Dict(items) => {
                if items.len() > MAX_COLLECTION_ITEMS {
                    return Err(EvalError::ExecutionLimit {
                        limit: "4 096 éléments par collection",
                    });
                }
                for (key, item) in items {
                    text(key, bytes)?;
                    walk(item, depth + 1, nodes, bytes)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut nodes = 0;
    let mut bytes = 0;
    walk(value, 0, &mut nodes, &mut bytes)?;
    Ok(nodes + bytes / 32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_depth_size_and_numbers_are_required() {
        assert!(value_work(&Value::Float(f32::INFINITY)).is_err());
        assert!(value_work(&Value::List(vec![Value::Int(0); 4097])).is_err());
        assert!(value_work(&Value::Str("x".repeat(MAX_VALUE_BYTES + 1))).is_err());
        let mut value = Value::Int(0);
        for _ in 0..65 {
            value = Value::List(vec![value]);
        }
        assert!(value_work(&value).is_err());
    }
    #[test]
    fn exponential_growth_stops_before_exhausting_memory() {
        let script=rvn_parser::parse("function grow() { set items=[0] while true { set items=list_concat(items,items) } return items }").unwrap();
        let library = crate::eval::FunctionLibrary::from_script(&script).unwrap();
        assert!(matches!(
            library.eval(
                &rvn_parser::Expr::Call {
                    name: "grow".into(),
                    args: vec![]
                },
                &Default::default()
            ),
            Err(EvalError::ExecutionLimit { .. })
        ));
        let globals =
            std::collections::HashMap::from([("large".into(), Value::Str("x".repeat(4096)))]);
        let expression = rvn_parser::Expr::Call {
            name: "replace".into(),
            args: vec![
                rvn_parser::Expr::Var("large".into()),
                rvn_parser::Expr::Str("x".into()),
                rvn_parser::Expr::Var("large".into()),
            ],
        };
        assert!(matches!(
            library.eval(&expression, &globals),
            Err(EvalError::ExecutionLimit { .. })
        ));
    }
}
