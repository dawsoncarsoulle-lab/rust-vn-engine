//! Private C11 Engine contracts. NOT_RUN, no native/rendering claim.
use rvn_core::renderer::SceneRenderBatch;
use rvn_core::retired_presentations::{PresentationSlot, RetiredModel};
use rvn_core::scene_resources::{
    resource_key, EffectClock, EffectParameters, ResourceTask, ResourceTaskKind, ResourceToken,
    SavedEffect, SpriteEffectKind, TargetKind, VisualTransition,
};
use rvn_core::scene_tasks::TaskOwner;
use rvn_core::{Engine, GameFrameReports, Interaction, Renderer, SpriteState};
use rvn_parser::{Hotspot, Position, Transition};

#[derive(Default)]
struct Host {
    batches: Vec<SceneRenderBatch>,
    reject: bool,
    legacy: Vec<String>,
}
impl Renderer for Host {
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn supports_layered_characters(&self) -> bool {
        true
    }
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(), String> {
        if self.reject {
            Err("C11 test rejected final candidate".into())
        } else {
            Ok(())
        }
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.batches.push(batch);
    }
    fn set_background(&mut self, path: &str, _: &Transition) {
        self.legacy.push(format!("bg:{path}"));
    }
    fn show_sprite(
        &mut self,
        id: &str,
        _: Option<&str>,
        _: &Position,
        _: &Transition,
        _: Option<&SpriteState>,
    ) {
        self.legacy.push(format!("show:{id}"));
    }
    fn hide_sprite(&mut self, id: &str, _: &Transition, _: &SpriteState) {
        self.legacy.push(format!("hide:{id}"));
    }
    fn move_sprite(&mut self, id: &str, _: &Position, _: &Transition, _: &SpriteState) {
        self.legacy.push(format!("move:{id}"));
    }
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {
        0
    }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
        0
    }
    fn music_play(&mut self, file: &str, _: &Transition, _: Option<&str>) {
        self.legacy.push(format!("music:{file}"));
    }
    fn music_stop(&mut self, _: &Transition) {
        self.legacy.push("stop".into());
    }
    fn set_typewriter_config(&mut self, speed_cps: f32) {
        self.legacy.push(format!("typewriter:{speed_cps}"));
    }
    fn show_cinematic(&mut self,id:&str,_:Option<&str>) {self.legacy.push(format!("cg:{id}"));}
    fn hide_cinematic(&mut self,_:Option<&str>) {self.legacy.push("cg-hide".into());}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}
