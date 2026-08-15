use crate::types::{GameState, MusicState, SpriteState};
use rvn_parser::{AnimationParam, Hotspot, Position, Transition};
use std::io;

// ─── TRAIT ───────────────────────────────────────────────────────────────────

pub trait Renderer {
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
