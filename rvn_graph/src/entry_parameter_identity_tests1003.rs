//! Prepared real Unit source/Graph transaction tests; execution requires Root slot.
use super::*;
use crate::{GraphKind, PinCardinality};

struct Fixture {
    root: PathBuf,
    workspace: SourceWorkspace,
    graphs: Vec<GraphDocument>,
    declaration: GraphId,
    entry: NodeId,
    parameters: [PinId; 2],
    identities: Vec<ParameterId>,
    caller: GraphId,
    call: NodeId,
    arguments: Vec<PinId>,
}

impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.root); }
}

impl Fixture {
    fn new(with_arguments: bool) -> Self {
        let root = std::env::temp_dir().join(format!("rvn-entry-pin-binding-{}-{}",
            std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("main.rvn");
        let source=if with_arguments {
            "init {set result=0}\nfunction scoped(alpha,beta) -> () {set result=alpha*10+beta}\nscene label start\ncall scoped(10,11)\nreturn\n"
        } else {"function scoped(alpha,beta) -> () {}\nscene label start\nreturn\n"};
        std::fs::write(&path,source).unwrap();
        let mut allocated=0;
        let prepared=crate::import_script_with_parameter_allocator(&rvn_parser::parse(source).unwrap(),|| {
            allocated+=1;Ok(format!("{allocated:032x}"))
        }).unwrap();
        let workspace=SourceWorkspace::link(&root,&path,&prepared).unwrap();
        let graphs=workspace.project().graphs().to_vec();
        let graph=graphs.iter().find(|graph|matches!(&graph.kind,GraphKind::UnitFunction{name}if name=="scoped")).unwrap();
        let declaration=graph.graph_id;
        let entry=crate::unit_graph_source1003::entry(graph).unwrap();
        let pins=crate::unit_graph_source1003::parameters(graph).unwrap();
        let parameters=pins.iter().map(|pin|pin.id).collect::<Vec<_>>();
        let identities=pins.iter().map(|pin|pin.parameter_id.clone().unwrap()).collect::<Vec<_>>();
        let graph=graphs.iter().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="start")).unwrap();
        let caller=graph.graph_id;
        let call=graph.nodes.values().find(|node|node.kind==NodeKind::UnitCall).map(|node|node.id).unwrap_or(NodeId::new(0));
        let arguments=if with_arguments {crate::unit_graph_source1003::resolve_call(&graphs,graph,call).unwrap().1.iter().map(|pin|pin.id).collect()}else{vec![]};
        Self { root, workspace, graphs, declaration, entry,
            parameters: [parameters[0], parameters[1]], identities, caller, call, arguments }
    }

    fn view<'a>(&'a self, graphs: &'a [GraphDocument]) -> EntryParameters<'a> {
        EntryParameters::from_admitted_projection(&self.workspace, graphs, self.declaration, self.entry).unwrap()
    }
}

