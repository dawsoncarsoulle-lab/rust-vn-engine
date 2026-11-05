
use rvn_parser::{parse_program,scheduled_source::scheduled_step_sources};
const ACTUAL:&str=include_str!("../../../fixtures/static-audio-source-0507.rvn");
#[test]fn actual_static_audio_source_declares_and_proves_active_asset_and_transition_outputs(){
 let p=parse_program(ACTUAL).unwrap();let sources=scheduled_step_sources(&p).unwrap();assert_eq!(sources.len(),3);assert!(sources[0].consumer_identity.contains("file=3@4~11"));assert_eq!(sources[0].endpoint.as_ref().unwrap().kind,"music_asset");assert_eq!(sources[0].endpoint.as_ref().unwrap().ports[0].pin,11);
}
#[test]fn old_metadata_without_source_suffix_keeps_identical_omitted_wire_fields(){
 let old=ACTUAL.replace("@4~11","@4").replace("@5~12","@5");let p=parse_program(&old).unwrap();let sources=scheduled_step_sources(&p).unwrap();let json=serde_json::to_string(&sources).unwrap();assert!(!json.contains("source_pin"));let restored:Vec<rvn_parser::scheduled_source::ScheduledStepSource>=serde_json::from_str(&json).unwrap();assert_eq!(serde_json::to_string(&restored).unwrap(),json);
}
#[test]fn missing_wrong_type_role_edge_or_producer_refuse_before_any_engine_initialization(){
 for(old,new)in [
  ("file=3@4~11","file=3@4~1000000"),("file=3@4~11","file=3@4~011"),("file=3@4~11","file=3@4~18446744073709551615"),
  ("file=3@4~11","file=3@0~11"),("file=3@4~11","file=3@1~11"),("file=3@4~11","file=3@4~12"),
  ("file=3@4~11","character=3@4~11"),("n2:music_play|","n2:set_variable|"),("n5:music_asset|","n5:text_value|"),
  ("music.play(\"host.wav\")","music.play(\"other.wav\")"),("music.play(\"host.wav\")","music.stop()"),
  ("capture \"n5:music_asset|value=11@0\" = \"host.wav\"","capture \"n5:music_asset|value=11@0\" = random(1,2)")
 ]{let source=ACTUAL.replacen(old,new,1);assert_ne!(source,ACTUAL);let p=parse_program(&source).unwrap();assert!(scheduled_step_sources(&p).is_err(),"forged {new} admitted");assert!(rvn_core::Engine::from_program(p,rvn_core::TerminalRenderer,8).is_err(),"Engine admitted forged {new}");}
}

#[test]fn reroute_source_requires_earlier_typed_active_capture_and_actual_output(){
 let actual=include_str!("../../../fixtures/static-audio-reroute-0507.rvn");let p=parse_program(actual).unwrap();assert!(scheduled_step_sources(&p).is_ok());assert!(rvn_core::Engine::from_program(p,rvn_core::TerminalRenderer,8).is_ok());
 let asset="        capture \"n5:sound_effect_asset|value=10@0\" = \"host.wav\"";
 let reroute="        capture \"n6:reroute|value=11@5,value_out=12@0\" = __rvn_captured_v1(\"n5:sound_effect_asset|value=10@0\")";
 let mut cases=vec![actual.replace("value_out=12@0","value_out=12@7"),actual.replace("value=11@5","value=11@0"),actual.replace("= __rvn_captured_v1(\"n5:sound_effect_asset|value=10@0\")","= random(1,2)"),actual.replace("__rvn_captured_v1(\"n5:sound_effect_asset|value=10@0\")","__rvn_captured_v1(\"n6:reroute|value=11@5,value_out=12@0\")")];
 assert!(actual.contains(&asset));assert!(actual.contains(&reroute));cases.push(actual.replace(&format!("{asset}\n{reroute}"),&format!("{reroute}\n{asset}")));
 for source in cases{assert_ne!(source,actual);let p=parse_program(&source).unwrap();assert!(scheduled_step_sources(&p).is_err(),"forged reroute admitted: {source}");assert!(rvn_core::Engine::from_program(p,rvn_core::TerminalRenderer,8).is_err());}
}
