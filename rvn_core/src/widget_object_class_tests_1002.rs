//! Existing real SourceProject/admission/Host/Save15 only; no physical claim.
use super::*;
use crate::random::RandomState;
fn expr(source:&str)->rvn_parser::Expr{
    let script=rvn_parser::parse(&format!("set result={source}")).unwrap();
    let [Statement::SetVar{value,..}]=script.as_slice()else{panic!("expression fixture")};value.clone()
}
fn object(engine:&mut Engine<Host>,name:&str)->Value{
    let handle=engine.create_widget(A,name,vec![Value::Str("allowed".into())]).unwrap();
    engine.widget_reference_value(&handle).unwrap()
}
fn class(fixture:&Fixture)->Value{Value::WidgetClass(Box::new(crate::widget_class_getter_0981::resolve_class_data(&fixture.classes,Some(A)).unwrap()))}
fn object_null()->Value{Value::Widget(Box::new(rvn_parser::widget_reference::WidgetReference::null()))}
fn query()->rvn_parser::Expr{expr("__rvn_widget_object_class_v1(saved_object)")}

// Author and publish real Graph identities before the Fixture opens its final
// SourceProject and prepares the Widget class closure. No identity strings are
// manufactured here and no Engine/frozen AST is replaced after admission.
fn connect_fixture(graph:&mut rvn_graph::GraphDocument,a:rvn_graph::NodeId,ak:&str,b:rvn_graph::NodeId,bk:&str){
    let from=graph.pin_by_key(a,ak).unwrap().id;let to=graph.pin_by_key(b,bk).unwrap().id;graph.connect(from,to).unwrap();
}
fn native_start_source(root:&PathBuf,source:String,nullable_object_binding:bool,inspector_getter:bool)->String{
    let path=root.join("main.rvn");fs::write(&path,&source).unwrap();
    let mut project=rvn_graph::SourceProject::open_at(&path,&source,&[]).unwrap();
    let mut edited=project.graphs().to_vec();
    let graph=edited.iter_mut().find(|g|matches!(&g.kind,rvn_graph::GraphKind::Label{name}if name=="start")).unwrap();
    graph.enable_native_frame_preparation().unwrap();
    if nullable_object_binding{
        let assignments=graph.nodes.values().filter(|node|node.kind==rvn_graph::NodeKind::SetVariable&&node.properties.get("name")==Some(&rvn_graph::PropertyValue::String("saved_object".into()))).map(|node|node.id).collect::<Vec<_>>();
        let [assignment]=assignments.as_slice()else{panic!("one real source assignment")};
        graph.replace_assignment_with_widget_factory(*assignment,&rvn_parser::Expr::Call{name:"__rvn_widget_class_none_v1".into(),args:vec![]},&rvn_parser::Expr::ListLit(vec![])).unwrap();
        assert_eq!(graph.variables["saved_object"].value_type,rvn_graph::ValueType::WidgetObject);
    }
    if inspector_getter{
        let previous=graph.nodes.values().find(|n|n.kind==rvn_graph::NodeKind::GetStringTableIdAndKey).unwrap().id;
        let input=graph.pin_by_key(previous,"text").unwrap().id;
        let edge=graph.edges.values().find(|e|e.input==input).unwrap().clone();let text_output=edge.output;
        let outputs=graph.edges.values().filter_map(|e|{let pin=&graph.pins[&e.output];(pin.node==previous).then(||(pin.key.clone(),e.input))}).collect::<Vec<_>>();
        graph.remove_node(previous);
        // Opt-in preserves old Legacy inspectors. A new real catalogue node
        // gets NativeFrameV1 through the normal public construction path.
        let inspector=graph.add_catalog_node(rvn_graph::NodeKind::GetStringTableIdAndKey,[800.,300.]).unwrap();
        for(key,destination)in outputs{graph.connect(graph.pin_by_key(inspector,&key).unwrap().id,destination).unwrap();}
        let read=graph.add_catalog_node(rvn_graph::NodeKind::VariableGet,[0.,300.]).unwrap();
        graph.set_property(read,"name",rvn_graph::PropertyValue::String("saved_object".into())).unwrap();
        graph.refresh_variable_usage_bindings().unwrap();
        assert_eq!(graph.effective_pin_type(graph.pin_by_key(read,"value").unwrap().id),Some(rvn_graph::ValueType::WidgetObject));
        let getter=graph.add_catalog_node(rvn_graph::NodeKind::WidgetObjectClass,[200.,300.]).unwrap();
        connect_fixture(graph,read,"value",getter,"object");
        let list=graph.add_catalog_node(rvn_graph::NodeKind::ListLiteral,[400.,300.]).unwrap();graph.resize_value_inputs(list,2).unwrap();
        connect_fixture(graph,getter,"value",list,"item_0");graph.connect(text_output,graph.pin_by_key(list,"item_1").unwrap().id).unwrap();
        let index=graph.add_catalog_node(rvn_graph::NodeKind::Index,[600.,300.]).unwrap();graph.set_pin_default(index,"index",rvn_graph::PropertyValue::Int(1)).unwrap();
        connect_fixture(graph,list,"list",index,"target");connect_fixture(graph,index,"value",inspector,"text");
    }
    // The binding stamp includes topology. Refresh after the last connection,
    // as the real Graph inspector fixtures do, rather than keeping a stamp
    // taken before the getter/list/index were added.
    graph.refresh_variable_usage_bindings().unwrap();
    project.apply_visual(&source,&edited).unwrap();
    project.source().to_owned()
}
fn source_with_functions(mut source:String,functions:&str)->String{
    if functions.is_empty(){return source;}
    let parsed=rvn_parser::parse_with_origins(&source,rvn_parser::NarrativeContext::ExplicitScenes).unwrap();
    let start=parsed.script.iter().zip(&parsed.spans).find(|(statement,_)|matches!(statement,Statement::SceneLabel{name}|Statement::Label{name}if name=="start")).unwrap().1.range.start;
    // A new declaration is outside every existing Native graph. Its first
    // import now sees saved_object from the already published Scene factory.
    source.insert_str(start,&functions.replace("__rvn_widget_object_class_v1(value)","__rvn_widget_object_class_v1(saved_object)"));source
}
fn edit_native_start(root:&PathBuf,source:String,edit:impl FnOnce(&mut rvn_graph::GraphDocument))->String{
    let path=root.join("main.rvn");fs::write(&path,&source).unwrap();
    let mut project=rvn_graph::SourceProject::open_at(&path,&source,&[]).unwrap();
    let mut edited=project.graphs().to_vec();
    let graph=edited.iter_mut().find(|g|matches!(&g.kind,rvn_graph::GraphKind::Label{name}if name=="start")).unwrap();
    assert_eq!(graph.variables["saved_object"].value_type,rvn_graph::ValueType::WidgetObject);
    edit(graph);graph.refresh_variable_usage_bindings().unwrap();
    project.apply_visual(&source,&edited).unwrap();project.source().to_owned()
}
fn one_ready(graph:&rvn_graph::GraphDocument)->rvn_graph::NodeId{
    let nodes=graph.nodes.values().filter(|node|node.kind==rvn_graph::NodeKind::Dialogue).map(|node|node.id).collect::<Vec<_>>();
    let [ready]=nodes.as_slice()else{panic!("one actual Ready dialogue before editing")};*ready
}
fn raw_scene_function_tint(root:&PathBuf,source:String,body:&str,functions:&str)->String{
    let seed=source.replacen("label start\n","scene label start\nset saved_object=0\n",1);
    let source=native_start_source(root,seed,true,false);
    let source=source_with_functions(source,functions)+&format!("\nlabel getter_tint\nhero.show(\"hero.png\") at center\n\"Ready\"\n{body}\n\"Applied\"\nreturn\n");
    edit_native_start(root,source,|graph|{
        let ready=one_ready(graph);
        let input=graph.pin_by_key(ready,"exec_in").unwrap().id;
        let prior=graph.edges.values().find(|edge|edge.input==input).unwrap().output;
        let text_input=graph.pin_by_key(ready,"text").unwrap().id;
        let text=graph.edges.values().find(|edge|edge.input==text_input).map(|edge|graph.pins[&edge.output].node).unwrap();
        let returns=graph.nodes.values().filter(|node|node.kind==rvn_graph::NodeKind::Return).map(|node|node.id).collect::<Vec<_>>();
        let [end]=returns.as_slice()else{panic!("one actual Return")};let end=*end;
        graph.remove_node(ready);graph.remove_node(text);graph.remove_node(end);
        let jump=graph.add_catalog_node(rvn_graph::NodeKind::Jump,[800.,0.]).unwrap();
        graph.set_pin_default(jump,"target",rvn_graph::PropertyValue::String("getter_tint".into())).unwrap();
        graph.connect(prior,graph.pin_by_key(jump,"exec_in").unwrap().id).unwrap();
    })
}
fn query_fixture()->Fixture{
    Fixture::with_source_transform_at(false,false,false,false,|root,source|{
        let seed=source.replacen("label start\n","scene label start\nset saved_object=0\n",1);
        // The actual Set/Dialogue already exist before Native preparation.
        // Their initializer is edited through Graph only after its typed
        // factory and function context are both published.
        let seed=seed.replacen("\"Ready\"\nreturn","\"Ready\"\nset queried=__rvn_widget_class_none_v1()\n\"Resolved\"\nreturn",1);
        let source=native_start_source(root,seed,true,false);
        let source=source_with_functions(source,"\nfunction object_class(value){local roll=random(1,9) local resolved=__rvn_widget_object_class_v1(saved_object) return resolved}\n");
        // Every new producer/wire uses public Graph commands. No raw Set or
        // Dialogue is appended to this already-Native label, and no queried
        // Init default is introduced before the first Ready boundary.
        edit_native_start(root,source,|graph|{
            let factory=graph.nodes.values().find(|node|node.kind==rvn_graph::NodeKind::WidgetCreate).unwrap().id;
            let call=graph.add_catalog_node(rvn_graph::NodeKind::FunctionCall,[400.,200.]).unwrap();
            graph.set_property(call,"function",rvn_graph::PropertyValue::String("object_class".into())).unwrap();graph.resize_value_inputs(call,1).unwrap();connect_fixture(graph,factory,"value_out",call,"item_0");
            let sets=graph.nodes.values().filter(|node|node.kind==rvn_graph::NodeKind::SetVariable&&node.properties.get("name")==Some(&rvn_graph::PropertyValue::String("queried".into()))).map(|node|node.id).collect::<Vec<_>>();
            let [set]=sets.as_slice()else{panic!("one source-authored query assignment")};
            assert_eq!(graph.variables["queried"].value_type,rvn_graph::ValueType::WidgetClass);
            // Class None is a real typed producer, not a scalar pin default.
            // Retire precisely its one actual edge and mono-use source before
            // connecting the function, leaving no orphan identity to export.
            let input=graph.pin_by_key(*set,"value").unwrap().id;
            let edges=graph.edges.values().filter(|edge|edge.input==input).cloned().collect::<Vec<_>>();
            let [old]=edges.as_slice()else{panic!("one actual Class None source")};
            let prior=graph.pins[&old.output].node;
            assert_eq!(graph.nodes[&prior].kind,rvn_graph::NodeKind::WidgetClassValue);
            assert_eq!(graph.nodes[&prior].properties.get("uuid"),Some(&rvn_graph::PropertyValue::String(String::new())));
            assert_eq!(graph.edges.values().filter(|edge|graph.pins[&edge.output].node==prior).count(),1);
            graph.remove_edge(old.id);graph.remove_node(prior);
            connect_fixture(graph,call,"result",*set,"value");
        })
    })
}

