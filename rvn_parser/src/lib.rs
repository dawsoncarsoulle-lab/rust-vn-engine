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

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Parse un fichier `.rvn` en résolvant récursivement les instructions `use`.
///
/// Les chemins sont résolus relativement au fichier qui contient le `use`.
/// Les cycles sont détectés et signalés clairement. Un fichier déjà chargé est
/// ignoré lors des inclusions suivantes afin d'éviter les doubles définitions.
pub fn parse_file_with_uses<P: AsRef<Path>>(path: P) -> Result<Script, String> {
    let mut stack = Vec::new();
    let mut loaded = HashSet::new();
    parse_file_with_uses_inner(path.as_ref(), &mut stack, &mut loaded)
}

fn parse_file_with_uses_inner(
    path: &Path,
    stack: &mut Vec<PathBuf>,
    loaded: &mut HashSet<PathBuf>,
) -> Result<Script, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|e| format!("fichier RVN introuvable `{}`: {e}", path.display()))?;

    if let Some(pos) = stack.iter().position(|p| p == &canonical) {
        let mut cycle: Vec<String> = stack[pos..]
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        cycle.push(canonical.display().to_string());
        return Err(format!("cycle de `use` détecté: {}", cycle.join(" -> ")));
    }

    if !loaded.insert(canonical.clone()) {
        return Ok(Vec::new());
    }

    stack.push(canonical.clone());
    let source = fs::read_to_string(&canonical)
        .map_err(|e| format!("impossible de lire `{}`: {e}", canonical.display()))?;
    let script = parse(&source)
        .map_err(|e| format!("Erreur de parsing dans `{}`:\n{}", canonical.display(), e))?;
    let base_dir = canonical.parent().unwrap_or_else(|| Path::new("."));

    let mut resolved = Vec::new();
    for stmt in script {
        match stmt {
            Statement::Use { paths } => {
                for use_path in paths {
                    let targets = expand_use_path(base_dir, &use_path)?;
                    for target in targets {
                        let mut nested = parse_file_with_uses_inner(&target, stack, loaded)?;
                        resolved.append(&mut nested);
                    }
                }
            }
            other => resolved.push(other),
        }
    }

    stack.pop();
    Ok(resolved)
}

fn expand_use_path(base_dir: &Path, raw: &str) -> Result<Vec<PathBuf>, String> {
    if raw.ends_with("/*") || raw.ends_with("/*.rvn") {
        let dir_part = raw
            .strip_suffix("/*.rvn")
            .or_else(|| raw.strip_suffix("/*"))
            .unwrap_or(raw);
        let dir = base_dir.join(dir_part);
        let mut files = Vec::new();
        let entries = fs::read_dir(&dir).map_err(|e| {
            format!(
                "impossible de lire le dossier `use` `{}`: {e}",
                dir.display()
            )
        })?;
        for entry in entries {
            let entry =
                entry.map_err(|e| format!("erreur de lecture dans `{}`: {e}", dir.display()))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("rvn") {
                files.push(path);
            }
        }
        files.sort();
        if files.is_empty() {
            return Err(format!(
                "aucun fichier `.rvn` trouvé pour le `use` wildcard `{}`",
                dir.display()
            ));
        }
        return Ok(files);
    }

    Ok(vec![base_dir.join(raw)])
}

// ─── TESTS ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
    #[test]
    fn test_use_single_file() {
        let s = parse(r#"use "chapitres/intro.rvn""#).unwrap();
        let Statement::Use { paths } = &s[0] else {
            panic!()
        };
        assert_eq!(paths, &vec!["chapitres/intro.rvn".to_string()]);
    }

    #[test]
    fn test_use_group() {
        let s = parse(r#"use { "a.rvn", "b.rvn", }"#).unwrap();
        let Statement::Use { paths } = &s[0] else {
            panic!()
        };
        assert_eq!(paths, &vec!["a.rvn".to_string(), "b.rvn".to_string()]);
    }
    #[test]
    fn test_sprite_animate_with_params() {
        let s =
            parse(r#"eileen.animate("bounce", loop: true, duration: 0.5, height: 18)"#).unwrap();
        let Statement::SpriteAnimate {
            character_id,
            animation,
            params,
        } = &s[0]
        else {
            panic!("expected SpriteAnimate");
        };
        assert_eq!(character_id, "eileen");
        assert_eq!(animation, "bounce");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_sprite_stop_animation() {
        let s = parse("eileen.stop_animation()").unwrap();
        assert!(matches!(
            &s[0],
            Statement::SpriteStopAnimation { character_id } if character_id == "eileen"
        ));
    }
}
