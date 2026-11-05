use rvn_core::{GameState, Renderer, SpriteState};
use rvn_parser::{AnimationParam, Hotspot, Position, Transition};

use crate::vn_command::VnCommand;
use std::sync::atomic::{AtomicU64,Ordering};

// Transient process allocation: a fresh Engine or restored GameState never
// reuses an identity issued for an earlier authoritative menu candidate.
static NEXT_MENU_RECEIPT:AtomicU64=AtomicU64::new(1);
/// Opaque preparation owned by the same actual bootstrap renderer.
pub struct PreparedStartupPublication {
    gate:crate::checkpoint_pcm::CheckpointGate,
    source:rvn_core::engine::StartupPublicationSource,
    stamp:crate::source_menus::Stamp,
    first_menu:u64,
    menus:usize,
}
impl PreparedStartupPublication{
    pub(crate) fn accepts_broker(&self,broker:&crate::checkpoint_broker::AudioCheckpointBroker)->bool{self.gate.same_gate(&broker.gate())}
}
/// A complete accepted replacement transferred directly to the real World.
/// Core has already published exactly this state under its exclusive guard.
pub struct StartupPublication {
    pub(crate) source:rvn_core::engine::StartupPublicationSource,
    pub(crate) batch:rvn_core::renderer::SceneRenderBatch,
    pub(crate) commands:Vec<VnCommand>,
    pub(crate) widget_backend:crate::widget_backend_0956::WidgetBackend,
}
impl rvn_core::engine::StartupBatchSink for BevyRenderer {
    type PreparedPublication=PreparedStartupPublication;
    type Publication=StartupPublication;
    fn prepare_startup_publication(&self,source:&rvn_core::engine::StartupPublicationSource,batch:&rvn_core::renderer::SceneRenderBatch)->Result<PreparedStartupPublication,String>{
        if source.runtime_epoch()!=batch.runtime_epoch{return Err("Startup owner/runtime epoch differs from candidate batch".into());}
        if self.scene_batch.is_some()||!self.scene_menus.is_empty()||!self.pending.is_empty()||self.menu_pending{return Err("Startup renderer has an unsettled previous publication".into());}
        self.validate_scene_batch(batch)?;
        let count=u64::try_from(batch.menu_requests.len()).map_err(|_|"Startup menu count exceeds identity range")?;
        // Reserving unused transient IDs publishes no menu effect/receipt. A
        // refusal/drop never advances the actual player or continuation.
        let first_menu=if count==0{0}else{NEXT_MENU_RECEIPT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|value|value.checked_add(count)).map_err(|_|"Menu receipt identities exhausted")?};
        Ok(PreparedStartupPublication{gate:self.audio_checkpoints.gate(),source:source.clone(),stamp:self.menu_stamp.clone(),first_menu,menus:batch.menu_requests.len()})
    }
    fn validate_startup_effects(&self,_source:&rvn_core::engine::StartupPublicationSource,_effects:&rvn_core::engine::StartupHostEffects)->Result<(),String>{Ok(())}
    fn publish_startup_publication_with_effects(&mut self,source:rvn_core::engine::StartupPublicationSource,batch:rvn_core::renderer::SceneRenderBatch,prepared:PreparedStartupPublication,effects:rvn_core::engine::StartupHostEffects)->StartupPublication{
        self.complete_startup_publication(source,batch,prepared,Some(effects))
    }
    fn publish_startup_publication(&mut self,source:rvn_core::engine::StartupPublicationSource,batch:rvn_core::renderer::SceneRenderBatch,prepared:PreparedStartupPublication)->StartupPublication{
        self.complete_startup_publication(source,batch,prepared,None)
    }
}
// Declares this concrete adapter's software capability, not readiness of
// a Window, image or input device. World publication and completed-image
// admission below remain mandatory before a selection can reach Core.
impl rvn_core::engine::StartupInputBatchSink for BevyRenderer {
    fn validate_startup_input_host(&self)->Result<(),String>{
        if self.scene_batch.is_some()||!self.pending.is_empty()||self.menu_pending{
            return Err("Input Startup renderer has an unsettled previous publication".into());
        }
        Ok(())
    }
}
impl BevyRenderer{
    fn complete_startup_publication(&mut self,source:rvn_core::engine::StartupPublicationSource,batch:rvn_core::renderer::SceneRenderBatch,prepared:PreparedStartupPublication,effects:Option<rvn_core::engine::StartupHostEffects>)->StartupPublication{
        // The actual Core guard holds exclusive renderer ownership throughout
        // the producer fence. These identities can never be client-supplied.
        debug_assert_eq!(source,prepared.source);debug_assert_eq!(self.menu_stamp,prepared.stamp);
        debug_assert_eq!(batch.menu_requests.len(),prepared.menus);
        self.publish_scene_batch(batch);
        if let Some(effects)=effects{effects.publish(self);}
        let mut commands=std::mem::take(&mut self.pending);
        for(index,request)in self.scene_menus.drain(..).enumerate(){commands.push(VnCommand::SourceMenu(crate::source_menus::Receipt{identity:prepared.first_menu+index as u64,effect:request.effect,previous:request.previous,stamp:request.stamp,history_previous:request.history_previous}));}
        self.menu_pending=!commands.iter().all(|command|!matches!(command,VnCommand::SourceMenu(_)));
        StartupPublication{source,batch:*self.scene_batch.take().expect("guard published its one actual batch"),commands,widget_backend:self.widget_backend.clone()}
    }
}

