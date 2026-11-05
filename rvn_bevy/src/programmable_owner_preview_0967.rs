//! Owner input witnesses belong to the same bounded PhysicalFrame allocation.
//! This port retains actual installed data; it does not acquire focus, route
//! owner input, synthesize lifecycle, or create a new wire/Main authority.
use super::*;
use super::{owner_hosts_0943 as owners,owner_focus_0943 as focus};
#[derive(Clone)]
pub(crate) struct PreviewOwnerFrame{owners:owners::CapturedOwnerFrame,focus:Option<focus::AcquiredFocus>,cost:usize}
impl PreviewOwnerFrame{
 pub(crate) fn cost(&self)->usize{self.cost}
 pub(crate) fn current(&self,world:&World)->bool{
  if !self.owners.current(world){return false;}
  let Ok(current)=focus::captured_focus(world,self.owners.camera())else{return false;};
  match (&self.focus,current){(None,None)=>true,(Some(before),Some(now))=>before.same_target(now),_=>false}
 }
 pub(crate) fn belongs_to(&self,scope:&rvn_core::editor_preview::FrameScope)->bool{
  self.owners.belongs_to(scope.instance,scope.epoch)
 }
 pub(crate) fn owners(&self)->&owners::CapturedOwnerFrame{&self.owners}
 pub(crate) fn focus(&self)->Option<&focus::AcquiredFocus>{self.focus.as_ref()}
}
pub(crate) fn preview_cost(world:&World)->Option<usize>{
 if !owners::snapshot_expected(world){return Some(0);}
 let owner_cost=owners::installed_snapshot_cost(world)?;
 // Resolve camera only from the actual installed metric, never a Main token.
 let camera=owners::installed_camera(world)?;
 let acquired=focus::captured_focus(world,camera).ok()?;
 let focus_cost=if let Some(acquired)=acquired{focus::captured_focus_cost(acquired)?}else{0};
 std::mem::size_of::<PreviewOwnerFrame>().checked_add(owner_cost)?.checked_add(focus_cost).filter(|bytes|*bytes<=2*1024*1024)
}
// Keep Option arithmetic explicit: no saturation can turn overflow into a
// cheap context which then bypasses the shared Output union budget.
pub(crate) fn capture(world:&World)->Option<Option<PreviewOwnerFrame>>{
 if !owners::snapshot_expected(world){return Some(None);}
 let cost=preview_cost(world)?;let owners=owners::capture_installed(world)?;
 let acquired=focus::captured_focus(world,owners.camera()).ok()?.cloned();
 if acquired.as_ref().is_some_and(|acquired|!owners.rows().iter().any(|row|&row.source==acquired.source())){return None;}
 let frame=PreviewOwnerFrame{owners,focus:acquired,cost};frame.current(world).then_some(Some(frame))
}
