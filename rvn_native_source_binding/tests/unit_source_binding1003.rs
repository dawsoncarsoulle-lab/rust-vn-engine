use rvn_native_source_binding::FrozenNativeSource;
use rvn_core::{consumer_schedule::ConsumerSourceBinding,random::RandomState,Engine,TerminalRenderer};
use std::{fs,path::PathBuf};
fn folder(name:&str)->PathBuf {
    let base=std::env::var_os("RVN_UNIT_SOURCE_TEST_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let path=base.join(format!("rvn-unit-binding-{name}-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&path).unwrap();fs::canonicalize(path).unwrap()
}
fn files(name:&str)->PathBuf {
    let root=folder(name);
    fs::write(root.join("functions.rvn"),"// é library\r\nfunction unused()->(){set forbidden=1}\r\nfunction inner(n)->(){set score=n+1 set calls=calls+1}\r\nfunction outer(n)->(){local private=7 call inner(n)}\r\n").unwrap();
    fs::write(root.join("main.rvn"),"use \"functions.rvn\"\r\ninit {set score=10 set calls=0}\r\nscene label start\r\nlocal score=99\r\ncall outer(score)\r\n\"After\"\r\n").unwrap();root
}
#[test]
fn unit_source1003_real_included_nonempty_functions_freeze_and_reach_actual_core_owner(){
    let root=files("reached");let main=root.join("main.rvn");let library=root.join("functions.rvn");
    let program=rvn_parser::parse_file_program_with_uses(&main).unwrap();
    assert_eq!(program.sources.len(),2);
    assert!(program.origins.iter().any(|origin|origin.source.as_ref()==Some(&library)&&!origin.span.children.is_empty()));
    let frozen=FrozenNativeSource::from_frozen_program(program).unwrap();
    assert_eq!(*frozen.binding(),ConsumerSourceBinding::from_program(frozen.program()).unwrap());
    assert_ne!(frozen.binding().emitted,[0;32]);
    let binding=frozen.binding().clone();
    // Plain source has no identity-bearing NativeFrame directives. Zero sites
    // are deliberately not a Graph/debug/portable-activation capability.
    assert!(frozen.sites().is_empty());assert!(frozen.debug_origins().origins().is_empty());
    // The actual Engine intentionally starts with RandomState::fresh(). This
    // source has no random initializer; seed both real Engines before entry.
    let seed=RandomState::seeded(775);
    let mut game=frozen.into_engine(TerminalRenderer,8).unwrap();game.state.random=seed;game.enter_label("start").unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..})if text=="After"));
    assert_eq!(game.state.vars["score"],rvn_parser::Value::Int(100));assert_eq!(game.state.vars["calls"],rvn_parser::Value::Int(1));
    assert!(!game.state.vars.contains_key("private"));assert!(!game.state.vars.contains_key("forbidden"));
    assert_eq!(game.state.scene_execution.as_ref().unwrap().active.as_ref().unwrap().locals["score"],rvn_parser::Value::Int(99));
    // Unit bodies/arguments here contain no random call. The real dialogue
    // checkpoint consumes exactly one draw through the existing RNG library.
    let mut expected_random=seed;expected_random.next_u64();assert_eq!(game.state.random,expected_random);
    let refused_before=serde_json::to_vec(&game.state).unwrap();let refused_random=game.state.random;
    assert!(game.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).is_err());
    assert_eq!(game.state.random,refused_random);assert_eq!(serde_json::to_vec(&game.state).unwrap(),refused_before);
    assert!(game.snapshot_for_save().is_err());
    assert_eq!(game.state.random,refused_random);assert_eq!(serde_json::to_vec(&game.state).unwrap(),refused_before);
    let cold=FrozenNativeSource::from_frozen_program(rvn_parser::parse_file_program_with_uses(&main).unwrap()).unwrap();
    assert_eq!(cold.binding(),&binding);
    let mut cold=cold.into_engine(TerminalRenderer,8).unwrap();cold.state.random=seed;cold.enter_label("start").unwrap();cold.step_until_interaction().unwrap();
    assert_eq!(cold.state.vars,game.state.vars);assert_eq!(cold.state.random,game.state.random);
}
#[test]
fn unit_source1003_actual_byte_binding_rejects_stale_library_and_forged_body_spans(){
    let root=files("stale");let main=root.join("main.rvn");let library=root.join("functions.rvn");
    let program=rvn_parser::parse_file_program_with_uses(&main).unwrap();
    let frozen=FrozenNativeSource::from_frozen_program(program.clone()).unwrap();let old=frozen.binding().clone();
    let unit=program.script.iter().position(|s|matches!(s,rvn_parser::Statement::UnitFunction{name,..}if name=="outer")).unwrap();
    let mut forged=program.clone();forged.origins[unit].span.children.clear();
    assert!(FrozenNativeSource::from_frozen_program(forged).is_err());
    let mut forged=program.clone();let rvn_parser::Statement::UnitFunction{body,..}=&mut forged.script[unit]else{panic!()};body.clear();
    assert!(FrozenNativeSource::from_frozen_program(forged).is_err());
    let previous=fs::read_to_string(&library).unwrap();let changed=previous.replace("score=n+1","score=n+2");assert_ne!(changed,previous);fs::write(&library,&changed).unwrap();
    let current=rvn_parser::parse_file_program_with_uses(&main).unwrap();
    assert!(Engine::from_program_with_consumer_binding(current.clone(),old,TerminalRenderer,8).is_err());
    let frozen=FrozenNativeSource::from_frozen_program(current).unwrap();let mut game=frozen.into_engine(TerminalRenderer,8).unwrap();game.enter_label("start").unwrap();game.step_until_interaction().unwrap();
    assert_eq!(game.state.vars["score"],rvn_parser::Value::Int(101));assert_eq!(fs::read_to_string(&library).unwrap(),changed);
}
