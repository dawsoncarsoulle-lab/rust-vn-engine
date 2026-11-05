// ─── LOCALISATION ────────────────────────────────────────────────────────────

/// Position ligne/colonne + longueur du token dans le source (1-indexé).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceLocation {
    pub line: usize,
    pub col: usize,
    /// Longueur en caractères du token — utilisée pour dessiner `^^^^`.
    /// 0 = inconnu (EOF ou erreur synthétique).
    pub len: usize,
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ligne {}, col {}", self.line, self.col)
    }
}

pub fn byte_offset_to_location(source: &str, offset: usize, len_bytes: usize) -> SourceLocation {
    let clamped = offset.min(source.len());
    let prefix = &source[..clamped];
    let line = prefix.chars().filter(|&c| c == '\n').count() + 1;
    let col = if let Some(last_nl) = prefix.rfind('\n') {
        source[last_nl + 1..clamped].chars().count() + 1
    } else {
        prefix.chars().count() + 1
    };
    let len = source
        .get(offset..offset + len_bytes)
        .map(|s| s.chars().count())
        .unwrap_or(len_bytes)
        .max(1);
    SourceLocation { line, col, len }
}

/// Single lexing pass only. Logos emits ordered UTF-8 boundary spans; keep
/// line/scalar-column state across their starts instead of rescanning the
/// complete source prefix for every successful token. No global/source cache.
pub(crate) struct SourceLocationCursor<'a>{
    source:&'a str,offset:usize,line:usize,col:usize,
}
impl<'a> SourceLocationCursor<'a>{
    pub(crate) fn new(source:&'a str)->Self{Self{source,offset:0,line:1,col:1}}
    pub(crate) fn location(&mut self,offset:usize,len_bytes:usize)->SourceLocation{
        let clamped=offset.min(self.source.len());
        assert!(clamped>=self.offset,"lexer source spans must be monotone");
        let delta=self.source.get(self.offset..clamped).expect("lexer source spans must use UTF-8 boundaries");
        for scalar in delta.chars(){
            if scalar=='\n'{self.line+=1;self.col=1;}else{self.col+=1;}
        }
        self.offset=clamped;
        let len=offset.checked_add(len_bytes).and_then(|end|self.source.get(offset..end))
            .map(|token|token.chars().count()).unwrap_or(len_bytes).max(1);
        SourceLocation{line:self.line,col:self.col,len}
    }
}

// ─── ERREURS ─────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub enum ParseErrorKind {
    UnexpectedToken { got: String, expected: &'static str },
    UnexpectedEof { expected: &'static str },
    LexError { slice: String },
    InvalidAssignment { ident: String, suggestion: String },
}

/// Erreur de parsing avec contexte source.
///
/// Format d'affichage :
/// ```text
/// ligne 67, col 4 : token inattendu `Ident("suis")`, attendu : `:`, `.` ou une string après l'identifiant
///    |
/// 67 |     je suis ici
///    |        ^^^^
/// ```
#[derive(Debug, PartialEq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub location: SourceLocation,
    /// Texte brut de la ligne source (sans `\n` terminal).
    pub source_line: String,
}

impl std::error::Error for ParseError {}

impl ParseError {
    pub fn build(kind: ParseErrorKind, location: SourceLocation, source: &str) -> Self {
        let source_line = source
            .lines()
            .nth(location.line.saturating_sub(1))
            .unwrap_or("")
            .to_string();
        Self {
            kind,
            location,
            source_line,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // ── Titre ─────────────────────────────────────────────────────────────
        let msg = match &self.kind {
            ParseErrorKind::UnexpectedToken { got, expected } => {
                format!("token inattendu `{}`, attendu : {}", got, expected)
            }
            ParseErrorKind::UnexpectedEof { expected } => {
                format!("fin de fichier inattendue, attendu : {}", expected)
            }
            ParseErrorKind::LexError { slice } => format!("caractère non reconnu : `{}`", slice),
            ParseErrorKind::InvalidAssignment { .. } => "affectation invalide".to_string(),
        };
        writeln!(f, "{} : {}", self.location, msg)?;

        // ── Contexte source ───────────────────────────────────────────────────
        if self.source_line.is_empty() {
            return Ok(());
        }
        let line_no = self.location.line.to_string();
        let gutter = line_no.len();
        let pad = " ".repeat(gutter);

        writeln!(f, "{pad} |")?;
        writeln!(f, "{line_no} | {}", self.source_line)?;

        let col0 = self.location.col.saturating_sub(1);
        let carets = "^".repeat(self.location.len.max(1));
        write!(f, "{pad} | {}{}", " ".repeat(col0), carets)?;
        if let ParseErrorKind::InvalidAssignment { suggestion, .. } = &self.kind {
            write!(
                f,
                "\n{pad} |\n{pad} = en RVN, les variables s'assignent avec `set`\n{pad} = suggestion: `{suggestion}`"
            )?;
        }
        Ok(())
    }
}

pub type ParseResult<T> = Result<T, ParseError>;