#[test]
fn object_class1002_typed_none_and_wrong_types_never_coerce_or_publish(){
    let fixture=Fixture::new(false);let engine=fixture.engine();
    let before=serde_json::to_vec(&engine.state).unwrap();let batches=engine.renderer.batches.lock().unwrap().len();
    assert_eq!(engine.widget_object_class(&object_null()).unwrap(),Value::WidgetClass(Box::new(None)));
    for value in [Value::Str(A.into()),class(&fixture),Value::WidgetClass(Box::new(None)),Value::Int(0)]{assert!(engine.widget_object_class(&value).is_err());}
    assert_eq!(before,serde_json::to_vec(&engine.state).unwrap());assert_eq!(batches,engine.renderer.batches.lock().unwrap().len());
    assert_eq!(crate::eval::FunctionLibrary::default().eval(&expr("__rvn_widget_object_class_v1(x)"),&HashMap::from([("x".into(),object_null())])).unwrap(),Value::WidgetClass(Box::new(None)));
}

#[test]
fn object_class1002_actual_instances_keep_class_after_detach_and_retirement(){
    let fixture=Fixture::new(false);let mut engine=fixture.engine();let a=object(&mut engine,"a");let b=object(&mut engine,"b");assert_ne!(a,b);
    let expected=class(&fixture);assert_eq!(engine.widget_object_class(&a).unwrap(),expected);assert_eq!(engine.widget_object_class(&b).unwrap(),expected);
    let handle=engine.widget_reference_handle(&a).unwrap();engine.add_widget_to_viewport(&handle,false,0).unwrap();engine.remove_widget_from_viewport(&handle).unwrap();
    assert_eq!(engine.widget_object_class(&a).unwrap(),expected,"detached logical instance remains resident");
    engine.destroy_widget(handle).unwrap();assert!(engine.widget_object_class(&a).is_err());
    crate::widget_class_getter_0981::validate_class_values(engine.widget_classes(),&expected).unwrap();assert_eq!(engine.widget_object_class(&b).unwrap(),expected,"class data and other instances do not depend on a retired object");
}

