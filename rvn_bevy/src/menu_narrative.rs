//! Custom dialogue presentation reuses the engine's already parsed typing state.
//! The original widgets remain hidden data consumers, never a second visible UI.
use super::*;
use crate::components::{CharacterNameText,DialogueBox};
use crate::components::{ChoiceButton,ChoiceContainer};
use crate::resources::ChoiceFocus;

fn page_event(menus:&mut Menus,previous:&mut Option<String>,next:Option<String>){if *previous!=next{if let Some(old)=previous.take(){menus.page_events.push((old,rvn_ui::Event::Close));}if let Some(id)=&next{menus.page_events.push((id.clone(),rvn_ui::Event::Open));}*previous=next;}}

#[derive(Component)]pub(super) struct QuickActionsRoot;
pub(super) fn render_quick_actions(mut commands:Commands,mut menus:ResMut<Menus>,ctx:Context,assets:Res<AssetServer>,roots:Query<Entity,With<QuickActionsRoot>>,mut last:Local<String>,mut previous:Local<Option<String>>){
    let index=menus.doc.as_ref().and_then(|d|d.pages.iter().position(|p|p.role==Some(rvn_ui::PageRole::QuickActions)));
    if index.is_none()||*ctx.state.get()!=VnState::Waiting||menus.page.is_some(){page_event(&mut menus,&mut previous,None);for root in &roots{commands.entity(root).despawn_recursive();}last.clear();return;}
    let doc=menus.session.present(menus.doc.as_ref().unwrap());let index=index.unwrap();let window=ctx.windows.single();let size=[window.width(),window.height()];
    page_event(&mut menus,&mut previous,Some(doc.pages[index].id.clone()));
    let key=format!("{size:?}:{}",serde_json::to_string(&doc.pages[index]).unwrap_or_default());let key=format!("{key}:fonts={}:lang={}",ctx.fonts.len(),ctx.engine.0.locale.as_ref().map(|l|l.current_lang()).unwrap_or(""));if *last==key{return;}*last=key;
    for root in &roots{commands.entity(root).despawn_recursive();}
    let root=commands.spawn((QuickActionsRoot,NodeBundle{style:Style{position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),..default()},background_color:color(doc.pages[index].background).into(),z_index:ZIndex::Global(700),..default()})).id();
    for e in menu_layout::tree(&doc,index,size,&assets,&ctx,0,&[]){spawn_element(&mut commands,root,&e,&doc.pages[index].id,size,size,&assets,&ctx,&doc,0);}
}

#[derive(Component)]pub(super) struct ChoicesRoot;
#[derive(Component)]pub(super) struct ChoiceAppearance{pub(super) normal:Color,pub(super) hover:Color,pub(super) pressed:Color,pub(super) focus:Color}
impl ChoiceAppearance{pub(super) fn from_element(e:&Element)->Self{if !e.enabled{let disabled=color(e.disabled);return Self{normal:disabled,hover:disabled,pressed:disabled,focus:disabled};}Self{normal:color(e.normal),hover:color(e.hover),pressed:color(e.pressed),focus:color(e.appearance.focus.unwrap_or(e.hover))}}}
#[derive(Component)]pub(super) struct ChoiceVisual(pub(super) usize);
#[derive(Component)]pub(super) struct ChoiceScroll{gap:f32}

