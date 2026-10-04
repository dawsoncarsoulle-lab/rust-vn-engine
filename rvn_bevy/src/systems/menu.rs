use bevy::prelude::*;

use crate::resources::{MenuState, VnState};
use crate::systems::save_menu::{SaveMenuMode, SaveMenuOrigin, SaveMenuState};
use crate::systems::settings_menu::SettingsMenuState;

// ─── Composants locaux ───────────────────────────────────────────────────────

#[derive(Component)]
pub struct MenuOverlay;

#[derive(Component, Clone, PartialEq)]
pub enum MenuButton {
    Resume,
    Save,
    Load,
    Settings,
    Quit,
}

// ─── Spawn / Despawn ─────────────────────────────────────────────────────────

pub fn spawn_menu_overlay(mut commands: Commands) {
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(16.0),
                    ..default()
                },
                background_color: Color::srgba(0.0, 0.0, 0.0, 0.80).into(),
                z_index: ZIndex::Global(1000),
                ..default()
            },
            MenuOverlay,
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "PAUSE",
                TextStyle {
                    font_size: 48.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            spawn_menu_button(parent, "Reprendre", MenuButton::Resume);
            spawn_menu_button(parent, "Sauvegarder", MenuButton::Save);
            spawn_menu_button(parent, "Charger", MenuButton::Load);
            spawn_menu_button(parent, "Paramètres", MenuButton::Settings);
            spawn_menu_button(parent, "Quitter", MenuButton::Quit);
        });
}

