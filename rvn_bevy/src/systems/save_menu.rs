//! Systems and components to display a save/load menu overlay.
//!
//! The save menu appears on top of the main pause menu when the player
//! chooses to save or load.  It lists several save slots along with
//! metadata such as timestamp and label.  Selecting a slot will save
//! or load depending on the current mode.  The menu can be cancelled
//! to return to the pause menu without taking any action.

use std::time::{Duration, UNIX_EPOCH};

use bevy::prelude::*;
use chrono::{DateTime, Local};
use rvn_core::save::{SaveData, SaveManager};

use crate::project_paths::ProjectPaths;
use crate::resources::{
    DialogueHistory, ImagemapState, MenuState, PersistentDataResource, TypewriterState, VnEngine,
    VnRenderState, VnState,
};
use crate::vn_command::VnCommand;
use rvn_core::persistent::LastResumeTarget;
use rvn_parser::Transition;

/// Mode of the save menu: Save or Load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveMenuMode {
    Save,
    Load,
    Delete,
    ToggleProtection,
}

/// Origin of the save/load overlay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SaveMenuOrigin {
    #[default]
    InGame,
    TitleScreen,
}

/// Resource tracking whether the save menu is active and which mode it is in.
#[derive(Resource, Debug)]
pub struct SaveMenuState {
    pub revision:u64,
    pub active: bool,
    pub mode: SaveMenuMode,
    pub origin: SaveMenuOrigin,
}

impl Default for SaveMenuState {
    fn default() -> Self {
        Self {
            revision:0,
            active: false,
            mode: SaveMenuMode::Save,
            origin: SaveMenuOrigin::InGame,
        }
    }
}

impl SaveMenuState {
    pub fn open(&mut self, mode: SaveMenuMode, origin: SaveMenuOrigin) {
        self.mode = mode;
        self.origin = origin;
        self.active = true;
    }

    fn close(&mut self) {
        self.active = false;
        self.origin = SaveMenuOrigin::InGame;
    }
}

/// Component marking the root entity of the save menu overlay.
#[derive(Component)]
pub struct SaveMenuOverlay;

/// Component on each save slot button.  Holds the slot index (starting at 1).
#[derive(Component)]
pub struct SaveSlotButton(pub usize);
#[derive(Component)]pub(crate) struct SaveSlotMode(pub SaveMenuMode);

/// Approval is transient and bound to one exact slot and operation.
#[derive(Resource,Default)]
pub(crate) struct SaveConfirmation {
    pub pending:Option<(usize,SaveMenuMode)>,
    pub approved:Option<(usize,SaveMenuMode)>,
    /// Slot zero is reserved for quickload, never a manual save slot.
    pub quick_return:Option<VnState>,
    pub action_pending:Option<(rvn_ui::Action,String,VnState)>,
}
impl SaveConfirmation{pub fn active(&self)->bool{self.pending.is_some()||self.action_pending.is_some()}}

/// Component on the cancel button.
#[derive(Component)]
pub struct SaveMenuCancelButton;

/// Maximum number of save slots to display.
pub const MAX_SLOTS: usize = 5;
pub const SUPPORTED_SLOTS:u32=1000;
#[derive(Component)]pub struct SaveSlotColors{pub normal:Color,pub hover:Color,pub pressed:Color}

pub fn apply_loaded_game(
    engine: &mut VnEngine,
    render_state: &mut VnRenderState,
    imagemap_state: &mut ImagemapState,
    tw_state: &mut TypewriterState,
    history: &mut DialogueHistory,
    vn_events: &mut EventWriter<VnCommand>,
) {
    render_state.choice_options.clear();
    imagemap_state.clear();
    tw_state.skip();
    history.clear();

    let mut pending = engine.0.renderer.take_pending();
    for cmd in pending.iter_mut() {
        match cmd {
            VnCommand::SetBackground { transition, .. } => *transition = Transition::None,
            VnCommand::ShowSprite { transition, .. } => *transition = Transition::None,
            VnCommand::HideSprite { transition, .. } => *transition = Transition::None,
            VnCommand::MoveSprite { transition, .. } => *transition = Transition::None,
            _ => {}
        }
    }
    for cmd in pending {
        vn_events.send(cmd);
    }
}

