use crate::error::EvalErrorKind;
use rvn_parser::{BinOpKind, Expr, InterpolatedText, TextSegment, Value};
use std::collections::HashMap;

pub type EvalError = EvalErrorKind;
pub type EvalResult<T> = Result<T, EvalError>;

// ─── ÉVALUATEUR ──────────────────────────────────────────────────────────────

pub fn eval_expr(expr: &Expr, vars: &HashMap<String, Value>) -> EvalResult<Value> {
    match expr {
        Expr::Int(n) => Ok(Value::Int(*n)),
        Expr::Float(f) => Ok(Value::Float(*f)),
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Str(s) => Ok(Value::Str(s.clone())),

        Expr::Var(name) => vars
            .get(name)
            .cloned()
            .ok_or_else(|| EvalError::UndefinedVar(name.clone())),

        Expr::Neg(inner) => match eval_expr(inner, vars)? {
            Value::Int(n) => Ok(Value::Int(-n)),
            Value::Float(f) => Ok(Value::Float(-f)),
            other => Err(EvalError::TypeMismatch {
                op: "-".into(),
                left: type_name(&other).into(),
                right: "".into(),
            }),
        },

        Expr::Not(inner) => Ok(Value::Bool(!eval_bool(inner, vars)?)),

        Expr::And(l, r) => {
            if !eval_bool(l, vars)? {
                return Ok(Value::Bool(false));
            }
            Ok(Value::Bool(eval_bool(r, vars)?))
        }
        Expr::Or(l, r) => {
            if eval_bool(l, vars)? {
                return Ok(Value::Bool(true));
            }
            Ok(Value::Bool(eval_bool(r, vars)?))
        }

        Expr::BinOp { op, left, right } => eval_binop(op, left, right, vars),
        Expr::Call { name, args } => eval_call(name, args, vars),
        Expr::ListLit(items) => {
            let evaluated: Vec<Value> = items
                .iter()
                .map(|e| eval_expr(e, vars))
                .collect::<Result<_, _>>()?;
            Ok(Value::List(evaluated))
        }
        Expr::Index { target, index } => {
            let target_val = eval_expr(target, vars)?;
            let index_val = eval_expr(index, vars)?;
            match (&target_val, &index_val) {
                (Value::List(items), Value::Int(i)) => {
                    let idx = *i as usize;
                    items
                        .get(idx)
                        .cloned()
                        .ok_or_else(|| EvalError::TypeMismatch {
                            op: "index".into(),
                            left: format!("index {} out of bounds (len {})", idx, items.len()),
                            right: "".into(),
                        })
                }
                (Value::Str(string), Value::Int(i)) => {
                    let chars: Vec<char> = string.chars().collect();
                    let idx = *i as usize;
                    chars
                        .get(idx)
                        .map(|c| Value::Str(c.to_string()))
                        .ok_or_else(|| EvalError::TypeMismatch {
                            op: "index".into(),
                            left: format!("index {} out of bounds (len {})", idx, chars.len()),
                            right: "".into(),
                        })
                }
                _ => Err(EvalError::TypeMismatch {
                    op: "index".into(),
                    left: "list or string".into(),
                    right: "int".into(),
                }),
            }
        }
    }
}

