/// Erreur survenue pendant l'exécution du moteur.
/// Porte le contexte nécessaire pour produire un message utile à l'auteur.
#[derive(Debug, PartialEq)]
pub struct RuntimeError {
    pub kind: RuntimeErrorKind,
    /// Index du statement fautif dans le script aplati (pc).
    pub pc: usize,
    /// Représentation Debug du statement fautif (pour le contexte).
    pub stmt_debug: String,
}

impl RuntimeError {
    pub fn new(kind: RuntimeErrorKind, pc: usize, stmt_debug: impl Into<String>) -> Self {
        Self {
            kind,
            pc,
            stmt_debug: stmt_debug.into(),
        }
    }

    /// Construit sans contexte de statement (erreurs de navigation : jump, return…).
    pub fn no_stmt(kind: RuntimeErrorKind, pc: usize) -> Self {
        Self {
            kind,
            pc,
            stmt_debug: String::new(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum RuntimeErrorKind {
    UndefinedLabel(String),
    ReturnWithoutCall,
    SpriteNotVisible(String),
    /// Erreur d'évaluation d'expression.
    EvalError(EvalErrorKind),
}

/// Détail d'une erreur d'évaluation, pour l'affichage à l'auteur.
#[derive(Debug, PartialEq, Clone)]
pub enum EvalErrorKind {
    UndefinedVar(String),
    TypeMismatch {
        op: String,
        left: String,
        right: String,
    },
    DivisionByZero,
}

impl std::fmt::Display for EvalErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UndefinedVar(n) => write!(
                f,
                "variable non définie : `{n}`\n  aide : utilise `set {n} = <valeur>` avant de l'utiliser"
            ),
            Self::TypeMismatch { op, left, right } => {
                write!(f, "types incompatibles pour `{op}` : {left} et {right}")
            }
            Self::DivisionByZero => write!(f, "division par zéro"),
        }
    }
}

impl std::fmt::Display for RuntimeErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UndefinedLabel(l) => write!(
                f,
                "label inconnu : `{l}`\n  aide : vérifie qu'un `label {l}` existe dans le script"
            ),
            Self::ReturnWithoutCall => write!(f, "return sans call correspondant"),
            Self::SpriteNotVisible(id) => write!(
                f,
                "sprite `{id}` non visible\n  aide : appelle `{id}.show()` avant de le cacher ou déplacer"
            ),
            Self::EvalError(e) => write!(f, "{e}"),
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

// ─── SCRIPT ERROR ─────────────────────────────────────────────────────────────
//
// Type de rapport final, produit par le moteur et affiché à l'auteur.
// Formatage style rustc :
//
//   erreur d'exécution au statement #42
//    |
//    | Dialogue { character_id: Some("sarah"), text: "Bonjour [prenom] !" }
//    |
//    = variable non définie : `prenom`
//      aide : utilise `set prenom = <valeur>` avant de l'utiliser

#[derive(Debug, Clone)]
pub struct ScriptError {
    pub pc: usize,
    pub stmt_debug: String,
    pub message: String,
}

impl ScriptError {
    pub fn from_runtime(e: &RuntimeError) -> Self {
        Self {
            pc: e.pc,
            stmt_debug: e.stmt_debug.clone(),
            message: e.kind.to_string(),
        }
    }
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "erreur d'exécution au statement #{}", self.pc)?;
        if !self.stmt_debug.is_empty() {
            writeln!(f, " |")?;
            // Tronque les statements très longs pour lisibilité
            let preview = if self.stmt_debug.len() > 120 {
                format!("{}…", &self.stmt_debug[..120])
            } else {
                self.stmt_debug.clone()
            };
            writeln!(f, " | {}", preview)?;
            writeln!(f, " |")?;
        }
        for (i, line) in self.message.lines().enumerate() {
            if i == 0 {
                writeln!(f, " = {}", line)?;
            } else {
                writeln!(f, "   {}", line)?;
            }
        }
        Ok(())
    }
}