pub fn record_resume_target(persistent: &mut PersistentDataResource, target: LastResumeTarget) {
    persistent.data.last_resume_target = Some(target);
    if let Err(e) = persistent.manager.save(&persistent.data) {
        error!("[persistent] impossible d'enregistrer le point de reprise: {e}");
    }
}

/// Spawn the save menu overlay when becoming active.
pub fn spawn_save_menu_overlay(
    mut commands: Commands,
    save_state: Res<SaveMenuState>,
    project_paths: Res<ProjectPaths>,
    _engine: Res<VnEngine>,
    query: Query<Entity, With<SaveMenuOverlay>>,
) {
    // Only spawn when becoming active and overlay is not already present
    if !save_state.active {
        return;
    }
    if !query.is_empty() {
        return;
    }

    // Build a SaveManager to list existing saves
    let save_mgr = match SaveManager::new(&project_paths.saves, crate::systems::save_menu::SUPPORTED_SLOTS) {
        Ok(mgr) => mgr,
        Err(e) => {
            error!("[save_menu] failed to create SaveManager: {e}");
            return;
        }
    };
    // Gather metadata for each slot
    let mut slots: Vec<Option<SaveData>> = Vec::new();
    for slot_index in 1..=MAX_SLOTS {
        match save_mgr.load(slot_index as u32) {
            Ok(data) => slots.push(Some(data)),
            Err(_) => slots.push(None),
        }
    }

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
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                background_color: Color::srgba(0.0, 0.0, 0.0, 0.90).into(),
                z_index: ZIndex::Global(2000),
                ..default()
            },
            SaveMenuOverlay,
        ))
        .with_children(|parent| {
            // Title
            let title_text = match save_state.mode {
                SaveMenuMode::Save => "Sauvegarder",
                SaveMenuMode::Load => "Charger",
                SaveMenuMode::Delete => "Supprimer",
                SaveMenuMode::ToggleProtection => "Protéger",
            };
            parent.spawn(TextBundle::from_section(
                title_text,
                TextStyle {
                    font_size: 40.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            // Slots
            for (i, meta_opt) in slots.iter().enumerate() {
                let slot_index = i + 1;
                let label_text: String;
                let timestamp_text: String;
                if let Some(meta) = meta_opt {
                    label_text = meta.label.clone();
                    let ts = UNIX_EPOCH + Duration::from_secs(meta.timestamp);
                    let datetime: DateTime<Local> = ts.into();
                    timestamp_text = datetime.format("%d/%m/%Y %H:%M").to_string();
                } else {
                    label_text = format!("Slot {} (vide)", slot_index);
                    timestamp_text = "".to_string();
                }

                parent
                    .spawn((
                        ButtonBundle {
                            style: Style {
                                width: Val::Px(400.0),
                                height: Val::Px(60.0),
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(8.0)),
                                ..default()
                            },
                            background_color: Color::srgba(0.10, 0.10, 0.25, 0.95).into(),
                            ..default()
                        },
                        SaveSlotButton(slot_index),
                    ))
                    .with_children(|btn| {
                        // Label text
                        btn.spawn(TextBundle::from_section(
                            label_text.clone(),
                            TextStyle {
                                font_size: 24.0,
                                color: Color::WHITE,
                                ..default()
                            },
                        ));
                        // Timestamp text
                        if !timestamp_text.is_empty() {
                            btn.spawn(TextBundle::from_section(
                                timestamp_text.clone(),
                                TextStyle {
                                    font_size: 16.0,
                                    color: Color::srgba(0.5, 0.5, 0.5, 1.0),
                                    ..default()
                                },
                            ));
                        }
                    });
            }

            // Cancel button
            parent
                .spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Px(200.0),
                            height: Val::Px(40.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            margin: UiRect::all(Val::Px(12.0)),
                            ..default()
                        },
                        background_color: Color::srgba(0.15, 0.15, 0.30, 0.95).into(),
                        ..default()
                    },
                    SaveMenuCancelButton,
                ))
                .with_children(|btn| {
                    btn.spawn(TextBundle::from_section(
                        "Annuler",
                        TextStyle {
                            font_size: 22.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    ));
                });
        });
}

