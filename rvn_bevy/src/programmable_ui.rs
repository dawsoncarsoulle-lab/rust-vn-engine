//! Reusable screens share a description with the RVN evaluator and Blueprints.
//! These overlays complement (and never replace) authored game-menu documents.
use crate::{
    resources::{ScriptErrorMessage, VnEngine, VnState},
    vn_command::VnCommand,
};
use bevy::{
    input::{
        keyboard::{Key, KeyboardInput},
        mouse::{MouseScrollUnit, MouseWheel},
        ButtonState,
    },
    prelude::*,
    ui::FocusPolicy,
    window::Ime,
};
use rvn_core::ui::UiInput;
use rvn_core::ui_input::{UiInputSource, ScopedUiInput};
use rvn_parser::Value;
use rvn_ui::programmable::{
    layout_rects, Component as UiComponent, ComponentKind as Kind, ScreenEventKind as EventKind,
    ScreenView, ScrollAxis, TextAlignment,
};
use std::collections::HashMap;

#[derive(Component,Clone)]
struct OwnerChildSource(UiInputSource);

#[path="programmable_button_down_up1030.rs"]
pub(crate) mod widget_buttons1030;
pub struct ProgrammableUiPlugin;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceScrollSet;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceInputSet;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceFocusSet;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct UiInputCaptureSet;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceReceiveSet;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InterfaceRenderSet;
impl Plugin for ProgrammableUiPlugin {
    fn build(&self, app: &mut App) {
        correct_bevy_ui_antialias(app.world_mut());
        app.init_resource::<Screens>().init_resource::<WidgetLayouts>()
            .add_systems(PreUpdate, capture_input_sources.after(bevy::input::InputSystem).in_set(UiInputCaptureSet))
            .add_systems(PreUpdate, widget_buttons1030::capture.after(bevy::input::InputSystem).after(capture_input_sources))
            .add_systems(
                PreUpdate,
                (transformed_focus
                    .after(bevy::ui::UiSystem::Focus)
                    .in_set(InterfaceFocusSet)).distributive_run_if(crate::runtime_restart::input_available),
            )
            .add_systems(
                Update,
                ((
                    receive.run_if(crate::scene_publication::projection_ready)
                        .after(crate::systems::stepping_system)
                        .before(input)
                        .in_set(InterfaceReceiveSet),
                    prepare_widget_layouts.after(receive).before(input).before(assistive_input),
                    owner_hosts_0943::install_prepared_geometry.after(prepare_widget_layouts).before(render),
                    assistive_input.run_if(crate::runtime_restart::input_available).run_if(resource_exists::<VnEngine>).run_if(crate::scene_publication::ready).before(input).after(receive),
                    input.run_if(crate::runtime_restart::input_available).run_if(resource_exists::<VnEngine>).run_if(crate::scene_publication::ready)
                        .before(crate::systems::input_system)
                        .before(crate::systems::player_input_system)
                        .in_set(InterfaceInputSet),
                    render
                        .after(receive)
                        .after(input)
                        .in_set(InterfaceRenderSet),
                    collect_scroll.run_if(crate::runtime_restart::input_available)
                        .after(input)
                        .after(crate::custom_canvas::CanvasInputSet)
                        .before(crate::systems::input_system)
                        .before(crate::systems::player_input_system),
                    image_failures.after(render),
                    feedback.after(render),
                    slider_values.after(render),
                )).distributive_run_if(crate::runtime_restart::presentation_available),
            )
            .add_systems(PostUpdate,owner_hosts_0943::parent_actual_children.before(bevy::ui::UiSystem::Layout).run_if(crate::runtime_restart::presentation_available))
            .add_systems(
                PostUpdate,
                (scroll
                    .after(bevy::ui::UiSystem::Layout)
                    .after(bevy::transform::TransformSystem::TransformPropagate)
                    .in_set(InterfaceScrollSet)).distributive_run_if(crate::runtime_restart::presentation_available),
            );
        #[cfg(target_arch = "wasm32")]
        app.add_systems(PostUpdate, (crate::web_inputs::render.after(scroll)).distributive_run_if(crate::runtime_restart::presentation_available));
    }
}

// Bevy 0.14.2's UI shader has clamp arguments reversed. WGSL/GLSL defines
// clamp(value,min,max); the old call creates min>max, undefined on WebGL2.
// ANGLE/Firefox consequently paint border colors over the entire node.
// Correct only the known embedded expression, leaving imports/definitions,
// geometry, colors and assets unchanged. Menus use this same shader too.
fn corrected_ui_antialias(source: &str) -> Result<Option<String>, String> {
    const OLD: &str = "clamp(0.0, 1.0, 0.5 - 2.0 * distance)";
    const NEW: &str = "clamp(0.5 - 2.0 * distance, 0.0, 1.0)";
    match source.matches(OLD).count(){
        1=>Ok(Some(source.replacen(OLD,NEW,1))),
        0 if source.matches(NEW).count()==1=>Ok(None),
        _=>Err("Unrecognized Bevy UI antialias shader: verify the dependency before shipping borders and rounded corners".into()),
    }
}
fn correct_bevy_ui_antialias(world: &mut World) {
    use bevy::render::render_resource::Source;
    let Some(mut shaders) = world.get_resource_mut::<Assets<Shader>>() else {
        warn!("Bevy UI shader unavailable: no rendering resources initialized");
        return;
    };
    // The UI shader handle is private in Bevy 0.14. Locate the embedded asset
    // by its source path, not a copied private UUID or an unrelated material.
    let candidates: Vec<_> = shaders
        .iter()
        .filter(|(_, shader)| shader.path.ends_with("ui.wgsl"))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(candidates.len(),1,"Expected the single known Bevy UI shader; initialize DefaultPlugins before ProgrammableUiPlugin");
    let shader = shaders.get_mut(candidates[0]).unwrap();
    let Source::Wgsl(source) = &mut shader.source else {
        panic!("Expected the known WGSL Bevy UI shader")
    };
    if let Some(corrected) = corrected_ui_antialias(source).expect("Unsupported Bevy UI shader") {
        *source = corrected.into();
    }
}

fn image_failures(
    screens: Res<Screens>,
    assets: Res<AssetServer>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
) {
    if !screens.active {
        return;
    }
    fn missing(node: &UiComponent, assets: &AssetServer) -> Option<(String, String)> {
        if !node.visible {
            return None;
        }
        if let Some(path) = &node.image {
            if let Some(handle) = assets.get_handle::<Image>(path.clone()) {
                if let Some(bevy::asset::LoadState::Failed(problem)) =
                    assets.get_load_state(handle.id())
                {
                    return Some((node.id.clone(), format!("{path}: {problem}")));
                }
            }
        }
        if let Some(path) = &node.font {
            if let Some(handle) = assets.get_handle::<Font>(path.clone()) {
                if let Some(bevy::asset::LoadState::Failed(problem)) =
                    assets.get_load_state(handle.id())
                {
                    return Some((node.id.clone(), format!("{path}: {problem}")));
                }
            }
        }
        node.children
            .iter()
            .find_map(|child| missing(child, assets))
    }
    for view in &screens.views {
        if let Some((component, problem)) = missing(&view.root, &assets) {
            error.0 = format!(
                "Screen '{}' / component '{}': interface resource could not be loaded: {}",
                view.name, component, problem
            );
            next.set(VnState::Error);
            break;
        }
    }
}

#[derive(Clone)]
struct NativeKeyProducer {
    focus: Option<(String,String)>,
    source: Option<UiInputSource>,
    sources: HashMap<String,UiInputSource>,
    key_edges: std::collections::HashSet<KeyCode>,
    component_kind: Option<Kind>,
    canvas_token: Option<rvn_core::scene_resources::ResourceToken>,
    preview_focus_order:Option<Vec<(String,String)>>,
    preview_focus_kinds:Option<HashMap<(String,String),Kind>>,
    preview_canvas_tokens:Option<HashMap<(String,String),rvn_core::scene_resources::ResourceToken>>,
}
fn queued_native_current(screens:&Screens,engine:&VnEngine,id:usize,ime:bool)->bool {
    if !screens.input_snapshot_captured && engine.0.state.scene_execution.is_none() && engine.0.state.ui.instance_registry.is_none() {return true;}
    let rows=if ime {&screens.ime_producers} else {&screens.key_producers};
    let Some(producer)=rows.get(&id) else {return false;};
    let Some(source)=&producer.source else {return false;};
    if let Some((screen,element))=&producer.focus {
        let kind=screens.views.iter().find(|view|view.name==*screen).and_then(|view|view.root.find(element)).map(|component|component.kind);
        if producer.component_kind.is_none() || producer.component_kind!=kind {return false;}
        if producer.component_kind==Some(Kind::Canvas) {
            match &producer.canvas_token {
                Some(token) => if !engine.0.state.scene_execution.as_ref().is_some_and(|execution|execution.resources.accepts(token,engine.0.runtime_epoch(),execution.owner())) {return false;},
                None => if engine.0.state.scene_execution.is_some() {return false;},
            }
        }
    }
    producer.focus==screens.input_focus
        && (producer.focus.is_some() || screens.views.last().is_some_and(|view|view.name==source.screen))
        && screens.frame_sources.get(&source.screen)==Some(source)
        && engine.0.ui_input_source_current(source)
}
#[derive(Clone)]
pub(crate) struct PreviewNativeFrame(std::sync::Arc<NativeKeyProducer>,usize);
fn preview_focus_order(views:&[ScreenView])->Vec<(String,String)>{
    let modal=views.iter().rposition(|view|view.modal).unwrap_or(0);
    views.iter().skip(modal).flat_map(|view|view.root.focus_order().into_iter().map(|id|(view.name.clone(),id))).collect()
}
pub(crate) fn borrowed_canvas_input_token<'a>(screens:&'a Screens,screen:&str,element:&str)->Option<&'a rvn_core::scene_resources::ResourceToken>{
    screens.rendered_canvas_tokens.iter().find(|((name,id),_)|name==screen&&id==element).map(|(_,token)|token)
}
pub(crate) fn preview_native_cost(world:&World)->Option<usize>{
    let Some(screens)=world.get_resource::<Screens>()else{return Some(0);};
    if screens.keyboard_focus.is_none()&&screens.rendered.is_empty(){return Some(0);}
    let mut cost=crate::editor_preview_input::PreviewCost::new(2*1024*1024);
    cost.add(std::mem::size_of::<NativeKeyProducer>())?;
    if let Some((screen,element))=&screens.keyboard_focus {cost.add(screen.len().checked_add(element.len())?)?;}
    for (screen,source) in &screens.rendered_sources {cost.add(screen.len())?;cost.add(crate::editor_preview_input::preview_source_cost(source)?)?;}
    // The selected source/token are also cloned independently of their maps.
    let name=screens.keyboard_focus.as_ref().map(|focus|focus.0.as_str()).or_else(||screens.rendered.last().map(|view|view.name.as_str()));
    if let Some(source)=name.and_then(|name|screens.rendered_sources.get(name)){cost.add(crate::editor_preview_input::preview_source_cost(source)?)?;}
    if let Some(focus)=&screens.keyboard_focus {if let Some(token)=screens.rendered_canvas_tokens.get(focus){cost.add(crate::editor_preview_input::preview_token_cost(token)?)?;}}
    fn visit(component:&UiComponent,screen:&str,available:bool,depth:usize,count:&mut usize,cost:&mut crate::editor_preview_input::PreviewCost)->Option<()> {
        *count=count.checked_add(1)?;if depth>32||*count>32768{return None;}
        let available=available&&component.visible&&component.enabled;
        if available&&component.is_focusable(){
            // Focus-order, kind-map, temporary stable-sort and screen pair.
            cost.add(screen.len().checked_add(component.id.len())?.checked_mul(3)?.checked_add(256)?)?;
        }
        for child in &component.children {visit(child,screen,available,depth+1,count,cost)?;}
        Some(())
    }
    let modal=screens.rendered.iter().rposition(|view|view.modal).unwrap_or(0);let mut count=0;
    for view in screens.rendered.iter().skip(modal){visit(&view.root,&view.name,true,0,&mut count,&mut cost)?;}
    for ((screen,element),token) in &screens.rendered_canvas_tokens {cost.add(screen.len().checked_add(element.len())?.checked_add(128)?)?;cost.add(crate::editor_preview_input::preview_token_cost(token)?)?;}
    Some(cost.bytes)
}

pub(crate) fn capture_preview_native(world:&World)->Option<PreviewNativeFrame> {
    let cost=preview_native_cost(world)?;
    let screens=world.get_resource::<Screens>()?;
    let focus=screens.keyboard_focus.clone();
    let name=focus.as_ref().map(|v|v.0.as_str()).or_else(||screens.rendered.last().map(|view|view.name.as_str()));
    let name=name?;
    let source=screens.rendered_sources.get(name).cloned();
    let kind=focus.as_ref().and_then(|(screen,element)|screens.rendered.iter().find(|view|view.name==*screen).and_then(|view|view.root.find(element))).map(|component|component.kind);
    let canvas_token=focus.as_ref().and_then(|focus|screens.rendered_canvas_tokens.get(focus)).cloned();
    let order=preview_focus_order(&screens.rendered);
    let kinds=order.iter().filter_map(|(screen,element)|screens.rendered.iter().find(|view|view.name==*screen).and_then(|view|view.root.find(element)).map(|component|((screen.clone(),element.clone()),component.kind))).collect();
    Some(PreviewNativeFrame(std::sync::Arc::new(NativeKeyProducer {focus,source,sources:screens.rendered_sources.clone(),key_edges:Default::default(),component_kind:kind,canvas_token,preview_focus_order:Some(order),preview_focus_kinds:Some(kinds),preview_canvas_tokens:Some(screens.rendered_canvas_tokens.clone())}),cost))
}
impl PreviewNativeFrame {
    pub(crate) fn cost(&self)->Option<usize>{Some(self.1)}
    pub(crate) fn current(&self,world:&World)->bool {
        let Some(engine)=world.get_resource::<VnEngine>()else{return false;};
        let Some(screens)=world.get_resource::<Screens>()else{return false;};
        let Some(source)=&self.0.source else{return false;};
        if !engine.0.ui_input_source_current(source) || self.0.focus!=screens.keyboard_focus {return false;}
        if let Some((screen,element))=&self.0.focus {
            let kind=screens.views.iter().find(|view|view.name==*screen).and_then(|view|view.root.find(element)).map(|component|component.kind);
            if kind!=self.0.component_kind{return false;}
            if kind==Some(Kind::Canvas) && !self.0.canvas_token.as_ref().is_some_and(|token|engine.0.state.scene_execution.as_ref().is_some_and(|execution|execution.resources.accepts(token,engine.0.runtime_epoch(),execution.owner()))) && engine.0.state.scene_execution.is_some() {return false;}
        }
        true
    }
    pub(crate) fn keyboard(&self,world:&mut World,event:KeyboardInput,edge:bool) {
        let mut producer=(*self.0).clone();
        if edge {producer.key_edges.insert(event.key_code);}
        let id=world.resource_mut::<Events<KeyboardInput>>().send(event).id;
        world.resource_mut::<Screens>().key_producers.insert(id,std::sync::Arc::new(producer));
    }
    pub(crate) fn ime(&self,world:&mut World,event:Ime) {
        let id=world.resource_mut::<Events<Ime>>().send(event).id;
        world.resource_mut::<Screens>().ime_producers.insert(id,self.0.clone());
    }
}
pub(crate) fn queued_native_key_current(screens:&Screens,engine:&VnEngine,id:usize)->bool {
    queued_native_current(screens,engine,id,false)
}

#[derive(Resource, Default)]
pub(crate) struct Screens {
    pub(super) views: Vec<ScreenView>,
    rendered: Vec<ScreenView>,
    rendered_projections: Vec<Option<crate::composed_motion::InterfaceProjectionTarget>>,
    size: Vec2,
    layout_text_scale: f32,
    widget_layout_revision: u64,
    widget_plans:HashMap<String,WidgetPlan>,
    pub(super) active: bool,
    // Owner modality survives an absent authored child tree. This is not a
    // fabricated Node and does not represent owner keyboard focus/geometry.
    resident_modal: bool,
    dropdown: Option<Dropdown>,
    rendered_dropdown: Option<Dropdown>,
    editing: Editing,
    rendered_editing: Editing,
    dragging: Option<Dragging>,
    pub(super) keyboard_focus: Option<(String, String)>,
    scroll_positions: HashMap<(String, String), Vec2>,
    reveal_focus: Option<(String, String)>,
    pending_scroll: Vec2,
    modifiers: [bool; 4],
    pub pointer_consumed: bool,
    pub keyboard_consumed: bool,
    rendered_sources: HashMap<String, UiInputSource>,
    rendered_canvas_tokens: HashMap<(String,String),rvn_core::scene_resources::ResourceToken>,
    frame_sources: HashMap<String, UiInputSource>,
    input_snapshot_captured: bool,
    input_focus: Option<(String, String)>,
    key_producer_reader: bevy::ecs::event::ManualEventReader<KeyboardInput>,
    ime_producer_reader: bevy::ecs::event::ManualEventReader<Ime>,
    key_producers: HashMap<usize,std::sync::Arc<NativeKeyProducer>>,
    ime_producers: HashMap<usize,std::sync::Arc<NativeKeyProducer>>,
    dispatch_source: Option<UiInputSource>,
    editing_source: Option<UiInputSource>,
    dropdown_source: Option<UiInputSource>,
    dragging_source: Option<UiInputSource>,
    dragging_preview: Option<crate::editor_preview_input::PointerOverride>,
    pending_press: Option<PendingPress>,
    widget_buttons1030:widget_buttons1030::State,
}
/// Cancel transient captures, retaining the old authored view and focus.
pub(crate) fn cancel_runtime_input(world:&mut World){
    if let Some(mut screens)=world.get_resource_mut::<Screens>(){
        screens.pending_press=None;screens.dragging=None;screens.dragging_source=None;screens.dragging_preview=None;
        screens.widget_buttons1030.cancel();
        screens.pending_scroll=Vec2::ZERO;screens.dispatch_source=None;
    }
    cancel_preview_keyboard(world);
}
pub(crate) fn cancel_preview_pointer(world:&mut World){
    if let Some(mut screens)=world.get_resource_mut::<Screens>() {
        screens.widget_buttons1030.cancel();
        if screens.pending_press.as_ref().is_some_and(|press|press.image.is_some()){screens.pending_press=None;}
        if screens.dragging_preview.take().is_some(){screens.dragging=None;screens.dragging_source=None;}
    }
}
pub(crate) fn cancel_preview_keyboard(world:&mut World){
    if let Some(mut screens)=world.get_resource_mut::<Screens>() {
        let canceled=std::sync::Arc::new(NativeKeyProducer{focus:None,source:None,sources:HashMap::new(),key_edges:Default::default(),component_kind:None,canvas_token:None,preview_focus_order:None,preview_focus_kinds:None,preview_canvas_tokens:None});
        for producer in screens.key_producers.values_mut(){*producer=canceled.clone();}
        for producer in screens.ime_producers.values_mut(){*producer=canceled.clone();}
    }
}
pub(crate) fn frame_input_captured(screens: &Screens) -> bool { screens.input_snapshot_captured }
pub(crate) fn frame_input_source(screens: &Screens,screen:&str) -> Option<UiInputSource> { screens.frame_sources.get(screen).cloned() }

pub(crate) fn frame_input_target(screens: &Screens) -> Option<(String,String,UiInputSource)> {
    let (screen,element)=screens.input_focus.as_ref()?;
    Some((screen.clone(),element.clone(),screens.frame_sources.get(screen)?.clone()))
}

#[derive(Component, Clone)]
pub(crate) struct ControlSource(pub(crate) UiInputSource);

// Capture displayed identities before Update stepping/publication/callbacks.
fn capture_input_sources(mut screens: ResMut<Screens>, keyboard: Option<Res<Events<KeyboardInput>>>, ime: Option<Res<Events<Ime>>>, keys: Option<Res<ButtonInput<KeyCode>>>, input:crate::runtime_restart::InputAccess) {
    // No rendered producer means no external authority, including legacy.
    screens.frame_sources = screens.rendered_sources.clone();
    screens.input_focus = screens.keyboard_focus.clone();
    screens.input_snapshot_captured = true;
    screens.dispatch_source = None;
    if !input.available(){
        if let Some(keys)=keys.as_ref(){
            screens.modifiers=[keys.pressed(KeyCode::ControlLeft),keys.pressed(KeyCode::ControlRight),keys.pressed(KeyCode::ShiftLeft),keys.pressed(KeyCode::ShiftRight)];
        }
        screens.frame_sources.clear();screens.input_focus=None;
        let canceled=std::sync::Arc::new(NativeKeyProducer{focus:None,source:None,sources:HashMap::new(),key_edges:Default::default(),component_kind:None,canvas_token:None,preview_focus_order:None,preview_focus_kinds:None,preview_canvas_tokens:None});
        for producer in screens.key_producers.values_mut(){*producer=canceled.clone();}
        for producer in screens.ime_producers.values_mut(){*producer=canceled.clone();}
    }
    let name=screens.input_focus.as_ref().map(|focus|focus.0.as_str()).or_else(||screens.views.last().map(|view|view.name.as_str()));
    let displayed=if screens.rendered.is_empty(){&screens.views}else{&screens.rendered};
    let component_kind=screens.input_focus.as_ref().and_then(|(screen,element)|displayed.iter().find(|view|view.name==*screen).and_then(|view|view.root.find(element))).map(|component|component.kind);
    let canvas_token=screens.input_focus.as_ref().and_then(|focus|screens.rendered_canvas_tokens.get(focus)).cloned();
    let producer=std::sync::Arc::new(NativeKeyProducer {focus:screens.input_focus.clone(),source:name.and_then(|name|screens.frame_sources.get(name)).cloned(),sources:screens.frame_sources.clone(),key_edges:keys.as_ref().map(|keys|keys.get_just_pressed().copied().collect()).unwrap_or_default(),component_kind,canvas_token,preview_focus_order:None,preview_focus_kinds:None,preview_canvas_tokens:None});
    if let Some(events)=keyboard {
        screens.key_producers.retain(|id,_|*id>=events.oldest_event_count());
        let mut reader=std::mem::take(&mut screens.key_producer_reader);
        for (_,id) in reader.read_with_id(&events) {
            let actual=if input.accepts::<KeyboardInput>(id.id){producer.clone()}else{std::sync::Arc::new(NativeKeyProducer{focus:None,source:None,sources:HashMap::new(),key_edges:Default::default(),component_kind:None,canvas_token:None,preview_focus_order:None,preview_focus_kinds:None,preview_canvas_tokens:None})};
            screens.key_producers.entry(id.id).or_insert(actual);
        }
        screens.key_producer_reader=reader;
    }
    if let Some(events)=ime {
        screens.ime_producers.retain(|id,_|*id>=events.oldest_event_count());
        let mut reader=std::mem::take(&mut screens.ime_producer_reader);
        for (_,id) in reader.read_with_id(&events) {
            let actual=if input.accepts::<Ime>(id.id){producer.clone()}else{std::sync::Arc::new(NativeKeyProducer{focus:None,source:None,sources:HashMap::new(),key_edges:Default::default(),component_kind:None,canvas_token:None,preview_focus_order:None,preview_focus_kinds:None,preview_canvas_tokens:None})};
            screens.ime_producers.entry(id.id).or_insert(actual);
        }
        screens.ime_producer_reader=reader;
    }
}
#[cfg(test)]
pub(crate) fn capture_frame_for_test(world: &mut World,sources: HashMap<String,UiInputSource>) {
    use bevy::ecs::system::RunSystemOnce;
    world.resource_mut::<Screens>().rendered_sources=sources;
    world.run_system_once(capture_input_sources);
}
pub(crate) fn rendered_canvas_input_token(screens:&Screens,screen:&str,element:&str)->Option<rvn_core::scene_resources::ResourceToken> {
    screens.rendered_canvas_tokens.get(&(screen.to_owned(),element.to_owned())).cloned()
}
fn canvas_input_tokens(engine:&rvn_core::Engine<crate::bevy_renderer::BevyRenderer>,views:&[ScreenView],sources:&HashMap<String,UiInputSource>)->HashMap<(String,String),rvn_core::scene_resources::ResourceToken> {
    let mut tokens=HashMap::new();
    for view in views {
        let Some(source)=sources.get(&view.name).filter(|source|source.order==view.order && engine.ui_input_source_current(source))else{continue;};
        view.root.visit(&mut |component|if component.kind==Kind::Canvas {
            if let Some(token)=engine.scene_resource_token(rvn_core::scene_resources::ResourceTaskKind::Canvas,&format!("{}/{}",source.screen,component.id)) {tokens.insert((source.screen.clone(),component.id.clone()),token);}
        });
    }
    tokens
}
#[cfg(test)]
pub(crate) fn publish_canvas_input_tokens_for_test(world:&mut World) {
    let screens=world.resource::<Screens>();
    let tokens=canvas_input_tokens(&world.resource::<VnEngine>().0,&screens.views,&screens.rendered_sources);
    let rendered=screens.views.clone();
    let mut screens=world.resource_mut::<Screens>();screens.rendered_canvas_tokens=tokens;screens.rendered=rendered;
}

fn legacy_input_sources(engine: &rvn_core::Engine<crate::bevy_renderer::BevyRenderer>, views: &[ScreenView]) -> HashMap<String, UiInputSource> {
    views.iter().filter_map(|view| engine.capture_ui_input_source(&view.name)
        .filter(|source| source.order == view.order)
        .map(|source| (view.name.clone(), source))).collect()
}
fn control_source(engine: &VnEngine, screens: &Screens, control: &Control, captured: Option<&ControlSource>) -> Option<UiInputSource> {
    let source = if let Some(captured) = captured { Some(captured.0.clone()) }
        else if engine.0.state.scene_execution.is_none() { screens.frame_sources.get(&control.screen).cloned() }
        else { None }?;
    if screens.input_snapshot_captured && screens.frame_sources.get(&control.screen) != Some(&source) { return None; }
    (source.screen == control.screen && engine.0.ui_input_source_current(&source)).then_some(source)
}

impl Screens {
    pub fn modal(&self) -> bool {
        self.resident_modal || self.views.iter().any(|view| view.modal)
    }
}
/// Actual scene publication supplies the preflighted owner snapshot from its
/// same batch. A legacy direct helper does not manufacture this source witness.
pub(crate) fn install_scene_with_owners(world:&mut World,views:Vec<ScreenView>,owners:owner_hosts_0943::PreparedOwnerProjection){
 install_scene(world,views);world.insert_resource(owner_hosts_0943::OwnerSceneProjection::new(owners));
}
pub(crate) fn install_scene(world: &mut World, views: Vec<ScreenView>) {
    let mut screens = world.remove_resource::<Screens>().unwrap_or_default();
    let epoch=crate::text_runtime_owner::current(world).expect("Preflighted actual scene owner").epoch;
    let same_runtime=world.get_resource::<crate::scene_publication::ScenePresentation>()
        .and_then(|presentation|presentation.batch.as_ref()).is_some_and(|old|old.runtime_epoch==epoch);
    // Screen order identifies the live instance. Unrelated locals/timer
    // publications preserve its focus, scroll, selection and pointer capture.
    // A recreated name cannot inherit those transient capabilities.
    let retained = |name: &str| same_runtime && screens.views.iter().find(|view|view.name == name)
        .is_some_and(|old|views.iter().any(|new|new.name == old.name && new.order == old.order));
    let keep_editing = retained(&screens.editing.screen);
    let keep_drag = screens.dragging.as_ref().is_none_or(|drag|retained(&drag.screen));
    let keep_dropdown = screens.dropdown.as_ref().is_none_or(|menu|retained(&menu.screen));
    let keep_focus = screens.keyboard_focus.as_ref().is_none_or(|(name,_)|retained(name));
    let retained_names: std::collections::HashSet<_> = screens.views.iter().filter(|view|retained(&view.name)).map(|view|view.name.clone()).collect();
    // Input runs before the redraw rebuild. An entity from a replaced same-
    // named instance must not grant authority to its successor for that frame.
    let retired:Vec<_>=world.query::<(Entity,&Control)>().iter(world)
        .filter(|(_,control)|!retained_names.contains(&control.screen)).map(|(entity,_)|entity).collect();
    let force_rebuild=!same_runtime || !retired.is_empty();
    for entity in retired {if let Some(entity)=world.get_entity_mut(entity) {entity.despawn_recursive();}}
    if !keep_editing { screens.editing = default(); screens.editing_source = None; }
    if !keep_drag { screens.dragging = None; screens.dragging_source = None; screens.dragging_preview=None; }
    if !keep_dropdown { screens.dropdown = None; screens.dropdown_source = None; }
    if !keep_focus { screens.keyboard_focus = None; screens.reveal_focus = None; }
    screens.scroll_positions.retain(|(name,_),_|retained_names.contains(name));
    if force_rebuild {screens.rendered.clear();}
    update_views(&mut screens, views);
    world.insert_resource(screens);
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Editing {
    screen: String,
    element: String,
    caret: usize,
    anchor: Option<usize>,
    composing: bool,
}
#[derive(Clone, Debug, PartialEq)]
struct Dropdown {
    screen: String,
    element: String,
    options: Vec<String>,
    labels: Vec<String>,
    highlighted: usize,
    position: Vec2,
    width: f32,
}
#[derive(Clone)]
struct Dragging {
    screen: String,
    element: String,
}
#[derive(Component)]
struct ScreenRoot;
#[derive(Component,Clone)]
struct WidgetRootSource(UiInputSource);
#[derive(Component)]
struct ScrollPort {
    screen: String,
    element: String,
    axis: ScrollAxis,
    depth: usize,
}
#[derive(Component)]
struct ScrollContent {
    port: Entity,
}
#[derive(Component)]
struct ScrollThumb {
    port: Entity,
    horizontal: bool,
}
// One accepted Focus may require a complete publication before the rest of
// the same physical press. No fresh address or hit test is made on continuation.
struct PendingPress {
    entity:Entity,control:Control,source:UiInputSource,component:[u8;32],
    image:Option<(u64,u64,u64)>,
    pointer:Option<crate::editor_preview_input::PointerOverride>,
    action:UiInput,
}
pub(crate) fn preview_component_digest(world:&World,control:&Control)->Option<[u8;32]>{
    let screens=world.get_resource::<Screens>()?;
    let component=screens.rendered.iter().find(|view|view.name==control.screen)?.root.find(&control.element)?;
    pointer_component_digest(component,false)
}
pub(crate) fn pointer_component_digest(component:&UiComponent,include_value:bool)->Option<[u8;32]>{
    use sha2::{Digest,Sha256};
    struct Writer {digest:Sha256,bytes:usize}
    impl std::io::Write for Writer {
        fn write(&mut self,data:&[u8])->std::io::Result<usize>{
            let total=self.bytes.checked_add(data.len()).filter(|bytes|*bytes<=2*1024*1024)
                .ok_or_else(||std::io::Error::new(std::io::ErrorKind::InvalidData,"Physical component proof budget"))?;
            self.bytes=total;self.digest.update(data);Ok(data.len())
        }
        fn flush(&mut self)->std::io::Result<()>{Ok(())}
    }
    // Render geometry belongs to the frozen image; these are the handler and
    // value-domain fields. An owned drag may change its bound value, while a
    // pending press must also retain the value observed before its Focus.
    let semantic=(&component.id,component.kind,&component.binding,&component.events,&component.event_data,
        (&component.props,&component.state),component.min,component.max,&component.options,
        (component.visible,component.enabled,component.focus_order,component.capture_pointer,component.consume_input),
        include_value.then_some(&component.value));
    let mut writer=Writer{digest:Sha256::new(),bytes:0};serde_json::to_writer(&mut writer,&semantic).ok()?;
    Some(writer.digest.finalize().into())
}
#[derive(Component)]
struct SliderLabel(Entity);
#[derive(Component)]
struct SliderFill(Entity);
fn slider_values(screens:Res<Screens>,controls:Query<&Control>,mut labels:Query<(&SliderLabel,&mut Text)>,mut fills:Query<(&SliderFill,&mut Style)>){
    let component=|entity|controls.get(entity).ok().and_then(|control|screens.views.iter().find(|view|view.name==control.screen)?.root.find(&control.element)).filter(|component|component.kind==Kind::Slider);
    for (label,mut text) in &mut labels {if let Some(component)=component(label.0){if let Some(section)=text.sections.first_mut(){section.value=slider_label(component);}}}
    for (fill,mut style) in &mut fills {if let Some(component)=component(fill.0){style.width=Val::Percent(((component.value.as_f64().unwrap_or(component.min)-component.min)/(component.max-component.min)).clamp(0.0,1.0) as f32*100.0);}}
}
fn slider_label(component:&UiComponent)->String {format!("{}  {:.0} %",component.text,(component.value.as_f64().unwrap_or(component.min)-component.min)/(component.max-component.min)*100.0)}
#[derive(Component, Clone)]
pub(crate) struct Control {
    pub screen: String,
    pub element: String,
    pub(super) option: Option<String>,
}

fn local_pointer(transform: &GlobalTransform, cursor: Vec2) -> Option<Vec2> {
    let matrix = transform.compute_matrix();
    if !matrix.is_finite() || matrix.determinant().abs() < 1e-8 {
        return None;
    }
    let local = matrix
        .inverse()
        .transform_point3(cursor.extend(transform.translation().z))
        .truncate();
    local.is_finite().then_some(local)
}
fn transformed_contains(size: Vec2, transform: &GlobalTransform, cursor: Vec2) -> bool {
    size.min_element() > 0.0
        && local_pointer(transform, cursor).is_some_and(|point| point.abs().cmple(size * 0.5).all())
}
fn slider_fraction(size: Vec2, transform: &GlobalTransform, cursor: Vec2) -> Option<f32> {
    local_pointer(transform, cursor).map(|point| (point.x / size.x.max(1.0) + 0.5).clamp(0.0, 1.0))
}
/// Bevy 0.14's default UI hit-test ignores scale and rotation. Pick in local
/// coordinates instead, using the actual stack and blocking policies, so
/// animated controls stay clickable and disabled/modal controls absorb hits.
fn transformed_focus(
    screens: Res<Screens>,
    preview: Option<Res<crate::editor_preview_input::PointerOverride>>,
    stack: Res<bevy::ui::UiStack>,
    windows: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut nodes: Query<(
        &Node,
        &GlobalTransform,
        &ViewVisibility,
        Option<&bevy::ui::CalculatedClip>,
        Option<&FocusPolicy>,
        Option<&Control>,
        Option<&mut Interaction>,
    )>,
) {
    if !screens.active {
        return;
    }
    let cursor = windows
        .get_single()
        .ok()
        .and_then(|window| window.cursor_position());
    let mut picked = None;
    if let Some(cursor) = cursor {
        for entity in stack.uinodes.iter().rev() {
            let Ok((node, transform, visible, clip, policy, control, _)) = nodes.get(*entity)
            else {
                continue;
            };
            if !visible.get()
                || clip.is_some_and(|clip| !clip.clip.contains(cursor))
                || !transformed_contains(node.size(), transform, cursor)
            {
                continue;
            }
            if control.is_some() {
                picked = Some(*entity);
                break;
            }
            if policy == Some(&FocusPolicy::Block) {
                break;
            }
        }
    }
    if let Some(preview)=preview {picked=preview.control;}
    for entity in &stack.uinodes {
        if let Ok((_, _, _, _, _, Some(_), Some(mut interaction))) = nodes.get_mut(*entity) {
            // Winit may deliver down and up between two frames. Bevy retains
            // both edges even though the final held state is false; preserve
            // that press for this frame so input can dispatch it exactly once.
            let next = if picked != Some(*entity) {
                Interaction::None
            } else if mouse.pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Left) {
                Interaction::Pressed
            } else {
                Interaction::Hovered
            };
            if *interaction != next {
                *interaction = next;
            }
        }
    }
}

