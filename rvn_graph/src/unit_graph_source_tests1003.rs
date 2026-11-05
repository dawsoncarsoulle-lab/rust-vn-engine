//! Targeted real source/Graph/Engine coverage; prepared, not yet executed.
use super::*;
use crate::entry_parameter_identity1003::{EntryParameters, validate_parameter_identity_edit};

fn imported(source:&str)->Vec<GraphDocument>{
    let mut identity=0u64;
    crate::import_script_with_parameter_allocator(&rvn_parser::parse(source).unwrap(),||{
        identity+=1;Ok(format!("{identity:032x}"))
    }).unwrap()
}

fn source()->&'static str {
    "init {set result=0}\nfunction scoped(alpha,beta) -> () {set result=alpha*10+beta return}\nscene label start\ncall scoped(10,11)\n\"After\"\nreturn\n"
}

fn execute(source:&str)->rvn_parser::Value {
    let program=rvn_parser::parse_program(source).unwrap();
    let mut engine=rvn_core::Engine::from_program(program,rvn_core::TerminalRenderer,16).unwrap();
    engine.state.random=rvn_core::random::RandomState::seeded(775);
    engine.enter_label("start").unwrap();
    assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"After".into()}));
    engine.state.vars["result"].clone()
}

#[test]
fn unit_graph1003_real_source_graph_source_engine_preserves_completion_and_parameter_dataflow(){
    let graphs=imported(source());
    let unit=graphs.iter().find(|graph|matches!(graph.kind,GraphKind::UnitFunction{..})).unwrap();
    let pins=parameters(unit).unwrap();
    assert_eq!(pins.iter().map(|pin|pin.label.as_str()).collect::<Vec<_>>(),["alpha","beta"]);
    assert!(pins.iter().all(|pin|pin.cardinality==PinCardinality::Many && pin.default_value.is_none()));
    assert!(unit.nodes.values().any(|node|node.kind==NodeKind::UnitReturn));
    assert!(!graphs.iter().flat_map(|graph|graph.nodes.values()).any(|node|node.kind==NodeKind::FunctionCall));
    let compiled=crate::transpile_project(&graphs).unwrap();
    assert_eq!(execute(source()),rvn_parser::Value::Int(111));
    assert_eq!(execute(&compiled.source),rvn_parser::Value::Int(111));
    assert!(compiled.source.contains("function scoped(alpha, beta) -> ()"));
    let quoted=source().replace("alpha", "`first value`").replace("beta", "`return`");
    let quoted_graphs=imported(&quoted);
    let quoted_compiled=crate::transpile_project(&quoted_graphs).unwrap();
    assert!(quoted_compiled.source.contains("function scoped(`first value`, `return`) -> ()"));
    assert_eq!(execute(&quoted),rvn_parser::Value::Int(111));
    assert_eq!(execute(&quoted_compiled.source),rvn_parser::Value::Int(111));
    let reopened=crate::SourceProject::open(&compiled.source,&graphs).unwrap();
    let cold=reopened.graphs().iter().find(|graph|matches!(graph.kind,GraphKind::UnitFunction{..})).unwrap();
    assert_eq!(parameters(cold).unwrap().iter().map(|pin|pin.parameter_id.clone()).collect::<Vec<_>>(),pins.iter().map(|pin|pin.parameter_id.clone()).collect::<Vec<_>>());
}

