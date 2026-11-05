use super::*;
#[test]
fn explicit_empty_owner_keeps_properties_and_only_authored_empty_list_denotes_no_child(){
    let value=serde_json::json!({"host":{"focusable":true,"minimum_desired_size":[80,40],"events":{"focus_received":"focus"}},"root":[]});
    let description=parse_screen_description(value.clone()).unwrap();assert!(description.root.is_none());assert!(description.host.supports_owner_focus());assert_eq!(description.host.minimum_desired_size,[80.0,40.0]);
    assert!(super::super::parse_screen_root(value).unwrap().is_none());
    for invalid in [serde_json::json!({"host":{},"root":null}),serde_json::json!({"host":{},"root":[],"extra":true}),serde_json::json!({"host":{"wrong":true},"root":[]}),serde_json::json!({"host":{},"root":[1]})]{assert!(parse_screen_description(invalid).is_err());}
    let old=parse_screen_description(serde_json::json!({"id":"actual","kind":"text"})).unwrap();assert_eq!(old.host,ScreenHostProperties::default());assert!(old.root.is_some());
}
#[test]
fn root_absence_does_not_erase_default_fill_host_or_clamp_an_allocated_slot_to_desired_size(){
    let mut host=ScreenHostProperties::default();host.minimum_desired_size=[500.0,400.0];let desired=host.desired_size([0.0,0.0]).unwrap();
    assert_eq!(host.viewport_slot.geometry([10.0,20.0,100.0,80.0],desired,1.25).unwrap().rect,[10.0,20.0,100.0,80.0]);
    host.viewport_slot.anchors=[0.5,0.5,0.5,0.5];host.viewport_slot.offsets=SlotEdges{left:5.0,top:7.0,right:30.0,bottom:20.0};host.viewport_slot.alignment=[0.5,0.5];
    assert_eq!(host.viewport_slot.geometry([10.0,20.0,100.0,80.0],desired,1.25).unwrap().rect,[50.0,57.0,30.0,20.0]);
}
#[test]
fn autosize_requires_zero_both_sizes_and_no_stretched_axis(){
    let mut slot=ViewportSlot::default();slot.anchors=[0.0;4];
    assert_eq!(slot.geometry([0.0,0.0,100.0,80.0],[12.0,18.0],1.0).unwrap().rect,[0.0,0.0,12.0,18.0]);
    slot.offsets.right=30.0;assert_eq!(slot.geometry([0.0,0.0,100.0,80.0],[12.0,18.0],1.0).unwrap().rect,[0.0,0.0,30.0,0.0]);
    slot.offsets.right=0.0;slot.anchors[2]=1.0;assert_eq!(slot.geometry([0.0,0.0,100.0,80.0],[12.0,18.0],1.0).unwrap().rect,[0.0,0.0,100.0,0.0]);
}
#[test]
fn invalid_properties_geometry_and_child_focus_addresses_refuse_without_mutating_inputs(){
    let mut slot=ViewportSlot::default();slot.offsets.left=150.0;let before=slot.clone();assert_eq!(slot.geometry([0.0,0.0,100.0,80.0],[0.0,0.0],1.0).unwrap().rect,[150.0,0.0,-50.0,80.0]);assert_eq!(slot,before);assert!(slot.geometry([0.0,0.0,100.0,80.0],[0.0,0.0],f32::NAN).is_err());
    let mut host=ScreenHostProperties::default();host.minimum_desired_size=[f32::NAN,0.0];assert!(host.validate().is_err());
    assert!(HostFocusTarget::Element(String::new()).validate().is_err());assert!(HostFocusTarget::Owner.validate().is_ok());
    assert!(!HostVisibility::SelfHitTestInvisible.owner_hit_test());assert!(HostVisibility::SelfHitTestInvisible.child_hit_test());assert!(!HostVisibility::HitTestInvisible.child_hit_test());
    host=ScreenHostProperties::default();host.focusable=true;host.visibility=HostVisibility::Hidden;assert!(host.supports_owner_focus());assert!(!host.focus_properties_allow_acquisition());
}
#[test]
fn wrapper_lifecycle_is_independent_of_child_publications_and_viewport_attachment(){
    let mut state=HostLifecycle::default();assert_eq!(state.initialize(true),vec![HostLifecycleEvent::Initialized]);assert!(state.initialize(true).is_empty());
    assert_eq!(state.rebuild_wrapper(false,false,true).unwrap(),vec![HostLifecycleEvent::PreConstruct{design_time:false},HostLifecycleEvent::Construct]);let generation=state.generation();
    state.attach_viewport().unwrap();state.detach_viewport();assert_eq!(state.generation(),generation);state.attach_viewport().unwrap();
    // A child None->Rooted->None publication is deliberately no wrapper call.
    let unchanged=state.clone();assert_eq!(state,unchanged);
    assert_eq!(state.rebuild_wrapper(false,false,true).unwrap(),vec![HostLifecycleEvent::Destruct,HostLifecycleEvent::PreConstruct{design_time:false},HostLifecycleEvent::Construct]);assert_eq!(state.generation(),generation+1);
    assert_eq!(state.release_wrapper(true),vec![HostLifecycleEvent::Destruct]);assert!(state.release_wrapper(true).is_empty());assert!(!state.attached());
}
#[test]
fn design_lifecycle_and_unsafe_route_do_not_fabricate_runtime_construct_destruct(){
    let mut state=HostLifecycle::default();assert!(state.initialize(false).is_empty());assert!(state.initialize(true).is_empty());
    assert!(state.rebuild_wrapper(true,false,true).unwrap().is_empty());assert!(state.release_wrapper(true).is_empty());
    assert_eq!(state.rebuild_wrapper(true,true,true).unwrap(),vec![HostLifecycleEvent::PreConstruct{design_time:true}]);assert!(state.release_wrapper(true).is_empty());
    state.rebuild_wrapper(false,false,true).unwrap();assert!(state.release_wrapper(false).is_empty());
    let mut overflow:HostLifecycle=serde_json::from_value(serde_json::json!({"wrapper_generation":u64::MAX})).unwrap();let before=overflow.clone();assert!(overflow.rebuild_wrapper(false,false,true).is_err());assert_eq!(overflow,before);
}