#[test]
fn object_class1002_foreign_portable_forged_and_reused_alias_are_not_objects(){
    let fixture=Fixture::new(false);let mut engine=fixture.engine();let value=object(&mut engine,"same");
    let inert:Value=serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();assert!(engine.widget_object_class(&inert).is_err());
    let Value::Widget(reference)=&value else{panic!("object")};let portable=reference.portable().unwrap();let mut receipt=*portable.class().receipt();receipt[0]^=1;
    let forged_class=rvn_parser::widget_reference::WidgetClassReference::new(A.into(),receipt).unwrap();let forged_link=rvn_parser::widget_reference::PortableWidgetReference::new(forged_class,portable.alias().into(),portable.incarnation()).unwrap();let forged=Value::Widget(Box::new(rvn_parser::widget_reference::WidgetReference::bind_admitted(forged_link,engine.instance_id(),engine.runtime_epoch()).unwrap()));assert!(engine.widget_object_class(&forged).is_err(),"a bound envelope is not current class/source admission");
    let mut foreign=fixture.engine();let _=object(&mut foreign,"same");assert!(foreign.widget_object_class(&value).is_err());
    let handle=engine.widget_reference_handle(&value).unwrap();engine.destroy_widget(handle).unwrap();let new=object(&mut engine,"same");assert_ne!(value,new);assert!(engine.widget_object_class(&value).is_err());assert_eq!(engine.widget_object_class(&new).unwrap(),class(&fixture));
}

