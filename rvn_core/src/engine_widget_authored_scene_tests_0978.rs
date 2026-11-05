//! Real .rvn source, actual Graph descriptor admission and existing CPU Host.
//! Real Graph factory nodes feed Core; physical viewport parity remains open.
use super::*;
use rvn_graph::GraphKind;
struct Story {
    files: Fixture,
    program: rvn_parser::ParsedProgram,
    classes: AdmittedWidgetClasses,
}
impl Story {
    fn new(body: &str) -> Self {
        Self::with_initializer(
            body,
            "set initialized=initialized+1 set init_roll=random(1,999)",
        )
    }
    fn with_initializer(body: &str, initializer: &str) -> Self {
        Self::with_initializers(body, initializer, "set initialized=0 set init_roll=0 set class_input=__rvn_widget_class_none_v1() set factory_arguments=[]")
    }
    fn with_initializers(body: &str, initializer: &str, game_initializer: &str) -> Self {
        let files = Fixture::new(false);
        let class_source=format!("handler initialize(event){{{initializer}}}\nscreen panel(value){{return {{\"host\":{{\"events\":{{\"initialized\":\"initialize\"}}}},\"root\":[]}}}}\n");
        let class_path = files.root.join("class.rvn");
        fs::write(&class_path, &class_source).unwrap();
        let source=format!("use \"class.rvn\"\ninit{{{game_initializer}}}\nscene label Play\n{body}\n\"Ready\"\nreturn\n");
        let source=Self::actual_graph_source(&files,&source);
        fs::write(files.root.join("main.rvn"), source).unwrap();
        let project = rvn_graph::SourceProject::open_at(&class_path, &class_source, &[]).unwrap();
        let program =
            rvn_parser::parse_file_program_with_uses(files.root.join("main.rvn")).unwrap();
        let screen = project
            .graphs()
            .iter()
            .find(|graph| matches!(&graph.kind,GraphKind::Screen{name}if name=="panel"))
            .unwrap();
        let handler = project
            .graphs()
            .iter()
            .find(|graph| matches!(&graph.kind,GraphKind::Handler{name}if name=="initialize"))
            .unwrap();
        let screen_id = WidgetGraphRef16::from_u64(screen.graph_id.get());
        let descriptor = WidgetBlueprintV1 {
            uuid: A.into(),
            source: "class.rvn".into(),
            screen_graph: screen_id,
            members: vec![
                screen_id,
                WidgetGraphRef16::from_u64(handler.graph_id.get()),
            ],
        }
        .encode()
        .unwrap();
        let entries = [(A, Some("widgets/authored.rvnwidget"))];
        let admitted = rvn_graph::prepare_widget_runtime_classes(
            &files.root,
            &program,
            WidgetIndexInput::Table(&entries),
            LocatorCase::Insensitive,
            &[rvn_graph::WidgetRuntimeAsset {
                locator: "widgets/authored.rvnwidget",
                bytes: descriptor.as_bytes(),
            }],
            &[rvn_graph::WidgetRuntimeSource {
                source: "class.rvn",
                project: &project,
            }],
        )
        .unwrap();
        let classes = AdmittedWidgetClasses::admit(
            &program,
            admitted
                .classes()
                .values()
                .map(|class| CompilerWidgetClass {
                    index_uuid: class.uuid().into(),
                    locator: class.locator().into(),
                    descriptor_bytes: class.descriptor_bytes().to_vec(),
                    members: class
                        .members()
                        .iter()
                        .map(|member| CompilerWidgetMember {
                            graph: member.graph().get(),
                            kind: match member.kind() {
                                rvn_graph::WidgetDefinitionKind::Screen => WidgetMemberKind::Screen,
                                rvn_graph::WidgetDefinitionKind::Handler => {
                                    WidgetMemberKind::Handler
                                }
                                rvn_graph::WidgetDefinitionKind::Function => {
                                    WidgetMemberKind::Function
                                }
                            },
                            definition: member.definition().into(),
                            root_index: member.root_index(),
                            origin: member.origin().clone(),
                        })
                        .collect(),
                })
                .collect(),
        )
        .unwrap();
        Self {
            files,
            program,
            classes,
        }
    }
    fn actual_graph_source(files:&Fixture,raw:&str)->String {
        let parsed=rvn_parser::parse_with_origins(raw,rvn_parser::NarrativeContext::ExplicitScenes).unwrap();
        let mut edits=Vec::new();let mut factories=Vec::new();
        fn copies(statement:&Statement,rows:&mut Vec<(String,String)>){match statement{
            Statement::SetVar{name,value:rvn_parser::Expr::Var(source)}|Statement::LocalVar{name,value:rvn_parser::Expr::Var(source)}=>rows.push((name.clone(),source.clone())),
            Statement::ScheduledConsumer{body,..}=>for child in body{copies(child,rows)},_=>{},
        }}
        let mut authored_copies=Vec::new();for statement in &parsed.script{copies(statement,&mut authored_copies);}
        for(statement,span)in parsed.script.iter().zip(&parsed.spans){
            if let Statement::ScheduledConsumer{steps,body,..}=statement {
                assert!(steps.is_empty(),"the fixture must author real producer nodes rather than copying opaque capture strings");
                let [leaf]=body.as_slice() else{panic!("one actual fixture instruction required")};
                let [leaf_span]=span.children.as_slice() else{panic!("actual statement span required")};
                let replacement=if let Some(input)=rvn_parser::widget_create_effect::parts(leaf).unwrap(){
                    let class=match input.class{
                        rvn_parser::widget_create_effect::ClassInput::Asset(uuid)=>rvn_parser::Expr::Call{name:"__rvn_widget_class_v1".into(),args:vec![rvn_parser::Expr::Str(uuid.into())]},
                        rvn_parser::widget_create_effect::ClassInput::Null=>rvn_parser::Expr::Call{name:"__rvn_widget_class_none_v1".into(),args:vec![]},
                        rvn_parser::widget_create_effect::ClassInput::Value(value)=>value.clone(),
                    };
                    factories.push((input.binding.to_owned(),input.local,class,input.arguments.clone()));
                    format!("{} {}=0",if input.local{"local"}else{"set"},input.binding)
                }else{raw[leaf_span.range.clone()].to_owned()};
                edits.push((span.range.clone(),replacement));
            }
        }
        let mut seed=raw.to_owned();for(range,value)in edits.into_iter().rev(){seed.replace_range(range,&value);}
        let path=files.root.join("main.rvn");fs::write(&path,&seed).unwrap();
        let mut project=rvn_graph::SourceProject::open_at(&path,&seed,&[]).unwrap();
        let mut edited=project.graphs().to_vec();
        let graph=edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label{name}if name=="Play")).unwrap();
        graph.enable_native_frame_preparation().unwrap();
        // These are explicit fixture authoring choices derived from its real
        // factory/copy expressions, not Any or an automatic user-Int conversion.
        let mut object_bindings:std::collections::BTreeSet<String>=factories.iter().map(|(name,_,_,_)|name.clone()).collect();
        for _ in 0..=authored_copies.len(){let before=object_bindings.len();for(name,source)in &authored_copies{if object_bindings.contains(source){object_bindings.insert(name.clone());}}if before==object_bindings.len(){break;}}
        for name in &object_bindings{if let Some(definition)=graph.variables.get_mut(name){definition.value_type=rvn_graph::ValueType::WidgetObject;}}
        let pins:Vec<_>=graph.nodes.values().filter(|node|node.properties.get("name").is_some_and(|v|matches!(v,rvn_graph::PropertyValue::String(name)if object_bindings.contains(name)))).flat_map(|node|node.pins.clone()).collect();
        for pin in pins{let pin=graph.pins.get_mut(&pin).unwrap();if matches!(pin.key.as_str(),"value"|"value_out"){pin.value_type=rvn_graph::ValueType::WidgetObject;}}
        let mut replacements=Vec::new();
        for(name,local,class,args)in factories{
            let candidates:Vec<_>=graph.nodes.values().filter(|n|n.kind==if local{rvn_graph::NodeKind::LocalVariable}else{rvn_graph::NodeKind::SetVariable}).filter(|n|n.properties.get("name")==Some(&rvn_graph::PropertyValue::String(name.clone()))).map(|n|n.id).collect();
            let [assignment]=candidates.as_slice()else{panic!("one genuine reached source assignment required: {name}")};
            replacements.push((*assignment,class,args));
        }
        graph.replace_assignments_with_widget_factories(&replacements).unwrap();
        project.apply_visual(&seed,&edited).unwrap();
        let source=project.source().to_owned();
        // Identities and complete source come only from the real Graph writer.
        assert!(!source.contains("scene:Play"));
        source
    }
    fn engine(&self, host: Host) -> Engine<Host> {
        Engine::from_program_with_widget_classes(
            self.program.clone(),
            self.classes.clone(),
            host,
            16,
        )
        .unwrap()
    }
    fn at_factory(&self, host: Host) -> Engine<Host> {
        let mut engine = self.engine(host);
        for _ in 0..64 {
            if matches!(engine.script.get(engine.state.pc),Some(Statement::ScheduledConsumer{body,..})if body.first().is_some_and(|leaf|matches!(rvn_parser::widget_create_effect::parts(leaf),Ok(Some(_)))))
            {
                assert!(engine
                    .state
                    .scene_execution
                    .as_ref()
                    .and_then(|execution| execution.active.as_ref())
                    .is_some());
                return engine;
            }
            engine.preview_step_statement().unwrap();
        }
        panic!("actual Scene factory was not reached within bounded source statements")
    }
}
fn create(binding: &str, uuid: &str, args: &str, local: bool) -> String {
    format!("flow.consume_v2(\"scene:Play\",\"factory-{binding}\"){{{} {binding}=__rvn_widget_create_v1(__rvn_widget_class_v1(\"{uuid}\"),{args})}}",if local{"local"}else{"set"})
}
#[test]
fn authored0978_scene_source_actual_graph_class_factory_returns_two_real_values_and_reads_never_recreate(
) {
    let story = Story::new(&format!(
        "{}\nset first_copy=first\n{}\nset second_copy=second",
        create("first", A, "[random(1,100)]", false),
        create("second", A, "[7]", true)
    ));
    let host = Host::default();
    let mut engine = story.at_factory(host.clone());
    let before = host.batches.lock().unwrap().len();
    engine.preview_step_statement().unwrap();
    assert_eq!(host.batches.lock().unwrap().len(), before + 1);
    assert_eq!(engine.state.vars["initialized"], Value::Int(1));
    let first = engine.state.vars["first"].clone();
    let first_handle = engine.widget_reference_handle(&first).unwrap();
    engine.step_until_interaction().unwrap();
    let second = engine.state.vars["second_copy"].clone();
    let second_handle = engine.widget_reference_handle(&second).unwrap();
    assert_ne!(first, second);
    assert_ne!(first_handle, second_handle);
    assert_eq!(engine.state.vars["first_copy"], first);
    assert_eq!(engine.state.vars["initialized"], Value::Int(2));
    assert_eq!(engine.state.ui.screens.len(), 2);
    assert!(engine.state.ui.screens.iter().all(|screen| screen
        .widget_class
        .as_ref()
        .is_some_and(|binding| binding.class.uuid == A
            && !binding.viewport_attached
            && !binding.pending_materialization)));
    assert_eq!(engine.state.ui.screens[1].arguments, vec![Value::Int(7)]);
    assert!(engine.interface_hosts().unwrap().is_empty());
    assert!(engine
        .capture_ui_input_source(first_handle.alias())
        .is_none());
}
#[test]
fn authored0978_factory_argument_class_and_renderer_refusals_keep_result_pc_rng_registry_and_initialized_atomic(
) {
    for (body,tamper_arguments) in [
        (create("result", "27411a3e431e4c9388b78a2b9825b130", "[random(1,100)]", false),false),
        (create("result", A, "[1,2]", false),false),
        (create("result", A, "factory_arguments", false),true),
    ] {
        let story = Story::new(&body);
        let host = Host::default();
        let mut engine = story.at_factory(host.clone());
        if tamper_arguments{engine.state.vars.insert("factory_arguments".into(),Value::Int(99));}
        let before = serde_json::to_vec(&engine.state).unwrap();
        let published = host.batches.lock().unwrap().len();
        assert!(engine.preview_step_statement().is_err());
        assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
        assert_eq!(host.batches.lock().unwrap().len(), published);
        assert!(!engine.state.vars.contains_key("result"));
    }
    let story = Story::new(&create("result", A, "[random(1,100)]", false));
    let host = Host::default();
    let mut engine = story.at_factory(host.clone());
    let before = serde_json::to_vec(&engine.state).unwrap();
    let published = host.batches.lock().unwrap().len();
    host.reject.store(true, Ordering::SeqCst);
    assert!(engine.preview_step_statement().is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    assert_eq!(host.batches.lock().unwrap().len(), published);
    assert_eq!(engine.state.vars["initialized"], Value::Int(0));
    assert!(engine.state.ui.screens.is_empty());
}
#[test]
fn authored0978_explicit_null_class_returns_typed_null_without_an_object_initializer_or_seed_draw()
{
    let story=Story::new("flow.consume_v2(\"scene:Play\",\"factory-null\"){set result=__rvn_widget_create_v1(__rvn_widget_class_none_v1(),[])}");
    let mut engine = story.at_factory(Host::default());
    let random = engine.state.random;
    let order = engine.state.ui.next_order;
    engine.preview_step_statement().unwrap();
    assert!(matches!(&engine.state.vars["result"],Value::Widget(reference)if reference.is_null()));
    assert_eq!(engine.state.random, random);
    assert_eq!(engine.state.ui.next_order, order);
    assert!(engine.state.ui.screens.is_empty());
    assert_eq!(engine.state.vars["initialized"], Value::Int(0));
}
#[test]
fn authored0978_dynamic_class_requires_actual_typed_uuid_receipt_and_never_casts_strings_or_dicts()
{
    let source="flow.consume_v2(\"scene:Play\",\"factory-dynamic\"){set result=__rvn_widget_create_v1(class_input,[random(1,100)])}";
    for value in [
        Value::Str(A.into()),
        Value::Dict(Default::default()),
        Value::WidgetClass(Box::new(Some(
            rvn_parser::widget_reference::WidgetClassReference::new(A.into(), [0; 32]).unwrap(),
        ))),
    ] {
        let story = Story::new(source);
        let host = Host::default();
        let mut engine = story.at_factory(host.clone());
        engine.state.vars.insert("class_input".into(), value);
        let before = serde_json::to_vec(&engine.state).unwrap();
        let published = host.batches.lock().unwrap().len();
        assert!(engine.preview_step_statement().is_err());
        assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
        assert_eq!(host.batches.lock().unwrap().len(), published);
    }
    let story = Story::new(source);
    let mut engine = story.at_factory(Host::default());
    let class = engine.widget_classes.class(A).unwrap().identity();
    let class =
        rvn_parser::widget_reference::WidgetClassReference::new(class.uuid.clone(), class.receipt)
            .unwrap();
    engine
        .state
        .vars
        .insert("class_input".into(), Value::WidgetClass(Box::new(Some(class))));
    engine.preview_step_statement().unwrap();
    assert!(engine
        .widget_reference_handle(&engine.state.vars["result"])
        .is_ok());
    assert_eq!(engine.state.vars["initialized"], Value::Int(1));
}
#[test]
fn authored0978_saved_real_scene_result_hydrates_at_new_epoch_without_replaying_factory_or_initialized(
) {
    let story = Story::new(&format!(
        "{}\nset result_copy=result",
        create("result", A, "[7]", true)
    ));
    let mut engine = story.engine(Host::default());
    engine.step_until_interaction().unwrap();
    let old = engine.state.vars["result_copy"].clone();
    let old_handle = engine.widget_reference_handle(&old).unwrap();
    let ui = serde_json::to_value(&engine.state.ui).unwrap();
    let saved = engine
        .capture_save15(&story.program, None, 1, "actual Scene factory".into())
        .unwrap()
        .encode()
        .unwrap();
    let mut restored = story.engine(Host::default());
    restored.step_until_interaction().unwrap();
    restored
        .restore_save15(
            &story.program,
            crate::save::consumer_checkpoint::Save15::decode(&saved).unwrap(),
        )
        .unwrap();
    assert_eq!(serde_json::to_value(&restored.state.ui).unwrap(), ui);
    assert_eq!(restored.state.vars["initialized"], Value::Int(1));
    assert_eq!(restored.state.ui.screens.len(), 1);
    assert!(restored.widget_reference_handle(&old).is_err());
    let rebound = restored
        .widget_reference_handle(&restored.state.vars["result_copy"])
        .unwrap();
    assert_eq!(rebound.alias(), old_handle.alias());
    assert_ne!(rebound, old_handle);
}
#[test]
fn authored0978_pure_function_screen_handler_and_captured_producer_never_execute_the_factory_fallback(
) {
    let parsed = rvn_parser::parse(&format!(
        "function factory_refusal() {{local result=__rvn_widget_create_v1(__rvn_widget_class_v1(\"{A}\"),[1])}}"
    )).unwrap();
    let [Statement::Function { body: leaf, .. }] = parsed.as_slice() else { panic!("actual function owner required") };
    assert!(crate::eval::validate_screen(&leaf).is_err());
    assert!(crate::eval::validate_handler(&leaf).is_err());
    let story=Story::new(&format!("set result=[__rvn_widget_create_v1(__rvn_widget_class_v1(\"{A}\"),[1])]"));
    let host = Host::default();
    let mut engine = story.engine(host.clone());
    let before_count = engine.state.ui.screens.len();
    assert!(engine.step_until_interaction().is_err());
    assert_eq!(engine.state.ui.screens.len(), before_count);
    assert_eq!(engine.state.vars["initialized"], Value::Int(0));
    assert!(!engine.state.vars.contains_key("result"));
}

#[test]
fn authored0978_initialized_mutates_candidate_and_rng_then_actual_command_failure_publishes_no_result_or_partial_object(
) {
    let body = create("result", A, "[random(1,100)]", false);
    let story=Story::with_initializer(&body,"set initialized=initialized+1 set init_roll=random(1,999) local unavailable=\"missing_widget_screen\" ui.open(unavailable,[],false,0)");
    let host = Host::default();
    let mut engine = story.at_factory(host.clone());
    let before = serde_json::to_vec(&engine.state).unwrap();
    let published = host.batches.lock().unwrap().len();
    assert!(engine.preview_step_statement().is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    assert_eq!(host.batches.lock().unwrap().len(), published);
    assert_eq!(engine.state.vars["initialized"], Value::Int(0));
    assert_eq!(engine.state.vars["init_roll"], Value::Int(0));
    assert!(engine.state.ui.screens.is_empty());
    assert!(!engine.state.vars.contains_key("result"));
}
#[test]
fn authored0978_real_stopped_factory_trace_save15_cold_restore_resumes_single_result_and_initialized(
) {
    let story = Story::new(&create("result", A, "[random(1,100)]", false));
    let mut engine = story.at_factory(Host::default());
    let before = serde_json::to_vec(&engine.state).unwrap();
    let mut session = engine
        .debug_begin(DebugOperation::Statement, vec![])
        .unwrap();
    for _ in 0..32 {
        if engine
            .debug_step(&mut session, crate::editor_debugger::StepKind::Into)
            .unwrap()
            .0
            == DebugProgress::Stopped
        {
            break;
        }
    }
    assert!(!session.is_complete());
    assert!(session.snapshot().error.is_none());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    let saved = engine
        .capture_save15(
            &story.program,
            Some(&session),
            1,
            "stopped actual Scene factory".into(),
        )
        .unwrap()
        .encode()
        .unwrap();
    let mut cold = story.at_factory(Host::default());
    let mut resumed = cold
        .restore_save15(
            &story.program,
            crate::save::consumer_checkpoint::Save15::decode(&saved).unwrap(),
        )
        .unwrap()
        .expect("actual retained factory Statement operation");
    for _ in 0..256 {
        if cold
            .debug_step(&mut resumed, crate::editor_debugger::StepKind::Continue)
            .unwrap()
            .0
            == DebugProgress::Complete
        {
            break;
        }
    }
    assert!(resumed.is_complete());
    assert_eq!(cold.state.vars["initialized"], Value::Int(1));
    assert_eq!(cold.state.ui.screens.len(), 1);
    assert!(cold
        .widget_reference_handle(&cold.state.vars["result"])
        .is_ok());
    let state = serde_json::to_vec(&cold.state).unwrap();
    assert!(cold
        .debug_step(&mut resumed, crate::editor_debugger::StepKind::Continue)
        .is_err());
    assert_eq!(serde_json::to_vec(&cold.state).unwrap(), state);
}

#[test]
fn authored0980_legacy_public_factory_homonyms_execute_actual_user_function_map_in_wrapper() {
    let script=rvn_parser::parse(r#"
        function widget_create(a,b) {return a+b}
        function widget_class(a) {return a*2}
        function widget_class_none() {return 3}
        function customwrapper(a,b) {local result=widget_create(a,b) return result+widget_class(a)+widget_class_none()}
    "#).unwrap();
    assert!(rvn_parser::validate_logic(&script, false).is_empty());
    let library = crate::eval::FunctionLibrary::from_script(&script).unwrap();
    let expression = rvn_parser::Expr::Call {
        name: "customwrapper".into(),
        args: vec![rvn_parser::Expr::Int(4), rvn_parser::Expr::Int(5)],
    };
    assert_eq!(
        library
            .eval(&expression, &std::collections::HashMap::new())
            .unwrap(),
        Value::Int(20)
    );
    // The internal versioned command is still an effect, never a pure producer.
    let effect = rvn_parser::Expr::Call {
        name: "__rvn_widget_create_v1".into(),
        args: vec![rvn_parser::Expr::Int(4), rvn_parser::Expr::ListLit(vec![])],
    };
    assert!(library
        .eval(&effect, &std::collections::HashMap::new())
        .is_err());
}

#[test]
fn class_vm1000_actual_scene_typed_nullable_getter_feeds_factory_and_initialized_handler_uses_same_catalog() {
    for nullable in [false, true] {
        let class = if nullable { "__rvn_widget_class_none_v1()".to_owned() } else { format!("__rvn_widget_class_v1(\"{A}\")") };
        let arguments = if nullable { "[]" } else { "[1]" };
        let source = format!("flow.consume_v2(\"scene:Play\",\"class-value\"){{local selected={class}}}\nflow.consume_v2(\"scene:Play\",\"create-value\"){{local made=__rvn_widget_create_v1(selected,{arguments})}}");
        let initializer = format!("set initialized=initialized+1 set callback_class=__rvn_widget_class_v1(\"{A}\") set callback_null=__rvn_widget_class_none_v1()");
        let story = Story::with_initializer(&source, &initializer);
        let mut engine = story.at_factory(Host::default());
        let before = match engine.state.vars["initialized"] { Value::Int(v) => v, _ => panic!("counter") };
        engine.preview_step_statement().unwrap();
        let active = engine.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap();
        assert_eq!(active.locals["selected"], if nullable {Value::WidgetClass(Box::new(None))} else {engine.functions.eval(&rvn_parser::Expr::Call{name:"__rvn_widget_class_v1".into(), args:vec![rvn_parser::Expr::Str(A.into())]}, &Default::default()).unwrap()});
        if nullable {
            assert!(matches!(&active.locals["made"],Value::Widget(reference)if reference.portable().is_none()));
            assert_eq!(engine.state.vars["initialized"], Value::Int(before));
            assert!(engine.state.ui.screens.iter().all(|instance|instance.widget_class.is_none()));
        } else {
            assert!(engine.widget_reference_handle(&active.locals["made"]).is_ok());
            assert_eq!(engine.state.vars["initialized"], Value::Int(before+1));
            assert_eq!(engine.state.vars["callback_class"], active.locals["selected"]);
            assert_eq!(engine.state.vars["callback_null"], Value::WidgetClass(Box::new(None)));
        }
    }
}

#[test]
fn class_vm1000_actual_startup_and_fresh_inject_catalog_before_first_init_getter() {
    let game_initializer = format!("set initialized=0 set init_roll=0 set init_class=__rvn_widget_class_v1(\"{A}\") set init_null=__rvn_widget_class_none_v1()");
    let story = Story::with_initializers("", "set initialized=initialized+1", &game_initializer);
    let engine = story.engine(Host::default());
    let class = crate::widget_class_getter_0981::resolve_class_data(&story.classes, Some(A)).unwrap();
    let expected = Value::WidgetClass(Box::new(class));
    assert_eq!(engine.state.vars["init_class"], expected);
    assert_eq!(engine.state.vars["init_null"], Value::WidgetClass(Box::new(None)));
    assert_eq!(engine.state.vars["initialized"], Value::Int(0));
    assert!(Arc::ptr_eq(&engine.functions.widget_classes, &engine.widget_classes));
    let fresh = engine.prepare_fresh_startup(Host::default(), 16).unwrap().finish_synchronous().unwrap();
    assert_eq!(fresh.state.vars["init_class"], expected);
    assert_eq!(fresh.state.vars["init_null"], Value::WidgetClass(Box::new(None)));
    assert!(Arc::ptr_eq(&fresh.functions.widget_classes, &fresh.widget_classes));
    let foreign = Fixture::new(true);
    assert!(Engine::from_program_with_widget_classes(story.program.clone(), foreign.classes.clone(), Host::default(), 16).is_err(), "foreign catalog must fail source admission before any getter/Init");
}

#[test]
fn class_vm1000_dynamic_null_with_arguments_refuses_without_pc_rng_or_publication_commit() {
    let source = "flow.consume_v2(\"scene:Play\",\"select-null\"){local selected=__rvn_widget_class_none_v1()}\nflow.consume_v2(\"scene:Play\",\"null-invalid-args\"){local made=__rvn_widget_create_v1(selected,[random(1,99)])}";
    let story = Story::new(source);
    let host = Host::default();
    let mut engine = story.at_factory(host.clone());
    let before = serde_json::to_vec(&engine.state).unwrap();
    let publications = host.batches.lock().unwrap().len();
    assert!(engine.preview_step_statement().is_err());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), before);
    assert_eq!(host.batches.lock().unwrap().len(), publications);
}

/// Source authoring and logical event routing, not a physical viewport test.
#[test]
fn authored1021_same_class_two_live_instances_keep_state_properties_and_callbacks_separate() {
    use crate::widget_class_source_0950::standard_macros::CellInspection;
    use rvn_graph::{GraphDocument, NodeId, NodeKind, PropertyValue as P, SourceWorkspace};
    use rvn_ui::programmable::ScreenEventKind;

    fn wire(g: &mut GraphDocument, from: NodeId, output: &str, to: NodeId, input: &str) {
        let output = g.pin_by_key(from, output).unwrap().id;
        let input = g.pin_by_key(to, input).unwrap().id;
        g.connect(output, input).unwrap();
    }
    fn handle_for_factory(engine: &Engine<Host>, binding: &str) -> WidgetInstanceHandle {
        let Value::Widget(reference) = &engine.state.vars[binding] else {
            panic!("the actual source factory must return a typed Widget value");
        };
        let portable = reference.portable().unwrap();
        assert_eq!(portable.class().uuid(), A);
        // The value supplies only the observed alias. The actual Engine must
        // re-acquire and validate the resident incarnation; no handle is built.
        let handle = engine.widget_instance_handle(portable.alias()).unwrap();
        let owner = engine.widget_backend_owner(&handle).unwrap();
        assert_eq!(owner.target().incarnation, portable.incarnation());
        assert_eq!(owner.class().uuid, portable.class().uuid());
        handle
    }
    fn instance<'a>(engine: &'a Engine<Host>, alias: &str) -> &'a crate::ui::ScreenInstance {
        engine
            .state
            .ui
            .screens
            .iter()
            .find(|i| i.name == alias)
            .unwrap()
    }
    fn caption(engine: &Engine<Host>, alias: &str) -> String {
        engine
            .interface_views()
            .unwrap()
            .into_iter()
            .find(|v| v.name == alias)
            .unwrap()
            .root
            .find("caption")
            .unwrap()
            .text
            .clone()
    }
    fn dispatch(engine: &mut Engine<Host>, alias: &str, kind: ScreenEventKind) {
        let event = input(engine, alias, kind);
        assert!(engine.interface_event_scoped(event).unwrap());
    }

    // Reuse the existing owned temporary-directory/cleanup fixture and CPU
    // Host, not its old program/classes. All admission below uses this source.
    let files = Fixture::new(false);
    let source_path = files.root.join("main.rvn");
    let source = r#"
