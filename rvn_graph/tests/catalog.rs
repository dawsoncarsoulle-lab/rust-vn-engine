use rvn_graph::{
    node_definition, AssetKind, NodeCategory, NodeKind, PinDirection, ValueType, ALL_NODE_KINDS,
};
use std::collections::HashSet;

#[test]
fn catalog_has_one_definition_per_node_kind() {
    let unique: HashSet<_> = ALL_NODE_KINDS.iter().copied().collect();
    assert_eq!(unique.len(), ALL_NODE_KINDS.len());

    for kind in ALL_NODE_KINDS {
        let definition = node_definition(*kind);
        assert_eq!(definition.kind, *kind);
        assert!(!definition.title.is_empty());
    }
}

#[test]
fn catalog_preserves_the_visual_language() {
    assert_eq!(
        node_definition(NodeKind::Label).category,
        NodeCategory::Event
    );
    assert_eq!(
        node_definition(NodeKind::Dialogue).category,
        NodeCategory::Narrative
    );
    assert_eq!(
        node_definition(NodeKind::MusicPlay).category,
        NodeCategory::Audio
    );
    assert_eq!(NodeCategory::Event.header_rgb(), [0x9e, 0x1b, 0x1b]);
    assert!(node_definition(NodeKind::Choice).has_dynamic_pins);
}

#[test]
fn conversion_nodes_expose_their_real_input_and_output_types() {
    let int_to_float = node_definition(NodeKind::ConvertIntToFloat);
    assert_eq!(int_to_float.pins[0].direction, PinDirection::Input);
    assert_eq!(int_to_float.pins[0].value_type, ValueType::Int);
    assert_eq!(int_to_float.pins[1].direction, PinDirection::Output);
    assert_eq!(int_to_float.pins[1].value_type, ValueType::Float);

    let number_to_text = node_definition(NodeKind::ConvertNumberToText);
    assert_eq!(number_to_text.pins[0].value_type, ValueType::Float);
    assert!(number_to_text.pins[0].value_type.accepts(&ValueType::Int));
    assert_eq!(number_to_text.pins[1].value_type, ValueType::String);

    let text_to_int = node_definition(NodeKind::ConvertTextToInt);
    assert_eq!(text_to_int.pins[0].value_type, ValueType::String);
    assert_eq!(text_to_int.pins[1].value_type, ValueType::Int);
}

#[test]
fn every_asset_has_a_strongly_typed_reference_node() {
    for (node, asset) in [
        (NodeKind::SceneAsset, AssetKind::Background),
        (NodeKind::SpriteAsset, AssetKind::Sprite),
        (NodeKind::MusicAsset, AssetKind::Music),
        (NodeKind::SoundEffectAsset, AssetKind::SoundEffect),
        (NodeKind::VoiceAsset, AssetKind::Voice),
        (NodeKind::CinematicAsset, AssetKind::Cinematic),
        (NodeKind::HoverImageAsset, AssetKind::HoverImage),
        (NodeKind::ScriptAsset, AssetKind::Script),
    ] {
        let definition = node_definition(node);
        assert_eq!(definition.pins.len(), 1);
        assert_eq!(definition.pins[0].direction, PinDirection::Output);
        assert_eq!(definition.pins[0].value_type, ValueType::Asset(asset));
    }
}
