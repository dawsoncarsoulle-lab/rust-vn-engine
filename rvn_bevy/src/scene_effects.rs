//! ECS presentation of neutral Core effect descriptions. Tokens carry the
//! exact task revision/target incarnation; entities never enter a save.
use bevy::prelude::*;
use crate::{components::*,resources::{VnEngine,VnState}};
use rvn_core::scene_resources::{EffectParameters,EffectProjectionBase,ResourceTask,ResourceToken,SavedEffect,SpriteEffectKind,TargetKind,TransformSnapshot,VisualTransition};

#[derive(Component,Clone,Debug,PartialEq,Eq)]
pub(crate) struct AnimationLease(pub ResourceToken);
#[derive(Component,Clone,Debug,PartialEq,Eq)]
pub(crate) struct FadeLease(pub ResourceToken);
#[derive(Component,Clone,Copy)]
pub(crate) struct OwnedFadeBase {pub transform:Transform,pub alpha:f32}
#[derive(Component,Clone,Debug,PartialEq,Eq)]
pub(crate) struct AudioLease(pub ResourceToken);
#[derive(Resource,Default)]
struct ActiveEffects { epoch:u64, tasks:Vec<ResourceTask> }
#[derive(Resource,Default)]
struct AppliedStyles(std::collections::HashMap<Entity,rvn_core::sprite_effects::SpriteStyle>);

pub(crate) struct SceneEffectsPlugin;
impl Plugin for SceneEffectsPlugin {
    fn build(&self,app:&mut App) {
        app.init_resource::<ActiveEffects>().init_resource::<AppliedStyles>().add_systems(Update,((
            reconcile.run_if(crate::scene_publication::projection_story_ready)
                .after(crate::scene_publication::HostPublicationSet)
                .after(crate::systems::sprite_system).after(crate::systems::background_system)
                .after(crate::systems::background_cover_resize_system)
                .after(crate::systems::audio_system).after(crate::systems::cinematic_system)
                .after(crate::systems::cinematic_cover_resize_system).after(crate::layered_characters::synchronize)
                .before(crate::programmable_ui::InterfaceInputSet).before(crate::systems::player_input_system),
        )).distributive_run_if(crate::runtime_restart::presentation_available));
    }
}

pub(crate) fn install(world:&mut World,tasks:Vec<ResourceTask>,epoch:u64) {
    let accepts=|token:&ResourceToken|token.task.runtime_epoch==epoch && tasks.iter().any(|task|
        task.key==token.task.key && task.revision==token.task.revision && task.target==token.target);
    let entities:Vec<_>=world.query_filtered::<Entity,Without<crate::scene_retired::RetiredProjection>>().iter(world).collect();
    for entity in entities {
        if world.get::<AnimationLease>(entity).is_some_and(|lease|!accepts(&lease.0)) {
            if let Some(base)=world.get::<SpriteBaseTransform>(entity).copied() {
                if let Some(mut transform)=world.get_mut::<Transform>(entity) {transform.translation=base.translation;transform.scale=base.scale;}
            }
            world.entity_mut(entity).remove::<(AnimationLease,SpriteAnimation)>();
        }
        if world.get::<FadeLease>(entity).is_some_and(|lease|!accepts(&lease.0)) {
            restore_visual_base(world,entity);
            world.entity_mut(entity).remove::<(FadeLease,OwnedFadeBase,FadeAnim,crate::layered_characters::LayeredFade)>();
        }
        if world.get::<AudioLease>(entity).is_some_and(|lease|!accepts(&lease.0)) {
            restore_audio_volume(world,entity);
            world.entity_mut(entity).remove::<(AudioLease,AudioFade)>();
        }
    }
    world.insert_resource(ActiveEffects{epoch,tasks});
}
fn restore_visual_base(world:&mut World,entity:Entity) {
    let mut base=world.get::<OwnedFadeBase>(entity).map(|base|base.transform)
        .or_else(||world.get::<crate::layered_characters::LayeredFade>(entity).map(|fade|fade.base));
    if let Some(current)=world.get::<SpriteBaseTransform>(entity) {
        let pose=base.get_or_insert_with(||world.get::<Transform>(entity).copied().unwrap_or_default());
        pose.translation=current.translation;pose.scale=current.scale;
    }
    if let Some(base)=base {if let Some(mut pose)=world.get_mut::<Transform>(entity) {*pose=base;}}
    let alpha=game_visual_alpha(world,entity);
    if let Some(mut sprite)=world.get_mut::<Sprite>(entity) {sprite.color.set_alpha(alpha);}
}
fn actual_visual_state(world:&World)->Option<(&rvn_core::GameState,u64)>{
 match(world.get_resource::<VnEngine>(),world.get_resource::<crate::debugger_startup::DebugBootstrap>()){
  (Some(game),None)if !game.0.renderer.scene_batch_pending()&&!game.0.renderer.scene_frame_failed=>Some((&game.0.state,game.0.runtime_epoch())),
  (None,Some(startup))if !startup.publication_pending&&startup.failed.is_none()=>Some((startup.owner.state(),startup.owner.snapshot().epoch)),
  _=>None,
 }
}
fn game_visual_alpha(world:&World,entity:Entity)->f32 {
    if let Some(sprite)=world.get::<VnSprite>(entity) {
        return actual_visual_state(world).and_then(|(state,_)|state.sprite_styles())
            .and_then(|styles|styles.get(&sprite.id)).and_then(|style|style.tint.as_ref())
            .and_then(|tint|Srgba::hex(tint.trim_start_matches('#')).ok()).map(|tint|tint.alpha).unwrap_or(1.0);
    }
    world.get::<crate::layered_characters::LayerTarget>(entity).map(|layer|layer.description.opacity).unwrap_or(1.0)
}
fn restore_audio_volume(world:&World,entity:Entity) {
    if !world.get_resource::<crate::resources::MusicEntity>().is_some_and(|music|music.0==Some(entity)) {return;}
    if let Some(sink)=world.get::<AudioSink>(entity) {
        let gain=if let Some(engine)=world.get_resource::<VnEngine>().filter(|engine|engine.0.state.audio_resource_registry().is_some()) {
            let Ok(gain)=crate::systems::audio::current_music_gain_from_state(&engine.0.state,engine.0.runtime_epoch()).and_then(|logical|crate::music_mix::in_world(world,logical)) else {return;};
            Some(gain)
        } else {world.get_resource::<crate::resources::MusicVolume>().map(|volume|volume.0)};
        if let Some(gain)=gain {sink.set_volume(gain);}
    }
}

pub(crate) fn allowed(engine:Option<&VnEngine>,state:Option<&State<VnState>>,token:Option<&ResourceToken>)->bool {
    let Some(token)=token else {return true;};
    let Some(engine)=engine else {return false;};
    if engine.0.renderer.scene_batch_pending() {return false;}
    let Some(resources)=engine.0.state.effect_resource_registry(token.target.kind) else {return false;};
    let active=engine.0.state.scene_execution.as_ref().and_then(|execution|execution.owner());
    if !resources.accepts(token,engine.0.runtime_epoch(),active) {return false;}
    match &token.task.key.owner {
        rvn_core::scene_tasks::TaskOwner::Host{..}=>true,
        _=>state.is_some_and(|state|matches!(state.get(),VnState::Waiting|VnState::Stepping|VnState::Animating)),
    }
}

fn entity_for(world:&mut World,task:&ResourceTask)->Option<Entity> {
    match task.target.kind {
        TargetKind::Sprite=>world.query_filtered::<(Entity,&VnSprite),Without<crate::scene_retired::RetiredProjection>>().iter(world).find(|(_,sprite)|sprite.id==task.target.alias).map(|(entity,_)|entity),
        TargetKind::Layer=>world.query_filtered::<(Entity,&crate::layered_characters::LayerTarget),Without<crate::scene_retired::RetiredProjection>>().iter(world)
            .find(|(_,layer)|format!("{}/{}",layer.character,layer.layer)==task.target.alias).map(|(entity,_)|entity),
        TargetKind::Background=>world.query_filtered::<Entity,(With<CurrentMotionBackground>,Without<crate::scene_retired::RetiredProjection>)>().iter(world).next(),
        TargetKind::Cinematic=>world.query_filtered::<(Entity,&crate::systems::cinematic::CurrentCinematic),Without<crate::scene_retired::RetiredProjection>>().iter(world)
            .find(|(_,current)|current.target==task.target && world.get_resource::<VnEngine>().is_some_and(|game|current.epoch==game.0.runtime_epoch() && current.instance_id==game.0.instance_id())).map(|(entity,_)|entity),
        TargetKind::Music=>world.get_resource::<crate::resources::MusicEntity>().and_then(|music|music.0).filter(|entity|world.get_entity(*entity).is_some() && world.get::<crate::scene_retired::RetiredProjection>(*entity).is_none()),
        _=>None,
    }
}
fn snapshot(transform:&Transform)->TransformSnapshot {
    TransformSnapshot{translation:transform.translation.to_array(),scale:transform.scale.to_array(),rotation:transform.rotation.to_array()}
}
fn transform(base:&TransformSnapshot)->Transform {
    Transform{translation:Vec3::from_array(base.translation),scale:Vec3::from_array(base.scale),rotation:Quat::from_array(base.rotation)}
}
fn rebase_live_visual(world:&mut World,entity:Entity) {
    let Some(current)=world.get::<SpriteBaseTransform>(entity).copied() else {return;};
    let Some(mut base)=world.get::<OwnedFadeBase>(entity).copied() else {return;};
    base.transform.translation=current.translation;base.transform.scale=current.scale;base.alpha=game_visual_alpha(world,entity);
    if let Some(sprite)=world.get::<VnSprite>(entity) {
        if let Some(style)=actual_visual_state(world).and_then(|(state,_)|state.sprite_styles()).and_then(|styles|styles.get(&sprite.id)) {base.transform.rotation=Quat::from_rotation_z(style.rotation.to_radians());}
    }
    let animation=world.get::<FadeAnim>(entity).map(|fade|FadeAnim{from:fade.from,to:fade.to,duration_secs:fade.duration_secs,elapsed_secs:fade.elapsed_secs,despawn_on_finish:fade.despawn_on_finish,kind:fade.kind});
    world.entity_mut(entity).insert(base);
    if let Some(animation)=animation {
        let t=if animation.duration_secs<=0.0 {1.0}else{(animation.elapsed_secs/animation.duration_secs).clamp(0.0,1.0)};
        if let Some(mut pose)=world.get_mut::<Transform>(entity) {apply_visual_pose(&animation,&base.transform,&mut pose,t);}
        if let Some(mut sprite)=world.get_mut::<Sprite>(entity) {sprite.color.set_alpha(base.alpha*(animation.from+(animation.to-animation.from)*t));}
    }
}

