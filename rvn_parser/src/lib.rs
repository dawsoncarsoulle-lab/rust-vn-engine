// rvn_parser/src/lib.rs

pub mod ast;
pub mod error;
pub mod expr;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use error::*;
pub use expr::{BinOpKind, Expr, InterpolatedText, TextSegment};
pub use parser::{parse, parse_interpolated_str};

// ─── TESTS ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn plain(s: &str) -> InterpolatedText {
        InterpolatedText::plain(s)
    }

    // ── Interpolation ─────────────────────────────────────────────────────────

    #[test]
    fn test_dialogue_plain() {
        let s = parse(r#"sarah "Bonjour !""#).unwrap();
        assert!(matches!(&s[0], Statement::Dialogue { text, .. } if text.is_plain()));
    }

    #[test]
    fn test_dialogue_interpolated_var() {
        let s = parse(r#""Bonjour [prenom] !""#).unwrap();
        let Statement::Dialogue { text, .. } = &s[0] else {
            panic!()
        };
        assert_eq!(text.0.len(), 3);
        assert!(matches!(&text.0[0], TextSegment::Lit(s) if s == "Bonjour "));
        assert!(matches!(&text.0[1], TextSegment::Interp(Expr::Var(v)) if v == "prenom"));
        assert!(matches!(&text.0[2], TextSegment::Lit(s) if s == " !"));
    }

    #[test]
    fn test_dialogue_interpolated_expr() {
        let s = parse(r#""Tu as [score + 1] points.""#).unwrap();
        let Statement::Dialogue { text, .. } = &s[0] else {
            panic!()
        };
        assert!(matches!(
            &text.0[1],
            TextSegment::Interp(Expr::BinOp {
                op: BinOpKind::Add,
                ..
            })
        ));
    }

    #[test]
    fn test_choice_interpolated_label() {
        let s = parse(r#"choice { "Option [n]" => { jump a } }"#).unwrap();
        let Statement::Choice { options } = &s[0] else {
            panic!()
        };
        assert!(!options[0].0.is_plain());
    }

    #[test]
    fn test_interpolation_escape_bracket() {
        let s = parse(r#""un [[crochet]]""#).unwrap();
        let Statement::Dialogue { text, .. } = &s[0] else {
            panic!()
        };
        // [[ → [ et ]] → ] donc texte plain "un [crochet]"
        assert!(text.is_plain());
        assert_eq!(text.as_plain().unwrap(), "un [crochet]");
    }

    // ── Expressions arithmétiques ─────────────────────────────────────────────

    #[test]
    fn test_set_expr_add() {
        let s = parse("set x = score + 10").unwrap();
        assert!(matches!(
            &s[0],
            Statement::SetVar {
                value: Expr::BinOp {
                    op: BinOpKind::Add,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn test_set_expr_mul() {
        let s = parse("set y = x * 2").unwrap();
        assert!(matches!(
            &s[0],
            Statement::SetVar {
                value: Expr::BinOp {
                    op: BinOpKind::Mul,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn test_set_expr_precedence() {
        // a + b * c doit parser comme a + (b * c)
        let s = parse("set z = 1 + 2 * 3").unwrap();
        let Statement::SetVar { value, .. } = &s[0] else {
            panic!()
        };
        // Le nœud racine est Add
        assert!(matches!(
            value,
            Expr::BinOp {
                op: BinOpKind::Add,
                ..
            }
        ));
        if let Expr::BinOp { right, .. } = value {
            // Le membre droit est Mul
            assert!(matches!(
                **right,
                Expr::BinOp {
                    op: BinOpKind::Mul,
                    ..
                }
            ));
        }
    }

    #[test]
    fn test_set_expr_parens() {
        let s = parse("set z = (1 + 2) * 3").unwrap();
        let Statement::SetVar { value, .. } = &s[0] else {
            panic!()
        };
        // Le nœud racine est Mul
        assert!(matches!(
            value,
            Expr::BinOp {
                op: BinOpKind::Mul,
                ..
            }
        ));
    }

    #[test]
    fn test_set_expr_neg() {
        let s = parse("set x = -5").unwrap();
        assert!(matches!(
            &s[0],
            Statement::SetVar {
                value: Expr::Neg(_),
                ..
            }
        ));
    }

    #[test]
    fn test_set_expr_float() {
        let s = parse("set ratio = 3.14").unwrap();
        assert!(matches!(
            &s[0],
            Statement::SetVar {
                value: Expr::Float(_),
                ..
            }
        ));
    }

    // ── Conditions booléennes ─────────────────────────────────────────────────

    #[test]
    fn test_if_and() {
        let s = parse("if score > 5 and flag == true { jump a }").unwrap();
        assert!(matches!(
            &s[0],
            Statement::If {
                condition: Expr::And(..),
                ..
            }
        ));
    }

    #[test]
    fn test_if_or() {
        let s = parse("if a == 1 or b == 2 { jump x }").unwrap();
        assert!(matches!(
            &s[0],
            Statement::If {
                condition: Expr::Or(..),
                ..
            }
        ));
    }

    #[test]
    fn test_if_not_expr() {
        let s = parse("if not flag { jump x }").unwrap();
        assert!(matches!(
            &s[0],
            Statement::If {
                condition: Expr::Not(_),
                ..
            }
        ));
    }

    #[test]
    fn test_if_complex() {
        // not (x == 0 or y == 0)
        let s = parse("if not (x == 0 or y == 0) { jump ok }").unwrap();
        assert!(matches!(
            &s[0],
            Statement::If {
                condition: Expr::Not(_),
                ..
            }
        ));
    }

    #[test]
    fn test_if_precedence_and_over_or() {
        // a or b and c  →  a or (b and c)
        let s = parse("if a or b and c { jump x }").unwrap();
        let Statement::If { condition, .. } = &s[0] else {
            panic!()
        };
        assert!(matches!(condition, Expr::Or(..)));
        if let Expr::Or(_, right) = condition {
            assert!(matches!(**right, Expr::And(..)));
        }
    }

    // ── Rétrocompatibilité ────────────────────────────────────────────────────
    // Les tests existants continuent de compiler avec les nouveaux types.

    #[test]
    fn test_show_sans_position_ni_transition() {
        let s = parse(r#"sarah.show("sourire")"#).unwrap();
        assert!(matches!(&s[0], Statement::ShowSprite {
            character_id, emotion: Some(_), position: None, transition: Transition::None,
        } if character_id == "sarah"));
    }

    #[test]
    fn test_set_int_literal() {
        // set x = 42  → SetVar { value: Expr::Int(42) }
        let s = parse("set score = 42").unwrap();
        assert!(matches!(
            &s[0],
            Statement::SetVar {
                value: Expr::Int(42),
                ..
            }
        ));
    }

    #[test]
    fn test_set_bool_literal() {
        let s = parse("set flag = true").unwrap();
        assert!(matches!(
            &s[0],
            Statement::SetVar {
                value: Expr::Bool(true),
                ..
            }
        ));
    }

    #[test]
    fn test_set_string_literal() {
        let s = parse(r#"set nom = "Sarah""#).unwrap();
        assert!(matches!(
            &s[0],
            Statement::SetVar {
                value: Expr::Str(_),
                ..
            }
        ));
    }

    #[test]
    fn test_if_cmp_simple() {
        let s = parse("if score > 5 { sarah \"Bravo !\" }").unwrap();
        assert!(matches!(
            &s[0],
            Statement::If {
                condition: Expr::BinOp {
                    op: BinOpKind::Gt,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn test_error_contains_line_number() {
        let src = "label debut\n@erreur";
        assert!(parse(src).unwrap_err().to_string().contains("ligne 2"));
    }

    #[test]
    fn test_lex_error_location() {
        let src = "label debut\n    @ token inconnu";
        let err = parse(src).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ligne 2"));
        assert!(msg.contains('@'));
    }

    #[test]
    fn test_imagemap_minimal() {
        let src = r#"imagemap { background : "ville.jpg" hotspot { area : (100, 80, 280, 200) } => { jump marche } }"#;
        assert_eq!(parse(src).unwrap().len(), 1);
    }

    #[test]
    fn test_choice_simple() {
        let s = parse(r#"choice { "Oui" => { jump a } "Non" => { jump b } }"#).unwrap();
        assert!(matches!(&s[0], Statement::Choice { .. }));
    }

    #[test]
    fn test_music_play_simple() {
        let s = parse(r#"music.play("theme.ogg")"#).unwrap();
        assert!(matches!(&s[0], Statement::MusicPlay { .. }));
    }

    #[test]
    fn test_byte_offset_to_location_basics() {
        let src = "hello\nworld\nfoo";
        assert_eq!(
            byte_offset_to_location(src, 0, 5),
            SourceLocation {
                line: 1,
                col: 1,
                len: 5
            }
        );
        assert_eq!(
            byte_offset_to_location(src, 6, 5),
            SourceLocation {
                line: 2,
                col: 1,
                len: 5
            }
        );
        assert_eq!(
            byte_offset_to_location(src, 12, 3),
            SourceLocation {
                line: 3,
                col: 1,
                len: 3
            }
        );
    }
}