fn engine(source: &str) -> Engine<Host> {
    Engine::from_program(
        rvn_parser::parse_program(source).unwrap(),
        Host::default(),
        32,
    )
    .unwrap()
}
fn dialogue(game: &mut Engine<Host>) -> String {
    match game.step_until_interaction().unwrap().unwrap() {
        Interaction::Dialogue { text, .. } => text,
        other => panic!("Expected dialogue, got {other:?}"),
    }
}
fn at_first(source: &str) -> Engine<Host> {
    let mut game = engine(source);
    assert_eq!(dialogue(&mut game), "before");
    game
}
fn ready(game: &mut Engine<Host>) {
    game.advance_dialogue().unwrap();
    game.renderer.batches.clear();
    game.renderer.legacy.clear();
}
fn command(game: &mut Engine<Host>) {
    ready(game);
    publish_ready_command(game);
}
fn publish_ready_command(game: &mut Engine<Host>) {
    game.step_silent().unwrap();
    assert_eq!(game.renderer.batches.len(), 1);
    assert!(game.renderer.legacy.is_empty());
    let batch = game.renderer.batches.last().unwrap();
    assert_eq!(
        serde_json::to_value(&batch.state).unwrap(),
        serde_json::to_value(&game.state).unwrap()
    );
}
fn scene_owner(game: &Engine<Host>) -> TaskOwner {
    TaskOwner::Scene {
        activation: game
            .state
            .scene_execution
            .as_ref()
            .unwrap()
            .owner()
            .unwrap(),
    }
}
fn target(
    game: &Engine<Host>,
    kind: TargetKind,
    alias: &str,
) -> rvn_core::scene_resources::TargetIdentity {
    game.state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .target(&TaskOwner::Game, kind, alias)
        .unwrap()
        .clone()
}
fn outgoing(game: &Engine<Host>, slot: &PresentationSlot) -> ResourceTask {
    let tasks: Vec<_> = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .filter(|task| {
            task.effect
                .as_ref()
                .and_then(|effect| effect.presentation.as_ref())
                .is_some_and(|payload| payload.original_slot == *slot)
        })
        .cloned()
        .collect();
    assert_eq!(tasks.len(), 1);
    tasks[0].clone()
}
fn incoming(game: &Engine<Host>, kind: TargetKind, alias: &str) -> ResourceTask {
    let current = target(game, kind, alias);
    let tasks: Vec<_> = game
        .state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .tasks
        .iter()
        .filter(|task| {
            task.target == current
                && task.effect.as_ref().is_some_and(|effect| {
                    effect.presentation.is_none()
                        && matches!(
                            effect.parameters,
                            EffectParameters::VisualFade { .. }
                                | EffectParameters::AudioFade { .. }
                        )
                })
        })
        .cloned()
        .collect();
    assert_eq!(tasks.len(), 1);
    tasks[0].clone()
}
fn token(game: &Engine<Host>, task: &ResourceTask) -> ResourceToken {
    game.state
        .scene_execution
        .as_ref()
        .unwrap()
        .resources
        .effect_token(
            game.runtime_epoch(),
            &task.key,
            game.state.scene_execution.as_ref().unwrap().owner(),
        )
        .unwrap()
}
fn frame(game: &mut Engine<Host>, seconds: f64) {
    game.renderer.batches.clear();
    game.tick_game_frame(seconds, GameFrameReports::default())
        .unwrap();
}
fn wire(game: &Engine<Host>) -> serde_json::Value {
    serde_json::json!({"state":&game.state,"history":game.history.entries().iter().map(|entry|(&entry.state,&entry.display)).collect::<Vec<_>>(),
        "runtime_epoch":game.runtime_epoch(),"interface_epoch":game.interface_epoch(),"legacy":&game.renderer.legacy,
        "batches":game.renderer.batches.iter().map(|batch|(&batch.state,&batch.previous,batch.runtime_epoch,batch.interface_epoch,batch.video_epoch)).collect::<Vec<_>>()})
}
fn visual(task: &ResourceTask, from: f32, to: f32, despawn: bool) {
    let effect = task.effect.as_ref().unwrap();
    assert!(!effect.clock.looping);
    match &effect.parameters {
        EffectParameters::VisualFade {
            from: a,
            to: b,
            despawn_on_finish,
            ..
        } => {
            assert_eq!(a.to_bits(), from.to_bits());
            assert_eq!(b.to_bits(), to.to_bits());
            assert_eq!(*despawn_on_finish, despawn);
        }
        other => panic!("Expected visual fade, got {other:?}"),
    }
}

#[test]
fn cg_show_is_one_candidate_current_game_target_and_scene_owned_incoming_without_direct_hook() {
    let mut game=at_first("scene label P\n\"before\"\ncinematic \"image\" with fade\n\"after\"\n");
    ready(&mut game);
    let previous=serde_json::to_value(&game.state).unwrap();
    publish_ready_command(&mut game);
    assert_eq!(game.state.cinematic.current.as_deref(),Some("image"));
    assert_eq!(game.state.cinematic.transition.as_deref(),Some("fade"));
    assert_eq!(serde_json::to_value(&game.renderer.batches[0].previous).unwrap(),previous);
    let current=target(&game,TargetKind::Cinematic,"cinematic");
    let effect=incoming(&game,TargetKind::Cinematic,"cinematic");
    assert_eq!(effect.key.owner,scene_owner(&game));assert_eq!(effect.target,current);
    visual(&effect,0.0,1.0,false);assert!(effect.dependencies.is_empty());
    assert_eq!(effect.effect.unwrap().clock.duration,f64::from(Transition::DEFAULT_FADE_MS)/1000.0);
}

