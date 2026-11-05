//! PRIVATE NOT_RUN. ECS adapters for independent historical appearances.
//! Saved payloads/clocks remain in Core; this cache owns only assets/entities.
use bevy::prelude::*;
use rvn_core::retired_presentations::{RetiredModel, RetiredPresentation};
use rvn_core::retired_sampling::{self, FrozenAppearance, HistoricalGeometry, LayerGeometry, NeutralGeometry, RetiredFrame};
use rvn_core::scene_resources::{ResourceToken, SavedEffect, TargetIdentity, TargetKind, TransformSnapshot};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(crate) struct RetiredProjection { pub token: ResourceToken }

#[derive(Resource, Default)]
struct RetiredEntities { entries: HashMap<TargetIdentity, Entry> }
struct Entry {
    instance_id:u64,
    token: ResourceToken,
    presentation: RetiredPresentation,
    effect: SavedEffect,
    parked: bool,
    ready: bool,
    root: Option<Entity>,
    leaves: Vec<Entity>,
    images: BTreeMap<String, Handle<Image>>,
    geometry_key: Option<GeometryKey>,
    frozen: Option<FrozenAppearance>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct GeometryKey { viewport: [u32; 2], background_path: Option<String>, background: Option<[u32; 2]>, reduced: bool }
pub(crate) struct PreparedRetired { items: Vec<PreparedItem> }
struct PreparedItem {
    token: ResourceToken, presentation: RetiredPresentation, effect: SavedEffect,
    parked: bool, key: GeometryKey, frozen: Option<FrozenAppearance>,
    frame: Option<RetiredFrame>, handoff: Option<Entity>,
}

fn snapshot(pose:&Transform)->TransformSnapshot {
    TransformSnapshot{translation:pose.translation.to_array(),scale:pose.scale.to_array(),rotation:pose.rotation.to_array()}
}
fn transform(pose:&TransformSnapshot)->Transform {
    Transform{translation:Vec3::from_array(pose.translation),scale:Vec3::from_array(pose.scale),rotation:Quat::from_array(pose.rotation)}
}
fn viewport(world:&World)->Vec2 {
    world.iter_entities().filter(|entity|entity.contains::<bevy::window::PrimaryWindow>())
        .find_map(|entity|entity.get::<Window>()).or_else(||world.iter_entities().find_map(|entity|entity.get::<Window>()))
        .map(|window|Vec2::new(window.width(),window.height())).unwrap_or(Vec2::new(crate::systems::WIN_W,crate::systems::WIN_H))
}
fn reduced(world:&World)->bool {
    world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access|access.settings.reduced_motion)
}
fn image_size(world:&World,path:&str)->Option<[u32;2]> {
    let assets=world.get_resource::<AssetServer>()?;
    let handle=assets.get_handle::<Image>(crate::systems::ensure_extension(path))?;
    let size=world.get_resource::<Assets<Image>>()?.get(&handle)?.size();
    (size.x>0 && size.y>0).then_some([size.x,size.y])
}
fn geometry_key(world:&World,presentation:&RetiredPresentation)->GeometryKey {
    let size=viewport(world);
    let reduced = reduced(world);
    let background_path = match &presentation.model {
        RetiredModel::Background { path, sample } => Some(sample.root_motion.as_ref()
            .and_then(|motion| motion.selected(reduced).frame.as_ref())
            .unwrap_or(path).clone()),
        RetiredModel::Cinematic { id,.. }=>crate::systems::cinematic::resolve_cg_asset(world,id).ok(),
        _ => None,
    };
    let background = background_path.as_deref().and_then(|path| image_size(world,path));
    GeometryKey { viewport:[size.x.to_bits(),size.y.to_bits()], background_path, background, reduced }
}
fn geometry(presentation:&RetiredPresentation,key:GeometryKey)->Result<Option<HistoricalGeometry>,String> {
    use rvn_core::effect_sampling::MotionGeometry;
    let viewport=Vec2::new(f32::from_bits(key.viewport[0]),f32::from_bits(key.viewport[1]));
    if !viewport.is_finite() || viewport.min_element()<=0.0 {return Ok(None);}
    let (root,layers)=match &presentation.model {
        RetiredModel::Music{..}=>return Ok(None),
        RetiredModel::Cinematic{..}=>{
            let Some([width,height])=key.background else {return Ok(None);};
            let size=Vec2::new(width as f32,height as f32);
            if !size.is_finite() || size.min_element()<=0.0 {return Ok(None);}
            let scale=(viewport.x/size.x).max(viewport.y/size.y);
            let pose=Transform::from_xyz(0.0,0.0,50.0).with_scale(Vec3::new(scale,scale,1.0));
            (NeutralGeometry{transform:snapshot(&pose),motion:MotionGeometry{size:size.to_array(),factor:[1.0;2],ui:false}},BTreeMap::new())
        }
        RetiredModel::Background{..}=>{
            let Some([width,height])=key.background else {return Ok(None);};
            let size=Vec2::new(width as f32,height as f32);
            let Some(geometry) = crate::background_geometry::resolve(viewport,size) else { return Ok(None); };
            (NeutralGeometry { transform:snapshot(&geometry.cover), motion:MotionGeometry {
                size:geometry.pixels.to_array(), factor:geometry.motion_factor.to_array(), ui:false
            } }, BTreeMap::new())
        }
        RetiredModel::Sprite{state,composition,sample,..}=>{
            let pose=crate::systems::sprite::sprite_default_transform(&state.position);
            let height=crate::systems::WIN_H*0.85;
            let root_size=composition.as_ref().map(|composition|Vec2::new(composition.definition.size[0]*(height/composition.definition.size[1]),height)).unwrap_or(Vec2::new(height*0.55,height));
            let motion=MotionGeometry{size:root_size.to_array(),factor:[crate::systems::WIN_W/1920.0;2],ui:false};
            let mut layers=BTreeMap::new();
            if let Some(composition)=composition {
                let factor=height/composition.definition.size[1];
                for (order,layer) in composition.definition.layers.iter().enumerate().filter(|(_,layer)|composition.definition.visible_layers(&composition.attributes).any(|visible|visible.id==layer.id)) {
                    let [x,y,width,height]=layer.rect.unwrap_or([0.0,0.0,composition.definition.size[0],composition.definition.size[1]]);
                    let pose=Transform::from_xyz((x+width*0.5-composition.definition.size[0]*0.5)*factor,(composition.definition.size[1]*0.5-y-height*0.5)*factor,order as f32*0.01);
                    layers.insert(layer.id.clone(),LayerGeometry{
                        source:sample.layers.get(&layer.id).map(|sample|sample.original_target.identity.clone()),
                        neutral:NeutralGeometry{transform:snapshot(&pose),motion:MotionGeometry{size:[width*factor,height*factor],factor:motion.factor,ui:false}}
                    });
                }
            }
            (NeutralGeometry{transform:snapshot(&pose),motion},layers)
        }
    };
    Ok(Some(HistoricalGeometry{source:presentation.original_target.identity.clone(),root,layers}))
}
fn sampled(presentation:&RetiredPresentation,effect:&SavedEffect,key:GeometryKey)->Result<Option<(FrozenAppearance,RetiredFrame)>,String> {
    let geometry=geometry(presentation,key.clone())?;
    if geometry.is_none() && !matches!(presentation.model,RetiredModel::Music{..}) {return Ok(None);}
    let frozen=retired_sampling::freeze_appearance(presentation,geometry.as_ref(),key.reduced)?;
    let frame=retired_sampling::sample_outgoing(&frozen,effect,key.reduced)?;
    Ok(Some((frozen,frame)))
}