pub(super) fn render_choices(mut commands:Commands,mut menus:ResMut<Menus>,ctx:Context,assets:Res<AssetServer>,state:Res<VnRenderState>,mut original:Query<&mut Visibility,With<ChoiceContainer>>,roots:Query<Entity,With<ChoicesRoot>>,mut last:Local<String>,mut previous:Local<Option<String>>){
    let index=menus.doc.as_ref().and_then(|d|d.pages.iter().position(|p|p.role==Some(rvn_ui::PageRole::Choices)));
    for mut visibility in &mut original{*visibility=if index.is_some(){Visibility::Hidden}else{Visibility::Inherited};}
    if index.is_none()||state.choice_options.is_empty(){page_event(&mut menus,&mut previous,None);for root in &roots{commands.entity(root).despawn_recursive();}last.clear();return;}
    let doc=menus.session.present(menus.doc.as_ref().unwrap());let index=index.unwrap();let window=ctx.windows.single();let size=[window.width(),window.height()];
    page_event(&mut menus,&mut previous,Some(doc.pages[index].id.clone()));
    let key=format!("{size:?}:{:?}:{}",state.choice_options,serde_json::to_string(&doc.pages[index]).unwrap_or_default());let key=format!("{key}:fonts={}",ctx.fonts.len());if *last==key{return;}*last=key;
    for root in &roots{commands.entity(root).despawn_recursive();}
    let root=commands.spawn((ChoicesRoot,NodeBundle{style:Style{position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),..default()},background_color:color(doc.pages[index].background).into(),z_index:ZIndex::Global(600),..default()})).id();
    for e in menu_layout::tree(&doc,index,size,&assets,&ctx,0,&[]){spawn_choices(&mut commands,root,&e,&doc.pages[index].id,size,&assets,&ctx,&doc,&state.choice_options);}
}
fn spawn_choices(commands:&mut Commands,root:Entity,e:&Element,page:&str,size:[f32;2],assets:&AssetServer,ctx:&Context,doc:&Document,options:&[String]){
        if e.kind!=Kind::ChoiceList{let mut shallow=e.clone();shallow.children.clear();if let Some(parent)=spawn_element(commands,root,&shallow,page,size,size,assets,ctx,doc,0){if e.kind==Kind::Scroll{fn height(es:&[Element],y:f32)->f32{es.iter().map(|e|(y+e.rect[1]+e.rect[3]).max(height(&e.children,y+e.rect[1]))).fold(0.0,f32::max)}commands.entity(parent).insert(MenuScroll{offset:0.0,content_height:height(&e.children,0.0)});}for child in &e.children{spawn_choices(commands,parent,child,page,[e.rect[2],e.rect[3]],assets,ctx,doc,options);}}return;}
        let list=commands.spawn((ChoiceScroll{gap:e.layout_options.gap},MenuScroll{offset:0.0,content_height:0.0},NodeBundle{style:Style{position_type:PositionType::Absolute,left:Val::Px(e.rect[0]),top:Val::Px(e.rect[1]),width:Val::Px(e.rect[2]),height:Val::Px(e.rect[3]),flex_direction:FlexDirection::Column,row_gap:Val::Px(e.layout_options.gap),overflow:Overflow::clip_y(),..default()},..default()})).id();commands.entity(root).add_child(list);commands.entity(list).insert(scrollbars::ScrollAppearance::from_element(e));commands.entity(list).insert(animations::VisualNode{page:page.into(),id:e.id.clone(),scale:(ctx.windows.single().width()/doc.reference[0]).min(ctx.windows.single().height()/doc.reference[1])});
        for (index,label) in options.iter().enumerate(){
            if let Some(template)=&e.list.template{let data=[("choice.text".into(),label.clone())].into_iter().collect();dynamic_rows::spawn(commands,list,e,page,template,&data,assets,ctx,doc,Some(index));continue;}
            let row=commands.spawn((ChoiceButton(index),ChoiceAppearance::from_element(e),ButtonBundle{style:Style{position_type:PositionType::Relative,top:Val::Px(0.0),width:Val::Percent(100.0),min_height:Val::Px(e.font_size*2.2),flex_shrink:0.0,padding:UiRect::axes(Val::Px(18.0),Val::Px(12.0)),align_items:AlignItems::Center,border:UiRect::all(Val::Px(e.appearance.border_width)),..default()},background_color:color(e.normal).into(),border_color:color(e.appearance.border_color).into(),border_radius:BorderRadius::all(Val::Px(e.appearance.radius)),..default()})).with_children(|p|{p.spawn(TextBundle{style:Style{width:Val::Percent(100.0),max_width:Val::Percent(100.0),..default()},text:Text::from_section(label.clone(),TextStyle{font:e.font.as_ref().map(|f|assets.load(f.clone())).unwrap_or_else(||ctx.font.0.clone()),font_size:e.font_size,color:color(e.foreground)}).with_justify(match e.layout_options.text_align{rvn_ui::TextAlign::Left=>JustifyText::Left,rvn_ui::TextAlign::Center=>JustifyText::Center,rvn_ui::TextAlign::Right=>JustifyText::Right}),..default()}).insert(text_states::TextAppearance::new(e,false));}).id();commands.entity(list).add_child(row);
        }
}
pub(super) fn choice_focus_visuals(focus:Res<ChoiceFocus>,mut rows:Query<(&ChoiceButton,&ChoiceAppearance,&Interaction,&mut BackgroundColor)>,buttons:Query<(&ChoiceButton,&Interaction)>,mut visuals:Query<(&ChoiceVisual,&ChoiceAppearance,&mut BackgroundColor),Without<ChoiceButton>>){
    fn paint(index:usize,a:&ChoiceAppearance,i:Interaction,focus:Option<usize>)->Color{if i==Interaction::Pressed{a.pressed}else if focus==Some(index){a.focus}else if i==Interaction::Hovered{a.hover}else{a.normal}}
    for(id,a,i,mut color)in &mut rows{color.0=paint(id.0,a,*i,focus.0);}for(id,a,mut color)in &mut visuals{let i=buttons.iter().find(|(b,_)|b.0==id.0).map(|(_,i)|*i).unwrap_or(Interaction::None);color.0=paint(id.0,a,i,focus.0);}
}
pub(super) fn choice_scroll_bounds(focus:Res<ChoiceFocus>,mut lists:Query<(&Node,&GlobalTransform,&Children,&ChoiceScroll,&mut MenuScroll)>,mut children:Query<(&Node,&GlobalTransform,&ChoiceButton,&mut Style)>){
    for (node,transform,ids,settings,mut scroll) in &mut lists{
        scroll.content_height=ids.iter().filter_map(|id|children.get(*id).ok()).map(|(n,_,_,_)|n.size().y).sum::<f32>()+settings.gap*ids.iter().filter(|id|children.get(**id).is_ok()).count().saturating_sub(1) as f32;
        if !focus.is_changed(){continue;}let origin=transform.translation().y-node.size().y*0.5;
        let delta=ids.iter().filter_map(|id|children.get(*id).ok()).find(|(_,_,id,_)|Some(id.0)==focus.0).map(|(n,t,_,_)|{let top=t.translation().y-n.size().y*0.5;if top<origin{top-origin}else{(top+n.size().y-origin-node.size().y).max(0.0)}}).unwrap_or(0.0);
        let old=scroll.offset;scroll.offset=(old+delta).clamp(0.0,(scroll.content_height-node.size().y).max(0.0));for id in ids{if let Ok((_,_,_,mut style))=children.get_mut(*id){style.top=Val::Px(-scroll.offset);}}
    }
}

