//! Neutral optional checkpoint data. These values never authorize publication.
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
pub const MAX_SAMPLES:u64=8_294_400_000;
pub const BOUNDARY_SAMPLES:usize=64;
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedSourceProfile{
 pub version:u32,pub video_stream:u32,pub audio_stream:Option<u32>,pub video_time_base:(i32,i32),pub audio_time_base:Option<(i32,i32)>,pub codec_version:u32,pub format_version:u32,pub util_version:u32,pub configuration:String,pub sample_rate:u32,pub channels:u16,
}
impl DecodedSourceProfile{
 pub fn validate(&self)->Result<(),String>{
  let rational=|r:(i32,i32)|r.0>0&&r.1>0;
  if self.version!=1||self.sample_rate!=48_000||self.channels!=2||self.configuration.len()>65536||!rational(self.video_time_base)||self.audio_stream.is_some()!=self.audio_time_base.is_some()||self.audio_time_base.is_some_and(|r|!rational(r)){return Err("Invalid canonical native decode profile".into());}Ok(())
 }
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioCursor{pub next_sample:u64,pub next_channel:u16,pub boundary_samples:u16,pub boundary_sha256:[u8;32],pub ended:bool}
impl AudioCursor{
 pub fn validate(&self)->Result<(),String>{if self.next_sample>MAX_SAMPLES||self.next_channel>=2||self.next_channel as u64!=self.next_sample%2||self.boundary_samples as usize>BOUNDARY_SAMPLES||(!self.ended&&self.boundary_samples==0){return Err("Invalid native sample ordinal/channel/boundary".into());}Ok(())}
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureCursor{pub ordinal:u64,pub pts:i64,pub time_base:(i32,i32),pub rgba_sha256:[u8;32]}
impl PictureCursor {
 fn validate_for(&self,profile:&DecodedSourceProfile)->Result<(),String>{
  if self.ordinal>10_368_000 || self.time_base!=profile.video_time_base || self.pts<0 {
   return Err("Invalid native picture cursor".into());
  }
  Ok(())
 }
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskCheckpoint{pub source_locator:String,pub asset_sha256:[u8;32],pub profile:DecodedSourceProfile,pub picture:PictureCursor}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCheckpoint{pub id:String,pub source_locator:String,pub asset_sha256:[u8;32],pub profile:DecodedSourceProfile,pub picture:PictureCursor,pub audio:Option<AudioCursor>,#[serde(default,skip_serializing_if="Option::is_none")]pub mask:Option<MaskCheckpoint>}
impl NativeCheckpoint{
 pub fn validate(&self)->Result<(),String>{self.profile.validate()?;if let Some(mask)=&self.mask{mask.profile.validate()?;if !rvn_ui::programmable::safe_asset_path(&mask.source_locator)||mask.picture.validate_for(&mask.profile).is_err(){return Err("Invalid native mask source/picture cursor".into());}}if self.id.is_empty()||self.id.len()>4096||!rvn_ui::programmable::safe_asset_path(&self.source_locator)||self.picture.validate_for(&self.profile).is_err(){ return Err("Invalid native checkpoint source/picture".into());}if let Some(audio)=&self.audio{if self.profile.audio_stream.is_none(){return Err("Native cursor declares absent stream".into());}audio.validate()?;}Ok(())}
}
pub fn pcm_boundary(ordinal:u64,samples:impl IntoIterator<Item=f32>)->[u8;32]{let mut hash=Sha256::new();hash.update(b"rvn.native.pcm-boundary.0405\0");hash.update(ordinal.to_le_bytes());for sample in samples{hash.update(sample.to_bits().to_le_bytes());}hash.finalize().into()}
