use crate::types::{GameState, MusicState, SpriteState};
use rvn_parser::{AnimationParam, Hotspot, Position, Transition};
use std::io;

// ─── TRAIT ───────────────────────────────────────────────────────────────────

/// One preflighted publication. Hosts must consume this as a single state
/// replacement before clocks/input, rather than emitting fallible half updates.
#[derive(Clone, Debug)]
pub struct SceneRenderBatch {
    pub previous: GameState,
    pub history_previous: Option<crate::audio_checkpoint::HistoricalGameSnapshot>,
    pub state: GameState,
    pub display: Option<crate::engine::Interaction>,
    pub interfaces: Vec<rvn_ui::programmable::ScreenView>,
    pub motions: Vec<crate::motion::MotionView>,
    pub layered: Vec<crate::composition::LayeredView>,
    pub videos: Vec<crate::video::VideoView>,
    pub effects: Vec<crate::scene_resources::ResourceTask>,
    /// Transient requests are delivered once, even when a host coalesces batches.
    pub menu_requests: Vec<rvn_ui::source_menus::MenuEffect>,
    pub speech_requests: Vec<rvn_ui::accessibility::SpeechRequest>,
    /// Old Destruct effects, resolved on the actual host before replacing its selected settings.
    pub retired_widget_speech:Vec<RetiredWidgetSpeech>,
    pub runtime_epoch: u64,
    pub interface_epoch: u64,
    pub video_epoch: u64,
}

/// Non-serializable intent from an actual old Widget lifetime. It grants no source/input authority.
#[derive(Clone,Debug,PartialEq)]
pub struct RetiredWidgetSpeech {
 authored:rvn_ui::accessibility::AccessibilitySettings,
 fallback_language:Option<String>,
 request:rvn_ui::accessibility::SpeechRequest,
}
impl RetiredWidgetSpeech {
 pub(crate) fn new(authored:rvn_ui::accessibility::AccessibilitySettings,fallback_language:Option<String>,request:rvn_ui::accessibility::SpeechRequest)->Self{Self{authored,fallback_language,request}}
 pub fn authored(&self)->&rvn_ui::accessibility::AccessibilitySettings{&self.authored}
 pub fn fallback_language(&self)->Option<&str>{self.fallback_language.as_deref()}
 pub fn request(&self)->&rvn_ui::accessibility::SpeechRequest{&self.request}
 pub fn validate(&self)->Result<usize,String>{
  self.authored.validate()?;
  if self.fallback_language.as_ref().is_some_and(|language|language.len()>64||language.contains('\0')){return Err("Invalid old Widget Speech language witness".into());}
  let text=match &self.request{rvn_ui::accessibility::SpeechRequest::Speak(text)=>{rvn_ui::accessibility::validate_speech(text)?;text.len()},rvn_ui::accessibility::SpeechRequest::Stop=>0};
  std::mem::size_of::<Self>().checked_add(text).and_then(|n|n.checked_add(self.authored.language.as_ref().map_or(0,String::len))).and_then(|n|n.checked_add(self.fallback_language.as_ref().map_or(0,String::len))).ok_or_else(||"Old Widget Speech size overflow".into())
 }
}