// Borrow the one actual owner. A host preference projects onto the existing
// player only; it cannot advance Core, replace a Source or authenticate a new
// broker. Startup and Game share the same validated sampled-gain path.
fn current_music_mix(world:&World)->Result<Option<f32>,String> {
    let actual=match (world.get_resource::<VnEngine>(),world.get_resource::<crate::debugger_startup::DebugBootstrap>()) {
        (Some(game),None) if !game.0.renderer.scene_batch_pending() && !game.0.renderer.scene_frame_failed
            =>Some((&game.0.state,game.0.runtime_epoch())),
        (None,Some(startup)) if !startup.publication_pending && startup.failed.is_none() && startup.accepts_actual_audio_broker()
            =>Some((startup.owner.state(),startup.owner.snapshot().epoch)),
        _=>None,
    };
    let Some((state,epoch))=actual.filter(|(state,_)|state.audio_resource_registry().is_some()) else{return Ok(None);};
    crate::systems::audio::current_music_gain_from_state(state,epoch)
        .and_then(|logical|crate::music_mix::in_world(world,logical)).map(Some)
}

fn reconcile(world:&mut World) {
    if let Some(game)=world.get_resource::<VnEngine>().filter(|game|game.0.state.scene_execution.is_some()) {
        if let Err(error)=crate::systems::cinematic::preflight_cg(world,&game.0.state) {failure(world,error);return;}
    }
    apply_styles(world);
    let mixed = match current_music_mix(world) {Ok(gain)=>gain,Err(error)=>{failure(world,error);return;}};
    if let Some(gain)=mixed {
        if let Some(entity)=world.get_resource::<crate::resources::MusicEntity>().and_then(|music|music.0).filter(|entity|world.get_entity(*entity).is_some() && world.get::<crate::scene_retired::RetiredProjection>(*entity).is_none()) {
            if let Some(mut settings)=world.get_mut::<PlaybackSettings>(entity) {settings.volume=bevy::audio::Volume::new(gain);}
            if let Some(sink)=world.get::<AudioSink>(entity) {sink.set_volume(gain);}
        }
    }
    let (epoch,tasks)={let effects=world.resource::<ActiveEffects>();(effects.epoch,effects.tasks.clone())};
    for task in tasks {
        let Some(mut effect)=task.effect.clone() else {continue;};
        let Some(entity)=entity_for(world,&task) else {continue;};
        let token=ResourceToken{task:rvn_core::scene_tasks::TaskToken{runtime_epoch:epoch,key:task.key.clone(),revision:task.revision},target:task.target.clone()};
        // The same exact lease receives the new Core elapsed too. It is a
        // projection update, not a second ECS clock or a new operation.
        let mut current=world.get::<Transform>(entity).copied().unwrap_or_default();
        if let Some(base)=world.get::<SpriteBaseTransform>(entity) {current.translation=base.translation;current.scale=base.scale;}
        match &mut effect.parameters {
            EffectParameters::SpriteAnimation{base,..}|EffectParameters::VisualFade{base,..}=>{
                if !effect.base_resolved {*base=snapshot(&current);}
                if world.get::<crate::systems::cinematic::CurrentCinematic>(entity).is_some() {
                    // Always derive cover from selected asset + current viewport, never yesterday's fade pose.
                    if let Some(neutral)=world.get::<OwnedFadeBase>(entity) {*base=snapshot(&neutral.transform);}
                }
                // A child may Move the shared Game object; its current base
                // placement wins over the parked visit's old pixel position.
                if world.get::<VnSprite>(entity).is_some() {base.translation=current.translation.to_array();base.scale=current.scale.to_array();base.rotation=current.rotation.to_array();}
            }
            _=>{}
        }
        effect.base_resolved=true;
        let clock=&effect.clock;
        match effect.parameters {
            EffectParameters::SpriteAnimation{animation,base}=>{
                let pose=transform(&base);
                let kind=match animation {SpriteEffectKind::Shake{intensity}=>AnimationKind::Shake{intensity},SpriteEffectKind::Bounce{height}=>AnimationKind::Bounce{height},SpriteEffectKind::Pulse{scale}=>AnimationKind::Pulse{scale}};
                let base=SpriteBaseTransform{translation:pose.translation,scale:pose.scale};
                let animation=SpriteAnimation{kind,duration_secs:clock.duration as f32,elapsed_secs:clock.elapsed as f32,looping:clock.looping};
                world.entity_mut(entity).insert((base,animation,AnimationLease(token)));
            }
            EffectParameters::VisualFade{from,to,despawn_on_finish,transition,base,layered}=>{
                let animation=FadeAnim{from,to,despawn_on_finish,kind:transition_into(transition),duration_secs:clock.duration as f32,elapsed_secs:clock.elapsed as f32};
                let base=transform(&base);
                let base_alpha=game_visual_alpha(world,entity);
                if layered {world.entity_mut(entity).insert(crate::layered_characters::LayeredFade{animation,base});} else {world.entity_mut(entity).insert((animation,OwnedFadeBase{transform:base,alpha:base_alpha}));}
                world.entity_mut(entity).insert(FadeLease(token));
            }
            EffectParameters::AudioFade{from,to,despawn_on_done}=>{
                world.entity_mut(entity).insert((AudioFade{from_vol:from,to_vol:to,despawn_on_done,duration_secs:clock.duration as f32,elapsed_secs:clock.elapsed as f32},AudioLease(token)));
                if let Some(sink)=world.get::<AudioSink>(entity) {
                    let t=effect_progress(clock);
                    let logical=(f64::from(from)+(f64::from(to)-f64::from(from))*t) as f32;
                    if let Ok(gain)=crate::music_mix::in_world(world,logical) {sink.set_volume(gain);}
                }
            }
        }
    }
    sample_owned_effects(world);
    crate::layered_characters::sample_owned(world);
    sample_owned_layer_fades(world);
}
// A LayeredFade can address a leaf as well as a CompositionRoot. Reset its
// neutral root color before its own fade, then let PostUpdate Motion compose.
// Never reuse a leaf color or transform already sampled on the previous frame.
fn sample_owned_layer_fades(world:&mut World) {
    let reduced=world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access|access.settings.reduced_motion);
    let leaves:Vec<_>=world.query_filtered::<Entity,(With<crate::layered_characters::LayerTarget>,With<crate::layered_characters::LayeredFade>,With<FadeLease>,Without<crate::scene_retired::RetiredProjection>)>().iter(world).collect();
    for entity in leaves {
        let Some(lease)=world.get::<FadeLease>(entity) else {continue;};
        let Some(effect)=authoritative_effect(world,&lease.0) else {continue;};
        let Some(parent)=world.get::<Parent>(entity).map(Parent::get) else {continue;};
        let Some(root)=world.get::<crate::layered_characters::CompositionRoot>(parent) else {continue;};
        let tint=root.tint.to_srgba();
        let opacity=world.get::<crate::layered_characters::LayerTarget>(entity).unwrap().description.opacity;
        let root_factor=if let Some(fade)=world.get::<crate::layered_characters::LayeredFade>(parent) {
            let phase=if reduced {1.0} else if let Some(lease)=world.get::<FadeLease>(parent) {
                let Some(effect)=authoritative_effect(world,&lease.0) else {continue;};
                effect_progress(&effect.clock)
            } else {f64::from((fade.animation.elapsed_secs/fade.animation.duration_secs.max(0.001)).clamp(0.0,1.0))};
            if world.get::<FadeLease>(parent).is_some() {
                (f64::from(fade.animation.from)+(f64::from(fade.animation.to)-f64::from(fade.animation.from))*phase) as f32
            } else {fade.animation.from+(fade.animation.to-fade.animation.from)*phase as f32}
        } else {1.0};
        let fade=world.get::<crate::layered_characters::LayeredFade>(entity).unwrap();
        let mut pose=fade.base;
        let (from,to,kind)=(fade.animation.from,fade.animation.to,fade.animation.kind);
        if let (Some(animation),Some(base),Some(lease))=(world.get::<SpriteAnimation>(entity),world.get::<SpriteBaseTransform>(entity),world.get::<AnimationLease>(entity)) {
            if let Some(effect)=authoritative_effect(world,&lease.0) {
                crate::systems::sprite::sample_owned_sprite_animation(animation,base,&mut pose,reduced,effect.clock.elapsed,effect.clock.duration);
            }
        }
        let phase=if reduced {1.0}else{effect_progress(&effect.clock)};
        let animated=pose;
        let animation=FadeAnim{from,to,kind,duration_secs:0.0,elapsed_secs:0.0,despawn_on_finish:false};
        apply_owned_visual_pose(&animation,&animated,&mut pose,phase);
        let neutral_alpha=tint.alpha*opacity*root_factor;
        let factor=f64::from(from)+(f64::from(to)-f64::from(from))*phase;
        if let Some(mut sprite)=world.get_mut::<Sprite>(entity) {
            sprite.color=Color::srgba(tint.red,tint.green,tint.blue,(f64::from(neutral_alpha)*factor) as f32);
        }
        if let Some(mut actual)=world.get_mut::<Transform>(entity) {*actual=pose;}
    }
}
fn sample_owned_effects(world:&mut World) {
    let reduced=world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access|access.settings.reduced_motion);
    let entities:Vec<_>=world.query_filtered::<Entity,(Or<(With<AnimationLease>,With<FadeLease>)>,Without<crate::scene_retired::RetiredProjection>)>().iter(world).collect();
    for entity in entities {
        let mut pose=world.get::<OwnedFadeBase>(entity).map(|base|base.transform)
            .unwrap_or_else(||world.get::<Transform>(entity).copied().unwrap_or_default());
        if let (Some(animation),Some(base),Some(lease))=(world.get::<SpriteAnimation>(entity),world.get::<SpriteBaseTransform>(entity),world.get::<AnimationLease>(entity)) {
            if let Some(effect)=authoritative_effect(world,&lease.0) {
                crate::systems::sprite::sample_owned_sprite_animation(animation,base,&mut pose,reduced,effect.clock.elapsed,effect.clock.duration);
            }
        }
        if let (Some(fade),Some(base),Some(lease))=(world.get::<FadeAnim>(entity),world.get::<OwnedFadeBase>(entity),world.get::<FadeLease>(entity)) {
            // Fade composes with this frame's animation pose, while the saved
            // projection base remains the unanimated Game placement.
            let Some(effect)=authoritative_effect(world,&lease.0) else {continue;};
            let t=if reduced {1.0}else{effect_progress(&effect.clock)};
            let from=fade.from;let to=fade.to;let alpha=base.alpha;
            let animated=pose;
            apply_owned_visual_pose(fade,&animated,&mut pose,t);
            if let Some(mut sprite)=world.get_mut::<Sprite>(entity) {sprite.color.set_alpha((f64::from(alpha)*(f64::from(from)+(f64::from(to)-f64::from(from))*t)) as f32);}
        }
        if world.get::<crate::layered_characters::LayeredFade>(entity).is_none() {
            if let Some(mut actual)=world.get_mut::<Transform>(entity) {*actual=pose;}
        }
    }
}
fn apply_styles(world:&mut World) {
    let Some(styles)=actual_visual_state(world).and_then(|(state,_)|state.sprite_styles()).cloned() else {return;};
    let sprites:Vec<_>=world.query_filtered::<(Entity,&VnSprite),Without<crate::scene_retired::RetiredProjection>>().iter(world).map(|(entity,sprite)|(entity,sprite.id.clone())).collect();
    let mut applied=world.remove_resource::<AppliedStyles>().unwrap_or_default();
    applied.0.retain(|entity,_|world.get_entity(*entity).is_some());
    for (entity,id) in sprites {
        let style=styles.get(&id).cloned().unwrap_or_default();
        if applied.0.get(&entity)==Some(&style) {continue;}
        let scale=Vec3::new(if style.flip_x {-style.scale} else {style.scale},if style.flip_y {-style.scale} else {style.scale},1.0);
        if let Some(mut transform)=world.get_mut::<Transform>(entity) {crate::systems::sprite::apply_sprite_transform_style(&mut transform,&style);}
        if let Some(mut base)=world.get_mut::<SpriteBaseTransform>(entity) {base.scale=scale;}
        let tint=style.tint.as_ref().and_then(|tint|Srgba::hex(tint.trim_start_matches('#')).ok()).map(Color::from).unwrap_or(Color::WHITE);
        let factor=world.get::<FadeAnim>(entity).map(|fade|{let t=if fade.duration_secs<=0.0 {1.0} else {(fade.elapsed_secs/fade.duration_secs).clamp(0.0,1.0)};fade.from+(fade.to-fade.from)*t}).unwrap_or(1.0);
        if let Some(mut sprite)=world.get_mut::<Sprite>(entity) {sprite.color=tint;sprite.color.set_alpha(tint.alpha()*factor);}
        if let Some(mut base)=world.get_mut::<OwnedFadeBase>(entity) {base.alpha=tint.alpha();}
        if let Some(mut root)=world.get_mut::<crate::layered_characters::CompositionRoot>(entity) {root.tint=tint;}
        applied.0.insert(entity,style);
    }
    world.insert_resource(applied);
}
#[cfg(test)]
pub(crate) fn apply_styles_for_test(world:&mut World) {apply_styles(world);}
#[cfg(test)]
pub(crate) fn reconcile_for_test(world:&mut World) {reconcile(world);}

