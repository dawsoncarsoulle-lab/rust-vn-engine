//! Source-fenced logical/physical owner projection. Not an input authority.
//! The full release, focus and preview ports are still a private integration WIP.
use super::*;
use rvn_ui::programmable::{HostGeometry,ScreenHostProperties};
use rvn_core::scene_resources::TargetIdentity;
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub(crate) struct OwnerPublicationSource{pub instance:u64,pub epoch:u64,pub interface_epoch:u64}
#[derive(Debug,Clone,PartialEq)]
pub(crate) struct OwnerDescription{
    pub widget_class:Option<rvn_core::widget_class_source_0950::WidgetClassIdentity>,
    pub pending_materialization:bool,
    pub name:String,pub order:u64,pub role:Option<rvn_ui::PageRole>,pub target:Option<TargetIdentity>,
    pub layer:i32,pub modal:bool,pub properties:ScreenHostProperties,pub focus_request:Option<rvn_ui::programmable::HostFocusRequest>,pub has_child:bool,
}
#[derive(Clone,PartialEq)]
pub(crate) struct PreparedOwnerProjection{source:OwnerPublicationSource,descriptions:Vec<OwnerDescription>,state_digest:[u8;32]}
#[derive(Clone,PartialEq)]
pub(crate) struct PreparedOwnerGeometry{projection:PreparedOwnerProjection,metric:WidgetMetrics,geometry:Vec<Option<HostGeometry>>}
/// Actual owner selection is shared by capture and current checks. Neither
/// a pending publication nor ambiguous player/startup ownership grants input.
pub(crate) fn installed_input_source_current(world:&World,source:&UiInputSource)->bool{
 let Ok(actual)=crate::text_runtime_owner::current(world)else{return false;};if actual.blocked{return false;}
 match(world.get_resource::<VnEngine>(),world.get_resource::<crate::debugger_startup::DebugBootstrap>()){
  (Some(engine),None)=>engine.0.ui_input_source_current(source),
  (None,Some(startup))=>startup.owner.ui_input_source_current(source),
  _=>false,
 }
}
fn capture_installed_input_source(world:&World,screen:&str)->Option<UiInputSource>{
 let actual=crate::text_runtime_owner::current(world).ok()?;if actual.blocked{return None;}
 match(world.get_resource::<VnEngine>(),world.get_resource::<crate::debugger_startup::DebugBootstrap>()){
  (Some(engine),None)=>engine.0.capture_ui_input_source(screen),
  (None,Some(startup))=>startup.owner.capture_ui_input_source(screen),
  _=>None,
 }
}
#[derive(Resource)]
struct StartupWidgetBackend{source:OwnerPublicationSource,backend:crate::widget_backend_0956::WidgetBackend}
pub(crate) fn install_startup_backend(world:&mut World,source:OwnerPublicationSource,backend:crate::widget_backend_0956::WidgetBackend){
 world.insert_resource(StartupWidgetBackend{source,backend});
}
fn digest(ui:&rvn_core::ui::UiState)->Result<[u8;32],String>{use sha2::Digest;Ok(sha2::Sha256::digest(serde_json::to_vec(ui).map_err(|e|e.to_string())?).into())}
/// Read the already authenticated actual Core candidate; no alias is turned
/// into a producer token here. No child tree is duplicated in the host plan.
pub(crate) fn prepare(state:&rvn_core::GameState,children:&[ScreenView],source:OwnerPublicationSource)->Result<PreparedOwnerProjection,String>{
    if source.instance==0{return Err("Owner projection requires actual nonzero Engine lineage".into());}
    let views=rvn_core::ui::published_owner_hosts(&state.ui,children)?;let mut descriptions=Vec::new();
    for view in views{
        let instance=state.ui.screens.iter().find(|instance|instance.name==view.name&&instance.order==view.order).ok_or("Prepared owner has no actual resident instance")?;
        let target=if state.scene_execution.is_some()||state.ui.instance_registry.is_some(){
            let target=rvn_core::ui_residency::visible_interface_target(state,&view.name)?;
            let resident=rvn_core::ui_residency::resident_interface(state,&target)?;
            if resident.residence!=rvn_core::ui_residency::ProjectionResidence::Visible||resident.instance.order!=view.order||resident.instance.host_role!=instance.host_role{return Err("Prepared owner differs from actual visible resident incarnation".into());}
            Some(target)
        }else{None};
        descriptions.push(OwnerDescription{widget_class:instance.widget_class.as_ref().map(|binding|binding.class.clone()),pending_materialization:instance.widget_class.as_ref().is_some_and(|binding|binding.pending_materialization),name:view.name,order:view.order,role:instance.host_role,target,layer:view.layer,modal:view.modal,properties:view.properties,focus_request:instance.owner_focus_request.clone(),has_child:view.child.is_some()});
    }
    Ok(PreparedOwnerProjection{source,descriptions,state_digest:digest(&state.ui)?})
}
/// Logical snapshot installed by the actual ScenePublication fence, before
/// geometry/wrappers. It is not a focus/input grant or a second Core state.
#[derive(Resource)]
pub(crate) struct OwnerSceneProjection{projection:PreparedOwnerProjection}
impl OwnerSceneProjection{
 pub(crate) fn new(projection:PreparedOwnerProjection)->Self{Self{projection}}
 pub(crate) fn projection(&self)->&PreparedOwnerProjection{&self.projection}
}
pub(crate) fn prepare_scene(batch:&rvn_core::renderer::SceneRenderBatch,instance:u64)->Result<PreparedOwnerProjection,String>{prepare(&batch.state,&batch.interfaces,OwnerPublicationSource{instance,epoch:batch.runtime_epoch,interface_epoch:batch.interface_epoch})}
impl PreparedOwnerProjection{
    pub(crate) fn source(&self)->OwnerPublicationSource{self.source}
    pub(crate) fn descriptions(&self)->&[OwnerDescription]{&self.descriptions}
    pub(crate) fn matches_state(&self,state:&rvn_core::GameState)->bool{digest(&state.ui).ok()==Some(self.state_digest)}
}
/// Child desired sizes must come from the existing real font/layout measure.
/// Empty owners need no invented child size. Minimum desired size is not an
/// allocation clamp. The metric belongs to the actual camera render target.
pub(crate) fn prepare_geometry(world:&World,projection:PreparedOwnerProjection,metric:WidgetMetrics,child_desired:&HashMap<String,[f32;2]>)->Result<PreparedOwnerGeometry,String>{
    if current_widget_camera_metrics(world,&metric).as_ref()!=Some(&metric){return Err("Owner geometry waits for matching actual camera/render-target metrics".into());}
    prepare_geometry_with_metric(projection,metric,child_desired)
}
pub(super) fn prepare_geometry_with_metric(projection:PreparedOwnerProjection,metric:WidgetMetrics,child_desired:&HashMap<String,[f32;2]>)->Result<PreparedOwnerGeometry,String>{
    let mut geometry=Vec::new();
    for owner in &projection.descriptions{
        if owner.pending_materialization{geometry.push(None);continue;}
        // Only an autosized allocation depends on intrinsic child size.
        // A fixed/fill slot never fabricates or clamps its allocation from it.
        let child=if owner.has_child&&owner.properties.viewport_slot.auto_size(){*child_desired.get(&owner.name).ok_or("Owner autosize waits for actual measured child desired size")?}else{child_desired.get(&owner.name).copied().unwrap_or([0.0,0.0])};
        let desired=owner.properties.desired_size(child)?;
        geometry.push(Some(owner.properties.viewport_slot.geometry(metric.bounds,desired,metric.physical_scale)?));
    }
    Ok(PreparedOwnerGeometry{projection,metric,geometry})
}
impl PreparedOwnerGeometry{
    pub(crate) fn projection(&self)->&PreparedOwnerProjection{&self.projection}
    pub(super) fn child_bounds(&self,name:&str,order:u64)->Option<[f32;4]>{self.rows().find(|(owner,_)|owner.name==name&&owner.order==order).map(|(_,geometry)|[0.0,0.0,geometry.rect[2].abs(),geometry.rect[3].abs()])}

