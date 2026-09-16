//! Destructive save operations have one transient approval gate.
use super::*;
use crate::systems::save_menu::SaveConfirmation;
#[derive(Component)]pub(super) struct ConfirmationRoot;
#[derive(Component)]pub(crate) struct ConfirmationButton(pub(crate) bool);
#[derive(Component)]pub(super) struct ConfirmationColors{normal:Color,hover:Color,pressed:Color}
#[derive(Component)]pub(super) struct ConfirmationTarget(String,Option<i32>);
fn focused_element(elements:&[Element],approve:bool)->Option<&Element>{
    for e in elements{if !e.visible||!e.enabled{continue;}if e.action==if approve{Action::Confirm}else{Action::Back}{return Some(e);}if let Some(child)=focused_element(&e.children,approve){return Some(child);}}
    None
}

fn available_targets(doc:&Document,page:usize)->(std::collections::BTreeSet<String>,Vec<(i32,String)>){
    let mut pointer=std::collections::BTreeSet::new();let mut keyboard=Vec::new();
    for e in doc.layout_page(page,doc.reference){
        if !e.enabled{continue;}
        let events=doc.pages[page].graphs.iter().filter(|g|g.target.as_deref()==Some(e.id.as_str())).map(|g|g.event.clone());
        let(p,k)=interaction_policy(e.kind.clone(),events);let decision=matches!(e.action,Action::Confirm|Action::Back);
        if p||decision{pointer.insert(e.id.clone());}if k||decision{keyboard.push((e.focus_order,e.id));}
    }
    keyboard.sort();(pointer,keyboard)
}

fn dispatch(commands:&mut Commands,menus:&mut Menus,assets:&AssetServer,volume:f32,page:&rvn_ui::Page,target:Option<&str>,event:rvn_ui::Event,fallback:Action)->Option<bool>{
    let mut decision=None;
    match menus.session.dispatch_confirmation(page,target,event,fallback){
        Err(error)=>{error!("Confirmation : {error}");report_preview_error(&menus.session);},
        Ok(effects)=>for effect in effects{match effect{
            Effect::Action(Action::Confirm)=>decision=Some(true),Effect::Action(Action::Back)=>decision=Some(false),
            Effect::Animate(id,clip)=>menus.animation_requests.push((page.id.clone(),id,clip)),
            Effect::Sound(path)=>{commands.spawn(AudioBundle{source:assets.load(path),settings:bevy::audio::PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::new(volume)),..default()});},
            effect=>menus.session.apply_presentation(&page.id,&effect),
        }}
    }
    decision
}

