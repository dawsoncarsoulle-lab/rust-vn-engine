//! Input capabilities travel with a completed real image, never a later pick.
use bevy::prelude::*;
use bevy::input::{ButtonState, keyboard::{Key,KeyboardInput,NativeKey},mouse::{MouseButtonInput,MouseWheel,MouseScrollUnit}};
use bevy::window::{CursorMoved,Ime};
use rvn_core::editor_preview::{ImageInput,PointerKind,FrameScope,InteractionSource};
use crate::resources::{VnEngine,ImagemapState};
use std::{collections::HashMap,sync::Arc};

#[derive(Clone)]
struct PhysicalNode {
    entity:Entity, inverse:Mat4, matrix:Mat4, size:Vec2, z:f32,
    clip:Option<Rect>, blocking:bool, choice:Option<usize>,
    control:Option<(crate::programmable_ui::Control,crate::programmable_ui::ControlSource)>,
    semantic:Option<[u8;32]>, button_geometry:Option<[u8;32]>, confirmation:bool, pause:bool,
}
impl PhysicalNode {
    fn contains(&self,point:Vec2)->bool {
        if self.clip.is_some_and(|clip|!clip.contains(point)){return false;}
        let local=self.inverse.transform_point3(point.extend(self.z)).truncate();
        local.abs().cmple(self.size*0.5).all()
    }
    fn control_current(&self,world:&World)->bool {
        let Some((control,source))=&self.control else{return false;};
        let Some(current)=world.get::<crate::programmable_ui::Control>(self.entity)else{return false;};
        current.screen==control.screen&&current.element==control.element&&current.option==control.option
            && world.get::<crate::programmable_ui::ControlSource>(self.entity).is_some_and(|current|current.0==source.0)
            && world.get::<Node>(self.entity).is_some()
            && world.get::<GlobalTransform>(self.entity).is_some()
            && world.get::<ViewVisibility>(self.entity).is_some_and(|visibility| visibility.get())
            && world.get_resource::<bevy::ui::UiStack>().is_some_and(|stack|stack.uinodes.contains(&self.entity))
            && world.resource::<VnEngine>().0.ui_input_source_current(&source.0)
            && self.button_geometry.is_none_or(|proof|crate::programmable_ui::widget_buttons1030::geometry_current(world,control,source,proof))
            && world.get_resource::<crate::programmable_ui::Screens>().and_then(|screens|screens.views.iter().find(|view|view.name==control.screen))
                .and_then(|view|view.root.find(&control.element)).and_then(|component|crate::programmable_ui::pointer_component_digest(component,false))==self.semantic
    }
}
pub(crate) struct PhysicalFrame {
    pub(crate) scope:FrameScope,
    pub(crate) cost:usize,
    nodes:Vec<PhysicalNode>,
    native:Option<crate::programmable_ui::PreviewNativeFrame>,
    owners:Option<crate::programmable_ui::owner_preview_0967::PreviewOwnerFrame>,
    confirmation:Option<crate::menu_documents::ConfirmationImageFrame>,
    pause:Option<crate::menu_documents::legacy_navigation::PauseImageFrame>,
    canvas:crate::custom_canvas::PreviewCanvasFrame,
    camera:Option<(Camera,GlobalTransform)>,
    imagemap:ImagemapState,
}
impl PhysicalFrame {
    pub(crate) fn owner_snapshot(&self)->Option<&crate::programmable_ui::owner_preview_0967::PreviewOwnerFrame>{self.owners.as_ref()}
    fn picked(&self,point:Vec2)->Option<&PhysicalNode>{self.nodes.iter().rev().find(|node|node.contains(point)&&(node.control.is_some()||node.choice.is_some()||node.confirmation||node.pause||node.blocking))}
    /// Same completed geometry as normal input, including real blocking
    /// owner/control/confirmation nodes. Never click an Imagemap through UI.
    pub(crate) fn startup_imagemap_hit(&self,point:Vec2)->Option<usize>{
        if self.picked(point).is_some(){return None;}
        self.imagemap_hit(point)
    }
    pub(crate) fn imagemap_hit(&self,point:Vec2)->Option<usize>{
        let (camera,transform)=self.camera.as_ref()?;
        self.imagemap.hit_test_world(camera.viewport_to_world_2d(transform,point)?)
    }
}
// The same admitted-image limits are checked on borrowed producer data before
// any Canvas, control, source map, focus order or Imagemap snapshot is cloned.
pub(crate) struct PreviewCost {pub(crate) bytes:usize,limit:usize}
impl PreviewCost {
    pub(crate) fn new(limit:usize)->Self{Self{bytes:0,limit}}
    pub(crate) fn add(&mut self,bytes:usize)->Option<()> {
        self.bytes=self.bytes.checked_add(bytes).filter(|size|*size<=self.limit)?;Some(())
    }
    pub(crate) fn serialized(&mut self,value:&impl serde::Serialize)->Option<()> {
        serde_json::to_writer(self,value).ok()
    }
}
impl std::io::Write for PreviewCost {
    fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{
        self.add(bytes.len()).ok_or_else(||std::io::Error::other("Preview input snapshot budget"))?;Ok(bytes.len())
    }
    fn flush(&mut self)->std::io::Result<()>{Ok(())}
}
pub(crate) fn preview_source_cost(source:&rvn_core::ui_input::UiInputSource)->Option<usize>{
    // Six bytes is the largest JSON escape of one UTF-8 byte. The fixed head
    // also covers the cloned task/token fields, without first calling to_wire.
    let mut strings=source.screen.len();
    if let rvn_core::ui_input::UiInputAuthority::Narrative(token)|rvn_core::ui_input::UiInputAuthority::Instance{token,..}=&source.authority {
        strings=strings.checked_add(token.target.alias.len())?.checked_add(token.task.key.local_id.len())?;
        if let rvn_core::scene_tasks::TaskOwner::Host{role}=&token.task.key.owner {strings=strings.checked_add(role.len())?;}
    }
    strings.checked_mul(6)?.checked_add(512)
}
pub(crate) fn preview_token_cost(token:&rvn_core::scene_resources::ResourceToken)->Option<usize>{
    let mut strings=token.target.alias.len().checked_add(token.task.key.local_id.len())?;
    if let rvn_core::scene_tasks::TaskOwner::Host{role}=&token.task.key.owner {strings=strings.checked_add(role.len())?;}
    strings.checked_mul(6)?.checked_add(256)
}
// Count backing allocations, including retained spare capacity, rather than
// serialized bytes: empty Vec/String values can still own substantial memory.
fn scope_heap_cost(scope:&FrameScope,cost:&mut PreviewCost)->Option<()>{
    fn string(value:&String,cost:&mut PreviewCost)->Option<()>{cost.add(value.capacity())}
    fn path(value:&std::path::PathBuf,cost:&mut PreviewCost)->Option<()>{cost.add(value.capacity())}
    fn position(value:&rvn_core::scene::ResumePosition,cost:&mut PreviewCost)->Option<()>{
        let Some(origin)=&value.origin else{return Some(());};
        if let Some(source)=&origin.source{path(source,cost)?;}
        cost.add(origin.route.capacity().checked_mul(std::mem::size_of::<rvn_core::program::AstRoutePart>())?)?;
        if let Some(source)=origin.graph.as_ref().and_then(|graph|graph.source.as_ref()){path(source,cost)?;}
        if let Some(step)=&origin.scheduled_step{
            if let Some(scope)=&step.scope{string(scope,cost)?;}
            cost.add(step.route.capacity().checked_mul(std::mem::size_of::<rvn_parser::scheduled_source::ScheduledSourceRoute>())?)?;
            for value in [&step.plan_scope,&step.consumer_identity,&step.producer_identity]{string(value,cost)?;}
            if let Some(endpoint)=&step.endpoint{
                string(&endpoint.kind,cost)?;
                cost.add(endpoint.ports.capacity().checked_mul(std::mem::size_of::<rvn_parser::scheduled_source::DeclaredPort>())?)?;
                for port in &endpoint.ports{string(&port.role,cost)?;}
                for value in [endpoint.legacy_terminal.as_ref(),endpoint.native_loop_output.as_ref()].into_iter().flatten(){string(value,cost)?;}
            }
        }
        Some(())
    }
    if let Some(error)=&scope.control_error{string(error,cost)?;}
    position(&scope.position,cost)?;
    if let Some(interaction)=&scope.interaction{position(&interaction.position,cost)?;cost.add(interaction.choice_indices.capacity().checked_mul(std::mem::size_of::<usize>())?)?;}
    if let Some(active)=&scope.active{string(&active.scene,cost)?;}
    cost.add(scope.parked.capacity().checked_mul(std::mem::size_of::<rvn_core::editor_preview::ActivationScope>())?)?;
    for parked in &scope.parked{string(&parked.scene,cost)?;}Some(())
}
fn preflight_physical_frame(world:&World,scope:&FrameScope)->Option<usize>{
    let stack=world.get_resource::<bevy::ui::UiStack>()?;
    if stack.uinodes.len()>4096{return None;}
    let mut cost=PreviewCost::new(2*1024*1024);
    cost.add(std::mem::size_of::<PhysicalFrame>())?;cost.add(2*std::mem::size_of::<usize>())?;
    scope_heap_cost(scope,&mut cost)?;
    cost.add(crate::custom_canvas::preview_canvas_cost(world)?)?;
    cost.add(crate::programmable_ui::preview_native_cost(world)?)?;
    cost.add(crate::programmable_ui::owner_preview_0967::preview_cost(world)?)?;
    crate::menu_documents::confirmation_image_cost(world,&mut cost)?;
    crate::menu_documents::legacy_navigation::pause_image_cost(world,&mut cost)?;
    for &entity in &stack.uinodes {
        cost.add(std::mem::size_of::<PhysicalNode>())?;
        if let Some((control,source))=world.get::<crate::programmable_ui::Control>(entity).zip(world.get::<crate::programmable_ui::ControlSource>(entity)) {
            cost.add(control.screen.len().checked_add(control.element.len())?.checked_add(control.option.as_ref().map_or(0,String::len))?)?;
            cost.add(preview_source_cost(&source.0)?)?;
        }
    }
    if let Some(map)=world.get_resource::<ImagemapState>() {cost.add(map.hotspots.len().checked_mul(std::mem::size_of::<crate::vn_command::ImagemapZone>())?)?;}
    Some(cost.bytes)
}

