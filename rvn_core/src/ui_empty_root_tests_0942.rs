use super::*;
const SOURCE:&str=r#"
screen optional() {
    if visible {return component("root","column",{},[component("choice","button",{},[]),component("drawing","canvas",{"state":{"x":1}},[])])}
    return []
}
"#;
fn fixture()->(UiLibrary,FunctionLibrary,HashMap<String,Value>,UiState,RandomState){
    let script=rvn_parser::parse(SOURCE).unwrap();
    (UiLibrary::from_script(&script).unwrap(),FunctionLibrary::from_script(&script).unwrap(),HashMap::from([("visible".into(),Value::Bool(false))]),UiState::default(),RandomState::seeded(17))
}
fn open(library:&UiLibrary,functions:&FunctionLibrary,globals:&HashMap<String,Value>,state:&mut UiState,random:&mut RandomState)->Option<(UiInput,Component)>{
    library.command(state,UiCommand::Open{name:"optional".into(),arguments:vec![],modal:true,layer:5,host_role:None,inherit_host:false},functions,globals,random).unwrap()
}
#[test]
fn absent_root_is_a_resident_instance_and_vm_can_create_remove_and_recreate_children(){
    let(library,functions,mut globals,mut state,mut random)=fixture();
    assert!(open(&library,&functions,&globals,&mut state,&mut random).is_none());
    let owner=state.screens[0].clone();let counter=state.next_order;
    assert!(owner.modal);assert!(library.views(&state,&functions,&globals).unwrap().is_empty());
    globals.insert("visible".into(),Value::Bool(true));
    let views=library.views(&state,&functions,&globals).unwrap();assert_eq!(views.len(),1);assert_eq!(views[0].root.id,"root");
    library.reconcile_canvas_states(&mut state,&functions,&globals).unwrap();assert!(state.screens[0].canvas_states.contains_key("drawing"));
    state.screens[0].focus=Some("choice".into());
    state.screens[0].canvas_states.get_mut("drawing").unwrap().elapsed=11.0;
    let snapshot=serde_json::to_vec(&state).unwrap();
    globals.insert("visible".into(),Value::Bool(false));
    assert!(library.views(&state,&functions,&globals).unwrap().is_empty());
    assert_eq!(serde_json::to_vec(&state).unwrap(),snapshot,"reading a missing child tree is pure");
    library.reconcile_canvas_states(&mut state,&functions,&globals).unwrap();
    assert!(state.screens[0].canvas_states.is_empty());assert_eq!(state.screens[0].focus,None,"only detached Component focus is cleared");
    let saved=serde_json::to_vec(&state).unwrap();state=serde_json::from_slice(&saved).unwrap();
    globals.insert("visible".into(),Value::Bool(true));
    library.reconcile_canvas_states(&mut state,&functions,&globals).unwrap();
    let again=library.views(&state,&functions,&globals).unwrap();assert_eq!(again[0].focus.as_deref(),Some("choice"));
    assert_eq!(state.screens[0].canvas_states["drawing"].elapsed,0.0);
    assert_eq!(state.screens[0].order,owner.order);assert_eq!(state.next_order,counter);assert_eq!(state.screens[0].random,owner.random);assert!(state.screens[0].modal);
}
#[test]
fn child_focus_and_close_commands_do_not_require_a_fabricated_empty_component(){
    let(library,functions,globals,mut state,mut random)=fixture();open(&library,&functions,&globals,&mut state,&mut random);
    let before=state.clone();assert!(library.command(&mut state,UiCommand::Focus{name:"optional".into(),element:"ghost".into()},&functions,&globals,&mut random).is_err());assert_eq!(state,before);
    assert!(library.command(&mut state,UiCommand::Close{name:"optional".into()},&functions,&globals,&mut random).unwrap().is_none());assert!(state.screens.is_empty());
}
#[test]
fn absent_root_does_not_bypass_instance_validation_or_invalid_nonempty_descriptions(){
    let(library,functions,globals,mut state,mut random)=fixture();open(&library,&functions,&globals,&mut state,&mut random);
    state.screens[0].layer=1001;assert!(library.views(&state,&functions,&globals).is_err());
    let script=rvn_parser::parse("screen bad(){return [1]}").unwrap();let l=UiLibrary::from_script(&script).unwrap();let f=FunctionLibrary::from_script(&script).unwrap();
    assert!(l.command(&mut UiState::default(),UiCommand::Open{name:"bad".into(),arguments:vec![],modal:false,layer:0,host_role:None,inherit_host:false},&f,&HashMap::new(),&mut random).is_err());
}

