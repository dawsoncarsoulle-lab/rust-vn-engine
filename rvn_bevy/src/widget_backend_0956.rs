//! Actual backend wrapper ownership, independent of paint and authored child.
//! There is one existing viewport parent and no second VM/object writer. ECS
//! layout nodes and physical frame witnesses retain only weak wrapper handles.
use bevy::prelude::*;
use rvn_core::widget_backend_0956::{PreparedWidgetViewport,WidgetBackendOwner,WidgetViewportOperation};
use std::{collections::BTreeMap,sync::{Arc,Mutex,Weak,atomic::{AtomicU64,AtomicBool,Ordering}}};
static NEXT_WRAPPER:AtomicU64=AtomicU64::new(1);
#[derive(Debug)]
pub(crate) struct WidgetWrapper{
 owner:WidgetBackendOwner,generation:u64,
 // The real backend root exists before Bevy paints/projects it into ECS.
 // It is a generic host, never an authored Component or fake child Panel.
 node:Mutex<NodeBundle>,attached:AtomicBool,
}
impl WidgetWrapper{
 pub(crate) fn owner(&self)->&WidgetBackendOwner{&self.owner}
 pub(crate) fn generation(&self)->u64{self.generation}
 pub(crate) fn attached(&self)->bool{self.attached.load(Ordering::Acquire)}
 pub(crate) fn prepared_node(&self)->Result<NodeBundle,String>{self.node.lock().map(|node|node.clone()).map_err(|_|"Widget backend node lock unavailable".into())}
}
#[derive(Default)]
struct ViewportTree{revision:u64,reserved:Option<u64>,children:BTreeMap<String,Arc<WidgetWrapper>>}
#[derive(Clone,Default)]
pub(crate) struct WidgetBackend{tree:Arc<Mutex<ViewportTree>>}
#[derive(Debug)]
struct Prepared{
 backend:Weak<Mutex<ViewportTree>>,revision:u64,owner:WidgetBackendOwner,operation:WidgetViewportOperation,
 previous:Option<Arc<WidgetWrapper>>,next:Option<Arc<WidgetWrapper>>,
}
impl PreparedWidgetViewport for Prepared{
 fn owner(&self)->&WidgetBackendOwner{&self.owner}
 fn operation(&self)->WidgetViewportOperation{self.operation}
 fn constructs_wrapper(&self)->bool{self.next.is_some()}
 fn releases_wrapper(&self)->bool{self.previous.is_some()}
 fn released_object(&self)->Option<rvn_core::widget_backend_0956::WidgetBackendObjectIdentity>{self.previous.as_ref().map(|old|rvn_core::widget_backend_0956::WidgetBackendObjectIdentity{owner:old.owner.clone(),generation:old.generation})}
 fn constructed_object(&self)->Option<rvn_core::widget_backend_0956::WidgetBackendObjectIdentity>{self.next.as_ref().map(|next|rvn_core::widget_backend_0956::WidgetBackendObjectIdentity{owner:next.owner.clone(),generation:next.generation})}
 fn as_any(&self)->&dyn std::any::Any{self}
}
fn key(owner:&WidgetBackendOwner)->String{format!("{}:{}:{}:{}:{}",owner.lineage(),owner.epoch(),owner.target().alias.len(),owner.target().alias,owner.order())}
impl WidgetBackend{
 pub(crate) fn prepare(&self,owner:&WidgetBackendOwner,operation:WidgetViewportOperation)->Result<Arc<dyn PreparedWidgetViewport>,String>{
  if owner.lineage()==0{return Err("Widget wrapper has no actual Engine lineage".into());}
  let tree=self.tree.try_lock().map_err(|_|"Widget viewport ownership is busy or unavailable")?;
  if tree.reserved.is_some(){return Err("Widget viewport publication is reserved by its actual owner".into());}
  let previous=tree.children.get(&key(owner)).cloned();
  if previous.as_ref().is_some_and(|wrapper|wrapper.owner!=*owner||!wrapper.attached()){return Err("Widget viewport child differs from actual owner/source incarnation".into());}
  let next=match operation{
   WidgetViewportOperation::Attach=>{
    if previous.is_some(){return Err("Widget wrapper already has its viewport parent".into());}
    let generation=NEXT_WRAPPER.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|value|value.checked_add(1)).map_err(|_|"Widget backend wrapper generations exhausted")?;
    Some(Arc::new(WidgetWrapper{owner:owner.clone(),generation,node:Mutex::new(NodeBundle{background_color:BackgroundColor(Color::NONE),focus_policy:bevy::ui::FocusPolicy::Pass,..default()}),attached:AtomicBool::new(false)}))
   },
   WidgetViewportOperation::Detach=>{if previous.is_none(){return Err("Widget wrapper has no actual viewport parent to remove".into());}None},
   WidgetViewportOperation::Destroy=>None,
  };
  tree.revision.checked_add(1).ok_or("Widget viewport revision exhausted")?;
  Ok(Arc::new(Prepared{backend:Arc::downgrade(&self.tree),revision:tree.revision,owner:owner.clone(),operation,previous,next}))
 }
 fn checked<'a>(&self,tree:&ViewportTree,prepared:&'a dyn PreparedWidgetViewport)->Result<&'a Prepared,String>{
  if tree.reserved.is_some(){return Err("Widget viewport publication is reserved by its actual owner".into());}
  let prepared=prepared.as_any().downcast_ref::<Prepared>().ok_or("Widget backend preparation belongs to another renderer type")?;
  if prepared.backend.upgrade().is_none_or(|backend|!Arc::ptr_eq(&backend,&self.tree))||prepared.revision!=tree.revision{return Err("Widget viewport owner changed during callback preparation".into());}
  let current=tree.children.get(&key(&prepared.owner));
  if match(current,prepared.previous.as_ref()){(None,None)=>false,(Some(actual),Some(expected))=>!Arc::ptr_eq(actual,expected),_=>true}{return Err("Widget viewport parent changed before publication".into());}
  if prepared.previous.as_ref().is_some_and(|wrapper|wrapper.owner!=prepared.owner||!wrapper.attached())||prepared.next.as_ref().is_some_and(|wrapper|wrapper.owner!=prepared.owner||wrapper.attached()){return Err("Widget backend wrapper transition is no longer current".into());}
  tree.revision.checked_add(1).ok_or("Widget viewport revision exhausted")?;Ok(prepared)
 }
 pub(crate) fn validate(&self,prepared:&dyn PreparedWidgetViewport)->Result<(),String>{let tree=self.tree.try_lock().map_err(|_|"Widget viewport ownership is busy or unavailable")?;self.checked(&tree,prepared)?;Ok(())}
 /// No VM callback runs under this lock. All fallible validation finishes
 /// before changing either backend parent ownership or the actual Core state.
 pub(crate) fn commit(&self,prepared:&dyn PreparedWidgetViewport,commit:impl FnOnce())->Result<(),String>{
  let mut tree=self.tree.try_lock().map_err(|_|"Widget viewport ownership is busy or unavailable")?;
  let prepared=self.checked(&tree,prepared)?;
  if let Some(previous)=&prepared.previous{previous.attached.store(false,Ordering::Release);}
  tree.children.remove(&key(&prepared.owner));
  if let Some(next)=&prepared.next{next.attached.store(true,Ordering::Release);tree.children.insert(key(&prepared.owner),Arc::clone(next));}
  tree.revision+=1;commit();Ok(())
 }
 pub(crate) fn wrapper(&self,owner:&WidgetBackendOwner)->Option<Weak<WidgetWrapper>>{let tree=self.tree.try_lock().ok()?;let wrapper=tree.children.get(&key(owner))?;(wrapper.owner==*owner&&wrapper.attached()).then(||Arc::downgrade(wrapper))}
}