#[test]
fn object_class1002_pure_preview_uses_current_owner_and_rolls_back_failed_random_work(){
    let fixture=Fixture::new(false);let mut engine=fixture.engine();let value=object(&mut engine,"preview");engine.state.vars.insert("saved_object".into(),value.clone());
    let before=serde_json::to_vec(&engine.state).unwrap();let batches=engine.renderer.batches.lock().unwrap().len();
    assert_eq!(engine.eval_narrative_expression(&query()).unwrap(),class(&fixture));
    assert!(engine.functions.eval(&query(),&engine.state.vars).is_err(),"an unscoped public library cannot borrow Engine authority");
    let handle=engine.widget_reference_handle(&value).unwrap();engine.destroy_widget(handle).unwrap();
    let mut random=RandomState::seeded(1002);let original=random;
    let admission=crate::widget_reference_values_0974::WidgetValueAdmission::new(&engine.state,engine.widget_classes.clone(),engine.instance_id(),engine.runtime_epoch()).unwrap();
    assert!(engine.functions.eval_scoped_with_locale_with_widget_objects(Some(&admission),&expr("[random(1,9),__rvn_widget_object_class_v1(saved_object)]"),&engine.state.vars,&engine.state.vars,&mut random,None).is_err());assert_eq!(random,original);
    assert_ne!(before,serde_json::to_vec(&engine.state).unwrap(),"only the explicit real destroy changed state");assert_eq!(batches+1,engine.renderer.batches.lock().unwrap().len());
}

#[test]
fn object_class1002_reached_rvn_function_uses_borrowed_current_object_context(){
    let fixture=query_fixture();let mut engine=fixture.engine();assert!(engine.functions.uses_widget_object_class);
    let value=object(&mut engine,"source_value");engine.state.vars.insert("saved_object".into(),value);engine.advance_dialogue().unwrap();engine.step_until_interaction().unwrap();
    assert_eq!(engine.state.vars["queried"],class(&fixture));assert!(!engine.state.vars.contains_key("resolved"),"function locals do not leak into globals");
}

