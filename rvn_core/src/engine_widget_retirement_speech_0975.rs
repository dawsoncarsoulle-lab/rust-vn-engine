//! Transient old-lifetime Speech intents. Neither saved state nor incoming DTO authority.
use super::*;
#[derive(Default)]
pub(super) struct SpeechLog { entries:Vec<crate::renderer::RetiredWidgetSpeech>, bytes:usize }
impl SpeechLog {
 fn push(&mut self,authored:&rvn_ui::accessibility::AccessibilitySettings,language:Option<&str>,request:&rvn_ui::accessibility::SpeechRequest)->Result<(),String>{
  authored.validate()?;
  let text=match request {rvn_ui::accessibility::SpeechRequest::Speak(text)=>{rvn_ui::accessibility::validate_speech(text)?;text.len()},rvn_ui::accessibility::SpeechRequest::Stop=>0};
  let bytes=self.bytes.checked_add(std::mem::size_of::<crate::renderer::RetiredWidgetSpeech>()).and_then(|n|n.checked_add(text)).and_then(|n|n.checked_add(authored.language.as_ref().map_or(0,String::len))).and_then(|n|n.checked_add(language.map_or(0,str::len))).filter(|n|*n<=64*1024).ok_or("Old Widget Speech exceeds its 64 KiB publication budget")?;
  if self.entries.len()>=128{return Err("Old Widget Speech exceeds 128 commands".into());}
  self.entries.push(crate::renderer::RetiredWidgetSpeech::new(authored.clone(),language.map(str::to_owned),request.clone()));self.bytes=bytes;Ok(())
 }
 pub(super) fn finish(&self)->Vec<crate::renderer::RetiredWidgetSpeech>{self.entries.clone()}
}
impl<R:Renderer> Engine<R>{
 pub(super) fn collect_retirement_speech(&self,next:&GameState,request:&rvn_ui::accessibility::SpeechRequest)->Result<bool,RuntimeError>{
  let Some(log)=&self.retirement_speech else{return Ok(false)};
  log.lock().map_err(|_|self.scene_resource_error("Old Widget Speech collector unavailable".into()))?.push(&next.accessibility,self.locale.as_ref().map(|locale|locale.current_lang()),request).map_err(|message|self.scene_resource_error(message))?;Ok(true)
 }
}
