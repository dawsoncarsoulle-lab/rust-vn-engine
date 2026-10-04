//! Opt-in native-window test of authored source presenters. Pointer events hit
//! real rendered controls through Bevy UiFocus; no OS input or direct UI calls.
use super::QaPointer;
use crate::{project_paths::ProjectPaths, resources::*, systems::{save_menu::SUPPORTED_SLOTS, settings_menu::Settings}};
use bevy::{app::AppExit, input::ButtonState, prelude::*, render::view::screenshot::ScreenshotManager, window::{CursorMoved, PrimaryWindow}};
use rvn_core::{save::{SaveData, SaveManager}, GameState};
use rvn_parser::Value;
use rvn_ui::PageRole;
use std::{collections::{BTreeMap,BTreeSet}, path::PathBuf, time::Instant};

const TIMEOUT:f64=45.0;
const SETTLE:f64=0.18;
#[derive(Clone,Copy,Debug)]
enum Check { None, Reject, ConfirmIoReject, ConfirmCompatibilityReject, Preference, Rollback, Amber, Saved, Protected(bool), Deleted, Loaded, Legacy, SourceTitle, Gallery, GallerySelection, GalleryEndings }
#[derive(Clone,Copy)]
struct Step { screen:&'static str, button:&'static str, check:Check }
fn steps()->Vec<Step> {
    use Check::*;
    let actions=[
        ("qa_title","qa_title_settings",None),
        ("qa_settings","qa_settings_invalid",Reject),
        ("qa_settings","qa_settings_music_down",Preference),
        ("qa_settings","qa_settings_typewriter",None),
        ("qa_settings","qa_settings_back",None),
        ("qa_title","qa_title_new",None),
        ("qa_dialogue","qa_dialogue_next",None),
        ("qa_quick_actions","qa_quick_rollback",Rollback),
        ("qa_dialogue","qa_dialogue_next",None),
        ("qa_dialogue","qa_dialogue_next",None),
        ("qa_dialogue","qa_dialogue_next",None),
        ("qa_choices","qa_choice_0",Amber),
        ("qa_dialogue","qa_dialogue_next",Gallery),
        ("qa_quick_actions","qa_quick_menu",None),
        ("qa_pause","qa_pause_title",SourceTitle),
        ("qa_title","qa_title_back",None),
        ("qa_pause","qa_pause_custom_page",Legacy),
        ("legacy","qa_legacy_custom_back",None),
        ("qa_pause","qa_pause_history",None),
        ("qa_history","qa_history_back",None),
        ("qa_pause","qa_pause_gallery",None),
        ("qa_gallery","qa_gallery_cg_qa-memory",GallerySelection),
        ("qa_gallery","qa_gallery_ending_tab",GalleryEndings),
        ("qa_gallery","qa_gallery_back",None),
        ("qa_pause","qa_pause_save",None),
        ("qa_save","qa_save_use_1",Saved),
        ("qa_pause","qa_pause_save",None),
        ("qa_save","qa_save_protect_1",Protected(true)),
        ("qa_save","qa_save_protect_1",Protected(false)),
        ("qa_save","qa_save_use_1",None),
        ("qa_confirm","qa_confirm_no",None),
        ("qa_save","qa_save_use_1",None),
        ("qa_confirm","qa_confirm_yes",ConfirmIoReject),
        ("qa_confirm","qa_confirm_yes",Saved),
        ("qa_pause","qa_pause_save",None),
        ("qa_save","qa_save_use_1",None),
        ("qa_confirm","qa_confirm_stale",Reject),
        ("qa_confirm","qa_confirm_no",None),
        ("qa_save","qa_save_back",None),
        ("qa_pause","qa_pause_load",None),
        ("qa_load","qa_load_use_1",None),
        ("qa_confirm","qa_confirm_yes",ConfirmCompatibilityReject),
        ("qa_confirm","qa_confirm_yes",Loaded),
        ("qa_quick_actions","qa_quick_inventory",None),
        ("qa_inventory","qa_inventory_invalid",Reject),
        ("qa_inventory","qa_inventory_none",None),
        ("qa_inventory","qa_inventory_close",None),
        ("qa_quick_actions","qa_quick_map",None),
        ("qa_map","qa_map_none",None),
        ("qa_map","qa_map_close",None),
        ("qa_quick_actions","qa_quick_menu",None),
        ("qa_pause","qa_pause_save",None),
        ("qa_save","qa_save_delete_1",None),
        ("qa_confirm","qa_confirm_yes",Deleted),
    ];
    actions.into_iter().map(|(screen,button,check)|Step{screen,button,check}).collect()
}
#[derive(Resource)]
struct Run {
    start:Instant, dir:PathBuf, phase:usize, stage:u8, entered:f64,
    captures:BTreeSet<String>,checks:Vec<String>,samples:Vec<serde_json::Value>,
    before:Option<GameState>,before_disk:BTreeMap<String,Vec<u8>>,before_token:Option<u64>,
    before_preference:f32,before_gate:String,saved:Option<SaveData>,failure:Option<String>,finished:Option<f64>,
    injected_failure:Option<InjectedFailure>,
    before_request_kind:String,before_dialogue:String,
    rollback_target:Option<(usize,String)>,typing_skips:Vec<serde_json::Value>,
    frames:Vec<f64>,
}
enum InjectedFailure {
    SaveLock(std::fs::File),
    ForeignStory(Vec<u8>),
}
fn gate_snapshot(world:&World)->String {
    let gate=world.resource::<crate::systems::save_menu::SaveConfirmation>();
    format!("{:?}/{:?}/{:?}/{:?}",gate.pending,gate.approved,gate.quick_return,gate.action_pending)
}
fn inject_failure(world:&mut World,check:Check)->Result<(),String> {
    let slot=world.resource::<ProjectPaths>().saves.join("slot_01.json");
    let injected=match check {
        Check::ConfirmIoReject=>{
            #[cfg(windows)] {
                use std::os::windows::fs::OpenOptionsExt;
                // Reads remain possible; replacing this owned file must fail.
                InjectedFailure::SaveLock(std::fs::OpenOptions::new().read(true).share_mode(1).open(&slot).map_err(|error|error.to_string())?)
            }
            #[cfg(not(windows))] {return Err("Save-lock native QA requires Windows".into());}
        }
        Check::ConfirmCompatibilityReject=>{
            let original=std::fs::read(&slot).map_err(|error|error.to_string())?;
            let mut data:serde_json::Value=serde_json::from_slice(&original).map_err(|error|error.to_string())?;
            data["story_identity"]=serde_json::Value::String("qa-foreign-story-after-confirmation-opened".into());
            std::fs::write(&slot,serde_json::to_vec_pretty(&data).map_err(|error|error.to_string())?).map_err(|error|error.to_string())?;
            InjectedFailure::ForeignStory(original)
        }
        _=>return Ok(()),
    };
    world.resource_mut::<Run>().injected_failure=Some(injected);Ok(())
}
fn restore_injected_failure(world:&mut World)->Result<(),String> {
    let injected=world.resource_mut::<Run>().injected_failure.take();
    match injected {
        Some(InjectedFailure::SaveLock(file))=>drop(file),
        Some(InjectedFailure::ForeignStory(bytes))=>std::fs::write(world.resource::<ProjectPaths>().saves.join("slot_01.json"),bytes).map_err(|error|error.to_string())?,
        None=>{},
    }
    Ok(())
}
fn primary(world:&mut World)->Result<Entity,String> {
    world.query_filtered::<Entity,With<PrimaryWindow>>().get_single(world).map_err(|error|error.to_string())
}
fn manager(world:&World)->Result<SaveManager,String> {
    SaveManager::new(&world.resource::<ProjectPaths>().saves,SUPPORTED_SLOTS).map_err(|error|error.to_string())
}
fn disk(world:&World)->Result<BTreeMap<String,Vec<u8>>,String> {
    let root=&world.resource::<ProjectPaths>().saves;
    if !root.exists(){return Ok(BTreeMap::new());}
    std::fs::read_dir(root).map_err(|error|error.to_string())?.filter_map(|entry|entry.ok())
        .filter(|entry|entry.path().is_file()).map(|entry| {
            let bytes=std::fs::read(entry.path()).map_err(|error|error.to_string())?;
            Ok((entry.file_name().to_string_lossy().into_owned(),bytes))
        }).collect()
}
fn values(state:&GameState)->serde_json::Value {
    serde_json::json!(state.ui.screens.iter().map(|screen|(&screen.name,&screen.values,&screen.canvas_states)).collect::<Vec<_>>())
}
fn confirmation_token(world:&World)->Option<u64> {
    let instance=world.resource::<VnEngine>().0.state.ui.screens.iter().find(|screen|screen.host_role==Some(PageRole::Confirm)&&screen.host_root)?;
    let Value::Dict(context)=instance.arguments.first()? else{return None;};
    let Value::Dict(gate)=context.get("confirmation")? else{return None;};
    let Value::Int(token)=gate.get("token")? else{return None;};
    u64::try_from(*token).ok().filter(|token|*token>0)
}
fn capture(world:&mut World,name:&str)->Result<(),String> {
    let window=primary(world)?;
    let path=world.resource::<Run>().dir.join(name);
    if path.exists(){return Err(format!("QA capture already exists: {}",path.display()));}
    world.resource_mut::<ScreenshotManager>().save_screenshot_to_disk(window,&path).map_err(|error|error.to_string())?;
    world.resource_mut::<Run>().captures.insert(name.into());Ok(())
}
fn record_roles(world:&mut World)->Result<bool,String> {
    let roles:Vec<_>=world.resource::<VnEngine>().0.state.ui.screens.iter().filter_map(|screen|screen.host_role).collect();
    for role in roles {
        let name=format!("role_{}.png",role.id());
        if !world.resource::<Run>().captures.contains(&name){capture(world,&name)?;return Ok(false);}
    }
    Ok(true)
}
fn centre(world:&mut World,step:Step)->Option<Vec2> {
    if step.screen=="legacy" {
        return world.query::<(&crate::menu_documents::MenuFocus,&GlobalTransform,&Node,&Style)>().iter(world)
            .find(|(focus,_,node,style)|focus.1.ends_with(&format!("/{}",step.button))&&node.size().min_element()>0.0&&style.display!=Display::None)
            .map(|(_,transform,_,_)|transform.translation().truncate());
    }
    world.query::<(&crate::programmable_ui::Control,&GlobalTransform,&Node,&Style)>().iter(world)
        .find(|(control,_,node,style)|control.screen==step.screen&&control.element==step.button&&node.size().min_element()>0.0&&style.display!=Display::None)
        .map(|(_,transform,_,_)|transform.translation().truncate())
}
fn validate_output(dir:&std::path::Path,saves:&std::path::Path)->Result<(),String> {
    let output_metadata=std::fs::symlink_metadata(dir).map_err(|error|error.to_string())?;
    if !output_metadata.is_dir()||output_metadata.file_type().is_symlink(){return Err("Source QA output must be a regular owned directory".into());}
    let output=dir.canonicalize().map_err(|error|error.to_string())?;
    let saves_metadata=std::fs::symlink_metadata(saves).map_err(|error|error.to_string())?;
    if !saves_metadata.is_dir()||saves_metadata.file_type().is_symlink()
        ||saves.canonicalize().map_err(|error|error.to_string())?!=output.join("saves") {
        return Err("Source QA saves must be the runtime-created output/saves directory".into());
    }
    // Runtime initialization creates this empty directory before the first QA
    // frame. No existing result, capture, preference or save is admissible.
    for entry in std::fs::read_dir(dir).map_err(|error|error.to_string())? {
        let entry=entry.map_err(|error|error.to_string())?;
        let kind=entry.file_type().map_err(|error|error.to_string())?;
        if entry.file_name().as_os_str()!=std::ffi::OsStr::new("saves")||!kind.is_dir()||kind.is_symlink() {
            return Err("Source QA output must contain only its fresh empty saves directory".into());
        }
    }
    if std::fs::read_dir(saves).map_err(|error|error.to_string())?.next().transpose().map_err(|error|error.to_string())?.is_some() {
        return Err("Source QA requires an empty runtime-created saves directory".into());
    }
    Ok(())
}

