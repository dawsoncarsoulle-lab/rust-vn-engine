use super::*;
use crate::{
    bevy_renderer::BevyRenderer,
    components::{ChoiceContainer, DialogueBox},
    menu_documents::{ChoicesRoot, NarrativeRoot},
    resources::VnEngine,
};

#[cfg(feature = "video")]
pub(crate) fn resident_game_fixture() -> rvn_core::Engine<BevyRenderer> {
    let source=r#"
screen shared(marker) {return component("root","column",{},[
    component("player","panel",{},[]),component("label","text",{"text":marker},[])])}
handler setup_media(event) {
    video.play("embedded",video_clip("parent.webm",{"target":"ui:shared/player"}))
    motion.play("ui:shared/player",motion_pause(10))
}
init {video.play("bootstrap",video_clip("bootstrap.webm",{"on_end":"setup_media"}))}
scene label P
ui.open("shared",["parent"],false,0)
"parent"
call C
"returned"
jump finish
scene label C
ui.open("shared",["child"],false,0)
"child"
return
label finish
"#;
    let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),BevyRenderer::new(),32).unwrap();
    assert!(matches!(game.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{text,..}) if text=="parent"));
    let bootstrap=game.video_views().unwrap().into_iter().find(|view|view.id=="bootstrap").unwrap();
    game.video_feedback_from_view(&bootstrap,rvn_core::video::Feedback::End).unwrap();
    game
}
#[cfg(feature = "video")]
pub(crate) fn embedded_view(game:&rvn_core::Engine<BevyRenderer>) -> rvn_core::video::VideoView {
    game.video_views().unwrap().into_iter().find(|view|view.id=="embedded").unwrap()
}

#[test]
#[cfg(feature = "video")]
fn resident_game_projection_keeps_nonce_and_clock_through_parked_parent_and_real_return() {
    use rvn_core::ui_residency::ProjectionResidence;
    let mut game=resident_game_fixture();
    let original=embedded_view(&game);
    assert_eq!(original.resource_token.as_ref().unwrap().task.key.owner,rvn_core::scene_tasks::TaskOwner::Game);
    let mut projections=VideoProjections::default();
    projections.reconcile(std::slice::from_ref(&original)).unwrap();
    let nonce=projections.nonce(&original).unwrap();
    let resource=projections.resource_nonce(&original).unwrap();
    game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
    let hidden=embedded_view(&game);
    assert_eq!(hidden.ui_projection.as_ref().unwrap().residence,ProjectionResidence::Parked);
    assert_eq!(hidden.resource_token,original.resource_token);
    assert!(!projection_visible(&hidden));
    projections.reconcile(std::slice::from_ref(&hidden)).unwrap();
    assert_eq!(projections.nonce(&hidden).as_deref(),Some(nonce.as_str()));
    assert_eq!(projections.resource_nonce(&hidden).as_deref(),Some(resource.as_str()));
    game.tick_game_frame(0.5,rvn_core::GameFrameReports::default()).unwrap();
    assert_eq!(game.state.motions.tracks["ui:shared/player"].elapsed,0.5);
    let motion=game.renderer.scene_batch.as_ref().unwrap().motions.iter().find(|view|matches!(&view.target,rvn_core::motion::MotionTarget::Interface{screen,..} if screen=="shared")).unwrap();
    assert_eq!(motion.ui_projection.as_ref().unwrap().residence,ProjectionResidence::Parked);
    game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
    let returned=embedded_view(&game);
    assert_eq!(returned.ui_projection,original.ui_projection);
    assert_eq!(returned.resource_token,original.resource_token);
    assert_eq!(game.state.motions.tracks["ui:shared/player"].elapsed,0.5);
    projections.reconcile(std::slice::from_ref(&returned)).unwrap();
    assert_eq!(projections.nonce(&returned).as_deref(),Some(nonce.as_str()));
    let saved=rvn_core::save::SaveData::try_from_state(&game.state,1,"resident".into(),"main.rvn".into()).unwrap();
    game.load_data(saved).unwrap();
    let loaded=embedded_view(&game);
    projections.reconcile(std::slice::from_ref(&loaded)).unwrap();
    assert_ne!(projections.resource_nonce(&loaded).as_deref(),Some(resource.as_str()));
    assert!(!projections.accepts(&loaded,&original.epoch.to_string(),&nonce));
}