#[test]
fn cg_same_id_replacement_freezes_old_fade_and_revokes_old_token_before_invalid_payload() {
    let mut game=at_first("scene label P\ncinematic \"same\" with fade\n\"before\"\ncinematic \"same\" with dissolve\n\"after\"\n");
    let duration=f64::from(Transition::DEFAULT_FADE_MS)/1000.0;frame(&mut game,duration/4.0);
    let old=target(&game,TargetKind::Cinematic,"cinematic");let old_task=incoming(&game,TargetKind::Cinematic,"cinematic");let old_token=token(&game,&old_task);
    command(&mut game);assert_ne!(target(&game,TargetKind::Cinematic,"cinematic"),old);
    let retired=outgoing(&game,&PresentationSlot::Cinematic);visual(&retired,1.0,0.0,true);
    let payload=retired.effect.as_ref().unwrap().presentation.as_ref().unwrap();
    assert_eq!(payload.original_target.identity,old);assert!(retired.dependencies.is_empty());
    match &payload.model {RetiredModel::Cinematic{id,sample}=>{assert_eq!(id,"same");assert_eq!(sample.fade.as_ref().unwrap().clock.elapsed,duration/4.0);},other=>panic!("{other:?}")}
    let before=wire(&game);let mut bad=rvn_core::scene_resources::TransformSnapshot::default();bad.translation[0]=f32::NAN;
    assert!(!game.capture_scene_effect_base(&old_token,rvn_core::scene_resources::EffectProjectionBase::VisualTransform(bad)).unwrap());
    assert_eq!(wire(&game),before);
}

#[test]
fn cg_hide_repeated_absent_keeps_exact_outgoing_clock_and_terminal_removes_only_that_target() {
    let mut game=at_first("scene label P\ncinematic \"image\"\n\"before\"\ncinematic hide with fade\n\"after\"\ncinematic hide with fade\n\"done\"\n");
    command(&mut game);assert!(game.state.cinematic.current.is_none());
    let duration=f64::from(Transition::DEFAULT_FADE_MS)/1000.0;frame(&mut game,duration/4.0);
    let retired=outgoing(&game,&PresentationSlot::Cinematic);let old_token=token(&game,&retired);
    dialogue(&mut game);command(&mut game);assert_eq!(outgoing(&game,&PresentationSlot::Cinematic),retired);
    frame(&mut game,duration);assert!(!game.complete_scene_effect(&old_token).unwrap());
    assert!(game.state.scene_execution.as_ref().unwrap().resources.targets.iter().all(|record|record.identity!=old_token.target));
    assert!(game.state.cinematic.current.is_none());
}

#[test]
fn cg_renderer_refusal_invalid_reference_and_revision_overflow_are_whole_candidate_neutral() {
    let source="scene label P\ncinematic \"old\"\n\"before\"\ncinematic \"new\" with fade\n\"after\"\n";
    let mut refused=at_first(source);ready(&mut refused);refused.renderer.reject=true;let before=wire(&refused);
    assert!(refused.step_silent().is_err());assert_eq!(wire(&refused),before);
    let mut invalid=at_first("scene label P\n\"before\"\ncinematic \"../escape\" with fade\n\"after\"\n");ready(&mut invalid);let before=wire(&invalid);
    assert!(invalid.step_silent().is_err());assert_eq!(wire(&invalid),before);
    let mut overflow=at_first(source);ready(&mut overflow);overflow.state.scene_execution.as_mut().unwrap().resources.next_incarnation=u64::MAX;let before=wire(&overflow);
    assert!(overflow.step_silent().is_err());assert_eq!(wire(&overflow),before);
}

