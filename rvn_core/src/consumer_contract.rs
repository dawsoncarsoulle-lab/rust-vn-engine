//! Runtime adapter admission is separate from Parser grammar classification.
//! Sites below are derived only by the frozen AST traversal in ConsumerProgram;
//! neither a serialized descriptor nor a client declaration grants ownership.
use rvn_parser::{Statement,ScheduledConsumerKind,ClassicConsumerKind as C};
#[derive(Clone,Copy)]
pub(super) enum ClassicSite {Story,Initializer{narrative_resources:bool,game_tasks:bool},Computation,HostDeclaration}
pub(super) fn validate_site(command:&Statement,site:ClassicSite)->Result<(),&'static str>{
 let Some(ScheduledConsumerKind::Classic(kind))=rvn_parser::scheduled_consumer_kind(command)else{return Ok(());};
 if matches!(site,ClassicSite::Computation|ClassicSite::HostDeclaration){return Err("Native classic commands require an actual Game/Scene or Init command site, not a computation or host declaration");}
 match kind {
  C::MethodCall if matches!(site,ClassicSite::Initializer{narrative_resources:false,..})=>Err("Native MethodCall requires actual narrative video resource ownership"),
  C::MethodCall=>crate::engine::object_methods::validate_native_signature(command),
  C::Imagemap if matches!(site,ClassicSite::Initializer{..})=>Err("Native Init Imagemap requires an actual Startup input owner and continuation"),
  // Native Init endings are selected into the actual owned Startup below.
  // Its prepared one-shot host effects carry the command across publication.
  C::SpriteAnimate|C::SpriteStopAnimation|C::SpriteEffect if matches!(site,ClassicSite::Initializer{narrative_resources:false,game_tasks:false})=>Err("Native Init sprite effects require actual narrative resource ownership and a coherent batch"),
  C::Config if matches!(command,Statement::Config{key,..}if key!="bg")=>Err("Native Config key has no executable runtime adapter"),
  // Every admitted family has the concrete historical execution arm reused by
  // engine_classic_consumer. No wildcard admits a newly classified family.
  C::Imagemap|C::CharacterCreate|C::Config|C::Background|C::CinematicShow|C::CinematicHide|C::UnlockEnding|C::SpriteShow|C::SpriteHide|C::SpriteMove|C::SpriteAnimate|C::SpriteStopAnimation|C::SpriteEffect|C::Timer|C::TimerCancel|C::TypewriterSet|C::TypewriterSpeed=>Ok(()),
 }
}
