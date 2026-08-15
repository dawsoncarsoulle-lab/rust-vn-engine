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
    }
}

fn eval_call(name: &str, args: &[Expr], vars: &HashMap<String, Value>) -> EvalResult<Value> {
    let evaluated: Vec<Value> = args
        .iter()
        .map(|a| eval_expr(a, vars))
        .collect::<Result<_, _>>()?;
    match name {
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
            _ => Err(EvalError::TypeMismatch {
                op: "len".into(),
                left: "string".into(),
                right: "".into(),
            }),
        },
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