pub(crate) struct SceneMenuRequest {
    effect: rvn_ui::source_menus::MenuEffect,
    previous: GameState,
    history_previous: Option<rvn_core::audio_checkpoint::HistoricalGameSnapshot>,
    stamp: crate::source_menus::Stamp,
}

pub struct BevyRenderer {
    pub(crate) widget_backend:crate::widget_backend_0956::WidgetBackend,
    #[cfg(all(feature="video",not(target_arch="wasm32")))]
    pub(crate) native_admission:Option<crate::native_video::NativeAdmission>,
    pub(crate) debugger:std::sync::Arc<std::sync::Mutex<crate::editor_debugger::Mailbox>>,
    pub(crate) audio_checkpoints:crate::checkpoint_broker::AudioCheckpointBroker,
    pub(crate) menu_authority: rvn_ui::source_menus::MenuAuthority,
    pub(crate) menu_stamp: crate::source_menus::Stamp,
    pub(crate) menu_pending: bool,
    pub(crate) menu_can_rollback: bool,
    pub pending: Vec<VnCommand>,
    // A complete replacement is owned by the renderer until the exclusive
    // scene consumer installs it. Ordinary drainers cannot split that batch.
    // The unique pending publication is heap-owned so every Engine transport
    // does not reserve complete previous/current/history states on its stack.
    pub(crate) scene_batch: Option<Box<rvn_core::renderer::SceneRenderBatch>>,
    pub(crate) scene_stepped: bool,
    // Host-only failure latch; never enters GameState or a save. Only a fresh
    // successful runtime publication releases it.
    pub(crate) scene_frame_failed: bool,
    scene_menus: Vec<SceneMenuRequest>,
    scene_epoch: Option<u64>,
    scene_owner: Option<Option<rvn_core::scene::ActivationId>>,
    pub choice_result: Option<usize>,
    pub waiting_for_input: bool,
}

impl BevyRenderer {
    pub fn new() -> Self {
        Self {
            widget_backend:Default::default(),
            #[cfg(all(feature="video",not(target_arch="wasm32")))]
            native_admission:None,
            menu_authority: Default::default(),
            menu_stamp: Default::default(),
            menu_pending: false,
            menu_can_rollback: false,
            audio_checkpoints:Default::default(),
            debugger:Default::default(),
            pending: Vec::new(),
            scene_batch: None,
            scene_stepped: false,
            scene_frame_failed: false,
            scene_menus: Vec::new(),
            scene_epoch:None,
            scene_owner:None,
            choice_result: None,
            waiting_for_input: false,
        }
    }

    pub fn take_pending(&mut self) -> Vec<VnCommand> {
        if self.scene_batch.is_some() { return Vec::new(); }
        std::mem::take(&mut self.pending)
    }
    pub(crate) fn scene_batch_pending(&self) -> bool { self.scene_batch.is_some() }
    pub(crate) fn take_scene_menu_receipts(&mut self) -> Result<Vec<VnCommand>,String> {
        if self.scene_menus.is_empty() { return Ok(Vec::new()); }
        let count = u64::try_from(self.scene_menus.len()).map_err(|_|"Too many menu requests".to_string())?;
        let first = NEXT_MENU_RECEIPT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|value|value.checked_add(count))
            .map_err(|_|"Menu receipt identities exhausted".to_string())?;
        let requests = std::mem::take(&mut self.scene_menus);
        let mut receipts = Vec::with_capacity(requests.len());
        for (index,request) in requests.into_iter().enumerate() {
            // Publication itself only queues a preflighted effect. This host
            // allocation is fallible before mutating the menu authority.
            let identity = first + index as u64;
            receipts.push(VnCommand::SourceMenu(crate::source_menus::Receipt {
                identity, effect:request.effect, previous:request.previous, stamp:request.stamp, history_previous:request.history_previous,
            }));
        }
        if !receipts.is_empty() { self.menu_pending = true; }
        Ok(receipts)
    }
}

