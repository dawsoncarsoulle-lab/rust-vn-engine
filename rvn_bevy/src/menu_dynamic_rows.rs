//! Reusable choice/history rows. Text height drives the row, not truncation.
use super::*;
#[derive(Component)]pub(super) struct RowText{owner:Entity,y:f32,padding:f32,minimum:f32}
#[derive(Component)]pub(super) struct RowBackground;

pub(super) fn grow(texts:Query<(&Node,&RowText)>,mut styles:Query<&mut Style>){
    let mut heights=std::collections::HashMap::<Entity,f32>::new();for(node,text)in &texts{let h=(text.y+node.size().y+text.padding).max(text.minimum);heights.entry(text.owner).and_modify(|v|*v=v.max(h)).or_insert(h);}
    for(owner,height)in heights{if let Ok(mut style)=styles.get_mut(owner){let target=Val::Px(height);if style.height!=target{style.height=target;}}}
}
pub(super) fn spawn(commands:&mut Commands,parent:Entity,e:&Element,page:&str,template:&str,data:&std::collections::BTreeMap<String,String>,assets:&AssetServer,ctx:&Context,doc:&Document,choice:Option<usize>)->Entity{
    let source=doc.resolved_element(&doc.components[template]);let width=e.rect[2];let height=(source.rect[3]*width/source.rect[2].max(1.0)).max(1.0);let size=[width,height];
    let row=commands.spawn(NodeBundle{style:Style{position_type:PositionType::Relative,top:Val::Px(0.0),width:Val::Percent(100.0),height:Val::Px(height),flex_shrink:0.0,..default()},..default()}).id();commands.entity(parent).add_child(row);
    if let Some(index)=choice{commands.entity(row).insert((Button,Interaction::None,crate::components::ChoiceButton(index)));}
    for(index,mut child)in menu_layout::item(doc,template,size,data,assets,ctx).into_iter().enumerate(){
        // The list and its ancestors are outside the component's own layout
        // pass. Their opacity still applies to every pixel of the instance.
        child.inherit_rendered_parent(e);
        child.action=Action::None;if child.kind==Kind::Button{child.kind=Kind::Panel;}let dynamic=matches!(child.binding.as_deref(),Some("choice.text"|"history.text"|"history.name"));if dynamic&&child.text.is_empty(){continue;}
        let Some(entity)=spawn_element(commands,row,&child,page,size,size,assets,ctx,doc,0)else{continue};
        if index==0{commands.entity(entity).insert((RowBackground,Style{position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),border:UiRect::all(Val::Px(child.appearance.border_width)),..default()}));if let Some(index)=choice{commands.entity(entity).insert((narrative::ChoiceVisual(index),narrative::ChoiceAppearance::from_element(&child)));}}
        if dynamic{commands.entity(entity).insert((RowText{owner:row,y:child.rect[1],padding:14.0*width/source.rect[2].max(1.0),minimum:height},Style{position_type:PositionType::Absolute,left:Val::Px(child.rect[0]),top:Val::Px(child.rect[1]),width:Val::Px(child.rect[2]),height:Val::Auto,min_height:Val::Px(child.rect[3]),..default()}));}
    }row
}
