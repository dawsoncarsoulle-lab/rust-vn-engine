use rvn_core::{
    eval::FunctionLibrary,
    locale::{LocaleManager, LocaleTable},
    localized_text::{render_text_with_resources, source_expression, VnTextScalars},
    random::RandomState,
    save::{SaveData, STRING_TABLE_SAVE_FORMAT_VERSION},
    string_table::{
        StringTableAsset, StringTableEntry, StringTableReference, StringTableResources,
    },
    Engine, Interaction, TerminalRenderer,
};
use rvn_parser::{
    localized_text::{DeferredText, TextArgument, TextIdentity},
    Expr, Statement, Value,
};
use std::collections::{BTreeMap, HashMap};

fn reference(key: &str) -> DeferredText {
    DeferredText::StringTable {
        reference: StringTableReference {
            table: "asset:greetings".into(),
            key: key.into(),
        },
    }
}
fn asset(pattern: &str) -> StringTableAsset {
    StringTableAsset {
        id: "asset:greetings".into(),
        namespace: "VN".into(),
        entries: BTreeMap::from([
            (
                "welcome".into(),
                StringTableEntry {metadata:Default::default(),
                    source: pattern.into(),
                    developer_notes: "Table context".into(),
                },
            ),
            (
                "name".into(),
                StringTableEntry {metadata:Default::default(),
                    source: "Visitor".into(),
                    developer_notes: String::new(),
                },
            ),
            (
                "empty".into(),
                StringTableEntry {metadata:Default::default(),
                    source: String::new(),
                    developer_notes: String::new(),
                },
            ),
        ]),
    }
}
fn resources(pattern: &str) -> StringTableResources {
    let mut resources = StringTableResources::default();
    resources.publish(asset(pattern)).unwrap();
    resources
}
fn locale() -> LocaleManager {
    let key = |key: &str| {
        TextIdentity {
            owner: "string-table:asset:greetings".into(),
            namespace: "VN".into(),
            key: key.into(),
        }
        .locale_key()
    };
    LocaleManager::from_tables(
        "unused",
        "en",
        "en",
        vec!["en".into(), "fr".into()],
        HashMap::from([(
            "fr".into(),
            LocaleTable {
                lang: "fr".into(),
                strings: HashMap::from([
                    (key("welcome"), "Bienvenue {Name}!".into()),
                    (key("name"), "Éloïse".into()),
                ]),
            },
        )]),
    )
}
fn expression(source: &str) -> Expr {
    let statements = rvn_parser::parse(&format!("set answer={source}")).unwrap();
    let Statement::SetVar { value, .. } = &statements[0] else {
        unreachable!()
    };
    value.clone()
}

#[test]
fn retained_reference_formats_nested_table_text_in_current_resources_and_culture() {
    let text = DeferredText::Format {
        pattern: Box::new(reference("welcome")),
        arguments: BTreeMap::from([(
            "Name".into(),
            TextArgument::Text(Box::new(reference("name"))),
        )]),
    };
    let encoded = serde_json::to_string(&text).unwrap();
    assert!(!encoded.contains("Visitor"));
    assert!(!encoded.contains("Table context"));
    let restored: DeferredText = serde_json::from_str(&encoded).unwrap();
    let mut host = resources("Hello {Name}!");
    let mut culture = locale();
    let display = |host: &StringTableResources, culture: &LocaleManager| {
        render_text_with_resources(&restored, Some(culture), &VnTextScalars, Some(host)).unwrap()
    };
    assert_eq!(display(&host, &culture), "Hello Visitor!");
    culture.set_language("fr").unwrap();
    assert_eq!(display(&host, &culture), "Bienvenue Éloïse!");
    culture.set_language("en").unwrap();
    host.publish(asset("Good evening {Name}.")).unwrap();
    assert_eq!(display(&host, &culture), "Good evening Visitor.");
    assert_eq!(serde_json::to_string(&text).unwrap(), encoded);
    assert!(restored.source_display().is_err());
    assert!(rvn_core::localized_text::render_text(&restored, None, &VnTextScalars).is_err());
}

