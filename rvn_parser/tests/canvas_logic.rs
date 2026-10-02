use rvn_parser::{parse, validate_logic};

#[test]
fn literal_canvas_callback_checks_names_arity_and_transitive_purity() {
    for (declaration, callback, code) in [
        ("", "missing", "canvas-draw-function"),
        ("handler paint(event) {}", "paint", "canvas-draw-function"),
        (
            "function paint(state) {return []}",
            "paint",
            "canvas-draw-arity",
        ),
        (
            "function helper() {return random(0,1)} function paint(s,p,f){return [helper()]}",
            "paint",
            "canvas-draw-purity",
        ),
        (
            "function helper() {return mouse_x()} function paint(s,p,f){return [helper()]}",
            "paint",
            "canvas-draw-purity",
        ),
    ] {
        let script=parse(&format!("{declaration}\nscreen custom(){{return component(\"one\",\"canvas\",{{\"draw\":\"{callback}\"}},[])}}\nlabel start\nreturn")).unwrap();
        assert!(
            validate_logic(&script, false)
                .iter()
                .any(|error| error.code == code),
            "{declaration}"
        );
    }
    assert!(
        parse("function paint(s,p,f){ui.close(\"custom\") return []}").is_err(),
        "Narrative commands are rejected by the calculation grammar itself"
    );
}

#[test]
fn raw_dictionary_recursive_helpers_and_dynamic_references_are_supported() {
    let source = r#"function helper(s,p,f){if s["done"] {return []} return helper(dict_set(s,"done",true),p,f)}
function paint(s,p,f){return helper(s,p,f)}
screen custom(){return {"id":"one","kind":"canvas","draw":"paint"}}
screen dynamic(name){return component("dynamic","canvas",{"draw":name},[])}
label start
return"#;
    assert!(validate_logic(&parse(source).unwrap(), false).is_empty());
    let source = source.replace("\"draw\":\"paint\"", "\"draw\":\"external\"");
    assert!(validate_logic(&parse(&source).unwrap(), true).is_empty());
    assert!(validate_logic(&parse(&source).unwrap(), false)
        .iter()
        .any(|error| error.code == "canvas-draw-function"));
}
