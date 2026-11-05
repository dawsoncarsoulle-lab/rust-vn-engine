// PRIVATE NOT_RUN. Actual graph editing, pins, import/export, project compiler, Engine.
use rvn_graph::*;use rvn_parser::{parse_program,Value};use rvn_core::{Engine,Interaction,TerminalRenderer};

#[test]
fn name_typed_graph_cold_json_source_roundtrip_and_engine_keep_spelling_and_strings() {
    let source = r#"scene label start
set named = __rvn_literal_name("RvnCase")
set converted = __rvn_string_to_name("rvncase")
set same = named == converted
set strings_same = "RvnCase" == "rvncase"
set names = [named, __rvn_literal_name("None_0")]
set display = __rvn_name_to_string(names[0])
"[display] [same] [strings_same]"
"#;
    let mut graphs=documents(source);
    for _ in 0..3 {
        assert!(graphs.iter().any(|g| g.nodes.values().any(|node| node.kind==NodeKind::ConvertStringToName)));
        assert!(graphs.iter().any(|g| g.nodes.values().any(|node| node.kind==NodeKind::ConvertNameToString)));
        assert!(graphs.iter().any(|g| g.nodes.values().any(|node| matches!(node.properties.get("value"),Some(PropertyValue::Name(name)) if name.as_str()=="RvnCase"))
            || g.pins.values().any(|pin|matches!(&pin.default_value,Some(PropertyValue::Name(name)) if name.as_str()=="RvnCase"))));
        for graph in &mut graphs {
            let cold=GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap();
            assert_eq!(cold,*graph); // Includes graph/node/pin/edge UUIDs.
            *graph=cold;
        }
        let compiled=transpile_project(&graphs).unwrap();
        let mut engine=Engine::from_program(parse_program(&compiled.source).unwrap(),TerminalRenderer,16).unwrap();
        assert!(matches!(engine.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="RvnCase true false"));
        assert!(matches!(&engine.state.vars["named"],Value::Name(name) if name.as_str()=="RvnCase"));
        let Value::List(items)=&engine.state.vars["names"]else{panic!("typed Name list")};
        assert!(matches!(&items[1],Value::Name(name) if name.as_str()=="None_0" && !name.is_none()));
        graphs=import_script(&compiled.ast).unwrap();
    }
}

#[test]
fn name_automatic_conversion_replaces_only_the_occupied_input_and_keeps_defaults() {
    let mut graph=documents("scene label start\n").into_iter().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();
    let original=graph.add_catalog_node(NodeKind::ConvertStringToName,[0.,0.]).unwrap();
    let target=graph.add_catalog_node(NodeKind::ConvertStringToName,[480.,0.]).unwrap();
    graph.set_pin_default(original,"value",PropertyValue::String("OriginalCase".into())).unwrap();
    graph.set_pin_default(target,"value",PropertyValue::String("RetainedDefault".into())).unwrap();
    let output=graph.pin_by_key(original,"result").unwrap().id;
    let input=graph.pin_by_key(target,"value").unwrap().id;
    let inserted=graph.connect_with_conversions(output,input,[240.,0.]).unwrap();
    assert_eq!(inserted.conversions.len(),1);
    let intermediate=inserted.conversions[0];
    assert_eq!(graph.nodes[&intermediate].kind,NodeKind::ConvertNameToString);
    assert_eq!(graph.pins[&input].default_value,Some(PropertyValue::String("RetainedDefault".into())));
    let old_input=graph.pin_by_key(intermediate,"value").unwrap().id;
    let downstream=graph.edges[&inserted.edge].clone();
    let plain=graph.add_catalog_node(NodeKind::Literal,[0.,160.]).unwrap();
    graph.set_property(plain,"value",PropertyValue::String("ReplacementCase".into())).unwrap();
    let plain_output=graph.pin_by_key(plain,"value").unwrap().id;
    // A graph refuses occupied inputs; the existing editor owner prepares its
    // replacement transaction on a copy before installing the conversion.
    let occupied=graph.clone();
    assert!(graph.connect_with_conversions(plain_output,old_input,[240.,160.]).is_err());
    assert_eq!(graph,occupied);
    let mut replacement_trial=graph.clone();
    let old_edge=replacement_trial.edges.values().find(|edge|edge.input==old_input).unwrap().id;
    replacement_trial.remove_edge(old_edge);
    let replacement=replacement_trial.connect_with_conversions(plain_output,old_input,[240.,160.]).unwrap();
    graph=replacement_trial;
    assert_eq!(replacement.conversions.len(),1);
    assert_eq!(graph.nodes[&replacement.conversions[0]].kind,NodeKind::ConvertStringToName);
    assert_eq!(graph.edges[&inserted.edge],downstream);
    assert!(!graph.edges.values().any(|edge|edge.output==output && edge.input==old_input));
    assert_eq!(graph.pin_by_key(original,"value").unwrap().default_value,Some(PropertyValue::String("OriginalCase".into())));
    let before=graph.clone();
    assert!(graph.connect_with_conversions(output,graph.pin_by_key(plain,"value").unwrap().id,[0.,0.]).is_err());
    assert_eq!(graph,before);
    assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(),graph);
    assert_ne!(PropertyValue::Name(rvn_graph::NameDefault::new("AuthoredCase").unwrap()),PropertyValue::Name(rvn_graph::NameDefault::new("authoredcase").unwrap()));
}
#[test]
fn name_authoring_keeps_blank_suffix_and_case_through_source_cold_graph_and_runtime() {
    use rvn_core::{Engine, Interaction, TerminalRenderer};
    use rvn_graph::NameDefault;
    use rvn_parser::{Name, Value};
    for raw in ["", "_1", "none", "None_0", "AuthoredCase"] {
        let quoted = serde_json::to_string(raw).unwrap();
        let source = format!("init {{set named=__rvn_literal_name({quoted})}}\nscene label start\n\"Name [__rvn_name_to_string(named)]\"\n");
        let mut graphs = documents(&source);
        for _ in 0..3 {
            assert!(graphs.iter().any(|graph| graph.nodes.values().any(|node|
                matches!(node.properties.get("value"), Some(PropertyValue::Name(value)) if value.as_str()==raw))
                || graph.pins.values().any(|pin|
                    matches!(&pin.default_value, Some(PropertyValue::Name(value)) if value.as_str()==raw))));
            for graph in &graphs {
                assert_eq!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap(), *graph);
            }
            let compiled = transpile_project(&graphs).unwrap();
            assert!(compiled.source.contains(&format!("__rvn_literal_name({quoted})")));
            let mut engine = Engine::from_program(parse_program(&compiled.source).unwrap(), TerminalRenderer, 16).unwrap();
            let runtime = Name::new(raw).unwrap();
            assert!(matches!(engine.step_until_interaction().unwrap(), Some(Interaction::Dialogue{text,..}) if text==format!("Name {}", runtime.as_str())));
            assert_eq!(engine.state.vars["named"], Value::Name(runtime));
            graphs = import_script(&compiled.ast).unwrap();
        }
    }
    assert_ne!(NameDefault::new("").unwrap(), NameDefault::default());
    assert_eq!(NameDefault::new("").unwrap().runtime(), NameDefault::default().runtime());
    let legacy: PropertyValue = serde_json::from_str(r#"{"type":"name","value":"None"}"#).unwrap();
    assert_eq!(legacy, PropertyValue::Name(NameDefault::default()));
    let blank = PropertyValue::Name(NameDefault::new("").unwrap());
    assert_eq!(serde_json::to_value(&blank).unwrap(), serde_json::json!({"type":"name","value":""}));
    assert_eq!(serde_json::from_value::<PropertyValue>(serde_json::to_value(blank.clone()).unwrap()).unwrap(), blank);
    for raw in ["bad\0tail".to_owned(), "a".repeat(1024)] {
        assert!(serde_json::from_value::<PropertyValue>(serde_json::json!({"type":"name","value":raw})).is_err());
    }
    assert!(serde_json::from_value::<NameDefault>(serde_json::json!({"display":"Valid","runtime":"forged"})).is_err());
}