pub(crate) fn capture(world:&World,scope:FrameScope)->Option<Arc<PhysicalFrame>> {
    let cost=preflight_physical_frame(world,&scope)?;
    let (canvas,_)=crate::custom_canvas::capture_preview_canvas(world)?;
    let stack=world.get_resource::<bevy::ui::UiStack>()?;
    if stack.uinodes.len()>4096{return None;}
    let confirmation=crate::menu_documents::ConfirmationImageFrame::capture(world);
    let pause=crate::menu_documents::legacy_navigation::PauseImageFrame::capture(world);
    // The borrowed preflight charged this exact stack capacity. Reserve once
    // only after admission; geometric Vec growth must not retain extra nodes
    // outside the frame's shared union cost.
    let mut nodes=Vec::with_capacity(stack.uinodes.len());
    let mut button_geometry_cache=HashMap::<String,Option<[u8;32]>>::new();
    for &entity in &stack.uinodes {
        let (Some(node),Some(transform),Some(visible))=(world.get::<Node>(entity),world.get::<GlobalTransform>(entity),world.get::<ViewVisibility>(entity))else{continue;};
        if !visible.get()||!node.size().cmpgt(Vec2::ZERO).all(){continue;}
        let matrix=transform.compute_matrix();
        if !matrix.is_finite()||matrix.determinant().abs()<1e-8{continue;}
        let control=world.get::<crate::programmable_ui::Control>(entity).zip(world.get::<crate::programmable_ui::ControlSource>(entity)).map(|(control,source)|(control.clone(),source.clone()));
        let semantic=control.as_ref().and_then(|(control,_)|crate::programmable_ui::preview_component_digest(world,control));
        if control.is_some()&&semantic.is_none(){return None;}
        let button_geometry=control.as_ref().filter(|(control,_)|crate::programmable_ui::widget_buttons1030::is_rendered_button(world,control)).and_then(|(control,source)|*button_geometry_cache.entry(source.0.to_wire().ok()?).or_insert_with(||crate::programmable_ui::widget_buttons1030::rendered_geometry(world,control,source)));
        nodes.push(PhysicalNode {entity,inverse:matrix.inverse(),matrix,size:node.size(),z:transform.translation().z,semantic,button_geometry,
            clip:world.get::<bevy::ui::CalculatedClip>(entity).map(|clip|clip.clip),
            blocking:world.get::<bevy::ui::FocusPolicy>(entity)==Some(&bevy::ui::FocusPolicy::Block),
            choice:world.get::<crate::components::ChoiceButton>(entity).map(|choice|choice.0),
            confirmation:confirmation.as_ref().is_some_and(|frame|frame.contains(entity)),
            pause:pause.as_ref().is_some_and(|frame|frame.contains(entity)),control});
    }
    let native=crate::programmable_ui::capture_preview_native(world);
    let owners=crate::programmable_ui::owner_preview_0967::capture(world)?;
    if owners.as_ref().is_some_and(|owners|!owners.belongs_to(&scope)){return None;}
    let camera=world.iter_entities().find(|entity|entity.contains::<Camera2d>()).and_then(|entity|Some((entity.get::<Camera>()?.clone(),*entity.get::<GlobalTransform>()?)));
    let state=world.get_resource::<ImagemapState>();
    let imagemap=ImagemapState {hotspots:state.map_or_else(Vec::new,|v|v.hotspots.clone()),source_w:state.map_or(0.0,|v|v.source_w),source_h:state.map_or(0.0,|v|v.source_h),active:state.is_some_and(|v|v.active),bg_handle:None};
    Some(Arc::new(PhysicalFrame {scope,cost,nodes,native,owners,confirmation,pause,canvas,camera,imagemap}))
}
#[derive(Resource)]
struct PauseImagePress {entity:Option<Entity>,frame:Arc<PhysicalFrame>}
#[derive(Resource,Clone)]
pub(crate) struct PointerOverride {pub(crate) control:Option<Entity>,confirmation:Option<Entity>,pause:Option<Entity>,frame:Arc<PhysicalFrame>}
impl PointerOverride {
    pub(crate) fn image(&self)->(u64,u64,u64){(self.frame.scope.instance,self.frame.scope.epoch,self.frame.scope.sequence)}
    /// Read only the actual admitted image node, not a later named control.
    pub(crate) fn button_entity_current1030(&self,world:&World)->Option<(Entity,[u8;32])>{
        let entity=self.control?;let node=self.frame.nodes.iter().find(|node|node.entity==entity)?;
        node.control_current(world).then_some((entity,node.button_geometry?))
    }
    pub(crate) fn slider_fraction(&self,entity:Entity,control:&crate::programmable_ui::Control,source:Option<&crate::programmable_ui::ControlSource>,component:&rvn_ui::programmable::Component,point:Vec2)->Option<f32>{
        let node=self.frame.nodes.iter().find(|node|node.entity==entity)?;
        let (captured,captured_source)=node.control.as_ref()?;
        if captured.screen!=control.screen||captured.element!=control.element||captured.option!=control.option
            || source.is_none_or(|source|source.0!=captured_source.0)
            || crate::programmable_ui::pointer_component_digest(component,false)!=node.semantic{return None;}
        let local=node.inverse.transform_point3(point.extend(node.z)).truncate();
        local.is_finite().then(||(local.x/node.size.x.max(1.0)+0.5).clamp(0.0,1.0))
    }
}