#[test]
fn rvn_reference_and_nested_format_round_trip_without_resource_lookup_or_rng() {
    let text = DeferredText::Format {
        pattern: Box::new(DeferredText::invariant("{Name}")),
        arguments: BTreeMap::from([(
            "Name".into(),
            TextArgument::Text(Box::new(reference("name"))),
        )]),
    };
    let library = FunctionLibrary::default();
    let mut random = RandomState::seeded(71);
    for text in [reference("welcome"), text] {
        let source = source_expression(&text).unwrap();
        let script = rvn_parser::parse(&format!("set text={source}")).unwrap();
        assert!(rvn_parser::validate_logic(&script, false).is_empty());
        assert_eq!(
            library
                .eval_with_random(&expression(&source), &HashMap::new(), &mut random)
                .unwrap(),
            Value::Text(Box::new(text))
        );
    }
    assert_eq!(random, RandomState::seeded(71));
    for source in [
        "__rvn_string_table_text(\"\",\"key\")",
        "__rvn_string_table_text(\"table\",\"a b\")",
        "__rvn_string_table_text(1,\"key\")",
        "__rvn_string_table_text(\"table\")",
    ] {
        assert!(
            library.eval(&expression(source), &HashMap::new()).is_err(),
            "{source}"
        );
    }
}

#[test]
fn evaluation_conversion_interpolation_and_truthiness_use_explicit_resources() {
    let mut library = FunctionLibrary::default();
    library
        .text_resources_mut()
        .publish(asset("Hello {Name}!"))
        .unwrap();
    let values = HashMap::from([
        ("name".into(), Value::Text(Box::new(reference("name")))),
        ("empty".into(), Value::Text(Box::new(reference("empty")))),
    ]);
    let mut random = RandomState::seeded(7);
    assert_eq!(
        library
            .eval_with_random(&expression("text_to_string(name)"), &values, &mut random)
            .unwrap(),
        Value::Str("Visitor".into())
    );
    assert!(library.eval_bool(&expression("name"), &values).unwrap());
    assert!(!library.eval_bool(&expression("empty"), &values).unwrap());
    assert_eq!(
        library
            .interpolate_with_locale(
                &rvn_parser::parse_interpolated_str("[name]!").unwrap(),
                &values,
                &mut random,
                None
            )
            .unwrap(),
        "Visitor!"
    );
    let old_snapshot = library.clone();
    library
        .text_resources_mut()
        .unload("asset:greetings")
        .unwrap();
    assert!(library.eval_bool(&expression("name"), &values).unwrap());
    assert_eq!(library.eval(&expression("text_to_string(name)"), &values).unwrap(), Value::Str(rvn_core::string_table::MISSING_STRING_TABLE_ENTRY.into()));
    assert_eq!(
        old_snapshot
            .eval(&expression("text_to_string(name)"), &values)
            .unwrap(),
        Value::Str("Visitor".into())
    );
    assert_eq!(random, RandomState::seeded(7));
}

const INIT: &str = r#"init {
set pattern=__rvn_string_table_text("asset:greetings","welcome")
set person=__rvn_string_table_text("asset:greetings","name")
set message=__rvn_format_text(pattern,{"Name":person})
set initialized=text_to_string(person)
}"#;