pub trait Renderer {
    /// The trusted backend prepares a real wrapper transition without changing
    /// parent ownership. Its receipt is process-local and never a saved grant.
    fn prepare_widget_viewport(&self,_owner:&crate::widget_backend_0956::WidgetBackendOwner,_operation:crate::widget_backend_0956::WidgetViewportOperation)->Result<std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewport>,String>{Err("Renderer has no real Widget viewport backend".into())}
    fn validate_widget_viewport(&self,_prepared:&dyn crate::widget_backend_0956::PreparedWidgetViewport)->Result<(),String>{Err("Renderer cannot validate a Widget backend transition".into())}
    /// Full actual viewport reconciliation. Owners were admitted by Core from
    /// this exact candidate, never discovered from a child or an alias. A
    /// prepared single operation may seed the same already constructed object.
    fn prepare_widget_viewport_batch(&self,_owners:&[crate::widget_backend_0956::WidgetBackendOwner],_transition:Option<&dyn crate::widget_backend_0956::PreparedWidgetViewport>)->Result<std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>,String>{Err("Renderer has no full Widget viewport reconciliation".into())}
    fn validate_widget_viewport_batch(&self,_batch:&dyn crate::widget_backend_0956::PreparedWidgetViewportBatch)->Result<(),String>{Err("Renderer cannot validate a full Widget viewport reconciliation".into())}
    fn reserve_widget_viewport_batch(&self,_batch:std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>)->Result<Box<dyn crate::widget_backend_0956::WidgetViewportCommitReservation>,String>{Err("Renderer cannot reserve an actual Widget viewport publication".into())}
    /// The owned reservation spans the final source validation and the
    /// infallible Core/publication closure. No authored callback runs here.
    fn commit_widget_scene_batch_atomic(&mut self,batch:SceneRenderBatch,widget:std::sync::Arc<dyn crate::widget_backend_0956::PreparedWidgetViewportBatch>,commit:&mut dyn FnMut(&SceneRenderBatch))->Result<(),String>{
        self.validate_scene_batch(&batch)?;self.validate_widget_viewport_batch(widget.as_ref())?;
        let reservation=self.reserve_widget_viewport_batch(widget)?;
        let mut batch=Some(batch);reservation.commit(&mut||{let batch=batch.take().expect("single-use Widget scene publication");commit(&batch);self.publish_scene_batch(batch)});Ok(())
    }
    /// The final backend fence covers infallible Core mutation and publication.
    /// An override may hold its local ownership lock here, never during the VM
    /// callbacks which prepared the candidate. Planning renderers only validate.
    fn commit_scene_batch_atomic(&mut self,batch:SceneRenderBatch,widget:Option<&dyn crate::widget_backend_0956::PreparedWidgetViewport>,commit:&mut dyn FnMut(&SceneRenderBatch))->Result<(),String>{
        self.validate_scene_batch(&batch)?;if let Some(widget)=widget{self.validate_widget_viewport(widget)?;}
        commit(&batch);self.publish_scene_batch(batch);Ok(())
    }
    /// Optional exact native data needs the actual host's staged publication.
    /// A desired wire cursor is never a permission to commit a media restore.
    fn validate_prepared_native_restore(&self,prepared:&crate::engine::PreparedSave15)->Result<(),String>{
        if prepared.native_media_checkpoint().is_some(){return Err("Renderer has no exact native staged publication adapter".into());}Ok(())
    }
    /// Additional stopped-frame boundary. A renderer must cover actual Story
    /// and Host Sources under one fence, never reuse an old saved Some cursor.
    fn capture_frame_audio_checkpoint(&self,state:&GameState,instance:u64,epoch:u64)->Result<crate::audio_checkpoint::AudioCheckpointSnapshot,String>{
        if crate::frame_audio_checkpoint::has_host_readers(state)?{return Err("Renderer has no exact all-Source Host frame checkpoint adapter".into());}
        self.capture_audio_checkpoint(state,instance,epoch)
    }
    fn validate_frame_audio_restore(&self,state:&GameState)->Result<(),String>{
        crate::frame_audio_checkpoint::require_frame_checkpoints(state)?;
        if crate::frame_audio_checkpoint::has_host_readers(state)?{return Err("Renderer cannot verify restored Host assets/PCM Source metadata".into());}
        self.validate_audio_restore(state)
    }

