use bevy::prelude::*;
use rvn_core::save::SaveManager;
use rvn_parser::Transition;

use crate::project_paths::ProjectPaths;
use crate::resources::{
    PersistentDataResource, ScriptErrorMessage, VnEngine, VnRenderState, VnState,
};
use crate::systems::save_menu::record_resume_target;
use crate::vn_command::VnCommand;
use rvn_core::persistent::LastResumeTarget;
use rvn_core::{error::ScriptError, Interaction};

use crate::systems::debug_overlay::{DebugOverlayState, DebugStepRequest};

pub fn stepping_system(
    mut engine: ResMut<VnEngine>,
    mut next_state: ResMut<NextState<VnState>>,
    mut render_state: ResMut<VnRenderState>,
    mut vn_events: EventWriter<VnCommand>,
    mut error_msg: ResMut<ScriptErrorMessage>,
    debug_state: Res<DebugOverlayState>,
    mut step_req: ResMut<DebugStepRequest>,
    project_paths: Res<ProjectPaths>,
    mut persistent: ResMut<PersistentDataResource>,
    mut thumbnails: ResMut<crate::save_thumbnails::SaveThumbnails>,
    accessibility: Res<crate::accessibility::Accessibility>,
    typing: Res<crate::resources::TypewriterState>,
    binding: Option<Res<crate::display_reveal::DialogueRevealBinding>>,
    mut requests:Option<ResMut<crate::checkpoint_requests::CheckpointRequests>>,
    mut preview:Option<ResMut<crate::editor_preview::PreviewState>>,
) {
    if accessibility.open {
        return;
    }
    // En mode debug, on ne progresse que si un step a été demandé (touche F10).
    if debug_state.visible {
        if !step_req.pending {
            return;
        } else {
            // Consomme la demande de step
            step_req.pending = false;
        }
    }
    if engine.0.is_finished() {
        vn_events.send(VnCommand::ScriptFinished);
        next_state.set(VnState::Finished);
        return;
    }

    if let Err(error)=crate::display_reveal::flush(&mut engine,&typing,binding.as_deref()) {
        engine.0.renderer.scene_frame_failed=true;error_msg.0=error;next_state.set(VnState::Error);return;
    }
    let single_statement=preview.as_deref_mut().is_some_and(|preview|{
        if preview.paused&&preview.step_requested{preview.step_requested=false;true}else{false}
    });
    let result=match crate::editor_debugger::statement(&engine){
        crate::editor_debugger::Poll::Immediate=>if single_statement{engine.0.preview_step_statement()}else{engine.0.step_until_interaction()},
        crate::editor_debugger::Poll::Pending=>return,
        crate::editor_debugger::Poll::Complete(result)=>result.map_err(|message|rvn_core::RuntimeError::no_stmt(rvn_core::error::RuntimeErrorKind::EvalError(rvn_core::error::EvalErrorKind::InvalidFunction(message)),engine.0.state.pc)),
    };
    let interaction = match result {
        Ok(interaction) => interaction,
        Err(e) => {
            let script_err = ScriptError::from_runtime(&e);
            error!("{script_err}");
            eprintln!("\n{script_err}");
            error_msg.0 = script_err.to_string();
            next_state.set(VnState::Error);
            return;
        }
    };

    // A direct Dialogue -> Choice/Imagemap can reach a fresh checkpoint without
    // an intervening model mutation. Publish it through the same preflighted
    // scene path; old pixels keep their old scope until consume accepts it.
    if interaction.is_some() && engine.0.state.scene_execution.is_some()
        && !engine.0.renderer.scene_batch_pending()
    {
        if let Err(error) = engine.0.publish_current_scene_interaction() {
            engine.0.renderer.scene_frame_failed = true;
            error_msg.0 = ScriptError::from_runtime(&error).to_string();
            next_state.set(VnState::Error);
            return;
        }
    }

    let mut pending = if engine.0.renderer.scene_batch_pending() {
        engine.0.renderer.scene_stepped = true;
        std::mem::take(&mut engine.0.renderer.pending)
    } else { engine.0.renderer.take_pending() };
    let wants_to_wait = interaction.is_some();
    let current_background_changed=engine.0.renderer.scene_batch.as_ref().is_some_and(|batch|
        batch.previous.background_image!=batch.state.background_image);
    let should_autosave = current_background_changed || pending
        .iter()
        .any(|cmd| matches!(cmd, VnCommand::SetBackground { .. }))
        || matches!(interaction, Some(Interaction::Choice { .. }));

    if should_autosave {
        match SaveManager::new(
            &project_paths.saves,
            crate::systems::save_menu::SUPPORTED_SLOTS,
        ) {
            Ok(mgr) => {
                let deferred=crate::checkpoint_requests::defer_save(&engine,requests.as_deref_mut(),
                    crate::checkpoint_requests::SaveDestination::Auto,"Autosave".into(),"script.rvn".into(),mgr.load_autosave().ok());
                if matches!(deferred,Ok(true)) {
                    // The producer below still supplies the exact new display.
                } else if let Err(error)=deferred {
                    error!("[autosave] admission refused: {error}");
                } else if let Err(e) = engine.0.save_autosave(&mgr,
                    "Autosave".to_string(),
                    "script.rvn".to_string(),
                ) {
                    error!("[autosave] échec: {e}");
                } else if let Ok(data) = mgr.load_autosave() {
                    record_resume_target(
                        &mut persistent,
                        LastResumeTarget::autosave(data.timestamp),
                    );
                    thumbnails.request_resume(rvn_core::save::ResumeSlot::Auto, &engine, data);
                }
            }
            Err(e) => error!("[autosave] SaveManager indisponible: {e}"),
        }
    }

    if let Some(interaction) = interaction {
        match interaction {
            Interaction::Dialogue { character, text } => {
                pending.push(crate::display_reveal::command(&engine, character, text));
            }
            Interaction::Choice { options } => {
                pending.push(VnCommand::ShowChoice { options });
            }
            Interaction::Imagemap {
                background,
                hover_image,
                hotspots,
            } => {
                let hotspots = hotspots
                    .iter()
                    .map(crate::vn_command::ImagemapZone::from)
                    .collect();
                pending.push(VnCommand::ShowImagemap {
                    background,
                    hover_image,
                    hotspots,
                });
            }
        }
    }

    let has_anim = pending.iter().any(|cmd| {
        matches!(
            cmd,
            VnCommand::SetBackground { transition, .. }
                | VnCommand::ShowSprite { transition, .. }
                | VnCommand::HideSprite { transition, .. }
                | VnCommand::MoveSprite { transition, .. }
            if *transition != Transition::None
        ) || matches!(
            cmd,
            VnCommand::ShowCinematic {
                transition: Some(transition),
                ..
            } | VnCommand::HideCinematic {
                transition: Some(transition)
            } if transition == "fade" || transition == "dissolve"
        )
    });

    for cmd in pending {
        if engine.0.renderer.scene_batch_pending() { engine.0.renderer.pending.push(cmd); }
        else { vn_events.send(cmd); }
    }

    if has_anim {
        render_state.state_after_anim = Some(if wants_to_wait {
            VnState::Waiting
        } else {
            VnState::Stepping
        });
        next_state.set(VnState::Animating);
    } else if wants_to_wait {
        next_state.set(VnState::Waiting);
    } else {
        next_state.set(VnState::Stepping);
    }
}