    pub(crate) fn rows(&self)->impl Iterator<Item=(&OwnerDescription,&HostGeometry)>{self.projection.descriptions.iter().zip(&self.geometry).filter_map(|(owner,geometry)|geometry.as_ref().map(|geometry|(owner,geometry)))}
    pub(crate) fn metric_current(&self,world:&World)->bool{current_widget_camera_metrics(world,&self.metric).as_ref()==Some(&self.metric)}
    /// An actual installed scene fence is mandatory before these rows could be
    /// used as display/input observations. Preparation is not publication.
    pub(crate) fn installed_current(&self,world:&World)->bool{
        let Some(batch)=world.get_resource::<crate::scene_publication::ScenePresentation>().and_then(|presentation|presentation.batch.as_ref())else{return false;};
        let Ok(actual)=crate::text_runtime_owner::current(world)else{return false;};
        actual.instance==self.projection.source.instance&&actual.epoch==self.projection.source.epoch
            &&!actual.blocked&&batch.runtime_epoch==self.projection.source.epoch&&batch.interface_epoch==self.projection.source.interface_epoch
            &&self.projection.matches_state(&batch.state)&&self.metric_current(world)
    }
}

/// A real generic owner host, not an authored Component, Control or Panel.
/// Its existence does not by itself grant an external input capability.
#[derive(Component,Clone)]
pub(crate) struct OwnerHostNode{
    identity:[u8;32],pub description:OwnerDescription,pub source:OwnerPublicationSource,
    pub wrapper_generation:u64,pub geometry:HostGeometry,
    backend:Option<std::sync::Weak<crate::widget_backend_0956::WidgetWrapper>>,
}
#[derive(Resource,Default)]
pub(crate) struct OwnerHostRuntime{next_generation:u64,nodes:std::collections::BTreeMap<[u8;32],Entity>,installed:Option<InstalledOwnerFence>}
#[derive(Clone)]
struct InstalledOwnerFence{source:OwnerPublicationSource,state_digest:[u8;32],metric:WidgetMetrics}
fn logical_owner_current(world:&World,node:&OwnerHostNode)->bool{
 if node.description.widget_class.is_some()&&node.backend.as_ref().and_then(|weak|weak.upgrade()).is_none_or(|backend|!backend.attached()||backend.generation()!=node.wrapper_generation){return false;}
 let Some(scene)=world.get_resource::<OwnerSceneProjection>()else{return true;};
 scene.projection.source.instance==node.source.instance&&scene.projection.source.epoch==node.source.epoch
  &&scene.projection.descriptions.iter().any(|owner|owner==&node.description)
}
/// Immutable provenance for the actual installed owner wrapper. Raw Main input
/// never supplies this struct and an owner name alone cannot construct it.
#[derive(Clone,Debug,PartialEq)]
pub(crate) struct CapturedOwnerRow{
    pub source:UiInputSource,pub publication:OwnerPublicationSource,pub wrapper_generation:u64,
    pub geometry:HostGeometry,pub properties:ScreenHostProperties,pub focus_request:Option<rvn_ui::programmable::HostFocusRequest>,pub layer:i32,pub order:u64,pub modal:bool,
}
#[derive(Clone)]
pub(crate) struct CapturedOwnerFrame{rows:std::sync::Arc<[CapturedOwnerRow]>,metric:WidgetMetrics,source:OwnerPublicationSource,state_digest:[u8;32],cost:usize}
impl CapturedOwnerFrame{
    pub(crate) fn rows(&self)->&[CapturedOwnerRow]{&self.rows}
    pub(crate) fn camera(&self)->Entity{self.metric.camera}
    pub(crate) fn belongs_to(&self,instance:u64,epoch:u64)->bool{self.source.instance==instance&&self.source.epoch==epoch}
    pub(crate) fn owner_entity(&self,world:&World,row:usize)->Option<Entity>{
        if !self.current(world){return None;}let row=self.rows.get(row)?;
        world.get_resource::<OwnerHostRuntime>()?.nodes.values().copied().find(|entity|world.get::<OwnerHostNode>(*entity).is_some_and(|node|node.wrapper_generation==row.wrapper_generation&&node.source.instance==row.publication.instance&&node.source.epoch==row.publication.epoch&&node.description.name==row.source.screen&&node.description.order==row.source.order))
    }
    pub(crate) fn cost(&self)->usize{self.cost}
    pub(crate) fn hosted(&self,world:&World,row:&CapturedOwnerRow)->bool{
        let Some(runtime)=world.get_resource::<OwnerHostRuntime>()else{return false;};
        runtime.nodes.values().any(|entity|world.get::<OwnerHostNode>(*entity).is_some_and(|node|node.wrapper_generation==row.wrapper_generation&&node.source.instance==row.publication.instance&&node.source.epoch==row.publication.epoch&&node.description.name==row.source.screen&&node.description.order==row.source.order)
            &&world.get::<Node>(*entity).is_some_and(|node|{let bounds=row.geometry.bounding_rect();let actual=node.unrounded_size();(actual.x-bounds[2]).abs()<=0.01&&(actual.y-bounds[3]).abs()<=0.01})
            &&world.get::<GlobalTransform>(*entity).is_some()
            &&world.get::<ViewVisibility>(*entity).is_some_and(|visible|visible.get())
            &&world.get_resource::<bevy::ui::UiStack>().is_some_and(|stack|stack.uinodes.contains(entity)))
    }
    pub(crate) fn current(&self,world:&World)->bool{
        let Some(runtime)=world.get_resource::<OwnerHostRuntime>()else{return false;};
        let Some(fence)=&runtime.installed else{return false;};
        if fence.source.instance!=self.source.instance||fence.source.epoch!=self.source.epoch||fence.metric!=self.metric{return false;}
        if current_widget_camera_metrics(world,&self.metric).as_ref()!=Some(&self.metric){return false;}
        let Ok(actual)=crate::text_runtime_owner::current(world)else{return false;};
        if actual.blocked||actual.instance!=self.source.instance||actual.epoch!=self.source.epoch{return false;}
        // `actual` above already requires exactly one live player/startup
        // owner. Both expose the same read-only source admission; an Init
        // blocking grid must never fabricate a VnEngine for its UI wrappers.
        let input_current=|source:&UiInputSource|installed_input_source_current(world,source);
        let Some(batch)=world.get_resource::<crate::scene_publication::ScenePresentation>().and_then(|value|value.batch.as_ref())else{return false;};
        if batch.runtime_epoch!=self.source.epoch||batch.interface_epoch!=fence.source.interface_epoch||digest(&batch.state.ui).ok()!=Some(fence.state_digest){return false;}
        // Candidate UI counters/Canvas frames are not input authority. A
        // still-pinned displayed frame survives unrelated publications when
        // its original token, wrapper, owner properties and geometry remain.
        // Re-check actual wrapper generations. An identical alias in a later
        // wrapper or owner incarnation cannot revive a captured frame.
        self.rows.iter().all(|row|input_current(&row.source)&&runtime.nodes.values().any(|entity|world.get::<OwnerHostNode>(*entity).is_some_and(|node|logical_owner_current(world,node)&&node.source.instance==row.publication.instance&&node.source.epoch==row.publication.epoch&&node.wrapper_generation==row.wrapper_generation&&node.description.name==row.source.screen&&node.description.order==row.source.order&&node.geometry==row.geometry&&node.description.properties==row.properties&&node.description.layer==row.layer&&node.description.modal==row.modal)))
    }
}
/// Freeze actual installed rows, never a latest alias lookup at dequeue. The
/// cost includes every independently cloned source/property/handler allocation.
/// The enclosing preview union still enforces its shared 16 MiB budget.
pub(crate) fn installed_camera(world:&World)->Option<Entity>{Some(world.get_resource::<OwnerHostRuntime>()?.installed.as_ref()?.metric.camera)}
pub(crate) fn snapshot_expected(world:&World)->bool{
    world.contains_resource::<OwnerSceneProjection>()||world.get_resource::<OwnerHostRuntime>().is_some_and(|runtime|runtime.installed.is_some())
}
/// Bounded borrowed preflight, before rows/properties/focus strings are cloned.
/// The source token is itself bounded by Core; its cost includes its cloned
/// authority/token strings rather than serializing another copy first.
pub(crate) fn installed_snapshot_cost(world:&World)->Option<usize>{
    let runtime=world.get_resource::<OwnerHostRuntime>()?;let fence=runtime.installed.as_ref()?;
    if runtime.nodes.len()>32{return None;}
    let actual=crate::text_runtime_owner::current(world).ok()?;if actual.blocked{return None;}
    let mut cost=crate::editor_preview_input::PreviewCost::new(2*1024*1024);
    cost.add(std::mem::size_of::<CapturedOwnerFrame>())?;cost.add(2*std::mem::size_of::<usize>())?;
    if let Some(language)=&fence.metric.settings.language{cost.add(language.len())?;}
    for entity in runtime.nodes.values(){
        let node=world.get::<OwnerHostNode>(*entity)?;
        let source=capture_installed_input_source(world,&node.description.name)?;
        if source.order!=node.description.order||node.source!=fence.source{return None;}
        cost.add(std::mem::size_of::<CapturedOwnerRow>())?;cost.add(crate::editor_preview_input::preview_source_cost(&source)?)?;
        for handler in node.description.properties.events.values(){cost.add(handler.len())?;cost.add(64)?;}
        for target in [node.description.properties.desired_focus.as_ref(),node.description.focus_request.as_ref().map(|request|&request.target)].into_iter().flatten(){if let rvn_ui::programmable::HostFocusTarget::Element(id)=target{cost.add(id.len())?;}}
    }
    Some(cost.bytes)
}
pub(crate) fn capture_installed(world:&World)->Option<CapturedOwnerFrame>{
    let cost=installed_snapshot_cost(world)?;
    let runtime=world.get_resource::<OwnerHostRuntime>()?;let fence=runtime.installed.as_ref()?;
    let _actual=crate::text_runtime_owner::current(world).ok()?;
    if _actual.blocked{return None;}let mut rows=Vec::with_capacity(runtime.nodes.len());
    for entity in runtime.nodes.values(){
        let node=world.get::<OwnerHostNode>(*entity)?;let source=capture_installed_input_source(world,&node.description.name)?;
        if source.order!=node.description.order||node.source!=fence.source{return None;}
        rows.push(CapturedOwnerRow{source,publication:node.source,wrapper_generation:node.wrapper_generation,geometry:node.geometry,properties:node.description.properties.clone(),focus_request:node.description.focus_request.clone(),layer:node.description.layer,order:node.description.order,modal:node.description.modal});
    }
    rows.sort_by_key(|row|(row.layer,row.order));let frame=CapturedOwnerFrame{rows:rows.into(),metric:fence.metric.clone(),source:fence.source,state_digest:fence.state_digest,cost};
    frame.current(world).then_some(frame)
}
/// Resolve the already captured entity/incarnation; never choose a new owner
/// from a current alias. A focus witness does not pin a whole physical frame.
pub(crate) fn retained_owner<'a>(world:&'a World,entity:Entity,source:&UiInputSource,publication:OwnerPublicationSource,generation:u64,camera:Entity)->Option<&'a OwnerHostNode>{
    let runtime=world.get_resource::<OwnerHostRuntime>()?;let fence=runtime.installed.as_ref()?;let node=world.get::<OwnerHostNode>(entity)?;
    if runtime.nodes.get(&node.identity)!=Some(&entity)||node.wrapper_generation!=generation||node.source.instance!=publication.instance||node.source.epoch!=publication.epoch
        ||node.description.name!=source.screen||node.description.order!=source.order||node.description.role!=source.host_role||fence.metric.camera!=camera
        ||world.get::<TargetCamera>(entity).is_none_or(|target|target.0!=camera)||current_widget_camera_metrics(world,&fence.metric).as_ref()!=Some(&fence.metric){return None;}
    if !installed_input_source_current(world,source){return None;}
    let batch=world.get_resource::<crate::scene_publication::ScenePresentation>()?.batch.as_ref()?;
    if batch.runtime_epoch!=fence.source.epoch||batch.interface_epoch!=fence.source.interface_epoch||digest(&batch.state.ui).ok()!=Some(fence.state_digest){return None;}
    logical_owner_current(world,node).then_some(node)
}
pub(crate) fn owner_covered(world:&World,node:&OwnerHostNode)->bool{
    world.get_resource::<OwnerHostRuntime>().is_none_or(|runtime|runtime.nodes.values().filter_map(|entity|world.get::<OwnerHostNode>(*entity)).any(|other|logical_owner_current(world,other)&&other.description.modal&&(other.description.layer,other.description.order)>(node.description.layer,node.description.order)))
}
pub(crate) fn owner_entity_hosted(world:&World,entity:Entity,node:&OwnerHostNode)->bool{
    world.get::<Node>(entity).is_some_and(|layout|{let bounds=node.geometry.bounding_rect();let actual=layout.unrounded_size();(actual.x-bounds[2]).abs()<=0.01&&(actual.y-bounds[3]).abs()<=0.01})
        &&world.get::<GlobalTransform>(entity).is_some()&&world.get::<ViewVisibility>(entity).is_some_and(|visible|visible.get())
        &&world.get_resource::<bevy::ui::UiStack>().is_some_and(|stack|stack.uinodes.contains(&entity))
}
/// Retire a real host projection while preserving its generation allocator.
/// The caller drains original-source releases/lifecycle before this boundary.
/// A later install in the same Engine cannot reuse a former wrapper number.
pub(crate) fn retire_installed(world:&mut World)->Result<(),String>{
    let Some(runtime)=world.get_resource::<OwnerHostRuntime>()else{return Ok(());};
    for(key,entity)in &runtime.nodes{if !world.get::<OwnerHostNode>(*entity).is_some_and(|node|node.identity==*key){return Err("Owner retirement lost exact entity ownership".into());}}
    let mut runtime=world.remove_resource::<OwnerHostRuntime>().ok_or("Owner retirement resource disappeared")?;
    for(_,entity)in std::mem::take(&mut runtime.nodes){world.entity_mut(entity).despawn_recursive();}
    runtime.installed=None;world.insert_resource(runtime);Ok(())
}
pub(super) fn host_entity_for_source(world:&World,source:&UiInputSource)->Option<Entity>{
 if !installed_input_source_current(world,source){return None;}
 let runtime=world.get_resource::<OwnerHostRuntime>()?;runtime.nodes.values().copied().find(|entity|world.get::<OwnerHostNode>(*entity).is_some_and(|node|logical_owner_current(world,node)&&node.source.instance==source.lineage&&node.description.name==source.screen&&node.description.order==source.order&&node.description.role==source.host_role))
}
/// Physical installation consumes the exact logical ScenePublication snapshot.
/// It never recomputes an owner from latest Engine aliases or runs a callback.
pub(super) fn install_prepared_geometry(world:&mut World){
 let Some(plan)=world.get_resource::<WidgetLayouts>().and_then(|layout|layout.owner_geometry.clone())else{return;};
 let Some(scene)=world.get_resource::<OwnerSceneProjection>()else{return;};if scene.projection!=plan.projection{return;}
 let current=world.get_resource::<crate::scene_publication::ScenePresentation>().and_then(|scene|scene.batch.as_ref()).is_some_and(|batch|batch.runtime_epoch==plan.projection.source.epoch&&batch.interface_epoch==plan.projection.source.interface_epoch&&plan.projection.matches_state(&batch.state));if !current{return;}
 if let Err(message)=install_checked_geometry(world,plan){
  if let Some(mut diagnostic)=world.get_resource_mut::<ScriptErrorMessage>(){diagnostic.0=message;}
  if let Some(mut next)=world.get_resource_mut::<NextState<VnState>>(){next.set(VnState::Error);}
 }
}
/// Update flushes actual draw commands before this PostUpdate boundary. Child
/// roots attach to the generic wrapper; their authored root is left intact.
pub(super) fn parent_actual_children(world:&mut World){
 let children=world.iter_entities().filter_map(|entity|entity.get::<OwnerChildSource>().map(|source|(entity.id(),source.0.clone()))).collect::<Vec<_>>();
 for(entity,source)in children{let Some(owner)=host_entity_for_source(world,&source)else{continue;};if world.get::<Parent>(entity).is_none_or(|parent|parent.get()!=owner){world.entity_mut(entity).set_parent(owner);}if let Some(mut z)=world.get_mut::<ZIndex>(entity){*z=ZIndex::Local(0);}}
}
fn identity(source:OwnerPublicationSource,owner:&OwnerDescription)->Result<[u8;32],String>{
    use sha2::Digest;
    Ok(sha2::Sha256::digest(serde_json::to_vec(&(source.instance,source.epoch,&owner.name,owner.order,owner.role,&owner.target)).map_err(|e|e.to_string())?).into())
}
/// Complete preparation is required by the caller's opaque publication guard.
/// This only installs generic backend hosts; VM lifecycle/focus callbacks are
/// deliberately not synthesized from node/tree existence by this primitive.
pub(crate) fn install_preflighted(world:&mut World,batch:&rvn_core::renderer::SceneRenderBatch,plan:PreparedOwnerGeometry)->Result<(),String>{
    let source=plan.projection.source;
    if batch.runtime_epoch!=source.epoch||batch.interface_epoch!=source.interface_epoch||!plan.projection.matches_state(&batch.state)||!plan.metric_current(world){return Err("Owner host plan differs from its actual prepared publication/metrics".into());}
    install_checked_geometry(world,plan)
}
struct PreparedBackendNode{weak:std::sync::Weak<crate::widget_backend_0956::WidgetWrapper>,generation:u64,bundle:NodeBundle}
fn actual_backend_wrapper(world:&World,source:OwnerPublicationSource,description:&OwnerDescription)->Result<Option<PreparedBackendNode>,String>{
 if description.pending_materialization{return Err("Widget representation is explicitly deferred until its owned lifecycle transaction completes".into());}
 let Some(class)=description.widget_class.as_ref()else{return Ok(None);};
 let actual=crate::text_runtime_owner::current(world)?;
 if actual.blocked||actual.instance!=source.instance||actual.epoch!=source.epoch{return Err("Widget wrapper geometry differs from its actual installed owner lineage/epoch".into());}
 let(owner,weak)=match(world.get_resource::<VnEngine>(),world.get_resource::<crate::debugger_startup::DebugBootstrap>()){
  (Some(engine),None)=>{
   let handle=engine.0.widget_instance_handle(&description.name).ok_or("Typed wrapper lost its actual class instance")?;
   let owner=engine.0.widget_backend_owner(&handle).map_err(|error|error.to_string())?;
   let weak=engine.0.renderer.widget_backend.wrapper(&owner).ok_or("Typed wrapper has no accepted backend lifecycle publication")?;(owner,weak)
  },
  (None,Some(startup))=>{
   let backend=world.get_resource::<StartupWidgetBackend>().ok_or("Startup wrapper has no installed renderer backend")?;
   if backend.source!=source{return Err("Startup wrapper backend differs from its exact installed publication".into());}
   let owner=startup.owner.widget_backend_owner_for_alias(&description.name).map_err(|error|error.to_string())?;
   let weak=backend.backend.wrapper(&owner).ok_or("Startup wrapper has no accepted backend lifecycle publication")?;(owner,weak)
  },
  _=>return Err("Widget wrapper requires exactly one actual installed owner".into()),
 };
 if owner.class()!=class||owner.order()!=description.order||description.target.as_ref()!=Some(owner.target()){return Err("Widget geometry source differs from its real backend object owner".into());}
 let actual=weak.upgrade().ok_or("Widget backend parent released during geometry preflight")?;
 if !actual.attached(){return Err("Widget backend parent is no longer attached".into());}
 Ok(Some(PreparedBackendNode{weak,generation:actual.generation(),bundle:actual.prepared_node()?}))
}
fn install_checked_geometry(world:&mut World,plan:PreparedOwnerGeometry)->Result<(),String>{
    let source=plan.projection.source;
    if !plan.metric_current(world){return Err("Owner metrics changed before physical installation".into());}
    let actual=crate::text_runtime_owner::current(world)?;
    if actual.instance!=source.instance||actual.epoch!=source.epoch{return Err("Owner host plan belongs to another actual runtime owner".into());}
    let mut rows=Vec::new();
    for(owner,geometry)in plan.rows(){rows.push((identity(source,owner)?,owner.clone(),*geometry,actual_backend_wrapper(world,source,owner)?));}
    let present=world.get_resource::<OwnerHostRuntime>();let prior_generation=present.map_or(0,|runtime|runtime.next_generation);
    // A damaged ownership map is not permission to replace or destroy an
    // unrelated entity. Refuse before removing the resource or any node.
    if let Some(runtime)=present{
        let mut entities=std::collections::HashSet::new();
        for(key,entity)in &runtime.nodes{
            let node=world.get::<OwnerHostNode>(*entity).ok_or("Owner host ownership map lost its actual node")?;
            if node.identity!=*key||node.wrapper_generation==0||!entities.insert(*entity){return Err("Owner host ownership map is inconsistent".into());}
        }
    }
    let new_count=rows.iter().filter(|(key,_,_,backend)|backend.is_none()&&present.is_none_or(|runtime|!runtime.nodes.contains_key(key))).count();
    let final_generation=prior_generation.checked_add(u64::try_from(new_count).map_err(|_|"Owner wrapper generation count overflow")?).ok_or("Owner wrapper generation exhausted")?;
    // All fallible checks precede entity/resource mutation. Entity installation
    // carries one frozen source and actual render-target camera with its slot.
    let mut runtime=world.remove_resource::<OwnerHostRuntime>().unwrap_or_default();let mut old=std::mem::take(&mut runtime.nodes);let mut generation=prior_generation;
    for(rank,(key,description,geometry,backend))in rows.into_iter().enumerate(){
        let old_entity=old.remove(&key);
        let wrapper_generation=if let Some(backend)=&backend{backend.generation}else{old_entity.and_then(|entity|world.get::<OwnerHostNode>(entity).map(|node|node.wrapper_generation)).unwrap_or_else(||{generation+=1;generation})};
        let bounds=geometry.bounding_rect();
        let style=Style{position_type:PositionType::Absolute,left:Val::Px(bounds[0]),top:Val::Px(bounds[1]),width:Val::Px(bounds[2]),height:Val::Px(bounds[3]),display:if description.properties.visibility==rvn_ui::programmable::HostVisibility::Collapsed{Display::None}else{Display::Flex},..default()};
        let visibility=if description.properties.visibility.visible(){Visibility::Visible}else{Visibility::Hidden};
        let focus_policy=if description.properties.visibility.owner_hit_test(){FocusPolicy::Block}else{FocusPolicy::Pass};
        // Descriptions are sorted by the same logical owner layer/order as
        // Core. One actual rank includes empty owners and child owners alike.
        let layer=3000+rank as i32*10;
        let scale=Vec3::new(if geometry.rect[2]<0.0{-1.0}else{1.0},if geometry.rect[3]<0.0{-1.0}else{1.0},1.0);
        let node=OwnerHostNode{identity:key,description,source,wrapper_generation,geometry,backend:backend.as_ref().map(|backend|backend.weak.clone())};
        let entity=if let Some(entity)=old_entity{
            // Preserve the actual Node/Transform/layout caches of the cached
            // wrapper. Source/property refresh is not a physical reconstruction.
            world.entity_mut(entity).insert((node,style,visibility,focus_policy,ZIndex::Global(layer),TargetCamera(plan.metric.camera)));
            if let Some(mut transform)=world.get_mut::<Transform>(entity){transform.scale=scale;}entity
        }else{
            let mut bundle=backend.map(|backend|backend.bundle).unwrap_or_default();bundle.style=style;bundle.visibility=visibility;bundle.focus_policy=focus_policy;bundle.z_index=ZIndex::Global(layer);bundle.transform=Transform::from_scale(scale);
            world.spawn((node,bundle,TargetCamera(plan.metric.camera))).id()
        };runtime.nodes.insert(key,entity);
    }
    for(key,entity)in old{if world.get::<OwnerHostNode>(entity).is_some_and(|node|node.identity==key){world.entity_mut(entity).despawn_recursive();}}
    runtime.next_generation=final_generation;runtime.installed=Some(InstalledOwnerFence{source,state_digest:plan.projection.state_digest,metric:plan.metric});world.insert_resource(runtime);Ok(())
}

