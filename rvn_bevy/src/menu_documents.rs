//! Optional user-authored menu presentation. Built-in actions remain authoritative.
use crate::{
    project_paths::ProjectPaths,
    resources::*,
    systems::{
        gallery::{GalleryButton, GalleryOverlay},
        history::HistoryOverlay,
        menu::{MenuButton, MenuOverlay},
        save_menu::{
            SaveMenuCancelButton, SaveMenuMode, SaveMenuOverlay, SaveMenuState, SaveSlotButton,
        },
        settings_menu::{SettingsButton, SettingsMenuOverlay, SettingsMenuState},
        title::{TitleButton, TitleButtonColors, TitleOverlay},
    },
};
use bevy::{ecs::system::SystemParam, prelude::*};
use rvn_ui::{Action, Document, Effect, Element, Kind};

pub struct MenuDocumentsPlugin;
#[cfg(target_arch = "wasm32")]
pub(crate) fn web_menus(doc:Option<Document>)->Result<Menus,String>{
    let mut menus=Menus::default();
    if let Some(doc)=doc{menus.session.initialize_controls(&doc)?;menus.doc=Some(doc);}
    Ok(menus)
}
#[path="menu_narrative.rs"]mod narrative;
#[path="menu_confirmation.rs"]mod confirmation;
#[path="menu_scrollbars.rs"]mod scrollbars;
#[path="menu_dropdown.rs"]mod dropdown;
#[path="menu_dynamic_rows.rs"]mod dynamic_rows;
#[path="menu_animations.rs"]mod animations;
#[path="menu_shadows.rs"]mod shadows;
#[path="menu_text_states.rs"]mod text_states;
#[path="menu_image_states.rs"]mod image_states;
#[path="menu_layout.rs"]mod menu_layout;
#[path="menu_local_controls.rs"]mod local_controls;
pub(crate) use confirmation::ConfirmationButton;
pub(crate) use dropdown::{LanguageSelect,Dropdown};
pub(crate) use animations::VisualNode;
pub(crate) use scrollbars::Scrollbar;
fn report_preview_error(session:&rvn_ui::Session){
    if std::env::var_os("RVN_UI_PREVIEW_DATA").is_none(){return;}
    let Some(path)=std::env::var_os("RVN_UI_DIAGNOSTICS")else{return};
    if let Some(diagnostic)=&session.last_error{if let Ok(data)=serde_json::to_vec(diagnostic){if let Err(error)=std::fs::write(path,data){warn!("Diagnostic d’aperçu : {error}");}}}
}
impl Plugin for MenuDocumentsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Menus>()
            .init_resource::<local_controls::Drag>()
            .init_resource::<crate::save_thumbnails::SaveThumbnails>()
            .add_systems(Last,crate::save_thumbnails::capture)
            .add_systems(PostUpdate,(text_states::update,dropdown::text_colors))
            .add_systems(PostUpdate,image_states::update.after(bevy::ui::UiSystem::Layout).before(animations::fade))
            .add_systems(Update,crate::save_thumbnails::load.before(render))
            .init_resource::<dropdown::Dropdown>()
            .init_resource::<animations::RuntimeAnimations>()
            .init_resource::<crate::systems::save_menu::SaveConfirmation>()
            .add_systems(Startup, load_menu_font)
            .add_systems(
                Update,
                (gamepad_input, load, local_controls::update, sliders, render, dropdown::update, keyboard, focus_visuals, scrollbars::drag, scroll_lists, paginate, interact)
                    .chain()
                    .before(crate::systems::input_system)
                    .before(crate::systems::title_interaction_system)
                    .before(crate::systems::menu_interaction_system)
                    .before(crate::systems::save_menu_interaction_system)
                    .before(crate::systems::settings_menu_interaction_system),
            )
            .add_systems(Update,narrative::render_narrative.after(crate::systems::dialogue_system).after(crate::systems::typewriter_system).after(crate::systems::apply_theme_system).after(render))
            .add_systems(Update,narrative::render_quick_actions.after(render).before(keyboard))
            .add_systems(Update,confirmation::update.after(interact).before(crate::systems::save_menu_interaction_system).before(crate::systems::menu_input_system))
            .add_systems(Update,(narrative::render_choices,narrative::choice_focus_visuals,narrative::choice_scroll_bounds).chain().after(crate::systems::update_choice_buttons).after(crate::systems::choice_interaction_system).before(scroll_lists))
            .add_systems(Last, cleanup_proxies);
        app.add_systems(Update,animations::start.after(interact));
        app.add_systems(PostUpdate,(animations::transform,animations::fade).chain().after(bevy::ui::UiSystem::Layout).before(bevy::transform::TransformSystem::TransformPropagate));
        app.add_systems(PostUpdate,shadows::follow.after(bevy::transform::TransformSystem::TransformPropagate));
        app.add_systems(PostUpdate,(dropdown::reveal_selection,dynamic_rows::grow,auto_list_bounds,narrative::follow_dialogue,scroll_to_focus,scrollbars::create,scrollbars::sync).chain().after(bevy::ui::UiSystem::Layout).before(animations::fade));
    }
}
#[derive(Resource, Default)]
pub(crate) struct Menus {
    doc: Option<Document>,
    modified: Option<std::time::SystemTime>,
    key: String,
    page: Option<String>,
    history: Vec<String>,
    overlay_return:Option<VnState>,
    pub(crate) session: rvn_ui::Session,
    error: String,
    start_label: Option<String>,
    pending: Option<Action>,
    focus: Option<Entity>,
    focus_key: Option<String>,
    pressed: Option<Entity>,
    gamepad_navigation:bool,
    list_pages:std::collections::BTreeMap<String,usize>,
    slider_drag:Option<(String,f32,f32)>,
    active_page:Option<String>,
    page_events:Vec<(String,rvn_ui::Event)>,
    hovering:std::collections::BTreeSet<(String,String)>,
    automatic_navigations:usize,
    activations:Vec<(String,Option<String>,rvn_ui::Event,Action)>,
    settings_values:std::collections::BTreeMap<String,serde_json::Value>,
    animation_requests:Vec<(String,String,rvn_ui::AnimationClip)>,
}
impl Menus {
    pub(crate) fn document(&self)->Option<&Document>{self.doc.as_ref()}
    pub(crate) fn choices_interactive(&self)->bool{
        let Some(doc)=&self.doc else{return true};let Some(page)=doc.pages.iter().position(|p|p.role==Some(rvn_ui::PageRole::Choices))else{return true};
        self.session.present(doc).layout_page(page,doc.reference).iter().any(|e|e.kind==Kind::ChoiceList&&e.enabled)
    }
}
/// Route controller activations through the same consumable keys as keyboard
/// input. In particular closing a dropdown must not activate its parent again.
fn gamepad_input(mut pads:ResMut<ButtonInput<GamepadButton>>,mut keys:ResMut<ButtonInput<KeyCode>>,mut menus:ResMut<Menus>){
    menus.gamepad_navigation=false;
    let pressed:Vec<_>=pads.get_just_pressed().copied().collect();
    for button in pressed{let key=match button.button_type{
        GamepadButtonType::DPadUp=>{menus.gamepad_navigation=true;KeyCode::ArrowUp},
        GamepadButtonType::DPadDown=>{menus.gamepad_navigation=true;KeyCode::ArrowDown},
        GamepadButtonType::DPadLeft=>KeyCode::ArrowLeft,GamepadButtonType::DPadRight=>KeyCode::ArrowRight,
        GamepadButtonType::South=>KeyCode::Enter,GamepadButtonType::East=>KeyCode::Escape,_=>continue,
    };if !keys.pressed(key){keys.press(key);keys.release(key);}pads.clear_just_pressed(button);}
}
impl Menus{pub(crate) fn custom_choices(&self)->bool{self.doc.as_ref().is_some_and(|d|d.pages.iter().any(|p|p.role==Some(rvn_ui::PageRole::Choices)))}}
#[derive(Component)]
pub(crate) struct MenuSlider(pub(crate) String);
fn slider_range(binding:&str)->Option<(f32,f32)>{match binding{"music_volume"|"sfx_volume"=>Some((0.0,1.0)),"text_speed"|"auto_speed"=>Some((0.1,5.0)),_=>None}}
fn sliders(confirmation:Res<crate::systems::save_menu::SaveConfirmation>,dropdown:Res<dropdown::Dropdown>,mut menus:ResMut<Menus>,mouse:Res<ButtonInput<MouseButton>>,keys:Res<ButtonInput<KeyCode>>,pads:Res<ButtonInput<GamepadButton>>,windows:Query<&Window>,query:Query<(&Interaction,&MenuSlider,&Node,&GlobalTransform)>,focused:Query<&MenuSlider>,mut values:ResMut<crate::systems::settings_menu::Settings>,mut persistent:ResMut<PersistentDataResource>){
    if dropdown.open||confirmation.active(){return;}
    let mut keyboard_changed=false;
    let direction=if keys.just_pressed(KeyCode::ArrowLeft)||pads.get_just_pressed().any(|p|p.button_type==GamepadButtonType::DPadLeft){-1.0}else if keys.just_pressed(KeyCode::ArrowRight)||pads.get_just_pressed().any(|p|p.button_type==GamepadButtonType::DPadRight){1.0}else{0.0};
    if direction!=0.0{if let Some(slider)=menus.focus.and_then(|e|focused.get(e).ok()){if let Some((min,max))=slider_range(&slider.0){let slot=match slider.0.as_str(){"music_volume"=>&mut values.music_volume,"sfx_volume"=>&mut values.sfx_volume,"text_speed"=>&mut values.text_speed,_=>&mut values.auto_speed};*slot=(*slot+direction*(max-min)*0.05).clamp(min,max);keyboard_changed=true;}}}
    if mouse.just_pressed(MouseButton::Left){for (interaction,slider,node,transform) in &query{if *interaction==Interaction::Pressed{let left=transform.translation().x-node.size().x*0.5+12.0;menus.slider_drag=Some((slider.0.clone(),left,(node.size().x-24.0).max(1.0)));break}}}
    if mouse.pressed(MouseButton::Left){if let (Some((binding,left,width)),Some(cursor))=(&menus.slider_drag,windows.get_single().ok().and_then(|w|w.cursor_position())){if let Some((min,max))=slider_range(binding){let value=min+(max-min)*((cursor.x-left)/width).clamp(0.0,1.0);match binding.as_str(){"music_volume"=>values.music_volume=value,"sfx_volume"=>values.sfx_volume=value,"text_speed"=>values.text_speed=value,"auto_speed"=>values.auto_speed=value,_=>{}}}}}
    if (!mouse.pressed(MouseButton::Left)&&menus.slider_drag.take().is_some())||keyboard_changed{persistent.data.music_volume=Some(values.music_volume);persistent.data.sfx_volume=Some(values.sfx_volume);persistent.data.text_speed=Some(values.text_speed);persistent.data.auto_speed=Some(values.auto_speed);if let Err(e)=persistent.manager.save(&persistent.data){error!("[menus] Enregistrement du réglage impossible : {e}");}}
}
#[derive(Component)]
struct MenuRoot;
#[derive(Resource)]
struct MenuFont(Handle<Font>);
fn load_menu_font(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    commands.insert_resource(MenuFont(
        fonts.add(
            Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec())
                .expect("Bundled menu font"),
        ),
    ));
}
#[derive(Component)]
struct MenuProxy;
#[derive(Component,Clone)]
pub(crate) struct MenuElement {
    page: String,
    id: String,
    pub(crate) action: Action,
    normal: Color,
    hover: Color,
    pressed: Color,
}
#[derive(Component)]
struct MenuFocus(i32,String);
#[derive(Component)]
struct FocusAppearance(Color);
fn focus_visuals(mut commands:Commands,menus:Res<Menus>,query:Query<(Entity,&FocusAppearance)>){
    for (entity,appearance) in &query{if menus.focus==Some(entity){commands.entity(entity).insert(Outline{width:Val::Px(2.0),offset:Val::Px(2.0),color:appearance.0});}else{commands.entity(entity).remove::<Outline>();}}
}
#[derive(Component)]struct PageButton{page:usize,owner:String}
fn paginate(mut menus:ResMut<Menus>,buttons:Query<(&Interaction,&PageButton),Changed<Interaction>>){for (interaction,button) in &buttons{if *interaction==Interaction::Pressed{menus.list_pages.insert(button.owner.clone(),button.page);menus.key.clear();}}}
#[derive(Component)]
pub(crate) struct MenuScroll {
    pub(crate) offset: f32,
    pub(crate) content_height: f32,
}
#[derive(Component)]struct AutoList;
fn auto_list_bounds(mut lists:Query<(&Children,&Style,&mut MenuScroll),With<AutoList>>,nodes:Query<&Node,Without<scrollbars::Scrollbar>>){for(children,style,mut scroll)in &mut lists{let gap=if let Val::Px(gap)=style.row_gap{gap}else{0.0};scroll.content_height=children.iter().filter_map(|id|nodes.get(*id).ok()).map(|n|n.size().y).sum::<f32>()+gap*children.iter().filter(|id|nodes.get(**id).is_ok()).count().saturating_sub(1) as f32;}}
fn scroll_to_focus(menus:Res<Menus>,mut previous:Local<Option<Entity>>,nodes:Query<(&Node,&GlobalTransform)>,parents:Query<&Parent>,mut lists:Query<(&Children,&mut MenuScroll)>,mut styles:Query<&mut Style,Without<scrollbars::Scrollbar>>){
    if menus.focus==*previous{return;}*previous=menus.focus;
    let Some(target)=menus.focus else{return;};let Ok((target_node,target_transform))=nodes.get(target)else{return;};
    let top=target_transform.translation().y-target_node.size().y*0.5;let bottom=top+target_node.size().y;
    let mut ancestor=target;
    for _ in 0..64{let Ok(parent)=parents.get(ancestor)else{break};ancestor=parent.get();let Ok((children,mut scroll))=lists.get_mut(ancestor)else{continue};let Ok((node,transform))=nodes.get(ancestor)else{continue};
        let origin=transform.translation().y-node.size().y*0.5;let delta=if top<origin{top-origin}else{(bottom-origin-node.size().y).max(0.0)};
        let old=scroll.offset;scroll.offset=(old+delta).clamp(0.0,(scroll.content_height-node.size().y).max(0.0));
        for child in children{if let Ok(mut style)=styles.get_mut(*child){if let Val::Px(y)=style.top{style.top=Val::Px(y+old-scroll.offset);}}}
    }
}
fn keyboard(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    pads: Res<ButtonInput<GamepadButton>>,
    mut menus: ResMut<Menus>,
    mut query: Query<(Entity, &MenuFocus, &mut Interaction)>,
    render_state:Res<VnRenderState>,
    confirmation:Res<crate::systems::save_menu::SaveConfirmation>,
    dropdown:Res<dropdown::Dropdown>,
) {
    if dropdown.open||confirmation.active(){return;}
    if let Some(previous) = menus.pressed.take() {
        if let Ok((_, _, mut i)) = query.get_mut(previous) {
            *i = Interaction::Hovered;
        }
    }
    if menus.active_page.is_none()&&!render_state.choice_options.is_empty(){menus.focus=None;menus.focus_key=None;return;}
    let mut items: Vec<_> = query
        .iter()
        .map(|(entity, focus, _)| (focus.0, entity,focus.1.clone()))
        .collect();
    items.sort_by_key(|(order, _,key)| (*order,key.clone()));
    if items.is_empty() {
        menus.focus = None;
        return;
    }
    if menus.focus.is_some_and(|entity|query.get(entity).is_err()){
        menus.focus=items.iter().find(|(_,_,key)|Some(key)==menus.focus_key.as_ref()).map(|(_,entity,_)|*entity);
    }
    let pad=|button|pads.get_just_pressed().any(|p|p.button_type==button);
    if keys.just_pressed(KeyCode::Escape)||pad(GamepadButtonType::East){if let Some(page)=menus.active_page.clone(){keys.clear_just_pressed(KeyCode::Escape);menus.activations.push((page,None,rvn_ui::Event::Click,Action::Back));menus.automatic_navigations=0;return;}}
    let backward = keys.just_pressed(KeyCode::ArrowUp) || pad(GamepadButtonType::DPadUp)
        || (keys.just_pressed(KeyCode::Tab)
            && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)));
    if keys.just_pressed(KeyCode::Tab) || ((menus.active_page.is_some()||menus.focus.is_some()||menus.gamepad_navigation)&&(backward || keys.just_pressed(KeyCode::ArrowDown))) {
        let current = items.iter().position(|(_, e,_)| Some(*e) == menus.focus);
        let index = match current {
            Some(i) if backward => (i + items.len() - 1) % items.len(),
            Some(i) => (i + 1) % items.len(),
            None if backward => items.len() - 1,
            None => 0,
        };
        if let Some(old) = menus.focus {
            if let Ok((_, _, mut i)) = query.get_mut(old) {
                *i = Interaction::None;
            }
        }
        menus.focus = Some(items[index].1);
        menus.focus_key = Some(items[index].2.clone());
        if let Some((page,id))=items[index].2.split_once('/'){
            // Quick-action and dialogue pages coexist with gameplay and do
            // not occupy active_page. Their focus graphs still belong to them.
            if menus.doc.as_ref().is_some_and(|doc|doc.pages.iter().any(|p|p.id==page)){
                menus.activations.push((page.into(),Some(id.into()),rvn_ui::Event::Focus,Action::None));
            }
        }
        if let Ok((_, _, mut i)) = query.get_mut(items[index].1) {
            *i = Interaction::Hovered;
        }
        for key in [KeyCode::Tab,KeyCode::ArrowUp,KeyCode::ArrowDown]{keys.clear_just_pressed(key);}
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) || pad(GamepadButtonType::South) {
        if let Some(entity) = menus.focus {
            if let Ok((_, _, mut i)) = query.get_mut(entity) {
                *i = Interaction::Pressed;
                menus.pressed = Some(entity);
                keys.clear_just_pressed(KeyCode::Enter);keys.clear_just_pressed(KeyCode::Space);
            }
        }
    }
}
fn scroll_lists(
    mut wheel: EventReader<bevy::input::mouse::MouseWheel>,
    windows: Query<&Window>,
    mut lists: Query<(Entity,&Node, &GlobalTransform, &Children, &mut MenuScroll)>,
    parents:Query<&Parent>,
    dropdown:Res<dropdown::Dropdown>,
    popups:Query<Entity,With<dropdown::Popup>>,
    mut styles: Query<&mut Style,Without<scrollbars::Scrollbar>>,
) {
    let delta: f32 = wheel
        .read()
        .map(|e| {
            e.y * if e.unit == bevy::input::mouse::MouseScrollUnit::Line {
                44.0
            } else {
                1.0
            }
        })
        .sum();
    if delta == 0.0 {
        return;
    }
    let Some(cursor) = windows.get_single().ok().and_then(Window::cursor_position) else {
        return;
    };
    let contains=|node:&Node,transform:&GlobalTransform|{let origin=transform.translation().truncate()-node.size()*0.5;cursor.x>=origin.x&&cursor.y>=origin.y&&cursor.x<origin.x+node.size().x&&cursor.y<origin.y+node.size().y};
    let target=lists.iter().filter(|(_,n,t,_,s)|contains(n,t)&&(s.offset-delta).clamp(0.0,(s.content_height-n.size().y).max(0.0))!=s.offset).filter_map(|(id,_,_,_,_)|{
        let mut depth=0;let mut at=id;let mut in_popup=false;while let Ok(parent)=parents.get(at){at=parent.get();depth+=1;in_popup|=popups.contains(at);if depth>64{return None;}if let Ok((_,n,t,_,_))=lists.get(at){if !contains(n,t){return None;}}}if dropdown.open&&!in_popup{return None;}Some((depth,id))
    }).max_by_key(|(depth,_)|*depth).map(|(_,id)|id);
    if let Some(target)=target{if let Ok((_,node,_,children,mut scroll))=lists.get_mut(target){
        let old = scroll.offset;
        scroll.offset = (old - delta).clamp(0.0, (scroll.content_height - node.size().y).max(0.0));
        let change = old - scroll.offset;
        for child in children.iter() {
            if let Ok(mut style) = styles.get_mut(*child) {
                if let Val::Px(top) = style.top {
                    style.top = Val::Px(top + change);
                }
            }
        }
    }}
}
fn color(c: [f32; 4]) -> Color {
    Color::srgba(c[0], c[1], c[2], c[3])
}
fn load(paths: Res<ProjectPaths>, mut menus: ResMut<Menus>, engine: Res<VnEngine>) {
    if menus.start_label.is_none() {
        if let Some(rvn_parser::Statement::Label { name }) = engine.0.script.get(engine.0.state.pc)
        {
            menus.start_label = Some(name.clone());
        }
    }
    // Web documents are fetched before startup; there is no filesystem watcher.
    if cfg!(target_arch = "wasm32") { return; }
    let path = match rvn_ui::document_path(&paths.root) {
        Ok(path) => path,
        Err(e) => {
            if menus.error != e {
                error!("Menus : {e}");
                menus.error = e;
            }
            return;
        }
    };
    let modified = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok());
    if modified == menus.modified {
        return;
    }
    menus.modified = modified;
    if modified.is_none() {
        menus.doc = None;
        menus.key.clear();
        return;
    }
    let labels = engine
        .0
        .script
        .iter()
        .filter_map(|s| {
            if let rvn_parser::Statement::Label { name } = s {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();
    let result = std::fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|s| Document::from_json(&s))
        .and_then(|d| {
            d.validate_files(&paths.assets, &labels)?;
            Ok(d)
        });
    match result {
        Ok(doc) => {
            menus.session = rvn_ui::Session::default();
            if let Err(error)=menus.session.initialize_controls(&doc){error!("Contrôles d’interface : {error}");return;}
            menus.doc = Some(doc);
            menus.active_page = None;
            menus.page_events.clear();
            menus.hovering.clear();
            menus.automatic_navigations = 0;
            menus.activations.clear();
            menus.settings_values.clear();
            menus.focus=None;menus.focus_key=None;menus.pressed=None;menus.list_pages.clear();
            menus.page=None;menus.history.clear();menus.pending=None;menus.overlay_return=None;
            menus.key.clear();
            menus.error.clear();
        }
        Err(e) => {
            error!("Menus : {e}");
            menus.error = e;
        }
    }
}
type OldMenus = Or<(
    With<TitleOverlay>,
    With<MenuOverlay>,
    With<SaveMenuOverlay>,
    With<SettingsMenuOverlay>,
    With<GalleryOverlay>,
    With<HistoryOverlay>,
)>;
#[derive(SystemParam)]
struct Context<'w, 's> {
    engine: Res<'w, VnEngine>,
    state: Res<'w, State<VnState>>,
    save: Res<'w, SaveMenuState>,
    settings: Res<'w, SettingsMenuState>,
    gallery: Res<'w, GalleryState>,
    cgs:Res<'w,CgAssetRegistry>,
    persistent: Res<'w, PersistentDataResource>,
    history: Res<'w, DialogueHistory>,
    paths: Res<'w, ProjectPaths>,
    windows: Query<'w, 's, &'static Window>,
    theme: Res<'w, Theme>,
    font: Res<'w, MenuFont>,
    images: Res<'w,Assets<Image>>,
    fonts: Res<'w,Assets<Font>>,
    thumbnails:Res<'w,crate::save_thumbnails::SaveThumbnails>,
    values: Res<'w, crate::systems::settings_menu::Settings>,
    menu: Res<'w, MenuState>,
}
fn translated(ctx:&Context,text:&str)->String {
    ctx.engine.0.locale.as_ref().map(|locale|locale.translate(text)).unwrap_or(text).to_owned()
}
fn render(
    mut commands: Commands,
    mut menus: ResMut<Menus>,
    ctx: Context,
    assets: Res<AssetServer>,
    roots: Query<Entity, With<MenuRoot>>,
    mut old: Query<&mut Style, (OldMenus, Without<MenuRoot>)>,
) {
    let base = if ctx.save.active {
        if ctx.save.mode == SaveMenuMode::Save {
            "save"
        } else {
            "load"
        }
    } else if ctx.settings.active {
        "settings"
    } else {
        match ctx.state.get() {
            VnState::TitleScreen => "title",
            VnState::Menu => "pause",
            VnState::Gallery if ctx.gallery.selected_cg.is_none() => "gallery",
            VnState::History => "history",
            _ => "",
        }
    };
    let page_id = menus.page.clone().unwrap_or_else(|| {
        menus.doc.as_ref().and_then(|d| d.pages.iter().find(|p|
            p.role.is_some() && p.role == rvn_ui::PageRole::from_id(base)))
            .map(|p| p.id.clone()).unwrap_or_default()
    });
    menus.session.variables.insert(
        "state.has_save".into(),
        ctx.persistent.data.last_resume_target.is_some().into(),
    );
    menus.session.variables.insert(
        "state.game_active".into(),
        (*ctx.state.get() != VnState::TitleScreen
            && ctx.menu.return_to != Some(VnState::TitleScreen))
        .into(),
    );
    let page = menus
        .doc
        .as_ref()
        .and_then(|d| d.pages.iter().find(|p| p.id == page_id))
        .cloned();
    let active = page.is_some() && !base.is_empty();
    let settings_values:std::collections::BTreeMap<String,serde_json::Value>=[("music_volume",serde_json::json!(ctx.values.music_volume)),("sfx_volume",serde_json::json!(ctx.values.sfx_volume)),("text_speed",serde_json::json!(ctx.values.text_speed)),("auto_speed",serde_json::json!(ctx.values.auto_speed)),("typewriter",serde_json::json!(ctx.values.typewriter)),("fullscreen",serde_json::json!(ctx.values.fullscreen)),("language",serde_json::json!(ctx.values.language))].into_iter().map(|(k,v)|(k.into(),v)).collect();
    if active{if let Some(page)=&page{let changed:std::collections::BTreeSet<_>=page.graphs.iter().filter(|g|g.event==rvn_ui::Event::ValueChanged).filter_map(|g|{let id=g.target.as_ref()?;let doc=menus.doc.as_ref()?;let index=doc.pages.iter().position(|p|p.id==page.id)?;let element=doc.resolved_element(doc.find_element(index,id)?);let binding=element.binding.as_ref()?;let old=menus.settings_values.get(binding)?;let value=settings_values.get(binding)?;if old!=value{Some(id.clone())}else{None}}).collect();for id in changed{menus.activations.push((page.id.clone(),Some(id),rvn_ui::Event::ValueChanged,Action::None));}}}
    for (binding,value) in &settings_values{menus.session.variables.insert(format!("state.settings.{binding}"),value.clone());}menus.settings_values=settings_values;
    let active_page=if active{Some(page_id.clone())}else{None};
    if menus.active_page!=active_page{if let Some(old)=menus.active_page.take(){menus.page_events.push((old,rvn_ui::Event::Close));}if let Some(new)=&active_page{menus.page_events.push((new.clone(),rvn_ui::Event::Open));}menus.active_page=active_page;menus.hovering.clear();}
    for mut style in &mut old {
        style.display = if active { Display::None } else { Display::Flex };
    }
    if !active {
        for e in &roots {
            commands.entity(e).despawn_recursive();
        }
        menus.key.clear();
        return;
    }
    let Ok(window) = ctx.windows.get_single() else { return; };
    let size = [window.width(), window.height()];
    let doc=menus.session.present(menus.doc.as_ref().unwrap());
    let image_state:Vec<_>=doc.resource_paths().into_iter().filter_map(|path|assets.get_handle::<Image>(path).and_then(|handle|ctx.images.get(&handle).map(|image|(handle.id(),image.size())))).collect();
    let key = format!(
        "{base}:{page_id}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
        size[0],
        size[1],
        ctx.values.music_volume,
        ctx.values.sfx_volume,
        ctx.values.text_speed,
        ctx.values.typewriter,
        ctx.values.fullscreen,
        ctx.values.language,format!("{:?}",menus.list_pages)
    );
    let presentation:Vec<_>=menus.session.presentation.iter().map(|(id,s)|(id,s.visible,s.enabled,&s.text,&s.image)).collect();
    let locals:Vec<_>=menus.session.variables.iter().filter(|(k,_)|!k.starts_with("state.")).collect();
    let key=format!("{key}:{image_state:?}:{}:{:?}:{presentation:?}:{locals:?}:{}",ctx.values.auto_speed,(ctx.images.len(),ctx.fonts.len(),ctx.save.revision),ctx.engine.0.locale.as_ref().map(|l|l.current_lang()).unwrap_or(""));
    if menus.key == key {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn_recursive();
    }
    menus.key = key;
    let page = page.unwrap();
    let root = commands
        .spawn((
            MenuRoot,
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                background_color: color(page.background).into(),
                z_index: ZIndex::Global(2000),
                ..default()
            },
        ))
        .id();
    let page_index=doc.pages.iter().position(|p|p.id==page.id).unwrap();
    for element in &menu_layout::tree(&doc,page_index,size,&assets,&ctx,menus.list_pages.get(&page.id).copied().unwrap_or(0),&[]) {
        spawn_element(
            &mut commands,
            root,
            element,
            &page.id,
            size,
            size,
            &assets,
            &ctx,
            &doc,menus.list_pages.get(&page.id).copied().unwrap_or(0),
        );
    }
}
fn spawn_element(
    commands: &mut Commands,
    parent: Entity,
    e: &Element,
    page: &str,
    reference: [f32; 2],
    size: [f32; 2],
    assets: &AssetServer,
    ctx: &Context,
    doc:&Document,list_page:usize,
) -> Option<Entity> {
    if !e.visible {
        return None;
    }
    let role=doc.pages.iter().find(|p|p.id==page).and_then(|p|p.role);
    let selected=match &e.action{
        Action::SavePage(rvn_ui::SavePage::Number(n))=>*n==list_page+1,
        Action::OpenPage(id)=>id==page,Action::Save=>role==Some(rvn_ui::PageRole::Save),Action::Load=>role==Some(rvn_ui::PageRole::Load),
        Action::Settings=>role==Some(rvn_ui::PageRole::Settings),Action::History=>role==Some(rvn_ui::PageRole::History),Action::Gallery=>role==Some(rvn_ui::PageRole::Gallery),
        _=>e.kind==Kind::CheckBox&&match e.binding.as_deref(){Some("typewriter")=>ctx.values.typewriter,Some("fullscreen")=>ctx.values.fullscreen,_=>e.local_control.as_ref().and_then(|c|c.initial.as_bool()).unwrap_or(false)},
    };
    let mut selected_visual;
    let e=if selected&&e.appearance.selected.is_some(){selected_visual=e.clone();selected_visual.normal=e.appearance.selected.unwrap();&selected_visual}else{e};
    let r = e.layout(reference, size);
    let style = Style {
        position_type: PositionType::Absolute,
        left: Val::Px(r[0]),
        top: Val::Px(r[1]),
        width: Val::Px(r[2]),
        height: Val::Px(r[3]),
        align_items: if e.kind == Kind::Button { AlignItems::Center } else { AlignItems::FlexStart },
        justify_content: JustifyContent::FlexStart,
        padding: UiRect::all(Val::Px(if e.kind==Kind::Text{0.0}else{(e.font_size*0.15).min(6.0)})),
        overflow: if matches!(e.kind,Kind::Panel|Kind::Horizontal|Kind::Vertical|Kind::Grid|Kind::Overlay){Overflow::visible()}else{Overflow::clip()},
        border: UiRect::all(Val::Px(e.appearance.border_width)),
        ..default()
    };
    let page_action_enabled=if doc.pages.iter().filter(|p|p.id==page).flat_map(|p|&p.graphs).any(|g|g.target.as_deref()==Some(e.id.as_str())&&g.event==rvn_ui::Event::Click){true}else if let Action::SavePage(target)=&e.action{matches!(target,rvn_ui::SavePage::Number(_))||doc.pages.iter().position(|p|p.id==page).and_then(|i|doc.save_page_count(i)).is_some_and(|count|target.resolve(list_page,count)!=list_page)}else{true};
    let enabled = e.enabled&&page_action_enabled
        && !(e.action == Action::Continue && ctx.persistent.data.last_resume_target.is_none());
    let entity = commands
        .spawn(NodeBundle {
            style,
            border_color: color(e.appearance.border_color).into(),
            border_radius: BorderRadius::all(Val::Px(e.appearance.radius)),
            background_color: if e.kind == Kind::Text {
                Color::NONE
            } else {
                color(if enabled { e.normal } else { e.disabled })
            }
            .into(),
            ..default()
        })
        .id();
    shadows::spawn(commands,parent,entity,e,r);
    commands.entity(entity).insert(scrollbars::ScrollAppearance::from_element(e));
    commands.entity(parent).add_child(entity);
    commands.entity(entity).insert(animations::VisualNode{page:page.into(),id:e.id.clone(),scale:(ctx.windows.single().width()/doc.reference[0]).min(ctx.windows.single().height()/doc.reference[1])});
    attach_interactions(commands,entity,e,page,doc,enabled);
    if enabled {
        if let Some((kind,slot))=e.binding.as_deref().and_then(|b|b.split_once(':')){if let Ok(slot)=slot.parse::<usize>(){let mode=match kind{"save.save"=>Some(SaveMenuMode::Save),"save.load"=>Some(SaveMenuMode::Load),"save.delete"=>Some(SaveMenuMode::Delete),"save.protect"=>Some(SaveMenuMode::ToggleProtection),_=>None};if let Some(mode)=mode{commands.entity(entity).insert((SaveSlotButton(slot),crate::systems::save_menu::SaveSlotMode(mode)));}}}
    }
    if e.asset.is_some() || !e.appearance.image_states.is_empty() {
        image_states::spawn(commands,entity,e,enabled,selected,assets,ctx);
    }
    if !matches!(
        e.kind,
        Kind::Panel | Kind::Image | Kind::SaveList | Kind::ChoiceList | Kind::Gallery | Kind::History | Kind::Horizontal | Kind::Vertical | Kind::Grid | Kind::Overlay | Kind::Scroll
    ) {
        let value = match e.binding.as_deref() {
            Some("save.page")=>Some((list_page+1).to_string()),
            Some("music_volume") => Some(format!("{:.0} %", ctx.values.music_volume * 100.0)),
            Some("sfx_volume") => Some(format!("{:.0} %", ctx.values.sfx_volume * 100.0)),
            Some("text_speed") => Some(format!("{:.1}", ctx.values.text_speed)),
            Some("auto_speed") => Some(format!("{:.1}", ctx.values.auto_speed)),
            Some("typewriter") => Some(
                if ctx.values.typewriter {
                    "Activé"
                } else {
                    "Désactivé"
                }
                .into(),
            ),
            Some("fullscreen") => Some(if ctx.values.fullscreen { "Oui" } else { "Non" }.into()),
            Some("language") => Some(ctx.values.language.clone()),
            _ => None,
        };
        let value=value.or_else(||e.local_control.as_ref().map(|c|c.initial.as_str().map(str::to_owned).unwrap_or_else(||c.initial.to_string())));
        let label = value.filter(|_|e.kind!=Kind::CheckBox)
            .map(|v| format!("{} : {}", translated(ctx,&e.text),translated(ctx,&v)))
            .unwrap_or_else(|| translated(ctx,&e.text));
        commands.entity(entity).with_children(|p| {
            let mut text=TextBundle::from_section(
                label,
                TextStyle {
                    font: e
                        .font
                        .as_ref()
                        .or(ctx.theme.text.name.font_path.as_ref())
                        .map(|f| assets.load(f.clone()))
                        .unwrap_or_else(|| ctx.font.0.clone()),
                    font_size: e.font_size * (size[0] / reference[0]).min(size[1] / reference[1]),
                    color: color(e.foreground),
                },
            ).with_text_justify(match e.layout_options.text_align{rvn_ui::TextAlign::Left=>JustifyText::Left,rvn_ui::TextAlign::Center=>JustifyText::Center,rvn_ui::TextAlign::Right=>JustifyText::Right});
            text.style.width=Val::Percent(100.0);if matches!(e.kind,Kind::CheckBox|Kind::Select){text.style.padding.right=Val::Px(34.0);}if !e.layout_options.text_wrap{text=text.with_no_wrap();}p.spawn((text,text_states::TextAppearance::new(e,selected)));
        });
    }
    if e.kind==Kind::CheckBox{
        let checked=match e.binding.as_deref(){Some("typewriter")=>ctx.values.typewriter,Some("fullscreen")=>ctx.values.fullscreen,_=>e.local_control.as_ref().and_then(|c|c.initial.as_bool()).unwrap_or(false)};let side=e.font_size.clamp(16.0,28.0);
        commands.entity(entity).with_children(|p|{p.spawn(NodeBundle{style:Style{position_type:PositionType::Absolute,right:Val::Px(8.0),top:Val::Px((r[3]-side)*0.5),width:Val::Px(side),height:Val::Px(side),border:UiRect::all(Val::Px(1.0)),align_items:AlignItems::Center,justify_content:JustifyContent::Center,..default()},border_color:color(e.foreground).into(),background_color:if checked{color(e.hover)}else{Color::NONE}.into(),..default()}).with_children(|p|{if checked{p.spawn(TextBundle::from_section("✓",TextStyle{font:ctx.font.0.clone(),font_size:side*0.85,color:color(e.foreground)}));}});});
    }else if e.kind==Kind::Select{
        commands.entity(entity).with_children(|p|{p.spawn(TextBundle{style:Style{position_type:PositionType::Absolute,right:Val::Px(10.0),top:Val::Px((r[3]-e.font_size)*0.5),..default()},text:Text::from_section("▾",TextStyle{font:ctx.font.0.clone(),font_size:e.font_size,color:color(e.foreground)}),..default()});});
    }
    if e.kind == Kind::SaveList {
        let automatic=!doc.pages.iter().position(|p|p.id==page).is_some_and(|i|doc.custom_save_pagination(i));
        let mgr = rvn_core::save::SaveManager::new(&ctx.paths.saves, crate::systems::save_menu::SUPPORTED_SLOTS).ok();
        let capacity=e.list.columns*e.list.rows;let pages=e.list.slots.div_ceil(capacity);let current=list_page.min(pages.saturating_sub(1));
        let mut card_layouts=Vec::new();
        for (index,slot) in ((current*capacity+1)..=(current*capacity+capacity).min(e.list.slots)).enumerate() {
            let saved=mgr.as_ref().and_then(|m|m.load(slot as u32).ok());let present=saved.is_some();
            let protected=mgr.as_ref().is_none_or(|m|m.is_protected(slot as u32).unwrap_or(true));
            let label = format!("{} — {}", slot, translated(ctx,if present { "Sauvegarde" } else { "Vide" }));
            let mut explicit_controls=false;
            let b = if let Some(template)=&e.list.template{
                let bounds=e.list.card_rect_with_navigation(index,[r[2],r[3]],e.layout_options.gap,automatic);let card=[bounds[2],bounds[3]];
                let root=commands.spawn(ButtonBundle{style:Style{position_type:PositionType::Absolute,left:Val::Px(bounds[0]),top:Val::Px(bounds[1]),width:Val::Px(card[0]),height:Val::Px(card[1]),overflow:Overflow::clip(),..default()},background_color:color(e.normal).into(),..default()}).id();commands.entity(entity).add_child(root);
                let date=saved.as_ref().and_then(|s|chrono::DateTime::from_timestamp(s.timestamp as i64,0)).map(|d|d.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string()).unwrap_or_else(||"Emplacement vide".into());
                let data=[("save.slot".into(),slot.to_string()),("save.thumbnail".into(),saved.as_ref().and_then(|s|s.thumbnail.clone()).unwrap_or_default()),("save.date".into(),date),("save.summary".into(),saved.as_ref().map(|s|s.label.clone()).unwrap_or_else(||format!("{} {slot}",translated(ctx,"Emplacement"))))].into_iter().collect();
                let mut data:std::collections::BTreeMap<String,String>=data;data.extend(rvn_ui::card_state_bindings(false,present,protected));
                let children=menu_layout::item(doc,template,card,&data,assets,ctx);if children.is_empty(){commands.entity(root).despawn_recursive();continue;}
                card_layouts.push((root,bounds,children[0].rect[3]));
                for mut child in children{child.inherit_rendered_parent(e);child.action=Action::None;if child.kind==Kind::Button{if let Some(binding)=child.binding.clone().filter(|b|matches!(b.as_str(),"save.save"|"save.load"|"save.delete"|"save.protect")){explicit_controls=true;child.enabled&=enabled&&match binding.as_str(){"save.save"=>!protected&&ctx.save.origin==crate::systems::save_menu::SaveMenuOrigin::InGame,"save.delete"=>present&&!protected,_=>present};if binding=="save.protect"&&protected{child.text="Déprotéger".into();}child.binding=Some(format!("{binding}:{slot}"));child.id=format!("{}/{slot}/{}",e.id,child.id);}}spawn_element(commands,root,&child,page,card,card,assets,ctx,doc,current);}
                root
            }else{list_button(
                commands,
                entity,
                &label,
                index,
                r[2],
                e,
                assets,
                &ctx.font.0,
            )};
            if enabled&&!explicit_controls&&(present || ctx.save.mode == SaveMenuMode::Save)&&!(protected&&ctx.save.mode==SaveMenuMode::Save) {
                commands
                    .entity(b)
                    .insert((SaveSlotButton(slot), MenuFocus(e.focus_order + slot as i32,format!("{page}/{}/slot/{slot}",e.id)),FocusAppearance(color(e.appearance.focus.unwrap_or(e.hover))),crate::systems::save_menu::SaveSlotColors{normal:color(e.normal),hover:color(e.hover),pressed:color(e.pressed)}));
            }
        }
        let mut fitted:Vec<_>=card_layouts.iter().map(|(_,b,_)|*b).collect();
        rvn_ui::fit_card_rows(&mut fitted,&card_layouts.iter().map(|(_,_,h)|*h).collect::<Vec<_>>(),e.layout_options.gap);
        let bottom=fitted.iter().map(|b|b[1]+b[3]).fold(0.0,f32::max);
        for ((card,_,_),bounds) in card_layouts.iter().zip(&fitted){commands.entity(*card).insert(Style{position_type:PositionType::Absolute,left:Val::Px(bounds[0]),top:Val::Px(bounds[1]),width:Val::Px(bounds[2]),height:Val::Px(bounds[3]),overflow:Overflow::clip(),..default()});}
        let footer=(r[3]-36.0).max(bottom+8.0).max(0.0);
        if !fitted.is_empty(){commands.entity(entity).insert(MenuScroll{offset:0.0,content_height:if automatic&&pages>1{footer+36.0}else{bottom}});}
        if automatic&&pages>1{for (x,label,target) in [(0.0,"Précédent",current.saturating_sub(1)),((r[2]-120.0).max(0.0),"Suivant",(current+1).min(pages-1))]{let b=list_button(commands,entity,label,0,120.0,e,assets,&ctx.font.0);commands.entity(b).insert(Style{position_type:PositionType::Absolute,left:Val::Px(x),top:Val::Px(footer),width:Val::Px(120.0),height:Val::Px(32.0),..default()});if enabled{commands.entity(b).insert((PageButton{page:target,owner:page.into()},MenuFocus(10000+x as i32,format!("{page}/{}/page/{x}",e.id)),FocusAppearance(color(e.appearance.focus.unwrap_or(e.hover)))));}}}
    }
    if e.kind == Kind::History {
        commands.entity(entity).insert((AutoList,MenuScroll{offset:0.0,content_height:0.0},Style{position_type:PositionType::Absolute,left:Val::Px(r[0]),top:Val::Px(r[1]),width:Val::Px(r[2]),height:Val::Px(r[3]),flex_direction:FlexDirection::Column,row_gap:Val::Px(e.layout_options.gap),overflow:Overflow::clip_y(),..default()}));
        for (i, line) in ctx.history.lines.iter().rev().enumerate() {
            let label = if line.character.is_empty(){line.text.clone()}else{format!("{} : {}", line.character, line.text)};
            if let Some(template)=&e.list.template{let data=doc.history_data(template,&line.character,&line.text);dynamic_rows::spawn(commands,entity,e,page,template,&data,assets,ctx,doc,None);continue;}
            let row=commands.spawn(NodeBundle{style:Style{position_type:PositionType::Relative,top:Val::Px(0.0),width:Val::Percent(100.0),flex_shrink:0.0,padding:UiRect::all(Val::Px(12.0)),..default()},background_color:color(e.normal).into(),..default()}).with_children(|p|{p.spawn(TextBundle{style:Style{max_width:Val::Percent(100.0),..default()},text:Text::from_section(label,TextStyle{font:e.font.as_ref().map(|f|assets.load(f.clone())).unwrap_or_else(||ctx.font.0.clone()),font_size:e.font_size,color:color(e.foreground)}),..default()});}).id();commands.entity(entity).add_child(row);let _=i;
        }
    }
    if e.kind == Kind::Gallery {
        let mut entries:Vec<_>=ctx.cgs.0.iter().collect();entries.sort_by_key(|(id,_)|*id);
        let mut gallery_layouts=Vec::new();
        for (i,(id,path)) in entries.iter().enumerate() {
            let unlocked=ctx.persistent.data.seen_cgs.contains(*id);
            let b=if let Some(template)=&e.list.template{let bounds=e.list.card_rect(i,[r[2],r[3]],e.layout_options.gap);let size=[bounds[2],bounds[3]];let b=commands.spawn(ButtonBundle{style:Style{position_type:PositionType::Absolute,left:Val::Px(bounds[0]),top:Val::Px(bounds[1]),width:Val::Px(bounds[2]),height:Val::Px(bounds[3]),overflow:Overflow::clip(),..default()},background_color:color(e.normal).into(),..default()}).id();commands.entity(entity).add_child(b);let data=[("gallery.thumbnail".into(),if unlocked{(*path).clone()}else{String::new()}),("gallery.title".into(),if unlocked{(*id).clone()}else{"Illustration verrouillée".into()}),("gallery.status".into(),if unlocked{"Disponible".into()}else{"À découvrir dans l’histoire".into()})].into_iter().collect();let mut data:std::collections::BTreeMap<String,String>=data;data.extend(rvn_ui::card_state_bindings(true,true,!unlocked));let children=menu_layout::item(doc,template,size,&data,assets,ctx);if children.is_empty(){commands.entity(b).despawn_recursive();continue;}
                gallery_layouts.push((b,bounds,children[0].rect[3]));
                for mut child in children{child.inherit_rendered_parent(e);child.action=Action::None;if child.kind==Kind::Button{child.enabled=false;}spawn_element(commands,b,&child,page,size,size,assets,ctx,doc,0);}b}else{list_button(commands,entity,if unlocked{id}else{"Illustration verrouillée"},i,r[2],e,assets,&ctx.font.0)};
            if enabled&&unlocked{commands.entity(b).insert((
                GalleryButton::Cg((*id).clone()),
                crate::systems::gallery::GalleryColors{normal:color(e.normal),hover:color(e.hover),pressed:color(e.pressed)},
                MenuFocus(e.focus_order + i as i32,format!("{page}/{}/gallery/{id}",e.id)),
                FocusAppearance(color(e.appearance.focus.unwrap_or(e.hover))),
            ));}
        }
        if e.list.template.is_some(){
            let mut bounds:Vec<_>=gallery_layouts.iter().map(|(_,r,_)|*r).collect();rvn_ui::fit_card_rows(&mut bounds,&gallery_layouts.iter().map(|(_,_,h)|*h).collect::<Vec<_>>(),e.layout_options.gap);
            for ((card,_,_),r) in gallery_layouts.iter().zip(&bounds){commands.entity(*card).insert(Style{position_type:PositionType::Absolute,left:Val::Px(r[0]),top:Val::Px(r[1]),width:Val::Px(r[2]),height:Val::Px(r[3]),overflow:Overflow::clip(),..default()});}
            commands.entity(entity).insert(MenuScroll{offset:0.0,content_height:bounds.iter().map(|r|r[1]+r[3]).fold(0.0,f32::max)});
        }
    }
    let count = match e.kind {
        Kind::SaveList => 0,
        Kind::Gallery if e.list.template.is_none()=>ctx.cgs.0.len(),
        _ => 0,
    };
    if count > 0 {
        commands.entity(entity).insert(MenuScroll {
            offset: 0.0,
            content_height: count as f32 * 44.0,
        });
    }
    if enabled && matches!(e.kind, Kind::CheckBox | Kind::Select) {
        let b = match e.binding.as_deref() {
            Some("typewriter") => Some(SettingsButton::TypewriterToggle),
            Some("fullscreen") => Some(SettingsButton::FullscreenToggle),
            Some("language") => None,
            _ => None,
        };
        if let Some(b) = b {
            commands.entity(entity).insert(b);
        }
        if e.kind==Kind::Select&&(e.binding.as_deref()==Some("language")||e.local_control.is_some()){commands.entity(entity).insert(dropdown::LanguageSelect{normal:color(e.normal),hover:color(e.hover),selected:color(e.appearance.selected.unwrap_or(e.hover)),foreground:color(e.foreground),text_states:e.appearance.text_states.clone(),font:e.font.as_ref().map(|f|assets.load(f.clone())).unwrap_or_else(||ctx.font.0.clone()),font_size:e.font_size,scrollbar:scrollbars::ScrollAppearance::from_element(e)});}
    }
    if e.kind == Kind::Slider {
        let slider=e.local_control.as_ref().map(|c|(c.initial.as_f64().unwrap_or(c.minimum)as f32,c.minimum as f32,c.maximum as f32)).or_else(||e.binding.as_deref().and_then(|binding|slider_range(binding).map(|(min,max)|{let value=match binding{"music_volume"=>ctx.values.music_volume,"sfx_volume"=>ctx.values.sfx_volume,"text_speed"=>ctx.values.text_speed,_=>ctx.values.auto_speed};(value,min,max)})));
        if let Some((value,min,max))=slider{
            let fraction=((value-min)/(max-min)).clamp(0.0,1.0);
            let ui_scale=(size[0]/doc.reference[0]).min(size[1]/doc.reference[1]);
            if enabled&&e.local_control.is_none(){commands.entity(entity).insert(MenuSlider(e.binding.clone().unwrap()));}
            commands.entity(entity).with_children(|p|{
                p.spawn(NodeBundle{style:Style{position_type:PositionType::Absolute,left:Val::Px(12.0),right:Val::Px(12.0),bottom:Val::Px(8.0*ui_scale),height:Val::Px(4.0*ui_scale),..default()},background_color:color(e.disabled).into(),..default()});
                p.spawn(NodeBundle{style:Style{position_type:PositionType::Absolute,left:Val::Px(12.0),bottom:Val::Px(8.0*ui_scale),width:Val::Px((r[2]-24.0).max(1.0)*fraction),height:Val::Px(4.0*ui_scale),..default()},background_color:color(e.hover).into(),..default()});
                p.spawn(NodeBundle{style:Style{position_type:PositionType::Absolute,left:Val::Px(7.0+(r[2]-24.0).max(1.0)*fraction),bottom:Val::Px(3.0*ui_scale),width:Val::Px(10.0),height:Val::Px(14.0*ui_scale),..default()},background_color:color(e.foreground).into(),..default()});
            });
        }
    }
    if e.kind==Kind::Scroll{
        fn height(es:&[Element],y:f32)->f32{es.iter().map(|e|(y+e.rect[1]+e.rect[3]).max(height(&e.children,y+e.rect[1]))).fold(0.0,f32::max)}
        commands.entity(entity).insert(MenuScroll{offset:0.0,content_height:height(&e.children,0.0)});
    }
    for child in &e.children {
        spawn_element(
            commands,
            entity,
            child,
            page,
            [r[2], r[3]],
            [r[2], r[3]],
            assets,
            ctx,
            doc,list_page,
        );
    }
    Some(entity)
}
/// Decorative elements participate only when they actually declare an event.
/// Hover-only regions deliberately stay out of keyboard/controller traversal.
fn interaction_policy(kind:Kind,events:impl Iterator<Item=rvn_ui::Event>)->(bool,bool){
    let control=matches!(kind,Kind::Button|Kind::CheckBox|Kind::Slider|Kind::Select);
    let(mut pointer,mut focus)=(control,control);
    for event in events{match event{
        rvn_ui::Event::Click|rvn_ui::Event::Focus=>{pointer=true;focus=true;},
        rvn_ui::Event::Hover|rvn_ui::Event::HoverLeave=>pointer=true,
        _=>{},
    }}
    (pointer,focus)
}
#[derive(Component)]pub(crate) struct UiInputBlocker;
fn attach_interactions(commands:&mut Commands,entity:Entity,e:&Element,page:&str,doc:&Document,enabled:bool){
    let events=doc.pages.iter().filter(|p|p.id==page).flat_map(|p|&p.graphs)
        .filter(|g|g.target.as_deref()==Some(e.id.as_str())).map(|g|g.event.clone())
        .chain((e.action!=Action::None).then_some(rvn_ui::Event::Click));
    let(pointer,focus)=interaction_policy(e.kind.clone(),events);
    if pointer{commands.entity(entity).insert(bevy::ui::FocusPolicy::Block);}
    if !enabled{if pointer{commands.entity(entity).insert((Interaction::None,UiInputBlocker));}return;}
    if let Some(control)=&e.local_control{commands.entity(entity).insert(local_controls::LocalInput{page:page.into(),id:e.id.clone(),kind:e.kind,control:control.clone()});}
    if pointer{commands.entity(entity).insert((Interaction::None,MenuElement{
        page:page.into(),id:e.id.clone(),action:e.action.clone(),
        normal:color(e.normal),hover:color(e.hover),pressed:color(e.pressed),
    }));}
    if focus{commands.entity(entity).insert((Button,MenuFocus(e.focus_order,format!("{page}/{}",e.id)),FocusAppearance(color(e.appearance.focus.unwrap_or(e.hover)))));}
}