#[test]
fn entry_pin_identity_rename_and_reorder_preserve_real_argument_bindings_and_bytes() {
    let fixture = Fixture::new(true);
    let before = fixture.graphs.clone();
    let source = std::fs::read(fixture.workspace.source_path()).unwrap();
    let sidecar = std::fs::read(fixture.root.join(".rvn-authoring.json")).unwrap();
    let view = fixture.view(&fixture.graphs);
    let bindings = view.bindings().unwrap();
    assert_eq!(view.ordered_arguments(fixture.caller, fixture.call).unwrap().iter().map(|pin| pin.id).collect::<Vec<_>>(), fixture.arguments);
    let renamed = view.rename_projection(&bindings[0], "renamed_alpha").unwrap();
    assert_eq!(fixture.graphs, before);
    let renamed_view = fixture.view(&renamed);
    assert_eq!(renamed_view.bindings().unwrap(), bindings);
    assert_eq!(renamed_view.pins().unwrap()[0].label, "renamed_alpha");
    assert!(renamed_view.reorder_projection(&bindings).is_err(), "identity equality does not authorize a stale observed graph");
    let current = renamed_view.bindings().unwrap();
    let mapping = bindings.iter().cloned().zip(current.iter().cloned()).collect::<Vec<_>>();
    view.require_preserved_identities(&renamed, &mapping).unwrap();
    assert!(view.require_preserved_identities(&renamed, &[]).is_err());
    let reordered = renamed_view.reorder_projection(&[current[1].clone(), current[0].clone()]).unwrap();
    let reordered_view = fixture.view(&reordered);
    assert_eq!(reordered_view.bindings().unwrap(), vec![bindings[1].clone(), bindings[0].clone()]);
    assert_eq!(reordered_view.ordered_arguments(fixture.caller, fixture.call).unwrap().iter().map(|pin| pin.id).collect::<Vec<_>>(), vec![fixture.arguments[1], fixture.arguments[0]]);
    for old in &before {
        let next = reordered.iter().find(|graph| graph.graph_id == old.graph_id).unwrap();
        assert_eq!(next.edges, old.edges);
        assert_eq!(next.pins.keys().collect::<Vec<_>>(), old.pins.keys().collect::<Vec<_>>());
        for (id, pin) in &old.pins {
            let mut expected = pin.clone();
            if (old.graph_id == fixture.declaration && *id == fixture.parameters[0])
                || pin.key == format!("argument_{}", fixture.identities[0].as_str()) { expected.label = "renamed_alpha".into(); }
            assert_eq!(next.pins[id], expected);
        }
    }
    let cold = reordered.iter().map(|graph| GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap()).collect::<Vec<_>>();
    let cold_view = fixture.view(&cold);
    assert_eq!(cold_view.pins().unwrap().iter().map(|pin| pin.label.as_str()).collect::<Vec<_>>(), vec!["beta", "renamed_alpha"]);
    assert_eq!(cold_view.bindings().unwrap(), reordered_view.bindings().unwrap());
    assert_eq!(cold_view.ordered_arguments(fixture.caller, fixture.call).unwrap().iter().map(|pin| pin.id).collect::<Vec<_>>(), vec![fixture.arguments[1], fixture.arguments[0]]);
    assert_eq!(std::fs::read(fixture.workspace.source_path()).unwrap(), source);
    assert_eq!(std::fs::read(fixture.root.join(".rvn-authoring.json")).unwrap(), sidecar);
}

#[test]
fn entry_pin_identity_rejects_duplicate_names_bad_keys_owners_and_incomplete_reorders() {
    let fixture = Fixture::new(false); let before = fixture.graphs.clone();
    let view = fixture.view(&fixture.graphs); let bindings = view.bindings().unwrap();
    assert!(view.rename_projection(&bindings[0], "beta").is_err());
    // Semantic parameter names follow the existing quoted-binding grammar.
    // Spaces and keyword spellings are valid; these shapes are truly invalid.
    for invalid in ["", "not.a.name", "line\nbreak", "__rvn_hidden"] {
        assert!(rvn_parser::variable_binding_source_name(invalid).is_none());
        assert!(view.rename_projection(&bindings[0], invalid).is_err());
    }
    assert!(view.reorder_projection(&[bindings[0].clone(), bindings[0].clone()]).is_err());
    assert!(view.reorder_projection(&bindings[..1]).is_err());
    let foreign = Fixture::new(false); let foreign_binding = foreign.view(&foreign.graphs).bindings().unwrap().remove(0);
    assert!(view.rename_projection(&foreign_binding, "renamed").is_err());
    for case in 0..6 {
        let mut malformed = before.clone();
        let graph = malformed.iter_mut().find(|graph| graph.graph_id == fixture.declaration).unwrap();
        match case {
            0 => graph.pins.get_mut(&fixture.parameters[0]).unwrap().key = "parameter_999999".into(),
            1 => graph.pins.get_mut(&fixture.parameters[0]).unwrap().direction = PinDirection::Input,
            2 => graph.nodes.get_mut(&fixture.entry).unwrap().pins.push(fixture.parameters[0]),
            3 => graph.pins.get_mut(&fixture.parameters[1]).unwrap().label = "alpha".into(),
            4 => graph.pins.get_mut(&fixture.parameters[0]).unwrap().default_value = Some(PropertyValue::Int(1)),
            _ => graph.pins.get_mut(&fixture.parameters[0]).unwrap().cardinality = PinCardinality::One,
        }
        let snapshot = malformed.clone();
        assert!(EntryParameters::from_admitted_projection(&fixture.workspace, &malformed, fixture.declaration, fixture.entry).is_err());
        assert_eq!(malformed, snapshot);
    }
    assert_eq!(fixture.graphs, before);
}