pub(crate) fn authoritative_effect(world:&World,token:&ResourceToken)->Option<SavedEffect> {
    let(state,epoch)=actual_visual_state(world)?;
    let resources=state.effect_resource_registry(token.target.kind)?;
    let active=state.scene_execution.as_ref().and_then(|execution|execution.owner());
    if !resources.accepts(token,epoch,active) {return None;}
    resources.tasks.iter().find(|task|task.key==token.task.key && task.revision==token.task.revision && task.target==token.target)?.effect.clone()
}
pub(crate) fn capture_bases(world:&mut World)->Result<(),String> {
    let mut captures=Vec::new();
    for (entity,lease) in world.query_filtered::<(Entity,&AnimationLease),Without<crate::scene_retired::RetiredProjection>>().iter(world) {
        if world.get::<SpriteAnimation>(entity).is_some() && authoritative_effect(world,&lease.0).is_some() {
            let mut pose=world.get::<Transform>(entity).copied().unwrap_or_default();
            if let Some(original)=world.get::<SpriteBaseTransform>(entity) {pose.translation=original.translation;pose.scale=original.scale;}
            captures.push((lease.0.clone(),EffectProjectionBase::SpriteTransform(snapshot(&pose))));
        }
    }
    for (entity,lease) in world.query_filtered::<(Entity,&FadeLease),Without<crate::scene_retired::RetiredProjection>>().iter(world) {
        let layered=world.get::<crate::layered_characters::LayeredFade>(entity);
        let animation=layered.map(|fade|&fade.animation).or_else(||world.get::<FadeAnim>(entity));
        if animation.is_some() && authoritative_effect(world,&lease.0).is_some() {
            let mut pose=layered.map(|fade|fade.base).or_else(||world.get::<OwnedFadeBase>(entity).map(|base|base.transform)).unwrap_or_else(||world.get::<Transform>(entity).copied().unwrap_or_default());
            if let Some(original)=world.get::<SpriteBaseTransform>(entity) {pose.translation=original.translation;pose.scale=original.scale;}
            captures.push((lease.0.clone(),EffectProjectionBase::VisualTransform(snapshot(&pose))));
        }
    }
    for (entity,lease) in world.query_filtered::<(Entity,&AudioLease),Without<crate::scene_retired::RetiredProjection>>().iter(world) {
        if world.get::<AudioFade>(entity).is_some() && authoritative_effect(world,&lease.0).is_some() {
            captures.push((lease.0.clone(),EffectProjectionBase::AudioResolved));
        }
    }
    let state_before=world.resource::<VnEngine>().0.state.clone();
    for (token,base) in captures {
        let result=world.resource_mut::<VnEngine>().0.capture_scene_effect_base(&token,base);
        if let Err(error)=result {
            world.resource_mut::<VnEngine>().0.state=state_before;
            return Err(error.to_string());
        }
    }
    Ok(())
}
pub(crate) fn apply_visual_pose(animation:&FadeAnim,base:&Transform,pose:&mut Transform,t:f32) {
    *pose=*base;
    let progress=if animation.to>animation.from {1.0-t} else {t};
    match animation.kind {
        TransitionKind::SlideLeft=>pose.translation.x+=1920.0*progress,
        TransitionKind::SlideRight=>pose.translation.x-=1920.0*progress,
        TransitionKind::SlideUp=>pose.translation.y-=1920.0*progress,
        TransitionKind::SlideDown=>pose.translation.y+=1920.0*progress,
        TransitionKind::ZoomIn=>pose.scale*=1.0-0.5*progress,
        TransitionKind::ZoomOut=>pose.scale*=1.0+0.5*progress,
        _=>{}
    }
}
pub(crate) fn effect_progress(clock:&rvn_core::scene_resources::EffectClock)->f64 {
    if clock.duration<=0.0 {1.0}else{(clock.elapsed/clock.duration).clamp(0.0,1.0)}
}
pub(crate) fn apply_owned_visual_pose(animation:&FadeAnim,base:&Transform,pose:&mut Transform,t:f64) {
    *pose=*base;
    let progress=if animation.to>animation.from {1.0-t}else{t};
    match animation.kind {
        TransitionKind::SlideLeft=>pose.translation.x+=(1920.0*progress) as f32,
        TransitionKind::SlideRight=>pose.translation.x-=(1920.0*progress) as f32,
        TransitionKind::SlideUp=>pose.translation.y-=(1920.0*progress) as f32,
        TransitionKind::SlideDown=>pose.translation.y+=(1920.0*progress) as f32,
        TransitionKind::ZoomIn=>pose.scale*=(1.0-0.5*progress) as f32,
        TransitionKind::ZoomOut=>pose.scale*=(1.0+0.5*progress) as f32,
        _=>{},
    }
}
fn failure(world:&mut World,error:String) {
    if let Some(mut diagnostic)=world.get_resource_mut::<crate::resources::ScriptErrorMessage>() {diagnostic.0=error;}
    if let Some(mut next)=world.get_resource_mut::<NextState<VnState>>() {next.set(VnState::Error);}
}
fn transition_into(kind:VisualTransition)->TransitionKind {match kind {VisualTransition::Fade=>TransitionKind::Fade,VisualTransition::Dissolve=>TransitionKind::Dissolve,VisualTransition::SlideLeft=>TransitionKind::SlideLeft,VisualTransition::SlideRight=>TransitionKind::SlideRight,VisualTransition::SlideUp=>TransitionKind::SlideUp,VisualTransition::SlideDown=>TransitionKind::SlideDown,VisualTransition::ZoomIn=>TransitionKind::ZoomIn,VisualTransition::ZoomOut=>TransitionKind::ZoomOut,VisualTransition::Wipe=>TransitionKind::Wipe,VisualTransition::Blur=>TransitionKind::Blur}}
fn transition_from(kind:TransitionKind)->VisualTransition {match kind {TransitionKind::Fade=>VisualTransition::Fade,TransitionKind::Dissolve=>VisualTransition::Dissolve,TransitionKind::SlideLeft=>VisualTransition::SlideLeft,TransitionKind::SlideRight=>VisualTransition::SlideRight,TransitionKind::SlideUp=>VisualTransition::SlideUp,TransitionKind::SlideDown=>VisualTransition::SlideDown,TransitionKind::ZoomIn=>VisualTransition::ZoomIn,TransitionKind::ZoomOut=>VisualTransition::ZoomOut,TransitionKind::Wipe=>VisualTransition::Wipe,TransitionKind::Blur=>VisualTransition::Blur}}

#[cfg(test)]
mod current_leaf_fade_tests {
    use super::*;
    use rvn_core::scene_resources::{EffectClock, ResourceTaskKind};
    use rvn_core::scene_tasks::TaskOwner;