// Standard Bevy focus cannot pick an Image camera. Project only the recipient
// named by the admitted completed image, after its normal focus pass.
pub(crate) fn project_confirmation(world:&mut World) {
    let Some(pointer)=world.get_resource::<PointerOverride>().cloned() else{return;};
    if let Some(frame)=&pointer.frame.pause {
        let press=world.remove_resource::<PauseImagePress>().and_then(|press|
            press.entity.filter(|&entity|press.frame.pause.as_ref().is_some_and(|frame|frame.current(world,entity))));
        frame.project(world,pointer.pause,press);
    } else {
        world.remove_resource::<PauseImagePress>();
        crate::menu_documents::legacy_navigation::clear_pause_image_interactions(world);
    }
    if let Some(frame)=&pointer.frame.confirmation {
        frame.project(world,pointer.confirmation);
    } else {
        // An image preceding a modal supplies no authority to its new controls.
        crate::menu_documents::clear_confirmation_image_interactions(world);
    }
}

#[derive(Resource,Clone)]
pub(crate) struct Routing {frame:Arc<PhysicalFrame>, pointer:Option<Vec2>, choice:Option<usize>, blocked:bool}
// Internal service authority, never a fabricated dialogue/choice source.
pub(crate) struct TerminalLoadSource {
    instance: u64,
    epoch: u64,
    position: rvn_core::scene::ResumePosition,
}
impl TerminalLoadSource {
    fn of(engine: &VnEngine) -> Self {
        Self { instance: engine.0.instance_id(), epoch: engine.0.runtime_epoch(),
            position: engine.0.state.scene_execution.as_ref().map(|execution| execution.live_position.clone())
                .unwrap_or(rvn_core::scene::ResumePosition { pc: engine.0.state.pc, origin: None }) }
    }
    fn current(&self, engine: &VnEngine) -> bool {
        let current = Self::of(engine);
        engine.0.is_finished() && !engine.0.renderer.scene_batch_pending() && !engine.0.renderer.scene_frame_failed
            && self.instance == current.instance && self.epoch == current.epoch && self.position == current.position
    }
}
#[derive(Resource,Default)]
pub(crate) struct Receipts {
    reader:bevy::ecs::event::ManualEventReader<crate::vn_command::PlayerInput>,
    sources:HashMap<usize,Option<InteractionSource>>,
    terminal_loads:HashMap<usize,TerminalLoadSource>,
    ranges:std::collections::VecDeque<(usize,usize,Option<InteractionSource>)>,
}
impl Receipts {
    pub(crate) fn remember_terminal_load(&mut self,id:usize,source:TerminalLoadSource) { self.terminal_loads.insert(id,source); }
    pub(crate) fn admit(&mut self,id:usize,engine:&crate::resources::VnEngine,input:&crate::vn_command::PlayerInput)->bool {
        if let Some(source)=self.terminal_loads.remove(&id) {
            if !matches!(input,crate::vn_command::PlayerInput::QuickLoad) || !source.current(engine) { return false; }
        }
        let source=if let Some(source)=self.sources.remove(&id){source}else if let Some((_,_,source))=self.ranges.iter().find(|(first,last,_)|id>=*first&&id<=*last){source.clone()}else{return true;};
        if matches!(input,crate::vn_command::PlayerInput::Advance|crate::vn_command::PlayerInput::Choose(_)|crate::vn_command::PlayerInput::SkipTypewriter) {
            source.as_ref().is_some_and(|source|engine.0.accepts_preview_input_source(source))
        }else{true}
    }
}
pub(crate) fn queue_player(world:&mut World,input:crate::vn_command::PlayerInput,source:Option<InteractionSource>){
    world.init_resource::<Receipts>();
    let id=world.resource_mut::<Events<crate::vn_command::PlayerInput>>().send(input).id;
    world.resource_mut::<Receipts>().sources.insert(id,source);
}
#[derive(Resource,Default)]
struct PhysicalKeys {instance:u64,epoch:u64,modifiers:[bool;4],held:std::collections::HashSet<KeyCode>}
pub(crate) fn release_held(world:&mut World){
    world.remove_resource::<PauseImagePress>();
    crate::programmable_ui::cancel_preview_pointer(world);
    crate::programmable_ui::cancel_preview_keyboard(world);
    crate::custom_canvas::cancel_preview_pointer_capture(world,true);
    if world.contains_resource::<Events<bevy::input::keyboard::KeyboardFocusLost>>(){world.send_event(bevy::input::keyboard::KeyboardFocusLost);}
    if let Some(mut mouse)=world.get_resource_mut::<ButtonInput<MouseButton>>(){mouse.release_all();}
    world.init_resource::<PhysicalKeys>();let mut keys=world.resource_mut::<PhysicalKeys>();keys.modifiers=[false;4];keys.held.clear();
}
pub(crate) fn begin(world:&mut World){
    let engine=&world.resource::<VnEngine>().0;let current=(engine.instance_id(),engine.runtime_epoch());
    if let Some(output)=world.get_resource::<crate::editor_preview::Output>(){output.discard_foreign(current.0,current.1);}
    world.init_resource::<PhysicalKeys>();
    let changed={let old=world.resource::<PhysicalKeys>();old.instance!=0 && (old.instance,old.epoch)!=current};
    if changed {
        if world.contains_resource::<Events<bevy::input::keyboard::KeyboardFocusLost>>() {world.send_event(bevy::input::keyboard::KeyboardFocusLost);}
        if let Some(mut mouse)=world.get_resource_mut::<ButtonInput<MouseButton>>(){mouse.release_all();}
        world.resource_mut::<PhysicalKeys>().modifiers=[false;4];world.resource_mut::<PhysicalKeys>().held.clear();
    }
    {let mut keys=world.resource_mut::<PhysicalKeys>();keys.instance=current.0;keys.epoch=current.1;}
    world.remove_resource::<Routing>();world.remove_resource::<PointerOverride>();world.remove_resource::<PauseImagePress>();
    if let Some(events)=world.get_resource::<Events<crate::vn_command::PlayerInput>>() {
        let reader=events.get_reader_current();let oldest=events.oldest_event_count();
        world.init_resource::<Receipts>();let mut receipts=world.resource_mut::<Receipts>();receipts.reader=reader;receipts.sources.retain(|id,_|*id>=oldest);receipts.terminal_loads.retain(|id,_|*id>=oldest);
    }
}
pub(crate) fn stamp(mut receipts:ResMut<Receipts>,events:Res<Events<crate::vn_command::PlayerInput>>,routing:Option<Res<Routing>>){
    receipts.sources.retain(|id,_|*id>=events.oldest_event_count());
    receipts.terminal_loads.retain(|id,_|*id>=events.oldest_event_count());
    let mut reader=std::mem::take(&mut receipts.reader);
    let mut range=None;
    for (_,id) in reader.read_with_id(&events){range=Some(range.map_or((id.id,id.id),|(first,_)|(first,id.id)));}
    receipts.ranges.retain(|(_,last,_)|*last>=events.oldest_event_count());
    if let (Some((first,last)),Some(routing))=(range,routing){receipts.ranges.push_back((first,last,routing.frame.scope.interaction.clone()));}
    receipts.reader=reader;
}
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct StoryContext<'w> {
    current:Res<'w,ImagemapState>,
    routing:Option<Res<'w,Routing>>,
    engine:Res<'w,VnEngine>,
}
impl StoryContext<'_> {
    pub(crate) fn imagemap(&self)->&ImagemapState {self.routing.as_ref().map_or(&self.current,|routing|&routing.frame.imagemap)}
    pub(crate) fn permitted(&self)->bool {self.routing.as_ref().is_none_or(|routing|!routing.blocked&&routing.frame.scope.interaction.as_ref().is_some_and(|source|self.engine.0.accepts_preview_input_source(source)))}
    pub(crate) fn terminal_load_source(&self)->Option<TerminalLoadSource> {
        let source=TerminalLoadSource::of(&self.engine);
        if !source.current(&self.engine) { return None; }
        if self.routing.as_ref().is_some_and(|routing| {
            let scope=&routing.frame.scope;
            routing.blocked || scope.finished!=Some(true)
                || scope.paused || scope.publication_pending || scope.sequence==0
                || scope.instance!=source.instance || scope.epoch!=source.epoch || scope.position!=source.position
        }) { return None; }
        Some(source)
    }
    pub(crate) fn pointer_hit(&self)->Option<Option<usize>> {self.routing.as_ref().and_then(|routing|routing.pointer.map(|point|routing.frame.imagemap_hit(point)))}
    pub(crate) fn choice_hit(&self)->Option<usize> {self.routing.as_ref().and_then(|routing|routing.choice)}
}
fn key_code(value:&str)->Option<KeyCode>{
    Some(match value {
        "Escape"=>KeyCode::Escape,"ReturnKey"=>KeyCode::Enter,"Space"=>KeyCode::Space,"Tab"=>KeyCode::Tab,
        "Backspace"=>KeyCode::Backspace,"Delete"=>KeyCode::Delete,"Home"=>KeyCode::Home,"End"=>KeyCode::End,
        "ArrowLeft"=>KeyCode::ArrowLeft,"ArrowRight"=>KeyCode::ArrowRight,"ArrowUp"=>KeyCode::ArrowUp,"ArrowDown"=>KeyCode::ArrowDown,
        "Control"=>KeyCode::ControlLeft,"Shift"=>KeyCode::ShiftLeft,"Alt"=>KeyCode::AltLeft,"Logo"=>KeyCode::SuperLeft,
        "F5"=>KeyCode::F5,"F6"=>KeyCode::F6,"Key0"=>KeyCode::Digit0,"Key1"=>KeyCode::Digit1,"Key2"=>KeyCode::Digit2,"Key3"=>KeyCode::Digit3,"Key4"=>KeyCode::Digit4,"Key5"=>KeyCode::Digit5,"Key6"=>KeyCode::Digit6,"Key7"=>KeyCode::Digit7,"Key8"=>KeyCode::Digit8,"Key9"=>KeyCode::Digit9,
        "KeyA"=>KeyCode::KeyA,"KeyB"=>KeyCode::KeyB,"KeyC"=>KeyCode::KeyC,"KeyD"=>KeyCode::KeyD,"KeyE"=>KeyCode::KeyE,"KeyF"=>KeyCode::KeyF,"KeyG"=>KeyCode::KeyG,"KeyH"=>KeyCode::KeyH,"KeyI"=>KeyCode::KeyI,"KeyJ"=>KeyCode::KeyJ,"KeyK"=>KeyCode::KeyK,"KeyL"=>KeyCode::KeyL,"KeyM"=>KeyCode::KeyM,"KeyN"=>KeyCode::KeyN,"KeyO"=>KeyCode::KeyO,"KeyP"=>KeyCode::KeyP,"KeyQ"=>KeyCode::KeyQ,"KeyR"=>KeyCode::KeyR,"KeyS"=>KeyCode::KeyS,"KeyT"=>KeyCode::KeyT,"KeyU"=>KeyCode::KeyU,"KeyV"=>KeyCode::KeyV,"KeyW"=>KeyCode::KeyW,"KeyX"=>KeyCode::KeyX,"KeyY"=>KeyCode::KeyY,"KeyZ"=>KeyCode::KeyZ,
        _=>return None,
    })
}
fn logical(code:KeyCode)->Key {
    match code {KeyCode::Enter=>Key::Enter,KeyCode::Tab=>Key::Tab,KeyCode::Space=>Key::Space,KeyCode::Backspace=>Key::Backspace,KeyCode::Delete=>Key::Delete,KeyCode::Escape=>Key::Escape,KeyCode::ShiftLeft=>Key::Shift,KeyCode::ControlLeft=>Key::Control,KeyCode::AltLeft=>Key::Alt,KeyCode::SuperLeft=>Key::Super,KeyCode::Home=>Key::Home,KeyCode::End=>Key::End,KeyCode::ArrowLeft=>Key::ArrowLeft,KeyCode::ArrowRight=>Key::ArrowRight,KeyCode::ArrowUp=>Key::ArrowUp,KeyCode::ArrowDown=>Key::ArrowDown,_=>Key::Unidentified(NativeKey::Unidentified)}
}
fn keyboard(world:&mut World,frame:&PhysicalFrame,window:Entity,code:KeyCode,pressed:bool,text:Option<String>){
    world.init_resource::<PhysicalKeys>();
    let edge=if pressed{world.resource_mut::<PhysicalKeys>().held.insert(code)}else{world.resource_mut::<PhysicalKeys>().held.remove(&code);false};
    let event=KeyboardInput{key_code:code,logical_key:text.map_or_else(||logical(code),|text|Key::Character(text.into())),state:if pressed{ButtonState::Pressed}else{ButtonState::Released},window};
    if let Some(native)=&frame.native {native.keyboard(world,event,edge);}else{world.send_event(event);}
}
pub(crate) fn neutralize_passive(world:&mut World,input:&ImageInput)->Result<(),String>{
    match input {
        ImageInput::FocusLost=>{
            release_held(world);
            if let Some(mut mouse)=world.get_resource_mut::<ButtonInput<MouseButton>>(){mouse.reset_all();}
            if let Some(mut keys)=world.get_resource_mut::<ButtonInput<KeyCode>>(){keys.reset_all();}
            if world.contains_resource::<Events<bevy::window::WindowFocused>>() {
                if let Some(window)=world.query_filtered::<Entity,With<Window>>().iter(world).next(){world.send_event(bevy::window::WindowFocused{window,focused:false});}
            }
        }
        ImageInput::Pointer{kind:PointerKind::Up,button,..}=>{
            let button=match button{0=>MouseButton::Left,1=>MouseButton::Right,_=>MouseButton::Middle};
            if let Some(mut mouse)=world.get_resource_mut::<ButtonInput<MouseButton>>(){mouse.reset(button);}
            if button==MouseButton::Left {crate::programmable_ui::widget_buttons1030::neutral_preview_up(world);}
            crate::custom_canvas::cancel_preview_pointer_capture(world,false);
        }
        ImageInput::Key{key,pressed:false,..}=>{
            let code=key_code(key).ok_or("Unsupported preview key release")?;
            if let Some(mut keys)=world.get_resource_mut::<ButtonInput<KeyCode>>(){keys.reset(code);}
            world.init_resource::<PhysicalKeys>();let mut keys=world.resource_mut::<PhysicalKeys>();keys.held.remove(&code);
            for (index,modifier) in [KeyCode::ControlLeft,KeyCode::ShiftLeft,KeyCode::AltLeft,KeyCode::SuperLeft].into_iter().enumerate(){if code==modifier{keys.modifiers[index]=false;}}
        }
        _=>return Err("Only passive preview neutralization is permitted".into()),
    }
    world.remove_resource::<Routing>();world.remove_resource::<PointerOverride>();world.remove_resource::<PauseImagePress>();Ok(())
}
pub(crate) fn inject(world:&mut World,frame:Arc<PhysicalFrame>,input:ImageInput)->Result<(),String>{
    let passive=matches!(&input,ImageInput::FocusLost|ImageInput::Key{pressed:false,..}|ImageInput::Pointer{kind:PointerKind::Up,..});
    if !passive && world.get_resource::<Routing>().is_some_and(|routing|routing.frame.scope.sequence!=frame.scope.sequence){return Err("Another physical image already supplied this frame's input".into());}
    let window=world.query_filtered::<Entity,With<Window>>().iter(world).next().ok_or("The preview camera has no logical input window")?;
    let mut routing=world.get_resource::<Routing>().cloned().unwrap_or(Routing{frame:frame.clone(),pointer:None,choice:None,blocked:false});
    match input {
        ImageInput::FocusLost=>{
            world.remove_resource::<PauseImagePress>();
    crate::programmable_ui::cancel_preview_pointer(world);
            world.send_event(bevy::input::keyboard::KeyboardFocusLost);
            world.init_resource::<PhysicalKeys>();world.resource_mut::<PhysicalKeys>().modifiers=[false;4];world.resource_mut::<PhysicalKeys>().held.clear();
            world.resource_mut::<ButtonInput<MouseButton>>().release_all();
            world.send_event(bevy::window::WindowFocused{window,focused:false});
        }
        ImageInput::Key{key,logical,pressed,modifiers}=>{
            let code=key_code(&key).ok_or("Unsupported preview key")?;
            if pressed && frame.native.as_ref().is_some_and(|native|!native.current(world)){return Err("The image's keyboard recipient changed".into());}
            // Modifier edges keep their event ordering instead of relying on the
            // final held state of a complete short Shift+Tab chord.
            for (index,code) in [KeyCode::ControlLeft,KeyCode::ShiftLeft,KeyCode::AltLeft,KeyCode::SuperLeft].into_iter().enumerate(){
                world.init_resource::<PhysicalKeys>();
                let held=world.resource::<PhysicalKeys>().modifiers[index];
                if held!=modifiers[index]{keyboard(world,&frame,window,code,modifiers[index],None);world.resource_mut::<PhysicalKeys>().modifiers[index]=modifiers[index];}
            }
            keyboard(world,&frame,window,code,pressed,logical);
        }
        ImageInput::Text{value,composition}=>{
            let native=frame.native.as_ref().filter(|native|native.current(world)).ok_or("The image has no current native text recipient")?;
            native.ime(world,if let Some(range)=composition{
                let offset=|index|value.char_indices().nth(index).map_or(value.len(),|(offset,_)|offset);
                let cursor=Some((offset(range[0]),offset(range[1])));
                Ime::Preedit{window,value,cursor}
            }else{Ime::Commit{window,value}});
        }
        ImageInput::Pointer{kind,button,point,wheel}=>{
            let point=Vec2::from_array(point);let picked=frame.picked(point);
            let control=if let Some(node)=picked.filter(|node|node.control.is_some()){
                if !node.control_current(world){if kind==PointerKind::Up {None}else{return Err("The image's physical control changed".into());}}else{Some(node.entity)}
            }else{None};
            routing.pointer=Some(point);routing.choice=picked.and_then(|node|node.choice);
            routing.blocked=picked.is_some_and(|node|(node.blocking||node.confirmation||node.pause)&&node.choice.is_none());
            if kind==PointerKind::Down && button==0 {
                world.insert_resource(PauseImagePress{entity:picked.filter(|node|node.pause).map(|node|node.entity),frame:frame.clone()});
            }
            world.insert_resource(PointerOverride{control,confirmation:picked.filter(|node|node.confirmation).map(|node|node.entity),
                pause:picked.filter(|node|node.pause).map(|node|node.entity),frame:frame.clone()});
            world.get_mut::<Window>(window).unwrap().set_cursor_position(Some(point));
            let pointer=world.resource::<PointerOverride>().clone();
            crate::programmable_ui::widget_buttons1030::image_edge(world,&pointer,kind,button,window,false);
            let button=match button{0=>MouseButton::Left,1=>MouseButton::Right,_=>MouseButton::Middle};
            match kind {
                PointerKind::Down|PointerKind::Up=>frame.canvas.mouse(world,MouseButtonInput{button,state:if kind==PointerKind::Down{ButtonState::Pressed}else{ButtonState::Released},window},point),
                PointerKind::Move=>frame.canvas.motion(world,CursorMoved{window,position:point,delta:None}),
                PointerKind::Wheel=>frame.canvas.wheel(world,MouseWheel{unit:MouseScrollUnit::Pixel,x:wheel[0],y:wheel[1],window},point),
            }
        }
    }
    world.insert_resource(routing);Ok(())
}

