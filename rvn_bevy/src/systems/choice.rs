use bevy::color::Srgba;
use bevy::prelude::*;

use super::CHOICE_MARGIN_ABOVE_BOX;
use crate::components::{ChoiceButton, ChoiceContainer, DialogueBox};
use crate::resources::{ChoiceFocus, Theme, VnRenderState};
use crate::vn_command::{PlayerInput, VnCommand};

pub fn choice_system(
    mut vn_events: EventReader<VnCommand>,
    mut render_state: ResMut<VnRenderState>,
    mut dialogue_box_query: Query<&mut Style, With<DialogueBox>>,
) {
    for cmd in vn_events.read() {
        let VnCommand::ShowChoice { options } = cmd else {
            continue;
        };
        if let Some(mut style) = dialogue_box_query.iter_mut().next() {
            style.display = Display::Flex;
        }
        render_state.choice_options = options.clone();
    }
}

pub fn update_choice_buttons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    render_state: Res<VnRenderState>,
    theme: Res<Theme>,
    choice_focus: Res<ChoiceFocus>,
    existing: Query<Entity, With<ChoiceButton>>,
    container_query: Query<Entity, With<ChoiceContainer>>,
    custom: Res<crate::menu_documents::Menus>,
    mut customized: Local<bool>,
) {
    let role_changed = *customized != custom.custom_choices();
    *customized = custom.custom_choices();
    if *customized {
        return;
    }
    if !role_changed
        && !render_state.is_changed()
        && !theme.is_changed()
        && !choice_focus.is_changed()
    {
        return;
    }
    for entity in existing.iter() {
        commands.entity(entity).despawn_recursive();
    }
    if render_state.choice_options.is_empty() {
        return;
    }
    let Ok(container) = container_query.get_single() else {
        return;
    };

    let btn_w = 640.0_f32;
    let btn_h = 52.0_f32;
    let gap = 10.0_f32;
    let base_bottom = theme.textbox.height + CHOICE_MARGIN_ABOVE_BOX;

    // Couleurs normales
    let bg_normal = Srgba::hex(&theme.choice.background_color)
        .map(Color::from)
        .unwrap_or(Color::srgba(0.05, 0.05, 0.18, 0.95));
    // Couleur focus : légèrement plus clair + saturé
    let bg_focus = Color::srgba(0.20, 0.20, 0.50, 0.98);
    let num_color = Srgba::hex(&theme.choice.number_color)
        .map(Color::from)
        .unwrap_or(Color::WHITE);
    let txt_color = Srgba::hex(&theme.choice.text_color)
        .map(Color::from)
        .unwrap_or(Color::WHITE);

    let choice_font: Handle<Font> = theme
        .choice
        .font_path
        .as_ref()
        .map(|p| asset_server.load(p.clone()))
        .unwrap_or_default();

    for (i, label) in render_state.choice_options.iter().enumerate() {
        // Bottom anchoring grows upwards: keep script index 0 at the top,
        // without reversing the labels, destination indices or number keys.
        let row_from_bottom = render_state.choice_options.len() - 1 - i;
        let bottom = base_bottom + row_from_bottom as f32 * (btn_h + gap);
        let focused = choice_focus.0 == Some(i);
        let bg_color = if focused { bg_focus } else { bg_normal };

        let btn = commands
            .spawn((
                ButtonBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        width: Val::Px(btn_w),
                        height: Val::Px(btn_h),
                        left: Val::Percent(50.0),
                        bottom: Val::Px(bottom),
                        margin: UiRect::left(Val::Px(-btn_w / 2.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(22.0)),
                        column_gap: Val::Px(4.0),
                        border: if focused {
                            UiRect::all(Val::Px(2.0))
                        } else {
                            UiRect::all(Val::Px(0.0))
                        },
                        ..default()
                    },
                    background_color: bg_color.into(),
                    border_color: if focused {
                        Color::srgba(0.7, 0.7, 1.0, 1.0).into()
                    } else {
                        Color::NONE.into()
                    },
                    ..default()
                },
                ChoiceButton(i),
            ))
            .with_children(|p| {
                p.spawn(TextBundle::from_section(
                    format!("{}. ", i + 1),
                    TextStyle {
                        font: choice_font.clone(),
                        font_size: theme.choice.font_size,
                        color: num_color,
                    },
                ));
                p.spawn(TextBundle::from_section(
                    label.clone(),
                    TextStyle {
                        font: choice_font.clone(),
                        font_size: theme.choice.font_size,
                        color: txt_color,
                    },
                ));
            })
            .id();
        commands.entity(container).add_child(btn);
    }
}