#[test]
fn signed_owner_contains_preserves_orientation_and_zero_area_distance_is_separate(){
    let geometry=HostGeometry{rect:[100.0,20.0,-40.0,30.0],viewport:[0.0,0.0,200.0,100.0],scale:1.0};assert_eq!(geometry.bounding_rect(),[60.0,20.0,40.0,30.0]);assert!(geometry.contains_point([80.0,30.0]));assert!(geometry.contains_point([60.0,50.0]));assert!(!geometry.contains_point([59.0,30.0]));assert_eq!(geometry.distance_squared([59.0,30.0]),Some(1.0));
    let line=HostGeometry{rect:[60.0,20.0,0.0,30.0],..geometry};assert!(!line.contains_point([60.0,30.0]));assert_eq!(line.distance_squared([60.0,30.0]),Some(0.0));
}

#[test]
fn legacy_default_omission_never_discards_an_authored_negative_zero(){
    let mut props=ScreenHostProperties::default();assert!(props.is_default());props.minimum_desired_size[0]=-0.0;assert_eq!(props,ScreenHostProperties::default());assert!(!props.is_default());
    let encoded=serde_json::to_string(&props).unwrap();let decoded:ScreenHostProperties=serde_json::from_str(&encoded).unwrap();assert_eq!(decoded.minimum_desired_size[0].to_bits(),(-0.0f32).to_bits());
}

#[test]
fn explicit_owner_focus_operand_is_typed_and_cannot_fabricate_element_identity(){
    let(target,user)=parse_host_focus_target(serde_json::json!({"target":"owner","user":2})).unwrap();assert_eq!(target,HostFocusTarget::Owner);assert_eq!(user,2);
    assert_eq!(parse_host_focus_target(serde_json::json!({"target":"element","node":"actual","user":0})).unwrap().0,HostFocusTarget::Element("actual".into()));
    for value in [serde_json::json!({"target":"owner","node":"fake"}),serde_json::json!({"target":"element"}),serde_json::json!({"target":"element","node":""}),serde_json::json!({"target":"owner","user":4096}),serde_json::json!({"target":"owner","extra":0})]{assert!(parse_host_focus_target(value).is_err());}
    assert!(HostFocusRequest{target:HostFocusTarget::Owner,user:0,serial:0}.validate().is_err());
}

#[test]
fn owner_visibility_hit_policy_and_keyboard_availability_are_distinct(){
    use super::super::ScreenEventKind as E;
    let mut host=ScreenHostProperties::default();host.visibility=HostVisibility::HitTestInvisible;assert!(!host.allows_child_event(E::PointerDown));assert!(!host.allows_child_event(E::Click));assert!(host.allows_child_event(E::Activate));assert!(host.allows_child_event(E::Key));
    host.visibility=HostVisibility::Hidden;assert!(!host.allows_child_event(E::Activate));assert!(!host.allows_child_event(E::Key));assert!(host.allows_child_event(E::PointerCancel));
    host.visibility=HostVisibility::SelfHitTestInvisible;assert!(host.allows_child_event(E::PointerDown));host.enabled=false;assert!(!host.allows_child_event(E::Key));assert!(host.allows_child_event(E::PointerCancel));
}

#[test]
fn negative_authored_minimum_is_retained_but_desired_size_is_max_with_actual_child(){
    let description=parse_screen_description(serde_json::json!({"host":{"minimum_desired_size":[-20,-30]},"root":[]})).unwrap();assert_eq!(description.host.minimum_desired_size,[-20.0,-30.0]);assert_eq!(description.host.desired_size([0.0,0.0]).unwrap(),[0.0,0.0]);assert_eq!(description.host.desired_size([12.0,8.0]).unwrap(),[12.0,8.0]);
}
