//! Root-only fixture export for the existing real Bevy player/editor.
//! No new renderer, native authority or observation is supplied by this export.
use super::*;
use sha2::{Digest,Sha256};

// Independent code-generated PNGs only. No engine artwork/font/brush is copied.
fn crc(bytes:&[u8])->u32 {let mut value=!0u32;for byte in bytes{value^=u32::from(*byte);for _ in 0..8{value=(value>>1)^(0xedb88320u32 & 0u32.wrapping_sub(value&1));}}!value}
fn chunk(out:&mut Vec<u8>,kind:&[u8;4],data:&[u8]) {out.extend((data.len()as u32).to_be_bytes());out.extend(kind);out.extend(data);let mut checked=kind.to_vec();checked.extend(data);out.extend(crc(&checked).to_be_bytes());}
fn independent_png(path:&std::path::Path,width:u32,height:u32,mode:u8) {
 assert!(width<=320&&height<=180);let mut raw=Vec::new();
 for y in 0..height {raw.push(0);for x in 0..width {
  let pixel=match mode {
   0=>if (x/20+y/20)%2==0{[28,88,48,255]}else{[44,126,65,255]},
   1=>if x<width/2{[25,45,130,255]}else{[130,55,25,255]},
   2=>if (x/16+y/16)%2==0{[150,25,125,255]}else{[235,190,35,255]},
   3=>if y<height/5{[245,220,20,255]}else if x<width/3{[230,40,35,255]}else if x<width*2/3{[35,210,230,255]}else{[0,0,0,0]},
   _=>panic!("No unbounded image generator"),
  };raw.extend(pixel);
 }}
 let mut zlib=vec![0x78,0x01];let mut remaining=raw.as_slice();while !remaining.is_empty(){let n=remaining.len().min(65535);zlib.push(u8::from(n==remaining.len()));zlib.extend((n as u16).to_le_bytes());zlib.extend((!(n as u16)).to_le_bytes());zlib.extend(&remaining[..n]);remaining=&remaining[n..];}
 let(mut a,mut b)=(1u32,0u32);for byte in &raw{a=(a+u32::from(*byte))%65521;b=(b+a)%65521;}zlib.extend(((b<<16)|a).to_be_bytes());
 let mut out=b"\x89PNG\r\n\x1a\n".to_vec();let mut ihdr=Vec::new();ihdr.extend(width.to_be_bytes());ihdr.extend(height.to_be_bytes());ihdr.extend([8,6,0,0,0]);chunk(&mut out,b"IHDR",&ihdr);chunk(&mut out,b"IDAT",&zlib);chunk(&mut out,b"IEND",&[]);std::fs::create_dir_all(path.parent().unwrap()).unwrap();std::fs::write(path,out).unwrap();
}
fn write_project(root:&PathBuf,game:bool,init:bool)->serde_json::Value {
 std::fs::create_dir(root).unwrap();
 let mut source=if init{String::from("init{\nset sampled=random(1,9)\nset timer_fired=0\ntypewriter=false\n")}else{format!("{}label start\nset sampled=random(1,9)\nset timer_fired=0\ntypewriter=false\n\"00 READY sampled={{sampled}}\"\n",if game{""}else{"scene "})};
 let mut stages=Vec::new();
 // Cancel is deliberately before the firing Timer: no looping jump-to-start,
 // and the cancellation can be observed without racing another source command.
 for (step,index) in (0..12).chain([13,12]).enumerate() {
  let family=FAMILIES[index];let number=step+1;
  if family.kind==NodeKind::Scene {source.push_str("scene \"previous.png\"\n");}
  source.push_str(family.setup);
  let body=if family.kind==NodeKind::Timer {"timer 2.0 => jump timer_complete"}else{family.body};
  if init&&family.kind==NodeKind::CharacterCreate{source.push_str("set create_stage=0\n");}else{source.push_str(body);source.push('\n');}
  let text=format!("{number:02} {} sampled={{sampled}}",family.id);
  if init{source.push_str(&format!("set init_step={number}\n"));}else if family.kind==NodeKind::CharacterCreate {source.push_str(&format!("hero \"{text}\"\n"));}else{source.push_str(&format!("\"{text}\"\n"));}
  stages.push(serde_json::json!({"step":number,"existing_criterion":family.id,"node_kind":family.kind,"authored_body":body,"setup":family.setup,"dialogue_marker":if init{serde_json::Value::Null}else{serde_json::Value::String(text)},"Init_CharacterCreate_actual_catalogue_id":if init&&family.kind==NodeKind::CharacterCreate{Some("created_native")}else{None},"source_only_before_real_native_observation":true}));
 }
 if init{source.push_str("}\nscene label start\n\"INIT completed step={init_step} sampled={sampled} - enter via real editor Init stepping for individual observations\"\n");}
 source.push_str("jump completed\nscene label timer_complete\nset timer_fired=timer_fired+1\n\"15 TIMER FIRED ONCE fired={timer_fired} sampled={sampled}\"\njump completed\nscene label completed\n\"16 END - do not advance; use save/rollback/reload here sampled={sampled}\"\nreturn\n");
 let input=root.join("authored.rvn");std::fs::write(&input,&source).unwrap();let original=rvn_parser::parse_file_program_with_uses(&input).unwrap();let mut graphs=rvn_graph::import_script(&original.script).unwrap();
 if init {
  // As in the existing Character18/R4c fixtures, ordinary Init character
  // declarations are shared metadata. Author the real executable catalogue
  // node through its public API, after a genuine source marker, instead.
  let graph=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Init)).unwrap();
  let marker=graph.nodes.values().find(|n|n.kind==NodeKind::SetVariable&&n.properties.get("name")==Some(&PropertyValue::String("create_stage".into()))).unwrap().id;
  let out=graph.pin_by_key(marker,"exec_out").unwrap().id;let next=graph.edges.values().find(|e|e.output==out).unwrap().clone();graph.remove_edge(next.id);
  let node=graph.add_catalog_node(NodeKind::CharacterCreate,[400.,240.]).unwrap();graph.set_pin_default(node,"id",PropertyValue::String("created_native".into())).unwrap();graph.set_pin_default(node,"display_name",PropertyValue::String("Created Native".into())).unwrap();graph.connect(out,graph.pin_by_key(node,"exec_in").unwrap().id).unwrap();graph.connect(graph.pin_by_key(node,"exec_out").unwrap().id,next.input).unwrap();graph.materialize_visible_defaults().unwrap();
 }
 let kinds=FAMILIES.iter().map(|f|f.kind).collect::<Vec<_>>();let parsed=strict_native_consumer::finish_program(root.clone(),original,graphs,&kinds);
 // Frozen source admission only: do not construct an Engine here, since its
 // real constructor executes Init. Effects remain for Rootâ€™s actual player.
 let _binding=rvn_core::consumer_schedule::ConsumerSourceBinding::from_program(&parsed).unwrap();
 let assets=root.join("assets");for (relative,w,h,mode) in [("garden.png",320,180,0),("previous.png",320,180,1),("cgs/cg.png",320,180,2),("cgs/previous.png",320,180,1),("sprites/hero/default.png",96,144,3)]{independent_png(&assets.join(relative),w,h,mode);}
 let config=format!("[project]\ntitle=\"RVN independent Classic14 {}\"\nmain_script=\"main.rvn\"\nstart_label=\"start\"\n[window]\nwidth=1280\nheight=720\nresizable=true\n[paths]\nassets=\"assets\"\nlocales=\"locales\"\nsaves=\"saves\"\ntheme=\"theme.toml\"\n",if init{"Init"}else if game{"Game"}else{"Scene"});std::fs::write(root.join("rvn.toml"),config).unwrap();std::fs::create_dir(root.join("locales")).unwrap();std::fs::create_dir(root.join("saves")).unwrap();
 // Missing optional theme/config use the genuine loader defaults. Bevy's
 // default font is used; no font or Epic asset is supplied by this fixture.
 let mut files=Vec::new();let mut pending=vec![root.clone()];while let Some(dir)=pending.pop(){for item in std::fs::read_dir(dir).unwrap(){let path=item.unwrap().path();if path.is_dir(){pending.push(path);}else{let bytes=std::fs::read(&path).unwrap();files.push(serde_json::json!({"path":path.strip_prefix(root).unwrap(),"sha256":format!("{:X}",Sha256::digest(&bytes)),"bytes":bytes.len()}));}}}files.sort_by_key(|row|row["path"].as_str().unwrap().to_owned());
 serde_json::json!({"project":root,"initial_owner":if init{"Init"}else if game{"Game"}else{"Scene"},"later_timer_target_is_real_Scene":true,"stages":stages,"files":files,"native_observed":false,"current_binary":serde_json::Value::Null})
}
#[test]
#[ignore="Root-only source fixture export; actual current player/editor and UI observations are a separate admission"]
fn classic1003_export_real_strict_fourteen_family_native_projects() {
 assert_eq!(std::env::var("RVN_CLASSIC1003_EXPORT").as_deref(),Ok("1"));
 let output=PathBuf::from(std::env::var_os("RVN_NATIVE_TYPEWRITER_FIXTURES").expect("Existing isolated fixture output directory"));assert!(output.is_dir());
 let root=output.join(format!("classic1003-native-{}",std::process::id()));std::fs::create_dir(&root).expect("Fresh export directory required; never overwrite a previous native corpus");
 let projects=[write_project(&root.join("scene"),false,false),write_project(&root.join("game"),true,false),write_project(&root.join("init"),false,true)];
 std::fs::write(root.join("fixture-index.private.json"),serde_json::to_vec_pretty(&serde_json::json!({"state":"STRICT_SOURCE_PROJECTS_PREPARED_NATIVE_NOT_EXECUTED","projects":projects,"new_criterion_ids":0,"native_pass":0,"pixel_pass":0})).unwrap()).unwrap();
 eprintln!("CLASSIC1003_PREPARED {}",root.display());
}
