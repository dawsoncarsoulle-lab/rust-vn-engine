//! Browser decoding and audio use HTMLVideoElement. Pixels join the shared
//! Bevy stage, so scene/UI layering, clipping and menus are not DOM overlays.
use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use rvn_core::video::{Feedback, Playback};
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;
#[wasm_bindgen(inline_js = r#"
const players=new Map();
function current(player,nonce){return players.get(player.id)===player&&player.nonce===nonce;}
function dispose(player){player.events?.abort();player.video.pause();player.video.removeAttribute('src');player.video.load();player.video.remove();if(player.mask){player.mask.pause();player.mask.removeAttribute('src');player.mask.load();player.mask.remove();}for(const url of player.blobs||[])URL.revokeObjectURL(url);player.button?.remove();player.aborter.abort();}
function url(path){return new URL('assets/'+path.split('/').map(encodeURIComponent).join('/'),document.baseURI).href;}
async function validate(player,path,mask,check,nonce){
    const response=await fetch(url(path),{headers:{Range:'bytes=0-1048575'},signal:player.aborter.signal});
    if(!response.ok)throw new Error(`Video resource ${path}: HTTP ${response.status}`);
    if(Number(response.headers.get('Content-Length'))>1073741824)throw new Error('Video exceeds the 1 GiB limit');
    const reader=response.body.getReader();let parts=[],length=0;
    try{while(length<1048576){const item=await reader.read();if(item.done)break;const part=item.value.slice(0,1048576-length);parts.push(part);length+=part.length;}}finally{await reader.cancel();}
    const bytes=new Uint8Array(length);let offset=0;for(const part of parts){bytes.set(part,offset);offset+=part.length;}
    const problem=check(bytes,mask);if(problem)throw new Error(problem);
    // A basic static HTTP server may ignore byte ranges. Chromium cannot seek
    // those streams reliably. Small clips use a bounded local Blob; longer
    // videos need a range-capable server, reported as an error, not a false seek.
    if(response.status!==206&&!/bytes/i.test(response.headers.get('Accept-Ranges')||'')){
        const limit=64*1024*1024;
        const whole=await fetch(url(path),{signal:player.aborter.signal});
        if(!whole.ok)throw new Error(`Video resource ${path}: HTTP ${whole.status}`);
        if(Number(whole.headers.get('Content-Length'))>limit)throw new Error('HTTP byte-range support is required for videos above 64 MiB');
        const stream=whole.body.getReader(),chunks=[];let total=0;
        try{while(true){const item=await stream.read();if(item.done)break;total+=item.value.length;if(total>limit)throw new Error('HTTP byte-range support is required for videos above 64 MiB');chunks.push(item.value);}}finally{await stream.cancel();}
        if(!current(player,nonce))return;
        const local=URL.createObjectURL(new Blob(chunks,{type:'video/webm'}));player.blobs.push(local);(mask?player.mask:player.video).src=local;
    }
}
function media(path,muted){const video=document.createElement('video');video.preload='auto';video.playsInline=true;video.muted=muted;video.style.display='none';video.src=url(path);document.body.appendChild(video);return video;}
function resumeButton(player){
    if(player.button)return;
    const button=document.createElement('button');button.textContent=player.resumeLabel;button.setAttribute('aria-label',player.resumeLabel);button.style.cssText='position:fixed;left:50%;top:45%;transform:translate(-50%,-50%);padding:16px 24px;background:#12344d;color:white;border:2px solid #4ccaff;border-radius:8px;z-index:20000;font:18px sans-serif;cursor:pointer;';
    const nonce=player.nonce;
    button.addEventListener('click',event=>{event.stopPropagation();if(!current(player,nonce))return;player.blocked=false;player.ended=false;player.failed=false;player.notice={kind:'resume',epoch:player.epoch,nonce};begin(player);});document.body.appendChild(button);player.button=button;
}
function begin(player){
    if(!player.validated||player.starting||player.failed||player.ended||player.wanted!==null)return;
    const nonce=player.nonce;
    player.starting=true;
    Promise.all([player.video.play(),...(player.mask?[player.mask.play()]:[])]).then(()=>{if(!current(player,nonce))return;player.starting=false;player.blocked=false;player.button?.remove();player.button=null;}).catch(error=>{
        if(!current(player,nonce))return;player.starting=false;player.video.pause();player.mask?.pause();
        if(error.name==='NotAllowedError'){
            player.blocked=true;player.notice={kind:'blocked',message:'The browser requires a click to start video audio',epoch:player.epoch,nonce:player.nonce};
            resumeButton(player);
        }else if(error.name!=='AbortError'){player.failed=true;player.notice={kind:'error',message:String(error.message).slice(0,4000),epoch:player.epoch,nonce:player.nonce};}
    });
}
function settleSeek(player,nonce){
    if(!current(player,nonce)||player.wanted===null||player.video.seeking||player.mask?.seeking)return;
    if(Math.abs(player.video.currentTime-player.wanted)>0.001||(player.mask&&Math.abs(player.mask.currentTime-player.wanted)>0.001))return;
    player.wanted=null;player.seekNonce=null;
}
function bindProjection(player,check){
    const nonce=player.nonce;
    player.events?.abort();player.events=new AbortController();const options={signal:player.events.signal};
    const fail=()=>{if(current(player,nonce)&&player.playback==='playing'&&(player.video.error||player.mask?.error)){player.failed=true;player.notice={kind:'error',message:'Browser video resource or codec could not be loaded',epoch:player.epoch,nonce};}};
    player.video.addEventListener('error',fail,options);player.mask?.addEventListener('error',fail,options);
    player.video.addEventListener('ended',()=>{const video=player.video;if(current(player,nonce)&&player.playback==='playing'&&!video.loop&&video.ended&&!video.seeking&&player.wanted===null&&Number.isFinite(video.duration)&&Math.abs(video.currentTime-video.duration)<=0.1){player.ended=true;player.notice={kind:'end',epoch:player.epoch,nonce};}},options);
    const seeked=()=>{if(player.seekNonce===nonce)settleSeek(player,nonce);};
    player.video.addEventListener('seeked',seeked,options);player.mask?.addEventListener('seeked',seeked,options);
    if(!player.validated){
        player.aborter.abort();player.aborter=new AbortController();
        Promise.all([validate(player,player.source,false,check,nonce),...(player.mask?[validate(player,player.maskSource,true,check,nonce)]:[])]).then(()=>{if(current(player,nonce))player.validated=true;}).catch(error=>{if(error.name!=='AbortError'&&current(player,nonce)){player.failed=true;player.notice={kind:'error',message:String(error.message).slice(0,4000),epoch:player.epoch,nonce};}});
    }
}
export function rvn_video_sync(description,active,volume,resumeLabel,check){
    const views=JSON.parse(description),seen=new Set();
    for(const view of views){
        seen.add(view.id);let player=players.get(view.id);
        if(player&&(player.source!==view.source||player.maskSource!==view.mask||player.identity!==view.identity)){dispose(player);players.delete(view.id);player=null;}
        if(!player){
            const video=media(view.source,false),mask=view.mask?media(view.mask,true):null;
            const canvas=document.createElement('canvas'),context=canvas.getContext('2d',{willReadFrequently:true});
            player={id:view.id,identity:view.identity,source:view.source,maskSource:view.mask,video,mask,canvas,context,aborter:new AbortController(),events:null,blobs:[],epoch:view.epoch,nonce:view.nonce,playback:view.playback,lastReport:view.position,wanted:view.position,seekNonce:null,validated:false,notice:null,failed:view.playback==='failed',ended:view.playback==='ended',blocked:view.playback==='blocked',starting:false,button:null,resumeLabel};players.set(view.id,player);
            bindProjection(player,check);
        }else if(player.nonce!==view.nonce){
            // Keep the user-authorized element for renewal of the same exact
            // resource. Every asynchronous completion still belongs to the
            // nonce captured when that operation was started.
            player.nonce=view.nonce;player.epoch=view.epoch;player.notice=null;player.starting=false;player.seekNonce=null;player.wanted=view.position;player.lastReport=view.position;
            player.failed=view.playback==='failed';player.ended=view.playback==='ended';player.blocked=view.playback==='blocked';
            player.button?.remove();player.button=null;bindProjection(player,check);
        }
        if(player.playback==='blocked'&&view.playback==='playing'){player.blocked=false;}
        player.playback=view.playback;player.epoch=view.epoch;player.resumeLabel=resumeLabel;player.video.loop=view.looping;if(player.mask)player.mask.loop=view.looping;
        player.video.volume=Math.max(0,Math.min(1,view.volume*volume));
        if(Math.abs(view.position-player.lastReport)>0.000001){player.wanted=view.position;player.ended=false;player.failed=false;player.notice=null;player.lastReport=view.position;}
        if(player.validated&&player.video.readyState>=1&&(!player.mask||player.mask.readyState>=1)&&player.wanted!==null&&player.seekNonce===null){
            if(!player.video.seeking&&!player.mask?.seeking&&Math.abs(player.video.currentTime-player.wanted)<=0.001&&(!player.mask||Math.abs(player.mask.currentTime-player.wanted)<=0.001))settleSeek(player,player.nonce);
            else {player.seekNonce=player.nonce;player.video.currentTime=player.wanted;if(player.mask)player.mask.currentTime=player.wanted;}
        }
        const admitted=typeof view.active==='boolean'?view.active:active;
        const running=admitted&&!document.hidden&&view.playback==='playing';
        if(view.playback==='blocked'){player.blocked=true;resumeButton(player);}
        if(player.button){player.button.hidden=!admitted||document.hidden||view.playback!=='blocked';player.button.textContent=resumeLabel;player.button.setAttribute('aria-label',resumeLabel);}
        if(!running){player.video.pause();player.mask?.pause();if(view.playback!=='blocked'){player.button?.remove();player.button=null;}}
        else if(player.video.paused&&!player.blocked){begin(player);}
        if(player.mask?.readyState>=1&&Math.abs(player.mask.currentTime-player.video.currentTime)>0.05){player.mask.currentTime=player.video.currentTime;}
    }
    for(const [id,player]of players){if(!seen.has(id)){dispose(player);players.delete(id);}}
}
export function rvn_video_reports(){
    const output=[];
    for(const player of players.values()){
        const video=player.video;
        if(player.validated&&video.readyState>=1){
            if(!Number.isFinite(video.duration)||video.duration<=0||video.duration>86400||video.videoWidth>3840||video.videoHeight>2160){player.failed=true;player.notice={kind:'error',message:'Video dimensions or duration exceed the runtime limits',epoch:player.epoch,nonce:player.nonce};}
            else if(player.mask?.readyState>=1&&(!Number.isFinite(player.mask.duration)||player.mask.duration+0.1<video.duration)){player.failed=true;player.notice={kind:'error',message:'Video mask ends before the video',epoch:player.epoch,nonce:player.nonce};}
            else if(!player.failed&&player.wanted===null&&!video.seeking&&!player.mask?.seeking){player.lastReport=video.currentTime;output.push({id:player.id,epoch:player.epoch,nonce:player.nonce,kind:player.ended?'metadata':'position',seconds:video.currentTime,duration:video.duration,width:video.videoWidth,height:video.videoHeight});}
        }
        if(player.notice){output.push({id:player.id,...player.notice});}
    }return JSON.stringify(output);
}
export function rvn_video_ack(id,nonce,kind){
    const player=players.get(id);
    if(player&&player.nonce===nonce&&player.notice?.nonce===nonce&&player.notice.kind===kind)player.notice=null;
}
export function rvn_video_pixels(id,nonce){
    const player=players.get(id);if(!player||player.nonce!==nonce||!player.validated||player.video.readyState<2||player.video.seeking||player.wanted!==null)return new Uint8Array();
    const video=player.video,w=video.videoWidth,h=video.videoHeight;if(!w||!h||w>3840||h>2160)throw new Error('Invalid browser video dimensions');
    player.canvas.width=w;player.canvas.height=h;player.context.drawImage(video,0,0,w,h);
    const pixels=player.context.getImageData(0,0,w,h).data;
    if(player.mask){const mask=player.mask;if(mask.readyState<2||mask.seeking)return new Uint8Array();if(mask.videoWidth!==w||mask.videoHeight!==h)throw new Error('Video mask dimensions must match the video');player.context.drawImage(mask,0,0,w,h);const alpha=player.context.getImageData(0,0,w,h).data;for(let i=0;i<pixels.length;i+=4)pixels[i+3]=Math.round((54*alpha[i]+183*alpha[i+1]+19*alpha[i+2])/256);}
    return new Uint8Array(pixels.buffer);
}
document.addEventListener('visibilitychange',()=>{if(document.hidden)for(const player of players.values()){player.video.pause();player.mask?.pause();}});
export function rvn_video_retire(description){
    const views=JSON.parse(description);
    for(const [id,player]of players){const view=views.find(view=>view.id===id);if(!view||view.identity!==player.identity){dispose(player);players.delete(id);}else if(view.nonce!==player.nonce){player.video.pause();player.mask?.pause();player.events?.abort();player.button?.remove();player.button=null;}}
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    fn rvn_video_sync(
        description: &str,
        active: bool,
        volume: f32,
        resume_label: &str,
        check: &js_sys::Function,
    ) -> Result<(), JsValue>;
    #[wasm_bindgen(catch)]
    fn rvn_video_reports() -> Result<String, JsValue>;
    fn rvn_video_ack(id:&str,nonce:&str,kind:&str);
    #[wasm_bindgen(catch)]
    fn rvn_video_pixels(id: &str, nonce: &str) -> Result<Vec<u8>, JsValue>;
    #[wasm_bindgen(catch)]
    fn rvn_video_retire(description: &str) -> Result<(), JsValue>;
}
pub struct WebVideoPlugin;
impl Plugin for WebVideoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Textures>().init_resource::<Projections>()
            .add_systems(
                PostUpdate,
                crate::video_visual::cinematic_visibility
                    .after(bevy::transform::TransformSystem::TransformPropagate)
                    .before(bevy::render::view::VisibilitySystems::VisibilityPropagate),
            )
            .add_systems(
                Update,
                (
                    tick.run_if(crate::scene_publication::ready).in_set(crate::scene_publication::HostClockSet),
                    crate::video_visual::cinematic_input.run_if(crate::scene_publication::ready)
                        .before(crate::systems::player_input_system),
                ),
            );
    }
}
#[derive(Resource, Default)]
struct Textures(BTreeMap<String, Texture>);
#[derive(Resource,Default)]
struct Projections(crate::video_visual::VideoProjections);
pub(crate) fn install_scene(world:&mut World,views:Vec<rvn_core::video::VideoView>) {
    if !world.contains_resource::<Projections>() {world.init_resource::<Projections>();}
    if let Err(error)=world.resource_mut::<Projections>().0.reconcile(&views) {fail(world,error);return;}
    let identities:Vec<_>=views.iter().map(|view|serde_json::json!({"id":view.id,"nonce":world.resource::<Projections>().0.nonce(view),"identity":world.resource::<Projections>().0.resource_nonce(view)})).collect();
    if let Err(error)=rvn_video_retire(&serde_json::to_string(&identities).unwrap()) {fail(world,error);}
    if let Some(mut textures) = world.remove_resource::<Textures>() {
        for view in &views {
            if !crate::video_visual::projection_visible(view) {
                if let Some(texture) = textures.0.get_mut(&view.id) {
                    crate::video_visual::retire_visual(world, &mut texture.entity, &mut texture.caption);
                }
            }
        }
        world.insert_resource(textures);
    }
}
struct Texture {
    image: Handle<Image>,
    entity: Option<Entity>,
    caption: Option<Entity>,
    clip: rvn_ui::video::VideoClip,
    nonce: String,
    width: u32,
    height: u32,
}
#[derive(Resource,Default)]
struct PendingFrameErrors(BTreeMap<String,(rvn_core::video::VideoView,crate::scene_publication::PendingVideoTerminal)>);
fn retain_frame_error(world:&mut World,pending:&mut PendingFrameErrors,view:&rvn_core::video::VideoView,error:String) {
    let (_,terminal)=pending.0.entry(view.id.clone()).or_insert_with(||(view.clone(),default()));
    terminal.retain(Feedback::Error(error));terminal.submit(world,view);
}
#[derive(serde::Deserialize)]
struct Report {
    id: String,
    epoch: String,
    nonce: String,
    kind: String,
    seconds: Option<f64>,
    duration: Option<f64>,
    width: Option<u32>,
    height: Option<u32>,
    message: Option<String>,
}
fn fail(world: &mut World, error: impl std::fmt::Debug) {
    world
        .resource_mut::<crate::resources::ScriptErrorMessage>()
        .0 = format!("Browser video: {error:?}");
    world
        .resource_mut::<NextState<crate::resources::VnState>>()
        .set(crate::resources::VnState::Error);
}
fn tick(world: &mut World) {
    use crate::resources::{VnEngine, VnState};
    let views = match world.resource::<VnEngine>().0.video_views() {
        Ok(views) => views,
        Err(error) => {
            fail(world, error);
            return;
        }
    };
    let active = if world.resource::<VnEngine>().0.state.scene_execution.is_some() {crate::scene_publication::game_active(world)} else {!world
        .resource::<crate::accessibility::Accessibility>()
        .blocked
        && matches!(
            world.resource::<State<VnState>>().get(),
            VnState::Stepping | VnState::Waiting | VnState::Animating
        )};
    let label = world
        .resource::<VnEngine>()
        .0
        .locale
        .as_ref()
        .map(|locale| {
            if locale.current_lang() == "fr" {
                "Cliquer pour lire la vidéo"
            } else {
                "Click to play video"
            }
        })
        .unwrap_or("Click to play video");
    if let Err(error)=world.resource_mut::<Projections>().0.reconcile(&views) {fail(world,error);return;}
    let descriptions:Vec<_>=views.iter().map(|view|serde_json::json!({"id":view.id,"epoch":view.epoch.to_string(),"nonce":world.resource::<Projections>().0.nonce(view),"identity":world.resource::<Projections>().0.resource_nonce(view),"source":view.track.clip.source,"mask":view.track.clip.mask,"position":view.track.position,"playback":view.track.playback,"looping":view.track.clip.looping,"volume":view.track.clip.volume,"active":if world.resource::<VnEngine>().0.state.scene_execution.is_some(){crate::scene_publication::video_clock_allowed(world,view)}else{active}})).collect();
    thread_local! {static CHECK:Closure<dyn Fn(Vec<u8>,bool)->String>=Closure::new(|data:Vec<u8>,mask:bool|rvn_ui::webm::validate_header(&data,mask).err().unwrap_or_default());}
    let result = CHECK.with(|check| {
        rvn_video_sync(
            &serde_json::to_string(&descriptions).unwrap(),
            active,
            world
                .resource::<crate::systems::settings_menu::Settings>()
                .sfx_volume,
            label,
            check.as_ref().unchecked_ref(),
        )
    });
    if let Err(error) = result {
        fail(world, error);
        return;
    }
    let reports = match rvn_video_reports()
        .map_err(|error| format!("{error:?}"))
        .and_then(|text| {
            serde_json::from_str::<Vec<Report>>(&text).map_err(|error| error.to_string())
        }) {
        Ok(reports) => reports,
        Err(error) => {
            fail(world, error);
            return;
        }
    };
    let mut textures = world.remove_resource::<Textures>().unwrap();
    let mut pending_frames=world.remove_resource::<PendingFrameErrors>().unwrap_or_default();
    pending_frames.0.retain(|id,(old,_)|views.iter().any(|view|view.id==*id && old.epoch==view.epoch && old.resource_token==view.resource_token));
    for (view,terminal) in pending_frames.0.values_mut(){terminal.submit(world,view);}
    let obsolete: Vec<_> = textures
        .0
        .keys()
        .filter(|id| !views.iter().any(|view| view.id == **id))
        .cloned()
        .collect();
    for id in obsolete {
        if let Some(texture) = textures.0.remove(&id) {
            for entity in [texture.entity, texture.caption].into_iter().flatten() {
                if let Some(entity) = world.get_entity_mut(entity) {
                    entity.despawn_recursive();
                }
            }
        }
    }
    for view in &views {
        let nonce=world.resource::<Projections>().0.nonce(view).unwrap();
        let replace = textures.0.get(&view.id).is_some_and(|texture| {
            let mut old = texture.clip.clone();
            old.volume = view.track.clip.volume;
            old != view.track.clip || texture.nonce!=nonce
        });
        if replace {
            if let Some(texture) = textures.0.remove(&view.id) {
                for entity in [texture.entity, texture.caption].into_iter().flatten() {
                    if let Some(entity) = world.get_entity_mut(entity) {
                        entity.despawn_recursive();
                    }
                }
            }
        }
        textures.0.entry(view.id.clone()).or_insert_with(|| {
            let mut image = Image::new_fill(
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[0, 0, 0, 255],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            image.sampler = bevy::render::texture::ImageSampler::linear();
            Texture {
                image: world.resource_mut::<Assets<Image>>().add(image),
                entity: None,
                caption: None,
                clip: view.track.clip.clone(),
                nonce,
                width: 0,
                height: 0,
            }
        });
    }
    for report in reports {
        let Some(view)=views.iter().find(|view|view.id==report.id && world.resource::<Projections>().0.accepts(view,&report.epoch,&report.nonce)) else {continue;};
        // A one-shot Rust pixel error is held by its exact capability too.
        // Later DOM positions/EOF cannot replace it while admission is blocked.
        if pending_frames.0.contains_key(&view.id){continue;}
        if matches!(report.kind.as_str(), "position" | "metadata") {
            if let Some(image) = textures.0.get_mut(&report.id) {
                image.width = report.width.unwrap_or(0);
                image.height = report.height.unwrap_or(0);
            }
        }
        let feedback = match report.kind.as_str() {
            "position" => Some(Feedback::Position {
                seconds: report.seconds.unwrap_or(f64::NAN),
                duration: report.duration.unwrap_or(f64::NAN),
            }),
            "end" => Some(Feedback::End),
            "error" => Some(Feedback::Error(
                report
                    .message
                    .unwrap_or_else(|| "Unknown browser video failure".into()),
            )),
            "blocked" => Some(Feedback::AutoplayBlocked(
                report
                    .message
                    .unwrap_or_else(|| "Click to start video".into()),
            )),
            "resume" => {
                if crate::scene_publication::queue_video_resume(world,view) {rvn_video_ack(&report.id,&report.nonce,&report.kind);}
                None
            }
            _ => None,
        };
        if let Some(feedback) = feedback {
            if crate::scene_publication::queue_video_feedback(world,view,feedback) {rvn_video_ack(&report.id,&report.nonce,&report.kind);}
        }
    }
    for view in &views {
        if let Some(texture) = textures.0.get_mut(&view.id) {
            if !crate::video_visual::projection_visible(view) {
                crate::video_visual::retire_visual(world, &mut texture.entity, &mut texture.caption);
                continue;
            }
            if texture
                .entity
                .is_some_and(|entity| world.get_entity(entity).is_none())
            {
                texture.entity = None;
                texture.caption = None;
            }
            if texture.entity.is_none() {
                if let Some((entity, caption)) =
                    crate::video_visual::create(world, view, texture.image.clone())
                {
                    texture.entity = Some(entity);
                    texture.caption = Some(caption);
                }
            }
            if let Some(entity) = texture.entity {
                if !pending_frames.0.contains_key(&view.id) {
                match rvn_video_pixels(&view.id, &texture.nonce) {
                    Ok(pixels) if !pixels.is_empty() => {
                        if pixels.len() != texture.width as usize * texture.height as usize * 4 {
                            retain_frame_error(world,&mut pending_frames,view,"Browser video returned an invalid RGBA buffer".into());
                        } else {
                            if let Some(image) = world
                                .resource_mut::<Assets<Image>>()
                                .get_mut(&texture.image)
                            {
                                image.resize(Extent3d {
                                    width: texture.width,
                                    height: texture.height,
                                    depth_or_array_layers: 1,
                                });
                                image.data = pixels;
                            }
                            crate::video_visual::set_texture(world, entity, texture.image.clone());
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        retain_frame_error(world,&mut pending_frames,view,format!("Browser video frame: {error:?}"));
                    }
                }
                }
                if let Some(caption) = texture.caption {
                    let subtitle = world
                        .resource::<VnEngine>()
                        .0
                        .video_views()
                        .ok()
                        .and_then(|views| views.into_iter().find(|other| other.id == view.id))
                        .and_then(|view| view.subtitle)
                        .unwrap_or_default();
                    crate::video_visual::caption(world, caption, &subtitle);
                }
                if let Some(mut visible) = world.get_mut::<Visibility>(entity) {
                    *visible = if view.track.finished()
                        && !view.track.clip.keep_last_frame
                        && view.track.clip.fallback.is_none()
                    {
                        Visibility::Hidden
                    } else {
                        Visibility::Inherited
                    };
                }
                if view.track.playback == Playback::Failed {
                    if let Some(path) = &view.track.clip.fallback {
                        let image = world.resource::<AssetServer>().load::<Image>(path.clone());
                        crate::video_visual::set_texture(world, entity, image);
                    }
                }
            }
        }
    }
    world.insert_resource(textures);
    world.insert_resource(pending_frames);
    for command in world.resource_mut::<VnEngine>().0.renderer.take_pending() {
        world.send_event(command);
    }
}