pub(super) fn update(
    mut commands:Commands,mut confirmation:ResMut<SaveConfirmation>,mut menus:ResMut<Menus>,
    ctx:Context,assets:Res<AssetServer>,roots:Query<Entity,With<ConfirmationRoot>>,
    mut buttons:Query<(Ref<Interaction>,Option<&ConfirmationButton>,&ConfirmationColors,&mut BackgroundColor,&ConfirmationTarget)>,
    mut keys:ResMut<ButtonInput<KeyCode>>,pads:Res<ButtonInput<GamepadButton>>,
    mut previous:Local<Option<String>>,mut focus:Local<Option<String>>,
    mut opened:Local<Option<rvn_ui::Page>>,mut hovering:Local<std::collections::BTreeSet<String>>,
    mut next:ResMut<NextState<VnState>>,mut player:EventWriter<crate::vn_command::PlayerInput>,
){
    let operation=confirmation.pending;let action=confirmation.action_pending.clone();
    if !confirmation.active(){
        if let Some(page)=opened.take(){dispatch(&mut commands,&mut menus,&assets,ctx.values.sfx_volume,&page,None,rvn_ui::Event::Close,Action::None);}
        hovering.clear();for root in &roots{commands.entity(root).despawn_recursive();}*previous=None;return;
    }
    let mut doc=menus.doc.clone().unwrap_or_else(Document::defaults);
    let index=doc.pages.iter().position(|p|p.role==Some(rvn_ui::PageRole::Confirm)).unwrap_or_else(||{let page=Document::defaults().pages.into_iter().find(|p|p.role==Some(rvn_ui::PageRole::Confirm)).unwrap();doc.pages.push(page);doc.pages.len()-1});
    let opening=opened.is_none();
    if opening{*focus=focused_element(&doc.pages[index].elements,false).map(|e|e.id.clone());}
    if opening{*opened=Some(doc.pages[index].clone());dispatch(&mut commands,&mut menus,&assets,ctx.values.sfx_volume,&doc.pages[index],None,rvn_ui::Event::Open,Action::None);}
    let next_hover:std::collections::BTreeSet<_>=buttons.iter().filter(|(i,_,_,_,_)|**i!=Interaction::None).map(|(_,_,_,_,t)|t.0.clone()).collect();
    for id in next_hover.difference(&hovering){dispatch(&mut commands,&mut menus,&assets,ctx.values.sfx_volume,&doc.pages[index],Some(id),rvn_ui::Event::Hover,Action::None);}
    for id in hovering.difference(&next_hover){dispatch(&mut commands,&mut menus,&assets,ctx.values.sfx_volume,&doc.pages[index],Some(id),rvn_ui::Event::HoverLeave,Action::None);}
    *hovering=next_hover;
    // Presentation graphs can hide/disable the focused control during this
    // frame, before its old entity is rebuilt. Do not activate stale entities.
    let presented=menus.session.present(&doc);
    let (available,items)=available_targets(&presented,index);
    let repaired=focus.as_ref().is_none_or(|id|!items.iter().any(|(_,item)|item==id));
    if repaired{*focus=focused_element(&presented.pages[index].elements,false).map(|e|e.id.clone()).filter(|id|items.iter().any(|(_,item)|item==id)).or_else(||items.first().map(|(_,id)|id.clone()));*previous=None;}
    let pad=|kind|pads.get_just_pressed().any(|p|p.button_type==kind);
    let mut activation=buttons.iter().find(|(i,_,_,_,t)|**i==Interaction::Pressed&&i.is_changed()&&available.contains(&t.0)).map(|(_,b,_,_,target)|(b.map(|b|b.0),target.0.clone()));
    let mut decision=None;
    for(i,_,colors,mut bg,_)in &mut buttons{bg.0=match *i{Interaction::Pressed=>colors.pressed,Interaction::Hovered=>colors.hover,Interaction::None=>colors.normal};}
    let moved=keys.just_pressed(KeyCode::Tab)||keys.just_pressed(KeyCode::ArrowUp)||keys.just_pressed(KeyCode::ArrowDown)||pad(GamepadButtonType::DPadUp)||pad(GamepadButtonType::DPadDown)||keys.just_pressed(KeyCode::ArrowLeft)||keys.just_pressed(KeyCode::ArrowRight)||pad(GamepadButtonType::DPadLeft)||pad(GamepadButtonType::DPadRight);
    if moved{
        if !items.is_empty(){let backward=keys.just_pressed(KeyCode::ArrowUp)||keys.just_pressed(KeyCode::ArrowLeft)||pad(GamepadButtonType::DPadUp)||pad(GamepadButtonType::DPadLeft)||(keys.just_pressed(KeyCode::Tab)&&(keys.pressed(KeyCode::ShiftLeft)||keys.pressed(KeyCode::ShiftRight)));
            let current=items.iter().position(|(_,id)|focus.as_ref()==Some(id));let index=match current{Some(i)if backward=>(i+items.len()-1)%items.len(),Some(i)=>(i+1)%items.len(),None if backward=>items.len()-1,None=>0};*focus=Some(items[index].1.clone());*previous=None;
        }
    }
    if opening||moved||repaired{if let Some(id)=focus.as_deref(){dispatch(&mut commands,&mut menus,&assets,ctx.values.sfx_volume,&doc.pages[index],Some(id),rvn_ui::Event::Focus,Action::None);}}
    if keys.just_pressed(KeyCode::Escape)||pad(GamepadButtonType::East){decision=Some(false);}
    if keys.just_pressed(KeyCode::Enter)||keys.just_pressed(KeyCode::Space)||pad(GamepadButtonType::South){activation=buttons.iter().find(|(_,_,_,_,t)|focus.as_ref()==Some(&t.0)).map(|(_,b,_,_,target)|(b.map(|b|b.0),target.0.clone()));}
    if let Some((_,id))=&activation{if !available_targets(&menus.session.present(&doc),index).0.contains(id){activation=None;}}
    if decision.is_none(){if let Some((approved,id))=activation{
        decision=dispatch(&mut commands,&mut menus,&assets,ctx.values.sfx_volume,&doc.pages[index],Some(&id),rvn_ui::Event::Click,match approved{Some(true)=>Action::Confirm,Some(false)=>Action::Back,None=>Action::None});
    }}
    for key in [KeyCode::Escape,KeyCode::ArrowUp,KeyCode::ArrowDown,KeyCode::Tab,KeyCode::ArrowLeft,KeyCode::ArrowRight,KeyCode::Enter,KeyCode::Space]{keys.clear_just_pressed(key);}
    if let Some(approved)=decision{
        if let Some(page)=opened.take(){for id in hovering.iter(){dispatch(&mut commands,&mut menus,&assets,ctx.values.sfx_volume,&page,Some(id),rvn_ui::Event::HoverLeave,Action::None);}dispatch(&mut commands,&mut menus,&assets,ctx.values.sfx_volume,&page,None,rvn_ui::Event::Close,Action::None);}hovering.clear();
        info!("[confirmation] decision {operation:?} / {action:?}: {approved}");
        confirmation.pending=None;
        if let Some(operation)=operation{
            if operation.0==0{let previous=confirmation.quick_return.take().unwrap_or(VnState::Waiting);next.set(previous);if approved{confirmation.approved=Some(operation);player.send(crate::vn_command::PlayerInput::QuickLoad);}}
            else if approved{confirmation.approved=Some(operation);commands.spawn((MenuProxy,SaveSlotButton(operation.0),crate::systems::save_menu::SaveSlotMode(operation.1),ButtonBundle{style:Style{display:Display::None,..default()},interaction:Interaction::Pressed,..default()}));}
        }else if let Some((_,page,origin))=action{
            confirmation.action_pending=None;next.set(origin);
            // Empty element IDs are forbidden in documents: this internal
            // acknowledgement cannot re-run the source button's graph.
            if approved{menus.activations.push((page,Some(String::new()),rvn_ui::Event::Click,Action::Confirm));}else{menus.pending=None;}
        }
        for root in &roots{commands.entity(root).despawn_recursive();}*previous=None;*focus=None;return;
    }
    doc=menus.session.present(&doc);
    let window=ctx.windows.single();let size=[window.width(),window.height()];let key=format!("{operation:?}/{action:?}/{size:?}/{}",serde_json::to_string(&doc.pages[index]).unwrap_or_default());let key=format!("{key}:fonts={}",ctx.fonts.len());if previous.as_ref()==Some(&key){return;}*previous=Some(key);
    for root in &roots{commands.entity(root).despawn_recursive();}
    let root=commands.spawn((ConfirmationRoot,NodeBundle{style:Style{position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),..default()},background_color:color(doc.pages[index].background).into(),z_index:ZIndex::Global(4000),focus_policy:bevy::ui::FocusPolicy::Block,..default()})).id();
    let title=if let Some((slot,mode))=operation{if slot==0{translated(&ctx,"Charger la sauvegarde rapide et remplacer la partie en cours ?")}else if mode==SaveMenuMode::Delete{translated(&ctx,"Supprimer définitivement la sauvegarde [slot] ?").replace("[slot]",&slot.to_string())}else if mode==SaveMenuMode::Save{translated(&ctx,"Remplacer la sauvegarde [slot] ?").replace("[slot]",&slot.to_string())}else{translated(&ctx,"Charger la sauvegarde [slot] et remplacer la partie en cours ?").replace("[slot]",&slot.to_string())}}else if matches!(action,Some((Action::Continue,_,_))){translated(&ctx,"Continuer la sauvegarde et remplacer la partie en cours ?")}else{translated(&ctx,"Démarrer une nouvelle partie et remplacer la partie en cours ?")};
    for e in menu_layout::tree(&doc,index,size,&assets,&ctx,0,&[("confirmation.message",&title)]){spawn_confirmation(&mut commands,root,&e,&doc.pages[index].id,size,&assets,&ctx,&doc,&title,focus.as_deref());}
}

