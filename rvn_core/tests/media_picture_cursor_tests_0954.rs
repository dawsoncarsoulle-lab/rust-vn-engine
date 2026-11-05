use rvn_media::checkpoint::*;
fn checkpoint()->NativeCheckpoint{
 let profile=DecodedSourceProfile{version:1,video_stream:0,audio_stream:None,video_time_base:(1,1000),audio_time_base:None,codec_version:1,format_version:1,util_version:1,configuration:String::new(),sample_rate:48_000,channels:2};
 let picture=PictureCursor{ordinal:0,pts:0,time_base:profile.video_time_base,rgba_sha256:[7;32]};
 NativeCheckpoint{id:"movie".into(),source_locator:"videos/movie.webm".into(),asset_sha256:[1;32],profile:profile.clone(),picture:picture.clone(),audio:None,mask:Some(MaskCheckpoint{source_locator:"videos/mask.webm".into(),asset_sha256:[2;32],profile,picture})}
}
#[test]
fn checkpoint0954_forged_mask_timestamp_is_rejected_after_actual_json_roundtrip(){
 let original=checkpoint();original.validate().unwrap();
 let encoded=serde_json::to_value(&original).unwrap();
 for mask in [false,true]{
  let mut forged=encoded.clone();
  let picture=if mask{&mut forged["mask"]["picture"]}else{&mut forged["picture"]};
  picture["pts"]=serde_json::json!(-1);
  let restored:NativeCheckpoint=serde_json::from_value(forged).unwrap();
  assert!(restored.validate().is_err(),"negative presentation timestamp accepted for mask={mask}");
  assert_eq!(original,serde_json::from_value::<NativeCheckpoint>(encoded.clone()).unwrap());
 }
}
#[test]
fn checkpoint0954_picture_bounds_and_profile_are_identical_for_color_and_mask(){
 let valid=checkpoint();
 for mask in [false,true]{
  for change in 0..3{
   let mut corrupt=valid.clone();
   let picture=if mask{&mut corrupt.mask.as_mut().unwrap().picture}else{&mut corrupt.picture};
   match change{0=>picture.ordinal=10_368_001,1=>picture.time_base=(1,2000),2=>picture.pts=-1,_=>unreachable!()}
   assert!(corrupt.validate().is_err(),"invalid picture accepted for mask={mask}, change={change}");
  }
  let mut boundary=valid.clone();
  let picture=if mask{&mut boundary.mask.as_mut().unwrap().picture}else{&mut boundary.picture};
  picture.ordinal=10_368_000;picture.pts=i64::MAX;boundary.validate().unwrap();
 }
 let mut legacy=valid;legacy.mask=None;let encoded=serde_json::to_value(&legacy).unwrap();assert!(encoded.get("mask").is_none());
 let restored:NativeCheckpoint=serde_json::from_value(encoded).unwrap();assert_eq!(legacy,restored);restored.validate().unwrap();
}