    /// Capture real consumed Source positions at one shared fence. No renderer
    /// may substitute elapsed time or a saved cursor for a missing actual reader.
    fn capture_audio_checkpoint(&self,state:&GameState,instance_id:u64,runtime_epoch:u64)->Result<crate::audio_checkpoint::AudioCheckpointSnapshot,String> {
        // A renderer without actual readers preserves the existing guarded
        // SFX/Voice refusal and its public diagnostic; opt-in renderers override.
        crate::audio_playback::require_portable_checkpoint(state)?;
        let one_shots=crate::audio_playback::describe(state,runtime_epoch)?;
        let music=crate::audio_checkpoint::music_views(state,runtime_epoch)?;
        if one_shots.iter().any(|view|!matches!(view.token.task.key.owner,crate::scene_tasks::TaskOwner::Host{..}))
            || music.iter().any(|view|!matches!(view.owner,crate::scene_tasks::TaskOwner::Host{..})) {
            return Err("This renderer has no exact portable audio Source checkpoint".into());
        }
        Ok(crate::audio_checkpoint::AudioCheckpointSnapshot{instance_id,runtime_epoch,rows:Vec::new(),music:Vec::new()})
    }
    /// Verify all exact saved source metadata/assets before a Load candidate commits.
    fn validate_audio_restore(&self,state:&GameState)->Result<(),String> {
        crate::audio_checkpoint::require_saved_checkpoints(state)?;
        if crate::audio_playback::describe(state,0)?.iter().any(|view|!matches!(view.token.task.key.owner,crate::scene_tasks::TaskOwner::Host{..}))
            || crate::audio_checkpoint::music_views(state,0)?.iter().any(|view|!matches!(view.owner,crate::scene_tasks::TaskOwner::Host{..})) {
            return Err("This renderer cannot verify restored audio assets and PCM metadata".into());
        }
        Ok(())
    }
    /// Pure validation: an unsupported renderer rejects before Core changes
    /// its continuation, locals, random streams, history or callback epochs.
    fn validate_scene_batch(&self, _batch: &SceneRenderBatch) -> Result<(), String> {
        Err("This renderer does not support coherent narrative scene transfers".into())
    }
    /// Infallible after preflight. Resource failures arriving asynchronously
    /// belong to their original token and are reported through normal feedback.
    fn publish_scene_batch(&mut self, _batch: SceneRenderBatch) {}
    fn validate_menu_request(
        &self,
        _effect: &rvn_ui::source_menus::MenuEffect,
        _candidate: &GameState,
    ) -> Result<(), String> {
        Err("This renderer has no game-menu action authority".into())
    }
    /// Accept only already validated requests. Hosts queue them for their
    /// existing game authority; this method must not perform file I/O.
    fn menu_request(
        &mut self,
        _effect: &rvn_ui::source_menus::MenuEffect,
        _previous: &GameState,
        _candidate: &GameState,
    ) -> Result<(), String> {
        Err("This renderer has no game-menu action authority".into())
    }
    fn menu_request_pending(&self) -> bool {
        false
    }
    /// Report successful legacy loads without pretending their story identity
    /// was verified. Default keeps existing third-party renderers compatible.
    fn loaded_compatibility(&mut self, _compatibility: crate::engine::LoadCompatibility) {}
    fn supports_accessibility(&self) -> bool {
        false
    }
    fn update_accessibility(
        &mut self,
        settings: &rvn_ui::accessibility::AccessibilitySettings,
    ) -> Result<(), String> {
        if settings == &Default::default() {
            Ok(())
        } else {
            Err("This renderer does not support accessibility settings".into())
        }
    }
    fn accessibility_speech(
        &mut self,
        _request: &rvn_ui::accessibility::SpeechRequest,
    ) -> Result<(), String> {
        Err("This renderer does not support speech synthesis".into())
    }
    /// Side-effect-free preflight before committing any interface or policy.
    fn validate_accessibility_speech(
        &self,
        request: &rvn_ui::accessibility::SpeechRequest,
    ) -> Result<(), String> {
        if !self.supports_accessibility() {
            return Err("This renderer does not support speech synthesis".into());
        }
        if let rvn_ui::accessibility::SpeechRequest::Speak(text) = request {
            rvn_ui::accessibility::validate_speech(text)?;
        }
        Ok(())
    }
    fn supports_video(&self) -> bool {
        false
    }
    fn update_videos(&mut self, views: &[crate::video::VideoView]) -> Result<(), String> {
        if views.is_empty() {
            Ok(())
        } else {
            Err("This renderer does not support video playback".into())
        }
    }
    fn supports_layered_characters(&self) -> bool {
        false
    }
    fn update_layered_characters(
        &mut self,
        views: &[crate::composition::LayeredView],
    ) -> Result<(), String> {
        if views.is_empty() {
            Ok(())
        } else {
            Err("This renderer does not support layered characters".into())
        }
    }
    fn supports_composable_motion(&self) -> bool {
        false
    }
    fn update_motions(&mut self, views: &[crate::motion::MotionView]) -> Result<(), String> {
        if views.is_empty() {
            Ok(())
        } else {
            Err("This renderer does not support composable animations".into())
        }
    }
    fn supports_programmable_ui(&self) -> bool {
        false
    }
    fn supports_custom_canvas(&self) -> bool {
        false
    }
    /// Unsupported capabilities must fail before the engine commits game state.
    fn update_interfaces(
        &mut self,
        screens: &[rvn_ui::programmable::ScreenView],
    ) -> Result<(), String> {
        if screens.is_empty() {
            Ok(())
        } else {
            Err("This renderer does not support programmable interfaces".into())
        }
    }
    fn set_background(&mut self, path: &str, transition: &Transition);
    fn show_cinematic(&mut self, _id: &str, _transition: Option<&str>) {}
    fn hide_cinematic(&mut self, _transition: Option<&str>) {}
    fn unlock_ending(&mut self, _id: &str) {}
    fn show_sprite(
        &mut self,
        id: &str,
        emotion: Option<&str>,
        position: &Position,
        transition: &Transition,
        from: Option<&SpriteState>,
    );
    fn hide_sprite(&mut self, id: &str, transition: &Transition, from: &SpriteState);
    fn move_sprite(
        &mut self,
        id: &str,
        position: &Position,
        transition: &Transition,
        from: &SpriteState,
    );

