//! Real browser resolver and concrete Bevy resource/renderer; no window/GPU.
use super::*;
#[test]
fn unit_bevy1003_browser_includes_and_real_world_update_share_the_actual_core_owner(){
    let main="use \"functions.rvn\"\r\ninit {set score=10 set calls=0}\r\nscene label start\r\nlocal score=99\r\ncall outer(score)\r\n\"After\"\r\n";
    let library="function inner(n)->(){set score=n+1 set calls=calls+1}\nfunction outer(n)->(){local private=7 call inner(n)}\nfunction unused()->(){set forbidden=1}\n";
    let sources=HashMap::from([("main.rvn".into(),main.into()),("functions.rvn".into(),library.into())]);
    let program=resolve_web_program("main.rvn",&sources,&["functions.rvn".into(),"main.rvn".into()]).unwrap();
    assert_eq!(program.sources.iter().map(|source|source.source.as_str()).collect::<Vec<_>>(),vec![main,library]);
    let unit=program.script.iter().position(|s|matches!(s,Statement::UnitFunction{name,..}if name=="outer")).unwrap();
    assert_eq!(program.origins[unit].source.as_deref(),Some(Path::new("functions.rvn")));assert_eq!(program.origins[unit].span.children.len(),2);
    let mut game=Engine::from_program(program,BevyRenderer::new(),8).unwrap();game.enter_label("start").unwrap();
    let mut app=App::new();app.insert_resource(VnEngine(game));
    app.add_systems(Update,|mut game:ResMut<VnEngine>|{
        assert!(matches!(game.0.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text=="After"));
    });
    app.update();
    let game=&app.world().resource::<VnEngine>().0;
    assert_eq!(game.state.vars["score"],rvn_parser::Value::Int(100));assert_eq!(game.state.vars["calls"],rvn_parser::Value::Int(1));
    assert!(!game.state.vars.contains_key("private"));assert!(!game.state.vars.contains_key("forbidden"));
    assert_eq!(game.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals["score"],rvn_parser::Value::Int(99));
    let batch=game.renderer.scene_batch.as_ref().unwrap();assert_eq!(batch.state.vars["score"],rvn_parser::Value::Int(100));
    let previous=serde_json::to_vec(&game.state).unwrap();app.update();
    assert_eq!(serde_json::to_vec(&app.world().resource::<VnEngine>().0.state).unwrap(),previous);
}
#[test]
fn unit_bevy1003_failed_candidate_cannot_replace_an_existing_world_engine(){
    let old=Engine::from_program(rvn_parser::parse_program("scene label previous\n\"Old\"\n").unwrap(),BevyRenderer::new(),8).unwrap();
    let mut app=App::new();app.insert_resource(VnEngine(old));
    let old=app.world().resource::<VnEngine>();let instance=old.0.instance_id();let before=serde_json::to_vec(&old.0.state).unwrap();
    let sources=HashMap::from([("main.rvn".into(),"function bad()->(){set persistent.flag=true}\nscene label start\ncall bad()\n\"After\"\n".into())]);
    let candidate=resolve_web_program("main.rvn",&sources,&["main.rvn".into()]).unwrap();
    assert!(Engine::from_program(candidate,BevyRenderer::new(),8).is_err());
    let old=app.world().resource::<VnEngine>();assert_eq!(old.0.instance_id(),instance);assert_eq!(serde_json::to_vec(&old.0.state).unwrap(),before);
}
