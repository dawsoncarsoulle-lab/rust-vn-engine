use rvn_graph::*;
use rvn_core::{Engine,Interaction,TerminalRenderer};
fn graph(source:&str)->GraphDocument{
    import_script(&rvn_parser::parse_program(source).unwrap().script).unwrap().into_iter()
        .find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap()
}
fn initial(g:&GraphDocument,name:&str)->LocalVariableInitializer{g.local_variable_initializers(name).unwrap().remove(0)}

#[test]
fn scene_boundary_type_embedded_reference_confirmation_uses_binding_and_parsed_text() {
    for (source, expected) in [
        ("init {set value=true}\nscene label start\n\"Global [value]\"\nlocal value=true\n\"Plain value\"\nreturn\n",false),
        ("scene label start\nlocal value=true\n\"Local [value]\"\nreturn\n",true),
        ("scene label start\nlocal value=true\nchoice {\"Choice [value]\" => {return}}\n",true),
    ] {
        let g=graph(source);let before=g.clone();
        let p=g.prepare_local_variable_type_slot("value",ValueType::Int).unwrap();
        assert_eq!(p.requires_confirmation(),expected);
        let next=p.reconstruct(&g).unwrap();assert_eq!(g,before);
        for (id,node) in &g.nodes {
            if matches!(node.kind,NodeKind::TextValue|NodeKind::Choice){assert_eq!(&next.nodes[id],node);}
        }
    }
}

#[test]
fn type_proposal_separates_homonymous_global_and_local_references_without_mutation() {
    let g=graph("init {set value=true}\nscene label start\nset before=value\nlocal value=2\nset after=value\nreturn\n");
    let declaration=initial(&g,"value");let before=g.clone();
    let projection=g.variable_binding_projection("value").unwrap();
    let proposal=g.prepare_local_variable_type("value",&declaration,ValueType::Float).unwrap();
    assert_eq!(g,before);assert_eq!(proposal.previous_type(),&ValueType::Int);
    assert_eq!(proposal.target_type(),&ValueType::Float);assert!(!proposal.is_noop());
    assert!(proposal.requires_confirmation());
    let mut globals=0;let mut locals=0;
    for (pin,context) in projection.read_contexts {
        match context {
            VariableBindingContext::ProjectGlobal => {globals+=1;assert!(proposal.foreign_pins().contains(&pin));assert!(!proposal.local_pins().contains(&pin));}
            VariableBindingContext::ActivationLocal => {locals+=1;assert!(proposal.local_pins().contains(&pin));assert!(!proposal.foreign_pins().contains(&pin));}
            VariableBindingContext::GlobalOrLocal => panic!("fixture should have resolved consumers"),
        }
    }
    assert!(globals>0&&locals>0,"both actual domains must be exercised");
    assert!(proposal.shared_pins().is_empty()&&proposal.unresolved_pins().is_empty());
    assert!(!proposal.requires_project_analysis());proposal.validate_against(&g).unwrap();
}

#[test]
fn type_proposal_captures_all_declarations_full_types_and_full_graph_guard() {
    let g=graph("scene label start\nlocal value=true\nlocal value=false\nreturn\n");
    let before=g.clone();
    let declarations=g.local_variable_initializers("value").unwrap();assert_eq!(declarations.len(),2);
    let noop=g.prepare_local_variable_type("value",&declarations[1],ValueType::Bool).unwrap();
    assert!(noop.is_noop());assert!(!noop.requires_confirmation()&&!noop.requires_project_analysis());
    assert_eq!(noop.declarations(),declarations.as_slice());
    let int_list=g.prepare_local_variable_type("value",&declarations[0],ValueType::List(Box::new(ValueType::Int))).unwrap();
    let str_list=g.prepare_local_variable_type("value",&declarations[0],ValueType::List(Box::new(ValueType::String))).unwrap();
    assert_ne!(int_list.target_type(),str_list.target_type());assert!(!int_list.is_noop());
    let string=g.prepare_local_variable_type("value",&declarations[0],ValueType::String).unwrap();
    let text=g.prepare_local_variable_type("value",&declarations[0],ValueType::InterpolatedText).unwrap();
    assert_ne!(string.target_type(),text.target_type());
    for edit in 0..4 {
        let mut changed=g.clone();
        match edit {
            0=>changed.graph_id=GraphId::new(g.graph_id.get()+1000),
            1=>changed.nodes.get_mut(&declarations[0].node).unwrap().position[0]+=1.0,
            2=>changed.nodes.get_mut(&declarations[0].node).unwrap().title_override=Some("Changed title".into()),
            _=>changed.variables.get_mut("value").unwrap().default_value=PropertyValue::Bool(!matches!(g.variables["value"].default_value,PropertyValue::Bool(true))),
        }
        assert_ne!(changed,g);assert!(noop.validate_against(&changed).is_err());assert_eq!(g,before);
    }
}

