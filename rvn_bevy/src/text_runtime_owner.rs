//! Borrowed projection of the actual mutually exclusive player/startup owner.
use bevy::prelude::*;
pub(crate) struct TextRuntimeOwner<'a>{
    pub(crate) instance:u64,
    pub(crate) epoch:u64,
    /// Borrow of the same actual owner, never a second mutable state store.
    pub(crate) state:&'a rvn_core::GameState,
    pub(crate) resources:&'a rvn_core::string_table::StringTableResources,
    pub(crate) locale:Option<&'a rvn_core::locale::LocaleManager>,
    pub(crate) blocked:bool,
}
pub(crate) fn current(world:&World)->Result<TextRuntimeOwner<'_>,String>{
    let engine=world.get_resource::<crate::resources::VnEngine>();
    let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>();
    match(engine,startup){
        (Some(engine),None)=>Ok(TextRuntimeOwner{instance:engine.0.instance_id(),epoch:engine.0.runtime_epoch(),state:&engine.0.state,resources:engine.0.text_resources(),locale:engine.0.locale.as_ref(),blocked:engine.0.renderer.scene_batch_pending()||engine.0.renderer.scene_frame_failed}),
        (None,Some(startup))=>{let snapshot=startup.owner.snapshot();Ok(TextRuntimeOwner{instance:snapshot.instance,epoch:snapshot.epoch,state:startup.owner.state(),resources:startup.owner.text_resources(),locale:startup.owner.locale(),blocked:startup.publication_pending||startup.failed.is_some()})},
        _=>Err("Text provider requires exactly one actual player/startup owner".into()),
    }
}