    fn fixture(root_fade:bool,elapsed:f64)->(World,Entity,Entity,ResourceToken) {
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(
            "scene label A\neileen.show(\"smile\") at left\neileen.effect(tint:\"#80402066\")\n\"parent\"\nscene label B\n\"child\"\nreturn\n"
        ).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        game.step_until_interaction().unwrap();
        let definition=rvn_ui::composition::Composition::parse(serde_json::json!({
            "size":[300.0,600.0],"layers":[{"id":"body","image":"sprites/eileen/body.png","opacity":0.8}]
        })).unwrap();
        game.state.layered.compose("eileen".into(),definition.clone()).unwrap();
        rvn_core::scene_resources::sync_current(&mut game.state).unwrap();
        let owner=TaskOwner::Scene{activation:game.state.scene_execution.as_ref().unwrap().owner().unwrap()};
        let leaf_target=game.state.scene_execution.as_ref().unwrap().resources.target(&TaskOwner::Game,TargetKind::Layer,"eileen/body").unwrap().clone();
        let root_target=game.state.scene_execution.as_ref().unwrap().resources.target(&TaskOwner::Game,TargetKind::Sprite,"eileen").unwrap().clone();
        let local=Transform::from_xyz(10.0,20.0,0.01);
        let effect=SavedEffect{
            presentation:None,base_resolved:true,clock:EffectClock{duration:10.0,elapsed,looping:false},
            parameters:EffectParameters::VisualFade{from:0.0,to:1.0,despawn_on_finish:false,transition:VisualTransition::SlideLeft,base:snapshot(&local),layered:true}
        };
        let token=game.register_scene_effect(rvn_core::scene_resources::resource_key(owner.clone(),ResourceTaskKind::Effect,"layer/eileen/body/fade"),leaf_target,effect).unwrap();
        if root_fade {
            game.register_scene_effect(rvn_core::scene_resources::resource_key(owner,ResourceTaskKind::Effect,"sprite/eileen/fade"),root_target,SavedEffect{
                presentation:None,base_resolved:true,clock:EffectClock{duration:10.0,elapsed:5.0,looping:false},
                parameters:EffectParameters::VisualFade{from:0.0,to:1.0,despawn_on_finish:false,transition:VisualTransition::Fade,base:TransformSnapshot::default(),layered:true}
            }).unwrap();
        }
        let mut world=World::new();world.insert_resource(VnEngine(game));
        world.init_resource::<crate::scene_publication::ScenePresentation>();
        world.init_resource::<Events<crate::vn_command::VnCommand>>();world.init_resource::<Events<crate::vn_command::PlayerInput>>();
        world.init_resource::<crate::accessibility::Accessibility>();world.insert_resource(Time::<()>::default());world.insert_resource(State::new(VnState::Waiting));
        let base=crate::systems::sprite::sprite_default_transform(&rvn_parser::Position::Left);
        let root=world.spawn((VnSprite{id:"eileen".into()},crate::layered_characters::CompositionRoot{extent:Vec2::new(300.0,600.0),position:rvn_parser::Position::Left,tint:Color::WHITE},SpriteBaseTransform{translation:base.translation,scale:base.scale},base)).id();
        let leaf=world.spawn((crate::layered_characters::LayerTarget{character:"eileen".into(),layer:"body".into(),description:definition.layers[0].clone()},local,Sprite::default())).id();
        world.entity_mut(root).add_child(leaf);
        crate::scene_publication::consume(&mut world);
        publish(&mut world);(world,root,leaf,token)
    }
    fn publish(world:&mut World) {
        let (tasks,epoch)={let game=&world.resource::<VnEngine>().0;let execution=game.state.scene_execution.as_ref().unwrap();(execution.resources.active_effects(execution.owner()).into_iter().cloned().collect(),game.runtime_epoch())};
        install(world,tasks,epoch);reconcile(world);
    }
    #[test]
    fn leaf_fade_preserves_root_tint_opacity_and_root_fade_before_motion_without_accumulation() {
        let (mut world,_root,leaf,_)=fixture(true,2.5);
        let expected=(f64::from(Srgba::hex("80402066").unwrap().alpha*0.8*0.5)*0.25) as f32;
        let before=serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap();
        for _ in 0..8 {
            assert_eq!(world.get::<Transform>(leaf).unwrap().translation,Vec3::new(1450.0,20.0,0.01));
            let color=world.get::<Sprite>(leaf).unwrap().color.to_srgba();
            assert_eq!(color.alpha.to_bits(),expected.to_bits());
            assert_eq!([color.red,color.green,color.blue],[128.0/255.0,64.0/255.0,32.0/255.0]);
            reconcile(&mut world);
        }
        assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(),before);
    }
    #[test]
    fn leaf_fade_uses_core_f64_even_when_component_elapsed_rounds_to_the_same_f32() {
        let elapsed=f64::from(f32::from_bits(0x40200000))+0.000000116825104;
        let (mut world,_,leaf,token)=fixture(false,elapsed);
        let base_alpha=Srgba::hex("80402066").unwrap().alpha*0.8;
        let expected=(f64::from(base_alpha)*(elapsed/10.0)) as f32;
        let rounded=(f64::from(base_alpha)*(f64::from(elapsed as f32)/10.0)) as f32;
        assert_eq!(elapsed as f32,2.5);
        assert_eq!(expected.to_bits(),0x3da3d70c);
        assert_eq!(rounded.to_bits(),0x3da3d70b);
        assert_ne!(expected.to_bits(),rounded.to_bits());
        assert_eq!(world.get::<Sprite>(leaf).unwrap().color.alpha().to_bits(),expected.to_bits());
        world.get_mut::<crate::layered_characters::LayeredFade>(leaf).unwrap().animation.elapsed_secs=9.0;
        reconcile(&mut world);
        assert_eq!(world.get::<Sprite>(leaf).unwrap().color.alpha().to_bits(),expected.to_bits());
        assert_eq!(authoritative_effect(&world,&token).unwrap().clock.elapsed,elapsed);
    }
    #[test]
    fn cancelling_a_leaf_fade_restores_its_neutral_local_pose_and_current_root_color() {
        let (mut world,_,leaf,token)=fixture(false,2.5);
        rvn_core::scene_resources::supersede_task(&mut world.resource_mut::<VnEngine>().0.state,&token.task.key).unwrap();
        publish(&mut world);
        assert!(world.get::<FadeLease>(leaf).is_none() && world.get::<crate::layered_characters::LayeredFade>(leaf).is_none());
        assert_eq!(world.get::<Transform>(leaf).unwrap().translation,Vec3::new(10.0,20.0,0.01));
        assert_eq!(world.get::<Sprite>(leaf).unwrap().color.alpha().to_bits(),(Srgba::hex("80402066").unwrap().alpha*0.8).to_bits());
    }
    #[test]
    fn reduced_leaf_fade_selects_endpoint_without_advancing_the_saved_clock() {
        let (mut world,_,leaf,token)=fixture(false,2.5);
        world.resource_mut::<crate::accessibility::Accessibility>().settings.reduced_motion=true;
        reconcile(&mut world);
        assert_eq!(world.get::<Transform>(leaf).unwrap().translation,Vec3::new(10.0,20.0,0.01));
        assert_eq!(world.get::<Sprite>(leaf).unwrap().color.alpha().to_bits(),(Srgba::hex("80402066").unwrap().alpha*0.8).to_bits());
        assert_eq!(authoritative_effect(&world,&token).unwrap().clock.elapsed,2.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use rvn_core::scene_resources::EffectClock;
    fn frame(world:&mut World,seconds:f64) {
        capture_bases(world).unwrap();
        world.resource_mut::<VnEngine>().0.tick_game_frame(seconds,rvn_core::GameFrameReports::default()).unwrap();
        crate::scene_publication::consume(world);reconcile(world);
    }
    fn world(source:&str)->(World,Entity) {
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        game.step_until_interaction().unwrap();
        let mut world=World::new();world.insert_resource(VnEngine(game));
        world.init_resource::<crate::scene_publication::ScenePresentation>();
        world.init_resource::<Events<crate::vn_command::VnCommand>>();world.init_resource::<Events<crate::vn_command::PlayerInput>>();
        world.init_resource::<crate::accessibility::Accessibility>();world.insert_resource(Time::<()>::default());
        world.insert_resource(State::new(VnState::Waiting));
        let pose=crate::systems::sprite::sprite_default_transform(&rvn_parser::Position::Left);
        let entity=world.spawn((VnSprite{id:"eileen".into()},SpriteBaseTransform{translation:pose.translation,scale:pose.scale},pose,Sprite::default())).id();
        crate::scene_publication::consume(&mut world);reconcile(&mut world);capture_bases(&mut world).unwrap();
        (world,entity)
    }
    fn saved_effect(world:&World)->SavedEffect {
        world.resource::<VnEngine>().0.state.scene_execution.as_ref().unwrap().resources.tasks.iter().find_map(|task|task.effect.clone()).unwrap()
    }
    #[test]
    fn a_silent_parent_animation_is_first_projected_only_after_return_using_current_game_style() {
        let (mut world,entity)=world("scene label A\neileen.show(\"smile\") at left\neileen.animate(\"pulse\",duration:10)\ncall B\n\"parent\"\njump done\nscene label B\neileen.effect(scale:0.8,flip_x:true)\n\"child\"\nreturn\nlabel done\n");
        assert!(world.get::<SpriteAnimation>(entity).is_none());
        assert_eq!(saved_effect(&world).clock.elapsed,0.0);assert!(!saved_effect(&world).base_resolved);
        let bytes=serde_json::to_vec(&rvn_core::save::SaveData::from_state(&world.resource::<VnEngine>().0.state,1,"child".into(),"main.rvn".into())).unwrap();
        let save=serde_json::from_slice(&bytes).unwrap();
        world.resource_mut::<VnEngine>().0.load_data(save).unwrap();
        world.resource_mut::<VnEngine>().0.advance_dialogue().unwrap();world.resource_mut::<VnEngine>().0.step_until_interaction().unwrap();
        crate::scene_publication::consume(&mut world);reconcile(&mut world);
        assert_eq!(world.get::<SpriteAnimation>(entity).unwrap().elapsed_secs,0.0);
        assert_eq!(world.get::<SpriteBaseTransform>(entity).unwrap().scale,Vec3::new(-0.8,0.8,1.0));
        capture_bases(&mut world).unwrap();
        assert!(saved_effect(&world).base_resolved);
    }
    #[test]
    fn actual_ecs_clock_is_saved_paused_in_menu_parked_and_rebound_to_a_new_load_token() {
        let (mut world,entity)=world("scene label A\neileen.show(\"smile\") at left\neileen.animate(\"pulse\",duration:10,loop:true)\n\"parent\"\nscene label B\n\"child\"\nreturn\n");
        let old=world.get::<AnimationLease>(entity).unwrap().0.clone();
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
        world.run_system_once(crate::systems::sprite_animation_system);capture_bases(&mut world).unwrap();
        assert_eq!(saved_effect(&world).clock.elapsed,0.0);
        frame(&mut world,1.0);
        assert_eq!(saved_effect(&world).clock.elapsed,1.0);
        let saved_pose=*world.get::<Transform>(entity).unwrap();
        world.insert_resource(State::new(VnState::Menu));world.run_system_once(crate::systems::sprite_animation_system);capture_bases(&mut world).unwrap();
        frame(&mut world,0.0);
        assert_eq!(saved_effect(&world).clock.elapsed,1.0);
        let save=rvn_core::save::SaveData::from_state(&world.resource::<VnEngine>().0.state,1,"parent".into(),"main.rvn".into());
        assert_eq!(save.format_version,rvn_core::save::SCENE_SAVE_FORMAT_VERSION);
        world.resource_mut::<VnEngine>().0.execute_timer_action("call B").unwrap();crate::scene_publication::consume(&mut world);
        assert!(world.get::<SpriteAnimation>(entity).is_none());assert_eq!(saved_effect(&world).clock.elapsed,1.0);
        world.resource_mut::<VnEngine>().0.load_data(serde_json::from_slice(&serde_json::to_vec(&save).unwrap()).unwrap()).unwrap();
        crate::scene_publication::consume(&mut world);reconcile(&mut world);
        let fresh=world.get::<AnimationLease>(entity).unwrap().0.clone();
        assert_ne!(fresh,old);assert_eq!(world.get::<SpriteAnimation>(entity).unwrap().elapsed_secs,1.0);
        assert_eq!(*world.get::<Transform>(entity).unwrap(),saved_pose);
        assert!(!allowed(world.get_resource::<VnEngine>(),Some(&State::new(VnState::Waiting)),Some(&old)));
        world.insert_resource(State::new(VnState::Waiting));world.run_system_once(crate::systems::sprite_animation_system);capture_bases(&mut world).unwrap();
        assert_eq!(saved_effect(&world).clock.elapsed,1.0);
        frame(&mut world,1.0);
        assert_eq!(saved_effect(&world).clock.elapsed,2.0);
    }
    #[test]
    fn owned_animation_and_fade_sample_the_same_core_clock_without_losing_authored_tint_alpha() {
        let (mut world,entity)=world("scene label A\neileen.show(\"smile\") at left\neileen.effect(tint:\"#ffffff66\")\neileen.animate(\"bounce\",duration:1)\n\"line\"\n");
        world.init_resource::<NextState<VnState>>();
        world.init_resource::<crate::resources::VnRenderState>();
        let animation=world.get::<AnimationLease>(entity).unwrap().0.clone();
        let mut game=world.resource_mut::<VnEngine>();let owner=game.0.state.scene_execution.as_ref().unwrap().owner().unwrap();
        let fade=game.0.register_scene_effect(rvn_core::scene_resources::resource_key(rvn_core::scene_tasks::TaskOwner::Scene{activation:owner},rvn_core::scene_resources::ResourceTaskKind::Effect,"sprite/eileen/fade"),animation.target.clone(),SavedEffect{
            presentation:None,base_resolved:false,clock:EffectClock{duration:1.0,elapsed:0.0,looping:false},parameters:EffectParameters::VisualFade{from:0.0,to:1.0,despawn_on_finish:false,transition:VisualTransition::SlideLeft,base:TransformSnapshot::default(),layered:false}}).unwrap();
        drop(game);
        let (tasks,epoch)={let game=&world.resource::<VnEngine>().0;let execution=game.state.scene_execution.as_ref().unwrap();(execution.resources.active_effects(execution.owner()).into_iter().cloned().collect(),game.runtime_epoch())};
        install(&mut world,tasks,epoch);reconcile(&mut world);
        frame(&mut world,0.25);
        let anim=world.get::<SpriteAnimation>(entity).unwrap();let base=world.get::<SpriteBaseTransform>(entity).unwrap();
        let fade_component=world.get::<FadeAnim>(entity).unwrap();let neutral=world.get::<OwnedFadeBase>(entity).unwrap();
        assert_eq!(anim.elapsed_secs,0.25);assert_eq!(fade_component.elapsed_secs,0.25);
        let mut expected=neutral.transform;crate::systems::sprite::sample_owned_sprite_animation(anim,base,&mut expected,false,0.25,1.0);
        let animated=expected;apply_owned_visual_pose(fade_component,&animated,&mut expected,0.25);
        assert_eq!(*world.get::<Transform>(entity).unwrap(),expected);
        assert!((world.get::<Sprite>(entity).unwrap().color.alpha()-0.4*0.25).abs()<1e-6);
        let before=serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap();
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs(2));
        world.run_system_once(crate::systems::sprite_animation_system);world.run_system_once(crate::systems::fade_system);
        assert_eq!(serde_json::to_value(&world.resource::<VnEngine>().0.state).unwrap(),before);
        assert_eq!(world.get::<SpriteAnimation>(entity).unwrap().elapsed_secs,0.25);
        assert_eq!(world.get::<FadeAnim>(entity).unwrap().elapsed_secs,0.25);
        assert_eq!(world.get::<FadeLease>(entity).unwrap().0,fade);assert_eq!(*world.get::<Transform>(entity).unwrap(),expected);
    }
    #[test]
    fn owned_sampling_uses_authoritative_f64_phase_when_component_clocks_round_to_the_same_value() {
        let (mut world,entity)=world("scene label A\neileen.show(\"smile\") at left\neileen.animate(\"bounce\",duration:1)\n\"line\"\n");
        // Source AnimationValue remains f32. Exercise the exact saved Core
        // clock independently of that existing authored-precision boundary.
        let old=world.get::<AnimationLease>(entity).unwrap().0.clone();
        let mut effect=saved_effect(&world);effect.clock.duration=1.000000000001;
        world.resource_mut::<VnEngine>().0.register_scene_effect(old.task.key.clone(),old.target.clone(),effect).unwrap();
        let (tasks,epoch)={let game=&world.resource::<VnEngine>().0;let execution=game.state.scene_execution.as_ref().unwrap();(execution.resources.active_effects(execution.owner()).into_iter().cloned().collect(),game.runtime_epoch())};
        install(&mut world,tasks,epoch);reconcile(&mut world);
        {let mut base=world.get_mut::<SpriteBaseTransform>(entity).unwrap();base.translation=Vec3::ZERO;}
        world.get_mut::<Transform>(entity).unwrap().translation=Vec3::ZERO;
        frame(&mut world,1.0);
        let effect=saved_effect(&world);assert!(effect.clock.elapsed<effect.clock.duration);
        let progress=effect_progress(&effect.clock);assert!(progress<1.0);
        let animation=world.get::<SpriteAnimation>(entity).unwrap();assert_eq!(animation.elapsed_secs,animation.duration_secs);
        let pose=world.get::<Transform>(entity).unwrap();assert!(pose.translation.y>0.0);
        let mut legacy=Transform::default();crate::systems::sprite::sample_sprite_animation(animation,world.get::<SpriteBaseTransform>(entity).unwrap(),&mut legacy,false);
        assert_eq!(legacy.translation.y,0.0);
        let fade=FadeAnim{from:0.0,to:1.0,duration_secs:animation.duration_secs,elapsed_secs:animation.elapsed_secs,despawn_on_finish:false,kind:TransitionKind::SlideLeft};
        let mut sampled=Transform::default();apply_owned_visual_pose(&fade,&Transform::default(),&mut sampled,progress);
        assert!(sampled.translation.x>0.0);
        let token=world.get::<AnimationLease>(entity).unwrap().0.clone();
        assert!(!world.resource_mut::<VnEngine>().0.complete_scene_effect(&token).unwrap());
    }
    #[test]
    fn replacing_an_effect_revokes_the_actual_component_clock_before_installing_the_new_revision() {
        let (mut world,entity)=world("scene label A\neileen.show(\"smile\") at left\neileen.animate(\"shake\",duration:10)\n\"parent\"\n");
        let old=world.get::<AnimationLease>(entity).unwrap().0.clone();
        let mut effect=saved_effect(&world);effect.clock.elapsed=0.0;
        if let EffectParameters::SpriteAnimation{animation,..}=&mut effect.parameters {*animation=SpriteEffectKind::Bounce{height:30.0};}
        let new=world.resource_mut::<VnEngine>().0.register_scene_effect(old.task.key.clone(),old.target.clone(),effect).unwrap();
        assert_ne!(new.task.revision,old.task.revision);
        world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
        world.run_system_once(crate::systems::sprite_animation_system);
        assert_eq!(world.get::<SpriteAnimation>(entity).unwrap().elapsed_secs,0.0);
        let (tasks,epoch)={let game=&world.resource::<VnEngine>().0;let execution=game.state.scene_execution.as_ref().unwrap();(execution.resources.active_effects(execution.owner()).into_iter().cloned().collect(),game.runtime_epoch())};
        install(&mut world,tasks,epoch);reconcile(&mut world);
        assert_eq!(world.get::<AnimationLease>(entity).unwrap().0,new);
        assert!(matches!(world.get::<SpriteAnimation>(entity).unwrap().kind,AnimationKind::Bounce{height:30.0}));
    }
    #[test]
    fn ecs_capture_keeps_the_authoritative_f64_duration_and_authored_parameters_byte_exact() {
        let (mut world,entity)=world("scene label A\neileen.show(\"smile\") at left\neileen.animate(\"shake\",duration:10)\n\"parent\"\n");
        let old=world.get::<AnimationLease>(entity).unwrap().0.clone();
        let mut effect=saved_effect(&world);effect.clock.duration=1.0000000000000002;
        let expected=effect.clone();
        world.resource_mut::<VnEngine>().0.register_scene_effect(old.task.key.clone(),old.target.clone(),effect).unwrap();
        let (tasks,epoch)={let game=&world.resource::<VnEngine>().0;let execution=game.state.scene_execution.as_ref().unwrap();(execution.resources.active_effects(execution.owner()).into_iter().cloned().collect(),game.runtime_epoch())};
        install(&mut world,tasks,epoch);reconcile(&mut world);
        assert_ne!(world.get::<SpriteAnimation>(entity).unwrap().duration_secs as f64,expected.clock.duration);
        world.get_mut::<SpriteAnimation>(entity).unwrap().elapsed_secs=0.25;
        capture_bases(&mut world).unwrap();
        let captured=saved_effect(&world);
        assert_eq!(captured.clock.duration.to_bits(),expected.clock.duration.to_bits());
        assert_eq!(captured.clock.looping,expected.clock.looping);
        assert_eq!(captured.parameters,expected.parameters);
        assert_eq!(captured.clock.elapsed,0.0);
        assert!(!world.resource_mut::<VnEngine>().0.capture_scene_effect(&old,captured.clone()).unwrap());
        frame(&mut world,0.25);
        assert_eq!(saved_effect(&world).clock.elapsed,0.25);
        assert_eq!(world.get::<SpriteAnimation>(entity).unwrap().elapsed_secs,0.25);
    }
    #[test]
    fn cancelling_an_owned_fade_restores_current_game_pose_and_opacity_without_an_extra_tick() {
        let (mut world,entity)=world("scene label A\neileen.show(\"smile\") at left\neileen.effect(tint:\"#ffffff66\")\neileen.animate(\"shake\",duration:10)\n\"parent\"\n");
        let animation=world.get::<AnimationLease>(entity).unwrap().0.clone();
        let key=rvn_core::scene_resources::resource_key(animation.task.key.owner.clone(),rvn_core::scene_resources::ResourceTaskKind::Effect,"sprite/eileen/fade");
        let base=*world.get::<Transform>(entity).unwrap();
        let effect=SavedEffect{presentation:None,base_resolved:true,clock:EffectClock{duration:10.0,elapsed:5.0,looping:false},parameters:EffectParameters::VisualFade{from:0.0,to:1.0,despawn_on_finish:false,transition:VisualTransition::SlideLeft,base:snapshot(&base),layered:false}};
        let token=world.resource_mut::<VnEngine>().0.register_scene_effect(key,animation.target,effect).unwrap();
        let (tasks,epoch)={let game=&world.resource::<VnEngine>().0;let execution=game.state.scene_execution.as_ref().unwrap();(execution.resources.active_effects(execution.owner()).into_iter().cloned().collect(),game.runtime_epoch())};
        install(&mut world,tasks,epoch);reconcile(&mut world);
        let authored_alpha=Srgba::hex("ffffff66").unwrap().alpha;
        assert_eq!(world.get::<Sprite>(entity).unwrap().color.alpha(),authored_alpha*0.5);
        assert_eq!(world.get::<Transform>(entity).unwrap().translation.x,base.translation.x+960.0);
        let current=Vec3::new(73.0,22.0,base.translation.z);
        world.get_mut::<SpriteBaseTransform>(entity).unwrap().translation=current;
        let (tasks,epoch)={let game=&world.resource::<VnEngine>().0;let execution=game.state.scene_execution.as_ref().unwrap();(execution.resources.active_effects(execution.owner()).into_iter().cloned().collect(),game.runtime_epoch())};
        install(&mut world,tasks,epoch);reconcile(&mut world);
        assert_eq!(world.get::<OwnedFadeBase>(entity).unwrap().transform.translation,current);
        assert_eq!(world.get::<FadeAnim>(entity).unwrap().elapsed_secs,5.0);
        assert_eq!(world.get::<Transform>(entity).unwrap().translation.x,current.x+960.0);
        capture_bases(&mut world).unwrap();
        assert!(!world.resource_mut::<VnEngine>().0.complete_scene_effect(&token).unwrap());
        rvn_core::scene_resources::supersede_task(&mut world.resource_mut::<VnEngine>().0.state,&token.task.key).unwrap();
        let (tasks,epoch)={let game=&world.resource::<VnEngine>().0;let execution=game.state.scene_execution.as_ref().unwrap();(execution.resources.active_effects(execution.owner()).into_iter().cloned().collect(),game.runtime_epoch())};
        install(&mut world,tasks,epoch);
        assert!(world.get::<FadeLease>(entity).is_none() && world.get::<FadeAnim>(entity).is_none() && world.get::<OwnedFadeBase>(entity).is_none());
        assert_eq!(world.get::<Sprite>(entity).unwrap().color.alpha(),authored_alpha);
        assert_eq!(world.get::<Transform>(entity).unwrap().translation,current);
    }
}