#[cfg(test)]
mod completed_image_input_contracts {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use crate::{resources::*, vn_command::{PlayerInput,VnCommand}};
    struct Fixture {app:App,root:std::path::PathBuf}
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.root);}}
    fn fixture(script:&str)->Fixture {
        let root=std::env::temp_dir().join(format!("rvn-preview-input-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let manager=rvn_core::persistent::PersistentDataManager::new(&root).unwrap();
        let mut engine=rvn_core::Engine::new(rvn_parser::parse(script).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
        engine.step_until_interaction().unwrap();let _=engine.renderer.take_pending();
        let mut app=App::new();
        app.insert_resource(VnEngine(engine)).insert_resource(State::new(VnState::Waiting))
            .init_resource::<NextState<VnState>>().init_resource::<MenuState>()
            .init_resource::<crate::systems::save_menu::SaveMenuState>()
            .init_resource::<crate::systems::save_menu::SaveConfirmation>()
            .init_resource::<crate::save_thumbnails::SaveThumbnails>()
            .init_resource::<crate::accessibility::Accessibility>()
            .insert_resource(crate::project_paths::ProjectPaths::new(root.clone(),root.clone(),root.clone(),root.clone(),root.clone()))
            .insert_resource(PersistentDataResource{manager,data:rvn_core::persistent::PersistentData::default()})
            .init_resource::<VnRenderState>().init_resource::<ImagemapState>()
            .init_resource::<TypewriterState>().init_resource::<ChoiceFocus>().init_resource::<DialogueHistory>()
            .init_resource::<ScriptErrorMessage>().init_resource::<SkipMode>()
            .init_resource::<crate::menu_documents::Menus>().init_resource::<crate::programmable_ui::Screens>()
            .init_resource::<bevy::ui::UiStack>().init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>().init_resource::<Receipts>()
            .add_event::<PlayerInput>().add_event::<VnCommand>().add_event::<KeyboardInput>()
            .add_event::<bevy::input::keyboard::KeyboardFocusLost>().add_event::<MouseButtonInput>()
            .add_event::<MouseWheel>().add_event::<CursorMoved>().add_event::<Ime>().add_event::<bevy::window::WindowFocused>()
            .add_systems(PreUpdate,(bevy::input::mouse::mouse_button_input_system,bevy::input::keyboard::keyboard_input_system).chain())
            .add_systems(Update,(crate::systems::input::story_keyboard_systems(),stamp,crate::systems::input::player_input_system).chain());
        app.world_mut().spawn((Window{resolution:bevy::window::WindowResolution::new(1280.0,720.0).with_scale_factor_override(1.0),..default()},bevy::window::PrimaryWindow));
        Fixture{app,root}
    }
    fn scope(world:&World,sequence:u64)->FrameScope {
        let engine=&world.resource::<VnEngine>().0;
        FrameScope{instance:engine.instance_id(),epoch:engine.runtime_epoch(),sequence,control_sequence:0,control_ack:0,control_error:None,paused:false,publication_pending:false, finished: None,step_allowed:false,
            interaction:engine.preview_input_source(),position:engine.state.scene_execution.as_ref().map(|execution|execution.live_position.clone()).unwrap_or(rvn_core::scene::ResumePosition{pc:engine.state.pc,origin:None}),active:None,parked:vec![]}
    }
    fn button(world:&mut World,index:usize,center:Vec2)->Entity {
        let mut node=Node::default();
        let bevy::reflect::ReflectMut::Struct(fields)=node.reflect_mut()else{panic!("Node reflection")};
        fields.field_mut("calculated_size").unwrap().apply(&Vec2::new(100.0,40.0));
        let mut visible=ViewVisibility::default();visible.set();
        let entity=world.spawn((node,GlobalTransform::from_translation(center.extend(0.0)),visible,crate::components::ChoiceButton(index),bevy::ui::FocusPolicy::Block)).id();
        world.resource_mut::<bevy::ui::UiStack>().uinodes.push(entity);entity
    }
    fn down()->ImageInput {ImageInput::Pointer{kind:PointerKind::Down,button:0,point:[100.0,100.0],wheel:[0.0;2]}}
    fn selected(f:&Fixture)->rvn_parser::Value {f.app.world().resource::<VnEngine>().0.state.vars["selected"].clone()}
    #[test]
    fn choice_from_completed_physical_image_uses_its_button_in_the_real_receiver() {
        let mut f=fixture("set selected=0\nchoice {\"First\"=>{set selected=10} \"Second\"=>{set selected=20}}\n\"after\"\n");
        f.app.world_mut().resource_mut::<VnRenderState>().choice_options=vec!["First".into(),"Second".into()];
        let entity=button(f.app.world_mut(),1,Vec2::new(100.0,100.0));
        begin(f.app.world_mut());let frame=capture(f.app.world(),scope(f.app.world(),1)).unwrap();
        // A later layout/stack may differ; the click still names the pixels and
        // authored option that were actually captured, never a new raycast.
        f.app.world_mut().entity_mut(entity).insert((crate::components::ChoiceButton(0),GlobalTransform::from_translation(Vec3::new(800.0,400.0,0.0))));
        inject(f.app.world_mut(),frame,down()).unwrap();f.app.update();
        f.app.world_mut().resource_mut::<VnEngine>().0.step_until_interaction().unwrap();
        assert_eq!(selected(&f),rvn_parser::Value::Int(20));
        assert!(f.app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn changed_conditional_choice_rejects_completed_image_before_selecting_another_index() {
        let mut f=fixture("set flag=true\nset selected=0\nchoice {\"First\" if flag=>{set selected=10} \"Second\"=>{set selected=20}}\n\"after\"\n");
        f.app.world_mut().resource_mut::<VnRenderState>().choice_options=vec!["First".into(),"Second".into()];
        button(f.app.world_mut(),0,Vec2::new(100.0,100.0));begin(f.app.world_mut());
        let frame=capture(f.app.world(),scope(f.app.world(),1)).unwrap();
        f.app.world_mut().resource_mut::<VnEngine>().0.state.vars.insert("flag".into(),rvn_parser::Value::Bool(false));
        let before=serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
        let history=format!("{:?}",f.app.world().resource::<VnEngine>().0.history.entries());
        inject(f.app.world_mut(),frame,down()).unwrap();f.app.update();
        assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(),before);
        assert_eq!(format!("{:?}",f.app.world().resource::<VnEngine>().0.history.entries()),history);
        assert_eq!(selected(&f),rvn_parser::Value::Int(0));
    }
    #[test]
    fn imagemap_uses_completed_camera_and_hotspots_instead_of_current_resize_data() {
        let mut f=fixture("set selected=0\nimagemap {background:\"map.png\" hotspot {area:(0,0,640,720)}=>{set selected=10} hotspot {area:(640,0,1280,720)}=>{set selected=20}}\n\"after\"\n");
        f.app.init_resource::<Assets<Image>>().init_resource::<bevy::render::camera::ManualTextureViews>()
            .add_event::<bevy::window::WindowResized>().add_event::<bevy::window::WindowCreated>()
            .add_event::<bevy::window::WindowScaleFactorChanged>().add_event::<AssetEvent<Image>>();
        f.app.world_mut().spawn(Camera2dBundle::default());
        f.app.world_mut().run_system_once(bevy::render::camera::camera_system::<OrthographicProjection>);
        *f.app.world_mut().resource_mut::<ImagemapState>()=ImagemapState{active:true,source_w:1280.0,source_h:720.0,bg_handle:None,
            hotspots:vec![crate::vn_command::ImagemapZone{area:rvn_parser::Rect::new(0,0,640,720),hover_area:rvn_parser::Rect::new(0,0,640,720)},crate::vn_command::ImagemapZone{area:rvn_parser::Rect::new(640,0,1280,720),hover_area:rvn_parser::Rect::new(640,0,1280,720)}]};
        begin(f.app.world_mut());let frame=capture(f.app.world(),scope(f.app.world(),1)).unwrap();
        assert_eq!(frame.imagemap_hit(Vec2::new(100.0,100.0)),Some(0));
        f.app.world_mut().resource_mut::<ImagemapState>().hotspots.reverse();
        f.app.world_mut().resource_mut::<ImagemapState>().source_w=2560.0;
        let camera=f.app.world().iter_entities().find(|entity|entity.contains::<Camera2d>()).unwrap().id();
        f.app.world_mut().entity_mut(camera).insert(GlobalTransform::from_translation(Vec3::new(1000.0,0.0,1000.0)));
        inject(f.app.world_mut(),frame,down()).unwrap();f.app.update();
        f.app.world_mut().resource_mut::<VnEngine>().0.step_until_interaction().unwrap();
        assert_eq!(selected(&f),rvn_parser::Value::Int(10));
    }
    #[test]
    fn preview_control_aggregate_refuses_before_copying_the_producer_option(){
        let mut f=fixture("\"first\"\n");begin(f.app.world_mut());
        let source=f.app.world().resource::<VnEngine>().0.capture_ui_input_source("missing");
        // A corrupted ECS producer is still subject to the outer snapshot bound
        // before semantic lookup; it cannot allocate a multi-megabyte copy.
        let source=source.unwrap_or_else(||rvn_core::ui_input::UiInputSource{lineage:f.app.world().resource::<VnEngine>().0.instance_id(),screen:"missing".into(),order:0,host_role:None,authority:rvn_core::ui_input::UiInputAuthority::Legacy{interface_epoch:0}});
        let entity=f.app.world_mut().spawn((crate::programmable_ui::Control{screen:"missing".into(),element:"field".into(),option:Some("x".repeat(2*1024*1024))},crate::programmable_ui::ControlSource(source))).id();
        f.app.world_mut().resource_mut::<bevy::ui::UiStack>().uinodes=vec![entity];
        let before=serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
        assert!(preflight_physical_frame(f.app.world(),&scope(f.app.world(),1)).is_none());
        assert_eq!(f.app.world().get::<crate::programmable_ui::Control>(entity).unwrap().option.as_ref().unwrap().len(),2*1024*1024);
        assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(),before);
    }
    #[test]
    fn explicit_player_receipt_refuses_other_engine_with_the_same_statement_and_epoch() {
        let mut f=fixture("\"first\"\n\"second\"\n");begin(f.app.world_mut());
        let source=f.app.world().resource::<VnEngine>().0.preview_input_source().unwrap();
        queue_player(f.app.world_mut(),PlayerInput::Advance,Some(source));
        let mut replacement=rvn_core::Engine::new(rvn_parser::parse("\"first\"\n\"second\"\n").unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();replacement.step_until_interaction().unwrap();let _=replacement.renderer.take_pending();
        f.app.world_mut().insert_resource(VnEngine(replacement));
        let before=serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
        f.app.update();
        assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(),before);
    }

    fn terminal_quickload_fixture_0038() -> Fixture {
        let mut f=fixture("\"legacy placeholder\"\n");
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(
            "init{set score=0}\nscene label Saved\nset score=7\n\"Saved dialogue\"\njump End\nscene label End\nset score=99\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
        game.enter_label("Saved").unwrap();game.step_until_interaction().unwrap();
        f.app.world_mut().insert_resource(VnEngine(game));
        f.app.init_resource::<crate::scene_publication::ScenePresentation>().init_resource::<Time<()>>();
        crate::scene_publication::consume(f.app.world_mut());
        let manager=rvn_core::save::SaveManager::new(&f.root,crate::systems::save_menu::SUPPORTED_SLOTS).unwrap();
        f.app.world_mut().resource_mut::<VnEngine>().0.save_quicksave(&manager,"before end".into(),"program.rvn".into()).unwrap();
        {let mut game=f.app.world_mut().resource_mut::<VnEngine>();game.0.advance_dialogue().unwrap();game.0.step_until_interaction().unwrap();assert!(game.0.is_finished());}
        crate::scene_publication::consume(f.app.world_mut());
        assert!(!f.app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        f.app.world_mut().insert_resource(State::new(VnState::Finished));
        f
    }
    fn terminal_scope_0038(world:&World)->FrameScope {
        let mut s=scope(world,1);s.finished=Some(world.resource::<VnEngine>().0.is_finished());s
    }
    fn key_0038(key:&str)->ImageInput {ImageInput::Key{key:key.into(),logical:None,pressed:true,modifiers:[false;4]}}
    #[test]
    fn terminal_quickload_0038_completed_end_key_opens_confirmation_then_real_restore() {
        let mut f=terminal_quickload_fixture_0038();begin(f.app.world_mut());
        let old_scope=terminal_scope_0038(f.app.world());assert_eq!(old_scope.finished,Some(true));
        let frame=capture(f.app.world(),old_scope.clone()).unwrap();
        let saved=std::fs::read(f.root.join("quicksave.json")).unwrap();
        let before=serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
        inject(f.app.world_mut(),frame,key_0038("F6")).unwrap();f.app.update();
        let operation=(0,crate::systems::save_menu::SaveMenuMode::Load);
        let confirmation=f.app.world().resource::<crate::systems::save_menu::SaveConfirmation>();
        assert_eq!(confirmation.pending,Some(operation),"physical EOF F6 must reach the existing confirmation gate");
        assert_eq!(confirmation.quick_return,Some(VnState::Finished));
        assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(),before);
        // Exercise the existing approved-operation receiver, without claiming UI approval.
        {let mut c=f.app.world_mut().resource_mut::<crate::systems::save_menu::SaveConfirmation>();c.pending=None;c.approved=Some(operation);}
        f.app.world_mut().send_event(PlayerInput::QuickLoad);f.app.update();
        assert!(!f.app.world().resource::<VnEngine>().0.is_finished());
        assert_eq!(f.app.world().resource::<VnEngine>().0.state.vars["score"],rvn_parser::Value::Int(7));
        assert!(f.app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        crate::scene_publication::consume(f.app.world_mut());
        assert!(!f.app.world().resource::<VnEngine>().0.renderer.scene_batch_pending());
        assert_ne!(f.app.world().resource::<VnEngine>().0.runtime_epoch(),old_scope.epoch);
        assert_eq!(std::fs::read(f.root.join("quicksave.json")).unwrap(),saved);
        assert!(f.app.world().resource::<ScriptErrorMessage>().0.is_empty());
    }
    #[test]
    fn terminal_quickload_0038_unknown_stale_paused_or_pending_images_cannot_open_confirmation() {
        for defect in 0..7 {
            let mut f=terminal_quickload_fixture_0038();begin(f.app.world_mut());
            let mut s=terminal_scope_0038(f.app.world());
            match defect {0=>s.finished=None,1=>s.finished=Some(false),2=>s.instance+=1,3=>s.epoch+=1,4=>s.position.pc+=1,5=>s.paused=true,_=>s.publication_pending=true}
            let frame=capture(f.app.world(),s).unwrap();
            let before=serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
            inject(f.app.world_mut(),frame,key_0038("F6")).unwrap();f.app.update();
            assert!(f.app.world().resource::<crate::systems::save_menu::SaveConfirmation>().pending.is_none(),"defect {defect}");
            assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(),before);
        }
    }
    #[test]
    fn terminal_quickload_0038_terminal_keys_cannot_advance_choose_rollback_or_overwrite_save() {
        for key in ["ReturnKey","Space","Key1","Backspace","F5"] {
            let mut f=terminal_quickload_fixture_0038();begin(f.app.world_mut());
            let frame=capture(f.app.world(),terminal_scope_0038(f.app.world())).unwrap();
            let before=serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
            let history=format!("{:?}",f.app.world().resource::<VnEngine>().0.history.entries());
            let saved=std::fs::read(f.root.join("quicksave.json")).unwrap();
            inject(f.app.world_mut(),frame,key_0038(key)).unwrap();f.app.update();
            assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(),before,"{key}");
            assert_eq!(format!("{:?}",f.app.world().resource::<VnEngine>().0.history.entries()),history,"{key}");
            assert_eq!(std::fs::read(f.root.join("quicksave.json")).unwrap(),saved,"{key}");
            assert!(f.app.world().resource::<crate::systems::save_menu::SaveConfirmation>().pending.is_none());
        }
    }
    #[test]
    fn terminal_quickload_0038_accessibility_and_consumed_keyboard_keep_their_gate() {
        for blocked in [false,true] {
            let mut f=terminal_quickload_fixture_0038();begin(f.app.world_mut());
            let frame=capture(f.app.world(),terminal_scope_0038(f.app.world())).unwrap();
            if blocked {f.app.world_mut().resource_mut::<crate::accessibility::Accessibility>().blocked=true;}
            else {f.app.world_mut().resource_mut::<crate::programmable_ui::Screens>().keyboard_consumed=true;}
            inject(f.app.world_mut(),frame,key_0038("F6")).unwrap();f.app.update();
            assert!(f.app.world().resource::<crate::systems::save_menu::SaveConfirmation>().pending.is_none());
        }
    }

    #[test]
    fn terminal_quickload_0038_native_end_key_uses_the_same_confirmation_service() {
        let mut f=terminal_quickload_fixture_0038();
        assert!(f.app.world().get_resource::<Routing>().is_none());
        let window={let world=f.app.world_mut();world.query_filtered::<Entity,With<Window>>().single(world)};
        f.app.world_mut().send_event(KeyboardInput{key_code:KeyCode::F6,logical_key:logical(KeyCode::F6),state:ButtonState::Pressed,window});
        f.app.update();
        let c=f.app.world().resource::<crate::systems::save_menu::SaveConfirmation>();
        assert_eq!(c.pending,Some((0,crate::systems::save_menu::SaveMenuMode::Load)));
        assert_eq!(c.quick_return,Some(VnState::Finished));
        assert!(f.app.world().resource::<VnEngine>().0.is_finished());
    }
    #[test]
    fn terminal_quickload_0038_queued_terminal_receipt_is_revoked_by_real_restore_before_receiver() {
        let mut f=terminal_quickload_fixture_0038();begin(f.app.world_mut());
        let frame=capture(f.app.world(),terminal_scope_0038(f.app.world())).unwrap();
        inject(f.app.world_mut(),frame,key_0038("F6")).unwrap();
        f.app.world_mut().run_system_once(bevy::input::keyboard::keyboard_input_system);
        f.app.world_mut().run_system_once(crate::systems::input::finished_quick_load_system);
        assert_eq!(f.app.world().resource::<Events<PlayerInput>>().len(),1);
        let saved=rvn_core::save::SaveManager::new(&f.root,crate::systems::save_menu::SUPPORTED_SLOTS).unwrap().load_quicksave().unwrap();
        f.app.world_mut().resource_mut::<VnEngine>().0.load_data(saved).unwrap();
        crate::scene_publication::consume(f.app.world_mut());
        let restored=serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap();
        f.app.world_mut().run_system_once(crate::systems::input::player_input_system);
        assert!(f.app.world().resource::<crate::systems::save_menu::SaveConfirmation>().pending.is_none());
        assert_eq!(serde_json::to_vec(&f.app.world().resource::<VnEngine>().0.state).unwrap(),restored);
    }
    #[test]
    fn actual_five_visible_nodes_keep_exact_preflighted_backing_capacity_without_geometric_growth(){
        let mut app=crate::programmable_ui::owner_hosts_0943::tests::actual_owner_image_world();let camera=crate::programmable_ui::owner_hosts_0943::installed_camera(app.world()).unwrap();let prior=app.world().resource::<bevy::ui::UiStack>().uinodes.len();assert!(prior<=5);
        for index in prior..5{app.world_mut().spawn((NodeBundle{style:Style{position_type:PositionType::Absolute,left:Val::Px((index*20)as f32),top:Val::Px(20.0),width:Val::Px(10.0),height:Val::Px(10.0),..default()},..default()},TargetCamera(camera)));}
        app.update();let stack_len=app.world().resource::<bevy::ui::UiStack>().uinodes.len();assert_eq!(stack_len,5);let source=app.world().resource::<VnEngine>().0.debug_state_source().unwrap();let scope=crate::programmable_ui::owner_hosts_0943::tests::actual_scope(&app,1);let expected=preflight_physical_frame(app.world(),&scope).unwrap();let frame=capture(app.world(),scope).unwrap();assert_eq!(frame.nodes.len(),5);assert_eq!(frame.nodes.capacity(),5,"the admitted five-node allocation must not grow to eight slots");assert!(frame.cost>=frame.nodes.capacity()*std::mem::size_of::<PhysicalNode>());assert_eq!(frame.cost,expected);assert!(frame.owner_snapshot().unwrap().current(app.world()));assert!(app.world().resource::<VnEngine>().0.accepts_debug_state_source(&source));
    }

}


/// A pending Up may describe release geometry, never grant an input action.
pub(crate) fn capture_widget_button_neutral_release1030(world:&mut World,frame:&Arc<PhysicalFrame>,input:&ImageInput){
    let ImageInput::Pointer{kind:PointerKind::Up,button,point,..}=input else{return;};
    if frame.scope.paused||frame.scope.publication_pending{return;}
    let point=Vec2::from_array(*point);let picked=frame.picked(point);
    let control=picked.filter(|node|node.control.is_some()&&node.control_current(world)).map(|node|node.entity);
    let pointer=PointerOverride{control,confirmation:None,pause:None,frame:frame.clone()};
    let Some(window)=world.query_filtered::<Entity,With<Window>>().iter(world).next()else{return;};
    crate::programmable_ui::widget_buttons1030::image_edge(world,&pointer,PointerKind::Up,*button,window,true);
}
