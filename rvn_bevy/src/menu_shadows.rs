//! Shadows are presentation-only siblings, following the target's final transform.
use super::*;
#[derive(Component)]pub(super) struct ShadowOf{pub target:Entity,pub offset:Vec2,pub expansion:Vec2}
pub(super) fn spawn(commands:&mut Commands,parent:Entity,target:Entity,e:&Element,rect:[f32;4]){
    for(r,radius,rgba)in rvn_ui::shadow_layers(rect,e.appearance.radius,&e.appearance.shadow){
        let shadow=commands.spawn((ShadowOf{target,offset:Vec2::from_array(e.appearance.shadow.offset),expansion:Vec2::new(r[2]-rect[2],r[3]-rect[3])},NodeBundle{
            style:Style{position_type:PositionType::Absolute,left:Val::Px(r[0]),top:Val::Px(r[1]),width:Val::Px(r[2]),height:Val::Px(r[3]),..default()},
            background_color:color(rgba).into(),border_radius:BorderRadius::all(Val::Px(radius)),..default()
        })).id();commands.entity(parent).add_child(shadow);
    }
}
pub(super) fn follow(targets:Query<(&GlobalTransform,&Node),Without<ShadowOf>>,mut shadows:Query<(&ShadowOf,&mut GlobalTransform,&mut Style)>){
    for(shadow,mut transform,mut style)in &mut shadows{if let Ok((target,node))=targets.get(shadow.target){
        *transform=target.mul_transform(Transform::from_translation(shadow.offset.extend(0.0)));
        // Dynamic history/choice cards grow after text measurement. Their
        // decorative sibling must track the measured size, not the template.
        let size=node.size()+shadow.expansion;
        if style.width!=Val::Px(size.x){style.width=Val::Px(size.x);}
        if style.height!=Val::Px(size.y){style.height=Val::Px(size.y);}
    }}
}
