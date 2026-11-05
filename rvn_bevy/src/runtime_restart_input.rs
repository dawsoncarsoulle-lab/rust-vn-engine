//! Negative-only transient input quarantine. No permission, owner or portable receipt.
use bevy::{prelude::*,ecs::event::Event};
use std::{any::TypeId,collections::HashMap,ops::Range};
#[derive(Resource,Default)]
struct RejectedEvents{ranges:HashMap<TypeId,Range<usize>>}
fn frame_suppressed(world:&World)->bool{
    world.get_resource::<super::QuarantinedPointerFrame>().is_some_and(|closed|
        world.get_resource::<bevy::core::FrameCount>().is_some_and(|frame|frame.0==closed.0))
}
pub(crate) fn input_available(world:&World)->bool{super::presentation_available(world)&&!frame_suppressed(world)}
pub(super) fn reject_current_events(world:&mut World){
    fn capture<E:Event>(world:&World,rows:&mut RejectedEvents){
        if let Some(events)=world.get_resource::<Events<E>>(){
            let first=events.oldest_event_count();
            // Bevy retains two contiguous EventSequences. This is the actual
            // retained ID interval, including earlier edges not yet consumed.
            rows.ranges.insert(TypeId::of::<E>(),first..first.checked_add(events.len()).unwrap_or(usize::MAX));
        }
    }
    let mut rows=world.remove_resource::<RejectedEvents>().unwrap_or_default();
    capture::<bevy::input::keyboard::KeyboardInput>(world,&mut rows);
    capture::<bevy::window::Ime>(world,&mut rows);
    capture::<bevy::input::mouse::MouseButtonInput>(world,&mut rows);
    capture::<bevy::input::mouse::MouseWheel>(world,&mut rows);
    capture::<bevy::window::CursorMoved>(world,&mut rows);
    capture::<bevy::input::touch::TouchInput>(world,&mut rows);
    capture::<bevy::a11y::ActionRequest>(world,&mut rows);
    capture::<crate::vn_command::PlayerInput>(world,&mut rows);
    world.insert_resource(rows);
}
/// Runs even in the frame restored by First, before real source captures.
pub(crate) fn capture_suppressed_events(world:&mut World){if !input_available(world){reject_current_events(world);crate::menu_documents::cancel_runtime_input(world);}}
fn rejected<E:Event>(rows:Option<&RejectedEvents>,id:usize)->bool{
    rows.and_then(|rows|rows.ranges.get(&TypeId::of::<E>())).is_some_and(|range|range.contains(&id))
}
pub(crate) fn accepts_event<E:Event>(world:&World,id:usize)->bool{
    input_available(world)&&!rejected::<E>(world.get_resource::<RejectedEvents>(),id)
}
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct InputAccess<'w>{
    anchor:Option<Res<'w,super::Anchor>>,
    closed:Option<Res<'w,super::QuarantinedPointerFrame>>,
    frame:Option<Res<'w,bevy::core::FrameCount>>,
    rejected:Option<Res<'w,RejectedEvents>>,
}
impl InputAccess<'_>{
    pub(crate) fn available(&self)->bool{self.anchor.is_none()&&!self.closed.as_ref().is_some_and(|closed|self.frame.as_ref().is_some_and(|frame|frame.0==closed.0))}
    pub(crate) fn accepts<E:Event>(&self,id:usize)->bool{self.available()&&!rejected::<E>(self.rejected.as_deref(),id)}
}

#[cfg(test)]
mod tests{
 use super::*;
 use bevy::input::{ButtonState,keyboard::{KeyboardInput,Key},mouse::{MouseWheel,MouseScrollUnit}};
 #[test]
 fn restart_input_negative_ranges_preserve_real_shared_queues_and_fresh_event_ids(){
    let mut world=World::new();let window=world.spawn(Window::default()).id();
    world.init_resource::<Events<KeyboardInput>>();world.init_resource::<Events<bevy::window::Ime>>();
    world.init_resource::<Events<MouseWheel>>();world.init_resource::<Events<bevy::a11y::ActionRequest>>();
    let key=world.resource_mut::<Events<KeyboardInput>>().send(KeyboardInput{key_code:KeyCode::KeyA,logical_key:Key::Character("a".into()),state:ButtonState::Pressed,window});
    let ime=world.resource_mut::<Events<bevy::window::Ime>>().send(bevy::window::Ime::Commit{window,value:"before".into()});
    let wheel=world.resource_mut::<Events<MouseWheel>>().send(MouseWheel{unit:MouseScrollUnit::Line,x:0.0,y:3.0,window});
    let action=world.resource_mut::<Events<bevy::a11y::ActionRequest>>().send(bevy::a11y::ActionRequest(bevy::a11y::accesskit::ActionRequest{action:bevy::a11y::accesskit::Action::Focus,target:bevy::a11y::accesskit::NodeId(window.to_bits()),data:None}));
    reject_current_events(&mut world);
    assert!(!accepts_event::<KeyboardInput>(&world,key.id));assert!(!accepts_event::<bevy::window::Ime>(&world,ime.id));
    assert!(!accepts_event::<MouseWheel>(&world,wheel.id));assert!(!accepts_event::<bevy::a11y::ActionRequest>(&world,action.id));
    assert_eq!(world.resource::<Events<KeyboardInput>>().len(),1);assert_eq!(world.resource::<Events<bevy::window::Ime>>().len(),1);
    assert_eq!(world.resource::<Events<MouseWheel>>().len(),1);assert_eq!(world.resource::<Events<bevy::a11y::ActionRequest>>().len(),1,"Host consumers retain the actual event");
    let fresh=world.resource_mut::<Events<KeyboardInput>>().send(KeyboardInput{key_code:KeyCode::KeyB,logical_key:Key::Character("b".into()),state:ButtonState::Pressed,window});
    assert!(accepts_event::<KeyboardInput>(&world,fresh.id),"A negative range cannot issue or deny unrelated future identity");
    // Actual Bevy ring advancement, no replacement queue or fabricated IDs.
    world.resource_mut::<Events<KeyboardInput>>().update();world.resource_mut::<Events<KeyboardInput>>().update();
    let after=world.resource_mut::<Events<KeyboardInput>>().send(KeyboardInput{key_code:KeyCode::KeyC,logical_key:Key::Character("c".into()),state:ButtonState::Pressed,window});
    reject_current_events(&mut world);assert!(!accepts_event::<KeyboardInput>(&world,after.id));
    for _ in 0..100{reject_current_events(&mut world);}
    assert_eq!(world.resource::<RejectedEvents>().ranges.len(),4,"Storage stays bounded by actual registered input types, never by number of frames");
 }
}