fn faded(value: [f32; 4], opacity: f32) -> Color {
    Color::srgba(value[0], value[1], value[2], value[3] * opacity)
}

#[derive(Clone,Debug,PartialEq)]
struct WidgetMetrics{bounds:[f32;4],physical_scale:f32,ui_scale:f32,camera:Entity,target:bevy::render::camera::NormalizedRenderTarget,target_size:UVec2,viewport:Option<(UVec2,UVec2)>,settings:rvn_ui::accessibility::AccessibilitySettings}
#[derive(Clone,Debug,PartialEq)]
struct WidgetPlan {
    definition:[u8;32],rects:Vec<rvn_ui::programmable::ComponentRect>,
    source:UiInputSource,interactive:bool,projection:Option<crate::composed_motion::InterfaceProjectionTarget>,metrics:WidgetMetrics,fonts:HashMap<bevy::asset::AssetId<Font>,[u8;32]>,
}
fn font_fingerprint(font:&Font)->[u8;32]{use ab_glyph::Font as _;use sha2::Digest;sha2::Sha256::digest(font.font.font_data()).into()}
fn widget_plan_fonts_current(plan:&WidgetPlan,fonts:&Assets<Font>)->bool{plan.fonts.iter().all(|(id,digest)|fonts.get(*id).is_some_and(|font|font_fingerprint(font)==*digest))}
fn plan_for<'a>(plans:&'a HashMap<String,WidgetPlan>,view:&ScreenView,source:Option<&UiInputSource>)->Option<&'a WidgetPlan>{
    let source=source?;let wire=source.to_wire().ok()?;let plan=plans.get(&wire)?;
    (plan.source==*source&&plan.source.order==view.order&&widget_definition_digest(view).ok()==Some(plan.definition)).then_some(plan)
}
/// Geometry readiness is a renderer fact, not permission to publish a Core
/// frame or dispatch callbacks. Root/F must fence that publication separately.
#[derive(Resource,Default)]
pub(crate) struct WidgetLayouts {
    owner_geometry:Option<owner_hosts_0943::PreparedOwnerGeometry>,
    plans:HashMap<String,WidgetPlan>,problem:Option<WidgetLayoutProblem>,revision:u64,
    default_font:Option<Handle<Font>>,
    metrics:Option<WidgetMetrics>,
    pending_names:std::collections::HashSet<String>,
    // Receipt of the FontId binding already admitted to Bevy, retained across
    // Pending/removal because its glyph/atlas cache survives Widget entities.
    font_bindings:HashMap<bevy::asset::AssetId<Font>,[u8;32]>,
}
pub(crate) fn widget_layout_problem(world:&World)->Option<&str>{world.get_resource::<WidgetLayouts>().and_then(|layout|layout.problem.as_ref().map(WidgetLayoutProblem::message))}
/// Borrow the one real installed owner for layout/presentation. This grants
/// neither VM input dispatch nor an alternate Engine during pending Init.
enum WidgetPresentationOwner<'a>{Game(&'a VnEngine),Startup(&'a crate::debugger_startup::DebugBootstrap)}
impl WidgetPresentationOwner<'_>{
    fn state(&self)->&rvn_core::GameState{match self{Self::Game(engine)=>&engine.0.state,Self::Startup(startup)=>startup.owner.state()}}
    fn input_source(&self,screen:&str)->Option<UiInputSource>{match self{Self::Game(engine)=>engine.0.capture_ui_input_source(screen),Self::Startup(startup)=>startup.owner.capture_ui_input_source(screen)}}
}
fn widget_presentation_owner<'a>(engine:Option<&'a VnEngine>,startup:Option<&'a crate::debugger_startup::DebugBootstrap>)->Option<WidgetPresentationOwner<'a>>{
    match(engine,startup){
        (Some(engine),None)if !engine.0.renderer.scene_batch_pending()&&!engine.0.renderer.scene_frame_failed=>Some(WidgetPresentationOwner::Game(engine)),
        (None,Some(startup))if !startup.publication_pending&&startup.failed.is_none()&&startup.accepts_actual_audio_broker()=>Some(WidgetPresentationOwner::Startup(startup)),
        _=>None,
    }
}
pub(crate) fn widget_geometry_ready(world:&World)->bool{
    let Some(screens)=world.get_resource::<Screens>()else{return true;};
    if screens.views.iter().all(|view|view.root.layout_contract.is_none()){return true;}
    let(Some(layout),Some(fonts),Some(owner))=(world.get_resource::<WidgetLayouts>(),world.get_resource::<Assets<Font>>(),widget_presentation_owner(world.get_resource::<VnEngine>(),world.get_resource::<crate::debugger_startup::DebugBootstrap>()))else{return false;};
    layout.problem.is_none()&&layout.metrics.as_ref().is_some_and(|metric|current_widget_camera_metrics(world,metric).as_ref()==Some(metric))&&screens.views.iter().enumerate().filter(|(_,view)|view.root.layout_contract.is_some()).all(|(index,view)|{
        let source=owner.input_source(&view.name);plan_for(&layout.plans,view,source.as_ref()).is_some_and(|plan|widget_plan_fonts_current(plan,fonts)&&plan.interactive==screens.views.iter().rposition(|view|view.modal).is_none_or(|modal|index>=modal)&&plan.projection==screen_projection(owner.state(),view))
    })
}
/// Cancel only producer captures owned by the actual changed Widget sources.
/// A shared definition/local element label never identifies a runtime owner.
fn revoke_widget_sources(screens:&mut Screens,sources:&[UiInputSource]){
    let affected=|source:&UiInputSource|sources.contains(source);
    let focus_affected=|focus:&Option<(String,String)>|focus.as_ref().is_some_and(|(name,_)|screens.rendered_sources.get(name).or_else(||screens.frame_sources.get(name)).is_some_and(affected));
    let keyboard=focus_affected(&screens.keyboard_focus);let input=focus_affected(&screens.input_focus);let reveal=focus_affected(&screens.reveal_focus);
    if screens.pending_press.as_ref().is_some_and(|press|affected(&press.source)){screens.pending_press=None;}
    screens.widget_buttons1030.revoke(sources);
    if screens.dragging_source.as_ref().is_some_and(affected){screens.dragging=None;screens.dragging_source=None;screens.dragging_preview=None;}
    if screens.editing_source.as_ref().is_some_and(affected){screens.editing=default();screens.editing_source=None;}
    if screens.dropdown_source.as_ref().is_some_and(affected){screens.dropdown=None;screens.dropdown_source=None;}
    if screens.dispatch_source.as_ref().is_some_and(affected){screens.dispatch_source=None;}
    if keyboard{screens.keyboard_focus=None;}if input{screens.input_focus=None;}if reveal{screens.reveal_focus=None;}
    for producers in [&mut screens.key_producers,&mut screens.ime_producers]{for producer in producers.values_mut(){
        let mut next=(**producer).clone();next.sources.retain(|_,source|!affected(source));
        if next.source.as_ref().is_some_and(affected){next.focus=None;next.source=None;next.component_kind=None;next.canvas_token=None;next.key_edges.clear();}
        if let Some(order)=&mut next.preview_focus_order{order.retain(|(name,_)|next.sources.contains_key(name));}
        if let Some(kinds)=&mut next.preview_focus_kinds{kinds.retain(|(name,_),_|next.sources.contains_key(name));}
        if let Some(tokens)=&mut next.preview_canvas_tokens{tokens.retain(|(name,_),_|next.sources.contains_key(name));}
        *producer=std::sync::Arc::new(next);
    }}
    screens.frame_sources.retain(|_,source|!affected(source));screens.rendered_sources.retain(|_,source|!affected(source));
    screens.rendered_canvas_tokens.retain(|(name,_),_|screens.rendered_sources.contains_key(name));
}
fn widget_definition_digest(view:&ScreenView)->Result<[u8;32],WidgetLayoutProblem>{
    use sha2::Digest;
    struct Sink(sha2::Sha256);
    impl std::io::Write for Sink{fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{self.0.update(bytes);Ok(bytes.len())}fn flush(&mut self)->std::io::Result<()>{Ok(())}}
    let mut sink=Sink(sha2::Sha256::new());serde_json::to_writer(&mut sink,&(&view.root,view.order,view.modal,view.layer)).map_err(|error|WidgetLayoutProblem::Invalid(error.to_string()))?;Ok(sink.0.finalize().into())
}
#[derive(bevy::ecs::system::SystemParam)]
struct WidgetCameraInputs<'w,'s>{
    windows:Query<'w,'s,(Entity,&'static Window,Option<&'static bevy::window::PrimaryWindow>)>,
    images:Option<Res<'w,Assets<Image>>>,textures:Option<Res<'w,bevy::render::camera::ManualTextureViews>>,
}
fn actual_widget_camera_metrics(entity:Entity,camera:&Camera,ui:f32,settings:&rvn_ui::accessibility::AccessibilitySettings,target:bevy::render::camera::NormalizedRenderTarget,info:bevy::render::camera::RenderTargetInfo)->Result<WidgetMetrics,WidgetLayoutProblem>{
    settings.validate().map_err(WidgetLayoutProblem::Invalid)?;
    let viewport=camera.physical_viewport_size().ok_or_else(||WidgetLayoutProblem::Pending("Widget layout waits for the computed camera viewport".into()))?;
    let dpi=camera.target_scaling_factor().ok_or_else(||WidgetLayoutProblem::Pending("Widget layout waits for the computed camera DPI".into()))?;
    if camera.physical_target_size()!=Some(info.physical_size)||dpi!=info.scale_factor{return Err(WidgetLayoutProblem::Pending("Widget layout waits for camera geometry matching its actual render target".into()));}
    let scale=dpi*ui;if !scale.is_finite()||scale<=0.0{return Err(WidgetLayoutProblem::Invalid("Invalid Widget camera DPI/UiScale".into()));}
    if viewport.x==0||viewport.y==0{return Err(WidgetLayoutProblem::Pending("Widget layout waits for a nonzero actual viewport".into()));}
    Ok(WidgetMetrics{bounds:[0.0,0.0,viewport.x as f32/scale,viewport.y as f32/scale],physical_scale:scale,ui_scale:ui,camera:entity,target,target_size:info.physical_size,viewport:camera.viewport.as_ref().map(|viewport|(viewport.physical_position,viewport.physical_size)),settings:settings.clone()})
}
fn current_widget_camera_metrics(world:&World,metric:&WidgetMetrics)->Option<WidgetMetrics>{
    let camera=world.get::<Camera>(metric.camera)?;let ui=world.get_resource::<bevy::ui::UiScale>()?.0;let settings=&world.get_resource::<crate::accessibility::Accessibility>()?.settings;
    let windows=world.iter_entities().filter_map(|entity|entity.get::<Window>().map(|window|(entity.id(),window))).collect::<Vec<_>>();let primary=world.iter_entities().find(|entity|entity.contains::<bevy::window::PrimaryWindow>()).map(|entity|entity.id());let target=camera.target.normalize(primary)?;
    let info=target.get_render_target_info(windows.iter().map(|(entity,window)|(*entity,*window)),world.get_resource::<Assets<Image>>()?,world.get_resource::<bevy::render::camera::ManualTextureViews>()?)?;
    actual_widget_camera_metrics(metric.camera,camera,ui,settings,target,info).ok()
}
fn prepare_widget_layouts(
    mut commands:Commands,mut screens:ResMut<Screens>,mut prepared:ResMut<WidgetLayouts>,assets:Res<AssetServer>,
    mut fonts:Option<ResMut<Assets<Font>>>,accessibility:Res<crate::accessibility::Accessibility>,camera:bevy::ui::DefaultUiCamera,cameras:Query<&Camera>,ui_scale:Option<Res<bevy::ui::UiScale>>,engine:Option<Res<VnEngine>>,
    owners:Option<Res<owner_hosts_0943::OwnerSceneProjection>>,startup:Option<Res<crate::debugger_startup::DebugBootstrap>>,
    roots:Query<(Entity,&WidgetRootSource)>,controls:Query<&ControlSource>,mut focus:Option<ResMut<bevy::a11y::Focus>>,targets:WidgetCameraInputs,
){
    let mut owner_geometry=None;let mut plans=HashMap::new();let mut metrics=None;let mut pending_names=std::collections::HashSet::new();let mut problem=None;
    let modal=screens.views.iter().rposition(|view|view.modal);
    let widgets=screens.views.iter().enumerate().filter(|(_,view)|view.root.layout_contract.is_some()).collect::<Vec<_>>();
    if !widgets.is_empty()||owners.is_some(){
        let result=(||{
            let entity=camera.get().ok_or_else(||WidgetLayoutProblem::Pending("Widget layout waits for the actual default UI camera".into()))?;
            let camera=cameras.get(entity).map_err(|_|WidgetLayoutProblem::Pending("Widget UI camera no longer exists".into()))?;
            let primary=targets.windows.iter().find(|(_,_,primary)|primary.is_some()).map(|(entity,_,_)|entity);
            let target=camera.target.normalize(primary).ok_or_else(||WidgetLayoutProblem::Pending("Widget layout waits for the actual camera render target".into()))?;
            let(images,textures)=targets.images.as_deref().zip(targets.textures.as_deref()).ok_or_else(||WidgetLayoutProblem::Pending("Widget layout waits for actual render-target resources".into()))?;
            let info=target.get_render_target_info(targets.windows.iter().map(|(entity,window,_)|(entity,window)),images,textures).ok_or_else(||WidgetLayoutProblem::Pending("Widget layout waits for actual render-target information".into()))?;
            let ui=ui_scale.as_deref().ok_or_else(||WidgetLayoutProblem::Pending("Widget layout waits for Bevy UiScale".into()))?.0;
            actual_widget_camera_metrics(entity,camera,ui,&accessibility.settings,target,info)
        })();
        match result{
            Err(error)=>{pending_names.extend(widgets.iter().map(|(_,view)|view.name.clone()));problem=Some(error);},
            Ok(metric)=>{
                if widget_buttons1030::pending_paint(engine.as_deref(),&screens,&prepared,Some(&metric),fonts.as_deref()){return;}
                metrics=Some(metric.clone());
                if let Some(owners)=owners.as_ref(){
                    let desired=(||{
                        let mut desired=HashMap::new();
                        for owner in owners.projection().descriptions(){
                    if owner.pending_materialization{continue;}
                            if owner.has_child&&owner.properties.viewport_slot.auto_size(){
                                let view=screens.views.iter().find(|view|view.name==owner.name&&view.order==owner.order).ok_or_else(||WidgetLayoutProblem::Invalid("Owner autosize lost its actual published child".into()))?;
                                let fonts=fonts.as_deref_mut().ok_or_else(||WidgetLayoutProblem::Pending("Owner autosize waits for its actual Font asset service".into()))?;
                                let default_font=if widget_needs_default_font(&view.root){Some(widget_default_font_0957::ensure(fonts,&mut prepared.default_font).map_err(|error|match error{widget_default_font_0957::Problem::MissingAsset=>WidgetLayoutProblem::Pending(error.to_string()),_=>WidgetLayoutProblem::Invalid(error.to_string())})?)}else{None};
                                desired.insert(owner.name.clone(),widget_desired_size(view,metric.settings.text_scale,metric.physical_scale,&assets,fonts,default_font.as_ref())?);
                            }
                        }
                        owner_hosts_0943::prepare_geometry_with_metric(owners.projection().clone(),metric.clone(),&desired).map_err(WidgetLayoutProblem::Invalid)
                    })();
                    match desired{Ok(plan)=>owner_geometry=Some(plan),Err(error)=>problem=Some(error)}
                }
                for(index,view)in widgets{
                    let result=(||{
                        if screens.views.iter().filter(|other|other.name==view.name).count()!=1{return Err(WidgetLayoutProblem::Invalid("Ambiguous Widget projection address: actual per-mount identity port is required".into()));}
                        let owner=widget_presentation_owner(engine.as_deref(),startup.as_deref()).ok_or_else(||WidgetLayoutProblem::Pending("Widget layout waits for its actual Core input-source/mount identity port".into()))?;
                        let source=owner.input_source(&view.name).filter(|source|source.order==view.order).ok_or_else(||WidgetLayoutProblem::Pending("Widget projection has no current exact Core input source".into()))?;
                        let key=source.to_wire().map_err(WidgetLayoutProblem::Invalid)?;
                        let fonts=fonts.as_deref_mut().ok_or_else(||WidgetLayoutProblem::Pending("Widget layout waits for Bevy Font assets".into()))?;
                        let default_font=if widget_needs_default_font(&view.root){Some(widget_default_font_0957::ensure(fonts,&mut prepared.default_font).map_err(|error|match error{widget_default_font_0957::Problem::MissingAsset=>WidgetLayoutProblem::Pending(error.to_string()),_=>WidgetLayoutProblem::Invalid(error.to_string())})?)}else{None};let mut font_receipts=HashMap::new();
                        let definition=widget_definition_digest(view)?;let bounds=if owners.is_some(){owner_geometry.as_ref().and_then(|owners|owners.child_bounds(&view.name,view.order)).ok_or_else(||WidgetLayoutProblem::Pending("Widget child waits for its exact prepared owner geometry".into()))?}else{metric.bounds};let rects=widget_layout_rects(view,bounds,metric.settings.text_scale,metric.physical_scale,&assets,fonts,default_font.as_ref(),&mut font_receipts)?;
                        // Bevy0.14 keeps FontId/atlas caches. A changed font on the
                        // same live handle needs a real rebind, never stale glyphs.
                        if font_receipts.iter().any(|(id,digest)|prepared.font_bindings.get(id).is_some_and(|old|old!=digest)){return Err(WidgetLayoutProblem::Invalid("Widget Font changed in-place: a fresh renderer font binding is required".into()));}
                        Ok((key,WidgetPlan{definition,rects,source,interactive:modal.is_none_or(|modal|index>=modal),projection:screen_projection(owner.state(),view),metrics:metric.clone(),fonts:font_receipts}))
                    })();
                    match result{Ok((key,plan))=>{for(id,digest)in &plan.fonts{prepared.font_bindings.insert(*id,*digest);}plans.insert(key,plan);},Err(error)=>{pending_names.insert(view.name.clone());if problem.is_none(){problem=Some(error);}}}
                }
            }
        }
    }
    let affected=screens.widget_plans.iter().filter(|(key,old)|plans.get(*key)!=Some(*old)).map(|(_,old)|old.source.clone()).collect::<Vec<_>>();
    if !affected.is_empty(){
        revoke_widget_sources(&mut screens,&affected);
        if let Some(focus)=focus.as_mut(){if focus.0.is_some_and(|entity|controls.get(entity).ok().is_some_and(|source|affected.contains(&source.0))){focus.0=None;}}
        for(entity,source)in &roots{if affected.contains(&source.0){commands.entity(entity).despawn_recursive();}}
    }
    if prepared.owner_geometry!=owner_geometry||prepared.plans!=plans||prepared.problem!=problem||prepared.metrics!=metrics||prepared.pending_names!=pending_names{
        if let Some(problem)=&problem{warn!("{}",problem.message());}
        prepared.revision=prepared.revision.checked_add(1).expect("Widget layout revision exhausted");prepared.owner_geometry=owner_geometry;prepared.plans=plans;prepared.problem=problem;prepared.metrics=metrics;prepared.pending_names=pending_names;
    }
}
/// Typed Widget layout reads the same Font asset and text measurement engine
/// as Bevy's TextBundle. Loading is not a font or geometry substitute.
#[derive(Debug,Clone,PartialEq,Eq)]
enum WidgetLayoutProblem { Pending(String), Invalid(String) }
impl WidgetLayoutProblem {fn message(&self)->&str{match self{Self::Pending(message)|Self::Invalid(message)=>message}}}
#[path="programmable_widget_text_lines_0960.rs"]
mod widget_text_lines_0960;
#[path="widget_default_font_0957.rs"]
mod widget_default_font_0957;
#[path="widget_font_units_0956.rs"]
mod widget_font_units_0956;
fn widget_needs_default_font(root:&UiComponent)->bool{
    fn needs(root:&UiComponent,node:&UiComponent)->bool{
        node.visible&&((node.kind==Kind::Text&&root.effective_font(&node.id).is_none())||node.children.iter().any(|child|needs(root,child)))
    }
    needs(root,root)
}
fn widget_font_base_size(node:&UiComponent,font:&Handle<Font>,fonts:&Assets<Font>)->Result<f32,WidgetLayoutProblem>{
    widget_font_units_0956::point_size_to_logical_px(fonts.get(font),node.font_size).map_err(|error|match error{
        widget_font_units_0956::WidgetFontUnitError::MissingFont=>WidgetLayoutProblem::Pending(format!("Widget '{}' waits for its actual Font asset {:?}",node.id,font.id())),
        _=>WidgetLayoutProblem::Invalid(format!("Widget '{}': {error}",node.id)),
    })
}
/// Prepare only the private WidgetV1 render projection. Authoring/VM values
/// remain points; OriginalText will receive the converted, unscaled baseline.
fn widget_render_text_view(mut view:ScreenView,assets:&AssetServer,fonts:&Assets<Font>,default_font:Option<&Handle<Font>>)->Result<ScreenView,WidgetLayoutProblem>{
    if view.root.layout_contract.is_none(){return Ok(view);}
    fn apply(node:&mut UiComponent,assets:&AssetServer,fonts:&Assets<Font>,inherited:Option<&Handle<Font>>)->Result<(),WidgetLayoutProblem>{
        if !node.visible{return Ok(());}
        let selected=node.font.as_ref().map(|path|assets.load::<Font>(path.clone()));
        let font=selected.as_ref().or(inherited);
        if node.kind==Kind::Text{let font=font.ok_or_else(||WidgetLayoutProblem::Pending("Widget Text has no prepared default Font asset".into()))?;node.font_size=widget_font_base_size(node,font,fonts)?;node.text=widget_text_lines_0960::normalize(&node.text).map_err(WidgetLayoutProblem::Invalid)?;}
        for child in &mut node.children{apply(child,assets,fonts,font)?;}
        Ok(())
    }
    apply(&mut view.root,assets,fonts,default_font)?;Ok(view)
}
fn widget_text_size(
    node:&UiComponent,font:&Handle<Font>,fonts:&Assets<Font>,text_scale:f32,physical_scale:f32,
)->Result<[f32;2],WidgetLayoutProblem>{
    if !text_scale.is_finite()||text_scale<=0.0||!physical_scale.is_finite()||physical_scale<=0.0{
        return Err(WidgetLayoutProblem::Invalid("Invalid Widget text/DPI/UI scale".into()));
    }
    if fonts.get(font).is_none(){return Err(WidgetLayoutProblem::Pending(format!("Widget '{}' waits for its actual Font asset {:?}",node.id,font.id())));}
    let size=widget_font_base_size(node,font,fonts)?*text_scale;
    widget_text_lines_0960::layout(&node.text,font,fonts,size,physical_scale)
        .map(|layout|layout.size)
        .map_err(|error|WidgetLayoutProblem::Invalid(format!("Widget '{}': {error}",node.id)))
}
fn widget_desired_size(view:&ScreenView,text_scale:f32,physical_scale:f32,assets:&AssetServer,fonts:&Assets<Font>,default_font:Option<&Handle<Font>>)->Result<[f32;2],WidgetLayoutProblem>{
 let mut problem=None;
 rvn_ui::programmable::measure_widget_desired(&view.root,&mut|node|{
  let font=if let Some(path)=view.root.effective_font(&node.id){assets.load::<Font>(path.to_owned())}else{let Some(font)=default_font else{let error=WidgetLayoutProblem::Pending("Owner Text has no prepared default Font asset".into());let message=error.message().to_owned();problem=Some(error);return Err(message);};font.clone()};
  widget_text_size(node,&font,fonts,text_scale,physical_scale).map_err(|error|{let message=error.message().to_owned();problem=Some(error);message})
 }).map_err(|message|problem.unwrap_or(WidgetLayoutProblem::Invalid(message)))
}
fn widget_layout_rects(
    view:&ScreenView,bounds:[f32;4],text_scale:f32,physical_scale:f32,assets:&AssetServer,fonts:&Assets<Font>,default_font:Option<&Handle<Font>>,receipts:&mut HashMap<bevy::asset::AssetId<Font>,[u8;32]>,
)->Result<Vec<rvn_ui::programmable::ComponentRect>,WidgetLayoutProblem>{
    let mut problem=None;
    let rects=rvn_ui::programmable::layout_widget_rects(&view.root,bounds,&mut|node|{
        let font=if let Some(path)=view.root.effective_font(&node.id){assets.load::<Font>(path.to_owned())}else{let Some(font)=default_font else{let error=WidgetLayoutProblem::Pending("Widget Text has no prepared default Font asset".into());let message=error.message().to_owned();problem=Some(error);return Err(message);};font.clone()};
        if let Some(asset)=fonts.get(&font){receipts.entry(font.id()).or_insert_with(||font_fingerprint(asset));}
        let result=if let Some(path)=view.root.effective_font(&node.id){
            if let Some(bevy::asset::LoadState::Failed(error))=assets.get_load_state(font.id()){
                Err(WidgetLayoutProblem::Invalid(format!("Widget '{}' cannot load font '{}': {error}",node.id,path)))
            }else{widget_text_size(node,&font,fonts,text_scale,physical_scale)}
        }else{widget_text_size(node,&font,fonts,text_scale,physical_scale)};
        result.map_err(|error|{let message=error.message().to_owned();problem=Some(error);message})
    }).map_err(|error|problem.unwrap_or(WidgetLayoutProblem::Invalid(error)))?;
    Ok(rects)
}
fn widget_resolved_view(view:&ScreenView,rects:&[rvn_ui::programmable::ComponentRect])->ScreenView{
    let rects=rects.iter().map(|rect|(rect.id.clone(),rect.rect)).collect::<HashMap<_,_>>();
    let mut resolved=view.clone();
    fn apply(node:&mut UiComponent,parent:[f32;2],rects:&HashMap<String,[f32;4]>){
        if let Some(rect)=rects.get(&node.id){node.rect=Some([rect[0]-parent[0],rect[1]-parent[1],rect[2],rect[3]]);for child in &mut node.children{apply(child,[rect[0],rect[1]],rects);}}
    }
    apply(&mut resolved.root,[0.0,0.0],&rects);resolved
}

#[cfg(test)]
fn widget_layout_view(view:&ScreenView,bounds:[f32;4],text_scale:f32,physical_scale:f32,assets:&AssetServer,fonts:&Assets<Font>,default_font:Option<&Handle<Font>>)->Result<ScreenView,WidgetLayoutProblem>{
    let rects=widget_layout_rects(view,bounds,text_scale,physical_scale,assets,fonts,default_font,&mut HashMap::new())?;Ok(widget_resolved_view(view,&rects))
}

