//! PRIVATE staged adapter. No browser/wasm type compilation or installation.
//! Music readers are projections; Core clocks and historical payloads are never captured here.
use rvn_core::scene_resources::{ResourceToken, TargetIdentity, TargetKind};
use rvn_core::scene_tasks::TaskOwner;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MusicRole { Current, Retired }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireTarget {
    kind: TargetKind,
    alias: String,
    // Never send a u64 identity to JavaScript as a Number.
    incarnation: String,
}
impl From<&TargetIdentity> for WireTarget {
    fn from(target: &TargetIdentity) -> Self {
        Self { kind: target.kind, alias: target.alias.clone(), incarnation: target.incarnation.to_string() }
    }
}

#[derive(Clone, Debug)]
enum Authority {
    Current { epoch: u64, target: TargetIdentity, logical_file: String, operations: Vec<ResourceToken> },
    Retired { token: ResourceToken, logical_file: String },
}
fn token_wire(token: &ResourceToken) -> serde_json::Value {
    // TaskOwner is inside the opaque authority JSON string. JavaScript never
    // parses/coerces it; explicit epoch/revision/incarnation are strings as well.
    serde_json::json!({ "epoch": token.task.runtime_epoch.to_string(), "owner": token.task.key.owner,
        "local_id": token.task.key.local_id, "revision": token.task.revision.to_string(), "target": WireTarget::from(&token.target) })
}
impl Authority {
    fn wire(&self) -> Result<String, String> {
        let value = match self {
            Self::Current { epoch, target, logical_file, operations } => serde_json::json!({
                "role": "current", "epoch": epoch.to_string(), "target": WireTarget::from(target),
                "file": logical_file, "operations": operations.iter().map(token_wire).collect::<Vec<_>>() }),
            Self::Retired { token, logical_file } => serde_json::json!({
                "role": "retired", "token": token_wire(token), "file": logical_file }),
        };
        serde_json::to_string(&value).map_err(|error| error.to_string())
    }
    fn current(&self, state: &rvn_core::GameState, epoch: u64) -> bool {
        let Some(resources) = state.audio_resource_registry() else { return false; };
        match self {
            Self::Current { epoch: expected, target, logical_file, operations } => {
                *expected == epoch && state.music.current_file.as_ref() == Some(logical_file)
                    && resources.target(&TaskOwner::Game, TargetKind::Music, "music") == Some(target)
                    && operations.iter().all(|token| resources.tasks.iter().any(|task|
                        task.key == token.task.key && task.revision == token.task.revision && task.target == token.target)
                        && token.task.runtime_epoch == epoch)
            }
            Self::Retired { token, logical_file } => token.task.runtime_epoch == epoch
                && resources.tasks.iter().any(|task| task.key == token.task.key
                    && task.revision == token.task.revision && task.target == token.target
                    && task.effect.as_ref().and_then(|effect| effect.presentation.as_ref())
                        .is_some_and(|payload| matches!(&payload.model,
                            rvn_core::retired_presentations::RetiredModel::Music { file, .. } if file == logical_file))),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct HandoffWitness {
    epoch: String, role: MusicRole, target: WireTarget, file: String, authority: String,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct WebMusicView {
    epoch: String, role: MusicRole, target: WireTarget, file: String,
    authority: String, gain: f32, parked: bool, enabled: bool, handoff: Option<HandoffWitness>,
    #[serde(skip)] exact: Authority,
}
impl WebMusicView {
    fn key(&self) -> Result<String, String> {
        serde_json::to_string(&(&self.epoch, self.role, self.target.kind, &self.target.alias, &self.target.incarnation))
            .map_err(|error| error.to_string())
    }
    fn witness(&self) -> HandoffWitness {
        HandoffWitness { epoch: self.epoch.clone(), role: self.role, target: self.target.clone(),
            file: self.file.clone(), authority: self.authority.clone() }
    }
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct WebMusicSnapshot { epoch: String, serial: String, modern: bool, views: Vec<WebMusicView> }

/// Actual checkpointed Source owns modern playback on native and Web. Clearing
/// the former indexed bridge avoids a second decoder with a different cursor.
/// Legacy singleton playback and the historical pure descriptor builder stay
/// available, but cannot silently stand in for a measured modern PCM source.
fn checkpoint_bridge_reset(state:&rvn_core::GameState,epoch:u64,serial:u64)->Option<WebMusicSnapshot> {
    state.audio_resource_registry().map(|_|WebMusicSnapshot {epoch:epoch.to_string(),serial:serial.to_string(),modern:true,views:Vec::new()})
}

/// Pure builder. resolve/mix/enabled are caller policies: no AssetServer load,
/// Settings mutation, JS, entity, timer, renderer publication or Core mutation.
pub(crate) fn build_snapshot(
    state: &rvn_core::GameState, epoch: u64, serial: u64, previous: Option<&WebMusicSnapshot>,
    resolve: impl Fn(&str) -> String, mix: impl Fn(f32) -> Result<f32, String>,
    enabled: impl Fn(&TaskOwner) -> bool,
) -> Result<WebMusicSnapshot, String> {
    let mut views = Vec::new();
    let Some(resources) = state.audio_resource_registry() else {
        // Mode transition clears modern readers. Legacy never gets a invented target.
        return Ok(WebMusicSnapshot { epoch: epoch.to_string(), serial: serial.to_string(), modern: false, views });
    };
    rvn_core::scene_resources::validate_current_resource_state(state)?;
    let checked_gain = |logical| -> Result<f32, String> {
        let gain = mix(logical)?;
        if !gain.is_finite() || !(0.0..=1.0).contains(&gain) { return Err("Invalid final browser music gain".into()); }
        Ok(gain)
    };
    if let Some(file) = &state.music.current_file {
        let target = resources.target(&TaskOwner::Game, TargetKind::Music, "music")
            .ok_or("Current browser music has no exact Game target")?.clone();
        let logical = crate::systems::audio::current_music_gain_from_state(state, epoch)?;
        let operations = resources.active_effects(state.scene_execution.as_ref().and_then(|execution|execution.owner())).into_iter()
            .filter(|task| task.target == target)
            .map(|task| ResourceToken { task: rvn_core::scene_tasks::TaskToken {
                runtime_epoch: epoch, key: task.key.clone(), revision: task.revision }, target: task.target.clone() })
            .collect();
        let exact = Authority::Current { epoch, target: target.clone(), logical_file: file.clone(), operations };
        views.push(WebMusicView { epoch: epoch.to_string(), role: MusicRole::Current, target: (&target).into(),
            file: resolve(file), authority: exact.wire()?, gain: checked_gain(logical)?, parked: false,
            enabled: enabled(&TaskOwner::Game), handoff: None, exact });
    }
    let previous_current = previous.filter(|snapshot| snapshot.epoch == epoch.to_string())
        .and_then(|snapshot| snapshot.views.iter().find(|view| view.role == MusicRole::Current));
    let mut handoff_claimed = false;
    for task in &resources.tasks {
        if task.target.kind != TargetKind::PresentationRetired { continue; }
        let effect = task.effect.as_ref().ok_or("Retired browser music task has no effect")?;
        let payload = effect.presentation.as_ref().ok_or("Retired browser music task has no payload")?;
        let rvn_core::retired_presentations::RetiredModel::Music { file, .. } = &payload.model else { continue; };
        payload.validate_outgoing(effect)?;
        payload.validate_for_target(&task.target, resources.next_incarnation)?;
        let gain = checked_gain(payload.outgoing_music_gain(effect)?.ok_or("Retired Music had no gain")?)?;
        let token = ResourceToken { task: rvn_core::scene_tasks::TaskToken {
            runtime_epoch: epoch, key: task.key.clone(), revision: task.revision }, target: task.target.clone() };
        let exact = Authority::Retired { token, logical_file: file.clone() };
        let resolved = resolve(file);
        let retained_current = resources.target(&TaskOwner::Game, TargetKind::Music, "music")
            == Some(&payload.original_target.identity);
        let handoff = previous_current.filter(|old| !handoff_claimed && !retained_current
            && old.file == resolved && old.target == WireTarget::from(&payload.original_target.identity)
            && payload.original_target.owner == TaskOwner::Game).map(WebMusicView::witness);
        handoff_claimed |= handoff.is_some();
        views.push(WebMusicView { epoch: epoch.to_string(), role: MusicRole::Retired, target: (&task.target).into(),
            file: resolved, authority: exact.wire()?, gain, parked: task.parked,
            enabled: enabled(&task.key.owner), handoff, exact });
    }
    if views.len() > rvn_core::scene_resources::MAX_RESOURCES + 1 { return Err("Too many indexed music readers".into()); }
    Ok(WebMusicSnapshot { epoch: epoch.to_string(), serial: serial.to_string(), modern: true, views })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NoticeKind { Blocked, Error }
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MusicNotice {
    report_id: String, kind: NoticeKind, message: String, key: String, epoch: String,
    role: MusicRole, target: WireTarget, authority: String, nonce: String,
}
pub(crate) fn validate_notices(
    snapshot: &WebMusicSnapshot, state: &rvn_core::GameState, epoch: u64, json: &str,
) -> Result<Vec<MusicNotice>, String> {
    let notices: Vec<MusicNotice> = serde_json::from_str(json).map_err(|error| error.to_string())?;
    if notices.len() > snapshot.views.len() { return Err("Browser music notice capacity exceeded".into()); }
    let mut keys = std::collections::HashSet::new();
    let mut accepted = Vec::new();
    for notice in notices {
        if !keys.insert(notice.key.clone()) || notice.message.len() > 16000 || notice.nonce.is_empty()
            || notice.report_id.is_empty() { return Err("Invalid bounded music notice".into()); }
        let Some(view) = snapshot.views.iter().find(|view| view.key().as_ref() == Ok(&notice.key)) else { continue; };
        if notice.epoch != view.epoch || notice.role != view.role || notice.target != view.target
            || notice.authority != view.authority || !view.exact.current(state, epoch) { continue; }
        accepted.push(notice);
    }
    Ok(accepted)
}

// Exact JS factory + browser exports are generated into a separate private
// inline-JS fragment from music_bridge.prepared.mjs, never edited independently.
#[cfg(target_arch = "wasm32")]
#[path = "web_music_inline.rs"]
mod browser;

#[cfg(target_arch = "wasm32")]
#[derive(bevy::prelude::Resource, Default)]
pub(crate) struct WebMusicRuntime { serial: u64, installed: Option<WebMusicSnapshot> }

#[cfg(target_arch = "wasm32")]
pub(crate) fn prepare(
    world: &bevy::prelude::World, state: &rvn_core::GameState, epoch: u64,
    enabled: impl Fn(&TaskOwner) -> bool,
) -> Result<WebMusicSnapshot, String> {
    let runtime = world.get_resource::<WebMusicRuntime>();
    let serial = runtime.map_or(Some(1), |runtime| runtime.serial.checked_add(1))
        .ok_or("Browser music publication serial exhausted")?;
    let registry = world.get_resource::<crate::resources::MusicAssetRegistry>();
    let snapshot = if let Some(reset)=checkpoint_bridge_reset(state,epoch,serial) {reset} else {
        build_snapshot(state, epoch, serial, runtime.and_then(|runtime| runtime.installed.as_ref()),
            |file| registry.map_or_else(|| file.to_string(), |registry| registry.resolve(file)),
            |logical| crate::music_mix::in_world(world, logical), enabled)?
    };
    let json = serde_json::to_string(&snapshot).map_err(|error| error.to_string())?;
    let errors = browser::errors_for_plan(&json)?;
    if let Some(error) = validate_notices(&snapshot, state, epoch, &errors)?.into_iter().find(|notice| notice.kind == NoticeKind::Error) {
        return Err(format!("Browser music resource {:?}: {}", error.target, error.message));
    }
    Ok(snapshot)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn install(world: &mut bevy::prelude::World, mut snapshot: WebMusicSnapshot) -> Result<(), String> {
    // The caller runs prepare before taking its pending Core publication.
    // The fallible browser boundary is not presented as multi-element DOM atomicity.
    // Installation precedes pending take. Clock focus/menu/pending policy
    // never pauses an admitted reader. The later after-HostPublicationSet
    // refresh samples the newly published gains; exact target, token, file
    // and parked residence remain the preflighted candidate values.
    for view in &mut snapshot.views {
        let owner = match &view.exact { Authority::Current { .. } => TaskOwner::Game,
            Authority::Retired { token, .. } => token.task.key.owner.clone() };
        view.enabled = enabled(world, &owner);
    }
    let json = serde_json::to_string(&snapshot).map_err(|error| error.to_string())?;
    browser::apply(&json)?;
    // Only authoritative consumer failure controls the common playback
    // barrier. Clock focus/menu/pending policy never pauses a live reader.
    browser::suspend(!playback_allowed(world))?;
    let serial = snapshot.serial.parse::<u64>().map_err(|error| error.to_string())?;
    let mut runtime = world.get_resource_or_insert_with(WebMusicRuntime::default);
    runtime.serial = serial; runtime.installed = Some(snapshot); Ok(())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn unlock() -> Result<(), String> { browser::unlock() }

#[cfg(target_arch = "wasm32")]
pub(crate) fn drain(world: &mut bevy::prelude::World) -> Result<Vec<MusicNotice>, String> {
    let Some(engine) = world.get_resource::<crate::resources::VnEngine>() else { return Ok(Vec::new()); };
    // During a pending publication, prepare remaps a persistent same-reader error
    // to the candidate token. Do not acknowledge a now-stale installed authority.
    if engine.0.renderer.scene_batch_pending() { return Ok(Vec::new()); }
    let Some(snapshot) = world.get_resource::<WebMusicRuntime>().and_then(|runtime| runtime.installed.as_ref()) else { return Ok(Vec::new()); };
    let json = browser::reports()?;
    let notices = validate_notices(snapshot, &engine.0.state, engine.0.runtime_epoch(), &json)?;
    // Caller diagnoses errors/blocked status. Only then explicitly acknowledges.
    Ok(notices)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn acknowledge(notice: &MusicNotice) -> Result<(), String> {
    let wire = serde_json::json!({ "key": notice.key, "epoch": notice.epoch, "authority": notice.authority,
        "nonce": notice.nonce, "report_id": notice.report_id });
    browser::ack(&wire.to_string())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn reset_engine_instance(world: &mut bevy::prelude::World) -> Result<(), String> {
    // Mandatory before replacing a VnEngine by an independently constructed
    // Engine, whose real runtime_epoch can validly start again at zero.
    browser::reset()?;
    world.insert_resource(WebMusicRuntime::default()); Ok(())
}

// Playback admission is distinct from an Effect/Canvas clock. Current's
// target is Game even when its temporary fade task belongs to Scene/Host.
// Retired's exact lifetime is validated by build_snapshot; only its parked
// residence pauses the reader. A common real failure stops all readers.
pub(crate) fn playback_allowed(world: &bevy::prelude::World) -> bool {
    world.get_resource::<crate::resources::VnEngine>().is_some_and(|engine|
        engine.0.state.audio_resource_registry().is_some() && !engine.0.renderer.scene_frame_failed)
}
#[cfg(target_arch = "wasm32")]
pub(crate) fn enabled(world: &bevy::prelude::World, _owner: &TaskOwner) -> bool {
    playback_allowed(world)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn failure(world: &mut bevy::prelude::World, message: String) {
    use bevy::prelude::*;
    if let Some(mut engine) = world.get_resource_mut::<crate::resources::VnEngine>() {
        engine.0.renderer.scene_frame_failed = true;
    }
    if let Some(mut diagnostic) = world.get_resource_mut::<crate::resources::ScriptErrorMessage>() { diagnostic.0 = message; }
    if let Some(mut next) = world.get_resource_mut::<NextState<crate::resources::VnState>>() { next.set(crate::resources::VnState::Error); }
    // Actual source error stays stored in the reader; stopping siblings does not
    // acknowledge/remove it, advance their clocks or reinterpret them as cancelled.
    let _ = browser::suspend(true);
}

#[cfg(target_arch = "wasm32")]
fn receive(world: &mut bevy::prelude::World) {
    match drain(world) {
        Ok(notices) => for notice in notices {
            match notice.kind {
                NoticeKind::Error => { failure(world, format!("Browser music {:?}: {}", notice.target, notice.message)); },
                // Policy block is distinct from decode/source failure. Existing
                // gesture unlock retries only the indexed pending readers.
                NoticeKind::Blocked => bevy::log::info!("Browser music {:?} awaits a gesture", notice.target),
            }
            if let Err(error) = acknowledge(&notice) { failure(world, error); }
        },
        Err(error) => failure(world, error),
    }
}

#[cfg(target_arch = "wasm32")]
fn reconcile(world: &mut bevy::prelude::World) {
    let Some(engine) = world.get_resource::<crate::resources::VnEngine>() else { return; };
    if engine.0.renderer.scene_batch_pending() { return; }
    if engine.0.renderer.scene_frame_failed { let _ = browser::suspend(true); return; }
    let state = engine.0.state.clone(); let epoch = engine.0.runtime_epoch();
    let snapshot = match prepare(world, &state, epoch, |owner| enabled(world, owner)) {
        Ok(snapshot) => snapshot,
        Err(error) => { failure(world, error); return; }
    };
    if let Err(error) = install(world, snapshot) { failure(world, error); return; }
    receive(world);
}

pub(crate) struct WebMusicPlugin;
impl bevy::prelude::Plugin for WebMusicPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        #[cfg(target_arch = "wasm32")]
        {
            use bevy::prelude::*;
            app.init_resource::<WebMusicRuntime>().add_systems(PreStartup, |world: &mut World| {
                if let Err(error) = reset_engine_instance(world) { failure(world, error); }
            }).add_systems(First, receive.before(crate::scene_publication::consume))
            .add_systems(Update, reconcile
                .after(crate::systems::audio_system).after(crate::scene_retired::reconcile)
                .after(crate::scene_publication::GameFrameSet)
                .after(crate::scene_publication::HostPublicationSet)
                .before(crate::programmable_ui::InterfaceInputSet).before(crate::systems::player_input_system));
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = app;
    }
}

// PRIVATE child fragment: append to web_music.prepared.rs after the browser glue.
// Seven real Core/typed-authority contracts. NOT_RUN until collective type compilation.
#[cfg(test)]
mod web_music_builder_contracts {
    use super::*;
    fn game(source: &str) -> rvn_core::Engine<crate::bevy_renderer::BevyRenderer> {
        let mut game = rvn_core::Engine::from_program(rvn_parser::parse_program(source).unwrap(),
            crate::bevy_renderer::BevyRenderer::new(), 32).unwrap();
        game.step_until_interaction().unwrap(); game
    }
    fn snapshot(game: &rvn_core::Engine<crate::bevy_renderer::BevyRenderer>, serial: u64,
        previous: Option<&WebMusicSnapshot>) -> WebMusicSnapshot {
        build_snapshot(&game.state, game.runtime_epoch(), serial, previous, str::to_owned,
            |gain| Ok(gain * 0.5), |_| true).unwrap()
    }
    const SOURCE: &str = "scene label A\nmusic.volume(0.4)\nmusic.play(\"old.ogg\")\n\"before\"\nmusic.play(\"new.ogg\") with fade(4000)\n\"after\"\nscene label B\n\"child\"\nreturn\n";
    fn replace(game: &mut rvn_core::Engine<crate::bevy_renderer::BevyRenderer>) {
        game.advance_dialogue().unwrap(); game.step_until_interaction().unwrap();
    }
    #[test]
    fn web_builder_has_real_current_identity_without_a_fabricated_task_token_and_mixes_once() {
        let game = game(SOURCE); let before = serde_json::to_value(&game.state).unwrap(); let plan = snapshot(&game, 1, None);
        assert_eq!(plan.views.len(), 1); let current = &plan.views[0]; assert_eq!(current.gain, 0.2);
        assert_eq!(current.role, MusicRole::Current); assert_eq!(current.target.kind, TargetKind::Music);
        let authority: serde_json::Value = serde_json::from_str(&current.authority).unwrap();
        assert_eq!(authority["operations"], serde_json::json!([]));
        assert!(current.exact.current(&game.state, game.runtime_epoch()));
        assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
    }
    #[test]
    fn web_builder_replacement_uses_actual_historical_identity_and_core_absolute_samples() {
        let mut game = game(SOURCE); let old = snapshot(&game, 1, None); replace(&mut game);
        game.tick_game_frame(1.0, rvn_core::GameFrameReports::default()).unwrap();
        let before = serde_json::to_value(&game.state).unwrap(); let plan = snapshot(&game, 2, Some(&old));
        assert_eq!(plan.views.len(), 2); let current = plan.views.iter().find(|view| view.role == MusicRole::Current).unwrap();
        let outgoing = plan.views.iter().find(|view| view.role == MusicRole::Retired).unwrap();
        assert_eq!(current.gain, (f64::from(0.4f32) * 0.25) as f32 * 0.5);
        assert_eq!(outgoing.gain, (f64::from(0.4f32) * 0.75) as f32 * 0.5);
        assert_eq!(outgoing.handoff.as_ref().unwrap().target, old.views[0].target);
        assert_ne!(current.target, old.views[0].target); assert!(outgoing.exact.current(&game.state, game.runtime_epoch()));
        assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
    }
    #[test]
    fn web_builder_park_return_retains_retired_identity_and_renews_actual_token_without_clock_replay() {
        // Timer Call resumes at the next statement after the displayed Dialogue.
        // Keep a parent Dialogue there rather than falling into the child entry.
        let source = SOURCE.replace("\"after\"\nscene label B", "\"after\"\n\"returned\"\nscene label B");
        let mut game = game(&source); let old = snapshot(&game, 1, None); replace(&mut game);
        game.tick_game_frame(1.0, rvn_core::GameFrameReports::default()).unwrap(); let before = snapshot(&game, 2, Some(&old));
        let parent_owner = game.state.scene_execution.as_ref().unwrap().owner().unwrap();
        let retired_before = before.views.iter().find(|view| view.role == MusicRole::Retired).unwrap();
        game.execute_timer_action("call B").unwrap(); let parked = snapshot(&game, 3, Some(&before));
        let retired_parked = parked.views.iter().find(|view| view.role == MusicRole::Retired).unwrap();
        assert!(retired_parked.parked); assert_eq!(retired_parked.target, retired_before.target);
        assert_eq!(retired_parked.gain, retired_before.gain);
        game.step_until_interaction().unwrap(); game.advance_dialogue().unwrap(); game.step_until_interaction().unwrap();
        assert!(matches!(game.current_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "returned"));
        assert_eq!(game.state.scene_execution.as_ref().unwrap().owner(), Some(parent_owner));
        let returned = snapshot(&game, 4, Some(&parked)); let retired = returned.views.iter().find(|view| view.role == MusicRole::Retired).unwrap();
        assert!(!retired.parked); assert_eq!(retired.target, retired_before.target); assert_eq!(retired.gain, retired_before.gain);
        assert_ne!(retired.authority, retired_parked.authority); assert!(retired.exact.current(&game.state, game.runtime_epoch()));
    }
    #[test]
    fn web_builder_new_epoch_cannot_publish_an_old_current_handoff_witness() {
        let mut game = game(SOURCE); let before = snapshot(&game, 1, None); replace(&mut game);
        let mut app=crate::checkpoint_sources::source_app_for_test(game);
        crate::checkpoint_sources::prime_assets_for_test(app.world_mut()).unwrap();
        crate::scene_publication::consume(app.world_mut());
        let renderer=&app.world().resource::<crate::resources::VnEngine>().0.renderer;
        assert!(!renderer.scene_batch_pending() && !renderer.scene_frame_failed);
        let captured=app.world().resource::<crate::resources::VnEngine>().0.snapshot_for_save().unwrap();
        let save=rvn_core::save::SaveData::try_from_snapshot(&captured,1,"hold".into(),"main.rvn".into()).unwrap();
        let save=serde_json::from_value(serde_json::to_value(&save).unwrap()).unwrap();
        assert!(crate::checkpoint_sources::warm_restore(app.world_mut(),&save).unwrap());
        let mut game=app.world_mut().remove_resource::<crate::resources::VnEngine>().unwrap().0;
        game.load_data(save).unwrap();
        let new = snapshot(&game, 2, Some(&before)); assert_ne!(new.epoch, before.epoch);
        assert!(new.views.iter().all(|view| view.handoff.is_none()));
        assert!(!before.views[0].exact.current(&game.state, game.runtime_epoch()));
    }
    #[test]
    fn web_builder_rejects_invalid_master_gain_without_changing_state_history_or_publication() {
        let game = game(SOURCE); let before = serde_json::to_value(&game.state).unwrap();
        let history = game.history.entries().iter().map(|entry|
            serde_json::to_value((&entry.state, &entry.display)).unwrap()).collect::<Vec<_>>();
        let pending = game.renderer.scene_batch_pending();
        let result = build_snapshot(&game.state, game.runtime_epoch(), 1, None, str::to_owned, |_| Ok(f32::NAN), |_| true);
        assert!(result.is_err()); assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
        assert_eq!(game.history.entries().iter().map(|entry|
            serde_json::to_value((&entry.state, &entry.display)).unwrap()).collect::<Vec<_>>(), history);
        assert_eq!(game.renderer.scene_batch_pending(), pending);
    }
    #[test]
    fn web_notice_requires_latest_real_authority_epoch_target_and_never_mutates_core() {
        let mut game = game(SOURCE); let old = snapshot(&game, 1, None); replace(&mut game); let plan = snapshot(&game, 2, Some(&old));
        let outgoing = plan.views.iter().find(|view| view.role == MusicRole::Retired).unwrap();
        let raw = serde_json::json!([{ "report_id": "1", "kind": "error", "message": "decoder",
            "key": outgoing.key().unwrap(), "epoch": outgoing.epoch, "role": outgoing.role,
            "target": outgoing.target, "authority": outgoing.authority, "nonce": "2" }]);
        let before = serde_json::to_value(&game.state).unwrap();
        assert_eq!(validate_notices(&plan, &game.state, game.runtime_epoch(), &raw.to_string()).unwrap().len(), 1);
        let mut wrong = raw.clone(); wrong[0]["authority"] = "old revision".into();
        assert!(validate_notices(&plan, &game.state, game.runtime_epoch(), &wrong.to_string()).unwrap().is_empty());
        let mut wrong = raw.clone(); wrong[0]["target"]["incarnation"] = "999".into();
        assert!(validate_notices(&plan, &game.state, game.runtime_epoch(), &wrong.to_string()).unwrap().is_empty());
        assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
    }
    #[test]
    fn web_builder_legacy_clear_does_not_invent_identity_or_adopt_a_singleton() {
        let game = game("label A\nmusic.play(\"old.ogg\")\n\"hold\"\n");
        assert!(game.state.scene_execution.is_none()); let plan = snapshot(&game, 1, None);
        assert!(plan.views.is_empty());
    }
}

// PRIVATE new child appended after the seven unchanged Web builder tests.
#[cfg(test)]
mod web_music_playback_admission_contracts {
    use super::*;
    #[test]
    fn web_playback_admission_ignores_menu_focus_access_and_pending_clock_guards_but_honors_failure() {
        use bevy::prelude::*;
        let mut game = rvn_core::Engine::from_program(rvn_parser::parse_program(
            "scene label A\nmusic.volume(0.4)\nmusic.play(\"theme.ogg\")\n\"hold\"\n").unwrap(),
            crate::bevy_renderer::BevyRenderer::new(), 32).unwrap();
        game.step_until_interaction().unwrap();
        let before = serde_json::to_value(&game.state).unwrap();
        let history = game.history.entries().iter().map(|entry|
            serde_json::to_value((&entry.state, &entry.display)).unwrap()).collect::<Vec<_>>();
        let epoch = game.runtime_epoch();
        let mut world = World::new(); world.insert_resource(crate::resources::VnEngine(game));
        world.insert_resource(State::new(crate::resources::VnState::Menu));
        world.init_resource::<crate::accessibility::Accessibility>();
        world.resource_mut::<crate::accessibility::Accessibility>().blocked = true;
        world.spawn((Window { focused: false, ..default() }, bevy::window::PrimaryWindow));
        assert!(world.resource::<crate::resources::VnEngine>().0.renderer.scene_batch_pending());
        assert!(!crate::scene_publication::game_active(&world));
        assert!(!crate::scene_publication::host_active(&world));
        assert!(playback_allowed(&world));
        let state = &world.resource::<crate::resources::VnEngine>().0.state;
        let admitted = build_snapshot(state, epoch, 1, None, str::to_owned,
            Ok::<f32, String>, |_| playback_allowed(&world)).unwrap();
        assert!(admitted.views.iter().all(|view| view.enabled && !view.parked));
        world.resource_mut::<crate::resources::VnEngine>().0.renderer.scene_frame_failed = true;
        assert!(!playback_allowed(&world));
        let game = &world.resource::<crate::resources::VnEngine>().0;
        let refused = build_snapshot(&game.state, epoch, 2, Some(&admitted), str::to_owned,
            Ok::<f32, String>, |_| playback_allowed(&world)).unwrap();
        assert_eq!(refused.views.len(), 1); assert!(!refused.views[0].enabled);
        assert_eq!(refused.views[0].target, admitted.views[0].target);
        assert_eq!(serde_json::to_value(&game.state).unwrap(), before);
        assert_eq!(game.history.entries().iter().map(|entry|
            serde_json::to_value((&entry.state, &entry.display)).unwrap()).collect::<Vec<_>>(), history);
        assert_eq!(game.runtime_epoch(), epoch); assert!(game.renderer.scene_batch_pending());
    }
}

#[cfg(test)]
mod checkpoint_bridge_tests {
    use super::*;
    #[test]
    fn actual_modern_checkpoint_bridge_never_projects_a_second_current_or_retired_html_player() {
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program("scene label P\nmusic.play(\"old.wav\")\n\"first\"\nmusic.play(\"new.wav\") with fade(1000)\n\"second\"").unwrap(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();
        game.step_until_interaction().unwrap();game.advance_dialogue().unwrap();game.step_until_interaction().unwrap();
        let pure=build_snapshot(&game.state,game.runtime_epoch(),1,None,str::to_owned,Ok,|_|true).unwrap();
        assert_eq!(pure.views.len(),2);assert!(pure.views.iter().any(|view|view.role==MusicRole::Current));assert!(pure.views.iter().any(|view|view.role==MusicRole::Retired));
        let actual=checkpoint_bridge_reset(&game.state,game.runtime_epoch(),1).unwrap();
        assert!(actual.modern);assert!(actual.views.is_empty());assert_eq!(actual.epoch,pure.epoch);
        assert_eq!(actual.serial,pure.serial);assert!(serde_json::to_string(&actual).unwrap().contains("\"views\":[]"));
        let legacy=rvn_core::Engine::new(rvn_parser::parse("label P\nmusic.play(\"legacy.wav\")\n\"line\"").unwrap(),crate::bevy_renderer::BevyRenderer::new(),8).unwrap();
        assert!(checkpoint_bridge_reset(&legacy.state,0,2).is_none());
    }
}
