//! Private focus publication port. Not scheduled until the legacy child-focus
//! adapters and owner VM notifications are migrated at one integration boundary.
use super::*;
use rvn_ui::programmable::HostFocusTarget;
/// Actual render-target camera and Slate-user equivalent. A camera entity is a
/// transient backend identity, never a saved or client-supplied domain token.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub(crate) struct FocusDomain{pub camera:Entity,pub user:u32}
#[derive(Clone,Debug,PartialEq)]
pub(crate) enum FocusedTarget{
    Owner,
    Element{id:String,entity:Entity,kind:Kind},
}
/// One canonical target address with a minimal original-source witness.
/// It never pins an entire physical frame outside the shared Output budget.
/// Immutable preview snapshots retain their own bounded copy of this witness.
#[derive(Clone)]
pub(crate) struct AcquiredFocus{
    domain:FocusDomain,owner_entity:Entity,source:UiInputSource,publication:owner_hosts_0943::OwnerPublicationSource,wrapper_generation:u64,target:FocusedTarget,
}
/// Same physical owner path, not merely a control carrying the same alias.
/// Bound traversal also refuses a malformed ECS parent cycle without a hang.
fn child_hosted_under(world:&World,mut entity:Entity,owner:Entity)->bool{
    for _ in 0..4096{if entity==owner{return true;}let Some(parent)=world.get::<Parent>(entity)else{return false;};entity=parent.get();}false
}
impl AcquiredFocus{
    pub(crate) fn domain(&self)->FocusDomain{self.domain}
    pub(crate) fn target(&self)->&FocusedTarget{&self.target}
    pub(crate) fn source(&self)->&UiInputSource{&self.source}
    pub(crate) fn owner<'a>(&self,world:&'a World)->Option<&'a owner_hosts_0943::OwnerHostNode>{owner_hosts_0943::retained_owner(world,self.owner_entity,&self.source,self.publication,self.wrapper_generation,self.domain.camera)}
    /// Still the same admitted physical target. Focus support and event
    /// routing are separate from identity retention: SetIsFocusable(false)
    /// alone does not manufacture a FocusLost notification in local Slate.
    pub(crate) fn current(&self,world:&World)->bool{
        if self.domain.user!=0{return false;}
        let Some(_owner)=self.owner(world)else{return false;};
        match &self.target{
            FocusedTarget::Owner=>true,
            FocusedTarget::Element{id,entity,kind}=>world.get::<Control>(*entity).is_some_and(|control|control.screen==self.source.screen&&control.element==*id&&control.option.is_none())
                &&world.get::<ControlSource>(*entity).is_some_and(|source|source.0==self.source)
                &&world.get::<Node>(*entity).is_some()&&child_hosted_under(world,*entity,self.owner_entity)
                &&world.get_resource::<crate::scene_publication::ScenePresentation>().and_then(|presentation|presentation.batch.as_ref()).and_then(|batch|batch.interfaces.iter().find(|view|view.name==self.source.screen&&view.order==self.source.order)).and_then(|view|view.root.find(id)).is_some_and(|component|component.kind==*kind),
        }
    }
    pub(crate) fn route_allowed(&self,world:&World)->bool{
        if !self.current(world){return false;}
        let Some(owner)=self.owner(world)else{return false;};
        if !owner_hosts_0943::owner_entity_hosted(world,self.owner_entity,owner){return false;}
        let Ok(actual)=crate::text_runtime_owner::current(world)else{return false;};if actual.blocked{return false;}
        if !owner.description.properties.enabled||!owner.description.properties.visibility.visible()||owner_hosts_0943::owner_covered(world,owner){return false;}
        match &self.target{
            FocusedTarget::Owner=>true,
            FocusedTarget::Element{id,entity,..}=>world.get::<GlobalTransform>(*entity).is_some()
                &&world.get::<ViewVisibility>(*entity).is_some_and(|value|value.get())
                &&world.get_resource::<bevy::ui::UiStack>().is_some_and(|stack|stack.uinodes.contains(entity))
                &&world.get_resource::<crate::scene_publication::ScenePresentation>().and_then(|presentation|presentation.batch.as_ref()).and_then(|batch|batch.interfaces.iter().find(|view|view.name==self.source.screen&&view.order==self.source.order)).is_some_and(|view|view.root.available(id)),
        }
    }
    pub(crate) fn can_acquire(&self,world:&World)->bool{
        if !self.route_allowed(world){return false;}
        match &self.target{
            FocusedTarget::Owner=>self.owner(world).is_some_and(|owner|owner.description.properties.supports_owner_focus()),
            FocusedTarget::Element{id,..}=>world.get_resource::<crate::scene_publication::ScenePresentation>().and_then(|presentation|presentation.batch.as_ref()).and_then(|batch|batch.interfaces.iter().find(|view|view.name==self.source.screen&&view.order==self.source.order)).is_some_and(|view|view.root.focus_order().contains(id)),
        }
    }
    pub(crate) fn same_target(&self,other:&Self)->bool{self.domain==other.domain&&self.owner_entity==other.owner_entity&&self.source==other.source&&self.wrapper_generation==other.wrapper_generation&&self.target==other.target}
}
#[derive(Resource,Default)]
pub(crate) struct FocusRuntime{revision:u64,domains:HashMap<FocusDomain,AcquiredFocus>}
pub(crate) struct PreparedFocus{revision:u64,domain:FocusDomain,previous:Option<AcquiredFocus>,next:Option<AcquiredFocus>,acquisition:bool}
/// Eligibility has passed before executing a candidate focus notification.
/// Opaque and consumed once. This is preparation, not an externally acquired
/// focus capability: only the combined VM/renderer publication may install it.
pub(crate) struct AdmittedFocus{plan:PreparedFocus}
pub(crate) struct FocusChange{pub previous:Option<AcquiredFocus>,pub next:Option<AcquiredFocus>}
impl FocusChange{
    pub(crate) fn changed(&self)->bool{match(&self.previous,&self.next){(None,None)=>false,(Some(previous),Some(next))=>!previous.same_target(next),_=>true}}
}
/// Read the one real local keyboard domain without granting focus from a
/// preview frame. Stale original focus must be released by its owner; capture
/// refuses instead of silently replacing it with a latest alias or None.
pub(crate) fn captured_focus(world:&World,camera:Entity)->Result<Option<&AcquiredFocus>,String>{
    let current=world.get_resource::<FocusRuntime>().and_then(|runtime|runtime.domains.get(&FocusDomain{camera,user:0}));
    if current.is_some_and(|focus|!focus.current(world)){return Err("Original acquired focus must be drained before image capture".into());}Ok(current)
}
pub(crate) fn captured_focus_cost(focus:&AcquiredFocus)->Option<usize>{
    let mut cost=crate::editor_preview_input::PreviewCost::new(2*1024*1024);cost.add(std::mem::size_of::<AcquiredFocus>())?;
    cost.add(crate::editor_preview_input::preview_source_cost(focus.source())?)?;
    if let FocusedTarget::Element{id,..}=focus.target(){cost.add(id.len().checked_add(64)?)?;}Some(cost.bytes)
}
/// Prepare against an actual installed owner frame. Only the existing single
/// local keyboard user is currently admitted; another user stays an intent
/// until its real input-device route exists, never aliases the first user.
pub(crate) fn prepare_focus(world:&World,frame:owner_hosts_0943::CapturedOwnerFrame,row:usize,target:HostFocusTarget,user:u32)->Result<PreparedFocus,String>{
    target.validate()?;
    if user!=0{return Err("Focus user has no actual local input-device route".into());}
    if !frame.current(world){return Err("Focus requires the actual installed frame and owner wrappers".into());}
    let owner=frame.rows().get(row).ok_or("Focus row is outside the actual frame")?;
    let domain=FocusDomain{camera:frame.camera(),user};
    let target=match target{
        HostFocusTarget::Owner=>FocusedTarget::Owner,
        HostFocusTarget::Element(id)=>{
            let mut entities=world.iter_entities().filter(|entity|entity.get::<Control>().is_some_and(|control|control.screen==owner.source.screen&&control.element==id&&control.option.is_none())&&entity.get::<ControlSource>().is_some_and(|source|source.0==owner.source));
            let entity=entities.next().ok_or("Desired focus child has no actual hosted control")?.id();
            if entities.next().is_some(){return Err("Desired focus child is ambiguous in its actual owner path".into());}
            let kind=world.get_resource::<crate::scene_publication::ScenePresentation>().and_then(|presentation|presentation.batch.as_ref()).and_then(|batch|batch.interfaces.iter().find(|view|view.name==owner.source.screen&&view.order==owner.source.order)).and_then(|view|view.root.find(&id)).map(|component|component.kind).ok_or("Desired focus child is absent from its published root")?;
            FocusedTarget::Element{id,entity,kind}
        }
    };
    let owner_entity=frame.owner_entity(world,row).ok_or("Original focus owner lost its exact hosted entity")?;
    let next=AcquiredFocus{domain,owner_entity,source:owner.source.clone(),publication:owner.publication,wrapper_generation:owner.wrapper_generation,target};if !next.can_acquire(world){return Err("Desired target does not accept focus through its actual owner/viewport path".into());}
    let runtime=world.get_resource::<FocusRuntime>();
    if runtime.is_some_and(|runtime|!runtime.domains.contains_key(&domain)&&runtime.domains.len()>=32){return Err("Actual focus domain capacity exhausted".into());}
    let revision=runtime.map_or(0,|runtime|runtime.revision);revision.checked_add(1).ok_or("Focus publication revision exhausted")?;
    Ok(PreparedFocus{revision,domain,previous:runtime.and_then(|runtime|runtime.domains.get(&domain)).cloned(),next:Some(next),acquisition:true})
}
/// Original-source cancellation can retire focus without granting a new target.
/// Its callbacks must consume the returned original capture before old Main pins
/// are released; this never re-captures a current alias or new owner incarnation.
pub(crate) fn prepare_release(world:&World,domain:FocusDomain)->Result<PreparedFocus,String>{
    let runtime=world.get_resource::<FocusRuntime>();let revision=runtime.map_or(0,|runtime|runtime.revision);revision.checked_add(1).ok_or("Focus publication revision exhausted")?;
    Ok(PreparedFocus{revision,domain,previous:runtime.and_then(|runtime|runtime.domains.get(&domain)).cloned(),next:None,acquisition:false})
}
/// Carry an already acquired target across an accepted property/layout
/// publication with the same original source and wrapper. This is not a new
/// grant and does not generate FocusLost/Received merely for flag changes.
pub(crate) fn prepare_refresh(world:&World,frame:owner_hosts_0943::CapturedOwnerFrame,domain:FocusDomain)->Result<PreparedFocus,String>{
    let runtime=world.get_resource::<FocusRuntime>().ok_or("No acquired focus to refresh")?;
    let previous=runtime.domains.get(&domain).ok_or("No acquired focus in this actual domain")?;
    if frame.camera()!=domain.camera||!frame.current(world){return Err("Focus refresh requires the actual accepted owner frame".into());}
    let row=frame.rows().iter().position(|row|row.source==previous.source&&row.wrapper_generation==previous.wrapper_generation).ok_or("Focus refresh cannot replace the original source or wrapper")?;
    if frame.owner_entity(world,row)!=Some(previous.owner_entity){return Err("Focus refresh cannot replace its original hosted entity".into());}
    let next=previous.clone();if !next.current(world){return Err("Original focus target left its actual hosted path".into());}
    runtime.revision.checked_add(1).ok_or("Focus publication revision exhausted")?;
    Ok(PreparedFocus{revision:runtime.revision,domain,previous:Some(previous.clone()),next:Some(next),acquisition:false})
}
impl PreparedFocus{
    pub(crate) fn previous(&self)->Option<&AcquiredFocus>{self.previous.as_ref()}
    pub(crate) fn next(&self)->Option<&AcquiredFocus>{self.next.as_ref()}
    fn identity_current(&self,world:&World)->bool{
        let runtime=world.get_resource::<FocusRuntime>();
        if runtime.map_or(0,|runtime|runtime.revision)!=self.revision{return false;}
        let current=runtime.and_then(|runtime|runtime.domains.get(&self.domain));
        let same=match(&self.previous,current){(None,None)=>true,(Some(previous),Some(current))=>previous.same_target(current),_=>false};
        same&&self.next.as_ref().is_none_or(|next|next.current(world))
    }
    pub(crate) fn current(&self,world:&World)->bool{
        self.identity_current(world)&&(!self.acquisition||self.next.as_ref().is_none_or(|next|next.can_acquire(world)))
    }
}
/// Complete eligibility before invoking candidate callbacks. A same-source
/// FocusReceived callback can change focusability without retroactively
/// cancelling the focus that it just received. Source/entity/version changes
/// still invalidate the admission. Candidate callback reads will need the
/// transaction's focus overlay, not a premature mutation of FocusRuntime.
pub(crate) fn admit_focus_before_callbacks(world:&World,plan:PreparedFocus)->Result<AdmittedFocus,String>{
    if !plan.current(world){return Err("Focus eligibility/source changed before callbacks; no focus state was mutated".into());}
    Ok(AdmittedFocus{plan})
}
/// Final identity/version fence after candidate VM callbacks and renderer
/// preflight, without a second acquisition eligibility test. A competing
/// focus operation or owner replacement refuses without mutation.
pub(crate) fn install_admitted_focus_preflighted(world:&mut World,admitted:AdmittedFocus)->Result<FocusChange,String>{
    let plan=admitted.plan;
    if !plan.identity_current(world){return Err("Focus identity changed during callback/preflight; no focus state was mutated".into());}
    let revision=plan.revision.checked_add(1).ok_or("Focus publication revision exhausted")?;
    let mut runtime=world.remove_resource::<FocusRuntime>().unwrap_or_default();
    if let Some(next)=&plan.next{runtime.domains.insert(plan.domain,next.clone());}else{runtime.domains.remove(&plan.domain);}
    runtime.revision=revision;world.insert_resource(runtime);Ok(FocusChange{previous:plan.previous,next:plan.next})
}
/// No-callback compatibility path. A future combined focus transaction must
/// admit BEFORE its callbacks and use install_admitted_focus_preflighted.
/// No caller schedules this port until legacy focus has been replaced.
pub(crate) fn install_focus_preflighted(world:&mut World,plan:PreparedFocus)->Result<FocusChange,String>{
    let admitted=admit_focus_before_callbacks(world,plan)?;
    install_admitted_focus_preflighted(world,admitted)
}

