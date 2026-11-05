//! Shared AST shape classification for scheduled consumers. This is grammar
//! evidence only: it grants neither an owner nor a runtime/publication permit.
use crate::Statement;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassicConsumerKind {
    Config,
    CharacterCreate,
    Background,
    CinematicShow,
    CinematicHide,
    UnlockEnding,
    SpriteShow,
    SpriteHide,
    SpriteMove,
    SpriteAnimate,
    SpriteStopAnimation,
    SpriteEffect,
    Timer,
    TimerCancel,
    MethodCall,
    Imagemap,
    TypewriterSet,
    TypewriterSpeed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduledConsumerKind {
    Value,
    Control,
    Dialogue,
    Choice,
    Interface,
    Motion,
    CharacterComposition,
    Video,
    Accessibility,
    Audio,
    Classic(ClassicConsumerKind),
}

/// A single classification used by Parser and Core instead of diverging body
/// allow-lists. Context, source topology, direct inputs and concrete dispatch
/// remain separate checks against their actual owner and immutable programme.
pub fn scheduled_consumer_kind(statement: &Statement) -> Option<ScheduledConsumerKind> {
    use ClassicConsumerKind as C;
    use ScheduledConsumerKind as K;
    use Statement as S;
    Some(match statement {
        S::SetVar { .. } | S::LocalVar { .. } | S::FunctionReturn { .. } => K::Value,
        S::If { .. } | S::While { .. } | S::ForEach { .. } | S::ForLoopWithBreak { .. }
        | S::Sequence { .. } | S::Jump { .. } | S::Call { .. } | S::Return => K::Control,
        S::Dialogue { .. } => K::Dialogue,
        S::Choice { .. } => K::Choice,
        S::WidgetAddToViewport { .. } | S::WidgetRemoveFromParent { .. } => K::Interface,
        S::UiOpen { .. } | S::UiClose { .. } | S::UiFocus { .. } | S::UiSetState { .. }
        | S::MenuExecute { .. } => K::Interface,
        S::MotionPlay { .. } | S::MotionStop { .. } | S::MotionWait { .. } => K::Motion,
        S::CharacterCompose { .. } | S::CharacterAttributes { .. } => K::CharacterComposition,
        S::VideoPlay { .. } | S::VideoPause { .. } | S::VideoResume { .. }
        | S::VideoStop { .. } | S::VideoSkip { .. } | S::VideoSeek { .. }
        | S::VideoVolume { .. } | S::VideoWait { .. } => K::Video,
        S::AccessibilityConfigure { .. } | S::AccessibilitySpeak { .. }
        | S::AccessibilityStop => K::Accessibility,
        S::MusicPlay { .. } | S::MusicStop { .. } | S::MusicVolume { .. }
        | S::MusicVolumeValue { .. } | S::SfxPlay { .. } | S::SfxStop { .. } | S::VoicePlay { .. } | S::VoiceStop => K::Audio,
        S::Config { .. } => K::Classic(C::Config),
        S::CharacterCreate { .. } => K::Classic(C::CharacterCreate),
        S::Scene { .. } => K::Classic(C::Background),
        S::CinematicShow { .. } => K::Classic(C::CinematicShow),
        S::CinematicHide { .. } => K::Classic(C::CinematicHide),
        S::UnlockEnding { .. } => K::Classic(C::UnlockEnding),
        S::ShowSprite { .. } => K::Classic(C::SpriteShow),
        S::HideSprite { .. } => K::Classic(C::SpriteHide),
        S::MoveSprite { .. } => K::Classic(C::SpriteMove),
        S::SpriteAnimate { .. } => K::Classic(C::SpriteAnimate),
        S::SpriteStopAnimation { .. } => K::Classic(C::SpriteStopAnimation),
        S::SpriteEffect { .. } => K::Classic(C::SpriteEffect),
        S::Timer { .. } => K::Classic(C::Timer),
        S::TimerCancel => K::Classic(C::TimerCancel),
        S::MethodCall { .. } => K::Classic(C::MethodCall),
        S::Imagemap { .. } => K::Classic(C::Imagemap),
        S::TypewriterSet { .. } | S::TypewriterSetValue { .. } => K::Classic(C::TypewriterSet),
        S::TypewriterSpeed { .. } | S::TypewriterSpeedValue { .. } => K::Classic(C::TypewriterSpeed),
        _ => return None,
    })
}

pub fn scheduled_consumer_body_supported(version: u8, body: &[Statement]) -> bool {
    match (version, body) {
        (1, [Statement::SetVar { .. } | Statement::LocalVar { .. } | Statement::FunctionReturn { .. }]) => true,
        (2, []) => true, // caller also enforces zero producers for BranchEnd
        (2, [statement]) => scheduled_consumer_kind(statement).is_some(),
        _ => false,
    }
}
