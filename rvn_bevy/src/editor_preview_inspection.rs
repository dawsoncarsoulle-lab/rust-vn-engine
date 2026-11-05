//! Read-only physical VN instances, retained with their completed image.
//! A renderer-only row never mints a Story identity from an authored alias.
use bevy::prelude::*;
use rvn_core::{editor_inspection::{FrameIdentity,Inspection,Instance,InstanceKey,Key,MAX_INSPECTION_ROWS},
    editor_preview::FrameScope,scene_resources::{ResourceToken,TargetIdentity}};
use serde::{Serialize,ser::{Error as _,SerializeSeq}};
use crate::{components::*,checkpoint_pcm::CheckpointPcm};

pub(crate) fn project(world:&World,logical:&Inspection,scope:&FrameScope)->Result<Inspection,String>{
    if logical.position!=scope.position{return Err("Physical inspection has a different installed position".into());}
    if logical.unavailable.is_some()||logical.frame.instance!=scope.instance||logical.frame.epoch!=scope.epoch||scope.sequence==0{return Err("Physical inspection has no matching installed image".into());}
    let game=&world.get_resource::<crate::resources::VnEngine>().ok_or("Physical inspection has no player instance")?.0;
    if game.instance_id()!=scope.instance||game.runtime_epoch()!=scope.epoch{return Err("Physical inspection belongs to a different player instance".into());}
    // Only fixed-size metadata is retained. The model already bounds Music at
    // Current+64 outgoing readers and one-shots at 64; no PCM buffer is copied.
    let mut sources=Vec::new();
    let max_sources=rvn_core::audio_playback::MAX_AUDIO_PLAYBACKS+rvn_core::scene_resources::MAX_MOTIONS+1;
    for entity in world.iter_entities(){
        if entity.get::<crate::scene_audio::OwnedAudio>().is_none()&&entity.get::<crate::checkpoint_sources::ExactMusicSource>().is_none(){continue;}
        let Some(handle)=entity.get::<Handle<CheckpointPcm>>()else{continue;};
        let pcm=world.get_resource::<Assets<CheckpointPcm>>().and_then(|assets|assets.get(handle)).ok_or("Physical audio source has lost its actual decoder asset")?;
        if sources.len()>=max_sources{return Err("Inspection exceeds the existing bounded audio source count".into());}
        sources.push((entity.id().to_bits(),pcm));
    }
    let readers=sources.iter().map(|(_,reader)|*reader).collect::<Vec<_>>();
    // The current renderer's real fence also rejects a Source from an old
    // independent Engine whose numeric target and epoch happen to be equal.
    let metadata=game.renderer.audio_checkpoints.gate().snapshot_with_status(&readers,Ok)?;
    let pcm=sources.iter().zip(metadata).map(|((entity,_),value)|(*entity,value)).collect::<std::collections::BTreeMap<_,_>>();
    struct Rows<'a>{world:&'a World,logical:&'a Inspection,scope:&'a FrameScope,
        pcm:&'a std::collections::BTreeMap<u64,(rvn_core::audio_checkpoint::AudioCheckpoint,bool)>}
    fn task_parts<'a>(logical:&'a Inspection,identity:&TargetIdentity,token:&rvn_core::scene_tasks::TaskToken)->Result<&'a Instance,String>{
        if logical.frame.epoch!=token.runtime_epoch{return Err("Rendered resource lease has a different installed epoch".into());}
        logical.rows.iter().find(|row|matches!(&row.key,InstanceKey::Task{key,revision,target}
            if key==&token.key&&*revision==token.revision&&target==identity))
            .ok_or_else(||"Rendered resource has no exact logical task".into())
    }
    fn task<'a>(logical:&'a Inspection,token:&ResourceToken)->Result<&'a Instance,String>{task_parts(logical,&token.target,&token.task)}
    fn target<'a>(logical:&'a Inspection,identity:&TargetIdentity)->Result<&'a Instance,String>{
        logical.rows.iter().find(|row|matches!(&row.key,InstanceKey::Target{identity:actual,..}if actual==identity))
            .ok_or_else(||"Rendered resource has no exact logical target".into())
    }
    fn lease(token:&ResourceToken)->Key<'_>{Key::Task{key:&token.task.key,revision:token.task.revision,target:&token.target}}
    struct Sections<'a>(&'a [TextSection]);
    impl Serialize for Sections<'_>{
        fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error>{
            #[derive(Serialize)]struct Section<'a>{text:&'a str,font_size:f32,color:[f32;4]}
            let mut seq=serializer.serialize_seq(None)?;
            for section in self.0{let color=section.style.color.to_srgba();
                let color=[color.red,color.green,color.blue,color.alpha];
                if !section.style.font_size.is_finite()||color.iter().any(|value|!value.is_finite()){return Err(S::Error::custom("Inspection text style is not finite"));}
                seq.serialize_element(&Section{text:&section.value,font_size:section.style.font_size,color})?;
            }seq.end()
        }
    }
    #[derive(Serialize)]struct Playback{paused:bool,speed:f32,volume:f32}
    #[derive(Serialize)]struct Details<'a>{
        renderer_kind:&'static str,entity:u64,parent_entity:Option<u64>,logical_target:Option<&'a TargetIdentity>,
        // Inspection keys describe the associated task. Runtime capability
        // objects are neither serialized nor reconstructed by this channel.
        retired:Option<Key<'a>>,audio:Option<Key<'a>>,animation:Option<Key<'a>>,
        fade:Option<Key<'a>>,audio_fade:Option<Key<'a>>,sprite:Option<&'a str>,
        character:Option<&'a str>,layer:Option<&'a str>,text_sections:Option<Sections<'a>>,choice:Option<usize>,
        size:Option<[f32;2]>,transform:Option<[[f32;4];4]>,visible:Option<bool>,
        sprite_color:Option<[f32;4]>,sprite_rect:Option<[f32;4]>,flip:Option<[bool;2]>,playback:Option<Playback>,
        checkpoint:Option<&'a rvn_core::audio_checkpoint::AudioCheckpoint>,decoder_parked:Option<bool>,
        cursor_semantics:&'static str,
    }
    #[derive(Serialize)]struct Physical<'a>{key:Key<'a>,parent:Option<&'a InstanceKey>,label:&'a str,parked:bool,details:Details<'a>}
    impl Serialize for Rows<'_>{
        fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error>{
            let mut seq=serializer.serialize_seq(None)?;let mut count=self.logical.rows.len();
            for row in &self.logical.rows{seq.serialize_element(row)?;}
            let game=self.logical.rows.iter().find(|row|row.key==InstanceKey::Game)
                .ok_or_else(||S::Error::custom("Physical inspection has no Game root"))?;
            for entity in self.world.iter_entities(){
                // Programmable controls already have a richer source-bound row.
                if entity.contains::<crate::programmable_ui::Control>()||entity.contains::<crate::composed_motion::InterfaceTarget>(){continue;}
                let retired=entity.get::<crate::scene_retired::RetiredProjection>();
                let audio=entity.get::<crate::scene_audio::OwnedAudio>();
                let music=entity.get::<crate::checkpoint_sources::ExactMusicSource>();
                let cinematic=entity.get::<crate::systems::cinematic::CurrentCinematic>();
                let animation=entity.get::<crate::scene_effects::AnimationLease>();
                let fade=entity.get::<crate::scene_effects::FadeLease>();
                let audio_fade=entity.get::<crate::scene_effects::AudioLease>();
                let sprite=entity.get::<VnSprite>();let layer=entity.get::<crate::layered_characters::LayerTarget>();
                let kind=if retired.is_some(){"Retired"}else if audio.is_some(){"Owned audio"}else if music.is_some(){"Music"}
                    else if cinematic.is_some()||entity.contains::<VnCinematic>(){"Cinematic"}else if entity.contains::<VnBackground>(){"Background"}
                    else if layer.is_some(){"Sprite layer"}else if sprite.is_some(){"Sprite"}else if entity.contains::<DialogueBox>(){"Dialogue box"}
                    else if entity.contains::<CharacterNameText>(){"Character name"}else if entity.contains::<DialogueText>(){"Dialogue text"}
                    else if entity.contains::<ChoiceButton>(){"Choice"}else if entity.contains::<ChoiceContainer>(){"Choices"}
                    else if entity.contains::<ImagemapBackground>(){"Imagemap"}else if entity.contains::<SfxSource>(){"Legacy sound"}
                    else if animation.is_some()||fade.is_some()||audio_fade.is_some(){"Effect projection"}else{continue;};
                if count>=MAX_INSPECTION_ROWS{return Err(S::Error::custom("Inspection physical instance budget"));}count+=1;
                let mut parent=game;let mut identity=None;
                for token in [retired.map(|value|&value.token),audio.map(|value|&value.token),animation.map(|value|&value.0),fade.map(|value|&value.0),audio_fade.map(|value|&value.0)].into_iter().flatten(){
                    let associated=task(self.logical,token).map_err(S::Error::custom)?;
                    if parent.key==InstanceKey::Game{parent=associated;identity=Some(&token.target);}
                }
                if let Some(music)=music{
                    if music.instance!=self.scope.instance||music.epoch!=self.scope.epoch{return Err(S::Error::custom("Music entity belongs to another player instance"));}
                    let associated=if let Some(outgoing)=&music.authority.outgoing{
                        task_parts(self.logical,&music.authority.target,outgoing).map_err(S::Error::custom)?
                    }else{target(self.logical,&music.authority.target).map_err(S::Error::custom)?};
                    parent=associated;identity=Some(&music.authority.target);
                }
                if let Some(cinematic)=cinematic{
                    if cinematic.instance_id!=self.scope.instance||cinematic.epoch!=self.scope.epoch{return Err(S::Error::custom("Cinematic entity belongs to another player instance"));}
                    parent=target(self.logical,&cinematic.target).map_err(S::Error::custom)?;identity=Some(&cinematic.target);
                }
                let matrix=entity.get::<GlobalTransform>().map(|transform|transform.compute_matrix());
                let size=entity.get::<Node>().map(Node::size).or_else(||entity.get::<Sprite>().and_then(|sprite|sprite.custom_size));
                if matrix.is_some_and(|matrix|!matrix.is_finite())||size.is_some_and(|size|!size.is_finite()){return Err(S::Error::custom("Inspection geometry is not finite"));}
                let physical_sprite=entity.get::<Sprite>();let color=physical_sprite.map(|sprite|sprite.color.to_srgba())
                    .map(|color|[color.red,color.green,color.blue,color.alpha]);
                let rect=physical_sprite.and_then(|sprite|sprite.rect).map(|rect|[rect.min.x,rect.min.y,rect.max.x,rect.max.y]);
                let playback=entity.get::<bevy::audio::PlaybackSettings>().map(|settings|Playback{paused:settings.paused,speed:settings.speed,volume:settings.volume.get()});
                if color.is_some_and(|color|color.iter().any(|value|!value.is_finite()))||rect.is_some_and(|rect|rect.iter().any(|value|!value.is_finite()))
                    ||playback.as_ref().is_some_and(|playback|!playback.speed.is_finite()||!playback.volume.is_finite()){return Err(S::Error::custom("Inspection renderer properties are not finite"));}
                let id=entity.id().to_bits();
                let (checkpoint,decoder_parked)=self.pcm.get(&id).map_or((None,None),|(checkpoint,parked)|(Some(checkpoint),Some(*parked)));
                seq.serialize_element(&Physical{key:Key::RenderedEntity{entity:id},parent:Some(&parent.key),label:kind,parked:parent.parked,
                    details:Details{renderer_kind:kind,entity:id,parent_entity:entity.get::<Parent>().map(|parent|parent.get().to_bits()),logical_target:identity,
                        retired:retired.map(|value|lease(&value.token)),audio:audio.map(|value|lease(&value.token)),animation:animation.map(|value|lease(&value.0)),fade:fade.map(|value|lease(&value.0)),audio_fade:audio_fade.map(|value|lease(&value.0)),
                        sprite:sprite.map(|value|value.id.as_str()),character:layer.map(|value|value.character.as_str()),layer:layer.map(|value|value.layer.as_str()),
                        text_sections:entity.get::<Text>().map(|text|Sections(&text.sections)),choice:entity.get::<ChoiceButton>().map(|value|value.0),
                        size:size.map(|size|size.to_array()),transform:matrix.map(|matrix|matrix.to_cols_array_2d()),visible:entity.get::<InheritedVisibility>().map(|visibility|visibility.get()),
                        sprite_color:color,sprite_rect:rect,flip:physical_sprite.map(|sprite|[sprite.flip_x,sprite.flip_y]),playback,
                        checkpoint,decoder_parked,cursor_semantics:"Consumed interleaved Source samples; hardware and resampler buffers are not observed"}})?;
            }
            seq.end()
        }
    }
    logical.project_rows(FrameIdentity::of(scope),&Rows{world,logical,scope,pcm:&pcm})
}

