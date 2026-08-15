// rvn_parser/src/expr.rs
//
// Arbre d'expression : arithmétique, comparaison, logique, interpolation.
//
// Grammaire (par précédence croissante) :
//
//   expr       = or_expr
//   or_expr    = and_expr  ( "or"  and_expr  )*
//   and_expr   = not_expr  ( "and" not_expr  )*
//   not_expr   = "not" not_expr | cmp_expr
//   cmp_expr   = add_expr  ( ("==" | "!=" | "<" | "<=" | ">" | ">=") add_expr )?
//   add_expr   = mul_expr  ( ("+" | "-") mul_expr )*
//   mul_expr   = unary     ( ("*" | "/") unary    )*
//   unary      = "-" unary | atom
//   atom       = INT | FLOAT | STRING | BOOL | IDENT | "(" expr ")"
//
// Interpolation dans les textes de dialogue :
//   "Bonjour [prenom], tu as [score + 1] points."
//   Les segments entre crochets sont parsés comme des `expr`.

use serde::{Deserialize, Serialize};

/// Un nœud d'expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    // ── Littéraux ─────────────────────────────────────────────────────────────
    Int(i64),
    Float(f32),
    Bool(bool),
    Str(String),

    // ── Variable ──────────────────────────────────────────────────────────────
    Var(String),

    // ── Arithmétique ──────────────────────────────────────────────────────────
    BinOp {
        op: BinOpKind,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Neg(Box<Expr>),

    // ── Logique ───────────────────────────────────────────────────────────────
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    /// Function call: min(a, b), max(a, b), abs(n), random(lo, hi), etc.
    Call {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinOpKind {
    // Arithmétique
    Add,
    Sub,
    Mul,
    Div,
    // Comparaison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl std::fmt::Display for BinOpKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinOpKind::Add => "+",
            BinOpKind::Sub => "-",
            BinOpKind::Mul => "*",
            BinOpKind::Div => "/",
            BinOpKind::Eq => "==",
            BinOpKind::Ne => "!=",
            BinOpKind::Lt => "<",
            BinOpKind::Le => "<=",
            BinOpKind::Gt => ">",
            BinOpKind::Ge => ">=",
        };
        write!(f, "{s}")
    }
}

/// Un segment d'un texte interpolé.
/// "Bonjour [prenom] !" → [Lit("Bonjour "), Interp(Var("prenom")), Lit(" !")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextSegment {
    /// Texte brut.
    Lit(String),
    /// Expression à évaluer et convertir en string.
    Interp(Expr),
}

/// Texte potentiellement interpolé.
/// Si le vecteur contient un seul `Lit`, c'est un texte plain — pas d'overhead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpolatedText(pub Vec<TextSegment>);

impl InterpolatedText {
    /// Construit un texte plain sans interpolation.
    pub fn plain(s: impl Into<String>) -> Self {
        Self(vec![TextSegment::Lit(s.into())])
    }

    /// Retourne `true` si le texte ne contient aucun segment interpolé.
    pub fn is_plain(&self) -> bool {
        self.0.iter().all(|s| matches!(s, TextSegment::Lit(_)))
    }

    /// Si le texte est plain, retourne la string sous-jacente.
    pub fn as_plain(&self) -> Option<&str> {
        if self.0.len() == 1 {
            if let TextSegment::Lit(s) = &self.0[0] {
                return Some(s);
            }
        }
        None
    }
}

impl std::fmt::Display for InterpolatedText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for seg in &self.0 {
            match seg {
                TextSegment::Lit(s) => write!(f, "{s}")?,
                TextSegment::Interp(e) => write!(f, "[{e:?}]")?,
            }
        }
        Ok(())
    }
}