#[test]
fn object_class1002_cold_save15_rehydrates_object_epoch_but_not_a_class_lifetime(){
    let fixture=Fixture::new(false);let mut engine=fixture.engine();let value=object(&mut engine,"saved");let expected=engine.widget_object_class(&value).unwrap();engine.state.vars.insert("saved_object".into(),value.clone());engine.state.vars.insert("saved_class".into(),expected.clone());engine.state.vars.insert("hits".into(),Value::Int(71));
    let save=engine.capture_save15(&fixture.program,None,1,"Get Class".into()).unwrap();let save=crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap();
    let startup=Engine::prepare_widget_class_startup(fixture.program.clone(),fixture.classes.clone(),Host::default(),16,Default::default(),Default::default()).unwrap();let(mut cold,session)=startup.restore_completed_save15(save).unwrap();assert!(session.is_none());
    assert_ne!(cold.instance_id(),engine.instance_id());assert!(cold.widget_object_class(&value).is_err());let restored=cold.state.vars["saved_object"].clone();assert_ne!(restored,value);assert_eq!(cold.widget_object_class(&restored).unwrap(),expected);assert_eq!(cold.state.vars["hits"],Value::Int(71));
    let handle=cold.widget_reference_handle(&restored).unwrap();cold.destroy_widget(handle).unwrap();assert!(cold.widget_object_class(&restored).is_err());assert_eq!(cold.state.vars["saved_class"],expected);crate::widget_class_getter_0981::validate_class_values(cold.widget_classes(),&cold.state.vars["saved_class"]).unwrap();
}

#[test]
fn object_class1002_actual_stopped_rvn_save15_resumes_the_getter_once_at_current_epoch(){
    let fixture=query_fixture();let mut engine=fixture.engine();let value=object(&mut engine,"pending");engine.state.vars.insert("saved_object".into(),value.clone());engine.advance_dialogue().unwrap();
    let mut session=engine.debug_begin(DebugOperation::Statement,vec![]).unwrap();let mut stopped=false;
    for _ in 0..128{let(progress,_)=engine.debug_step(&mut session,crate::editor_debugger::StepKind::Into).unwrap();assert_ne!(progress,DebugProgress::Complete);if session.snapshot().frames.iter().any(|frame|frame.bindings.iter().any(|(name,_)|name=="roll")){stopped=true;break;}}
    assert!(stopped);assert!(!engine.state.vars.contains_key("queried"));
    let save=engine.capture_save15(&fixture.program,Some(&session),1,"pending Get Class".into()).unwrap();let save=crate::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap();
    let startup=Engine::prepare_widget_class_startup(fixture.program.clone(),fixture.classes.clone(),Host::default(),16,Default::default(),Default::default()).unwrap();let(mut cold,session)=startup.restore_completed_save15(save).unwrap();let mut session=session.unwrap();assert!(cold.widget_object_class(&value).is_err());
    let mut complete=false;for _ in 0..512{let(progress,_)=cold.debug_step(&mut session,crate::editor_debugger::StepKind::Continue).unwrap();if progress==DebugProgress::Complete{complete=true;break;}}assert!(complete);assert_eq!(cold.state.vars["queried"],class(&fixture));let random=cold.state.random;assert!(cold.debug_step(&mut session,crate::editor_debugger::StepKind::Continue).is_err());assert_eq!(cold.state.random,random);
}

#[test]
fn object_class1002_existing_debugger_anchor_rejects_retirement_before_resume_without_revoking_class_data(){
    let fixture=query_fixture();let mut engine=fixture.engine();let value=object(&mut engine,"anchored");let expected=engine.widget_object_class(&value).unwrap();engine.state.vars.insert("saved_object".into(),value.clone());engine.advance_dialogue().unwrap();let mut session=engine.debug_begin(DebugOperation::Statement,vec![]).unwrap();
    let handle=engine.widget_reference_handle(&value).unwrap();engine.destroy_widget(handle).unwrap();let before=serde_json::to_vec(&engine.state).unwrap();let publications=engine.renderer.batches.lock().unwrap().len();assert!(engine.debug_step(&mut session,crate::editor_debugger::StepKind::Continue).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);assert_eq!(engine.renderer.batches.lock().unwrap().len(),publications);crate::widget_class_getter_0981::validate_class_values(engine.widget_classes(),&expected).unwrap();
}