/// Called BEFORE taking the pending batch/output. Pure validation cannot
/// create an entity, consume an event, load an asset, or change an AudioSink.
pub(crate) fn prepare(world:&World,state:&rvn_core::GameState,epoch:u64)->Result<PreparedRetired,String> {
    let mut items=Vec::new();
    let Some(resources)=state.audio_resource_registry() else {return Ok(PreparedRetired{items});};
    let active=state.scene_execution.as_ref().and_then(|execution|execution.owner());
    crate::music_mix::user_gain(world)?;
    let instance_id=crate::text_runtime_owner::current(world)?.instance;
    let mut identities=HashSet::new();
    let mut audio_claims=HashSet::new();
    for task in &resources.tasks {
        if task.target.kind!=TargetKind::PresentationRetired {continue;}
        if !identities.insert(task.target.clone()) {return Err("Duplicate Retired target projection".into());}
        let effect=task.effect.as_ref().ok_or("Retired task has no saved effect")?;
        effect.validate()?;
        let presentation=effect.presentation.as_ref().ok_or("Retired task has no historical payload")?;
        presentation.validate_outgoing(effect)?;
        presentation.validate_for_target(&task.target,resources.next_incarnation)?;
        for path in presentation.resource_paths() {
            if let Some(assets)=world.get_resource::<AssetServer>() {
                let normalized=match &presentation.model {RetiredModel::Music{..}=>world.get_resource::<crate::resources::MusicAssetRegistry>().map(|registry|registry.resolve(&path)).unwrap_or(path.clone()),RetiredModel::Cinematic{..}=>crate::systems::cinematic::resolve_cg_asset(world,&path)?,_=>crate::systems::ensure_extension(&path)};
                let failed=match &presentation.model {
                    RetiredModel::Music{..}=>assets.get_handle::<AudioSource>(normalized).and_then(|handle|assets.get_load_state(handle.id())),
                    _=>assets.get_handle::<Image>(normalized).and_then(|handle|assets.get_load_state(handle.id())),
                };
                if let Some(bevy::asset::LoadState::Failed(error))=failed {return Err(format!("Retired asset '{path}' failed for exact target {:?}: {error}",task.target));}
            }
        }
        let token=ResourceToken{task:rvn_core::scene_tasks::TaskToken{runtime_epoch:epoch,key:task.key.clone(),revision:task.revision},target:task.target.clone()};
        if !task.parked && !resources.accepts(&token,epoch,active) {return Err("Retired active token is not accepted".into());}
        let key=geometry_key(world,presentation);
        let cached=world.get_resource::<RetiredEntities>().and_then(|cache|cache.entries.get(&task.target))
            .filter(|entry|(!matches!(entry.presentation.model,RetiredModel::Cinematic{..}) || entry.instance_id==instance_id) && entry.token.task.runtime_epoch==epoch && entry.presentation==*presentation && entry.geometry_key.as_ref()==Some(&key))
            .and_then(|entry|entry.frozen.as_ref());
        let mut sample=match cached {
            Some(frozen)=>Some((frozen.clone(),retired_sampling::sample_outgoing(frozen,effect,key.reduced)?)),
            None=>sampled(presentation,effect,key.clone())?
        };
        if world.get_resource::<AssetServer>().is_none() && !matches!(presentation.model,RetiredModel::Music{..}) {sample=None;}
        // Several owners may retire the same historical source. One physical
        // Current reader can be transferred once; other appearances get their
        // own player, never a shared entity/marker/gain.
        let retained_current=resources.target(&rvn_core::scene_tasks::TaskOwner::Game,TargetKind::Music,"music")==Some(&presentation.original_target.identity);
        let handoff=if retained_current {None}else{exact_audio_handoff(world,presentation,epoch).filter(|entity|audio_claims.insert(*entity))};
        let (frozen,frame)=sample.map(|(frozen,frame)|(Some(frozen),Some(frame))).unwrap_or((None,None));
        items.push(PreparedItem{token,presentation:presentation.clone(),effect:effect.clone(),parked:task.parked,key,frozen,frame,handoff});
    }
    Ok(PreparedRetired{items})
}
#[cfg(not(target_arch="wasm32"))]
fn exact_audio_handoff(world:&World,presentation:&RetiredPresentation,epoch:u64)->Option<Entity> {
    let RetiredModel::Music{file,..}=&presentation.model else {return None;};
    let previous=world.get_resource::<crate::scene_publication::ScenePresentation>()?.batch.as_ref()?;
    if previous.runtime_epoch!=epoch || previous.state.music.current_file.as_ref()!=Some(file) {return None;}
    let old=previous.state.audio_resource_registry()?.target(&rvn_core::scene_tasks::TaskOwner::Game,TargetKind::Music,"music")?;
    if old!=&presentation.original_target.identity || presentation.original_target.owner!=rvn_core::scene_tasks::TaskOwner::Game {return None;}
    let entity=world.get_resource::<crate::resources::MusicEntity>()?.0?;
    if world.get::<RetiredProjection>(entity).is_some() || world.get::<crate::components::MusicMarker>(entity).is_none() {return None;}
    let assets=world.get_resource::<AssetServer>()?;
    let handle=world.get::<Handle<AudioSource>>(entity)?;
    let expected=world.get_resource::<crate::resources::MusicAssetRegistry>()?.resolve(file);
    if assets.get_path(handle.id())?.path()!=std::path::Path::new(&expected) {return None;}
    Some(entity)
}
#[cfg(target_arch="wasm32")]
fn exact_audio_handoff(_world:&World,_presentation:&RetiredPresentation,_epoch:u64)->Option<Entity> {None}