fn spawn_menu_button(parent: &mut ChildBuilder, label: &str, action: MenuButton) {
    parent
        .spawn((
            ButtonBundle {
                style: Style {
                    width: Val::Px(320.0),
                    height: Val::Px(56.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: Color::srgba(0.08, 0.08, 0.20, 0.95).into(),
                ..default()
            },
            action,
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                label,
                TextStyle {
                    font_size: 26.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        });
}

pub fn despawn_menu_overlay(
    mut commands: Commands,
    overlay_query: Query<Entity, With<MenuOverlay>>,
) {
    for entity in overlay_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

// ─── Interaction ─────────────────────────────────────────────────────────────

pub fn menu_interaction_system(
    mut interaction_query: Query<
        (Ref<Interaction>, &MenuButton, &mut BackgroundColor),
        With<Button>,
    >,
    mut next_state: ResMut<NextState<VnState>>,
    mut menu_state: ResMut<MenuState>,
    mut exit: EventWriter<AppExit>,
    mut save_menu_state: ResMut<SaveMenuState>,
    mut settings_menu_state: ResMut<SettingsMenuState>,
) {
    for (interaction, button, mut bg_color) in interaction_query.iter_mut() {
        // Covered presenters already project Interaction::None. Paint that
        // state even while their actions are blocked, so a change consumed
        // under Save/Settings cannot leave a stale highlight on return.
        let background = match *interaction {
            Interaction::Hovered => Color::srgba(0.15, 0.15, 0.35, 0.95),
            Interaction::Pressed => Color::srgba(0.25, 0.25, 0.55, 0.95),
            Interaction::None => Color::srgba(0.08, 0.08, 0.20, 0.95),
        };
        if bg_color.0 != background {
            bg_color.0 = background;
        }
        // Activation still belongs only to a fresh press. An unchanged
        // Pressed component must never reopen a closed submenu or repeat Quit.
        if save_menu_state.active || settings_menu_state.active
            || !interaction.is_changed() || *interaction != Interaction::Pressed
        {
            continue;
        }
        match button {
            MenuButton::Resume => {
                if menu_state.return_to == Some(VnState::TitleScreen) {
                    menu_state.return_to = None;
                    next_state.set(VnState::TitleScreen);
                    return;
                }
                let ret = menu_state.return_to.take().unwrap_or(VnState::Waiting);
                next_state.set(ret);
            }

            MenuButton::Save => {
                if menu_state.return_to == Some(VnState::TitleScreen) {
                    warn!("[menu] save ignored outside an active game session");
                    return;
                }
                // Open save menu overlay in Save mode
                save_menu_state.open(SaveMenuMode::Save, SaveMenuOrigin::InGame);
            }

            MenuButton::Load => {
                if menu_state.return_to == Some(VnState::TitleScreen) {
                    warn!("[menu] load ignored outside an active game session");
                    return;
                }
                // Open save menu overlay in Load mode
                save_menu_state.open(SaveMenuMode::Load, SaveMenuOrigin::InGame);
            }

            MenuButton::Settings => {
                // Open settings overlay
                settings_menu_state.active = true;
            }

            MenuButton::Quit => {
                exit.send(AppExit::Success);
            }
        }
    }
}

#[cfg(test)]
mod feedback_tests {
    use super::*;
    #[derive(Resource, Default)]
    struct PaintChanges(usize);
    fn count_changes(
        colors: Query<(), (With<MenuButton>, Changed<BackgroundColor>)>,
        mut count: ResMut<PaintChanges>,
    ) {
        count.0 = colors.iter().count();
    }
    fn fixture() -> (App, [Entity; 5]) {
        let mut app=App::new();
        app.init_resource::<NextState<VnState>>()
            .init_resource::<MenuState>()
            .init_resource::<SaveMenuState>()
            .init_resource::<SettingsMenuState>()
            .init_resource::<PaintChanges>()
            .add_systems(Update,(menu_interaction_system,count_changes).chain());
        let buttons=[MenuButton::Resume,MenuButton::Save,MenuButton::Load,MenuButton::Settings,MenuButton::Quit]
            .map(|button|app.world_mut().spawn((Button,button,Interaction::None,
                BackgroundColor(Color::srgba(0.08,0.08,0.20,0.95)))).id());
        app.update();app.update();
        assert_eq!(app.world().resource::<PaintChanges>().0,0);
        (app,buttons)
    }
    #[test]
    fn each_builtin_color_tracks_current_interaction_without_redundant_changed_ticks() {
        let (mut app,buttons)=fixture();
        for (interaction,color) in [
            (Interaction::Hovered,Color::srgba(0.15,0.15,0.35,0.95)),
            (Interaction::None,Color::srgba(0.08,0.08,0.20,0.95)),
        ] {
            for entity in buttons {*app.world_mut().get_mut::<Interaction>(entity).unwrap()=interaction;}
            app.update();
            for entity in buttons {assert_eq!(app.world().get::<BackgroundColor>(entity).unwrap().0,color);}
            assert_eq!(app.world().resource::<PaintChanges>().0,5);
            app.update();assert_eq!(app.world().resource::<PaintChanges>().0,0,
                "matching colors must not be marked Changed merely by repaint projection");
        }
    }
    #[test]
    fn unchanged_pressed_does_not_reopen_save_and_a_covered_press_cannot_change_its_mode() {
        let (mut app,buttons)=fixture();let save=buttons[1];let load=buttons[2];
        *app.world_mut().get_mut::<Interaction>(save).unwrap()=Interaction::Pressed;
        app.update();assert!(app.world().resource::<SaveMenuState>().active);
        assert_eq!(app.world().resource::<SaveMenuState>().mode,SaveMenuMode::Save);
        assert_eq!(app.world().get::<BackgroundColor>(save).unwrap().0,Color::srgba(0.25,0.25,0.55,0.95));
        *app.world_mut().get_mut::<Interaction>(load).unwrap()=Interaction::Pressed;
        app.update();assert_eq!(app.world().resource::<SaveMenuState>().mode,SaveMenuMode::Save,
            "fresh Load under Save must remain blocked");
        app.world_mut().resource_mut::<SaveMenuState>().active=false;
        app.update();assert!(!app.world().resource::<SaveMenuState>().active,
            "retained Pressed components must not count as new activation");
        assert_eq!(app.world().resource::<PaintChanges>().0,0);
        *app.world_mut().get_mut::<Interaction>(save).unwrap()=Interaction::None;
        *app.world_mut().get_mut::<Interaction>(load).unwrap()=Interaction::None;
        app.update();
        *app.world_mut().get_mut::<Interaction>(save).unwrap()=Interaction::Pressed;
        app.update();assert!(app.world().resource::<SaveMenuState>().active,
            "a genuine new press must still activate the original action");
    }
    #[test]
    fn settings_cover_blocks_actions_without_altering_an_authored_button_or_its_outline() {
        let (mut app,buttons)=fixture();
        let authored=app.world_mut().spawn((Button,Interaction::Hovered,
            BackgroundColor(Color::srgba(0.77,0.11,0.24,0.63)),
            Outline{width:Val::Px(3.0),offset:Val::Px(4.0),color:Color::srgba(0.9,0.8,0.1,0.6)})).id();
        app.world_mut().resource_mut::<SettingsMenuState>().active=true;
        for entity in buttons {*app.world_mut().get_mut::<Interaction>(entity).unwrap()=Interaction::Pressed;}
        app.update();
        assert!(!app.world().resource::<SaveMenuState>().active);
        assert!(app.world().resource::<SettingsMenuState>().active);
        assert!(app.world().resource::<Events<AppExit>>().is_empty());
        assert!(matches!(app.world().resource::<NextState<VnState>>(),NextState::Unchanged));
        assert_eq!(app.world().get::<BackgroundColor>(authored).unwrap().0,Color::srgba(0.77,0.11,0.24,0.63));
        let outline=app.world().get::<Outline>(authored).unwrap();
        assert_eq!(outline.width,Val::Px(3.0));assert_eq!(outline.offset,Val::Px(4.0));
        assert_eq!(outline.color,Color::srgba(0.9,0.8,0.1,0.6));
        for entity in buttons {*app.world_mut().get_mut::<Interaction>(entity).unwrap()=Interaction::None;}
        app.update();app.world_mut().resource_mut::<SettingsMenuState>().active=false;
        app.update();assert_eq!(app.world().resource::<PaintChanges>().0,0);
        assert!(app.world().resource::<Events<AppExit>>().is_empty());
    }
}
