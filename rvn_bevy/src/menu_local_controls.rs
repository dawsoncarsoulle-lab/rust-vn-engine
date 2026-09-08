//! Controls that own UI-only variables. Never writes settings or story state.
use super::*;
#[derive(Component,Clone)]pub(super) struct LocalInput{pub page:String,pub id:String,pub kind:Kind,pub control:rvn_ui::LocalControl}
#[derive(Resource,Default)]pub(super) struct Drag(Option<(LocalInput,f32,f32)>);
pub(super) fn set(menus:&mut Menus,input:&LocalInput,value:serde_json::Value){
    match menus.session.set_control_value(input.kind,&input.control,value){Ok(true)=>{menus.activations.push((input.page.clone(),Some(input.id.clone()),rvn_ui::Event::ValueChanged,Action::None));menus.key.clear();},Ok(false)=>{},Err(error)=>warn!("Contrôle d’interface : {error}")}
}
pub(super) fn update(confirmation:Res<crate::systems::save_menu::SaveConfirmation>,dropdown:Res<dropdown::Dropdown>,mut menus:ResMut<Menus>,mut drag:ResMut<Drag>,mouse:Res<ButtonInput<MouseButton>>,keys:Res<ButtonInput<KeyCode>>,pads:Res<ButtonInput<GamepadButton>>,windows:Query<&Window>,query:Query<(Entity,&Interaction,&LocalInput,&Node,&GlobalTransform),Changed<Interaction>>,all:Query<&LocalInput>){
    if confirmation.active()||dropdown.open{drag.0=None;return;}
    for(_,interaction,input,node,t) in &query{if *interaction!=Interaction::Pressed{continue;}match input.kind{
        Kind::CheckBox=>{let checked=input.control.value(input.kind,&menus.session).as_bool().unwrap_or(false);set(&mut menus,input,(!checked).into());},
        Kind::Slider if mouse.pressed(MouseButton::Left)=>{drag.0=Some((input.clone(),t.translation().x-node.size().x/2.0+12.0,(node.size().x-24.0).max(1.0)));},_=>{}}
    }
    let direction=if keys.just_pressed(KeyCode::ArrowLeft)||pads.get_just_pressed().any(|p|p.button_type==GamepadButtonType::DPadLeft){-1.0}else if keys.just_pressed(KeyCode::ArrowRight)||pads.get_just_pressed().any(|p|p.button_type==GamepadButtonType::DPadRight){1.0}else{0.0};
    if direction!=0.0{if let Some(input)=menus.focus.and_then(|e|all.get(e).ok()).filter(|i|i.kind==Kind::Slider){let c=&input.control;let value=c.value(input.kind,&menus.session).as_f64().unwrap_or(c.minimum);set(&mut menus,input,c.quantize(value+direction*c.step).into());}}
    if mouse.pressed(MouseButton::Left){if let (Some((input,left,width)),Some(cursor))=(&drag.0,windows.get_single().ok().and_then(|w|w.cursor_position())){let c=&input.control;let value=c.minimum+(c.maximum-c.minimum)*((cursor.x-left)/width).clamp(0.0,1.0)as f64;set(&mut menus,input,c.quantize(value).into());}}else{drag.0=None;}
}