pub(crate) fn install(world:&mut World,plan:PreparedRetired) {
    let mut cache=world.remove_resource::<RetiredEntities>().unwrap_or_default();
    let instance_id=crate::text_runtime_owner::current(world).expect("Preflighted actual retired owner").instance;
    let old_cg:Vec<_>=cache.entries.iter().filter(|(_,entry)|matches!(entry.presentation.model,RetiredModel::Cinematic{..}) && entry.instance_id!=instance_id).map(|(target,_)|target.clone()).collect();
    for target in old_cg {if let Some(entry)=cache.entries.remove(&target){remove(world,&entry);}}
    let wanted:HashSet<_>=plan.items.iter().map(|item|item.token.target.clone()).collect();
    let stale:Vec<_>=cache.entries.keys().filter(|identity|!wanted.contains(*identity)).cloned().collect();
    for identity in stale {if let Some(entry)=cache.entries.remove(&identity){remove(world,&entry);}}
    for item in plan.items {
        let identity=item.token.target.clone();
        if cache.entries.get(&identity).is_some_and(|entry|entry.token.task.runtime_epoch!=item.token.task.runtime_epoch || entry.presentation!=item.presentation) {
            if let Some(old)=cache.entries.remove(&identity){remove(world,&old);}
        }
        let entry=cache.entries.entry(identity).or_insert_with(||Entry{
            instance_id,token:item.token.clone(),presentation:item.presentation.clone(),effect:item.effect.clone(),parked:item.parked,
            ready:false,root:None,leaves:Vec::new(),images:BTreeMap::new(),geometry_key:None,frozen:None
        });
        entry.token=item.token;entry.effect=item.effect;entry.parked=item.parked;
        entry.geometry_key=Some(item.key);entry.frozen=item.frozen;
        retain_assets(world,entry);
        entry.ready=item.frame.as_ref().is_some_and(|frame|match frame {
            RetiredFrame::Music{..}=>true,
            RetiredFrame::Visual{leaves,..}=>world.get_resource::<Assets<Image>>().is_some_and(|images|leaves.iter().all(|leaf|entry.images.get(&leaf.path).is_some_and(|handle|images.get(handle).is_some())))
        });
        if let Some(frame)=item.frame {apply_frame(world,entry,&frame,item.handoff);}
        residence(world,entry);
    }
    world.insert_resource(cache);
}
fn retain_assets(world:&World,entry:&mut Entry) {
    if matches!(entry.presentation.model,RetiredModel::Music{..}) {return;}
    let Some(assets)=world.get_resource::<AssetServer>() else {return;};
    for path in entry.presentation.resource_paths() {
        let resolved=match &entry.presentation.model {RetiredModel::Cinematic{..}=>crate::systems::cinematic::resolve_cg_asset(world,&path).ok(),_=>Some(crate::systems::ensure_extension(&path))};
        if let Some(resolved)=resolved {
            // Catalogue changes cannot reuse a handle whose path is now different.
            let same=entry.images.get(&path).and_then(|handle|assets.get_path(handle.id())).is_some_and(|old|old.path()==std::path::Path::new(&resolved));
            if !same {entry.images.insert(path.clone(),assets.load(resolved));}
        }
    }
}
fn remove(world:&mut World,entry:&Entry) {
    if let Some(root)=entry.root.filter(|root|world.get_entity(*root).is_some()) {
        if let Some(sink)=world.get::<AudioSink>(root) {sink.stop();}
        world.entity_mut(root).despawn_recursive();
    }
}
fn leaf_size(presentation:&RetiredPresentation,layer:&Option<String>)->Option<Vec2> {
    match &presentation.model {
        RetiredModel::Sprite{composition:Some(composition),..}=>{
            let layer=composition.definition.layers.iter().find(|leaf|Some(&leaf.id)==layer.as_ref())?;
            let rect=layer.rect.unwrap_or([0.0,0.0,composition.definition.size[0],composition.definition.size[1]]);
            Some(Vec2::new(rect[2],rect[3])*(crate::systems::WIN_H*0.85/composition.definition.size[1]))
        }
        RetiredModel::Sprite{..}=>Some(Vec2::new(crate::systems::WIN_H*0.85*0.55,crate::systems::WIN_H*0.85)),
        _=>None,
    }
}
fn apply_frame(world:&mut World,entry:&mut Entry,frame:&RetiredFrame,handoff:Option<Entity>) {
    match frame {
        RetiredFrame::Visual{root,leaves}=>{
            let entity=entry.root.filter(|entity|world.get_entity(*entity).is_some()).unwrap_or_else(||{
                let entity=world.spawn((SpatialBundle::default(),RetiredProjection{token:entry.token.clone()})).id();
                if matches!(entry.presentation.model,RetiredModel::Sprite{..}) {
                    if let Some(stage)=world.query_filtered::<Entity,With<crate::systems::sprite::SpriteStage>>().iter(world).next() {world.entity_mut(stage).add_child(entity);}
                }
                entry.root=Some(entity);entity
            });
            world.entity_mut(entity).insert((transform(root),RetiredProjection{token:entry.token.clone()}));
            if entry.leaves.len()!=leaves.len() || entry.leaves.iter().any(|leaf|world.get_entity(*leaf).is_none()) {
                for leaf in entry.leaves.drain(..) {if world.get_entity(leaf).is_some(){world.entity_mut(leaf).despawn_recursive();}}
                for _ in leaves {let leaf=world.spawn((SpriteBundle::default(),RetiredProjection{token:entry.token.clone()})).id();world.entity_mut(entity).add_child(leaf);entry.leaves.push(leaf);}
            }
            for (entity,leaf) in entry.leaves.iter().copied().zip(leaves) {
                let Some(texture)=entry.images.get(&leaf.path).cloned() else {continue;};
                world.entity_mut(entity).insert((transform(&leaf.pose),texture,Sprite{custom_size:leaf_size(&entry.presentation,&leaf.layer),color:Color::srgba(leaf.color[0],leaf.color[1],leaf.color[2],leaf.color[3]),..default()},RetiredProjection{token:entry.token.clone()}));
                let authored_leaf = match (&entry.presentation.model, &leaf.layer) {
                    (RetiredModel::Sprite { composition: Some(composition), .. }, Some(layer)) =>
                        composition.definition.layers.iter().position(|authored| authored.id == *layer),
                    (_, None) => Some(0),
                    _ => None,
                };
                if let Some(order) = authored_leaf.and_then(|index| crate::presentation_order::PresentationDrawOrder::retired(
                    &entry.presentation.original_slot, &entry.token, index)) {
                    world.entity_mut(entity).insert(order);
                }
            }
        }
        RetiredFrame::Music{file,gain}=>apply_music(world,entry,file,*gain,handoff),
    }
}
#[cfg(not(target_arch="wasm32"))]
fn apply_music(world:&mut World,entry:&mut Entry,file:&str,gain:f32,handoff:Option<Entity>) {
    use bevy::audio::Volume;
    if crate::checkpoint_sources::checkpointed_music(world){return;}
    let Ok(gain)=crate::music_mix::in_world(world,gain) else {return;};
    let entity=entry.root.filter(|entity|world.get_entity(*entity).is_some()).or(handoff.filter(|entity|world.get_entity(*entity).is_some()));
    let entity=match entity {
        Some(entity)=>{
            if world.get_resource::<crate::resources::MusicEntity>().is_some_and(|music|music.0==Some(entity)) {world.resource_mut::<crate::resources::MusicEntity>().0=None;}
            world.entity_mut(entity).remove::<(crate::components::MusicMarker,crate::scene_effects::AudioLease,crate::components::AudioFade)>();
            entity
        }
        None=>{
            let Some(assets)=world.get_resource::<AssetServer>() else {return;};
            let Some(registry)=world.get_resource::<crate::resources::MusicAssetRegistry>() else {return;};
            let source=assets.load::<AudioSource>(registry.resolve(file));
            world.spawn(AudioBundle{source,settings:PlaybackSettings::LOOP.with_volume(Volume::new(gain)),..default()}).id()
        }
    };
    entry.root=Some(entity);
    world.entity_mut(entity).insert(RetiredProjection{token:entry.token.clone()});
    if let Some(mut settings)=world.get_mut::<PlaybackSettings>(entity) {settings.volume=Volume::new(gain);settings.paused=entry.parked;}
    if let Some(sink)=world.get::<AudioSink>(entity) {sink.set_volume(gain);}
}
#[cfg(target_arch="wasm32")]
fn apply_music(_world:&mut World,_entry:&mut Entry,_file:&str,_gain:f32,_handoff:Option<Entity>) {
    // The complete WebMusic snapshot installer owns Current + Retired browser readers.
    // This native ECS metadata path must not spawn/alias a second singleton reader.
    // Actual browser capability remains unqualified until the indexed bridge is built/exercised.
}
fn residence(world:&mut World,entry:&Entry) {
    if let Some(entity)=entry.root {
        if let Some(mut visibility)=world.get_mut::<Visibility>(entity) {*visibility=if entry.parked || !entry.ready {Visibility::Hidden}else{Visibility::Inherited};}
        if let Some(mut settings)=world.get_mut::<PlaybackSettings>(entity) {settings.paused=entry.parked;}
        if let Some(sink)=world.get::<AudioSink>(entity) {if entry.parked {sink.pause();}else{sink.play();}}
        world.entity_mut(entity).insert(RetiredProjection{token:entry.token.clone()});
        for leaf in &entry.leaves {if world.get_entity(*leaf).is_some(){world.entity_mut(*leaf).insert(RetiredProjection{token:entry.token.clone()});}}
    }
}