    /// Lance une animation sur un sprite. Les renderers peuvent ignorer
    /// cette commande s'ils ne supportent pas les animations.
    fn animate_sprite(&mut self, _id: &str, _animation: &str, _params: &[AnimationParam]) {}

    /// Arrête l'animation en cours sur un sprite et restaure sa transform de base.
    fn stop_sprite_animation(&mut self, _id: &str) {}
    fn set_sprite_effect(
        &mut self,
        _id: &str,
        _flip_x: Option<bool>,
        _flip_y: Option<bool>,
        _scale: Option<f32>,
        _rotation: Option<f32>,
        _tint: Option<&str>,
    ) {
    }

    fn show_dialogue(&mut self, character: Option<&str>, text: &str);
    fn show_choice(&mut self, options: &[String]) -> usize;
    fn music_play(&mut self, file: &str, transition: &Transition, previous: Option<&str>);
    fn music_stop(&mut self, transition: &Transition);
    fn music_set_volume(&mut self, level: f32);
    fn sfx_play(&mut self, file: &str, transition: &Transition);
    fn sfx_stop(&mut self, file: &str, transition: &Transition);
    fn voice_play(&mut self, _file: &str) {}
    fn voice_stop(&mut self) {}
    fn show_imagemap(
        &mut self,
        background: &str,
        hover_image: Option<&str>,
        hotspots: &[Hotspot],
    ) -> usize;

    /// Appelé quand les paramètres typewriter changent.
    /// `speed_cps` = 0.0 → désactivé.
    fn set_typewriter_config(&mut self, _speed_cps: f32) {}

    /// Restaure l'audio depuis un état sauvegardé.
    fn restore_audio(&mut self, music: &MusicState) {
        if let Some(file) = &music.current_file {
            self.music_play(file, &Transition::None, None);
            self.music_set_volume(music.volume);
        } else {
            self.music_stop(&Transition::None);
        }
    }

    /// Restaure l'écran complet (fond + sprites + audio) depuis un GameState.
    /// Utilisé par rollback et load.
    fn restore_screen(&mut self, state: &GameState) {
        self.set_background(&state.background_image.clone(), &Transition::None);
        if let Some(id) = &state.cinematic.current {
            self.show_cinematic(id, None);
        } else {
            self.hide_cinematic(None);
        }
        let sprites: Vec<(String, SpriteState)> = state
            .sprites
            .iter()
            .filter(|(_, s)| s.visible)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (id, sprite) in &sprites {
            self.show_sprite(
                id,
                sprite.emotion.as_deref(),
                &sprite.position,
                &Transition::None,
                None,
            );
        }
        self.restore_audio(&state.music.clone());
    }
}

// ─── TERMINAL RENDERER ───────────────────────────────────────────────────────

/// Renderer minimaliste pour tests CLI et headless.
pub struct TerminalRenderer;

