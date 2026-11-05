use rvn_core::{
    eval::{EvalError, FunctionLibrary},
    random::RandomState,
    string_table::{StringTableAsset, StringTableEntry},
};
use rvn_parser::{
    localized_text::{DeferredText, StringTableReference, TextArgument},
    Expr, Statement, Value,
};
use std::collections::{BTreeMap, HashMap};

fn library(source: &str) -> FunctionLibrary {
    let mut library = FunctionLibrary::from_script(&rvn_parser::parse(source).unwrap()).unwrap();
    library
        .text_resources_mut()
        .publish(StringTableAsset {
            id: "asset:work".into(),
            namespace: "VN.Work".into(),
            entries: [(
                "payload".into(),
                StringTableEntry {metadata:Default::default(),
                    source: "X".repeat(400_000),
                    developer_notes: String::new(),
                },
            )]
            .into(),
        })
        .unwrap();
    library
}
fn expression(source: &str) -> Expr {
    let script = rvn_parser::parse(&format!("set answer={source}")).unwrap();
    let Statement::SetVar { value, .. } = &script[0] else {
        unreachable!()
    };
    value.clone()
}
const SCRIPT: &str = r#"function run() {
local count=0
local drawn=random(1,9)
while count<1000 {
    local message=__rvn_format_text("",{"unused":__rvn_string_table_text("asset:work","payload")})
    local converted=text_to_string(message)
    set count=count+1
}
return count
}"#;

#[test]
fn conversion_loop_counts_unrendered_table_arguments_and_preserves_failed_computation_rng() {
    let library = library(SCRIPT);
    let globals = HashMap::new();
    let mut random = RandomState::seeded(11);
    let result = library.eval_with_random(&expression("run()"), &globals, &mut random);
    assert!(
        matches!(result,Err(EvalError::ExecutionLimit{limit}) if limit.starts_with("1 000 000"))
    );
    assert_eq!(random, RandomState::seeded(11));
    assert!(globals.is_empty());
}

#[test]
fn interpolation_shares_hidden_table_work_through_nested_collections_before_committing_rng() {
    let library = library("");
    let text = DeferredText::Format {
        pattern: Box::new(DeferredText::invariant("")),
        arguments: BTreeMap::from([(
            "unused".into(),
            TextArgument::Text(Box::new(DeferredText::StringTable {
                reference: StringTableReference {
                    table: "asset:work".into(),
                    key: "payload".into(),
                },
            })),
        )]),
    };
    let values = HashMap::from([(
        "nested".into(),
        Value::List(vec![Value::Dict(BTreeMap::from([(
            "text".into(),
            Value::Text(Box::new(text)),
        )]))]),
    )]);
    let template =
        rvn_parser::parse_interpolated_str(&format!("[random(1,9)]{}", "[nested]".repeat(64)))
            .unwrap();
    let mut random = RandomState::seeded(12);
    let result = library.interpolate_with_locale(&template, &values, &mut random, None);
    assert!(
        matches!(result,Err(EvalError::ExecutionLimit{limit}) if limit.starts_with("1 000 000"))
    );
    assert_eq!(random, RandomState::seeded(12));
}

#[test]
fn unavailable_and_invalid_table_predicates_keep_distinct_contracts_at_all_host_entry_points() {
    let mut library = library(
        r#"function unavailable() { local drew=random(1,9) return __rvn_string_table_text("missing","key") }
function bad() { local drew=random(1,9) return __rvn_string_table_text("asset:oversized","key") }"#,
    );
    library.text_resources_mut().publish(StringTableAsset {
        id: "asset:oversized".into(), namespace: "VN.Work".into(),
        entries: BTreeMap::from([("key".into(), StringTableEntry {metadata:Default::default(), source: "X".repeat(1_048_577), developer_notes: String::new() })]),
    }).unwrap();
    let globals = HashMap::new();
    let mut once = RandomState::seeded(13);
    library.eval_with_random(&expression("random(1,9)"), &globals, &mut once).unwrap();
    for api in 0..4 {
        for (call, success) in [("unavailable()", true), ("bad()", false)] {
            let expr = expression(call); let mut random = RandomState::seeded(13);
            let result = match api {
                0 => library.eval_bool_with_random(&expr, &globals, &mut random),
                1 => library.eval_bool_scoped_with_random(&expr, &globals, &globals, &mut random),
                2 => library.eval_bool_with_locale(&expr, &globals, &mut random, None),
                _ => library.eval_bool_scoped_with_locale(&expr, &globals, &globals, &mut random, None),
            };
            if success { assert_eq!(result, Ok(true)); assert_eq!(random, once); }
            else { assert!(result.is_err()); assert_eq!(random, RandomState::seeded(13)); }
        }
    }
}

#[test]
fn final_host_predicate_shares_the_expression_budget_instead_of_starting_a_new_one() {
    let script = SCRIPT.replace("count<1000", "count<39").replace("return count", "return __rvn_format_text(\"\",{\"unused\":__rvn_string_table_text(\"asset:work\",\"payload\")})");
    let library = library(&script);
    let expr = expression("run()");
    let globals = HashMap::new();
    let mut random = RandomState::seeded(14);
    let mut budget = rvn_core::ui::CanvasBudget::default();
    assert!(matches!(
        library
            .eval_with_random_budgeted(&expr, &globals, &mut random, &mut budget)
            .unwrap(),
        Value::Text(_)
    ));
    random = RandomState::seeded(14);
    assert!(
        matches!(library.eval_bool_with_random(&expr, &globals, &mut random), Err(EvalError::ExecutionLimit { limit }) if limit.starts_with("1 000 000"))
    );
    assert_eq!(random, RandomState::seeded(14));
}