fn eval_call(name: &str, args: &[Expr], vars: &HashMap<String, Value>) -> EvalResult<Value> {
    let evaluated: Vec<Value> = args
        .iter()
        .map(|a| eval_expr(a, vars))
        .collect::<Result<_, _>>()?;
    match name {
        "make_color" | "make_color_rgb" => {
            let nums = expect_numbers(&evaluated, "make_color")?;
            if nums.len() != 4 || nums.iter().any(|n| !n.is_finite()) {
                return Err(EvalError::TypeMismatch { op: "make_color".into(), left: "4 finite RGBA numbers".into(), right: "".into() });
            }
            let bytes: Vec<_> = nums.into_iter().enumerate().map(|(channel, n)| {
                let maximum = if name == "make_color_rgb" && channel < 3 { 255.0 } else { 1.0 };
                (n.clamp(0.0, maximum) / maximum * 255.0).round() as u8
            }).collect();
            Ok(Value::Str(format!("#{:02x}{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2], bytes[3])))
        }
        "min" => {
            let nums = expect_numbers(&evaluated, "min")?;
            Ok(nums
                .into_iter()
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(to_value)
                .unwrap_or(Value::Int(0)))
        }
        "max" => {
            let nums = expect_numbers(&evaluated, "max")?;
            Ok(nums
                .into_iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(to_value)
                .unwrap_or(Value::Int(0)))
        }
        "abs" => match evaluated.as_slice() {
            [Value::Int(n)] => Ok(Value::Int(n.abs())),
            [Value::Float(f)] => Ok(Value::Float(f.abs())),
            _ => Err(EvalError::TypeMismatch {
                op: "abs".into(),
                left: "number".into(),
                right: "".into(),
            }),
        },
        "floor" => match evaluated.as_slice() {
            [Value::Float(f)] => Ok(Value::Int(*f as i64)),
            [Value::Int(n)] => Ok(Value::Int(*n)),
            _ => Err(EvalError::TypeMismatch {
                op: "floor".into(),
                left: "number".into(),
                right: "".into(),
            }),
        },
        "to_int" => match evaluated.as_slice() {
            // Comme la conversion String → Integer de Blueprint, un texte
            // vide ou non numérique produit 0 plutôt qu’une erreur fatale.
            [Value::Str(value)] => Ok(Value::Int(value.trim().parse::<i64>().unwrap_or(0))),
            [Value::Int(value)] => Ok(Value::Int(*value)),
            [Value::Float(value)] => Ok(Value::Int(*value as i64)),
            _ => Err(EvalError::TypeMismatch {
                op: "to_int".into(),
                left: "string or number".into(),
                right: "int".into(),
            }),
        },
        "ceil" => match evaluated.as_slice() {
            [Value::Float(f)] => Ok(Value::Int(
                (*f as i64) + if *f > 0.0 && f.fract() > 0.0 { 1 } else { 0 },
            )),
            [Value::Int(n)] => Ok(Value::Int(*n)),
            _ => Err(EvalError::TypeMismatch {
                op: "ceil".into(),
                left: "number".into(),
                right: "".into(),
            }),
        },
        "random" | "rand" => {
            match evaluated.as_slice() {
                [Value::Int(lo), Value::Int(hi)] => {
                    let lo = *lo;
                    let hi = *hi;
                    let range = (hi - lo + 1).max(1) as u64;
                    // Simple LCG for determinism without external deps
                    let seed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(0);
                    let val = lo + ((seed % range) as i64);
                    Ok(Value::Int(val))
                }
                _ => Err(EvalError::TypeMismatch {
                    op: "random".into(),
                    left: "int, int".into(),
                    right: "".into(),
                }),
            }
        }
        "len" => match evaluated.as_slice() {
            [Value::Str(s)] => Ok(Value::Int(s.len() as i64)),
            [Value::List(items)] => Ok(Value::Int(items.len() as i64)),
            _ => Err(EvalError::TypeMismatch {
                op: "len".into(),
                left: "string or list".into(),
                right: "".into(),
            }),
        },
        "contains" => match evaluated.as_slice() {
            [Value::List(items), item] => Ok(Value::Bool(items.iter().any(|i| i == item))),
            [Value::Str(haystack), Value::Str(needle)] => {
                Ok(Value::Bool(haystack.contains(needle.as_str())))
            }
            _ => Err(EvalError::TypeMismatch {
                op: "contains".into(),
                left: "list+value or string+string".into(),
                right: "".into(),
            }),
        },
        "upper" => match evaluated.as_slice() {
            [Value::Str(s)] => Ok(Value::Str(s.to_uppercase())),
            _ => Err(EvalError::TypeMismatch {
                op: "upper".into(),
                left: "string".into(),
                right: "".into(),
            }),
        },
        "lower" => match evaluated.as_slice() {
            [Value::Str(s)] => Ok(Value::Str(s.to_lowercase())),
            _ => Err(EvalError::TypeMismatch {
                op: "lower".into(),
                left: "string".into(),
                right: "".into(),
            }),
        },
        "capitalize" => match evaluated.as_slice() {
            [Value::Str(s)] => {
                let mut c = s.chars();
                match c.next() {
                    Some(first) => Ok(Value::Str(
                        first.to_uppercase().collect::<String>() + c.as_str(),
                    )),
                    None => Ok(Value::Str(String::new())),
                }
            }
            _ => Err(EvalError::TypeMismatch {
                op: "capitalize".into(),
                left: "string".into(),
                right: "".into(),
            }),
        },
        "trim" => match evaluated.as_slice() {
            [Value::Str(s)] => Ok(Value::Str(s.trim().to_string())),
            _ => Err(EvalError::TypeMismatch {
                op: "trim".into(),
                left: "string".into(),
                right: "".into(),
            }),
        },
        "replace" => match evaluated.as_slice() {
            [Value::Str(s), Value::Str(from), Value::Str(to)] => {
                Ok(Value::Str(s.replace(from.as_str(), to.as_str())))
            }
            _ => Err(EvalError::TypeMismatch {
                op: "replace".into(),
                left: "string, string, string".into(),
                right: "".into(),
            }),
        },
        "substring" => match evaluated.as_slice() {
            [Value::Str(s), Value::Int(start), Value::Int(end)] => {
                let chars: Vec<char> = s.chars().collect();
                let start = (*start as usize).min(chars.len());
                let end = (*end as usize).min(chars.len());
                if start <= end {
                    Ok(Value::Str(chars[start..end].iter().collect()))
                } else {
                    Ok(Value::Str(String::new()))
                }
            }
            _ => Err(EvalError::TypeMismatch {
                op: "substring".into(),
                left: "string, int, int".into(),
                right: "".into(),
            }),
        },
        "split" => match evaluated.as_slice() {
            [Value::Str(s), Value::Str(sep)] => {
                let parts: Vec<Value> = s
                    .split(sep.as_str())
                    .map(|p| Value::Str(p.to_string()))
                    .collect();
                Ok(Value::List(parts))
            }
            _ => Err(EvalError::TypeMismatch {
                op: "split".into(),
                left: "string, string".into(),
                right: "".into(),
            }),
        },
        "key_pressed" => match evaluated.as_slice() {
            [Value::Str(key)] => {
                let input_key = vars.get("__input_key");
                Ok(Value::Bool(input_key == Some(&Value::Str(key.clone()))))
            }
            _ => Err(EvalError::TypeMismatch {
                op: "key_pressed".into(),
                left: "string".into(),
                right: "".into(),
            }),
        },
        "mouse_clicked" => {
            let clicked = vars
                .get("__input_mouse_clicked")
                .and_then(|v| {
                    if let Value::Bool(b) = v {
                        Some(*b)
                    } else {
                        None
                    }
                })
                .unwrap_or(false);
            Ok(Value::Bool(clicked))
        }
        "mouse_x" => {
            let x = vars
                .get("__input_mouse_x")
                .and_then(|v| {
                    if let Value::Float(f) = v {
                        Some(*f)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);
            Ok(Value::Float(x))
        }
        "mouse_y" => {
            let y = vars
                .get("__input_mouse_y")
                .and_then(|v| {
                    if let Value::Float(f) = v {
                        Some(*f)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);
            Ok(Value::Float(y))
        }
        _ => Err(EvalError::UndefinedVar(format!(
            "unknown function `{name}`"
        ))),
    }
}

fn expect_numbers(vals: &[Value], _fn_name: &str) -> EvalResult<Vec<f64>> {
    vals.iter()
        .map(|v| match v {
            Value::Int(n) => Ok(*n as f64),
            Value::Float(f) => Ok(*f as f64),
            _ => Err(EvalError::TypeMismatch {
                op: "numeric".into(),
                left: "number".into(),
                right: "".into(),
            }),
        })
        .collect()
}

fn to_value(f: f64) -> Value {
    if f.fract() == 0.0 && f.is_finite() {
        Value::Int(f as i64)
    } else {
        Value::Float(f as f32)
    }
}

pub fn eval_bool(expr: &Expr, vars: &HashMap<String, Value>) -> EvalResult<bool> {
    match eval_expr(expr, vars)? {
        Value::Bool(b) => Ok(b),
        Value::Int(n) => Ok(n != 0),
        Value::Float(f) => Ok(f != 0.0),
        Value::Str(s) => Ok(!s.is_empty()),
        Value::List(items) => Ok(!items.is_empty()),
    }
}

pub fn eval_interpolated(
    text: &InterpolatedText,
    vars: &HashMap<String, Value>,
) -> EvalResult<String> {
    let mut out = String::new();
    for seg in &text.0 {
        match seg {
            TextSegment::Lit(s) => out.push_str(s),
            TextSegment::Interp(expr) => out.push_str(&eval_expr(expr, vars)?.to_string()),
        }
    }
    Ok(out)
}

// ─── Opérateurs binaires ──────────────────────────────────────────────────────

fn eval_binop(
    op: &BinOpKind,
    left_expr: &Expr,
    right_expr: &Expr,
    vars: &HashMap<String, Value>,
) -> EvalResult<Value> {
    let lv = eval_expr(left_expr, vars)?;
    let rv = eval_expr(right_expr, vars)?;

    match op {
        BinOpKind::Add => match (&lv, &rv) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f32 + b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f32)),
            (Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{a}{b}"))),
            (Value::Str(a), _) => Ok(Value::Str(format!("{a}{rv}"))),
            (_, Value::Str(b)) => Ok(Value::Str(format!("{lv}{b}"))),
            _ => Err(type_err(op, &lv, &rv)),
        },
        BinOpKind::Sub => numeric_binop(op, &lv, &rv, |a, b| a - b, |a, b| a - b),
        BinOpKind::Mul => numeric_binop(op, &lv, &rv, |a, b| a * b, |a, b| a * b),
        BinOpKind::Div => match (&lv, &rv) {
            (_, Value::Int(0)) => Err(EvalError::DivisionByZero),
            (_, Value::Float(f)) if *f == 0.0 => Err(EvalError::DivisionByZero),
            _ => numeric_binop(op, &lv, &rv, |a, b| a / b, |a, b| a / b),
        },
        BinOpKind::Eq => Ok(Value::Bool(values_eq(&lv, &rv))),
        BinOpKind::Ne => Ok(Value::Bool(!values_eq(&lv, &rv))),
        BinOpKind::Lt => cmp_values(op, &lv, &rv, |o| o.is_lt()),
        BinOpKind::Le => cmp_values(op, &lv, &rv, |o| o.is_le()),
        BinOpKind::Gt => cmp_values(op, &lv, &rv, |o| o.is_gt()),
        BinOpKind::Ge => cmp_values(op, &lv, &rv, |o| o.is_ge()),
    }
}

fn numeric_binop(
    op: &BinOpKind,
    lv: &Value,
    rv: &Value,
    ii: impl Fn(i64, i64) -> i64,
    ff: impl Fn(f32, f32) -> f32,
) -> EvalResult<Value> {
    match (lv, rv) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(ii(*a, *b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(ff(*a, *b))),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(ff(*a as f32, *b))),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(ff(*a, *b as f32))),
        _ => Err(type_err(op, lv, rv)),
    }
}

fn values_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x), Value::Float(y)) => (*x as f32) == *y,
        (Value::Float(x), Value::Int(y)) => *x == (*y as f32),
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::List(a), Value::List(b)) => a == b,
        _ => false,
    }
}

