//! Transient DownAndUp state for nominal Widget Buttons only. No VM/schema/owner.
//! Physical edges retain their producer. Hover is only a frozen geometry verdict.
use super::*;
use std::collections::VecDeque;
const LIMIT:usize=32;
#[derive(Clone)]
struct Hit {entity:Entity,control:Control,source:UiInputSource,semantic:[u8;32],geometry:[u8;32],window:Entity,image:Option<(u64,u64,u64)>}
impl Hit {
    fn same(&self,other:&Self)->bool {self.entity==other.entity&&self.window==other.window&&self.source==other.source
        &&self.control.screen==other.control.screen&&self.control.element==other.control.element&&self.control.option==other.control.option&&self.semantic==other.semantic&&self.geometry==other.geometry}
}
enum Edge {Down(Hit),Up{window:Entity,hit:Option<Hit>,image:bool}}
struct Press {original:Hit,released:Option<bool>,focus:Option<OwnFocus>}
// Minted only by our synchronous successful Focus call with no previous batch.
// The pointer observes the existing pending Box; it is NOT an owner/capability.
struct OwnFocus {source:rvn_core::engine::DebugStateSource,batch:[u8;32],address:usize}
fn batch_image(batch:&rvn_core::renderer::SceneRenderBatch)->Option<[u8;32]>{
    use sha2::{Digest,Sha256};
    struct Sink{hash:Sha256,bytes:usize}
    impl std::io::Write for Sink{
        fn write(&mut self,data:&[u8])->std::io::Result<usize>{
            self.bytes=self.bytes.checked_add(data.len()).filter(|n|*n<=2*1024*1024).ok_or_else(||std::io::Error::other("Button Focus after-image budget"))?;
            self.hash.update(data);Ok(data.len())
        }
        fn flush(&mut self)->std::io::Result<()>{Ok(())}
    }
    // Host drains speech/menu deliveries, not these original Core fields.
    // Stream into a fixed digest: never allocate/cache a second GameState/AST.
    let mut out=Sink{hash:Sha256::new(),bytes:0};
    serde_json::to_writer(&mut out,&(&batch.previous,&batch.state,&batch.interfaces,batch.runtime_epoch,batch.interface_epoch,batch.video_epoch)).ok()?;
    Some(out.hash.finalize().into())
}
impl OwnFocus {
    fn from_actual(engine:&VnEngine)->Option<Self>{
        if crate::editor_debugger::enabled(engine)||crate::editor_debugger::pending(engine){return None;}
        let batch=engine.0.renderer.scene_batch.as_deref()?;
        if batch.runtime_epoch!=engine.0.runtime_epoch()||batch.interface_epoch!=engine.0.interface_epoch(){return None;}
        Some(Self{source:engine.0.debug_state_source().ok()?,batch:batch_image(batch)?,address:batch as *const _ as usize})
    }
    fn pending(&self,engine:&VnEngine)->bool {engine.0.accepts_debug_state_source(&self.source)
        &&engine.0.renderer.scene_batch.as_deref().is_some_and(|batch|batch as *const _ as usize==self.address&&batch_image(batch).as_ref()==Some(&self.batch))}
    fn installed(&self,engine:&VnEngine,presentation:Option<&crate::scene_publication::ScenePresentation>)->bool {
        !engine.0.renderer.scene_batch_pending()&&!engine.0.renderer.scene_frame_failed
            &&engine.0.accepts_debug_state_source(&self.source)
            &&presentation.and_then(|p|p.batch.as_ref()).and_then(batch_image).as_ref()==Some(&self.batch)
    }
}
// Retain an already painted physical image while OUR exact Focus publication
// is waiting for the ordinary backend. This is not a published after-image,
// an input grant, or permission to prepare another owner's pending state.
pub(super) fn pending_paint(engine:Option<&VnEngine>,screens:&Screens,prepared:&WidgetLayouts,metric:Option<&WidgetMetrics>,fonts:Option<&Assets<Font>>)->bool {
    let Some(engine)=engine else{return false;};
    let Some(press)=screens.widget_buttons1030.press.as_ref()else{return false;};
    let Some(gate)=press.focus.as_ref()else{return false;};
    if !gate.pending(engine)||!component_current(engine,screens,&press.original){return false;}
    let Some(batch)=engine.0.renderer.scene_batch.as_deref()else{return false;};
    let before=&batch.previous.ui;let after=&batch.state.ui;
    if before.next_order!=after.next_order||before.host_next_order!=after.host_next_order||before.instance_registry!=after.instance_registry
        ||before.screens.len()!=after.screens.len()||before.screens.iter().zip(&after.screens).any(|(old,new)|{
            let mut old=old.clone();old.focus=new.focus.clone();old!=*new
        }) {return false;}
    let same_focus_only=|left:&[ScreenView],right:&[ScreenView]|left.len()==right.len()&&left.iter().zip(right).all(|(old,new)|{
        let mut old=old.clone();old.focus=new.focus.clone();old==*new
    });
    if !same_focus_only(&screens.rendered,&screens.views)||!same_focus_only(&screens.rendered,&batch.interfaces)
        ||prepared.problem.is_some()||!prepared.pending_names.is_empty()||prepared.plans!=screens.widget_plans{return false;}
    let Some(fonts)=fonts else{return false;};
    if prepared.plans.is_empty()||prepared.plans.values().any(|plan|{
        metric.is_some_and(|metric|plan.metrics!=*metric)||!widget_plan_fonts_current(plan,fonts)
            ||screens.rendered_sources.get(&plan.source.screen)!=Some(&plan.source)
            ||!engine.0.ui_input_source_current(&plan.source)
    }) {return false;}
    rendered_geometry_in(screens,&press.original.control,&press.original.source)==Some(press.original.geometry)
}
#[derive(Default)]
pub(super) struct State {
    edges:VecDeque<Edge>,press:Option<Press>,overflowed:bool,
    #[cfg(not(target_arch="wasm32"))] reader:bevy::ecs::event::ManualEventReader<bevy::winit::WinitEvent>,
    cursors:HashMap<Entity,Vec2>,
}
impl State {
    pub(super) fn cancel(&mut self){self.edges.clear();self.press=None;self.overflowed=false;}
    pub(super) fn pressed(&self,entity:Entity)->bool {self.press.as_ref().is_some_and(|p|p.original.entity==entity&&p.released.is_none())}
    fn queue(&mut self,edge:Edge){if self.overflowed{return;}if self.edges.len()>=LIMIT{self.cancel();self.overflowed=true;return;}self.edges.push_back(edge);}
    pub(super) fn revoke(&mut self,sources:&[UiInputSource]){
        if self.press.as_ref().is_some_and(|p|sources.contains(&p.original.source)){self.cancel();}
        self.edges.retain(|e|match e{Edge::Down(h)=>!sources.contains(&h.source),Edge::Up{hit,..}=>hit.as_ref().is_none_or(|h|!sources.contains(&h.source))});
    }
    fn preserve_own_released_image(&mut self){
        if !self.press.as_ref().is_some_and(|p|p.original.image.is_some()&&p.focus.is_some()&&p.released==Some(true)) {self.cancel();}
    }
    fn up(&mut self,window:Entity,hit:Option<&Hit>)->bool{
        if let Some(press)=self.press.as_mut().filter(|p|p.original.window==window){
            if press.released.is_none(){press.released=Some(hit.is_some_and(|hit|press.original.same(hit)));}
            return true;
        }false
    }
}
pub(super) fn nominal(engine:&VnEngine,view:&ScreenView,component:&UiComponent)->bool {
    component.kind==Kind::Button&&view.root.layout_contract.is_some()
        &&engine.0.state.ui.screens.iter().any(|screen|screen.name==view.name&&screen.order==view.order&&screen.widget_class.is_some())
}
// A bounded ephemeral digest of the rendered layout receipt, not an owner or
// a second AST. Fixed-size per PhysicalNode; frame budget charges that field.
fn geometry(view:&ScreenView,plan:Option<&WidgetPlan>)->Option<[u8;32]>{
    use sha2::{Digest,Sha256};use std::fmt::Write;
    struct Sink{hash:Sha256,bytes:usize}
    impl std::fmt::Write for Sink{fn write_str(&mut self,s:&str)->std::fmt::Result{
        self.bytes=self.bytes.checked_add(s.len()).filter(|n|*n<=2*1024*1024).ok_or(std::fmt::Error)?;self.hash.update(s.as_bytes());Ok(())}}
    let mut out=Sink{hash:Sha256::new(),bytes:0};write!(&mut out,"{:?}",widget_definition_digest(view).ok()?).ok()?;
    if let Some(plan)=plan{
        let mut fonts=plan.fonts.iter().collect::<Vec<_>>();fonts.sort_by_key(|(id,_)|format!("{id:?}"));
        write!(&mut out,"{:?}",(&plan.definition,&plan.rects,&plan.source,plan.interactive,&plan.projection,&plan.metrics,fonts)).ok()?;
    }else{out.write_str("no rendered plan").ok()?;}
    Some(out.hash.finalize().into())
}
pub(crate) fn is_rendered_button(world:&World,control:&Control)->bool{
    world.get_resource::<VnEngine>().zip(world.get_resource::<Screens>()).is_some_and(|(engine,screens)|
        screens.rendered.iter().find(|view|view.name==control.screen).and_then(|view|view.root.find(&control.element).map(|component|(view,component))).is_some_and(|(view,component)|nominal(engine,view,component)))
}
pub(crate) fn rendered_geometry(world:&World,control:&Control,source:&ControlSource)->Option<[u8;32]>{
    if !is_rendered_button(world,control){return None;}
    let screens=world.get_resource::<Screens>()?;rendered_geometry_in(screens,control,&source.0)
}
fn rendered_geometry_in(screens:&Screens,control:&Control,source:&UiInputSource)->Option<[u8;32]>{
    if screens.rendered_sources.get(&control.screen)!=Some(source){return None;}
    let view=screens.rendered.iter().find(|v|v.name==control.screen&&v.order==source.order)?;
    geometry(view,screens.widget_plans.get(&source.to_wire().ok()?))
}
pub(crate) fn geometry_current(world:&World,control:&Control,source:&ControlSource,proof:[u8;32])->bool{
    world.get_resource::<Screens>().is_some_and(|screens|geometry_current_in(screens,world.get_resource::<WidgetLayouts>(),control,&source.0,proof))
}
fn geometry_current_in(screens:&Screens,prepared:Option<&WidgetLayouts>,control:&Control,source:&UiInputSource,proof:[u8;32])->bool{
    let Some(view)=screens.views.iter().find(|v|v.name==control.screen&&v.order==source.order)else{return false;};
    let Some(key)=source.to_wire().ok()else{return false;};
    geometry(view,prepared.and_then(|p|p.plans.get(&key)))==Some(proof)
}
fn component_current(engine:&VnEngine,screens:&Screens,hit:&Hit)->bool {
    screens.active&&!crate::editor_debugger::enabled(engine)&&!crate::editor_debugger::pending(engine)&&engine.0.ui_input_source_current(&hit.source)
        &&screens.rendered_sources.get(&hit.control.screen)==Some(&hit.source)
        &&screens.views.iter().find(|v|v.name==hit.control.screen&&v.order==hit.source.order).is_some_and(|view|
            view.root.find(&hit.control.element).is_some_and(|component|nominal(engine,view,component)&&view.root.available(&component.id)
                &&pointer_component_digest(component,true)==Some(hit.semantic))
            &&!screens.views.iter().any(|other|other.modal&&(other.layer,other.order)>(view.layer,view.order)))
        &&hit.image.is_none_or(|(instance,epoch,_)|instance==engine.0.instance_id()&&epoch==engine.0.runtime_epoch())
}
fn world_current(world:&World,screens:&Screens,hit:&Hit)->bool {
    let Some(engine)=world.get_resource::<VnEngine>()else{return false;};
    component_current(engine,screens,hit)
        &&geometry_current_in(screens,world.get_resource::<WidgetLayouts>(),&hit.control,&hit.source,hit.geometry)
        &&world.get::<Control>(hit.entity).is_some_and(|c|c.screen==hit.control.screen&&c.element==hit.control.element&&c.option==hit.control.option)
        &&world.get::<ControlSource>(hit.entity).is_some_and(|s|s.0==hit.source)
        &&world.get::<ViewVisibility>(hit.entity).is_some_and(|v|v.get())
        &&world.get_resource::<bevy::ui::UiStack>().is_some_and(|s|s.uinodes.contains(&hit.entity))
        &&world.get::<Window>(hit.window).is_some_and(|window|window.focused)
}
fn hit_entity(world:&World,screens:&Screens,entity:Entity,window:Entity,image:Option<(u64,u64,u64)>,frozen:Option<[u8;32]>)->Option<Hit>{
    let control=world.get::<Control>(entity)?.clone();let source=world.get::<ControlSource>(entity)?.0.clone();
    let component=screens.rendered.iter().find(|v|v.name==control.screen)?.root.find(&control.element)?;
    let proof=match frozen{Some(p)=>p,None=>rendered_geometry_in(screens,&control,&source)?};
    let hit=Hit{entity,control,source,semantic:pointer_component_digest(component,true)?,geometry:proof,window,image};
    world_current(world,screens,&hit).then_some(hit)
}
fn pick(world:&World,point:Vec2)->Option<Entity>{
    if !point.is_finite(){return None;}
    let stack=world.get_resource::<bevy::ui::UiStack>()?;if stack.uinodes.len()>4096{return None;}
    for &entity in stack.uinodes.iter().rev(){
        let(Some(node),Some(transform),Some(visible))=(world.get::<Node>(entity),world.get::<GlobalTransform>(entity),world.get::<ViewVisibility>(entity))else{continue;};
        if !visible.get()||world.get::<bevy::ui::CalculatedClip>(entity).is_some_and(|clip|!clip.clip.contains(point))||!transformed_contains(node.size(),transform,point){continue;}
        if world.get::<Control>(entity).is_some(){return Some(entity);}
        if world.get::<FocusPolicy>(entity)==Some(&FocusPolicy::Block){return None;}
    }None
}
/// Producer pass runs while Focus publication is pending as well. Ordered Winit
/// positions are consumed here, not reconstructed from a later hover/Window.
pub(super) fn capture(world:&mut World){
    let Some(mut screens)=world.remove_resource::<Screens>()else{return;};
    let allowed=screens.active&&crate::runtime_restart::input_available(world)&&!world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|a|a.blocked)
        &&!crate::editor_preview::paused(world)&&!crate::checkpoint_requests::world_blocked(world)
        &&!world.get_resource::<VnEngine>().is_some_and(|e|e.0.renderer.scene_frame_failed||e.0.renderer.menu_pending);
    if !allowed{screens.widget_buttons1030.cancel();}
    if screens.widget_buttons1030.press.as_ref().is_some_and(|p|!world_current(world,&screens,&p.original)) {screens.widget_buttons1030.cancel();}
    if let Some(press)=screens.widget_buttons1030.press.as_ref(){if let Some(gate)=&press.focus{
        match world.get_resource::<VnEngine>(){
            Some(engine)if gate.pending(engine)=>{},
            Some(engine)if gate.installed(engine,world.get_resource::<crate::scene_publication::ScenePresentation>())=>{},
            _=>screens.widget_buttons1030.cancel(),
        }
    }}
    #[cfg(not(target_arch="wasm32"))]
    {
        let mut reader=std::mem::take(&mut screens.widget_buttons1030.reader);
        if let Some(events)=world.get_resource::<Events<bevy::winit::WinitEvent>>(){
            for event in reader.read(events){
                use bevy::winit::WinitEvent;
                match event{
                    WinitEvent::CursorMoved(e)=>{screens.widget_buttons1030.cursors.insert(e.window,e.position);},
                    WinitEvent::CursorLeft(e)=>{screens.widget_buttons1030.cursors.remove(&e.window);},
                    WinitEvent::WindowFocused(e)if !e.focused=>{screens.widget_buttons1030.cancel();screens.widget_buttons1030.cursors.remove(&e.window);},
                    WinitEvent::MouseButtonInput(e)if e.button==MouseButton::Left=>{
                        if !allowed{continue;}
                        let hit=screens.widget_buttons1030.cursors.get(&e.window).and_then(|point|pick(world,*point)).and_then(|entity|hit_entity(world,&screens,entity,e.window,None,None));
                        if e.state==ButtonState::Pressed{if !world.get_resource::<VnEngine>().is_some_and(|e|e.0.renderer.scene_batch_pending()||crate::editor_debugger::pending(e)){if let Some(hit)=hit{screens.widget_buttons1030.queue(Edge::Down(hit));}}}
                        else if !screens.widget_buttons1030.up(e.window,hit.as_ref()){screens.widget_buttons1030.queue(Edge::Up{window:e.window,hit,image:false});}
                    },_=>{},
                }
            }
        }
        screens.widget_buttons1030.reader=reader;
    }
    world.insert_resource(screens);
}
/// Called only by the admitted image adapter after it checked the original
/// PhysicalNode. Descriptive input capture only; no callback or VM runs here.
pub(crate) fn image_edge(world:&mut World,pointer:&crate::editor_preview_input::PointerOverride,kind:rvn_core::editor_preview::PointerKind,button:u8,window:Entity,neutral:bool)->bool{
    use rvn_core::editor_preview::PointerKind;
    if button!=0||!matches!(kind,PointerKind::Down|PointerKind::Up){return false;}
    let physical=pointer.button_entity_current1030(world);
    let Some(mut screens)=world.remove_resource::<Screens>()else{return false;};
    let allowed=screens.active&&crate::runtime_restart::input_available(world)&&!crate::editor_preview::paused(world)
        &&!world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|a|a.blocked)
        &&!crate::checkpoint_requests::world_blocked(world)
        &&!world.get_resource::<VnEngine>().is_some_and(|e|e.0.renderer.scene_frame_failed||e.0.renderer.menu_pending);
    let hit=physical.and_then(|(entity,proof)|hit_entity(world,&screens,entity,window,Some(pointer.image()),Some(proof)));
    let mut recorded=false;
    if allowed{
        if kind==PointerKind::Down&&!neutral{if !world.get_resource::<VnEngine>().is_some_and(|e|e.0.renderer.scene_batch_pending()||crate::editor_debugger::pending(e)){if let Some(hit)=hit{screens.widget_buttons1030.queue(Edge::Down(hit));recorded=true;}}}
        else if kind==PointerKind::Up{
            let pending_own=screens.widget_buttons1030.press.as_ref().and_then(|p|p.focus.as_ref()).is_some_and(|gate|world.get_resource::<VnEngine>().is_some_and(|engine|gate.pending(engine)));
            if !neutral||pending_own{
                let retained=screens.widget_buttons1030.up(window,hit.as_ref());
                if !neutral&&!retained{screens.widget_buttons1030.queue(Edge::Up{window,hit,image:true});}
                recorded=true;
            }else{screens.widget_buttons1030.cancel();}
        }
    }else{screens.widget_buttons1030.cancel();}
    world.insert_resource(screens);recorded
}
pub(crate) fn neutral_preview_up(world:&mut World){
    if let Some(mut screens)=world.get_resource_mut::<Screens>(){
        if screens.pending_press.as_ref().is_some_and(|p|p.image.is_some()){screens.pending_press=None;}
        if screens.dragging_preview.take().is_some(){screens.dragging=None;screens.dragging_source=None;}
        screens.widget_buttons1030.preserve_own_released_image();
    }
}
/// Existing input system calls this under its actual ready/availability gates.
pub(super) fn process(screens:&mut Screens,engine:&mut VnEngine,controls:&Query<(Entity,&Control,&Interaction,&Node,&GlobalTransform,Option<&ControlSource>)>,windows:&Query<&mut Window>,output:&mut EventWriter<VnCommand>,error:&mut ScriptErrorMessage,next:&mut NextState<VnState>,presentation:Option<&crate::scene_publication::ScenePresentation>,prepared:Option<&WidgetLayouts>)->bool{
    if screens.widget_buttons1030.overflowed{screens.widget_buttons1030.cancel();screens.widget_buttons1030.overflowed=false;return false;}
    if screens.widget_buttons1030.press.as_ref().is_some_and(|p|!component_current(engine,screens,&p.original)
        ||!geometry_current_in(screens,prepared,&p.original.control,&p.original.source,p.original.geometry)
        ||controls.get(p.original.entity).ok().is_none_or(|(_,c,_,_,_,s)|c.screen!=p.original.control.screen||c.element!=p.original.control.element||c.option!=p.original.control.option||s.is_none_or(|s|s.0!=p.original.source))
        ||windows.get(p.original.window).ok().is_none_or(|w|!w.focused)){screens.widget_buttons1030.cancel();}
    // Only the real dispatch phase may consume our Focus receipt. PreUpdate
    // may have observed it, but no observation there grants a later Click.
    if let Some(gate)=screens.widget_buttons1030.press.as_ref().and_then(|p|p.focus.as_ref()){
        if gate.pending(engine){screens.pointer_consumed=true;return true;}
        if !gate.installed(engine,presentation){screens.widget_buttons1030.cancel();return false;}
        screens.widget_buttons1030.press.as_mut().unwrap().focus=None;
    }
    if screens.widget_buttons1030.press.as_ref().is_some_and(|p|screens.keyboard_focus.as_ref()!=Some(&(p.original.control.screen.clone(),p.original.control.element.clone()))){screens.widget_buttons1030.cancel();}
    loop{
        if screens.widget_buttons1030.press.as_ref().is_some_and(|p|p.released.is_some()){
            let press=screens.widget_buttons1030.press.take().unwrap();screens.pointer_consumed=true;
            if press.released==Some(true)&&component_current(engine,screens,&press.original){
                screens.dispatch_source=Some(press.original.source);
                let _=send(engine,event(&press.original.control.screen,&press.original.control.element,EventKind::Click,None),screens,output,error,next);
                return true;
            }
        }
        let Some(edge)=screens.widget_buttons1030.edges.pop_front()else{break;};
        match edge{
            Edge::Down(hit)=>{
                if screens.widget_buttons1030.press.is_some()||!component_current(engine,screens,&hit)
                    ||!geometry_current_in(screens,prepared,&hit.control,&hit.source,hit.geometry)
                    ||windows.get(hit.window).ok().is_none_or(|w|!w.focused){continue;}
                if controls.get(hit.entity).ok().is_none_or(|(_,c,_,_,_,s)|c.screen!=hit.control.screen||c.element!=hit.control.element||s.is_none_or(|s|s.0!=hit.source)){continue;}
                screens.pointer_consumed=true;
                // Debugger admission can be asynchronous; there is no original
                // Focus receipt port here. Never infer one from its bool.
                if engine.0.renderer.scene_batch_pending()||crate::editor_debugger::enabled(engine){continue;}
                screens.dispatch_source=Some(hit.source.clone());
                if !send(engine,event(&hit.control.screen,&hit.control.element,EventKind::Focus,None),screens,output,error,next){continue;}
                let focus=if engine.0.renderer.scene_batch_pending(){
                    let Some(gate)=OwnFocus::from_actual(engine)else{continue;};Some(gate)
                }else{None};
                screens.keyboard_focus=Some((hit.control.screen.clone(),hit.control.element.clone()));
                screens.widget_buttons1030.press=Some(Press{original:hit,released:None,focus});
                if screens.widget_buttons1030.press.as_ref().is_some_and(|p|p.focus.is_some()){return true;}
            },
            Edge::Up{window,hit,image}=>{
                let _=image;let _=screens.widget_buttons1030.up(window,hit.as_ref());
            },
        }
    }
    if screens.widget_buttons1030.press.is_some(){screens.pointer_consumed=true;}
    false
}
#[cfg(test)]
#[path="programmable_button_down_up_tests1030.rs"]
mod tests;