#[cfg(test)]
pub(crate) mod tests{
 use super::*;
 #[test]
 fn empty_actual_owner_projection_has_no_placeholder_tree_and_changed_snapshot_refuses(){
    let source="screen owner(){return []}\nscene label P\nui.open(\"owner\",[],true,2)\n\"waiting\"\n";
    let mut engine=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();engine.step_until_interaction().unwrap();let views=engine.interface_views().unwrap();assert!(views.is_empty());
    let plan=prepare(&engine.state,&views,OwnerPublicationSource{instance:engine.instance_id(),epoch:engine.runtime_epoch(),interface_epoch:1}).unwrap();assert_eq!(plan.descriptions().len(),1);let owner=&plan.descriptions()[0];assert!(!owner.has_child);assert!(owner.target.is_some());assert!(owner.modal);assert!(plan.matches_state(&engine.state));
    let mut changed=engine.state.clone();changed.ui.screens[0].owner_properties.focusable=true;assert!(!plan.matches_state(&changed));
 }
 #[test]
 fn owner_projection_refuses_zero_authority_and_detached_child_identity(){
    let source="screen owner(){return component(\"root\",\"text\",{},[])}\nlabel P\nui.open(\"owner\",[],false,0)\n\"waiting\"\n";
    let mut engine=rvn_core::Engine::new(rvn_parser::parse(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();engine.step_until_interaction().unwrap();let mut views=engine.interface_views().unwrap();
    assert!(prepare(&engine.state,&views,OwnerPublicationSource{instance:0,epoch:engine.runtime_epoch(),interface_epoch:1}).is_err());views[0].order+=1;assert!(prepare(&engine.state,&views,OwnerPublicationSource{instance:engine.instance_id(),epoch:engine.runtime_epoch(),interface_epoch:1}).is_err());
 }
 fn runtime_fixture()->(App,rvn_core::renderer::SceneRenderBatch,WidgetMetrics){
    use bevy::ecs::system::RunSystemOnce;
    let mut app=App::new();app.insert_resource(Assets::<Image>::default()).init_resource::<bevy::render::camera::ManualTextureViews>().init_resource::<crate::accessibility::Accessibility>().insert_resource(bevy::ui::UiScale(1.0))
        .add_event::<bevy::window::WindowResized>().add_event::<bevy::window::WindowCreated>().add_event::<bevy::window::WindowScaleFactorChanged>().add_event::<bevy::asset::AssetEvent<Image>>();
    let window=app.world_mut().spawn((Window{resolution:bevy::window::WindowResolution::new(800.0,600.0).with_scale_factor_override(1.0),..default()},bevy::window::PrimaryWindow)).id();
    let camera=app.world_mut().spawn(Camera2dBundle::default()).id();app.world_mut().run_system_once(bevy::render::camera::camera_system::<OrthographicProjection>);
    let camera_value=app.world().get::<Camera>(camera).unwrap();let target=camera_value.target.normalize(Some(window)).unwrap();let window_value=app.world().get::<Window>(window).unwrap();
    let info=target.get_render_target_info(std::iter::once((window,window_value)),app.world().resource::<Assets<Image>>(),app.world().resource::<bevy::render::camera::ManualTextureViews>()).unwrap();
    let metric=actual_widget_camera_metrics(camera,camera_value,1.0,&rvn_ui::accessibility::AccessibilitySettings::default(),target,info).unwrap();
    let source=r#"init {set visible=false set owner_focusable=true}
handler defocus(event){set owner_focusable=false}
handler show(event){set visible=true}
handler hide(event){set visible=false}
screen owner(){if visible{return {"host":{"focusable":owner_focusable},"root":component("actual","text",{},[])}} return {"host":{"focusable":owner_focusable},"root":[]}}
screen controls(){return component("controller","column",{},[component("show","button",{"events":{"click":"show"}},[]),component("hide","button",{"events":{"click":"hide"}},[]),component("defocus","button",{"events":{"click":"defocus"}},[])])}
scene label P
ui.open("owner",[],false,1)
ui.open("controls",[],false,2)
"waiting"
"#;
    let mut engine=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();engine.step_until_interaction().unwrap();
    let mut batch=*engine.renderer.scene_batch.take().unwrap();batch.state=engine.state.clone();batch.interfaces=engine.interface_views().unwrap();batch.interface_epoch=engine.interface_epoch();app.insert_resource(VnEngine(engine));
    (app,batch,metric)
 }
 fn geometry_plan(app:&App,batch:&rvn_core::renderer::SceneRenderBatch,metric:&WidgetMetrics)->PreparedOwnerGeometry{
    let engine=&app.world().resource::<VnEngine>().0;let source=OwnerPublicationSource{instance:engine.instance_id(),epoch:batch.runtime_epoch,interface_epoch:batch.interface_epoch};
    prepare_geometry(app.world(),prepare(&batch.state,&batch.interfaces,source).unwrap(),metric.clone(),&HashMap::new()).unwrap()
 }
 fn install_fixture(app:&mut App,batch:rvn_core::renderer::SceneRenderBatch,metric:&WidgetMetrics){let plan=geometry_plan(app,&batch,metric);install_preflighted(app.world_mut(),&batch,plan).unwrap();app.insert_resource(crate::scene_publication::ScenePresentation{batch:Some(batch)});}
 fn published_delta(app:&mut App,id:&str)->rvn_core::renderer::SceneRenderBatch{
    let mut engine=app.world_mut().resource_mut::<VnEngine>();let source=engine.0.capture_ui_input_source("controls").unwrap();engine.0.interface_event_scoped(ScopedUiInput{source,event:UiInput{screen:"controls".into(),element:id.into(),kind:EventKind::Click,value:None,key:None}}).unwrap();let mut batch=*engine.0.renderer.scene_batch.take().unwrap();batch.state=engine.0.state.clone();batch.interfaces=engine.0.interface_views().unwrap();batch.interface_epoch=engine.0.interface_epoch();batch
 }
 #[test]
 fn actual_owner_backend_preserves_wrapper_across_vm_none_rooted_none_and_never_fabricates_component(){
    let(mut app,batch,metric)=runtime_fixture();assert!(batch.interfaces.iter().all(|view|view.name!="owner"));install_fixture(&mut app,batch,&metric);
    let original=app.world().resource::<OwnerHostRuntime>().nodes.values().find_map(|entity|app.world().get::<OwnerHostNode>(*entity).filter(|node|node.description.name=="owner").map(|node|(*entity,node.wrapper_generation))).unwrap();
    for(id,has_child)in [("show",true),("hide",false)]{let batch=published_delta(&mut app,id);assert_eq!(batch.interfaces.iter().any(|view|view.name=="owner"),has_child);install_fixture(&mut app,batch,&metric);let actual=app.world().get::<OwnerHostNode>(original.0).unwrap();assert_eq!(actual.wrapper_generation,original.1);assert_eq!(actual.description.has_child,has_child);assert_eq!(actual.geometry.rect,[0.0,0.0,800.0,600.0]);assert!(app.world().get::<Control>(original.0).is_none());}
 }
 #[test]
 fn owner_focus_refuses_source_only_plan_before_real_post_layout_wrapper_path(){
    let(mut app,batch,metric)=runtime_fixture();install_fixture(&mut app,batch,&metric);let frame=capture_installed(app.world()).unwrap();let row=frame.rows().iter().position(|row|row.source.screen=="owner").unwrap();assert!(frame.current(app.world()));assert!(!frame.hosted(app.world(),&frame.rows()[row]));assert!(super::super::owner_focus_0943::prepare_focus(app.world(),frame,row,rvn_ui::programmable::HostFocusTarget::Owner,0).is_err());assert!(!app.world().contains_resource::<super::super::owner_focus_0943::FocusRuntime>());
 }
 #[test]
 fn owner_entity_map_corruption_and_stale_camera_refuse_before_any_wrapper_mutation(){
    let(mut app,batch,metric)=runtime_fixture();install_fixture(&mut app,batch.clone(),&metric);let plan=geometry_plan(&app,&batch,&metric);let count=app.world().entities().len();let generation=app.world().resource::<OwnerHostRuntime>().next_generation;
    let entity=*app.world().resource::<OwnerHostRuntime>().nodes.values().next().unwrap();let original=app.world().get::<OwnerHostNode>(entity).unwrap().identity;app.world_mut().get_mut::<OwnerHostNode>(entity).unwrap().identity=[0;32];
    assert!(install_preflighted(app.world_mut(),&batch,plan).is_err());assert_eq!(app.world().entities().len(),count);assert_eq!(app.world().resource::<OwnerHostRuntime>().next_generation,generation);app.world_mut().get_mut::<OwnerHostNode>(entity).unwrap().identity=original;
    let plan=geometry_plan(&app,&batch,&metric);let window=app.world().iter_entities().find(|entity|entity.contains::<Window>()).unwrap().id();app.world_mut().get_mut::<Window>(window).unwrap().resolution.set(1000.0,600.0);assert!(install_preflighted(app.world_mut(),&batch,plan).is_err());assert_eq!(app.world().entities().len(),count);assert_eq!(app.world().resource::<OwnerHostRuntime>().next_generation,generation);
 }
 #[test]
 fn retiring_host_wrappers_preserves_allocator_and_revokes_original_captured_frame(){
    let(mut app,batch,metric)=runtime_fixture();install_fixture(&mut app,batch.clone(),&metric);let original=capture_installed(app.world()).unwrap();let generation=app.world().resource::<OwnerHostRuntime>().next_generation;retire_installed(app.world_mut()).unwrap();assert!(!original.current(app.world()));assert!(app.world().resource::<OwnerHostRuntime>().nodes.is_empty());assert_eq!(app.world().resource::<OwnerHostRuntime>().next_generation,generation);install_fixture(&mut app,batch,&metric);assert!(app.world().resource::<OwnerHostRuntime>().nodes.values().all(|entity|app.world().get::<OwnerHostNode>(*entity).unwrap().wrapper_generation>generation));assert!(!original.current(app.world()));
 }

 fn enable_actual_cpu_layout(app:&mut App){
    // Public Bevy plugins run the actual Taffy layout, transform propagation,
    // view visibility and UiStack. No RenderApp/device or GPU pixels are claimed.
    app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{watch_for_changes_override:Some(false),..default()}))
        .init_asset::<Font>().init_asset::<Shader>().init_asset::<Mesh>().init_asset::<bevy::sprite::TextureAtlasLayout>()
        .add_plugins(bevy::input::InputPlugin).add_plugins(bevy::transform::TransformPlugin)
        .add_plugins(bevy::render::camera::CameraPlugin).add_plugins(bevy::render::view::VisibilityPlugin)
        .add_plugins(bevy::text::TextPlugin).add_plugins(bevy::ui::UiPlugin);
    app.finish();app.cleanup();app.update();
 }
 #[test]
 fn actual_cpu_layout_empty_owner_acquires_focus_and_flag_change_does_not_fabricate_focus_loss(){
    let(mut app,batch,metric)=runtime_fixture();install_fixture(&mut app,batch,&metric);enable_actual_cpu_layout(&mut app);
    let frame=capture_installed(app.world()).unwrap();let row=frame.rows().iter().position(|row|row.source.screen=="owner").unwrap();assert!(frame.hosted(app.world(),&frame.rows()[row]));
    let focus=super::super::owner_focus_0943::prepare_focus(app.world(),frame,row,rvn_ui::programmable::HostFocusTarget::Owner,0).unwrap();assert!(focus.next().unwrap().can_acquire(app.world()));let admitted=super::super::owner_focus_0943::admit_focus_before_callbacks(app.world(),focus).unwrap();
    // Real existing VM handler and accepted publication, between admission
    // and final focus installation. This tests the primitive's fence only;
    // the future FocusReceived lifecycle/overlay route is not claimed here.
    let batch=published_delta(&mut app,"defocus");install_fixture(&mut app,batch,&metric);app.update();
    let change=super::super::owner_focus_0943::install_admitted_focus_preflighted(app.world_mut(),admitted).unwrap();assert!(change.changed());let acquired=change.next.unwrap();assert!(acquired.current(app.world()));assert!(acquired.route_allowed(app.world()));assert!(!acquired.can_acquire(app.world()));
    let frame=capture_installed(app.world()).unwrap();let refresh=super::super::owner_focus_0943::prepare_refresh(app.world(),frame,acquired.domain()).unwrap();let change=super::super::owner_focus_0943::install_focus_preflighted(app.world_mut(),refresh).unwrap();assert!(!change.changed());assert!(change.next.unwrap().current(app.world()));
    let before=super::super::owner_focus_0943::prepare_cleanup(app.world()).unwrap();assert_eq!(before.originals().count(),0,"Focusable=false alone cannot manufacture teardown");
    let domain=acquired.domain();app.world_mut().entity_mut(domain.camera).despawn_recursive();let cleanup=super::super::owner_focus_0943::prepare_cleanup(app.world()).unwrap();let originals=cleanup.originals().collect::<Vec<_>>();assert_eq!(originals.len(),1);assert_eq!(originals[0].source(),acquired.source());assert_eq!(originals[0].domain(),domain);drop(originals);
    let retired=super::super::owner_focus_0943::install_cleanup_preflighted(app.world_mut(),cleanup).unwrap();assert_eq!(retired.len(),1);assert_eq!(retired[0].source(),acquired.source());assert!(super::super::owner_focus_0943::prepare_release(app.world(),domain).unwrap().previous().is_none());
 }