#[test]
fn type_proposal_reports_shared_producers_and_unreachable_getters_as_unresolved() {
    let mut g=graph("init {set value=true}\nscene label start\nlocal value=value\nset result=value\nreturn\n");
    let declaration=initial(&g,"value");let input=g.pin_by_key(declaration.node,"value").unwrap().id;
    let shared=g.edges.values().find(|edge|edge.input==input).unwrap().output;
    let result=g.nodes.values().find(|node|node.kind==NodeKind::SetVariable&&node.properties.get("name")==Some(&PropertyValue::String("result".into()))).unwrap().id;
    let result_input=g.pin_by_key(result,"value").unwrap().id;
    g.edges.retain(|_,edge|edge.input!=result_input);g.connect(shared,result_input).unwrap();
    let unreachable=g.add_catalog_node(NodeKind::VariableGet,[500.0,500.0]).unwrap();
    g.set_property(unreachable,"name",PropertyValue::String("value".into())).unwrap();
    let unknown=g.pin_by_key(unreachable,"value").unwrap().id;
    let before=g.clone();let declaration=initial(&g,"value");
    let proposal=g.prepare_local_variable_type("value",&declaration,ValueType::Int).unwrap();
    assert!(proposal.shared_pins().contains(&shared));assert!(!proposal.local_pins().contains(&shared)&&!proposal.foreign_pins().contains(&shared));
    assert!(proposal.unresolved_pins().contains(&unknown));assert!(proposal.requires_project_analysis()&&proposal.requires_confirmation());
    assert_eq!(g,before);
}

#[test]
fn type_proposal_rejects_stale_declaration_and_structure_but_accepts_retained_type_errors() {
    let mut g=graph("scene label start\nlocal value=2\nif value>0 {\"ok\"}\nreturn\n");
    let captured=initial(&g,"value");g.replace_local_initializer("value",&captured,PropertyValue::Int(3)).unwrap();let before=g.clone();
    assert!(g.prepare_local_variable_type("value",&captured,ValueType::Float).is_err());assert_eq!(g,before);
    let captured=initial(&g,"value");assert!(g.prepare_local_variable_type("value",&captured,ValueType::Execution).is_err());
    let branch=g.nodes.values().find(|node|node.kind==NodeKind::If).unwrap().id;
    let condition=g.pin_by_key(branch,"condition").unwrap().id;
    let getter=g.add_catalog_node(NodeKind::VariableGet,[600.0,600.0]).unwrap();g.set_property(getter,"name",PropertyValue::String("value".into())).unwrap();
    let output=g.pin_by_key(getter,"value").unwrap().id;g.pins.get_mut(&output).unwrap().value_type=ValueType::Int;
    g.edges.retain(|_,edge|edge.input!=condition);
    let existing=EdgeId::new(900001);g.edges.insert(existing,GraphEdge{id:existing,output,input:condition});
    let captured=g.local_declaration_initializers_for_binding_analysis().unwrap().remove("value").unwrap().remove(0);let before=g.clone();
    let proposal=g.prepare_local_variable_type("value",&captured,ValueType::Float).unwrap();proposal.validate_against(&g).unwrap();
    assert!(transpile(&g).is_err());assert_eq!(g,before);assert_eq!(g.edges[&existing],before.edges[&existing]);
    g.pins.remove(&condition);let damaged=g.clone();assert!(g.prepare_local_variable_type("value",&captured,ValueType::Float).is_err());assert_eq!(g,damaged);
}

