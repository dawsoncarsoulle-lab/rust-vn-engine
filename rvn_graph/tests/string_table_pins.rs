use rvn_core::{
    eval::FunctionLibrary,
    string_table::{StringTableAsset, StringTableEntry, StringTableResources},
    Engine, Interaction, TerminalRenderer,
};
use rvn_graph::*;
use rvn_parser::localized_text::{LocalizedText, StringTableReference, TextLocalization};
use std::collections::{BTreeMap, HashMap};

const OWNER: &str = "b9ab9b6d1c9448ad8dcd799c23b4b550";
const TABLE: &str = "17411a3e431e4c9388b78a2b9825b130";

#[derive(Default)]
struct UiRenderer {
    views: Vec<rvn_ui::programmable::ScreenView>,
}
impl rvn_core::Renderer for UiRenderer {
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &rvn_core::renderer::SceneRenderBatch) -> Result<(), String> {
        Ok(())
    }
    fn publish_scene_batch(&mut self, batch: rvn_core::renderer::SceneRenderBatch) {
        self.views = batch.interfaces;
    }
    fn update_interfaces(
        &mut self,
        views: &[rvn_ui::programmable::ScreenView],
    ) -> Result<(), String> {
        self.views = views.to_vec();
        Ok(())
    }
    fn set_background(&mut self, _: &str, _: &rvn_parser::Transition) {}
    fn show_sprite(
        &mut self,
        _: &str,
        _: Option<&str>,
        _: &rvn_parser::Position,
        _: &rvn_parser::Transition,
        _: Option<&rvn_core::SpriteState>,
    ) {
    }
    fn hide_sprite(&mut self, _: &str, _: &rvn_parser::Transition, _: &rvn_core::SpriteState) {}
    fn move_sprite(
        &mut self,
        _: &str,
        _: &rvn_parser::Position,
        _: &rvn_parser::Transition,
        _: &rvn_core::SpriteState,
    ) {
    }
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {
        0
    }
    fn music_play(&mut self, _: &str, _: &rvn_parser::Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &rvn_parser::Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &rvn_parser::Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &rvn_parser::Transition) {}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[rvn_parser::Hotspot]) -> usize {
        0
    }
    fn restore_screen(&mut self, _: &rvn_core::GameState) {}
    fn restore_audio(&mut self, _: &rvn_core::MusicState) {}
}
fn owned(kind: GraphKind) -> GraphDocument {
    let mut graph = GraphDocument::new(GraphId(1), kind);
    graph
        .enable_text_owner(TextOwnerId::parse(OWNER).unwrap())
        .unwrap();
    graph
}
fn add(graph: &mut GraphDocument, kind: NodeKind) -> NodeId {
    graph.add_catalog_node(kind, [0., 0.]).unwrap()
}
fn pin(graph: &GraphDocument, node: NodeId, key: &str) -> PinId {
    graph.pin_by_key(node, key).unwrap().id
}
fn reference(key: &str) -> StringTableReference {
    StringTableReference {
        table: TABLE.into(),
        key: key.into(),
    }
}
fn table(source: &str) -> StringTableAsset {
    StringTableAsset {
        id: TABLE.into(),
        namespace: "VN".into(),
        entries: BTreeMap::from([(
            "hello".into(),
            StringTableEntry {metadata:Default::default(),
                source: source.into(),
                developer_notes: "Notes belong to the table".into(),
            },
        )]),
    }
}

