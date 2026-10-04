//! Token-backed source locations. Comments and ordinary string contents are
//! never treated as code, and LSP columns are UTF-16 rather than UTF-8 bytes.
use logos::Logos;
use rvn_parser::lexer::Token;
use tower_lsp::lsp_types::{Position, Range};

#[derive(Debug, Clone)]
pub struct Site {
    pub name: String,
    pub role: &'static str,
    pub range: Range,
    pub arity: Option<usize>,
}

pub fn position(text: &str, offset: usize) -> Position {
    let offset = offset.min(text.len());
    let prefix = &text[..offset];
    Position::new(
        prefix.bytes().filter(|byte| *byte == b'\n').count() as u32,
        prefix
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .encode_utf16()
            .count() as u32,
    )
}

pub fn byte_column(line: &str, column: u32) -> usize {
    let mut units = 0;
    for (offset, ch) in line.char_indices() {
        if units + ch.len_utf16() > column as usize {
            return offset;
        }
        units += ch.len_utf16();
        if units == column as usize {
            return offset + ch.len_utf8();
        }
    }
    line.len()
}

pub fn scan(text: &str) -> Vec<Site> {
    let tokens: Vec<_> = Token::lexer(text)
        .spanned()
        .filter_map(|(token, span)| {
            token
                .ok()
                .filter(|token| !matches!(token, Token::Newline))
                .map(|token| (token, span))
        })
        .collect();
    let mut sites = Vec::new();
    for (i, (token, span)) in tokens.iter().enumerate() {
        let Token::Ident(name) = token else {
            continue;
        };
        let before = i
            .checked_sub(1)
            .and_then(|i| tokens.get(i))
            .map(|(token, _)| token);
        let after = tokens.get(i + 1).map(|(token, _)| token);
        let role = match before {
            Some(Token::Function) => Some("function-declaration"),
            Some(Token::Screen) => Some("screen-declaration"),
            Some(Token::Handler) => Some("handler-declaration"),
            Some(Token::Dot) => None,
            _ if matches!(after, Some(Token::ParenOpen)) => Some("function"),
            _ => None,
        };
        if let Some(role) = role {
            sites.push(Site {
                name: (*name).into(),
                role,
                range: Range::new(position(text, span.start), position(text, span.end)),
                arity: if role == "function" {
                    arguments(&tokens, i + 1).map(|items| items.len())
                } else {
                    None
                },
            });
        }
        if *name == "component" {
            if let Some(args) = arguments(&tokens, i + 1) {
                if args.len() == 4 && matches!(tokens[args[1]].0, Token::String("\"canvas\"")) {
                    if matches!(tokens[args[2]].0, Token::BraceOpen) {
                        if let Some(site) = draw_dictionary(text, &tokens, args[2], false) {
                            sites.push(site);
                        }
                    } else {
                        let span = &tokens[args[2]].1;
                        sites.push(Site {
                            name: String::new(),
                            role: "dynamic-draw",
                            range: Range::new(position(text, span.start), position(text, span.end)),
                            arity: None,
                        });
                    }
                }
            }
        }
        if *name == "ui" && matches!(after, Some(Token::Dot)) {
            if matches!(
                tokens.get(i + 2).map(|(token, _)| token),
                Some(Token::Ident("open" | "open_story" | "close" | "focus" | "set_state"))
            ) && matches!(
                tokens.get(i + 3).map(|(token, _)| token),
                Some(Token::ParenOpen)
            ) {
                if let Some((Token::String(raw), span)) = tokens.get(i + 4) {
                    // Escaped identities are valid at runtime, but cannot be
                    // renamed as plain tokens without an escaping-aware edit.
                    let name = &raw[1..raw.len() - 1];
                    if rvn_parser::is_binding_name(name) {
                        let arity = if matches!(
                            tokens.get(i + 2).map(|(token, _)| token),
                            Some(Token::Ident("open" | "open_story"))
                        ) {
                            arguments(&tokens, i + 3)
                                .and_then(|args| args.get(1).copied())
                                .and_then(|index| arguments(&tokens, index).map(|args| args.len()))
                        } else {
                            None
                        };
                        sites.push(Site {
                            name: name.into(),
                            role: "screen",
                            range: Range::new(
                                position(text, span.start + 1),
                                position(text, span.end - 1),
                            ),
                            arity,
                        });
                    }
                }
            }
        }
    }
    for (i, (token, _)) in tokens.iter().enumerate() {
        if matches!(token, Token::BraceOpen) {
            if let Some(site) = draw_dictionary(text, &tokens, i, true) {
                sites.push(site);
            }
        }
    }
    sites
}