/// Immutable minimal original captures to drain before retiring dead domains.
/// Mere focusability/visibility changes or an unsettled publication are not a
/// teardown signal. No physical-frame Arc is held by this plan or runtime.
pub(crate) struct PreparedFocusCleanup{revision:u64,removed:Vec<(FocusDomain,AcquiredFocus)>}
fn cleanup_needed(world:&World,focus:&AcquiredFocus)->bool{
    world.get::<Camera>(focus.domain.camera).is_none()
        ||world.get::<owner_hosts_0943::OwnerHostNode>(focus.owner_entity).is_none_or(|node|node.wrapper_generation!=focus.wrapper_generation||node.source.instance!=focus.publication.instance||node.source.epoch!=focus.publication.epoch||node.description.name!=focus.source.screen||node.description.order!=focus.source.order||node.description.role!=focus.source.host_role)
        ||!owner_hosts_0943::installed_input_source_current(world,&focus.source)
        ||world.get::<TargetCamera>(focus.owner_entity).is_none_or(|target|target.0!=focus.domain.camera)
        ||match &focus.target{FocusedTarget::Owner=>false,FocusedTarget::Element{entity,..}=>world.get::<Control>(*entity).is_none()||!child_hosted_under(world,*entity,focus.owner_entity)}
}
pub(crate) fn prepare_cleanup(world:&World)->Result<PreparedFocusCleanup,String>{
    let runtime=world.get_resource::<FocusRuntime>();let revision=runtime.map_or(0,|runtime|runtime.revision);
    let removed=runtime.map(|runtime|runtime.domains.iter().filter(|(_,focus)|cleanup_needed(world,focus)).map(|(domain,focus)|(*domain,focus.clone())).collect::<Vec<_>>()).unwrap_or_default();
    if !removed.is_empty(){revision.checked_add(1).ok_or("Focus cleanup revision exhausted")?;}
    Ok(PreparedFocusCleanup{revision,removed})
}
impl PreparedFocusCleanup{
    pub(crate) fn originals(&self)->impl Iterator<Item=&AcquiredFocus>{self.removed.iter().map(|(_,focus)|focus)}
}
/// The caller must finish original-source release/cancellation before this
/// installation boundary. Cleanup cannot transfer focus or grant another input.
pub(crate) fn install_cleanup_preflighted(world:&mut World,plan:PreparedFocusCleanup)->Result<Vec<AcquiredFocus>,String>{
    let runtime=world.get_resource::<FocusRuntime>();
    if runtime.map_or(0,|runtime|runtime.revision)!=plan.revision||plan.removed.iter().any(|(domain,focus)|!cleanup_needed(world,focus)||!runtime.and_then(|runtime|runtime.domains.get(domain)).is_some_and(|actual|actual.same_target(focus))){return Err("Focus cleanup source changed; no domain was removed".into());}
    if plan.removed.is_empty(){return Ok(vec![]);}
    let revision=plan.revision.checked_add(1).ok_or("Focus cleanup revision exhausted")?;
    let mut runtime=world.remove_resource::<FocusRuntime>().ok_or("Focus cleanup resource disappeared")?;let mut originals=Vec::new();
    for(domain,focus)in plan.removed{runtime.domains.remove(&domain);originals.push(focus);}
    runtime.revision=revision;world.insert_resource(runtime);Ok(originals)
}