/// Shared reference layout is resolved once per redraw. Descendant rectangles
/// are translated back to parent-local coordinates for Bevy's hit testing,
/// scrolling and transform animations; all targets retain authored identities.
#[cfg(test)]
fn layout_view(view: &ScreenView) -> ScreenView {
    layout_view_with_text_scale(view, 1.0)
}
fn layout_view_with_text_scale(view: &ScreenView, text_scale: f32) -> ScreenView {
    assert!(view.root.layout_contract.is_none(),"Typed Widgets require real Font measurement; legacy layout is not a fallback");
    let mut resolved = view.clone();
    let absolute_root = view.root.rect.is_none()
        && view.root.width.is_none()
        && view.root.height.is_none()
        && !view.root.children.is_empty()
        && view
            .root
            .children
            .iter()
            .filter(|child| child.visible)
            .all(|child| child.rect.is_some());
    if !absolute_root
        && view.root.rect.is_none()
        && matches!(view.root.kind, Kind::Panel | Kind::Row | Kind::Column)
        && view.root.scroll == ScrollAxis::None
    {
        resolved.root.scroll = ScrollAxis::Both;
    }
    let mut measurement = view.root.clone();
    measurement.visit_mut(&mut |component| component.font_size *= text_scale);
    let Ok(rects) = layout_rects(&measurement, [1920.0, 1080.0]) else {
        return resolved;
    };
    let rects: HashMap<_, _> = rects.into_iter().map(|item| (item.id, item.rect)).collect();
    fn apply(node: &mut UiComponent, parent: [f32; 2], rects: &HashMap<String, [f32; 4]>) {
        if let Some(rect) = rects.get(&node.id) {
            node.rect = Some([rect[0] - parent[0], rect[1] - parent[1], rect[2], rect[3]]);
            for child in &mut node.children {
                apply(child, [rect[0], rect[1]], rects);
            }
        }
    }
    apply(&mut resolved.root, [0.0, 0.0], &rects);
    resolved
}
pub(crate) fn update_views(screens: &mut Screens, views: Vec<ScreenView>) {
    // Authored ui.focus(), load and rollback win over transient input focus.
    // Ordinary redraws keep the current screen in the multi-screen tab order.
    if let Some(focus) = views.iter().rev().find_map(|view| {
        let old = screens.views.iter().find(|old| old.name == view.name);
        (old.is_none_or(|old| old.focus != view.focus))
            .then(|| view.focus.clone().map(|id| (view.name.clone(), id)))
            .flatten()
    }) {
        screens.reveal_focus = Some(focus.clone());
        screens.keyboard_focus = Some(focus);
    }
    screens.scroll_positions.retain(|(screen, element), _| {
        views
            .iter()
            .any(|view| &view.name == screen && view.root.find(element).is_some())
    });
    // Detached child focus/captures cannot authorize events against an old
    // component. The resident Screen owner is deliberately not retired here.
    let present = |name: &str, id: &str| views.iter().any(|view| view.name == name && view.root.find(id).is_some());
    if screens.keyboard_focus.as_ref().is_some_and(|(name,id)| !present(name,id)) { screens.keyboard_focus = None; }
    if screens.reveal_focus.as_ref().is_some_and(|(name,id)| !present(name,id)) { screens.reveal_focus = None; }
    if screens.dropdown.as_ref().is_some_and(|menu| !present(&menu.screen,&menu.element)) { screens.dropdown=None; screens.dropdown_source=None; }
    if screens.dragging.as_ref().is_some_and(|drag| !present(&drag.screen,&drag.element)) { screens.dragging=None; screens.dragging_source=None; screens.dragging_preview=None; }
    if !present(&screens.editing.screen,&screens.editing.element) { screens.editing=default(); screens.editing_source=None; }
    if screens.pending_press.as_ref().is_some_and(|press| !views.iter().any(|view| view.name == press.source.screen)) { screens.pending_press=None; }
    screens.views = views;
}
fn receive(mut commands: EventReader<VnCommand>, mut screens: ResMut<Screens>, engine: Option<Res<VnEngine>>) {
    // Read the actual committed owner state before programmable/story input.
    // Do this even when the child projection stays empty and emits no changes.
    screens.resident_modal = engine.as_deref().is_some_and(|engine| engine.0.interface_is_modal());
    for command in commands.read() {
        if let VnCommand::Interfaces(views) = command {
            update_views(&mut screens, views.clone());
        }
    }
    if screens.dropdown.as_ref().is_some_and(|menu| {
        !screens
            .views
            .iter()
            .any(|view| view.name == menu.screen && view.root.find(&menu.element).is_some())
    }) {
        screens.dropdown = None;
    }
}

/// Canvas frame/drawing snapshots do not change the host UI layout. Keep its
/// entities (and focus, DOM fields, scroll state) while only refreshing canvas
/// leaves. Any authored layout/style/control change still follows full redraw.
fn same_host_views(left: &[ScreenView], right: &[ScreenView]) -> bool {
    fn host(views: &[ScreenView]) -> Vec<ScreenView> {
        let mut views = views.to_vec();
        for view in &mut views {
            // Focus color is updated by the normal interaction/style pass.
            // A focus-only publication must not replace its physical control.
            view.focus=None;
            view.root.visit_mut(&mut |component| {
                if component.kind == Kind::Slider {component.value=serde_json::Value::Null;}
                if component.kind == Kind::Canvas {
                    component.drawing = None;
                    if let Some(frame) = &mut component.canvas_frame {
                        frame.time = 0.0;
                    }
                }
            });
        }
        views
    }
    left == right || host(left) == host(right)
}

fn render(
    mut commands: Commands,
    mut screens: ResMut<Screens>,
    roots: Query<(Entity,Option<&WidgetRootSource>), With<ScreenRoot>>,
    windows: Query<&Window>,
    assets: Res<AssetServer>,
    font: Res<crate::menu_documents::MenuFont>,
    state: Res<State<VnState>>,
    accessibility: Res<crate::accessibility::Accessibility>,
    source: Option<Res<crate::source_menus::SourceMenus>>,
    engine: Option<Res<VnEngine>>,
    startup:Option<Res<crate::debugger_startup::DebugBootstrap>>,
    prepared:Option<Res<WidgetLayouts>>,
    fonts:Option<Res<Assets<Font>>>,
) {
    let no_widget_plans=WidgetLayouts::default();let prepared=prepared.as_deref().unwrap_or(&no_widget_plans);
    let active = !screens.views.is_empty()
        && (source.as_deref().is_some_and(|source|source.any()) || matches!(
            state.get(),
            VnState::Waiting | VnState::Stepping | VnState::Animating
        ));
    let Ok(window) = windows.get_single() else {
        return;
    };
    let size = Vec2::new(window.width(), window.height());
    if screens.size==size&&prepared.metrics.as_ref().is_some_and(|metric|metric.settings==accessibility.settings)
        &&widget_buttons1030::pending_paint(engine.as_deref(),&screens,prepared,prepared.metrics.as_ref(),fonts.as_deref()){return;}
    let owner=widget_presentation_owner(engine.as_deref(),startup.as_deref());
    let mut input_sources:HashMap<_,_>=screens.views.iter().filter_map(|view|owner.as_ref().and_then(|owner|owner.input_source(&view.name)).filter(|source|source.order==view.order).map(|source|(view.name.clone(),source))).collect();
    // Names remain the existing legacy alias. Typed projections are admitted
    // only through their genuine Core source, including owner and incarnation.
    input_sources.retain(|name,source|screens.views.iter().enumerate().find(|(_,view)|view.name==*name).is_some_and(|(index,view)|view.root.layout_contract.is_none()||plan_for(&prepared.plans,view,Some(source)).is_some_and(|plan|plan.metrics.settings==accessibility.settings&&plan.interactive==screens.views.iter().rposition(|view|view.modal).is_none_or(|modal|index>=modal)&&plan.projection==owner.as_ref().and_then(|owner|screen_projection(owner.state(),view))&&fonts.as_deref().is_some_and(|fonts|widget_plan_fonts_current(plan,fonts)))));
    // Complete all fallible unit/font preparation before any source revocation,
    // Screens mutation, or queued ECS spawn/despawn in either render branch.
    let mut widget_render_views=HashMap::new();
    for view in screens.views.iter().filter(|view|view.root.layout_contract.is_some()){
        let Some(plan)=plan_for(&prepared.plans,view,input_sources.get(&view.name))else{continue;};
        let Some(fonts)=fonts.as_deref()else{return;};
        let resolved=widget_resolved_view(view,&plan.rects);
        let default_font=if widget_needs_default_font(&view.root){prepared.default_font.as_ref()}else{None};
        let Ok(resolved)=widget_render_text_view(resolved,&assets,fonts,default_font)else{return;};
        let Ok(wire)=plan.source.to_wire()else{return;};
        widget_render_views.insert(wire,resolved);
    }
    let canvas_tokens=engine.as_deref().map(|engine|canvas_input_tokens(&engine.0,&screens.views,&input_sources)).unwrap_or_default();
    let same_input_sources = screens.rendered_sources == input_sources && screens.rendered_canvas_tokens==canvas_tokens;
    let actual_state=owner.as_ref().map(WidgetPresentationOwner::state);
    let projections: Vec<_> = screens.views.iter()
        .map(|view|actual_state.and_then(|state|screen_projection(state,view)))
        .collect();
    let old_legacy=screens.rendered.iter().filter(|view|view.root.layout_contract.is_none()).cloned().collect::<Vec<_>>();
    let new_legacy=screens.views.iter().filter(|view|view.root.layout_contract.is_none()).cloned().collect::<Vec<_>>();
    let old_legacy_sources=screens.rendered_sources.iter().filter(|(name,_)|old_legacy.iter().any(|view|view.name==**name)).map(|(name,source)|(name.clone(),source.clone())).collect::<HashMap<_,_>>();
    let new_legacy_sources=input_sources.iter().filter(|(name,_)|new_legacy.iter().any(|view|view.name==**name)).map(|(name,source)|(name.clone(),source.clone())).collect::<HashMap<_,_>>();
    let legacy_projection=|views:&[ScreenView],projections:&[Option<crate::composed_motion::InterfaceProjectionTarget>]|views.iter().enumerate().filter(|(_,view)|view.root.layout_contract.is_none()).map(|(index,_)|projections.get(index).cloned().flatten()).collect::<Vec<_>>();
    let stable_legacy=same_host_views(&old_legacy,&new_legacy)&&old_legacy_sources==new_legacy_sources
        &&legacy_projection(&screens.rendered,&screens.rendered_projections)==legacy_projection(&screens.views,&projections)
        &&screens.size==size&&screens.active==active&&screens.rendered_dropdown==screens.dropdown&&screens.rendered_editing==screens.editing&&screens.layout_text_scale==accessibility.settings.text_scale;
    let has_widgets=!screens.widget_plans.is_empty()||screens.views.iter().any(|view|view.root.layout_contract.is_some());
    if has_widgets&&stable_legacy&&active{
        // This redraw belongs to specific Widget sources. Keep unchanged
        // neighbouring entities, their focus and their physical captures.
        let next=prepared.plans.iter().filter(|(_,plan)|input_sources.get(&plan.source.screen)==Some(&plan.source)).map(|(key,plan)|(key.clone(),plan.clone())).collect::<HashMap<_,_>>();
        let affected=screens.widget_plans.iter().filter(|(key,old)|next.get(*key)!=Some(*old)).map(|(_,old)|old.source.clone()).collect::<Vec<_>>();
        revoke_widget_sources(&mut screens,&affected);
        for(entity,source)in &roots{if let Some(source)=source{if affected.contains(&source.0){commands.entity(entity).despawn_recursive();}}}
        let modal=screens.views.iter().rposition(|view|view.modal);
        for(index,view)in screens.views.iter().enumerate().filter(|(_,view)|view.root.layout_contract.is_some()){
            let Some(plan)=plan_for(&next,view,input_sources.get(&view.name))else{continue;};let wire=plan.source.to_wire().expect("admitted real source");
            if screens.widget_plans.get(&wire)==Some(plan){
                for(entity,source)in &roots{if source.is_some_and(|source|source.0==plan.source){commands.entity(entity).insert(ZIndex::Global(3000+index as i32*10));}}
                continue;
            }
            let Some(resolved)=widget_render_views.get(&wire).cloned()else{return;};
            let default_font=if widget_needs_default_font(&view.root){prepared.default_font.as_ref()}else{None};
            spawn_projected_screen(&mut commands,view,&resolved,index,modal.is_none_or(|modal|index>=modal),&assets,default_font,size,&screens.editing,&screens.scroll_positions,projections.get(index).and_then(Option::as_ref),Some(&plan.source),Some(&accessibility.settings));
        }
        screens.widget_plans=next;screens.widget_layout_revision=prepared.revision;screens.rendered=screens.views.clone();screens.rendered_projections=projections;screens.rendered_sources=input_sources;screens.rendered_canvas_tokens=canvas_tokens;
        return;
    }
    if screens.widget_layout_revision==prepared.revision && same_input_sources && screens.rendered == screens.views
        && screens.rendered_projections == projections
        && screens.size == size
        && screens.active == active
        && screens.rendered_dropdown == screens.dropdown
        && screens.rendered_editing == screens.editing
        && screens.layout_text_scale == accessibility.settings.text_scale
    {
        return;
    }
    if screens.widget_layout_revision==prepared.revision && same_input_sources && screens.size == size
        && screens.rendered_projections == projections
        && screens.active == active
        && screens.rendered_dropdown == screens.dropdown
        && screens.rendered_editing == screens.editing
        && screens.layout_text_scale == accessibility.settings.text_scale
        && same_host_views(&screens.rendered, &screens.views)
    {
        screens.rendered = screens.views.clone();
        return;
    }
    debug!(
        "Interface redraw: views={}, size={}, active={}, dropdown={}, editing={}",
        screens.rendered != screens.views,
        screens.size != size,
        screens.active != active,
        screens.rendered_dropdown != screens.dropdown,
        screens.rendered_editing != screens.editing
    );
    let keep_widgets=if active&&screens.active{prepared.plans.iter().filter(|(key,plan)|screens.widget_plans.get(*key)==Some(*plan)&&input_sources.get(&plan.source.screen)==Some(&plan.source)).map(|(key,plan)|(key.clone(),plan.clone())).collect::<HashMap<_,_>>()}else{HashMap::new()};
    let affected=screens.widget_plans.iter().filter(|(key,old)|keep_widgets.get(*key)!=Some(*old)).map(|(_,old)|old.source.clone()).collect::<Vec<_>>();
    revoke_widget_sources(&mut screens,&affected);
    for(root,source)in &roots{if !source.is_some_and(|source|keep_widgets.values().any(|plan|plan.source==source.0)){commands.entity(root).despawn_recursive();}}
    screens.rendered = screens.views.clone();
    screens.rendered_projections = projections.clone();
    screens.size = size;
    screens.active = active;
    screens.layout_text_scale = accessibility.settings.text_scale;
    screens.widget_layout_revision=prepared.revision;
    screens.rendered_sources = input_sources;
    screens.rendered_canvas_tokens = canvas_tokens;
    screens.rendered_dropdown = screens.dropdown.clone();
    screens.rendered_editing = screens.editing.clone();
    if !active {
        screens.widget_plans.clear();return;
    }
    let modal=screens.views.iter().rposition(|view|view.modal);let mut widget_plans=HashMap::new();
    for(index,view)in screens.views.iter().enumerate(){
        let widget_plan=if view.root.layout_contract.is_some(){let Some(plan)=plan_for(&prepared.plans,view,screens.rendered_sources.get(&view.name))else{continue;};Some(plan)}else{None};
        let resolved=if let Some(plan)=widget_plan{let Ok(wire)=plan.source.to_wire()else{return;};let Some(resolved)=widget_render_views.get(&wire).cloned()else{return;};resolved}else{layout_view_with_text_scale(view,accessibility.settings.text_scale)};
        if let Some(plan)=widget_plan{let wire=plan.source.to_wire().expect("admitted source");widget_plans.insert(wire.clone(),plan.clone());if keep_widgets.contains_key(&wire){for(entity,source)in &roots{if source.is_some_and(|source|source.0==plan.source){commands.entity(entity).insert(ZIndex::Global(3000+index as i32*10));}}continue;}}
        let selected_font=if widget_plan.is_some(){if widget_needs_default_font(&view.root){prepared.default_font.as_ref()}else{None}}else{Some(&font.0)};
        spawn_projected_screen(&mut commands,view,&resolved,index,modal.is_none_or(|modal|index>=modal),&assets,selected_font,size,&screens.editing,&screens.scroll_positions,projections.get(index).and_then(Option::as_ref),screens.rendered_sources.get(&view.name),view.root.layout_contract.is_some().then_some(&accessibility.settings));
    }
    screens.widget_plans=widget_plans;
    if let Some(menu) = &screens.dropdown {
        let visible = ((size.y - 24.0) / 36.0).floor().clamp(1.0, 8.0) as usize;
        let first = dropdown_first(menu, visible);
        let height = menu.options.len().min(visible) as f32 * 36.0 + 8.0;
        let width = menu.width.max(180.0).min((size.x - 24.0).max(40.0));
        let top = if menu.position.y + height > size.y - 12.0 {
            (menu.position.y - height - 44.0).max(12.0)
        } else {
            menu.position.y.max(12.0)
        };
        let root = commands
            .spawn((
                ScreenRoot,
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        left: Val::Px(
                            menu.position
                                .x
                                .clamp(12.0, (size.x - width - 12.0).max(12.0)),
                        ),
                        top: Val::Px(top),
                        width: Val::Px(width),
                        height: Val::Px(height),
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::clip_y(),
                        padding: UiRect::all(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    background_color: Color::srgb(0.04, 0.07, 0.10).into(),
                    border_color: Color::srgb(0.2, 0.5, 0.7).into(),
                    z_index: ZIndex::Global(5000),
                    ..default()
                },
            ))
            .id();
        for (index, option) in menu.options.iter().enumerate().skip(first).take(visible) {
            let button = commands
                .spawn((
                    Control {
                        screen: menu.screen.clone(),
                        element: menu.element.clone(),
                        option: Some(option.clone()),
                    },
                    ButtonBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            height: Val::Px(36.0),
                            flex_shrink: 0.0,
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        background_color: if index == menu.highlighted {
                            Color::srgb(0.07, 0.28, 0.42).into()
                        } else {
                            Color::NONE.into()
                        },
                        ..default()
                    },
                ))
                .id();
            if let Some(source) = &screens.dropdown_source { commands.entity(button).insert(ControlSource(source.clone())); }
            commands.entity(root).add_child(button);
            let text = commands
                .spawn(TextBundle::from_section(
                    menu.labels.get(index).unwrap_or(option),
                    TextStyle {
                        font: font.0.clone(),
                        font_size: 20.0,
                        color: Color::WHITE,
                    },
                ))
                .id();
            commands.entity(button).add_child(text);
        }
    }
}

fn spawn_projected_screen(
    commands:&mut Commands,view:&ScreenView,resolved:&ScreenView,index:usize,interactive:bool,assets:&AssetServer,font:Option<&Handle<Font>>,size:Vec2,editing:&Editing,positions:&HashMap<(String,String),Vec2>,projection:Option<&crate::composed_motion::InterfaceProjectionTarget>,input_source:Option<&UiInputSource>,settings:Option<&rvn_ui::accessibility::AccessibilitySettings>,
){
    let root = commands
        .spawn((
        ScreenRoot,
        NodeBundle {
            style: Style {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
            },
            background_color: if view.modal {
            Color::srgba(0.0, 0.0, 0.0, 0.45).into()
            } else {
            Color::NONE.into()
            },
            focus_policy: if view.modal {
            FocusPolicy::Block
            } else {
            FocusPolicy::Pass
            },
            z_index: ZIndex::Global(3000 + index as i32 * 10),
            ..default()
        },
        ))
        .id();
    if let Some(source)=input_source{commands.entity(root).insert(OwnerChildSource(source.clone()));}
    if view.root.layout_contract.is_some(){commands.entity(root).insert(WidgetRootSource(input_source.expect("Widget requires actual Core source").clone()));}
    spawn(commands,root,resolved,&resolved.root,interactive,assets,font,size,editing,positions,1.0,0.0,0,projection,input_source,settings);
}

fn dropdown_first(menu: &Dropdown, visible: usize) -> usize {
    menu.highlighted
        .saturating_sub(visible.saturating_sub(1))
        .min(menu.options.len().saturating_sub(visible))
}

/// Authoring controls typography; only keep a one-physical-pixel safety floor.
/// Browser input overlays use the same base size before accessibility scaling.
pub(super) fn component_font_size(font_size: f32, scale: f32) -> f32 {
    (font_size * scale).max(1.0)
}

fn screen_projection(state:&rvn_core::GameState, view: &ScreenView)
    -> Option<crate::composed_motion::InterfaceProjectionTarget>
{
    let identity = rvn_core::ui_residency::visible_interface_target(state, &view.name).ok()?;
    let resident = rvn_core::ui_residency::resident_interface(state, &identity).ok()?;
    (resident.instance.order == view.order).then_some(crate::composed_motion::InterfaceProjectionTarget {
        interface: identity, order: view.order, host_role: resident.instance.host_role,
    })
}
fn spawn(
    commands: &mut Commands,
    parent: Entity,
    view: &ScreenView,
    node: &UiComponent,
    ancestors_enabled: bool,
    assets: &AssetServer,
    font: Option<&Handle<Font>>,
    size: Vec2,
    editing: &Editing,
    positions: &HashMap<(String, String), Vec2>,
    inherited_opacity: f32,
    parent_border: f32,
    depth: usize,
    projection: Option<&crate::composed_motion::InterfaceProjectionTarget>,
    input_source: Option<&UiInputSource>,
    widget_text_settings:Option<&rvn_ui::accessibility::AccessibilitySettings>,
) {
    if !node.visible {
        return;
    }
    let enabled = ancestors_enabled && node.enabled;
    let opacity = inherited_opacity * node.opacity;
    let selected_font = node
        .font
        .as_ref()
        .map(|path| assets.load::<Font>(path.clone()));
    let font = selected_font.as_ref().or(font);
    let control = node.is_control() || matches!(node.kind, Kind::Button | Kind::Canvas);
    let widget=view.root.layout_contract.is_some();
    let scale=if widget{1.0}else{(size.x/1920.0).min(size.y/1080.0).max(0.25)};
    let border_width = node.border_width.unwrap_or(if control { 1.0 } else { 0.0 }) * scale;
    let mut style = Style {
        width: node.width.map(|v| Val::Px(v * scale)).unwrap_or(Val::Auto),
        height: node.height.map(|v| Val::Px(v * scale)).unwrap_or(Val::Auto),
        min_width: node
            .min_width
            .map(|value| Val::Px(value * scale))
            .unwrap_or(if control {
                Val::Px(180.0 * scale)
            } else {
                Val::Auto
            }),
        min_height: node
            .min_height
            .map(|value| Val::Px(value * scale))
            .unwrap_or(if control {
                Val::Px(44.0 * scale)
            } else {
                Val::Auto
            }),
        max_width: node
            .max_width
            .map(|value| Val::Px(value * scale))
            .unwrap_or(Val::Auto),
        max_height: node
            .max_height
            .map(|value| Val::Px(value * scale))
            .unwrap_or(Val::Auto),
        flex_shrink: 0.0,
        flex_direction: if matches!(node.kind, Kind::Row | Kind::Toggle) {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        },
        align_items: if node.kind == Kind::Toggle {
            AlignItems::Center
        } else {
            AlignItems::Stretch
        },
        row_gap: Val::Px(node.spacing * scale),
        column_gap: Val::Px(node.spacing * scale),
        padding: UiRect::all(Val::Px(if control && node.padding == 0.0 {
            10.0 * scale
        } else {
            node.padding * scale
        })),
        border: UiRect::all(Val::Px(border_width)),
        justify_content: if control {
            JustifyContent::Center
        } else {
            JustifyContent::FlexStart
        },
        ..default()
    };
    if let Some(rect) = node.rect {
        // Taffy resolves absolute children from the parent's inner border.
        // RVN reference rectangles use its outer top-left, as in the designer.
        style.position_type = PositionType::Absolute;
        let ratio=if widget{Vec2::ONE}else{size/Vec2::new(1920.0,1080.0)};
        style.left=Val::Px(rect[0]*ratio.x-parent_border);style.top=Val::Px(rect[1]*ratio.y-parent_border);
        style.width=Val::Px(rect[2]*ratio.x);style.height=Val::Px(rect[3]*ratio.y);
        // The common layout already accounts for padding and margins.
        style.padding = UiRect::ZERO;
        style.min_width = Val::Auto;
        style.min_height = Val::Auto;
        style.max_width = Val::Auto;
        style.max_height = Val::Auto;
    }
    if node.clip {
        style.overflow = Overflow::clip();
    }
    // Absolute children do not contribute to a flex parent's measured size.
    // Give an implicit absolute-layout root the screen's extent; otherwise
    // auto-scroll would clip all of its children to an empty rectangle.
    let absolute_root = depth == 0
        && node.rect.is_none()
        && node.width.is_none()
        && node.height.is_none()
        && !node.children.is_empty()
        && node
            .children
            .iter()
            .filter(|child| child.visible)
            .all(|child| child.rect.is_some());
    if absolute_root {
        style.width = Val::Px(size.x);
        style.height = Val::Px(size.y);
    }
    // A top-level flow layout must stay reachable on a small window, even
    // when its author hasn't explicitly added a scroll container.
    let axis = node.scroll;
    let content_style = style.clone();
    if axis != ScrollAxis::None {
        style.overflow = Overflow::clip();
        style.max_width = Val::Px((size.x - 32.0).max(40.0));
        style.max_height = Val::Px((size.y - 32.0).max(40.0));
        style.padding = UiRect::ZERO;
        style.row_gap = Val::Px(0.0);
        style.column_gap = Val::Px(0.0);
    }
    let background = if control
        && node.kind != Kind::Canvas
        && node.auto_background
        && node.background[3] == 0.0
    {
        faded([0.06, 0.12, 0.18, 1.0], opacity)
    } else {
        faded(node.background, opacity)
    };
    let entity = commands
        .spawn((
            crate::composed_motion::InterfaceTarget {
                screen: view.name.clone(),
                element: node.id.clone(),
            },
            NodeBundle {
                style,
                background_color: background.into(),
                border_color: faded(node.border_color, opacity).into(),
                focus_policy: FocusPolicy::Pass,
                border_radius: BorderRadius::all(Val::Px(node.radius * scale)),
                ..default()
            },
        ))
        .id();
    if let Some(projection)=projection {commands.entity(entity).insert(projection.clone());}
    commands.entity(parent).add_child(entity);
    let children_parent = if axis != ScrollAxis::None {
        commands.entity(entity).insert(ScrollPort {
            screen: view.name.clone(),
            element: node.id.clone(),
            axis,
            depth,
        });
        let offset = positions
            .get(&(view.name.clone(), node.id.clone()))
            .copied()
            .unwrap_or_default();
        let content_width = node
            .children
            .iter()
            .filter_map(|child| child.rect)
            .map(|rect| rect[0] + rect[2])
            .fold(node.rect.map_or(0.0, |rect| rect[2]), f32::max);
        let content_height = node
            .children
            .iter()
            .filter_map(|child| child.rect)
            .map(|rect| rect[1] + rect[3])
            .fold(node.rect.map_or(0.0, |rect| rect[3]), f32::max);
        let content = commands
            .spawn((
                ScrollContent { port: entity },
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Relative,
                        left: Val::Px(-offset.x - border_width),
                        top: Val::Px(-offset.y - border_width),
                        width: Val::Px(content_width * size.x / 1920.0),
                        height: Val::Px(content_height * size.y / 1080.0),
                        flex_shrink: 0.0,
                        flex_direction: content_style.flex_direction,
                        row_gap: content_style.row_gap,
                        column_gap: content_style.column_gap,
                        padding: content_style.padding,
                        ..default()
                    },
                    focus_policy: FocusPolicy::Pass,
                    ..default()
                },
            ))
            .id();
        commands.entity(entity).add_child(content);
        content
    } else {
        entity
    };
    if axis != ScrollAxis::None {
        for horizontal in [false, true] {
            if if horizontal {
                axis.horizontal()
            } else {
                axis.vertical()
            } {
                let thumb = commands
                    .spawn((
                        ScrollThumb {
                            port: entity,
                            horizontal,
                        },
                        NodeBundle {
                            style: Style {
                                position_type: PositionType::Absolute,
                                width: Val::Px(4.0),
                                height: Val::Px(4.0),
                                ..default()
                            },
                            background_color: faded([0.19, 0.40, 0.52, 1.0], opacity).into(),
                            border_radius: BorderRadius::all(Val::Px(2.0)),
                            visibility: Visibility::Hidden,
                            focus_policy: FocusPolicy::Pass,
                            z_index: ZIndex::Local(1),
                            ..default()
                        },
                    ))
                    .id();
                commands.entity(entity).add_child(thumb);
            }
        }
    }
    if let Some(source) = input_source { commands.entity(entity).insert(ControlSource(source.clone())); }
    // Disabled controls still absorb the pointer: clicking them must never
    // click through into the story or another screen beneath them.
    if control {
        commands.entity(entity).insert((
            Interaction::None,
            FocusPolicy::Block,
            Control {
                screen: view.name.clone(),
                element: node.id.clone(),
                option: None,
            },
        ));
    }
    if node.kind == Kind::Canvas {
        if let Some(drawing) = &node.drawing {
            let extent = node
                .canvas_frame
                .map(|frame| Vec2::new(frame.width, frame.height))
                .or_else(|| node.rect.map(|rect| Vec2::new(rect[2], rect[3])))
                .unwrap_or(Vec2::new(
                    node.width.unwrap_or(320.0),
                    node.height.unwrap_or(240.0),
                ));
            commands
                .entity(entity)
                .insert(crate::custom_canvas::CanvasSurface {
                    screen: view.name.clone(),
                    element: node.id.clone(),
                    drawing: drawing.clone(),
                    extent,
                    font: match font{Some(font)=>font.clone(),None=>return},
                    opacity,
                });
        }
    }
    if node.kind == Kind::Toggle {
        let checked = node.value.as_bool().unwrap_or(false);
        let box_node = commands
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Px(18.0 * scale),
                    height: Val::Px(18.0 * scale),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                background_color: faded(
                    if checked {
                        [0.08, 0.57, 0.76, 1.0]
                    } else {
                        [0.03, 0.06, 0.09, 1.0]
                    },
                    opacity,
                )
                .into(),
                border_color: faded([0.30, 0.55, 0.68, 1.0], opacity).into(),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                focus_policy: FocusPolicy::Pass,
                ..default()
            })
            .id();
        commands.entity(entity).add_child(box_node);
        if checked {
            let mark = commands
                .spawn(TextBundle::from_section(
                    "✓",
                    TextStyle {
                        font: match font{Some(font)=>font.clone(),None=>return},
                        font_size: (16.0 * scale).max(12.0),
                        color: faded([1.0; 4], opacity),
                    },
                ))
                .id();
            commands.entity(box_node).add_child(mark);
        }
    }
    if node.kind == Kind::Image {
        if let Some(label) = node
            .accessible_label
            .as_ref()
            .filter(|label| !label.is_empty())
        {
            let mut semantics =
                bevy::a11y::accesskit::NodeBuilder::new(bevy::a11y::accesskit::Role::Image);
            semantics.set_name(label.clone());
            if !enabled {
                semantics.set_disabled();
            }
            commands
                .entity(entity)
                .insert(bevy::a11y::AccessibilityNode::from(semantics));
        }
        let image = commands
            .spawn(ImageBundle {
                image: UiImage::new(assets.load(node.image.clone().unwrap()))
                    .with_color(Color::srgba(1.0, 1.0, 1.0, opacity)),
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                focus_policy: FocusPolicy::Pass,
                ..default()
            })
            .id();
        commands.entity(entity).add_child(image);
    } else if !matches!(
        node.kind,
        Kind::Panel | Kind::Column | Kind::Row | Kind::Canvas
    ) && !(widget&&node.kind==Kind::Button) {
        // Widget Button owns a real content child. Its legacy text leaf is not
        // an extra visual, hit target, accessibility node or measured child.
        let mut label = match node.kind {
            Kind::Input => {
                if cfg!(target_arch = "wasm32") && enabled {
                    String::new()
                } else {
                    node.value.as_str().unwrap_or_default().to_owned()
                }
            }
            Kind::Select => format!(
                "{}  ▾",
                node.option_label(node.value.as_str().unwrap_or_default())
            ),
            Kind::Toggle => node.text.clone(),
            Kind::Slider => slider_label(node),
            _ => node.text.clone(),
        };
        if node.kind == Kind::Input && !cfg!(target_arch = "wasm32") {
            if editing.screen == view.name && editing.element == node.id {
                let caret = editing.caret.min(label.len());
                if label.is_char_boundary(caret) {
                    label.insert(caret, '│');
                }
            } else if label.is_empty() {
                label = node.placeholder.clone();
            }
        }
        let justify = match node.text_align {
            TextAlignment::Left => JustifyText::Left,
            TextAlignment::Center => JustifyText::Center,
            TextAlignment::Right => JustifyText::Right,
        };
        let mut bundle = TextBundle::from_section(
            label,
            TextStyle {
                font: match font{Some(font)=>font.clone(),None=>return},
                font_size: if widget{node.font_size}else{component_font_size(node.font_size,scale)},
                color: if enabled {
                    faded(node.foreground, opacity)
                } else {
                    faded([0.4, 0.45, 0.5, 1.0], opacity)
                },
            },
        )
        .with_text_justify(justify);
        bundle.style.width = Val::Percent(100.0);
        bundle.style.padding = UiRect::all(Val::Px(if control && node.padding == 0.0 {
            10.0 * scale
        } else {
            node.padding * scale
        }));
        let text=if widget{
            bundle.style.height=Val::Percent(100.0);
            bundle.text=bundle.text.with_no_wrap();
            let settings=widget_text_settings.expect("Typed Widget spawn requires its exact accessibility settings");
            commands.spawn(crate::accessibility::initialize_widget_text(bundle,settings)).id()
        }else{commands.spawn(bundle).id()};
        if node.kind==Kind::Slider {commands.entity(text).insert(SliderLabel(entity));}
        commands.entity(entity).add_child(text);
    }
    if node.kind == Kind::Slider {
        let fraction = ((node.value.as_f64().unwrap_or(node.min) - node.min)
            / (node.max - node.min))
            .clamp(0.0, 1.0) as f32;
        let track = commands
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Px(6.0),
                    margin: UiRect::top(Val::Px(3.0)),
                    ..default()
                },
                background_color: faded([0.02, 0.06, 0.09, 1.0], opacity).into(),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                focus_policy: FocusPolicy::Pass,
                ..default()
            })
            .id();
        commands.entity(entity).add_child(track);
        let fill = commands
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(fraction * 100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                background_color: faded([0.08, 0.58, 0.78, 1.0], opacity).into(),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                focus_policy: FocusPolicy::Pass,
                ..default()
            })
            .id();
        commands.entity(fill).insert(SliderFill(entity));
        commands.entity(track).add_child(fill);
        let handle = commands
            .spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(fraction * 100.0),
                    top: Val::Px(-3.0),
                    width: Val::Px(12.0),
                    height: Val::Px(12.0),
                    margin: UiRect::left(Val::Px(-6.0)),
                    ..default()
                },
                background_color: faded([0.75, 0.93, 1.0, 1.0], opacity).into(),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                focus_policy: FocusPolicy::Pass,
                ..default()
            })
            .id();
        commands.entity(track).add_child(handle);
    }
    for child in &node.children {
        spawn(
            commands,
            children_parent,
            view,
            child,
            enabled,
            assets,
            font,
            size,
            editing,
            positions,
            opacity,
            if axis == ScrollAxis::None {
                border_width
            } else {
                0.0
            },
            depth + 1,
            projection,
            input_source,
            widget_text_settings,
        );
    }
}

