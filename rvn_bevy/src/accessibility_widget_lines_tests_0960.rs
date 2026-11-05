use super::*;

#[test]
fn widget0960_semantic_label_and_original_text_baseline_preserve_authored_source_separators(){
    use bevy::ecs::system::RunSystemOnce;
    let mut app=App::new();app.init_resource::<Accessibility>().init_resource::<crate::programmable_ui::Screens>();
    let authored="\r\nA\r\n";
    let root=rvn_ui::programmable::Component::parse(serde_json::json!({"id":"line","kind":"text","text":authored,"layout_contract":"widget_v1"})).unwrap();
    app.world_mut().resource_mut::<crate::programmable_ui::Screens>().views=vec![rvn_ui::programmable::ScreenView{name:"Lines0960".into(),modal:false,layer:0,order:0,focus:None,root}];
    let target=app.world_mut().spawn((crate::composed_motion::InterfaceTarget{screen:"Lines0960".into(),element:"line".into()},NodeBundle::default())).id();
    let base=20.0;let color=Color::srgb(0.2,0.4,0.8);let settings=AccessibilitySettings{text_scale:1.5,..Default::default()};
    let child=app.world_mut().spawn(initialize_widget_text(TextBundle::from_section("\nA\n",TextStyle{font_size:base,color,..Default::default()}),&settings)).id();app.world_mut().entity_mut(target).add_child(child);
    app.world_mut().run_system_once(interface_labels);
    let node=app.world().get::<bevy::a11y::AccessibilityNode>(target).unwrap().0.clone().build();assert_eq!(node.name(),Some(authored));
    assert_eq!(app.world().get::<OriginalText>(child).unwrap().0[0],(base,color,base*1.5,color));
    assert_eq!(app.world().resource::<crate::programmable_ui::Screens>().views[0].root.text,authored);
}