#[test]
fn table_value_pin_persists_only_reference_and_generates_typed_runtime_text() {
    let mut graph = owned(GraphKind::Init);
    let value = add(&mut graph, NodeKind::TextValue);
    let output = pin(&graph, value, "value");
    assert!(graph
        .set_pin_text_table_reference(output, reference("hello"), &[])
        .unwrap());
    assert_eq!(graph.schema_version, GRAPH_TABLE_TEXT_SCHEMA_VERSION);
    assert_eq!(
        graph.pin_text_table_reference(output).unwrap(),
        Some(&reference("hello"))
    );
    assert!(graph.pin_text(output).unwrap().is_none());
    assert!(graph.authored_text_literals().unwrap().is_empty());
    assert_eq!(
        graph.nodes[&value].properties["value"],
        PropertyValue::String(String::new())
    );
    let bytes = graph.to_pretty_json().unwrap();
    assert!(!bytes.contains("Notes belong"));
    assert_eq!(GraphDocument::from_json(&bytes).unwrap(), graph);
    let source = transpile_value_output(&graph, value, "value").unwrap();
    assert_eq!(
        source,
        format!("__rvn_string_table_text(\"{TABLE}\", \"hello\")")
    );
    let mut library = FunctionLibrary::default();
    library
        .text_resources_mut()
        .publish(table("Current"))
        .unwrap();
    let parsed = rvn_parser::parse(&format!("set value={source}")).unwrap();
    let rvn_parser::Statement::SetVar {
        value: expression, ..
    } = &parsed[0]
    else {
        panic!()
    };
    let rvn_parser::Value::Text(text) = library.eval(expression, &HashMap::new()).unwrap() else {
        panic!("Text flattened")
    };
    assert_eq!(
        rvn_core::localized_text::render_text_with_resources(
            &text,
            None,
            &rvn_core::localized_text::VnTextScalars,
            Some(library.text_resources())
        )
        .unwrap(),
        "Current"
    );
    library
        .text_resources_mut()
        .publish(table("Replacement"))
        .unwrap();
    assert_eq!(
        rvn_core::localized_text::render_text_with_resources(
            &text,
            None,
            &rvn_core::localized_text::VnTextScalars,
            Some(library.text_resources())
        )
        .unwrap(),
        "Replacement"
    );
    assert_eq!(graph.to_pretty_json().unwrap(), bytes);
}

#[test]
fn format_reference_retains_authored_argument_pins_while_current_source_and_save13_change() {
    let mut graph = owned(GraphKind::Init);
    let format = add(&mut graph, NodeKind::FormatText);
    graph.set_format_text_pattern(format, "Old {Name}").unwrap();
    let name = pin(&graph, format, "argument_Name");
    let result = pin(&graph, format, "result");
    let pattern = pin(&graph, format, "format");
    let arguments = vec!["Name".into(), "0".into(), "Prénom".into()];
    graph
        .set_pin_text_table_reference(pattern, reference("hello"), &arguments)
        .unwrap();
    assert_eq!(pin(&graph, format, "argument_Name"), name);
    assert_eq!(pin(&graph, format, "result"), result);
    graph
        .set_pin_default(
            format,
            "argument_Name",
            PropertyValue::String("Visitor".into()),
        )
        .unwrap();
    graph
        .set_pin_default(format, "argument_0", PropertyValue::Int(2))
        .unwrap();
    graph
        .set_pin_default(
            format,
            "argument_Prénom",
            PropertyValue::String("Éloïse".into()),
        )
        .unwrap();
    let before = graph.clone();
    let source = transpile_value_output(&graph, format, "result").unwrap();
    assert!(!source.contains("Old"));
    assert!(source.contains("\"0\": 2"));
    let script=rvn_parser::parse(&format!("init {{set message={source} set captured=text_to_string(message)}}\nscene label start\n\"[captured] | [message]\"\n")).unwrap();
    let mut resources = StringTableResources::default();
    resources
        .publish(table("Hello {Name}; {0}; {Prénom}"))
        .unwrap();
    let mut engine =
        Engine::new_with_text_resources(script, TerminalRenderer, 8, resources).unwrap();
    assert!(
        matches!(engine.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..}) if text=="Hello Visitor; 2; Éloïse | Hello Visitor; 2; Éloïse")
    );
    engine
        .text_resources_mut()
        .publish(table("Bonjour {Speaker} et {Count}!"))
        .unwrap();
    assert!(
        matches!(engine.displayed_interaction().unwrap(),Some(Interaction::Dialogue{text,..}) if text=="Hello Visitor; 2; Éloïse | Bonjour {Speaker} et {Count}!")
    );
    assert!(!graph
        .set_pin_text_table_reference(
            pattern,
            reference("hello"),
            &["Speaker".into(), "Count".into()]
        )
        .unwrap());
    assert_eq!(graph, before);
    let save = rvn_core::save::SaveData::try_from_snapshot(
        &engine.snapshot_for_save().unwrap(),
        1,
        "Reference".into(),
        "main.rvn".into(),
    )
    .unwrap();
    assert_eq!(save.format_version, 13);
    assert_eq!(
        GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),
        before
    );
    graph
        .set_pin_text_table_reference(
            pattern,
            reference("other"),
            &["Name".into(), "Speaker".into()],
        )
        .unwrap();
    assert_eq!(pin(&graph, format, "argument_Name"), name);
    assert_eq!(pin(&graph, format, "result"), result);
    assert!(graph.pin_by_key(format, "argument_0").is_none());
}