// All variants are authored before the real SourceProject, descriptor and
// class admission. No consumer identity or Engine state is substituted later.
fn executable_slot_fixture(scene:bool,owned_game:bool,body:&str,functions:&str)->Fixture{
    Fixture::with_source_transform_at(false,false,false,false,|root,source|{
        let inspector=body.starts_with("flow.consume_v1(\"getter_slot\"");
        let native_effect=body.starts_with("flow.consume_v2(\"scene_tint\",\"effect\"){");
        if scene&&!functions.is_empty()&&!native_effect{return raw_scene_function_tint(root,source,body,functions);}
        let source=if scene{source.replace("label start","scene label start")}else{source};
        let needs_typed_object=inspector||(!functions.is_empty()&&scene);
        let source=if needs_typed_object{source.replacen("scene label start\n","scene label start\nset saved_object=0\n",1)}else{source};
        // This source has no TextOwner. Use the actual public conversion node,
        // rather than a raw owner-dependent intrinsic imported as FunctionCall.
        let body=if inspector{"set inspected=__rvn_text_table_valid(string_to_text(\"probe\"))"}else if native_effect{body.strip_prefix("flow.consume_v2(\"scene_tint\",\"effect\"){").unwrap().strip_suffix('}').unwrap()}else{body};
        let setup=if owned_game{"timer 50 => jump done\n"}else{""};
        let source=source.replace("\n\"Ready\"\nreturn\n",&format!("\nhero.show(\"hero.png\") at center\n{setup}\"Ready\"\n{body}\n\"Applied\"\nreturn\n"));
        let source=if body.contains("getter_tint_color") {
            // SpriteEffect historically retains escaped source-string bytes.
            // Use a real declared color rather than nesting quoted strings in
            // that raw field; this isolates object-class interpolation without
            // claiming the separate escaped-tint parser gap is corrected.
            source.replacen("init{set hits=0}","init{set hits=0 set getter_tint_color=\"#ffffffff\"}",1)
        }else{source};
        let source=if owned_game{source+"\nlabel done\n\"Done\"\nreturn\n"}else{source};
        let source=if owned_game||native_effect||needs_typed_object{native_start_source(root,source,needs_typed_object,inspector)}else{source};
        let source=source_with_functions(source,functions);
        // Global source typing comes from the actual nullable factory emitted
        // above. Raw tint Game/historical variants exercise dynamic Core values;
        // they do not claim static WidgetObject authoring of their variable.
        source
    })
}
fn set_slot_object(engine:&mut Engine<Host>)->Value{
    let value=object(engine,"slot_object");engine.state.vars.insert("saved_object".into(),value.clone());engine.advance_dialogue().unwrap();value
}

#[test]
fn object_class1002_r1_inspector_value_is_the_only_getter_and_runs_with_actual_consumer_owner(){
    let body=r#"flow.consume_v1("getter_slot","inspect"){
inspect "text"=[__rvn_widget_object_class_v1(saved_object),__rvn_invariant_text("probe")][1]
set inspected=__rvn_terminal_v1("text","valid")
}"#;
    let fixture=executable_slot_fixture(true,false,body,"");let raw=fs::read_to_string(fixture.root.join("main.rvn")).unwrap();assert_eq!(raw.matches("__rvn_widget_object_class_v1").count(),1);
    let mut engine=fixture.engine();assert!(engine.narrative.is_some());assert!(engine.functions.uses_widget_object_class);let value=set_slot_object(&mut engine);assert_eq!(engine.widget_object_class(&value).unwrap(),class(&fixture));
    let Some(Statement::ScheduledConsumer{steps,..})=engine.script.get(engine.state.pc)else{panic!("actual source-owned inspector consumer")};assert_eq!(steps.iter().filter(|step|matches!(step,rvn_parser::ConsumerStep::InspectTextTable{..})).count(),1);assert!(steps.iter().any(|step|matches!(step,rvn_parser::ConsumerStep::Capture{..})),"real producer identities are preserved");
    assert_eq!(steps.iter().filter(|step|matches!(step,rvn_parser::ConsumerStep::Capture{value:rvn_parser::Expr::Call{name,..},..}if name=="__rvn_widget_object_class_v1")).count(),1,"the actual consumer gathers exactly one getter, not an inline source substitution");
    engine.step_silent().unwrap();assert_eq!(engine.state.vars["inspected"],Value::Bool(false),"valid lookup is followed by the existing non-table Text inspector result");
    let mut rejected=fixture.engine();let value=set_slot_object(&mut rejected);let handle=rejected.widget_reference_handle(&value).unwrap();rejected.destroy_widget(handle).unwrap();let before=serde_json::to_vec(&rejected.state).unwrap();let publications=rejected.renderer.batches.lock().unwrap().len();assert!(rejected.step_silent().is_err());assert_eq!(serde_json::to_vec(&rejected.state).unwrap(),before);assert_eq!(rejected.renderer.batches.lock().unwrap().len(),publications);assert!(!rejected.state.vars.contains_key("inspected"));
}