#[test]
fn cg_scene_fade_parks_and_exact_return_renews_revision_without_replaying_phase() {
    let source="scene label P\ncinematic \"parent\" with fade\n\"before\"\ncall C\n\"returned\"\njump done\nscene label C\n\"child\"\nreturn\nlabel done\n";
    let mut game=at_first(source);let duration=f64::from(Transition::DEFAULT_FADE_MS)/1000.0;frame(&mut game,duration/4.0);
    let old=incoming(&game,TargetKind::Cinematic,"cinematic");let old_token=token(&game,&old);let identity=old.target.clone();
    game.advance_dialogue().unwrap();assert_eq!(dialogue(&mut game),"child");frame(&mut game,duration);
    let parked=game.state.scene_execution.as_ref().unwrap().resources.tasks.iter().find(|task|task.key==old.key).unwrap();
    assert!(parked.parked);assert_eq!(parked.effect.as_ref().unwrap().clock.elapsed,duration/4.0);
    game.advance_dialogue().unwrap();assert_eq!(dialogue(&mut game),"returned");
    let returned=incoming(&game,TargetKind::Cinematic,"cinematic");assert_eq!(returned.target,identity);assert!(returned.revision>old.revision);
    assert_eq!(returned.effect.as_ref().unwrap().clock.elapsed,duration/4.0);
    assert!(!game.complete_scene_effect(&old_token).unwrap());
}

#[test]
fn cg_jump_revokes_scene_exit_but_keeps_actual_init_game_fade_and_current_identity() {
    let mut game=engine("init {cinematic \"game\" with fade}\nscene label P\n\"before\"\njump C\nscene label C\n\"child\"\n");
    game.enter_label("P").unwrap();assert_eq!(dialogue(&mut game),"before");
    let current=target(&game,TargetKind::Cinematic,"cinematic");let init=incoming(&game,TargetKind::Cinematic,"cinematic");assert_eq!(init.key.owner,TaskOwner::Game);
    game.advance_dialogue().unwrap();assert_eq!(dialogue(&mut game),"child");assert_eq!(target(&game,TargetKind::Cinematic,"cinematic"),current);
    let kept=incoming(&game,TargetKind::Cinematic,"cinematic");assert_eq!(kept.key,init.key);frame(&mut game,f64::from(Transition::DEFAULT_FADE_MS)/2000.0);
    assert!(incoming(&game,TargetKind::Cinematic,"cinematic").effect.unwrap().clock.elapsed>0.0);
    let mut scene=at_first("scene label P\ncinematic \"old\"\ncinematic hide with fade\n\"before\"\njump C\nscene label C\n\"child\"\n");let old=outgoing(&scene,&PresentationSlot::Cinematic);let capability=token(&scene,&old);
    scene.advance_dialogue().unwrap();assert_eq!(dialogue(&mut scene),"child");assert!(!scene.complete_scene_effect(&capability).unwrap());
    assert!(scene.state.scene_execution.as_ref().unwrap().resources.tasks.iter().all(|task|task.key!=old.key));
}

#[test]
fn cg_story_replacement_keeps_autonomous_host_retired_and_physical_current_dependencies_are_revoked() {
    let mut game=at_first("scene label P\ncinematic \"old\"\n\"before\"\ncinematic \"new\" with fade\n\"after\"\n");
    let payload=rvn_core::retired_presentations::snapshot_retired_source(&game.state,&PresentationSlot::Cinematic).unwrap().unwrap();
    let effect=SavedEffect{presentation:None,base_resolved:false,clock:EffectClock{duration:10.0,elapsed:0.0,looping:false},parameters:EffectParameters::VisualFade{from:1.0,to:0.0,despawn_on_finish:true,layered:false,transition:VisualTransition::Fade,base:Default::default()}};
    let epoch=game.runtime_epoch();let host=rvn_core::scene_resources::register_retired_effect(&mut game.state,epoch,TaskOwner::Host{role:"settings".into()},payload,effect).unwrap().unwrap();
    let current=target(&game,TargetKind::Cinematic,"cinematic");
    let physical=game.register_scene_effect(resource_key(TaskOwner::Host{role:"settings".into()},ResourceTaskKind::Effect,"cg/physical"),current,SavedEffect{presentation:None,base_resolved:false,clock:EffectClock{duration:10.0,elapsed:0.0,looping:false},parameters:EffectParameters::VisualFade{from:0.0,to:1.0,despawn_on_finish:false,layered:false,transition:VisualTransition::Fade,base:Default::default()}}).unwrap();
    command(&mut game);
    assert!(game.state.scene_execution.as_ref().unwrap().resources.accepts(&host,game.runtime_epoch(),game.state.scene_execution.as_ref().unwrap().owner()));
    assert!(!game.complete_scene_effect(&physical).unwrap());
    assert_eq!(game.state.cinematic.current.as_deref(),Some("new"));
}

