use rvn_parser::{is_binding_name, parse, validate_logic};

#[test]
fn ui_names_and_arities_are_checked_without_running_handlers() {
    let script = parse(
        r#"
screen form(title) { return component("root","column",{},[]) }
handler click(event) { ui.open("form",[],true,1) }
handler invalid(a,b) {}
label start
ui.open("missing",[],true,1)
ui.close("form")
ui.focus("form","root")
return
"#,
    )
    .unwrap();
    let diagnostics = validate_logic(&script, false);
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "screen-arity" && d.name == "form"));
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "unknown-screen" && d.name == "missing"));
    assert!(diagnostics
        .iter()
        .any(|d| d.code == "handler-arity" && d.name == "invalid"));
    assert!(diagnostics
        .iter()
        .all(|d| !d.english.is_empty() && !d.french.is_empty()));
    assert!(!validate_logic(&script, true)
        .iter()
        .any(|d| d.code == "unknown-screen"));
}

#[test]
fn declarations_do_not_masquerade_as_calculation_functions() {
    let script = parse(
        r#"screen form() { return component("root","text",{},[]) }
handler click(event) {}
label start
set value = form()
set other = click(1)
return"#,
    )
    .unwrap();
    assert_eq!(
        validate_logic(&script, false)
            .iter()
            .filter(|d| d.code == "unknown-function")
            .count(),
        2
    );
    for valid in ["title", "player_name", "item_1"] {
        assert!(is_binding_name(valid));
    }
    for invalid in [
        "",
        "set",
        "local",
        "screen",
        "__rvn_secret",
        "player.name",
        "a b",
    ] {
        assert!(!is_binding_name(invalid), "{invalid}");
    }
}