fn collect_scroll(
    mut screens: ResMut<Screens>,
    windows: Query<&Window>,
    ports: Query<(&ScrollPort, &Node, &GlobalTransform)>,
    mut wheel: EventReader<MouseWheel>,
    input:crate::runtime_restart::InputAccess,
) {
    let movement = wheel.read_with_id().filter(|(_,id)|input.accepts::<MouseWheel>(id.id)).fold(Vec2::ZERO, |sum, (event,_)| {
        sum + Vec2::new(event.x, event.y)
            * if event.unit == MouseScrollUnit::Line {
                36.0
            } else {
                1.0
            }
    });
    if !screens.active
        || screens.dropdown.is_some()
        || screens.pointer_consumed
        || movement == Vec2::ZERO
    {
        return;
    }
    let cursor = windows
        .get_single()
        .ok()
        .and_then(|window| window.cursor_position());
    let modal = screens
        .views
        .iter()
        .rposition(|view| view.modal)
        .unwrap_or(0);
    if ports.iter().any(|(port, node, transform)| {
        screens
            .views
            .iter()
            .skip(modal)
            .any(|view| view.name == port.screen)
            && cursor.is_some_and(|cursor| transformed_contains(node.size(), transform, cursor))
    }) {
        screens.pointer_consumed = true;
        screens.pending_scroll += movement;
    }
}

fn scroll(
    mut screens: ResMut<Screens>,
    windows: Query<&Window>,
    ports: Query<(Entity, &ScrollPort, &Node, &GlobalTransform)>,
    mut contents: Query<(&ScrollContent, &Node, &mut Style), Without<ScrollThumb>>,
    mut thumbs: Query<(&ScrollThumb, &mut Style, &mut Visibility), Without<ScrollContent>>,
    controls: Query<(Entity, &Control, &Node, &GlobalTransform)>,
    parents: Query<&Parent>,
) {
    // Geometry is only authoritative after layout and transform propagation.
    // Reading it during Update can acknowledge a focus request against an old
    // or newly spawned layout, leaving the requested control off screen.
    let movement = std::mem::take(&mut screens.pending_scroll);
    if !screens.active || screens.dropdown.is_some() {
        return;
    }
    let cursor = windows
        .get_single()
        .ok()
        .and_then(|window| window.cursor_position());
    let modal = screens
        .views
        .iter()
        .rposition(|view| view.modal)
        .unwrap_or(0);
    let hit = ports
        .iter()
        .filter(|(_, port, node, transform)| {
            screens
                .views
                .iter()
                .skip(modal)
                .any(|view| view.name == port.screen)
                && cursor.is_some_and(|cursor| {
                    (cursor - transform.translation().truncate())
                        .abs()
                        .cmple(node.size() * 0.5)
                        .all()
                })
        })
        .max_by_key(|(_, port, _, _)| {
            (
                screens
                    .views
                    .iter()
                    .position(|view| view.name == port.screen)
                    .unwrap_or(0),
                port.depth,
            )
        })
        .map(|(entity, _, _, _)| entity);
    let reveal = screens.reveal_focus.clone();
    let focused = reveal.as_ref().and_then(|(screen, element)| {
        controls.iter().find(|(_, control, node, _)| {
            &control.screen == screen
                && &control.element == element
                && control.option.is_none()
                && node.size().cmpgt(Vec2::ZERO).all()
        })
    });
    for (content, node, mut style) in &mut contents {
        let Ok((entity, port, viewport, transform)) = ports.get(content.port) else {
            continue;
        };
        // Commands spawn fresh nodes before Bevy's layout pass. Their zero
        // dimensions must not erase scroll restoration or pending focus.
        if !viewport.size().cmpgt(Vec2::ZERO).all() || !node.size().cmpgt(Vec2::ZERO).all() {
            continue;
        }
        let max = (node.size() - viewport.size()).max(Vec2::ZERO);
        let key = (port.screen.clone(), port.element.clone());
        let mut offset = screens
            .scroll_positions
            .get(&key)
            .copied()
            .unwrap_or_default();
        if hit == Some(entity) && movement != Vec2::ZERO && max != Vec2::ZERO {
            if port.axis.vertical() {
                offset.y -= movement.y;
            }
            if port.axis.horizontal() {
                offset.x -= if movement.x != 0.0 {
                    movement.x
                } else if !port.axis.vertical() {
                    movement.y
                } else {
                    0.0
                };
            }
            screens.pointer_consumed = true;
        }
        if let Some((focused_entity, control, focused_node, focused_transform)) = focused {
            // Only ancestors of this exact entity may reveal it.
            let mut ancestor = focused_entity;
            let mut owned = false;
            while let Ok(parent) = parents.get(ancestor) {
                ancestor = parent.get();
                if ancestor == entity {
                    owned = true;
                    break;
                }
            }
            if owned && port.screen == control.screen {
                let center =
                    focused_transform.translation().truncate() - transform.translation().truncate();
                let low = center - focused_node.size() * 0.5;
                let high = center + focused_node.size() * 0.5;
                debug!("Reveal {} / {}: viewport={:?}, content={:?}, center={center:?}, offset={offset:?}",port.screen,control.element,viewport.size(),node.size());
                if port.axis.vertical() {
                    if low.y < -viewport.size().y * 0.5 {
                        offset.y += low.y + viewport.size().y * 0.5;
                    } else if high.y > viewport.size().y * 0.5 {
                        offset.y += high.y - viewport.size().y * 0.5;
                    }
                }
                if port.axis.horizontal() {
                    if low.x < -viewport.size().x * 0.5 {
                        offset.x += low.x + viewport.size().x * 0.5;
                    } else if high.x > viewport.size().x * 0.5 {
                        offset.x += high.x - viewport.size().x * 0.5;
                    }
                }
            }
        }
        if !port.axis.vertical() {
            offset.y = 0.0;
        }
        if !port.axis.horizontal() {
            offset.x = 0.0;
        }
        offset = offset.clamp(Vec2::ZERO, max);
        if style.left != Val::Px(-offset.x) {
            style.left = Val::Px(-offset.x);
        }
        if style.top != Val::Px(-offset.y) {
            style.top = Val::Px(-offset.y);
        }
        for (thumb, mut style, mut visibility) in &mut thumbs {
            if thumb.port != entity {
                continue;
            }
            let (viewport_length, content_length, position, maximum) = if thumb.horizontal {
                (viewport.size().x, node.size().x, offset.x, max.x)
            } else {
                (viewport.size().y, node.size().y, offset.y, max.y)
            };
            *visibility = if maximum > 0.5 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            let track = (viewport_length - 12.0).max(1.0);
            let length =
                (track * viewport_length / content_length.max(1.0)).clamp(16.0, track.max(16.0));
            let start = 6.0 + (track - length).max(0.0) * position / maximum.max(1.0);
            if thumb.horizontal {
                style.width = Val::Px(length);
                style.height = Val::Px(4.0);
                style.left = Val::Px(start);
                style.bottom = Val::Px(3.0);
            } else {
                style.height = Val::Px(length);
                style.width = Val::Px(4.0);
                style.top = Val::Px(start);
                style.right = Val::Px(3.0);
            }
        }
        screens.scroll_positions.insert(key, offset);
    }
    if focused.is_some() {
        screens.reveal_focus = None;
    }
}