fn documents(source:&str)->Vec<GraphDocument>{import_script(&parse_program(source).unwrap().script).unwrap()}
fn wire(g:&mut GraphDocument,a:NodeId,out:&str,b:NodeId,input:&str){let a=g.pin_by_key(a,out).unwrap().id;let b=g.pin_by_key(b,input).unwrap().id;g.connect(a,b).unwrap();}
#[test]
fn real_flow_catalogue_preserves_old156_and_exposes_int_and_execution_pins(){
    // The historical ordered catalogue survives after removing exactly the two
    // additive Unit species and the four additive StandardMacro species. Each
    // additive family retains its own exact order and real pin signatures.
    const LEGACY180:[NodeKind;180]=[
        NodeKind::WidgetClassValue, NodeKind::WidgetObjectClass, NodeKind::WidgetAddToViewport, NodeKind::WidgetRemoveFromParent, NodeKind::WidgetCreate, NodeKind::TextFromStringTable,
        NodeKind::GetStringTableIdAndKey, NodeKind::StringTableId, NodeKind::StringTableAsset, NodeKind::StringTableRegistered, NodeKind::StringTableEntryRegistered, NodeKind::StringTableNamespace,
        NodeKind::StringTableSource, NodeKind::StringTableMetadata, NodeKind::RegisteredStringTables, NodeKind::StringTableKeys, NodeKind::StringTableMetadataIds, NodeKind::MenuExecute,
        NodeKind::MenuAction, NodeKind::MenuStartScene, NodeKind::MenuOpenPage, NodeKind::MenuSlot, NodeKind::MenuProtect, NodeKind::MenuSavePage,
        NodeKind::MenuNumberPreference, NodeKind::MenuBoolPreference, NodeKind::MenuLanguage, NodeKind::MenuAdvance, NodeKind::MenuSkipTypewriter, NodeKind::MenuChoose,
        NodeKind::MenuGalleryCg, NodeKind::MenuGalleryTab, NodeKind::MenuConfirm, NodeKind::MenuCancel, NodeKind::FunctionEntry, NodeKind::ScreenEntry,
        NodeKind::HandlerEntry, NodeKind::LocalVariable, NodeKind::UiOpen, NodeKind::UiClose, NodeKind::UiFocus, NodeKind::UiSetState,
        NodeKind::UiComponent, NodeKind::CanvasRect, NodeKind::CanvasEllipse, NodeKind::CanvasLine, NodeKind::CanvasPolygon, NodeKind::CanvasText,
        NodeKind::CanvasImage, NodeKind::CanvasGroup, NodeKind::CanvasHit, NodeKind::MotionPlay, NodeKind::MotionStop, NodeKind::MotionWait,
        NodeKind::MotionTween, NodeKind::MotionSpline, NodeKind::MotionBezier, NodeKind::MotionCurve, NodeKind::MotionPause, NodeKind::MotionSequence,
        NodeKind::MotionParallel, NodeKind::MotionRepeat, NodeKind::MotionFrames, NodeKind::CharacterCompose, NodeKind::CharacterAttributes, NodeKind::LayeredImage,
        NodeKind::ImageLayer, NodeKind::ImageLayers, NodeKind::VideoPlay, NodeKind::VideoPause, NodeKind::VideoResume, NodeKind::VideoStop,
        NodeKind::VideoSkip, NodeKind::VideoSeek, NodeKind::VideoVolume, NodeKind::VideoWait, NodeKind::VideoClip, NodeKind::AccessibilityConfigure,
        NodeKind::AccessibilitySpeak, NodeKind::AccessibilityStop, NodeKind::FunctionReturn, NodeKind::Sequence, NodeKind::ForLoopWithBreak, NodeKind::While,
        NodeKind::ForEach, NodeKind::Use, NodeKind::Init, NodeKind::Config, NodeKind::CharacterCreate, NodeKind::Dialogue,
        NodeKind::Choice, NodeKind::SetVariable, NodeKind::If, NodeKind::Label, NodeKind::Jump, NodeKind::Call,
        NodeKind::Return, NodeKind::Scene, NodeKind::CinematicShow, NodeKind::CinematicHide, NodeKind::UnlockEnding, NodeKind::SpriteShow,
        NodeKind::SpriteHide, NodeKind::SpriteMove, NodeKind::SpriteAnimate, NodeKind::SpriteStopAnimation, NodeKind::SpriteEffect, NodeKind::Timer,
        NodeKind::TimerCancel, NodeKind::MethodCall, NodeKind::MusicPlay, NodeKind::MusicStop, NodeKind::MusicVolume, NodeKind::SfxPlay,
        NodeKind::SfxStop, NodeKind::VoicePlay, NodeKind::VoiceStop, NodeKind::Imagemap, NodeKind::TypewriterSet, NodeKind::TypewriterSpeed,
        NodeKind::MakeColor, NodeKind::Literal, NodeKind::TextValue, NodeKind::FormatText, NodeKind::Reroute, NodeKind::CharacterValue,
        NodeKind::LabelValue, NodeKind::PositionValue, NodeKind::SceneAsset, NodeKind::SpriteAsset, NodeKind::MusicAsset, NodeKind::SoundEffectAsset,
        NodeKind::VoiceAsset, NodeKind::CinematicAsset, NodeKind::HoverImageAsset, NodeKind::ScriptAsset, NodeKind::TransitionNone, NodeKind::TransitionFade,
        NodeKind::TransitionDissolve, NodeKind::TransitionSlideLeft, NodeKind::TransitionSlideRight, NodeKind::TransitionSlideUp, NodeKind::TransitionSlideDown, NodeKind::TransitionZoomIn,
        NodeKind::TransitionZoomOut, NodeKind::TransitionWipe, NodeKind::TransitionBlur, NodeKind::VariableGet, NodeKind::ConvertBoolToInt, NodeKind::ConvertBoolToFloat,
        NodeKind::ConvertIntToBool, NodeKind::ConvertIntToFloat, NodeKind::ConvertNumberToText, NodeKind::ConvertTextToInt, NodeKind::ConvertStringToText, NodeKind::ConvertTextToString,
        NodeKind::ConvertStringToName, NodeKind::ConvertNameToString, NodeKind::VariableReference, NodeKind::BinaryOperator, NodeKind::UnaryOperator, NodeKind::MathAdd,
        NodeKind::StringAppend, NodeKind::MathSubtract, NodeKind::MathMultiply, NodeKind::MathDivide, NodeKind::MathEqual, NodeKind::MathNotEqual,
        NodeKind::MathLess, NodeKind::MathLessEqual, NodeKind::MathGreater, NodeKind::MathGreaterEqual, NodeKind::LogicAnd, NodeKind::LogicOr,
        NodeKind::LogicNot, NodeKind::MathNegate, NodeKind::FunctionCall, NodeKind::ListLiteral, NodeKind::Index, NodeKind::BranchEnd,
    ];
    let legacy:Vec<_>=ALL_NODE_KINDS.iter().copied().filter(|kind|!matches!(kind,NodeKind::UnitCall|NodeKind::UnitReturn|NodeKind::DoOnce|NodeKind::DoN|NodeKind::FlipFlop|NodeKind::Gate)).collect();
    assert_eq!(legacy,LEGACY180);
    let added:Vec<_>=ALL_NODE_KINDS.iter().copied().filter(|kind|matches!(kind,NodeKind::UnitCall|NodeKind::UnitReturn)).collect();
    assert_eq!(added,[NodeKind::UnitCall,NodeKind::UnitReturn]);
    let macros:Vec<_>=ALL_NODE_KINDS.iter().copied().filter(|kind|matches!(kind,NodeKind::DoOnce|NodeKind::DoN|NodeKind::FlipFlop|NodeKind::Gate)).collect();
    assert_eq!(macros,[NodeKind::DoOnce,NodeKind::DoN,NodeKind::FlipFlop,NodeKind::Gate]);
    assert_eq!(ALL_NODE_KINDS.len(),LEGACY180.len()+added.len()+macros.len());
    let mut set=std::collections::HashSet::new();for kind in ALL_NODE_KINDS{assert!(set.insert(*kind));assert_eq!(node_definition(*kind).kind,*kind);}
    let call=node_definition(NodeKind::UnitCall);let finish=node_definition(NodeKind::UnitReturn);
    assert_eq!(call.category,NodeCategory::Flow);assert_eq!(finish.category,NodeCategory::Flow);
    assert!(!call.has_dynamic_pins);assert!(!finish.has_dynamic_pins);
    let signature=|definition:&NodeDefinition|definition.pins.iter().map(|pin|(pin.key,pin.direction,pin.value_type.clone(),pin.cardinality,pin.default_value.clone())).collect::<Vec<_>>();
    assert_eq!(signature(&call),[("exec_in",PinDirection::Input,ValueType::Execution,PinCardinality::One,None),("exec_out",PinDirection::Output,ValueType::Execution,PinCardinality::Many,None)]);
    assert_eq!(signature(&finish),[("exec_in",PinDirection::Input,ValueType::Execution,PinCardinality::One,None)]);
    // F12 actual catalogue: execution inputs accept fan-in, execution outputs
    // are single continuation ports, and data outputs permit fan-out. These
    // differ intentionally from the unchanged Unit signatures checked above.
    for (kind,expected) in [
        (NodeKind::DoOnce,vec![
            ("exec_in",PinDirection::Input,ValueType::Execution,PinCardinality::Many,None),
            ("reset",PinDirection::Input,ValueType::Execution,PinCardinality::Many,None),
            ("start_closed",PinDirection::Input,ValueType::Bool,PinCardinality::One,Some(PropertyValue::Bool(false))),
            ("completed",PinDirection::Output,ValueType::Execution,PinCardinality::One,None),
        ]),
        (NodeKind::DoN,vec![
            ("exec_in",PinDirection::Input,ValueType::Execution,PinCardinality::Many,None),
            ("n",PinDirection::Input,ValueType::Int,PinCardinality::One,Some(PropertyValue::Int(0))),
            ("reset",PinDirection::Input,ValueType::Execution,PinCardinality::Many,None),
            ("exit",PinDirection::Output,ValueType::Execution,PinCardinality::One,None),
            ("counter",PinDirection::Output,ValueType::Int,PinCardinality::Many,None),
        ]),
        (NodeKind::FlipFlop,vec![
            ("exec_in",PinDirection::Input,ValueType::Execution,PinCardinality::Many,None),
            ("a",PinDirection::Output,ValueType::Execution,PinCardinality::One,None),
            ("b",PinDirection::Output,ValueType::Execution,PinCardinality::One,None),
            ("is_a",PinDirection::Output,ValueType::Bool,PinCardinality::Many,None),
        ]),
        (NodeKind::Gate,vec![
            ("exec_in",PinDirection::Input,ValueType::Execution,PinCardinality::Many,None),
            ("open",PinDirection::Input,ValueType::Execution,PinCardinality::Many,None),
            ("close",PinDirection::Input,ValueType::Execution,PinCardinality::Many,None),
            ("toggle",PinDirection::Input,ValueType::Execution,PinCardinality::Many,None),
            ("start_closed",PinDirection::Input,ValueType::Bool,PinCardinality::One,Some(PropertyValue::Bool(true))),
            ("exit",PinDirection::Output,ValueType::Execution,PinCardinality::One,None),
        ]),
    ] {
        let definition=node_definition(kind);
        assert_eq!(definition.category,NodeCategory::Flow);
        assert!(!definition.has_dynamic_pins);
        assert_eq!(signature(&definition),expected,"real StandardMacro signature for {kind:?}");
    }
    let d=node_definition(NodeKind::ForLoopWithBreak);for key in ["first","last","index"]{assert_eq!(d.pins.iter().find(|p|p.key==key).unwrap().value_type,ValueType::Int);}
    for key in ["exec_in","break","body","completed"]{assert_eq!(d.pins.iter().find(|p|p.key==key).unwrap().value_type,ValueType::Execution);}
    assert!(node_definition(NodeKind::Sequence).has_dynamic_pins);
}
#[test]
fn visual_sequence_resize_keeps_exact_pin_ids_and_edges_and_refuses_invalid_count(){
    let mut g=documents("scene label start\n").remove(0);let seq=g.add_catalog_node(NodeKind::Sequence,[240.,80.]).unwrap();
    let first=g.pin_by_key(seq,"then_0").unwrap().id;g.resize_sequence_outputs(seq,4).unwrap();assert_eq!(g.pin_by_key(seq,"then_0").unwrap().id,first);
    let outputs:Vec<_>=g.nodes[&seq].pins.iter().map(|id|&g.pins[id]).filter(|pin|pin.direction==PinDirection::Output).map(|pin|pin.key.as_str()).collect();assert_eq!(outputs,["then_0","then_1","then_2","then_3","completed"]);
    let end=g.add_branch_end(seq,[400.,80.]).unwrap();wire(&mut g,seq,"then_3",end,"exec_in");let before=g.clone();
    assert!(g.resize_sequence_outputs(seq,0).is_err());assert!(g.resize_sequence_outputs(seq,33).is_err());assert_eq!(g,before);
    g.resize_sequence_outputs(seq,2).unwrap();assert_eq!(g.pin_by_key(seq,"then_0").unwrap().id,first);assert!(g.pin_by_key(seq,"then_3").is_none());assert!(g.edges.is_empty());
    let json=g.to_pretty_json().unwrap();assert_eq!(GraphDocument::from_json(&json).unwrap(),g);
}
#[test]
fn visual_sequence_selected_output_removal_retains_order_ids_edges_and_legacy_one_output() {
    for removed_index in [0usize, 2, 4] {
        let mut graph=documents("scene label start\n").into_iter().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap();
        let sequence=graph.add_catalog_node(NodeKind::Sequence,[240.,80.]).unwrap();
        graph.resize_sequence_outputs(sequence,5).unwrap();
        let outputs:Vec<_>=(0..5).map(|index|graph.pin_by_key(sequence,&format!("then_{index}")).unwrap().id).collect();
        // Historical catalogue outputs and resized outputs intentionally retain
        // their real metadata; removal must not require a normalization pass.
        for (index,pin) in outputs.iter().enumerate(){
            assert_eq!(graph.pins[pin].cardinality,if index<2{PinCardinality::Many}else{PinCardinality::One});
            assert!(graph.can_remove_sequence_output(*pin), "all real ordered branch pins are removable above the minimum");
        }
        let mut edges=Vec::new();
        for (index,pin) in outputs.iter().enumerate(){
            let end=graph.add_branch_end(sequence,[600.,index as f64*100.]).unwrap();
            edges.push(graph.connect(*pin,graph.pin_by_key(end,"exec_in").unwrap().id).unwrap());
        }
        let entry=graph.nodes.values().find(|node|matches!(node.kind,NodeKind::Label)).unwrap().id;
        let done=graph.add_catalog_node(NodeKind::Return,[1000.,100.]).unwrap();
        wire(&mut graph,entry,"exec_out",sequence,"exec_in");wire(&mut graph,sequence,"completed",done,"exec_in");
        let before=graph.clone();let removed=outputs[removed_index];
        graph.remove_sequence_output(removed).unwrap();
        assert!(!graph.pins.contains_key(&removed));assert!(!graph.edges.contains_key(&edges[removed_index]));
        let survivors:Vec<_>=outputs.iter().copied().filter(|pin|*pin!=removed).collect();
        for (index,pin) in survivors.iter().enumerate(){
            let mut expected=before.pins[pin].clone();expected.key=format!("then_{index}");expected.label=format!("Puis {index}");
            assert_eq!(graph.pin_by_key(sequence,&expected.key),Some(&expected));
        }
        for (index,edge) in edges.iter().enumerate(){if index!=removed_index{assert_eq!(graph.edges.get(edge),before.edges.get(edge));}}
        for (id,edge) in &before.edges{if edge.output!=removed&&edge.input!=removed{assert_eq!(graph.edges.get(id),Some(edge));}}
        assert_eq!(graph.nodes[&sequence].properties["output_count"],PropertyValue::Int(4));
        for key in ["exec_in","completed"]{let pin=before.pin_by_key(sequence,key).unwrap().id;assert_eq!(graph.pins[&pin],before.pins[&pin]);}
        for (id,node) in &before.nodes{if *id!=sequence{assert_eq!(graph.nodes.get(id),Some(node));}}
        let stable=graph.clone();assert!(graph.remove_sequence_output(removed).is_err());assert_eq!(graph,stable);
        let json=graph.to_pretty_json().unwrap();assert_eq!(GraphDocument::from_json(&json).unwrap(),graph);
    }
    // Observe the selected removal through emitted code, the real Engine,
    // recoverable SourceWorkspace Save and a cold reopen, not cached graph data.
    for removed_index in [0usize, 2, 4] {
        let root=std::env::temp_dir().join(format!("rvn-sequence-selected-save-{}-{}-{removed_index}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();let path=root.join("story.rvn");
        let source="// Retenir les branches et leurs identités\ninit {set order=\"\"}\nscene label start\nflow.sequence() {{set order=order+\"A\"} {set order=order+\"B\"} {set order=order+\"C\"} {set order=order+\"D\"} {set order=order+\"E\"}}\n\"[order]\"\nreturn\n";
        std::fs::write(&path,source).unwrap();let mut workspace=SourceWorkspace::link(&root,&path,&[]).unwrap();
        let mut baseline=Engine::from_program(workspace.project().resolved_program().unwrap(),TerminalRenderer,16).unwrap();
        baseline.enter_label("start").unwrap();
        assert_eq!(baseline.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:"ABCDE".into()}));
        assert_eq!(baseline.state.vars.get("order"),Some(&Value::Str("ABCDE".into())));
        let mut graphs=workspace.project().graphs().to_vec();
        let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
        let sequence=graph.nodes.values().find(|n|n.kind==NodeKind::Sequence).unwrap().id;
        let outputs:Vec<_>=(0..5).map(|index|graph.pin_by_key(sequence,&format!("then_{index}")).unwrap().id).collect();
        let before=graph.clone();let removed=outputs[removed_index];let graph_id=graph.graph_id;
        graph.remove_sequence_output(removed).unwrap();
        let edited=graph.clone();assert_eq!(edited.graph_id,before.graph_id);
        let survivors:Vec<_>=outputs.iter().copied().filter(|pin|*pin!=removed).collect();
        for (index,pin) in survivors.iter().enumerate(){
            assert_eq!(edited.pin_by_key(sequence,&format!("then_{index}")).unwrap().id,*pin);
        }
        for (id,edge) in &before.edges{
            if edge.output!=removed&&edge.input!=removed{assert_eq!(edited.edges.get(id),Some(edge));}
            else{assert!(!edited.edges.contains_key(id));}
        }
        assert_eq!(GraphDocument::from_json(&edited.to_pretty_json().unwrap()).unwrap(),edited);
        let expected:String=['A','B','C','D','E'].into_iter().enumerate().filter_map(|(index,ch)|(index!=removed_index).then_some(ch)).collect();
        let compiled=transpile_project(&graphs).unwrap();assert!(compiled.source.contains("flow.sequence()"));
        let emitted=rvn_parser::parse_program(&compiled.source).unwrap();
        let mut engine=Engine::from_program(emitted,TerminalRenderer,16).unwrap();engine.enter_label("start").unwrap();
        assert_eq!(engine.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:expected.clone()}));
        assert_eq!(engine.state.vars.get("order"),Some(&Value::Str(expected.clone())));
        workspace.save(&graphs).unwrap();let saved=std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("// Retenir les branches et leurs identités"));
        let omitted=format!("{{set order=order+\"{}\"}}",['A','B','C','D','E'][removed_index]);
        let expected_source=source.replace(&omitted,"");assert_ne!(saved,source);
        assert_eq!(rvn_parser::parse_program(&saved).unwrap().script,rvn_parser::parse_program(&expected_source).unwrap().script);
        let cold=SourceWorkspace::open(&root).unwrap().unwrap();assert!(cold.source_error.is_none());
        assert_eq!(cold.project().graphs(),workspace.project().graphs());
        let retained=cold.project().graphs().iter().find(|g|g.graph_id==graph_id).unwrap();
        assert_eq!(retained.graph_id,edited.graph_id);assert_eq!(retained.nodes.keys().collect::<Vec<_>>(),edited.nodes.keys().collect::<Vec<_>>());
        assert_eq!(retained.nodes[&sequence],edited.nodes[&sequence]);
        assert_eq!(retained.pins,edited.pins);assert_eq!(retained.edges,edited.edges);assert!(!retained.pins.contains_key(&removed));
        let cold_program=cold.project().resolved_program().unwrap();
        assert_eq!(cold_program.script,rvn_parser::parse_program(&saved).unwrap().script);
        let mut cold_engine=Engine::from_program(cold_program,TerminalRenderer,16).unwrap();cold_engine.enter_label("start").unwrap();
        assert_eq!(cold_engine.step_until_interaction().unwrap(),Some(Interaction::Dialogue{character:None,text:expected.clone()}));
        assert_eq!(cold_engine.state.vars.get("order"),Some(&Value::Str(expected)));
        std::fs::remove_dir_all(root).unwrap();
    }
    for count in [1usize,2] {
        let mut graph=documents("scene label start\n").into_iter().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap();let sequence=graph.add_catalog_node(NodeKind::Sequence,[0.,0.]).unwrap();
        graph.resize_sequence_outputs(sequence,count).unwrap();let pin=graph.pin_by_key(sequence,"then_0").unwrap().id;
        let before=graph.clone();let bytes=graph.to_pretty_json().unwrap();
        assert!(!graph.can_remove_sequence_output(pin));assert!(graph.remove_sequence_output(pin).is_err());assert_eq!(graph,before);
        assert_eq!(graph.to_pretty_json().unwrap(),bytes);assert_eq!(GraphDocument::from_json(&bytes).unwrap().to_pretty_json().unwrap(),bytes);
    }
    let mut graph=documents("scene label start\n").into_iter().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap();let sequence=graph.add_catalog_node(NodeKind::Sequence,[0.,0.]).unwrap();graph.resize_sequence_outputs(sequence,3).unwrap();
    for key in ["exec_in","completed"]{let pin=graph.pin_by_key(sequence,key).unwrap().id;let before=graph.clone();assert!(graph.remove_sequence_output(pin).is_err());assert_eq!(graph,before);}
    let canonical=graph.clone();
    let pin=graph.pin_by_key(sequence,"then_1").unwrap().id;graph.pins.get_mut(&pin).unwrap().key="then_9".into();let before=graph.clone();
    assert!(!graph.can_remove_sequence_output(pin));assert!(graph.remove_sequence_output(pin).is_err());assert_eq!(graph,before);
    // Both existing output cardinalities are legal; the complete ordered
    // structural invariant still rejects malformed owners/ports before edits.
    for cardinality in [PinCardinality::One,PinCardinality::Many] {
        let mut graph=canonical.clone();
        for id in graph.nodes[&sequence].pins.clone(){if graph.pins[&id].key.starts_with("then_"){graph.pins.get_mut(&id).unwrap().cardinality=cardinality;}}
        assert!(graph.can_remove_sequence_output(pin));
    }
    for malformed in 0..6 {
        let mut graph=canonical.clone();
        match malformed {
            0=>graph.pins.get_mut(&pin).unwrap().direction=PinDirection::Input,
            1=>graph.pins.get_mut(&pin).unwrap().value_type=ValueType::Any,
            2=>graph.nodes.get_mut(&sequence).unwrap().pins.push(pin),
            3=>graph.nodes.get_mut(&sequence).unwrap().pins.retain(|id|*id!=pin),
            4=>{graph.nodes.get_mut(&sequence).unwrap().properties.insert("output_count".into(),PropertyValue::Int(4));},
            _=>graph.pins.get_mut(&pin).unwrap().node=rvn_graph::NodeId::new(u64::MAX),
        }
        let before=graph.clone();let bytes=graph.to_pretty_json().unwrap();
        assert!(!graph.can_remove_sequence_output(pin), "malformed case {malformed}");
        assert!(graph.remove_sequence_output(pin).is_err());assert_eq!(graph,before);
        assert_eq!(graph.to_pretty_json().unwrap(),bytes);
    }
}