#[test]
fn invalid_reference_inline_bypasses_and_oversized_argument_edits_refuse_atomically() {
    let mut graph = owned(GraphKind::Init);
    let format = add(&mut graph, NodeKind::FormatText);
    let pattern = pin(&graph, format, "format");
    graph
        .set_pin_text_table_reference(pattern, reference("hello"), &["Name".into()])
        .unwrap();
    let before = graph.clone();
    assert!(graph
        .set_pin_text_table_reference(
            pattern,
            StringTableReference {
                table: "not-an-asset-uuid".into(),
                key: "hello".into()
            },
            &[]
        )
        .is_err());
    assert!(graph
        .set_pin_text_table_reference(pattern, reference("other"), &vec!["x".into(); 4097])
        .is_err());
    assert!(graph
        .set_pin_text_table_reference(pattern, reference("other"), &["Name".into(), "Name".into()])
        .is_err());
    assert!(graph
        .set_pin_text_table_reference(pattern, reference("other"), &["x".repeat(1_048_577)])
        .is_err());
    assert!(graph.set_format_text_pattern(format, "Bypass").is_err());
    assert!(graph
        .set_pin_default(format, "format", PropertyValue::String("Bypass".into()))
        .is_err());
    assert!(graph
        .set_pin_text(
            pattern,
            LocalizedText {
                source: "Bypass".into(),
                localization: TextLocalization::CultureInvariant
            }
        )
        .is_err());
    assert!(graph
        .adopt_pin_text(pattern, || panic!("No identity allocation on refusal"))
        .is_err());
    assert_eq!(graph, before);
    let mut bad = graph.clone();
    bad.schema_version = 7;
    assert!(GraphDocument::from_json(&bad.to_pretty_json().unwrap()).is_err());
    let mut bad = graph.clone();
    bad.pins.get_mut(&pattern).unwrap().text_localization = Some(TextLocalization::Empty);
    assert!(GraphDocument::from_json(&bad.to_pretty_json().unwrap()).is_err());
    let mut bad = graph.clone();
    bad.pins.get_mut(&pattern).unwrap().default_value =
        Some(PropertyValue::String("Second source".into()));
    assert!(GraphDocument::from_json(&bad.to_pretty_json().unwrap()).is_err());
    let legacy = GraphDocument::new(GraphId(2), GraphKind::Init);
    assert_eq!(legacy.schema_version, 5);
    assert!(!legacy
        .to_pretty_json()
        .unwrap()
        .contains("text_table_reference"));
    let mut legacy = legacy;
    legacy
        .enable_text_owner(TextOwnerId::parse(OWNER).unwrap())
        .unwrap();
    assert_eq!(legacy.schema_version, 7);
    let mut no_owner = GraphDocument::new(GraphId(3), GraphKind::Init);
    let value = add(&mut no_owner, NodeKind::TextValue);
    let output = pin(&no_owner, value, "value");
    let before = no_owner.clone();
    assert!(no_owner
        .set_pin_text_table_reference(output, reference("hello"), &[])
        .is_err());
    assert_eq!(no_owner, before);
}

#[test]
fn a_table_value_pin_reaches_real_narrative_interpolation_without_flattening() {
    let mut graph = owned(GraphKind::Label {
        name: "start".into(),
    });
    let root = add(&mut graph, NodeKind::Label);
    let dialogue = add(&mut graph, NodeKind::Dialogue);
    let value = add(&mut graph, NodeKind::TextValue);
    let output = pin(&graph, value, "value");
    graph
        .set_pin_text_table_reference(output, reference("hello"), &[])
        .unwrap();
    graph
        .connect(output, pin(&graph, dialogue, "text"))
        .unwrap();
    graph
        .connect(
            pin(&graph, root, "exec_out"),
            pin(&graph, dialogue, "exec_in"),
        )
        .unwrap();
    let generated = transpile(&graph).unwrap();
    assert!(generated.source.contains("__rvn_string_table_text"));
    let mut resources = StringTableResources::default();
    resources.publish(table("Direct table text")).unwrap();
    let mut engine =
        Engine::new_with_text_resources(generated.ast, TerminalRenderer, 8, resources).unwrap();
    assert!(
        matches!(engine.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..}) if text=="Direct table text")
    );
}