fn feedback(
    screens: Res<Screens>,
    engine:Option<Res<VnEngine>>,
    mut controls: Query<(
        Entity,
        &Control,
        &Interaction,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    for (entity, control, interaction, mut background, mut border) in &mut controls {
        let Some(view) = screens
            .views
            .iter()
            .find(|view| view.name == control.screen)
        else {
            continue;
        };
        let Some(component) = view.root.find(&control.element) else {
            continue;
        };
        let enabled = view.root.available(&control.element)
            && !screens
                .views
                .iter()
                .any(|other| other.modal && (other.layer, other.order) > (view.layer, view.order));
        let focused = enabled
            && screens.keyboard_focus.as_ref().map_or(
                view.focus.as_ref() == Some(&control.element),
                |(screen, element)| screen == &control.screen && element == &control.element,
            );
        let opacity = view.root.effective_opacity(&control.element).unwrap_or(1.0);
        *border = faded(
            if focused {
                component.focus_color.unwrap_or([0.12, 0.72, 1.0, 1.0])
            } else {
                component.border_color
            },
            opacity,
        )
        .into();
        let widget_button=engine.as_deref().is_some_and(|engine|widget_buttons1030::nominal(engine,view,component));
        let interaction=if widget_button {if screens.widget_buttons1030.pressed(entity){Interaction::Pressed}else if *interaction==Interaction::Pressed{Interaction::Hovered}else{*interaction}}else{*interaction};
        *background = match interaction {
            _ if component.kind == Kind::Canvas => faded(component.background, opacity).into(),
            _ if !enabled => faded(component.background, opacity).into(),
            Interaction::Pressed => faded(
                component
                    .pressed_background
                    .unwrap_or([0.05, 0.37, 0.52, 1.0]),
                opacity,
            )
            .into(),
            Interaction::Hovered => faded(
                component
                    .hover_background
                    .unwrap_or([0.10, 0.25, 0.34, 1.0]),
                opacity,
            )
            .into(),
            _ if control.option.as_ref().is_some_and(|option| {
                screens
                    .dropdown
                    .as_ref()
                    .is_some_and(|menu| menu.options.get(menu.highlighted) == Some(option))
            }) =>
            {
                Color::srgb(0.07, 0.28, 0.42).into()
            }
            _ if control.option.is_none()
                && (component.background[3] != 0.0 || !component.auto_background) =>
            {
                faded(component.background, opacity).into()
            }
            _ => faded([0.06, 0.12, 0.18, 1.0], opacity).into(),
        };
    }
}

fn send(
    engine: &mut VnEngine,
    event: UiInput,
    screens: &mut Screens,
    output: &mut EventWriter<VnCommand>,
    error: &mut ScriptErrorMessage,
    next: &mut NextState<VnState>,
) -> bool {
    if engine.0.renderer.menu_pending || engine.0.renderer.scene_batch_pending() {return false;}
    let source_event=engine.0.state.ui.screens.iter().any(|screen|screen.name==event.screen && screen.host_role.is_some());
    if engine.0.renderer.scene_frame_failed && !source_event {return false;}
    let Some(source) = screens.dispatch_source.as_ref().filter(|source|source.screen == event.screen)
        .or_else(||screens.frame_sources.get(&event.screen)).cloned() else { return false; };
    if crate::editor_debugger::enabled(engine){
        return match crate::editor_debugger::input(engine,ScopedUiInput{source,event}){
            Ok(accepted)=>accepted,
            Err(problem)=>{error.0=problem;if !source_event{next.set(VnState::Error);}false}
        };
    }
    match engine.0.interface_event_scoped(ScopedUiInput { source: source.clone(), event }) {
        Ok(false) => return false,
        Ok(true) => {},
        Err(problem) => {
        error.0 = rvn_core::error::ScriptError::from_runtime(&problem).to_string();
        if source_event||problem.is_menu_request_rejection() {warn!("Game menu event: {}",error.0);} else {next.set(VnState::Error);}
        return false;
        }
    }
    let still_current=engine.0.ui_input_source_current(&source);
    if engine.0.renderer.scene_batch_pending() { return still_current; }
    match engine.0.interface_views() {
        Ok(views) => update_views(screens, views),
        Err(problem) => {
            error.0 = rvn_core::error::ScriptError::from_runtime(&problem).to_string();
            next.set(VnState::Error);
            return false;
        }
    }
    for command in engine.0.renderer.take_pending() {
        output.send(command);
    }
    still_current
}

fn event(screen: &str, element: &str, kind: EventKind, value: Option<Value>) -> UiInput {
    UiInput {
        screen: screen.into(),
        element: element.into(),
        kind,
        value,
        key: None,
    }
}

/// Assistive technology uses the same validated event path as pointer and
/// keyboard input. Stale, hidden, disabled and covered controls are ignored.
fn assistive_input(
    mut requests: EventReader<bevy::a11y::ActionRequest>,
    mut screens: ResMut<Screens>,
    mut engine: ResMut<VnEngine>,
    controls: Query<(&Control, &Node, &GlobalTransform, Option<&ControlSource>)>,
    mut output: EventWriter<VnCommand>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
    state: Res<State<VnState>>,
    accessibility: Res<crate::accessibility::Accessibility>,
    source: Option<Res<crate::source_menus::SourceMenus>>,
    input:crate::runtime_restart::InputAccess,
) {
    use bevy::a11y::accesskit::Action;
    for (request,id) in requests.read_with_id() {
        if !input.accepts::<bevy::a11y::ActionRequest>(id.id){continue;}
        if accessibility.blocked
            || (!source.as_deref().is_some_and(|source|source.any()) && !matches!(
                state.get(),
                VnState::Waiting | VnState::Stepping | VnState::Animating
            ))
        {
            continue;
        }
        let Ok(entity) = Entity::try_from_bits(request.0.target.0) else {
            continue;
        };
        let Ok((control, node, transform, captured)) = controls.get(entity) else {
            continue;
        };
        let Some(source) = control_source(&engine, &screens, control, captured) else { continue; };
        screens.dispatch_source = Some(source);
        let Some(view) = screens
            .views
            .iter()
            .find(|view| view.name == control.screen)
            .cloned()
        else {
            continue;
        };
        let Some(component) = view.root.find(&control.element).cloned() else {
            continue;
        };
        if !view.root.available(&control.element)
            || screens
                .views
                .iter()
                .any(|other| other.modal && (other.layer, other.order) > (view.layer, view.order))
        {
            continue;
        }
        if request.0.action == Action::Focus {
            if !send(
                &mut engine,
                event(&control.screen, &control.element, EventKind::Focus, None),
                &mut screens,
                &mut output,
                &mut error,
                &mut next,
            ) {
                return;
            }
            screens.keyboard_focus = Some((control.screen.clone(), control.element.clone()));
            screens.reveal_focus = screens.keyboard_focus.clone();
            continue;
        }
        if request.0.action == Action::Default
            && component.kind == Kind::Select
            && control.option.is_none()
        {
            screens.dropdown_source = screens.dispatch_source.clone();
            screens.dropdown = Some(Dropdown {
                screen: control.screen.clone(),
                element: control.element.clone(),
                options: component.options.clone(),
                labels: component.option_labels.clone(),
                highlighted: component
                    .options
                    .iter()
                    .position(|value| component.value.as_str() == Some(value))
                    .unwrap_or(0),
                position: transform.translation().truncate()
                    + Vec2::new(-node.size().x * 0.5, node.size().y * 0.5),
                width: node.size().x,
            });
            continue;
        }
        let input = if let Some(option) = &control.option {
            (request.0.action == Action::Default).then(|| {
                event(
                    &control.screen,
                    &control.element,
                    EventKind::Change,
                    Some(Value::Str(option.clone())),
                )
            })
        } else {
            assistive_event(&control.screen, &component, &request.0)
        };
        if let Some(input) = input {
            screens.dropdown = None;
            if !send(
                &mut engine,
                input,
                &mut screens,
                &mut output,
                &mut error,
                &mut next,
            ) {
                return;
            }
        }
    }
}
fn assistive_event(
    screen: &str,
    component: &UiComponent,
    request: &bevy::a11y::accesskit::ActionRequest,
) -> Option<UiInput> {
    use bevy::a11y::accesskit::{Action, ActionData};
    let value = match (&component.kind, request.action, &request.data) {
        (Kind::Button, Action::Default, _) => {
            return Some(event(screen, &component.id, EventKind::Activate, None))
        }
        (Kind::Canvas, Action::Default, _)
            if component.events.contains_key(&EventKind::Activate) =>
        {
            return Some(event(screen, &component.id, EventKind::Activate, None))
        }
        (Kind::Toggle, Action::Default, _) => {
            Value::Bool(!component.value.as_bool().unwrap_or(false))
        }
        (Kind::Input, Action::SetValue, Some(ActionData::Value(value)))
            if value.len() <= 65_536 && !value.contains('\0') =>
        {
            Value::Str(value.to_string())
        }
        (Kind::Select, Action::SetValue, Some(ActionData::Value(value)))
            if component
                .options
                .iter()
                .any(|option| option == value.as_ref()) =>
        {
            Value::Str(value.to_string())
        }
        (Kind::Slider, Action::SetValue, Some(ActionData::NumericValue(value)))
            if value.is_finite() =>
        {
            Value::Float(value.clamp(component.min, component.max) as f32)
        }
        (Kind::Slider, Action::SetValue, Some(ActionData::Value(value))) => {
            let value = value.parse::<f64>().ok()?;
            if !value.is_finite() {
                return None;
            }
            Value::Float(value.clamp(component.min, component.max) as f32)
        }
        _ => return None,
    };
    Some(event(screen, &component.id, EventKind::Change, Some(value)))
}

fn input(
    mut screens: ResMut<Screens>,
    mut engine: ResMut<VnEngine>,
    controls: Query<(Entity, &Control, &Interaction, &Node, &GlobalTransform, Option<&ControlSource>)>,
    preview: Option<Res<crate::editor_preview_input::PointerOverride>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window>,
    mut keyboard: EventReader<KeyboardInput>,
    mut ime: EventReader<Ime>,
    mut wheel: EventReader<MouseWheel>,
    mut output: EventWriter<VnCommand>,
    mut error: ResMut<ScriptErrorMessage>,
    mut next: ResMut<NextState<VnState>>,
    state: Res<State<VnState>>,
    accessibility: Res<crate::accessibility::Accessibility>,
    context:(Option<Res<crate::source_menus::SourceMenus>>,crate::runtime_restart::InputAccess,Option<Res<crate::scene_publication::ScenePresentation>>,Option<Res<WidgetLayouts>>),
) {
    let (source,input,presentation,button_layouts1030)=context;
    if accessibility.blocked || !input.available() {
        screens.widget_buttons1030.cancel();
        keyboard.clear();
        ime.clear();
        wheel.clear();
        screens.pointer_consumed = true;
        screens.keyboard_consumed = true;
        return;
    }
    if !screens.input_snapshot_captured && engine.0.state.scene_execution.is_none() {
        // Standalone legacy consumers have no plugin capture system.
        screens.frame_sources = legacy_input_sources(&engine.0, &screens.views);
        screens.input_focus = screens.keyboard_focus.clone();
    }
    screens.dispatch_source = None;
    screens.pointer_consumed = false;
    screens.keyboard_consumed = false;
    // Drain even while menus/DOM fields own focus; otherwise a release could
    // be lost and leave Shift/Ctrl stuck on the next canvas interaction.
    let raw_keys: Vec<_> = keyboard.read_with_id()
        .filter(|(_,id)|input.accepts::<KeyboardInput>(id.id))
        .map(|(event,id)|((*event).clone(),queued_native_current(&screens,&engine,id.id,false),screens.key_producers.get(&id.id).cloned())).collect();
    let has_control = raw_keys
        .iter()
        .any(|(key,_,_)| matches!(key.key_code, KeyCode::ControlLeft | KeyCode::ControlRight));
    let has_shift = raw_keys
        .iter()
        .any(|(key,_,_)| matches!(key.key_code, KeyCode::ShiftLeft | KeyCode::ShiftRight));
    // Drain every physical modifier edge, then admit only the authored events
    // whose producer still exists. Complete Shift+Tab chords keep their order.
    let admitted:Vec<_>=raw_keys.iter().filter(|(event,_,_)|event.state==ButtonState::Pressed).map(|(_,current,_)|*current).collect();
    let admitted_edges:std::collections::HashSet<_>=raw_keys.iter().filter(|(event,current,producer)|*current && event.state==ButtonState::Pressed && producer.as_ref().is_some_and(|producer|producer.key_edges.contains(&event.key_code))).map(|(event,_,_)|event.key_code).collect();
    let tab_producer=raw_keys.iter().find(|(event,current,_)|*current && event.state==ButtonState::Pressed && event.key_code==KeyCode::Tab).and_then(|(_,_,producer)|producer.clone());
    let mut pressed:Vec<_> = key_chords(raw_keys.into_iter().map(|(event,_,_)|event), &mut screens.modifiers)
        .into_iter().zip(admitted).filter_map(|(event,current)|current.then_some(event)).collect();
    let captured_native=screens.input_snapshot_captured;
    let native_just_pressed=|key|if captured_native {admitted_edges.contains(&key)}else{keys.just_pressed(key)};
    for (_, control, shift) in &mut pressed {
        if !has_control {
            *control |= keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        }
        if !has_shift {
            *shift |= keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        }
    }
    screens.modifiers = [
        keys.pressed(KeyCode::ControlLeft),
        keys.pressed(KeyCode::ControlRight),
        keys.pressed(KeyCode::ShiftLeft),
        keys.pressed(KeyCode::ShiftRight),
    ];
    #[cfg(target_arch = "wasm32")]
    {
        let events = match crate::web_inputs::events() {
            Ok(events) => events,
            Err(problem) => {
                error.0 = problem;
                next.set(VnState::Error);
                return;
            }
        };
        if source.as_deref().is_some_and(|source|source.any()) || matches!(
            state.get(),
            VnState::Waiting | VnState::Stepping | VnState::Animating
        ) {
            for browser_input in events {
                let input = match browser_input.admit(&engine.0) {
                    Ok(Some(input))=>input, Ok(None)=>continue,
                    Err(problem)=>{error.0=problem;next.set(VnState::Error);return;}
                };
                screens.dispatch_source=Some(input.source);
                let mut browser_event=input.event;
                let modal = screens
                    .views
                    .iter()
                    .rposition(|view| view.modal)
                    .unwrap_or(0);
                // A preceding handler may already have closed or covered the
                // field. Discard only those now-stale browser events.
                if !screens.views.iter().skip(modal).any(|view| {
                    view.name == browser_event.screen && view.root.available(&browser_event.element)
                }) {
                    continue;
                }
                if browser_event
                    .key
                    .as_deref()
                    .is_some_and(|key| matches!(key, "__rvn_next_focus" | "__rvn_previous_focus"))
                {
                    let order: Vec<_> = screens
                        .views
                        .iter()
                        .skip(modal)
                        .flat_map(|view| {
                            view.root
                                .focus_order()
                                .into_iter()
                                .map(|id| (view.name.clone(), id))
                        })
                        .collect();
                    if order.is_empty() {
                        continue;
                    }
                    let current = order
                        .iter()
                        .position(|(screen, element)| {
                            *screen == browser_event.screen && *element == browser_event.element
                        })
                        .unwrap_or(0);
                    let index = if browser_event.key.as_deref() == Some("__rvn_previous_focus") {
                        (current + order.len() - 1) % order.len()
                    } else {
                        (current + 1) % order.len()
                    };
                    let Some(source) = screens.frame_sources.get(&order[index].0).cloned()
                        .filter(|source|engine.0.ui_input_source_current(source)) else {continue;};
                    screens.dispatch_source=Some(source);
                    browser_event = event(&order[index].0, &order[index].1, EventKind::Focus, None);
                }
                if !send(
                    &mut engine,
                    browser_event,
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                ) {
                    return;
                }
                screens.keyboard_consumed = true;
            }
            if crate::web_inputs::focused() {
                keyboard.clear();
                ime.clear();
                wheel.clear();
                screens.keyboard_consumed = true;
                if let Ok(mut window) = windows.get_single_mut() {
                    window.ime_enabled = false;
                }
                return;
            }
        }
    }
    if !source.as_deref().is_some_and(|source|source.any()) && !matches!(
        state.get(),
        VnState::Waiting | VnState::Stepping | VnState::Animating
    ) {
        keyboard.clear();
        ime.clear();
        wheel.clear();
        screens.dragging = None;
        if let Ok(mut window) = windows.get_single_mut() {
            window.ime_enabled = false;
        }
        return;
    }
    if widget_buttons1030::process(&mut screens,&mut engine,&controls,&windows,&mut output,&mut error,&mut next,presentation.as_deref(),button_layouts1030.as_deref()){return;}
    if let Some(pending)=screens.pending_press.take(){
        if engine.0.renderer.scene_batch_pending()||crate::editor_debugger::pending(&engine){screens.pending_press=Some(pending);return;}
        let current=controls.get(pending.entity).ok().is_some_and(|(_,control,_,_,_,source)|
            control.screen==pending.control.screen&&control.element==pending.control.element&&control.option==pending.control.option
            && source.is_some_and(|source|source.0==pending.source));
        let target=screens.views.iter().find(|view|view.name==pending.control.screen);
        let continued_drag=target.and_then(|view|view.root.find(&pending.control.element)).is_some_and(|component|component.kind==Kind::Slider);
        let semantic=target.and_then(|view|view.root.find(&pending.control.element))
            .and_then(|component|pointer_component_digest(component,true));
        let available=target.is_some_and(|view|view.root.available(&pending.control.element)
            && !screens.views.iter().any(|other|other.modal&&(other.layer,other.order)>(view.layer,view.order)));
        let image=pending.image.is_none_or(|(instance,epoch,_)|instance==engine.0.instance_id()&&epoch==engine.0.runtime_epoch());
        if current&&available&&image&&semantic==Some(pending.component)&&engine.0.ui_input_source_current(&pending.source){
            if continued_drag&&mouse.pressed(MouseButton::Left){screens.dragging=Some(Dragging{screen:pending.control.screen.clone(),element:pending.control.element.clone()});screens.dragging_source=Some(pending.source.clone());screens.dragging_preview=pending.pointer;}
            screens.dispatch_source=Some(pending.source);screens.pointer_consumed=true;
            let _=send(&mut engine,pending.action,&mut screens,&mut output,&mut error,&mut next);
            // The accepted press is one action. Its own publication must be
            // accepted before held dragging or any other input can continue.
            return;
        }
    }
    if screens.dragging.is_none(){screens.dragging_preview=None;}
    if let Some(drag) = screens.dragging.clone() {
        let Some(source) = screens.dragging_source.clone().filter(|source|engine.0.ui_input_source_current(source)) else {
            screens.dragging = None; screens.dragging_source = None; return;
        };
        screens.dispatch_source = Some(source);
        screens.pointer_consumed = true;
        if !mouse.pressed(MouseButton::Left) {
            screens.dragging = None;
        } else if let Some(view) = screens.views.iter().find(|view| view.name == drag.screen) {
            if let Some(component) = view
                .root
                .find(&drag.element)
                .cloned()
                .filter(|component| view.root.available(&component.id))
            {
                if let Some(cursor) = windows
                    .get_single()
                    .ok()
                    .and_then(|window| window.cursor_position())
                {
                    let Some((entity, control, _, node, transform, captured)) =
                        controls.iter().find(|(_, control, _, _, _, _)| {
                            control.option.is_none()
                                && control.screen == drag.screen
                                && control.element == drag.element
                        })
                    else {
                        screens.dragging = None;
                        return;
                    };
                    let pointer=preview.as_deref().or(screens.dragging_preview.as_ref());
                    let Some(fraction) = pointer.map_or_else(||slider_fraction(node.size(),transform,cursor),|preview|preview.slider_fraction(entity,control,captured,&component,cursor)) else {
                        screens.dragging = None;
                        return;
                    };
                    let value =
                        component.min + (component.max - component.min) * f64::from(fraction);
                    if component.value.as_f64() != Some(f64::from(value as f32)) {
                        if !send(
                            &mut engine,
                            event(
                                &drag.screen,
                                &drag.element,
                                EventKind::Change,
                                Some(Value::Float(value as f32)),
                            ),
                            &mut screens,
                            &mut output,
                            &mut error,
                            &mut next,
                        ) {
                            return;
                        }
                    }
                }
            } else {
                screens.dragging = None;
            }
        } else {
            screens.dragging = None;
        }
    }
    for (entity, control, interaction, node, transform, captured) in &controls {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(view) = screens
            .views
            .iter()
            .find(|view| view.name == control.screen)
        {
            if let Some(component) = view
                .root
                .find(&control.element)
                .filter(|component| component.kind == Kind::Canvas)
            {
                // Canvas input/capture has its own validated event path. Honor
                // explicit passthrough while disabled canvases still block hits.
                screens.pointer_consumed |=
                    component.consume_input || !view.root.available(&component.id);
                continue;
            }
        }
        screens.pointer_consumed = true;
        // Rebuilding a control while the pointer is held must not re-trigger
        // its click or toggle. A press is an edge, not a per-frame action.
        if !mouse.just_pressed(MouseButton::Left) {
            continue;
        }
        let Some(source) = control_source(&engine, &screens, control, captured) else { continue; };
        screens.dispatch_source = Some(source);
        let Some(view) = screens
            .views
            .iter()
            .find(|view| view.name == control.screen)
            .cloned()
        else {
            continue;
        };
        let Some(component) = view.root.find(&control.element).cloned() else {
            continue;
        };
        if !view.root.available(&component.id)
            || screens
                .views
                .iter()
                .any(|other| other.modal && (other.layer, other.order) > (view.layer, view.order))
        {
            continue;
        }
        if let Some(option) = &control.option {
            screens.dropdown = None;
            send(
                &mut engine,
                event(
                    &control.screen,
                    &control.element,
                    EventKind::Change,
                    Some(Value::Str(option.clone())),
                ),
                &mut screens,
                &mut output,
                &mut error,
                &mut next,
            );
            continue;
        }
        if widget_buttons1030::nominal(&engine,&view,&component){
            // Actual Widget presses/releases are consumed with their original
            // producer in the DownAndUp FSM, never this legacy press path.
            screens.pointer_consumed=true;continue;
        }
        let delayed_action=match component.kind {
            Kind::Button=>Some(event(&control.screen,&control.element,EventKind::Click,None)),
            Kind::Toggle=>Some(event(&control.screen,&control.element,EventKind::Change,Some(Value::Bool(!component.value.as_bool().unwrap_or(false))))),
            Kind::Slider=>windows.get_single().ok().and_then(|window|window.cursor_position()).and_then(|cursor|
                preview.as_ref().map_or_else(||slider_fraction(node.size(),transform,cursor),|preview|preview.slider_fraction(entity,control,captured,&component,cursor)))
                .map(|fraction|event(&control.screen,&control.element,EventKind::Change,Some(Value::Float((component.min+(component.max-component.min)*f64::from(fraction)) as f32)))),
            _=>None,
        };
        let delayed_component=pointer_component_digest(&component,true);
        if component.is_focusable() {
            if !send(
                &mut engine,
                event(&control.screen, &control.element, EventKind::Focus, None),
                &mut screens,
                &mut output,
                &mut error,
                &mut next,
            ) {
                return;
            }
        }
        if engine.0.renderer.scene_batch_pending()||crate::editor_debugger::pending(&engine){
            if let (Some(action),Some(component),Some(source))=(delayed_action,delayed_component,screens.dispatch_source.clone()){
                // Single pending press; ordinary input remains behind ready().
                // Its action is computed before Focus, using the image's slider
                // coordinates. The accepted publication cannot retarget it.
                if screens.pending_press.is_none(){screens.pending_press=Some(PendingPress{entity,control:control.clone(),source,component,
                    image:preview.as_ref().map(|preview|preview.image()),pointer:preview.as_ref().map(|preview|(**preview).clone()),action});}
                continue;
            }
        }
        if !screens
            .views
            .iter()
            .any(|view| view.name == control.screen && view.root.find(&control.element).is_some())
        {
            continue;
        }
        match component.kind {
            Kind::Input => {
                screens.editing_source = screens.dispatch_source.clone();
                screens.editing = Editing {
                    screen: control.screen.clone(),
                    element: control.element.clone(),
                    caret: component.value.as_str().unwrap_or_default().len(),
                    ..default()
                };
            }
            Kind::Select => {
                screens.dropdown_source = screens.dispatch_source.clone();
                screens.dropdown = Some(Dropdown {
                    screen: control.screen.clone(),
                    element: control.element.clone(),
                    options: component.options.clone(),
                    labels: component.option_labels.clone(),
                    highlighted: component
                        .options
                        .iter()
                        .position(|option| component.value.as_str() == Some(option))
                        .unwrap_or(0),
                    position: transform.translation().truncate()
                        + Vec2::new(-node.size().x * 0.5, node.size().y * 0.5),
                    width: node.size().x,
                });
            }
            Kind::Toggle => {
                send(
                    &mut engine,
                    event(
                        &control.screen,
                        &control.element,
                        EventKind::Change,
                        Some(Value::Bool(!component.value.as_bool().unwrap_or(false))),
                    ),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            }
            Kind::Slider => {
                screens.dragging_source = screens.dispatch_source.clone();
                screens.dragging_preview = preview.as_ref().map(|preview|(**preview).clone());
                screens.dragging = Some(Dragging {
                    screen: control.screen.clone(),
                    element: control.element.clone(),
                });
                if let Some(cursor) = windows
                    .get_single()
                    .ok()
                    .and_then(|window| window.cursor_position())
                {
                    let Some(fraction) = preview.as_ref().map_or_else(||slider_fraction(node.size(),transform,cursor),|preview|preview.slider_fraction(entity,control,captured,&component,cursor)) else {
                        continue;
                    };
                    let value = component.min + (component.max - component.min) * fraction as f64;
                    send(
                        &mut engine,
                        event(
                            &control.screen,
                            &control.element,
                            EventKind::Change,
                            Some(Value::Float(value as f32)),
                        ),
                        &mut screens,
                        &mut output,
                        &mut error,
                        &mut next,
                    );
                }
            }
            Kind::Button => {
                send(
                    &mut engine,
                    event(&control.screen, &control.element, EventKind::Click, None),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            }
            _ => {}
        }
    }
    if screens.dropdown.is_some()
        && mouse.just_pressed(MouseButton::Left)
        && !screens.pointer_consumed
    {
        screens.dropdown = None;
        screens.pointer_consumed = true;
    }
    screens.dispatch_source = None;
    if screens.input_snapshot_captured && screens.keyboard_focus != screens.input_focus {
        pressed.clear(); ime.clear();
    }
    let modal = screens
        .views
        .iter()
        .rposition(|view| view.modal)
        .unwrap_or(0);
    let focus_order: Vec<_> = screens
        .views
        .iter()
        .skip(modal)
        .flat_map(|view| {
            view.root
                .focus_order()
                .into_iter()
                .map(|id| (view.name.clone(), id))
        })
        .collect();
    if !screens
        .keyboard_focus
        .as_ref()
        .is_some_and(|focus| focus_order.contains(focus))
    {
        screens.keyboard_focus = screens.views.iter().skip(modal).rev().find_map(|view| {
            view.focus
                .as_ref()
                .filter(|id| view.root.focus_order().contains(id))
                .map(|id| (view.name.clone(), id.clone()))
        });
    }
    // Normalisation may choose a different field after the early guard.
    // A queued key/IME belongs to the captured focus, not that new fallback.
    if screens.input_snapshot_captured && screens.keyboard_focus != screens.input_focus {
        pressed.clear(); ime.clear(); wheel.clear(); return;
    }
    let view = screens
        .keyboard_focus
        .as_ref()
        .and_then(|(name, _)| screens.views.iter().find(|view| &view.name == name))
        .or_else(|| screens.views.last())
        .cloned();
    let Some(view) = view else {
        keyboard.clear();
        ime.clear();
        wheel.clear();
        if let Ok(mut window) = windows.get_single_mut() {
            window.ime_enabled = false;
        }
        return;
    };
    let Some(source) = screens.frame_sources.get(&view.name).cloned()
        .filter(|source|engine.0.ui_input_source_current(source)) else { ime.clear(); return; };
    screens.dispatch_source = Some(source);
    let focused = screens
        .keyboard_focus
        .as_ref()
        .and_then(|(_, id)| view.root.find(id))
        .cloned();
    // Preserve modifiers at the time of each key event. A complete short
    // chord can arrive between two frames, after ButtonInput has released
    // Shift/Ctrl already; using only its final state reverses Shift+Tab.
    let shift = pressed
        .iter()
        .find(|(key, _, _)| key.key_code == KeyCode::Tab)
        .map(|(_, _, shift)| *shift)
        .unwrap_or(keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
    for (key, _, _) in &pressed {
        let target = focused
            .as_ref()
            .filter(|component| {
                component.kind != Kind::Canvas && component.events.contains_key(&EventKind::Key)
            })
            .or_else(|| {
                (view.root.kind != Kind::Canvas && view.root.events.contains_key(&EventKind::Key))
                    .then_some(&view.root)
            });
        if let Some(target) = target {
            let mut input = event(&view.name, &target.id, EventKind::Key, None);
            input.key = Some(match &key.logical_key {
                Key::Character(text) => text.to_string(),
                other => format!("{other:?}"),
            });
            if !send(
                &mut engine,
                input,
                &mut screens,
                &mut output,
                &mut error,
                &mut next,
            ) {
                return;
            }
            screens.keyboard_consumed = true;
        }
    }
    if !screens.views.iter().any(|screen| screen.name == view.name) {
        ime.clear();
        return;
    }
    if native_just_pressed(KeyCode::Tab) && !focus_order.is_empty() {
        let current = screens
            .keyboard_focus
            .as_ref()
            .and_then(|focus| focus_order.iter().position(|other| other == focus));
        let index = current.map_or(if shift { focus_order.len() - 1 } else { 0 }, |current| {
            if shift {
                (current + focus_order.len() - 1) % focus_order.len()
            } else {
                (current + 1) % focus_order.len()
            }
        });
        if tab_producer.as_ref().and_then(|producer|producer.preview_focus_order.as_ref()).is_some_and(|captured|captured!=&focus_order || captured!=&preview_focus_order(&screens.views)){keyboard.clear();ime.clear();wheel.clear();return;}
        let (name, id) = &focus_order[index];
        if let Some(producer)=&tab_producer {
            if let Some(kinds)=&producer.preview_focus_kinds {
                let destination=(name.clone(),id.clone());
                let current=screens.views.iter().find(|view|view.name==*name).and_then(|view|view.root.find(id)).map(|component|component.kind);
                let Some(captured)=kinds.get(&destination)else{keyboard.clear();ime.clear();wheel.clear();return;};
                if current!=Some(*captured){keyboard.clear();ime.clear();wheel.clear();return;}
                if *captured==Kind::Canvas && engine.0.state.scene_execution.is_some() && !producer.preview_canvas_tokens.as_ref().and_then(|tokens|tokens.get(&destination)).is_some_and(|token|engine.0.state.scene_execution.as_ref().is_some_and(|execution|execution.resources.accepts(token,engine.0.runtime_epoch(),execution.owner()))) {keyboard.clear();ime.clear();wheel.clear();return;}
            }
        }
        // Traversal is intentional, but its destination authority must have
        // existed when this physical Tab was produced, before publication.
        let target_source=if captured_native {
            tab_producer.as_ref().and_then(|producer|producer.sources.get(name)).cloned()
        }else{screens.frame_sources.get(name).cloned()};
        let Some(target_source)=target_source.filter(|source|source.screen==*name && engine.0.ui_input_source_current(source))else{keyboard.clear();ime.clear();wheel.clear();return;};
        screens.dispatch_source=Some(target_source);
        if !send(
            &mut engine,
            event(name, id, EventKind::Focus, None),
            &mut screens,
            &mut output,
            &mut error,
            &mut next,
        ) {
            return;
        }
        screens.keyboard_focus = Some((name.clone(), id.clone()));
        screens.reveal_focus = Some((name.clone(), id.clone()));
        screens.dropdown = None;
        screens.keyboard_consumed = true;
        keyboard.clear();
        ime.clear();
        wheel.clear();
        return;
    }
    if let Some(mut menu) = screens.dropdown.clone() {
        let Some(source) = screens.dropdown_source.clone().filter(|source|engine.0.ui_input_source_current(source)) else {
            screens.dropdown = None; screens.dropdown_source = None; return;
        };
        screens.dispatch_source = Some(source);
        screens.keyboard_consumed = true;
        if !menu.options.is_empty() {
            for (scroll,id) in wheel.read_with_id() {
                if !input.accepts::<MouseWheel>(id.id){continue;}
                let delta = match scroll.unit {
                    MouseScrollUnit::Line => scroll.y,
                    MouseScrollUnit::Pixel => scroll.y / 36.0,
                };
                if delta != 0.0 {
                    menu.highlighted = (menu.highlighted as isize - delta.signum() as isize)
                        .clamp(0, menu.options.len() as isize - 1)
                        as usize;
                }
            }
            if native_just_pressed(KeyCode::ArrowDown) {
                menu.highlighted = (menu.highlighted + 1) % menu.options.len();
            }
            if native_just_pressed(KeyCode::ArrowUp) {
                menu.highlighted = (menu.highlighted + menu.options.len() - 1) % menu.options.len();
            }
            if native_just_pressed(KeyCode::Enter) {
                screens.dropdown = None;
                send(
                    &mut engine,
                    event(
                        &menu.screen,
                        &menu.element,
                        EventKind::Change,
                        Some(Value::Str(menu.options[menu.highlighted].clone())),
                    ),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            } else if native_just_pressed(KeyCode::Escape) {
                screens.dropdown = None;
            } else {
                screens.dropdown = Some(menu);
            }
        }
        keyboard.clear();
        ime.clear();
        return;
    }
    wheel.clear();
    if let Some(component) = focused.clone() {
        if component.kind == Kind::Input {
            if screens.editing.screen == view.name && screens.editing.element == component.id {
                if let Some(source) = screens.editing_source.clone() {
                    if !engine.0.ui_input_source_current(&source) { screens.editing = default(); screens.editing_source = None; ime.clear(); return; }
                    screens.dispatch_source = Some(source);
                }
            }
            screens.keyboard_consumed = true;
            let mut text = component.value.as_str().unwrap_or_default().to_owned();
            if screens.editing.screen != view.name || screens.editing.element != component.id {
                screens.editing_source = screens.dispatch_source.clone();
                screens.editing = Editing {
                    screen: view.name.clone(),
                    element: component.id.clone(),
                    caret: text.len(),
                    ..default()
                };
            }
            let mut changed = false;
            let ime_events: Vec<_> = ime.read_with_id()
                .filter(|(_,id)|input.accepts::<Ime>(id.id)&&queued_native_current(&screens,&engine,id.id,true))
                .map(|(event,_)|(*event).clone()).collect();
            if ime_events
                .iter()
                .any(|event| matches!(event, Ime::Commit { .. } | Ime::Preedit { .. }))
            {
                screens.editing.composing = true;
            }
            for (input, control, key_shift) in &pressed {
                changed |= edit_text(&mut text, &mut screens.editing, input, *control, *key_shift);
            }
            for event in &ime_events {
                match event {
                    Ime::Preedit { value, .. } => screens.editing.composing = !value.is_empty(),
                    Ime::Commit { value, .. } => {
                        replace_selection(&mut text, &mut screens.editing, value);
                        changed = true;
                        screens.editing.composing = false;
                    }
                    _ => {}
                }
            }
            if let Ok(mut window) = windows.get_single_mut() {
                window.ime_enabled = true;
            }
            if changed {
                send(
                    &mut engine,
                    event(
                        &view.name,
                        &component.id,
                        EventKind::Change,
                        Some(Value::Str(text)),
                    ),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            }
        } else {
            if let Ok(mut window) = windows.get_single_mut() {
                window.ime_enabled = false;
            }
            if component.kind == Kind::Slider
                && (native_just_pressed(KeyCode::ArrowLeft) || native_just_pressed(KeyCode::ArrowRight))
            {
                screens.keyboard_consumed = true;
                let direction = if native_just_pressed(KeyCode::ArrowLeft) {
                    -1.0
                } else {
                    1.0
                };
                let value = (component.value.as_f64().unwrap_or(component.min)
                    + direction * (component.max - component.min) / 100.0)
                    .clamp(component.min, component.max);
                send(
                    &mut engine,
                    event(
                        &view.name,
                        &component.id,
                        EventKind::Change,
                        Some(Value::Float(value as f32)),
                    ),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            }
            if component.kind == Kind::Select
                && (native_just_pressed(KeyCode::Enter)
                    || native_just_pressed(KeyCode::Space)
                    || native_just_pressed(KeyCode::ArrowDown))
            {
                screens.keyboard_consumed = true;
                if let Some((_, _, _, node, transform, _)) = controls.iter().find(|(_, control, _, _, _, _)| {
                    control.screen == view.name
                        && control.element == component.id
                        && control.option.is_none()
                }) {
                    screens.dropdown_source = screens.dispatch_source.clone();
                    screens.dropdown = Some(Dropdown {
                        screen: view.name.clone(),
                        element: component.id.clone(),
                        options: component.options.clone(),
                        labels: component.option_labels.clone(),
                        highlighted: component
                            .options
                            .iter()
                            .position(|option| component.value.as_str() == Some(option))
                            .unwrap_or(0),
                        position: transform.translation().truncate()
                            + Vec2::new(-node.size().x * 0.5, node.size().y * 0.5),
                        width: node.size().x,
                    });
                }
            } else if (native_just_pressed(KeyCode::Enter) || native_just_pressed(KeyCode::Space))
                && (component.kind != Kind::Canvas
                    || component.events.contains_key(&EventKind::Activate))
            {
                screens.keyboard_consumed = true;
                let kind = if component.kind == Kind::Toggle {
                    EventKind::Change
                } else {
                    EventKind::Activate
                };
                let value = if component.kind == Kind::Toggle {
                    Some(Value::Bool(!component.value.as_bool().unwrap_or(false)))
                } else {
                    None
                };
                send(
                    &mut engine,
                    event(&view.name, &component.id, kind, value),
                    &mut screens,
                    &mut output,
                    &mut error,
                    &mut next,
                );
            }
            keyboard.clear();
            ime.clear();
        }
    } else {
        keyboard.clear();
        ime.clear();
        if let Ok(mut window) = windows.get_single_mut() {
            window.ime_enabled = false;
        }
    }
    if native_just_pressed(KeyCode::Escape)
        || native_just_pressed(KeyCode::F5)
        || native_just_pressed(KeyCode::F6)
    {
        if !view.root.events.contains_key(&EventKind::Key)
            && !focused.as_ref().is_some_and(|component| {
                component.kind != Kind::Canvas && component.events.contains_key(&EventKind::Key)
            })
        {
            screens.keyboard_consumed = false;
        }
    }
}

pub(crate) fn key_chords(
    events: impl Iterator<Item = KeyboardInput>,
    modifiers: &mut [bool; 4],
) -> Vec<(KeyboardInput, bool, bool)> {
    let mut pressed = Vec::new();
    for event in events {
        let index = match event.key_code {
            KeyCode::ControlLeft => Some(0),
            KeyCode::ControlRight => Some(1),
            KeyCode::ShiftLeft => Some(2),
            KeyCode::ShiftRight => Some(3),
            _ => None,
        };
        if let Some(index) = index {
            modifiers[index] = event.state == ButtonState::Pressed;
        }
        if event.state == ButtonState::Pressed {
            pressed.push((
                event,
                modifiers[0] || modifiers[1],
                modifiers[2] || modifiers[3],
            ));
        }
    }
    pressed
}

#[cfg(test)]
mod chord_tests {
    use super::*;
    #[test]
    fn loading_the_same_screen_order_retires_old_controls_and_focus_before_redraw() {
        let source="screen HUD(){return component(\"act\",\"button\",{},[])}\ninit{ui.open(\"HUD\",[],false,0)}\nscene label P\n\"waiting\"\n";
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
        game.step_until_interaction().unwrap();
        let views=game.interface_views().unwrap();let old_order=views[0].order;
        let installed=game.renderer.scene_batch.take().unwrap();
        let save=rvn_core::save::SaveData::try_from_state(&game.state,1,"waiting".into(),"main.rvn".into()).unwrap();
        let mut world=World::new();world.insert_resource(VnEngine(game));world.init_resource::<Screens>();
        world.insert_resource(crate::scene_publication::ScenePresentation{batch:Some(*installed)});
        {let mut screens=world.resource_mut::<Screens>();screens.views=views.clone();screens.rendered=views.clone();screens.keyboard_focus=Some(("HUD".into(),"act".into()));}
        let entity=world.spawn(Control{screen:"HUD".into(),element:"act".into(),option:None}).id();
        install_scene(&mut world,views);
        assert!(world.get_entity(entity).is_some());assert!(world.resource::<Screens>().keyboard_focus.is_some());
        world.resource_mut::<VnEngine>().0.load_data(save).unwrap();
        let restored=world.resource::<VnEngine>().0.interface_views().unwrap();assert_eq!(restored[0].order,old_order);
        install_scene(&mut world,restored);
        assert!(world.get_entity(entity).is_none());assert!(world.resource::<Screens>().keyboard_focus.is_none());
        assert!(world.resource::<Screens>().rendered.is_empty());assert_eq!(world.resource::<Screens>().views[0].order,old_order);
    }
    fn key(code: KeyCode, state: ButtonState) -> KeyboardInput {
        KeyboardInput {
            key_code: code,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            window: Entity::PLACEHOLDER,
        }
    }
    #[test]
    fn short_chords_use_modifiers_at_the_key_press_not_after_release() {
        let mut held = [false; 4];
        let events = vec![
            key(KeyCode::ShiftLeft, ButtonState::Pressed),
            key(KeyCode::Tab, ButtonState::Pressed),
            key(KeyCode::Tab, ButtonState::Released),
            key(KeyCode::ShiftLeft, ButtonState::Released),
        ];
        let pressed = key_chords(events.into_iter(), &mut held);
        assert!(
            pressed
                .iter()
                .find(|(key, _, _)| key.key_code == KeyCode::Tab)
                .unwrap()
                .2
        );
        assert_eq!(held, [false; 4]);
        let events = vec![
            key(KeyCode::ControlLeft, ButtonState::Pressed),
            key(KeyCode::KeyA, ButtonState::Pressed),
            key(KeyCode::ControlLeft, ButtonState::Released),
        ];
        assert!(
            key_chords(events.into_iter(), &mut held)
                .iter()
                .find(|(key, _, _)| key.key_code == KeyCode::KeyA)
                .unwrap()
                .1
        );
    }
    #[test]
    fn a_modifier_held_across_frames_is_preserved() {
        let mut held = [false; 4];
        key_chords(
            vec![key(KeyCode::ShiftRight, ButtonState::Pressed)].into_iter(),
            &mut held,
        );
        assert!(
            key_chords(
                vec![key(KeyCode::Tab, ButtonState::Pressed)].into_iter(),
                &mut held
            )[0]
            .2
        );
        key_chords(
            vec![key(KeyCode::ShiftRight, ButtonState::Released)].into_iter(),
            &mut held,
        );
        assert_eq!(held, [false; 4]);
    }
    #[test]
    fn a_later_modifier_press_does_not_rewrite_an_earlier_key() {
        let mut held = [false; 4];
        let events = vec![
            key(KeyCode::Tab, ButtonState::Pressed),
            key(KeyCode::ShiftLeft, ButtonState::Pressed),
        ];
        assert!(!key_chords(events.into_iter(), &mut held)[0].2);
    }
}

fn replace_selection(text: &mut String, editing: &mut Editing, inserted: &str) {
    editing.caret = editing.caret.min(text.len());
    while !text.is_char_boundary(editing.caret) {
        editing.caret -= 1;
    }
    let anchor = editing
        .anchor
        .take()
        .unwrap_or(editing.caret)
        .min(text.len());
    let mut start = anchor.min(editing.caret);
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let end = anchor.max(editing.caret);
    if !text.is_char_boundary(end) {
        return;
    }
    text.replace_range(start..end, inserted);
    editing.caret = start + inserted.len();
}

fn edit_text(
    text: &mut String,
    editing: &mut Editing,
    input: &KeyboardInput,
    control: bool,
    shift: bool,
) -> bool {
    let mut caret = editing.caret.min(text.len());
    while !text.is_char_boundary(caret) {
        caret -= 1;
    }
    editing.caret = caret;
    let previous = text[..caret]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0);
    let following = text[caret..]
        .chars()
        .next()
        .map(|c| caret + c.len_utf8())
        .unwrap_or(text.len());
    let move_to = match input.key_code {
        KeyCode::ArrowLeft => Some(previous),
        KeyCode::ArrowRight => Some(following),
        KeyCode::Home => Some(0),
        KeyCode::End => Some(text.len()),
        _ => None,
    };
    if let Some(next) = move_to {
        if shift && editing.anchor.is_none() {
            editing.anchor = Some(caret);
        }
        if !shift {
            editing.anchor = None;
        }
        editing.caret = next;
        return false;
    }
    if control && input.key_code == KeyCode::KeyA {
        editing.anchor = Some(0);
        editing.caret = text.len();
        return false;
    }
    if input.key_code == KeyCode::Backspace || input.key_code == KeyCode::Delete {
        if editing.anchor.is_none() {
            editing.anchor = Some(if input.key_code == KeyCode::Backspace {
                previous
            } else {
                following
            });
        }
        replace_selection(text, editing, "");
        return true;
    }
    if !control && !editing.composing {
        if let Key::Character(value) = &input.logical_key {
            if !value.chars().any(char::is_control) {
                replace_selection(text, editing, value);
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_native_budget_preflights_borrowed_focus_pairs_before_maps_are_cloned(){
        let (mut app,_,_)=pointer_app();
        let before=serde_json::to_vec(&app.world().resource::<VnEngine>().0.state).unwrap();
        let source=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        {let mut screens=app.world_mut().resource_mut::<Screens>();screens.rendered=screens.views.clone();screens.rendered_sources.insert("form".into(),source);}
        assert!(capture_preview_native(app.world()).unwrap().current(app.world()));
        let mut view=app.world().resource::<Screens>().rendered[0].clone();
        view.name="s".repeat(128);view.root.children=(0..511).map(|index|{let mut component=view.root.children[0].clone();component.id=format!("{index:04}{}","x".repeat(124));component}).collect();
        {let mut screens=app.world_mut().resource_mut::<Screens>();screens.rendered=(0..16).map(|index|{let mut item=view.clone();item.name.replace_range(..4,&format!("{index:04}"));item}).collect();}
        assert!(preview_native_cost(app.world()).is_none());assert!(capture_preview_native(app.world()).is_none());
        assert_eq!(app.world().resource::<Screens>().rendered.len(),16);assert_eq!(serde_json::to_vec(&app.world().resource::<VnEngine>().0.state).unwrap(),before);
    }

    use bevy::input::mouse::{mouse_button_input_system, MouseButtonInput};

    // A windowless schedule retains real Bevy mouse/keyboard event reduction,
    // both focus passes and the actual RVN input dispatcher. Geometry is a
    // completed-layout fixture; no Interaction or handler result is injected.
    fn pointer_app() -> (App, Entity, Vec<Entity>) {
        let script=rvn_parser::parse(r#"
handler clicked(event) { set clicks = clicks + 1 }
screen form() {
    return {"id":"root","kind":"panel","children":[
        {"id":"button","kind":"button","enabled":enabled,"events":{"click":"clicked","activate":"clicked"}},
        {"id":"toggle","kind":"toggle","binding":"toggled"},
        {"id":"slider","kind":"slider","binding":"level","min":0,"max":1},
        {"id":"name","kind":"input","binding":"name"}
    ]}
}
init { set clicks=0 set enabled=true set toggled=false set level=0 set name="Alix" }
label start
    ui.open("form",[],false,0)
    "Waiting"
"#).unwrap();
        let mut engine =
            rvn_core::Engine::new(script, crate::bevy_renderer::BevyRenderer::new(), 32).unwrap();
        engine.step_until_interaction().unwrap();
        let mut screens = Screens {
            active: true,
            ..default()
        };
        update_views(&mut screens, engine.interface_views().unwrap());
        let mut app = App::new();
        app.insert_resource(VnEngine(engine))
            .insert_resource(screens)
            .insert_resource(State::new(VnState::Waiting))
            .init_resource::<NextState<VnState>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<bevy::input::touch::Touches>()
            .init_resource::<bevy::ui::UiScale>()
            .init_resource::<bevy::ui::UiStack>()
            .init_resource::<ScriptErrorMessage>()
            .init_resource::<crate::accessibility::Accessibility>()
            .add_event::<MouseButtonInput>()
            .add_event::<KeyboardInput>()
            .add_event::<bevy::input::keyboard::KeyboardFocusLost>()
            .add_event::<Ime>()
            .add_event::<MouseWheel>()
            .add_event::<VnCommand>()
            .add_systems(
                PreUpdate,
                (
                    mouse_button_input_system,
                    bevy::input::keyboard::keyboard_input_system,
                    bevy::ui::ui_focus_system,
                    transformed_focus,
                )
                    .chain(),
            )
            .add_systems(Update, input);
        let window = app
            .world_mut()
            .spawn((Window::default(), bevy::window::PrimaryWindow))
            .id();
        let camera = app
            .world_mut()
            .spawn((Camera::default(), GlobalTransform::default()))
            .id();
        let mut controls = Vec::new();
        for (index, id) in ["button", "toggle", "slider", "name"]
            .into_iter()
            .enumerate()
        {
            controls.push(pointer_node(
                app.world_mut(),
                Some(id),
                camera,
                Vec2::new(100.0, 80.0 + index as f32 * 80.0),
                Vec2::new(120.0, 40.0),
            ));
        }
        app.world_mut().resource_mut::<bevy::ui::UiStack>().uinodes = controls.clone();
        (app, window, controls)
    }

    fn story_menu_pointer_app()->(App,Entity) {
        let source=r#"
handler open_inventory(event){ui.open_story("form",[],false,60)}
handler bad(event){set clicks=clicks+1 local roll=random(1,10) ui.focus("form","name") menu.execute({"kind":"bool_preference","key":"fullscreen","value":1})}
handler bad_constructor(event){set clicks=clicks+1 menu.execute(menu_bool("fullscreen",1))}
handler good(event){set clicks=clicks+1 menu.execute(menu_action("none"))}
handler general_fault(event){local missing=unknown_scenario_variable}
screen host(){return component("open","button",{"events":{"click":"open_inventory"}},[])}
screen form(){return component("root","panel",{},[
    component("button","button",{"events":{"click":"bad"}},[]),
    component("toggle","button",{"events":{"click":"good"}},[]),
    component("slider","button",{"events":{"click":"bad_constructor"}},[]),
    component("name","input",{"binding":"name"},[]),
    component("general","button",{"events":{"click":"general_fault"}},[])
])}
init{set clicks=0 set name="Alix"}
label start
"Waiting"
"#;
        let mut engine=rvn_core::Engine::new(rvn_parser::parse(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        engine.step_until_interaction().unwrap();
        engine.synchronize_source_menu(rvn_ui::PageRole::QuickActions,Some("host"),Value::Dict(Default::default()),false,100).unwrap();
        engine.interface_event(event("host","open",EventKind::Click,None)).unwrap();
        engine.synchronize_source_menu(rvn_ui::PageRole::QuickActions,None,Value::Dict(Default::default()),false,0).unwrap();
        assert_eq!(engine.state.ui.screens.len(),1);assert_eq!(engine.state.ui.screens[0].host_role,None);
        engine.renderer.menu_authority.story_ui_active=true;engine.renderer.menu_authority.game_active=true;engine.renderer.menu_authority.waiting=true;
        engine.renderer.take_pending();
        let views=engine.interface_views().unwrap();let(mut app,window,controls)=pointer_app();
        let camera=app.world().get::<bevy::ui::TargetCamera>(controls[0]).unwrap().entity();
        let extra=pointer_node(app.world_mut(),Some("general"),camera,Vec2::new(100.0,400.0),Vec2::new(120.0,40.0));
        app.world_mut().resource_mut::<bevy::ui::UiStack>().uinodes.push(extra);
        app.insert_resource(VnEngine(engine));update_views(&mut app.world_mut().resource_mut::<Screens>(),views);
        (app,window)
    }

    #[test]
    fn narrative_menu_rejections_preserve_globals_random_controls_disk_and_next_real_click() {
        let(mut app,window)=story_menu_pointer_app();
        let directory=std::env::temp_dir().join(format!("rvn-narrative-menu-rejection-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let manager=rvn_core::save::SaveManager::new(&directory,10).unwrap();
        app.world().resource::<VnEngine>().0.save(&manager,1,"Preserved".into(),"qa.rvn".into()).unwrap();
        let disk=||std::fs::read_dir(&directory).unwrap().map(|entry|{let entry=entry.unwrap();(entry.file_name(),std::fs::read(entry.path()).unwrap())}).collect::<std::collections::BTreeMap<_,_>>();
        let before_disk=disk();let before=app.world().resource::<VnEngine>().0.state.clone();
        let controls=|state:&rvn_core::GameState|serde_json::json!(state.ui.screens.iter().map(|screen|(&screen.name,&screen.values,&screen.canvas_states)).collect::<Vec<_>>());
        for y in [80.0,240.0] {
            pointer_move(&mut app,window,Vec2::new(100.0,y));pointer_edge(&mut app,window,ButtonState::Pressed);pointer_edge(&mut app,window,ButtonState::Released);app.update();
            let engine=&app.world().resource::<VnEngine>().0;
            assert_eq!(engine.state.vars,before.vars);assert_eq!(engine.state.random,before.random);assert_eq!(engine.state.display_random,before.display_random);
            assert_eq!(controls(&engine.state),controls(&before));assert_eq!(disk(),before_disk);
            assert!(matches!(app.world().resource::<NextState<VnState>>(),NextState::Unchanged));
            assert!(!app.world().resource::<ScriptErrorMessage>().0.is_empty());assert!(!engine.renderer.menu_pending);
        }
        pointer_move(&mut app,window,Vec2::new(100.0,160.0));pointer_edge(&mut app,window,ButtonState::Pressed);pointer_edge(&mut app,window,ButtonState::Released);app.update();
        assert_eq!(test_var(&app,"clicks"),Value::Int(1));assert!(app.world().resource::<VnEngine>().0.renderer.menu_pending);
        assert!(app.world().resource::<Events<VnCommand>>().iter_current_update_events().any(|event|matches!(event,VnCommand::SourceMenu(receipt)if receipt.effect.host_role.is_none()&&receipt.effect.screen=="form")));
        assert_eq!(disk(),before_disk);std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn general_fault_in_an_ordinary_interface_keeps_the_fatal_story_error_path() {
        let(mut app,window)=story_menu_pointer_app();
        pointer_move(&mut app,window,Vec2::new(100.0,400.0));pointer_edge(&mut app,window,ButtonState::Pressed);pointer_edge(&mut app,window,ButtonState::Released);app.update();
        assert!(matches!(app.world().resource::<NextState<VnState>>(),NextState::Pending(VnState::Error)));
        assert!(app.world().resource::<ScriptErrorMessage>().0.contains("unknown_scenario_variable"));
    }

    fn pointer_node(
        world: &mut World,
        id: Option<&str>,
        camera: Entity,
        center: Vec2,
        size: Vec2,
    ) -> Entity {
        let mut node = Node::default();
        let bevy::reflect::ReflectMut::Struct(fields) = node.reflect_mut() else {
            unreachable!()
        };
        fields.field_mut("calculated_size").unwrap().apply(&size);
        let mut visible = ViewVisibility::default();
        visible.set();
        let entity = world
            .spawn((
                node,
                GlobalTransform::from_translation(center.extend(0.0)),
                visible,
                FocusPolicy::Block,
                Interaction::None,
                bevy::ui::TargetCamera(camera),
            ))
            .id();
        if let Some(id) = id {
            world.entity_mut(entity).insert(Control {
                screen: "form".into(),
                element: id.into(),
                option: None,
            });
        }
        entity
    }

    fn pointer_move(app: &mut App, window: Entity, point: Vec2) {
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(point));
    }
    fn pointer_edge(app: &mut App, window: Entity, state: ButtonState) {
        app.world_mut().send_event(MouseButtonInput {
            window,
            button: MouseButton::Left,
            state,
        });
    }
    fn short_click(app: &mut App, window: Entity, point: Vec2) {
        pointer_move(app, window, point);
        pointer_edge(app, window, ButtonState::Pressed);
        pointer_edge(app, window, ButtonState::Released);
        app.update();
        assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }
    fn test_var(app: &App, name: &str) -> Value {
        app.world().resource::<VnEngine>().0.state.vars[name].clone()
    }
    fn key_edge(app: &mut App, window: Entity, code: KeyCode, logical: Key) {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            app.world_mut().send_event(KeyboardInput {
                window,
                key_code: code,
                logical_key: logical.clone(),
                state,
            });
        }
        app.update();
    }

    #[test]
    fn authored_screen_short_shift_tab_preserves_both_directions_and_modifier_event_order() {
        use ButtonState::{Pressed, Released};
        fn edges(app: &mut App, window: Entity, sequence: &[(KeyCode, ButtonState)]) {
            for &(code, state) in sequence {
                app.world_mut().send_event(KeyboardInput {
                    key_code: code,
                    logical_key: if code == KeyCode::Tab { Key::Tab } else { Key::Shift },
                    state,
                    window,
                });
            }
            app.update();
            assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
        }
        fn focused(app: &App) -> Option<(&str, &str)> {
            app.world().resource::<Screens>().keyboard_focus.as_ref()
                .map(|(screen, element)| (screen.as_str(), element.as_str()))
        }
        for shift in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
            let (mut app, window, _) = pointer_app();
            edges(&mut app, window, &[(shift, Pressed), (KeyCode::Tab, Pressed),
                (KeyCode::Tab, Released), (shift, Released)]);
            assert_eq!(focused(&app), Some(("form", "name")));
            assert!(!app.world().resource::<ButtonInput<KeyCode>>().pressed(shift));
            edges(&mut app, window, &[(KeyCode::Tab, Pressed), (KeyCode::Tab, Released)]);
            assert_eq!(focused(&app), Some(("form", "button")));
            edges(&mut app, window, &[(shift, Pressed), (KeyCode::Tab, Pressed),
                (KeyCode::Tab, Released), (shift, Released)]);
            assert_eq!(focused(&app), Some(("form", "name")));
            edges(&mut app, window, &[(shift, Pressed), (shift, Released),
                (KeyCode::Tab, Pressed), (KeyCode::Tab, Released)]);
            assert_eq!(focused(&app), Some(("form", "button")));
            edges(&mut app, window, &[(KeyCode::Tab, Pressed), (shift, Pressed),
                (KeyCode::Tab, Released), (shift, Released)]);
            assert_eq!(focused(&app), Some(("form", "toggle")));
            edges(&mut app, window, &[(shift, Pressed)]);
            edges(&mut app, window, &[(KeyCode::Tab, Pressed), (KeyCode::Tab, Released)]);
            assert_eq!(focused(&app), Some(("form", "button")));
            edges(&mut app, window, &[(shift, Released), (KeyCode::Tab, Pressed),
                (KeyCode::Tab, Released)]);
            assert_eq!(focused(&app), Some(("form", "toggle")));
        }
    }

    #[test]
    fn same_frame_mouse_down_up_activates_once_through_real_focus_and_input_schedule() {
        let (mut app, window, controls) = pointer_app();
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        let mouse = app.world().resource::<ButtonInput<MouseButton>>();
        assert!(!mouse.pressed(MouseButton::Left));
        assert!(mouse.just_pressed(MouseButton::Left) && mouse.just_released(MouseButton::Left));
        assert_eq!(
            test_var(&app, "clicks"),
            Value::Int(1),
            "A complete short click between frames must reach its actual RVN handler"
        );
        app.update();
        assert_eq!(test_var(&app, "clicks"), Value::Int(1));
        assert_eq!(
            *app.world().get::<Interaction>(controls[0]).unwrap(),
            Interaction::Hovered
        );
        pointer_edge(&mut app, window, ButtonState::Pressed);
        app.update();
        app.update();
        assert_eq!(
            test_var(&app, "clicks"),
            Value::Int(2),
            "Holding the pointer across frames must not repeat a click"
        );
        pointer_move(&mut app, window, Vec2::new(100.0, 160.0));
        pointer_edge(&mut app, window, ButtonState::Released);
        app.update();
        assert_eq!(
            test_var(&app, "toggled"),
            Value::Bool(false),
            "Releasing a prior press over another control must not activate that control"
        );
        short_click(&mut app, window, Vec2::new(100.0, 160.0));
        assert_eq!(test_var(&app, "toggled"), Value::Bool(true));
        app.update();
        assert_eq!(test_var(&app, "toggled"), Value::Bool(true));
        short_click(&mut app, window, Vec2::new(130.0, 240.0));
        assert_eq!(test_var(&app, "level"), Value::Float(0.75));
        pointer_move(&mut app, window, Vec2::new(45.0, 240.0));
        app.update();
        assert_eq!(
            test_var(&app, "level"),
            Value::Float(0.75),
            "A released short slider click must not retain a drag"
        );
    }

    #[test]
    fn short_click_respects_disabled_blocked_hidden_and_modal_controls() {
        let (mut app, window, controls) = pointer_app();
        app.world_mut()
            .resource_mut::<VnEngine>()
            .0
            .state
            .vars
            .insert("enabled".into(), Value::Bool(false));
        let views = app
            .world()
            .resource::<VnEngine>()
            .0
            .interface_views()
            .unwrap();
        update_views(&mut app.world_mut().resource_mut::<Screens>(), views);
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        assert_eq!(test_var(&app, "clicks"), Value::Int(0));
        app.world_mut()
            .resource_mut::<crate::accessibility::Accessibility>()
            .blocked = true;
        short_click(&mut app, window, Vec2::new(100.0, 160.0));
        assert_eq!(test_var(&app, "toggled"), Value::Bool(false));
        app.world_mut()
            .resource_mut::<crate::accessibility::Accessibility>()
            .blocked = false;
        *app.world_mut()
            .get_mut::<ViewVisibility>(controls[1])
            .unwrap() = ViewVisibility::default();
        short_click(&mut app, window, Vec2::new(100.0, 160.0));
        assert_eq!(test_var(&app, "toggled"), Value::Bool(false));
        app.world_mut()
            .get_mut::<ViewVisibility>(controls[1])
            .unwrap()
            .set();
        let camera = app
            .world()
            .get::<bevy::ui::TargetCamera>(controls[1])
            .unwrap()
            .entity();
        let overlay = pointer_node(
            app.world_mut(),
            None,
            camera,
            Vec2::new(100.0, 160.0),
            Vec2::new(120.0, 40.0),
        );
        app.world_mut()
            .resource_mut::<bevy::ui::UiStack>()
            .uinodes
            .push(overlay);
        short_click(&mut app, window, Vec2::new(100.0, 160.0));
        assert_eq!(
            test_var(&app, "toggled"),
            Value::Bool(false),
            "A blocking overlay must absorb the short click"
        );
        app.world_mut()
            .resource_mut::<bevy::ui::UiStack>()
            .uinodes
            .pop();
        let root = UiComponent::parse(serde_json::json!({"id":"cover","kind":"panel"})).unwrap();
        app.world_mut()
            .resource_mut::<Screens>()
            .views
            .push(ScreenView {
                name: "modal".into(),
                modal: true,
                layer: 10,
                order: 10,
                focus: None,
                root,
            });
        short_click(&mut app, window, Vec2::new(100.0, 160.0));
        assert_eq!(
            test_var(&app, "toggled"),
            Value::Bool(false),
            "A higher authored modal must block underlying controls"
        );
    }

    #[test]
    fn short_mouse_click_preserves_keyboard_activation_tab_unicode_and_ime() {
        let (mut app, window, _) = pointer_app();
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        key_edge(&mut app, window, KeyCode::Enter, Key::Enter);
        assert_eq!(test_var(&app, "clicks"), Value::Int(2));
        key_edge(&mut app, window, KeyCode::Tab, Key::Tab);
        assert_eq!(
            app.world().resource::<Screens>().keyboard_focus,
            Some(("form".into(), "toggle".into()))
        );
        short_click(&mut app, window, Vec2::new(100.0, 320.0));
        key_edge(&mut app, window, KeyCode::KeyQ, Key::Character("q".into()));
        assert_eq!(test_var(&app, "name"), Value::Str("Alixq".into()));
        app.world_mut().send_event(Ime::Commit {
            window,
            value: "é".into(),
        });
        app.update();
        assert_eq!(test_var(&app, "name"), Value::Str("Alixqé".into()));
    }

    #[test]
    fn legacy_title_choice_and_pause_menu_keep_bevys_complete_short_click() {
        let (mut app, window, controls) = pointer_app();
        app.world_mut()
            .entity_mut(controls[0])
            .remove::<Control>()
            .insert((Button, crate::components::ChoiceButton(3)));
        app.init_resource::<crate::menu_documents::Menus>()
            .init_resource::<crate::resources::ChoiceFocus>()
            .add_event::<crate::vn_command::PlayerInput>()
            .add_systems(
                Update,
                crate::systems::choice_interaction_system.after(input),
            );
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        let mut reader = app
            .world()
            .resource::<Events<crate::vn_command::PlayerInput>>()
            .get_reader();
        let events = app
            .world()
            .resource::<Events<crate::vn_command::PlayerInput>>();
        assert_eq!(
            reader
                .read(events)
                .filter(|event| matches!(event, crate::vn_command::PlayerInput::Choose(3)))
                .count(),
            1
        );
        app.update();
        assert_eq!(
            reader
                .read(
                    app.world()
                        .resource::<Events<crate::vn_command::PlayerInput>>()
                )
                .count(),
            0
        );

        let (mut app, window, controls) = pointer_app();
        app.world_mut()
            .entity_mut(controls[0])
            .remove::<Control>()
            .insert((
                Button,
                crate::systems::menu::MenuButton::Save,
                BackgroundColor(Color::NONE),
            ));
        app.init_resource::<crate::resources::MenuState>()
            .init_resource::<crate::systems::save_menu::SaveMenuState>()
            .init_resource::<crate::systems::settings_menu::SettingsMenuState>()
            .add_event::<bevy::app::AppExit>()
            .add_systems(Update, crate::systems::menu_interaction_system.after(input));
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        assert!(
            app.world()
                .resource::<crate::systems::save_menu::SaveMenuState>()
                .active
        );
        app.update();
        assert_eq!(
            *app.world().get::<Interaction>(controls[0]).unwrap(),
            Interaction::Hovered
        );

        let (mut app, window, controls) = pointer_app();
        app.world_mut()
            .entity_mut(controls[0])
            .remove::<Control>()
            .insert((
                Button,
                crate::systems::title::TitleButton::Settings,
                crate::systems::title::TitleButtonColors {
                    normal: Color::NONE,
                    hover: Color::NONE,
                    pressed: Color::NONE,
                },
                BackgroundColor(Color::NONE),
            ));
        let empty = std::path::PathBuf::new();
        app.insert_resource(crate::project_paths::ProjectPaths::new(
            empty.clone(),
            empty.clone(),
            empty.clone(),
            empty.clone(),
            empty,
        ))
        .insert_resource(crate::resources::PersistentDataResource {
            // Settings activation neither reads nor writes this manager.
            manager: rvn_core::PersistentDataManager::new(std::env::temp_dir()).unwrap(),
            data: rvn_core::PersistentData::default(),
        })
        .init_resource::<crate::resources::VnRenderState>()
        .init_resource::<crate::resources::ImagemapState>()
        .init_resource::<crate::resources::TypewriterState>()
        .init_resource::<crate::resources::DialogueHistory>()
        .init_resource::<crate::resources::MenuState>()
        .init_resource::<crate::systems::save_menu::SaveMenuState>()
        .init_resource::<crate::systems::settings_menu::SettingsMenuState>()
        .add_event::<bevy::app::AppExit>()
        .add_systems(
            Update,
            crate::systems::title_interaction_system.after(input),
        );
        short_click(&mut app, window, Vec2::new(100.0, 80.0));
        assert!(
            app.world()
                .resource::<crate::systems::settings_menu::SettingsMenuState>()
                .active
        );
        assert_eq!(
            app.world()
                .resource::<crate::resources::MenuState>()
                .return_to,
            Some(VnState::TitleScreen)
        );
    }

    #[test]
    fn canvas_frames_refresh_without_rebuilding_host_widgets() {
        use super::*;
        use rvn_ui::custom_canvas::{CanvasDrawing, CanvasFrame, CanvasPrimitive};
        let mut component = UiComponent::parse(
            serde_json::json!({"kind":"canvas","id":"canvas","width":300,"height":180}),
        )
        .unwrap();
        component.canvas_frame = Some(CanvasFrame {
            width: 300.0,
            height: 180.0,
            time: 0.0,
        });
        let first = vec![ScreenView {
            name: "example".into(),
            modal: false,
            layer: 0,
            order: 0,
            focus: None,
            root: component,
        }];
        let mut second = first.clone();
        second[0].root.canvas_frame.as_mut().unwrap().time = 2.0;
        second[0].root.drawing = Some(CanvasDrawing {
            primitives: vec![CanvasPrimitive::Rect {
                rect: [0.0, 0.0, 10.0, 10.0],
                color: [1.0; 4],
                radius: 0.0,
            }],
        });
        assert!(same_host_views(&first, &second));
        second[0].root.canvas_frame.as_mut().unwrap().width = 400.0;
        assert!(!same_host_views(&first, &second));
        second = first.clone();
        second[0].root.enabled = false;
        assert!(!same_host_views(&first, &second));
        second = first.clone();
        second[0].root.font_size = 42.0;
        assert!(!same_host_views(&first, &second));
    }
    #[test]
    fn authored_small_fonts_follow_reference_scale_without_an_arbitrary_readability_floor() {
        use super::*;
        let component = UiComponent::parse(
            serde_json::json!({"id":"tiny","kind":"input","font_size":6,"value":"Small"}),
        )
        .unwrap();
        assert_eq!(
            component_font_size(component.font_size, 720.0 / 1080.0),
            4.0
        );
        assert_eq!(component_font_size(component.font_size, 1.0), 6.0);
        assert_eq!(component_font_size(component.font_size, 0.1), 1.0);
        assert_eq!(component_font_size(24.0, 720.0 / 1080.0), 16.0);
    }
    #[test]
    fn ui_shader_fix_is_exact_idempotent_and_rejects_unknown_dependencies() {
        use super::*;
        let old="// Geometry and flags unchanged\nfn antialias(distance:f32)->f32{return clamp(0.0, 1.0, 0.5 - 2.0 * distance);}";
        let corrected = corrected_ui_antialias(old).unwrap().unwrap();
        assert_eq!(
            corrected,
            old.replace(
                "clamp(0.0, 1.0, 0.5 - 2.0 * distance)",
                "clamp(0.5 - 2.0 * distance, 0.0, 1.0)"
            )
        );
        assert_eq!(corrected_ui_antialias(&corrected).unwrap(), None);
        assert!(corrected_ui_antialias("unrecognized shader").is_err());
        assert!(corrected_ui_antialias(&format!("{old}\n{old}")).is_err());
        assert!(corrected_ui_antialias(&format!("{corrected}\n{corrected}")).is_err());
        for (distance, alpha) in [(-1.0f32, 1.0f32), (0.0, 0.5), (1.0, 0.0)] {
            assert_eq!((0.5 - 2.0 * distance).clamp(0.0, 1.0), alpha);
        }
    }
    #[test]
    fn designer_and_runtime_share_nested_geometry_and_keep_scroll_content_reachable() {
        use super::*;
        let root=UiComponent::parse(serde_json::json!({"id":"root","kind":"column","rect":[100,100,300,240],"padding":10,"children":[{"id":"nested","kind":"panel","width":80,"height":60,"children":[{"id":"caption","kind":"text","rect":[4,6,20,10]}]}]})).unwrap();
        let view = ScreenView {
            name: "form".into(),
            modal: false,
            layer: 0,
            order: 0,
            focus: None,
            root,
        };
        let resolved = layout_view(&view);
        let rects = layout_rects(&view.root, [1920.0, 1080.0]).unwrap();
        let nested = rects.iter().find(|item| item.id == "nested").unwrap().rect;
        assert_eq!(resolved.root.rect, Some([100.0, 100.0, 300.0, 240.0]));
        assert_eq!(
            resolved.root.find("caption").unwrap().rect,
            Some([4.0, 6.0, 20.0, 10.0])
        );
        assert_eq!(
            resolved.root.find("nested").unwrap().rect,
            Some([nested[0] - 100.0, nested[1] - 100.0, nested[2], nested[3]])
        );
        let root=UiComponent::parse(serde_json::json!({"id":"root","kind":"column","width":300,"children":[{"id":"long","kind":"text","height":3000}]})).unwrap();
        let view = ScreenView { root, ..view };
        let resolved = layout_view(&view);
        assert_eq!(resolved.root.scroll, ScrollAxis::Both);
        assert_eq!(resolved.root.rect.unwrap()[3], 1048.0);
        assert_eq!(resolved.root.find("long").unwrap().rect.unwrap()[3], 3000.0);
    }

    #[test]
    fn accessibility_text_size_reflows_auto_controls_without_scaling_authored_fonts_twice() {
        use super::*;
        let root=UiComponent::parse(serde_json::json!({"id":"root","kind":"column","children":[{"id":"caption","kind":"text","text":"Title"},{"id":"action","kind":"button","text":"Continue"}]})).unwrap();
        let view = ScreenView {
            name: "form".into(),
            modal: false,
            layer: 0,
            order: 0,
            focus: None,
            root,
        };
        let normal = layout_view_with_text_scale(&view, 1.0);
        let enlarged = layout_view_with_text_scale(&view, 2.0);
        assert!(
            enlarged.root.find("action").unwrap().rect.unwrap()[3]
                > normal.root.find("action").unwrap().rect.unwrap()[3]
        );
        assert_eq!(enlarged.root.find("action").unwrap().font_size, 24.0);
    }

    #[test]
    fn custom_feedback_colors_include_inherited_opacity_and_visible_focus() {
        use super::*;
        let root=UiComponent::parse(serde_json::json!({"id":"root","kind":"column","opacity":0.5,"children":[{"id":"action","kind":"button","opacity":0.5,"hover_background":[0.8,0.2,0.1,0.6],"focus_color":[0.1,0.9,0.2,1]}]})).unwrap();
        let mut app = App::new();
        app.init_resource::<Screens>().add_systems(Update, feedback);
        app.world_mut()
            .resource_mut::<Screens>()
            .views
            .push(ScreenView {
                name: "form".into(),
                modal: false,
                layer: 0,
                order: 0,
                focus: Some("action".into()),
                root,
            });
        let entity = app
            .world_mut()
            .spawn((
                Control {
                    screen: "form".into(),
                    element: "action".into(),
                    option: None,
                },
                Interaction::Hovered,
                BackgroundColor(Color::NONE),
                BorderColor(Color::NONE),
            ))
            .id();
        app.update();
        let background = app
            .world()
            .get::<BackgroundColor>(entity)
            .unwrap()
            .0
            .to_srgba();
        let border = app.world().get::<BorderColor>(entity).unwrap().0.to_srgba();
        assert!((background.red - 0.8).abs() < 0.001);
        assert!((background.alpha - 0.15).abs() < 0.001);
        assert!((border.green - 0.9).abs() < 0.001);
        assert!((border.alpha - 0.25).abs() < 0.001);
    }

    #[test]
    fn assistive_actions_validate_values_and_match_pointer_events() {
        use super::*;
        use bevy::a11y::accesskit::{Action, ActionData, ActionRequest, NodeId};
        let request = |action, data| ActionRequest {
            target: NodeId(1),
            action,
            data,
        };
        let component = |kind: &str, extra: serde_json::Value| {
            let mut value = serde_json::json!({"id":"control","kind":kind});
            value
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            serde_json::from_value::<UiComponent>(value).unwrap()
        };
        let button = component("button", serde_json::json!({}));
        assert_eq!(
            assistive_event("inventory", &button, &request(Action::Default, None))
                .unwrap()
                .kind,
            EventKind::Activate
        );
        let toggle = component("toggle", serde_json::json!({"value":true}));
        assert_eq!(
            assistive_event("inventory", &toggle, &request(Action::Default, None))
                .unwrap()
                .value,
            Some(Value::Bool(false))
        );
        let select = component(
            "select",
            serde_json::json!({"options":["letter","key"],"value":"letter"}),
        );
        assert!(assistive_event(
            "inventory",
            &select,
            &request(Action::SetValue, Some(ActionData::Value("missing".into())))
        )
        .is_none());
        assert_eq!(
            assistive_event(
                "inventory",
                &select,
                &request(Action::SetValue, Some(ActionData::Value("key".into())))
            )
            .unwrap()
            .value,
            Some(Value::Str("key".into()))
        );
        let slider = component("slider", serde_json::json!({"min":0,"max":1,"value":0.5}));
        assert_eq!(
            assistive_event(
                "inventory",
                &slider,
                &request(Action::SetValue, Some(ActionData::NumericValue(50.0)))
            )
            .unwrap()
            .value,
            Some(Value::Float(1.0))
        );
        assert!(assistive_event(
            "inventory",
            &slider,
            &request(Action::SetValue, Some(ActionData::NumericValue(f64::NAN)))
        )
        .is_none());
        let input = component("input", serde_json::json!({"value":""}));
        for invalid in ["\0".to_owned(), "x".repeat(65_537)] {
            assert!(assistive_event(
                "inventory",
                &input,
                &request(
                    Action::SetValue,
                    Some(ActionData::Value(invalid.into_boxed_str()))
                )
            )
            .is_none());
        }
        assert_eq!(
            assistive_event(
                "inventory",
                &input,
                &request(Action::SetValue, Some(ActionData::Value("Éloïse".into())))
            )
            .unwrap()
            .value,
            Some(Value::Str("Éloïse".into()))
        );
        let canvas = component("canvas", serde_json::json!({}));
        assert!(assistive_event("inventory", &canvas, &request(Action::Default, None)).is_none());
        assert!(assistive_event(
            "inventory",
            &canvas,
            &request(Action::SetValue, Some(ActionData::Value("ignored".into())))
        )
        .is_none());
        let canvas = component(
            "canvas",
            serde_json::json!({"events":{"activate":"custom_activate"}}),
        );
        assert_eq!(
            assistive_event("inventory", &canvas, &request(Action::Default, None))
                .unwrap()
                .kind,
            EventKind::Activate
        );
    }
    #[test]
    fn animated_hit_testing_and_slider_drag_use_the_rendered_affine_transform() {
        use super::*;
        let transform = GlobalTransform::from(Transform {
            translation: Vec3::new(300.0, 200.0, 0.0),
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            scale: Vec3::new(2.0, 0.5, 1.0),
        });
        let size = Vec2::new(100.0, 40.0);
        assert!(transformed_contains(
            size,
            &transform,
            Vec2::new(300.0, 270.0)
        ));
        assert!(!transformed_contains(
            size,
            &transform,
            Vec2::new(320.0, 200.0)
        ));
        assert!(
            (slider_fraction(size, &transform, Vec2::new(300.0, 250.0)).unwrap() - 0.75).abs()
                < 0.001
        );
        assert!(local_pointer(
            &GlobalTransform::from(Transform::from_scale(Vec3::ZERO)),
            Vec2::ZERO
        )
        .is_none());
    }
    #[test]
    fn text_selection_preserves_unicode_boundaries() {
        let mut text = "Éloïse".to_owned();
        let mut editing = Editing {
            caret: 2,
            anchor: Some(0),
            ..default()
        };
        replace_selection(&mut text, &mut editing, "Å");
        assert_eq!(text, "Åloïse");
        assert_eq!(editing.caret, 2);
    }
}

#[cfg(test)]
mod scoped_ui_input_contracts {
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn completed_image_modern_pending_press_refuses_changed_component_and_focus_loss(){
        for focus_loss in [false,true] {
            let (mut app,_,mut controls)=fixture(&source(false,false));
            app.add_event::<bevy::window::WindowFocused>();
            prepare_modern_pointer_publication(&mut app,&mut controls);
            let frame=completed_native_image(app.world(),1);
            crate::editor_preview_input::inject(app.world_mut(),frame.clone(),rvn_core::editor_preview::ImageInput::Pointer{kind:rvn_core::editor_preview::PointerKind::Down,button:0,point:[100.0,80.0],wheel:[0.0;2]}).unwrap();app.update();
            assert_eq!(test_var(&app,"focuses"),Value::Int(1));assert_eq!(test_var(&app,"clicks"),Value::Int(0));
            consume_completed_owner_publication_for_test(&mut app);
            let captured=app.world().get::<ControlSource>(controls[0]).unwrap().0.clone();
            assert!(app.world().resource::<VnEngine>().0.ui_input_source_current(&captured));
            if focus_loss {crate::editor_preview_input::inject(app.world_mut(),frame,rvn_core::editor_preview::ImageInput::FocusLost).unwrap();}
            else {app.world_mut().resource_mut::<Screens>().views[0].root.visit_mut(&mut |component|if component.id=="button"{component.max=2.0;});}
            let before=frozen(&app);app.update();assert_eq!(frozen(&app),before);
            assert_eq!(test_var(&app,"clicks"),Value::Int(0));assert!(app.world().resource::<Screens>().pending_press.is_none());
        }
    }
    #[test]
    fn completed_image_passive_release_during_focus_publication_cancels_without_a_handler(){
        for focus_loss in [false,true] {
            let (mut app,_,mut controls)=fixture(&source(false,false));
            app.add_event::<bevy::window::WindowFocused>();prepare_modern_pointer_publication(&mut app,&mut controls);
            let frame=completed_native_image(app.world(),1);
            let down=rvn_core::editor_preview::ImageInput::Pointer{kind:rvn_core::editor_preview::PointerKind::Down,button:0,point:[100.0,80.0],wheel:[0.0;2]};
            crate::editor_preview::drain_images_for_test(app.world_mut(),frame.clone(),vec![(1,down)]);app.update();
            assert_eq!(test_var(&app,"focuses"),Value::Int(1));assert_eq!(test_var(&app,"clicks"),Value::Int(0));
            assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());assert!(app.world().resource::<Screens>().pending_press.is_some());
            let before=frozen(&app);let release=if focus_loss {rvn_core::editor_preview::ImageInput::FocusLost}
                else {rvn_core::editor_preview::ImageInput::Pointer{kind:rvn_core::editor_preview::PointerKind::Up,button:0,point:[100.0,80.0],wheel:[0.0;2]}};
            crate::editor_preview::drain_images_for_test(app.world_mut(),frame,vec![(2,release)]);
            assert_eq!(frozen(&app),before);assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
            assert!(app.world().resource::<Screens>().pending_press.is_none());assert!(!app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
            consume_completed_owner_publication_for_test(&mut app);app.update();assert_eq!(test_var(&app,"clicks"),Value::Int(0));assert_eq!(test_var(&app,"focuses"),Value::Int(1));
            assert!(!app.world().resource::<ButtonInput<MouseButton>>().just_pressed(MouseButton::Left));
        }
    }
    #[test]
    fn completed_image_focus_loss_discards_down_before_the_actual_input_system_represses_it(){
        let (mut app,_,mut controls)=fixture(&source(false,false));app.add_event::<bevy::window::WindowFocused>();prepare_modern_pointer_publication(&mut app,&mut controls);
        let frame=completed_native_image(app.world(),1);let before=frozen(&app);
        crate::editor_preview::drain_images_for_test(app.world_mut(),frame,vec![
            (1,rvn_core::editor_preview::ImageInput::Pointer{kind:rvn_core::editor_preview::PointerKind::Down,button:0,point:[100.0,80.0],wheel:[0.0;2]}),
            (2,rvn_core::editor_preview::ImageInput::FocusLost)]);
        app.update();assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"focuses"),Value::Int(0));assert_eq!(test_var(&app,"clicks"),Value::Int(0));
        assert!(!app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));assert!(!app.world().resource::<ButtonInput<MouseButton>>().just_pressed(MouseButton::Left));
        assert!(app.world().resource::<Screens>().pending_press.is_none());assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
    }
    /// A completed image must own the actual installed owner projection as
    /// well as its child controls. Use the same production camera metrics,
    /// geometry preparation and backend installation as the normal pipeline.
    pub(super) fn install_completed_owner_geometry_for_test(world:&mut World){
        let engine=&world.resource::<VnEngine>().0;
        if engine.renderer.scene_batch_pending()||engine.renderer.scene_frame_failed{return;}
        let instance=engine.instance_id();
        let Some(batch)=world.get_resource::<crate::scene_publication::ScenePresentation>().and_then(|value|value.batch.clone())else{return;};
        // Refusal fixtures intentionally retain an old installed batch after
        // replacing the Engine without accepting its new publication. No
        // current physical owner may be reconstructed from that stale image.
        if batch.runtime_epoch!=engine.runtime_epoch()||batch.interface_epoch!=engine.interface_epoch()||batch.state.ui!=engine.state.ui{return;}
        world.init_resource::<Assets<Image>>();world.init_resource::<bevy::render::camera::ManualTextureViews>();
        world.init_resource::<Events<bevy::window::WindowResized>>();world.init_resource::<Events<bevy::window::WindowCreated>>();
        world.init_resource::<Events<bevy::window::WindowScaleFactorChanged>>();world.init_resource::<Events<bevy::asset::AssetEvent<Image>>>();
        let camera=world.iter_entities().find(|entity|entity.contains::<Camera>()).unwrap().id();
        if world.get::<OrthographicProjection>(camera).is_none(){world.entity_mut(camera).insert(Camera2dBundle::default());}
        world.run_system_once(bevy::render::camera::camera_system::<OrthographicProjection>);
        let primary=world.iter_entities().find(|entity|entity.contains::<bevy::window::PrimaryWindow>()).unwrap().id();
        let camera_value=world.get::<Camera>(camera).unwrap();let target=camera_value.target.normalize(Some(primary)).unwrap();
        let window=world.get::<Window>(primary).unwrap();
        let info=target.get_render_target_info(std::iter::once((primary,window)),world.resource::<Assets<Image>>(),world.resource::<bevy::render::camera::ManualTextureViews>()).unwrap();
        let metric=actual_widget_camera_metrics(camera,camera_value,world.resource::<bevy::ui::UiScale>().0,&world.resource::<crate::accessibility::Accessibility>().settings,target,info).unwrap();
        let projection=owner_hosts_0943::prepare_scene(&batch,instance).unwrap();
        let plan=owner_hosts_0943::prepare_geometry(world,projection,metric,&HashMap::new()).unwrap();
        owner_hosts_0943::install_preflighted(world,&batch,plan).unwrap();
        assert!(owner_hosts_0943::capture_installed(world).is_some(),"the fixture must have a real installed owner fence before producing a complete image");
    }
    pub(super) fn consume_completed_owner_publication_for_test(app:&mut App){
        crate::scene_publication::consume(app.world_mut());
        install_completed_owner_geometry_for_test(app.world_mut());
    }
    fn prepare_modern_pointer_publication(app:&mut App,controls:&mut Vec<Entity>) {
        app.init_resource::<crate::scene_publication::ScenePresentation>().init_resource::<crate::resources::VnRenderState>()
            .init_resource::<crate::resources::ChoiceFocus>().init_resource::<crate::resources::ImagemapState>().init_resource::<crate::resources::TypewriterState>();
        // A real unhandled Key produces an initial complete Core batch without
        // invoking Focus or changing a binding. Accept it through the real
        // publication consumer before the fixture's physical controls are used.
        app.world_mut().resource_mut::<VnEngine>().0.interface_event(event("form","slider",EventKind::Key,None)).unwrap();
        consume_completed_owner_publication_for_test(app);
        app.add_systems(PostUpdate,install_completed_owner_geometry_for_test);
        let engine=&app.world().resource::<VnEngine>().0;
        assert!(!engine.renderer.scene_frame_failed);assert!(!engine.renderer.scene_batch_pending());
        let source=engine.capture_ui_input_source("form").unwrap();
        // The first accepted publication retires controls constructed before
        // any installed scene. Build the physical fixture only after that
        // boundary; subsequent Focus publications must retain these entities.
        assert!(controls.iter().all(|entity|app.world().get_entity(*entity).is_none()));
        let camera=app.world().iter_entities().find(|entity|entity.contains::<Camera>()).unwrap().id();
        *controls=["button","toggle","slider","name"].into_iter().enumerate().map(|(index,id)|
            pointer_node(app.world_mut(),Some(id),camera,Vec2::new(100.0,80.0+index as f32*80.0),Vec2::new(120.0,40.0))).collect();
        app.world_mut().resource_mut::<bevy::ui::UiStack>().uinodes=controls.clone();
        for &entity in controls.iter() {app.world_mut().entity_mut(entity).insert(ControlSource(source.clone()));}
        app.world_mut().resource_mut::<Screens>().rendered_sources.insert("form".into(),source);
        publish_canvas_input_tokens_for_test(app.world_mut());
    }
    #[test]
    fn completed_image_modern_button_continues_once_after_real_focus_publication_and_refuses_replacement(){
        for replace in [false,true] {
            let (mut app,_,mut controls)=fixture(&source(false,false));prepare_modern_pointer_publication(&mut app,&mut controls);
            let frame=completed_native_image(app.world(),1);
            crate::editor_preview_input::inject(app.world_mut(),frame,rvn_core::editor_preview::ImageInput::Pointer{kind:rvn_core::editor_preview::PointerKind::Down,button:0,point:[100.0,80.0],wheel:[0.0;2]}).unwrap();app.update();
            assert_eq!(test_var(&app,"focuses"),Value::Int(1));assert_eq!(test_var(&app,"clicks"),Value::Int(0));
            assert!(app.world().resource::<Screens>().pending_press.is_some());assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
            if replace {reopen(&mut app);}
            consume_completed_owner_publication_for_test(&mut app);
            assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
            let before=frozen(&app);app.update();
            if replace {assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"clicks"),Value::Int(0));}
            else {assert_eq!(test_var(&app,"clicks"),Value::Int(1));consume_completed_owner_publication_for_test(&mut app);app.update();assert_eq!(test_var(&app,"clicks"),Value::Int(1));}
            assert!(app.world().resource::<Screens>().pending_press.is_none());
        }
    }
    #[test]
    fn completed_image_modern_slider_continues_its_old_coordinates_and_drag_without_refocusing(){
        let (mut app,_,mut controls)=fixture(&source(false,false));
        app.add_event::<bevy::window::CursorMoved>();
        prepare_modern_pointer_publication(&mut app,&mut controls);
        let frame=completed_native_image(app.world(),1);
        app.world_mut().entity_mut(controls[2]).insert(GlobalTransform::from_translation(Vec3::new(800.0,500.0,0.0)));
        crate::editor_preview_input::inject(app.world_mut(),frame.clone(),rvn_core::editor_preview::ImageInput::Pointer{kind:rvn_core::editor_preview::PointerKind::Down,button:0,point:[130.0,240.0],wheel:[0.0;2]}).unwrap();app.update();
        assert_eq!(test_var(&app,"level"),Value::Int(0));assert!(app.world().resource::<Screens>().pending_press.is_some());
        consume_completed_owner_publication_for_test(&mut app);
        crate::editor_preview_input::begin(app.world_mut());
        assert!(!app.world().contains_resource::<crate::editor_preview_input::PointerOverride>());
        app.update();assert_eq!(test_var(&app,"level"),Value::Float(0.75));
        assert!(app.world().resource::<Screens>().dragging.is_some());consume_completed_owner_publication_for_test(&mut app);
        app.world_mut().remove_resource::<crate::editor_preview_input::Routing>();
        crate::editor_preview_input::inject(app.world_mut(),frame.clone(),rvn_core::editor_preview::ImageInput::Pointer{kind:rvn_core::editor_preview::PointerKind::Move,button:0,point:[70.0,240.0],wheel:[0.0;2]}).unwrap();app.update();
        assert_eq!(test_var(&app,"level"),Value::Float(0.25));consume_completed_owner_publication_for_test(&mut app);
        crate::editor_preview_input::inject(app.world_mut(),frame,rvn_core::editor_preview::ImageInput::Pointer{kind:rvn_core::editor_preview::PointerKind::Up,button:0,point:[70.0,240.0],wheel:[0.0;2]}).unwrap();app.update();
        assert!(app.world().resource::<Screens>().dragging.is_none());
    }
    #[test]
    fn focus_and_slider_value_publications_preserve_entity_while_actual_label_and_fill_update(){
        let (mut app,_,controls)=fixture(&source(false,false));publish_canvas_input_tokens_for_test(app.world_mut());
        let before=app.world().resource::<Screens>().views.clone();let mut after=before.clone();after[0].focus=Some("slider".into());after[0].root.visit_mut(&mut |node|if node.id=="slider"{node.value=serde_json::json!(0.75);});
        assert!(same_host_views(&before,&after));
        let label=app.world_mut().spawn((SliderLabel(controls[2]),Text::from_section("old",TextStyle::default()))).id();
        let fill=app.world_mut().spawn((SliderFill(controls[2]),Style::default())).id();
        app.world_mut().resource_mut::<Screens>().views=after;
        app.world_mut().run_system_once(slider_values);
        assert_eq!(app.world().get::<Text>(label).unwrap().sections[0].value,"  75 %");
        assert_eq!(app.world().get::<Style>(fill).unwrap().width,Val::Percent(75.0));assert!(app.world().get::<Control>(controls[2]).is_some());
        let mut replaced=before.clone();replaced[0].root.visit_mut(&mut |node|if node.id=="slider"{node.max=2.0;});assert!(!same_host_views(&before,&replaced));
    }
    #[test]
    fn completed_image_modern_native_focus_keeps_click_closed_until_publication_is_accepted() {
        let (mut app,_,_)=fixture(&source(false,false));publish_canvas_input_tokens_for_test(app.world_mut());
        let source=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        let frame=completed_native_image(app.world(),1);
        crate::editor_preview_input::inject(app.world_mut(),frame,rvn_core::editor_preview::ImageInput::Pointer{kind:rvn_core::editor_preview::PointerKind::Down,button:0,point:[100.0,80.0],wheel:[0.0;2]}).unwrap();app.update();
        assert_eq!(test_var(&app,"focuses"),Value::Int(1));assert_eq!(test_var(&app,"clicks"),Value::Int(0));
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert!(app.world().resource::<VnEngine>().0.ui_input_source_current(&source));
        assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn completed_image_native_pointer_keeps_animated_button_but_refuses_a_rebuilt_entity() {
        for replace in [false,true] {
            let (mut app,_,controls)=fixture(&source(false,true));
            publish_canvas_input_tokens_for_test(app.world_mut());
            let frame=completed_native_image(app.world(),1);
            if replace {
                let old=controls[0];let control=app.world().get::<Control>(old).unwrap().clone();let source=app.world().get::<ControlSource>(old).unwrap().clone();
                app.world_mut().despawn(old);
                let camera=app.world().iter_entities().find(|entity|entity.contains::<Camera>()).unwrap().id();
                let fresh=pointer_node(app.world_mut(),Some("button"),camera,Vec2::new(100.0,80.0),Vec2::new(120.0,40.0));
                app.world_mut().entity_mut(fresh).insert((control,source));
                app.world_mut().resource_mut::<bevy::ui::UiStack>().uinodes[0]=fresh;
            }else{app.world_mut().entity_mut(controls[0]).insert(GlobalTransform::from_translation(Vec3::new(900.0,500.0,0.0)));}
            let before=frozen(&app);
            let result=crate::editor_preview_input::inject(app.world_mut(),frame,rvn_core::editor_preview::ImageInput::Pointer{kind:rvn_core::editor_preview::PointerKind::Down,button:0,point:[100.0,80.0],wheel:[0.0;2]});
            if replace {assert!(result.is_err());assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"clicks"),Value::Int(0));}
            else {result.unwrap();app.update();assert_eq!(test_var(&app,"clicks"),Value::Int(1));}
        }
    }
    #[test]
    fn completed_image_native_slider_reads_its_captured_scale_and_pivot() {
        let (mut app,_,controls)=fixture(&source(false,true));publish_canvas_input_tokens_for_test(app.world_mut());
        let frame=completed_native_image(app.world(),1);
        app.world_mut().entity_mut(controls[2]).insert(GlobalTransform::from(Transform{translation:Vec3::new(700.0,500.0,0.0),scale:Vec3::new(3.0,0.5,1.0),rotation:Quat::from_rotation_z(0.7)}));
        crate::editor_preview_input::inject(app.world_mut(),frame,rvn_core::editor_preview::ImageInput::Pointer{kind:rvn_core::editor_preview::PointerKind::Down,button:0,point:[130.0,240.0],wheel:[0.0;2]}).unwrap();app.update();
        assert_eq!(test_var(&app,"level"),Value::Float(0.75));
        assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }
    pub(super) fn completed_native_image(world:&World,sequence:u64)->std::sync::Arc<crate::editor_preview_input::PhysicalFrame>{
        let engine=&world.resource::<VnEngine>().0;
        let scope=rvn_core::editor_preview::FrameScope{instance:engine.instance_id(),epoch:engine.runtime_epoch(),sequence,control_sequence:0,control_ack:0,control_error:None,paused:false,publication_pending:false, finished: None,step_allowed:false,interaction:engine.preview_input_source(),position:rvn_core::scene::ResumePosition{pc:engine.state.pc,origin:None},active:None,parked:vec![]};
        crate::editor_preview_input::capture(world,scope).unwrap()
    }
    #[test]
    fn completed_image_native_ime_keeps_exact_recipient_and_refuses_reopened_interface() {
        let (mut app,window,mut controls)=fixture(&source(false,false));
        prepare_modern_pointer_publication(&mut app,&mut controls);
        app.world_mut().resource_mut::<Screens>().keyboard_focus=Some(("form".into(),"name".into()));
        publish_canvas_input_tokens_for_test(app.world_mut());
        let old=capture_preview_native(app.world()).unwrap();assert!(old.current(app.world()));
        let physical=completed_native_image(app.world(),1);
        crate::editor_preview_input::inject(app.world_mut(),physical,rvn_core::editor_preview::ImageInput::Text{value:" é🌟".into(),composition:None}).unwrap();app.update();
        assert_eq!(test_var(&app,"name"),Value::Str("original é🌟".into()));
        // Text produced a real new Core image. A new completed physical frame
        // must wait for its actual publication and owner geometry installation.
        // Preserve this same IME recipient through that scalar value refresh.
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert!(owner_hosts_0943::capture_installed(app.world()).is_none());
        let editing_source=app.world().resource::<Screens>().editing_source.clone().expect("actual first text owns its editing source");
        consume_completed_owner_publication_for_test(&mut app);
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        assert_eq!(app.world().resource::<Screens>().editing_source.as_ref(),Some(&editing_source));
        publish_canvas_input_tokens_for_test(app.world_mut());
        let stale=capture_preview_native(app.world()).unwrap();let old_physical=completed_native_image(app.world(),2);
        // Keep the actual replacement publication pending. The standalone
        // repaint helper deliberately discards it and cannot retire Editing.
        app.world_mut().resource_mut::<VnEngine>().0.interface_event(event("form","renew",EventKind::Click,None)).unwrap();
        assert!(app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        let before=frozen(&app);let history=format!("{:?}",app.world().resource::<VnEngine>().0.history.entries());
        assert!(!stale.current(app.world()));
        app.world_mut().remove_resource::<crate::editor_preview_input::Routing>();
        assert!(crate::editor_preview_input::inject(app.world_mut(),old_physical,rvn_core::editor_preview::ImageInput::Text{value:" forbidden".into(),composition:None}).is_err());app.update();
        assert_eq!(frozen(&app),before);assert_eq!(format!("{:?}",app.world().resource::<VnEngine>().0.history.entries()),history);
        consume_completed_owner_publication_for_test(&mut app);
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        let cap=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        {let mut screens=app.world_mut().resource_mut::<Screens>();
            assert!(screens.editing_source.is_none());
            screens.rendered_sources.insert("form".into(),cap);
            screens.keyboard_focus=Some(("form".into(),"name".into()));}
        publish_canvas_input_tokens_for_test(app.world_mut());let fresh=capture_preview_native(app.world()).unwrap();
        assert!(fresh.current(app.world()));app.world_mut().remove_resource::<crate::editor_preview_input::Routing>();
        let physical=completed_native_image(app.world(),3);
        crate::editor_preview_input::inject(app.world_mut(),physical,rvn_core::editor_preview::ImageInput::Text{value:" fresh".into(),composition:None}).unwrap();app.update();
        assert_eq!(test_var(&app,"name"),Value::Str("original é🌟 fresh".into()));
    }
    #[test]
    fn completed_image_native_tab_captures_both_interfaces_and_keeps_the_original_destination() {
        for replace in [false,true] {
            let other=r#"
handler entered_other(event){set other_focuses=other_focuses+1}
handler renew_other(event){ui.close("other") ui.open("other",[],false,0)}
screen other(){return component("other_root","panel",{},[component("outside","button",{"events":{"focus":"entered_other"}},[]),component("reopen","button",{"events":{"click":"renew_other"}},[])])}
"#;
            let base=source(false,false).replace("{\"events\":{\"key\":\"keyed\"}}","{}")
                .replacen("init {","init {set other_focuses=0 ",1)
                .replacen("\n\"parent\"","\nui.open(\"other\",[],false,0)\n\"parent\"",1);
            let (mut app,window,_)=fixture(&format!("{other}\n{base}"));
            let cap=app.world().resource::<VnEngine>().0.capture_ui_input_source("other").unwrap();
            {let mut screens=app.world_mut().resource_mut::<Screens>();screens.keyboard_focus=Some(("form".into(),"select".into()));screens.rendered_sources.insert("other".into(),cap.clone());}
            publish_canvas_input_tokens_for_test(app.world_mut());let frame=capture_preview_native(app.world()).unwrap();
            if replace {app.world_mut().resource_mut::<VnEngine>().0.interface_event(event("other","reopen",EventKind::Click,None)).unwrap();repaint_state(&mut app,true);}
            let before=frozen(&app);
            for (state,edge) in [(ButtonState::Pressed,true),(ButtonState::Released,false)] {frame.keyboard(app.world_mut(),KeyboardInput{key_code:KeyCode::Tab,logical_key:Key::Tab,state,window},edge);}
            app.update();
            if replace {assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"other_focuses"),Value::Int(0));assert_eq!(app.world().resource::<Screens>().keyboard_focus,Some(("form".into(),"select".into())));}
            else {assert_eq!(test_var(&app,"other_focuses"),Value::Int(1));assert_eq!(app.world().resource::<Screens>().keyboard_focus,Some(("other".into(),"outside".into())));}
            assert_eq!(frame.0.sources.get("other"),Some(&cap));assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
        }
    }

    use super::*;
    use bevy::input::mouse::{MouseButtonInput,mouse_button_input_system};
    #[test]
    fn debugger_completed_image_button_retains_its_producer_through_focus_stop_and_publishes_each_callback_once() {
        let (mut app,_,mut controls)=fixture(&source(false,false));
        prepare_modern_pointer_publication(&mut app,&mut controls);
        app.init_resource::<Time<bevy::time::Virtual>>().init_resource::<crate::editor_preview::PreviewState>();
        // Exercise the production input admission, rather than dispatching a
        // second event manually while the real Focus publication is pending.
        app.world_mut().resource_mut::<bevy::ecs::schedule::Schedules>().remove(Update).unwrap();
        app.add_systems(First,crate::editor_debugger::drain)
            .add_systems(Update,(assistive_input.run_if(crate::scene_publication::ready),input.run_if(crate::scene_publication::ready)).chain());
        let breakpoint=app.world().resource::<VnEngine>().0.debug_site_page(0).unwrap().entries.into_iter()
            .find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.function.as_deref()==Some("handler:focused"))).unwrap().breakpoint;
        crate::editor_debugger::configure(app.world_mut(),vec![breakpoint],vec![]).unwrap();
        let captured=app.world().get::<ControlSource>(controls[0]).unwrap().0.clone();
        let frame=completed_native_image(app.world(),1);
        let before=frozen(&app);let history=format!("{:?}",app.world().resource::<VnEngine>().0.history.entries());
        crate::editor_preview_input::inject(app.world_mut(),frame.clone(),rvn_core::editor_preview::ImageInput::Pointer{
            kind:rvn_core::editor_preview::PointerKind::Down,button:0,point:[100.0,80.0],wheel:[0.0;2]}).unwrap();
        app.update();
        assert!(crate::editor_debugger::pending(app.world().resource::<VnEngine>()));
        let press=app.world().resource::<Screens>().pending_press.as_ref().unwrap();
        assert_eq!(press.entity,controls[0]);assert_eq!(press.source,captured);
        for _ in 0..8 {
            app.update();
            if crate::editor_debugger::snapshot(app.world().resource::<VnEngine>()).is_some_and(|snapshot|
                snapshot.site.is_some_and(|site|site.function.as_deref()==Some("handler:focused"))) {break;}
        }
        assert_eq!(frozen(&app),before);assert_eq!(format!("{:?}",app.world().resource::<VnEngine>().0.history.entries()),history);
        assert_eq!(test_var(&app,"focuses"),Value::Int(0));assert_eq!(test_var(&app,"clicks"),Value::Int(0));
        assert!(crate::editor_preview::paused(app.world()));
        let stopped=crate::editor_debugger::snapshot(app.world().resource::<VnEngine>()).unwrap();
        assert_eq!(stopped.site.unwrap().function.as_deref(),Some("handler:focused"));
        let work=stopped.steps;
        app.update();app.update();
        assert_eq!(crate::editor_debugger::snapshot(app.world().resource::<VnEngine>()).unwrap().steps,work);
        assert_eq!(frozen(&app),before);assert_eq!(app.world().get::<ControlSource>(controls[0]).unwrap().0,captured);
        crate::editor_debugger::run(app.world_mut(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
        for _ in 0..16 {
            app.update();
            if !crate::editor_debugger::pending(app.world().resource::<VnEngine>())&&test_var(&app,"clicks")==Value::Int(1){break;}
        }
        assert_eq!(test_var(&app,"focuses"),Value::Int(1));assert_eq!(test_var(&app,"clicks"),Value::Int(1));
        assert!(!crate::editor_debugger::pending(app.world().resource::<VnEngine>()));
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_frame_failed);
        assert!(app.world().resource::<Screens>().pending_press.is_none());
        assert_eq!(app.world().get::<ControlSource>(controls[0]).unwrap().0,captured);
        let committed=frozen(&app);let visits=format!("{:?}",app.world().resource::<VnEngine>().0.history.entries());
        app.update();app.update();assert_eq!(frozen(&app),committed);
        assert_eq!(format!("{:?}",app.world().resource::<VnEngine>().0.history.entries()),visits);
        assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
        // An old physical source must not become a new interface after this
        // successful continuation, even if its screen and component aliases match.
        app.world_mut().resource_mut::<VnEngine>().0.interface_event(event("form","renew",EventKind::Click,None)).unwrap();
        consume_completed_owner_publication_for_test(&mut app);
        let replaced=frozen(&app);assert!(!app.world().resource::<VnEngine>().0.ui_input_source_current(&captured));
        assert!(crate::editor_preview_input::inject(app.world_mut(),frame,rvn_core::editor_preview::ImageInput::Pointer{
            kind:rvn_core::editor_preview::PointerKind::Down,button:0,point:[100.0,80.0],wheel:[0.0;2]}).is_err());
        assert_eq!(frozen(&app),replaced);
    }
    pub(super) fn fixture(source: &str) -> (App, Entity, Vec<Entity>) {
        let mut engine = if source.contains("scene label") {
            rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(), crate::bevy_renderer::BevyRenderer::new(),32).unwrap()
        } else { rvn_core::Engine::new(rvn_parser::parse(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap() };
        engine.step_until_interaction().unwrap();
        engine.renderer.scene_batch=None;
        let mut screens = Screens {
            active: true,
            ..default()
        };
        update_views(&mut screens, engine.interface_views().unwrap());
        screens.keyboard_focus=Some(("form".into(),"button".into()));
        let captured=engine.capture_ui_input_source("form").unwrap();
        screens.rendered_sources.insert("form".into(),captured.clone());
        let mut app = App::new();
        app.insert_resource(VnEngine(engine))
            .insert_resource(screens)
            .insert_resource(State::new(VnState::Waiting))
            .init_resource::<NextState<VnState>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<bevy::input::touch::Touches>()
            .init_resource::<bevy::ui::UiScale>()
            .init_resource::<bevy::ui::UiStack>()
            .init_resource::<ScriptErrorMessage>()
            .init_resource::<crate::accessibility::Accessibility>()
            .add_event::<MouseButtonInput>()
            .add_event::<KeyboardInput>()
            .add_event::<bevy::input::keyboard::KeyboardFocusLost>()
            .add_event::<Ime>()
            .add_event::<MouseWheel>()
            .add_event::<VnCommand>()
            .add_event::<bevy::a11y::ActionRequest>()
            .add_systems(
                PreUpdate,
                (
                    mouse_button_input_system,
                    bevy::input::keyboard::keyboard_input_system,
                    bevy::ui::ui_focus_system,
                    transformed_focus,
                    capture_input_sources,
                )
                    .chain(),
            )
            .add_systems(Update, (assistive_input,input).chain());
        let window = app
            .world_mut()
            .spawn((Window::default(), bevy::window::PrimaryWindow))
            .id();
        let camera = app
            .world_mut()
            .spawn((Camera::default(), GlobalTransform::default()))
            .id();
        let mut controls = Vec::new();
        for (index, id) in ["button", "toggle", "slider", "name"]
            .into_iter()
            .enumerate()
        {
            controls.push(pointer_node(
                app.world_mut(),
                Some(id),
                camera,
                Vec2::new(100.0, 80.0 + index as f32 * 80.0),
                Vec2::new(120.0, 40.0),
            ));
        }
        app.world_mut().resource_mut::<bevy::ui::UiStack>().uinodes = controls.clone();
        for &entity in &controls {app.world_mut().entity_mut(entity).insert(ControlSource(captured.clone()));}
        (app, window, controls)
    }

    pub(super) fn pointer_node(
        world: &mut World,
        id: Option<&str>,
        camera: Entity,
        center: Vec2,
        size: Vec2,
    ) -> Entity {
        let mut node = Node::default();
        let bevy::reflect::ReflectMut::Struct(fields) = node.reflect_mut() else {
            unreachable!()
        };
        fields.field_mut("calculated_size").unwrap().apply(&size);
        let mut visible = ViewVisibility::default();
        visible.set();
        let entity = world
            .spawn((
                node,
                GlobalTransform::from_translation(center.extend(0.0)),
                visible,
                FocusPolicy::Block,
                Interaction::None,
                bevy::ui::TargetCamera(camera),
            ))
            .id();
        if let Some(id) = id {
            world.entity_mut(entity).insert(Control {
                screen: "form".into(),
                element: id.into(),
                option: None,
            });
        }
        entity
    }

    fn pointer_move(app: &mut App, window: Entity, point: Vec2) {
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(point));
    }

    fn pointer_edge(app: &mut App, window: Entity, state: ButtonState) {
        app.world_mut().send_event(MouseButtonInput {
            window,
            button: MouseButton::Left,
            state,
        });
    }

    fn short_click(app: &mut App, window: Entity, point: Vec2) {
        pointer_move(app, window, point);
        pointer_edge(app, window, ButtonState::Pressed);
        pointer_edge(app, window, ButtonState::Released);
        app.update();
        assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }

    fn test_var(app: &App, name: &str) -> Value {
        app.world().resource::<VnEngine>().0.state.vars[name].clone()
    }
    const DECLARATIONS:&str=r#"
handler clicked(event){set clicks=clicks+1}
handler keyed(event){set key_count=key_count+1}
handler focused(event){set focuses=focuses+1 if renew {ui.close("form") ui.open("form",[],false,0)}}
handler recreate(event){ui.close("form") ui.open("form",[],false,0)}
screen form(){return component("root","panel",{"events":{"key":"keyed"}},[
 component("button","button",{"events":{"click":"clicked","focus":"focused"}},[]),
 component("toggle","toggle",{"binding":"toggled"},[]),
 component("slider","slider",{"binding":"level","min":0,"max":1},[]),
 component("name","input",{"binding":"name"},[]),
 component("renew","button",{"events":{"click":"recreate"}},[]),
 component("select","select",{"binding":"selection","options":["one","two"]},[])])}
"#;
    fn source(game_owned:bool,legacy:bool)->String {
        let marker=if legacy{"label"}else{"scene label"};
        format!("{DECLARATIONS}\ninit {{set clicks=0 set focuses=0 set renew=false set key_count=0 set toggled=false set level=0 set name=\"original\" set selection=\"one\" {}}}\n{marker} P\n{}\n\"parent\"\njump done\n{marker} C\n{}\n\"child\"\nreturn\nlabel done\n",if game_owned{"ui.open(\"form\",[],false,0)"}else{""},if game_owned{""}else{"ui.open(\"form\",[],false,0)"},if game_owned{""}else{"ui.open(\"form\",[],false,0)"})
    }
    fn repaint_state(app:&mut App,refresh_sources:bool) {
        let views=app.world().resource::<VnEngine>().0.interface_views().unwrap();
        let cap=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.renderer.scene_batch=None;
        let mut screens=app.world_mut().resource_mut::<Screens>();screens.views=views;
        if refresh_sources {screens.rendered_sources.insert("form".into(),cap);}
    }
    fn reopen(app:&mut App){
        let engine=&mut app.world_mut().resource_mut::<VnEngine>().0;
        engine.interface_event(event("form","renew",EventKind::Click,None)).unwrap();
        repaint_state(app,false);
    }
    fn frozen(app:&App)->serde_json::Value {serde_json::to_value(&app.world().resource::<VnEngine>().0.state).unwrap()}
    fn key(app:&mut App,window:Entity){app.world_mut().send_event(KeyboardInput{key_code:KeyCode::KeyK,logical_key:Key::Character("k".into()),state:ButtonState::Pressed,window});}

    #[test]
    fn scoped_native_focus_handler_replacement_cannot_redirect_the_following_click(){
        let (mut app,window,_)=fixture(&source(false,true));
        app.world_mut().resource_mut::<VnEngine>().0.state.vars.insert("renew".into(),Value::Bool(true));
        let old=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        short_click(&mut app,window,Vec2::new(100.0,80.0));
        assert_eq!(test_var(&app,"focuses"),Value::Int(1));assert_eq!(test_var(&app,"clicks"),Value::Int(0));
        assert!(!app.world().resource::<VnEngine>().0.ui_input_source_current(&old));
    }
    #[test]
    fn scoped_native_old_entity_press_after_reopen_is_neutral_before_new_handler(){
        let (mut app,window,controls)=fixture(&source(false,false));let original=app.world().get::<ControlSource>(controls[0]).unwrap().0.clone();
        reopen(&mut app);let before=frozen(&app);short_click(&mut app,window,Vec2::new(100.0,80.0));
        assert_eq!(frozen(&app),before);assert!(!app.world().resource::<VnEngine>().0.ui_input_source_current(&original));
        assert_eq!(test_var(&app,"clicks"),Value::Int(0));
    }
    #[test]
    fn scoped_native_queued_a11y_old_entity_does_not_adopt_reopened_alias(){
        use bevy::a11y::accesskit::{Action,ActionRequest,NodeId};
        let (mut app,_,controls)=fixture(&source(false,false));
        app.world_mut().send_event(bevy::a11y::ActionRequest(ActionRequest{action:Action::Default,target:NodeId(controls[0].to_bits()),data:None}));
        reopen(&mut app);let before=frozen(&app);app.update();assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"clicks"),Value::Int(0));
    }
    #[test]
    fn scoped_native_scene_keyboard_queue_is_not_retargeted_after_call_to_child_homonym(){
        let (mut app,window,_)=fixture(&source(false,false));key(&mut app,window);
        app.world_mut().resource_mut::<VnEngine>().0.execute_timer_action("call C").unwrap();
        app.world_mut().resource_mut::<VnEngine>().0.step_until_interaction().unwrap();repaint_state(&mut app,false);
        let before=frozen(&app);app.update();assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"key_count"),Value::Int(0));
    }
    #[test]
    fn scoped_native_game_keyboard_capability_remains_live_across_call(){
        let (mut app,window,_)=fixture(&source(true,false));let source=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();key(&mut app,window);
        app.world_mut().resource_mut::<VnEngine>().0.execute_timer_action("call C").unwrap();app.world_mut().resource_mut::<VnEngine>().0.step_until_interaction().unwrap();repaint_state(&mut app,false);app.update();
        assert!(app.world().resource::<VnEngine>().0.ui_input_source_current(&source));assert_eq!(test_var(&app,"key_count"),Value::Int(1));
    }
    #[test]
    fn scoped_native_drag_and_dropdown_keep_original_instance_capabilities(){
        for dropdown in [false,true]{
            let (mut app,window,_)=fixture(&source(false,false));let captured=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
            {let mut screens=app.world_mut().resource_mut::<Screens>();if dropdown{
                screens.dropdown=Some(Dropdown{screen:"form".into(),element:"select".into(),options:vec!["one".into(),"two".into()],labels:vec![],highlighted:1,position:Vec2::ZERO,width:100.0});screens.dropdown_source=Some(captured);
            }else{screens.dragging=Some(Dragging{screen:"form".into(),element:"slider".into()});screens.dragging_source=Some(captured);}}
            reopen(&mut app);let before=frozen(&app);
            if dropdown{app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Enter);}else{pointer_move(&mut app,window,Vec2::new(150.0,240.0));pointer_edge(&mut app,window,ButtonState::Pressed);}
            app.update();assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"selection"),Value::Str("one".into()));assert_eq!(test_var(&app,"level"),Value::Int(0));
        }
    }
    #[test]
    fn scoped_native_ime_retains_editing_source_even_after_new_render_source_is_available(){
        let (mut app,window,_)=fixture(&source(false,false));let old=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();reopen(&mut app);repaint_state(&mut app,true);
        {let mut screens=app.world_mut().resource_mut::<Screens>();screens.keyboard_focus=Some(("form".into(),"name".into()));screens.editing=Editing{screen:"form".into(),element:"name".into(),caret:8,..default()};screens.editing_source=Some(old);}
        app.world_mut().send_event(Ime::Commit{window,value:" delayed 🍃".into()});let before=frozen(&app);app.update();assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"name"),Value::Str("original".into()));
    }
    #[test]
    fn scoped_browser_stale_header_discards_unconvertible_payload_before_interpretation(){
        let (mut app,_,_)=fixture(&source(false,false));let source=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();let text=serde_json::json!([{"source":source.to_wire().unwrap(),"screen":"form","element":"button","kind":{"bad":"kind"},"value":1e40,"key":null}]).to_string();reopen(&mut app);let before=frozen(&app);
        let input=crate::ui_input_bridge::decode_batch(&text).unwrap().pop().unwrap();assert!(input.admit(&app.world().resource::<VnEngine>().0).unwrap().is_none());assert_eq!(frozen(&app),before);
        let nested=format!("[{{\"source\":{},\"value\":{}0{}}}]",serde_json::to_string(&source.to_wire().unwrap()).unwrap(),"[".repeat(129),"]".repeat(129));assert!(crate::ui_input_bridge::decode_batch(&nested).is_err());assert_eq!(frozen(&app),before);
    }
    #[test]
    fn scoped_browser_current_payload_validation_and_bounded_stale_envelopes_are_kept(){
        let (app,_,_)=fixture(&source(false,false));let source=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        let text=serde_json::json!([{"source":source.to_wire().unwrap(),"screen":"form","element":"name","kind":"change","value":1e40,"key":null}]).to_string();assert!(crate::ui_input_bridge::decode_batch(&text).unwrap().pop().unwrap().admit(&app.world().resource::<VnEngine>().0).is_err());
        let foreign=serde_json::json!([{"source":source.to_wire().unwrap(),"screen":"foreign","element":"button","kind":{"bad":"kind"},"value":1e40}]).to_string();let before=frozen(&app);assert!(crate::ui_input_bridge::decode_batch(&foreign).unwrap().pop().unwrap().admit(&app.world().resource::<VnEngine>().0).unwrap().is_none());assert_eq!(frozen(&app),before);
        assert!(crate::ui_input_bridge::decode_batch(&" ".repeat(9*1024*1024+1)).is_err());
        let item=serde_json::json!({"source":source.to_wire().unwrap(),"screen":"form","element":"button","kind":"click","value":null,"key":null});assert!(crate::ui_input_bridge::decode_batch(&serde_json::to_string(&vec![item;129]).unwrap()).is_err());
    }


    #[test]
    fn scoped_native_queued_key_and_ime_do_not_follow_focus_normalisation_within_the_same_instance(){
        let text=r#"
handler keyed(event){set key_count=key_count+1}
handler swap(event){set replace=true ui.focus("form","other")}
screen form(){if replace {return component("root","panel",{"events":{"key":"keyed"}},[component("other","input",{"binding":"other"},[])])} return component("root","panel",{"events":{"key":"keyed"}},[component("name","input",{"binding":"name"},[]),component("renew","button",{"events":{"click":"swap"}},[])])}
init {set key_count=0 set replace=false set name="original" set other=""}
scene label P
ui.open("form",[],false,0)
"parent"
"#;
        let (mut app,window,_)=fixture(text);
        let source=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        {let mut screens=app.world_mut().resource_mut::<Screens>();screens.keyboard_focus=Some(("form".into(),"name".into()));screens.editing=Editing{screen:"form".into(),element:"name".into(),caret:8,..default()};screens.editing_source=Some(source.clone());}
        key(&mut app,window);app.world_mut().send_event(Ime::Commit{window,value:"delayed".into()});
        app.world_mut().resource_mut::<VnEngine>().0.interface_event(event("form","renew",EventKind::Click,None)).unwrap();repaint_state(&mut app,false);
        assert!(app.world().resource::<VnEngine>().0.ui_input_source_current(&source));
        assert_eq!(app.world().resource::<Screens>().views[0].focus.as_deref(),Some("other"));
        let before=frozen(&app);app.update();assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"key_count"),Value::Int(0));assert_eq!(test_var(&app,"other"),Value::Str(String::new()));assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn scoped_native_raw_press_does_not_adopt_a_control_published_after_frame_capture(){
        #[derive(Resource,Default)]struct Published(bool);
        fn publish(world:&mut World){
            if world.resource::<Published>().0{return;}world.resource_mut::<Published>().0=true;
            world.resource_mut::<VnEngine>().0.interface_event(event("form","renew",EventKind::Click,None)).unwrap();
            let source=world.resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();let views=world.resource::<VnEngine>().0.interface_views().unwrap();world.resource_mut::<VnEngine>().0.renderer.scene_batch=None;
            {let mut screens=world.resource_mut::<Screens>();screens.views=views;screens.rendered_sources.insert("form".into(),source.clone());}
            let entities=world.query_filtered::<Entity,With<Control>>().iter(world).collect::<Vec<_>>();for entity in entities{world.entity_mut(entity).insert(ControlSource(source.clone()));}
        }
        let (mut app,window,_)=fixture(&source(false,false));let old=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();app.init_resource::<Published>().add_systems(Update,publish.before(assistive_input));
        short_click(&mut app,window,Vec2::new(100.0,80.0));let current=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();assert_ne!(current,old);
        assert_eq!(app.world().resource::<Screens>().frame_sources.get("form"),Some(&old));assert_eq!(test_var(&app,"focuses"),Value::Int(0));assert_eq!(test_var(&app,"clicks"),Value::Int(0));
    }

    #[test]
    fn scoped_native_old_entity_and_keyboard_do_not_adopt_an_identical_new_engine_at_epoch_zero(){
        let text=source(false,false);let (mut app,window,_)=fixture(&text);let old=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();key(&mut app,window);
        let mut replacement=rvn_core::Engine::from_program(rvn_parser::parse_program(&text).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();replacement.step_until_interaction().unwrap();replacement.renderer.scene_batch=None;let current=replacement.capture_ui_input_source("form").unwrap();
        assert_eq!(current.authority,old.authority);assert_eq!(current.order,old.order);assert_ne!(current.lineage,old.lineage);app.world_mut().insert_resource(VnEngine(replacement));repaint_state(&mut app,false);let before=frozen(&app);
        short_click(&mut app,window,Vec2::new(100.0,80.0));assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"clicks"),Value::Int(0));assert_eq!(test_var(&app,"key_count"),Value::Int(0));assert!(!app.world().resource::<VnEngine>().0.ui_input_source_current(&old));
    }
    #[derive(Resource,Default)]
    struct DeferredNativeEvents {
        keyboard:Vec<KeyboardInput>,
        ime:Vec<Ime>,
    }
    fn emit_deferred_native_events(mut pending:ResMut<DeferredNativeEvents>,mut keyboard:EventWriter<KeyboardInput>,mut ime:EventWriter<Ime>) {
        for event in pending.keyboard.drain(..) {keyboard.send(event);}
        for event in pending.ime.drain(..) {ime.send(event);}
    }
    fn deferred_key(app:&mut App,window:Entity) {
        app.world_mut().resource_mut::<DeferredNativeEvents>().keyboard.push(KeyboardInput{key_code:KeyCode::KeyK,logical_key:Key::Character("k".into()),state:ButtonState::Pressed,window});
    }
    fn deferred_ime(app:&mut App,window:Entity,value:&str) {
        app.world_mut().resource_mut::<DeferredNativeEvents>().ime.push(Ime::Commit{window,value:value.into()});
    }
    fn deferred_fixture(text:&str) -> (App,Entity) {
        let (mut app,window,_)=fixture(text);
        // Produce after First's event maintenance, then use the real input
        // and capture systems. The event stays live across the deferred frame.
        app.init_resource::<DeferredNativeEvents>().add_systems(PreUpdate,
            emit_deferred_native_events.before(bevy::input::keyboard::keyboard_input_system));
        // Reproduce the actual plugin condition: these EventReaders are not
        // advanced while a genuine SceneRenderBatch is awaiting installation.
        app.world_mut().resource_mut::<bevy::ecs::schedule::Schedules>().remove(Update).unwrap();
        app.add_systems(Update,(assistive_input.run_if(crate::scene_publication::ready),input.run_if(crate::scene_publication::ready)).chain());
        (app,window)
    }
    fn pending_focus(app:&mut App,element:&str) {
        let engine=&mut app.world_mut().resource_mut::<VnEngine>().0;
        engine.interface_event(event("form",element,EventKind::Focus,None)).unwrap();
        assert!(engine.renderer.scene_batch.is_some());
    }
    fn replace_deferred_engine(app:&mut App,text:&str) {
        let mut engine=rvn_core::Engine::from_program(rvn_parser::parse_program(text).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();
        engine.step_until_interaction().unwrap();
        engine.renderer.scene_batch=None;
        app.world_mut().insert_resource(VnEngine(engine));
        repaint_state(app,true);
        let source=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        let mut query=app.world_mut().query_filtered::<Entity,With<Control>>();
        let entities=query.iter(app.world()).collect::<Vec<_>>();
        for entity in entities {app.world_mut().entity_mut(entity).insert(ControlSource(source.clone()));}
    }
    fn deferred_history(app:&App) -> serde_json::Value {
        serde_json::to_value(app.world().resource::<VnEngine>().0.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>()).unwrap()
    }
    #[test]
    fn scoped_native_deferred_keyboard_event_retains_its_producer_after_identical_engine_replacement(){
        let text=source(false,false);let (mut app,window)=deferred_fixture(&text);
        let old=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        pending_focus(&mut app,"button");deferred_key(&mut app,window);app.update();
        assert_eq!(test_var(&app,"key_count"),Value::Int(0));
        let (id,producer)={let screens=app.world().resource::<Screens>();assert_eq!(screens.key_producers.len(),1);let (id,producer)=screens.key_producers.iter().next().unwrap();(*id,producer.clone())};
        assert_eq!(producer.source.as_ref(),Some(&old));
        replace_deferred_engine(&mut app,&text);
        let current=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        assert_eq!(current.authority,old.authority);assert_eq!(current.order,old.order);assert_ne!(current.lineage,old.lineage);
        let before=frozen(&app);let history=deferred_history(&app);app.update();
        assert_eq!(frozen(&app),before);assert_eq!(deferred_history(&app),history);
        let screens=app.world().resource::<Screens>();assert_eq!(screens.frame_sources.get("form"),Some(&current));assert!(std::sync::Arc::ptr_eq(screens.key_producers.get(&id).unwrap(),&producer));
        key(&mut app,window);app.update();assert_eq!(test_var(&app,"key_count"),Value::Int(1));
        assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn scoped_native_deferred_ime_commit_does_not_attach_to_the_new_editing_session(){
        let text=source(false,false);let (mut app,window)=deferred_fixture(&text);
        app.world_mut().resource_mut::<Screens>().keyboard_focus=Some(("form".into(),"name".into()));
        let old=app.world().resource::<VnEngine>().0.capture_ui_input_source("form").unwrap();
        pending_focus(&mut app,"name");
        deferred_ime(&mut app,window," delayed 🍃");app.update();
        let (id,producer)={let screens=app.world().resource::<Screens>();assert_eq!(screens.ime_producers.len(),1);let (id,producer)=screens.ime_producers.iter().next().unwrap();(*id,producer.clone())};
        assert_eq!(producer.source.as_ref(),Some(&old));assert_eq!(producer.focus,Some(("form".into(),"name".into())));
        replace_deferred_engine(&mut app,&text);
        {let mut screens=app.world_mut().resource_mut::<Screens>();screens.keyboard_focus=Some(("form".into(),"name".into()));screens.editing=default();screens.editing_source=None;}
        let before=frozen(&app);let history=deferred_history(&app);app.update();
        assert_eq!(frozen(&app),before);assert_eq!(deferred_history(&app),history);assert_eq!(test_var(&app,"name"),Value::Str("original".into()));
        assert!(std::sync::Arc::ptr_eq(app.world().resource::<Screens>().ime_producers.get(&id).unwrap(),&producer));
        app.world_mut().send_event(Ime::Commit{window,value:" fresh 🍃".into()});app.update();
        assert_eq!(test_var(&app,"name"),Value::Str("original fresh 🍃".into()));
        assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn scoped_native_admitted_complete_shift_tab_chord_keeps_modifier_order_and_releases(){
        let text=source(false,false).replace("{\"events\":{\"key\":\"keyed\"}}","{}");
        let (mut app,window)=deferred_fixture(&text);
        app.world_mut().resource_mut::<Screens>().keyboard_focus=Some(("form".into(),"toggle".into()));
        for (key_code,logical_key,state) in [
            (KeyCode::ShiftLeft,Key::Shift,ButtonState::Pressed),
            (KeyCode::Tab,Key::Tab,ButtonState::Pressed),
            (KeyCode::Tab,Key::Tab,ButtonState::Released),
            (KeyCode::ShiftLeft,Key::Shift,ButtonState::Released),
        ] {app.world_mut().send_event(KeyboardInput{key_code,logical_key,state,window});}
        app.update();
        assert_eq!(app.world().resource::<Screens>().keyboard_focus,Some(("form".into(),"button".into())));
        assert_eq!(app.world().resource::<Screens>().modifiers,[false;4]);
        assert!(!app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::ShiftLeft));
        assert_eq!(test_var(&app,"focuses"),Value::Int(1));
        assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn scoped_native_deferred_tab_uses_the_captured_destination_between_two_interfaces(){
        for replace_destination in [false,true] {
            let other=r#"
handler entered_other(event){set other_focuses=other_focuses+1}
handler renew_other(event){ui.close("other") ui.open("other",[],false,0)}
screen other(){return component("other_root","panel",{},[component("outside","button",{"events":{"focus":"entered_other"}},[]),component("reopen","button",{"events":{"click":"renew_other"}},[])])}
"#;
            let base=source(false,false).replace("{\"events\":{\"key\":\"keyed\"}}","{}")
                .replacen("init {","init {set other_focuses=0 ",1)
                .replacen("\n\"parent\"","\nui.open(\"other\",[],false,0)\n\"parent\"",1);
            let text=format!("{other}\n{base}");let (mut app,window)=deferred_fixture(&text);
            let old_other=app.world().resource::<VnEngine>().0.capture_ui_input_source("other").unwrap();
            {let mut screens=app.world_mut().resource_mut::<Screens>();screens.keyboard_focus=Some(("form".into(),"select".into()));screens.rendered_sources.insert("other".into(),old_other.clone());}
            pending_focus(&mut app,"select");
            for state in [ButtonState::Pressed,ButtonState::Released] {app.world_mut().resource_mut::<DeferredNativeEvents>().keyboard.push(KeyboardInput{key_code:KeyCode::Tab,logical_key:Key::Tab,state,window});}
            app.update();assert_eq!(test_var(&app,"other_focuses"),Value::Int(0));
            let producer=app.world().resource::<Screens>().key_producers.values().next().unwrap().clone();
            assert_eq!(producer.sources.get("other"),Some(&old_other));assert!(producer.key_edges.contains(&KeyCode::Tab));
            if replace_destination {app.world_mut().resource_mut::<VnEngine>().0.renderer.scene_batch=None;app.world_mut().resource_mut::<VnEngine>().0.interface_event(event("other","reopen",EventKind::Click,None)).unwrap();}
            repaint_state(&mut app,true);
            let current_other=app.world().resource::<VnEngine>().0.capture_ui_input_source("other").unwrap();
            app.world_mut().resource_mut::<Screens>().rendered_sources.insert("other".into(),current_other.clone());
            let before=frozen(&app);let pc=app.world().resource::<VnEngine>().0.state.pc;let random=app.world().resource::<VnEngine>().0.state.random;let history=deferred_history(&app);
            app.update();
            assert!(!app.world().resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::Tab));
            assert_eq!(app.world().resource::<VnEngine>().0.state.pc,pc);assert_eq!(app.world().resource::<VnEngine>().0.state.random,random);
            assert_eq!(test_var(&app,"key_count"),Value::Int(0));
            if replace_destination {
                assert_ne!(current_other,old_other);assert!(!app.world().resource::<VnEngine>().0.ui_input_source_current(&old_other));assert_eq!(frozen(&app),before);assert_eq!(deferred_history(&app),history);
                assert_eq!(test_var(&app,"other_focuses"),Value::Int(0));assert_eq!(app.world().resource::<Screens>().keyboard_focus,Some(("form".into(),"select".into())));
            }else{
                let current_history=deferred_history(&app);let old_history=history.as_array().unwrap();let rows=current_history.as_array().unwrap();assert_eq!(rows.len(),old_history.len()+1);assert_eq!(&rows[..old_history.len()],old_history.as_slice());assert_eq!(rows.last().unwrap()[0],before);assert!(rows.last().unwrap()[1].is_null());
                assert_eq!(current_other,old_other);assert_eq!(test_var(&app,"other_focuses"),Value::Int(1));assert_eq!(app.world().resource::<Screens>().keyboard_focus,Some(("other".into(),"outside".into())));
                assert_eq!(app.world().resource::<VnEngine>().0.state.ui.screens.iter().find(|screen|screen.name=="other").unwrap().focus.as_deref(),Some("outside"));
            }
            assert_eq!(producer.sources.get("other"),Some(&old_other));assert!(app.world().resource::<ScriptErrorMessage>().0.is_empty());
        }
    }
    #[test]
    fn runtime_inspection_projects_actual_installed_controls_geometry_without_dispatching_input(){
        use rvn_core::editor_inspection::{capture,FrameIdentity,InstanceKey,Inspection};
        let (mut app,_,mut controls)=fixture(&source(false,false));prepare_modern_pointer_publication(&mut app,&mut controls);
        let engine=&app.world().resource::<VnEngine>().0;let before=frozen(&app);
        let mut scope=completed_native_image(app.world(),1).scope.clone();scope.position=engine.state.scene_execution.as_ref().unwrap().live_position.clone();
        let logical=capture(&engine.state,FrameIdentity{instance:scope.instance,epoch:scope.epoch,sequence:0});assert!(logical.unavailable.is_none(),"{:?}",logical.unavailable);
        let rendered=inspect_rendered_ui(app.world(),&logical,&scope).unwrap();
        for (index,entity) in controls.iter().enumerate(){
            let row=rendered.rows.iter().find(|row|matches!(&row.key,InstanceKey::UiEntity{entity:actual,..}if *actual==entity.to_bits())).unwrap();
            assert_eq!(row.details["entity"],entity.to_bits());assert_eq!(row.details["size"],serde_json::json!([120.0,40.0]));
            assert_eq!(row.details["transform"][3],serde_json::json!([100.0,80.0+index as f32*80.0,0.0,1.0]));
            assert!(matches!(&row.parent,Some(InstanceKey::UiComponent{target:Some(_),..})));
        }
        let wire=rendered.encode_for(FrameIdentity::of(&scope)).unwrap();let received=Inspection::decode(&wire).unwrap();assert_eq!(received.frame,FrameIdentity::of(&scope));
        assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"clicks"),Value::Int(0));assert_eq!(test_var(&app,"focuses"),Value::Int(0));
        assert!(!app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
    }
    #[test]
    fn runtime_inspection_refuses_mixed_old_rendered_source_after_actual_interface_replacement(){
        use rvn_core::editor_inspection::{capture,FrameIdentity,InstanceKey};
        let (mut app,_,mut controls)=fixture(&source(false,false));prepare_modern_pointer_publication(&mut app,&mut controls);
        let engine=&app.world().resource::<VnEngine>().0;let mut scope=completed_native_image(app.world(),1).scope.clone();scope.position=engine.state.scene_execution.as_ref().unwrap().live_position.clone();
        let old=capture(&engine.state,FrameIdentity{instance:scope.instance,epoch:scope.epoch,sequence:0});let observed=inspect_rendered_ui(app.world(),&old,&scope).unwrap();
        let identity=observed.rows.iter().find(|row|matches!(row.key,InstanceKey::UiEntity{..})).unwrap().key.clone();
        assert!(observed.selected(&observed.frame,&identity).is_some());
        reopen(&mut app);let engine=&app.world().resource::<VnEngine>().0;let before=frozen(&app);
        let current=capture(&engine.state,FrameIdentity{instance:scope.instance,epoch:scope.epoch,sequence:0});scope.sequence=2;
        assert!(inspect_rendered_ui(app.world(),&current,&scope).is_err(),"an old visible source is never adopted as the replacement instance");
        assert_eq!(frozen(&app),before);assert_eq!(test_var(&app,"clicks"),Value::Int(0));
    }


}