fn draw_dictionary(
    text: &str,
    tokens: &[(Token<'_>, std::ops::Range<usize>)],
    open: usize,
    require_kind: bool,
) -> Option<Site> {
    let mut depth = 0;
    let mut canvas = !require_kind;
    let mut draw = None;
    for i in open..tokens.len() {
        match tokens[i].0 {
            Token::BraceOpen | Token::ParenOpen | Token::BracketOpen => depth += 1,
            Token::BraceClose | Token::ParenClose | Token::BracketClose => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        if depth != 1
            || !matches!(
                tokens.get(i + 1).map(|(token, _)| token),
                Some(Token::Colon)
            )
        {
            continue;
        }
        if matches!(tokens[i].0, Token::String("\"kind\""))
            && matches!(
                tokens.get(i + 2).map(|(token, _)| token),
                Some(Token::String("\"canvas\""))
            )
        {
            canvas = true;
        }
        if matches!(tokens[i].0, Token::String("\"draw\"")) {
            draw = tokens.get(i + 2);
        }
    }
    if !canvas {
        return None;
    }
    let (token, span) = draw?;
    if let Token::String(raw) = token {
        let name = &raw[1..raw.len() - 1];
        if rvn_parser::is_binding_name(name) {
            return Some(Site {
                name: name.into(),
                role: "function",
                range: Range::new(position(text, span.start + 1), position(text, span.end - 1)),
                arity: Some(3),
            });
        }
        if name.is_empty() {
            return None;
        }
    }
    // A calculated or escaped callback cannot receive a partial rename.
    Some(Site {
        name: String::new(),
        role: "dynamic-draw",
        range: Range::new(position(text, span.start), position(text, span.end)),
        arity: None,
    })
}

fn arguments(tokens: &[(Token<'_>, std::ops::Range<usize>)], open: usize) -> Option<Vec<usize>> {
    if !matches!(tokens.get(open)?.0, Token::ParenOpen | Token::BracketOpen) {
        return None;
    }
    let mut depth = 1;
    let mut starts = Vec::new();
    let mut next = true;
    for (index, (token, _)) in tokens.iter().enumerate().skip(open + 1) {
        match token {
            Token::ParenClose | Token::BracketClose | Token::BraceClose => {
                depth -= 1;
                if depth == 0 {
                    return Some(starts);
                }
            }
            Token::ParenOpen | Token::BracketOpen | Token::BraceOpen => {
                if depth == 1 && next {
                    starts.push(index);
                    next = false;
                }
                depth += 1;
            }
            Token::Comma if depth == 1 => next = true,
            _ if depth == 1 && next => {
                starts.push(index);
                next = false;
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexes_real_calls_and_screen_references_without_comment_or_string_matches() {
        let source="// fake(1)\nfunction value(n) { return n }\nscreen inventory() { return component(\"x\", \"text\", {}, []) }\nlabel start\nset x = value(1)\nui.open(\"inventory\", [], true, 1)\n\"not_a_call(2)\"";
        let sites = scan(source);
        assert_eq!(sites.iter().filter(|site| site.name == "value").count(), 2);
        assert_eq!(
            sites.iter().filter(|site| site.name == "inventory").count(),
            2
        );
        assert!(!sites
            .iter()
            .any(|site| site.name == "fake" || site.name == "not_a_call"));
    }
    #[test]
    fn utf16_columns_do_not_split_unicode() {
        let source = "\"É😀\" value(1)";
        let site = scan(source).pop().unwrap();
        assert_eq!(site.range.start.character, 6);
        assert_eq!(byte_column(source, 6), 9);
        assert_eq!(byte_column("É😀", 2), 2);
    }
    #[test]
    fn custom_draw_and_state_screen_references_are_navigable_without_scanning_other_text() {
        let source = r#"function card(state,props,frame){return []}
screen board(){return component("root","canvas",{"draw":"card","props":{"draw":"unrelated"}},[])}
handler change(event){ui.set_state("board","root",{})}
screen raw(){return {"id":"raw","kind":"canvas","draw":"card"}}
"#;
        let sites = scan(source);
        assert_eq!(sites.iter().filter(|site| site.name == "card").count(), 3);
        assert!(sites
            .iter()
            .any(|site| site.name == "card" && site.arity == Some(3)));
        assert_eq!(sites.iter().filter(|site| site.name == "board").count(), 2);
        assert!(!sites.iter().any(|site| site.name == "unrelated"));
    }
}
