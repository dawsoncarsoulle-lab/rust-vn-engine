//! Startup selection uses the completed image's retained owner and geometry.
//! No VnEngine, Game input owner or current/latest replacement frame is made.
use super::*;
pub(super) fn input(world:&mut World,header:&Header)->Option<(u64,Result<(),String>)>{
    let (instance,epoch,sequence,frame_sequence)=match header{
        Header::Input{instance,epoch,sequence,frame_sequence,..}|Header::Release{instance,epoch,sequence,frame_sequence,..}=>(*instance,*epoch,*sequence,*frame_sequence),
        _=>return None,
    };
    if sequence<=world.resource::<PreviewState>().received_sequence{return Some((sequence,Ok(())));}
    let result=(||{
        let startup=world.resource::<DebugBootstrap>();let current=startup.owner.snapshot();
        if instance!=current.instance||epoch!=current.epoch{return Err("The completed Init input owner changed".into());}
        let output=world.resource::<Output>();
        let (scope,observation)=output.completed_startup(instance,epoch,frame_sequence).ok_or("The completed Init input image expired")?;
        let physical=output.context(instance,epoch,frame_sequence).ok_or("The completed Init image has no retained physical input frame")?;
        if physical.scope!=scope{return Err("The retained Init geometry belongs to another completed image".into());}
        // Typed releases may retire a held gesture after that source advanced.
        // They authorize no selected branch, Core state or new physical input.
        if let Header::Release{release,..}=header{
            release.validate()?;let release=release.clone().into_input();output.hold(physical,&release)?;
            if matches!(release,wire::ImageInput::FocusLost){output.release_held();}
            return Ok(());
        }
        let Header::Input{input,..}=header else{unreachable!()};
        if input.len()>wire::MAX_INPUT_BYTES{return Err("Oversized Init input envelope".into());}
        if startup.publication_pending||startup.failed.is_some()||crate::debugger_startup::driver::pending(world)||crate::checkpoint_requests::world_blocked(world)
            ||paused(world)||scope.paused||scope.publication_pending||scope.control_sequence!=world.resource::<PreviewState>().sequence{
            return Err("The completed Init input publication is not interactive".into());
        }
        if world.get_resource::<crate::accessibility::Accessibility>().is_some_and(|access|access.blocked)
            ||world.get_resource::<crate::source_menus::SourceMenus>().is_some_and(|menus|menus.modal()){
            return Err("A real modal host blocks Init input".into());
        }
        crate::debugger_startup::validate_installed_imagemap_images(world)?;
        let observation=observation.ok_or("The completed image has no retained Startup observation")?;
        let source=startup.owner.imagemap_input_source(observation).map_err(|error|error.to_string())?;
        // Positive input JSON is examined only after actual owner, completed
        // image, physical geometry and waiting source admission above.
        let decoded=wire::ImageInput::decode(input)?;
        if decoded.is_passive(){
            world.resource::<Output>().hold(physical,&decoded)?;
            if matches!(decoded,wire::ImageInput::FocusLost){world.resource::<Output>().release_held();}
            return Ok(());
        }
        let wire::ImageInput::Pointer{kind,button,point,..}=&decoded else{return Err("This actual Init Imagemap has no keyboard/text input target".into());};
        let point=bevy::prelude::Vec2::from_array(*point);
        if *kind==wire::PointerKind::Move{
            let window=world.query_filtered::<bevy::prelude::Entity,bevy::prelude::With<bevy::window::PrimaryWindow>>().iter(world).next().ok_or("The actual preview input Window is absent")?;
            world.get_mut::<bevy::window::Window>(window).ok_or("The actual preview input Window disappeared")?.set_cursor_position(Some(point));return Ok(());
        }
        if *kind!=wire::PointerKind::Down||*button!=0{return Ok(());}
        let Some(selected)=physical.startup_imagemap_hit(point)else{return Ok(());};
        if !world.resource::<Output>().hold(physical,&decoded)?{return Ok(());}
        let prepared=crate::debugger_startup::prepare_world_imagemap_selection(world,&source,selected)?;
        crate::debugger_startup::driver::retain_imagemap_selection(world,prepared)
    })();
    Some((sequence,result))
}
