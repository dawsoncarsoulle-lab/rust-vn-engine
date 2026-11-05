//! Actual host policy witness for old-source Speech. No platform work during preparation.
use super::*;
#[derive(Clone,Debug,PartialEq)]
pub(crate) struct FrozenSpeech { settings:AccessibilitySettings, request:SpeechRequest }
impl FrozenSpeech {
 pub(crate) fn settings(&self)->&AccessibilitySettings{&self.settings}
 pub(crate) fn request(&self)->&SpeechRequest{&self.request}
 pub(super) fn deliver(&self,runtime:&mut Accessibility,speech:&Speech){
  if let Err(problem)=speech.request(&self.settings,self.request.clone()){runtime.status=problem;runtime.redraw=true;}
 }
}
#[derive(Default,PartialEq)]
pub(crate) struct PreparedRetiredSpeech { entries:Vec<FrozenSpeech>, player:Option<AccessibilitySettings>, language:Option<String> }
impl PreparedRetiredSpeech {
 pub(crate) fn prepare(world:&World,rows:&[rvn_core::renderer::RetiredWidgetSpeech])->Result<Self,String>{
  if rows.is_empty(){return Ok(Self::default());}
  let runtime=world.get_resource::<Accessibility>().ok_or("Old Widget Speech requires the actual accessibility runtime")?;
  let player=if runtime.initialized{runtime.player.clone()}else{world.get_resource::<PersistentDataResource>().ok_or("Old Widget Speech requires actual player policy")?.data.accessibility.clone().filter(|settings|settings.validate().is_ok())};
  let language=world.get_resource::<Settings>().ok_or("Old Widget Speech requires actual selected language")?.language.clone();
  let mut entries=Vec::with_capacity(rows.len());let mut bytes=0usize;
  for row in rows {
   bytes=bytes.checked_add(row.validate()?).filter(|bytes|*bytes<=64*1024).ok_or("Old Widget Speech publication exceeds 64 KiB")?;
   if rows.len()>128{return Err("Old Widget Speech publication exceeds 128 commands".into());}
   let mut settings=player.as_ref().unwrap_or(row.authored()).clone();
   if settings.language.is_none(){settings.language=row.fallback_language().filter(|value|!value.is_empty()).or((!language.is_empty()).then_some(language.as_str())).map(str::to_owned);}
   settings.validate()?;
   let request=match row.request(){SpeechRequest::Speak(text)=>{let plain=rvn_core::parse_text_tags(text).map(|rich|rich.plain_text()).unwrap_or_else(|_|text.clone());if plain.trim().is_empty(){continue;}rvn_ui::accessibility::validate_speech(&plain)?;SpeechRequest::Speak(plain)},SpeechRequest::Stop=>SpeechRequest::Stop};
   entries.push(FrozenSpeech{settings,request});
  }
  Ok(Self{entries,player,language:Some(language)})
 }
 pub(crate) fn verify(&self,world:&World,rows:&[rvn_core::renderer::RetiredWidgetSpeech])->Result<(),String>{
  if Self::prepare(world,rows)?!=*self{return Err("Old Widget Speech player/selected-language witness changed before replacement".into());}Ok(())
 }
 pub(crate) fn commands(self)->Vec<VnCommand>{self.entries.into_iter().map(VnCommand::RetiredWidgetSpeech).collect()}
}