/// Handle interactions with the save menu overlay.
pub fn save_menu_interaction_system(
    mut interaction_query: Query<
        (
            &Interaction,
            Option<&SaveSlotButton>,
            Option<&SaveMenuCancelButton>,
            &mut BackgroundColor,
            Option<&SaveSlotColors>,
            Option<&SaveSlotMode>,
        ),
        Changed<Interaction>,
    >,
    mut save_state: ResMut<SaveMenuState>,
    mut next_state: ResMut<NextState<VnState>>,
    mut menu_state: ResMut<MenuState>,
    project_paths: Res<ProjectPaths>,
    mut engine: ResMut<VnEngine>,
    mut render_state: ResMut<VnRenderState>,
    mut imagemap_state: ResMut<ImagemapState>,
    mut tw_state: ResMut<TypewriterState>,
    mut history: ResMut<DialogueHistory>,
    mut persistent: ResMut<PersistentDataResource>,
    mut vn_events: EventWriter<VnCommand>,
    mut confirmation: ResMut<SaveConfirmation>,
    thumbnails:Res<crate::save_thumbnails::SaveThumbnails>,
) {
    if !save_state.active || confirmation.action_pending.is_some() {
        return;
    }

    for (interaction, slot_comp, cancel_comp, mut bg_color, colors, mode_override) in interaction_query.iter_mut() {
        if slot_comp.is_none() && cancel_comp.is_none() {continue;}
        match interaction {
            Interaction::Hovered => {
                *bg_color = colors.map(|v|v.hover).unwrap_or(Color::srgba(0.20, 0.20, 0.40, 0.95)).into();
            }
            Interaction::None => {
                *bg_color = colors.map(|v|v.normal).unwrap_or(Color::srgba(0.10, 0.10, 0.25, 0.95)).into();
            }
            Interaction::Pressed => {
                *bg_color = colors.map(|v|v.pressed).unwrap_or(Color::srgba(0.30, 0.30, 0.50, 0.95)).into();
                if cancel_comp.is_some() {
                    if confirmation.pending.take().is_some(){return;}
                    let origin = save_state.origin;
                    save_state.close();
                    if origin == SaveMenuOrigin::TitleScreen {
                        menu_state.return_to = None;
                        next_state.set(VnState::TitleScreen);
                    }
                    return;
                }
                if let Some(slot) = slot_comp {
                    if confirmation.pending.is_some(){return;}
                    let slot_index = slot.0;
                    let mode=mode_override.map(|m|m.0).unwrap_or(save_state.mode);
                    let operation=(slot_index,mode);
                    let approved=confirmation.approved==Some(operation);
                    if approved{confirmation.approved=None;}
                    info!("[save_menu] activation {operation:?}, approved={approved}");
                    let occupied=SaveManager::new(&project_paths.saves,SUPPORTED_SLOTS).ok().is_some_and(|m|m.load(slot_index as u32).is_ok());
                    if !approved&&occupied&&mode!=SaveMenuMode::ToggleProtection&&(matches!(mode,SaveMenuMode::Save|SaveMenuMode::Delete)||save_state.origin==SaveMenuOrigin::InGame){confirmation.pending=Some(operation);return;}
                    // Create SaveManager
                    match SaveManager::new(&project_paths.saves, crate::systems::save_menu::SUPPORTED_SLOTS) {
                        Ok(mgr) => match mode {
                            SaveMenuMode::ToggleProtection => {
                                if occupied {
                                    match mgr.is_protected(slot_index as u32).and_then(|locked|mgr.set_protected(slot_index as u32,!locked)) {
                                        Ok(()) => info!("[save_menu] protection changed for slot {slot_index}"),
                                        Err(e) => error!("[save_menu] protection: {e}"),
                                    }
                                }
                                save_state.revision=save_state.revision.wrapping_add(1);
                            }
                            SaveMenuMode::Delete => {
                                match mgr.delete(slot_index as u32) {
                                    Ok(()) => {
                                        if persistent.data.last_resume_target.as_ref().is_some_and(|target|target.kind==rvn_core::persistent::ResumeSaveKind::Manual&&target.slot==Some(slot_index as u32)) {
                                            persistent.data.last_resume_target=None;
                                            if let Err(e)=persistent.manager.save(&persistent.data){error!("[save_menu] resume target: {e}");}
                                        }
                                        info!("[save_menu] deleted slot {slot_index}");
                                    }
                                    Err(e)=>error!("[save_menu] delete: {e}"),
                                }
                                save_state.revision=save_state.revision.wrapping_add(1);
                            }
                            SaveMenuMode::Save => {
                                let label = format!("Sauvegarde {}", slot_index);
                                let script_name = "script.rvn".to_string();
                                match engine.0.save(&mgr, slot_index as u32, label, script_name) {
                                    Ok(_) => {
                                        if let Err(e)=thumbnails.persist(&engine,&mgr,&project_paths.saves,slot_index as u32){warn!("[save_menu] miniature : {e}");}
                                        if let Ok(data) = mgr.load(slot_index as u32) {
                                            record_resume_target(
                                                &mut persistent,
                                                LastResumeTarget::manual(
                                                    slot_index as u32,
                                                    data.timestamp,
                                                ),
                                            );
                                        }
                                        info!("[save_menu] saved slot {}", slot_index);
                                    }
                                    Err(e) => error!(
                                        "[save_menu] error saving slot {}: {}",
                                        slot_index, e
                                    ),
                                }
                                save_state.close();
                            }
                            SaveMenuMode::Load => match engine.0.load(&mgr, slot_index as u32) {
                                Ok(_) => {
                                    info!("[save_menu] loaded slot {}", slot_index);
                                    apply_loaded_game(
                                        &mut engine,
                                        &mut render_state,
                                        &mut imagemap_state,
                                        &mut tw_state,
                                        &mut history,
                                        &mut vn_events,
                                    );
                                    if let Ok(data) = mgr.load(slot_index as u32) {
                                        record_resume_target(
                                            &mut persistent,
                                            LastResumeTarget::manual(
                                                slot_index as u32,
                                                data.timestamp,
                                            ),
                                        );
                                    }
                                    menu_state.return_to = None;
                                    next_state.set(VnState::Waiting);
                                    save_state.close();
                                }
                                Err(e) => {
                                    error!("[save_menu] error loading slot {}: {}", slot_index, e)
                                }
                            },
                        },
                        Err(e) => error!("[save_menu] failed to create SaveManager: {}", e),
                    }
                }
            }
        }
    }
}