// PRIVATE fragment: append as a child module of rvn_bevy/src/scene_effects.rs.
// Requires the future rvn_core::effect_sampling module and SavedEffect.presentation.
// Calls actual C10 helpers/consumer bodies. NOT_RUN; no GPU/decoder/pixel claim.
#[cfg(test)]
mod c11_shared_sampler_comparisons {
    use super::*;
    use rvn_core::effect_sampling as shared;
    use rvn_core::scene_resources::{resource_key, EffectClock, ResourceTaskKind};
    use rvn_core::scene_tasks::TaskOwner;
    use rvn_core::sprite_effects::SpriteStyle;

    fn make_clock(elapsed: f64, duration: f64, looping: bool) -> EffectClock {
        let clock = EffectClock {
            elapsed,
            duration,
            looping,
        };
        clock.validate().unwrap();
        clock
    }
    fn exact_transform(actual: &TransformSnapshot, expected: &Transform) {
        for (left, right) in actual
            .translation
            .into_iter()
            .chain(actual.scale)
            .chain(actual.rotation)
            .zip(
                expected
                    .translation
                    .to_array()
                    .into_iter()
                    .chain(expected.scale.to_array())
                    .chain(expected.rotation.to_array()),
            )
        {
            assert!(left.is_finite() && right.is_finite());
            assert_eq!(left.to_bits(), right.to_bits());
        }
    }
    fn styled_base(scale: f32, rotation: f32, flip_x: bool, flip_y: bool) -> Transform {
        let style = SpriteStyle {
            scale,
            rotation,
            flip_x,
            flip_y,
            tint: None,
        };
        style.validate().unwrap();
        let mut base = Transform::from_xyz(17.25, -83.5, 10.0);
        crate::systems::sprite::apply_sprite_transform_style(&mut base, &style);
        base
    }
    fn clocks() -> Vec<EffectClock> {
        vec![
            make_clock(0.0, 0.0, false),
            make_clock(0.0, 1.0, false),
            make_clock(0.25, 1.0, false),
            make_clock(1.0, 1.0000000000000002, false),
            make_clock(1.0, 1.0, false),
            make_clock(20.25, 1.0, true),
            make_clock(1.0e12, 86400.0, true),
        ]
    }
    fn visual_transitions() -> [VisualTransition; 10] {
        [
            VisualTransition::Fade,
            VisualTransition::Dissolve,
            VisualTransition::SlideLeft,
            VisualTransition::SlideRight,
            VisualTransition::SlideUp,
            VisualTransition::SlideDown,
            VisualTransition::ZoomIn,
            VisualTransition::ZoomOut,
            VisualTransition::Wipe,
            VisualTransition::Blur,
        ]
    }
    fn fade(kind: VisualTransition, from: f32, to: f32, clock: &EffectClock) -> FadeAnim {
        FadeAnim {
            kind: transition_into(kind),
            from,
            to,
            duration_secs: clock.duration as f32,
            elapsed_secs: clock.elapsed as f32,
            despawn_on_finish: false,
        }
    }
    fn animation(kind: &SpriteEffectKind, clock: &EffectClock) -> SpriteAnimation {
        let kind = match *kind {
            SpriteEffectKind::Shake { intensity } => AnimationKind::Shake { intensity },
            SpriteEffectKind::Bounce { height } => AnimationKind::Bounce { height },
            SpriteEffectKind::Pulse { scale } => AnimationKind::Pulse { scale },
        };
        SpriteAnimation {
            kind,
            duration_secs: clock.duration as f32,
            elapsed_secs: clock.elapsed as f32,
            looping: clock.looping,
        }
    }