#[test]
fn cg_extended_writer_reader_restore_phase_stack_and_fresh_epoch_with_old_reader10_barrier() {
    let source="scene label P\ncinematic \"parent\" with fade\n\"before\"\ncall C\n\"returned\"\njump done\nscene label C\n\"child\"\nreturn\nlabel done\n";
    let mut game=at_first(source);frame(&mut game,f64::from(Transition::DEFAULT_FADE_MS)/4000.0);game.advance_dialogue().unwrap();assert_eq!(dialogue(&mut game),"child");
    let saved=rvn_core::save::SaveData::try_from_state(&game.state,1,"CG".into(),"main.rvn".into()).unwrap();assert_eq!(saved.format_version,11);
    let bytes=serde_json::to_vec(&saved).unwrap();assert!(serde_json::from_slice::<frozen_reader10::Header>(&bytes).is_err());
    let old=incoming_or_parked(&game);let old_epoch=game.runtime_epoch();
    let loaded=serde_json::from_slice(&bytes).unwrap();game.load_data(loaded).unwrap();assert!(game.runtime_epoch()>old_epoch);
    assert_eq!(game.state.call_stack,saved.call_stack);assert_eq!(incoming_or_parked(&game).effect,old.effect);
    game.advance_dialogue().unwrap();assert_eq!(dialogue(&mut game),"returned");let returned=incoming(&game,TargetKind::Cinematic,"cinematic");assert_eq!(returned.effect,old.effect);
    assert_eq!(serde_json::to_vec(&saved).unwrap(),bytes);
}
fn incoming_or_parked(game:&Engine<Host>)->ResourceTask {
    game.state.scene_execution.as_ref().unwrap().resources.tasks.iter().find(|task|task.target.kind==TargetKind::Cinematic).unwrap().clone()
}

#[test]
fn cg_new_reader_loads_old_immediate_scene10_and_refuses_unmeasured_fade_before_all_mutation() {
    for name in [None,Some("instant"),Some("fade"),Some("dissolve")] {
        let mut game=at_first("scene label P\n\"before\"\n");
        let mut old=rvn_core::save::SaveData::try_from_state(&game.state,1,"old".into(),"main.rvn".into()).unwrap();assert_eq!(old.format_version,10);
        old.cinematic.current=Some("historical".into());old.cinematic.transition=name.map(str::to_owned);
        let original=serde_json::to_vec(&old).unwrap();let before=wire(&game);
        if matches!(name,Some("fade"|"dissolve")) {let error=game.load_data(old.clone()).unwrap_err().to_string();assert!(error.contains("trustworthy saved phase"));assert_eq!(wire(&game),before);}
        else {game.load_data(old.clone()).unwrap();assert_eq!(game.state.cinematic.current.as_deref(),Some("historical"));let _=target(&game,TargetKind::Cinematic,"cinematic");assert!(game.state.scene_execution.as_ref().unwrap().resources.tasks.iter().all(|task|task.target.kind!=TargetKind::Cinematic));}
        assert_eq!(serde_json::to_vec(&old).unwrap(),original);
    }
}

