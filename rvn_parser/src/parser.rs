// rvn_parser/src/parser.rs
//
// Parser à descente récursive manuelle.
//
// CHANGEMENTS vs version précédente :
//   - parse_expr() : évaluateur d'expressions complet (A+B+C)
//   - parse_condition() remplacé par parse_expr()
//   - parse_value() remplacé par parse_expr() pour SetVar
//   - parse_dialogue_text_only() et strings de dialogue →  parse_interpolated_str()
//   - Nouveaux tokens : And, Or, Plus, Minus, Star, Slash, BracketOpen/Close

use logos::Logos;

use crate::ast::*;
use crate::error::*;
use crate::expr::*;
use crate::lexer::Token;

pub struct Parser<'a> {
    source: &'a str,
    tokens: Vec<(Token<'a>, SourceLocation)>,
    pos: usize,
    eof_location: SourceLocation,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> ParseResult<Self> {
        let mut tokens = Vec::new();
        for (result, span) in Token::lexer(source).spanned() {
            let loc = byte_offset_to_location(source, span.start, span.len());
            match result {
                Ok(tok) => tokens.push((tok, loc)),
                Err(_) => {
                    return Err(ParseError::build(
                        ParseErrorKind::LexError {
                            slice: source[span].to_string(),
                        },
                        loc,
                        source,
                    ));
                }
            }
        }
        let eof_location = byte_offset_to_location(source, source.len(), 1);
        Ok(Self {
            source,
            tokens,
            pos: 0,
            eof_location,
        })
    }

    // ── Helpers d'erreur ─────────────────────────────────────────────────────

    fn err_msg(&self, loc: SourceLocation, got: String, expected: &'static str) -> ParseError {
        ParseError::build(
            ParseErrorKind::UnexpectedToken { got, expected },
            loc,
            self.source,
        )
    }

    fn err_token(
        &self,
        loc: SourceLocation,
        tok: &Token<'a>,
        expected: &'static str,
    ) -> ParseError {
        self.err_msg(loc, format!("{tok:?}"), expected)
    }

    fn err_eof(&self, loc: SourceLocation, expected: &'static str) -> ParseError {
        ParseError::build(ParseErrorKind::UnexpectedEof { expected }, loc, self.source)
    }

    // ── Navigation ───────────────────────────────────────────────────────────

    fn peek(&self) -> Option<&Token<'a>> {
        let mut i = self.pos;
        loop {
            match self.tokens.get(i) {
                Some((Token::Newline, _)) => i += 1,
                Some((tok, _)) => return Some(tok),
                None => return None,
            }
        }
    }

    fn peek_raw(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn current_location(&self) -> SourceLocation {
        let mut i = self.pos;
        loop {
            match self.tokens.get(i) {
                Some((Token::Newline, _)) => i += 1,
                Some((_, loc)) => return *loc,
                None => return self.eof_location,
            }
        }
    }

    fn advance(&mut self) -> Option<&Token<'a>> {
        self.skip_newlines();
        let tok = self.tokens.get(self.pos).map(|(t, _)| t);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn skip_newlines(&mut self) {
        while matches!(self.tokens.get(self.pos), Some((Token::Newline, _))) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, expected: &'static str) -> ParseResult<Token<'a>> {
        // Consume the next non-newline token and ensure it matches the given expectation
        // when the expected string corresponds to a specific punctuation token.  For
        // contextual expectations such as "string pour name" this simply returns the
        // next token without additional checks.  If the end of file is reached, an EOF
        // error is returned with the expected description.
        let loc = self.current_location();
        let tok = match self.advance().cloned() {
            Some(tok) => tok,
            None => return Err(self.err_eof(loc, expected)),
        };
        // Only perform a token kind check when the expected string represents a
        // punctuation token.  Otherwise, callers handle the returned token and
        // implement their own checks.
        let matches = match expected {
            "{" => matches!(tok, Token::BraceOpen),
            "}" => matches!(tok, Token::BraceClose),
            "(" => matches!(tok, Token::ParenOpen),
            ")" => matches!(tok, Token::ParenClose),
            "[" => matches!(tok, Token::BracketOpen),
            "]" => matches!(tok, Token::BracketClose),
            ":" => matches!(tok, Token::Colon),
            "," => matches!(tok, Token::Comma),
            "." => matches!(tok, Token::Dot),
            "=" => matches!(tok, Token::Assign),
            "=>" => matches!(tok, Token::Arrow),
            // For descriptive expectations we skip checking here
            _ => true,
        };
        if !matches {
            return Err(self.err_token(loc, &tok, expected));
        }
        Ok(tok)
    }

    fn unwrap_string(tok: &Token<'a>) -> &'a str {
        if let Token::String(s) = tok {
            &s[1..s.len() - 1]
        } else {
            panic!("unwrap_string sur non-String")
        }
    }

    fn expect_ident(&mut self, ctx: &'static str) -> ParseResult<String> {
        let loc = self.current_location();
        match self.advance().cloned() {
            Some(Token::Ident(s)) => Ok(s.to_string()),
            Some(tok) => Err(self.err_token(loc, &tok, ctx)),
            None => Err(self.err_eof(loc, ctx)),
        }
    }

    fn expect_i32(&mut self, ctx: &'static str) -> ParseResult<i32> {
        // Accept an optional leading minus sign followed by an Int token.  This allows
        // negative coordinate values to be parsed in rect definitions.  If a minus
        // appears without an integer following it, an error is raised.
        let loc = self.current_location();
        let tok = match self.advance().cloned() {
            Some(tok) => tok,
            None => return Err(self.err_eof(loc, ctx)),
        };
        match tok {
            Token::Int(n) => Ok(n as i32),
            Token::Minus => {
                // Expect an integer after the minus sign
                let next_tok = match self.advance().cloned() {
                    Some(tok) => tok,
                    None => return Err(self.err_eof(loc, ctx)),
                };
                if let Token::Int(n) = next_tok {
                    Ok(-(n as i32))
                } else {
                    Err(self.err_token(loc, &next_tok, ctx))
                }
            }
            other => Err(self.err_token(loc, &other, ctx)),
        }
    }

    // ── EXPRESSIONS ──────────────────────────────────────────────────────────
    //
    // Grammaire (précédence croissante) :
    //   expr     = or_expr
    //   or_expr  = and_expr  ( "or"  and_expr  )*
    //   and_expr = not_expr  ( "and" not_expr  )*
    //   not_expr = "not" not_expr | cmp_expr
    //   cmp_expr = add_expr  [ CMP_OP add_expr ]
    //   add_expr = mul_expr  ( ("+" | "-") mul_expr )*
    //   mul_expr = unary     ( ("*" | "/") unary    )*
    //   unary    = "-" unary | atom
    //   atom     = INT | FLOAT | STRING | BOOL | IDENT | "(" expr ")"

    fn parse_expr(&mut self) -> ParseResult<Expr> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_and_expr()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.advance();
            let right = self.parse_and_expr()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_not_expr()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.advance();
            let right = self.parse_not_expr()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not_expr(&mut self) -> ParseResult<Expr> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.advance();
            let inner = self.parse_not_expr()?;
            return Ok(Expr::Not(Box::new(inner)));
        }
        self.parse_cmp_expr()
    }

    fn parse_cmp_expr(&mut self) -> ParseResult<Expr> {
        let left = self.parse_add_expr()?;
        let op = match self.peek() {
            Some(Token::Eq) => BinOpKind::Eq,
            Some(Token::Ne) => BinOpKind::Ne,
            Some(Token::Lt) => BinOpKind::Lt,
            Some(Token::Le) => BinOpKind::Le,
            Some(Token::Gt) => BinOpKind::Gt,
            Some(Token::Ge) => BinOpKind::Ge,
            _ => return Ok(left),
        };
        self.advance();
        let right = self.parse_add_expr()?;
        Ok(Expr::BinOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    fn parse_add_expr(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_mul_expr()?;
        loop {
            let op = match self.peek_raw() {
                Some(Token::Plus) => BinOpKind::Add,
                Some(Token::Minus) => BinOpKind::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_mul_expr()?;
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_mul_expr(&mut self) -> ParseResult<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek_raw() {
                Some(Token::Star) => BinOpKind::Mul,
                Some(Token::Slash) => BinOpKind::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> ParseResult<Expr> {
        if matches!(self.peek_raw(), Some(Token::Minus)) {
            self.advance();
            let inner = self.parse_unary()?;
            return Ok(Expr::Neg(Box::new(inner)));
        }
        self.parse_atom()
    }

    fn parse_atom(&mut self) -> ParseResult<Expr> {
        let loc = self.current_location();
        match self.advance().cloned() {
            Some(Token::Int(n)) => Ok(Expr::Int(n)),
            Some(Token::Float(f)) => Ok(Expr::Float(f)),
            Some(Token::True) => Ok(Expr::Bool(true)),
            Some(Token::False) => Ok(Expr::Bool(false)),
            Some(Token::String(s)) => Ok(Expr::Str(s[1..s.len() - 1].to_string())),
            Some(Token::Ident(s)) => Ok(Expr::Var(s.to_string())),
            Some(Token::ParenOpen) => {
                let inner = self.parse_expr()?;
                let loc2 = self.current_location();
                match self.advance().cloned() {
                    Some(Token::ParenClose) => Ok(inner),
                    Some(tok) => Err(self.err_token(loc2, &tok, "`)` pour fermer l'expression")),
                    None => Err(self.err_eof(loc2, "`)`")),
                }
            }
            Some(tok) => Err(self.err_token(loc, &tok, "valeur, variable ou `(expr)`")),
            None => Err(self.err_eof(loc, "expression")),
        }
    }

    // ── INTERPOLATION ─────────────────────────────────────────────────────────
    //
    // Parse "Bonjour [prenom], tu as [score + 1] points."
    // en [Lit("Bonjour "), Interp(Var("prenom")), Lit(", tu as "), Interp(Add...), Lit(" points.")]
    //
    // Les crochets [ ] délimitent une sous-expression.
    // Pour inclure un crochet littéral, doubler : [[ → [, ]] → ]

    pub fn parse_interpolated_str(raw: &str) -> ParseResult<InterpolatedText> {
        let mut segments = Vec::new();
        let mut lit = String::new();
        let mut chars = raw.char_indices().peekable();

        while let Some((_i, c)) = chars.next() {
            match c {
                '[' => {
                    // Échappement [[ → [
                    if chars.peek().map(|(_, c)| *c) == Some('[') {
                        chars.next();
                        lit.push('[');
                        continue;
                    }
                    // Fin du segment littéral courant
                    if !lit.is_empty() {
                        segments.push(TextSegment::Lit(std::mem::take(&mut lit)));
                    }
                    // Collecte tout jusqu'au ] correspondant
                    let mut expr_src = String::new();
                    let mut depth = 1usize;
                    for (_, ec) in chars.by_ref() {
                        match ec {
                            '[' => {
                                depth += 1;
                                expr_src.push(ec);
                            }
                            ']' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                                expr_src.push(ec);
                            }
                            _ => expr_src.push(ec),
                        }
                    }
                    if depth != 0 {
                        // Crée une erreur synthétique : pas de SourceLocation précise ici
                        // car on opère sur la string extraite, pas sur le source complet.
                        // On renvoie un message clair.
                        let loc = SourceLocation {
                            line: 0,
                            col: 0,
                            len: 1,
                        };
                        return Err(ParseError::build(
                            ParseErrorKind::UnexpectedEof {
                                expected: "`]` pour fermer l'interpolation",
                            },
                            loc,
                            raw,
                        ));
                    }
                    // Parse l'expression interne
                    let mut sub_parser = Parser::new(&expr_src)?;
                    let expr = sub_parser.parse_expr()?;
                    segments.push(TextSegment::Interp(expr));
                }
                ']' => {
                    // Échappement ]] → ]
                    if chars.peek().map(|(_, c)| *c) == Some(']') {
                        chars.next();
                        lit.push(']');
                    } else {
                        lit.push(']');
                    }
                }
                _ => lit.push(c),
            }
        }

        if !lit.is_empty() {
            segments.push(TextSegment::Lit(lit));
        }

        Ok(InterpolatedText(segments))
    }

    // ── Point d'entrée ───────────────────────────────────────────────────────

    pub fn parse_script(&mut self) -> ParseResult<Script> {
        let mut stmts = Vec::new();
        while self.peek().is_some() {
            stmts.push(self.parse_statement()?);
        }
        Ok(stmts)
    }

    // ── Dispatch ─────────────────────────────────────────────────────────────

    fn parse_statement(&mut self) -> ParseResult<Statement> {
        let loc = self.current_location();
        match self.peek().cloned() {
            Some(Token::Use) => self.parse_use(),
            Some(Token::Init) => self.parse_init(),
            Some(Token::Choice) => self.parse_choice(),
            Some(Token::Set) => self.parse_set(),
            Some(Token::If) => self.parse_if(),
            Some(Token::Label) => self.parse_label(),
            Some(Token::Jump) => self.parse_jump(),
            Some(Token::Call) => self.parse_call(),
            Some(Token::Return) => {
                self.advance();
                Ok(Statement::Return)
            }
            Some(Token::Scene) => self.parse_scene(),
            Some(Token::Imagemap) => self.parse_imagemap(),
            Some(Token::Typewriter) => self.parse_typewriter(),
            Some(Token::String(_)) => self.parse_dialogue_text_only(),
            Some(Token::Ident(_)) => self.parse_ident_statement(),
            Some(tok) => Err(self.err_token(loc, &tok, "début d'une instruction")),
            None => Err(self.err_eof(loc, "début d'une instruction")),
        }
    }

    // ── `use "file.rvn"` / `use { "a.rvn", "b.rvn" }` ─────────────────────

    fn parse_use(&mut self) -> ParseResult<Statement> {
        self.advance();
        let loc = self.current_location();
        match self.peek().cloned() {
            Some(Token::String(_)) => {
                let tok = self.expect("chemin de fichier après `use`")?;
                Ok(Statement::Use {
                    paths: vec![Self::unwrap_string(&tok).to_string()],
                })
            }
            Some(Token::BraceOpen) => {
                self.advance();
                let mut paths = Vec::new();
                loop {
                    let item_loc = self.current_location();
                    match self.peek().cloned() {
                        Some(Token::BraceClose) => {
                            self.advance();
                            break;
                        }
                        Some(Token::String(_)) => {
                            let tok = self.expect("chemin de fichier dans `use { ... }`")?;
                            paths.push(Self::unwrap_string(&tok).to_string());
                            match self.peek().cloned() {
                                Some(Token::Comma) => {
                                    self.advance();
                                }
                                Some(Token::BraceClose) => {}
                                Some(tok) => {
                                    return Err(self.err_token(
                                        item_loc,
                                        &tok,
                                        "`,` ou `}` après un chemin de fichier",
                                    ));
                                }
                                None => return Err(self.err_eof(item_loc, "}` pour fermer `use`")),
                            }
                        }
                        Some(tok) => {
                            return Err(self.err_token(
                                item_loc,
                                &tok,
                                "string de chemin ou `}` dans `use { ... }`",
                            ));
                        }
                        None => return Err(self.err_eof(item_loc, "}` pour fermer `use`")),
                    }
                }
                Ok(Statement::Use { paths })
            }
            Some(tok) => Err(self.err_token(loc, &tok, "chemin de fichier après `use`")),
            None => Err(self.err_eof(loc, "chemin de fichier après `use`")),
        }
    }

    // ── `init { … }` ─────────────────────────────────────────────────────────

    fn parse_init(&mut self) -> ParseResult<Statement> {
        self.advance();
        self.expect("{")?;
        let body = self.parse_block()?;
        Ok(Statement::Init { body })
    }

    // ── `typewriter` ─────────────────────────────────────────────────────────

    fn parse_typewriter(&mut self) -> ParseResult<Statement> {
        self.advance();
        let loc = self.current_location();
        match self.peek().cloned() {
            Some(Token::Assign) => {
                self.advance();
                let loc2 = self.current_location();
                match self.advance().cloned() {
                    Some(Token::True) => Ok(Statement::TypewriterSet { enabled: true }),
                    Some(Token::False) => Ok(Statement::TypewriterSet { enabled: false }),
                    Some(tok) => {
                        Err(self.err_token(loc2, &tok, "true ou false après `typewriter =`"))
                    }
                    None => Err(self.err_eof(loc2, "true ou false après `typewriter =`")),
                }
            }
            Some(Token::Dot) => {
                self.advance();
                let loc2 = self.current_location();
                match self.advance().cloned() {
                    Some(Token::Ident(m)) if m == "speed" => {
                        self.expect("(")?;
                        let loc3 = self.current_location();
                        let chars_per_sec = match self.advance().cloned() {
                            Some(Token::Int(n)) if n >= 0 => n as u32,
                            Some(tok) => {
                                return Err(self.err_token(
                                    loc3,
                                    &tok,
                                    "entier >= 0 pour typewriter.speed(n)",
                                ));
                            }
                            None => {
                                return Err(self.err_eof(loc3, "vitesse dans typewriter.speed(n)"));
                            }
                        };
                        self.expect(")")?;
                        Ok(Statement::TypewriterSpeed { chars_per_sec })
                    }
                    Some(Token::Ident(m)) => {
                        Err(self.err_msg(loc2, format!("typewriter.{m}"), "typewriter.speed(n)"))
                    }
                    Some(tok) => Err(self.err_token(loc2, &tok, "méthode après `typewriter.`")),
                    None => Err(self.err_eof(loc2, "méthode après `typewriter.`")),
                }
            }
            Some(tok) => {
                Err(self.err_token(loc, &tok, "`= bool` ou `.speed(n)` après `typewriter`"))
            }
            None => Err(self.err_eof(loc, "`= bool` ou `.speed(n)` après `typewriter`")),
        }
    }

    // ── `imagemap { … }` ─────────────────────────────────────────────────────

    fn parse_imagemap(&mut self) -> ParseResult<Statement> {
        self.advance();
        self.expect("{")?;
        let mut background: Option<String> = None;
        let mut hover_image: Option<String> = None;
        let mut hotspots: Vec<Hotspot> = Vec::new();

        loop {
            let loc = self.current_location();
            match self.peek().cloned() {
                Some(Token::BraceClose) => {
                    self.advance();
                    break;
                }
                Some(Token::Hotspot) => hotspots.push(self.parse_hotspot()?),
                Some(Token::Ident(_)) => {
                    let key = self.expect_ident("clé de propriété imagemap")?;
                    self.expect(":")?;
                    let val_tok = self.expect("valeur string")?.clone();
                    let val = Self::unwrap_string(&val_tok).to_string();
                    match key.as_str() {
                        "background" => background = Some(val),
                        "hover" => hover_image = Some(val),
                        other => {
                            return Err(self.err_msg(
                                loc,
                                format!("propriété inconnue `{other}`"),
                                "background ou hover",
                            ));
                        }
                    }
                }
                Some(tok) => {
                    return Err(self.err_token(
                        loc,
                        &tok,
                        "hotspot, background ou } dans imagemap",
                    ));
                }
                None => return Err(self.err_eof(loc, "} pour fermer imagemap")),
            }
        }

        let loc = self.current_location();
        let background = background
            .ok_or_else(|| self.err_eof(loc, "propriété `background` obligatoire dans imagemap"))?;

        Ok(Statement::Imagemap {
            background,
            hover_image,
            hotspots,
        })
    }

    fn parse_hotspot(&mut self) -> ParseResult<Hotspot> {
        self.advance();
        self.expect("{")?;
        let mut name: Option<String> = None;
        let mut area: Option<Rect> = None;
        let mut hover_area: Option<Rect> = None;

        loop {
            let loc = self.current_location();
            match self.peek().cloned() {
                Some(Token::BraceClose) => {
                    self.advance();
                    break;
                }
                Some(Token::Ident(_)) => {
                    let key = self.expect_ident("clé de propriété hotspot")?;
                    self.expect(":")?;
                    match key.as_str() {
                        "name" => {
                            let tok = self.expect("string pour name")?.clone();
                            name = Some(Self::unwrap_string(&tok).to_string());
                        }
                        "area" => area = Some(self.parse_rect()?),
                        "hover_area" => hover_area = Some(self.parse_rect()?),
                        other => {
                            return Err(self.err_msg(
                                loc,
                                format!("propriété inconnue `{other}`"),
                                "name, area ou hover_area",
                            ));
                        }
                    }
                }
                Some(tok) => return Err(self.err_token(loc, &tok, "propriété ou } dans hotspot")),
                None => return Err(self.err_eof(loc, "} pour fermer hotspot")),
            }
        }

        let loc = self.current_location();
        let area =
            area.ok_or_else(|| self.err_eof(loc, "propriété `area` obligatoire dans hotspot"))?;
        self.expect("=>")?;
        self.expect("{")?;
        let body = self.parse_block()?;
        Ok(Hotspot {
            name,
            area,
            hover_area,
            body,
        })
    }

    fn parse_rect(&mut self) -> ParseResult<Rect> {
        self.expect("(")?;
        let x1 = self.expect_i32("x1")?;
        self.expect(",")?;
        let y1 = self.expect_i32("y1")?;
        self.expect(",")?;
        let x2 = self.expect_i32("x2")?;
        self.expect(",")?;
        let y2 = self.expect_i32("y2")?;
        let loc = self.current_location();
        self.expect(")")?;
        if x2 <= x1 || y2 <= y1 {
            return Err(self.err_msg(
                loc,
                format!("({x1},{y1},{x2},{y2})"),
                "rect valide : x2 > x1 et y2 > y1",
            ));
        }
        Ok(Rect::new(x1, y1, x2, y2))
    }

    // ── `scene` ───────────────────────────────────────────────────────────────

    fn parse_scene(&mut self) -> ParseResult<Statement> {
        self.advance();
        let loc = self.current_location();
        let background = match self.advance().cloned() {
            Some(Token::Ident(s)) => s.to_string(),
            Some(Token::String(s)) => s[1..s.len() - 1].to_string(),
            Some(tok) => return Err(self.err_token(loc, &tok, "nom ou chemin du fond")),
            None => return Err(self.err_eof(loc, "nom du fond")),
        };
        let transition = self.try_parse_with()?;
        Ok(Statement::Scene {
            background,
            transition,
        })
    }

    // ── Identifiant ──────────────────────────────────────────────────────────

    fn parse_ident_statement(&mut self) -> ParseResult<Statement> {
        let ident = match self.advance().cloned() {
            Some(Token::Ident(s)) => s.to_string(),
            _ => unreachable!(),
        };
        let loc = self.current_location();
        match self.peek().cloned() {
            Some(Token::Colon) => {
                self.advance();
                let tok = self.expect("valeur de config")?.clone();
                Ok(Statement::Config {
                    key: ident,
                    value: Self::unwrap_string(&tok).to_string(),
                })
            }
            Some(Token::String(raw)) => {
                let raw = raw.to_string();
                self.advance();
                let text = Self::parse_interpolated_str(&raw[1..raw.len() - 1])?;
                Ok(Statement::Dialogue {
                    character_id: Some(ident),
                    text,
                })
            }
            Some(Token::Dot) => self.parse_method_call(ident),
            Some(tok) => {
                Err(self.err_token(loc, &tok, "`:`, `.` ou une string après l'identifiant"))
            }
            None => Err(self.err_eof(loc, "suite de l'instruction")),
        }
    }

    // ── `ident.method(args) …` ───────────────────────────────────────────────

    fn parse_method_call(&mut self, target: String) -> ParseResult<Statement> {
        self.advance();
        let method = self.expect_ident("nom de méthode")?;
        self.expect("(")?;

        if method == "animate" {
            return self.parse_animate_call(target);
        }

        if method == "stop_animation" {
            self.expect(")")?;
            return Ok(Statement::SpriteStopAnimation {
                character_id: target,
            });
        }

        let args = self.parse_optional_args()?;
        self.expect(")")?;

        if target == "character" && method == "create" {
            let mut it = args.into_iter();
            return Ok(Statement::CharacterCreate {
                id: it.next().unwrap_or_default(),
                display_name: it.next().unwrap_or_default(),
            });
        }

        let arg = args.into_iter().next();
        let loc = self.current_location();

        match method.as_str() {
            "show" => {
                let position = self.try_parse_at()?;
                let transition = self.try_parse_with()?;
                Ok(Statement::ShowSprite {
                    character_id: target,
                    emotion: arg,
                    position,
                    transition,
                })
            }
            "hide" => {
                let transition = self.try_parse_with()?;
                Ok(Statement::HideSprite {
                    character_id: target,
                    transition,
                })
            }
            "move" => {
                let position = self.try_parse_at()?.unwrap_or(Position::Center);
                let transition = self.try_parse_with()?;
                Ok(Statement::MoveSprite {
                    character_id: target,
                    position,
                    transition,
                })
            }
            _ if target == "music" => match method.as_str() {
                "play" => {
                    let file = arg.ok_or_else(|| {
                        self.err_msg(loc, "()".into(), "nom de fichier audio dans music.play()")
                    })?;
                    let transition = self.try_parse_with()?;
                    Ok(Statement::MusicPlay { file, transition })
                }
                "stop" => {
                    let transition = self.try_parse_with()?;
                    Ok(Statement::MusicStop { transition })
                }
                "volume" => {
                    let level = match arg.as_deref() {
                        Some(s) => s.parse::<f32>().unwrap_or(1.0).clamp(0.0, 1.0),
                        None => 1.0,
                    };
                    Ok(Statement::MusicVolume { level })
                }
                other => Err(self.err_msg(loc, format!("music.{other}"), "play, stop ou volume")),
            },
            _ if target == "sfx" => match method.as_str() {
                "play" => {
                    let file = arg.ok_or_else(|| {
                        self.err_msg(loc, "()".into(), "nom de fichier audio dans sfx.play()")
                    })?;
                    let transition = self.try_parse_with()?;
                    Ok(Statement::SfxPlay { file, transition })
                }
                "stop" => {
                    let file = arg.ok_or_else(|| {
                        self.err_msg(loc, "()".into(), "nom de fichier dans sfx.stop()")
                    })?;
                    let transition = self.try_parse_with()?;
                    Ok(Statement::SfxStop { file, transition })
                }
                other => Err(self.err_msg(loc, format!("sfx.{other}"), "play ou stop")),
            },
            _ => {
                let transition = self.try_parse_with()?;
                Ok(Statement::MethodCall {
                    target,
                    method,
                    arg,
                    transition,
                })
            }
        }
    }

    fn parse_animate_call(&mut self, target: String) -> ParseResult<Statement> {
        let loc = self.current_location();
        let animation = match self.advance().cloned() {
            Some(Token::String(raw)) => raw[1..raw.len() - 1].to_string(),
            Some(tok) => return Err(self.err_token(loc, &tok, "nom d'animation string")),
            None => return Err(self.err_eof(loc, "nom d'animation")),
        };

        let mut params = Vec::new();
        if matches!(self.peek(), Some(Token::Comma)) {
            self.advance();
        }

        loop {
            match self.peek() {
                Some(Token::ParenClose) => {
                    self.advance();
                    break;
                }
                Some(Token::Ident(_)) => {
                    let name = self.expect_ident("nom de paramètre d'animation")?;
                    self.expect(":")?;
                    let loc = self.current_location();
                    let value = match self.advance().cloned() {
                        Some(Token::True) => AnimationValue::Bool(true),
                        Some(Token::False) => AnimationValue::Bool(false),
                        Some(Token::Int(n)) => AnimationValue::Int(n),
                        Some(Token::Float(f)) => AnimationValue::Float(f),
                        Some(Token::String(raw)) => {
                            AnimationValue::Str(raw[1..raw.len() - 1].to_string())
                        }
                        Some(tok) => {
                            return Err(self.err_token(
                                loc,
                                &tok,
                                "valeur de paramètre animation (bool, nombre ou string)",
                            ));
                        }
                        None => return Err(self.err_eof(loc, "valeur de paramètre animation")),
                    };
                    params.push(AnimationParam { name, value });
                    if matches!(self.peek(), Some(Token::Comma)) {
                        self.advance();
                    }
                }
                Some(tok) => {
                    let loc = self.current_location();
                    return Err(self.err_token(loc, tok, "paramètre nommé ou `)`"));
                }
                None => return Err(self.err_eof(self.current_location(), "`)`")),
            }
        }

        Ok(Statement::SpriteAnimate {
            character_id: target,
            animation,
            params,
        })
    }

    // ── `[at …]` optionnel ───────────────────────────────────────────────────

    fn try_parse_at(&mut self) -> ParseResult<Option<Position>> {
        if matches!(self.peek_raw(), Some(Token::Newline) | None) {
            return Ok(None);
        }
        if !matches!(self.peek(), Some(Token::At)) {
            return Ok(None);
        }
        self.advance();
        let loc = self.current_location();

        if matches!(self.peek_raw(), Some(Token::ParenOpen)) {
            self.advance();
            let loc2 = self.current_location();
            let x = match self.advance().cloned() {
                Some(Token::Float(f)) => f,
                Some(Token::Int(n)) => n as f32,
                Some(tok) => {
                    return Err(self.err_token(loc2, &tok, "position normalisée (ex: 0.35)"));
                }
                None => return Err(self.err_eof(loc2, "position")),
            };
            self.expect(")")?;
            return Ok(Some(Position::Custom(x)));
        }

        match self.advance().cloned() {
            Some(Token::Ident(s)) => {
                let pos = match s {
                    "left" => Position::Left,
                    "center" => Position::Center,
                    "right" => Position::Right,
                    other => {
                        eprintln!("avertissement : position inconnue `{other}`, utilise centre");
                        Position::Center
                    }
                };
                Ok(Some(pos))
            }
            Some(tok) => Err(self.err_token(loc, &tok, "left, center, right ou (x) après `at`")),
            None => Err(self.err_eof(loc, "position après `at`")),
        }
    }

    // ── `[with fade|dissolve[(ms)]]` optionnel ───────────────────────────────

    fn try_parse_with(&mut self) -> ParseResult<Transition> {
        if matches!(self.peek_raw(), Some(Token::Newline) | None) {
            return Ok(Transition::None);
        }
        if !matches!(self.peek(), Some(Token::With)) {
            return Ok(Transition::None);
        }
        self.advance();
        let loc = self.current_location();

        match self.advance().cloned() {
            Some(Token::Fade) => {
                let d = self.try_parse_duration(Transition::DEFAULT_FADE_MS)?;
                Ok(Transition::Fade { duration_ms: d })
            }
            Some(Token::Dissolve) => {
                let d = self.try_parse_duration(Transition::DEFAULT_DISSOLVE_MS)?;
                Ok(Transition::Dissolve { duration_ms: d })
            }
            Some(tok) => Err(self.err_token(loc, &tok, "`fade` ou `dissolve` après `with`")),
            None => Err(self.err_eof(loc, "nom de transition")),
        }
    }

    fn try_parse_duration(&mut self, default: u32) -> ParseResult<u32> {
        if !matches!(self.peek_raw(), Some(Token::ParenOpen)) {
            return Ok(default);
        }
        self.advance();
        let loc = self.current_location();
        let ms = match self.advance().cloned() {
            Some(Token::Int(n)) if n > 0 => n as u32,
            Some(tok) => return Err(self.err_token(loc, &tok, "durée en ms (entier positif)")),
            None => return Err(self.err_eof(loc, "durée")),
        };
        self.expect(")")?;
        Ok(ms)
    }

    // ── Primitives ────────────────────────────────────────────────────────────

    fn parse_label(&mut self) -> ParseResult<Statement> {
        self.advance();
        Ok(Statement::Label {
            name: self.expect_ident("nom de label")?,
        })
    }

    fn parse_jump(&mut self) -> ParseResult<Statement> {
        self.advance();
        Ok(Statement::Jump {
            target: self.expect_ident("nom du label cible")?,
        })
    }

    fn parse_call(&mut self) -> ParseResult<Statement> {
        self.advance();
        Ok(Statement::Call {
            target: self.expect_ident("nom du label à appeler")?,
        })
    }

    /// `set nom = expr`
    fn parse_set(&mut self) -> ParseResult<Statement> {
        self.advance();
        let name = self.expect_ident("nom de variable")?;
        self.expect("=")?;
        let value = self.parse_expr()?;
        Ok(Statement::SetVar { name, value })
    }

    /// `if expr { … } [else { … }]`
    fn parse_if(&mut self) -> ParseResult<Statement> {
        self.advance();
        let condition = self.parse_expr()?;
        self.expect("{")?;
        let then_branch = self.parse_block()?;
        let else_branch = if matches!(self.peek(), Some(Token::Else)) {
            self.advance();
            self.expect("{")?;
            self.parse_block()?
        } else {
            vec![]
        };
        Ok(Statement::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_block(&mut self) -> ParseResult<Vec<Statement>> {
        let mut stmts = Vec::new();
        loop {
            let loc = self.current_location();
            match self.peek().cloned() {
                Some(Token::BraceClose) => {
                    self.advance();
                    break;
                }
                Some(_) => stmts.push(self.parse_statement()?),
                None => return Err(self.err_eof(loc, "} pour fermer le bloc")),
            }
        }
        Ok(stmts)
    }

    fn parse_dialogue_text_only(&mut self) -> ParseResult<Statement> {
        let raw = match self.advance().cloned() {
            Some(Token::String(s)) => s.to_string(),
            _ => unreachable!(),
        };
        let text = Self::parse_interpolated_str(&raw[1..raw.len() - 1])?;
        Ok(Statement::Dialogue {
            character_id: None,
            text,
        })
    }

    fn parse_optional_args(&mut self) -> ParseResult<Vec<String>> {
        let mut args = Vec::new();
        loop {
            match self.peek() {
                Some(Token::String(_)) => {
                    let tok = self.advance().unwrap().clone();
                    args.push(Self::unwrap_string(&tok).to_string());
                    if matches!(self.peek(), Some(Token::Comma)) {
                        self.advance();
                    }
                }
                _ => break,
            }
        }
        Ok(args)
    }

    fn parse_choice(&mut self) -> ParseResult<Statement> {
        self.advance();
        self.expect("{")?;
        let mut options = Vec::new();
        loop {
            let loc = self.current_location();
            match self.peek().cloned() {
                Some(Token::BraceClose) => {
                    self.advance();
                    break;
                }
                Some(Token::String(raw)) => {
                    let raw = raw.to_string();
                    self.advance();
                    let label = Self::parse_interpolated_str(&raw[1..raw.len() - 1])?;
                    self.expect("=>")?;
                    self.expect("{")?;
                    options.push((label, self.parse_block()?));
                }
                Some(tok) => return Err(self.err_token(loc, &tok, "string ou } dans choice")),
                None => return Err(self.err_eof(loc, "} pour fermer choice")),
            }
        }
        Ok(Statement::Choice { options })
    }
}

// ─── API PUBLIQUE ─────────────────────────────────────────────────────────────

pub fn parse(source: &str) -> ParseResult<Script> {
    Parser::new(source)?.parse_script()
}

/// Parse une string interpolée hors contexte script.
/// Utilisé par le moteur pour parser les templates traduits à la volée.
pub fn parse_interpolated_str(raw: &str) -> ParseResult<InterpolatedText> {
    Parser::parse_interpolated_str(raw)
}
