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
#[path="parser_function_libraries1023.rs"]
pub mod function_libraries1023;
#[path="parser_standard_macros_1020.rs"]
mod standard_macros_1020;

use crate::ast::*;
use crate::error::*;
use crate::expr::*;
use crate::lexer::Token;
use crate::program::{NarrativeContext, ParsedSource, StatementSpan};

pub struct Parser<'a> {
    source: &'a str,
    tokens: Vec<(Token<'a>, SourceLocation)>,
    token_ranges: Vec<std::ops::Range<usize>>,
    pos: usize,
    function_depth: usize,
    unit_depth: usize,
    handler_depth: usize,
    block_depth: usize,
    init_depth: usize,
    for_loop_depth: usize,
    narrative_context: NarrativeContext,
    capture_spans: bool,
    span_frames: Vec<Vec<StatementSpan>>,
    root_spans: Vec<StatementSpan>,
    eof_location: SourceLocation,
}

#[derive(Debug, PartialEq)]
pub struct RecoveredScript {
    pub script: Script,
    pub errors: Vec<ParseError>,
}

/// Byte range excludes surrounding whitespace and comments. Offsets always
/// refer to the original UTF-8 source, never normalized or generated code.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedStatement {
    pub statement: Statement,
    pub range: std::ops::Range<usize>,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> ParseResult<Self> {
        let mut tokens = Vec::new();
        let mut token_ranges = Vec::new();
        let mut locations = SourceLocationCursor::new(source);
        for (result, span) in Token::lexer(source).spanned() {
            let loc = locations.location(span.start, span.len());
            match result {
                Ok(tok) => {
                    tokens.push((tok, loc));
                    token_ranges.push(span);
                }
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
        let eof_location = locations.location(source.len(), 1);
        Ok(Self {
            source,
            tokens,
            token_ranges,
            pos: 0,
            function_depth: 0,
            unit_depth: 0,
            handler_depth: 0,
            block_depth: 0,
            init_depth: 0,
            for_loop_depth: 0,
            narrative_context: NarrativeContext::Legacy,
            capture_spans: false,
            span_frames: Vec::new(),
            root_spans: Vec::new(),
            eof_location,
        })
    }

    pub fn new_with_context(source: &'a str, context: NarrativeContext) -> ParseResult<Self> {
        let mut parser = Self::new(source)?;
        parser.narrative_context = context;
        Ok(parser)
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

    fn err_invalid_assignment(&self, loc: SourceLocation, ident: &str) -> ParseError {
        ParseError::build(
            ParseErrorKind::InvalidAssignment {
                ident: ident.to_string(),
                suggestion: self.assignment_suggestion(loc.line),
            },
            loc,
            self.source,
        )
    }

    fn assignment_suggestion(&self, line: usize) -> String {
        let line_source = self
            .source
            .lines()
            .nth(line.saturating_sub(1))
            .unwrap_or("")
            .trim();
        if line_source.is_empty() {
            return "set <variable> = <valeur>".to_string();
        }
        format!("set {line_source}")
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
        let mut expr = match self.advance().cloned() {
            Some(Token::Int(n)) => Ok(Expr::Int(n)),
            Some(Token::Float(f)) => Ok(Expr::Float(f)),
            Some(Token::True) => Ok(Expr::Bool(true)),
            Some(Token::False) => Ok(Expr::Bool(false)),
            Some(Token::String(s)) => Ok(Expr::Str(Self::decode_string(&s[1..s.len() - 1]))),
            Some(Token::QuotedIdent(raw)) => Ok(Expr::Var(self.decode_variable_name(raw,loc,true)?)),
            Some(Token::Ident(s)) => {
                let mut name = s.to_string();
                let mut quoted_part = false;
                while matches!(self.peek(), Some(Token::Dot)) {
                    self.advance();
                    if let Some(Token::Ident(part)) = &self.peek().cloned() {
                        self.advance();
                        name.push('.');
                        name.push_str(&part[..]);
                    } else if let Some(Token::QuotedIdent(raw)) = self.peek().cloned() {
                        let part_loc = self.current_location();self.advance();
                        name.push('.');name.push_str(&self.decode_variable_name(raw,part_loc,false)?);
                        quoted_part = true;
                    } else {
                        break;
                    }
                }
                if quoted_part && matches!(self.peek(),Some(Token::ParenOpen)) {
                    return Err(self.err_msg(loc,name,"nom de fonction non délimité"));
                }
                // Function call: name(args)
                if matches!(self.peek(), Some(Token::ParenOpen)) {
                    self.advance();
                    if name == "functions.value_v1" {
                        let (_, call) = self.parse_function_library_call1023(crate::function_library_program1023::LibraryCallMode::Value)?;
                        return Ok(Expr::LibraryCall { call });
                    }
                    if name.starts_with("functions."){return Err(self.err_msg(loc,name,"functions.value_v1"));}
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Token::ParenClose)) {
                        args.push(self.parse_expr()?);
                        while matches!(self.peek(), Some(Token::Comma)) {
                            self.advance();
                            args.push(self.parse_expr()?);
                        }
                    }
                    self.expect(")")?;
                    match self.typed_macro_read(&name,&args)? {
                        Some(value)=>Ok(value),None=>Ok(Expr::Call { name, args }),
                    }
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Some(Token::ParenOpen) => {
                let inner = self.parse_expr()?;
                let loc2 = self.current_location();
                match self.advance().cloned() {
                    Some(Token::ParenClose) => Ok(inner),
                    Some(tok) => Err(self.err_token(loc2, &tok, "`)` pour fermer l'expression")),
                    None => Err(self.err_eof(loc2, "`)`")),
                }
            }
            Some(Token::BraceOpen) => {
                // Dictionary syntax is represented by the same call IR as its
                // Blueprint node, avoiding a second execution implementation.
                let mut args = Vec::new();
                let mut keys = std::collections::BTreeSet::new();
                while !matches!(self.peek(), Some(Token::BraceClose)) {
                    let loc = self.current_location();
                    let Some(Token::String(raw)) = self.advance().cloned() else {
                        return Err(self.err_msg(
                            loc,
                            "clé invalide".into(),
                            "clé texte entre guillemets",
                        ));
                    };
                    let key = Self::decode_string(&raw[1..raw.len() - 1]);
                    if !keys.insert(key.clone()) {
                        return Err(self.err_msg(loc, key, "clé de dictionnaire unique"));
                    }
                    self.expect(":")?;
                    args.push(Expr::Str(key));
                    args.push(self.parse_expr()?);
                    if matches!(self.peek(), Some(Token::Comma)) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.expect("}")?;
                Ok(Expr::Call {
                    name: "dict".into(),
                    args,
                })
            }
            Some(Token::BracketOpen) => {
                // List literal: [a, b, c]
                let mut items = Vec::new();
                if !matches!(self.peek(), Some(Token::BracketClose)) {
                    items.push(self.parse_expr()?);
                    while matches!(self.peek(), Some(Token::Comma)) {
                        self.advance();
                        if matches!(self.peek(), Some(Token::BracketClose)) {
                            break; // trailing comma
                        }
                        items.push(self.parse_expr()?);
                    }
                }
                self.expect("]")?;
                Ok(Expr::ListLit(items))
            }
            Some(tok) => Err(self.err_token(loc, &tok, "valeur, variable, `(expr)` ou `[list]`")),
            None => Err(self.err_eof(loc, "expression")),
        }?;
        // Check for index access: expr[0], arr[i]
        while matches!(self.peek(), Some(Token::BracketOpen)) {
            self.advance();
            let index = self.parse_expr()?;
            self.expect("]")?;
            expr = Expr::Index {
                target: Box::new(expr),
                index: Box::new(index),
            };
        }
        Ok(expr)
    }

    // ── INTERPOLATION ─────────────────────────────────────────────────────────
    //
    // Parse "Bonjour [prenom], tu as [score + 1] points."
    // en [Lit("Bonjour "), Interp(Var("prenom")), Lit(", tu as "), Interp(Add...), Lit(" points.")]
    //
    // Les crochets [ ] délimitent une sous-expression.
    // Pour inclure un crochet littéral, doubler : [[ → [, ]] → ]

    fn decode_string(raw: &str) -> String {
        let mut decoded = String::new();
        let mut chars = raw.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                decoded.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => decoded.push('\n'),
                Some('r') => decoded.push('\r'),
                Some('t') => decoded.push('\t'),
                Some('"') => decoded.push('"'),
                Some('\\') => decoded.push('\\'),
                Some(other) => {
                    decoded.push('\\');
                    decoded.push(other);
                }
                None => decoded.push('\\'),
            }
        }
        decoded
    }

    pub fn parse_interpolated_str(raw: &str) -> ParseResult<InterpolatedText> {
        let mut segments = Vec::new();
        let mut lit = String::new();
        // Remove the outer dialogue-string escaping once. Quoted dictionary
        // keys inside an interpolation remain ordinary expression strings.
        let decoded = Self::decode_string(raw);
        let mut chars = decoded.char_indices().peekable();

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
                    let mut quoted = None;
                    let mut escaped = false;
                    for (_, ec) in chars.by_ref() {
                        if let Some(delimiter) = quoted {
                            expr_src.push(ec);
                            if escaped {
                                escaped = false;
                            } else if ec == '\\' {
                                escaped = true;
                            } else if ec == delimiter {
                                quoted = None;
                            }
                            continue;
                        }
                        match ec {
                            '"' | '`' => {
                                quoted = Some(ec);
                                expr_src.push(ec);
                            }
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
                    if let Some(token) = sub_parser.peek() {
                        return Err(sub_parser.err_token(
                            sub_parser.current_location(),
                            token,
                            "fin de l'expression interpolée",
                        ));
                    }
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

    pub fn parse_script_spanned(&mut self) -> ParseResult<Vec<SpannedStatement>> {
        let mut statements = Vec::new();
        while self.peek().is_some() {
            self.skip_newlines();
            let start = self.token_ranges[self.pos].start;
            let statement = self.parse_statement()?;
            let last = (0..self.pos)
                .rev()
                .find(|&index| !matches!(self.tokens[index].0, Token::Newline))
                .expect("a parsed statement consumes at least one token");
            statements.push(SpannedStatement {
                statement,
                range: start..self.token_ranges[last].end,
            });
        }
        Ok(statements)
    }

    pub fn parse_script_recovering(&mut self) -> RecoveredScript {
        let mut stmts = Vec::new();
        let mut errors = Vec::new();
        while self.peek().is_some() {
            match self.parse_statement() {
                Ok(stmt) => stmts.push(stmt),
                Err(err) => {
                    errors.push(err);
                    self.synchronize_after_error();
                }
            }
        }
        RecoveredScript {
            script: stmts,
            errors,
        }
    }

    fn synchronize_after_error(&mut self) {
        while let Some((tok, _)) = self.tokens.get(self.pos) {
            if matches!(tok, Token::Newline) {
                self.pos += 1;
                break;
            }
            self.pos += 1;
        }
    }

    // ── Dispatch ─────────────────────────────────────────────────────────────

    fn parse_statement(&mut self) -> ParseResult<Statement> {
        if !self.capture_spans {
            return self.parse_statement_inner();
        }
        self.skip_newlines();
        let start = self.token_ranges.get(self.pos).map(|range| range.start);
        self.span_frames.push(Vec::new());
        let result = self.parse_statement_inner();
        // Restore the frame on every error path; children of a refused
        // statement must never become independent, accepted source origins.
        let children = self.span_frames.pop().expect("statement span frame");
        if result.is_ok() {
            let end = (0..self.pos)
                .rev()
                .find(|&index| !matches!(self.tokens[index].0, Token::Newline))
                .map(|index| self.token_ranges[index].end)
                .expect("a parsed statement consumes a token");
            let span = StatementSpan {
                range: start.expect("statement initial token")..end,
                children,
            };
            if let Some(parent) = self.span_frames.last_mut() {
                parent.push(span);
            } else {
                self.root_spans.push(span);
            }
        }
        result
    }

    fn parse_statement_inner(&mut self) -> ParseResult<Statement> {
        self.skip_newlines();
        let loc = self.current_location();
        if self.function_depth > 0
            && !matches!(
                self.peek(),
                Some(
                    Token::Set
                        | Token::Define
                        | Token::Default
                        | Token::Local
                        | Token::If
                        | Token::While
                        | Token::For
                        | Token::Return
                )
            )
            && !(self.unit_depth > 0 && matches!(self.peek(), Some(Token::Call)))
            && !((self.unit_depth > 0 || self.handler_depth > 0)
                && matches!(self.peek(),Some(Token::Ident("functions")))
                && matches!(self.tokens.get(self.pos+2).map(|token|&token.0),Some(Token::Ident("invoke_v1"))))
            && !(matches!(self.peek(), Some(Token::Ident("flow")))
                && matches!(self.tokens.get(self.pos+2).map(|token|&token.0),Some(Token::Ident("sequence"|"for_loop"|"break"|"consume_v1"|"consume_v2"|"consume_v3"|"macro_dispatch_v1"))))
            && !(self.handler_depth > 0
                && ((matches!(self.peek(), Some(Token::Ident("widget")))
                    && matches!(self.tokens.get(self.pos + 2).map(|token| &token.0), Some(Token::Ident("remove_from_parent"))))
                || matches!(
                    self.peek(),
                    Some(Token::Ident(
                        "ui" | "motion" | "video" | "accessibility" | "menu"
                    ))
                ) || (matches!(self.peek(), Some(Token::Ident("character")))
                    && matches!(
                        self.tokens.get(self.pos + 2).map(|token| &token.0),
                        Some(Token::Ident("compose" | "attributes"))
                    ))))
        {
            return Err(self.err_msg(loc, format!("{:?}", self.peek()),
                "fonction de calcul : set, if, while, for ou return <valeur> (sans opération narrative)"));
        }
        match self.peek().cloned() {
            Some(Token::Function) => self.parse_function(),
            Some(Token::Screen | Token::Handler) => self.parse_function(),
            Some(Token::Local) => {
                if self.function_depth == 0
                    && (self.narrative_context == NarrativeContext::Legacy || self.init_depth > 0)
                {
                    return Err(self.err_msg(
                        loc,
                        "local".into(),
                        if self.init_depth > 0
                            && self.narrative_context == NarrativeContext::ExplicitScenes
                        {
                            "local hors d'un bloc init"
                        } else {
                            "local dans une fonction, un écran ou un gestionnaire"
                        },
                    ));
                }
                let Statement::SetVar { name, value } = self.parse_set()? else {
                    unreachable!()
                };
                Ok(Statement::LocalVar { name, value })
            }
            Some(Token::While) => self.parse_while(),
            Some(Token::For) => self.parse_for(),
            Some(Token::Use) => self.parse_use(),
            Some(Token::Init) => self.parse_init(),
            Some(Token::Choice) => self.parse_choice(),
            Some(Token::Set) | Some(Token::Define) | Some(Token::Default) => self.parse_set(),
            Some(Token::Voice) => self.parse_voice(),
            Some(Token::Timer) => self.parse_timer(),
            Some(Token::If) => self.parse_if(),
            Some(Token::Label) => self.parse_label(),
            Some(Token::Jump) => self.parse_jump(),
            Some(Token::Call) => self.parse_call(),
            Some(Token::Return) => {
                self.advance();
                if self.unit_depth > 0 {
                    Ok(Statement::UnitReturn)
                } else if self.function_depth > 0 {
                    Ok(Statement::FunctionReturn {
                        value: self.parse_expr()?,
                    })
                } else {
                    Ok(Statement::Return)
                }
            }
            Some(Token::Scene) => self.parse_scene(),
            Some(Token::Cinematic) => self.parse_cinematic(),
            Some(Token::UnlockEnding) => self.parse_unlock_ending(),
            Some(Token::Imagemap) => self.parse_imagemap(),
            Some(Token::Typewriter) => self.parse_typewriter(),
            Some(Token::String(_)) => self.parse_dialogue_text_only(),
            Some(Token::Ident(_)) => self.parse_ident_statement(),
            Some(tok) => Err(self.err_token(loc, &tok, "début d'une instruction")),
            None => Err(self.err_eof(loc, "début d'une instruction")),
        }
    }

    fn parse_function(&mut self) -> ParseResult<Statement> {
        let declaration = self.peek().cloned();
        if self.block_depth > 0 {
            return Err(self.err_msg(
                self.current_location(),
                "function".into(),
                "déclaration de fonction hors des blocs",
            ));
        }
        self.advance();
        let name = self.parse_binding_name()?;
        self.expect("(")?;
        let mut parameters = Vec::new();
        while !matches!(self.peek(), Some(Token::ParenClose)) {
            let loc = self.current_location();
            let parameter = self.parse_variable_name_part(true)?;
            if parameters.contains(&parameter) || parameters.len() >= 128 {
                return Err(self.err_msg(loc, parameter, "paramètre unique (128 maximum)"));
            }
            parameters.push(parameter);
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(")")?;
        let unit = matches!(self.peek(),Some(Token::FunctionResultArrow));
        if unit {
            if !matches!(declaration,Some(Token::Function)) {
                return Err(self.err_msg(self.current_location(),"-> ()".into(),"résultat unit réservé à une fonction séquencée"));
            }
            self.advance();self.expect("(")?;self.expect(")")?;
        }
        self.expect("{")?;
        self.function_depth += 1;
        if unit { self.unit_depth += 1; }
        if matches!(declaration, Some(Token::Handler)) {
            self.handler_depth += 1;
        }
        let parsed = self.parse_block();
        if matches!(declaration, Some(Token::Handler)) {
            self.handler_depth -= 1;
        }
        self.function_depth -= 1;
        if unit { self.unit_depth -= 1; }
        let body = parsed?;
        if unit { return Ok(Statement::UnitFunction{name,parameters,body}); }
        Ok(match declaration {
            Some(Token::Screen) => Statement::Screen {
                name,
                parameters,
                body,
            },
            Some(Token::Handler) => Statement::Handler {
                name,
                parameters,
                body,
            },
            _ => Statement::Function {
                name,
                parameters,
                body,
            },
        })
    }


    fn decode_variable_name(&self, raw: &str, loc: SourceLocation, reserve_internal: bool) -> ParseResult<String> {
        let mut chars = raw[1..raw.len()-1].chars();
        let mut name = String::new();
        while let Some(c) = chars.next() {
            if c != '\\' { name.push(c); continue; }
            match chars.next() {
                Some(c @ ('\\' | '`')) => name.push(c),
                _ => return Err(self.err_msg(loc,raw.into(),"échappement de backslash ou backtick dans le nom")),
            }
        }
        if name.is_empty() || name.chars().any(|c|c.is_control() || c=='.')
            || reserve_internal && name.starts_with("__rvn_") {
            return Err(self.err_msg(loc,name,"nom non vide sans contrôle, point ni préfixe interne réservé"));
        }
        Ok(name)
    }

    fn parse_variable_name_part(&mut self, reserve_internal: bool) -> ParseResult<String> {
        let loc = self.current_location();
        match self.advance().cloned() {
            Some(Token::Ident(name)) if !reserve_internal || !name.starts_with("__rvn_") => Ok(name.into()),
            Some(Token::QuotedIdent(raw)) => self.decode_variable_name(raw,loc,reserve_internal),
            Some(token) => Err(self.err_token(loc,&token,"nom de variable valide")),
            None => Err(self.err_eof(loc,"nom de variable")),
        }
    }

    fn parse_binding_name(&mut self) -> ParseResult<String> {
        let loc = self.current_location();
        match self.advance().cloned() {
            Some(Token::Ident(name)) if !name.starts_with("__rvn_") => Ok(name.to_owned()),
            Some(Token::Ident(name)) => {
                Err(self.err_msg(loc, name.into(), "nom sans le préfixe réservé __rvn_"))
            }
            Some(token) => Err(self.err_token(loc, &token, "nom de variable ou fonction")),
            None => Err(self.err_eof(loc, "nom de variable ou fonction")),
        }
    }

    fn parse_while(&mut self) -> ParseResult<Statement> {
        self.advance();
        let condition = self.parse_expr()?;
        self.expect("{")?;
        Ok(Statement::While {
            condition,
            body: self.parse_block()?,
        })
    }

    fn parse_for(&mut self) -> ParseResult<Statement> {
        self.advance();
        let name = self.parse_variable_name_part(true)?;
        let loc = self.current_location();
        if !matches!(self.peek(), Some(Token::In)) {
            return Err(self.err_msg(
                loc,
                format!("{:?}", self.peek()),
                "in après la variable de boucle",
            ));
        }
        self.advance();
        let collection = self.parse_expr()?;
        self.expect("{")?;
        Ok(Statement::ForEach {
            name,
            collection,
            body: self.parse_block()?,
        })
    }

    // ── `use "file.rvn"` / `use { "a.rvn", "b.rvn" }` ─────────────────────

    fn parse_use(&mut self) -> ParseResult<Statement> {
        self.advance();
        let loc = self.current_location();
        match self.peek().cloned() {
            Some(Token::String(_)) => {
                let tok = self.expect("chemin de fichier après `use`")?;
                Ok(Statement::Use {
                    paths: vec![Parser::unwrap_string(&tok).to_string()],
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
                            paths.push(Parser::unwrap_string(&tok).to_string());
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
        self.init_depth += 1;
        let parsed = self.parse_block();
        self.init_depth -= 1;
        let body = parsed?;
        Ok(Statement::Init { body })
    }

    // ── `typewriter` ─────────────────────────────────────────────────────────

    fn parse_typewriter(&mut self) -> ParseResult<Statement> {
        self.advance();
        let loc = self.current_location();
        match self.peek().cloned() {
            Some(Token::Assign) => {
                self.advance();
                let enabled = self.parse_expr()?;
                Ok(match enabled {
                    Expr::Bool(enabled) => Statement::TypewriterSet { enabled },
                    enabled => Statement::TypewriterSetValue { enabled },
                })
            }
            Some(Token::Dot) => {
                self.advance();
                let loc2 = self.current_location();
                match self.advance().cloned() {
                    Some(Token::Ident("speed")) => {
                        self.expect("(")?;
                        let loc3 = self.current_location();
                        let value = self.parse_expr()?;
                        self.expect(")")?;
                        match value {
                            Expr::Int(value) => {
                                let chars_per_sec = u32::try_from(value).map_err(|_| self.err_msg(
                                    loc3, value.to_string(), "entier entre 0 et 4294967295 pour typewriter.speed(n)"))?;
                                Ok(Statement::TypewriterSpeed { chars_per_sec })
                            }
                            Expr::Neg(ref inner) if matches!(inner.as_ref(), Expr::Int(_)) => Err(self.err_msg(
                                loc3, "vitesse négative".into(), "entier entre 0 et 4294967295 pour typewriter.speed(n)")),
                            chars_per_sec => Ok(Statement::TypewriterSpeedValue { chars_per_sec }),
                        }
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
                            name = Some(Parser::unwrap_string(&tok).to_string());
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
        if matches!(self.peek(), Some(Token::Label)) {
            if self.block_depth > 0 || self.function_depth > 0 || self.init_depth > 0 {
                return Err(self.err_msg(
                    loc,
                    "scene label".into(),
                    "entrée `scene label` à la racine du programme",
                ));
            }
            self.advance();
            return Ok(Statement::SceneLabel {
                name: self.expect_ident("nom de label de scène")?,
            });
        }
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

    // ── `cinematic "id" [with transition]` / `cinematic hide [with transition]` ──

    fn parse_cinematic(&mut self) -> ParseResult<Statement> {
        self.advance();
        let loc = self.current_location();
        match self.advance().cloned() {
            Some(Token::String(raw)) => {
                let id = raw[1..raw.len() - 1].to_string();
                let transition = self.try_parse_with_name()?;
                Ok(Statement::CinematicShow { id, transition })
            }
            Some(Token::Ident("hide")) => {
                let transition = self.try_parse_with_name()?;
                Ok(Statement::CinematicHide { transition })
            }
            Some(tok) => Err(self.err_token(loc, &tok, "`\"id\"` ou `hide` après `cinematic`")),
            None => Err(self.err_eof(loc, "`\"id\"` ou `hide` après `cinematic`")),
        }
    }

    fn parse_unlock_ending(&mut self) -> ParseResult<Statement> {
        self.advance();
        let loc = self.current_location();
        match self.advance().cloned() {
            Some(Token::String(raw)) => Ok(Statement::UnlockEnding {
                id: raw[1..raw.len() - 1].to_string(),
            }),
            Some(tok) => Err(self.err_token(loc, &tok, "`\"id\"` après `unlock_ending`")),
            None => Err(self.err_eof(loc, "`\"id\"` après `unlock_ending`")),
        }
    }

    // ── Identifiant ──────────────────────────────────────────────────────────

    fn parse_ident_statement(&mut self) -> ParseResult<Statement> {
        let ident_loc = self.current_location();
        let ident = match self.advance().cloned() {
            Some(Token::Ident(s)) => s.to_string(),
            _ => unreachable!(),
        };
        let loc = self.current_location();
        match self.peek().cloned() {
            Some(Token::Colon) => {
                self.advance();
                let tok = self.expect("valeur de config")?.clone();
                if !matches!(tok, Token::String(_)) {
                    return Err(self.err_token(loc,&tok,"une valeur de configuration entre guillemets (utilisez set pour une variable)"));
                }
                Ok(Statement::Config {
                    key: ident,
                    value: Parser::unwrap_string(&tok).to_string(),
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
            Some(Token::Assign) => Err(self.err_invalid_assignment(ident_loc, &ident)),
            Some(tok) => {
                Err(self.err_token(loc, &tok, "`:`, `.` ou une string après l'identifiant"))
            }
            None => Err(self.err_eof(loc, "suite de l'instruction")),
        }
    }

    fn parse_flow_method(&mut self, method: &str) -> ParseResult<Statement> {
        let loc = self.current_location();
        match method {
            "macro_dispatch_v1" => self.parse_standard_macro(),
            "consume_v1" | "consume_v2" | "consume_v3" => {
                let literal = |parser: &mut Self| -> ParseResult<String> {
                    match parser.parse_expr()? {
                        Expr::Str(value) if !value.is_empty() && value.len() <= 256 && !value.contains('\0') => Ok(value),
                        _ => Err(parser.err_msg(loc, "identity".into(), "identité littérale non vide (256 octets maximum)")),
                    }
                };
                let scope = literal(self)?; self.expect(",")?;
                let consumer = literal(self)?; self.expect(")")?; self.expect("{")?;
                let mut steps = Vec::new();
                self.skip_newlines();
                while matches!(self.peek(), Some(Token::Ident("capture"|"inspect"))) {
                    let inspection = matches!(self.advance(), Some(Token::Ident("inspect")));
                    let producer = literal(self)?; self.expect("=")?;
                    let value = self.parse_expr()?;
                    if steps.len() >= 1024 || steps.iter().any(|step| match step {
                        crate::ConsumerStep::Capture { producer: id, .. } |
                        crate::ConsumerStep::InspectTextTable { producer: id, .. } => id == &producer,
                    }) { return Err(self.err_msg(loc, producer, "producteur unique (1024 maximum)")); }
                    steps.push(if inspection { crate::ConsumerStep::InspectTextTable { producer, value } }
                        else { crate::ConsumerStep::Capture { producer, value } });
                    self.skip_newlines();
                }
                let body = self.parse_block()?;
                if method=="consume_v1" && !matches!(body.as_slice(), [Statement::SetVar {..} | Statement::LocalVar {..} | Statement::FunctionReturn {..}]) {
                    return Err(self.err_msg(loc, consumer, "exactement un consommateur set, local ou return"));
                }
                if method=="consume_v2" && (!crate::scheduled_consumer_body_supported(2, &body) || body.is_empty() && !steps.is_empty()) {
                    return Err(self.err_msg(loc,consumer,"un consommateur v2 représentable, sans boucle pré-évaluée ni continuation opaque"));
                }
                let version=match method {"consume_v1"=>1,"consume_v2"=>2,_=>3};
                let statement=Statement::ScheduledConsumer { version, scope, consumer, steps, body };
                if version==3 {
                    crate::scheduled_source::native_loop_contract::binding(&statement)
                        .map_err(|error|self.err_msg(loc,error,"une boucle Native v3 avec son identite/pins reels"))?;
                }
                Ok(statement)
            }
            "sequence" => {
                self.expect(")")?; self.expect("{")?;
                let mut branches = Vec::new();
                self.skip_newlines();
                while !matches!(self.peek(), Some(Token::BraceClose)) {
                    if branches.len() >= 32 { return Err(self.err_msg(loc, "sequence".into(), "1 à 32 sorties de séquence")); }
                    self.expect("{")?;
                    branches.push(self.parse_block()?);
                    self.skip_newlines();
                }
                self.expect("}")?;
                if branches.is_empty() { return Err(self.err_msg(loc, "sequence vide".into(), "1 à 32 sorties de séquence")); }
                Ok(Statement::Sequence { branches })
            }
            "for_loop" => {
                let name = self.parse_variable_name_part(true)?; self.expect(",")?;
                let first = self.parse_expr()?; self.expect(",")?;
                let last = self.parse_expr()?; self.expect(")")?; self.expect("{")?;
                self.for_loop_depth += 1;
                let body = self.parse_block();
                self.for_loop_depth -= 1;
                Ok(Statement::ForLoopWithBreak { name, first, last, body: body? })
            }
            "break" => {
                self.expect(")")?;
                if self.for_loop_depth == 0 { return Err(self.err_msg(loc, "flow.break".into(), "break dans le corps d'un ForLoopWithBreak")); }
                Ok(Statement::LoopBreak)
            }
            _ => Err(self.err_msg(loc, method.into(), "flow.sequence, flow.for_loop ou flow.break")),
        }
    }

    // ── `ident.method(args) …` ───────────────────────────────────────────────

    fn parse_method_call(&mut self, target: String) -> ParseResult<Statement> {
        self.advance();
        let method = self.expect_ident("nom de méthode")?;
        self.expect("(")?;
        if target == "functions" && ["library_","member_","invoke_","value_"].iter().any(|prefix|method.starts_with(*prefix)) {
            return match method.as_str() {
                "library_v1" => self.parse_function_library_scope1023().map(|scope|Statement::FunctionLibrary { scope }),
                "invoke_v1" => self.parse_function_library_call1023(crate::function_library_program1023::LibraryCallMode::Completion).map(|(_,call)|Statement::LibraryUnitCall { call }),
                _ => Err(self.err_msg(self.current_location(),method,"functions.library_v1 ou functions.invoke_v1")),
            };
        }
        if target == "flow" && matches!(method.as_str(),"sequence"|"for_loop"|"break"|"consume_v1"|"consume_v2"|"consume_v3"|"macro_dispatch_v1") { return self.parse_flow_method(&method); }

        // Preserve historical single literals/strings/defaults on their exact AST path.
        // Other arguments are genuine expressions, never parsed from a string.
        if target == "music" && method == "volume" {
            let literal_end = match self.peek() {
                Some(Token::Int(_) | Token::Float(_) | Token::String(_)) => Some(self.pos + 1),
                Some(Token::Minus) if matches!(self.tokens.get(self.pos+1).map(|t| &t.0), Some(Token::Int(_) | Token::Float(_))) => Some(self.pos + 2),
                Some(Token::ParenClose) => Some(self.pos),
                _ => None,
            };
            let historical = literal_end.is_some_and(|end| matches!(self.tokens.get(end).map(|t| &t.0),Some(Token::ParenClose)));
            if !historical {
                let level = self.parse_expr()?;
                self.expect(")")?;
                return Ok(Statement::MusicVolumeValue { level });
            }
        }
        // The public music.volume literal form shares the historical f32
        // level/clamp, without widening string-only generic method arguments.
        // Keep the quoted/default path below untouched for old authored ASTs.
        if target == "music"
            && method == "volume"
            && matches!(self.peek(), Some(Token::Int(_) | Token::Float(_) | Token::Minus))
        {
            let loc = self.current_location();
            let negative = matches!(self.peek(), Some(Token::Minus));
            if negative {
                self.advance();
            }
            let level = match self.advance().cloned() {
                Some(Token::Int(value)) => value as f32,
                Some(Token::Float(value)) => value,
                Some(token) => return Err(self.err_token(loc, &token, "nombre littéral fini dans music.volume()")),
                None => return Err(self.err_eof(loc, "nombre littéral dans music.volume()")),
            };
            if !level.is_finite() {
                return Err(self.err_msg(loc, "volume non fini".into(), "nombre littéral fini dans music.volume()"));
            }
            // One literal only. A comma, expression, or second token remains
            // an error at its actual location instead of being ignored.
            self.expect(")")?;
            let level = if negative { -level } else { level };
            return Ok(Statement::MusicVolume { level: level.clamp(0.0, 1.0) });
        }

        if (target == "widget" && matches!(method.as_str(),"add_to_viewport"|"remove_from_parent"))
            || target == "ui"
            || target == "menu"
            || target == "motion"
            || target == "video"
            || target == "accessibility"
            || (target == "character" && matches!(method.as_str(), "compose" | "attributes"))
        {
            let mut args = Vec::new();
            if !matches!(self.peek(), Some(Token::ParenClose)) {
                loop {
                    args.push(self.parse_expr()?);
                    if !matches!(self.peek(), Some(Token::Comma)) {
                        break;
                    }
                    self.advance();
                }
            }
            self.expect(")")?;
            if target == "menu" {
                return match (method.as_str(), args.as_slice()) {
                    ("execute", [request]) => Ok(Statement::MenuExecute {
                        request: request.clone(),
                    }),
                    _ => Err(self.err_msg(
                        self.current_location(),
                        format!("menu.{method}"),
                        "menu.execute(demande typée)",
                    )),
                };
            }
            if target == "accessibility" {
                return match (method.as_str(),args.as_slice()) {
                    ("configure",[settings])=>Ok(Statement::AccessibilityConfigure{settings:settings.clone()}),
                    ("speak",[text])=>Ok(Statement::AccessibilitySpeak{text:text.clone()}),
                    ("stop",[])=>Ok(Statement::AccessibilityStop),
                    _=>Err(self.err_msg(self.current_location(),format!("accessibility.{method}"),"accessibility.configure(propriétés), accessibility.speak(texte) ou accessibility.stop()")),
                };
            }
            if target == "video" {
                return match (method.as_str(),args.as_slice()) {
                    ("play",[name,definition])=>Ok(Statement::VideoPlay{name:name.clone(),definition:definition.clone()}),
                    ("pause",[name])=>Ok(Statement::VideoPause{name:name.clone()}),
                    ("resume",[name])=>Ok(Statement::VideoResume{name:name.clone()}),
                    ("stop",[name])=>Ok(Statement::VideoStop{name:name.clone()}),
                    ("skip",[name])=>Ok(Statement::VideoSkip{name:name.clone()}),
                    ("wait",[name])=>Ok(Statement::VideoWait{name:name.clone()}),
                    ("seek",[name,seconds])=>Ok(Statement::VideoSeek{name:name.clone(),seconds:seconds.clone()}),
                    ("volume",[name,volume])=>Ok(Statement::VideoVolume{name:name.clone(),volume:volume.clone()}),
                    _=>Err(self.err_msg(self.current_location(),format!("video.{method}"),"video.play(nom, clip), pause/resume/stop/skip/wait(nom), seek(nom, secondes) ou volume(nom, volume)")),
                };
            }
            if target == "character" {
                return match (method.as_str(), args.as_slice()) {
                    ("compose",[character,definition]) => Ok(Statement::CharacterCompose {character:character.clone(),definition:definition.clone()}),
                    ("attributes",[character,attributes]) => Ok(Statement::CharacterAttributes {character:character.clone(),attributes:attributes.clone()}),
                    _ => Err(self.err_msg(self.current_location(),format!("character.{method}"),"character.compose(personnage, composition) ou character.attributes(personnage, attributs)")),
                };
            }
            if target == "motion" {
                return match (method.as_str(), args.as_slice()) {
                    ("play", [target, definition]) => Ok(Statement::MotionPlay {
                        target: target.clone(),
                        definition: definition.clone(),
                    }),
                    ("stop", [target]) => Ok(Statement::MotionStop {
                        target: target.clone(),
                    }),
                    ("wait", [target]) => Ok(Statement::MotionWait {
                        target: target.clone(),
                    }),
                    _ => Err(self.err_msg(
                        self.current_location(),
                        format!("motion.{method}"),
                        "motion.play(cible, animation), motion.stop(cible) ou motion.wait(cible)",
                    )),
                };
            }
            if target == "widget" {
                return crate::widget_viewport_effect_1003::from_call(&method, &args)
                    .map_err(|_| self.err_msg(self.current_location(),format!("widget.{method}"),"widget.add_to_viewport(objet, ordreZ=0) ou widget.remove_from_parent(objet)"));
            }
            return match (method.as_str(), args.as_slice()) {
                ("open" | "open_story", [name, arguments, modal, layer]) => Ok(Statement::UiOpen {
                    name: name.clone(), arguments: arguments.clone(), modal: modal.clone(), layer: layer.clone(),
                    story: method == "open_story",
                }),
                ("close", [name]) => Ok(Statement::UiClose { name: name.clone() }),
                ("focus", [name, element]) => Ok(Statement::UiFocus { name: name.clone(), element: element.clone() }),
                ("set_state", [name, element, state]) => Ok(Statement::UiSetState { name:name.clone(),element:element.clone(),state:state.clone() }),
                _ => Err(self.err_msg(self.current_location(), format!("ui.{method}"),
                    "ui.open/open_story(nom, liste d’arguments, modal, couche), ui.close(nom), ui.focus(nom, élément) ou ui.set_state(nom, élément, état)")),
            };
        }

        if method == "effect" {
            return self.parse_effect_call(target);
        }
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
            Some(Token::SlideLeft) => {
                let d = self.try_parse_duration(Transition::DEFAULT_SLIDE_MS)?;
                Ok(Transition::SlideLeft { duration_ms: d })
            }
            Some(Token::SlideRight) => {
                let d = self.try_parse_duration(Transition::DEFAULT_SLIDE_MS)?;
                Ok(Transition::SlideRight { duration_ms: d })
            }
            Some(Token::SlideUp) => {
                let d = self.try_parse_duration(Transition::DEFAULT_SLIDE_MS)?;
                Ok(Transition::SlideUp { duration_ms: d })
            }
            Some(Token::SlideDown) => {
                let d = self.try_parse_duration(Transition::DEFAULT_SLIDE_MS)?;
                Ok(Transition::SlideDown { duration_ms: d })
            }
            Some(Token::ZoomIn) => {
                let d = self.try_parse_duration(Transition::DEFAULT_ZOOM_MS)?;
                Ok(Transition::ZoomIn { duration_ms: d })
            }
            Some(Token::ZoomOut) => {
                let d = self.try_parse_duration(Transition::DEFAULT_ZOOM_MS)?;
                Ok(Transition::ZoomOut { duration_ms: d })
            }
            Some(Token::Wipe) => {
                let d = self.try_parse_duration(Transition::DEFAULT_WIPE_MS)?;
                Ok(Transition::Wipe { duration_ms: d })
            }
            Some(Token::Blur) => {
                let d = self.try_parse_duration(Transition::DEFAULT_BLUR_MS)?;
                Ok(Transition::Blur { duration_ms: d })
            }
            Some(tok) => Err(self.err_token(loc, &tok, "`fade`, `dissolve`, `slideleft`, `slideright`, `slideup`, `slidedown`, `zoomin`, `zoomout`, `wipe` ou `blur` après `with`")),
            None => Err(self.err_eof(loc, "nom de transition")),
        }
    }

    fn try_parse_with_name(&mut self) -> ParseResult<Option<String>> {
        if matches!(self.peek_raw(), Some(Token::Newline) | None) {
            return Ok(None);
        }
        if !matches!(self.peek(), Some(Token::With)) {
            return Ok(None);
        }
        self.advance();
        let loc = self.current_location();

        match self.advance().cloned() {
            Some(Token::Fade) => Ok(Some("fade".to_string())),
            Some(Token::Dissolve) => Ok(Some("dissolve".to_string())),
            Some(Token::Ident(name)) => Ok(Some(name.to_string())),
            Some(tok) => Err(self.err_token(loc, &tok, "nom de transition après `with`")),
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
        let target=self.expect_ident("nom du label à appeler")?;
        if matches!(self.peek(),Some(Token::ParenOpen)) {
            self.advance();let mut arguments=Vec::new();
            if !matches!(self.peek(),Some(Token::ParenClose)) { loop {
                if arguments.len()>=128{return Err(self.err_msg(self.current_location(),target.clone(),"128 arguments au maximum"));}
                arguments.push(self.parse_expr()?);
                if !matches!(self.peek(),Some(Token::Comma)){break;}
                self.advance();
            }}
            self.expect(")")?;
            Ok(Statement::UnitCall{name:target,arguments})
        } else if self.unit_depth>0 {
            Err(self.err_msg(self.current_location(),target,"appel synchrone avec parenthèses, sans transfert narratif"))
        } else { Ok(Statement::Call { target }) }
    }

    /// `set nom = expr`
    // SpriteStyle scale/rotation are finite f32 literals, including a minus.
    // Expressions and nonnumeric values cannot be silently discarded.
    fn parse_signed_effect_number(&mut self) -> ParseResult<f32> {
        let loc = self.current_location();
        let negative = matches!(self.peek(), Some(Token::Minus));
        if negative { self.advance(); }
        let value = match self.advance().cloned() {
            Some(Token::Int(value)) => value as f32,
            Some(Token::Float(value)) => value,
            Some(token) => return Err(self.err_token(loc, &token, "finite signed numeric literal in sprite.effect")),
            None => return Err(self.err_eof(loc, "finite signed numeric literal in sprite.effect")),
        };
        if !value.is_finite() {
            return Err(self.err_msg(loc, "non-finite sprite effect value".into(), "finite signed numeric literal in sprite.effect"));
        }
        Ok(if negative { -value } else { value })
    }

    fn parse_effect_call(&mut self, target: String) -> ParseResult<Statement> {
        // `char.effect(flip_x: true, scale: 1.5, rotation: 45, tint: "#ff0000")`
        let mut flip_x = None;
        let mut flip_y = None;
        let mut scale = None;
        let mut rotation = None;
        let mut tint = None;
        loop {
            let loc = self.current_location();
            match self.peek().cloned() {
                Some(Token::ParenClose) => {
                    self.advance();
                    break;
                }
                Some(Token::Ident(_)) => {
                    let key = self.expect_ident("effect parameter name")?;
                    self.expect(":")?;
                    match key.as_str() {
                        "flip_x" => {
                            let tok = self.advance().cloned();
                            flip_x = Some(matches!(tok, Some(Token::True)));
                        }
                        "flip_y" => {
                            let tok = self.advance().cloned();
                            flip_y = Some(matches!(tok, Some(Token::True)));
                        }
                        "scale" => { scale = Some(self.parse_signed_effect_number()?); }
                        "rotation" => { rotation = Some(self.parse_signed_effect_number()?); }
                        "tint" => {
                            let tok = self.expect("string color for tint")?.clone();
                            tint = Some(Parser::unwrap_string(&tok).to_string());
                        }
                        _ => {
                            return Err(self.err_msg(
                                loc,
                                format!("unknown effect parameter `{key}`"),
                                "flip_x, flip_y, scale, rotation, tint",
                            ))
                        }
                    }
                    if matches!(self.peek(), Some(Token::Comma)) {
                        self.advance();
                    }
                }
                Some(tok) => return Err(self.err_token(loc, &tok, "effect parameter or )")),
                None => return Err(self.err_eof(loc, ") pour fermer effect")),
            }
        }
        Ok(Statement::SpriteEffect {
            character_id: target,
            flip_x,
            flip_y,
            scale,
            rotation,
            tint,
        })
    }

    fn parse_set(&mut self) -> ParseResult<Statement> {
        self.advance();
        let mut name = self.parse_variable_name_part(true)?;
        // Allow dotted names like `persistent.flag` for persistent variables.
        while matches!(self.peek(), Some(Token::Dot)) {
            self.advance();
            let part = self.parse_variable_name_part(false)?;
            name.push('.');
            name.push_str(&part[..]);
        }
        self.expect("=")?;
        let value = self.parse_expr()?;
        Ok(Statement::SetVar { name, value })
    }

    /// `voice "file.ogg"` or `voice stop`
    fn parse_voice(&mut self) -> ParseResult<Statement> {
        self.advance();
        // `voice stop` stops the current voice line.
        if matches!(self.peek(), Some(Token::Ident(_))) {
            let id = self.expect_ident("voice command")?;
            if id == "stop" {
                return Ok(Statement::VoiceStop);
            }
            return Err(self.err_msg(
                self.current_location(),
                format!("unknown voice subcommand `{id}`"),
                "`stop` or a string filename",
            ));
        }
        let loc = self.current_location();
        let file = match self.advance().cloned() {
            Some(Token::String(s)) => s[1..s.len() - 1].to_string(),
            Some(tok) => return Err(self.err_token(loc, &tok, "string filename for voice")),
            None => return Err(self.err_eof(loc, "string filename for voice")),
        };
        Ok(Statement::VoicePlay { file })
    }

    /// `timer 5.0 => jump label` or `timer cancel`
    fn parse_timer(&mut self) -> ParseResult<Statement> {
        self.advance();
        // `timer cancel` cancels any active timer.
        if matches!(self.peek(), Some(Token::Ident(_))) {
            let id = self.expect_ident("timer command")?;
            if id == "cancel" {
                return Ok(Statement::TimerCancel);
            }
            return Err(self.err_msg(
                self.current_location(),
                format!("unknown timer subcommand `{id}`"),
                "`cancel` or a number",
            ));
        }
        // Parse duration (int or float).
        let duration = match self.advance().cloned() {
            Some(Token::Int(n)) => n as f32,
            Some(Token::Float(f)) => f,
            Some(tok) => {
                return Err(self.err_token(
                    self.current_location(),
                    &tok,
                    "number (duration in seconds)",
                ))
            }
            None => return Err(self.err_eof(self.current_location(), "duration")),
        };
        // Expect => then action (jump or call).
        self.expect("=>")?;
        let action_kind = match self.advance().cloned() {
            Some(Token::Jump) => "jump",
            Some(Token::Call) => "call",
            Some(tok) => {
                return Err(self.err_token(self.current_location(), &tok, "jump or call after =>"))
            }
            None => return Err(self.err_eof(self.current_location(), "jump or call after =>")),
        };
        let target = self.expect_ident("label name")?;
        let action = format!("{} {}", action_kind, target);
        Ok(Statement::Timer {
            duration_secs: duration,
            action,
        })
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
        self.block_depth += 1;
        let result = self.parse_block_contents();
        self.block_depth -= 1;
        result
    }

    fn parse_block_contents(&mut self) -> ParseResult<Vec<Statement>> {
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
        while let Some(Token::String(_)) = self.peek() {
            let tok = self.advance().unwrap().clone();
            args.push(Parser::unwrap_string(&tok).to_string());
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
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
                    // Optional condition: `"option" if condition => { ... }`
                    let condition = if matches!(self.peek(), Some(Token::If)) {
                        self.advance();
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    self.expect("=>")?;
                    self.expect("{")?;
                    options.push(ChoiceOption {
                        label,
                        condition,
                        body: self.parse_block()?,
                    });
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

/// Parse an authored fragment under the context of its resolved program.
/// This does not create a runtime scene activation or invent an entry marker.
pub fn parse_with_context(source: &str, context: NarrativeContext) -> ParseResult<Script> {
    Parser::new_with_context(source, context)?.parse_script()
}

/// Syntax-only first pass used by import resolvers. The returned script must
/// be reparsed under `NarrativeContext::from_script` of the complete resolved
/// program before use; a fragment alone does not establish a scene program.
pub fn parse_for_context_discovery(source: &str) -> ParseResult<Script> {
    parse_with_context(source, NarrativeContext::ExplicitScenes)
}

/// Exact, nested UTF-8 source spans, parallel to the AST in lexical order.
pub fn parse_with_origins(source: &str, context: NarrativeContext) -> ParseResult<ParsedSource> {
    let mut parser = Parser::new_with_context(source, context)?;
    parser.capture_spans = true;
    let script = parser.parse_script()?;
    Ok(ParsedSource {
        script,
        spans: parser.root_spans,
    })
}

/// Parse with editable source ranges without changing the legacy parse API.
pub fn parse_spanned(source: &str) -> ParseResult<Vec<SpannedStatement>> {
    Parser::new(source)?.parse_script_spanned()
}

pub fn parse_spanned_with_context(
    source: &str,
    context: NarrativeContext,
) -> ParseResult<Vec<SpannedStatement>> {
    Parser::new_with_context(source, context)?.parse_script_spanned()
}

pub fn parse_recovering(source: &str) -> ParseResult<RecoveredScript> {
    Ok(Parser::new(source)?.parse_script_recovering())
}

pub fn parse_recovering_with_context(
    source: &str,
    context: NarrativeContext,
) -> ParseResult<RecoveredScript> {
    Ok(Parser::new_with_context(source, context)?.parse_script_recovering())
}

/// Parse une string interpolée hors contexte script.
/// Utilisé par le moteur pour parser les templates traduits à la volée.
pub fn parse_interpolated_str(raw: &str) -> ParseResult<InterpolatedText> {
    Parser::parse_interpolated_str(raw)
}


/// Emit an already authored raw string-token interior without decoding it or
/// applying expression-string escaping a second time. Historical fields such
/// as SpriteEffect.tint are stored this way and decoded once by interpolation.
/// Refuse interiors that cannot be precisely one real quoted token; this is
/// not an encoder for an arbitrary logical String or a source-injection escape.
pub fn raw_string_token_source(raw: &str) -> Result<String, String> {
    let source = format!("\"{raw}\"");
    let parser = Parser::new(&source).map_err(|error| error.to_string())?;
    if !matches!(parser.tokens.as_slice(), [(Token::String(token), _)] if *token == source.as_str()) {
        return Err("Raw string interior must preserve exactly one quoted source token".into());
    }
    Ok(source)
}

#[cfg(test)]
mod raw_tint_string_tests1002 {
    use super::*;
    fn tint(source: &str) -> String {
        let script = parse(source).unwrap();
        let [Statement::SpriteEffect { tint: Some(tint), .. }] = script.as_slice() else { panic!("actual effect") };
        tint.clone()
    }
    #[test]
    fn raw_tint1002_source_and_json_keep_historical_raw_bytes_and_one_interpolation_decode() {
        for raw in [r##"#ffffffff"##, r##"[( [1, \"#ffffffff\"] )[1]]"##, r##"literal\\path\n\t\q"##, "", "literal\nnewline"] {
            let source = format!("hero.effect(tint:{})\n", raw_string_token_source(raw).unwrap());
            assert_eq!(tint(&source), raw);
            let ast = parse(&source).unwrap();
            let restored: Script = serde_json::from_str(&serde_json::to_string(&ast).unwrap()).unwrap();
            assert_eq!(restored, ast, "the stored historical raw representation does not change");
            assert_eq!(parse_interpolated_str(&tint(&source)).unwrap(), parse_interpolated_str(raw).unwrap());
        }
        let raw = r##"[( [1, \"#ffffffff\"] )[1]]"##;
        let parsed = parse_interpolated_str(raw).unwrap();
        assert!(matches!(parsed.0.as_slice(), [crate::TextSegment::Interp(crate::Expr::Index { .. })]));
        let expression_string = parse(&format!("set decoded={}\n", raw_string_token_source(raw).unwrap())).unwrap();
        let [Statement::SetVar { value: crate::Expr::Str(decoded), .. }] = expression_string.as_slice() else { panic!("real expression string") };
        assert_ne!(decoded, raw, "expression strings decode their outer escapes; raw fields intentionally do not");
    }
    #[test]
    fn raw_tint1002_unrepresentable_interior_is_refused_without_escaping_or_source_injection() {
        for raw in ["unterminated\\", "unescaped\"quote", "\")\nreturn\n//"] {
            assert!(raw_string_token_source(raw).is_err());
        }
    }
}

#[cfg(test)]
#[path="unit_sequenced_parser_tests1003.rs"]
mod unit_sequenced_parser_tests1003;

#[cfg(test)]
#[path="parser_location_cursor_tests1022.rs"]
mod location_cursor1022;