#[derive(Component)]
pub(super) struct NarrativeRoot;
#[derive(Component)]
pub(super) struct NarrativeText{base:TextStyle,align:JustifyText,wrap:bool}
#[derive(Component,Default)]pub(super) struct NarrativeViewport{text:String,chars:usize,offset:f32,following:bool}
pub(super) fn follow_dialogue(typing:Res<TypewriterState>,mut keys:ResMut<ButtonInput<KeyCode>>,mut views:Query<(&Node,&Children,&mut MenuScroll,&mut NarrativeViewport)>,mut texts:Query<&mut Style,With<NarrativeText>>){
    for(node,children,mut scroll,mut viewport)in &mut views{
        let reset=viewport.text!=typing.full_text||typing.visible_chars<viewport.chars;
        let max=(scroll.content_height-node.size().y).max(0.0);
        if reset{viewport.text=typing.full_text.clone();viewport.following=true;scroll.offset=0.0;}
        else if (scroll.offset-viewport.offset).abs()>0.5{viewport.following=scroll.offset>=max-0.5;}
        if typing.visible_chars!=viewport.chars&&viewport.following{scroll.offset=max;}
        if max>0.0{
            if keys.just_pressed(KeyCode::PageUp){scroll.offset-=node.size().y*0.8;viewport.following=false;keys.clear_just_pressed(KeyCode::PageUp);}
            if keys.just_pressed(KeyCode::PageDown){scroll.offset+=node.size().y*0.8;keys.clear_just_pressed(KeyCode::PageDown);}
        }
        scroll.offset=scroll.offset.clamp(0.0,max);viewport.offset=scroll.offset;viewport.chars=typing.visible_chars;
        for child in children{if let Ok(mut style)=texts.get_mut(*child){style.top=Val::Px(-scroll.offset);}}
    }
}