#[test]
fn object_class1002_r1_raw_tint_is_the_only_getter_in_actual_scene_game_and_historical_routes(){
    let tint="[( [__rvn_widget_object_class_v1(saved_object), getter_tint_color] )[1]]";
    let body=format!("hero.effect(tint:{})",serde_json::to_string(tint).unwrap());
    for(scene,owned_game)in [(true,false),(false,true),(false,false)]{
        let fixture=executable_slot_fixture(scene,owned_game,&body,"");
        // Native static provenance also captures the raw tint as Expr::Str.
        // Its spelling is data, not a second executed getter. Inspect the one
        // reached tint expression instead of counting bytes in source text.
        let mut blocks=vec![fixture.program.script.as_slice()];let mut tints=Vec::new();let mut direct_getters=0;
        while let Some(block)=blocks.pop(){for statement in block{
            if let Statement::SpriteEffect{tint:Some(tint),..}=statement{tints.push(tint);}
            if let Statement::ScheduledConsumer{steps,..}=statement{direct_getters+=steps.iter().filter(|step|matches!(step,
                rvn_parser::ConsumerStep::Capture{value:rvn_parser::Expr::Call{name,..},..}
                    |rvn_parser::ConsumerStep::InspectTextTable{value:rvn_parser::Expr::Call{name,..},..}
                    if name=="__rvn_widget_object_class_v1")).count();}
            blocks.extend(crate::consumer_schedule::statement_children(statement));
        }}
        assert_eq!(direct_getters,0,"a raw tint provenance capture does not execute its string content");
        let [tint]=tints.as_slice()else{panic!("one real reached SpriteEffect tint")};
        let interpolated=rvn_parser::parse_interpolated_str(tint).unwrap();
        let [rvn_parser::TextSegment::Interp(rvn_parser::Expr::Index{target,index})]=interpolated.0.as_slice()else{panic!("one real tint interpolation")};
        let rvn_parser::Expr::ListLit(items)=target.as_ref()else{panic!("one actual list")};
        assert_eq!(index.as_ref(),&rvn_parser::Expr::Int(1));
        assert_eq!(items.as_slice(),&[query(),rvn_parser::Expr::Var("getter_tint_color".into())],"exactly one eager object-class lookup precedes the selected color");
        let mut engine=fixture.engine();assert_eq!(engine.narrative.is_some(),scene);assert_eq!(engine.state.game_tasks.is_some(),owned_game);assert!(engine.functions.uses_widget_object_class);set_slot_object(&mut engine);
        assert!(engine.state.sprites.get("hero").is_some_and(|sprite|sprite.visible),"the actual authored show precedes this reached effect");
        let publications=engine.renderer.batches.lock().unwrap().len();
        let random=engine.state.random;engine.step_silent().unwrap();assert_eq!(engine.state.random,random,"GetClass and list-index color selection draw no random value");
        if scene||owned_game{
            let style=if scene{&engine.state.scene_execution.as_ref().unwrap().sprite_styles["hero"]}
                else{&engine.state.game_tasks.as_ref().unwrap().sprite_styles["hero"]};
            assert_eq!(style.tint.as_deref(),Some("#ffffffff"));
            assert!(engine.renderer.tints.lock().unwrap().is_empty(),"Native Scene and standalone Game publish the owned batch, never a second historical callback");
            let batches=engine.renderer.batches.lock().unwrap();
            assert_eq!(batches.len(),publications+1,"one accepted native effect publishes exactly one real batch");
            let published=&batches.last().unwrap().state;
            assert!(published.sprites.get("hero").is_some_and(|sprite|sprite.visible));
            let published_style=if scene{&published.scene_execution.as_ref().unwrap().sprite_styles["hero"]}
                else{&published.game_tasks.as_ref().unwrap().sprite_styles["hero"]};
            assert_eq!(published_style.tint.as_deref(),Some("#ffffffff"));assert_eq!(published.pc,engine.state.pc);
        }else{
            assert_eq!(engine.renderer.tints.lock().unwrap().as_slice(),&[("hero".into(),Some("#ffffffff".into()))],"the historical extension point still receives exactly one resolved tint callback");
        }
        // A valid selected color cannot hide a failed lookup in item_0.
        // Exercise the same real reached command with a retired current object.
        let mut refused=fixture.engine();let value=set_slot_object(&mut refused);let handle=refused.widget_reference_handle(&value).unwrap();refused.destroy_widget(handle).unwrap();
        let before=serde_json::to_vec(&refused.state).unwrap();let publications=refused.renderer.batches.lock().unwrap().len();
        assert!(refused.step_silent().is_err());assert_eq!(serde_json::to_vec(&refused.state).unwrap(),before);
        assert_eq!(refused.renderer.batches.lock().unwrap().len(),publications);assert!(refused.renderer.tints.lock().unwrap().is_empty());
    }
}

