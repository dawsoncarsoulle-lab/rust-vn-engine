//! Decode bounded external envelopes without interpreting a stale event payload.
use rvn_core::ui_input::{UiInputSource, ScopedUiInput};
use serde::Deserialize;
#[derive(Deserialize)]
struct RawEnvelope {
    source: String,
    #[serde(flatten)]
    payload: serde_json::Map<String,serde_json::Value>,
}
pub(crate) struct BrowserInput {
    pub(crate) source: UiInputSource,
    payload: serde_json::Map<String,serde_json::Value>,
}
pub(crate) fn decode_batch(text: &str) -> Result<Vec<BrowserInput>,String> {
    // Queue128, text65536 and header4096 are producer limits; the whole raw
    // batch is also bounded before serde's default recursion limit is applied.
    if text.len() > 9*1024*1024 { return Err("Browser input batch exceeds 9 MiB".into()); }
    let events:Vec<RawEnvelope> = serde_json::from_str(text).map_err(|error|format!("Browser input envelope: {error}"))?;
    if events.len()>128 { return Err("Browser input queue exceeds 128 events".into()); }
    events.into_iter().map(|event|Ok(BrowserInput { source:UiInputSource::from_wire(&event.source)?, payload:event.payload })).collect()
}
impl BrowserInput {
    pub(crate) fn admit<R:rvn_core::Renderer>(self, engine: &rvn_core::Engine<R>) -> Result<Option<ScopedUiInput>,String> {
        if !engine.ui_input_source_current(&self.source) {return Ok(None);}
        // The redundant event address is part of admission, not permission
        // to interpret an otherwise malformed kind/value for another screen.
        match self.payload.get("screen") {
            Some(serde_json::Value::String(screen)) if screen!=&self.source.screen=>return Ok(None),
            Some(serde_json::Value::String(_))=>{},
            _=>return Err("Browser input address must be a string".into()),
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Payload { screen:String, element:String, kind:rvn_ui::programmable::ScreenEventKind, value:Option<serde_json::Value>, key:Option<String> }
        let payload:Payload=serde_json::from_value(serde_json::Value::Object(self.payload)).map_err(|error|format!("Browser input payload: {error}"))?;
        if payload.screen!=self.source.screen {return Ok(None);}
        if payload.element.len()>128 || payload.key.as_ref().is_some_and(|key|key.len()>256) {return Err("Browser input payload address exceeds limits".into());}
        let value=payload.value.as_ref().map(rvn_core::ui::value_from_json).transpose().map_err(|error|format!("Browser input value: {error}"))?;
        Ok(Some(ScopedUiInput {source:self.source,event:rvn_core::ui::UiInput {screen:payload.screen,element:payload.element,kind:payload.kind,value,key:payload.key}}))
    }
}