/// Refresh metadata/policy after asynchronous asset loading and resize. Every
/// descriptor is re-read under its exact live epoch/revision/identity; no
/// projection clock, elapsed replay, or completion callback is introduced.
pub(crate) fn reconcile(world:&mut World) {
    let Some(engine)=world.get_resource::<crate::resources::VnEngine>() else {return;};
    if engine.0.renderer.scene_batch_pending() || engine.0.renderer.scene_frame_failed {return;}
    let state=engine.0.state.clone();let epoch=engine.0.runtime_epoch();
    let plan=match prepare(world,&state,epoch) {Ok(plan)=>plan,Err(message)=>{
        if let Some(mut engine)=world.get_resource_mut::<crate::resources::VnEngine>(){engine.0.renderer.scene_frame_failed=true;}
        if let Some(mut diagnostic)=world.get_resource_mut::<crate::resources::ScriptErrorMessage>(){diagnostic.0=message;}
        if let Some(mut next)=world.get_resource_mut::<NextState<crate::resources::VnState>>(){next.set(crate::resources::VnState::Error);}
        return;
    }};
    install(world,plan);
}

pub(crate) struct SceneRetiredPlugin;
impl Plugin for SceneRetiredPlugin {
    fn build(&self,app:&mut App) {
        app.init_resource::<RetiredEntities>().add_systems(Update,(reconcile
            .after(crate::scene_publication::HostPublicationSet)
            .after(crate::systems::sprite_system).after(crate::systems::background_system)
            .after(crate::systems::audio_system).after(crate::layered_characters::synchronize)
            .before(crate::programmable_ui::InterfaceInputSet).before(crate::systems::player_input_system)).distributive_run_if(crate::runtime_restart::presentation_available));
    }
}