#[cfg(test)]mod interaction_policy_tests{
    #[test]fn disabled_controls_block_pointer_without_receiving_an_action_or_focus(){
        let mut world=World::new();let entity=world.spawn_empty().id();let mut queue=bevy::ecs::world::CommandQueue::default();let doc=Document::defaults();let mut e=Element::new("disabled".into(),Kind::Button);e.enabled=false;e.action=Action::NewGame;
        {let mut commands=Commands::new(&mut queue,&world);attach_interactions(&mut commands,entity,&e,"title",&doc,false);}queue.apply(&mut world);
        assert!(world.get::<UiInputBlocker>(entity).is_some());assert!(world.get::<Interaction>(entity).is_some());assert!(matches!(world.get::<bevy::ui::FocusPolicy>(entity),Some(bevy::ui::FocusPolicy::Block)));assert!(world.get::<MenuElement>(entity).is_none());assert!(world.get::<MenuFocus>(entity).is_none());
    }
    #[test]fn hidden_or_disabled_choices_block_every_activation_path(){
        let mut menus=Menus::default();assert!(menus.choices_interactive());let mut doc=Document::defaults();let page=doc.add_choices_page();let page_id=doc.pages[page].id.clone();let list_id=doc.pages[page].elements[0].id.clone();let before=doc.clone();menus.doc=Some(doc);assert!(menus.choices_interactive());
        let key=(page_id,list_id);menus.session.presentation.insert(key.clone(),rvn_ui::ElementState{visible:Some(false),..default()});assert!(!menus.choices_interactive());
        menus.session.presentation.insert(key.clone(),rvn_ui::ElementState{enabled:Some(false),..default()});assert!(!menus.choices_interactive());
        menus.session.presentation.remove(&key);assert!(menus.choices_interactive());assert_eq!(menus.doc.as_ref().unwrap(),&before);
    }
    use super::*;
    #[test]fn decorative_regions_do_not_steal_focus(){
        assert_eq!(interaction_policy(Kind::Image,std::iter::empty()),(false,false));
        assert_eq!(interaction_policy(Kind::Panel,[rvn_ui::Event::Hover,rvn_ui::Event::HoverLeave].into_iter()),(true,false));
        assert_eq!(interaction_policy(Kind::Image,[rvn_ui::Event::Click].into_iter()),(true,true));
        assert_eq!(interaction_policy(Kind::Text,[rvn_ui::Event::Focus].into_iter()),(true,true));
        assert_eq!(interaction_policy(Kind::Button,std::iter::empty()),(true,true));
    }
    #[test]fn authored_nine_slice_matches_native_texture_slicer(){
        let image=Vec2::new(1920.0,505.0);let border=[140.0,90.0,140.0,90.0];
        let slicer=bevy::sprite::TextureSlicer{border:bevy::sprite::BorderRect{left:border[0],top:border[1],right:border[2],bottom:border[3]},..default()};
        for size in [Vec2::new(1200.0,180.0),Vec2::new(500.0,400.0),Vec2::new(2400.0,800.0)]{
            let native=slicer.compute_slices(Rect::from_corners(Vec2::ZERO,image),Some(size));
            let preview=rvn_ui::image_quads(image.to_array(),size.to_array(),rvn_ui::ImageFit::NineSlice,border);assert_eq!(native.len(),preview.len());
            for(r,uv)in preview{let s=native.iter().find(|s|(s.texture_rect.min.x/image.x-uv[0]).abs()<0.00001&&(s.texture_rect.min.y/image.y-uv[1]).abs()<0.00001).unwrap();
                let actual=[size.x*0.5+s.offset.x-s.draw_size.x*0.5,size.y*0.5-s.offset.y-s.draw_size.y*0.5,s.draw_size.x,s.draw_size.y];
                for(a,b)in actual.into_iter().zip(r){assert!((a-b).abs()<0.001,"{actual:?} != {r:?}");}
            }
        }
    }
}
fn list_button(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    index: usize,
    width: f32,
    e: &Element,
    assets: &AssetServer,
    fallback: &Handle<Font>,
) -> Entity {
    let button = commands
        .spawn(ButtonBundle {
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(index as f32 * 44.0),
                width: Val::Px(width),
                height: Val::Px(40.0),
                ..default()
            },
            background_color: color(e.normal).into(),
            ..default()
        })
        .with_children(|p| {
            p.spawn(TextBundle::from_section(
                label,
                TextStyle {
                    font: e
                        .font
                        .as_ref()
                        .map(|f| assets.load(f.clone()))
                        .unwrap_or_else(|| fallback.clone()),
                    font_size: 18.0,
                    color: color(e.foreground),
                },
            )).insert(text_states::TextAppearance::new(e,false));
        })
        .id();
    commands.entity(parent).add_child(button);
    button
}
#[derive(SystemParam)]
struct ActionContext<'w> {
    state: Res<'w, State<VnState>>,
    save: ResMut<'w, SaveMenuState>,
    settings: ResMut<'w, SettingsMenuState>,
    next: ResMut<'w, NextState<VnState>>,
    menu: ResMut<'w, MenuState>,
    engine: ResMut<'w, VnEngine>,
    history: ResMut<'w, DialogueHistory>,
    imagemap: ResMut<'w, ImagemapState>,
    render: ResMut<'w, VnRenderState>,
    typing: ResMut<'w, TypewriterState>,
    skip: ResMut<'w, SkipMode>,
    events: EventWriter<'w, crate::vn_command::VnCommand>,
    player_events: EventWriter<'w, crate::vn_command::PlayerInput>,
    confirmation:ResMut<'w,crate::systems::save_menu::SaveConfirmation>,
    paths:Res<'w,ProjectPaths>,
    persistent:ResMut<'w,PersistentDataResource>,
}
fn interact(
    mut commands: Commands,
    mut menus: ResMut<Menus>,
    mut ctx: ActionContext,
    mut query: Query<(&Interaction, &MenuElement, &mut BackgroundColor), Changed<Interaction>>,
    assets:Res<AssetServer>,values:Res<crate::systems::settings_menu::Settings>,
) {
    let mut triggers=Vec::new();
    if ctx.confirmation.active(){return;}
    for (interaction, e, mut bg) in &mut query {
        *bg = match interaction {
            Interaction::Hovered => e.hover,
            Interaction::Pressed => e.pressed,
            Interaction::None => e.normal,
        }
        .into();
        let key=(e.page.clone(),e.id.clone());
        if *interaction==Interaction::Pressed&&menus.doc.as_ref().is_some_and(|d|d.pages.iter().position(|p|p.id==e.page).and_then(|p|d.find_element(p,&e.id)).is_some_and(|e|d.resolved_element(e).local_control.is_some())){continue;}
        let event = if *interaction == Interaction::None {if !menus.hovering.remove(&key){continue}rvn_ui::Event::HoverLeave} else if *interaction == Interaction::Pressed {
            menus.automatic_navigations=0;
            rvn_ui::Event::Click
        } else {
            if !menus.hovering.insert(key){continue}
            rvn_ui::Event::Hover
        };
        triggers.push((e.clone(),Some(e.id.clone()),event));
    }
    for (page,event) in std::mem::take(&mut menus.page_events){triggers.push((MenuElement{page,id:String::new(),action:Action::None,normal:Color::NONE,hover:Color::NONE,pressed:Color::NONE},None,event));}
    for (page,target,event,action) in std::mem::take(&mut menus.activations){triggers.push((MenuElement{page,id:target.clone().unwrap_or_default(),action,normal:Color::NONE,hover:Color::NONE,pressed:Color::NONE},target,event));}
    for (e,target,event) in triggers{
        if ctx.confirmation.active(){break;}
        let Some(page)=menus.doc.as_ref().and_then(|d|d.pages.iter().find(|p|p.id==e.page)).cloned()else{continue};
        let effects=match menus.session.dispatch(&page,target.as_deref(),event.clone(),e.action.clone()){Ok(es)=>es,Err(err)=>{error!("Menus : {err}");report_preview_error(&menus.session);continue}};
        for effect in effects {
            if target.is_none()&&matches!(&effect,Effect::Action(a) if *a!=Action::None){menus.automatic_navigations+=1;if menus.automatic_navigations>16{let message="Boucle de navigation automatique interrompue (16 transitions)";error!("{message}");menus.session.action_diagnostic(&page,target.as_deref(),&event,message);report_preview_error(&menus.session);continue}}
            match effect {
                Effect::Animate(id,clip)=>menus.animation_requests.push((e.page.clone(),id,clip)),
                Effect::Sound(path)=>{commands.spawn(AudioBundle{source:assets.load(path),settings:bevy::audio::PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::new(values.sfx_volume)),..default()});},
                Effect::Action(Action::None) => {}
                Effect::Action(Action::SavePage(target))=>{
                    if let Some(index)=menus.doc.as_ref().and_then(|d|d.pages.iter().position(|p|p.id==e.page)){
                        if let Some(count)=menus.doc.as_ref().and_then(|d|d.save_page_count(index)){
                            let current=menus.list_pages.get(&e.page).copied().unwrap_or(0);menus.list_pages.insert(e.page.clone(),target.resolve(current,count));menus.key.clear();
                        }
                    }
                }
                Effect::Action(Action::OpenPage(id)) => {
                    let previous=menus.page.clone().unwrap_or_default();menus.history.push(previous);
                    menus.page = Some(id);
                    if *ctx.state.get()==VnState::Waiting{menus.overlay_return=Some(VnState::Waiting);ctx.next.set(VnState::Menu);}
                    menus.key.clear();
                }
                Effect::Action(Action::Back) if menus.page.is_some() => {
                    menus.pending = None;
                    menus.page = menus.history.pop().filter(|id|!id.is_empty());
                    if menus.page.is_none(){if let Some(ret)=menus.overlay_return.take(){ctx.next.set(ret);}}
                    menus.key.clear();
                }
                Effect::Action(mut a) => {
                    use crate::vn_command::PlayerInput;
                    let quick=match a{Action::QuickSave=>Some(PlayerInput::QuickSave),Action::QuickLoad=>Some(PlayerInput::QuickLoad),Action::Rollback=>Some(PlayerInput::Rollback),Action::ToggleMenu=>Some(PlayerInput::ToggleMenu),_=>None};
                    if let Some(input)=quick{if *ctx.state.get()==VnState::Waiting{ctx.player_events.send(input);}continue;}
                    if a==Action::ToggleSkip{if *ctx.state.get()==VnState::Waiting{ctx.skip.active=!ctx.skip.active;}continue;}
                    if *ctx.state.get()==VnState::Waiting{
                        match a{
                            Action::History=>{ctx.player_events.send(PlayerInput::OpenHistory);continue;},
                            Action::Save|Action::Load=>{ctx.save.open(if a==Action::Save{SaveMenuMode::Save}else{SaveMenuMode::Load},crate::systems::save_menu::SaveMenuOrigin::InGame);ctx.next.set(VnState::Menu);continue;},
                            Action::Settings=>{ctx.settings.active=true;ctx.next.set(VnState::Menu);continue;},
                            _=>{}
                        }
                    }
                    let approved = a == Action::Confirm;
                    if approved {
                        let Some(pending) = menus.pending.take() else {
                            continue;
                        };
                        a = pending;
                    }
                    if matches!(a,Action::NewGame|Action::StartScene(_)|Action::Continue)&&*ctx.state.get()!=VnState::TitleScreen&&ctx.menu.return_to!=Some(VnState::TitleScreen)&&!approved{
                        menus.pending=Some(a.clone());ctx.confirmation.action_pending=Some((a,e.page.clone(),ctx.state.get().clone()));
                        if *ctx.state.get()==VnState::Waiting{ctx.next.set(VnState::Menu);}continue;
                    }
                    if a==Action::Continue{
                        let result=rvn_core::save::SaveManager::new(&ctx.paths.saves,crate::systems::save_menu::SUPPORTED_SLOTS).and_then(|manager|crate::systems::title::resolve_continue_data(&manager,&ctx.persistent));
                        match result{Ok((data,target))=>{
                            ctx.engine.0.load_data(data);
                            crate::systems::save_menu::apply_loaded_game(&mut ctx.engine,&mut ctx.render,&mut ctx.imagemap,&mut ctx.typing,&mut ctx.history,&mut ctx.events);
                            if let Some(target)=target{ctx.persistent.data.last_resume_target=Some(target);if let Err(e)=ctx.persistent.manager.save(&ctx.persistent.data){error!("Reprise : {e}");}}
                            ctx.menu.return_to=None;ctx.save.active=false;ctx.settings.active=false;ctx.skip.active=false;menus.page=None;menus.history.clear();menus.key.clear();ctx.next.set(VnState::Waiting);
                        },Err(e)=>error!("Reprise impossible : {e}")}
                        continue;
                    }
                    if matches!(a, Action::NewGame | Action::StartScene(_)) {
                        let label = if let Action::StartScene(label) = a {
                            Some(label)
                        } else {
                            menus.start_label.clone()
                        };
                        let Ok(mut fresh) = ctx.engine.0.fresh(
                            crate::bevy_renderer::BevyRenderer::new(),
                            64,
                        ) else {
                            error!("Impossible de créer la nouvelle partie");
                            continue;
                        };
                        if let Some(label) = label {
                            let Some(pc) = fresh.script.iter().position(
                                |s| matches!(s,rvn_parser::Statement::Label{name} if name==&label),
                            ) else {
                                error!("Label absent : {label}");
                                continue;
                            };
                            fresh.state.pc = pc;
                        }
                        fresh.locale = ctx.engine.0.locale.take();
                        fresh.persistent_vars = ctx.engine.0.persistent_vars.clone();
                        let visible: Vec<_> = ctx
                            .engine
                            .0
                            .state
                            .sprites
                            .iter()
                            .filter(|(_, s)| s.visible)
                            .map(|(id, _)| id.clone())
                            .collect();
                        for id in visible {
                            ctx.events.send(crate::vn_command::VnCommand::HideSprite {
                                id,
                                transition: rvn_parser::Transition::None,
                            });
                        }
                        ctx.engine.0 = fresh;
                        ctx.history.clear();
                        ctx.imagemap.clear();
                        ctx.save.active=false;
                        ctx.settings.active=false;
                        *ctx.render=VnRenderState::default();
                        *ctx.typing=TypewriterState::default();
                        ctx.skip.active=false;
                        ctx.menu.return_to = None;
                        menus.page = None;
                        menus.history.clear();
                        menus.key.clear();
                        ctx.next.set(VnState::Stepping);
                        continue;
                    }
                    menus.page=None;
                    menus.history.clear();
                    menus.key.clear();
                    // These are explicit controls on the visible custom page,
                    // not clicks passing through to the hidden pause overlay.
                    if matches!(ctx.state.get(), VnState::Menu | VnState::Gallery | VnState::History) && matches!(a, Action::Save | Action::Load | Action::Settings) {
                        let from_title = ctx.menu.return_to == Some(VnState::TitleScreen);
                        if a == Action::Save && from_title { continue; }
                        ctx.next.set(VnState::Menu);
                        match a {
                            Action::Settings => { ctx.save.active = false; ctx.settings.active = true; }
                            Action::Save if from_title => continue,
                            _ => {
                                ctx.settings.active = false;
                                ctx.save.open(if a == Action::Save { SaveMenuMode::Save } else { SaveMenuMode::Load },
                                    if from_title { crate::systems::save_menu::SaveMenuOrigin::TitleScreen } else { crate::systems::save_menu::SaveMenuOrigin::InGame });
                            }
                        }
                        continue;
                    }
                    let proxy = commands
                        .spawn((
                            MenuProxy,
                            ButtonBundle {
                                style: Style {
                                    display: Display::None,
                                    ..default()
                                },
                                interaction: Interaction::Pressed,
                                ..default()
                            },
                        ))
                        .id();
                    if ctx.save.active && a == Action::Back {
                        commands.entity(proxy).insert(SaveMenuCancelButton);
                    } else if ctx.settings.active && a == Action::Back {
                        commands.entity(proxy).insert(SettingsButton::Close);
                    } else if a == Action::History {
                        let return_to = if ctx.menu.return_to == Some(VnState::TitleScreen) { VnState::TitleScreen } else { VnState::Menu };
                        ctx.save.active = false;
                        ctx.settings.active = false;
                        ctx.menu.return_to = Some(return_to);
                        ctx.next.set(VnState::History);
                    } else if a == Action::Gallery {
                        ctx.menu.return_to = Some(ctx.state.get().clone());
                        ctx.next.set(VnState::Gallery);
                    } else if *ctx.state.get() == VnState::Gallery && a == Action::Back {
                        let ret = ctx.menu.return_to.take().unwrap_or(VnState::TitleScreen);
                        ctx.next.set(ret);
                    } else if *ctx.state.get() == VnState::History && a == Action::Back {
                        ctx.next.set(ctx.menu.return_to.take().unwrap_or(VnState::Waiting));
                    } else if *ctx.state.get() == VnState::TitleScreen {
                        let b = match a {
                            Action::Continue => Some(TitleButton::Continue),
                            Action::NewGame => Some(TitleButton::NewGame),
                            Action::Load => Some(TitleButton::LoadGame),
                            Action::Settings => Some(TitleButton::Settings),
                            Action::Gallery => Some(TitleButton::Gallery),
                            Action::Quit => Some(TitleButton::Quit),
                            _ => None,
                        };
                        if let Some(b) = b {
                            commands.entity(proxy).insert((
                                b,
                                TitleButtonColors {
                                    normal: e.normal,
                                    hover: e.hover,
                                    pressed: e.pressed,
                                },
                            ));
                        } else {
                            warn!("Action indisponible dans ce menu : {a:?}");
                        }
                    } else {
                        let b = match a {
                            Action::Resume | Action::Back => Some(MenuButton::Resume),
                            Action::Save => Some(MenuButton::Save),
                            Action::Load => Some(MenuButton::Load),
                            Action::Settings => Some(MenuButton::Settings),
                            Action::Quit => Some(MenuButton::Quit),
                            _ => None,
                        };
                        if let Some(b) = b {
                            commands.entity(proxy).insert(b);
                        } else {
                            warn!("Action indisponible dans ce menu : {a:?}");
                        }
                    }
                }
                effect => {
                    menus.session.apply_presentation(&e.page,&effect);
                    menus.key.clear();
                }
            }
        }
    }
}
fn cleanup_proxies(mut commands: Commands, query: Query<Entity, With<MenuProxy>>) {
    for e in &query {
        commands.entity(e).despawn_recursive();
    }
}
