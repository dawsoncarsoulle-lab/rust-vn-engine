//! Runtime-only UI animation tracks; no document or story writes.
use super::*;
use bevy::color::Alpha as ColorAlpha;
use rvn_ui::{AnimationClip,AnimationKind};
#[derive(Component)]pub(crate) struct VisualNode{pub page:String,pub id:String,pub scale:f32}
#[derive(Component,Default,Clone,PartialEq)]pub(super) struct Tracks(std::collections::BTreeMap<AnimationKind,(AnimationClip,f32)>);
#[derive(Resource,Default)]pub(super) struct RuntimeAnimations{modified:Option<std::time::SystemTime>,running:std::collections::BTreeMap<(String,String),Tracks>}
#[derive(Clone,Default)]struct Alpha{base:f32,last:f32,ready:bool}
impl Alpha{fn apply(&mut self,color:&mut Color,factor:f32){let now=color.alpha();if !self.ready||(now-self.last).abs()>0.00001{self.base=now;self.ready=true;}self.last=self.base*factor;*color=color.with_alpha(self.last);}}
#[derive(Component,Default)]pub(super) struct PaintMemo{bg:Alpha,border:Alpha,image:Alpha,text:Vec<Alpha>}
pub(super) fn start(mut commands:Commands,mut menus:ResMut<Menus>,mut runtime:ResMut<RuntimeAnimations>,time:Res<Time>,nodes:Query<(Entity,&VisualNode)>,tracks:Query<&Tracks>){
    if runtime.modified!=menus.modified{runtime.running.clear();runtime.modified=menus.modified;}
    for(page,id,clip)in std::mem::take(&mut menus.animation_requests){runtime.running.entry((page,id)).or_default().0.insert(clip.kind,(clip,time.elapsed_seconds()));}
    for(entity,node)in &nodes{if let Some(active)=runtime.running.get(&(node.page.clone(),node.id.clone())){if tracks.get(entity).ok()!=Some(active){commands.entity(entity).insert(active.clone());}}}
}
pub(super) fn transform(time:Res<Time>,mut nodes:Query<(&VisualNode,&Tracks,&mut Transform,Option<&mut BackgroundColor>)>){
    for(node,tracks,mut transform,mut bg)in &mut nodes{for(kind,(clip,start))in &tracks.0{let value=clip.sample(time.elapsed_seconds()-start);match kind{AnimationKind::Move=>{transform.translation.x+=value[0]*node.scale;transform.translation.y+=value[1]*node.scale;},AnimationKind::Scale=>transform.scale=Vec3::splat(value[0]),AnimationKind::Color=>{if let Some(bg)=&mut bg{bg.0=color(value);}},AnimationKind::Fade=>{}}}}
}
#[cfg(test)]mod tests{use super::*;#[test]fn fade_does_not_accumulate_and_respects_new_style(){let mut alpha=Alpha::default();let mut c=Color::srgba(1.0,0.0,0.0,0.8);for _ in 0..20{alpha.apply(&mut c,0.5);assert!((c.alpha()-0.4).abs()<0.0001);}c=Color::srgba(0.0,1.0,0.0,0.6);alpha.apply(&mut c,0.5);assert!((c.alpha()-0.3).abs()<0.0001);alpha.apply(&mut c,1.0);assert!((c.alpha()-0.6).abs()<0.0001);}}
pub(super) fn fade(mut commands:Commands,time:Res<Time>,parents:Query<&Parent>,tracks:Query<&Tracks>,shadows:Query<&super::shadows::ShadowOf>,mut nodes:Query<(Entity,Option<&mut BackgroundColor>,Option<&mut BorderColor>,Option<&mut UiImage>,Option<&mut Text>,Option<&mut PaintMemo>),With<Node>>){
    for(entity,bg,border,image,text,memo)in &mut nodes{let mut factor=1.0;let mut ancestor=shadows.get(entity).map(|s|s.target).unwrap_or(entity);for _ in 0..64{if let Ok(t)=tracks.get(ancestor){if let Some((clip,start))=t.0.get(&AnimationKind::Fade){factor*=clip.sample(time.elapsed_seconds()-start)[0];}}let Ok(parent)=parents.get(ancestor)else{break};ancestor=parent.get();}
        if factor==1.0&&memo.is_none(){continue;}
        let mut fresh=PaintMemo::default();let mut memo=memo;let paint=memo.as_deref_mut().unwrap_or(&mut fresh);
        if let Some(mut bg)=bg{paint.bg.apply(&mut bg.0,factor);}if let Some(mut border)=border{paint.border.apply(&mut border.0,factor);}if let Some(mut image)=image{paint.image.apply(&mut image.color,factor);}if let Some(mut text)=text{paint.text.resize_with(text.sections.len(),Alpha::default);for(section,alpha)in text.sections.iter_mut().zip(&mut paint.text){alpha.apply(&mut section.style.color,factor);}}
        if memo.is_none(){commands.entity(entity).insert(fresh);}
    }
}