#[cfg(all(test,not(target_arch="wasm32")))]
mod tests {
    use super::*;
    use bevy::reflect as bevy_reflect;
    use crate::resources::{VnEngine,MusicEntity,MusicAssetRegistry};
    // Typed, in-memory fixture images. This tests no native decoder/GPU. The
    // actual background dimensions/readiness are inserted explicitly below.
    #[derive(Default,bevy::reflect::TypePath)]
    struct FixtureImageLoader;
    impl bevy::asset::AssetLoader for FixtureImageLoader {
        type Asset=Image;type Settings=();type Error=std::io::Error;
        async fn load<'a>(&'a self,_reader:&'a mut bevy::asset::io::Reader<'_>,_settings:&'a Self::Settings,_context:&'a mut bevy::asset::LoadContext<'_>)->Result<Image,Self::Error> {Ok(Image::default())}
        fn extensions(&self)->&[&str] {&["png"]}
    }
    fn app(source:&str)->App {
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        game.step_until_interaction().unwrap();
        let mut app=App::new();
        let directory=bevy::asset::io::memory::Dir::default();
        for path in ["sprites/eileen/old.png","sprites/eileen/new.png","images/old.png","images/new.png","music/old.ogg"] {directory.insert_asset(std::path::Path::new(path),if path.starts_with("music/"){crate::checkpoint_sources::fixture_wave_for_test()}else{vec![0u8]});}
        let reader=bevy::asset::io::memory::MemoryAssetReader{root:directory};
        app.register_asset_source(bevy::asset::io::AssetSourceId::Default,bevy::asset::io::AssetSource::build().with_reader(move||Box::new(reader.clone())));
        app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{watch_for_changes_override:Some(false),..default()}));
        app.init_asset_loader::<FixtureImageLoader>().init_asset_loader::<bevy::audio::AudioLoader>();
        app.init_asset::<Image>().init_asset::<AudioSource>().init_asset::<crate::checkpoint_pcm::CheckpointPcm>().insert_resource(VnEngine(game)).init_resource::<crate::accessibility::Accessibility>().init_resource::<RetiredEntities>().init_resource::<MusicEntity>().init_resource::<MusicAssetRegistry>().init_resource::<crate::scene_publication::ScenePresentation>();
        app.world_mut().spawn((crate::systems::sprite::SpriteStage,SpatialBundle::default()));app
    }
    fn apply(world:&mut World) {
        crate::checkpoint_sources::install_ready_sources_for_test(world).unwrap();
        let game=&world.resource::<VnEngine>().0;
        let plan=prepare(world,&game.state,game.runtime_epoch()).unwrap();install(world,plan);
    }
    fn only(world:&World)->(&TargetIdentity,&Entry) {
        let cache=world.resource::<RetiredEntities>();assert_eq!(cache.entries.len(),1);cache.entries.iter().next().unwrap()
    }
    fn tick(world:&mut World,seconds:f64) {world.resource_mut::<VnEngine>().0.tick_game_frame(seconds,rvn_core::GameFrameReports::default()).unwrap();}
    #[test]
    fn retired_image_has_exact_separate_role_path_and_local_leaf_without_current_alias_or_clock() {
        let mut app=app("scene label A\neileen.show(\"old\") at left\neileen.show(\"new\") at right with fade(4000)\n\"hold\"\n");
        let world=app.world_mut();
        let current=world.spawn((crate::components::VnSprite{id:"eileen".into()},Transform::from_xyz(900.0,800.0,700.0),Sprite::default())).id();
        let state=serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap();apply(world);
        let (_,entry)=only(world);let root=entry.root.unwrap();let leaf=entry.leaves[0];let token=entry.token.clone();
        assert_eq!(world.get::<Parent>(leaf).unwrap().get(),root);
        assert_eq!(*world.get::<Transform>(leaf).unwrap(),Transform::default());
        assert_eq!(world.get::<Transform>(root).unwrap().translation,crate::systems::sprite::sprite_default_transform(&rvn_parser::Position::Left).translation);
        assert!(world.get::<crate::components::VnSprite>(root).is_none() && world.get::<crate::layered_characters::LayerTarget>(leaf).is_none());
        assert!(world.get::<crate::components::FadeAnim>(root).is_none() && world.get::<crate::scene_effects::FadeLease>(root).is_none());
        assert_eq!(&world.get::<RetiredProjection>(leaf).unwrap().token,&token);
        let handle=world.get::<Handle<Image>>(leaf).unwrap();assert_eq!(world.resource::<AssetServer>().get_path(handle.id()).unwrap().path(),std::path::Path::new("sprites/eileen/old.png"));
        assert_eq!(world.get::<Transform>(current).unwrap().translation,Vec3::new(900.0,800.0,700.0));
        apply(world);assert_eq!(only(world).1.root,Some(root));assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(),state);
    }
    #[test]
    fn deferred_background_arrives_at_current_core_elapsed_and_cannot_resurrect_after_completion() {
        let mut app=app("scene label A\nscene \"images/old.png\"\nscene \"images/new.png\" with fade(4000)\n\"hold\"\n");let world=app.world_mut();
        apply(world);assert!(only(world).1.root.is_none());tick(world,1.0);apply(world);assert!(only(world).1.root.is_none());
        let handle=world.resource::<AssetServer>().get_handle::<Image>("images/old.png").unwrap();
        let mut image=Image::default();image.resize(bevy::render::render_resource::Extent3d{width:640,height:360,depth_or_array_layers:1});world.resource_mut::<Assets<Image>>().insert(handle.id(),image);
        apply(world);let (_,entry)=only(world);let root=entry.root.unwrap();let leaf=entry.leaves[0];
        assert_eq!(entry.effect.clock.elapsed,1.0);assert_eq!(world.get::<Sprite>(leaf).unwrap().color.alpha(),0.75);assert_eq!(world.get::<Transform>(root).unwrap().scale,Vec3::new(2.0,2.0,1.0));
        tick(world,3.0);apply(world);assert!(world.get_entity(root).is_none());assert!(world.resource::<RetiredEntities>().entries.is_empty());
        apply(world);assert!(world.resource::<RetiredEntities>().entries.is_empty());
    }
    #[test]
    fn parked_scene_retired_keeps_entity_hides_projection_and_returns_with_fresh_exact_revision() {
        let mut app=app("scene label A\neileen.show(\"old\")\neileen.hide() with fade(4000)\n\"parent\"\n\"returned\"\nscene label B\n\"child\"\nreturn\n");let world=app.world_mut();
        tick(world,1.0);apply(world);let (_,entry)=only(world);let root=entry.root.unwrap();let before=entry.token.clone();
        assert_eq!(*world.get::<Visibility>(root).unwrap(),Visibility::Hidden);
        let handle=world.resource::<AssetServer>().get_handle::<Image>("sprites/eileen/old.png").unwrap();world.resource_mut::<Assets<Image>>().insert(handle.id(),Image::default());
        apply(world);assert_eq!(*world.get::<Visibility>(root).unwrap(),Visibility::Inherited);
        world.resource_mut::<VnEngine>().0.execute_timer_action("call B").unwrap();apply(world);
        assert_eq!(only(world).1.root,Some(root));assert!(only(world).1.parked);assert_eq!(*world.get::<Visibility>(root).unwrap(),Visibility::Hidden);
        tick(world,1.0);apply(world);assert_eq!(only(world).1.effect.clock.elapsed,1.0);
        world.resource_mut::<VnEngine>().0.step_until_interaction().unwrap();world.resource_mut::<VnEngine>().0.advance_dialogue().unwrap();world.resource_mut::<VnEngine>().0.step_until_interaction().unwrap();apply(world);
        // Return resumes after the interrupted parent interaction. A holding
        // dialogue prevents the fixture from immediately entering B again.
        assert!(matches!(world.resource::<VnEngine>().0.current_interaction().unwrap(),
            Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "returned"));
        let rvn_core::scene_tasks::TaskOwner::Scene { activation } = &before.task.key.owner else { panic!("scene-owned retired"); };
        assert_eq!(world.resource::<VnEngine>().0.state.scene_execution.as_ref().unwrap().owner(), Some(*activation));
        let (_,entry)=only(world);assert_eq!(entry.root,Some(root));assert!(!entry.parked);assert_eq!(entry.effect.clock.elapsed,1.0);
        assert_eq!(entry.token.target,before.target);assert_eq!(entry.token.task.runtime_epoch,before.task.runtime_epoch);assert_ne!(entry.token.task.revision,before.task.revision);assert_eq!(*world.get::<Visibility>(root).unwrap(),Visibility::Inherited);
        assert_eq!(&world.get::<RetiredProjection>(root).unwrap().token,&entry.token);
    }
    #[test]
    fn loading_saved_retired_identity_replaces_old_epoch_entity_and_preserves_saved_phase() {
        let mut app=app("scene label A\neileen.show(\"old\")\neileen.hide() with fade(4000)\n\"hold\"\n");let world=app.world_mut();tick(world,1.0);apply(world);
        let (_,entry)=only(world);let root=entry.root.unwrap();let token=entry.token.clone();
        let save=rvn_core::save::SaveData::from_state(&world.resource::<VnEngine>().0.state,1,"hold".into(),"main.rvn".into());
        world.resource_mut::<VnEngine>().0.load_data(serde_json::from_slice(&serde_json::to_vec(&save).unwrap()).unwrap()).unwrap();apply(world);
        let (_,entry)=only(world);assert_ne!(entry.root,Some(root));assert!(world.get_entity(root).is_none());assert_ne!(entry.token.task.runtime_epoch,token.task.runtime_epoch);assert_eq!(entry.token.target,token.target);assert_eq!(entry.effect.clock.elapsed,1.0);
        assert_eq!(world.get::<Sprite>(entry.leaves[0]).unwrap().color.alpha(),0.75);
    }
    #[test]
    fn invalid_resolved_retired_style_refuses_pure_preflight_without_ecs_or_output_changes() {
        let mut app=app("scene label A\neileen.show(\"old\")\neileen.hide() with fade(4000)\n\"hold\"\n");let world=app.world_mut();
        let game=&world.resource::<VnEngine>().0;let mut candidate=game.state.clone();let epoch=game.runtime_epoch();let before=serde_json::to_vec(&game.state).unwrap();let pending=game.renderer.scene_batch_pending();
        let task=candidate.scene_execution.as_mut().unwrap().resources.tasks.iter_mut().find(|task|task.target.kind==TargetKind::PresentationRetired).unwrap();
        if let RetiredModel::Sprite{style,..}=&mut task.effect.as_mut().unwrap().presentation.as_mut().unwrap().model {style.scale=f32::MAX;}else{panic!("sprite required");}
        let entities=world.entities().len();assert!(prepare(world,&candidate,epoch).is_err());assert_eq!(world.entities().len(),entities);assert!(world.resource::<RetiredEntities>().entries.is_empty());
        assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(),before);assert_eq!(world.resource::<VnEngine>().0.renderer.scene_batch_pending(),pending);
    }
    #[test]
    fn exact_old_music_bundle_handoff_preserves_source_entity_and_sets_absolute_gain_before_sink() {
        let mut app=app("scene label A\nmusic.volume(0.4)\nmusic.play(\"old.ogg\")\n\"before\"\nmusic.stop() with fade(4000)\n\"after\"\n");let world=app.world_mut();
        let entity=current_music(world);let handle=world.get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(entity).unwrap().clone();
        world.resource_mut::<VnEngine>().0.advance_dialogue().unwrap();world.resource_mut::<VnEngine>().0.step_until_interaction().unwrap();tick(world,1.0);apply(world);
        let (_,entry)=only(world);assert_eq!(crate::checkpoint_sources::music_entity_for_test(world,&entry.token.target),Some(entity));
        assert!(entry.root.is_none(),"Music is owned by exact Source cache, not the visual cache");
        assert!(world.resource::<MusicEntity>().0.is_none());assert!(world.get::<crate::components::MusicMarker>(entity).is_none());assert!(world.get::<AudioSink>(entity).is_none());assert_eq!(world.get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(entity).unwrap(),&handle);
        assert_eq!(world.get::<PlaybackSettings>(entity).unwrap().volume.get(),(f64::from(0.4f32)*0.75) as f32);assert_eq!(&world.get::<RetiredProjection>(entity).unwrap().token,&entry.token);
    }
    fn current_music(world:&mut World)->Entity {
        crate::checkpoint_sources::install_ready_sources_for_test(world).unwrap();
        let old=world.resource_mut::<VnEngine>().0.renderer.scene_batch.take().unwrap();
        world.resource_mut::<crate::scene_publication::ScenePresentation>().batch=Some(*old);
        world.resource::<MusicEntity>().0.unwrap()
    }
    fn autonomous(world:&mut World,owner:rvn_core::scene_tasks::TaskOwner)->ResourceToken {
        let mut wrapper=world.resource_mut::<VnEngine>();let game=&mut wrapper.0;let epoch=game.runtime_epoch();
        let presentation=rvn_core::retired_presentations::snapshot_retired_source(&game.state,&rvn_core::retired_presentations::PresentationSlot::Music).unwrap().unwrap();
        rvn_core::scene_resources::register_retired_effect(&mut game.state,epoch,owner,presentation,SavedEffect{
            presentation:None,base_resolved:false,clock:rvn_core::scene_resources::EffectClock{duration:4.0,elapsed:0.0,looping:false},
            parameters:rvn_core::scene_resources::EffectParameters::AudioFade{from:1.0,to:0.0,despawn_on_done:true}
        }).unwrap().unwrap()
    }
    #[test]
    fn autonomous_host_retired_music_cannot_take_the_reader_still_owned_by_current_candidate() {
        let mut app=app("scene label A\nmusic.volume(0.4)\nmusic.play(\"old.ogg\")\n\"hold\"\n");let world=app.world_mut();
        let current=current_music(world);let source=world.get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(current).unwrap().clone();
        let token=autonomous(world,rvn_core::scene_tasks::TaskOwner::Host{role:"pause".into()});
        apply(world);let root=crate::checkpoint_sources::music_entity_for_test(world,&token.target).unwrap();assert_ne!(root,current);assert_eq!(only(world).1.token,token);
        assert_eq!(world.resource::<MusicEntity>().0,Some(current));assert!(world.get::<crate::components::MusicMarker>(current).is_some());assert!(world.get::<RetiredProjection>(current).is_none());assert_eq!(world.get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(current).unwrap(),&source);
        assert_ne!(world.get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(root).unwrap(),&source);
    }
    #[test]
    fn two_retired_owners_claim_one_old_reader_once_and_cancelling_one_keeps_the_other() {
        let mut app=app("scene label A\nmusic.volume(0.4)\nmusic.play(\"old.ogg\")\n\"hold\"\n");let world=app.world_mut();let current=current_music(world);
        let first=autonomous(world,rvn_core::scene_tasks::TaskOwner::Game);let second=autonomous(world,rvn_core::scene_tasks::TaskOwner::Host{role:"pause".into()});
        {let mut game=world.resource_mut::<VnEngine>();game.0.state.music.current_file=None;rvn_core::scene_resources::sync_current(&mut game.0.state).unwrap();}
        apply(world);
        let first_root=crate::checkpoint_sources::music_entity_for_test(world,&first.target).unwrap();let second_root=crate::checkpoint_sources::music_entity_for_test(world,&second.target).unwrap();assert_ne!(first_root,second_root);
        assert_eq!([first_root,second_root].into_iter().filter(|entity|*entity==current).count(),1);
        let source=world.get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(second_root).unwrap().clone();let gain=world.get::<PlaybackSettings>(second_root).unwrap().volume.get();
        assert_eq!(&world.get::<RetiredProjection>(first_root).unwrap().token,&first);assert_eq!(&world.get::<RetiredProjection>(second_root).unwrap().token,&second);
        rvn_core::scene_resources::supersede_task(&mut world.resource_mut::<VnEngine>().0.state,&first.task.key).unwrap();apply(world);
        assert!(world.get_entity(first_root).is_none());assert!(world.get_entity(second_root).is_some());assert_eq!(crate::checkpoint_sources::music_entity_for_test(world,&only(world).1.token.target),Some(second_root));assert_eq!(world.get::<Handle<crate::checkpoint_pcm::CheckpointPcm>>(second_root).unwrap(),&source);assert_eq!(world.get::<PlaybackSettings>(second_root).unwrap().volume.get(),gain);
    }
    #[test]
    fn load_epoch_prevents_adopting_a_same_file_old_current_music_reader() {
        let mut app=app("scene label A\nmusic.volume(0.4)\nmusic.play(\"old.ogg\")\n\"hold\"\n");let world=app.world_mut();let current=current_music(world);
        let token=autonomous(world,rvn_core::scene_tasks::TaskOwner::Game);
        {let mut game=world.resource_mut::<VnEngine>();game.0.state.music.current_file=None;rvn_core::scene_resources::sync_current(&mut game.0.state).unwrap();}
        apply(world);assert_eq!(crate::checkpoint_sources::music_entity_for_test(world,&token.target),Some(current));
        let snapshot=world.resource::<VnEngine>().0.snapshot_for_save().unwrap();
        let save=rvn_core::save::SaveData::try_from_snapshot(&snapshot,1,"hold".into(),"main.rvn".into()).unwrap();
        let save=serde_json::from_slice(&serde_json::to_vec(&save).unwrap()).unwrap();assert!(crate::checkpoint_sources::warm_restore(world,&save).unwrap());
        world.resource_mut::<VnEngine>().0.load_data(save).unwrap();apply(world);
        let (_,entry)=only(world);let root=crate::checkpoint_sources::music_entity_for_test(world,&entry.token.target).unwrap();assert_ne!(root,current);assert_ne!(entry.token.task.runtime_epoch,token.task.runtime_epoch);
        assert!(world.get_entity(current).is_none());assert!(world.get::<crate::components::MusicMarker>(root).is_none());assert_eq!(&world.get::<RetiredProjection>(root).unwrap().token,&entry.token);
    }