#[test]
fn cg_no_extended_story_shape_keeps_scene10_legacy9_and_refuses_header_downgrade() {
    let mut plain=at_first("scene label P\n\"before\"\n");assert_eq!(rvn_core::save::SaveData::try_from_state(&plain.state,1,"plain".into(),"main.rvn".into()).unwrap().format_version,10);
    let mut legacy=Engine::new(rvn_parser::parse("label start\ncinematic \"old\" with fade\n\"before\"\n").unwrap(),Host::default(),16).unwrap();dialogue(&mut legacy);
    assert_eq!(rvn_core::save::SaveData::try_from_state(&legacy.state,1,"legacy".into(),"main.rvn".into()).unwrap().format_version,9);assert_eq!(legacy.renderer.legacy,vec!["cg:old"]);
    let mut extended=at_first("scene label P\ncinematic \"owned\"\n\"before\"\n");let mut saved=rvn_core::save::SaveData::try_from_state(&extended.state,1,"CG".into(),"main.rvn".into()).unwrap();assert_eq!(saved.format_version,11);saved.format_version=10;
    let before=wire(&extended);assert!(extended.load_data(saved).is_err());assert_eq!(wire(&extended),before);
    let old=rvn_core::save::SaveData::try_from_state(&plain.state,1,"plain".into(),"main.rvn".into()).unwrap();plain.load_data(old).unwrap();
}

#[test]
fn cg_invalid_owned_layer_family_and_frozen_payload_are_rejected_before_publication() {
    let mut game=at_first("scene label P\ncinematic \"image\"\n\"before\"\n");let target=target(&game,TargetKind::Cinematic,"cinematic");let before=wire(&game);
    let bad=SavedEffect{presentation:None,base_resolved:false,clock:EffectClock{duration:1.0,elapsed:0.0,looping:false},parameters:EffectParameters::VisualFade{from:0.0,to:1.0,despawn_on_finish:false,layered:true,transition:VisualTransition::Fade,base:Default::default()}};
    assert!(game.register_scene_effect(resource_key(scene_owner(&game),ResourceTaskKind::Effect,"invalid-layered-cg"),target,bad).is_err());assert_eq!(wire(&game),before);
    let mut payload=rvn_core::retired_presentations::snapshot_retired_source(&game.state,&PresentationSlot::Cinematic).unwrap().unwrap();
    if let RetiredModel::Cinematic{sample,..}=&mut payload.model {sample.root_motion=Some(rvn_core::retired_presentations::FrozenMotionSample{pose:Default::default(),reduced_pose:Default::default()});}
    assert!(payload.validate_structure().is_err());assert_eq!(wire(&game),before);
}

#[test]
fn cg_frozen_source_phase_outgoing_factor_and_reduced_sampling_are_independent_of_current() {
    let mut game=at_first("scene label P\ncinematic \"old\" with fade\n\"before\"\ncinematic \"new\" with fade\n\"after\"\n");let duration=f64::from(Transition::DEFAULT_FADE_MS)/1000.0;frame(&mut game,duration/2.0);command(&mut game);
    let task=outgoing(&game,&PresentationSlot::Cinematic);let effect=task.effect.as_ref().unwrap();let payload=effect.presentation.as_ref().unwrap();
    let geometry=rvn_core::retired_sampling::HistoricalGeometry{source:payload.original_target.identity.clone(),root:rvn_core::retired_sampling::NeutralGeometry{transform:rvn_core::scene_resources::TransformSnapshot{translation:[0.0,0.0,50.0],..Default::default()},motion:rvn_core::effect_sampling::MotionGeometry{size:[100.0,200.0],factor:[1.0;2],ui:false}},layers:Default::default()};
    let frozen=rvn_core::retired_sampling::freeze_appearance(payload,Some(&geometry),false).unwrap();let mut sampled_effect=effect.clone();sampled_effect.clock.elapsed=duration/2.0;
    let before=wire(&game);match rvn_core::retired_sampling::sample_outgoing(&frozen,&sampled_effect,false).unwrap() {rvn_core::retired_sampling::RetiredFrame::Visual{root,leaves}=>{assert_eq!(root.translation[2],50.0);assert_eq!(leaves[0].path,"old");assert_eq!(leaves[0].color[3],0.25);},other=>panic!("{other:?}")}
    let reduced=rvn_core::retired_sampling::freeze_appearance(payload,Some(&geometry),true).unwrap();match rvn_core::retired_sampling::sample_outgoing(&reduced,&sampled_effect,true).unwrap() {rvn_core::retired_sampling::RetiredFrame::Visual{leaves,..}=>assert_eq!(leaves[0].color[3],0.0),other=>panic!("{other:?}")}
    assert_eq!(wire(&game),before);assert_eq!(game.state.cinematic.current.as_deref(),Some("new"));
}