#[test]
fn actual_loop_graph_index_wire_break_and_completed_compile_and_run(){
    let mut g=documents("scene label start\n\"done [seen]\"\n").into_iter().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).expect("actual start label graph");
    g.add_variable("seen",ValueType::Int,PropertyValue::Int(0)).unwrap();
    let entry=g.nodes.values().find(|n|n.kind==NodeKind::Label).unwrap().id;let dialogue=g.nodes.values().find(|n|n.kind==NodeKind::Dialogue).unwrap().id;
    let root=g.pin_by_key(entry,"exec_out").unwrap().id;let old=g.edges.values().find(|e|e.output==root).unwrap().id;g.remove_edge(old);
    let loop_node=g.add_catalog_node(NodeKind::ForLoopWithBreak,[200.,0.]).unwrap();let set=g.add_catalog_node(NodeKind::SetVariable,[400.,0.]).unwrap();
    g.set_pin_default(loop_node,"first",PropertyValue::Int(7)).unwrap();g.set_pin_default(loop_node,"last",PropertyValue::Int(9)).unwrap();
    g.set_property(set,"name",PropertyValue::String("seen".into())).unwrap();g.set_pin_default(set,"name",PropertyValue::String("seen".into())).unwrap();
    wire(&mut g,entry,"exec_out",loop_node,"exec_in");wire(&mut g,loop_node,"body",set,"exec_in");wire(&mut g,loop_node,"index",set,"value");wire(&mut g,set,"exec_out",loop_node,"break");wire(&mut g,loop_node,"completed",dialogue,"exec_in");
    let compiled=transpile(&g).unwrap();assert!(compiled.source.contains("flow.break()"));assert!(compiled.source.contains("flow.for_loop(index, 7, 9)"));
    let mut engine=Engine::new(compiled.ast,TerminalRenderer,16).unwrap();assert!(matches!(engine.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="done 7"));assert_eq!(engine.state.vars["seen"],Value::Int(7));
}
#[test]
fn structured_flow_roundtrip_keeps_nodes_order_scope_and_real_origins(){
    let source="function total() {set n=0\nflow.sequence() {{flow.for_loop(i,-1,2) {for item in [1] {while i==1 {flow.break()}\nset n=n+i}}} {} {set n=n+10}}\nreturn n}\nscene label start\n\"[total()]\"\n";
    let mut docs=documents(source);
    for _ in 0..3 {let output=transpile_project(&docs).unwrap();let bytes=serde_json::to_vec(&output.ast).unwrap();assert_eq!(serde_json::from_slice::<Vec<rvn_parser::Statement>>(&bytes).unwrap(),output.ast);let function=docs.iter().find(|g|matches!(g.kind,GraphKind::Function{..})).unwrap();
        assert!(function.nodes.values().any(|n|n.kind==NodeKind::Sequence));assert!(function.nodes.values().any(|n|n.kind==NodeKind::ForLoopWithBreak));assert_eq!(function.variable_scope("i"),Some(VariableScope::Local));
        let mut engine=Engine::from_program(parse_program(&output.source).unwrap(),TerminalRenderer,16).unwrap();assert!(matches!(engine.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="9"));
        docs=import_script(&output.ast).unwrap();
        for g in &docs {let json=g.to_pretty_json().unwrap();assert_eq!(GraphDocument::from_json(&json).unwrap(),*g);}
    }
}
#[test]
fn typed_loop_float_first_and_foreign_break_cycle_refuse_without_editing_graph(){
    let mut g=documents("scene label start\n").into_iter().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).expect("actual start label graph");let range=g.add_catalog_node(NodeKind::ForLoopWithBreak,[200.,0.]).unwrap();let value=g.add_catalog_node(NodeKind::Literal,[0.,100.]).unwrap();
    g.set_property(value,"value",PropertyValue::Float(0.5)).unwrap();let before=g.clone();let out=g.pin_by_key(value,"value").unwrap().id;let first=g.pin_by_key(range,"first").unwrap().id;assert!(g.connect(out,first).is_err());assert_eq!(g,before);
    let entry=g.nodes.values().find(|n|n.kind==NodeKind::Label).unwrap().id;wire(&mut g,entry,"exec_out",range,"break");let before=g.clone();assert!(transpile(&g).is_err());assert_eq!(g,before);
}