#[test]
fn type_reconstruction_resets_every_local_declaration_and_preserves_global_homonyms() {
    let g=graph("init {set value=true}\nscene label start\nset before=value\nlocal value=true\n\"First [value]\"\nset value=false\nlocal value=false\n\"Second [value]\"\nreturn\n");
    let before=g.clone();let captured=initial(&g,"value");
    let proposal=g.prepare_local_variable_type("value",&captured,ValueType::Int).unwrap();
    assert!(proposal.requires_confirmation());assert!(!proposal.requires_project_analysis());
    let next=proposal.reconstruct(&g).unwrap();assert_eq!(g,before);
    assert_eq!(next.variables["value"].value_type,ValueType::Int);
    assert_eq!(next.variables["value"].default_value,PropertyValue::Int(0));
    assert_eq!(next.edges,g.edges);
    assert_eq!(next.local_variable_initializers("value").unwrap().iter().map(|d|d.literal_value.clone()).collect::<Vec<_>>(),
        vec![Some(PropertyValue::Int(0)),Some(PropertyValue::Int(0))]);
    for id in proposal.foreign_pins() {assert_eq!(next.pins[id],g.pins[id]);assert_eq!(next.pin_constraint_type(*id),g.pin_constraint_type(*id));}
    for (id,node) in &g.nodes {assert_eq!(next.nodes[id].kind,node.kind);assert_eq!(next.nodes[id].position,node.position);
        assert_eq!(next.nodes[id].annotation,node.annotation);assert_eq!(next.nodes[id].title_override,node.title_override);}
    let cold=GraphDocument::from_json(&next.to_pretty_json().unwrap()).unwrap();assert_eq!(cold,next);
    let mut program=rvn_parser::parse_program("init {set value=true}\n").unwrap().script;
    program.extend(transpile(&cold).unwrap().ast);
    let mut runtime=Engine::new(program,TerminalRenderer,20).unwrap();
    assert!(matches!(runtime.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="First 0"));
    runtime.advance_dialogue().unwrap();
    assert!(matches!(runtime.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Second 0"));
}

#[test]
fn type_reconstruction_keeps_wired_assignments_and_compile_rejects_retained_conflicts() {
    let g=graph("scene label start\nlocal value=true\nset value=false\nif value {\"yes\"}\nreturn\n");
    let declaration=initial(&g,"value");let proposal=g.prepare_local_variable_type("value",&declaration,ValueType::Int).unwrap();
    let next=proposal.reconstruct(&g).unwrap();
    assert_eq!(next.edges,g.edges,"type changes must not heal existing wires");
    let set=g.nodes.values().find(|node|node.kind==NodeKind::SetVariable).unwrap();
    let input=g.pin_by_key(set.id,"value").unwrap().id;
    let incoming=g.edges.values().find(|edge|edge.input==input);
    // The importer may use an inline constant. Exercise the genuinely wired
    // form rather than mistaking a default conversion for retained wiring.
    if incoming.is_none() {
        let mut wired=g.clone();let literal=wired.add_catalog_node(NodeKind::Literal,[100.0,700.0]).unwrap();
        wired.set_property(literal,"value",PropertyValue::Bool(false)).unwrap();
        let output=wired.pin_by_key(literal,"value").unwrap().id;let edge=wired.connect(output,input).unwrap();
        let declaration=initial(&wired,"value");let proposal=wired.prepare_local_variable_type("value",&declaration,ValueType::Int).unwrap();
        let next=proposal.reconstruct(&wired).unwrap();assert_eq!(next.edges,wired.edges);
        assert_eq!(next.nodes[&literal],wired.nodes[&literal]);assert_eq!(next.pins[&output],wired.pins[&output]);
        assert!(next.validate().iter().any(|d|d.code=="incompatible_types"&&d.edge==Some(edge)));
        assert!(transpile(&next).is_err());
    } else {assert!(transpile(&next).is_err());}
    assert!(next.validate().iter().any(|d|d.code=="incompatible_types"));assert!(transpile(&next).is_err());
    assert_eq!(GraphDocument::from_json(&next.to_pretty_json().unwrap()).unwrap(),next);
}

#[test]
fn type_reconstruction_converts_explicit_set_default_but_resets_automatic_default() {
    for (old,expected,conversion) in [(true,"0/1",true),(false,"0/0",false)] {
        let mut g=graph(&format!("scene label start\nlocal value=true\n\"Before [value]\"\nset value={old}\n\"After [value]\"\nreturn\n"));
        let set=g.nodes.values().find(|node|node.kind==NodeKind::SetVariable).unwrap().id;
        let input=g.pin_by_key(set,"value").unwrap().id;
        g.edges.retain(|_,edge|edge.input!=input);g.pins.get_mut(&input).unwrap().default_value=Some(PropertyValue::Bool(old));
        let before=g.clone();let declaration=initial(&g,"value");let proposal=g.prepare_local_variable_type("value",&declaration,ValueType::Int).unwrap();
        let next=proposal.reconstruct(&g).unwrap();assert_eq!(g,before);
        assert_eq!(next.nodes.values().filter(|node|node.kind==NodeKind::ConvertBoolToInt).count(),usize::from(conversion));
        for(id,edge)in &g.edges{assert_eq!(next.edges[id],*edge);}
        assert_eq!(next.pins[&input].default_value,Some(PropertyValue::Int(0)));
        let cold=GraphDocument::from_json(&next.to_pretty_json().unwrap()).unwrap();assert_eq!(cold,next);
        let mut runtime=Engine::new(transpile(&cold).unwrap().ast,TerminalRenderer,20).unwrap();
        let Some(Interaction::Dialogue{text:first,..})=runtime.step_until_interaction().unwrap() else{panic!()};runtime.advance_dialogue().unwrap();
        let Some(Interaction::Dialogue{text:second,..})=runtime.step_until_interaction().unwrap() else{panic!()};
        assert_eq!(format!("{}/{}",first.strip_prefix("Before ").unwrap(),second.strip_prefix("After ").unwrap()),expected);
    }
}

#[test]
fn type_reconstruction_copies_shared_initial_literal_without_touching_foreign_consumers() {
    let mut g=graph("scene label start\nlocal first=-12\nlocal second=5\n\"[first]/[second]\"\nreturn\n");
    let first=initial(&g,"first");let second=initial(&g,"second");
    let input=g.pin_by_key(first.node,"value").unwrap().id;let foreign=g.pin_by_key(second.node,"value").unwrap().id;
    let output=g.edges.values().find(|edge|edge.input==input).unwrap().output;
    g.edges.retain(|_,edge|edge.input!=foreign);g.connect(output,foreign).unwrap();
    let declaration=initial(&g,"first");let proposal=g.prepare_local_variable_type("first",&declaration,ValueType::Bool).unwrap();
    let next=proposal.reconstruct(&g).unwrap();assert_eq!(initial(&next,"first").literal_value,Some(PropertyValue::Bool(false)));
    assert_eq!(initial(&next,"second").literal_value,Some(PropertyValue::Int(-12)));
    for(id,node)in &g.nodes{if node.kind!=NodeKind::LocalVariable{assert_eq!(next.nodes[id],*node);}}
    for(id,pin)in &g.pins{if pin.node!=first.node{assert_eq!(next.pins[id],*pin);}}
    for(id,edge)in &g.edges{assert_eq!(next.edges[id].id,edge.id);assert_eq!(next.edges[id].input,edge.input);
        if edge.input!=input{assert_eq!(next.edges[id],*edge);}else{assert_ne!(next.edges[id].output,edge.output);}}
    let mut runtime=Engine::new(transpile(&next).unwrap().ast,TerminalRenderer,20).unwrap();
    assert!(matches!(runtime.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="false/-12"));
}

#[test]
fn type_reconstruction_preserves_computed_initializer_and_full_typed_empty_arrays() {
    let g=graph("scene label start\nlocal input=2\nlocal value=input+1\n\"[value]\"\nreturn\n");
    let declaration=initial(&g,"value");assert!(declaration.literal_value.is_none());
    let proposal=g.prepare_local_variable_type("value",&declaration,ValueType::Bool).unwrap();
    let next=proposal.reconstruct(&g).unwrap();assert_eq!(next.edges,g.edges);assert!(transpile(&next).is_err());
    assert!(next.validate().iter().any(|d|d.code=="incompatible_types"));
    for target in [ValueType::String,ValueType::InterpolatedText,ValueType::List(Box::new(ValueType::Int)),
        ValueType::List(Box::new(ValueType::List(Box::new(ValueType::Bool))))] {
        let g=graph("scene label start\nlocal value=true\nreturn\n");let declaration=initial(&g,"value");
        let proposal=g.prepare_local_variable_type("value",&declaration,target.clone()).unwrap();
        let next=proposal.reconstruct(&g).unwrap();assert_eq!(next.variables["value"].value_type,target);
        assert_eq!(GraphDocument::from_json(&next.to_pretty_json().unwrap()).unwrap(),next);
        assert!(transpile(&next).is_ok());
        for id in proposal.local_pins(){assert_eq!(next.pin_constraint_type(*id),Some(target.clone()));}
    }
}

#[test]
fn type_reconstruction_refusals_and_noop_preserve_entire_authoring_model() {
    let g=graph("scene label start\nlocal value=true\nreturn\n");let declaration=initial(&g,"value");
    let noop=g.prepare_local_variable_type("value",&declaration,ValueType::Bool).unwrap();assert_eq!(noop.reconstruct(&g).unwrap(),g);
    let mut changed=g.clone();changed.nodes.get_mut(&declaration.node).unwrap().position[0]+=1.0;
    assert!(noop.reconstruct(&changed).is_err());
    for target in [ValueType::Execution,ValueType::List(Box::new(ValueType::Execution)),ValueType::Asset(AssetKind::Music)] {
        if let Ok(proposal)=g.prepare_local_variable_type("value",&declaration,target){assert!(proposal.reconstruct(&g).is_err());}
        assert_eq!(g,noop.reconstruct(&g).unwrap());
    }
    let mut g=graph("scene label start\nlocal value=true\nreturn\n");
    let unreachable=g.add_catalog_node(NodeKind::VariableGet,[600.0,600.0]).unwrap();g.set_property(unreachable,"name",PropertyValue::String("value".into())).unwrap();
    let declaration=initial(&g,"value");let before=g.clone();let proposal=g.prepare_local_variable_type("value",&declaration,ValueType::Int).unwrap();
    assert!(proposal.requires_project_analysis());assert!(proposal.reconstruct(&g).is_err());assert_eq!(g,before);
}
#[test]
fn editing_initial_value_keeps_later_assignment_and_actual_runtime_order(){
    let mut g=graph("scene label start\nlocal duration=2\n\"Before [duration]\"\nset duration=7\n\"After [duration]\"\nreturn\n");
    let expected=initial(&g,"duration");let before=g.clone();
    let sets:Vec<_>=g.nodes.values().filter(|n|n.kind==NodeKind::SetVariable).cloned().collect();
    assert!(g.replace_local_initializer("duration",&expected,PropertyValue::Int(3)).unwrap());
    assert_eq!(g.edges,before.edges);assert_eq!(g.variables,before.variables);
    for set in sets{assert_eq!(g.nodes[&set.id],set);for pin in set.pins{assert_eq!(g.pins[&pin],before.pins[&pin]);}}
    let cold=GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();assert_eq!(cold,g);
    let mut runtime=Engine::new(transpile(&cold).unwrap().ast,TerminalRenderer,20).unwrap();
    assert!(matches!(runtime.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Before 3"));
    runtime.advance_dialogue().unwrap();
    assert!(matches!(runtime.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="After 7"));
}
#[test]
fn shared_signed_literal_is_copied_without_changing_foreign_consumers_or_existing_edges(){
    let mut g=graph("scene label start\nlocal first=-12\nlocal second=5\n\"[first]/[second]\"\nreturn\n");
    let first=initial(&g,"first");let second=initial(&g,"second");
    let first_input=g.pin_by_key(first.node,"value").unwrap().id;let second_input=g.pin_by_key(second.node,"value").unwrap().id;
    let output=g.edges.values().find(|edge|edge.input==first_input).unwrap().output;
    g.edges.retain(|_,edge|edge.input!=second_input);
    g.connect(output,second_input).unwrap();
    let expected=initial(&g,"first");let before=g.clone();
    assert_eq!(initial(&g,"second").literal_value,Some(PropertyValue::Int(-12)),"fixture has a real shared expression");
    assert!(g.replace_local_initializer("first",&expected,PropertyValue::Int(3)).unwrap());
    assert_eq!(initial(&g,"first").literal_value,Some(PropertyValue::Int(3)));
    assert_eq!(initial(&g,"second").literal_value,Some(PropertyValue::Int(-12)));
    for(id,node)in &before.nodes{assert_eq!(&g.nodes[id],node);}
    for(id,pin)in &before.pins{assert_eq!(&g.pins[id],pin);}
    for(id,edge)in &before.edges{let after=&g.edges[id];assert_eq!(after.id,edge.id);assert_eq!(after.input,edge.input);
        if edge.input!=first_input{assert_eq!(after,edge);}else{assert_ne!(after.output,edge.output);}}
    let mut runtime=Engine::new(transpile(&g).unwrap().ast,TerminalRenderer,20).unwrap();
    assert!(matches!(runtime.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="3/-12"));
}
#[test]
fn primitive_unicode_bool_float_and_inline_defaults_roundtrip_without_cache_mutation(){
    for(expression,value)in[("false",PropertyValue::Bool(true)),("-1.25",PropertyValue::Float(2.5)),
        ("\"Before\"",PropertyValue::String("Été \"Iris\" \\ 🍃\nseconde ligne".into()))]{
        let source=format!("scene label start\nlocal value={expression}\nreturn\n");let mut g=graph(&source);let expected=initial(&g,"value");let before=g.clone();
        assert!(g.replace_local_initializer("value",&expected,value.clone()).unwrap());
        assert_eq!(initial(&g,"value").literal_value,Some(value));assert_eq!(g.edges,before.edges);assert_eq!(g.variables,before.variables);
        let cold=GraphDocument::from_json(&g.to_pretty_json().unwrap()).unwrap();assert_eq!(cold,g);
    }
    let mut g=graph("scene label start\nlocal value=2\nreturn\n");let node=initial(&g,"value").node;
    let input=g.pin_by_key(node,"value").unwrap().id;g.edges.retain(|_,edge|edge.input!=input);g.pins.get_mut(&input).unwrap().default_value=Some(PropertyValue::Int(2));
    let before=g.clone();let expected=initial(&g,"value");g.replace_local_initializer("value",&expected,PropertyValue::Int(4)).unwrap();
    let mut wanted=before;wanted.pins.get_mut(&input).unwrap().default_value=Some(PropertyValue::Int(4));assert_eq!(g,wanted);
}
#[test]
fn invalid_stale_computed_and_noop_initial_edits_are_atomic(){
    let mut g=graph("scene label start\nlocal value=2\nlocal decimal=1.25\nlocal computed=value+1\nreturn\n");let expected=initial(&g,"value");let before=g.clone();
    assert!(!g.replace_local_initializer("value",&expected,PropertyValue::Int(2)).unwrap());assert_eq!(g,before);
    let mut foreign=g.clone();foreign.graph_id=GraphId::new(g.graph_id.get()+1000);let other=foreign.clone();
    assert!(foreign.replace_local_initializer("value",&expected,PropertyValue::Int(3)).is_err());assert_eq!(foreign,other);
    assert!(g.replace_local_initializer("value",&expected,PropertyValue::String("3".into())).is_err());assert_eq!(g,before);
    let decimal=initial(&g,"decimal");
    for n in [f64::NAN,f64::INFINITY,1e40]{assert!(g.replace_local_initializer("decimal",&decimal,PropertyValue::Float(n)).is_err());assert_eq!(g,before);}
    let computed=initial(&g,"computed");assert!(g.replace_local_initializer("computed",&computed,PropertyValue::Int(9)).is_err());assert_eq!(g,before);
    assert!(g.replace_local_initializer("value",&expected,PropertyValue::Int(3)).unwrap());let changed=g.clone();
    assert!(g.replace_local_initializer("value",&expected,PropertyValue::Int(4)).is_err());assert_eq!(g,changed);
}
#[test]
fn repeated_declarations_use_explicit_identity_and_source_publication_preserves_unrelated_lines(){
    let source="// Owned source\nscene label start\nlocal value=2 // first\nlocal value=3 // second\nset value=7 // later\n\"[value]\"\nreturn\n";
    let mut project=SourceProject::open(source,&[]).unwrap();let mut gs=project.graphs().to_vec();
    let g=gs.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let declarations=g.local_variable_initializers("value").unwrap();assert_eq!(declarations.len(),2);
    g.replace_local_initializer("value",&declarations[1],PropertyValue::Int(4)).unwrap();
    assert_eq!(g.local_variable_initializers("value").unwrap()[0],declarations[0]);
    project.apply_visual(source,&gs).unwrap();let published=project.source();
    assert!(published.contains("local value=2 // first"));assert!(published.contains("set value=7 // later"));assert!(published.contains("// second"));
    let reopened=graph(published);assert_eq!(reopened.local_variable_initializers("value").unwrap()[1].literal_value,Some(PropertyValue::Int(4)));
}

#[test]
fn type_reconstruction_detached_calculation_slot_keeps_computed_rhs_and_reconstructs_dead_get(){
    let mut g=GraphDocument::new(GraphId::new(990081),GraphKind::Function{name:"detached".into()});
    let entry=g.add_catalog_node(NodeKind::FunctionEntry,[0.0,0.0]).unwrap();
    let ret=g.add_catalog_node(NodeKind::FunctionReturn,[600.0,0.0]).unwrap();g.set_pin_default(ret,"value",PropertyValue::Int(0)).unwrap();
    g.connect(g.pin_by_key(entry,"exec_out").unwrap().id,g.pin_by_key(ret,"exec_in").unwrap().id).unwrap();
    let local=g.add_catalog_node(NodeKind::LocalVariable,[200.0,200.0]).unwrap();g.set_property(local,"name",PropertyValue::String("value".into())).unwrap();
    let append=g.add_catalog_node(NodeKind::StringAppend,[0.0,200.0]).unwrap();
    g.connect(g.pin_by_key(append,"value").unwrap().id,g.pin_by_key(local,"value").unwrap().id).unwrap();
    let get=g.add_catalog_node(NodeKind::VariableGet,[400.0,200.0]).unwrap();g.set_property(get,"name",PropertyValue::String("value".into())).unwrap();
    g.variables.insert("value".into(),VariableDefinition{name:"value".into(),value_type:ValueType::String,default_value:PropertyValue::String(String::new())});
    let before=g.clone();assert!(g.local_variable_initializers("value").unwrap().is_empty());
    let proposal=g.prepare_local_variable_type_slot("value",ValueType::InterpolatedText).unwrap();assert_eq!(g,before);
    assert!(proposal.requires_confirmation());assert_eq!(proposal.detached_declarations().iter().copied().collect::<Vec<_>>(),vec![local]);assert!(!proposal.requires_project_analysis());
    let next=proposal.reconstruct(&g).unwrap();assert_eq!(next.edges,before.edges);assert_eq!(next.nodes[&append],before.nodes[&append]);
    for(node,key)in[(local,"value"),(local,"value_out"),(get,"value")]{assert_eq!(next.pin_by_key(node,key).unwrap().value_type,ValueType::InterpolatedText);}
    assert_eq!(GraphDocument::from_json(&next.to_pretty_json().unwrap()).unwrap(),next);
}

#[test]
fn type_reconstruction_detached_primitive_resets_default_without_emitting_or_attaching_execution(){
    let mut g=GraphDocument::new(GraphId::new(990082),GraphKind::Function{name:"detached".into()});
    let entry=g.add_catalog_node(NodeKind::FunctionEntry,[0.0,0.0]).unwrap();let ret=g.add_catalog_node(NodeKind::FunctionReturn,[600.0,0.0]).unwrap();
    g.set_pin_default(ret,"value",PropertyValue::Int(0)).unwrap();g.connect(g.pin_by_key(entry,"exec_out").unwrap().id,g.pin_by_key(ret,"exec_in").unwrap().id).unwrap();
    let local=g.add_catalog_node(NodeKind::LocalVariable,[200.0,200.0]).unwrap();g.set_property(local,"name",PropertyValue::String("value".into())).unwrap();
    g.set_pin_default(local,"value",PropertyValue::Bool(true)).unwrap();g.variables.insert("value".into(),VariableDefinition{name:"value".into(),value_type:ValueType::Bool,default_value:PropertyValue::Bool(true)});
    let before=g.clone();let proposal=g.prepare_local_variable_type_slot("value",ValueType::Int).unwrap();assert!(!proposal.requires_confirmation());
    let next=proposal.reconstruct(&g).unwrap();assert_eq!(next.edges,before.edges);assert_eq!(next.pin_by_key(local,"value").unwrap().default_value,Some(PropertyValue::Int(0)));
    assert!(next.local_variable_initializers("value").unwrap().is_empty());assert!(!transpile(&next).unwrap().source.contains("local value"));assert_eq!(g,before);
}

#[test]
fn owned_unit_call1021_detached_unit_local_type_retains_real_entry_and_execution() {
    let parsed=rvn_parser::parse_program("function detached() -> () {return}\nscene label start\ncall detached()\nreturn\n").unwrap();
    let mut project=prepare_initial_unit_link_graphs(&parsed.script,&[],||panic!("zero-parameter Unit allocates no opaque parameter")).unwrap();
    let index=project.iter().position(|graph|matches!(&graph.kind,GraphKind::UnitFunction{name}if name=="detached")).unwrap();
    let g=&mut project[index];
    let local=g.add_catalog_node(NodeKind::LocalVariable,[200.0,200.0]).unwrap();g.set_property(local,"name",PropertyValue::String("value".into())).unwrap();
    g.set_pin_default(local,"value",PropertyValue::Bool(true)).unwrap();
    g.variables.insert("value".into(),VariableDefinition{name:"value".into(),value_type:ValueType::Bool,default_value:PropertyValue::Bool(true)});
    let before=project.clone();let g=&project[index];
    assert!(g.local_declaration_initializers_for_binding_analysis_in_project(&project).unwrap().get("value").is_none());
    let proposal=g.prepare_local_variable_type_slot_in_project("value",ValueType::Int,&project).unwrap();
    assert!(!proposal.requires_confirmation());assert_eq!(proposal.detached_declarations().iter().copied().collect::<Vec<_>>(),vec![local]);
    let next=proposal.reconstruct_in_project(g,&project).unwrap();
    assert_eq!(next.edges,g.edges);assert_eq!(next.pin_by_key(local,"value").unwrap().default_value,Some(PropertyValue::Int(0)));
    let mut candidate=project.clone();candidate[index]=next.clone();
    assert!(next.local_declaration_initializers_for_binding_analysis_in_project(&candidate).unwrap().get("value").is_none());
    let compiled=transpile_project(&candidate).unwrap();assert!(!compiled.source.contains("local value"));
    assert_eq!(project,before);assert_eq!(GraphDocument::from_json(&next.to_pretty_json().unwrap()).unwrap(),next);
}
