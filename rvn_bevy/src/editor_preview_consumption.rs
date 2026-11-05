//! Authenticated Main-consumed image credit; not runtime command authority.
use super::*;

impl Latest {
    pub(super) fn can_write(&self)->bool {
        self.failure_reason.is_none() && self.writing.is_none() && self.frame.as_ref().is_some_and(|packet| {
            !matches!(&packet.header,Header::Frame{..}) || self.awaiting_consumption.is_none()
        })
    }
}

impl Output {
    /// Called after the World drained all earlier controls, debugger requests
    /// and physical releases. Matching the in-flight write records the race;
    /// only its actual successful finish can promote the Main pin.
    pub(super) fn consume_frame(&self,instance:u64,epoch:u64,sequence:u64)->Result<bool,String> {
        if instance==0 || sequence==0{return Err("Invalid Main image consumption identity".into());}
        let mut queue=self.queue.0.lock().map_err(|_|"Image queue poisoned")?;
        if queue.session_retired || self.phase!=queue.phase{return Ok(false);}
        let Some((owner,current_epoch))=queue.owner else{return Err("No actual published image owner".into());};
        if instance!=owner || epoch>current_epoch{return Err("Foreign Main image consumption refused".into());}
        if epoch<current_epoch || sequence<=queue.consumption_floor{return Ok(false);}
        if queue.writing.as_ref().is_some_and(|(_,image)| (image.scope.instance,image.scope.epoch,image.scope.sequence)==(instance,epoch,sequence)) {
            queue.writing_consumed=true;return Ok(false);
        }
        if queue.awaiting_consumption.as_ref().is_some_and(|image| (image.scope.instance,image.scope.epoch,image.scope.sequence)==(instance,epoch,sequence)) {
            let image=queue.awaiting_consumption.take().expect("the exact actual successor was checked");
            queue.consumption_floor=sequence;queue.main_consumed=Some(image);
            self.queue.1.notify_one();return Ok(true);
        }
        Err("Main consumption has no actual written successor".into())
    }
}

#[derive(Resource,Default)]
pub(super) struct PendingConsumed(pub(super) Vec<Header>);

/// First admits commands immediately but queues physical inputs for PreUpdate.
/// This barrier follows drain_input, so no preceding Release can lose its pin
/// while it is still awaiting admission. It also follows all First debugger
/// and control dispatch. These headers never change PreviewState ordinals.
pub(super) fn drain_consumed(world:&mut World) {
    let pending=world.get_resource_mut::<PendingConsumed>()
        .map(|mut value|std::mem::take(&mut value.0)).unwrap_or_default();
    for header in pending {
        if let Header::FrameConsumed{instance,epoch,frame_sequence}=header {
            if world.resource::<Output>().consume_frame(instance,epoch,frame_sequence).is_err(){world.resource::<Output>().failed();}
        }
    }
}