fn spawn_dialogue(commands:&mut Commands,parent:Entity,e:&Element,page:&str,size:[f32;2],assets:&AssetServer,ctx:&Context,doc:&Document,name:&str,typing:&TypewriterState){
    if !e.visible||name.is_empty()&&matches!(e.binding.as_deref(),Some("dialogue.name"|"dialogue.speaker")){return;}
    let mut element=e.clone();element.children.clear();if element.binding.as_deref()==Some("dialogue.name"){element.text=name.into();}
    let entity=if element.binding.as_deref()==Some("dialogue.text"){
        let base=TextStyle{font:e.font.as_ref().map(|p|assets.load(p.clone())).unwrap_or_else(||ctx.font.0.clone()),font_size:e.font_size,color:color(e.foreground)};
        let align=match e.layout_options.text_align{rvn_ui::TextAlign::Left=>JustifyText::Left,rvn_ui::TextAlign::Center=>JustifyText::Center,rvn_ui::TextAlign::Right=>JustifyText::Right};let wrap=e.layout_options.text_wrap;
        let mut text=Text::from_section("",base.clone()).with_justify(align);if !wrap{text=text.with_no_wrap();}crate::systems::typewriter::apply_visible_sections(&mut text,typing);
        let entity=commands.spawn((NarrativeViewport{following:true,..default()},AutoList,MenuScroll{offset:0.0,content_height:0.0},NodeBundle{style:Style{position_type:PositionType::Absolute,left:Val::Px(e.rect[0]),top:Val::Px(e.rect[1]),width:Val::Px(e.rect[2]),height:Val::Px(e.rect[3]),overflow:Overflow::clip_y(),..default()},..default()})).id();
        shadows::spawn(commands,parent,entity,e,e.rect);commands.entity(entity).insert(scrollbars::ScrollAppearance::from_element(e));commands.entity(parent).add_child(entity);attach_interactions(commands,entity,e,page,doc,e.enabled);
        let text=commands.spawn((NarrativeText{base,align,wrap},TextBundle{text,style:Style{position_type:PositionType::Absolute,left:Val::Px(0.0),top:Val::Px(0.0),width:Val::Px((e.rect[2]-e.appearance.scrollbar.width-2.0).max(1.0)),height:Val::Auto,..default()},..default()})).id();commands.entity(entity).add_child(text);entity
    }else{let Some(entity)=spawn_element(commands,parent,&element,page,size,size,assets,ctx,doc,0)else{return};entity};
    commands.entity(entity).insert(animations::VisualNode{page:page.into(),id:e.id.clone(),scale:(ctx.windows.single().width()/doc.reference[0]).min(ctx.windows.single().height()/doc.reference[1])});
    if e.kind==Kind::Scroll{fn height(es:&[Element],y:f32)->f32{es.iter().map(|e|(y+e.rect[1]+e.rect[3]).max(height(&e.children,y+e.rect[1]))).fold(0.0,f32::max)}commands.entity(entity).insert(MenuScroll{offset:0.0,content_height:height(&e.children,0.0)});}
    for child in &e.children{spawn_dialogue(commands,entity,child,page,[e.rect[2],e.rect[3]],assets,ctx,doc,name,typing);}
}

pub(super) fn render_narrative(
    mut commands:Commands,mut menus:ResMut<Menus>,ctx:Context,assets:Res<AssetServer>,typing:Res<TypewriterState>,
    mut original:Query<(&Style,&mut Visibility),With<DialogueBox>>,
    names:Query<&Text,With<CharacterNameText>>,roots:Query<Entity,With<NarrativeRoot>>,
    mut texts:Query<(&NarrativeText,&mut Text),Without<CharacterNameText>>,
    mut last:Local<String>,mut previous:Local<Option<String>>,
){
    let role=menus.doc.as_ref().and_then(|d|d.pages.iter().position(|p|p.role==Some(rvn_ui::PageRole::Dialogue)));
    let in_story=matches!(ctx.state.get(),VnState::Waiting|VnState::Stepping|VnState::Animating);
    let active=role.is_some()&&in_story&&original.iter().any(|(style,_)|style.display!=Display::None);
    for (_,mut visibility) in &mut original{*visibility=if active||!in_story{Visibility::Hidden}else{Visibility::Inherited};}
    if !active{page_event(&mut menus,&mut previous,None);for root in &roots{commands.entity(root).despawn_recursive();}last.clear();return;}
    let name=names.iter().next().map(|t|t.sections.iter().map(|s|s.value.as_str()).collect::<String>()).unwrap_or_default();
    let window=ctx.windows.single();let size=[window.width(),window.height()];
    let doc=menus.session.present(menus.doc.as_ref().unwrap());let index=role.unwrap();
    page_event(&mut menus,&mut previous,Some(doc.pages[index].id.clone()));
    let key=format!("{:?}:{size:?}:{name}:{}:{}",menus.modified,ctx.fonts.len(),serde_json::to_string(&doc.pages[index]).unwrap_or_default());
    if *last!=key{
        for root in &roots{commands.entity(root).despawn_recursive();}
        *last=key;
        let root=commands.spawn((NarrativeRoot,NodeBundle{style:Style{position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),..default()},background_color:color(doc.pages[index].background).into(),z_index:ZIndex::Global(500),..default()})).id();
        for e in menu_layout::tree(&doc,index,size,&assets,&ctx,0,&[("dialogue.name",&name)]){spawn_dialogue(&mut commands,root,&e,&doc.pages[index].id,size,&assets,&ctx,&doc,&name,&typing);}
    }else if typing.is_changed(){for (style,mut text) in &mut texts{*text=Text::from_section("",style.base.clone()).with_justify(style.align);if !style.wrap{*text=text.clone().with_no_wrap();}crate::systems::typewriter::apply_visible_sections(&mut text,&typing);}}
}