#[cfg(test)]
mod contracts{
    use super::*;use bevy::audio::Decodable;use crate::resources::VnEngine;
    fn game(source:&str)->rvn_core::Engine<crate::bevy_renderer::BevyRenderer>{
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();
        game.step_until_interaction().unwrap();game
    }
    fn scope(world:&World,sequence:u64)->FrameScope{
        let game=&world.resource::<VnEngine>().0;
        FrameScope{instance:game.instance_id(),epoch:game.runtime_epoch(),sequence,control_sequence:0,control_ack:0,control_error:None,
            paused:false,publication_pending:false, finished: None,step_allowed:false,interaction:None,active:None,parked:Vec::new(),
            position:game.state.scene_execution.as_ref().unwrap().live_position.clone()}
    }
    fn publish(world:&mut World){
        crate::checkpoint_sources::prime_assets_for_test(world).unwrap();crate::scene_publication::consume(world);
        assert!(!world.resource::<VnEngine>().0.renderer.scene_batch_pending());assert!(!world.resource::<VnEngine>().0.renderer.scene_frame_failed);
    }
    fn image(world:&World,sequence:u64)->Inspection{
        let scope=scope(world,sequence);let logical=rvn_core::editor_inspection::capture(&world.resource::<VnEngine>().0.state,
            FrameIdentity{instance:scope.instance,epoch:scope.epoch,sequence:0});assert!(logical.unavailable.is_none(),"{:?}",logical.unavailable);
        project(world,&logical,&scope).unwrap()
    }
    fn frozen(world:&World)->serde_json::Value{
        let game=&world.resource::<VnEngine>().0;serde_json::json!({"state":game.state,"epoch":game.runtime_epoch(),"history":game.history.entries().iter().map(|entry|format!("{entry:?}")).collect::<Vec<_>>(),"outputs":format!("{:?}",game.renderer.pending)})
    }
    #[test]
    fn physical_music_handoff_keeps_real_entity_generation_and_decoder_cursor_under_its_exact_new_logical_parent(){
        let mut app=crate::checkpoint_sources::source_app_for_test(game("scene label P\nmusic.play(\"one.wav\")\n\"first\"\nmusic.play(\"two.wav\") with fade(1000)\n\"second\""));
        let world=app.world_mut();publish(world);
        let first=rvn_core::audio_checkpoint::music_views(&world.resource::<VnEngine>().0.state,0).unwrap().remove(0);
        let entity=crate::checkpoint_sources::music_entity_for_test(world,&first.authority.target).unwrap();
        let handle=world.entity(entity).get::<Handle<CheckpointPcm>>().unwrap();let reader=world.resource::<Assets<CheckpointPcm>>().get(handle).unwrap().clone();
        let mut decoder=reader.decoder();assert_eq!([decoder.next(),decoder.next(),decoder.next()],[Some(100),Some(200),Some(300)]);
        let before=frozen(world);let first_image=image(world,1);assert_eq!(frozen(world),before);
        let key=InstanceKey::RenderedEntity{entity:entity.to_bits()};let observed=first_image.selected(&first_image.frame,&key).unwrap();
        assert!(matches!(&observed.parent,Some(InstanceKey::Target{identity,..})if identity==&first.authority.target));
        assert_eq!(observed.details["checkpoint"]["next_sample"],3);assert_eq!(observed.details["decoder_parked"],false);
        {let mut game=world.resource_mut::<VnEngine>();game.0.advance_dialogue().unwrap();game.0.step_until_interaction().unwrap();}publish(world);
        let views=rvn_core::audio_checkpoint::music_views(&world.resource::<VnEngine>().0.state,0).unwrap();
        let outgoing=views.iter().find(|view|view.authority.outgoing.is_some()).unwrap();let current=views.iter().find(|view|view.authority.outgoing.is_none()).unwrap();
        assert_eq!(crate::checkpoint_sources::music_entity_for_test(world,&outgoing.authority.target),Some(entity));
        let current_entity=crate::checkpoint_sources::music_entity_for_test(world,&current.authority.target).unwrap();assert_ne!(entity,current_entity);
        let before=frozen(world);let second=image(world,2);assert_eq!(frozen(world),before);let observed=second.selected(&second.frame,&key).unwrap();
        let outgoing_token=outgoing.authority.outgoing.as_ref().unwrap();
        assert!(matches!(&observed.parent,Some(InstanceKey::Task{key,revision,target})if key==&outgoing_token.key&&*revision==outgoing_token.revision&&target==&outgoing.authority.target));
        assert_eq!(observed.details["checkpoint"]["next_sample"],3);assert_eq!(decoder.next(),Some(400));
        let current=second.selected(&second.frame,&InstanceKey::RenderedEntity{entity:current_entity.to_bits()}).unwrap();assert_eq!(current.details["checkpoint"]["next_sample"],0);
    }
    #[test]
    fn physical_audio_refuses_old_entity_from_independent_engine_even_when_targets_and_epoch_match(){
        for command in ["music.play(\"same.wav\")","sfx.play(\"same.wav\")"]{
            let source=format!("scene label P\n{command}\n\"line\"");let mut app=crate::checkpoint_sources::source_app_for_test(game(&source));let world=app.world_mut();publish(world);
            // One-shot publication prepares its exact reader first. The next
            // real audio install creates the physical entity being tested.
            let engine=&world.resource::<VnEngine>().0;
            let ready=crate::scene_audio::prepare(world,&engine.state,engine.runtime_epoch()).unwrap();
            crate::scene_audio::install(world,ready);
            assert!(world.iter_entities().any(|entity|
                (entity.contains::<crate::scene_audio::OwnedAudio>()||entity.contains::<crate::checkpoint_sources::ExactMusicSource>())
                &&entity.contains::<Handle<CheckpointPcm>>()),"the original engine must own an actual decoder entity");
            let old=image(world,1);let replacement=game(&source);assert_eq!(replacement.runtime_epoch(),old.frame.epoch);assert_ne!(replacement.instance_id(),old.frame.instance);
            world.insert_resource(VnEngine(replacement));let before=frozen(world);let scope=scope(world,2);
            let logical=rvn_core::editor_inspection::capture(&world.resource::<VnEngine>().0.state,FrameIdentity{instance:scope.instance,epoch:scope.epoch,sequence:0});
            if let Some(token)=world.iter_entities().find_map(|entity|entity.get::<crate::scene_audio::OwnedAudio>()){
                assert!(logical.has_resource_token(&token.token),"numeric token equality cannot establish Engine lineage");
            }
            assert!(project(world,&logical,&scope).unwrap_err().contains("Foreign or duplicate decoder"));assert_eq!(frozen(world),before);
        }
    }
    #[test]
    fn physical_text_budget_refuses_complete_observation_without_cloning_or_publishing_partial_details(){
        let mut world=World::new();world.insert_resource(VnEngine(game("scene label P\n\"line\"")));
        let entity=world.spawn((DialogueText,Text::from_section("é".repeat(rvn_core::editor_inspection::MAX_INSPECTION_BYTES),TextStyle::default()))).id();
        let before=frozen(&world);let scope=scope(&world,1);let logical=rvn_core::editor_inspection::capture(&world.resource::<VnEngine>().0.state,FrameIdentity{instance:scope.instance,epoch:scope.epoch,sequence:0});
        assert!(project(&world,&logical,&scope).is_err());assert_eq!(frozen(&world),before);
        assert_eq!(world.entity(entity).get::<Text>().unwrap().sections[0].value.len(),2*rvn_core::editor_inspection::MAX_INSPECTION_BYTES);
        assert!(logical.rows.iter().all(|row|!matches!(row.key,InstanceKey::RenderedEntity{..})));
    }
}
