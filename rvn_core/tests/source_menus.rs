use rvn_core::{ui::UiInput, Engine, GameState, Renderer, SpriteState};
use rvn_parser::{parse, Hotspot, Position, Transition, Value};
use rvn_ui::{programmable::{ScreenEventKind, ScreenView}, source_menus::{MenuAuthority, MenuEffect}, PageRole};

#[derive(Default)]
struct Host { views:Vec<ScreenView>, authority:MenuAuthority, requests:Vec<(MenuEffect,GameState)> }
impl Renderer for Host {
    fn supports_programmable_ui(&self)->bool {true}
    fn update_interfaces(&mut self,views:&[ScreenView])->Result<(),String>{self.views=views.to_vec();Ok(())}
    fn validate_menu_request(&self,effect:&MenuEffect,candidate:&GameState)->Result<(),String>{let mut authority=self.authority.clone(); let instance=candidate.ui.screens.iter().find(|screen|screen.name==effect.screen && screen.order==effect.screen_order && screen.host_role==effect.host_role).ok_or_else(||"Inactive origin".to_string())?; if instance.host_role.is_some_and(|role|authority.screen_roles.values().any(|active|*active==role)) || (instance.host_role.is_none() && authority.story_ui_active){authority.live_screens.insert(instance.name.clone());if let Some(role)=instance.host_role{authority.screen_roles.insert(instance.name.clone(),role);}} authority.validate(effect)}
    fn menu_request(&mut self,effect:&MenuEffect,previous:&GameState,_candidate:&GameState)->Result<(),String>{self.requests.push((effect.clone(),previous.clone()));Ok(())}
    fn menu_request_pending(&self)->bool{!self.requests.is_empty()}
    fn set_background(&mut self,_:&str,_:&Transition){}
    fn show_sprite(&mut self,_:&str,_:Option<&str>,_:&Position,_:&Transition,_:Option<&SpriteState>){}
    fn hide_sprite(&mut self,_:&str,_:&Transition,_:&SpriteState){}
    fn move_sprite(&mut self,_:&str,_:&Position,_:&Transition,_:&SpriteState){}
    fn show_dialogue(&mut self,_:Option<&str>,_:&str){}
    fn show_choice(&mut self,_:&[String])->usize{0}
    fn music_play(&mut self,_:&str,_:&Transition,_:Option<&str>){}
    fn music_stop(&mut self,_:&Transition){}
    fn music_set_volume(&mut self,_:f32){}
    fn sfx_play(&mut self,_:&str,_:&Transition){}
    fn sfx_stop(&mut self,_:&str,_:&Transition){}
    fn show_imagemap(&mut self,_:&str,_:Option<&str>,_:&[Hotspot])->usize{0}
    fn restore_screen(&mut self,_:&GameState){}
}
fn game(source:&str)->Engine<Host>{let mut game=Engine::new(parse(source).unwrap(),Host::default(),16).unwrap();game.step_until_interaction().unwrap();game}
fn click(screen:&str,element:&str)->UiInput{UiInput{screen:screen.into(),element:element.into(),kind:ScreenEventKind::Click,value:None,key:None}}
fn context(role:PageRole)->Value{rvn_core::ui::value_from_json(&rvn_ui::source_menus::preview_context(role,true)).unwrap()}
fn authorize(game:&mut Engine<Host>,screen:&str,role:PageRole){game.renderer.authority.live_screens.insert(screen.into());game.renderer.authority.screen_roles.insert(screen.into(),role);game.renderer.authority.waiting=true;game.renderer.authority.game_active=true;}