#[cfg(test)]
mod restore_tests {
    use super::*;
    #[test]
    fn a_pending_complete_batch_preserves_earliest_previous_and_delivers_transients_once() {
        let mut game=rvn_core::Engine::from_program(rvn_parser::parse_program("init {set global=1}\ninit {set global=2}\nscene label A\nlocal score=3\n\"A\"\n").unwrap(),BevyRenderer::new(),32).unwrap();
        game.enter_label("A").unwrap();game.step_until_interaction().unwrap();
        let initial=game.renderer.scene_batch.as_deref().unwrap().clone();
        let previous=serde_json::to_value(&initial.previous).unwrap();
        let mut first=initial.clone();first.speech_requests=vec![rvn_ui::accessibility::SpeechRequest::Speak("first".into())];
        game.renderer.publish_scene_batch(first);
        game.renderer.pending.push(VnCommand::SfxPlay{file:"once.wav".into()});
        let mut next=initial;next.speech_requests=vec![rvn_ui::accessibility::SpeechRequest::Speak("second".into())];
        game.renderer.publish_scene_batch(next);
        let queued=game.renderer.scene_batch.as_ref().unwrap();
        assert_eq!(serde_json::to_value(&queued.previous).unwrap(),previous);
        assert_eq!(queued.speech_requests,vec![rvn_ui::accessibility::SpeechRequest::Speak("first".into()),rvn_ui::accessibility::SpeechRequest::Speak("second".into())]);
        assert_eq!(game.renderer.pending.iter().filter(|command|matches!(command,VnCommand::SfxPlay{file} if file=="once.wav")).count(),1);
        assert!(game.renderer.take_pending().is_empty());
        game.renderer.scene_batch.take();
        assert_eq!(game.renderer.take_pending().iter().filter(|command|matches!(command,VnCommand::SfxPlay{..})).count(),1);
        assert!(game.renderer.take_pending().is_empty());
    }
    #[test]
    fn load_compatibility_is_reported_only_after_a_successful_load() {
        let script = rvn_parser::parse("label start\n\"Hello\"\nreturn").unwrap();
        let mut engine = rvn_core::Engine::new(script, BevyRenderer::new(), 10).unwrap();
        engine.step_until_interaction().unwrap();
        let saved = rvn_core::save::SaveData::from_state(
            &engine.state,
            1,
            "test".into(),
            "main.rvn".into(),
        );
        engine.renderer.take_pending();
        let mut invalid = saved.clone();
        invalid.pc = usize::MAX;
        assert!(engine.load_data(invalid).is_err());
        assert!(!engine
            .renderer
            .take_pending()
            .iter()
            .any(|command| matches!(command, VnCommand::LoadedCompatibility(_))));
        let mut legacy = saved.clone();
        legacy.story_identity = None;
        engine.load_data(legacy).unwrap();
        assert!(matches!(
            engine.renderer.take_pending().last(),
            Some(VnCommand::LoadedCompatibility(
                rvn_core::LoadCompatibility::LegacyUnchecked
            ))
        ));
        engine.load_data(saved).unwrap();
        assert!(matches!(
            engine.renderer.take_pending().last(),
            Some(VnCommand::LoadedCompatibility(
                rvn_core::LoadCompatibility::Verified
            ))
        ));
    }
    #[test]
    fn restoring_an_empty_cast_discards_previous_visual_and_music_commands() {
        let mut engine = rvn_core::Engine::new(vec![], BevyRenderer::new(), 10).unwrap();
        engine.renderer.pending.push(VnCommand::ShowSprite {
            id: "old_character".into(),
            emotion: None,
            position: rvn_parser::Position::Left,
            transition: Transition::None,
        });
        engine.renderer.restore_screen(&engine.state);
        let commands = engine.renderer.take_pending();
        assert!(matches!(commands.first(), Some(VnCommand::ClearSprites)));
        assert!(!commands
            .iter()
            .any(|cmd| matches!(cmd, VnCommand::ShowSprite { .. })));
        assert!(commands
            .iter()
            .any(|cmd| matches!(cmd, VnCommand::MusicStop)));
    }

