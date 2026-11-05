//! Position acceptance keeps the very same worker, image and PCM buffer.
//! The Core candidate and its opaque post-commit receipts authenticate both ends.
use super::*;
pub(crate) struct PreparedStartupPositions {
    runtime:Option<Arc<()>>,
    rows:Vec<(VideoView,VideoView,Option<VideoAudio>,Handle<Image>)>,
}
pub(crate) fn prepare_positions(world:&World,core:&rvn_core::engine::PreparedStartupVideoFeedback)->Result<PreparedStartupPositions,String>{
    let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>().ok_or("Position preflight requires actual Startup owner")?;
    if world.contains_resource::<VnEngine>()||startup.publication_pending||startup.failed.is_some()||!startup.accepts_actual_audio_broker(){return Err("Position preflight has no admitted actual owner/broker".into());}
    startup.owner.validate_prepared_video_feedback(core).map_err(|error|error.to_string())?;
    let instance=startup.owner.snapshot().instance;let gate=startup.broker.gate();
    let mut rows:Vec<(VideoView,VideoView,Option<VideoAudio>,Handle<Image>)>=Vec::new();
    for (prior,accepted) in core.position_views(){
        if let Some(row)=rows.iter().find(|row|row.0.id==prior.id){if row.0!=*prior||row.1!=*accepted{return Err("Conflicting actual position members".into());}continue;}
        let player=world.get_resource::<Players>().and_then(|players|players.players.get(&prior.id)).ok_or("Accepted Startup position has no actual Native player")?;
        if player.instance!=instance||player.view!=*prior||!same_capability(prior,accepted)||prior.track.clip!=accepted.track.clip{return Err("Startup position would replace its actual Native source".into());}
        if world.get_resource::<Assets<Image>>().is_none_or(|images|images.get(&player.image).is_none()){return Err("Startup position image vanished before admission".into());}
        if let Some(audio)=&player.audio{let buffer=audio.0.lock().map_err(|_|"Startup position PCM poisoned")?;if !buffer.verifies_owner(instance,&gate)||!buffer.verifies_view(instance,prior)||!buffer.verifies_view(instance,accepted)||buffer.revoked{return Err("Startup position reader is not the same owned PCM source".into());}}
        rows.push((prior.clone(),accepted.clone(),player.audio.clone(),player.image.clone()));
    }
    Ok(PreparedStartupPositions{runtime:world.get_resource::<Players>().map(|players|players.restore_authority.clone()),rows})
}
pub(crate) fn install_positions(world:&mut World,plan:PreparedStartupPositions,committed:&rvn_core::engine::CommittedStartupVideoFeedback){
    let startup=world.resource::<crate::debugger_startup::DebugBootstrap>();
    assert!(committed.accepted_reports().iter().all(|receipt|startup.owner.accepts_video_report_receipt(receipt)),"Actual Core post-source was preflighted and committed under exclusive World");
    if plan.rows.is_empty(){return;}
    let mut players=world.resource_mut::<Players>();assert!(plan.runtime.as_ref().is_some_and(|runtime|Arc::ptr_eq(runtime,&players.restore_authority)));
    for(prior,accepted,audio,image)in plan.rows{
        assert!(committed.accepted_reports().iter().any(|receipt|receipt.prior_view()==&prior&&receipt.accepted_view()==&accepted));
        let player=players.players.get_mut(&prior.id).expect("Preflighted Native player remains owned");
        assert!(player.view==prior&&player.image==image&&match(&player.audio,&audio){(None,None)=>true,(Some(a),Some(b))=>Arc::ptr_eq(&a.0,&b.0),_=>false});
        // Position is observation only. In particular do not seek, set a new
        // running flag, recreate a buffer or consume its pending right sample.
        player.view=accepted;
    }
}