#[test]
fn host_resources_are_installed_before_init_and_retained_for_a_fresh_game() {
    for header in ["label start", "scene label start"] {
        let source = format!("{INIT}\n{header}\n\"[message]\"");
        let unavailable = Engine::new(rvn_parser::parse(&source).unwrap(), TerminalRenderer, 8).unwrap();
        assert_eq!(unavailable.state.vars["initialized"], Value::Str(rvn_core::string_table::MISSING_STRING_TABLE_ENTRY.into()));
        let game = Engine::new_with_text_resources(
            rvn_parser::parse(&source).unwrap(),
            TerminalRenderer,
            8,
            resources("Hello {Name}!"),
        )
        .unwrap();
        assert_eq!(game.state.vars["initialized"], Value::Str("Visitor".into()));
        let mut fresh = game.fresh(TerminalRenderer, 8).unwrap();
        assert!(
            matches!(fresh.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Hello Visitor!")
        );
        assert_eq!(
            fresh.text_resources().revision_stamp(),
            game.text_resources().revision_stamp()
        );
        fresh
            .text_resources_mut()
            .publish(asset("Goodbye {Name}."))
            .unwrap();
        assert_ne!(
            fresh.text_resources().revision_stamp(),
            game.text_resources().revision_stamp()
        );
        assert!(
            matches!(fresh.displayed_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Goodbye Visitor.")
        );
    }
}

#[test]
fn save13_restores_table_addresses_and_re_resolves_dialogue_history_without_rng() {
    for header in ["label start", "scene label start"] {
        let source = format!("{INIT}\n{header}\n\"[message]\"\n\"Second\"");
        let mut game = Engine::new_with_text_resources(
            rvn_parser::parse(&source).unwrap(),
            TerminalRenderer,
            8,
            resources("Hello {Name}!"),
        )
        .unwrap();
        game.locale = Some(locale());
        game.step_until_interaction().unwrap();
        let saved =
            SaveData::try_from_state(&game.state, 1, "text".into(), "text.rvn".into()).unwrap();
        assert_eq!(saved.format_version, STRING_TABLE_SAVE_FORMAT_VERSION);
        let encoded = serde_json::to_string(&saved).unwrap();
        assert!(encoded.contains("asset:greetings"));
        assert!(!encoded.contains("Table context"));
        assert!(!encoded.contains("Hello {Name}!"));
        let random = (game.state.random, game.state.display_random);
        game.locale.as_mut().unwrap().set_language("fr").unwrap();
        assert!(
            matches!(game.displayed_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Bienvenue Éloïse!")
        );
        game.advance_dialogue().unwrap();
        game.step_until_interaction().unwrap();
        assert!(game
            .localized_dialogue_history()
            .unwrap()
            .iter()
            .any(|(_, text)| text == "Bienvenue Éloïse!"));
        game.load_data(serde_json::from_str(&encoded).unwrap())
            .unwrap();
        assert_eq!((game.state.random, game.state.display_random), random);
        let before = serde_json::to_value(&game.state).unwrap();
        let mut old: SaveData = serde_json::from_str(&encoded).unwrap();
        old.format_version = 12;
        assert!(game.load_data(old).is_err());
        assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
        game.locale.as_mut().unwrap().set_language("en").unwrap();
        game.text_resources_mut()
            .publish(asset("Welcome back {Name}."))
            .unwrap();
        game.load_data(serde_json::from_str(&encoded).unwrap())
            .unwrap();
        assert!(
            matches!(game.displayed_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text == "Welcome back Visitor.")
        );
        assert_eq!((game.state.random, game.state.display_random), random);
    }
}

#[test]
fn save13_gate_detects_nested_references_in_all_existing_portable_contexts() {
    let mut game = Engine::new(
        rvn_parser::parse("scene label start\n\"Waiting\"").unwrap(),
        TerminalRenderer,
        8,
    )
    .unwrap();
    game.step_until_interaction().unwrap();
    let value = Value::List(vec![Value::Text(Box::new(DeferredText::Format {
        pattern: Box::new(DeferredText::invariant("{Name}")),
        arguments: BTreeMap::from([(
            "Name".into(),
            TextArgument::Text(Box::new(reference("name"))),
        )]),
    }))]);
    let screen = || rvn_core::ui::ScreenInstance {
        widget_class:None,
        name: "fixture".into(),
        host_role: None,
        host_root: false,
        arguments: Vec::new(),
        modal: false,
        layer: 0,
        order: 0,
        focus: None,
        owner_properties:Default::default(),owner_focus_request:None,
        values: BTreeMap::new(),
        random: Default::default(),
        canvas_states: BTreeMap::new(),
        consumer_frames: BTreeMap::new(),
        consumer_projection: None,
        standard_macro_frame: None,
    };
    for location in 0..19 {
        let mut state = game.state.clone();
        if state.last_dialogue.is_none() {
            state.last_dialogue = Some(rvn_core::types::DialogueSnapshot {
                consumer_dialogue: None,
                pc: 0,
                vars: HashMap::new(),
                lexical_globals: None,
                random: Default::default(),
            });
        }
        let scene = state.scene_execution.as_mut().unwrap();
        if location >= 13 {
            scene.frames.push(rvn_core::scene::NarrativeFrame {
                kind: rvn_core::scene::FrameKind::SceneCall,
                continuation: scene.live_position.clone(),
                owner: None,
                called_activation: None,
                parked: scene.active.clone(),
                pre_scene_temporaries: Some(HashMap::new()),
                parked_native_loops_v1:None,
                pre_scene_resources: Some(Default::default()),
            });
        }
        match location {
            0 => {
                state.vars.insert("nested".into(), value.clone());
            }
            1 => {
                state.ui.screens.push(screen());
                state.ui.screens[0].arguments.push(value.clone());
            }
            2 => {
                state.ui.screens.push(screen());
                state.ui.screens[0].values.insert("x".into(), value.clone());
            }
            3 => {
                state.ui.screens.push(screen());
                state.ui.screens[0].canvas_states.insert(
                    "x".into(),
                    rvn_core::ui::CanvasState {
                        state: value.clone(),
                        elapsed: 0.0,
                    },
                );
            }
            4 => {
                state
                    .last_dialogue
                    .as_mut()
                    .unwrap()
                    .vars
                    .insert("x".into(), value.clone());
            }
            5 => {
                state.last_dialogue.as_mut().unwrap().lexical_globals =
                    Some(HashMap::from([("x".into(), value.clone())]));
            }
            6 => {
                scene
                    .pre_scene_temporaries
                    .insert("x".into(), value.clone());
            }
            7 => {
                scene
                    .active
                    .as_mut()
                    .unwrap()
                    .locals
                    .insert("x".into(), value.clone());
            }
            8 => {
                scene
                    .active
                    .as_mut()
                    .unwrap()
                    .temporaries
                    .insert("x".into(), value.clone());
            }
            9 => {
                scene
                    .display
                    .as_mut()
                    .unwrap()
                    .variables
                    .insert("x".into(), value.clone());
            }
            10 => {
                scene
                    .display
                    .as_mut()
                    .unwrap()
                    .lexical_globals
                    .insert("x".into(), value.clone());
            }
            11 => {
                scene.game_tasks.suspended_ui.screens.push(screen());
                scene.game_tasks.suspended_ui.screens[0]
                    .arguments
                    .push(value.clone());
            }
            12 => {
                scene
                    .active
                    .as_mut()
                    .unwrap()
                    .tasks
                    .suspended_ui
                    .screens
                    .push(screen());
                scene.active.as_mut().unwrap().tasks.suspended_ui.screens[0]
                    .values
                    .insert("x".into(), value.clone());
            }
            13 => {
                scene.frames[0]
                    .parked
                    .as_mut()
                    .unwrap()
                    .locals
                    .insert("x".into(), value.clone());
            }
            14 => {
                scene.frames[0]
                    .parked
                    .as_mut()
                    .unwrap()
                    .temporaries
                    .insert("x".into(), value.clone());
            }
            15 => {
                scene.frames[0]
                    .parked
                    .as_mut()
                    .unwrap()
                    .tasks
                    .suspended_ui
                    .screens
                    .push(screen());
                scene.frames[0]
                    .parked
                    .as_mut()
                    .unwrap()
                    .tasks
                    .suspended_ui
                    .screens[0]
                    .arguments
                    .push(value.clone());
            }
            16 => {
                scene.frames[0]
                    .pre_scene_temporaries
                    .as_mut()
                    .unwrap()
                    .insert("x".into(), value.clone());
            }
            17 => {
                scene.frames[0]
                    .pre_scene_resources
                    .as_mut()
                    .unwrap()
                    .suspended_ui
                    .screens
                    .push(screen());
                scene.frames[0]
                    .pre_scene_resources
                    .as_mut()
                    .unwrap()
                    .suspended_ui
                    .screens[0]
                    .arguments
                    .push(value.clone());
            }
            _ => {
                scene.frames[0]
                    .pre_scene_resources
                    .as_mut()
                    .unwrap()
                    .suspended_ui
                    .screens
                    .push(screen());
                scene.frames[0]
                    .pre_scene_resources
                    .as_mut()
                    .unwrap()
                    .suspended_ui
                    .screens[0]
                    .values
                    .insert("x".into(), value.clone());
            }
        }
        // Some intentionally constructed payloads are not valid UI components;
        // this assertion specifically exercises the typed format barrier.
        let mut save = SaveData::from_state(&state, 1, String::new(), String::new());
        assert_eq!(save.format_version, 13, "location {location}");
        save.validate_checkpoint_shape().unwrap();
        save.format_version = 12;
        assert!(
            save.validate_checkpoint_shape().is_err(),
            "location {location}"
        );
    }
    let mut plain = game.state.clone();
    plain.vars.insert(
        "ordinary".into(),
        Value::Dict(BTreeMap::from([(
            "StringTable".into(),
            Value::Str("ordinary".into()),
        )])),
    );
    assert_eq!(
        SaveData::from_state(&plain, 1, String::new(), String::new()).format_version,
        10
    );
}

#[test]
fn retained_reference_unavailable_projection_is_visible_and_size_errors_stay_bounded() {
    let text = reference("welcome");
    let mut host = resources("Hello");
    assert_eq!(
        render_text_with_resources(&text, None, &VnTextScalars, Some(&host)).unwrap(),
        "Hello"
    );
    let pending = host.begin_load("asset:greetings").unwrap();
    assert!(
        render_text_with_resources(&text, None, &VnTextScalars, Some(&host))
            .unwrap() == rvn_core::string_table::MISSING_STRING_TABLE_ENTRY
    );
    host.fail_load(&pending, "fixture failure".into()).unwrap();
    assert!(
        render_text_with_resources(&text, None, &VnTextScalars, Some(&host))
            .unwrap() == rvn_core::string_table::MISSING_STRING_TABLE_ENTRY
    );
    host.publish(asset(&"x".repeat(1_048_577))).unwrap();
    assert!(render_text_with_resources(&text, None, &VnTextScalars, Some(&host)).is_err());
    host.unload("asset:greetings").unwrap();
    assert!(
        render_text_with_resources(&text, None, &VnTextScalars, Some(&host))
            .unwrap() == rvn_core::string_table::MISSING_STRING_TABLE_ENTRY
    );
}

#[test]
fn equal_numeric_revisions_of_divergent_snapshots_have_distinct_cache_identities() {
    let initial = resources("Hello");
    let mut left = initial.clone();
    let mut right = initial.clone();
    assert_eq!(left.revision_stamp(), right.revision_stamp());
    left.publish(asset("Left")).unwrap();
    right.publish(asset("Right")).unwrap();
    assert_eq!(left.revision(), right.revision());
    assert_ne!(left.revision_stamp(), right.revision_stamp());
    let before = right.revision_stamp();
    assert!(!right.unload("missing").unwrap());
    assert_eq!(right.revision_stamp(), before);
}


mod qualified_name_history_0250 {
use rvn_core::{eval::FunctionLibrary,localized_text::{construct,inspect_table_identity,render_text_with_resources,source_expression,VnTextScalars},save::{SaveData,NAME_STRING_TABLE_SAVE_FORMAT_VERSION,STRING_TABLE_SAVE_FORMAT_VERSION},string_table::{StringTableAsset,StringTableEntry,StringTableResources,StringTableReference,MISSING_STRING_TABLE_ENTRY},Engine,Interaction,LoadCompatibility,TerminalRenderer};
use rvn_parser::{Name,Value,localized_text::DeferredText};
use std::collections::BTreeMap;

fn name(id:&str)->Value {Value::Name(Name::new(id).unwrap())}
fn modern(id:&str,key:&str)->Value {construct("__rvn_string_table_text_name",&[name(id),Value::Str(key.into())]).unwrap()}
fn legacy(id:&str,key:&str)->Value {construct("__rvn_string_table_text",&[Value::Str(id.into()),Value::Str(key.into())]).unwrap()}
fn text(value:&Value)->&DeferredText {let Value::Text(text)=value else{panic!("Text expected")};text}
fn table(id:&str,source:&str)->StringTableAsset {StringTableAsset{id:id.into(),namespace:"VN".into(),entries:BTreeMap::from([("key".into(),StringTableEntry{source:source.into(),developer_notes:String::new(),metadata:Default::default()})])}}
fn project(value:&Value,resources:&StringTableResources)->Result<String,String> {render_text_with_resources(text(value),None,&VnTextScalars,Some(resources))}

#[test]
fn name_history_is_additive_and_late_publication_uses_name_identity_without_rewriting_saved_address() {
    let named=modern("ASSET:LATE0250","key");let exact=legacy("ASSET:LATE0250","key");
    let snapshot_named=serde_json::to_value(&named).unwrap();let snapshot_exact=serde_json::to_value(&exact).unwrap();
    assert_eq!(snapshot_named["Text"]["kind"],"name_string_table");assert_eq!(snapshot_exact["Text"]["kind"],"string_table");
    let mut resources=StringTableResources::default();assert_eq!(project(&named,&resources).unwrap(),MISSING_STRING_TABLE_ENTRY);
    resources.publish(table("asset:Late0250","Published later")).unwrap();let before=resources.clone();
    assert_eq!(project(&named,&resources).unwrap(),"Published later");assert_eq!(project(&exact,&resources).unwrap(),MISSING_STRING_TABLE_ENTRY);
    assert_eq!(resources,before);assert_eq!(serde_json::to_value(&named).unwrap(),snapshot_named);assert_eq!(serde_json::to_value(&exact).unwrap(),snapshot_exact);
    assert_eq!(named,modern("asset:late0250","key"));assert_ne!(exact,legacy("asset:late0250","key"));
    let expression=source_expression(text(&named)).unwrap();assert!(expression.contains("__rvn_string_table_text_name(__rvn_literal_name(\"ASSET:LATE0250\")"));
    let parsed=rvn_parser::parse(&format!("set probe={expression}")).unwrap();let rvn_parser::Statement::SetVar{value,..}=&parsed[0] else{panic!("assignment expected")};
    assert_eq!(FunctionLibrary::default().eval(value,&Default::default()).unwrap(),named);
}

#[test]
fn inspection_reads_only_top_level_history_and_not_availability_or_nested_format_references() {
    let named=modern("missing","key");
    assert_eq!(inspect_table_identity("__rvn_text_table_valid",&[named.clone()]).unwrap(),Value::Bool(true));
    assert_eq!(inspect_table_identity("__rvn_text_table_id",&[named.clone()]).unwrap(),name("missing"));
    assert_eq!(inspect_table_identity("__rvn_text_table_key",&[named.clone()]).unwrap(),Value::Str("key".into()));
    let format=construct("__rvn_format_text",&[Value::Str("{entry}".into()),Value::Dict(BTreeMap::from([("entry".into(),named)]))]).unwrap();
    assert!(text(&format).has_string_table().unwrap());assert!(text(&format).has_name_string_table().unwrap());
    for ordinary in [format,construct("__rvn_invariant_text",&[Value::Str("Text".into())]).unwrap(),Value::Str("legacy alias".into())] {
        assert_eq!(inspect_table_identity("__rvn_text_table_valid",&[ordinary.clone()]).unwrap(),Value::Bool(false));
        assert_eq!(inspect_table_identity("__rvn_text_table_id",&[ordinary.clone()]).unwrap(),name("None"));
        assert_eq!(inspect_table_identity("__rvn_text_table_key",&[ordinary]).unwrap(),Value::Str(String::new()));
    }
    assert!(inspect_table_identity("__rvn_text_table_valid",&[Value::Int(3)]).is_err());
}

#[test]
fn dynamic_name_text_accepts_none_and_empty_key_but_rejects_wrong_types_forged_identity_and_portability_limits() {
    let missing=modern("_1","");assert!(matches!(text(&missing),DeferredText::NameStringTable{reference} if reference.table=="None"&&reference.key.is_empty()));
    assert_eq!(inspect_table_identity("__rvn_text_table_valid",&[missing.clone()]).unwrap(),Value::Bool(true));
    assert_eq!(project(&missing,&StringTableResources::default()).unwrap(),MISSING_STRING_TABLE_ENTRY);
    assert!(construct("__rvn_string_table_text_name",&[Value::Str("id".into()),Value::Str("key".into())]).is_err());
    assert!(construct("__rvn_string_table_text_name",&[name("id"),name("key")]).is_err());
    for key in ["x".repeat(4097),"NUL\0key".into()] {assert!(construct("__rvn_string_table_text_name",&[name("id"),Value::Str(key)]).is_err());}
    assert!(construct("__rvn_string_table_text",&[Value::Str("id".into()),Value::Str(String::new())]).is_err(),"legacy admission must stay exact");
    let forged=DeferredText::NameStringTable{reference:StringTableReference{table:"_1".into(),key:"key".into()}};assert!(forged.storage_work().is_err());
    let old=legacy(&"x".repeat(1024),"key");assert_eq!(inspect_table_identity("__rvn_text_table_valid",&[old.clone()]).unwrap(),Value::Bool(true));assert!(inspect_table_identity("__rvn_text_table_id",&[old]).is_err());
}

#[test]
fn name_history_redirects_are_bounded_name_based_and_do_not_modify_retained_reference() {
    let mut resources=StringTableResources::default();resources.publish(table("asset:Target0250","Redirected")).unwrap();
    resources.redirect(StringTableReference{table:"asset:Alias0250".into(),key:"key".into()},StringTableReference{table:"asset:Target0250".into(),key:"key".into()}).unwrap();
    let named=modern("ASSET:ALIAS0250","key");let before=resources.clone();let encoded=serde_json::to_value(&named).unwrap();
    assert_eq!(project(&named,&resources).unwrap(),"Redirected");assert_eq!(resources,before);assert_eq!(serde_json::to_value(&named).unwrap(),encoded);
    assert!(matches!(inspect_table_identity("__rvn_text_table_id",&[named]).unwrap(),Value::Name(id)if id.as_str()=="ASSET:ALIAS0250"));
    resources.redirect(StringTableReference{table:"asset:Loop0250".into(),key:"key".into()},StringTableReference{table:"ASSET:LOOP0250".into(),key:"key".into()}).unwrap();
    assert!(project(&modern("asset:Loop0250","key"),&resources).is_err(),"Name-equivalent cycle must fail within the bounded chain");
}

#[test]
fn name_history_detects_snapshot_name_collisions_and_keeps_legacy_access() {
    let mut resources=StringTableResources::default();resources.publish(table("asset:Case0250","First")).unwrap();resources.publish(table("ASSET:CASE0250","Second")).unwrap();
    let before=resources.clone();assert!(project(&modern("asset:Case0250","key"),&resources).is_err());
    assert_eq!(project(&legacy("asset:Case0250","key"),&resources).unwrap(),"First");assert_eq!(project(&legacy("ASSET:CASE0250","key"),&resources).unwrap(),"Second");assert_eq!(resources,before);
}

#[test]
fn save14_is_opt_in_and_cold_name_history_rehydrates_without_loading_or_rewriting_legacy13_values() {
    let source=r#"init {set named=__rvn_string_table_text_name(__rvn_literal_name("ASSET:LATE0250"),"key") set exact=__rvn_string_table_text("ASSET:LATE0250","key")}
label start
"Before"
"Modern [named]"
"Legacy [exact]"
"#;
    let script=rvn_parser::parse(source).unwrap();let mut original=Engine::new(script.clone(),TerminalRenderer,8).unwrap();
    assert!(matches!(original.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Before"));
    let saved=SaveData::from_state(&original.state,1,"Name Text".into(),"main.rvn".into());assert_eq!(saved.format_version,NAME_STRING_TABLE_SAVE_FORMAT_VERSION);
    let encoded=serde_json::to_value(&saved).unwrap();assert_eq!(encoded["vars"]["named"]["v"]["kind"],"name_string_table");assert_eq!(encoded["vars"]["exact"]["v"]["kind"],"string_table");
    let serialized=serde_json::to_vec(&saved).unwrap();
    let mut cold=Engine::new(script.clone(),TerminalRenderer,8).unwrap();let before=cold.text_resources().clone();
    assert_eq!(cold.load_data(serde_json::from_slice(&serialized).unwrap()).unwrap(),LoadCompatibility::Verified);assert_eq!(cold.text_resources(),&before);
    cold.text_resources_mut().publish(table("asset:Late0250","Fresh after cold load")).unwrap();let published=cold.text_resources().clone();cold.advance_dialogue().unwrap();
    assert!(matches!(cold.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Modern Fresh after cold load"));cold.advance_dialogue().unwrap();
    assert!(matches!(cold.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text==format!("Legacy {MISSING_STRING_TABLE_ENTRY}")));assert_eq!(cold.text_resources(),&published);
    let mut wrong=encoded.clone();wrong["format_version"]=serde_json::json!(STRING_TABLE_SAVE_FORMAT_VERSION);let mut rejected=Engine::new(script,TerminalRenderer,8).unwrap();assert!(rejected.load_data(serde_json::from_value(wrong).unwrap()).is_err());
    let mut legacy_state=original.state.clone();legacy_state.vars.remove("named");legacy_state.last_dialogue=None;
    assert_eq!(SaveData::from_state(&legacy_state,1,"Old".into(),"main.rvn".into()).format_version,STRING_TABLE_SAVE_FORMAT_VERSION);
}

}


mod qualified_name_history_budget_0261 {
use rvn_core::{
    localized_text::{construct, render_text_with_resources, VnTextScalars},
    string_table::{StringTableReference, StringTableResources, MISSING_STRING_TABLE_ENTRY},
};
use rvn_parser::{localized_text::DeferredText, Name, Value};
use std::collections::BTreeMap;

fn named(table: &str, key: &str) -> Value {
    construct("__rvn_string_table_text_name", &[Value::Name(Name::new(table).unwrap()), Value::Str(key.into())]).unwrap()
}

fn project(value: &Value, resources: &StringTableResources) -> Result<String, String> {
    let Value::Text(text) = value else { panic!("Text expected") };
    render_text_with_resources(text, None, &VnTextScalars, Some(resources))
}

fn nonmatching_redirects(count: usize, key_bytes: usize) -> StringTableResources {
    let mut resources = StringTableResources::default();
    for index in 0..count {
        let suffix = format!("{index:04}");
        let key = format!("{}{suffix}", "a".repeat(key_bytes - suffix.len()));
        resources.redirect(
            StringTableReference { table: format!("alias-{index}"), key },
            StringTableReference { table: "target".into(), key: "key".into() },
        ).unwrap();
    }
    resources
}

#[test]
fn name_history_rejects_nonmatching_redirect_scans_over_existing_work_budget_and_keeps_legacy_exact() {
    // Every key is valid and differs from the query. The old early continue
    // visited 4,096 entries without charging any of their comparison work.
    let resources = nonmatching_redirects(4096, 4096);
    let stamp = resources.revision_stamp();
    let value = named("missing", &"a".repeat(4096));
    let encoded = serde_json::to_value(&value).unwrap();
    let error = project(&value, &resources).unwrap_err();
    assert!(error.contains("1,000,000"), "{error}");
    assert_eq!(serde_json::to_value(&value).unwrap(), encoded);
    assert_eq!(resources.revision_stamp(), stamp);
    let legacy = Value::Text(Box::new(DeferredText::StringTable { reference: StringTableReference {
        table: "missing".into(), key: "a".repeat(4096),
    } }));
    assert_eq!(project(&legacy, &resources).unwrap(), MISSING_STRING_TABLE_ENTRY);
}

#[test]
fn formatted_name_history_aggregates_redirect_scan_work_across_individually_valid_arguments() {
    let resources = nonmatching_redirects(2048, 128);
    let stamp = resources.revision_stamp();
    let value = named("missing", &"a".repeat(128));
    assert_eq!(project(&value, &resources).unwrap(), MISSING_STRING_TABLE_ENTRY);
    let formatted = |count: usize| {
        let pattern = (0..count).map(|index| format!("{{k{index}}}")).collect::<String>();
        let arguments = (0..count).map(|index| (format!("k{index}"), value.clone())).collect::<BTreeMap<_, _>>();
        construct("__rvn_format_text", &[Value::Str(pattern), Value::Dict(arguments)]).unwrap()
    };
    assert_eq!(project(&formatted(50), &resources).unwrap(), MISSING_STRING_TABLE_ENTRY.repeat(50));
    let combined = formatted(64);
    let encoded = serde_json::to_value(&combined).unwrap();
    let error = project(&combined, &resources).unwrap_err();
    assert!(error.contains("1,000,000"), "{error}");
    assert_eq!(serde_json::to_value(&combined).unwrap(), encoded);
    assert_eq!(resources.revision_stamp(), stamp);
}

}