#[test]
fn all_eleven_roles_evaluate_source_and_are_excluded_from_story_saves() {
    let mut source=String::from("screen inventory(){return component(\"inventory\",\"text\",{\"text\":\"Kept\"},[])}\n");
    for role in PageRole::ALL {source.push_str(&format!("screen host_{}(ctx){{return component(\"root\",\"text\",{{\"text\":ctx[\"role\"]}},[])}}\n",role.id()));}
    source.push_str("label start\nui.open(\"inventory\",[],false,0)\n\"Waiting\"\n");
    let mut game=game(&source);let random=game.state.random;
    for role in PageRole::ALL {let name=format!("host_{}",role.id());assert!(game.synchronize_source_menu(role,Some(&name),context(role),false,500).unwrap());}
    assert_eq!(game.interface_views().unwrap().len(),12);assert_eq!(game.state.random,random);
    let saved=rvn_core::save::SaveData::from_state(&game.state,1,"QA".into(),"main.rvn".into());
    assert_eq!(saved.ui.screens.len(),1);assert_eq!(saved.ui.screens[0].name,"inventory");
    for role in PageRole::ALL {game.synchronize_source_menu(role,None,Value::Dict(Default::default()),false,0).unwrap();}
    assert_eq!(game.interface_views().unwrap().len(),1);assert_eq!(game.state.random,random);
}
#[test]
fn invalid_typed_request_rolls_back_globals_controls_random_and_keeps_next_action_usable() {
    let mut game=game(r#"
init{set attempts=0 set name=""}
handler bad(event){set attempts=attempts+1 local roll=random(1,10) ui.focus("host","name") menu.execute({"kind":"bool_preference","key":"fullscreen","value":1})}
handler good(event){set attempts=attempts+1 menu.execute(menu_action("none"))}
screen host(ctx){return component("root","column",{},[component("name","input",{"binding":"name"},[]),component("bad","button",{"events":{"click":"bad"}},[]),component("good","button",{"events":{"click":"good"}},[])])}
label start
"Waiting"
"#);
    game.synchronize_source_menu(PageRole::Pause,Some("host"),context(PageRole::Pause),true,500).unwrap();authorize(&mut game,"host",PageRole::Pause);
    let before=serde_json::to_value(&game.state).unwrap();let history=game.history.len();
    assert!(game.interface_event(click("host","bad")).unwrap_err().is_menu_request_rejection());
    assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert!(game.renderer.requests.is_empty());assert_eq!(game.history.len(),history);
    game.interface_event(click("host","good")).unwrap();assert_eq!(game.state.vars["attempts"],Value::Int(1));assert_eq!(game.renderer.requests.len(),1);
    let (_,previous)=game.renderer.requests.pop().unwrap();game.finish_source_menu_transaction(previous);assert_eq!(game.history.len(),history+1);
    assert!(game.rollback());assert_eq!(game.state.vars["attempts"],Value::Int(0));
}
#[test]
fn failed_external_operation_compensates_the_source_transaction_without_a_rollback_entry() {
    let mut game=game("init{set attempts=0}\nhandler act(event){set attempts=attempts+1 menu.execute(menu_action(\"none\"))}\nscreen host(){return component(\"button\",\"button\",{\"events\":{\"click\":\"act\"}},[])}\nlabel start\n\"Waiting\"");
    game.synchronize_source_menu(PageRole::Pause,Some("host"),context(PageRole::Pause),true,500).unwrap();authorize(&mut game,"host",PageRole::Pause);
    let before=serde_json::to_value(&game.state).unwrap();let history=game.history.len();
    game.interface_event(click("host","button")).unwrap();let (_,previous)=game.renderer.requests.pop().unwrap();
    game.cancel_source_menu_transaction(previous).unwrap();assert_eq!(serde_json::to_value(&game.state).unwrap(),before);assert_eq!(game.history.len(),history);
}
#[test]
fn updating_context_keeps_controls_and_source_children_until_the_role_closes() {
    let mut game=game("handler open_child(event){ui.open(\"child\",[],false,501)}\nscreen child(){return component(\"child\",\"text\",{\"text\":\"Submenu\"},[])}\nscreen host(ctx){return component(\"open\",\"button\",{\"text\":ctx[\"role\"],\"events\":{\"click\":\"open_child\"}},[])}\nlabel start\n\"Waiting\"");
    game.synchronize_source_menu(PageRole::Pause,Some("host"),context(PageRole::Pause),false,500).unwrap();
    let history=game.history.len();game.interface_event(click("host","open")).unwrap();
    assert_eq!(game.history.len(),history);assert_eq!(game.state.ui.screens.len(),2);
    game.state.ui.screens.iter_mut().find(|screen|screen.name=="host").unwrap().values.insert("draft".into(),Value::Str("Preserved".into()));
    let changed=rvn_core::ui::value_from_json(&serde_json::json!({"role":"updated"})).unwrap();
    game.synchronize_source_menu(PageRole::Pause,Some("host"),changed,false,500).unwrap();
    assert_eq!(game.state.ui.screens.len(),2);assert_eq!(game.state.ui.screens.iter().find(|screen|screen.name=="host").unwrap().values["draft"],Value::Str("Preserved".into()));
    game.synchronize_source_menu(PageRole::Pause,None,Value::Dict(Default::default()),false,0).unwrap();assert!(game.state.ui.screens.is_empty());
}
#[test]
fn failed_close_can_abandon_only_host_presenter_without_orphan_modal_or_story_loss() {
    let mut game=game("init{set marker=0}\nhandler bad_close(event){set marker=1 menu.execute({\"kind\":\"not_a_request\"})}\nscreen inventory(){return component(\"inventory\",\"text\",{},[])}\nscreen host(){return component(\"root\",\"panel\",{\"events\":{\"close\":\"bad_close\"}},[])}\nlabel start\nui.open(\"inventory\",[],false,0)\n\"Waiting\"");
    game.synchronize_source_menu(PageRole::Pause,Some("host"),context(PageRole::Pause),true,500).unwrap();
    assert!(game.synchronize_source_menu(PageRole::Pause,None,Value::Dict(Default::default()),false,0).is_err());
    assert_eq!(game.state.vars["marker"],Value::Int(0));game.abandon_source_menu(PageRole::Pause).unwrap();
    assert!(!game.interface_is_modal());assert_eq!(game.interface_views().unwrap().len(),1);assert_eq!(game.state.ui.screens[0].name,"inventory");
}

#[test]
fn host_allocations_preserve_legacy_narrative_counter_in_save_and_rollback() {
    let mut game=game("screen story(){return component(\"root\",\"text\",{},[])}\nscreen host(){return component(\"root\",\"text\",{},[])}\nlabel start\nui.open(\"story\",[],false,0)\nui.close(\"story\")\n\"Waiting\"");
    assert!(game.state.ui.screens.is_empty());assert_eq!(game.state.ui.next_order,1);
    let before=rvn_core::save::SaveData::from_state(&game.state,1,"QA".into(),"main.rvn".into()).ui;
    game.synchronize_source_menu(PageRole::Pause,Some("host"),context(PageRole::Pause),true,500).unwrap();
    assert_eq!(game.state.ui.next_order,1);assert_eq!(game.state.ui.host_next_order,1);
    let saved=rvn_core::save::SaveData::from_state(&game.state,1,"QA".into(),"main.rvn".into());
    assert_eq!(saved.ui,before);assert_eq!(saved.ui.host_next_order,0);
    game.synchronize_source_menu(PageRole::Pause,None,Value::Dict(Default::default()),false,0).unwrap();
    assert_eq!(game.state.ui.next_order,1);
}

#[test]
fn child_open_lifecycle_uses_candidate_instance_and_retires_with_its_role() {
    let mut game=game("init{set marker=0}\nhandler create(event){ui.open(\"child\",[],false,501)}\nhandler child_open(event){set marker=1 menu.execute(menu_action(\"none\"))}\nscreen child(){return component(\"root\",\"panel\",{\"events\":{\"open\":\"child_open\"}},[])}\nscreen host(){return component(\"open\",\"button\",{\"events\":{\"click\":\"create\"}},[])}\nlabel start\n\"Waiting\"");
    game.synchronize_source_menu(PageRole::Pause,Some("host"),context(PageRole::Pause),false,500).unwrap();authorize(&mut game,"host",PageRole::Pause);
    game.interface_event(click("host","open")).unwrap();
    let (effect,_)=game.renderer.requests.pop().unwrap();assert_eq!(effect.screen,"child");assert_eq!(effect.host_role,Some(PageRole::Pause));assert_eq!(game.state.vars["marker"],Value::Int(1));
    game.renderer.authority=MenuAuthority::default();
    assert!(game.renderer.validate_menu_request(&effect,&game.state).is_err());
    game.synchronize_source_menu(PageRole::Pause,None,Value::Dict(Default::default()),false,0).unwrap();
    assert!(game.renderer.validate_menu_request(&effect,&game.state).is_err());
}

#[test]
fn modal_host_is_above_authored_max_layer_but_dialogue_remains_below_inventory() {
    let mut game=game("screen inventory(){return component(\"root\",\"button\",{},[])}\nscreen dialogue(){return component(\"root\",\"button\",{},[])}\nscreen pause(){return component(\"root\",\"button\",{},[])}\nscreen confirm(){return component(\"root\",\"button\",{},[])}\nlabel start\nui.open(\"inventory\",[],true,1000)\n\"Waiting\"");
    game.synchronize_source_menu(PageRole::Dialogue,Some("dialogue"),context(PageRole::Dialogue),false,100).unwrap();
    assert!(game.interface_event(click("dialogue","root")).is_err());
    game.synchronize_source_menu(PageRole::Pause,Some("pause"),context(PageRole::Pause),true,800).unwrap();
    assert!(game.interface_event(click("pause","root")).is_ok());assert!(game.interface_event(click("inventory","root")).is_err());
    game.synchronize_source_menu(PageRole::Confirm,Some("confirm"),context(PageRole::Confirm),true,900).unwrap();
    assert_eq!(game.interface_views().unwrap().last().unwrap().name,"confirm");assert!(game.interface_event(click("pause","root")).is_err());
}

#[test]
fn visible_choice_index_selects_actual_destination_and_never_a_false_option() {
    let source="init{set route=\"\"}\nlabel start\nchoice{\"Hidden\" if false=>{jump hidden} \"Visible\" if true=>{jump visible}}\nlabel hidden\nset route=\"hidden\"\n\"Hidden destination\"\nlabel visible\nset route=\"visible\"\n\"Visible destination\"";
    for index in [0,usize::MAX] {
        let mut game=game(source);assert_eq!(game.active_choice_indices().unwrap(),vec![1]);
        assert_eq!(game.current_interaction().unwrap(),Some(rvn_core::Interaction::Choice{options:vec!["Visible".into()]}));
        game.submit_choice(index).unwrap();game.step_until_interaction().unwrap();assert_eq!(game.state.vars["route"],Value::Str("visible".into()));
    }
}

#[test]
fn all_hidden_choices_skip_and_homonymous_destinations_keep_distinct_identity() {
    let mut empty=game("label start\nchoice{\"Unavailable\" if false=>{jump hidden}}\n\"Reached\"\nlabel hidden\n\"Wrong\"");
    assert_eq!(empty.current_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"Reached".into()}));
    empty.advance_dialogue().unwrap();
    let mut game=game("init{set gate=false}\nlabel start\nchoice{\"Same\" if gate=>{jump a} \"Same\" if not gate=>{jump b}}\nlabel a\n\"A\"\nlabel b\n\"B\"");
    let labels=game.current_interaction().unwrap();assert_eq!(game.active_choice_indices().unwrap(),vec![1]);
    game.state.vars.insert("gate".into(),Value::Bool(true));assert_eq!(game.current_interaction().unwrap(),labels);assert_eq!(game.active_choice_indices().unwrap(),vec![0]);
}