#[cfg(not(target_arch="wasm32"))]
#[test]
fn cg_real_save_writers_roundtrip_extended_phase_and_refuse_invalid_state_without_touching_originals() {
    struct Files(std::path::PathBuf);
    impl Drop for Files {fn drop(&mut self) {
        for name in ["slot_01.json","autosave.json","quicksave.json"] {let _=std::fs::remove_file(self.0.join(name));}
        let _=std::fs::remove_dir(&self.0);
    }}
    let directory=std::env::temp_dir().join(format!("rvn-cg-writers-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&directory).unwrap();let files=Files(directory);
    let manager=rvn_core::save::SaveManager::new(&files.0,2).unwrap();
    let mut game=at_first("scene label P\ncinematic \"image\" with fade\n\"before\"\n");frame(&mut game,f64::from(Transition::DEFAULT_FADE_MS)/4000.0);
    manager.save(&game.state,1,"CG".into(),"main.rvn".into()).unwrap();
    manager.save_autosave(&game.state,"CG".into(),"main.rvn".into()).unwrap();
    manager.save_quicksave(&game.state,"CG".into(),"main.rvn".into()).unwrap();
    let names=["slot_01.json","autosave.json","quicksave.json"];
    let original:Vec<_>=names.iter().map(|name|std::fs::read(files.0.join(name)).unwrap()).collect();
    for bytes in &original {assert!(serde_json::from_slice::<frozen_reader10::Header>(bytes).is_err());assert_eq!(serde_json::from_slice::<rvn_core::save::SaveData>(bytes).unwrap().format_version,11);}
    let phase=incoming(&game,TargetKind::Cinematic,"cinematic").effect;
    game.load_data(manager.load(1).unwrap()).unwrap();assert_eq!(incoming(&game,TargetKind::Cinematic,"cinematic").effect,phase);
    let mut invalid=game.state.clone();invalid.cinematic.current=Some("../outside".into());let before=wire(&game);
    assert!(manager.save(&invalid,1,"invalid".into(),"main.rvn".into()).is_err());
    assert!(manager.save_autosave(&invalid,"invalid".into(),"main.rvn".into()).is_err());
    assert!(manager.save_quicksave(&invalid,"invalid".into(),"main.rvn".into()).is_err());
    assert_eq!(names.iter().map(|name|std::fs::read(files.0.join(name)).unwrap()).collect::<Vec<_>>(),original);assert_eq!(wire(&game),before);
}

mod frozen_reader10 {
use serde::Deserialize;
const MAX_SUPPORTED_SAVE_FORMAT_VERSION:u32=10;
fn version<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let version = u32::deserialize(deserializer)?;
    if (1..=MAX_SUPPORTED_SAVE_FORMAT_VERSION).contains(&version) {
        Ok(version)
    } else {
        Err(serde::de::Error::custom(format!(
            "Unsupported save format {version}; supported versions: 1–{MAX_SUPPORTED_SAVE_FORMAT_VERSION}"
        )))
    }
}
#[derive(Deserialize)] pub struct Header { #[serde(deserialize_with="version")] pub format_version:u32 }
}
