//! Opt-in system presenters evaluated from RVN source, with host-only authority.
use crate::{menu_documents::{ActionContext, Menus}, project_paths::ProjectPaths, resources::*,
    systems::{save_menu::{SaveConfirmation, SaveMenuMode, SaveMenuOrigin, SaveMenuState, SUPPORTED_SLOTS},
        settings_menu::{Settings, SettingsMenuState}}, vn_command::{PlayerInput, VnCommand}};
use bevy::{ecs::system::SystemParam, prelude::*};
use rvn_core::{save::{SaveData, SaveManager}, Interaction as StoryInteraction};
use rvn_ui::{source_menus::{BoolPreference, GalleryTab, MenuAuthority, MenuEffect, MenuRequest,
    NumberPreference, SlotAuthority}, Action, PageRole};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Stamp {
    generation: u64,
    epoch: u64,
    pc: usize,
    interaction: Option<StoryInteraction>,
    destinations: Vec<usize>,
    confirmation: Option<u64>,
}
#[derive(Clone, Debug)]
pub struct Receipt {
    pub(crate) identity: u64,
    pub(crate) effect: MenuEffect,
    pub(crate) stamp: Stamp,
    pub(crate) previous: rvn_core::GameState,
}
#[derive(Resource, Default)]
pub(crate) struct SourceMenus {
    roles: BTreeMap<PageRole, String>,
    generation: u64,
    page: usize,
    slot_key: String,
    slots: BTreeMap<u32, SaveData>,
    locks: std::collections::BTreeSet<u32>,
    confirmation_key: String,
    confirmation_serial: u64,
    confirmation_token: Option<u64>,
    consumed_receipt: u64,
    pub(crate) diagnostic: String,
    invalid: BTreeMap<PageRole,(String,Option<String>,u64)>,
    conflict_key: String,
}
impl SourceMenus {
    pub(crate) fn active(&self, role: PageRole) -> bool { self.roles.contains_key(&role) }
    pub(crate) fn any(&self) -> bool { !self.roles.is_empty() }
    pub(crate) fn modal(&self) -> bool { self.roles.keys().any(|role| is_modal(*role)) }
    fn confirmation(&mut self, gate: &SaveConfirmation) {
        let key = if gate.active() { format!("{:?}/{:?}", gate.pending, gate.action_pending) } else { String::new() };
        if self.confirmation_key != key {
            self.confirmation_key = key;
            self.confirmation_serial = self.confirmation_serial.checked_add(1).expect("Confirmation token exhausted");
            self.confirmation_token = gate.active().then_some(self.confirmation_serial);
        }
    }
}
fn is_modal(role: PageRole) -> bool { !matches!(role, PageRole::Dialogue | PageRole::Choices | PageRole::QuickActions) }
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct SourceMenuSyncSet;
#[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) struct SourceMenuActionSet;
pub struct SourceMenusPlugin;
impl Plugin for SourceMenusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SourceMenus>().add_systems(Update, (
            synchronize.in_set(SourceMenuSyncSet)
                .after(crate::systems::stepping_system)
                .after(crate::systems::dialogue_system)
                .after(crate::systems::typewriter_system)
                .before(crate::programmable_ui::InterfaceReceiveSet)
                .before(crate::programmable_ui::InterfaceInputSet),
            dispatch.in_set(SourceMenuActionSet)
                .after(crate::programmable_ui::InterfaceInputSet)
                .after(crate::custom_canvas::CanvasInputSet)
                .before(crate::systems::player_input_system),
        ));
    }
}
#[derive(SystemParam)]
struct Context<'w> {
    state: Res<'w, State<VnState>>, menu: Res<'w, MenuState>, save: Res<'w, SaveMenuState>,
    settings_open: Res<'w, SettingsMenuState>, settings: Res<'w, Settings>, gallery: Res<'w, GalleryState>,
    history: Res<'w, DialogueHistory>, persistent: Res<'w, PersistentDataResource>,
    locale: Res<'w, LocaleConfig>, characters: Res<'w, CharacterRegistry>,
    cgs: Res<'w, CgAssetRegistry>, render: Res<'w, VnRenderState>, typing: Res<'w, TypewriterState>,
    gate: Res<'w, SaveConfirmation>, paths: Res<'w, ProjectPaths>,
    error: Res<'w, ScriptErrorMessage>,
}
fn requested_roles(ctx: &Context, menus: &Menus) -> Vec<PageRole> {
    let mut roles = Vec::new();
    let base = if ctx.save.active { Some(if ctx.save.mode == SaveMenuMode::Save { PageRole::Save } else { PageRole::Load }) }
        else if ctx.settings_open.active { Some(PageRole::Settings) }
        else { match ctx.state.get() { VnState::TitleScreen => Some(PageRole::Title), VnState::Menu => Some(PageRole::Pause),
            VnState::Gallery => Some(PageRole::Gallery),
            VnState::History => Some(PageRole::History), _ => None } };
    let base=if let Some(page)=menus.current_page() {menus.document().and_then(|document|document.pages.iter().find(|item|item.id==page)).and_then(|page|page.role)} else {base};
    if let Some(role) = base { roles.push(role); }
    if matches!(ctx.state.get(), VnState::Waiting | VnState::Stepping | VnState::Animating) {
        if !ctx.typing.full_text.is_empty() { roles.push(PageRole::Dialogue); }
        if *ctx.state.get() == VnState::Waiting {
            roles.push(PageRole::QuickActions);
            if !ctx.render.choice_options.is_empty() { roles.push(PageRole::Choices); }
        }
    }
    if ctx.gate.active() { roles.push(PageRole::Confirm); }
    roles
}
fn refresh_slots(source: &mut SourceMenus, ctx: &Context) -> Result<(), String> {
    if !ctx.save.active { source.slot_key.clear(); return Ok(()); }
    let key = format!("{}/{:?}/{:?}/{}", ctx.paths.saves.display(), ctx.save.mode, ctx.save.origin, ctx.save.revision);
    if source.slot_key != key {
        let manager = SaveManager::new(&ctx.paths.saves, SUPPORTED_SLOTS).map_err(|error| error.to_string())?;
        let locks = manager.protected_slots().map_err(|error| error.to_string())?;
        let slots = manager.list_saves().into_iter().map(|slot| (slot.slot, slot)).collect();
        source.locks = locks; source.slots = slots; source.slot_key = key;
    }
    Ok(())
}
fn authority(source: &SourceMenus, ctx: &Context, engine: &VnEngine) -> MenuAuthority {
    let mut authority = MenuAuthority::default();
    // Include source sub-screens inherited through lifecycle handlers.
    for screen in &engine.0.state.ui.screens {
        if let Some(role) = screen.host_role {
            if source.active(role) { authority.live_screens.insert(screen.name.clone()); authority.screen_roles.insert(screen.name.clone(), role); }
        } else if matches!(ctx.state.get(),VnState::Waiting|VnState::Stepping|VnState::Animating) {
            authority.live_screens.insert(screen.name.clone());
        }
    }
    authority.from_title = *ctx.state.get() == VnState::TitleScreen || ctx.menu.return_to == Some(VnState::TitleScreen);
    authority.game_active = !authority.from_title;
    authority.waiting = *ctx.state.get() == VnState::Waiting;
    authority.story_ui_active = matches!(ctx.state.get(), VnState::Waiting | VnState::Stepping | VnState::Animating);
    authority.save_active = ctx.save.active;
    authority.settings_active = ctx.settings_open.active;
    authority.gallery_active = *ctx.state.get() == VnState::Gallery;
    authority.confirmation_token = source.confirmation_token;
    authority.choices_count = ctx.render.choice_options.len();
    authority.languages = ctx.locale.available_langs.iter().cloned().collect();
    authority.unlocked_cgs = ctx.persistent.data.seen_cgs.iter().filter(|id| ctx.cgs.0.contains_key(*id)).cloned().collect();
    authority.labels = engine.0.source_menu_labels().into_iter().collect();
    for slot in if ctx.save.active {1..=SUPPORTED_SLOTS} else {1..=0} {
        let data = source.slots.get(&slot);
        authority.slots.insert(slot, SlotAuthority { occupied: data.is_some(), protected: source.locks.contains(&slot),
            compatible: data.is_some_and(|data| data.story_identity.is_none() || data.story_identity == engine.0.state.story_identity) });
    }
    authority
}
fn context_value(role: PageRole, source: &SourceMenus, ctx: &Context, engine: &VnEngine) -> Value {
    let from_title = *ctx.state.get() == VnState::TitleScreen || ctx.menu.return_to == Some(VnState::TitleScreen)
        || (ctx.save.active && ctx.save.origin == SaveMenuOrigin::TitleScreen);
    let first = source.page * 10 + 1;
    let slots: Vec<_> = (first..first + 10).map(|slot| {
        let data = source.slots.get(&(slot as u32));
        json!({"slot":slot,"occupied":data.is_some(),"protected":source.locks.contains(&(slot as u32)),
            "compatible":data.is_some_and(|data| data.story_identity.is_none() || data.story_identity == engine.0.state.story_identity),
            "label":data.map(|data| data.label.as_str()).unwrap_or(""),"timestamp":data.map(|data| data.timestamp).unwrap_or(0),
            "thumbnail":data.and_then(|data| data.thumbnail.as_deref()).unwrap_or("")})
    }).collect();
    let mut cgs: Vec<_> = ctx.cgs.0.iter().map(|(id, path)| json!({"id":id,"path":path,
        "unlocked":ctx.persistent.data.seen_cgs.contains(id),"locked":!ctx.persistent.data.seen_cgs.contains(id)})).collect();
    cgs.sort_by(|a,b| a["id"].as_str().cmp(&b["id"].as_str()));
    let endings: Vec<_> = ctx.persistent.data.seen_endings.iter().map(|id| json!({"id":id,"unlocked":true,"locked":false})).collect();
    let history: Vec<_> = ctx.history.lines.iter().map(|line| json!({"character":line.character,"text":line.text})).collect();
    let choices: Vec<_> = ctx.render.choice_options.iter().enumerate().map(|(index,text)| json!({"index":index,"shortcut":index+1,"text":text})).collect();
    let character = engine.0.current_interaction().ok().flatten().and_then(|interaction| match interaction {
        StoryInteraction::Dialogue { character, .. } => character, _ => None })
        .or_else(||engine.0.last_dialogue_interaction().ok().flatten().and_then(|interaction|match interaction {
            StoryInteraction::Dialogue { character, .. }=>character,_=>None})).unwrap_or_default();
    let (operation, slot) = ctx.gate.pending.map(|(slot,mode)| (format!("{mode:?}").to_ascii_lowercase(),slot))
        .unwrap_or_else(|| (ctx.gate.action_pending.as_ref().map(|(action,_,_)| format!("{action:?}").to_ascii_lowercase()).unwrap_or_default(),0));
    json!({"role":role.id(),"origin":if from_title {"title"} else {"in_game"},"game_active":!from_title,"diagnostic":ctx.error.0,
        "can_rollback":crate::systems::input::can_rollback(&engine.0),
        "has_save":ctx.persistent.data.last_resume_target.is_some(),
        "settings":{"music_volume":ctx.settings.music_volume,"sfx_volume":ctx.settings.sfx_volume,
            "text_speed":ctx.settings.text_speed,"auto_speed":ctx.settings.auto_speed,"typewriter":ctx.settings.typewriter,
            "fullscreen":ctx.settings.fullscreen,"language":ctx.settings.language,"languages":ctx.locale.available_langs},
        "saves":{"page":source.page+1,"pages":SUPPORTED_SLOTS/10,"slots_per_page":10,"mode":format!("{:?}",ctx.save.mode).to_ascii_lowercase(),"slots":slots},
        "gallery":{"tab":if ctx.gallery.view == GalleryView::Cg {"cg"} else {"endings"},"selected":ctx.gallery.selected_cg.as_deref().unwrap_or(""),"cgs":cgs,"endings":endings},
        "history":history,"choices":choices,
        "dialogue":{"character":ctx.characters.display_name(&character),"text":ctx.typing.full_text,
            "visible_text":ctx.typing.full_text.chars().take(ctx.typing.visible_chars).collect::<String>(),"typing":ctx.typing.typing},
        "confirmation":{"active":ctx.gate.active(),"token":source.confirmation_token.unwrap_or(0),"operation":operation,"slot":slot,
            "description":if ctx.gate.active() {"Confirmer cette opération ?"} else {""}}})
}
fn synchronize(mut source: ResMut<SourceMenus>, menus: Res<Menus>, ctx: Context,
    mut engine: ResMut<VnEngine>, mut output: EventWriter<VnCommand>) {
    engine.0.renderer.menu_can_rollback=crate::systems::input::can_rollback(&engine.0);
    source.confirmation(&ctx.gate);
    let mut desired: BTreeMap<_,_> = requested_roles(&ctx,&menus).into_iter().filter_map(|role| {
        let name=menus.document()?.source_screen(role)?;
        let key=(name.to_string(),engine.0.state.story_identity.clone(),engine.0.interface_epoch());
        if source.invalid.get(&role)==Some(&key) {None} else {Some((role,name.to_string()))}
    }).collect();
    let mut names=BTreeMap::<String,Vec<PageRole>>::new();
    for (role,name) in &desired {names.entry(name.clone()).or_default().push(*role);}
    let conflicts:Vec<_>=names.into_iter().filter(|(_,roles)|roles.len()>1).collect();
    let conflict_key=format!("{conflicts:?}");
    if source.conflict_key!=conflict_key {
        source.conflict_key=conflict_key;
        if !conflicts.is_empty() {source.diagnostic=format!("Source screens shared by simultaneous roles: {conflicts:?}");warn!("{}",source.diagnostic);}
    }
    for (_,roles) in conflicts {for role in roles {desired.remove(&role);}}
    if desired != source.roles { source.generation = source.generation.wrapping_add(1); }
    source.roles = desired;
    if source.roles.is_empty() && engine.0.state.ui.screens.is_empty() {
        engine.0.renderer.menu_authority=MenuAuthority::default();
        return;
    }
    if let Err(error) = refresh_slots(&mut source, &ctx) { source.diagnostic = error; }
    // Retire all obsolete roots first: one Screen may be shared by exclusive
    // Title/Pause roles without being mistaken for a simultaneous collision.
    let closing:std::collections::BTreeSet<_>=engine.0.state.ui.screens.iter().filter_map(|screen| {
        let role=screen.host_role?;
        let root_matches=engine.0.state.ui.screens.iter().any(|root|root.host_role==Some(role)&&root.host_root
            &&source.roles.get(&role).is_some_and(|name|root.name==*name));
        (!root_matches).then_some(role)
    }).collect();
    for role in closing {
        if let Err(error)=engine.0.synchronize_source_menu(role,None,rvn_parser::Value::Dict(Default::default()),false,0) {
            source.diagnostic=format!("{} close: {error}",role.id());
            if let Err(error)=engine.0.abandon_source_menu(role) {source.diagnostic.push_str(&format!(" / cleanup: {error}"));}
        }
    }
    let mut auth = authority(&source, &ctx, &engine);
    for (role,name) in &source.roles {
        if engine.0.source_menu_arity(name).is_some_and(|arity|arity<=1) {
            auth.live_screens.insert(name.clone());auth.screen_roles.insert(name.clone(),*role);
        }
    }
    auth.pages = menus.document().map(|document| document.pages.iter().map(|page| page.id.clone()).collect()).unwrap_or_default();
    engine.0.renderer.menu_authority = auth;
    engine.0.renderer.menu_stamp = Stamp { generation: source.generation, epoch: engine.0.interface_epoch(),
        pc: engine.0.state.current_interactive_pc, interaction: engine.0.current_interaction().ok().flatten(),
        destinations: engine.0.active_choice_indices().unwrap_or_default(), confirmation: source.confirmation_token };
    for (index, role) in PageRole::ALL.into_iter().enumerate() {
        if engine.0.renderer.menu_pending {break;}
        let name = source.roles.get(&role).cloned();
        if name.is_none() {continue;}
        let context = context_value(role, &source, &ctx, &engine);
        let value = match rvn_core::ui::value_from_json(&context) { Ok(value) => value, Err(error) => { source.diagnostic = error.to_string(); continue; } };
        let layer = if is_modal(role) {0} else {100+index as i32*10};
        match engine.0.synchronize_source_menu(role, name.as_deref(), value, is_modal(role), layer) {
            Ok(_) => {}, Err(error) => {
                source.diagnostic = format!("{}: {error}", role.id());
                warn!("Source menu: {}", source.diagnostic);
                if let Some(name)=name { source.invalid.insert(role,(name,engine.0.state.story_identity.clone(),engine.0.interface_epoch())); }
                source.roles.remove(&role);
                if let Err(error)=engine.0.abandon_source_menu(role) {
                    source.diagnostic.push_str(&format!(" / presenter cleanup: {error}"));
                }
            }
        }
    }
    // Only actual presenter instances grant authority after a rejected mapping.
    let mut auth = authority(&source, &ctx, &engine);
    auth.pages = menus.document().map(|document| document.pages.iter().map(|page| page.id.clone()).collect()).unwrap_or_default();
    engine.0.renderer.menu_authority = auth;
    for command in engine.0.renderer.take_pending() { output.send(command); }
}
fn execute_slot(ctx: &mut ActionContext, thumbnails: &crate::save_thumbnails::SaveThumbnails,
    slot: u32, mode: SaveMenuMode, approved: bool) -> Result<(),String> {
    let manager=SaveManager::new(&ctx.paths.saves,SUPPORTED_SLOTS).map_err(|error|error.to_string())?;
    let saved=manager.load(slot).ok();
    if mode!=SaveMenuMode::Load && manager.is_protected(slot).map_err(|error|error.to_string())? {return Err("Save slot is protected".into());}
    if mode!=SaveMenuMode::Save && saved.is_none() {return Err("Save slot is empty or unreadable".into());}
    if !approved && saved.is_some() && (matches!(mode,SaveMenuMode::Save|SaveMenuMode::Delete)
        || ctx.save.origin==SaveMenuOrigin::InGame) {
        ctx.confirmation.pending=Some((slot as usize,mode)); return Ok(());
    }
    match mode {
        SaveMenuMode::Save=>{
            ctx.engine.0.save(&manager,slot,format!("Sauvegarde {slot}"),"script.rvn".into()).map_err(|error|error.to_string())?;
            if let Err(error)=thumbnails.persist(&ctx.engine,&manager,&ctx.paths.saves,slot) {warn!("Save thumbnail: {error}");}
            if let Ok(saved)=manager.load(slot) {
                crate::systems::save_menu::record_resume_target(&mut ctx.persistent,rvn_core::persistent::LastResumeTarget::manual(slot,saved.timestamp));
            }
            ctx.save.active=false; ctx.save.origin=SaveMenuOrigin::InGame;
        }
        SaveMenuMode::Load=>{
            ctx.engine.0.load_data(saved.unwrap()).map_err(|error|error.to_string())?;
            crate::systems::save_menu::apply_loaded_game(&mut ctx.engine,&mut ctx.render,&mut ctx.imagemap,&mut ctx.typing,&mut ctx.history,&mut ctx.events);
            if let Ok(saved)=manager.load(slot) {
                crate::systems::save_menu::record_resume_target(&mut ctx.persistent,rvn_core::persistent::LastResumeTarget::manual(slot,saved.timestamp));
            }
            ctx.menu.return_to=None; ctx.next.set(VnState::Waiting); ctx.save.active=false; ctx.save.origin=SaveMenuOrigin::InGame;
        }
        SaveMenuMode::Delete=>{
            manager.delete(slot).map_err(|error|error.to_string())?;
            if ctx.persistent.data.last_resume_target.as_ref().is_some_and(|target|
                target.kind==rvn_core::persistent::ResumeSaveKind::Manual && target.slot==Some(slot)) {
                let mut persistent=ctx.persistent.data.clone(); persistent.last_resume_target=None;
                match ctx.persistent.manager.save(&persistent) {Ok(())=>ctx.persistent.data=persistent,Err(error)=>warn!("Deleted save resume target: {error}")}
            }
            ctx.save.revision=ctx.save.revision.wrapping_add(1);
        }
        SaveMenuMode::ToggleProtection=>return Err("Protection uses an explicit Boolean request".into()),
    }
    Ok(())
}
fn persist_preferences(ctx: &mut ActionContext, settings: &mut Settings, request: &MenuRequest) -> Result<(), String> {
    let mut next = settings.clone();
    match request {
        MenuRequest::NumberPreference { key, value } => match key { NumberPreference::MusicVolume=>next.music_volume=*value,
            NumberPreference::SfxVolume=>next.sfx_volume=*value,NumberPreference::TextSpeed=>next.text_speed=*value,NumberPreference::AutoSpeed=>next.auto_speed=*value },
        MenuRequest::BoolPreference { key, value } => match key { BoolPreference::Typewriter=>next.typewriter=*value,BoolPreference::Fullscreen=>next.fullscreen=*value },
        MenuRequest::Language { value } => next.language=value.clone(), _=>return Ok(()) }
    let mut persistent = ctx.persistent.data.clone();
    persistent.language=Some(next.language.clone()); persistent.music_volume=Some(next.music_volume);
    persistent.sfx_volume=Some(next.sfx_volume); persistent.text_speed=Some(next.text_speed); persistent.auto_speed=Some(next.auto_speed);
    persistent.typewriter=Some(next.typewriter); persistent.fullscreen=Some(next.fullscreen);
    ctx.persistent.manager.save(&persistent).map_err(|error|error.to_string())?;
    ctx.persistent.data=persistent; *settings=next; Ok(())
}
fn execute_quick(ctx: &mut ActionContext, thumbnails: &mut crate::save_thumbnails::SaveThumbnails,
    load: bool, approved: bool) -> Result<(),String> {
    let manager=SaveManager::new(&ctx.paths.saves,SUPPORTED_SLOTS).map_err(|error|error.to_string())?;
    if !load {
        manager.save_quicksave(&ctx.engine.0.state,"Quicksave".into(),"script.rvn".into()).map_err(|error|error.to_string())?;
        if let Ok(data)=manager.load_quicksave() {
            crate::systems::save_menu::record_resume_target(&mut ctx.persistent,rvn_core::persistent::LastResumeTarget::quicksave(data.timestamp));
            thumbnails.request_resume(rvn_core::save::ResumeSlot::Quick,&ctx.engine,data);
        }
    } else {
        let data=manager.load_quicksave().map_err(|error|error.to_string())?;
        if !approved {
            ctx.confirmation.pending=Some((0,SaveMenuMode::Load));
            ctx.confirmation.quick_return=Some(ctx.state.get().clone());ctx.next.set(VnState::Menu);
        } else {
            ctx.engine.0.load_data(data).map_err(|error|error.to_string())?;
            crate::systems::save_menu::apply_loaded_game(&mut ctx.engine,&mut ctx.render,&mut ctx.imagemap,&mut ctx.typing,&mut ctx.history,&mut ctx.events);
            if let Ok(data)=manager.load_quicksave() {
                crate::systems::save_menu::record_resume_target(&mut ctx.persistent,rvn_core::persistent::LastResumeTarget::quicksave(data.timestamp));
            }
            ctx.confirmation.quick_return=None;ctx.menu.return_to=None;ctx.next.set(VnState::Waiting);
        }
    }
    Ok(())
}
fn dispatch(mut commands: Commands, mut source: ResMut<SourceMenus>, mut menus: ResMut<Menus>,
    mut io:ParamSet<(ActionContext,EventReader<VnCommand>)>, mut settings: ResMut<Settings>, mut gallery: ResMut<GalleryState>,
    mut choice_focus:ResMut<ChoiceFocus>,
    mut thumbnails: ResMut<crate::save_thumbnails::SaveThumbnails>, mut diagnostic: ResMut<ScriptErrorMessage>) {
    // ActionContext writes VnCommand as it applies game effects. Read the
    // receipt queue first, through a ParamSet, rather than aliasing Events.
    let receipts:Vec<_>=io.p1().read().filter_map(|event|if let VnCommand::SourceMenu(receipt)=event{Some(receipt.clone())}else{None}).collect();
    let mut ctx=io.p0();
    for receipt in receipts {
        if receipt.identity<=source.consumed_receipt {
            // Ordered, host-issued identities survive every GameState restore.
            // An old receipt never owns the candidate state of a newer click.
            source.diagnostic="Menu receipt already consumed".into();diagnostic.0=source.diagnostic.clone();warn!("Source menu: {}",source.diagnostic);continue;
        }
        // Refusal is terminal too. A corrected click receives a new identity.
        source.consumed_receipt=receipt.identity;
        ctx.engine.0.renderer.menu_pending = false;
        let current_interaction = ctx.engine.0.current_interaction().ok().flatten();
        let current_token = if ctx.confirmation.active() && source.confirmation_key == format!("{:?}/{:?}",ctx.confirmation.pending,ctx.confirmation.action_pending) { source.confirmation_token } else { None };
        let replaced_story=receipt.stamp.epoch!=ctx.engine.0.interface_epoch() || receipt.stamp.pc!=ctx.engine.0.state.current_interactive_pc;
        let stale = receipt.stamp.generation != source.generation || replaced_story
            || receipt.stamp.pc != ctx.engine.0.state.current_interactive_pc || receipt.stamp.interaction != current_interaction
            || receipt.stamp.destinations != ctx.engine.0.active_choice_indices().unwrap_or_default()
            || receipt.stamp.confirmation != current_token;
        let mut live = ctx.engine.0.renderer.menu_authority.clone();
        live.live_screens.retain(|name| ctx.engine.0.state.ui.screens.iter().any(|screen| screen.name == *name));
        if let Some(screen) = ctx.engine.0.state.ui.screens.iter().find(|screen| screen.name == receipt.effect.screen
            && screen.order == receipt.effect.screen_order && screen.host_role == receipt.effect.host_role) {
            if screen.host_role.is_some_and(|role| source.active(role)) || (screen.host_role.is_none() && live.story_ui_active) {
                live.live_screens.insert(screen.name.clone());
                if let Some(role) = screen.host_role { live.screen_roles.insert(screen.name.clone(), role); }
            }
        } else { live.live_screens.remove(&receipt.effect.screen); }
        live.confirmation_token = current_token; live.waiting = *ctx.state.get() == VnState::Waiting;
        live.save_active = ctx.save.active; live.game_active = *ctx.state.get() != VnState::TitleScreen && ctx.menu.return_to != Some(VnState::TitleScreen);
        live.choices_count = ctx.render.choice_options.len();
        let history_transition=matches!(&receipt.effect.request,MenuRequest::Advance|MenuRequest::Choose{..}|MenuRequest::LoadSlot{..}
            |MenuRequest::Action{action:Action::Rollback|Action::QuickLoad|Action::Continue|Action::NewGame|Action::StartScene(_)});
        let confirmed_transition=matches!(&receipt.effect.request,MenuRequest::Confirm{..}) &&
            (ctx.confirmation.pending.is_some_and(|(_,mode)|mode==SaveMenuMode::Load)
                || ctx.confirmation.action_pending.as_ref().is_some_and(|(action,_,_)|matches!(action,Action::NewGame|Action::StartScene(_)|Action::Continue)));
        let mut result = if stale { Err("Stale source-menu interaction".to_string()) } else { live.validate(&receipt.effect) };
        if result.is_ok() { result = apply_request(&mut commands,&mut source,&mut menus,&mut ctx,&mut settings,&mut gallery,&mut choice_focus,&mut thumbnails,&receipt.effect); }
        if let Err(error) = result {
            source.diagnostic = error;
            // A late receipt must never rewind an independently loaded or
            // rolled-back story. Its originating transaction no longer owns it.
            if !replaced_story {
                if let Err(error) = ctx.engine.0.cancel_source_menu_transaction(receipt.previous.clone()) { source.diagnostic.push_str(&format!(" / rollback: {error}")); }
            }
            warn!("Source menu: {}",source.diagnostic);
            diagnostic.0=source.diagnostic.clone();
            for command in ctx.engine.0.renderer.take_pending() { ctx.events.send(command); }
        } else {
            source.diagnostic.clear(); diagnostic.0.clear();
            if !history_transition && !confirmed_transition {ctx.engine.0.finish_source_menu_transaction(receipt.previous.clone());}
        }
    }
}
fn apply_request(commands: &mut Commands, source: &mut SourceMenus, menus: &mut Menus, ctx: &mut ActionContext,
    settings: &mut Settings, gallery: &mut GalleryState, choice_focus:&mut ChoiceFocus, thumbnails: &mut crate::save_thumbnails::SaveThumbnails, effect: &MenuEffect) -> Result<(), String> {
    match &effect.request {
        MenuRequest::Action { action } => {
            if *action==Action::Rollback {
                if !crate::systems::input::rollback_game(&mut ctx.engine.0){return Err("Rollback could not restore a previous narrative or UI state".into());}
                crate::systems::input::present_rollback(&mut ctx.engine,&mut ctx.render,&mut ctx.imagemap,choice_focus,&mut ctx.typing,&mut ctx.events);
                ctx.next.set(VnState::Waiting);
            }
            else if *action==Action::QuickSave {execute_quick(ctx,thumbnails,false,false)?;}
            else if *action==Action::QuickLoad {execute_quick(ctx,thumbnails,true,false)?;}
            else if *action==Action::Back && *ctx.state.get()==VnState::Gallery && gallery.selected_cg.is_some() {gallery.selected_cg=None;}
            else {crate::menu_documents::dispatch_source_action(commands,menus,ctx,&effect.screen,action.clone())?;}
        }
        MenuRequest::SaveSlot {slot} | MenuRequest::LoadSlot {slot} | MenuRequest::DeleteSlot {slot} => {
            let manager=SaveManager::new(&ctx.paths.saves,SUPPORTED_SLOTS).map_err(|error|error.to_string())?;
            let mode=match effect.request { MenuRequest::SaveSlot{..}=>SaveMenuMode::Save,MenuRequest::LoadSlot{..}=>SaveMenuMode::Load,_=>SaveMenuMode::Delete };
            if mode != SaveMenuMode::Load && manager.is_protected(*slot).map_err(|error|error.to_string())? { return Err("Save slot is protected".into()); }
            if mode != SaveMenuMode::Save { let saved=manager.load(*slot).map_err(|error|error.to_string())?;
                if mode==SaveMenuMode::Load && saved.story_identity.is_some() && saved.story_identity != ctx.engine.0.state.story_identity { return Err("Saved story is incompatible".into()); } }
            execute_slot(ctx,thumbnails,*slot,mode,false)?;
        }
        MenuRequest::ProtectSlot { slot,protected } => {
            let manager=SaveManager::new(&ctx.paths.saves,SUPPORTED_SLOTS).map_err(|error|error.to_string())?;
            manager.load(*slot).map_err(|error|error.to_string())?;
            manager.set_protected(*slot,*protected).map_err(|error|error.to_string())?;
            ctx.save.revision=ctx.save.revision.wrapping_add(1);
        }
        MenuRequest::SavePage {page} => source.page=page.resolve(source.page,(SUPPORTED_SLOTS/10) as usize),
        MenuRequest::NumberPreference {..} | MenuRequest::BoolPreference {..} | MenuRequest::Language {..}=>persist_preferences(ctx,settings,&effect.request)?,
        MenuRequest::Advance => { ctx.player_events.send(PlayerInput::Advance); },
        MenuRequest::SkipTypewriter => { ctx.player_events.send(PlayerInput::SkipTypewriter); },
        MenuRequest::Choose {index} => { ctx.player_events.send(PlayerInput::Choose(*index)); },
        MenuRequest::GalleryCg {id} => gallery.selected_cg=Some(id.clone()),
        MenuRequest::GalleryTab {tab} => { gallery.view=match tab {GalleryTab::Cg=>GalleryView::Cg,GalleryTab::Endings=>GalleryView::Endings}; gallery.selected_cg=None; },
        MenuRequest::Confirm {token} | MenuRequest::CancelConfirmation {token} => {
            if source.confirmation_token != Some(*token) { return Err("Confirmation token expired".into()); }
            let approved=matches!(effect.request,MenuRequest::Confirm{..});
            if let Some((slot,mode))=ctx.confirmation.pending.filter(|(slot,_)|*slot>0) {
                if approved {execute_slot(ctx,thumbnails,slot as u32,mode,true)?;}
                ctx.confirmation.pending=None;
            } else if ctx.confirmation.pending==Some((0,SaveMenuMode::Load)) {
                if approved {execute_quick(ctx,thumbnails,true,true)?;}
                else {ctx.next.set(ctx.confirmation.quick_return.take().unwrap_or(VnState::Waiting));}
                ctx.confirmation.pending=None;
            } else {crate::menu_documents::source_confirmation(commands,menus,ctx,approved)?;}
            // Primary I/O and compatibility validation must complete before
            // consuming the operation or its exact, non-replayable token.
            source.confirmation_token=None;
            source.confirmation_key.clear();
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod confirmation_tests {
    use super::*;
    pub(crate) struct Harness {pub(crate) app:App,pub(crate) directory:std::path::PathBuf,role:PageRole,screen:&'static str,request:Option<MenuRequest>}
    impl Drop for Harness {
        fn drop(&mut self) {
            let temporary=std::env::temp_dir();
            if self.directory.parent()==Some(temporary.as_path())&&self.directory.file_name().is_some_and(|name|name.to_string_lossy().starts_with("rvn-confirmation-")) {
                let _=std::fs::remove_dir_all(&self.directory);
            }
        }
    }
    pub(crate) fn application(mode:SaveMenuMode)->Harness {
        let directory=std::env::temp_dir().join(format!("rvn-confirmation-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let manager=SaveManager::new(&directory,10).unwrap();
        let mut engine=rvn_core::Engine::new(rvn_parser::parse(r#"
handler approve(event){set qa_events=qa_events+1 local roll=random(1,10) menu.execute(event["data"]["request"])}
screen confirmation(context){return component("yes","button",{"events":{"click":"approve"},"event_data":{"request":menu_confirm(context["confirmation"]["token"])}},[])}
screen ordinary(request){return component("yes","button",{"events":{"click":"approve"},"event_data":{"request":request}},[])}
init{set qa_events=0}
label start
"Waiting"
"Next"
"#).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        engine.step_until_interaction().unwrap();engine.save(&manager,1,"Preserved".into(),"qa.rvn".into()).unwrap();
        let mut source=SourceMenus::default();source.roles.insert(PageRole::Confirm,"confirmation".into());source.generation=1;
        let mut gate=SaveConfirmation::default();gate.pending=Some((1,mode));source.confirmation(&gate);
        let persistent=rvn_core::persistent::PersistentDataManager::new(&directory).unwrap();let data=persistent.load().unwrap();
        let mut app=App::new();
        app.insert_resource(VnEngine(engine)).insert_resource(source).insert_resource(gate)
            .insert_resource(State::new(VnState::Menu)).init_resource::<NextState<VnState>>()
            .insert_resource(SaveMenuState{revision:0,active:true,mode,origin:SaveMenuOrigin::InGame})
            .init_resource::<SettingsMenuState>().init_resource::<MenuState>().init_resource::<DialogueHistory>()
            .init_resource::<ImagemapState>().init_resource::<VnRenderState>().init_resource::<TypewriterState>()
            .init_resource::<SkipMode>().init_resource::<Menus>().init_resource::<Settings>().init_resource::<GalleryState>()
            .init_resource::<ChoiceFocus>().init_resource::<crate::save_thumbnails::SaveThumbnails>().init_resource::<ScriptErrorMessage>()
            .insert_resource(ProjectPaths::new(directory.clone(),directory.clone(),directory.clone(),directory.clone(),directory.clone()))
            .insert_resource(PersistentDataResource{manager:persistent,data}).add_event::<VnCommand>().add_event::<PlayerInput>()
            .add_systems(Update,dispatch);
        let mut harness=Harness{app,directory,role:PageRole::Confirm,screen:"confirmation",request:None};harness.refresh();harness
    }
    impl Harness {
        fn present(&mut self,role:PageRole,request:MenuRequest) {
            self.app.world_mut().resource_mut::<VnEngine>().0.synchronize_source_menu(self.role,None,rvn_parser::Value::Dict(Default::default()),false,0).unwrap();
            self.app.world_mut().resource_mut::<SaveConfirmation>().pending=None;
            self.app.world_mut().resource_mut::<SettingsMenuState>().active=role==PageRole::Settings;
            {
                let mut save=self.app.world_mut().resource_mut::<SaveMenuState>();
                match role {
                    PageRole::Save=>save.open(SaveMenuMode::Save,SaveMenuOrigin::InGame),
                    PageRole::Load=>save.open(SaveMenuMode::Load,SaveMenuOrigin::InGame),
                    _=>save.active=false,
                }
            }
            let mut source=self.app.world_mut().resource_mut::<SourceMenus>();source.roles.clear();source.roles.insert(role,"ordinary".into());source.generation+=1;
            drop(source);
            self.role=role;self.screen="ordinary";self.request=Some(request);self.refresh();
        }
        pub(crate) fn refresh(&mut self) {
            let world=self.app.world_mut();
            world.resource_scope(|world,mut source:Mut<SourceMenus>|source.confirmation(world.resource::<SaveConfirmation>()));
            let source=world.resource::<SourceMenus>();let token=source.confirmation_token;let generation=source.generation;
            let save_active=world.resource::<SaveMenuState>().active;
            let settings_active=world.resource::<SettingsMenuState>().active;
            let from_title=*world.resource::<State<VnState>>().get()==VnState::TitleScreen
                ||world.resource::<MenuState>().return_to==Some(VnState::TitleScreen);
            let manager=SaveManager::new(&self.directory,SUPPORTED_SLOTS).unwrap();
            let locks=manager.protected_slots().unwrap();
            let saved:BTreeMap<_,_>=manager.list_saves().into_iter().map(|data|(data.slot,data)).collect();
            let mut engine=world.resource_mut::<VnEngine>();
            let slots=if save_active {(1..=SUPPORTED_SLOTS).map(|slot| {
                let data=saved.get(&slot);
                (slot,SlotAuthority{occupied:data.is_some(),protected:locks.contains(&slot),
                    compatible:data.is_some_and(|data|data.story_identity.is_none()||data.story_identity==engine.0.state.story_identity)})
            }).collect()}else{BTreeMap::new()};
            engine.0.renderer.menu_authority=MenuAuthority{live_screens:[self.screen.into()].into(),screen_roles:[(self.screen.into(),self.role)].into(),confirmation_token:token,game_active:!from_title,from_title,save_active,settings_active,slots,..default()};
            let stamp=Stamp{generation,epoch:engine.0.interface_epoch(),pc:engine.0.state.current_interactive_pc,interaction:engine.0.current_interaction().unwrap(),destinations:engine.0.active_choice_indices().unwrap(),confirmation:token};
            engine.0.renderer.menu_stamp=stamp;
            if token.is_some()||self.request.is_some() {
                let data=if let Some(request)=&self.request{serde_json::to_value(request).unwrap()}else{json!({"confirmation":{"token":token.unwrap()}})};
                let context=rvn_core::ui::value_from_json(&data).unwrap();
                engine.0.synchronize_source_menu(self.role,Some(self.screen),context,true,0).unwrap();
                engine.0.renderer.take_pending();
            }
        }
        pub(crate) fn click(&mut self)->Receipt {
            self.refresh();
            let mut engine=self.app.world_mut().resource_mut::<VnEngine>();
            engine.0.interface_event(rvn_core::ui::UiInput{screen:self.screen.into(),element:"yes".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}).unwrap();
            let receipt=engine.0.renderer.take_pending().into_iter().find_map(|command|if let VnCommand::SourceMenu(receipt)=command{Some(receipt)}else{None}).unwrap();
            self.app.world_mut().send_event(VnCommand::SourceMenu(receipt.clone()));self.app.update();receipt
        }
        pub(crate) fn token(&self)->Option<u64>{self.app.world().resource::<SourceMenus>().confirmation_token}
        pub(crate) fn gate(&self)->String {
            let gate=self.app.world().resource::<SaveConfirmation>();format!("{:?}/{:?}/{:?}/{:?}",gate.pending,gate.approved,gate.quick_return,gate.action_pending)
        }
        pub(crate) fn disk(&self)->BTreeMap<String,Vec<u8>> {
            std::fs::read_dir(&self.directory).unwrap().map(|entry|{let entry=entry.unwrap();(entry.file_name().to_string_lossy().into_owned(),std::fs::read(entry.path()).unwrap())}).collect()
        }
        pub(crate) fn assert_state(&self, expected:&rvn_core::GameState) {
            let actual=&self.app.world().resource::<VnEngine>().0.state;
            // JSON values compare the complete persisted state independent of map order.
            assert_eq!(serde_json::to_value(actual).unwrap(),serde_json::to_value(expected).unwrap());
            // Host ownership and allocation are transient, so compare the full UI separately.
            assert_eq!(&actual.ui,&expected.ui);
        }
    }
    #[test]
    fn changed_save_compatibility_keeps_exact_confirmation_and_retry_loads_once() {
        let mut harness=application(SaveMenuMode::Load);let slot=harness.directory.join("slot_01.json");
        let original=std::fs::read(&slot).unwrap();let mut foreign:serde_json::Value=serde_json::from_slice(&original).unwrap();foreign["story_identity"]=json!("foreign-story");std::fs::write(&slot,serde_json::to_vec_pretty(&foreign).unwrap()).unwrap();
        let before=harness.app.world().resource::<VnEngine>().0.state.clone();let disk=harness.disk();let gate=harness.gate();let token=harness.token();
        let refused=harness.click();
        harness.assert_state(&before);assert_eq!(harness.disk(),disk);assert_eq!(harness.gate(),gate);assert_eq!(harness.token(),token);
        assert!(matches!(harness.app.world().resource::<NextState<VnState>>(),NextState::Unchanged));assert!(!harness.app.world().resource::<ScriptErrorMessage>().0.is_empty());
        std::fs::write(&slot,original).unwrap();let receipt=harness.click();
        assert!(harness.token().is_none());assert!(!harness.app.world().resource::<SaveConfirmation>().active());assert!(harness.app.world().resource::<ScriptErrorMessage>().0.is_empty());
        assert_eq!(harness.app.world().resource::<VnEngine>().0.state.vars["qa_events"],rvn_parser::Value::Int(0));assert_eq!(harness.app.world().resource::<VnEngine>().0.history.len(),0);
        let committed=harness.app.world().resource::<VnEngine>().0.state.clone();let disk=harness.disk();
        assert!(receipt.identity>refused.identity);
        harness.app.world_mut().send_event(VnCommand::SourceMenu(refused));harness.app.world_mut().send_event(VnCommand::SourceMenu(receipt));harness.app.update();
        harness.assert_state(&committed);assert_eq!(harness.disk(),disk);assert!(harness.token().is_none());
    }
    #[cfg(windows)]
    #[test]
    fn failed_atomic_save_keeps_confirmation_and_successful_retry_commits_one_handler() {
        use std::os::windows::fs::OpenOptionsExt;
        let mut harness=application(SaveMenuMode::Save);let slot=harness.directory.join("slot_01.json");
        let lock=std::fs::OpenOptions::new().read(true).share_mode(1).open(&slot).unwrap();
        let before=harness.app.world().resource::<VnEngine>().0.state.clone();let disk=harness.disk();let gate=harness.gate();let token=harness.token();
        let refused=harness.click();
        harness.assert_state(&before);assert_eq!(harness.disk(),disk);assert_eq!(harness.gate(),gate);assert_eq!(harness.token(),token);
        assert!(matches!(harness.app.world().resource::<NextState<VnState>>(),NextState::Unchanged));assert!(!harness.app.world().resource::<ScriptErrorMessage>().0.is_empty());drop(lock);
        let receipt=harness.click();assert!(harness.token().is_none());assert!(!harness.app.world().resource::<SaveConfirmation>().active());assert!(harness.app.world().resource::<ScriptErrorMessage>().0.is_empty());
        assert_eq!(harness.app.world().resource::<VnEngine>().0.state.vars["qa_events"],rvn_parser::Value::Int(1));
        let saved=SaveManager::new(&harness.directory,10).unwrap().load(1).unwrap();assert_eq!(rvn_parser::Value::from(saved.vars["qa_events"].clone()),rvn_parser::Value::Int(1));
        let committed=harness.app.world().resource::<VnEngine>().0.state.clone();let disk=harness.disk();
        assert!(receipt.identity>refused.identity);
        harness.app.world_mut().send_event(VnCommand::SourceMenu(refused));harness.app.world_mut().send_event(VnCommand::SourceMenu(receipt));harness.app.update();
        harness.assert_state(&committed);assert_eq!(harness.disk(),disk);assert!(harness.token().is_none());
    }
    #[test]
    fn successful_preference_and_save_slot_receipts_cannot_rewind_or_rewrite_at_same_pc() {
        let mut harness=application(SaveMenuMode::Save);
        harness.present(PageRole::Settings,MenuRequest::NumberPreference{key:NumberPreference::MusicVolume,value:0.25});
        harness.app.world_mut().resource_mut::<SettingsMenuState>().active=true;
        let preference=harness.click();assert_eq!(harness.app.world().resource::<Settings>().music_volume,0.25);
        let after_preference=harness.app.world().resource::<VnEngine>().0.state.clone();let pref_disk=harness.disk();let history=harness.app.world().resource::<VnEngine>().0.history.len();
        harness.app.world_mut().send_event(VnCommand::SourceMenu(preference.clone()));harness.app.update();
        harness.assert_state(&after_preference);assert_eq!(harness.disk(),pref_disk);assert_eq!(harness.app.world().resource::<VnEngine>().0.history.len(),history);
        harness.present(PageRole::Save,MenuRequest::SaveSlot{slot:2});
        assert!(harness.app.world().resource::<SaveMenuState>().active);
        assert!(!harness.app.world().resource::<SettingsMenuState>().active);
        assert_eq!(harness.app.world().resource::<VnEngine>().0.renderer.menu_authority.screen_roles["ordinary"],PageRole::Save);
        assert!(!harness.app.world().resource::<VnEngine>().0.renderer.menu_authority.slots[&2].occupied);
        let save=harness.click();assert!(save.identity>preference.identity);
        let after_save=harness.app.world().resource::<VnEngine>().0.state.clone();let saved_disk=harness.disk();let history=harness.app.world().resource::<VnEngine>().0.history.len();
        assert_eq!(after_save.pc,after_preference.pc);assert_eq!(save.stamp.epoch,preference.stamp.epoch);
        assert_eq!(after_save.vars["qa_events"],rvn_parser::Value::Int(2));assert!(SaveManager::new(&harness.directory,10).unwrap().load(2).is_ok());
        harness.app.world_mut().send_event(VnCommand::SourceMenu(preference));harness.app.world_mut().send_event(VnCommand::SourceMenu(save));harness.app.update();
        harness.assert_state(&after_save);assert_eq!(harness.disk(),saved_disk);assert_eq!(harness.app.world().resource::<VnEngine>().0.history.len(),history);
        assert_eq!(harness.app.world().resource::<Settings>().music_volume,0.25);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_dispatch_initializes_without_aliasing_its_command_event_queue() {
        let mut world=World::new();
        let mut system=IntoSystem::into_system(dispatch);
        system.initialize(&mut world);
    }
    #[test] fn token_changes_after_identical_confirmation_is_reopened() {
        let mut source=SourceMenus::default(); let mut gate=SaveConfirmation::default();
        gate.pending=Some((1,SaveMenuMode::Delete)); source.confirmation(&gate); let first=source.confirmation_token.unwrap();
        gate.pending=None; source.confirmation(&gate); assert!(source.confirmation_token.is_none());
        gate.pending=Some((1,SaveMenuMode::Delete)); source.confirmation(&gate); assert_ne!(first,source.confirmation_token.unwrap());
    }
    #[test] fn narrative_roles_do_not_block_existing_numerical_choice_shortcuts() {
        for role in [PageRole::Dialogue,PageRole::Choices,PageRole::QuickActions] { assert!(!is_modal(role)); }
        for role in [PageRole::Title,PageRole::Pause,PageRole::Save,PageRole::Load,PageRole::Settings,PageRole::Gallery,PageRole::History,PageRole::Confirm] { assert!(is_modal(role)); }
    }
    #[test] fn stamp_distinguishes_same_count_different_story_choices() {
        let first=Stamp { pc:2,interaction:Some(StoryInteraction::Choice{options:vec!["left".into(),"right".into()]}),..default() };
        let mut changed=first.clone(); changed.interaction=Some(StoryInteraction::Choice{options:vec!["north".into(),"south".into()]});
        assert_ne!(first,changed); changed=first.clone(); changed.epoch+=1; assert_ne!(first,changed);
        changed=first.clone();changed.destinations=vec![1];assert_ne!(first,changed);
    }
}