// Private post-F38 corpus exporter. This ignored authoring tool is deliberately
// outside selected155. No runtime host, UiOpen, fake Scene batch, or input is used.
#[test]
#[ignore = "Root-only actual SourceWorkspace corpus export after F38; requires existing empty owned D directory"]
fn viewport1003_export_actual_two_instance_native_corpus() {
    use std::{fs,io::Write,path::{Path,PathBuf}};
    use rvn_parser::widget_asset::{WidgetBlueprintV1,WidgetGraphRef16,WidgetIndexInput,LocatorCase};
    const CLASS:&str="9c203075eccb472ead4edc2d059cbf35";
    const LOCATOR:&str="widgets/ViewportCounter1003.rvnwidget";
    const SEED:&str=r###"// Owned RVN functional corpus1003. No Epic content or native parity claim.
handler press_left(event){set left_clicks=left_clicks+1}
handler press_right(event){set right_clicks=right_clicks+1}
handler init_left(event){set left_initialized=left_initialized+1}
handler init_right(event){set right_initialized=right_initialized+1}
handler construct_left(event){set left_constructed=left_constructed+1}
handler construct_right(event){set right_constructed=right_constructed+1}
handler destruct_left(event){set left_destructed=left_destructed+1}
handler destruct_right(event){set right_destructed=right_destructed+1}
screen ViewportCounter1003(method,caption,x,on_init,on_construct,on_destruct){
 local message=caption
 if method=="press_left"{set message=caption+" clics="+left_clicks+" I="+left_initialized+" C="+left_constructed+" D="+left_destructed}
 else{set message=caption+" clics="+right_clicks+" I="+right_initialized+" C="+right_constructed+" D="+right_destructed}
 return {"host":{"viewport_slot":{"anchors":[0,0,0,0],"offsets":{"left":x,"top":100,"right":360,"bottom":120}},"events":{"initialized":on_init,"construct":on_construct,"destruct":on_destruct}},"root":component("action","button",{"layout_contract":"widget_v1","events":{"click":method}},[component("caption","text",{"text":message,"parent_slot":{"kind":"button","horizontal":"center","vertical":"center","padding":{"left":4,"top":2,"right":4,"bottom":2}}},[])])}
init{set left_clicks=0 set right_clicks=0 set left_initialized=0 set right_initialized=0 set left_constructed=0 set right_constructed=0 set left_destructed=0 set right_destructed=0}
scene label start
set left_object=0
set right_object=0
"01 Creees : deux objets detaches. Continuer pour les afficher."
"02 Affichees : cliquer gauche deux fois puis droite une fois. Continuer pour retirer gauche."
"03 Gauche retiree : droite reste interactive. Cliquer droite une fois. Continuer pour reafficher gauche."
"04 Gauche revenue : ses valeurs sont conservees. Sauvegarder puis reprendre avant de continuer."
"05 Toutes retirees : aucun bouton ne doit rester."
return
"###;
    fn create(path:&Path,bytes:&[u8]) {let mut file=fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap();file.write_all(bytes).unwrap();file.sync_all().unwrap();}
    fn wire(graph:&mut GraphDocument,from:NodeId,output:&str,to:NodeId,input:&str) {
        graph.connect(graph.pin_by_key(from,output).unwrap().id,graph.pin_by_key(to,input).unwrap().id).unwrap();
    }
    fn insert(graph:&mut GraphDocument,before:NodeId,kind:NodeKind)->NodeId {
        let input=graph.pin_by_key(before,"exec_in").unwrap().id;
        let edge=graph.edges.values().find(|edge|edge.input==input).unwrap().clone();
        graph.remove_edge(edge.id);let node=graph.add_catalog_node(kind,[600.,0.]).unwrap();
        graph.connect(edge.output,graph.pin_by_key(node,"exec_in").unwrap().id).unwrap();wire(graph,node,"exec_out",before,"exec_in");node
    }
    let requested=PathBuf::from(std::env::var_os("RVN_WIDGET_VIEWPORT1003_CORPUS").expect("Root must name an existing empty owned D:/RVN-work directory"));
    assert!(requested.is_absolute()&&requested.is_dir()&&!requested.is_symlink());
    #[cfg(windows)]{use std::os::windows::fs::MetadataExt;assert_eq!(fs::symlink_metadata(&requested).unwrap().file_attributes()&0x400,0,"No reparse point/junction destination");}
    let root=fs::canonicalize(requested).unwrap();let allowed=fs::canonicalize("D:/RVN-work").unwrap();assert!(root.starts_with(&allowed)&&root!=allowed);
    assert_eq!(fs::read_dir(&root).unwrap().count(),0,"Never overwrite a project/corpus");
    fs::create_dir(root.join("assets")).unwrap();fs::create_dir(root.join("assets/widgets")).unwrap();fs::create_dir(root.join("locales")).unwrap();fs::create_dir(root.join("saves")).unwrap();
    let source_path=root.join("main.rvn");create(&source_path,SEED.as_bytes());
    // Build every producer/effect with real public Graph APIs BEFORE final
    // SourceProject publication and class/program admission. No raw append.
    let mut project=SourceProject::open_at(&source_path,SEED,&[]).unwrap();let mut graphs=project.graphs().to_vec();
    let graph=graphs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    graph.enable_native_frame_preparation().unwrap();
    let mut factories=Vec::new();
    for(binding,method,caption,x,initialized,constructed,destructed)in [
        ("left_object","press_left","GAUCHE",80,"init_left","construct_left","destruct_left"),
        ("right_object","press_right","DROITE",500,"init_right","construct_right","destruct_right")]{
        let assignment=graph.nodes.values().find(|n|n.kind==NodeKind::SetVariable&&n.properties.get("name")==Some(&PropertyValue::String(binding.into()))).unwrap().id;
        let class=rvn_parser::Expr::Call{name:"__rvn_widget_class_v1".into(),args:vec![rvn_parser::Expr::Str(CLASS.into())]};
        let arguments=rvn_parser::Expr::ListLit(vec![rvn_parser::Expr::Str(method.into()),rvn_parser::Expr::Str(caption.into()),rvn_parser::Expr::Int(x),rvn_parser::Expr::Str(initialized.into()),rvn_parser::Expr::Str(constructed.into()),rvn_parser::Expr::Str(destructed.into())]);
        let factory=graph.replace_assignment_with_widget_factory(assignment,&class,&arguments).unwrap();factories.push(factory);
    }
    assert_ne!(factories[0],factories[1]);
    for(stage,index,kind)in [("02",0,NodeKind::WidgetAddToViewport),("02",1,NodeKind::WidgetAddToViewport),("03",0,NodeKind::WidgetRemoveFromParent),("04",0,NodeKind::WidgetAddToViewport),("05",0,NodeKind::WidgetRemoveFromParent),("05",1,NodeKind::WidgetRemoveFromParent)]{
        let candidates=graph.nodes.values().filter(|node|{
            if node.kind!=NodeKind::Dialogue{return false;}
            let input=graph.pin_by_key(node.id,"text").expect("actual Dialogue text input");
            let incoming=graph.edges.values().filter(|edge|edge.input==input.id).collect::<Vec<_>>();
            let [edge]=incoming.as_slice()else{panic!("one authored Dialogue text wire");};
            let output=&graph.pins[&edge.output];let producer=&graph.nodes[&output.node];
            producer.kind==NodeKind::TextValue
                &&graph.pin_by_key(producer.id,"value").is_some_and(|pin|pin.id==output.id)
                &&matches!(producer.properties.get("value"),Some(PropertyValue::String(text))if text.starts_with(stage))
        }).map(|node|node.id).collect::<Vec<_>>();
        assert_eq!(candidates.len(),1,"one actual stage {stage} Dialogue literal producer");let before=candidates[0];
        let effect=insert(graph,before,kind);wire(graph,factories[index],"value_out",effect,"object");
        if kind==NodeKind::WidgetAddToViewport{graph.set_pin_default(effect,"z_order",PropertyValue::Int(0)).unwrap();}
    }
    let authored=graph.clone();assert_eq!(authored.nodes.values().filter(|n|n.kind==NodeKind::WidgetCreate).count(),2);
    assert_eq!(authored.nodes.values().filter(|n|n.kind==NodeKind::WidgetAddToViewport).count(),3);
    assert_eq!(authored.nodes.values().filter(|n|n.kind==NodeKind::WidgetRemoveFromParent).count(),3);
    project.apply_visual(SEED,&graphs).unwrap();let source=project.source().to_owned();
    // Replacing this owned seed happens before any SourceWorkspace/descriptor
    // admission; after this boundary, source remains byte-for-byte unchanged.
    fs::write(&source_path,&source).unwrap();
    let workspace=SourceWorkspace::link(&root,&source_path,project.graphs()).unwrap();
    let screen=workspace.project().graphs().iter().find(|g|matches!(&g.kind,GraphKind::Screen{name}if name=="ViewportCounter1003")).unwrap();
    let screen_id=WidgetGraphRef16::from_u64(screen.graph_id.get());
    let members=workspace.project().graphs().iter().filter(|g|g.graph_id==screen.graph_id||matches!(g.kind,GraphKind::Handler{..})).map(|g|WidgetGraphRef16::from_u64(g.graph_id.get())).collect::<Vec<_>>();assert_eq!(members.len(),9);
    let descriptor=WidgetBlueprintV1{uuid:CLASS.into(),source:"main.rvn".into(),screen_graph:screen_id,members}.encode().unwrap();
    create(&root.join("assets").join(LOCATOR),descriptor.as_bytes());
    create(&root.join("rvn.toml"),format!("[project]\ntitle='Deux Widgets - viewport1003'\nmain_script='main.rvn'\nstart_label='start'\n[paths]\nassets='assets'\nlocales='locales'\nsaves='saves'\ntheme='theme.toml'\n[widget_blueprints]\n'{CLASS}'='{LOCATOR}'\n").as_bytes());
    create(&root.join("theme.toml"),b"[title_screen]\nenabled=false\n");
    let program=workspace.project().resolved_program().unwrap();
    let classes=prepare_widget_runtime_classes(&root,&program,WidgetIndexInput::Table(&[(CLASS,Some(LOCATOR))]),LocatorCase::Insensitive,
        &[WidgetRuntimeAsset{locator:LOCATOR,bytes:descriptor.as_bytes()}],&[WidgetRuntimeSource{source:"main.rvn",project:workspace.project()}]).unwrap();
    assert_eq!(classes.classes().len(),1);assert_eq!(classes.classes().get(CLASS).unwrap().members().len(),9);
    let cold=SourceWorkspace::open(&root).unwrap().unwrap();assert_eq!(cold.project().source(),source);
    let actual=cold.project().graphs().iter().find(|g|g.graph_id==authored.graph_id).unwrap();
    assert_eq!(actual.nodes.keys().collect::<Vec<_>>(),authored.nodes.keys().collect::<Vec<_>>());assert_eq!(actual.pins.keys().collect::<Vec<_>>(),authored.pins.keys().collect::<Vec<_>>());assert_eq!(actual.edges,authored.edges);
    assert_eq!(fs::read_to_string(&source_path).unwrap(),source,"No source rewrite after source/class admission");
    let report=serde_json::json!({"state":"ACTUAL_AUTHORED_AND_COLD_REOPENED_SOURCE_CORPUS_ONLY_NOT_RUN_NATIVE","class_uuid":CLASS,"descriptor":LOCATOR,"source":"main.rvn","scene_graph":actual.graph_id.get(),"factory_nodes":factories.iter().map(|id|id.get()).collect::<Vec<_>>(),"node_ids":actual.nodes.keys().map(|id|id.get()).collect::<Vec<_>>(),"pin_ids":actual.pins.keys().map(|id|id.get()).collect::<Vec<_>>(),"edge_ids":actual.edges.keys().map(|id|id.get()).collect::<Vec<_>>(),"instances_expected":2,"same_class_expected":true,"native_input_executed":false,"backend_executed":false,"Runtime155_credit":0,"native_credit":0,"pixel_credit":0,"counters":"Scenario globals left/right are deliberately distinct; this is not a new per-instance UMG field system."});
    create(&root.join("corpus-report.private.json"),serde_json::to_vec_pretty(&report).unwrap().as_slice());
    println!("Owned authoring corpus exported to {}",root.display());
}