    // Pinned own-project C10 math oracle. Body hashes/anchors are in the private
    // receipt. Keep this independent if live Bevy adapters later delegate shared.
    mod c10_reference {
        use super::*;
        pub(crate) fn sample_owned_sprite_animation(
            animation: &SpriteAnimation,
            base: &SpriteBaseTransform,
            pose: &mut Transform,
            reduced: bool,
            elapsed: f64,
            duration: f64,
        ) {
            pose.translation = base.translation;
            pose.scale = base.scale;
            if reduced || !animation.looping && elapsed >= duration {
                return;
            }
            let raw = if duration > 0.0 {
                elapsed / duration
            } else {
                1.0
            };
            let progress = if animation.looping {
                raw.fract()
            } else {
                raw.min(1.0)
            };
            match animation.kind {
                AnimationKind::Shake { intensity } => {
                    let decay = if animation.looping {
                        1.0
                    } else {
                        1.0 - progress
                    };
                    pose.translation.x += ((progress * std::f64::consts::TAU * 6.0).sin()
                        * f64::from(intensity)
                        * decay) as f32;
                }
                AnimationKind::Bounce { height } => {
                    pose.translation.y +=
                        ((progress * std::f64::consts::PI).sin() * f64::from(height)) as f32
                }
                AnimationKind::Pulse { scale } => {
                    pose.scale = base.scale
                        * (1.0
                            + (f64::from(scale) - 1.0)
                                * (progress * std::f64::consts::TAU).sin().max(0.0))
                            as f32
                }
            }
        }
        pub(crate) fn apply_owned_visual_pose(
            animation: &FadeAnim,
            base: &Transform,
            pose: &mut Transform,
            t: f64,
        ) {
            *pose = *base;
            let progress = if animation.to > animation.from {
                1.0 - t
            } else {
                t
            };
            match animation.kind {
                TransitionKind::SlideLeft => pose.translation.x += (1920.0 * progress) as f32,
                TransitionKind::SlideRight => pose.translation.x -= (1920.0 * progress) as f32,
                TransitionKind::SlideUp => pose.translation.y -= (1920.0 * progress) as f32,
                TransitionKind::SlideDown => pose.translation.y += (1920.0 * progress) as f32,
                TransitionKind::ZoomIn => pose.scale *= (1.0 - 0.5 * progress) as f32,
                TransitionKind::ZoomOut => pose.scale *= (1.0 + 0.5 * progress) as f32,
                _ => {}
            }
        }
        pub(crate) fn effect_progress(clock: &rvn_core::scene_resources::EffectClock) -> f64 {
            if clock.duration <= 0.0 {
                1.0
            } else {
                (clock.elapsed / clock.duration).clamp(0.0, 1.0)
            }
        }
    }