#[test]
fn owner_description_uses_the_existing_vm_and_retains_owner_without_child(){
    let source=r#"handler focus(event){set count=count+1}
screen owner(){return {"host":{"focusable":enabled,"minimum_desired_size":[12,18],"events":{"focus_received":"focus"}},"root":[]}}"#;
    let script=rvn_parser::parse(source).unwrap();let library=UiLibrary::from_script(&script).unwrap();let functions=FunctionLibrary::from_script(&script).unwrap();let globals=HashMap::from([("enabled".into(),Value::Bool(true)),("count".into(),Value::Int(0))]);let mut state=UiState::default();
    library.command(&mut state,UiCommand::Open{name:"owner".into(),arguments:vec![],modal:true,layer:3,host_role:None,inherit_host:false},&functions,&globals,&mut RandomState::seeded(7)).unwrap();
    let before=state.clone();let hosts=library.host_views(&state,&functions,&globals).unwrap();assert_eq!(hosts.len(),1);assert!(hosts[0].child.is_none());assert!(hosts[0].properties.focusable);assert_eq!(hosts[0].properties.minimum_desired_size,[12.0,18.0]);assert_eq!(state,before);
    library.prepare_owner_properties(&mut state,&functions,&globals,None).unwrap();let published=published_owner_hosts(&state,&[]).unwrap();assert_eq!(published,hosts);assert_eq!(top_modal_owner(&state),Ok(Some((3,0))));
}
#[test]
fn absent_child_cannot_skip_owner_handler_admission(){
    let script=rvn_parser::parse(r#"screen owner(){return {"host":{"events":{"focus_received":"missing"}},"root":[]}}"#).unwrap();let library=UiLibrary::from_script(&script).unwrap();let functions=FunctionLibrary::from_script(&script).unwrap();let mut state=UiState::default();
    assert!(library.command(&mut state,UiCommand::Open{name:"owner".into(),arguments:vec![],modal:false,layer:0,host_role:None,inherit_host:false},&functions,&HashMap::new(),&mut RandomState::seeded(7)).is_err());
}

#[test]
fn owner_focus_intent_without_child_is_sequenced_pure_and_saveable_but_never_acquired_here(){
    let(library,functions,globals,mut state,mut random)=fixture();open(&library,&functions,&globals,&mut state,&mut random);
    for serial in 1..=2{assert!(library.command(&mut state,UiCommand::FocusTarget{name:"optional".into(),target:rvn_ui::programmable::HostFocusTarget::Owner,user:3},&functions,&globals,&mut random).unwrap().is_none());assert_eq!(state.screens[0].owner_focus_request.as_ref().unwrap().serial,serial);assert_eq!(state.screens[0].focus,None);}
    let saved=serde_json::to_vec(&state).unwrap();let restored:UiState=serde_json::from_slice(&saved).unwrap();assert_eq!(restored,state);
    let before=state.clone();for target in [rvn_ui::programmable::HostFocusTarget::Element("fake".into()),rvn_ui::programmable::HostFocusTarget::Element("".into())]{assert!(library.command(&mut state,UiCommand::FocusTarget{name:"optional".into(),target,user:0},&functions,&globals,&mut random).is_err());assert_eq!(state,before);}
    state.screens[0].owner_focus_request.as_mut().unwrap().serial=u64::MAX;let before=state.clone();assert!(library.command(&mut state,UiCommand::FocusTarget{name:"optional".into(),target:rvn_ui::programmable::HostFocusTarget::Owner,user:0},&functions,&globals,&mut random).is_err());assert_eq!(state,before);
}
#[test]
fn owner_slot_z_order_and_modal_order_use_same_root_independent_publication(){
    let source=r#"screen base_layer0942(){return component("button","button",{},[])}
screen empty_layer0942(){return {"host":{"viewport_slot":{"z_order":9}},"root":[]}}"#;
    let script=rvn_parser::parse(source).unwrap();let library=UiLibrary::from_script(&script).unwrap();let functions=FunctionLibrary::from_script(&script).unwrap();let globals=HashMap::new();let mut state=UiState::default();let mut random=RandomState::seeded(9);
    for(name,modal,layer)in [("base_layer0942",false,3),("empty_layer0942",true,0)]{library.command(&mut state,UiCommand::Open{name:name.into(),arguments:vec![],modal,layer,host_role:None,inherit_host:false},&functions,&globals,&mut random).unwrap();}
    library.prepare_owner_properties(&mut state,&functions,&globals,None).unwrap();let children=library.views(&state,&functions,&globals).unwrap();let owners=published_owner_hosts(&state,&children).unwrap();assert_eq!(owners.iter().map(|owner|(owner.name.as_str(),owner.layer)).collect::<Vec<_>>(),vec![("base_layer0942",3),("empty_layer0942",9)]);assert_eq!(top_modal_owner(&state),Ok(Some((9,1))));assert!(owners[1].covers(&owners[0]));
}

#[test]
fn owner_handler_return_data_uses_the_same_existing_vm_and_legacy_api_still_discards_only_the_result(){
    let script=rvn_parser::parse(r#"handler reply(event){set hits=hits+1 return {"handled":true}}"#).unwrap();let library=UiLibrary::from_script(&script).unwrap();let functions=FunctionLibrary::from_script(&script).unwrap();let globals=HashMap::from([("hits".into(),Value::Int(0))]);let event=Value::Dict(Default::default());
    let mut random=RandomState::seeded(33);let(mut vars,commands,reply)=library.with_locale(None).owner_event_result_budgeted_frame("reply",event.clone(),&functions,&globals,&mut random,&mut CanvasBudget::default(),None).unwrap();assert_eq!(vars.remove("hits"),Some(Value::Int(1)));assert!(commands.is_empty());assert_eq!(value_to_json(&reply.unwrap()).unwrap(),serde_json::json!({"handled":true}));
    let(legacy,commands)=library.media_event_budgeted("reply",event,&functions,&globals,&mut RandomState::seeded(33),&mut CanvasBudget::default()).unwrap();assert_eq!(legacy["hits"],Value::Int(1));assert!(commands.is_empty());
}
