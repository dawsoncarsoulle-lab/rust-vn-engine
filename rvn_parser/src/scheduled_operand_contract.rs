//! Authoritative static-field contracts shared by source validation and graph
//! preparation. A listed field remains a literal AST operand; its connection
//! records provenance and never grants permission to evaluate a live producer.
use crate::{Statement, Transition};

#[derive(Clone, Copy)]
pub struct StaticConsumerContract {
    pub ports: &'static [&'static str],
    pub static_roles: &'static [&'static str],
}

pub fn static_consumer_contract(kind: &str) -> Option<StaticConsumerContract> {
    let (ports, static_roles): (&[_], &[_]) = match kind {
        "music_play" => (&["exec_in", "file", "transition", "exec_out"], &["file", "transition"]),
        "music_stop" => (&["exec_in", "transition", "exec_out"], &["transition"]),
        "music_volume" => (&["exec_in", "level", "exec_out"], &["level"]),
        "sfx_play" | "sfx_stop" | "voice_play" => (&["exec_in", "file", "exec_out"], &["file"]),
        "imagemap" => (&["exec_in", "background", "hover", "completed"], &["background", "hover"]),
        "dialogue" => (&["exec_in", "character", "text", "exec_out"], &["character"]),
        "config" => (&["exec_in", "key", "value", "exec_out"], &["key", "value"]),
        "scene" => (&["exec_in", "background", "transition", "exec_out"], &["background", "transition"]),
        "cinematic_show" => (&["exec_in", "cinematic", "transition", "exec_out"], &["cinematic", "transition"]),
        "cinematic_hide" => (&["exec_in", "transition", "exec_out"], &["transition"]),
        "unlock_ending" => (&["exec_in", "id", "exec_out"], &["id"]),
        "sprite_show" => (&["exec_in", "character", "emotion", "position", "transition", "exec_out"], &["character", "emotion", "position", "transition"]),
        "sprite_hide" => (&["exec_in", "character", "transition", "exec_out"], &["character", "transition"]),
        "sprite_move" => (&["exec_in", "character", "position", "transition", "exec_out"], &["character", "position", "transition"]),
        "sprite_animate" => (&["exec_in", "character", "animation", "exec_out"], &["character", "animation"]),
        "sprite_stop_animation" => (&["exec_in", "character", "exec_out"], &["character"]),
        "sprite_effect" => (&["exec_in", "character", "flip_x", "flip_y", "scale", "rotation", "tint", "exec_out"], &["character", "flip_x", "flip_y", "scale", "rotation", "tint"]),
        "timer_cancel" => (&["exec_in", "exec_out"], &[]),
        "character_create" => (&["exec_in", "id", "display_name", "exec_out"], &["id", "display_name"]),
        "typewriter_set" => (&["exec_in", "enabled", "exec_out"], &["enabled"]),
        "typewriter_speed" => (&["exec_in", "speed", "exec_out"], &["speed"]),
        "jump" => (&["exec_in", "target"], &["target"]),
        "call" => (&["exec_in", "target", "exec_out"], &["target"]),
        "timer" => (&["exec_in", "duration", "target", "exec_out"], &["duration", "target"]),
        "method_call" => (&["exec_in", "target", "method", "arg", "transition", "exec_out"], &["target", "method", "arg", "transition"]),
        _ => return None,
    };
    Some(StaticConsumerContract { ports, static_roles })
}

pub fn is_static_operand_role(kind: &str, role: &str) -> bool {
    // This producer operand records real Sprite provenance; CharacterValue is
    // still not an executable consumer and receives no standalone runtime grant.
    (kind == "character_value" && role == "sprite") || static_consumer_contract(kind).is_some_and(|contract| contract.static_roles.contains(&role))
}

/// Source output identity of a dynamic authored presentation input. This is
/// deliberately separate from literal/static operands.
pub fn is_dynamic_source_role(kind: &str, role: &str) -> bool {
    kind == "dialogue" && role == "text"
        || matches!(kind,"do_once"|"gate") && role=="start_closed"
        || kind=="do_n" && role=="n"
}