#[test]
fn entry_pin_identity_removal_refuses_real_body_and_closed_caller_references() {
    let fixture = Fixture::new(true); let before = fixture.graphs.clone();
    let view = fixture.view(&fixture.graphs); let bindings = view.bindings().unwrap();
    assert!(view.remove_unreferenced_projection(&bindings[1]).is_err());
    assert_eq!(fixture.graphs, before);
    let isolated = Fixture::new(false); let mut graphs = isolated.graphs.clone();
    let graph = graphs.iter_mut().find(|graph| graph.graph_id == isolated.declaration).unwrap();
    let sink = graph.add_catalog_node(NodeKind::MathAdd, [400., 100.]).unwrap();
    let edge = graph.connect(isolated.parameters[0], graph.pin_by_key(sink, "left").unwrap().id).unwrap();
    let before = graphs.clone(); let view = isolated.view(&graphs); let bindings = view.bindings().unwrap();
    assert!(view.remove_unreferenced_projection(&bindings[0]).is_err());
    let removed = view.remove_unreferenced_projection(&bindings[1]).unwrap();
    assert_eq!(graphs, before);
    let next = removed.iter().find(|graph| graph.graph_id == isolated.declaration).unwrap();
    let previous = before.iter().find(|graph| graph.graph_id == isolated.declaration).unwrap();
    assert!(!next.pins.contains_key(&isolated.parameters[1]));
    assert_eq!(next.pins[&isolated.parameters[0]], previous.pins[&isolated.parameters[0]]);
    assert_eq!(next.edges[&edge], previous.edges[&edge]);
}

#[test]
fn entry_pin_identity_json_preserves_binding_but_plain_source_import_needs_explicit_mapping() {
    let fixture = Fixture::new(true); let view = fixture.view(&fixture.graphs);
    let cold = fixture.graphs.iter().map(|graph| GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap()).collect::<Vec<_>>();
    let mapping = view.bindings().unwrap().into_iter().map(|binding| (binding.clone(), binding)).collect::<Vec<_>>();
    view.require_preserved_identities(&cold, &mapping).unwrap();
    // Equal numeric IDs in a freshly imported candidate are not provenance.
    assert!(view.require_preserved_identities(&cold, &[]).is_err());
    assert_eq!(fixture.view(&cold).bindings().unwrap(), view.bindings().unwrap());
    let text=std::fs::read_to_string(fixture.workspace.source_path()).unwrap();
    assert!(crate::import_script(&rvn_parser::parse(&text).unwrap()).is_err());
    let mut next=100;
    let imported=crate::import_script_with_parameter_allocator(&rvn_parser::parse(&text).unwrap(),||{next+=1;Ok(format!("{next:032x}"))}).unwrap();
    assert!(view.require_preserved_identities(&imported,&mapping).is_err());
}

#[test]
fn entry_pin_identity_call_rejects_missing_foreign_arguments_and_data_results() {
    let fixture = Fixture::new(true); let before = fixture.graphs.clone();
    for case in 0..3 {
        let mut malformed = before.clone();
        let graph = malformed.iter_mut().find(|graph| graph.graph_id == fixture.caller).unwrap();
        match case {
            0 => { let pin = fixture.arguments[1]; graph.nodes.get_mut(&fixture.call).unwrap().pins.retain(|id| *id != pin); graph.pins.remove(&pin); },
            1 => graph.pins.get_mut(&fixture.arguments[1]).unwrap().key = "argument_999999_1".into(),
            _ => { graph.add_pin(fixture.call, "result", "result", PinDirection::Output, ValueType::Any, PinCardinality::Many).unwrap(); },
        }
        let snapshot = malformed.clone();
        assert!(EntryParameters::from_admitted_projection(&fixture.workspace, &malformed, fixture.declaration, fixture.entry).is_err());
        assert_eq!(malformed, snapshot);
    }
    assert_eq!(fixture.graphs, before);
}

#[test]
fn opaque_parameter_identity_survives_a_divergent_undo_despite_layout_pin_reuse() {
    let fixture = Fixture::new(false);
    let mut baseline = fixture.workspace.project().graphs().to_vec();
    baseline.iter_mut().find(|graph| graph.graph_id == fixture.declaration).unwrap().schema_version = GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION;
    let view = fixture.view(&baseline);
    let (first, first_id) = view.append_projection("first_history_branch", || Ok("11111111111111111111111111111111".into())).unwrap();
    let first_view = fixture.view(&first); let stale = first_view.bindings().unwrap().pop().unwrap();
    let first_pin = first_view.pins().unwrap().last().unwrap().id;
    // Same historical allocator, new transaction-supplied OS identity. These
    // fixed IDs are test injection only; Graph code creates no RNG/service.
    let (divergent, next_id) = view.append_projection("different_parameter", || Ok("22222222222222222222222222222222".into())).unwrap();
    let next = fixture.view(&divergent);
    assert_eq!(next.pins().unwrap().last().unwrap().id, first_pin);
    assert_ne!(first_id, next_id);assert_ne!(next.bindings().unwrap().last().unwrap(), &stale);
    assert!(next.rename_projection(&stale, "must_not_retarget").is_err());
    let cold = divergent.iter().map(|graph| GraphDocument::from_json(&graph.to_pretty_json().unwrap()).unwrap()).collect::<Vec<_>>();
    assert_eq!(fixture.view(&cold).pins().unwrap().last().unwrap().parameter_id, Some(next_id));
}