#[test]
fn unit_graph1003_owner_save_rename_reorder_and_cold_restore_keep_opaque_bindings(){
    let folder=std::env::temp_dir().join(format!("rvn-unit-source-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&folder).unwrap();
    let result=(||->Result<(),String>{
        let path=folder.join("main.rvn");std::fs::write(&path,source()).unwrap();
        let prepared=imported(source());
        let mut workspace=SourceWorkspace::link(&folder,&path,&prepared)?;
        let graphs=workspace.project().graphs().to_vec();
        let unit=graphs.iter().find(|graph|matches!(graph.kind,GraphKind::UnitFunction{..})).unwrap();
        let declaration=unit.graph_id;let root=entry(unit)?;
        let original_ids=parameters(unit)?.iter().map(|pin|pin.parameter_id.clone()).collect::<Vec<_>>();
        EntryParameters::from_admitted_projection(&workspace,&graphs,declaration,root)?;
        let renamed=workspace.prepare_unit_parameter_rename(&graphs,declaration,original_ids[0].as_ref().unwrap(),"not a name")?;
        let renamed=workspace.prepare_unit_parameter_rename(&renamed,declaration,original_ids[1].as_ref().unwrap(),"return")?;
        EntryParameters::from_admitted_projection(&workspace,&renamed,declaration,root)?;
        let reordered=workspace.prepare_unit_parameter_reorder(&renamed,declaration,&[original_ids[1].clone().unwrap(),original_ids[0].clone().unwrap()])?;
        validate_parameter_identity_edit(&graphs,&reordered)?;
        workspace.save(&reordered)?;
        let published=std::fs::read_to_string(&path).unwrap();
        let published_ast=rvn_parser::parse(&published).unwrap();
        let published_parameters=published_ast.iter().find_map(|statement|match statement {
            rvn_parser::Statement::UnitFunction{name,parameters,..}if name=="scoped"=>Some(parameters),_=>None,
        }).expect("published actual Unit declaration");
        assert_eq!(published_parameters,&["return".to_owned(),"not a name".to_owned()]);
        assert_eq!(execute(&published),rvn_parser::Value::Int(111));
        let cold=SourceWorkspace::open(&folder)?.ok_or("Cold authoring sidecar disappeared")?;
        let unit=cold.project().graphs().iter().find(|graph|graph.graph_id==declaration).unwrap();
        assert_eq!(parameters(unit)?.iter().map(|pin|pin.label.as_str()).collect::<Vec<_>>(),["return","not a name"]);
        assert_eq!(parameters(unit)?.iter().map(|pin|pin.parameter_id.clone()).collect::<Vec<_>>(),vec![original_ids[1].clone(),original_ids[0].clone()]);
        let cold_compiled=crate::transpile_project(cold.project().graphs())?;
        assert_eq!(execute(&cold_compiled.source),rvn_parser::Value::Int(111));
        let (_,arguments)=cold.project().graphs().iter().find_map(|caller|caller.nodes.values().find(|node|node.kind==NodeKind::UnitCall).map(|call|(caller,call.id))).map(|(caller,call)|resolve_call(cold.project().graphs(),caller,call)).ok_or("Cold caller disappeared")??;
        assert_eq!(arguments.iter().map(|pin|pin.parameter_id.clone()).collect::<Vec<_>>(),vec![original_ids[1].clone(),original_ids[0].clone()]);
        let renamed=crate::rename_callable_graphs(cold.project().graphs(),declaration,"renamed_scoped")?;
        assert!(crate::transpile_project(&renamed)?.source.contains("call renamed_scoped("));
        assert_eq!(execute(&crate::transpile_project(&renamed)?.source),rvn_parser::Value::Int(111));
        Ok(())
    })();
    let _=std::fs::remove_dir_all(&folder);result.unwrap();
}

#[test]
fn unit_graph1003_zero_arity_still_requires_real_callee_mode_and_exact_call_shape(){
    let graphs=imported("function empty() -> () {}\nscene label start\ncall empty()\n\"After\"\nreturn");
    crate::transpile_project(&graphs).unwrap();
    for case in 0..4 {
        let mut invalid=graphs.clone();
        if case==0 {invalid.retain(|graph|!matches!(graph.kind,GraphKind::UnitFunction{..}));}
        if case==1 {invalid.iter_mut().find(|graph|matches!(graph.kind,GraphKind::UnitFunction{..})).unwrap().kind=GraphKind::Function{name:"empty".into()};}
        if case>=2 {
            let graph=invalid.iter_mut().find(|graph|graph.nodes.values().any(|node|node.kind==NodeKind::UnitCall)).unwrap();
            let call=graph.nodes.values().find(|node|node.kind==NodeKind::UnitCall).unwrap().id;
            if case==2 {graph.add_pin(call,"result","result",PinDirection::Output,ValueType::Any,PinCardinality::Many).unwrap();}
            else {graph.nodes.get_mut(&call).unwrap().properties.insert("function".into(),PropertyValue::String("absent".into()));}
        }
        let before=invalid.clone();assert!(crate::transpile_project(&invalid).is_err());assert_eq!(invalid,before);
    }
}

#[test]
fn unit_graph1003_external_text_signature_change_requires_explicit_refactor_mapping(){
    let folder=std::env::temp_dir().join(format!("rvn-unit-source-refusal-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&folder).unwrap();
    let result=(||->Result<(),String>{
        let path=folder.join("main.rvn");std::fs::write(&path,source()).unwrap();
        let prepared=imported(source());
        let workspace=SourceWorkspace::link(&folder,&path,&prepared)?;
        let graphs=workspace.project().graphs().to_vec();
        let source_bytes=std::fs::read(&path).unwrap();
        let sidecar=folder.join(".rvn-authoring.json");let sidecar_bytes=std::fs::read(&sidecar).unwrap();
        for (case,changed) in [source().replace("alpha,beta","beta,alpha"),source().replace("alpha","renamed_alpha"),source().replace("alpha,beta","alpha,beta,gamma")].into_iter().enumerate() {
            let before=graphs.clone();
            let error=crate::SourceProject::open(&changed,&graphs).unwrap_err();
            // SourceProject validates the parsed callable arity before identity
            // reconciliation; parse_program itself intentionally only parses.
            if case<2 {assert!(error.contains("mapping"),"{error}");}
            else {
                let parsed=rvn_parser::parse(&changed).unwrap();
                let parameters=parsed.iter().find_map(|statement|match statement {
                    rvn_parser::Statement::UnitFunction{name,parameters,..}if name=="scoped"=>Some(parameters),_=>None,
                }).expect("actual changed declaration");
                let arguments=parsed.iter().find_map(|statement|match statement {
                    rvn_parser::Statement::UnitCall{name,arguments}if name=="scoped"=>Some(arguments),_=>None,
                }).expect("actual unchanged invocation");
                assert_eq!(parameters,&["alpha".to_owned(),"beta".to_owned(),"gamma".to_owned()]);
                assert_eq!(arguments.len(),2);
            }
            assert_eq!(graphs,before);
            assert_eq!(std::fs::read(&path).unwrap(),source_bytes);
            assert_eq!(std::fs::read(&sidecar).unwrap(),sidecar_bytes);
            assert_eq!(execute(&std::fs::read_to_string(&path).unwrap()),rvn_parser::Value::Int(111));
        }
        assert!(crate::import_script(&rvn_parser::parse(source()).unwrap()).unwrap_err().contains("allocator"));
        Ok(())
    })();
    let _=std::fs::remove_dir_all(&folder);result.unwrap();
}

#[test]
fn unit_graph1003_removed_declaration_does_not_authorize_parameter_identity_transfer(){
    let before=imported("function first(alpha) -> () {}\nfunction second(beta) -> () {}\nscene label start\nreturn");
    let first=before.iter().find(|graph|matches!(&graph.kind,GraphKind::UnitFunction{name}if name=="first")).unwrap();
    let identity=parameters(first).unwrap()[0].parameter_id.clone().unwrap();
    let mut after=before.iter().filter(|graph|graph.graph_id!=first.graph_id).cloned().collect::<Vec<_>>();
    let second=after.iter_mut().find(|graph|matches!(graph.kind,GraphKind::UnitFunction{..})).unwrap();
    let id=parameters(second).unwrap()[0].id;
    let pin=second.pins.get_mut(&id).unwrap();pin.key=format!("parameter_{}",identity.as_str());pin.parameter_id=Some(identity);
    let snapshot=after.clone();assert!(validate_parameter_identity_edit(&before,&after).is_err());assert_eq!(after,snapshot);
}

#[cfg(test)]
#[path="unit_macro_graph_contract_tests1021.rs"]
mod unit_macro_interop1021;


#[test]
fn unit_native_reimport1021_owned_parameters_survive_handler_upgrade_save_and_cold_open() {
    for init in ["init {set score=0}\n", "init {}\n", ""] {
        let raw=format!("{init}function scoped(alpha,beta) -> () {{return}}\nhandler clicked(event) {{set score=1}}\nscene label start\ncall scoped(10,11)\n\"After\"\nreturn\n");
        let folder=std::env::temp_dir().join(format!("rvn-unit-native-owner-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&folder).unwrap();
        let result=(||->Result<(),String>{
            let path=folder.join("main.rvn");std::fs::write(&path,&raw).unwrap();
            let mut allocated=0u64;
            let prepared=crate::prepare_initial_unit_link_graphs(&rvn_parser::parse(&raw).unwrap(),&[],||{
                allocated+=1;Ok(format!("{:032x}",0x1000+allocated))
            }).map_err(|error|format!("owner-stage initial-link-allocate init={init:?}: {error}"))?;
            assert_eq!(allocated,2,"only the actual initial link transaction allocates these parameters");
            let mut workspace=SourceWorkspace::link(&folder,&path,&prepared).map_err(|error|format!("owner-stage SourceWorkspace::link init={init:?}: {error}"))?;
            let before=workspace.project().graphs().to_vec();
            let unit=before.iter().find(|g|matches!(g.kind,GraphKind::UnitFunction{..})).unwrap();
            let ids=parameters(unit)?.iter().map(|p|p.parameter_id.clone()).collect::<Vec<_>>();
            assert!(ids.iter().all(Option::is_some));
            let unit_id=unit.graph_id;
            let mut edited=before.clone();
            let handler=edited.iter_mut().find(|g|matches!(g.kind,GraphKind::Handler{..})).unwrap();
            let handler_id=handler.graph_id;handler.enable_native_frame_preparation().map_err(|error|format!("owner-stage enable-Handler-Native init={init:?}: {error}"))?;
            if init=="init {}\n" {
                edited.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap().enable_native_frame_preparation().map_err(|error|format!("owner-stage enable-empty-Init-Native init={init:?}: {error}"))?;
            }
            let source_before=std::fs::read(&path).unwrap();
            let sidecar=folder.join(".rvn-authoring.json");let metadata_before=std::fs::read(&sidecar).unwrap();
            let mut trial=workspace.project().clone();
            trial.apply_visual(workspace.project().source(),&edited).map_err(|error|format!("owner-stage trial.apply_visual init={init:?}: {error}"))?;
            assert_eq!(workspace.project().graphs(),before.as_slice());
            assert_eq!(std::fs::read(&path).unwrap(),source_before);
            assert_eq!(std::fs::read(&sidecar).unwrap(),metadata_before);
            let projected=trial.graphs().iter().find(|g|g.graph_id==unit_id).unwrap();
            assert_eq!(parameters(projected)?.iter().map(|p|p.parameter_id.clone()).collect::<Vec<_>>(),ids);
            assert_eq!(projected.schema_version,GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION);
            assert!(projected.inspector_evaluation.is_legacy(),"the neighbouring Unit14 is never promoted");
            // Exercise the AST Native reimport port as well as SourceWorkspace's
            // source-aware port; an Empty Init declaration needs source text.
            if init!="init {}\n" {
                let imported=crate::reimport_native_frame_script(&rvn_parser::parse(trial.source()).unwrap(),&edited).map_err(|error|format!("owner-stage AST-Native-reimport init={init:?}: {error}"))?;
                assert_eq!(parameters(imported.iter().find(|g|g.graph_id==unit_id).unwrap())?.iter().map(|p|p.parameter_id.clone()).collect::<Vec<_>>(),ids);
            }
            workspace.save(&edited).map_err(|error|format!("owner-stage SourceWorkspace::save init={init:?}: {error}"))?;
            let saved=std::fs::read_to_string(&path).unwrap();
            assert!(saved.contains("function scoped(alpha,beta) -> () {return}"));
            if init=="init {}\n" {assert!(saved.contains("@rvn-native-empty-init"));}
            let cold=SourceWorkspace::open(&folder).map_err(|error|format!("owner-stage SourceWorkspace::cold-open init={init:?}: {error}"))?.ok_or("Cold source owner disappeared")?;
            assert!(cold.source_error.is_none(),"{:?}",cold.source_error);
            let restored=cold.project().graphs().iter().find(|g|g.graph_id==unit_id).unwrap();
            assert_eq!(parameters(restored)?.iter().map(|p|p.parameter_id.clone()).collect::<Vec<_>>(),ids);
            assert_eq!(restored.schema_version,GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION);
            assert!(restored.inspector_evaluation.is_legacy());
            let restored=cold.project().graphs().iter().find(|g|g.graph_id==handler_id).unwrap();
            let authored=edited.iter().find(|g|g.graph_id==handler_id).unwrap();
            assert_eq!(restored.inspector_evaluation,InspectorEvaluationContract::NativeFrameV1);
            assert_eq!(restored.nodes.keys().collect::<Vec<_>>(),authored.nodes.keys().collect::<Vec<_>>());
            assert_eq!(restored.pins.keys().collect::<Vec<_>>(),authored.pins.keys().collect::<Vec<_>>());
            assert_eq!(restored.edges,authored.edges);
            assert_eq!(allocated,2,"save/cold/reimport retain IDs without a fresh allocator");
            Ok(())
        })();
        let _=std::fs::remove_dir_all(&folder);result.unwrap();
    }
}

#[test]
fn unit_native_reimport1021_unowned_or_changed_parameters_refuse_before_publication() {
    let raw="init {set score=0}\nfunction scoped(alpha,beta) -> () {return}\nhandler clicked(event) {set score=1}\nscene label start\ncall scoped(10,11)\n\"After\"\nreturn\n";
    let folder=std::env::temp_dir().join(format!("rvn-unit-native-refusal-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&folder).unwrap();
    let result=(||->Result<(),String>{
        let path=folder.join("main.rvn");std::fs::write(&path,raw).unwrap();
        let mut allocated=0u64;
        let prepared=crate::prepare_initial_unit_link_graphs(&rvn_parser::parse(raw).unwrap(),&[],||{
            allocated+=1;Ok(format!("{:032x}",0x2000+allocated))
        })?;
        let workspace=SourceWorkspace::link(&folder,&path,&prepared)?;
        let before=workspace.project().graphs().to_vec();
        let mut handler=before.iter().find(|g|matches!(g.kind,GraphKind::Handler{..})).unwrap().clone();
        handler.enable_native_frame_preparation()?;
        let compiled=crate::prepare_native_frame_graph(&handler).map_err(|e|e.to_string())?;
        let native=raw.replace("handler clicked(event) {set score=1}",&compiled.source);
        let source_before=std::fs::read(&path).unwrap();
        let sidecar=folder.join(".rvn-authoring.json");let metadata_before=std::fs::read(&sidecar).unwrap();
        assert!(SourceProject::open_at(&path,&native,&[]).unwrap_err().contains("allocator"));
        assert!(crate::import_native_frame_source(&native,rvn_parser::NarrativeContext::ExplicitScenes).unwrap_err().contains("allocator"));
        for (changed,expected) in [
            (native.replace("alpha","renamed_alpha"),"Unit text signature changed"),
            (native.replace("alpha,beta","alpha,beta,gamma").replace("scoped(10,11)","scoped(10,11,12)"),"Unit text signature changed"),
            (native.replacen("handler clicked", "function added(amount) -> () {return}\nhandler clicked",1),"New, renamed or forked Unit text declaration"),
        ] {
            let error=SourceProject::open_at(&path,&changed,&before).unwrap_err();
            assert!(error.contains(expected),"{error}");
            assert_eq!(workspace.project().graphs(),before.as_slice());
            assert_eq!(std::fs::read(&path).unwrap(),source_before);
            assert_eq!(std::fs::read(&sidecar).unwrap(),metadata_before);
            assert_eq!(allocated,2);
        }
        let mut denied_allocations=0;
        assert!(crate::prepare_initial_unit_link_graphs(&rvn_parser::parse(raw).unwrap(),&before,||{
            denied_allocations+=1;Ok("ffffffffffffffffffffffffffffffff".into())
        }).is_err());
        assert_eq!(denied_allocations,0,"an existing owner is not relinked with replacement IDs");
        Ok(())
    })();
    let _=std::fs::remove_dir_all(&folder);result.unwrap();
}

// Distinct frozen/library boundary: no editor ParameterId owner is part of
// ParsedProgram. All authoring preconditions below must pass before diagnosing
// its read-only Native freeze/site/debug projection.
#[test]
fn unit_native_frozen1021_owned_legacy_unit_neighbour_is_not_an_authoring_allocation() {
    let raw="init {set score=0}\nfunction scoped(alpha,beta) -> () {return}\nhandler clicked(event) {set score=1}\nscene label start\ncall scoped(10,11)\n\"After\"\nreturn\n";
    let folder=std::env::temp_dir().join(format!("rvn-unit-frozen-owner-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&folder).unwrap();
    let result=(||->Result<(),String>{
        let path=folder.join("main.rvn");std::fs::write(&path,raw).unwrap();
        let mut allocated=0u64;
        let prepared=crate::prepare_initial_unit_link_graphs(&rvn_parser::parse(raw).unwrap(),&[],||{
            allocated+=1;Ok(format!("{:032x}",0x3000+allocated))
        }).map_err(|e|format!("frozen-owner-stage initial link: {e}"))?;
        assert_eq!(allocated,2);
        let mut workspace=SourceWorkspace::link(&folder,&path,&prepared)
            .map_err(|e|format!("frozen-owner-stage link: {e}"))?;
        let mut edited=workspace.project().graphs().to_vec();
        edited.iter_mut().find(|g|matches!(g.kind,GraphKind::Handler{..})).unwrap()
            .enable_native_frame_preparation().map_err(|e|format!("frozen-owner-stage Handler opt-in: {e}"))?;
        workspace.save(&edited).map_err(|e|format!("frozen-owner-stage save: {e}"))?;
        drop(workspace);
        let cold=SourceWorkspace::open(&folder).map_err(|e|format!("frozen-owner-stage cold open: {e}"))?
            .ok_or("frozen-owner-stage cold owner absent")?;
        assert!(cold.source_error.is_none(),"{:?}",cold.source_error);
        let unit=cold.project().graphs().iter().find(|g|matches!(g.kind,GraphKind::UnitFunction{..})).unwrap();
        assert_eq!(unit.schema_version,GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION);
        assert!(unit.inspector_evaluation.is_legacy());
        let expected=edited.iter().find(|g|g.graph_id==unit.graph_id).unwrap();
        assert_eq!(parameters(unit)?.iter().map(|p|p.parameter_id.clone()).collect::<Vec<_>>(),
            parameters(expected)?.iter().map(|p|p.parameter_id.clone()).collect::<Vec<_>>());
        let program=cold.project().resolved_program().map_err(|e|format!("frozen-owner-stage resolved Program: {e}"))?;
        assert!(program.script.iter().any(|s|matches!(s,rvn_parser::Statement::UnitFunction{..})));
        assert!(crate::native_source_binding::contains_native(&program.script));
        let source_before=std::fs::read(&path).unwrap();
        let sidecar=folder.join(".rvn-authoring.json");
        let metadata_before=std::fs::read(&sidecar).unwrap();
        let graphs_before=cold.project().graphs().to_vec();
        let outcomes=[
            ("freeze",cold.freeze_native_program().map(|p|p.script.len())),
            ("sites",crate::native_source_sites(&program).map(|sites|sites.len())),
            ("debug-map",crate::NativeDebugSourceMap::from_program(&program).map(|map|map.origins().len())),
        ];
        assert_eq!(std::fs::read(&path).unwrap(),source_before);
        assert_eq!(std::fs::read(&sidecar).unwrap(),metadata_before);
        assert_eq!(cold.project().graphs(),graphs_before.as_slice());
        assert_eq!(allocated,2);
        assert!(outcomes.iter().all(|(_,result)|result.is_ok()),
            "frozen-owner-final: frozen Native source validation must not allocate Legacy Unit authoring IDs: {outcomes:?}");
        Ok(())
    })();
    let _=std::fs::remove_dir_all(&folder);result.unwrap();
}