impl Renderer for TerminalRenderer {
    fn validate_scene_batch(&self, batch: &SceneRenderBatch) -> Result<(), String> {
        if !batch.interfaces.is_empty()
            || !batch.motions.is_empty()
            || !batch.layered.is_empty()
            || !batch.videos.is_empty()
            || !batch.effects.is_empty()
            || !crate::audio_playback::describe(&batch.state, batch.runtime_epoch)?.is_empty()
            || batch.state.accessibility != Default::default()
        {
            return Err("Terminal rendering does not support these scene resources".into());
        }
        Ok(())
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {
        self.restore_screen(&batch.state);
        match batch.display {
            Some(crate::Interaction::Dialogue { character, text }) => {
                self.show_dialogue(character.as_deref(), &text)
            }
            Some(crate::Interaction::Choice { options }) => {
                // Publication redraws a checkpoint; selection remains a
                // separate input operation and must not block a restore.
                for (index, text) in options.iter().enumerate() {
                    println!("  {}. {text}", index + 1);
                }
            }
            Some(crate::Interaction::Imagemap { background, .. }) => {
                println!("[imagemap] fond: {background}")
            }
            None => {}
        }
    }
    fn loaded_compatibility(&mut self, compatibility: crate::engine::LoadCompatibility) {
        if compatibility == crate::engine::LoadCompatibility::LegacyUnchecked {
            eprintln!("[save] Older save loaded. Compatibility with an edited story cannot be verified. Keep your original save.");
        }
    }
    fn set_background(&mut self, path: &str, t: &Transition) {
        println!("[bg] {path}  [{t}]");
    }
    fn show_cinematic(&mut self, id: &str, transition: Option<&str>) {
        println!("[cinematic] show {id}  [{}]", transition.unwrap_or("none"));
    }
    fn hide_cinematic(&mut self, transition: Option<&str>) {
        println!("[cinematic] hide  [{}]", transition.unwrap_or("none"));
    }
    fn show_sprite(
        &mut self,
        id: &str,
        emo: Option<&str>,
        pos: &Position,
        t: &Transition,
        _: Option<&SpriteState>,
    ) {
        println!("[show] {id} {} @ {pos}  [{t}]", emo.unwrap_or("—"));
    }
    fn hide_sprite(&mut self, id: &str, t: &Transition, _: &SpriteState) {
        println!("[hide] {id}  [{t}]");
    }
    fn move_sprite(&mut self, id: &str, pos: &Position, t: &Transition, _: &SpriteState) {
        println!("[move] {id} → {pos}  [{t}]");
    }
    fn animate_sprite(&mut self, id: &str, animation: &str, _params: &[AnimationParam]) {
        println!("[animate] {id}.{animation}");
    }
    fn stop_sprite_animation(&mut self, id: &str) {
        println!("[animate] stop {id}");
    }
    fn show_dialogue(&mut self, character: Option<&str>, text: &str) {
        match character {
            Some(name) => println!("{name} : \"{text}\""),
            None => println!("  {text}"),
        }
        let mut buf = String::new();
        print!("> ");
        let _ = io::stdin().read_line(&mut buf);
    }
    fn show_choice(&mut self, options: &[String]) -> usize {
        println!("\nCHOIX :");
        for (i, opt) in options.iter().enumerate() {
            println!("  {}. {}", i + 1, opt);
        }
        loop {
            let mut buf = String::new();
            let _ = io::stdin().read_line(&mut buf);
            if let Ok(n) = buf.trim().parse::<usize>() {
                if n >= 1 && n <= options.len() {
                    return n - 1;
                }
            }
        }
    }
    fn music_play(&mut self, file: &str, t: &Transition, prev: Option<&str>) {
        match prev {
            Some(p) => println!("[music] crossfade {p} → {file}  [{t}]"),
            None => println!("[music] play {file}  [{t}]"),
        }
    }
    fn music_stop(&mut self, t: &Transition) {
        println!("[music] stop  [{t}]");
    }
    fn music_set_volume(&mut self, level: f32) {
        println!("[music] volume {level:.2}");
    }
    fn sfx_play(&mut self, file: &str, t: &Transition) {
        println!("[sfx] play {file}  [{t}]");
    }
    fn sfx_stop(&mut self, file: &str, t: &Transition) {
        println!("[sfx] stop {file}  [{t}]");
    }
    fn show_imagemap(
        &mut self,
        background: &str,
        _hover: Option<&str>,
        hotspots: &[Hotspot],
    ) -> usize {
        println!("[imagemap] fond: {background}");
        for (i, hs) in hotspots.iter().enumerate() {
            let name = hs.name.as_deref().unwrap_or("?");
            println!(
                "  {}. {} ({},{}) → ({},{})",
                i + 1,
                name,
                hs.area.x1,
                hs.area.y1,
                hs.area.x2,
                hs.area.y2
            );
        }
        loop {
            print!("Cliquez sur une zone (1-{}) : ", hotspots.len());
            let mut buf = String::new();
            let _ = io::stdin().read_line(&mut buf);
            if let Ok(n) = buf.trim().parse::<usize>() {
                if n >= 1 && n <= hotspots.len() {
                    return n - 1;
                }
            }
        }
    }
}
