// rvn_parser/src/lexer.rs
//
// CHANGEMENTS vs version précédente :
//   - Ajout : And, Or, Plus, Minus, Star, Slash (pour les expressions)

use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t]+")]
#[logos(skip(r"//[^\r\n]*", allow_greedy = true))]
pub enum Token<'a> {
    #[token("init")]
    Init,
    #[token("choice")]
    Choice,
    #[token("set")]
    Set,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("not")]
    Not,
    #[token("and")]
    And,
    #[token("or")]
    Or,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("label")]
    Label,
    #[token("jump")]
    Jump,
    #[token("call")]
    Call,
    #[token("return")]
    Return,
    #[token("scene")]
    Scene,
    #[token("cinematic")]
    Cinematic,
    #[token("unlock_ending")]
    UnlockEnding,
    #[token("with")]
    With,
    #[token("fade")]
    Fade,
    #[token("dissolve")]
    Dissolve,
    #[token("imagemap")]
    Imagemap,
    #[token("hotspot")]
    Hotspot,
    #[token("at")]
    At,
    #[token("==")]
    Eq,
    #[token("!=")]
    Ne,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("=>")]
    Arrow,
    #[token("=")]
    Assign,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token(".")]
    Dot,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token("(")]
    ParenOpen,
    #[token(")")]
    ParenClose,
    #[token("{")]
    BraceOpen,
    #[token("}")]
    BraceClose,
    #[token("[")]
    BracketOpen,
    #[token("]")]
    BracketClose,
    #[token("typewriter")]
    Typewriter,
    #[token("use")]
    Use,

    #[regex("[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident(&'a str),

    #[regex(r#""([^"\\]|\\t|\\u|\\n|\\")*""#)]
    String(&'a str),

    // Match integer literals without a leading minus.  Negative numbers are lexed as a
    // separate Minus token followed by an Int token so that unary minus can be
    // distinguished from a literal negative value.  The parser will handle
    // combining a leading minus with the following integer as a negation.
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    Int(i64),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f32>().ok())]
    Float(f32),

    #[regex(r"\n+")]
    Newline,
}