    #[test]
    fn actual_narrative_screen_can_issue_requests_but_fictitious_and_closed_origins_cannot() {
        let source="handler act(event){menu.execute(menu_action(\"none\"))}\nscreen inventory(){return component(\"act\",\"button\",{\"events\":{\"click\":\"act\"}},[])}\nlabel start\nui.open(\"inventory\",[],false,0)\n\"Waiting\"";
        let mut engine=rvn_core::Engine::new(rvn_parser::parse(source).unwrap(),BevyRenderer::new(),10).unwrap();
        engine.step_until_interaction().unwrap();engine.renderer.menu_authority.story_ui_active=true;
        engine. interface_event(rvn_core::ui::UiInput{screen:"inventory".into(),element:"act".into(),kind:rvn_ui::programmable::ScreenEventKind::Click,value:None,key:None}).unwrap();
        let receipt=engine.renderer.take_pending().into_iter().find_map(|command|if let VnCommand::SourceMenu(receipt)=command{Some(receipt)}else{None}).unwrap();
        engine.renderer.menu_pending=false;assert!(engine.renderer.validate_menu_request(&receipt.effect,&engine.state).is_ok());
        let mut fake=receipt.effect.clone();fake.screen="invented".into();assert!(engine.renderer.validate_menu_request(&fake,&engine.state).is_err());
        let mut closed=engine.state.clone();closed.ui.screens.clear();assert!(engine.renderer.validate_menu_request(&receipt.effect,&closed).is_err());
    }
}

fn cg_unlock_receipt(batch:&rvn_core::renderer::SceneRenderBatch)->Option<&str> {
    let id=batch.state.cinematic.current.as_deref()?;
    let target=|state:&GameState|state.scene_execution.as_ref().and_then(|execution|execution.resources.target(&rvn_core::scene_tasks::TaskOwner::Game,rvn_core::scene_resources::TargetKind::Cinematic,"cinematic")).cloned();
    (batch.previous.cinematic.current.as_deref()!=Some(id) || target(&batch.previous)!=target(&batch.state)).then_some(id)
}

impl Renderer for BevyRenderer {
    fn prepare_widget_viewport_batch(&self,owners:&[rvn_core::widget_backend_0956::WidgetBackendOwner],transition:Option<&dyn rvn_core::widget_backend_0956::PreparedWidgetViewport>)->Result<std::sync::Arc<dyn rvn_core::widget_backend_0956::PreparedWidgetViewportBatch>,String>{self.widget_backend.prepare_batch_after(owners,transition)}
    fn validate_widget_viewport_batch(&self,batch:&dyn rvn_core::widget_backend_0956::PreparedWidgetViewportBatch)->Result<(),String>{self.widget_backend.validate_batch(batch)}
    fn reserve_widget_viewport_batch(&self,batch:std::sync::Arc<dyn rvn_core::widget_backend_0956::PreparedWidgetViewportBatch>)->Result<Box<dyn rvn_core::widget_backend_0956::WidgetViewportCommitReservation>,String>{self.widget_backend.reserve_batch(batch)}
    fn prepare_widget_viewport(&self,owner:&rvn_core::widget_backend_0956::WidgetBackendOwner,operation:rvn_core::widget_backend_0956::WidgetViewportOperation)->Result<std::sync::Arc<dyn rvn_core::widget_backend_0956::PreparedWidgetViewport>,String>{self.widget_backend.prepare(owner,operation)}
    fn validate_widget_viewport(&self,prepared:&dyn rvn_core::widget_backend_0956::PreparedWidgetViewport)->Result<(),String>{self.widget_backend.validate(prepared)}
    fn commit_scene_batch_atomic(&mut self,batch:rvn_core::renderer::SceneRenderBatch,widget:Option<&dyn rvn_core::widget_backend_0956::PreparedWidgetViewport>,commit:&mut dyn FnMut(&rvn_core::renderer::SceneRenderBatch))->Result<(),String>{
        self.validate_scene_batch(&batch)?;
        if let Some(widget)=widget{let backend=self.widget_backend.clone();backend.commit(widget,||{commit(&batch);self.publish_scene_batch(batch)})}else{commit(&batch);self.publish_scene_batch(batch);Ok(())}
    }
    fn validate_prepared_native_restore(&self,prepared:&rvn_core::engine::PreparedSave15)->Result<(),String>{
        if prepared.native_media_checkpoint().is_none(){return Ok(());}
        #[cfg(all(feature="video",not(target_arch="wasm32")))]
        if crate::native_video::admits_exact(self.native_admission.as_ref(),prepared)?{return Ok(());}
        Err("Exact native restore requires its actual staged World admission".into())
    }

