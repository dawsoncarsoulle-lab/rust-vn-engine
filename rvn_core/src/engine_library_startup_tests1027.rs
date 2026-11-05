//! Actual Source/Graph/codec/Core startup tests. CPU definitions, not native ACK.
use super::*;
use rvn_parser::{
    enum_asset::{EnumAsset, EnumAssetId, EnumCommand, EnumMemberId},
    enum_asset_json::EnumIndexInput,
    function_library_asset1023::{FunctionLibraryId, FunctionLibraryMember, LibraryFunctionAccess},
    widget_asset::{WidgetBlueprintV1, WidgetGraphRef16},
};
const L: &str = "11111111111141118111111111111111";
const P: &str = "33333333333343338333333333333333";
const E: &str = "329f41eae6ba4b4bb71dfaf60fb50001";
const M: &str = "229f41eae6ba4b4bb71dfaf60fb50001";
const W: &str = "17411a3e431e4c9388b78a2b9825b130";
#[derive(Clone, Default)]
struct Observe(
    std::rc::Rc<std::cell::Cell<usize>>,
    std::rc::Rc<std::cell::RefCell<crate::engine::debugger_commands::LibrarySceneTestHost1027>>,
);
impl Renderer for Observe {
    fn validate_scene_batch(&self,batch:&crate::renderer::SceneRenderBatch)->Result<(),String>{
        self.1.borrow().validate_scene_batch(batch)
    }
    fn publish_scene_batch(&mut self, batch: crate::renderer::SceneRenderBatch) {
        self.1.borrow_mut().publish_scene_batch(batch);
        self.0.set(self.0.get() + 1)
    }
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(
        &mut self,
        _: &str,
        _: Option<&str>,
        _: &Position,
        _: &Transition,
        _: Option<&SpriteState>,
    ) {
    }
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {
        0
    }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
        0
    }
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}
fn libraries() -> Vec<FunctionLibraryAsset> {
    vec![FunctionLibraryAsset::new(
        FunctionLibraryId::parse(L).unwrap(),
        vec![
            FunctionLibraryMember {
                graph_id: 11,
                access: LibraryFunctionAccess::Public,
                pure: false,
            },
            FunctionLibraryMember {
                graph_id: 12,
                access: LibraryFunctionAccess::Public,
                pure: true,
            },
        ],
    )
    .unwrap()]
}
fn enum_asset() -> EnumAsset {
    EnumAsset::new(EnumAssetId::parse(E).unwrap(), "Current".into())
        .unwrap()
        .prepare(EnumCommand::Add {
            member: EnumMemberId::parse(M).unwrap(),
            display_name: "First".into(),
            description: String::new(),
        })
        .unwrap()
        .after()
        .clone()
}
fn enums() -> EnumAssets1025 {
    let rows = [(E, Some("Current.rvnenum"))];
    EnumAssets1025::admit(
        EnumIndexInput::Table(&rows),
        &[(
            "Current.rvnenum".into(),
            enum_asset().encode().unwrap().into_bytes(),
        )],
        true,
    )
    .unwrap()
}
fn declarations() -> String {
    format!("functions.library_v1(\"{L}\"){{functions.member_v1(\"11\",[]){{function complete()->(){{set played=7}}}}functions.member_v1(\"12\",[\"{P}\"]){{function echo(n){{return n}}}}}}\nscreen panel(){{return []}}\n")
}
fn init() -> String {
    format!("init{{\nset played=0\nset chosen=functions.value_v1(\"{L}\",\"12\",{{\"{P}\":__rvn_enum_value_v1(\"{E}\",\"{M}\")}})\n}}\n")
}
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new(extra: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "rvn-library-joint1027-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let source = declarations()
            + extra
            + &format!("scene label start\nfunctions.invoke_v1(\"{L}\",\"11\",{{}})\n\"Ready\"\n");
        std::fs::write(root.join("main.rvn"), source).unwrap();
        Self(root)
    }
    fn program(&self) -> rvn_parser::ParsedProgram {
        rvn_parser::parse_file_program_with_uses(self.0.join("main.rvn")).unwrap()
    }
    fn classes(&self, program: &rvn_parser::ParsedProgram) -> AdmittedWidgetClasses {
        use crate::widget_class_source_0950::{
            CompilerWidgetClass, CompilerWidgetMember, WidgetMemberKind,
        };
        let scopes = program
            .script
            .iter()
            .filter_map(|s| {
                if let Statement::FunctionLibrary { scope } = s {
                    Some(scope.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let assets = libraries();
        let resources = rvn_graph::LibraryGraphResources1027::prepare(&scopes, &assets).unwrap();
        let graphs =
            rvn_graph::import_library_program1027(&program.script, &resources, &[]).unwrap();
        let screen = graphs
            .iter()
            .find(|g| matches!(g.kind, rvn_graph::GraphKind::Screen { .. }))
            .unwrap();
        let root_index = program
            .script
            .iter()
            .position(|s| matches!(s,Statement::Screen{name,..}if name=="panel"))
            .unwrap();
        let graph = WidgetGraphRef16::from_u64(screen.graph_id.get());
        let descriptor = WidgetBlueprintV1 {
            uuid: W.into(),
            source: "main.rvn".into(),
            screen_graph: graph,
            members: vec![graph],
        }
        .encode()
        .unwrap();
        AdmittedWidgetClasses::admit(
            program,
            vec![CompilerWidgetClass {
                index_uuid: W.into(),
                locator: "panel.rvnwidget".into(),
                descriptor_bytes: descriptor.into_bytes(),
                members: vec![CompilerWidgetMember {
                    graph: graph.get(),
                    kind: WidgetMemberKind::Screen,
                    definition: "panel".into(),
                    root_index,
                    origin: program.origins[root_index].clone(),
                }],
            }],
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn pending(
    program: rvn_parser::ParsedProgram,
    assets: &[FunctionLibraryAsset],
    enums: EnumAssets1025,
    classes: AdmittedWidgetClasses,
    renderer: Observe,
) -> Result<EngineStartup<Observe>, RuntimeError> {
    Engine::prepare_library_enum_startup1027(
        program,
        assets,
        enums,
        classes,
        renderer,
        8,
        HashMap::new(),
        Default::default(),
        None,
    )
}
#[test]
fn joint_startup1027_actual_library_enum_widget_classes_survive_init_then_scene_unit_uses_same_resources(
) {
    let fixture = Fixture::new(&init());
    let program = fixture.program();
    let classes = fixture.classes(&program);
    let widget_receipt = classes.receipt().cloned();
    let resources = libraries();
    let renderer = Observe::default();
    let publications = renderer.0.clone();
    let owner = pending(program, &resources, enums(), classes, renderer).unwrap();
    assert_eq!(publications.get(), 0, "preparation does not execute Init");
    let mut engine = owner.finish_synchronous().unwrap();
    assert_eq!(
        engine.state.vars.get("chosen"),
        Some(&Value::Enum(
            enum_asset().value(EnumMemberId::parse(M).unwrap()).unwrap()
        ))
    );
    assert_eq!(engine.functions.library_assets1023(), resources);
    assert_eq!(engine.widget_classes.receipt(), widget_receipt.as_ref());
    assert!(engine.widget_classes.class(W).is_some());
    engine.enter_label("start").unwrap();
    engine.step_until_interaction().unwrap();
    assert_eq!(engine.state.vars.get("played"), Some(&Value::Int(7)));
    assert!(engine
        .functions
        .enum_assets1025
        .value(
            EnumAssetId::parse(E).unwrap(),
            EnumMemberId::parse(M).unwrap()
        )
        .is_ok());
}
#[test]
fn joint_startup1027_all_actual_init_roots_still_refuse_retained_unit_calls_before_any_publication()
{
    for body in [
        format!("functions.invoke_v1(\"{L}\",\"11\",{{}})"),
        format!("if true{{functions.invoke_v1(\"{L}\",\"11\",{{}})}}"),
        format!("while false{{functions.invoke_v1(\"{L}\",\"11\",{{}})}}"),
    ] {
        let fixture = Fixture::new(&(init() + &format!("init{{{body}}}\n")));
        let renderer = Observe::default();
        let count = renderer.0.clone();
        let error = pending(
            fixture.program(),
            &libraries(),
            enums(),
            Default::default(),
            renderer,
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("Init Unit call"), "{error}");
        assert_eq!(count.get(), 0);
    }
}
#[test]
fn joint_startup1027_missing_actual_enum_library_and_changed_widget_program_refuse_before_init() {
    let fixture = Fixture::new(&init());
    let program = fixture.program();
    let classes = fixture.classes(&program);
    for missing in [true, false] {
        let renderer = Observe::default();
        let count = renderer.0.clone();
        let result = if missing {
            pending(
                program.clone(),
                &libraries(),
                Default::default(),
                classes.clone(),
                renderer,
            )
        } else {
            pending(program.clone(), &[], enums(), classes.clone(), renderer)
        };
        assert!(result.is_err());
        assert_eq!(count.get(), 0);
    }
    let replacement = Fixture::new(&(init() + "init{set different=1}\n"));
    let renderer = Observe::default();
    let count = renderer.0.clone();
    let error = pending(
        replacement.program(),
        &libraries(),
        enums(),
        classes,
        renderer,
    )
    .err()
    .unwrap();
    assert!(
        error.to_string().contains("Widget classes differ"),
        "{error}"
    );
    assert_eq!(count.get(), 0);
}
#[test]
fn joint_startup1027_old_global_unit14_retained_owner_guard_is_preserved() {
    let fixture = Fixture::new(&(init() + "function old_unit()->(){return}\n"));
    let renderer = Observe::default();
    let count = renderer.0.clone();
    let error = pending(
        fixture.program(),
        &libraries(),
        enums(),
        Default::default(),
        renderer,
    )
    .err()
    .unwrap();
    assert!(
        error.to_string().contains("Legacy Unit14 retained"),
        "{error}"
    );
    assert_eq!(count.get(), 0);
}

#[test]
fn joint_startup1027_public_sync_constructor_preserves_current_resources_and_persistent_state() {
    let fixture=Fixture::new(&init());let program=fixture.program();let classes=fixture.classes(&program);
    let mut engine=Engine::from_library_enum_program1027(program,&libraries(),enums(),classes,Observe::default(),8,HashMap::from([("retained".into(),Value::Int(19))]),Default::default()).unwrap();
    assert_eq!(engine.state.vars.get("chosen"),Some(&Value::Enum(enum_asset().value(EnumMemberId::parse(M).unwrap()).unwrap())));
    engine.step_until_interaction().unwrap();assert_eq!(engine.state.vars.get("played"),Some(&Value::Int(7)));
    assert_eq!(engine.persistent_vars.get("retained"),Some(&Value::Int(19)));
}

#[test]
fn joint_fresh1027_pure_library_alone_keeps_original_resources_and_selects_the_real_pending_owner(){
 let assets=vec![FunctionLibraryAsset::new(FunctionLibraryId::parse(L).unwrap(),vec![FunctionLibraryMember{graph_id:12,access:LibraryFunctionAccess::Public,pure:true}]).unwrap()];
 let program=rvn_parser::parse_program(&format!("functions.library_v1(\"{L}\"){{functions.member_v1(\"12\",[]){{function actual(){{return 9}}}}}}\ninit{{set initialized=functions.value_v1(\"{L}\",\"12\",{{}})}}\nscene label start\nset played=functions.value_v1(\"{L}\",\"12\",{{}})\n\"After\"\n")).unwrap();
 let engine=Engine::from_library_enum_program1027(program,&assets,Default::default(),Default::default(),Observe::default(),8,HashMap::from([("persistent.retained".into(),Value::Int(21))]),Default::default()).unwrap();
 assert!(engine.requires_owned_startup_for_fresh(),"Library alone is a retained resource, not an empty Enum/Widget shortcut");let before=serde_json::to_vec(&engine.state).unwrap();let old=engine.runtime_epoch();
 let pending=engine.prepare_fresh_startup(Observe::default(),8).unwrap();assert!(pending.snapshot().epoch>old);assert_ne!(pending.snapshot().instance,engine.instance_id());assert!(!pending.state().vars.contains_key("initialized"));assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);
 let mut fresh=pending.finish_synchronous().unwrap();assert_eq!(fresh.functions.library_assets1023(),assets);assert_eq!(fresh.state.vars.get("initialized"),Some(&Value::Int(9)));assert_eq!(fresh.persistent_vars.get("persistent.retained"),Some(&Value::Int(21)));fresh.step_until_interaction().unwrap();assert_eq!(fresh.state.vars.get("played"),Some(&Value::Int(9)));
}
#[test]
fn joint_fresh1027_library_unit_declaration_is_warm_source_not_portable_save_authority(){
 let fixture=Fixture::new(&init());let program=fixture.program();let classes=fixture.classes(&program);let engine=Engine::from_library_enum_program1027(program,&libraries(),enums(),classes,Observe::default(),8,HashMap::new(),Default::default()).unwrap();
 let frozen=engine.frozen_startup_program.as_ref().unwrap();assert!(engine.validate_save15_program(frozen).is_err(),"portable Unit Save15 remains refused");let original=serde_json::to_vec(&engine.state).unwrap();
 let pending=engine.prepare_fresh_startup(Observe::default(),8).unwrap();
 let mut fresh=pending.finish_synchronous().unwrap();assert!(Arc::ptr_eq(fresh.frozen_startup_program.as_ref().unwrap(),frozen));assert!(Arc::ptr_eq(&fresh.widget_classes,&engine.widget_classes));assert!(Arc::ptr_eq(&fresh.functions.enum_assets1025,&engine.functions.enum_assets1025));assert_eq!(fresh.functions.library_assets1023(),engine.functions.library_assets1023());assert_eq!(fresh.state.vars.get("chosen"),engine.state.vars.get("chosen"));fresh.step_until_interaction().unwrap();assert_eq!(fresh.state.vars.get("played"),Some(&Value::Int(7)));assert_eq!(serde_json::to_vec(&engine.state).unwrap(),original);
 assert!(fresh.validate_save15_program(fresh.frozen_startup_program.as_ref().unwrap()).is_err());
 let mut second=engine.fresh(Observe::default(),8).unwrap();second.step_until_interaction().unwrap();assert_eq!(second.state.vars.get("played"),Some(&Value::Int(7)));
}
#[test]
fn joint_fresh1027_changed_executable_or_frozen_source_refuses_without_changing_original_state(){
 let fixture=Fixture::new(&init());let program=fixture.program();let classes=fixture.classes(&program);let mut engine=Engine::from_library_enum_program1027(program,&libraries(),enums(),classes,Observe::default(),8,HashMap::new(),Default::default()).unwrap();let before=serde_json::to_vec(&engine.state).unwrap();
 let original=engine.script.clone();engine.script.push(Statement::SetVar{name:"injected".into(),value:rvn_parser::Expr::Int(1)});assert!(engine.prepare_fresh_startup(Observe::default(),8).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);engine.script=original;
 let original=engine.frozen_startup_program.clone();let changed=rvn_parser::parse_program("scene label start\nreturn\n").unwrap();engine.frozen_startup_program=Some(Arc::new(changed));assert!(engine.prepare_fresh_startup(Observe::default(),8).is_err());assert_eq!(serde_json::to_vec(&engine.state).unwrap(),before);engine.frozen_startup_program=original;assert!(engine.prepare_fresh_startup(Observe::default(),8).is_ok());
}

#[test]
fn joint_fresh1027_pure_no_plan_binds_actual_emitted_instructions_without_running_or_mutating_original(){
 let assets=vec![FunctionLibraryAsset::new(FunctionLibraryId::parse(L).unwrap(),vec![FunctionLibraryMember{graph_id:12,access:LibraryFunctionAccess::Public,pure:true}]).unwrap()];
 let program=rvn_parser::parse_program(&format!("functions.library_v1(\"{L}\"){{functions.member_v1(\"12\",[]){{function actual(){{return 9}}}}}}\nscene label start\nset played=functions.value_v1(\"{L}\",\"12\",{{}})\n\"After\"\n")).unwrap();
 let renderer=Observe::default();let publications=renderer.0.clone();
 let mut engine=Engine::from_library_enum_program1027(program,&assets,Default::default(),Default::default(),renderer,8,HashMap::new(),Default::default()).unwrap();
 assert!(!engine.consumer_program.has_plans());
 assert!(!FunctionLibrary::contains_unit_source(&engine.script));
 let original=engine.script.clone();let frozen=engine.frozen_startup_program.clone().unwrap();
 let baseline=serde_json::to_vec(&engine.state).unwrap();let published=publications.get();
 assert_eq!(crate::debugger_provenance::SourceMap::emitted_identity(&engine.script).unwrap(),engine.consumer_program.source.as_ref().unwrap().emitted);
 let pending=engine.prepare_fresh_startup(Observe::default(),8).unwrap();assert!(!pending.state().vars.contains_key("played"));drop(pending);
 engine.script.push(Statement::SetVar{name:"injected".into(),value:rvn_parser::Expr::Int(1)});
 let error=engine.prepare_fresh_startup(Observe::default(),8).err().expect("changed actual no-plan executable must refuse");
 assert!(error.to_string().contains("Fresh Library executable differs"),"{error}");
 assert_eq!(serde_json::to_vec(&engine.state).unwrap(),baseline);assert_eq!(publications.get(),published);assert!(Arc::ptr_eq(engine.frozen_startup_program.as_ref().unwrap(),&frozen));
 engine.script=original;let mut fresh=engine.prepare_fresh_startup(Observe::default(),8).unwrap().finish_synchronous().unwrap();
 fresh.step_until_interaction().unwrap();assert_eq!(fresh.state.vars.get("played"),Some(&Value::Int(9)));
 assert_eq!(serde_json::to_vec(&engine.state).unwrap(),baseline);assert_eq!(publications.get(),published);
}