/// Read only the resolved UI and entities that will be extracted with the
/// Camera2d image. No definition, input capability or new Core state is minted.
pub(crate) fn inspect_rendered_ui(world:&World,logical:&rvn_core::editor_inspection::Inspection,scope:&rvn_core::editor_preview::FrameScope)
    ->Result<rvn_core::editor_inspection::Inspection,String>{
    use rvn_core::editor_inspection::{FrameIdentity,Key,Row,MAX_INSPECTION_ROWS};
    use rvn_core::ui_input::{UiInputSource,UiInputAuthority};
    use serde::{Serialize,ser::{Error as _,SerializeSeq}};
    if logical.position!=scope.position{return Err("Rendered inspection has a different installed position".into());}
    struct Rows<'a>{world:&'a World,screens:Option<&'a Screens>,logical:&'a rvn_core::editor_inspection::Inspection,scope:&'a rvn_core::editor_preview::FrameScope}
    fn target(source:&UiInputSource)->Option<&rvn_core::scene_resources::TargetIdentity>{match &source.authority{UiInputAuthority::Narrative(token)|UiInputAuthority::Instance{token,..}=>Some(&token.target),UiInputAuthority::Legacy{..}=>None}}
    fn valid(source:&UiInputSource,view:&ScreenView,rows:&Rows<'_>)->bool{
        source.lineage==rows.scope.instance&&source.screen==view.name&&source.order==view.order
            &&rows.logical.has_interface(&view.name,view.order,source.host_role,target(source))
            &&match &source.authority{UiInputAuthority::Narrative(token)|UiInputAuthority::Instance{token,..}=>rows.logical.has_resource_token(token),UiInputAuthority::Legacy{..}=>true}
    }
    #[derive(Serialize)]struct ComponentDetails<'a>{
        kind:Kind,visible:bool,enabled:bool,text:&'a str,value:&'a serde_json::Value,binding:&'a Option<String>,
        rect:Option<[f32;4]>,width:Option<f32>,height:Option<f32>,focus_order:i32,
        events:&'a std::collections::BTreeMap<EventKind,String>,props:&'a std::collections::BTreeMap<String,serde_json::Value>,
        state:&'a std::collections::BTreeMap<String,serde_json::Value>,
    }
    fn component_key<'a>(view:&'a ScreenView,source:&'a UiInputSource,component:&'a str)->Key<'a>{
        Key::UiComponent{screen:&view.name,order:view.order,host_role:source.host_role,target:target(source),component}
    }
    fn components<S:SerializeSeq>(seq:&mut S,view:&ScreenView,source:&UiInputSource,node:&UiComponent,parent:Option<&str>,depth:usize,count:&mut usize)->Result<(),S::Error>{
        if depth>32||*count>=MAX_INSPECTION_ROWS{return Err(S::Error::custom("Inspection UI instance budget"));}*count+=1;
        let parent=parent.map(|id|component_key(view,source,id)).unwrap_or(Key::Interface{screen:&view.name,order:view.order,host_role:source.host_role,target:target(source)});
        seq.serialize_element(&Row{key:component_key(view,source,&node.id),parent:Some(parent),label:&node.id,parked:false,
            details:ComponentDetails{kind:node.kind,visible:node.visible,enabled:node.enabled,text:&node.text,value:&node.value,binding:&node.binding,
                rect:node.rect,width:node.width,height:node.height,focus_order:node.focus_order,events:&node.events,props:&node.props,state:&node.state}})?;
        for child in &node.children{components(seq,view,source,child,Some(&node.id),depth+1,count)?;}Ok(())
    }
    #[derive(Serialize)]struct EntityDetails<'a>{entity:u64,parent_entity:Option<u64>,size:Option<[f32;2]>,transform:Option<[[f32;4];4]>,visible:Option<bool>,option:Option<&'a str>}
    impl Serialize for Rows<'_>{
        fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error>{
            let mut seq=serializer.serialize_seq(None)?;let mut count=self.logical.rows.len();
            for row in &self.logical.rows{seq.serialize_element(row)?;}
            let Some(screens)=self.screens else{return seq.end();};
            if screens.rendered.len()>rvn_core::scene_resources::MAX_INTERFACES{return Err(S::Error::custom("Inspection UI view budget"));}
            for view in &screens.rendered{
                let source=screens.rendered_sources.get(&view.name).ok_or_else(||S::Error::custom("Rendered interface has no exact source"))?;
                if !valid(source,view,self){return Err(S::Error::custom("Rendered interface belongs to another installed state"));}
                components(&mut seq,view,source,&view.root,None,0,&mut count)?;
            }
            for entity in self.world.iter_entities(){
                let control=entity.get::<Control>();let projected=entity.get::<crate::composed_motion::InterfaceTarget>();
                let (screen,element)=if let Some(projected)=projected{(projected.screen.as_str(),projected.element.as_str())}
                    else if let Some(control)=control{(control.screen.as_str(),control.element.as_str())}else{continue;};
                let view=screens.rendered.iter().find(|view|view.name==screen).ok_or_else(||S::Error::custom("Physical component has no rendered interface"))?;
                let source=screens.rendered_sources.get(screen).ok_or_else(||S::Error::custom("Physical component has no rendered source"))?;
                if !valid(source,view,self)||view.root.find(element).is_none(){return Err(S::Error::custom("Physical component has lost its rendered instance"));}
                if projected.is_some(){
                    match (&source.authority,entity.get::<crate::composed_motion::InterfaceProjectionTarget>()){
                        (UiInputAuthority::Narrative(token)|UiInputAuthority::Instance{token,..},Some(projection))if projection.interface==token.target&&projection.order==source.order&&projection.host_role==source.host_role=>{},
                        (UiInputAuthority::Legacy{..},None)=>{},
                        _=>return Err(S::Error::custom("Physical component has a different exact interface projection")),
                    }
                }else if entity.get::<ControlSource>().is_none_or(|capture|&capture.0!=source){return Err(S::Error::custom("Popup control has a different rendered instance"));}
                if count>=MAX_INSPECTION_ROWS{return Err(S::Error::custom("Inspection physical instance budget"));}count+=1;
                let id=entity.id().to_bits();let matrix=entity.get::<GlobalTransform>().map(|transform|transform.compute_matrix());
                let size=entity.get::<Node>().map(Node::size);
                if matrix.is_some_and(|matrix|!matrix.is_finite())||size.is_some_and(|size|!size.is_finite()){return Err(S::Error::custom("Inspection geometry is not finite"));}
                seq.serialize_element(&Row{key:Key::UiEntity{screen:&view.name,order:view.order,host_role:source.host_role,target:target(source),component:element,entity:id},
                    parent:Some(component_key(view,source,element)),label:element,parked:false,
                    details:EntityDetails{entity:id,parent_entity:entity.get::<Parent>().map(|parent|parent.get().to_bits()),size:size.map(|size|size.to_array()),
                        transform:matrix.map(|matrix|matrix.to_cols_array_2d()),visible:entity.get::<InheritedVisibility>().map(|visibility|visibility.get()),option:control.and_then(|control|control.option.as_deref())}})?;
            }
            seq.end()
        }
    }
    logical.project_rows(FrameIdentity::of(scope),&Rows{world,screens:world.get_resource::<Screens>(),logical,scope})
}