#[test]
fn object_class1002_r1_scene_tint_function_borrows_candidate_and_refuses_retirement_or_publication_atomically(){
    // The function itself makes detection true; this independently isolates the
    // formerly context-free Scene interpolation route from the detector defect.
    let functions=r##"
function tint_of(value){local resolved=__rvn_widget_object_class_v1(value) return "#ffffffff"}
"##;
    for body in [
        "hero.effect(tint:\"[tint_of(saved_object)]\")",
        "flow.consume_v2(\"scene_tint\",\"effect\"){hero.effect(tint:\"[tint_of(saved_object)]\")}",
    ]{
        let fixture=executable_slot_fixture(true,false,body,functions);let mut engine=fixture.engine();assert!(engine.functions.uses_widget_object_class);set_slot_object(&mut engine);if body.starts_with("flow."){assert!(matches!(engine.script.get(engine.state.pc),Some(Statement::ScheduledConsumer{body,..}) if matches!(body.as_slice(),[Statement::SpriteEffect{..}])));}engine.step_silent().unwrap();assert_eq!(engine.state.scene_execution.as_ref().unwrap().sprite_styles["hero"].tint.as_deref(),Some("#ffffffff"));
        for retire in [false,true]{let mut refused=fixture.engine();let value=set_slot_object(&mut refused);if retire{let handle=refused.widget_reference_handle(&value).unwrap();refused.destroy_widget(handle).unwrap();}else{refused.renderer.reject.store(true,Ordering::SeqCst);}
            let before=serde_json::to_vec(&refused.state).unwrap();let publications=refused.renderer.batches.lock().unwrap().len();assert!(refused.step_silent().is_err());assert_eq!(serde_json::to_vec(&refused.state).unwrap(),before,"PC/RNG/frame/style remain in the actual candidate on refusal");assert_eq!(refused.renderer.batches.lock().unwrap().len(),publications);assert!(refused.renderer.tints.lock().unwrap().is_empty());
        }
    }
}

#[test]
fn object_class1002_detector_scans_only_inspector_expression_without_source_authority(){
    // AST-only capability detection: these descriptive strings grant no source,
    // Graph identity, class admission, execution or native authority.
    let statement=Statement::ScheduledConsumer{version:1,scope:"detector-unit-not-source".into(),consumer:"not-executed".into(),steps:vec![rvn_parser::ConsumerStep::InspectTextTable{producer:"not-executed".into(),value:query()}],body:vec![Statement::SetVar{name:"ignored".into(),value:rvn_parser::Expr::Bool(false)}]};
    assert!(crate::eval::FunctionLibrary::from_script(&[statement]).unwrap().uses_widget_object_class);
    let literal=Statement::SetVar{name:"name_only".into(),value:rvn_parser::Expr::Str("__rvn_widget_object_class_v1".into())};
    assert!(!crate::eval::FunctionLibrary::from_script(&[literal]).unwrap().uses_widget_object_class);
}
