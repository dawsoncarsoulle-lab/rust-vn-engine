use rvn_parser::{parse, Expr, Statement, TextSegment};

#[test]
fn quoted_keys_and_brackets_inside_interpolated_expressions() {
    let script = parse(r#""[quest[\"name]\"]]""#).unwrap();
    let Statement::Dialogue { text, .. } = &script[0] else {
        panic!()
    };
    assert_eq!(
        text.0,
        vec![TextSegment::Interp(Expr::Index {
            target: Box::new(Expr::Var("quest".into())),
            index: Box::new(Expr::Str("name]".into())),
        })]
    );
}

#[test]
fn expression_strings_and_dialogue_literals_are_unescaped_once() {
    let script = parse(
        r#"set key = "a\"b\\c"
"Path: C:\\new\\test, [[literal]]\n[values[\"a\\\"b\\\\c\"]]""#,
    )
    .unwrap();
    let Statement::SetVar { value, .. } = &script[0] else {
        panic!()
    };
    assert_eq!(value, &Expr::Str("a\"b\\c".into()));
    let Statement::Dialogue { text, .. } = &script[1] else {
        panic!()
    };
    assert_eq!(
        text.0[0],
        TextSegment::Lit("Path: C:\\new\\test, [literal]\n".into())
    );
    assert!(
        matches!(&text.0[1], TextSegment::Interp(Expr::Index { index, .. }) if **index == Expr::Str("a\"b\\c".into()))
    );
}

#[test]
fn interpolation_cannot_discard_trailing_code() {
    assert!(parse(r#""[1 set secret = 9]""#).is_err());
    assert!(parse(r#""[values[\"key\"]""#).is_err());
}