#[test]
fn interface_clipboard_clones_table_reference_to_a_different_owned_screen_without_source_copy() {
    let screen = |name: &str, text: &str| {
        import_script(&rvn_parser::parse(&format!("screen {name}() {{return component(\"root\",\"column\",{{}},[component(\"caption\",\"text\",{{\"text\":{text}}},[])])}}")).unwrap()).unwrap().into_iter().find(|g|matches!(g.kind,GraphKind::Screen{..})).unwrap()
    };
    let mut source = screen(
        "first",
        &format!("text_to_string(__rvn_string_table_text(\"{TABLE}\",\"hello\"))"),
    );
    source
        .enable_text_owner(TextOwnerId::parse(OWNER).unwrap())
        .unwrap();
    let constructor = source
        .nodes
        .values()
        .find(|n| {
            n.properties.get("function")
                == Some(&PropertyValue::String("__rvn_string_table_text".into()))
        })
        .unwrap()
        .id;
    let outgoing: Vec<_> = source
        .edges
        .values()
        .filter(|edge| source.pins[&edge.output].node == constructor)
        .cloned()
        .collect();
    source.remove_node(constructor);
    let value = add(&mut source, NodeKind::TextValue);
    let output = pin(&source, value, "value");
    source
        .set_pin_text_table_reference(output, reference("hello"), &[])
        .unwrap();
    for edge in outgoing {
        source.connect(output, edge.input).unwrap();
    }
    let returned = |g: &GraphDocument| {
        g.nodes
            .values()
            .find(|n| n.kind == NodeKind::FunctionReturn)
            .unwrap()
            .id
    };
    let clipboard = source
        .copy_interface_subtree(
            returned(&source),
            source.interface_component_ids()["caption"],
        )
        .unwrap();
    let mut target = screen("second", "\"Existing\"");
    target
        .enable_text_owner(TextOwnerId::parse("797679920f7142be99d9d6fe5553ca34").unwrap())
        .unwrap();
    target
        .paste_interface_subtree(
            returned(&target),
            target.interface_component_ids()["root"],
            &clipboard,
        )
        .unwrap();
    assert_eq!(target.schema_version, 8);
    target.validate_narrative_metadata().unwrap();
    assert!(target
        .pins
        .values()
        .any(|p| p.text_table_reference.as_ref() == Some(&reference("hello"))));
    assert!(transpile(&target)
        .unwrap()
        .source
        .contains(&format!("__rvn_string_table_text(\"{TABLE}\", \"hello\")")));
    assert_eq!(
        GraphDocument::from_json(&target.to_pretty_json().unwrap()).unwrap(),
        target
    );
    let generated = transpile(&target).unwrap();
    let script = rvn_parser::parse(&format!(
        "{}\nscene label start\nui.open(\"second\",[],false,0)\n\"Waiting\"\n",
        generated.source
    ))
    .unwrap();
    let mut resources = StringTableResources::default();
    resources.publish(table("Clipboard current")).unwrap();
    let mut engine =
        Engine::new_with_text_resources(script, UiRenderer::default(), 8, resources).unwrap();
    engine.step_until_interaction().unwrap();
    assert_eq!(
        engine.renderer.views[0]
            .root
            .find("caption_copy_1")
            .unwrap()
            .text,
        "Clipboard current"
    );
    assert_eq!(
        engine.interface_views().unwrap()[0]
            .root
            .find("caption_copy_1")
            .unwrap()
            .text,
        "Clipboard current"
    );
    engine
        .text_resources_mut()
        .publish(table("Clipboard replacement"))
        .unwrap();
    assert_eq!(
        engine.interface_views().unwrap()[0]
            .root
            .find("caption_copy_1")
            .unwrap()
            .text,
        "Clipboard replacement"
    );
}

#[test]
fn binding_clears_hidden_output_default_and_rejects_a_second_serialized_source() {
    let mut graph = owned(GraphKind::Init);
    let value = add(&mut graph, NodeKind::TextValue);
    let output = pin(&graph, value, "value");
    graph
        .set_pin_default(
            value,
            "value",
            PropertyValue::String("Old hidden default".into()),
        )
        .unwrap();
    graph
        .set_pin_text_table_reference(output, reference("hello"), &[])
        .unwrap();
    assert!(graph.pins[&output].default_value.is_none());
    let mut malformed = graph.clone();
    malformed.pins.get_mut(&output).unwrap().default_value =
        Some(PropertyValue::String("Hidden second source".into()));
    assert!(GraphDocument::from_json(&malformed.to_pretty_json().unwrap()).is_err());
    assert_eq!(
        GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),
        graph
    );
}