    fn capture_audio_checkpoint(&self,state:&GameState,instance_id:u64,epoch:u64)->Result<rvn_core::audio_checkpoint::AudioCheckpointSnapshot,String> {
        if self.scene_batch_pending() || self.scene_frame_failed {return Err("Audio Source checkpoint requires settled successful publication".into());}
        self.audio_checkpoints.capture(state,instance_id,epoch)
    }
    fn validate_audio_restore(&self,state:&GameState)->Result<(),String> {
        self.audio_checkpoints.validate_restore(state)
    }
    fn capture_frame_audio_checkpoint(&self,state:&GameState,instance:u64,epoch:u64)->Result<rvn_core::audio_checkpoint::AudioCheckpointSnapshot,String>{
        if self.scene_batch_pending()||self.scene_frame_failed{return Err("Frame Source checkpoint requires settled successful publication".into());}
        self.audio_checkpoints.capture_frame(state,instance,epoch)
    }
    fn validate_frame_audio_restore(&self,state:&GameState)->Result<(),String>{self.audio_checkpoints.validate_frame_restore(state)}
    fn validate_scene_batch(&self, batch: &rvn_core::renderer::SceneRenderBatch) -> Result<(), String> {
        // This preflight performs no I/O, worker creation or queue mutation.
        // Actual asynchronous asset failures retain their original epoch.
        if !batch.videos.is_empty() && !self.supports_video() {
            return Err("Video playback is unavailable in this renderer build".into());
        }
        rvn_core::audio_playback::describe(&batch.state, batch.runtime_epoch)?;
        if let Some(id)=cg_unlock_receipt(batch) {
            let same_epoch=self.scene_epoch.is_none_or(|epoch|epoch==batch.runtime_epoch);
            let queued:std::collections::BTreeSet<_>=self.pending.iter().filter_map(|command|match command {VnCommand::UnlockCinematic{id}=>Some(id.as_str()),_=>None}).collect();
            if same_epoch && !queued.contains(id) && queued.len()>=rvn_core::scene_resources::MAX_RESOURCES {
                return Err("Pending CG gallery receipt capacity exceeded".into());
            }
        }
        batch.state.motions.validate()?;
        batch.state.layered.validate()?;
        batch.state.videos.validate()?;
        let mut screens = std::collections::HashSet::new();
        for view in &batch.interfaces {
            if !screens.insert(&view.name) { return Err("Duplicate screen in scene render batch".into()); }
            view.root.validate()?;
        }
        rvn_core::ui::published_owner_hosts(&batch.state.ui,&batch.interfaces)?;
        let mut videos = std::collections::HashSet::new();
        for view in &batch.videos {
            let epoch=if view.resource_token.is_some(){batch.runtime_epoch}else{batch.video_epoch};
            if view.epoch != epoch || !videos.insert(&view.id) {
                return Err("Invalid video identity or epoch in scene render batch".into());
            }
            if let Some(token)=&view.resource_token {
                if !batch.state.scene_execution.as_ref().is_some_and(|execution|execution.resources.accepts(token,batch.runtime_epoch,execution.owner())) {
                    return Err("Inactive resource token in scene video projection".into());
                }
            }
            view.track.validate()?;
        }
        if batch.state.scene_execution.is_some() && !batch.menu_requests.is_empty() && batch.history_previous.is_none() {
            return Err("Modern source-menu publication has no valid pre-callback historical witness".into());
        }
        for effect in &batch.menu_requests {
            crate::source_menus::checkpoint_receipt_bytes(&batch.previous,effect,&self.menu_stamp,batch.history_previous.as_ref())?;
            self.validate_menu_request(effect,&batch.state)?;
        }
        for request in &batch.speech_requests { self.validate_accessibility_speech(request)?; }
        let old=self.scene_batch.as_ref();
        let mut count=0usize;let mut bytes=0usize;
        for request in old.into_iter().flat_map(|batch|batch.retired_widget_speech.iter()).chain(&batch.retired_widget_speech){
            count+=1;bytes=bytes.checked_add(request.validate()?).filter(|bytes|*bytes<=64*1024).ok_or("Old Widget Speech publication exceeds 64 KiB")?;
            if count>128{return Err("Old Widget Speech publication exceeds 128 commands".into());}
            self.validate_accessibility_speech(request.request())?;
        }
        Ok(())
    }
    fn publish_scene_batch(&mut self, mut batch: rvn_core::renderer::SceneRenderBatch) {
        let owner=batch.state.scene_execution.as_ref().and_then(|execution|execution.owner());
        let runtime_replaced = self.scene_epoch.is_some_and(|old|old != batch.runtime_epoch);
        let owner_changed = self.scene_owner.is_some_and(|old|old != owner);
        if runtime_replaced { self.pending.clear(); self.scene_menus.clear(); self.menu_pending=false; }
        if runtime_replaced || owner_changed {self.choice_result=None;self.waiting_for_input=false;}
        self.scene_epoch=Some(batch.runtime_epoch);self.scene_owner=Some(owner);
        for effect in batch.menu_requests.drain(..) {
            self.scene_menus.push(SceneMenuRequest {effect, previous:batch.previous.clone(), stamp:self.menu_stamp.clone(),history_previous:batch.history_previous.clone()});
        }
        // Idempotent gallery receipts retain accepted Shows across coalescing.
        // The bounded preflight is identical; refusal emits none.
        if let Some(id)=cg_unlock_receipt(&batch) {
            if !self.pending.iter().any(|command|matches!(command,VnCommand::UnlockCinematic{id:old} if old==id)) {
                self.pending.push(VnCommand::UnlockCinematic{id:id.to_owned()});
            }
        }
        if let Some(old) = self.scene_batch.take() {
            if !runtime_replaced {
                batch.previous = old.previous;
            batch.history_previous = old.history_previous;
                let mut requests = old.speech_requests;
                requests.append(&mut batch.speech_requests);
                batch.speech_requests = requests;
            }
            // Already accepted old-lifetime commands survive even a second
            // replacement before the consumer; this carries no source grant.
            let mut retired=old.retired_widget_speech;retired.append(&mut batch.retired_widget_speech);batch.retired_widget_speech=retired;
        }
        // Preserve validated one-shot commands (including Init sound/voice)
        // while the complete logical models coalesce into one pending slot.
        self.pending.retain(|command|!matches!(command,VnCommand::Interfaces(_)|VnCommand::Motions(_)|VnCommand::LayeredCharacters(_)|VnCommand::Videos(_)));
        self.scene_batch = Some(Box::new(batch));
        if !self.scene_menus.is_empty() { self.menu_pending = true; }
    }
    fn validate_menu_request(&self, effect: &rvn_ui::source_menus::MenuEffect, candidate: &GameState) -> Result<(), String> {
        if self.menu_pending { return Err("A menu operation is already pending".into()); }
        if matches!(&effect.request,rvn_ui::source_menus::MenuRequest::Action{action:rvn_ui::Action::Rollback})&&!self.menu_can_rollback {
            return Err("No previous narrative or UI state is available".into());
        }
        let instance = candidate.ui.screens.iter().find(|screen| screen.name == effect.screen
            && screen.order == effect.screen_order && screen.host_role == effect.host_role)
            .ok_or_else(|| "Menu request screen is not live in the candidate state".to_string())?;
        let mut authority = self.menu_authority.clone();
        if let Some(role) = instance.host_role {
            if !authority.screen_roles.values().any(|active| *active == role) {
                return Err("Source-menu role was retired".into());
            }
            authority.live_screens.insert(instance.name.clone());
            authority.screen_roles.insert(instance.name.clone(), role);
        } else if authority.story_ui_active {
            authority.live_screens.insert(instance.name.clone());
            authority.screen_roles.remove(&instance.name);
        }
        authority.validate(effect)
    }
    fn menu_request(&mut self, effect: &rvn_ui::source_menus::MenuEffect, previous: &GameState, candidate: &GameState) -> Result<(), String> {
        crate::source_menus::checkpoint_receipt_bytes(previous,effect,&self.menu_stamp,None)?;
        self.validate_menu_request(effect, candidate)?;
        let identity=NEXT_MENU_RECEIPT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|value|value.checked_add(1))
            .map_err(|_|"Menu receipt identities exhausted".to_string())?;
        self.pending.push(VnCommand::SourceMenu(crate::source_menus::Receipt {
            identity, effect: effect.clone(), stamp: self.menu_stamp.clone(), previous: previous.clone(),history_previous:None,
        }));
        self.menu_pending = true;
        Ok(())
    }
    fn menu_request_pending(&self) -> bool { self.menu_pending }
    fn loaded_compatibility(&mut self, compatibility: rvn_core::LoadCompatibility) {
        self.pending
            .retain(|command| !matches!(command, VnCommand::LoadedCompatibility(_)));
        self.pending
            .push(VnCommand::LoadedCompatibility(compatibility));
    }
    fn supports_accessibility(&self) -> bool {
        cfg!(any(
            target_os = "linux",
            target_os = "windows",
            target_arch = "wasm32"
        ))
    }
    fn update_accessibility(
        &mut self,
        settings: &rvn_ui::accessibility::AccessibilitySettings,
    ) -> Result<(), String> {
        settings.validate()?;
        if settings != &Default::default() && !self.supports_accessibility() {
            return Err("Accessibility is unavailable on this renderer platform".into());
        }
        self.pending
            .retain(|command| !matches!(command, VnCommand::Accessibility(_)));
        self.pending
            .push(VnCommand::Accessibility(settings.clone()));
        Ok(())
    }
    fn accessibility_speech(
        &mut self,
        request: &rvn_ui::accessibility::SpeechRequest,
    ) -> Result<(), String> {
        if !self.supports_accessibility() {
            return Err("Speech synthesis is unavailable on this renderer platform".into());
        }
        self.pending.push(VnCommand::Speech(request.clone()));
        Ok(())
    }
    fn supports_video(&self) -> bool {
        cfg!(any(target_arch = "wasm32", feature = "video"))
    }
    fn update_videos(&mut self, views: &[rvn_core::video::VideoView]) -> Result<(), String> {
        if !views.is_empty() && !self.supports_video() {
            return Err("Video playback is unavailable in this renderer build".into());
        }
        self.pending
            .retain(|command| !matches!(command, VnCommand::Videos(_)));
        self.pending.push(VnCommand::Videos(views.to_vec()));
        Ok(())
    }
    fn supports_layered_characters(&self) -> bool {
        true
    }
    fn update_layered_characters(
        &mut self,
        views: &[rvn_core::composition::LayeredView],
    ) -> Result<(), String> {
        self.pending
            .retain(|command| !matches!(command, VnCommand::LayeredCharacters(_)));
        self.pending
            .push(VnCommand::LayeredCharacters(views.to_vec()));
        Ok(())
    }
    fn supports_composable_motion(&self) -> bool {
        true
    }
    fn update_motions(&mut self, views: &[rvn_core::motion::MotionView]) -> Result<(), String> {
        self.pending
            .retain(|command| !matches!(command, VnCommand::Motions(_)));
        self.pending.push(VnCommand::Motions(views.to_vec()));
        Ok(())
    }
    fn supports_programmable_ui(&self) -> bool {
        true
    }
    fn supports_custom_canvas(&self) -> bool {
        true
    }
    fn update_interfaces(
        &mut self,
        screens: &[rvn_ui::programmable::ScreenView],
    ) -> Result<(), String> {
        self.pending
            .retain(|command| !matches!(command, VnCommand::Interfaces(_)));
        self.pending.push(VnCommand::Interfaces(screens.to_vec()));
        Ok(())
    }
    fn set_background(&mut self, path: &str, transition: &Transition) {
        self.pending.push(VnCommand::SetBackground {
            path: path.to_string(),
            transition: transition.clone(),
        });
    }

    fn show_cinematic(&mut self, id: &str, transition: Option<&str>) {
        self.pending.push(VnCommand::ShowCinematic {
            id: id.to_string(),
            transition: transition.map(str::to_string),
        });
    }

    fn hide_cinematic(&mut self, transition: Option<&str>) {
        self.pending.push(VnCommand::HideCinematic {
            transition: transition.map(str::to_string),
        });
    }

    fn unlock_ending(&mut self, id: &str) {
        self.pending
            .push(VnCommand::UnlockEnding { id: id.to_string() });
    }

    fn show_sprite(
        &mut self,
        id: &str,
        emotion: Option<&str>,
        position: &Position,
        transition: &Transition,
        _from: Option<&SpriteState>,
    ) {
        self.pending.push(VnCommand::ShowSprite {
            id: id.to_string(),
            emotion: emotion.map(str::to_string),
            position: position.clone(),
            transition: transition.clone(),
        });
    }

    fn hide_sprite(&mut self, id: &str, transition: &Transition, _from: &SpriteState) {
        self.pending.push(VnCommand::HideSprite {
            id: id.to_string(),
            transition: transition.clone(),
        });
    }

    fn move_sprite(
        &mut self,
        id: &str,
        position: &Position,
        transition: &Transition,
        _from: &SpriteState,
    ) {
        self.pending.push(VnCommand::MoveSprite {
            id: id.to_string(),
            position: position.clone(),
            transition: transition.clone(),
        });
    }

    fn animate_sprite(&mut self, id: &str, animation: &str, params: &[AnimationParam]) {
        self.pending.push(VnCommand::AnimateSprite {
            id: id.to_string(),
            animation: animation.to_string(),
            params: params.to_vec(),
        });
    }

    fn voice_play(&mut self, file: &str) {
        self.pending.push(VnCommand::VoicePlay {
            file: file.to_string(),
        });
    }

    fn voice_stop(&mut self) {
        self.pending.push(VnCommand::VoiceStop);
    }

    fn stop_sprite_animation(&mut self, id: &str) {
        self.pending
            .push(VnCommand::StopSpriteAnimation { id: id.to_string() });
    }

    fn show_dialogue(&mut self, character: Option<&str>, text: &str) {
        self.pending.push(VnCommand::ShowDialogue {
            character: character.map(str::to_string),
            text: text.to_string(),
        });
    }

    fn show_choice(&mut self, options: &[String]) -> usize {
        if let Some(idx) = self.choice_result.take() {
            self.waiting_for_input = false;
            idx
        } else {
            self.pending.push(VnCommand::ShowChoice {
                options: options.to_vec(),
            });
            self.waiting_for_input = true;
            0
        }
    }

    fn music_play(&mut self, file: &str, transition: &Transition, _previous: Option<&str>) {
        self.pending.push(VnCommand::MusicPlay {
            file: file.to_string(),
            transition: transition.clone(),
        });
    }

    fn music_stop(&mut self, _transition: &Transition) {
        self.pending.push(VnCommand::MusicStop);
    }

    fn music_set_volume(&mut self, level: f32) {
        self.pending.push(VnCommand::MusicSetVolume { level });
    }

    fn sfx_play(&mut self, file: &str, _transition: &Transition) {
        if _transition.duration_ms()>0 {self.pending.push(VnCommand::SfxPlayTransition{file:file.into(),transition:_transition.clone()});return;}
        self.pending.push(VnCommand::SfxPlay {
            file: file.to_string(),
        });
    }

    fn sfx_stop(&mut self, file: &str, _transition: &Transition) {
        if _transition.duration_ms()>0 {self.pending.push(VnCommand::SfxStopTransition{file:file.into(),transition:_transition.clone()});return;}
        self.pending.push(VnCommand::SfxStop {
            file: file.to_string(),
        });
    }

    fn show_imagemap(
        &mut self,
        background: &str,
        hover_image: Option<&str>,
        hotspots: &[Hotspot],
    ) -> usize {
        if let Some(idx) = self.choice_result.take() {
            self.waiting_for_input = false;
            idx
        } else {
            let hs = hotspots
                .iter()
                .map(crate::vn_command::ImagemapZone::from)
                .collect();
            self.pending.push(VnCommand::ShowImagemap {
                background: background.to_string(),
                hover_image: hover_image.map(str::to_string),
                hotspots: hs,
            });
            self.waiting_for_input = true;
            0
        }
    }

    fn restore_screen(&mut self, state: &GameState) {
        self.scene_batch = None;
        self.scene_stepped = false;
        self.scene_menus.clear();
        self.scene_epoch=None;self.scene_owner=None;
        self.menu_pending = false;
        self.pending.clear();
        self.pending.push(VnCommand::ClearSprites);
        self.pending.push(VnCommand::SetBackground {
            path: state.background_image.clone(),
            transition: Transition::None,
        });
        for (id, sprite) in &state.sprites {
            if sprite.visible {
                self.pending.push(VnCommand::ShowSprite {
                    id: id.clone(),
                    emotion: sprite.emotion.clone(),
                    position: sprite.position.clone(),
                    transition: Transition::None,
                });
            } else {
                self.pending.push(VnCommand::HideSprite {
                    id: id.clone(),
                    transition: Transition::None,
                });
            }
        }
        if let Some(id) = &state.cinematic.current {
            self.pending.push(VnCommand::ShowCinematic {
                id: id.clone(),
                transition: None,
            });
        } else {
            self.pending
                .push(VnCommand::HideCinematic { transition: None });
        }
        if let Some(file) = &state.music.current_file {
            self.pending.push(VnCommand::MusicPlay {
                file: file.clone(),
                transition: Transition::None,
            });
        } else {
            self.pending.push(VnCommand::MusicStop);
        }
    }

    fn set_typewriter_config(&mut self, speed_cps: f32) {
        self.pending
            .push(VnCommand::SetTypewriterConfig { speed_cps });
    }
}

#[cfg(test)]
#[path="startup_ending_bevy_tests_0904.rs"]
mod ending0904;

#[cfg(test)]
mod pending_batch_stack_contract_0974 {
    #[test]
    fn pending_publication_uses_unique_pointer_sized_storage_without_inline_states() {
        let renderer=super::BevyRenderer::new();
        assert_eq!(std::mem::size_of_val(&renderer.scene_batch),std::mem::size_of::<usize>());
        assert!(std::mem::size_of::<super::BevyRenderer>() < std::mem::size_of::<rvn_core::renderer::SceneRenderBatch>());
        assert!(!renderer.scene_batch_pending());
    }
}
