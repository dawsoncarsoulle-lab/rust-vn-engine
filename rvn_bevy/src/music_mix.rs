//! Host music preferences compose with the story's sampled logical gain.
//! Mixing never edits saved state, effect clocks, tokens or reader identity.
use bevy::prelude::*;

pub(crate) fn final_gain(logical: f32, preference: f32) -> Result<f32, String> {
    if !logical.is_finite() || !(0.0..=1.0).contains(&logical) {
        return Err("Invalid logical music gain".into());
    }
    if !preference.is_finite() || !(0.0..=1.0).contains(&preference) {
        return Err("Invalid Host music preference".into());
    }
    Ok((f64::from(logical) * f64::from(preference)) as f32)
}

pub(crate) fn user_gain(world: &World) -> Result<f32, String> {
    let preference = world
        .get_resource::<crate::systems::settings_menu::Settings>()
        .map(|settings| settings.music_volume)
        .unwrap_or(1.0);
    final_gain(1.0, preference)
}

pub(crate) fn in_world(world: &World, logical: f32) -> Result<f32, String> {
    final_gain(logical, user_gain(world)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{MusicEntity, VnEngine, VnState};

    fn fixture() -> (World, Entity) {
        let mut game = rvn_core::Engine::from_program(
            rvn_parser::parse_program("scene label A\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\") with fade(4000)\n\"before\"\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),
            32,
        ).unwrap();
        game.step_until_interaction().unwrap();
        game.tick_game_frame(1.0, rvn_core::GameFrameReports::default()).unwrap();
        game.renderer.scene_batch.take();
        let epoch = game.runtime_epoch();
        let effects = game.state.scene_execution.as_ref().unwrap().resources.active_effects(game.state.scene_execution.as_ref().unwrap().owner()).into_iter().cloned().collect();
        let mut world = World::new();
        world.init_resource::<Time<()>>();
        world.insert_resource(VnEngine(game));
        world.insert_resource(State::new(VnState::Waiting));
        world.insert_resource(crate::systems::settings_menu::Settings { music_volume: 0.25, ..default() });
        let entity = world.spawn((crate::components::MusicMarker, PlaybackSettings::LOOP,)).id();
        world.insert_resource(MusicEntity(Some(entity)));
        crate::scene_effects::install(&mut world, effects, epoch);
        (world, entity)
    }

    #[test]
    fn host_music_preference_multiplies_the_logical_gain_once_and_rejects_invalid_inputs() {
        assert_eq!(final_gain(0.4, 0.25).unwrap().to_bits(), 0.1f32.to_bits());
        assert_eq!(final_gain(0.4, 1.0).unwrap().to_bits(), 0.4f32.to_bits());
        assert_eq!(final_gain(0.0, 0.25).unwrap(), 0.0);
        assert_eq!(final_gain(0.4, 0.0).unwrap(), 0.0);
        for invalid in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(final_gain(invalid, 1.0).is_err());
            assert!(final_gain(0.4, invalid).is_err());
        }
    }

    #[test]
    fn host_music_preference_refreshes_pending_current_gain_without_advancing_or_rebasing_the_story() {
        let (mut world, entity) = fixture();
        let before = serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap();
        for _ in 0..5 {
            crate::scene_effects::reconcile_for_test(&mut world);
            assert_eq!(world.get::<PlaybackSettings>(entity).unwrap().volume.get().to_bits(), 0.025f32.to_bits());
            assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(), before);
            assert_eq!(world.resource::<MusicEntity>().0, Some(entity));
        }
        world.resource_mut::<crate::systems::settings_menu::Settings>().music_volume = 0.5;
        crate::scene_effects::reconcile_for_test(&mut world);
        assert_eq!(world.get::<PlaybackSettings>(entity).unwrap().volume.get().to_bits(), 0.05f32.to_bits());
        assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(), before);
    }

    #[test]
    fn absent_host_settings_is_unity_and_source_gain_is_independent_of_later_host_preferences() {
        let mut world = World::new();
        assert_eq!(in_world(&world, 0.4).unwrap(), 0.4);
        let saved_source_gain = 0.4;
        let outgoing_logical = rvn_core::effect_sampling::retired_music_gain(saved_source_gain,
            &rvn_core::scene_resources::EffectClock {duration:4.0,elapsed:1.0,looping:false});
        world.insert_resource(crate::systems::settings_menu::Settings { music_volume: 0.25, ..default() });
        assert_eq!(in_world(&world, outgoing_logical).unwrap().to_bits(), 0.075f32.to_bits());
        world.resource_mut::<crate::systems::settings_menu::Settings>().music_volume = 0.5;
        assert_eq!(in_world(&world, outgoing_logical).unwrap().to_bits(), 0.15f32.to_bits());
        assert_eq!(saved_source_gain, 0.4);
        assert_eq!(outgoing_logical.to_bits(), 0.3f32.to_bits());
    }

    fn run_settings_bridge(world: &mut World) {
        use bevy::ecs::system::RunSystemOnce;
        world.init_resource::<crate::resources::TypewriterConfig>();
        world.init_resource::<crate::resources::TypewriterState>();
        world.init_resource::<crate::resources::CharacterRegistry>();
        world.init_resource::<crate::resources::DialogueHistory>();
        world.init_resource::<crate::resources::ScriptErrorMessage>();
        world.init_resource::<NextState<VnState>>();
        world.init_resource::<Events<crate::vn_command::VnCommand>>();
        world.run_system_once(crate::systems::settings_menu::apply_settings_to_runtime_system);
    }

    #[test]
    fn real_host_settings_bridge_keeps_narrative_logical_volume_effects_and_tokens_exact() {
        let (mut world, entity) = fixture();
        world.insert_resource(crate::resources::MusicVolume(0.4));
        let before = serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap();
        let epoch = world.resource::<VnEngine>().0.runtime_epoch();
        run_settings_bridge(&mut world);
        assert_eq!(world.resource::<crate::resources::MusicVolume>().0, 0.4);
        assert_eq!(serde_json::to_vec(&world.resource::<VnEngine>().0.state).unwrap(), before);
        assert_eq!(world.resource::<VnEngine>().0.runtime_epoch(), epoch);
        crate::scene_effects::reconcile_for_test(&mut world);
        assert_eq!(world.get::<PlaybackSettings>(entity).unwrap().volume.get().to_bits(), 0.025f32.to_bits());
    }

    #[test]
    fn real_host_settings_bridge_preserves_the_existing_legacy_volume_assignment() {
        let game = rvn_core::Engine::from_program(
            rvn_parser::parse_program("label start\n\"before\"\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(), 32,
        ).unwrap();
        assert!(game.state.scene_execution.is_none());
        let mut world = World::new();
        world.insert_resource(VnEngine(game));
        world.insert_resource(crate::resources::MusicVolume(0.4));
        world.insert_resource(crate::systems::settings_menu::Settings { music_volume: 0.25, ..default() });
        run_settings_bridge(&mut world);
        assert_eq!(world.resource::<crate::resources::MusicVolume>().0, 0.25);
        assert!(world.resource::<VnEngine>().0.state.scene_execution.is_none());
    }
}