#[cfg(test)]
mod startup_guard_tests {
    use super::*;
    struct OutputFixture(PathBuf);
    impl OutputFixture {
        fn new()->Self {
            let output=std::env::temp_dir().join(format!("rvn-source-output-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            std::fs::create_dir_all(output.join("saves")).unwrap();Self(output)
        }
    }
    impl Drop for OutputFixture {
        fn drop(&mut self) {
            let temporary=std::env::temp_dir();
            if self.0.parent()==Some(temporary.as_path())&&self.0.file_name().is_some_and(|name|name.to_string_lossy().starts_with("rvn-source-output-")) {
                let _=std::fs::remove_dir_all(&self.0);
            }
        }
    }
    #[test]
    fn startup_accepts_only_host_created_empty_saves_and_rejects_existing_evidence_or_state() {
        let fixture=OutputFixture::new();let output=&fixture.0;let saves=output.join("saves");
        assert!(validate_output(output,&saves).is_ok());
        for name in ["native.json","role_title.png","unexpected.txt"] {
            let file=output.join(name);std::fs::write(&file,b"existing evidence").unwrap();
            assert!(validate_output(output,&saves).is_err());std::fs::remove_file(file).unwrap();
        }
        for name in ["persistent.json","slot_02.json"] {
            let file=saves.join(name);std::fs::write(&file,b"existing state").unwrap();
            assert!(validate_output(output,&saves).is_err());std::fs::remove_file(file).unwrap();
        }
        let other=output.join("other");std::fs::create_dir(&other).unwrap();
        assert!(validate_output(output,&saves).is_err());assert!(validate_output(output,&other).is_err());
        std::fs::remove_dir(other).unwrap();assert!(validate_output(output,&saves).is_ok());
    }
}
fn initialize(world:&World)->Result<Run,String> {
    let root=world.resource::<ProjectPaths>().root.canonicalize().map_err(|error|error.to_string())?;
    let temporary=std::env::temp_dir().canonicalize().map_err(|error|error.to_string())?;
    if !root.starts_with(temporary){return Err("RVN_QA_SOURCE requires a fresh project copy beneath SystemTemp".into());}
    let source=std::fs::read_to_string(root.join("main.rvn")).map_err(|error|error.to_string())?;
    if !source.contains("function qa_shell")||!source.contains("handler qa_request"){return Err("RVN_QA_SOURCE accepts only the original Source-menus-test fixture".into());}
    let dir=PathBuf::from(std::env::var_os("RVN_QA_OUTPUT").ok_or("QA output missing")?);
    validate_output(&dir,&world.resource::<ProjectPaths>().saves)?;
    let authored=std::fs::read_to_string(root.join("menus.rvnui")).map_err(|error|error.to_string())?;
    let document:rvn_ui::Document=serde_json::from_str(&authored).map_err(|error|error.to_string())?;
    if PageRole::ALL.iter().any(|role|document.source_screen(*role).is_none()){return Err("Source QA needs eleven explicit opt-in mappings".into());}
    if manager(world)?.load(1).is_ok()||manager(world)?.load_quicksave().is_ok(){return Err("Source QA requires empty owned saves".into());}
    Ok(Run{start:Instant::now(),dir,phase:0,stage:0,entered:0.0,captures:BTreeSet::new(),checks:Vec::new(),samples:Vec::new(),before:None,before_disk:BTreeMap::new(),before_token:None,before_preference:0.0,before_gate:String::new(),saved:None,failure:None,finished:None,injected_failure:None,before_request_kind:String::new(),before_dialogue:String::new(),rollback_target:None,typing_skips:Vec::new(),frames:Vec::new()})
}
fn check(world:&mut World,step:Step)->Result<bool,String> {
    let engine=&world.resource::<VnEngine>().0;
    let before=world.resource::<Run>().before.as_ref().ok_or("Missing source QA snapshot")?;
    let previous_dialogue=(before.current_interactive_pc,world.resource::<Run>().before_dialogue.clone());
    let outcome=serde_json::json!({"pc":engine.state.pc,"interactive_pc":engine.state.current_interactive_pc,
        "dialogue":world.resource::<TypewriterState>().full_text,"typing":world.resource::<TypewriterState>().typing,
        "qa_events":engine.state.vars.get("qa_events"),"qa_probe":engine.state.vars.get("qa_probe")});
    let replaces_state=matches!(step.check,Check::Reject|Check::ConfirmIoReject|Check::ConfirmCompatibilityReject|Check::Rollback|Check::Loaded)
        || matches!(step.button,"qa_title_new"|"qa_quick_inventory"|"qa_inventory_close"|"qa_quick_map"|"qa_map_close");
    if !replaces_state && step.screen!="legacy" {
        // Successful shared handlers increment both globals before the typed
        // request. This proves an authored UI activation really occurred,
        // including requests whose intended game effect is otherwise None.
        for (name,delta) in [("qa_events",1),("qa_probe",10)] {
            let Some(Value::Int(previous))=before.vars.get(name)else{return Err(format!("Fixture counter {name} is missing"));};
            if engine.state.vars.get(name)!=Some(&Value::Int(previous+delta)) {
                return Err(format!("{} did not commit exactly one authored handler: {name} should become {}",step.button,previous+delta));
            }
        }
        if !world.resource::<ScriptErrorMessage>().0.is_empty(){return Err(format!("Successful authored request {} left a local menu diagnostic",step.button));}
    }
    if step.button=="qa_dialogue_next" {
        let run=world.resource::<Run>();
        if run.before_request_kind=="skip_typewriter" {
            if engine.state.pc!=before.pc||world.resource::<TypewriterState>().typing
                ||world.resource::<TypewriterState>().full_text!=run.before_dialogue {
                return Err("Authored skip-typewriter click did not finish the same dialogue before advancing".into());
            }
            if run.typing_skips.iter().any(|sample|sample["phase"].as_u64()==Some(run.phase as u64)) {
                return Err("Authored dialogue control still requests SkipTypewriter after its completed first click".into());
            }
            let sample=serde_json::json!({"phase":run.phase,"pc":before.pc,"text":run.before_dialogue,"result":"same dialogue fully visible; next actual click must advance"});
            let mut run=world.resource_mut::<Run>();
            if let Some(activation)=run.samples.last_mut(){activation["after"]=outcome;}
            run.typing_skips.push(sample);
            return Ok(false);
        }
        if run.before_request_kind!="advance"||engine.state.pc==before.pc {
            return Err(format!("Authored dialogue control did not advance from PC{} (request {})",before.pc,run.before_request_kind));
        }
        if let Some(rvn_core::Interaction::Dialogue{text,..})=engine.current_interaction().map_err(|error|error.to_string())? {
            if text==run.before_dialogue {return Err("Measured dialogue advance kept the previous dialogue text".into());}
        }
    }
    match step.check {
        Check::None=>{},
        Check::Reject|Check::ConfirmIoReject|Check::ConfirmCompatibilityReject=>{
            if engine.state.vars!=before.vars||engine.state.random!=before.random||engine.state.display_random!=before.display_random
                || values(&engine.state)!=values(before)||disk(world)?!=world.resource::<Run>().before_disk {
                return Err("Rejected UI request changed globals, RNG, local values, canvas or disk".into());
            }
            if world.resource::<ScriptErrorMessage>().0.is_empty(){return Err("Rejected source request has no local diagnostic".into());}
            if world.resource::<Run>().before_token!=confirmation_token(world){return Err("Rejected stale token changed live confirmation authority".into());}
            if world.resource::<Run>().before_gate!=gate_snapshot(world){return Err("Rejected request consumed or changed the live confirmation operation".into());}
            capture(world,&format!("rejection_{}.png",world.resource::<Run>().phase))?;
            restore_injected_failure(world)?;
        },
        Check::Preference=>{
            if (world.resource::<Settings>().music_volume-world.resource::<Run>().before_preference).abs()<0.01{return Err("Valid UI preference did not persist after rejection".into());}
            if !world.resource::<ScriptErrorMessage>().0.is_empty(){return Err("Successful action did not clear source diagnostic".into());}
        },
        Check::Rollback=>{
            let target=world.resource::<Run>().rollback_target.as_ref().ok_or("No measured previous dialogue before the source rollback")?;
            if engine.state.pc>=before.pc||engine.state.current_interactive_pc!=target.0||world.resource::<TypewriterState>().full_text!=target.1 {
                return Err(format!("Source rollback did not restore measured previous dialogue PC{} from PC{}; actual PC{}",target.0,before.pc,engine.state.current_interactive_pc));
            }
        },
        Check::Amber=>{
            if engine.state.vars.get("qa_route")!=Some(&Value::Str("ambre".into())){return Err("Visible choice index0 did not reach authored amber response".into());}
            if world.resource::<Run>().samples.last().and_then(|sample|sample["destinations"].as_array()).and_then(|items|items.first()).and_then(|item|item.as_u64())!=Some(1){return Err("Conditional-choice fixture did not hide its first authored response".into());}
        },
        Check::Gallery=>{if !world.resource::<PersistentDataResource>().data.seen_cgs.contains("qa-memory"){return Err("Narrative CG was not unlocked through actual play".into());}},
        Check::GallerySelection=>{
            if world.resource::<GalleryState>().selected_cg.as_deref()!=Some("qa-memory"){return Err("Authored gallery selection did not use its live unlocked CG".into());}
            capture(world,"gallery_selection.png")?;
        },
        Check::GalleryEndings=>{
            if world.resource::<GalleryState>().view!=GalleryView::Endings||world.resource::<GalleryState>().selected_cg.is_some()
                || !world.resource::<PersistentDataResource>().data.seen_endings.contains("qa_ambre") {return Err("Authored gallery tab did not show its actual unlocked ending context".into());}
            capture(world,"gallery_endings.png")?;
        },
        Check::Saved=>{let data=manager(world)?.load(1).map_err(|error|error.to_string())?;world.resource_mut::<Run>().saved=Some(data);},
        Check::Protected(expected)=>{if manager(world)?.is_protected(1).map_err(|error|error.to_string())?!=expected{return Err("Save protection did not follow explicit Boolean request".into());}},
        Check::Deleted=>{if manager(world)?.load(1).is_ok(){return Err("Confirmed delete kept the owned save".into());}},
        Check::Loaded=>{
            let saved=world.resource::<Run>().saved.as_ref().ok_or("Missing saved source state")?;
            let saved_vars:std::collections::HashMap<_,_>=saved.vars.iter().map(|(name,value)|(name.clone(),Value::from(value.clone()))).collect();
            if engine.state.pc!=saved.pc||engine.state.vars!=saved_vars||engine.history.len()!=0{return Err("Same-PC source load did not restore exact globals and clear rollback".into());}
        },
        Check::Legacy=>{if world.resource::<crate::source_menus::SourceMenus>().modal(){return Err("Source presenter obscures an explicit legacy custom page".into());}},
        Check::SourceTitle=>{if !engine.state.ui.screens.iter().any(|screen|screen.host_role==Some(PageRole::Title)){return Err("Explicit title page did not select the source title role".into());}},
    }
    if step.button=="qa_dialogue_next" {
        world.resource_mut::<Run>().rollback_target=Some(previous_dialogue);
    }
    let mut run=world.resource_mut::<Run>();
    if let Some(activation)=run.samples.last_mut(){activation["after"]=outcome;}
    run.checks.push(format!("{}:{}",step.screen,step.button));Ok(true)
}
fn report(world:&World,result:&str)->Result<(),String> {
    let run=world.resource::<Run>();
    let mut frames=run.frames.clone();frames.sort_by(f64::total_cmp);
    let percentile=|ratio:f64|frames.get(((frames.len().saturating_sub(1)) as f64*ratio) as usize).copied();
    let captures:Vec<_>=run.captures.iter().map(|name| {
        let size=std::fs::read(run.dir.join(name)).ok().and_then(|bytes| {
            (bytes.len()>=24&&bytes.starts_with(b"\x89PNG\r\n\x1a\n")).then(||[u32::from_be_bytes(bytes[16..20].try_into().unwrap()),u32::from_be_bytes(bytes[20..24].try_into().unwrap())])
        });
        serde_json::json!({"file":name,"png_size":size})
    }).collect();
    let report=serde_json::json!({"result":result,"platform":std::env::consts::OS,"project":world.resource::<ProjectPaths>().root,
        "method":"Native Bevy GPU window: measured Control/GlobalTransform -> synthetic QaPointer -> UiFocus -> authored UI event -> typed core transaction -> game authority",
        "limits":["No OS input or native OLE claim","No Web/video certification","Manual native QA is complementary"],
        "elapsed_seconds":run.start.elapsed().as_secs_f64(),"phase":run.phase,"error":run.failure,"checks":run.checks,"captures":captures,"samples":run.samples,
        "logical_steps":steps().len(),"actual_pointer_activations":run.samples.len(),"verified_typing_skips":run.typing_skips,
        "frame_time_ms":{"samples":frames.len(),"median":percentile(0.5),"p95":percentile(0.95),"p99":percentile(0.99),"max":frames.last(),"scope":"End-to-end native app frame delta during this release QA scenario; not a GPU benchmark or UE5 equivalence"},
        "final_game":world.resource::<VnEngine>().0.state});
    std::fs::write(run.dir.join("native.json"),serde_json::to_vec_pretty(&report).map_err(|error|error.to_string())?).map_err(|error|error.to_string())
}
fn finish(world:&mut World,mut error:Option<String>,now:f64) {
    if let Err(problem)=restore_injected_failure(world){error=Some(format!("{} / QA fault restoration: {problem}",error.unwrap_or_default()));}
    if let Some(message)=&error{eprintln!("Source native QA failed: {message}");}
    world.resource_mut::<QaPointer>().buttons.push(ButtonState::Released);
    {let mut run=world.resource_mut::<Run>();run.failure=error;run.finished=Some(now);}
    if world.resource::<Run>().failure.is_some(){let _=capture(world,"failed_native_window.png");}
    let _=report(world,if world.resource::<Run>().failure.is_some(){"fail"}else{"pass"});
}
pub(crate) fn isolate_inputs(world:&mut World) {
    if let Some(mut keys)=world.get_resource_mut::<ButtonInput<KeyCode>>(){keys.reset_all();}
    if let Some(mut mouse)=world.get_resource_mut::<ButtonInput<MouseButton>>(){mouse.reset_all();}
    if let Some(mut pads)=world.get_resource_mut::<ButtonInput<GamepadButton>>(){pads.reset_all();}
    fn clear<E:Event>(world:&mut World){if let Some(mut events)=world.get_resource_mut::<Events<E>>(){events.clear();}}
    clear::<bevy::input::keyboard::KeyboardInput>(world);clear::<bevy::input::mouse::MouseButtonInput>(world);
    clear::<bevy::input::mouse::MouseWheel>(world);clear::<bevy::input::touch::TouchInput>(world);clear::<bevy::window::Ime>(world);clear::<CursorMoved>(world);
    if let Some(position)=world.resource::<QaPointer>().position {
        if let Ok(window)=primary(world){world.send_event(CursorMoved{window,position,delta:None});}
    }
}
pub(crate) fn drive(world:&mut World) {
    let now=world.resource::<Time>().elapsed_seconds_f64();
    if !world.contains_resource::<Run>(){
        match initialize(world){Ok(run)=>world.insert_resource(run),Err(error)=>{eprintln!("Source QA refused: {error}");world.send_event(AppExit::Error(std::num::NonZeroU8::new(1).unwrap()));return;}}
    }
    if let Some(finished)=world.resource::<Run>().finished {
        if now-finished>1.3{
            let captures_complete=world.resource::<Run>().captures.iter().all(|name|world.resource::<Run>().dir.join(name).is_file());
            if !captures_complete {world.resource_mut::<Run>().failure=Some("Native GPU capture was not flushed to disk".into());}
            let failed=world.resource::<Run>().failure.is_some();
            let _=report(world,if failed{"fail"}else{"pass"});world.send_event(if failed{AppExit::Error(std::num::NonZeroU8::new(1).unwrap())}else{AppExit::Success});
        }
        return;
    }
    if world.resource::<Run>().start.elapsed().as_secs_f64()>TIMEOUT{finish(world,Some("Source native QA exceeded 45-second watchdog".into()),now);return;}
    let frame=world.resource::<Time>().delta_seconds_f64()*1000.0;
    if world.resource::<Run>().frames.len()<10000{world.resource_mut::<Run>().frames.push(frame);}
    let actions=steps();let phase=world.resource::<Run>().phase;
    if phase>=actions.len(){
        let roles_complete=PageRole::ALL.iter().all(|role|world.resource::<Run>().captures.contains(&format!("role_{}.png",role.id())));
        finish(world,(!roles_complete).then(||"Not all eleven source roles were rendered".into()),now);return;
    }
    let step=actions[phase];let stage=world.resource::<Run>().stage;
    if now-world.resource::<Run>().entered<SETTLE{return;}
    if stage==0 {
        if matches!(step.screen,"qa_dialogue"|"qa_quick_actions"|"qa_choices") && *world.resource::<State<VnState>>().get()!=VnState::Waiting{return;}
        let Some(position)=centre(world,step) else{return;};
        match record_roles(world){Ok(true)=>{},Ok(false)=>return,Err(error)=>{finish(world,Some(error),now);return;}}
        let window=match primary(world){Ok(window)=>window,Err(error)=>{finish(world,Some(error),now);return;}};
        let own_window=world.get::<Window>(window).unwrap();
        if position.x<0.0||position.y<0.0||position.x>own_window.width()||position.y>own_window.height(){finish(world,Some(format!("Control {} is outside the real window: {position:?}",step.button)),now);return;}
        let request_kind=world.resource::<crate::programmable_ui::Screens>().views.iter().find(|view|view.name==step.screen)
            .and_then(|view|view.root.find(step.button)).and_then(|component|component.event_data.get("request"))
            .and_then(|request|request.get("kind")).and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        let before_dialogue=world.resource::<TypewriterState>().full_text.clone();
        let engine=&world.resource::<VnEngine>().0;
        let anchors:Vec<_>=engine.history.entries().iter().map(|entry|serde_json::json!({"pc":entry.state.pc,"narrative_display":entry.display.is_some(),"qa_events":entry.state.vars.get("qa_events")})).collect();
        let sample=serde_json::json!({"screen":step.screen,"button":step.button,"check":format!("{:?}",step.check),"pointer":[position.x,position.y],"window":[own_window.width(),own_window.height()],"dpi":own_window.resolution.scale_factor(),"phase":format!("{:?}",world.resource::<State<VnState>>().get()),"pc":engine.state.pc,"interactive_pc":engine.state.current_interactive_pc,"destinations":engine.active_choice_indices().unwrap_or_default(),"request_kind":request_kind,"typing":world.resource::<TypewriterState>().typing,"dialogue":before_dialogue,"history":anchors});
        if let Err(error)=inject_failure(world,step.check){finish(world,Some(error),now);return;}
        let state=world.resource::<VnEngine>().0.state.clone();
        let saved_disk=match disk(world){Ok(disk)=>disk,Err(error)=>{finish(world,Some(error),now);return;}};
        let token=confirmation_token(world);
        let volume=world.resource::<Settings>().music_volume;
        let before_gate=gate_snapshot(world);
        {let mut run=world.resource_mut::<Run>();run.before=Some(state);run.before_disk=saved_disk;run.before_token=token;run.before_preference=volume;run.before_gate=before_gate;run.before_request_kind=request_kind;run.before_dialogue=before_dialogue;run.samples.push(sample);run.stage=1;run.entered=now;}
        let mut pointer=world.resource_mut::<QaPointer>();pointer.position=Some(position);pointer.buttons.push(ButtonState::Pressed);
    } else if stage==1 {
        world.resource_mut::<QaPointer>().buttons.push(ButtonState::Released);
        let mut run=world.resource_mut::<Run>();run.stage=2;run.entered=now;
    } else {
        if step.button=="qa_dialogue_next"&&*world.resource::<State<VnState>>().get()!=VnState::Waiting{return;}
        if matches!(step.check,Check::Gallery)&&*world.resource::<State<VnState>>().get()==VnState::Animating{return;}
        match check(world,step) {
            Err(error)=>{finish(world,Some(error),now);},
            Ok(completed)=>{let mut run=world.resource_mut::<Run>();if completed{run.phase+=1;}run.stage=0;run.entered=now;},
        }
    }
}