#[test]
fn source_rollback_mutation_does_not_insert_a_spurious_intermediate_frame() {
    let source="init{set marker=0}\nhandler undo(event){set marker=marker+1 menu.execute(menu_action(\"rollback\"))}\nscreen host(){return component(\"undo\",\"button\",{\"events\":{\"click\":\"undo\"}},[])}\nlabel start\n\"First\"\n\"Second\"";
    let seeded_game=|| {
        let mut game=Engine::new(parse(source).unwrap(),Host::default(),16).unwrap();
        game.state.random=rvn_core::random::RandomState::seeded(42);
        game.step_until_interaction().unwrap();game
    };
    let mut baseline=seeded_game();baseline.advance_dialogue().unwrap();baseline.step_until_interaction().unwrap();
    let mut game=seeded_game();
    game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();let second=game.state.pc;let history=game.history.len();
    game.synchronize_source_menu(PageRole::QuickActions,Some("host"),context(PageRole::QuickActions),false,100).unwrap();authorize(&mut game,"host",PageRole::QuickActions);
    game.interface_event(click("host","undo")).unwrap();assert_eq!(game.history.len(),history);assert_eq!(game.state.vars["marker"],Value::Int(1));
    assert!(game.history.entries().iter().all(|entry|entry.state.vars["marker"]==Value::Int(0)&&entry.state.ui.screens.iter().all(|screen|screen.host_role.is_none())));
    assert!(baseline.rollback());assert!(game.rollback());assert_eq!(serde_json::to_value(&game.state).unwrap(),serde_json::to_value(&baseline.state).unwrap());assert_eq!(game.history.len(),baseline.history.len());
    assert_eq!(game.state.vars["marker"],Value::Int(0));
    assert!(baseline.rollback());assert!(game.rollback());assert!(game.state.pc<second);assert_eq!(serde_json::to_value(&game.state).unwrap(),serde_json::to_value(&baseline.state).unwrap());
}