#[cfg(test)]
#[path="programmable_widget_layout_tests_0912.rs"]
mod widget_layout_tests_0912;
#[cfg(test)]
#[path="programmable_widget_font_integration_tests_0956.rs"]
mod widget_font_integration_tests_0956;

#[cfg(test)]
#[path="programmable_empty_root_tests_0942.rs"]
mod empty_root_tests_0942;

#[path="programmable_owner_hosts_0943.rs"]
pub(crate) mod owner_hosts_0943;

#[path="programmable_owner_focus_0943.rs"]
pub(crate) mod owner_focus_0943;

#[path="programmable_owner_preview_0967.rs"]
pub(crate) mod owner_preview_0967;

// Real Startup publication + existing CPU font/layout/render path. This is
// neither an actual Widget class lifecycle nor GPU/native hit-test evidence.
#[cfg(all(test,not(target_arch="wasm32")))]
mod startup_widget_plan_tests_0998{
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn startup_widget_plan0998_actual_init_screen_publication_layout_and_render_keep_one_real_owner(){
        let source=r#"screen LayoutProbe0998(){return {"id":"root","kind":"column","layout_contract":"widget_v1","spacing":0,"children":[{"id":"button","kind":"button","spacing":0,"parent_slot":{"kind":"vertical_box"},"children":[{"id":"label","kind":"text","text":"Actual Init","spacing":0,"font_size":24,"parent_slot":{"kind":"button","padding":{"left":4,"right":4,"top":2,"bottom":2},"horizontal":"center","vertical":"center"}}]}]}}
init{ui.open("LayoutProbe0998",[],false,0) set initialized=1}
scene label start
"Boundary"
"#;
        let(_,mut app)=crate::debugger_startup::tests::fixture_source(source);
        app.init_asset::<Image>().init_asset::<Font>().init_resource::<bevy::render::camera::ManualTextureViews>()
            .add_event::<bevy::window::WindowResized>().add_event::<bevy::window::WindowCreated>().add_event::<bevy::window::WindowScaleFactorChanged>()
            .init_resource::<Screens>().init_resource::<WidgetLayouts>().init_resource::<crate::accessibility::Accessibility>()
            .init_resource::<crate::resources::ImagemapState>().init_resource::<crate::resources::VnRenderState>()
            .insert_resource(bevy::ui::UiScale(1.0)).insert_resource(State::new(VnState::Waiting));
        let font=app.world_mut().resource_mut::<Assets<Font>>().add(Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap());app.insert_resource(crate::menu_documents::MenuFont(font));
        app.world_mut().spawn((Window{resolution:bevy::window::WindowResolution::new(840.0,300.0).with_scale_factor_override(1.5),..default()},bevy::window::PrimaryWindow));
        app.world_mut().spawn((Camera::default(),OrthographicProjection::default()));
        app.world_mut().run_system_once(bevy::render::camera::camera_system::<OrthographicProjection>);
        for _ in 0..64{
            if app.world().resource::<crate::debugger_startup::DebugBootstrap>().owner.is_complete(){break;}
            let mut prepared=crate::debugger_startup::prepare_world_step_boxed(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
            let end=std::time::Instant::now()+std::time::Duration::from_secs(15);
            while !crate::debugger_startup::refresh_world_step(app.world(),&mut prepared).unwrap(){app.update();assert!(std::time::Instant::now()<end);std::thread::sleep(std::time::Duration::from_millis(2));}
            let committed=crate::debugger_startup::commit_world_step_boxed(app.world_mut(),prepared).unwrap();crate::debugger_startup::install_committed_world_step_boxed(app.world_mut(),committed);
        }
        assert!(app.world().resource::<crate::debugger_startup::DebugBootstrap>().owner.is_complete());assert!(!app.world().contains_resource::<VnEngine>());
        let real=app.world().resource::<crate::debugger_startup::DebugBootstrap>();let before=serde_json::to_value(real.owner.state()).unwrap();let observation=real.owner.observation_source().unwrap();let actual_source=real.owner.capture_ui_input_source("LayoutProbe0998").unwrap();
        app.world_mut().run_system_once(prepare_widget_layouts);assert!(widget_geometry_ready(app.world()),"{:?}",widget_layout_problem(app.world()));
        assert!(app.world().resource::<WidgetLayouts>().plans.contains_key(&actual_source.to_wire().unwrap()));
        app.world_mut().run_system_once(render);
        assert_eq!(app.world().resource::<Screens>().rendered_sources.get("LayoutProbe0998"),Some(&actual_source));
        assert!(app.world_mut().query::<&WidgetRootSource>().iter(app.world()).any(|root|root.0==actual_source));
        assert!(app.world_mut().query::<&ControlSource>().iter(app.world()).any(|control|control.0==actual_source));
        let real=app.world().resource::<crate::debugger_startup::DebugBootstrap>();assert_eq!(serde_json::to_value(real.owner.state()).unwrap(),before);assert!(real.owner.accepts_observation_source(&observation));assert!(!app.world().contains_resource::<VnEngine>());
        // A real unsettled publication revokes presentation readiness; no
        // source from the retained Screens can stand in for a live owner.
        let mut entry=crate::debugger_startup::prepare_world_entry_boxed(app.world()).unwrap();
        let end=std::time::Instant::now()+std::time::Duration::from_secs(15);
        while !crate::debugger_startup::refresh_world_step(app.world(),&mut entry).unwrap(){app.update();assert!(std::time::Instant::now()<end);std::thread::sleep(std::time::Duration::from_millis(2));}
        let _pending=crate::debugger_startup::commit_world_step_boxed(app.world_mut(),entry).unwrap();
        assert!(app.world().resource::<crate::debugger_startup::DebugBootstrap>().publication_pending);
        let pending_state=serde_json::to_value(app.world().resource::<crate::debugger_startup::DebugBootstrap>().owner.state()).unwrap();
        assert!(!widget_geometry_ready(app.world()));app.world_mut().run_system_once(prepare_widget_layouts);assert!(app.world().resource::<WidgetLayouts>().plans.is_empty());
        assert_eq!(serde_json::to_value(app.world().resource::<crate::debugger_startup::DebugBootstrap>().owner.state()).unwrap(),pending_state);
    }
}
