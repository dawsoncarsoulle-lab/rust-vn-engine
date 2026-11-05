use super::*;
#[test]
fn only_the_empty_list_denotes_an_absent_screen_child_tree() {
    assert_eq!(parse_screen_root(serde_json::json!([])).unwrap(), None);
    assert!(Component::parse(serde_json::json!([])).is_err());
    for invalid in [serde_json::json!(null),serde_json::json!([{}]),serde_json::json!(false),serde_json::json!(""),serde_json::json!({})] {
        assert!(parse_screen_root(invalid).is_err());
    }
    let object=serde_json::json!({"id":"actual","kind":"text","text":"hello"});
    assert_eq!(parse_screen_root(object.clone()).unwrap(),Some(Component::parse(object).unwrap()));
}
