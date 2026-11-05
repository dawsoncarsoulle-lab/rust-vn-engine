// PRIVATE NOT_RUN. Local render-phase ordering; no saved state, Transform.z,
// sort_key, vendor pipeline, elapsed clock, or appearance sampling is changed.
use bevy::prelude::*;
use bevy::core_pipeline::core_2d::Transparent2d;
use bevy::render::{extract_component::{ExtractComponent, ExtractComponentPlugin},
    render_phase::{sort_phase_system, ViewSortedRenderPhases}, Render, RenderApp, RenderSet};
use bevy::sprite::ExtractedSprites;
use bevy::math::FloatOrd;
use rvn_core::retired_presentations::PresentationSlot;
use rvn_core::scene_resources::{ResourceToken, TargetIdentity, TargetKind};
use rvn_core::scene_tasks::TaskOwner;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "startup_presentation_tests.rs"]
pub(crate) mod startup_owner_tests;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Band { Background, Sprite, Cinematic }
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Identity { alias: String, incarnation: u64 }
impl From<&TargetIdentity> for Identity {
    fn from(target: &TargetIdentity) -> Self {
        Self { alias: target.alias.clone(), incarnation: target.incarnation }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum StoryOwner { Game, Scene(u64) }
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Historical {
    Story { identity: Identity, owner: StoryOwner, revision: u64 },
    Host { role: String, identity: Identity, revision: u64 },
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Appearance { Retired(Historical), Current(Identity) }
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key { band: Band, slot: String, appearance: Appearance, authored_leaf: usize }

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(crate) struct PresentationDrawOrder(Key);
// Bevy 0.14 derives do not discover target-specific manifest dependencies.
// This is the same unconditional clone extraction emitted by its derive.
impl ExtractComponent for PresentationDrawOrder {
    type QueryData = &'static Self;
    type QueryFilter = ();
    type Out = Self;
    fn extract_component(item: bevy::ecs::query::QueryItem<'_, Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}
impl PresentationDrawOrder {
    pub(crate) fn current(target: &TargetIdentity, authored_leaf: usize) -> Option<Self> {
        let band = match target.kind { TargetKind::Background => Band::Background,
            TargetKind::Sprite => Band::Sprite, TargetKind::Cinematic=>Band::Cinematic, _ => return None };
        Some(Self(Key { band, slot: target.alias.clone(), appearance: Appearance::Current(target.into()), authored_leaf }))
    }
    pub(crate) fn retired(slot: &PresentationSlot, token: &ResourceToken, authored_leaf: usize) -> Option<Self> {
        let (band, slot) = match slot {
            PresentationSlot::Background => (Band::Background, "background".to_owned()),
            PresentationSlot::Sprite { id } => (Band::Sprite, id.clone()),
            PresentationSlot::Music => return None,
            PresentationSlot::Cinematic=>(Band::Cinematic,"cinematic".to_string()),
        };
        let identity = (&token.target).into();
        let revision = token.task.revision;
        let history = match &token.task.key.owner {
            TaskOwner::Game => Historical::Story { identity, owner: StoryOwner::Game, revision },
            TaskOwner::Scene { activation } => Historical::Story { identity, owner: StoryOwner::Scene(activation.0), revision },
            TaskOwner::Host { role } => Historical::Host { role: role.clone(), identity, revision },
        };
        Some(Self(Key { band, slot, appearance: Appearance::Retired(history), authored_leaf }))
    }
    fn band_depth(&self) -> FloatOrd {
        FloatOrd(match self.0.band { Band::Background => -100.0, Band::Sprite => 10.0, Band::Cinematic=>50.0 })
    }
}

pub(crate) struct PresentationOrderPlugin;
impl Plugin for PresentationOrderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<PresentationDrawOrder>::default())
            .add_systems(PostUpdate, (tag_current.after(crate::composed_motion::MotionApply)).distributive_run_if(crate::runtime_restart::presentation_available));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(Render, sort_managed_phases
                .in_set(RenderSet::PhaseSort)
                .after(sort_phase_system::<Transparent2d>)
                .before(RenderSet::Prepare));
        }
    }
}

// Only opted Current objects get keys. Retired leaves are tagged by their
// actual install producer from the historical payload and authored layer index.
fn tag_current(world: &mut World) {
    // A candidate may be ahead of the last installed frame. Keep its accepted
    // keys until the same real owner publishes; never choose Game over Startup
    // when both resources coexist and never infer a target from an alias alone.
    let Ok(owner) = crate::text_runtime_owner::current(world) else { return; };
    if owner.blocked || world.get_resource::<crate::debugger_startup::DebugBootstrap>()
        .is_some_and(|startup| !startup.accepts_actual_audio_broker()) { return; }
    let stale: Vec<_> = world.iter_entities().filter(|entity|
        entity.contains::<PresentationDrawOrder>() &&
        !entity.contains::<crate::scene_retired::RetiredProjection>())
        .map(|entity| entity.id()).collect();
    let keys: Vec<_> = owner.state.scene_execution.as_ref()
        .map(|execution| world.iter_entities().filter_map(|entity| {
            if entity.contains::<crate::scene_retired::RetiredProjection>() { return None; }
            let (kind, alias, index) = if entity.contains::<crate::components::CurrentMotionBackground>() {
                (TargetKind::Background, "background", 0)
            } else if let Some(layer) = entity.get::<crate::layered_characters::LayerTarget>() {
                let composition = owner.state.layered.characters.get(&layer.character)?;
                let index = composition.definition.layers.iter().position(|leaf| leaf.id == layer.layer)?;
                (TargetKind::Sprite, layer.character.as_str(), index)
            } else if let Some(cg)=entity.get::<crate::systems::cinematic::CurrentCinematic>() {
                if cg.instance_id!=owner.instance || cg.epoch!=owner.epoch || execution.resources.target(&TaskOwner::Game,TargetKind::Cinematic,"cinematic")!=Some(&cg.target) {return None;}
                (TargetKind::Cinematic,"cinematic",0)
            } else if let Some(sprite) = entity.get::<crate::components::VnSprite>() {
                (TargetKind::Sprite, sprite.id.as_str(), 0)
            } else { return None; };
            let target = execution.resources.target(&TaskOwner::Game, kind, alias)?;
            Some((entity.id(), PresentationDrawOrder::current(target, index)?))
        }).collect()).unwrap_or_default();
    for entity in stale { world.entity_mut(entity).remove::<PresentationDrawOrder>(); }
    for (entity, key) in keys { world.entity_mut(entity).insert(key); }
}

fn reorder(items: &mut Vec<Transparent2d>, mut key_for: impl FnMut(Entity) -> Option<PresentationDrawOrder>) {
    let mut rows: Vec<_> = items.drain(..).enumerate().map(|(index, item)| {
        let key = key_for(item.entity);
        let depth = key.as_ref().map_or(item.sort_key, PresentationDrawOrder::band_depth);
        (depth, key, index, item)
    }).collect();
    if rows.iter().all(|row| row.1.is_none()) {
        items.extend(rows.into_iter().map(|row| row.3));
        return; // Legacy/foreign-only phase preserves the exact standard result.
    }
    rows.sort_by(|a,b| a.0.0.total_cmp(&b.0.0)
        // At equal band depth, foreign precedes VN. This is one TOTAL key,
        // never a mixed identity/index comparator with potential cycles.
        .then_with(|| a.1.is_some().cmp(&b.1.is_some()))
        .then_with(|| match (&a.1, &b.1) {
            (Some(a), Some(b)) => a.0.cmp(&b.0),
            _ => std::cmp::Ordering::Equal,
        }).then_with(|| a.2.cmp(&b.2)));
    items.extend(rows.into_iter().map(|row| row.3));
}
fn sort_managed_phases(mut phases: ResMut<ViewSortedRenderPhases<Transparent2d>>,
    sprites: Res<ExtractedSprites>, markers: Query<&PresentationDrawOrder>) {
    for phase in phases.values_mut() {
        reorder(&mut phase.items, |entity| {
            // Sliced sprites use an auxiliary render entity but retain their
            // original source entity. Its logical appearance owns every slice.
            let original = sprites.sprites.get(&entity).and_then(|sprite| sprite.original_entity).unwrap_or(entity);
            markers.get(original).ok().cloned()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::render::{render_phase::{Draw, DrawFunctions, SortedPhaseItem, PhaseItemExtraIndex, TrackedRenderPass},
        render_resource::CachedRenderPipelineId};
    use rvn_core::scene_tasks::{TaskKey, TaskToken};

    struct NoDraw;
    impl Draw<Transparent2d> for NoDraw {
        fn draw<'w>(&mut self, _: &'w World, _: &mut TrackedRenderPass<'w>, _: Entity, _: &Transparent2d) { unreachable!("CPU ordering never issues a GPU draw"); }
    }
    fn phase_item(entity: Entity, depth: f32, draw: bevy::render::render_phase::DrawFunctionId) -> Transparent2d {
        Transparent2d { sort_key: FloatOrd(depth), entity, pipeline: CachedRenderPipelineId::INVALID,
            draw_function: draw, batch_range: 0..0, extra_index: PhaseItemExtraIndex::NONE }
    }
    fn token(owner: TaskOwner, alias: &str, incarnation: u64, revision: u64) -> ResourceToken {
        ResourceToken { target: TargetIdentity { kind: TargetKind::PresentationRetired, alias: alias.into(), incarnation },
            task: TaskToken { runtime_epoch: 99, key: TaskKey { owner, local_id: format!("effect:{alias}") }, revision } }
    }
    fn current(id: &str, index: usize) -> PresentationDrawOrder {
        PresentationDrawOrder::current(&TargetIdentity { kind: TargetKind::Sprite, alias: id.into(), incarnation: 70 }, index).unwrap()
    }
    fn retired(id: &str, owner: TaskOwner, alias: &str, incarnation: u64, revision: u64, index: usize) -> PresentationDrawOrder {
        PresentationDrawOrder::retired(&PresentationSlot::Sprite { id: id.into() }, &token(owner,alias,incarnation,revision),index).unwrap()
    }
    fn sorted(labels: &[(&str, f32, Option<PresentationDrawOrder>)]) -> Vec<String> {
        let mut world = World::new();
        let draw_functions = DrawFunctions::<Transparent2d>::default();
        let draw = draw_functions.write().add(NoDraw);
        let view = world.spawn_empty().id();
        let mut names = std::collections::HashMap::new();
        let mut items = Vec::new();
        for (name, depth, key) in labels {
            let entity = world.spawn((Transform::from_xyz(123.0, -456.0, *depth), Sprite::default())).id();
            if let Some(key) = key { world.entity_mut(entity).insert(key.clone()); }
            names.insert(entity, name.to_string()); items.push(phase_item(entity,*depth,draw));
        }
        let before: Vec<_> = world.iter_entities().filter_map(|entity| entity.get::<Transform>().map(|t|(entity.id(),*t))).collect();
        Transparent2d::sort(&mut items); // Actual Bevy standard depth sort first.
        let mut phases = ViewSortedRenderPhases::<Transparent2d>::default();
        phases.insert_or_clear(view); phases.get_mut(&view).unwrap().items = items;
        world.insert_resource(phases); world.init_resource::<ExtractedSprites>();
        world.run_system_once(sort_managed_phases);
        for (entity, pose) in before {
            let actual=world.get::<Transform>(entity).unwrap();
            // IEEE NaN != itself; immutable depth/pose are compared bit-exactly.
            assert_eq!(actual.translation.to_array().map(f32::to_bits),pose.translation.to_array().map(f32::to_bits));
            assert_eq!(actual.rotation.to_array().map(f32::to_bits),pose.rotation.to_array().map(f32::to_bits));
            assert_eq!(actual.scale.to_array().map(f32::to_bits),pose.scale.to_array().map(f32::to_bits));
        }
        let phase = &world.resource::<ViewSortedRenderPhases<Transparent2d>>()[&view];
        for item in &phase.items { assert_eq!(item.sort_key.0.to_bits(),world.get::<Transform>(item.entity).unwrap().translation.z.to_bits()); }
        phase.items.iter().map(|item|names[&item.entity].clone()).collect()
    }

    #[test]
    fn actual_phase_groups_whole_compositions_by_sorted_slot_then_retired_then_current_independent_of_allocation() {
        let entries = vec![
            ("zeta.current", -90.0, Some(current("zeta",0))),
            ("alpha.current.7", -30.0, Some(current("alpha",7))),
            ("alpha.old.7", 9000.0, Some(retired("alpha",TaskOwner::Game,"retired:a",5,2,7))),
            ("alpha.current.0", 70.0, Some(current("alpha",0))),
            ("alpha.old.0", -999.0, Some(retired("alpha",TaskOwner::Game,"retired:a",5,2,0))),
        ];
        let expected = ["alpha.old.0","alpha.old.7","alpha.current.0","alpha.current.7","zeta.current"];
        assert_eq!(sorted(&entries),expected);
        assert_eq!(sorted(&entries.into_iter().rev().collect::<Vec<_>>()),expected);
    }

    #[test]
    fn actual_phase_preserves_foreign_order_and_foreign_tie_before_vn_without_rewriting_depth() {
        let entries = vec![
            ("foreign.high",11.0,None), ("current",-1000.0,Some(current("hero",0))),
            ("foreign.tie.a",10.0,None), ("foreign.low",-101.0,None),
            ("foreign.tie.b",10.0,None),
            ("background",555.0,PresentationDrawOrder::current(&TargetIdentity{kind:TargetKind::Background,alias:"background".into(),incarnation:2},0)),
        ];
        assert_eq!(sorted(&entries),["foreign.low","background","foreign.tie.a","foreign.tie.b","current","foreign.high"]);
        assert_eq!(sorted(&[("legacy.a",10.0,None),("legacy.b",10.0,None),("legacy.low",-10.0,None)]),["legacy.low","legacy.a","legacy.b"]);
    }

    #[test]
    fn host_history_orders_by_role_then_exact_identity_revision_and_authored_leaf_not_entity_index() {
        let host = |role:&str| TaskOwner::Host{role:role.into()};
        let entries=vec![
            ("pause.z",10.0,Some(retired("hero",host("pause"),"z",3,1,0))),
            ("preferences.a",10.0,Some(retired("hero",host("preferences"),"a",1,1,0))),
            ("pause.a.rev2",10.0,Some(retired("hero",host("pause"),"a",2,2,0))),
            ("pause.a.rev1",10.0,Some(retired("hero",host("pause"),"a",2,1,0))),
            ("pause.a.inc1",10.0,Some(retired("hero",host("pause"),"a",1,9,0))),
            ("current",10.0,Some(current("hero",0))),
        ];
        assert_eq!(sorted(&entries),["pause.a.inc1","pause.a.rev1","pause.a.rev2","pause.z","preferences.a","current"]);
    }

    #[test]
    fn current_marker_waits_for_real_game_publication_and_legacy_removes_opted_keys() {
        let mut game=rvn_core::Engine::from_program(
            rvn_parser::parse_program("scene label A\nscene \"wide.png\"\nhero.show(\"one\")\n\"hold\"\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        game.step_until_interaction().unwrap();
        let before=serde_json::to_vec(&game.state).unwrap();
        let target=game.state.scene_execution.as_ref().unwrap().resources
            .target(&TaskOwner::Game,TargetKind::Sprite,"hero").unwrap().clone();
        let mut world=World::new();
        world.insert_resource(crate::resources::VnEngine(game));
        let pose=Transform::from_xyz(13.0,17.0,23.0);
        let sprite=world.spawn((crate::components::VnSprite{id:"hero".into()},pose)).id();
        tag_current(&mut world);
        // This World has not installed its BevyRenderer publication. An actual
        // Core target does not authorize projecting the candidate yet.
        assert!(world.get::<PresentationDrawOrder>(sprite).is_none());
        assert_eq!(world.get::<Transform>(sprite),Some(&pose));
        assert_eq!(serde_json::to_vec(&world.resource::<crate::resources::VnEngine>().0.state).unwrap(),before);
        let legacy=rvn_core::Engine::new(rvn_parser::parse("label A\n\"hold\"\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        world.entity_mut(sprite).insert(PresentationDrawOrder::current(&target,0).unwrap());
        world.insert_resource(crate::resources::VnEngine(legacy));
        tag_current(&mut world);
        assert!(world.get::<PresentationDrawOrder>(sprite).is_none());
        assert_eq!(world.get::<Transform>(sprite),Some(&pose));
    }

    #[test]
    fn actual_sliced_sprite_phase_uses_original_entity_marker_instead_of_auxiliary_allocation() {
        let mut world=World::new();
        let draw_functions=DrawFunctions::<Transparent2d>::default();
        let draw=draw_functions.write().add(NoDraw);
        let original=world.spawn(current("alpha",0)).id();
        let current_zeta=world.spawn(current("zeta",0)).id();
        let slice=world.spawn_empty().id();
        let view=world.spawn_empty().id();
        let mut sprites=ExtractedSprites::default();
        sprites.sprites.insert(slice,bevy::sprite::ExtractedSprite {
            transform:GlobalTransform::default(),color:Color::WHITE.to_linear(),rect:None,custom_size:None,
            image_handle_id:Handle::<Image>::default().id(),flip_x:false,flip_y:false,anchor:Vec2::ZERO,
            original_entity:Some(original),
        });
        let mut phases=ViewSortedRenderPhases::<Transparent2d>::default();
        phases.insert_or_clear(view);
        phases.get_mut(&view).unwrap().items=vec![phase_item(current_zeta,10.0,draw),phase_item(slice,10.0,draw)];
        world.insert_resource(phases);world.insert_resource(sprites);
        world.run_system_once(sort_managed_phases);
        let items=&world.resource::<ViewSortedRenderPhases<Transparent2d>>()[&view].items;
        assert_eq!(items.iter().map(|item|item.entity).collect::<Vec<_>>(),[slice,current_zeta]);
        assert!(world.get::<PresentationDrawOrder>(slice).is_none());
        assert_eq!(items.iter().map(|item|item.sort_key.0).collect::<Vec<_>>(),[10.0,10.0]);
    }
    #[test]
    fn actual_foreign_ieee_depth_bits_keep_standard_radix_order_with_managed_band_present() {
        let foreign=vec![
            ("positive.nan",f32::from_bits(0x7fc00003),None),
            ("positive.zero",0.0,None),
            ("negative.infinity",f32::NEG_INFINITY,None),
            ("negative.zero",-0.0,None),
            ("negative.nan",f32::from_bits(0xffc00004),None),
            ("positive.infinity",f32::INFINITY,None),
            ("foreign.tie.a",10.0,None),
            ("foreign.tie.b",10.0,None),
        ];
        let standard=sorted(&foreign); // Actual Transparent2d standard result, all bits retained.
        let mut managed=foreign;
        managed.insert(2,("current",0.0,Some(current("hero",0))));
        let actual=sorted(&managed);
        assert_eq!(actual.iter().filter(|name|name.as_str()!="current").cloned().collect::<Vec<_>>(),standard);
        assert!(actual.iter().position(|name|name=="foreign.tie.b").unwrap()
            <actual.iter().position(|name|name=="current").unwrap());
    }

}