#[test]
fn child_of_modal_host_shares_presentation_and_receives_real_hit_tests() {
    let mut game=game("init{set clicked=0}\nhandler open_child(event){ui.open(\"child\",[],false,0)}\nhandler activate(event){set clicked=clicked+1}\nscreen child(){return component(\"act\",\"button\",{\"events\":{\"click\":\"activate\"}},[])}\nscreen host(){return component(\"open\",\"button\",{\"events\":{\"click\":\"open_child\"}},[])}\nlabel start\n\"Waiting\"");
    game.synchronize_source_menu(PageRole::Pause,Some("host"),context(PageRole::Pause),true,0).unwrap();
    game.interface_event(click("host","open")).unwrap();
    let views=game.interface_views().unwrap();let child=views.iter().find(|view|view.name=="child").unwrap();let root=views.iter().find(|view|view.name=="host").unwrap();
    assert_eq!(child.layer,root.layer);assert!(child.order>root.order);
    game.interface_event(click("child","act")).unwrap();assert_eq!(game.state.vars["clicked"],Value::Int(1));
    game.synchronize_source_menu(PageRole::Pause,None,Value::Dict(Default::default()),false,0).unwrap();assert!(game.state.ui.screens.is_empty());
}

#[test]
fn explicit_story_open_from_source_is_narrative_saved_and_rollbackable() {
    let mut game=game("handler open_inventory(event){ui.open_story(\"inventory\",[],true,60)}\nscreen inventory(){return component(\"root\",\"button\",{},[])}\nscreen host(){return component(\"open\",\"button\",{\"events\":{\"click\":\"open_inventory\"}},[])}\nlabel start\n\"Waiting\"");
    game.synchronize_source_menu(PageRole::QuickActions,Some("host"),context(PageRole::QuickActions),false,100).unwrap();
    let history=game.history.len();game.interface_event(click("host","open")).unwrap();assert_eq!(game.history.len(),history+1);
    let inventory=game.state.ui.screens.iter().find(|screen|screen.name=="inventory").unwrap();assert_eq!(inventory.host_role,None);assert_eq!(inventory.order,0);
    assert_eq!(game.state.ui.next_order,1);assert!(game.interface_event(click("host","open")).is_err());
    let saved=rvn_core::save::SaveData::from_state(&game.state,1,"QA".into(),"main.rvn".into());assert_eq!(saved.ui.screens.len(),1);assert_eq!(saved.ui.screens[0].name,"inventory");
    game.synchronize_source_menu(PageRole::QuickActions,None,Value::Dict(Default::default()),false,0).unwrap();assert_eq!(game.state.ui.screens.len(),1);
    assert!(game.rollback());assert!(game.state.ui.screens.is_empty());assert_eq!(game.state.ui.next_order,0);
}
