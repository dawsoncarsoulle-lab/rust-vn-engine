use super::*;
use crate::{
    bevy_renderer::BevyRenderer,
    components::{ChoiceContainer, DialogueBox},
    menu_documents::{ChoicesRoot, NarrativeRoot},
    resources::VnEngine,
};

fn fixture() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::transform::TransformPlugin,
        bevy::render::view::VisibilityPlugin,
    ));
    app.init_resource::<Assets<Mesh>>();
    app.insert_resource(VnEngine(
        rvn_core::Engine::new(vec![], BevyRenderer::new(), 16).unwrap(),
    ));
    app.add_systems(
        PostUpdate,
        cinematic_visibility
            .after(bevy::transform::TransformSystem::TransformPropagate)
            .before(bevy::render::view::VisibilitySystems::VisibilityPropagate),
    );
    app
}

fn play(app: &mut App, cinematic: bool, wait: bool) {
    let clip = VideoClip::parse(serde_json::json!({
        "source":"original.webm", "cinematic":cinematic,
    }))
    .unwrap();
    let videos = &mut app.world_mut().resource_mut::<VnEngine>().0.state.videos;
    videos.play("scene".into(), clip).unwrap();
    if wait {
        videos.wait("scene").unwrap();
    }
}

fn roots(app: &mut App) -> Vec<(Entity, Visibility)> {
    vec![
        (
            app.world_mut()
                .spawn((DialogueBox, SpatialBundle::default()))
                .id(),
            Visibility::Inherited,
        ),
        (
            app.world_mut()
                .spawn((
                    ChoiceContainer,
                    SpatialBundle {
                        visibility: Visibility::Hidden,
                        ..default()
                    },
                ))
                .id(),
            Visibility::Hidden,
        ),
        (
            app.world_mut()
                .spawn((
                    NarrativeRoot,
                    SpatialBundle {
                        visibility: Visibility::Visible,
                        ..default()
                    },
                ))
                .id(),
            Visibility::Visible,
        ),
        (
            app.world_mut()
                .spawn((ChoicesRoot, SpatialBundle::default()))
                .id(),
            Visibility::Inherited,
        ),
    ]
}

#[test]
fn cinematic_hides_legacy_and_custom_roots_then_restores_exact_visibility() {
    let mut app = fixture();
    let roots = roots(&mut app);
    // The subtitle belongs to its video entity, not to either narrative UI.
    let video = app.world_mut().spawn(SpatialBundle::default()).id();
    let caption = app
        .world_mut()
        .spawn((
            Text::from_section("Original subtitle", default()),
            SpatialBundle::default(),
        ))
        .id();
    app.world_mut().entity_mut(video).add_child(caption);
    let hud = app.world_mut().spawn(SpatialBundle::default()).id();
    play(&mut app, true, true);
    app.update();
    for (entity, _) in &roots {
        assert_eq!(
            app.world().get::<Visibility>(*entity),
            Some(&Visibility::Hidden)
        );
        assert!(!app
            .world()
            .get::<InheritedVisibility>(*entity)
            .unwrap()
            .get());
    }
    for entity in [video, caption, hud] {
        assert!(
            app.world()
                .get::<InheritedVisibility>(entity)
                .unwrap()
                .get(),
            "Video, caption and unrelated HUD must remain visible"
        );
        assert!(app.world().get::<CinematicHidden>(entity).is_none());
    }
    app.world_mut()
        .resource_mut::<VnEngine>()
        .0
        .state
        .videos
        .stop("scene");
    app.update();
    for (entity, previous) in roots {
        assert_eq!(app.world().get::<Visibility>(entity), Some(&previous));
        assert!(app.world().get::<CinematicHidden>(entity).is_none());
    }
}

#[test]
fn cinematic_policy_is_applied_after_presentation_rewrites_in_the_same_frame() {
    fn presentation(mut roots: Query<&mut Visibility, With<NarrativeRoot>>) {
        for mut visibility in &mut roots {
            *visibility = Visibility::Visible;
        }
    }
    let mut app = fixture();
    app.add_systems(
        PostUpdate,
        presentation.before(bevy::transform::TransformSystem::TransformPropagate),
    );
    let root = app
        .world_mut()
        .spawn((NarrativeRoot, SpatialBundle::default()))
        .id();
    let child = app.world_mut().spawn(SpatialBundle::default()).id();
    app.world_mut().entity_mut(root).add_child(child);
    play(&mut app, true, true);
    for _ in 0..3 {
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(root),
            Some(&Visibility::Hidden)
        );
        assert!(
            !app.world().get::<InheritedVisibility>(child).unwrap().get(),
            "Propagation must hide descendants in the same frame"
        );
    }
    app.world_mut()
        .resource_mut::<VnEngine>()
        .0
        .state
        .videos
        .feedback("scene", rvn_core::video::Feedback::End)
        .unwrap();
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(root),
        Some(&Visibility::Visible)
    );
    assert!(app.world().get::<InheritedVisibility>(child).unwrap().get());
}

#[test]
fn roots_created_during_cinematic_are_hidden_and_restored_after_terminal_feedback() {
    let mut app = fixture();
    play(&mut app, true, true);
    app.update();
    let root = app
        .world_mut()
        .spawn((ChoicesRoot, SpatialBundle::default()))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(root),
        Some(&Visibility::Hidden)
    );
    app.world_mut()
        .resource_mut::<VnEngine>()
        .0
        .state
        .videos
        .feedback(
            "scene",
            rvn_core::video::Feedback::Error("Controlled decoder failure".into()),
        )
        .unwrap();
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(root),
        Some(&Visibility::Inherited)
    );
    assert!(app.world().get::<CinematicHidden>(root).is_none());
}

#[test]
fn embedded_or_nonwaiting_video_leaves_narrative_visibility_unchanged() {
    for (cinematic, wait) in [(false, true), (true, false)] {
        let mut app = fixture();
        let roots = roots(&mut app);
        play(&mut app, cinematic, wait);
        app.update();
        for (entity, previous) in roots {
            assert_eq!(app.world().get::<Visibility>(entity), Some(&previous));
            assert!(app.world().get::<CinematicHidden>(entity).is_none());
        }
    }
}
