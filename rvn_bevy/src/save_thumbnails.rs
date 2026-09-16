//! Capture gameplay only, before opening an overlay. Saves reuse the last
//! matching frame; the menu itself is never photographed as its thumbnail.
use bevy::{prelude::*,render::{render_asset::RenderAssetUsages,texture::{CompressedImageFormats,ImageSampler,ImageType},view::screenshot::ScreenshotManager},window::PrimaryWindow};
use std::{collections::HashMap,sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}}};
use crate::{resources::{VnEngine,VnState,TypewriterState},project_paths::ProjectPaths,systems::{save_menu::SaveMenuState,settings_menu::SettingsMenuState}};

#[derive(Resource,Default)]
pub(crate) struct SaveThumbnails {
    latest:Arc<Mutex<Option<(String,Image)>>>,
    pending:Arc<AtomicBool>,
    pub handles:HashMap<String,Handle<Image>>,
    stamps:HashMap<String,std::time::SystemTime>,
    resume:Vec<ResumeCapture>,
}
struct ResumeCapture {slot:rvn_core::save::ResumeSlot,key:String,expected:rvn_core::save::SaveData,created:bevy::utils::Instant}
fn scene_key(engine:&VnEngine)->String{format!("{:?}",engine.0.state)}
impl SaveThumbnails {
    pub fn request_resume(&mut self,slot:rvn_core::save::ResumeSlot,engine:&VnEngine,expected:rvn_core::save::SaveData){
        self.resume.retain(|request|request.slot!=slot);self.resume.push(ResumeCapture{slot,key:scene_key(engine),expected,created:bevy::utils::Instant::now()});
    }
    fn persist_resume(&mut self,directory:&std::path::Path){
        if self.resume.is_empty(){return;}
        #[cfg(not(target_arch="wasm32"))]{
            use std::hash::{Hash,Hasher};
            let latest=self.latest.lock().ok().and_then(|s|s.clone());
            self.resume.retain(|request|{
                if request.created.elapsed().as_secs()>30{return false;}
                let Some((key,image))=&latest else{return true;};if *key!=request.key{return true;}
                let result=(||->Result<(),String>{
                    let manager=rvn_core::save::SaveManager::new(directory,crate::systems::save_menu::SUPPORTED_SLOTS).map_err(|e|e.to_string())?;
                    let mut hash=std::collections::hash_map::DefaultHasher::new();format!("{:?}",request.slot).hash(&mut hash);serde_json::to_string(&request.expected).map_err(|e|e.to_string())?.hash(&mut hash);
                    let name=format!("thumbnail_resume_{:016x}.png",hash.finish());let path=directory.join(&name);let temporary=directory.join(format!("{name}.pending.png"));
                    image.clone().try_into_dynamic().map_err(|e|e.to_string())?.thumbnail(480,270).to_rgb8().save(&temporary).map_err(|e|e.to_string())?;
                    std::fs::rename(temporary,path).map_err(|e|e.to_string())?;
                    manager.set_resume_thumbnail(request.slot,&request.expected,format!("@save/{name}")).map_err(|e|e.to_string())?;Ok(())
                })();if let Err(error)=result{warn!("[save] miniature différée : {error}");}false
            });
        }
        #[cfg(target_arch="wasm32")]{let _=directory;self.resume.clear();}
    }
    pub fn ready(&self,engine:&VnEngine)->bool{let key=scene_key(engine);self.latest.lock().ok().is_some_and(|value|value.as_ref().is_some_and(|(captured,_)|*captured==key))}
    pub fn persist(&self,engine:&VnEngine,manager:&rvn_core::save::SaveManager,directory:&std::path::Path,slot:u32)->Result<bool,String>{
        #[cfg(target_arch="wasm32")]{let _=(engine,manager,directory,slot);return Ok(false);}
        #[cfg(not(target_arch="wasm32"))]{
            let key=scene_key(engine);
            let image={let latest=self.latest.lock().map_err(|_|"Capture indisponible")?;let Some((captured,image))=&*latest else{return Ok(false)};if *captured!=key{return Ok(false)}image.clone()};
            let name=format!("thumbnail_{slot}.png");let path=directory.join(&name);let temporary=directory.join(format!("thumbnail_{slot}.pending.png"));
            let small=image.try_into_dynamic().map_err(|e|e.to_string())?.thumbnail(480,270).to_rgb8();
            small.save(&temporary).map_err(|e|e.to_string())?;
            std::fs::rename(&temporary,&path).map_err(|e|e.to_string())?;
            manager.set_thumbnail(slot,format!("@save/{name}")).map_err(|e|e.to_string())?;
            Ok(true)
        }
    }
}
pub(crate) fn capture(
    mut thumbnails:ResMut<SaveThumbnails>,engine:Res<VnEngine>,state:Res<State<VnState>>,next:Res<NextState<VnState>>,paths:Res<ProjectPaths>,
    save:Res<SaveMenuState>,settings:Res<SettingsMenuState>,typing:Res<TypewriterState>,
    windows:Query<Entity,With<PrimaryWindow>>,mut screenshots:ResMut<ScreenshotManager>,time:Res<Time>,mut previous:Local<(String,f64)>,
){
    thumbnails.persist_resume(&paths.saves);
    if *state.get()!=VnState::Waiting||matches!(*next,NextState::Pending(_))||save.active||settings.active||!typing.is_done(){return;}
    let key=scene_key(&engine);
    if key!=previous.0{*previous=(key,time.elapsed_seconds_f64());return;}
    if time.elapsed_seconds_f64()-previous.1<0.4||thumbnails.pending.load(Ordering::Acquire){return;}
    if thumbnails.latest.lock().ok().is_some_and(|s|s.as_ref().is_some_and(|(k,_)|*k==key)){return;}
    let Ok(window)=windows.get_single()else{return};let latest=thumbnails.latest.clone();let pending=thumbnails.pending.clone();
    pending.store(true,Ordering::Release);let finished=pending.clone();
    if screenshots.take_screenshot(window,move|image|{if let Ok(mut cache)=latest.lock(){*cache=Some((key,image));}finished.store(false,Ordering::Release);}).is_err(){pending.store(false,Ordering::Release);}
}
pub(crate) fn load(mut thumbnails:ResMut<SaveThumbnails>,paths:Res<ProjectPaths>,save:Res<SaveMenuState>,mut images:ResMut<Assets<Image>>){
    if !save.active{return;}
    #[cfg(not(target_arch="wasm32"))]
    if let Ok(files)=std::fs::read_dir(&paths.saves){for file in files.flatten(){
        let name=file.file_name().to_string_lossy().into_owned();
        if !name.strip_prefix("thumbnail_").and_then(|s|s.strip_suffix(".png")).is_some_and(|s|s.parse::<u32>().is_ok()||s.strip_prefix("resume_").is_some_and(|s|s.len()==16&&s.chars().all(|c|c.is_ascii_hexdigit()))){continue;}
        let key=format!("@save/{name}");let Ok(stamp)=file.metadata().and_then(|m|m.modified())else{continue};if thumbnails.stamps.get(&key)==Some(&stamp){continue;}
        let Ok(bytes)=std::fs::read(file.path())else{continue};
        if let Ok(image)=Image::from_buffer(&bytes,ImageType::Extension("png"),CompressedImageFormats::NONE,true,ImageSampler::default(),RenderAssetUsages::default()){
            let handle=images.add(image);thumbnails.handles.insert(key.clone(),handle);thumbnails.stamps.insert(key,stamp);
        }
    }}
}