 pub(crate) fn actual_owner_image_world()->App{let(mut app,batch,metric)=runtime_fixture();install_fixture(&mut app,batch,&metric);enable_actual_cpu_layout(&mut app);app}
 pub(crate) fn actual_scope(app:&App,sequence:u64)->rvn_core::editor_preview::FrameScope{
    let engine=&app.world().resource::<VnEngine>().0;
    rvn_core::editor_preview::FrameScope{instance:engine.instance_id(),epoch:engine.runtime_epoch(),sequence,control_sequence:0,control_ack:0,control_error:None,paused:false,publication_pending:false,finished:Some(engine.is_finished()),step_allowed:false,interaction:None,position:engine.state.scene_execution.as_ref().map(|execution|execution.live_position.clone()).unwrap_or(rvn_core::scene::ResumePosition{pc:engine.state.pc,origin:None}),active:None,parked:vec![]}
 }
 #[test]
 fn actual_physical_image_keeps_rootless_owner_and_acquired_focus_witness_inside_its_existing_bounded_frame(){
    let(mut app,batch,metric)=runtime_fixture();install_fixture(&mut app,batch,&metric);enable_actual_cpu_layout(&mut app);
    let owner=capture_installed(app.world()).unwrap();let row=owner.rows().iter().position(|row|row.source.screen=="owner").unwrap();assert!(owner.hosted(app.world(),&owner.rows()[row]));let source=owner.rows()[row].source.clone();
    let plan=super::super::owner_focus_0943::prepare_focus(app.world(),owner,row,rvn_ui::programmable::HostFocusTarget::Owner,0).unwrap();super::super::owner_focus_0943::install_focus_preflighted(app.world_mut(),plan).unwrap();
    let scope=actual_scope(&app,1);let frame=crate::editor_preview_input::capture(app.world(),scope.clone()).unwrap();let witness=frame.owner_snapshot().unwrap();assert!(witness.current(app.world()));assert!(witness.belongs_to(&scope));assert_eq!(witness.focus().unwrap().source(),&source);assert!(matches!(witness.focus().unwrap().target(),super::super::owner_focus_0943::FocusedTarget::Owner));assert!(witness.owners().rows().iter().any(|row|row.source==source&&row.properties.focusable));assert!(frame.cost>=witness.cost());assert!(witness.cost()>=installed_snapshot_cost(app.world()).unwrap());assert!(frame.cost<=2*1024*1024);
    let held=frame.clone();assert!(std::sync::Arc::ptr_eq(&frame,&held),"Output already deduplicates this actual allocation for Main/writer/held pins");assert!(held.owner_snapshot().unwrap().current(app.world()));
 }
 #[test]
 fn owner_image_capture_refuses_foreign_scope_and_keeps_focus_and_source_without_a_main_grant(){
    let(mut app,batch,metric)=runtime_fixture();install_fixture(&mut app,batch,&metric);enable_actual_cpu_layout(&mut app);let token=app.world().resource::<VnEngine>().0.debug_state_source().unwrap();let scope=actual_scope(&app,3);let mut foreign=scope.clone();foreign.epoch+=1;assert!(crate::editor_preview_input::capture(app.world(),foreign).is_none());assert!(app.world().resource::<VnEngine>().0.accepts_debug_state_source(&token));assert!(!app.world().contains_resource::<super::super::owner_focus_0943::FocusRuntime>());
    let frame=crate::editor_preview_input::capture(app.world(),scope).unwrap();assert!(frame.owner_snapshot().unwrap().focus().is_none(),"capturing a focusable owner does not acquire it");
 }
 #[test]
 fn held_image_retains_original_owner_rows_but_a_new_wrapper_at_same_alias_does_not_revive_its_capture(){
    let(mut app,batch,metric)=runtime_fixture();install_fixture(&mut app,batch.clone(),&metric);enable_actual_cpu_layout(&mut app);let scope=actual_scope(&app,4);let frame=crate::editor_preview_input::capture(app.world(),scope).unwrap();let original=frame.owner_snapshot().unwrap().owners().rows().to_vec();assert!(frame.owner_snapshot().unwrap().current(app.world()));retire_installed(app.world_mut()).unwrap();install_fixture(&mut app,batch,&metric);app.update();assert!(!frame.owner_snapshot().unwrap().current(app.world()));assert_eq!(frame.owner_snapshot().unwrap().owners().rows(),original.as_slice());let next=crate::editor_preview_input::capture(app.world(),actual_scope(&app,5)).unwrap();assert!(next.owner_snapshot().unwrap().owners().rows().iter().all(|row|original.iter().find(|old|old.source.screen==row.source.screen).is_some_and(|old|old.wrapper_generation!=row.wrapper_generation)));assert_ne!(std::sync::Arc::as_ptr(&frame),std::sync::Arc::as_ptr(&next));
 }

}

#[cfg(all(test,not(target_arch="wasm32")))]
#[path="startup_imagemap_owner_capture_tests_0998.rs"]
pub(crate) mod startup_capture_tests;