/// Despawn the save menu overlay when it is no longer active.
pub fn despawn_save_menu_overlay(
    mut commands: Commands,
    save_state: Res<SaveMenuState>,
    overlay_query: Query<Entity, With<SaveMenuOverlay>>,
) {
    if save_state.active {
        return;
    }
    for entity in overlay_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_save_requires_exact_one_shot_approval(){
        let root=std::env::temp_dir().join(format!("rvn-confirm-test-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        let manager=SaveManager::new(&root,SUPPORTED_SLOTS).unwrap();
        let engine=rvn_core::Engine::new(rvn_parser::parse("label start\n\"Original\"\n").unwrap(),crate::bevy_renderer::BevyRenderer::new(),64).unwrap();
        engine.save(&manager,1,"Original protégé".into(),"test.rvn".into()).unwrap();
        let mut app=App::new();
        app.insert_resource(VnEngine(engine)).insert_resource(ProjectPaths::new(root.clone(),root.clone(),root.clone(),root.clone(),root.clone()))
            .insert_resource(PersistentDataResource{manager:rvn_core::PersistentDataManager::new(&root).unwrap(),data:Default::default()})
            .init_resource::<crate::save_thumbnails::SaveThumbnails>().init_resource::<SaveMenuState>().init_resource::<SaveConfirmation>().init_resource::<MenuState>().init_resource::<VnRenderState>().init_resource::<ImagemapState>().init_resource::<TypewriterState>().init_resource::<DialogueHistory>()
            .insert_resource(NextState::<VnState>::default()).add_event::<VnCommand>().add_systems(Update,save_menu_interaction_system);
        app.world_mut().resource_mut::<SaveMenuState>().open(SaveMenuMode::Save,SaveMenuOrigin::InGame);
        let button=app.world_mut().spawn((SaveSlotButton(1),Interaction::Pressed,BackgroundColor::default())).id();
        app.update();
        assert_eq!(app.world().resource::<SaveConfirmation>().pending,Some((1,SaveMenuMode::Save)));
        assert_eq!(manager.load(1).unwrap().label,"Original protégé");
        // Cancelling does not close the originating page or replace the save.
        app.world_mut().spawn((SaveMenuCancelButton,Interaction::Pressed,BackgroundColor::default()));app.update();
        assert!(app.world().resource::<SaveMenuState>().active);
        assert!(app.world().resource::<SaveConfirmation>().pending.is_none());
        assert_eq!(manager.load(1).unwrap().label,"Original protégé");
        app.world_mut().resource_mut::<SaveConfirmation>().approved=Some((2,SaveMenuMode::Save));
        app.world_mut().entity_mut(button).insert(Interaction::Pressed);app.update();
        assert_eq!(manager.load(1).unwrap().label,"Original protégé");
        {let mut confirm=app.world_mut().resource_mut::<SaveConfirmation>();confirm.pending=None;confirm.approved=Some((1,SaveMenuMode::Save));}
        app.world_mut().entity_mut(button).insert(Interaction::Pressed);app.update();
        assert_eq!(manager.load(1).unwrap().label,"Sauvegarde 1");
        assert!(app.world().resource::<SaveConfirmation>().approved.is_none());
        // A per-card Load button does not inherit the surrounding Save page mode.
        app.world_mut().resource_mut::<SaveMenuState>().open(SaveMenuMode::Save,SaveMenuOrigin::InGame);
        app.world_mut().resource_mut::<VnEngine>().0.state.pc=1;
        app.world_mut().resource_mut::<SaveConfirmation>().approved=Some((1,SaveMenuMode::Load));
        app.world_mut().entity_mut(button).insert((Interaction::Pressed,SaveSlotMode(SaveMenuMode::Load)));app.update();
        assert_eq!(app.world().resource::<VnEngine>().0.state.pc,0);
        assert_eq!(manager.load(1).unwrap().label,"Sauvegarde 1");
        // Per-card protection never navigates or changes the narrative.
        app.world_mut().resource_mut::<SaveMenuState>().open(SaveMenuMode::Save,SaveMenuOrigin::InGame);
        app.world_mut().entity_mut(button).insert((Interaction::Pressed,SaveSlotMode(SaveMenuMode::ToggleProtection)));app.update();
        assert!(manager.is_protected(1).unwrap());
        assert!(app.world().resource::<SaveConfirmation>().pending.is_none());
        app.world_mut().resource_mut::<SaveConfirmation>().approved=Some((1,SaveMenuMode::Delete));
        app.world_mut().entity_mut(button).insert((Interaction::Pressed,SaveSlotMode(SaveMenuMode::Delete)));app.update();
        assert!(manager.slot_occupied(1));
        app.world_mut().entity_mut(button).insert((Interaction::Pressed,SaveSlotMode(SaveMenuMode::ToggleProtection)));app.update();
        assert!(!manager.is_protected(1).unwrap());
        app.world_mut().entity_mut(button).insert((Interaction::Pressed,SaveSlotMode(SaveMenuMode::Delete)));app.update();
        assert_eq!(app.world().resource::<SaveConfirmation>().pending,Some((1,SaveMenuMode::Delete)));
        assert!(manager.slot_occupied(1));
        {let mut confirm=app.world_mut().resource_mut::<SaveConfirmation>();confirm.pending=None;confirm.approved=Some((1,SaveMenuMode::Delete));}
        let pc=app.world().resource::<VnEngine>().0.state.pc;
        app.world_mut().entity_mut(button).insert(Interaction::Pressed);app.update();
        assert!(!manager.slot_occupied(1));
        assert_eq!(app.world().resource::<VnEngine>().0.state.pc,pc);
        assert!(app.world().resource::<PersistentDataResource>().data.last_resume_target.is_none());
        assert!(app.world().resource::<SaveMenuState>().active);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn open_records_mode_origin_and_active_flag() {
        let mut state = SaveMenuState::default();

        state.open(SaveMenuMode::Load, SaveMenuOrigin::TitleScreen);

        assert!(state.active);
        assert_eq!(state.mode, SaveMenuMode::Load);
        assert_eq!(state.origin, SaveMenuOrigin::TitleScreen);
    }

    #[test]
    fn close_resets_to_ingame_origin() {
        let mut state = SaveMenuState {
            revision:0,
            active: true,
            mode: SaveMenuMode::Load,
            origin: SaveMenuOrigin::TitleScreen,
        };

        state.close();

        assert!(!state.active);
        assert_eq!(state.origin, SaveMenuOrigin::InGame);
    }
}