fn spawn_confirmation(commands:&mut Commands,parent:Entity,e:&Element,page:&str,size:[f32;2],assets:&AssetServer,ctx:&Context,doc:&Document,title:&str,focus:Option<&str>){
    let button=if e.enabled{match e.action{Action::Confirm=>Some(true),Action::Back=>Some(false),_=>None}}else{None};let mut shallow=e.clone();shallow.children.clear();shallow.action=Action::None;
    if e.kind==Kind::Text&&(e.id=="heading"||e.binding.as_deref()==Some("confirmation.message")){shallow.text=title.into();shallow.binding=None;}
    if focus==Some(e.id.as_str()){shallow.normal=e.appearance.focus.unwrap_or(e.hover);}
    let Some(entity)=spawn_element(commands,parent,&shallow,page,size,size,assets,ctx,doc,0)else{return};commands.entity(entity).remove::<MenuElement>().remove::<MenuFocus>();
    if e.kind==Kind::Text{commands.entity(entity).insert(Style{position_type:PositionType::Absolute,left:Val::Px(e.rect[0]),top:Val::Px(e.rect[1]),width:Val::Px(e.rect[2]),height:Val::Auto,min_height:Val::Px(e.rect[3]),..default()});}
    let events=doc.pages.iter().filter(|p|p.id==page).flat_map(|p|&p.graphs).filter(|g|g.target.as_deref()==Some(e.id.as_str())).map(|g|g.event.clone());
    let(pointer,keyboard)=interaction_policy(e.kind.clone(),events);
    if e.enabled&&pointer{commands.entity(entity).insert((Interaction::None,ConfirmationTarget(e.id.clone(),keyboard.then_some(e.focus_order)),ConfirmationColors{normal:color(shallow.normal),hover:color(e.hover),pressed:color(e.pressed)}));}
    if let Some(yes)=button{commands.entity(entity).insert((Button,Interaction::None,ConfirmationButton(yes),ConfirmationTarget(e.id.clone(),Some(e.focus_order)),ConfirmationColors{normal:color(shallow.normal),hover:color(e.hover),pressed:color(e.pressed)}));}
    for child in &e.children{spawn_confirmation(commands,entity,child,page,[e.rect[2],e.rect[3]],assets,ctx,doc,title,focus);}
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn modal_focus_excludes_hidden_and_disabled_descendants(){
        let mut doc=Document::defaults();let index=doc.pages.iter().position(|p|p.role==Some(rvn_ui::PageRole::Confirm)).unwrap();
        let mut panel=Element::new("group".into(),Kind::Panel);
        let mut child=Element::new("extra".into(),Kind::Button);child.focus_order=-10;panel.children.push(child);doc.pages[index].elements.push(panel);
        assert_eq!(available_targets(&doc,index).1.first().unwrap().1,"extra");
        for effect in [Effect::Visible("group".into(),false),Effect::Enabled("group".into(),false)]{
            let mut session=rvn_ui::Session::default();session.apply_presentation(&doc.pages[index].id,&effect);
            let (pointer,keyboard)=available_targets(&session.present(&doc),index);
            assert!(!pointer.contains("extra"));assert!(!keyboard.iter().any(|(_,id)|id=="extra"));assert!(!keyboard.is_empty());
        }
        assert!(doc.find_element(index,"group").unwrap().enabled);
    }
}
