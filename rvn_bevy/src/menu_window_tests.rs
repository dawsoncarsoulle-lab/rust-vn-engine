use super::*;

// Run the actual render systems, not just a stand-alone Query helper. No GPU or
// OS window is needed to reproduce Bevy's final Update after window despawn.
fn fixture() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
    app.init_asset::<Image>().init_asset::<Font>();
    let mut doc = Document::defaults();
    doc.add_dialogue_page();
    doc.add_choices_page();
    doc.add_quick_actions_page();
    app.insert_resource(Menus {
        doc: Some(doc),
        ..default()
    });
    app.insert_resource(VnEngine(
        rvn_core::Engine::new(vec![], crate::bevy_renderer::BevyRenderer::new(), 32).unwrap(),
    ));
    app.insert_resource(State::new(VnState::Waiting));
    app.init_resource::<SaveMenuState>();
    app.init_resource::<SettingsMenuState>();
    app.init_resource::<GalleryState>();
    app.init_resource::<CgAssetRegistry>();
    // The manager only ensures this existing directory exists; tests never save.
    app.insert_resource(PersistentDataResource {
        manager: rvn_core::PersistentDataManager::new(std::env::temp_dir()).unwrap(),
        data: default(),
    });
    app.init_resource::<DialogueHistory>();
    app.insert_resource(ProjectPaths::new(
        default(),
        default(),
        default(),
        default(),
        default(),
    ));
    app.init_resource::<Theme>();
    app.insert_resource(MenuFont(default()));
    app.init_resource::<crate::save_thumbnails::SaveThumbnails>();
    app.init_resource::<crate::systems::settings_menu::Settings>();
    app.init_resource::<MenuState>();
    app.init_resource::<TypewriterState>();
    app.insert_resource(VnRenderState {
        choice_options: vec!["A".into(), "B".into()],
        ..default()
    });
    app.world_mut()
        .spawn((DialogueBox, Style::default(), Visibility::Inherited));
    app.world_mut()
        .spawn((ChoiceContainer, Visibility::Inherited));
    app.add_systems(
        Update,
        (render_quick_actions, render_choices, render_narrative).chain(),
    );
    app
}

fn counts(app: &mut App) -> (usize, usize, usize) {
    let world = app.world_mut();
    (
        world
            .query_filtered::<Entity, With<QuickActionsRoot>>()
            .iter(world)
            .count(),
        world
            .query_filtered::<Entity, With<ChoicesRoot>>()
            .iter(world)
            .count(),
        world
            .query_filtered::<Entity, With<NarrativeRoot>>()
            .iter(world)
            .count(),
    )
}

#[test]
fn narrative_renderers_survive_window_destruction() {
    let mut app = fixture();
    let window = app.world_mut().spawn(Window::default()).id();
    app.update();
    assert_eq!(
        counts(&mut app),
        (1, 1, 1),
        "normal dialogue and choice rendering"
    );
    app.world_mut().despawn(window);
    app.update();
    app.update();
    assert_eq!(
        counts(&mut app),
        (1, 1, 1),
        "no extra UI spawned during shutdown"
    );
}

#[test]
fn narrative_renderers_tolerate_missing_or_ambiguous_window() {
    let mut app = fixture();
    app.update();
    assert_eq!(counts(&mut app), (0, 0, 0));
    let first = app.world_mut().spawn(Window::default()).id();
    let second = app.world_mut().spawn(Window::default()).id();
    app.update();
    assert_eq!(counts(&mut app), (0, 0, 0));
    app.world_mut().despawn(second);
    app.update();
    assert_eq!(counts(&mut app), (1, 1, 1));
    app.world_mut().despawn(first);
    app.update();
}

#[test]
fn default_choice_presenter_leaves_authored_menu_geometry_and_document_untouched() {
    let mut app = fixture();
    app.init_resource::<ChoiceFocus>();
    let document=app.world().resource::<Menus>().document().unwrap().clone();
    let authored_style=Style {
        position_type:PositionType::Absolute,
        left:Val::Px(123.0),top:Val::Px(456.0),width:Val::Px(321.0),height:Val::Px(65.0),
        ..default()
    };
    let authored=app.world_mut().spawn((ChoiceButton(1),Button,authored_style.clone())).id();
    app.add_systems(Update,crate::systems::choice::update_choice_buttons);
    app.update();
    assert_eq!(app.world().get::<Style>(authored),Some(&authored_style));
    assert_eq!(app.world().resource::<Menus>().document(),Some(&document));
    assert_eq!(app.world().get::<ChoiceButton>(authored).unwrap().0,1);
}