    // A minimal actual Engine token plus one presentation entity. No schedule,
    // assets, file lookup or decoder: the consumer math is invoked directly.
    fn visual_world(
        base: Transform,
        from: f32,
        to: f32,
        clock: &EffectClock,
        original_alpha: f32,
        layered: bool,
        opacity: f32,
        reduced: bool,
    ) -> (World, Entity, Entity) {
        let mut game = rvn_core::Engine::from_program(
            rvn_parser::parse_program("scene label P\nhero.show(\"smile\")\n\"line\"\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),
            32,
        )
        .unwrap();
        game.step_until_interaction().unwrap();
        let definition = rvn_ui::composition::Composition::parse(serde_json::json!({
            "size":[100,50], "layers":[{"id":"body","image":"sprites/hero/body.png","opacity":opacity}]
        })).unwrap();
        if layered {
            game.state
                .layered
                .compose("hero".into(), definition.clone())
                .unwrap();
            rvn_core::scene_resources::sync_current(&mut game.state).unwrap();
        }
        let execution = game.state.scene_execution.as_ref().unwrap();
        let owner = TaskOwner::Scene {
            activation: execution.owner().unwrap(),
        };
        let target = execution
            .resources
            .target(&TaskOwner::Game, TargetKind::Sprite, "hero")
            .unwrap()
            .clone();
        let token = game
            .register_scene_effect(
                resource_key(owner, ResourceTaskKind::Effect, "sprite/hero/fade"),
                target,
                SavedEffect {
                    presentation: None,
                    base_resolved: true,
                    clock: clock.clone(),
                    parameters: EffectParameters::VisualFade {
                        from,
                        to,
                        despawn_on_finish: false,
                        transition: VisualTransition::SlideLeft,
                        base: snapshot(&base),
                        layered,
                    },
                },
            )
            .unwrap();
        let mut world = World::new();
        world.insert_resource(VnEngine(game));
        world.insert_resource(Time::<()>::default());
        let mut access = crate::accessibility::Accessibility::default();
        access.settings.reduced_motion = reduced;
        world.insert_resource(access);
        if layered {
            let root = world
                .spawn((
                    crate::layered_characters::CompositionRoot {
                        extent: Vec2::new(100.0, 50.0),
                        position: rvn_parser::Position::Center,
                        tint: Color::srgba(0.7, 0.8, 0.9, original_alpha),
                    },
                    crate::layered_characters::LayeredFade {
                        animation: fade(VisualTransition::SlideLeft, from, to, clock),
                        base,
                    },
                    FadeLease(token),
                    base,
                ))
                .id();
            let child = world
                .spawn((
                    crate::layered_characters::LayerTarget {
                        character: "hero".into(),
                        layer: "body".into(),
                        description: definition.layers[0].clone(),
                    },
                    Sprite::default(),
                ))
                .id();
            world.entity_mut(root).add_child(child);
            (world, root, child)
        } else {
            let entity = world
                .spawn((
                    base,
                    Sprite {
                        color: Color::srgba(0.7, 0.8, 0.9, original_alpha),
                        ..default()
                    },
                    fade(VisualTransition::SlideLeft, from, to, clock),
                    OwnedFadeBase {
                        transform: base,
                        alpha: original_alpha,
                    },
                    FadeLease(token),
                ))
                .id();
            (world, entity, entity)
        }
    }

    #[test]
    fn shared_sprite_matches_actual_owned_sampler_with_signed_styles_and_f64_clock_edges() {
        for base in [
            styled_base(0.7, 37.0, true, false),
            styled_base(-1.25, -93.0, false, true),
        ] {
            for kind in [
                SpriteEffectKind::Shake { intensity: 30.0 },
                SpriteEffectKind::Bounce { height: 18.0 },
                SpriteEffectKind::Pulse { scale: 1.4 },
            ] {
                for clock in clocks() {
                    for reduced in [false, true] {
                        let bits = (clock.elapsed.to_bits(), clock.duration.to_bits());
                        let component = animation(&kind, &clock);
                        let sprite_base = SpriteBaseTransform {
                            translation: base.translation,
                            scale: base.scale,
                        };
                        let mut actual = base;
                        crate::systems::sprite::sample_owned_sprite_animation(
                            &component,
                            &sprite_base,
                            &mut actual,
                            reduced,
                            clock.elapsed,
                            clock.duration,
                        );
                        let mut pinned = base;
                        c10_reference::sample_owned_sprite_animation(
                            &component,
                            &sprite_base,
                            &mut pinned,
                            reduced,
                            clock.elapsed,
                            clock.duration,
                        );
                        exact_transform(&snapshot(&actual), &pinned);
                        let sampled =
                            shared::sample_sprite(&kind, &clock, &snapshot(&base), reduced);
                        exact_transform(&sampled, &actual);
                        assert_eq!((clock.elapsed.to_bits(), clock.duration.to_bits()), bits);
                        assert_eq!(
                            component.elapsed_secs.to_bits(),
                            (clock.elapsed as f32).to_bits()
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn shared_visual_matches_actual_owned_sampler_for_every_transition_and_both_directions() {
        let base = styled_base(-0.75, 41.0, true, false);
        for kind in visual_transitions() {
            for (from, to) in [(0.0, 1.0), (1.0, 0.0), (0.8, 0.2), (0.3, 0.3)] {
                for clock in clocks().into_iter().filter(|clock| !clock.looping) {
                    let source = serde_json::to_vec(&clock).unwrap();
                    let mut actual = base;
                    apply_owned_visual_pose(
                        &fade(kind, from, to, &clock),
                        &base,
                        &mut actual,
                        effect_progress(&clock),
                    );
                    let mut pinned = base;
                    c10_reference::apply_owned_visual_pose(
                        &fade(kind, from, to, &clock),
                        &base,
                        &mut pinned,
                        c10_reference::effect_progress(&clock),
                    );
                    exact_transform(&snapshot(&actual), &pinned);
                    let sampled =
                        shared::sample_visual(kind, from, to, &clock, &snapshot(&base), false);
                    exact_transform(&sampled.transform, &actual);
                    assert_eq!(
                        shared::progress(&clock).to_bits(),
                        effect_progress(&clock).to_bits()
                    );
                    assert_eq!(serde_json::to_vec(&clock).unwrap(), source);
                }
            }
        }
    }
    #[test]
    fn shared_animation_then_fade_keeps_actual_c10_order_and_one_f64_clock() {
        let clock = make_clock(0.3141592653589793, 1.0000000000000002, false);
        let base = styled_base(1.15, -28.0, true, true);
        let kind = SpriteEffectKind::Bounce { height: 27.0 };
        let mut actual = base;
        crate::systems::sprite::sample_owned_sprite_animation(
            &animation(&kind, &clock),
            &SpriteBaseTransform {
                translation: base.translation,
                scale: base.scale,
            },
            &mut actual,
            false,
            clock.elapsed,
            clock.duration,
        );
        let animated = actual;
        apply_owned_visual_pose(
            &fade(VisualTransition::ZoomOut, 1.0, 0.0, &clock),
            &animated,
            &mut actual,
            effect_progress(&clock),
        );
        let neutral = shared::sample_sprite(&kind, &clock, &snapshot(&base), false);
        let sampled =
            shared::sample_visual(VisualTransition::ZoomOut, 1.0, 0.0, &clock, &neutral, false);
        exact_transform(&sampled.transform, &actual);
        assert_eq!(clock.elapsed.to_bits(), 0.3141592653589793f64.to_bits());
    }
    #[test]
    fn shared_simple_alpha_matches_actual_consumer_single_final_cast_not_layer_factor_cast() {
        let clock = make_clock(0.18466034385487662, 1.0, false);
        let original = 0.4656500816345215f32;
        let from = 0.5078412890434265f32;
        let to = 0.587384819984436f32;
        let base = styled_base(0.8, 19.0, false, true);
        let (mut world, entity, _) =
            visual_world(base, from, to, &clock, original, false, 1.0, false);
        let before = serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap();
        sample_owned_effects(&mut world);
        let actual = world.get::<Sprite>(entity).unwrap().color.to_srgba().alpha;
        let sampled = shared::sample_visual(
            VisualTransition::SlideLeft,
            from,
            to,
            &clock,
            &snapshot(&base),
            false,
        );
        assert_eq!(actual.to_bits(), 0x3e7927d8);
        assert_eq!(
            sampled.with_original_alpha(original).to_bits(),
            actual.to_bits()
        );
        assert_ne!(
            (original * sampled.alpha as f32).to_bits(),
            actual.to_bits()
        );
        exact_transform(&sampled.transform, world.get::<Transform>(entity).unwrap());
        assert_eq!(
            serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(),
            before
        );
    }
    #[test]
    fn shared_layered_factor_cast_matches_actual_layer_consumer_before_tint_and_opacity() {
        let clock = make_clock(0.18466034385487662, 1.0, false);
        let (from, to, original, opacity) = (
            0.5078412890434265f32,
            0.587384819984436f32,
            0.4656500816345215f32,
            0.37f32,
        );
        let base = styled_base(0.8, -19.0, true, false);
        let (mut world, root, child) =
            visual_world(base, from, to, &clock, original, true, opacity, false);
        let before = serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap();
        crate::layered_characters::sample_owned(&mut world);
        let actual = world.get::<Sprite>(child).unwrap().color.to_srgba();
        let sampled = shared::sample_visual(
            VisualTransition::SlideLeft,
            from,
            to,
            &clock,
            &snapshot(&base),
            false,
        );
        let expected = (original * opacity) * sampled.alpha as f32;
        assert_eq!(actual.alpha.to_bits(), expected.to_bits());
        assert_eq!(
            [
                actual.red.to_bits(),
                actual.green.to_bits(),
                actual.blue.to_bits()
            ],
            [0.7f32.to_bits(), 0.8f32.to_bits(), 0.9f32.to_bits()]
        );
        exact_transform(&sampled.transform, world.get::<Transform>(root).unwrap());
        assert_eq!(
            serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(),
            before
        );
    }
    #[test]
    fn reduced_visual_uses_explicit_terminal_projection_policy_without_advancing_core_clock() {
        let clock = make_clock(0.25, 1.0, false);
        let base = styled_base(0.9, 22.0, false, false);
        let source = serde_json::to_vec(&clock).unwrap();
        let (mut world, root, child) = visual_world(base, 0.0, 1.0, &clock, 0.4, true, 0.5, true);
        let before = serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap();
        crate::layered_characters::sample_owned(&mut world);
        let terminal = shared::sample_visual(
            VisualTransition::SlideLeft,
            0.0,
            1.0,
            &clock,
            &snapshot(&base),
            true,
        );
        exact_transform(&terminal.transform, world.get::<Transform>(root).unwrap());
        assert_eq!(
            world
                .get::<Sprite>(child)
                .unwrap()
                .color
                .to_srgba()
                .alpha
                .to_bits(),
            0.2f32.to_bits()
        );
        // This is the C11 SIMPLE adaptation, not C10 equality: simple C10 did
        // not branch on reduced motion. The terminal helper phase is explicit.
        let mut desired_simple = base;
        apply_owned_visual_pose(
            &fade(VisualTransition::SlideLeft, 0.0, 1.0, &clock),
            &base,
            &mut desired_simple,
            1.0,
        );
        exact_transform(&terminal.transform, &desired_simple);
        assert_eq!(terminal.alpha, 1.0);
        assert_eq!(serde_json::to_vec(&clock).unwrap(), source);
        assert_eq!(
            serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(),
            before
        );
    }
    #[test]
    fn shared_audio_keeps_c10_absolute_gain_then_one_f64_retired_factor() {
        // The C10 sink formula at scene_effects.rs:164-166 is the reference;
        // no AudioSink/decoder is constructed by this CPU arithmetic contract.
        for (from, to) in [(0.0f32, 0.4f32), (0.9, 0.2), (0.3, 0.3)] {
            for clock in clocks().into_iter().filter(|clock| !clock.looping) {
                let phase = effect_progress(&clock);
                let c10_gain = (f64::from(from) + (f64::from(to) - f64::from(from)) * phase) as f32;
                assert_eq!(
                    shared::sample_audio(from, to, &clock).to_bits(),
                    c10_gain.to_bits()
                );
                let frozen = c10_gain;
                let outgoing = make_clock(0.18466034385487662, 1.0, false);
                let frozen_bits = frozen.to_bits();
                let expected = (f64::from(frozen) * (1.0 - effect_progress(&outgoing))) as f32;
                assert_eq!(
                    shared::retired_music_gain(frozen, &outgoing).to_bits(),
                    expected.to_bits()
                );
                assert_eq!(frozen.to_bits(), frozen_bits);
            }
        }
        let frozen = 0.4656500816345215f32;
        let outgoing = make_clock(0.18466034385487662, 1.0, false);
        assert_ne!(
            shared::retired_music_gain(frozen, &outgoing).to_bits(),
            (frozen * (1.0 - shared::progress(&outgoing)) as f32 * 0.32f32).to_bits()
        );
    }
}

#[cfg(test)]
mod standalone_game_sprites_0917 {
 use super::*;
 use bevy::ecs::system::RunSystemOnce;
 use bevy::asset::AssetApp;
 const SOURCE:&str="label start\nhero.show(\"images/card.png\") at center\nflow.consume_v2(\"animation\",\"command\"){hero.animate(\"bounce\",duration:2.0,loop:true)}\nflow.consume_v2(\"style\",\"command\"){hero.effect(scale:1.25)}\nflow.consume_v2(\"timer\",\"command\"){timer 5 => jump done}\n\"first\"\nflow.consume_v2(\"stop\",\"command\"){hero.stop_animation()}\n\"second\"\nlabel done\n\"done\"\n";
 fn program()->rvn_parser::ParsedProgram{
  static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
  let root=std::path::PathBuf::from(std::env::var("RVN_GAME_TASK_CONTRACT_FIXTURES").expect("isolated writable GameTasks fixtures"))
   .join(format!("sprite-projection-{}-{}",std::process::id(),NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed)));
  std::fs::create_dir_all(&root).unwrap();let file=root.join("main.rvn");std::fs::write(&file,SOURCE).unwrap();rvn_parser::parse_file_program_with_uses(file).unwrap()
 }
 fn project(game:rvn_core::Engine<crate::bevy_renderer::BevyRenderer>)->(World,Entity){
  let mut world=World::new();world.insert_resource(VnEngine(game));
  world.init_resource::<crate::scene_publication::ScenePresentation>();world.init_resource::<Events<crate::vn_command::VnCommand>>();world.init_resource::<Events<crate::vn_command::PlayerInput>>();
  world.init_resource::<crate::accessibility::Accessibility>();world.insert_resource(Time::<()>::default());world.insert_resource(State::new(VnState::Waiting));
  let pose=crate::systems::sprite::sprite_default_transform(&rvn_parser::Position::Center);
  let entity=world.spawn((VnSprite{id:"hero".into()},SpriteBaseTransform{translation:pose.translation,scale:pose.scale},pose,Sprite::default())).id();
  crate::scene_publication::consume(&mut world);reconcile(&mut world);capture_bases(&mut world).unwrap();(world,entity)
 }
 fn started()->(rvn_parser::ParsedProgram,World,Entity){
  let program=program();let mut game=rvn_core::Engine::from_program(program.clone(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();game.step_until_interaction().unwrap();
  let(world,entity)=project(game);(program,world,entity)
 }
 fn frame(world:&mut World,seconds:f64){world.resource_mut::<VnEngine>().0.tick_game_frame(seconds,Default::default()).unwrap();crate::scene_publication::consume(world);reconcile(world);capture_bases(world).unwrap();}
 fn saved(world:&World)->SavedEffect{world.resource::<VnEngine>().0.state.game_tasks.as_ref().unwrap().resources.tasks.iter().find_map(|t|t.effect.clone()).unwrap()}
 #[test]
 fn standalone_actual_projection_samples_core_clock_without_ecs_double_tick(){
  let(_,mut world,entity)=started();assert!(world.resource::<VnEngine>().0.state.scene_execution.is_none());assert!(world.resource::<VnEngine>().0.state.game_audio.is_none());
  assert!(world.get::<AnimationLease>(entity).is_some());assert!(saved(&world).base_resolved);
  frame(&mut world,0.125);assert_eq!(saved(&world).clock.elapsed,0.125);assert_eq!(world.get::<SpriteAnimation>(entity).unwrap().elapsed_secs,0.125);
  let pose=*world.get::<Transform>(entity).unwrap();world.resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));world.run_system_once(crate::systems::sprite_animation_system);
  assert_eq!(saved(&world).clock.elapsed,0.125);assert_eq!(world.get::<SpriteAnimation>(entity).unwrap().elapsed_secs,0.125);assert_eq!(*world.get::<Transform>(entity).unwrap(),pose);
  assert_eq!(world.get::<SpriteBaseTransform>(entity).unwrap().scale,Vec3::new(1.25,1.25,1.0));
 }
 #[test]
 fn standalone_stop_revokes_only_animation_lease_and_restores_the_live_style_base(){
  let(_,mut world,entity)=started();frame(&mut world,0.25);let old=world.get::<AnimationLease>(entity).unwrap().0.clone();let base=*world.get::<SpriteBaseTransform>(entity).unwrap();
  {let mut game=world.resource_mut::<VnEngine>();game.0.advance_dialogue().unwrap();game.0.step_until_interaction().unwrap();}
  crate::scene_publication::consume(&mut world);reconcile(&mut world);
  assert!(world.get::<AnimationLease>(entity).is_none());assert!(world.get::<SpriteAnimation>(entity).is_none());assert!(authoritative_effect(&world,&old).is_none());
  assert_eq!(world.get::<Transform>(entity).unwrap().translation,base.translation);assert_eq!(world.get::<Transform>(entity).unwrap().scale,base.scale);
  let tasks=world.resource::<VnEngine>().0.state.game_tasks.as_ref().unwrap();assert_eq!(tasks.sprite_styles["hero"].scale,1.25);assert_eq!(tasks.resources.targets.len(),1);assert_eq!(tasks.tasks.timer.as_ref().unwrap().elapsed,0.25);
 }
 #[test]
 fn standalone_save15_cold_projection_rebinds_epoch_and_reconstructs_the_same_pose(){
  let(program,mut world,entity)=started();frame(&mut world,0.125);let old=world.get::<AnimationLease>(entity).unwrap().0.clone();let pose=*world.get::<Transform>(entity).unwrap();
  let game=&world.resource::<VnEngine>().0;let save=game.capture_save15(&program,None,1,"sprite".into()).unwrap();let decoded=rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap();
  let mut cold=rvn_core::Engine::from_program(program.clone(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();cold.restore_save15(&program,decoded).unwrap();let(mut restored,entity)=project(cold);
  let fresh=restored.get::<AnimationLease>(entity).unwrap().0.clone();assert_ne!(fresh,old);assert!(authoritative_effect(&restored,&old).is_none());assert_eq!(*restored.get::<Transform>(entity).unwrap(),pose);assert_eq!(saved(&restored).clock.elapsed,0.125);
  frame(&mut restored,0.125);assert_eq!(saved(&restored).clock.elapsed,0.25);
 }
 #[test]
 fn actual_startup_sprite_publication_projects_before_same_owner_game_handoff(){
  let source="init{\nhero.show(\"images/card.png\") at center\nflow.consume_v2(\"animation\",\"command\"){hero.animate(\"bounce\",duration:2.0,loop:true)}\nflow.consume_v2(\"style\",\"command\"){hero.effect(scale:1.25)}\nflow.consume_v2(\"timer\",\"command\"){timer 5 => jump done}\n}\nlabel start\n\"hold\"\nlabel done\n\"done\"\n";
  let(root,mut app)=crate::debugger_startup::tests::fixture_source(source);
  let image=std::path::PathBuf::from(std::env::var("RVN_STARTUP_WORLD_FIXTURES").unwrap()).parent().unwrap().join("shadow/examples/animations/assets/card-one.png");
  std::fs::create_dir(root.join("assets/images")).unwrap();std::fs::copy(image,root.join("assets/images/card.png")).unwrap();
  app.init_asset::<Image>().init_asset_loader::<bevy::render::texture::ImageLoader>().init_resource::<crate::accessibility::Accessibility>().add_systems(Update,crate::systems::sprite_system);
  let instance=app.world().resource::<crate::debugger_startup::DebugBootstrap>().owner.snapshot().instance;
  for _ in 0..32{
   if app.world().resource::<crate::debugger_startup::DebugBootstrap>().owner.is_complete(){break;}
   let mut prepared=crate::debugger_startup::prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
   let deadline=std::time::Instant::now()+std::time::Duration::from_secs(15);
   while !prepared.ready(){app.update();assert!(std::time::Instant::now()<deadline,"Real Startup asset staging timed out");crate::debugger_startup::refresh_world_step(app.world(),&mut prepared).unwrap();}
   let committed=crate::debugger_startup::commit_world_step(app.world_mut(),prepared).unwrap();crate::debugger_startup::install_committed_world_step(app.world_mut(),committed);app.update();reconcile(app.world_mut());
  }
  assert!(!app.world().contains_resource::<VnEngine>());let startup=app.world().resource::<crate::debugger_startup::DebugBootstrap>();assert!(startup.owner.is_complete());assert!(startup.owner.state().scene_execution.is_none());assert!(startup.owner.state().game_audio.is_none());
  assert_eq!(startup.owner.state().game_tasks.as_ref().unwrap().tasks.timer.as_ref().unwrap().elapsed,0.0);
  let entity=app.world_mut().query_filtered::<Entity,With<VnSprite>>().single(app.world());let token=app.world().get::<AnimationLease>(entity).unwrap().0.clone();
  assert_eq!(authoritative_effect(app.world(),&token).unwrap().clock.elapsed,0.0);assert_eq!(app.world().get::<SpriteBaseTransform>(entity).unwrap().scale,Vec3::new(1.25,1.25,1.0));
  app.world_mut().run_system_once(crate::systems::sprite_animation_system);assert_eq!(authoritative_effect(app.world(),&token).unwrap().clock.elapsed,0.0);
  let startup=app.world_mut().remove_resource::<crate::debugger_startup::DebugBootstrap>().unwrap();let engine=startup.owner.into_engine().unwrap();assert_eq!(engine.instance_id(),instance);app.world_mut().insert_resource(VnEngine(engine));
  capture_bases(app.world_mut()).unwrap();frame(app.world_mut(),0.125);assert_eq!(saved(app.world()).clock.elapsed,0.125);
 }
}