#[derive(Clone, Copy)]
pub(crate) enum StaticOperand<'a> {
    Asset(&'a str, &'a str),
    Number(f32),
    /// Seconds use the existing timer domain, independently of unit volume.
    Duration(f32),
    /// Sprite scale/rotation preserve their finite f32 domain without a volume clamp.
    FiniteNumber(f32),
    Position(&'a crate::Position),
    /// Cinematic syntax permits only its existing canonical transition presets.
    CinematicTransition(Option<&'a str>),
    Transition(&'a Transition),
    Bool(bool),
    Unsigned(u32),
    Label(&'a str),
    Text(&'a str),
    Character { id: &'a str, shown_sprite: Option<&'a str>, shows_appearance: bool },
}

pub(crate) fn statement_static_operand<'a>(statement: &'a Statement, kind: &str, role: &str) -> Result<StaticOperand<'a>, String> {
    use Statement::*;
    match (statement, kind, role) {
        (Config { key, .. }, "config", "key") => Ok(StaticOperand::Text(key)),
        (Config { value, .. }, "config", "value") => Ok(StaticOperand::Text(value)),
        (Scene { background, .. }, "scene", "background") => Ok(StaticOperand::Asset("scene_asset", background)),
        (Scene { transition, .. }, "scene", "transition") => Ok(StaticOperand::Transition(transition)),
        (CinematicShow { id, .. }, "cinematic_show", "cinematic") => Ok(StaticOperand::Asset("cinematic_asset", id)),
        (CinematicShow { transition, .. }, "cinematic_show", "transition") | (CinematicHide { transition }, "cinematic_hide", "transition") => Ok(StaticOperand::CinematicTransition(transition.as_deref())),
        (UnlockEnding { id }, "unlock_ending", "id") => Ok(StaticOperand::Text(id)),
        (ShowSprite { character_id, emotion, .. }, "sprite_show", "character") => Ok(StaticOperand::Character { id: character_id, shown_sprite: emotion.as_deref(), shows_appearance: true }),
        (HideSprite { character_id, .. }, "sprite_hide", "character") | (MoveSprite { character_id, .. }, "sprite_move", "character") | (SpriteAnimate { character_id, .. }, "sprite_animate", "character") | (SpriteStopAnimation { character_id }, "sprite_stop_animation", "character") | (SpriteEffect { character_id, .. }, "sprite_effect", "character") => Ok(StaticOperand::Character { id: character_id, shown_sprite: None, shows_appearance: false }),
        (ShowSprite { emotion, .. }, "sprite_show", "emotion") => Ok(StaticOperand::Text(emotion.as_deref().unwrap_or(""))),
        (ShowSprite { position: Some(position), .. }, "sprite_show", "position") | (MoveSprite { position, .. }, "sprite_move", "position") => Ok(StaticOperand::Position(position)),
        (ShowSprite { transition, .. }, "sprite_show", "transition") | (HideSprite { transition, .. }, "sprite_hide", "transition") | (MoveSprite { transition, .. }, "sprite_move", "transition") => Ok(StaticOperand::Transition(transition)),
        (SpriteAnimate { animation, .. }, "sprite_animate", "animation") => Ok(StaticOperand::Text(animation)),
        (SpriteEffect { flip_x: Some(value), .. }, "sprite_effect", "flip_x") | (SpriteEffect { flip_y: Some(value), .. }, "sprite_effect", "flip_y") => Ok(StaticOperand::Bool(*value)),
        (SpriteEffect { scale: Some(value), .. }, "sprite_effect", "scale") | (SpriteEffect { rotation: Some(value), .. }, "sprite_effect", "rotation") => Ok(StaticOperand::FiniteNumber(*value)),
        (SpriteEffect { tint: Some(value), .. }, "sprite_effect", "tint") => Ok(StaticOperand::Text(value)),
        (MusicPlay { file, .. }, "music_play", "file") => Ok(StaticOperand::Asset("music_asset", file)),
        (SfxPlay { file, .. }, "sfx_play", "file") | (SfxStop { file, .. }, "sfx_stop", "file") => Ok(StaticOperand::Asset("sound_effect_asset", file)),
        (VoicePlay { file }, "voice_play", "file") => Ok(StaticOperand::Asset("voice_asset", file)),
        (MusicVolume { level }, "music_volume", "level") => Ok(StaticOperand::Number(*level)),
        (Timer { duration_secs, .. }, "timer", "duration") => Ok(StaticOperand::Duration(*duration_secs)),
        (Timer { action, .. }, "timer", "target") => {
            let target = action.strip_prefix("jump ").filter(|target| !target.is_empty() && !target.chars().any(char::is_whitespace))
                .ok_or("Timer static source requires its actual canonical jump target")?;
            Ok(StaticOperand::Label(target))
        },
        (MusicPlay { transition, .. }, "music_play", "transition") | (MusicStop { transition }, "music_stop", "transition") => Ok(StaticOperand::Transition(transition)),
        (Imagemap { background, .. }, "imagemap", "background") => Ok(StaticOperand::Asset("scene_asset", background)),
        (Imagemap { hover_image, .. }, "imagemap", "hover") => Ok(StaticOperand::Asset("hover_image_asset", hover_image.as_deref().unwrap_or(""))),
        (Dialogue { character_id, .. }, "dialogue", "character") => Ok(StaticOperand::Character { id: character_id.as_deref().unwrap_or(""), shown_sprite: None, shows_appearance: false }),
        (CharacterCreate { id, .. }, "character_create", "id") => Ok(StaticOperand::Text(id)),
        (CharacterCreate { display_name, .. }, "character_create", "display_name") => Ok(StaticOperand::Text(display_name)),
        (TypewriterSet { enabled }, "typewriter_set", "enabled") => Ok(StaticOperand::Bool(*enabled)),
        (TypewriterSpeed { chars_per_sec }, "typewriter_speed", "speed") => Ok(StaticOperand::Unsigned(*chars_per_sec)),
        (Jump { target }, "jump", "target") | (Call { target }, "call", "target") => Ok(StaticOperand::Label(target)),
        (MethodCall { target, .. }, "method_call", "target") => Ok(StaticOperand::Text(target)),
        (MethodCall { method, .. }, "method_call", "method") => Ok(StaticOperand::Text(method)),
        (MethodCall { arg, .. }, "method_call", "arg") => Ok(StaticOperand::Text(arg.as_deref().unwrap_or(""))),
        (MethodCall { transition, .. }, "method_call", "transition") => Ok(StaticOperand::Transition(transition)),
        _ => Err("Declared static operand role differs from the actual command".into()),
    }
}

/// Repeated execution outputs derive only from actual AST children. The fixed
/// static operand roles remain shared with Graph gathering/import.
pub(crate) fn actual_static_consumer_ports(statement:&Statement,kind:&str)->Result<Vec<String>,String>{
    let actual=match statement{
        Statement::Config{..}=>"config",Statement::Scene{..}=>"scene",Statement::CinematicShow{..}=>"cinematic_show",Statement::CinematicHide{..}=>"cinematic_hide",Statement::UnlockEnding{..}=>"unlock_ending",
        Statement::ShowSprite{..}=>"sprite_show",Statement::HideSprite{..}=>"sprite_hide",Statement::MoveSprite{..}=>"sprite_move",Statement::SpriteAnimate{..}=>"sprite_animate",Statement::SpriteStopAnimation{..}=>"sprite_stop_animation",Statement::SpriteEffect{..}=>"sprite_effect",
        Statement::CharacterCreate{..}=>"character_create",Statement::Timer{..}=>"timer",Statement::TimerCancel=>"timer_cancel",Statement::MethodCall{..}=>"method_call",
        Statement::MusicPlay{..}=>"music_play",Statement::MusicStop{..}=>"music_stop",Statement::MusicVolume{..}|Statement::MusicVolumeValue{..}=>"music_volume",Statement::SfxPlay{..}=>"sfx_play",Statement::SfxStop{..}=>"sfx_stop",Statement::VoicePlay{..}=>"voice_play",Statement::Imagemap{..}=>"imagemap",Statement::Dialogue{..}=>"dialogue",Statement::TypewriterSet{..}|Statement::TypewriterSetValue{..}=>"typewriter_set",Statement::TypewriterSpeed{..}|Statement::TypewriterSpeedValue{..}=>"typewriter_speed",Statement::Jump{..}=>"jump",Statement::Call{..}=>"call",
        _=>return Err("Static consumer identity has no matching actual command".into()),
    };
    if kind!=actual{return Err("Static consumer identity differs from its actual command kind".into());}
    let contract=static_consumer_contract(kind).ok_or("Static consumer has no operand contract")?;
    let mut ports=contract.ports.iter().map(|port|(*port).to_owned()).collect::<Vec<_>>();
    if kind=="imagemap"{
        let Statement::Imagemap{hotspots,..}=statement else{return Err("Imagemap static contract differs from its actual AST".into());};
        if hotspots.len()>1024{return Err("Imagemap exceeds1024 authored hotspot outputs".into());}
        ports.extend((0..hotspots.len()).map(|index|format!("hotspot_{index}")));
    }
    Ok(ports)
}
