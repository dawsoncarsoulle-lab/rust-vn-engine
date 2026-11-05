use super::*;
#[test]
fn empty_child_publication_retires_only_component_input_state(){
    let mut screens=Screens::default();let root=UiComponent::parse(serde_json::json!({"id":"act","kind":"button"})).unwrap();
    let view=ScreenView{name:"panel".into(),modal:false,layer:0,order:1,focus:Some("act".into()),root};
    update_views(&mut screens,vec![view.clone()]);screens.keyboard_focus=Some(("panel".into(),"act".into()));screens.dragging=Some(Dragging{screen:"panel".into(),element:"act".into()});screens.resident_modal=true;
    update_views(&mut screens,vec![]);assert!(screens.views.is_empty());assert!(screens.keyboard_focus.is_none());assert!(screens.dragging.is_none());assert!(screens.modal(),"owner modality is independent of child projection");
    update_views(&mut screens,vec![view]);assert_eq!(screens.keyboard_focus,Some(("panel".into(),"act".into())));
}
#[test]
fn actual_empty_screen_instance_supplies_modality_to_receive_without_a_component_node(){
    let source="screen panel(){return []}\nlabel start\nui.open(\"panel\",[],true,0)\n\"waiting\"\n";
    let mut engine=rvn_core::Engine::new(rvn_parser::parse(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();engine.step_until_interaction().unwrap();
    let views=engine.interface_views().unwrap();assert!(views.is_empty());let instance=engine.state.ui.screens[0].clone();
    let mut app=App::new();app.add_event::<VnCommand>().init_resource::<Screens>().insert_resource(VnEngine(engine)).add_systems(Update,receive);
    app.world_mut().send_event(VnCommand::Interfaces(views));app.update();
    assert!(app.world().resource::<Screens>().modal());assert!(app.world().resource::<Screens>().views.is_empty());assert_eq!(app.world().resource::<VnEngine>().0.state.ui.screens[0],instance);
    let mut controls=app.world_mut().query::<&Control>();assert_eq!(controls.iter(app.world()).count(),0,"receive creates no invented authored control");
    app.world_mut().resource_mut::<VnEngine>().0.state.ui.screens.clear();app.update();assert!(!app.world().resource::<Screens>().modal());
}