#[test]
#[cfg(feature = "video")]
fn embedded_visual_requires_exact_visible_incarnation_order_and_host_role_with_legacy_fallback() {
    use crate::composed_motion::{InterfaceTarget,InterfaceProjectionTarget};
    let game=resident_game_fixture();
    let view=embedded_view(&game);
    let binding=view.ui_projection.as_ref().unwrap().clone();
    let marker=InterfaceProjectionTarget{interface:binding.interface.clone(),order:binding.order,host_role:binding.host_role};
    let mut world=World::new();
    let mut wrong=marker.clone();wrong.interface.incarnation+=1;
    let stale=world.spawn((InterfaceTarget{screen:"shared".into(),element:"player".into()},wrong)).id();
    assert!(create(&mut world,&view,Handle::default()).is_none());
    let mut wrong=marker.clone();wrong.order+=1;
    world.entity_mut(stale).insert(wrong);
    assert!(create(&mut world,&view,Handle::default()).is_none());
    let mut wrong=marker.clone();wrong.host_role=Some(rvn_ui::PageRole::Pause);
    world.entity_mut(stale).insert(wrong);
    assert!(create(&mut world,&view,Handle::default()).is_none());
    world.entity_mut(stale).remove::<InterfaceProjectionTarget>();
    assert!(create(&mut world,&view,Handle::default()).is_none());
    let exact=world.spawn((InterfaceTarget{screen:"shared".into(),element:"player".into()},marker)).id();
    let (entity,caption)=create(&mut world,&view,Handle::default()).unwrap();
    assert_eq!(world.get::<Parent>(entity).unwrap().get(),exact);
    assert_eq!(world.get::<Parent>(caption).unwrap().get(),entity);
    let mut entity=Some(entity);let mut caption=Some(caption);
    retire_visual(&mut world,&mut entity,&mut caption);
    assert!(entity.is_none() && caption.is_none());
    assert!(world.get_entity(exact).is_some());
    let mut hidden=view.clone();hidden.ui_projection.as_mut().unwrap().residence=rvn_core::ui_residency::ProjectionResidence::Parked;
    assert!(create(&mut world,&hidden,Handle::default()).is_none());
    world.entity_mut(exact).despawn();
    let mut legacy=view;legacy.ui_projection=None;legacy.resource_token=None;
    let (entity,_)=create(&mut world,&legacy,Handle::default()).unwrap();
    assert_eq!(world.get::<Parent>(entity).unwrap().get(),stale);
}

#[test]
#[cfg(feature = "video")]
fn browser_projection_nonce_keeps_game_element_on_call_and_renewal_but_revokes_old_reports() {
    let source=r#"
init { video.play("clip",video_clip("game.webm",{})) }
scene label P
"parent"
call C
"returned"
jump finish
scene label C
video.pause("clip")
"paused"
video.seek("clip",0.5)
"seeked"
return
label finish
"#;
    let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),BevyRenderer::new(),16).unwrap();
    game.step_until_interaction().unwrap();
    let view=|game:&rvn_core::Engine<BevyRenderer>|game.video_views().unwrap().remove(0);
    let original=view(&game);
    let mut projections=VideoProjections::default();
    projections.reconcile(std::slice::from_ref(&original)).unwrap();
    let nonce=projections.nonce(&original).unwrap();
    let resource=projections.resource_nonce(&original).unwrap();
    game.execute_timer_action("call C").unwrap();
    let called=view(&game);
    projections.reconcile(std::slice::from_ref(&called)).unwrap();
    assert_eq!(projections.nonce(&called).as_deref(),Some(nonce.as_str()));
    assert_eq!(projections.resource_nonce(&called).as_deref(),Some(resource.as_str()));
    game.step_until_interaction().unwrap();
    let paused=view(&game);
    projections.reconcile(std::slice::from_ref(&paused)).unwrap();
    assert_ne!(projections.nonce(&paused).as_deref(),Some(nonce.as_str()));
    assert_eq!(projections.resource_nonce(&paused).as_deref(),Some(resource.as_str()));
    assert!(!projections.accepts(&paused,&original.epoch.to_string(),&nonce));
    assert!(!game.video_feedback_from_view(&original,rvn_core::video::Feedback::Position{seconds:f64::NAN,duration:f64::NAN}).unwrap());
    game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
    let seeked=view(&game);let paused_nonce=projections.nonce(&paused).unwrap();
    projections.reconcile(std::slice::from_ref(&seeked)).unwrap();
    assert_ne!(projections.nonce(&seeked),Some(paused_nonce));
    assert_eq!(projections.resource_nonce(&seeked).as_deref(),Some(resource.as_str()));
    let saved=rvn_core::save::SaveData::try_from_state(&game.state,1,"seeked".into(),"main.rvn".into()).unwrap();
    game.load_data(saved).unwrap();
    let loaded=view(&game);let previous_nonce=projections.nonce(&seeked).unwrap();
    projections.reconcile(std::slice::from_ref(&loaded)).unwrap();
    assert_ne!(projections.resource_nonce(&loaded).as_deref(),Some(resource.as_str()));
    assert!(!projections.accepts(&loaded,&seeked.epoch.to_string(),&previous_nonce));
    let accepted=projections.nonce(&loaded).unwrap();
    let identity=projections.resource_nonce(&loaded).unwrap();
    projections.next=u64::MAX;
    let mut replacement=loaded.clone();replacement.epoch+=1;
    assert!(projections.reconcile(&[replacement]).is_err());
    assert_eq!(projections.nonce(&loaded),Some(accepted));
    assert_eq!(projections.resource_nonce(&loaded),Some(identity));
}

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