// PRIVATE NOT_RUN. Insert inside scene_retired::tests, using its actual
// Engine/asset fixture and prepare/install consumers. No GPU/window proof.
mod selected_background_retired_contracts {
    use super::*;
    use rvn_core::retired_presentations::FrozenMotionSample;
    use rvn_ui::motion::Pose;

    fn fixture() -> (App, Entity, Handle<Image>, Handle<Image>) {
        let mut app = app("scene label A\nscene \"images/old.png\"\nscene \"images/new.png\" with fade(4000)\n\"hold\"\n");
        let world = app.world_mut();
        let mut window = Window::default();
        window.resolution.set(1280.0, 720.0);
        let window = world.spawn(window).id();
        {
            let mut game = world.resource_mut::<VnEngine>();
            let task = game.0.state.scene_execution.as_mut().unwrap().resources.tasks.iter_mut()
                .find(|task| task.target.kind == TargetKind::PresentationRetired).unwrap();
            let RetiredModel::Background { sample, .. } = &mut task.effect.as_mut().unwrap()
                .presentation.as_mut().unwrap().model else { panic!("historical Background required") };
            sample.root_motion = Some(FrozenMotionSample {
                pose: Pose { frame: Some("images/new.png".into()), rotation: 90.0,
                    pivot_x: 0.5, pivot_y: 1.0, ..default() },
                reduced_pose: Pose { frame: Some("images/old.png".into()), ..default() },
            });
        }
        apply(world); // Actual pure prepare followed by cache install requests both images.
        let assets = world.resource::<AssetServer>();
        let original = assets.get_handle::<Image>("images/old.png").unwrap();
        let selected = assets.get_handle::<Image>("images/new.png").unwrap();
        for (handle, width, height) in [(&original, 640, 360), (&selected, 320, 640)] {
            let mut image = Image::default();
            image.resize(bevy::render::render_resource::Extent3d { width, height, depth_or_array_layers: 1 });
            world.resource_mut::<Assets<Image>>().insert(handle.id(), image);
        }
        (app, window, original, selected)
    }

