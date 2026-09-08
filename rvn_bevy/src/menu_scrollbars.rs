//! Presentation-only scrollbars; content and narrative state stay untouched.
use super::*;

#[derive(Component)]
pub(crate) struct Scrollbar;
#[derive(Component)]
pub(super) struct Thumb;
#[derive(Component,Clone)]pub(super) struct ScrollAppearance(pub rvn_ui::ScrollbarStyle);
impl ScrollAppearance{pub fn from_element(e:&Element)->Self{let mut s=e.appearance.scrollbar.clone();for c in [&mut s.track,&mut s.thumb,&mut s.hover,&mut s.pressed]{c[3]*=e.appearance.opacity;}Self(s)}}
#[derive(Component)]
pub(super) struct Track { list:Entity, thumb:Entity,colors:rvn_ui::ScrollbarStyle }

fn metrics(view:f32,content:f32,offset:f32)->(f32,f32){
    if view<=0.0||content<=view{return(view.max(0.0),0.0)}
    let height=(view*view/content).max(24.0).min(view);
    (height,(view-height)*offset.clamp(0.0,content-view)/(content-view))
}

pub(super) fn create(mut commands:Commands,lists:Query<(Entity,Option<&ScrollAppearance>),Added<MenuScroll>>){
    for(list,appearance)in &lists{
        let colors=appearance.map(|s|s.0.clone()).unwrap_or_default();let inset=(colors.width/6.0).min(2.0);
        let thumb=commands.spawn((NodeBundle{style:Style{position_type:PositionType::Absolute,left:Val::Px(inset),width:Val::Px(colors.width-inset*2.0),top:Val::Px(0.0),height:Val::Px(24.0),..default()},background_color:color(colors.thumb).into(),border_radius:BorderRadius::all(Val::Px(colors.radius)),..default()},Thumb)).id();
        let track=commands.spawn((ButtonBundle{style:Style{position_type:PositionType::Absolute,right:Val::Px(0.0),top:Val::Px(0.0),width:Val::Px(colors.width),height:Val::Percent(100.0),..default()},background_color:color(colors.track).into(),z_index:ZIndex::Local(20),..default()},Scrollbar,Track{list,thumb,colors})).id();
        commands.entity(track).add_child(thumb);commands.entity(list).add_child(track);
    }
}

pub(super) fn sync(lists:Query<(&Node,&MenuScroll)>,tracks:Query<(Entity,&Track,&Interaction)>,mut styles:Query<&mut Style>,mut paints:Query<&mut BackgroundColor,With<Thumb>>){
    for(entity,track,interaction)in &tracks{let Ok((node,scroll))=lists.get(track.list)else{continue};let view=node.size().y;let(height,top)=metrics(view,scroll.content_height,scroll.offset);
        if let Ok(mut paint)=paints.get_mut(track.thumb){paint.0=color(match interaction{Interaction::Pressed=>track.colors.pressed,Interaction::Hovered=>track.colors.hover,Interaction::None=>track.colors.thumb});}
        if let Ok(mut style)=styles.get_mut(entity){style.display=if scroll.content_height>view+0.5{Display::Flex}else{Display::None};}
        if let Ok(mut style)=styles.get_mut(track.thumb){style.height=Val::Px(height);style.top=Val::Px(top);}
    }
}

pub(super) fn drag(mouse:Res<ButtonInput<MouseButton>>,windows:Query<&Window>,tracks:Query<(Entity,&Track,&Interaction,&Node,&GlobalTransform)>,mut lists:Query<(&Node,&Children,&mut MenuScroll)>,mut styles:Query<&mut Style,Without<Scrollbar>>,mut dragging:Local<Option<(Entity,f32)>>){
    if !mouse.pressed(MouseButton::Left){*dragging=None;return;}
    let Some(cursor)=windows.get_single().ok().and_then(|w|w.cursor_position())else{return};
    if mouse.just_pressed(MouseButton::Left){for(entity,track,interaction,node,transform)in &tracks{if *interaction!=Interaction::Pressed{continue}let Ok((_,_,scroll))=lists.get(track.list)else{continue};let(height,top)=metrics(node.size().y,scroll.content_height,scroll.offset);let local=cursor.y-(transform.translation().y-node.size().y*0.5);let grab=if local>=top&&local<=top+height{local-top}else{height*0.5};*dragging=Some((entity,grab));break;}}
    let Some((entity,grab))=*dragging else{return};let Ok((_,track,_,node,transform))=tracks.get(entity)else{*dragging=None;return};let Ok((_,children,mut scroll))=lists.get_mut(track.list)else{return};
    let view=node.size().y;let(height,_)=metrics(view,scroll.content_height,scroll.offset);if view<=height{return;}
    let local=cursor.y-(transform.translation().y-view*0.5)-grab;let old=scroll.offset;
    scroll.offset=(local/(view-height)).clamp(0.0,1.0)*(scroll.content_height-view).max(0.0);
    for child in children{if let Ok(mut style)=styles.get_mut(*child){if let Val::Px(y)=style.top{style.top=Val::Px(y+old-scroll.offset);}}}
}

#[cfg(test)]mod tests{use super::*;#[test]fn thumb_geometry_and_limits(){assert_eq!(metrics(100.0,50.0,0.0),(100.0,0.0));assert_eq!(metrics(100.0,400.0,150.0),(25.0,37.5));assert_eq!(metrics(100.0,400.0,500.0),(25.0,75.0));assert_eq!(metrics(0.0,400.0,5.0),(0.0,0.0));assert_eq!(metrics(100.0,10000.0,-1.0),(24.0,0.0));}}