#[test]
fn entry_pin_identity_legacy_metadata_is_never_materialized_or_reserialized_by_a_probe() {
    let graphs=crate::import_script(&rvn_parser::parse("function legacy(alpha){return alpha}\nlabel start\nreturn").unwrap()).unwrap();
    let bytes=graphs.iter().map(|graph|graph.to_pretty_json().unwrap()).collect::<Vec<_>>();
    assert!(graphs.iter().all(|graph|graph.schema_version<GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION));
    assert!(bytes.iter().all(|bytes|!bytes.contains("parameter_id")));
    let cold=bytes.iter().map(|bytes|GraphDocument::from_json(bytes).unwrap()).collect::<Vec<_>>();
    assert_eq!(cold,graphs);
    assert_eq!(cold.iter().map(|graph|graph.to_pretty_json().unwrap()).collect::<Vec<_>>(),bytes);
}

#[test]
fn opaque_parameter_id_strict_serde_and_legacy_schema_gate() {
    let valid = "b9ab9b6d1c9448ad8dcd799c23b4b550";
    let id = ParameterId::parse(valid).unwrap();
    assert_eq!(serde_json::to_string(&id).unwrap(), format!("\"{valid}\""));
    assert_eq!(serde_json::from_str::<ParameterId>(&format!("\"{valid}\"")).unwrap(), id);
    for invalid in ["", "00000000000000000000000000000000", "B9AB9B6D1C9448AD8DCD799C23B4B550", "not-an-id", "1111111111111111111111111111111"] {
        assert!(ParameterId::parse(invalid).is_err());
        assert!(serde_json::from_value::<ParameterId>(serde_json::Value::String(invalid.into())).is_err());
    }
    let fixture = Fixture::new(false);let mut malformed = fixture.graphs.clone();
    let graph = malformed.iter_mut().find(|graph| graph.graph_id == fixture.declaration).unwrap();
    graph.schema_version = 13;
    assert!(validate_graph_parameter_metadata(graph).is_err());
    assert!(GraphDocument::from_json(&graph.to_pretty_json().unwrap()).is_err());
}

#[test]
fn opaque_parameter_prepare_is_injected_atomic_and_identity_is_immutable() {
    let called = Fixture::new(true);
    assert!(called.view(&called.graphs).append_projection("gamma", || panic!("existing caller arguments need an explicit transaction before allocation")).is_err());
    let fixture = Fixture::new(false);let before = fixture.graphs.clone();
    let source = std::fs::read(fixture.workspace.source_path()).unwrap();
    let sidecar = std::fs::read(fixture.root.join(".rvn-authoring.json")).unwrap();
    let view = fixture.view(&fixture.graphs);
    assert!(view.append_projection("alpha", || panic!("invalid edit must not allocate")).is_err());
    assert!(view.append_projection("gamma", || Err("OS allocation unavailable".into())).is_err());
    assert!(view.append_projection("gamma", || Ok(fixture.identities[0].as_str().into())).is_err());
    let (prepared, id) = view.append_projection("gamma", || Ok("33333333333333333333333333333333".into())).unwrap();
    let prepared_view = fixture.view(&prepared);
    assert_eq!(prepared_view.pins().unwrap().last().unwrap().parameter_id, Some(id));
    validate_parameter_identity_edit(&before, &prepared).unwrap();
    let mut changed = before.clone();
    let graph = changed.iter_mut().find(|graph| graph.graph_id == fixture.declaration).unwrap();
    let port = graph.pins.get_mut(&fixture.parameters[0]).unwrap();
    let replacement = ParameterId::parse("44444444444444444444444444444444").unwrap();
    port.key = format!("parameter_{}", replacement.as_str());port.parameter_id = Some(replacement);
    assert!(validate_parameter_identity_edit(&before, &changed).is_err());
    assert_eq!(fixture.graphs, before);
    assert_eq!(std::fs::read(fixture.workspace.source_path()).unwrap(), source);
    assert_eq!(std::fs::read(fixture.root.join(".rvn-authoring.json")).unwrap(), sidecar);
}