fn cmp_values(
    op: &BinOpKind,
    lv: &Value,
    rv: &Value,
    check: impl Fn(std::cmp::Ordering) -> bool,
) -> EvalResult<Value> {
    let ord = match (lv, rv) {
        (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Int(a), Value::Float(b)) => (*a as f32).partial_cmp(b),
        (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f32)),
        (Value::Str(a), Value::Str(b)) => Some(a.as_str().cmp(b.as_str())),
        _ => return Err(type_err(op, lv, rv)),
    };
    Ok(Value::Bool(ord.map(check).unwrap_or(false)))
}

fn type_err(op: &BinOpKind, lv: &Value, rv: &Value) -> EvalError {
    EvalError::TypeMismatch {
        op: op.to_string(),
        left: type_name(lv).into(),
        right: type_name(rv).into(),
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Str(_) => "string",
        Value::List(_) => "list",
    }
}

// ─── TESTS ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rvn_parser::{BinOpKind, Expr, InterpolatedText, TextSegment};

    fn vars(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn test_int_literal() {
        assert_eq!(eval_expr(&Expr::Int(42), &vars(&[])), Ok(Value::Int(42)));
    }
    #[test]
    fn test_bool_literal() {
        assert_eq!(
            eval_expr(&Expr::Bool(true), &vars(&[])),
            Ok(Value::Bool(true))
        );
    }
    #[test]
    fn test_str_literal() {
        assert_eq!(
            eval_expr(&Expr::Str("hi".into()), &vars(&[])),
            Ok(Value::Str("hi".into()))
        );
    }

    #[test]
    fn test_text_to_integer_conversion() {
        let expression = Expr::Call {
            name: "to_int".into(),
            args: vec![Expr::Str(" 15 ".into())],
        };
        assert_eq!(eval_expr(&expression, &vars(&[])), Ok(Value::Int(15)));
        let invalid = Expr::Call {
            name: "to_int".into(),
            args: vec![Expr::Str("pas un nombre".into())],
        };
        assert_eq!(eval_expr(&invalid, &vars(&[])), Ok(Value::Int(0)));
    }

    #[test]
    fn test_var_lookup() {
        let v = vars(&[("score", Value::Int(10))]);
        assert_eq!(
            eval_expr(&Expr::Var("score".into()), &v),
            Ok(Value::Int(10))
        );
    }

    #[test]
    fn test_var_undefined_error() {
        let err = eval_expr(&Expr::Var("x".into()), &vars(&[])).unwrap_err();
        assert!(matches!(&err, EvalError::UndefinedVar(n) if n == "x"));
        assert!(err.to_string().contains("aide"));
    }

    #[test]
    fn test_add_int() {
        let e = Expr::BinOp {
            op: BinOpKind::Add,
            left: Box::new(Expr::Int(3)),
            right: Box::new(Expr::Int(4)),
        };
        assert_eq!(eval_expr(&e, &vars(&[])), Ok(Value::Int(7)));
    }

    #[test]
    fn test_division_by_zero() {
        let e = Expr::BinOp {
            op: BinOpKind::Div,
            left: Box::new(Expr::Int(1)),
            right: Box::new(Expr::Int(0)),
        };
        assert_eq!(eval_expr(&e, &vars(&[])), Err(EvalError::DivisionByZero));
    }

    #[test]
    fn test_and_short_circuit() {
        let e = Expr::And(Box::new(Expr::Bool(false)), Box::new(Expr::Var("x".into())));
        assert_eq!(eval_expr(&e, &vars(&[])), Ok(Value::Bool(false)));
    }

    #[test]
    fn test_interpolation_simple() {
        let text = InterpolatedText(vec![
            TextSegment::Lit("Bonjour ".into()),
            TextSegment::Interp(Expr::Var("prenom".into())),
            TextSegment::Lit(" !".into()),
        ]);
        let v = vars(&[("prenom", Value::Str("Sarah".into()))]);
        assert_eq!(eval_interpolated(&text, &v), Ok("Bonjour Sarah !".into()));
    }

    #[test]
    fn test_interpolation_undefined_var_error() {
        let text = InterpolatedText(vec![TextSegment::Interp(Expr::Var("inconnu".into()))]);
        let err = eval_interpolated(&text, &vars(&[])).unwrap_err();
        assert!(matches!(err, EvalError::UndefinedVar(_)));
    }
}