pub fn choice_interaction_system(
    custom_menus: Res<crate::menu_documents::Menus>,
    mut interaction_query: Query<
        (&Interaction, &ChoiceButton),
        (Changed<Interaction>, With<Button>, With<ChoiceButton>),
    >,
    mut choice_focus: ResMut<ChoiceFocus>,
    mut player_events: EventWriter<PlayerInput>,
) {
    if !custom_menus.choices_interactive() {
        return;
    }
    for (interaction, choice_button) in interaction_query.iter_mut() {
        if *interaction == Interaction::Pressed {
            choice_focus.clear();
            player_events.send(PlayerInput::Choose(choice_button.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn layout_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Image>()
            .init_asset::<Font>()
            .init_asset::<bevy::render::render_resource::Shader>()
            .init_resource::<bevy::render::camera::ManualTextureViews>()
            .add_event::<bevy::window::WindowResized>()
            .add_event::<bevy::window::WindowCreated>()
            .add_event::<bevy::window::WindowScaleFactorChanged>()
            .add_plugins(bevy::ui::UiPlugin);
        // UiPlugin initializes its private layout storage. Run the actual
        // camera/layout/transform pipeline, without a GPU, native window or
        // unrelated focus/text rasterization systems.
        let mut schedules = app.world_mut().resource_mut::<bevy::ecs::schedule::Schedules>();
        schedules.remove(PreUpdate);
        schedules.remove(PostUpdate);
        drop(schedules);
        app.add_systems(
            PostUpdate,
            (
                bevy::render::camera::camera_system::<OrthographicProjection>,
                bevy::ui::ui_layout_system,
                bevy::transform::systems::sync_simple_transforms,
                bevy::transform::systems::propagate_transforms,
            ).chain(),
        )
            .init_resource::<Theme>()
            .init_resource::<ChoiceFocus>()
            .init_resource::<crate::menu_documents::Menus>()
            .insert_resource(VnRenderState {
                choice_options: vec!["First destination".into(),"Second destination".into(),"Third destination".into()],
                ..default()
            })
            .add_event::<PlayerInput>()
            .add_systems(Update,(update_choice_buttons,choice_interaction_system).chain());
        app.world_mut().spawn((Window {
            resolution: bevy::window::WindowResolution::new(1600.0,900.0),
            ..default()
        },bevy::window::PrimaryWindow));
        app.world_mut().spawn(Camera2dBundle::default());
        app.world_mut().spawn((ChoiceContainer,NodeBundle {
            style: Style {width:Val::Percent(100.0),height:Val::Percent(100.0),..default()},
            ..default()
        }));
        app
    }
    #[test]
    fn default_choices_follow_script_order_in_real_layout_and_click_destinations() {
        let mut app = layout_app();
        app.update();
        let mut rows = app.world_mut().query::<(Entity,&ChoiceButton,&Node,&GlobalTransform)>();
        let mut geometry:Vec<_>=rows.iter(app.world()).map(|(entity,index,node,transform)|
            (index.0,entity,node.size(),transform.translation())).collect();
        geometry.sort_by_key(|row|row.0);
        assert_eq!(geometry.len(),3);
        for pair in geometry.windows(2) {
            assert!(pair[0].3.y+pair[0].2.y*0.5 < pair[1].3.y-pair[1].2.y*0.5,
                "Earlier script choice must be fully above the next choice: {geometry:?}");
        }
        for (index,entity,_,_) in &geometry {
            let labels:Vec<_>=app.world().get::<Children>(*entity).unwrap().iter()
                .filter_map(|child|app.world().get::<Text>(*child))
                .map(|text|text.sections.iter().map(|section|section.value.as_str()).collect::<String>()).collect();
            assert_eq!(labels[0],format!("{}. ",index+1));
            assert_eq!(labels[1],app.world().resource::<VnRenderState>().choice_options[*index]);
        }
        // Use the real interaction dispatcher after each rebuild. Geometry
        // changes cannot swap an option's engine destination or number label.
        for index in 0..3 {
            // Settle the previous focus repaint before pressing the next row.
            app.update();
            let mut rows=app.world_mut().query::<(Entity,&ChoiceButton)>();
            let entity=rows.iter(app.world()).find(|(_,button)|button.0==index).unwrap().0;
            *app.world_mut().get_mut::<Interaction>(entity).unwrap()=Interaction::Pressed;
            app.update();
            let mut events=app.world_mut().resource_mut::<Events<PlayerInput>>();
            let choices:Vec<_>=events.drain().filter_map(|event|match event {
                PlayerInput::Choose(index)=>Some(index),_=>None,
            }).collect();
            assert_eq!(choices,vec![index]);
        }
    }
}