use rvn_core::widget_backend_0956::{PreparedWidgetViewportBatch,WidgetBackendObjectIdentity,WidgetViewportCommitReservation};
static NEXT_RESERVATION:AtomicU64=AtomicU64::new(1);
#[derive(Debug)]
struct PreparedBatch{
 backend:Weak<Mutex<ViewportTree>>,revision:u64,previous:BTreeMap<String,Arc<WidgetWrapper>>,next:BTreeMap<String,Arc<WidgetWrapper>>,
 owners:Vec<WidgetBackendOwner>,created:Vec<WidgetBackendObjectIdentity>,retired:Vec<WidgetBackendObjectIdentity>,
}
impl PreparedWidgetViewportBatch for PreparedBatch{
 fn desired_owners(&self)->&[WidgetBackendOwner]{&self.owners}
 fn created(&self)->&[WidgetBackendObjectIdentity]{&self.created}
 fn retired(&self)->&[WidgetBackendObjectIdentity]{&self.retired}
 fn as_any(&self)->&dyn std::any::Any{self}
}
struct Reservation{backend:WidgetBackend,ticket:u64,prepared:Arc<dyn PreparedWidgetViewportBatch>,committed:bool}
impl Drop for Reservation{
 fn drop(&mut self){
  if self.committed{return;}
  let mut tree=self.backend.tree.lock().unwrap_or_else(|poisoned|poisoned.into_inner());
  if tree.reserved==Some(self.ticket){tree.reserved=None;}
 }
}
impl WidgetViewportCommitReservation for Reservation{
 fn commit(mut self:Box<Self>,publish:&mut dyn FnMut()){
  // Only this owned reservation can mutate its parent tree. Pure readers can
  // inspect it, but competing prepares/commits refuse instead of blocking a VM.
  let backend=self.backend.clone();let mut tree=backend.tree.lock().unwrap_or_else(|poisoned|poisoned.into_inner());
  assert_eq!(tree.reserved,Some(self.ticket),"exclusive backend reservation was lost");
  let prepared=self.prepared.as_any().downcast_ref::<PreparedBatch>().expect("reservation retained its exact validated backend type");
  for(key,old)in &tree.children{if prepared.next.get(key).is_none_or(|next|!Arc::ptr_eq(old,next)){old.attached.store(false,Ordering::Release);}}
  for wrapper in prepared.next.values(){wrapper.attached.store(true,Ordering::Release);}
  tree.children=prepared.next.clone();tree.revision+=1;tree.reserved=None;self.committed=true;
  publish();
 }
}
impl Drop for ViewportTree{fn drop(&mut self){for wrapper in self.children.values(){wrapper.attached.store(false,Ordering::Release);}}}
impl WidgetBackend{
 /// The authoritative logical batch supplies already admitted exact owners.
 /// No root geometry, current alias or UI label is used to discover a class.
 pub(crate) fn prepare_batch(&self,owners:&[WidgetBackendOwner])->Result<Arc<dyn PreparedWidgetViewportBatch>,String>{self.prepare_batch_after(owners,None)}
 pub(crate) fn prepare_batch_after(&self,owners:&[WidgetBackendOwner],transition:Option<&dyn PreparedWidgetViewport>)->Result<Arc<dyn PreparedWidgetViewportBatch>,String>{
  if owners.len()>32{return Err("Widget backend exceeds actual instance bound".into());}
  let tree=self.tree.try_lock().map_err(|_|"Widget viewport ownership is busy or unavailable")?;
  if tree.reserved.is_some(){return Err("Widget viewport publication is reserved by its actual owner".into());}
  tree.revision.checked_add(1).ok_or("Widget viewport revision exhausted")?;
  let transition=transition.map(|prepared|self.checked(&tree,prepared)).transpose()?;
  // Reuse the exact pure object which received PreConstruct/Construct in this
  // transaction. A second allocation here would give those events a false
  // generation and allow a coalesced UI batch to publish a different wrapper.
  let mut available=tree.children.clone();
  if let Some(transition)=transition{
   available.remove(&key(&transition.owner));
   if let Some(next)=&transition.next{available.insert(key(&transition.owner),Arc::clone(next));}
   let wants=owners.iter().any(|owner|owner==&transition.owner);
   if wants!=transition.next.is_some(){return Err("Widget scene candidate contradicts its exact prepared viewport operation".into());}
  }
  let mut next=BTreeMap::new();let mut created=Vec::new();
  for owner in owners{
   if owner.lineage()==0{return Err("Widget replacement lacks its actual Engine lineage".into());}
   let k=key(owner);if next.contains_key(&k){return Err("Duplicate actual owner in Widget backend replacement".into());}
   let wrapper=if let Some(existing)=available.get(&k){
    let from_transition=transition.and_then(|transition|transition.next.as_ref()).is_some_and(|prepared|Arc::ptr_eq(prepared,existing));
    if existing.owner!=*owner||existing.attached()==from_transition{return Err("Widget replacement found a different source incarnation under its exact parent key".into());}
    if from_transition{created.push(WidgetBackendObjectIdentity{owner:owner.clone(),generation:existing.generation});}Arc::clone(existing)
   }else{
    let generation=NEXT_WRAPPER.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|value|value.checked_add(1)).map_err(|_|"Widget backend wrapper generations exhausted")?;
    let wrapper=Arc::new(WidgetWrapper{owner:owner.clone(),generation,node:Mutex::new(NodeBundle{background_color:BackgroundColor(Color::NONE),focus_policy:bevy::ui::FocusPolicy::Pass,..default()}),attached:AtomicBool::new(false)});
    created.push(WidgetBackendObjectIdentity{owner:owner.clone(),generation});wrapper
   };next.insert(k,wrapper);
  }
  let retired=tree.children.iter().filter(|(key,_)|!next.contains_key(*key)).map(|(_,wrapper)|WidgetBackendObjectIdentity{owner:wrapper.owner.clone(),generation:wrapper.generation}).collect();
  Ok(Arc::new(PreparedBatch{backend:Arc::downgrade(&self.tree),revision:tree.revision,previous:tree.children.clone(),next,owners:owners.to_vec(),created,retired}))
 }
 fn checked_batch<'a>(&self,tree:&ViewportTree,batch:&'a dyn PreparedWidgetViewportBatch)->Result<&'a PreparedBatch,String>{
  if tree.reserved.is_some(){return Err("Widget viewport publication is reserved by its actual owner".into());}
  let batch=batch.as_any().downcast_ref::<PreparedBatch>().ok_or("Widget replacement belongs to another backend type")?;
  if batch.backend.upgrade().is_none_or(|backend|!Arc::ptr_eq(&backend,&self.tree))||batch.revision!=tree.revision||batch.previous.len()!=tree.children.len(){return Err("Widget backend changed after full-tree preparation".into());}
  if batch.previous.iter().any(|(key,expected)|tree.children.get(key).is_none_or(|actual|!Arc::ptr_eq(expected,actual)||!actual.attached())){return Err("Widget replacement parent/source ownership changed before commit".into());}
  if batch.next.values().any(|wrapper|{let existing=batch.previous.get(&key(&wrapper.owner));match existing{Some(old)=>!Arc::ptr_eq(old,wrapper)||!wrapper.attached(),None=>wrapper.attached()}}){return Err("Widget replacement object has another actual parent".into());}
  tree.revision.checked_add(1).ok_or("Widget viewport revision exhausted")?;Ok(batch)
 }
 pub(crate) fn validate_batch(&self,batch:&dyn PreparedWidgetViewportBatch)->Result<(),String>{let tree=self.tree.try_lock().map_err(|_|"Widget viewport ownership is busy or unavailable")?;self.checked_batch(&tree,batch)?;Ok(())}
 pub(crate) fn reserve_batch(&self,batch:Arc<dyn PreparedWidgetViewportBatch>)->Result<Box<dyn WidgetViewportCommitReservation>,String>{
  let mut tree=self.tree.try_lock().map_err(|_|"Widget viewport ownership is busy or unavailable")?;self.checked_batch(&tree,batch.as_ref())?;
  let ticket=NEXT_RESERVATION.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|value|value.checked_add(1)).map_err(|_|"Widget viewport reservations exhausted")?;
  tree.reserved=Some(ticket);Ok(Box::new(Reservation{backend:self.clone(),ticket,prepared:batch,committed:false}))
 }
}