pub fn script_finished_system(
    mut vn_events: EventReader<VnCommand>,
    mut next_state: ResMut<NextState<VnState>>,
    theme: Res<crate::resources::Theme>,
) {
    for cmd in vn_events.read() {
        if matches!(cmd, VnCommand::ScriptFinished) {
            info!("[moteur] script terminé");
            // The last batch (sprite removal, fades, audio) has already completed.
            // A game with a title screen must remain navigable after its ending.
            next_state.set(if theme.title_screen.enabled {
                VnState::TitleScreen
            } else {
                VnState::Finished
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ending_returns_to_an_enabled_title_screen() {
        for enabled in [false, true] {
            let mut app = App::new();
            let mut theme = crate::resources::Theme::default();
            theme.title_screen.enabled = enabled;
            app.insert_resource(theme)
                .insert_resource(NextState::<VnState>::Unchanged)
                .add_event::<VnCommand>()
                .add_systems(Update, script_finished_system);
            app.world_mut().send_event(VnCommand::ScriptFinished);
            app.update();
            let NextState::Pending(actual) = app.world().resource::<NextState<VnState>>() else {
                panic!("ending did not select a destination");
            };
            assert_eq!(
                *actual,
                if enabled {
                    VnState::TitleScreen
                } else {
                    VnState::Finished
                }
            );
        }
    }
}

#[cfg(test)]
mod preview_step_contract {
    use super::*;
    #[test]
    fn paused_real_stepping_system_executes_one_authored_statement_and_then_waits_for_another_command(){
        let directory=std::env::temp_dir().join(format!("rvn-editor-step-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let manager=rvn_core::PersistentDataManager::new(&directory).unwrap();
        let mut app=App::new();
        let game=rvn_core::Engine::new(rvn_parser::parse("set value = 1\nset value = 2\n\"Boundary\"\n").unwrap(),crate::bevy_renderer::BevyRenderer::new(),16).unwrap();
        let instance=game.instance_id();let random=game.state.random.clone();
        app.insert_resource(VnEngine(game)).insert_resource(State::new(VnState::Stepping)).init_resource::<NextState<VnState>>()
            .init_resource::<VnRenderState>().init_resource::<ScriptErrorMessage>()
            .init_resource::<DebugOverlayState>().init_resource::<DebugStepRequest>()
            .insert_resource(ProjectPaths::new(directory.clone(),directory.join("assets"),directory.join("locales"),directory.join("theme.toml"),directory.join("saves")))
            .insert_resource(PersistentDataResource{manager,data:rvn_core::PersistentData::default()})
            .init_resource::<crate::save_thumbnails::SaveThumbnails>().init_resource::<crate::accessibility::Accessibility>()
            .init_resource::<crate::resources::TypewriterState>().init_resource::<crate::editor_preview::PreviewState>()
            .add_event::<VnCommand>().add_systems(Update,stepping_system.run_if(crate::scene_publication::preview_statement_ready));
        {let mut control=app.world_mut().resource_mut::<crate::editor_preview::PreviewState>();control.paused=true;control.step_requested=true;}
        app.update();
        assert_eq!(app.world().resource::<VnEngine>().0.state.pc,1);
        assert_eq!(app.world().resource::<VnEngine>().0.state.vars["value"],rvn_parser::Value::Int(1));
        assert!(!app.world().resource::<crate::editor_preview::PreviewState>().step_requested);
        for _ in 0..3{app.update();}
        assert_eq!(app.world().resource::<VnEngine>().0.state.pc,1);
        app.world_mut().resource_mut::<crate::editor_preview::PreviewState>().step_requested=true;
        app.update();
        assert_eq!(app.world().resource::<VnEngine>().0.state.pc,2);
        assert_eq!(app.world().resource::<VnEngine>().0.state.vars["value"],rvn_parser::Value::Int(2));
        assert_eq!(app.world().resource::<VnEngine>().0.instance_id(),instance);
        assert_eq!(app.world().resource::<VnEngine>().0.state.random,random);
        assert!(app.world().resource::<Events<VnCommand>>().iter_current_update_events().all(|event|!matches!(event,VnCommand::ShowDialogue{..})));
        drop(app);
        let directory=directory.canonicalize().unwrap();assert_eq!(directory.parent(),Some(std::env::temp_dir().canonicalize().unwrap().as_path()));
        assert!(directory.file_name().unwrap().to_string_lossy().starts_with("rvn-editor-step-"));std::fs::remove_dir_all(directory).unwrap();
    }
}