    #[test]
    fn retired_background_selected_frame_cover_pivot_resize_and_reduced_pose_share_actual_geometry() {
        let (mut app, window, original, selected) = fixture();
        let state = serde_json::to_vec(&app.world().resource::<VnEngine>().0.state).unwrap();
        let mut identity = None;
        for (width, height, reduced, scale, pivot, texture, alpha) in [
            (1280.0, 720.0, false, 4.0, 1280.0, selected.clone(), 1.0),
            (800.0, 600.0, false, 2.5, 800.0, selected.clone(), 1.0),
            (1280.0, 720.0, true, 2.0, 0.0, original.clone(), 0.0),
            (1280.0, 720.0, false, 4.0, 1280.0, selected.clone(), 1.0),
        ] {
            let world = app.world_mut();
            world.get_mut::<Window>(window).unwrap().resolution.set(width, height);
            world.resource_mut::<crate::accessibility::Accessibility>().settings.reduced_motion = reduced;
            apply(world);
            let (_, entry) = only(world);
            let root = entry.root.unwrap();
            let leaf = entry.leaves[0];
            if let Some((previous_root, previous_leaf, token)) = &identity {
                assert_eq!((root, leaf), (*previous_root, *previous_leaf));
                assert_eq!(&entry.token, token);
            } else { identity = Some((root, leaf, entry.token.clone())); }
            let pose = world.get::<Transform>(root).unwrap();
            assert_eq!(pose.scale, Vec3::new(scale, scale, 1.0));
            assert!(pose.translation.abs_diff_eq(Vec3::new(pivot, -pivot, -100.0), 0.001));
            assert_eq!(world.get::<Handle<Image>>(leaf), Some(&texture));
            assert!(world.get::<Sprite>(leaf).unwrap().custom_size.is_none());
            assert_eq!(world.get::<Sprite>(leaf).unwrap().color.alpha(), alpha);
            assert_eq!(entry.effect.clock.elapsed, 0.0);
            assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(), state);
        }
    }

    #[test]
    fn selected_background_geometry_defers_missing_frame_and_preserves_explicit_captured_base() {
        let (mut app, _, _, selected) = fixture();
        let world = app.world_mut();
        world.resource_mut::<Assets<Image>>().remove(selected.id());
        tick(world, 1.0);
        apply(world);
        assert!(only(world).1.root.is_none());
        assert_eq!(only(world).1.effect.clock.elapsed, 1.0);
        let mut image = Image::default();
        image.resize(bevy::render::render_resource::Extent3d { width: 320, height: 640, depth_or_array_layers: 1 });
        world.resource_mut::<Assets<Image>>().insert(selected.id(), image);
        apply(world);
        let (_, entry) = only(world);
        assert_eq!(world.get::<Sprite>(entry.leaves[0]).unwrap().color.alpha(), 0.75);
        assert_eq!(world.get::<Transform>(entry.root.unwrap()).unwrap().scale, Vec3::new(4.0, 4.0, 1.0));
        // The logical cover is not permission to overwrite a measured source base.
        {
            let mut game = world.resource_mut::<VnEngine>();
            let task = game.0.state.scene_execution.as_mut().unwrap().resources.tasks.iter_mut()
                .find(|task| task.target.kind == TargetKind::PresentationRetired).unwrap();
            let RetiredModel::Background { sample, .. } = &mut task.effect.as_mut().unwrap().presentation.as_mut().unwrap().model else { unreachable!() };
            sample.fade = Some(rvn_core::retired_presentations::FrozenFadeSample {
                clock: rvn_core::scene_resources::EffectClock { duration: 2.0, elapsed: 0.0, looping: false },
                from: 1.0, to: 1.0, transition: rvn_core::scene_resources::VisualTransition::Fade,
                base: TransformSnapshot { translation: [7.0, 9.0, -100.0], scale: [1.5, 1.5, 1.0], ..default() },
                base_resolved: true, layered: false,
            });
        }
        apply(world);
        let root = only(world).1.root.unwrap();
        let pose = world.get::<Transform>(root).unwrap();
        assert_eq!(pose.scale, Vec3::new(1.5, 1.5, 1.0));
        assert!(pose.translation.abs_diff_eq(Vec3::new(487.0, -471.0, -100.0), 0.001));
        assert_eq!(only(world).1.effect.clock.elapsed, 1.0);
    }
}

}