#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn prepared_focus_release_revision_refuses_competing_commit_without_second_mutation(){
    let mut world=World::new();let domain=FocusDomain{camera:world.spawn_empty().id(),user:0};
    let first=prepare_release(&world,domain).unwrap();let stale=prepare_release(&world,domain).unwrap();assert!(stale.current(&world));let admitted=admit_focus_before_callbacks(&world,stale).unwrap();let change=install_focus_preflighted(&mut world,first).unwrap();assert!(!change.changed());assert_eq!(world.resource::<FocusRuntime>().revision,1);assert!(install_admitted_focus_preflighted(&mut world,admitted).is_err());assert_eq!(world.resource::<FocusRuntime>().revision,1);assert!(world.resource::<FocusRuntime>().domains.is_empty());
 }
 #[test]
 fn exhausted_focus_revision_refuses_before_creating_or_replacing_any_domain(){
    let mut world=World::new();let domain=FocusDomain{camera:world.spawn_empty().id(),user:0};world.insert_resource(FocusRuntime{revision:u64::MAX,domains:HashMap::new()});assert!(prepare_release(&world,domain).is_err());assert_eq!(world.resource::<FocusRuntime>().revision,u64::MAX);assert!(world.resource::<FocusRuntime>().domains.is_empty());
 }
}
