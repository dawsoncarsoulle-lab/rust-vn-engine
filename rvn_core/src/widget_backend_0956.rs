//! Transient backend object admission. This is not another logical instance,
//! VM, input source or saved owner. Only Engine constructs an owner witness.
use crate::{scene_resources::TargetIdentity,widget_class_source_0950::WidgetClassIdentity};
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct WidgetBackendOwner{pub(crate) lineage:u64,pub(crate) epoch:u64,pub(crate) target:TargetIdentity,pub(crate) order:u64,pub(crate) class:WidgetClassIdentity}
impl WidgetBackendOwner{
 pub fn lineage(&self)->u64{self.lineage}pub fn epoch(&self)->u64{self.epoch}pub fn target(&self)->&TargetIdentity{&self.target}pub fn order(&self)->u64{self.order}pub fn class(&self)->&WidgetClassIdentity{&self.class}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum WidgetViewportOperation{Attach,Detach,Destroy}
/// A real backend's opaque pure preparation, not deserializable and not
/// constructible from Main pixels, an alias, an authored child or a UUID alone.
pub trait PreparedWidgetViewport:std::fmt::Debug+Send+Sync{
 fn owner(&self)->&WidgetBackendOwner;
 fn operation(&self)->WidgetViewportOperation;
 fn constructs_wrapper(&self)->bool;
 fn releases_wrapper(&self)->bool;
 fn released_object(&self)->Option<WidgetBackendObjectIdentity>{None}
 fn constructed_object(&self)->Option<WidgetBackendObjectIdentity>{None}
 fn as_any(&self)->&dyn std::any::Any;
}

/// Identity of an actual prepared/retiring backend object, distinct from the
/// serialized class and the logical instance. No camera/input grant is encoded.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct WidgetBackendObjectIdentity{pub owner:WidgetBackendOwner,pub generation:u64}
/// A pure full-tree reconciliation against one actual backend revision. Strong
/// object references here retain preparations, not accepted viewport parents.
pub trait PreparedWidgetViewportBatch:std::fmt::Debug+Send+Sync{
 fn desired_owners(&self)->&[WidgetBackendOwner];
 fn created(&self)->&[WidgetBackendObjectIdentity];
 fn retired(&self)->&[WidgetBackendObjectIdentity];
 fn as_any(&self)->&dyn std::any::Any;
}
/// Issued only after the final actual backend/source validation. A dropped
/// guard revokes its reservation; it never publishes objects or runs callbacks.
/// Commit is infallible under that reservation and invokes only the already
/// preflighted Core/publication closure, never authored VM evaluation.
pub trait WidgetViewportCommitReservation{
 fn commit(self:Box<Self>,publish:&mut dyn FnMut());
}