init {set hits=0 set last_screen=""}
handler pressed(event){set hits=hits+1 set last_screen=event["screen"]}
screen panel(caption){return component("go","button",dict("layout_contract","widget_v1","events",dict("click","pressed"),"focus_order",0),[component("caption","text",dict("text",caption,"parent_slot",dict("kind","button","padding",dict("left",4,"top",2,"right",4,"bottom",2),"horizontal","center","vertical","center")),[])])}
scene label Play
"Ready"
return
"#;
    fs::write(&source_path, source).unwrap();
    let mut workspace = SourceWorkspace::link(&files.root, &source_path, &[]).unwrap();
    let mut graphs = workspace.project().graphs().to_vec();
    let screen = graphs
        .iter()
        .find(|g| matches!(&g.kind, GraphKind::Screen {name} if name == "panel"))
        .unwrap();
    let screen_id = WidgetGraphRef16::from_u64(screen.graph_id.get());
    let handler_id = graphs
        .iter()
        .find(|g| matches!(&g.kind, GraphKind::Handler {name} if name == "pressed"))
        .unwrap()
        .graph_id;
    let descriptor = WidgetBlueprintV1 {
        uuid: A.into(),
        source: "main.rvn".into(),
        screen_graph: screen_id,
        members: vec![screen_id, WidgetGraphRef16::from_u64(handler_id.get())],
    }
    .encode()
    .unwrap();
    let locator = "widgets/pair.rvnwidget";
    fs::create_dir_all(files.root.join("assets/widgets")).unwrap();
    fs::write(files.root.join("assets").join(locator), &descriptor).unwrap();
    let manifest = format!("[project]\nmain_script='main.rvn'\nstart_label='Play'\n[paths]\nassets='assets'\n[widget_blueprints]\n'{A}'='{locator}'\n");
    fs::write(files.root.join("rvn.toml"), &manifest).unwrap();

    // The one real Handler contains one real DoOnce site. Both instances will
    // call this same compiled member; only their actual runtime cells differ.
    let handler = graphs
        .iter_mut()
        .find(|g| g.graph_id == handler_id)
        .unwrap();
    let root = handler
        .nodes
        .values()
        .find(|n| n.kind == NodeKind::HandlerEntry)
        .unwrap()
        .id;
    let output = handler.pin_by_key(root, "exec_out").unwrap().id;
    let edge = handler
        .edges
        .values()
        .find(|e| e.output == output)
        .unwrap()
        .clone();
    let once = handler
        .add_catalog_node(NodeKind::DoOnce, [200.0, 0.0])
        .unwrap();
    handler.remove_edge(edge.id).unwrap();
    wire(handler, root, "exec_out", once, "exec_in");
    let completed = handler.pin_by_key(once, "completed").unwrap().id;
    handler.connect(completed, edge.input).unwrap();
    handler.enable_native_frame_preparation().unwrap();

    let scene = graphs
        .iter_mut()
        .find(|g| matches!(&g.kind, GraphKind::Label {name} if name == "Play"))
        .unwrap();
    scene.enable_native_frame_preparation().unwrap();
    let root = scene
        .nodes
        .values()
        .find(|n| n.kind == NodeKind::Label)
        .unwrap()
        .id;
    let output = scene.pin_by_key(root, "exec_out").unwrap().id;
    let old = scene
        .edges
        .values()
        .find(|e| e.output == output)
        .unwrap()
        .clone();
    assert_eq!(
        scene.nodes[&scene.pins[&old.input].node].kind,
        NodeKind::Dialogue
    );
    scene.remove_edge(old.id).unwrap();
    let mut previous = root;
    let mut result_bindings = Vec::new();
    for (index, text) in ["First instance", "Second instance"].iter().enumerate() {
        let x = 400.0 + index as f64 * 600.0;
        let create = scene
            .add_catalog_node(NodeKind::WidgetCreate, [x, 0.0])
            .unwrap();
        let Some(P::String(binding)) = scene.nodes[&create].properties.get("name") else {
            panic!("the actual catalogue factory must allocate its result binding");
        };
        assert!(rvn_parser::is_variable_binding_name(binding));
        result_bindings.push(binding.clone());
        let class = scene
            .add_catalog_node(NodeKind::WidgetClassValue, [x, 180.0])
            .unwrap();
        scene
            .set_property(class, "uuid", P::String(A.into()))
            .unwrap();
        wire(scene, class, "value", create, "class");
        let text_node = scene
            .add_catalog_node(NodeKind::Literal, [x, 300.0])
            .unwrap();
        scene
            .set_property(text_node, "value", P::String((*text).into()))
            .unwrap();
        assert!(scene
            .list_input_sources(create, "arguments")
            .unwrap()
            .is_empty());
        scene
            .append_list_input(create, "arguments", text_node, "value")
            .unwrap();
        let viewport = scene
            .add_catalog_node(NodeKind::WidgetAddToViewport, [x + 250.0, 0.0])
            .unwrap();
        scene
            .set_pin_default(viewport, "z_order", P::Int(index as i64))
            .unwrap();
        wire(scene, create, "value_out", viewport, "object");
        wire(scene, previous, "exec_out", create, "exec_in");
        wire(scene, create, "exec_out", viewport, "exec_in");
        previous = viewport;
    }
    scene
        .connect(
            scene.pin_by_key(previous, "exec_out").unwrap().id,
            old.input,
        )
        .unwrap();
    assert_eq!(result_bindings.len(), 2);
    assert_ne!(result_bindings[0], result_bindings[1]);
    assert_eq!(
        scene
            .nodes
            .values()
            .filter(|n| n.kind == NodeKind::WidgetCreate)
            .count(),
        2
    );
    assert_eq!(
        scene
            .nodes
            .values()
            .filter(|n| n.kind == NodeKind::WidgetAddToViewport)
            .count(),
        2
    );

    workspace.save(&graphs).unwrap();
    let saved = workspace.project().graphs().to_vec();
    let published = fs::read(&source_path).unwrap();
    drop(workspace);
    let cold = SourceWorkspace::open(&files.root).unwrap().unwrap();
    assert!(cold.source_error.is_none(), "{:?}", cold.source_error);
    assert!(cold.recovery().unwrap().is_none());
    assert_eq!(
        cold.project().graphs(),
        saved.as_slice(),
        "complete saved Graphs, IDs, caches, defaults and layout survive cold open"
    );
    assert_eq!(fs::read(&source_path).unwrap(), published);
    assert_eq!(
        fs::read_to_string(files.root.join("rvn.toml")).unwrap(),
        manifest
    );
    assert_eq!(
        fs::read_to_string(files.root.join("assets").join(locator)).unwrap(),
        descriptor
    );
    let program = cold.project().resolved_program().unwrap();
    let site = program
        .script
        .iter()
        .find_map(|s| match s {
            Statement::Handler { name, body, .. } if name == "pressed" => {
                body.iter().find_map(|s| match s {
                    Statement::StandardMacroDispatch { dispatch } => {
                        Some((dispatch.site.node.graph, dispatch.site.node.node))
                    }
                    _ => None,
                })
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        site,
        (handler_id.get(), once.get()),
        "cell observation uses the actual compiled Graph site"
    );
    let entries = [(A, Some(locator))];
    let admitted = rvn_graph::prepare_widget_runtime_classes(
        &files.root,
        &program,
        WidgetIndexInput::Table(&entries),
        LocatorCase::Insensitive,
        &[rvn_graph::WidgetRuntimeAsset {
            locator,
            bytes: descriptor.as_bytes(),
        }],
        &[rvn_graph::WidgetRuntimeSource {
            source: "main.rvn",
            project: cold.project(),
        }],
    )
    .unwrap();
    assert_eq!(admitted.classes().len(), 1);
    let classes = AdmittedWidgetClasses::admit(
        &program,
        admitted
            .classes()
            .values()
            .map(|class| CompilerWidgetClass {
                index_uuid: class.uuid().into(),
                locator: class.locator().into(),
                descriptor_bytes: class.descriptor_bytes().to_vec(),
                members: class
                    .members()
                    .iter()
                    .map(|member| CompilerWidgetMember {
                        graph: member.graph().get(),
                        kind: match member.kind() {
                            rvn_graph::WidgetDefinitionKind::Screen => WidgetMemberKind::Screen,
                            rvn_graph::WidgetDefinitionKind::Handler => WidgetMemberKind::Handler,
                            rvn_graph::WidgetDefinitionKind::Function => WidgetMemberKind::Function,
                        },
                        definition: member.definition().into(),
                        root_index: member.root_index(),
                        origin: member.origin().clone(),
                    })
                    .collect(),
            })
            .collect(),
    )
    .unwrap();
    let mut engine =
        Engine::from_program_with_widget_classes(program, classes, Host::default(), 16).unwrap();
    engine.enter_label("Play").unwrap();
    engine.step_until_interaction().unwrap();
    // The authored class explicitly chooses the existing WidgetV1 contract.
    // A real Button has one content child with its actual typed Button slot;
    // legacy unmarked component trees intentionally retain their old policy.
    let rendered = engine.interface_views().unwrap();
    assert_eq!(rendered.len(), 2);
    for view in rendered {
        assert_eq!(view.root.layout_contract, Some(rvn_ui::programmable::LayoutContract::WidgetV1));
        assert_eq!(view.root.kind, rvn_ui::programmable::ComponentKind::Button);
        assert_eq!(view.root.children.len(), 1);
        assert_eq!(view.root.children[0].id, "caption");
        assert_eq!(view.root.children[0].parent_slot,
            Some(rvn_ui::programmable::ParentSlot::for_parent(rvn_ui::programmable::ComponentKind::Button).unwrap()));
    }
    let a = handle_for_factory(&engine, &result_bindings[0]);
    let b = handle_for_factory(&engine, &result_bindings[1]);
    assert_ne!(
        engine.state.vars[&result_bindings[0]],
        engine.state.vars[&result_bindings[1]]
    );
    assert_ne!(a, b);
    let owner_a = engine.widget_backend_owner(&a).unwrap();
    let owner_b = engine.widget_backend_owner(&b).unwrap();
    assert_eq!(owner_a.class(), owner_b.class());
    assert_ne!(owner_a.target(), owner_b.target());
    assert_eq!(engine.state.ui.screens.len(), 2);
    assert!(engine
        .state
        .ui
        .screens
        .iter()
        .all(|i| i.viewport_attached()));
    assert_eq!(
        instance(&engine, a.alias()).arguments,
        vec![Value::Str("First instance".into())]
    );
    assert_eq!(
        instance(&engine, b.alias()).arguments,
        vec![Value::Str("Second instance".into())]
    );
    assert_eq!(caption(&engine, a.alias()), "First instance");
    assert_eq!(caption(&engine, b.alias()), "Second instance");
    let empty = CellInspection::DoOnce {
        initialized: false,
        completed: false,
    };
    let consumed = CellInspection::DoOnce {
        initialized: true,
        completed: true,
    };
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&a, site.0, site.1)
            .unwrap()
            .1,
        empty
    );
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&b, site.0, site.1)
            .unwrap()
            .1,
        empty
    );
    let untouched_b = instance(&engine, b.alias()).clone();
    // ScreenView.focus is the existing per-Screen effective child focus:
    // an unset stored focus projects to the first available focusable child.
    // Compare B's complete real projection, not an assumed None baseline.
    let untouched_b_projection = engine.interface_views().unwrap().into_iter()
        .find(|view| view.name == b.alias()).unwrap();
    assert_eq!(untouched_b_projection.focus.as_deref(), Some("go"));

    dispatch(&mut engine, a.alias(), ScreenEventKind::Focus);
    assert_eq!(instance(&engine, a.alias()).focus.as_deref(), Some("go"));
    assert_eq!(
        instance(&engine, b.alias()),
        &untouched_b,
        "focusing A does not change any B instance field"
    );
    let views = engine.interface_views().unwrap();
    assert_eq!(
        views
            .iter()
            .find(|v| v.name == a.alias())
            .unwrap()
            .focus
            .as_deref(),
        Some("go")
    );
    assert_eq!(
        views.iter().find(|v| v.name == b.alias()).unwrap(),
        &untouched_b_projection,
        "focusing A preserves B's complete projected root and effective focus"
    );
    assert_eq!(engine.state.vars["hits"], Value::Int(0));
    dispatch(&mut engine, a.alias(), ScreenEventKind::Click);
    assert_eq!(engine.state.vars["hits"], Value::Int(1));
    assert_eq!(
        engine.state.vars["last_screen"],
        Value::Str(a.alias().into())
    );
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&a, site.0, site.1)
            .unwrap()
            .1,
        consumed
    );
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&b, site.0, site.1)
            .unwrap()
            .1,
        empty
    );
    assert_eq!(
        instance(&engine, b.alias()),
        &untouched_b,
        "A's callback does not change any B instance field"
    );
    dispatch(&mut engine, a.alias(), ScreenEventKind::Click);
    assert_eq!(
        engine.state.vars["hits"],
        Value::Int(1),
        "A's own cell suppresses its second callback"
    );
    let untouched_a = instance(&engine, a.alias()).clone();
    dispatch(&mut engine, b.alias(), ScreenEventKind::Click);
    assert_eq!(
        engine.state.vars["hits"],
        Value::Int(2),
        "B has its own unconsumed cell at the identical member site"
    );
    assert_eq!(
        engine.state.vars["last_screen"],
        Value::Str(b.alias().into())
    );
    assert_eq!(
        instance(&engine, a.alias()),
        &untouched_a,
        "B's callback does not change any A instance field"
    );
    assert_eq!(
        engine
            .inspect_widget_macro_cell(&b, site.0, site.1)
            .unwrap()
            .1,
        consumed
    );
    assert_eq!(caption(&engine, a.alias()), "First instance");
    assert_eq!(caption(&engine, b.alias()), "Second instance");

    let stale_a = input(&engine, a.alias(), ScreenEventKind::Click);
    let stale_a_after_reattach = input(&engine, a.alias(), ScreenEventKind::Click);
    engine.remove_widget_from_viewport(&a).unwrap();
    assert!(engine.capture_ui_input_source(a.alias()).is_none());
    assert!(engine.capture_ui_input_source(b.alias()).is_some());
    let state = serde_json::to_vec(&engine.state).unwrap();
    assert!(!engine.interface_event_scoped(stale_a).unwrap());
    assert_eq!(
        serde_json::to_vec(&engine.state).unwrap(),
        state,
        "detached A input cannot affect B or any story state"
    );
    assert_eq!(engine.inspect_widget_macro_cell(&a, site.0, site.1).unwrap().1, consumed);
    assert_eq!(engine.inspect_widget_macro_cell(&b, site.0, site.1).unwrap().1, consumed);
    engine.add_widget_to_viewport(&a, false, 0).unwrap();
    // Detach invalidates the old presentation revision even when the same
    // logical target is attached again. Acquire no replacement old event.
    let reattached_state = serde_json::to_vec(&engine.state).unwrap();
    assert!(!engine.interface_event_scoped(stale_a_after_reattach).unwrap());
    assert_eq!(serde_json::to_vec(&engine.state).unwrap(), reattached_state);
    assert_eq!(engine.inspect_widget_macro_cell(&a, site.0, site.1).unwrap().1, consumed);
    assert_eq!(engine.inspect_widget_macro_cell(&b, site.0, site.1).unwrap().1, consumed);
    assert_eq!(
        engine.widget_backend_owner(&a).unwrap().target(),
        owner_a.target()
    );
    dispatch(&mut engine, a.alias(), ScreenEventKind::Click);
    dispatch(&mut engine, b.alias(), ScreenEventKind::Click);
    assert_eq!(
        engine.state.vars["hits"],
        Value::Int(2),
        "reattachment preserves each independent consumed cell"
    );
    assert_eq!(
        engine.state.vars["last_screen"],
        Value::Str(b.alias().into())
    );
    assert_eq!(engine.state.ui.screens.len(), 2);
    assert!(engine
        .state
        .ui
        .screens
        .iter()
        .all(|i| i.viewport_attached()));
    assert_eq!(
        fs::read(&source_path).unwrap(),
        published,
        "runtime did not edit the authored program"
    );
}
