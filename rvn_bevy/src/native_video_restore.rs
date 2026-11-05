use super::*;
use std::{path::{Path,PathBuf},sync::{atomic::{AtomicBool,AtomicU64,Ordering},mpsc},io::{Read,Write}};
use sha2::{Digest,Sha256};
const DECODED_LIMIT:usize=256*1024*1024;
const INPUT_LIMIT:u64=1024*1024*1024;
static NEXT_INPUT:AtomicU64=AtomicU64::new(1);
struct OwnedInput{path:PathBuf}
impl Drop for OwnedInput{fn drop(&mut self){
    let temp=std::env::temp_dir();
    if self.path.parent()==Some(temp.as_path())&&self.path.file_name().is_some_and(|p|p.to_string_lossy().starts_with(&format!("rvn-video-stage-0376-{}-",std::process::id()))){let _=std::fs::remove_file(&self.path);}
}}
struct InputReceipt{locator:String,original:PathBuf,digest:[u8;32],bytes:u64,owned:Arc<OwnedInput>}
struct StagedPlayer{view:VideoView,worker:Option<Worker>,metadata:Option<Metadata>,frames:VecDeque<Frame>,mask:Option<Mask>,audio:Option<VideoAudio>,eof:bool,inputs:Vec<InputReceipt>,exact:Option<rvn_media::checkpoint::NativeCheckpoint>,picture_ready:bool,sound_ready:bool,decoded_source:Option<([u8;32],rvn_media::checkpoint::DecodedSourceProfile)>}
#[derive(Clone)]
struct LiveInputFact{locator:String,original:PathBuf,digest:[u8;32],bytes:u64}
#[derive(Clone)]
struct RetainedPlayer{before:VideoView,authority:Option<crate::video_audio::VideoBufferAuthority>,view:VideoView,image:Handle<Image>,audio:Option<VideoAudio>,inputs:Vec<LiveInputFact>}
enum NativeSource{Game(rvn_core::engine::DebugStateSource),Startup(rvn_core::engine::StartupPublicationSource)}
pub(crate) struct PendingRestore{
    source:NativeSource,key:[u8;32],root:PathBuf,retained:Vec<RetainedPlayer>,cancel:Arc<AtomicBool>,receiver:mpsc::Receiver<Result<Vec<StagedPlayer>,String>>,
}
impl Drop for PendingRestore{fn drop(&mut self){self.cancel.store(true,Ordering::Release);}}
pub(crate) struct PreparedNativeRestore{
    source:NativeSource,key:[u8;32],root:PathBuf,players:Vec<StagedPlayer>,outgoing:Vec<VideoAudio>,instance:u64,gate:crate::checkpoint_pcm::CheckpointGate,
    runtime:Option<Arc<()>>,retained:Vec<RetainedPlayer>,retirement:Option<crate::runtime_restart::RetirementSource>,
}
#[cfg(test)]
pub(super) fn pending_cancel_for_test(runtime:&Players)->Option<std::sync::Weak<AtomicBool>>{runtime.restore_staging.lock().unwrap().as_ref().map(|p|Arc::downgrade(&p.cancel))}
fn key(core:&rvn_core::engine::PreparedSave15)->Result<[u8;32],String>{batch_key(core.render_batch(),core.native_media_checkpoint())}
fn batch_key(batch:&rvn_core::renderer::SceneRenderBatch,native:Option<&[rvn_media::checkpoint::NativeCheckpoint]>)->Result<[u8;32],String>{
    struct Writer{hash:Sha256,bytes:usize,depth:usize,nodes:usize,string:bool,escape:bool,token:bool}
    impl Writer{fn new()->Self{Self{hash:Sha256::new(),bytes:0,depth:0,nodes:0,string:false,escape:false,token:false}}}
    impl Write for Writer{fn write(&mut self,value:&[u8])->std::io::Result<usize>{
        self.bytes=self.bytes.checked_add(value.len()).filter(|n|*n<=16*1024*1024).ok_or_else(||std::io::Error::other("Native staging key exceeds Core bound"))?;
        for byte in value{if self.string{if self.escape{self.escape=false;}else if *byte==b'\\'{self.escape=true;}else if *byte==b'"'{self.string=false;}continue;}
            match byte{b'"'=>{self.string=true;self.token=false;self.nodes+=1;},b'{'|b'['=>{self.depth+=1;self.nodes+=1;self.token=false;if self.depth>256{return Err(std::io::Error::other("Native staging key depth exceeds256 before tree allocation"));}},b'}'|b']'=>{self.depth=self.depth.checked_sub(1).ok_or_else(||std::io::Error::other("Native key container mismatch"))?;self.token=false;},b','|b':'|b' '|b'\t'|b'\n'|b'\r'=>self.token=false,_=>{if !self.token{self.nodes+=1;self.token=true;}}}
            if self.nodes>1_048_576{return Err(std::io::Error::other("Native staging key node bound before tree allocation"));}
        }
        self.hash.update(value);Ok(value.len())
    }fn flush(&mut self)->std::io::Result<()>{Ok(())}}
    fn canonical(value:&mut serde_json::Value,depth:usize,nodes:&mut usize)->Result<(),String>{
        *nodes=nodes.checked_add(1).filter(|n|*n<=1_048_576).ok_or("Native canonical key exceeds its node bound")?;
        if depth>256{return Err("Native canonical key exceeds its depth bound".into());}
        match value{serde_json::Value::Object(object)=>{
            let mut entries:Vec<_>=std::mem::take(object).into_iter().collect();entries.sort_unstable_by(|a,b|a.0.cmp(&b.0));
            for(key,mut value)in entries{canonical(&mut value,depth+1,nodes)?;object.insert(key,value);}
        },serde_json::Value::Array(items)=>for item in items{canonical(item,depth+1,nodes)?;},_=>{}}Ok(())
    }
    fn write_canonical<T:serde::Serialize>(writer:&mut Writer,payload:&T)->Result<(),String>{
        // Check the serialized allocation bound before constructing a JSON tree.
        // Actual output is bounded again across the whole native key below.
        let mut bound=Writer::new();serde_json::to_writer(&mut bound,payload).map_err(|e|e.to_string())?;
        let mut value=serde_json::to_value(payload).map_err(|e|e.to_string())?;canonical(&mut value,0,&mut 0)?;
        serde_json::to_writer(writer,&value).map_err(|e|e.to_string())
    }
    let mut writer=Writer::new();writer.hash.update(b"rvn.native-restore.0376\0");
    write_canonical(&mut writer,&native)?;
    write_canonical(&mut writer,&(&batch.state,batch.runtime_epoch,batch.interface_epoch,batch.video_epoch))?;
    for view in &batch.videos{
        let token=view.resource_token.as_ref().map(|t|(t.task.runtime_epoch,&t.task.key,t.task.revision,&t.target));
        let projection=view.ui_projection.as_ref().map(|p|(&p.interface,&p.element,p.order,p.host_role.map(|r|r.id()),matches!(p.residence,rvn_core::ui_residency::ProjectionResidence::Visible)));
        write_canonical(&mut writer,&(&view.id,view.epoch,token,projection,&view.track,&view.subtitle))?;
    }
    Ok(writer.hash.finalize().into())
}
fn snapshot(root:&Path,locator:&str,cancel:&AtomicBool,total:&mut u64)->Result<InputReceipt,String>{
    let original=safe_path(root,locator)?;let meta=std::fs::metadata(&original).map_err(|e|e.to_string())?;
    if !meta.is_file()||meta.len()>INPUT_LIMIT{return Err("Native staged input is not a bounded regular file".into());}
    *total=total.checked_add(meta.len()).filter(|bytes|*bytes<=INPUT_LIMIT).ok_or("Native staged input aggregate exceeds1GiB")?;
    let nonce=NEXT_INPUT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|v|v.checked_add(1)).map_err(|_|"Native input identities exhausted")?;
    let path=std::env::temp_dir().join(format!("rvn-video-stage-0376-{}-{nonce}.webm",std::process::id()));
    let mut input=std::fs::File::open(&original).map_err(|e|e.to_string())?;
    let file=std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e|e.to_string())?;
    // Ownership exists only after create_new succeeds. Output is declared last
    // so its Windows handle closes before cleanup on every copy/cancel error.
    let owned=Arc::new(OwnedInput{path:path.clone()});let mut output=file;
    let mut digest=Sha256::new();let mut read=0u64;let mut block=[0u8;64*1024];
    loop{if cancel.load(Ordering::Acquire){return Err("Native staging cancelled".into());}let count=input.read(&mut block).map_err(|e|e.to_string())?;if count==0{break;}read=read.checked_add(count as u64).filter(|n|*n<=meta.len()).ok_or("Native input grew during snapshot")?;digest.update(&block[..count]);output.write_all(&block[..count]).map_err(|e|e.to_string())?;}
    if read!=meta.len(){return Err("Native input changed during snapshot".into());}output.flush().map_err(|e|e.to_string())?;drop(output);
    Ok(InputReceipt{locator:locator.into(),original,digest:digest.finalize().into(),bytes:read,owned})
}
fn decoded_bytes(players:&[StagedPlayer])->Result<usize,String>{
    let mut total=0usize;for p in players{for f in &p.frames{total=total.checked_add(f.rgba.len()).ok_or("Native decoded budget overflow")?;}if let Some(mask)=&p.mask{for f in &mask.frames{total=total.checked_add(f.rgba.len()).ok_or("Native mask budget overflow")?;}}if let Some(audio)=&p.audio{total=total.checked_add(audio.0.lock().map_err(|_|"Native staged PCM poisoned")?.len()*std::mem::size_of::<f32>()).ok_or("Native PCM budget overflow")?;}}if total>DECODED_LIMIT{return Err("Native staged decoded aggregate exceeds256MiB".into());}Ok(total)
}
fn frame(seconds:f64,width:u32,height:u32,rgba:Vec<u8>)->Result<Frame,String>{
    if !seconds.is_finite()||!(0.0..=86_400.0).contains(&seconds)||width==0||height==0||width>3840||height>2160||rgba.len()!=width as usize*height as usize*4{return Err("Invalid decoded native candidate frame".into());}Ok(Frame{seconds,width,height,rgba,proof:None})
}
fn pump_staged(p:&mut StagedPlayer)->Result<bool,String>{
    if let Some(mask)=&mut p.mask{if !mask.eof&&mask.frames.is_empty(){if let Some((packet,proof))=mask.worker.as_ref().ok_or("Native mask worker absent")?.poll_proven()?{match packet{
        Packet::Ready(meta)=>{
            if mask.exact.is_some()&&!matches!(proof,Some(DecodedProof::Ready{..})){return Err("Native exact mask lacks source provenance".into());}
            if let Some(DecodedProof::Ready{asset_sha256,profile})=proof{if let Some(expected)=&mask.exact{if asset_sha256!=expected.asset_sha256||profile!=expected.profile{return Err("Native exact mask asset/profile differs".into());}}mask.decoded_source=Some((asset_sha256,profile));}
            mask.metadata=Some(meta);
        },
        Packet::Data(Chunk::Frame{seconds,width,height,rgba})=>{
            let mut decoded=frame(seconds,width,height,rgba)?;
            if let Some(expected)=&mask.exact{let Some(DecodedProof::Video{ordinal,pts,time_base})=proof else{return Err("Native mask frame lacks exact ordinal/PTS".into());};if ordinal>expected.picture.ordinal{return Err("Native mask pre-roll passed saved frame".into());}if ordinal==expected.picture.ordinal{let actual=rvn_media::checkpoint::PictureCursor{ordinal,pts,time_base,rgba_sha256:Sha256::digest(&decoded.rgba).into()};if actual!=expected.picture{return Err("Native mask frame bytes/PTS differ".into());}decoded.proof=Some(actual);mask.frames.push_back(decoded);}}
            else{if let Some(DecodedProof::Video{ordinal,pts,time_base})=proof{decoded.proof=Some(rvn_media::checkpoint::PictureCursor{ordinal,pts,time_base,rgba_sha256:Sha256::digest(&decoded.rgba).into()});}mask.frames.push_back(decoded);}
        },
        Packet::Data(Chunk::Audio{..})=>{},Packet::Data(Chunk::End)=>{mask.eof=true;mask.worker=None;if mask.exact.is_some()&&mask.frames.is_empty(){return Err("Native exact mask ended before saved picture".into());}},Packet::Error(e)=>return Err(format!("Native staged mask: {e}")),
    }}}}
    let ready_picture=(p.exact.is_none()||p.picture_ready)&&!p.frames.is_empty()||p.view.track.playback==Playback::Failed||(!p.view.track.clip.keep_last_frame&&p.view.track.finished());
    let ready_sound=(p.exact.is_none()||p.sound_ready)&&(p.metadata.as_ref().is_some_and(|m|!m.audio||p.view.track.finished())||p.audio.as_ref().is_some_and(|a|a.0.lock().is_ok_and(|b|b.len()>0||b.ended)));
    let ready_mask=p.mask.as_ref().is_none_or(|m|m.metadata.is_some()&&(!m.frames.is_empty()||m.eof));
    if ready_picture&&ready_sound&&ready_mask&&(!p.view.track.finished()||p.eof){
        if let(Some(meta),Some(mask))=(&p.metadata,&p.mask){if let Some(other)=&mask.metadata{if (meta.width,meta.height)!=(other.width,other.height)||meta.duration.zip(other.duration).is_some_and(|(a,b)|b+0.1<a){return Err("Native staged mask metadata disagrees with source".into());}}}return Ok(true);
    }
    if p.eof{return Err("Native candidate ended before ready frame/PCM".into());}
    if let Some((packet,proof))=p.worker.as_ref().ok_or("Native candidate worker absent")?.poll_proven()?{match packet{
        Packet::Ready(meta)=>{
            if p.exact.is_some()&&!matches!(proof,Some(DecodedProof::Ready{..})){return Err("Exact native decoder did not provide provenance".into());}
            if let Some(DecodedProof::Ready{asset_sha256,profile})=proof{
                if let Some(expected)=&p.exact{if asset_sha256!=expected.asset_sha256||profile!=expected.profile||(meta.audio&&!p.view.track.finished())!=expected.audio.is_some(){return Err("Exact native asset/stream/profile differs from saved reader".into());}}
                if let Some(audio)=&p.audio{audio.0.lock().map_err(|_|"Native PCM poisoned")?.bind_exact(asset_sha256,profile.clone())?;}
                p.decoded_source=Some((asset_sha256,profile));p.sound_ready=!meta.audio||p.view.track.finished();
            }
            if meta.duration.is_none()||meta.width==0||meta.height==0{return Err("Native candidate has no usable metadata".into());}if !meta.audio||p.view.track.finished(){p.audio=None;}p.metadata=Some(meta);},
        Packet::Data(Chunk::Frame{seconds,width,height,rgba})=>{let mut next=frame(seconds,width,height,rgba)?;
            if let Some(expected)=&p.exact{let Some(DecodedProof::Video{ordinal,pts,time_base})=proof else{return Err("Native frame has no exact ordinal/PTS".into());};if ordinal>expected.picture.ordinal&&!p.picture_ready{return Err("Native pre-roll passed saved frame".into());}if ordinal==expected.picture.ordinal{let actual=rvn_media::checkpoint::PictureCursor{ordinal,pts,time_base,rgba_sha256:Sha256::digest(&next.rgba).into()};if actual!=expected.picture{return Err("Native frame bytes/PTS differ at saved ordinal".into());}next.proof=Some(actual);p.frames.clear();p.frames.push_back(next);p.picture_ready=true;}}
            else{if let Some(DecodedProof::Video{ordinal,pts,time_base})=proof{next.proof=Some(rvn_media::checkpoint::PictureCursor{ordinal,pts,time_base,rgba_sha256:Sha256::digest(&next.rgba).into()});}if p.frames.len()>=3||p.view.track.finished(){p.frames.clear();}p.frames.push_back(next);}},
        Packet::Data(Chunk::Audio{seconds,samples})=>{if let Some(audio)=&p.audio{let mut buffer=audio.0.lock().map_err(|_|"Native staged PCM poisoned")?;if p.decoded_source.is_some(){let Some(DecodedProof::Audio{first_sample})=proof else{return Err("Native PCM has no actual output ordinal".into());};buffer.push_exact(first_sample,samples)?;if let Some(expected)=&p.exact{p.sound_ready=buffer.preroll_exact(expected.audio.as_ref().ok_or("Saved native sound cursor absent")?)?;}}else if buffer.len()<=CAPACITY/2{buffer.push(seconds,samples)?;}}},
        Packet::Data(Chunk::End)=>{p.eof=true;p.worker=None;if let Some(audio)=&p.audio{audio.0.lock().map_err(|_|"Native staged PCM poisoned")?.ended=true;}},
        Packet::Error(e)=>return Err(format!("Native staged decoder: {e}")),
    }}Ok(false)
}
fn run(root:PathBuf,views:Vec<(VideoView,VideoAudio,Option<rvn_media::checkpoint::NativeCheckpoint>)>,cancel:Arc<AtomicBool>,proven_start:bool)->Result<Vec<StagedPlayer>,String>{
    let mut players=Vec::new();let mut total=0;let mut reserved=0usize;
    for(view,audio,exact)in views{
        if cancel.load(Ordering::Acquire){return Err("Native staging cancelled".into());}
        let decode=!view.track.finished()||(view.track.playback==Playback::Ended&&view.track.clip.keep_last_frame);
        let mut inputs=Vec::new();let seek=if view.track.finished(){(view.track.position-0.25).max(0.0)}else{view.track.position};
        let worker=if decode{let input=snapshot(&root,&view.track.clip.source,&cancel,&mut total)?;reserve_decoder(&input,&mut reserved)?;let worker=if exact.is_some()||(proven_start&&seek==0.0&&!view.track.finished()){Worker::spawn_proven(input.owned.path.clone(),Some(input.owned.clone()))?}else{Worker::spawn_with_keepalive(input.owned.path.clone(),seek,Some(input.owned.clone()))?};inputs.push(input);Some(worker)}else{None};
        let mask=if decode{if let Some(path)=&view.track.clip.mask{let input=snapshot(&root,path,&cancel,&mut total)?;reserve_decoder(&input,&mut reserved)?;let worker=if exact.is_some()||(proven_start&&seek==0.0&&!view.track.finished()){Worker::spawn_proven(input.owned.path.clone(),Some(input.owned.clone()))?}else{Worker::spawn_with_keepalive(input.owned.path.clone(),seek,Some(input.owned.clone()))?};inputs.push(input);Some(Mask{worker:Some(worker),frames:VecDeque::new(),current:None,decoded_source:None,exact:exact.as_ref().and_then(|e|e.mask.clone()),metadata:None,eof:false})}else{None}}else{None};
        players.push(StagedPlayer{view,worker,metadata:None,frames:VecDeque::new(),mask,audio:Some(audio),eof:!decode,inputs,exact,picture_ready:false,sound_ready:false,decoded_source:None});
    }
    let deadline=std::time::Instant::now()+std::time::Duration::from_secs(15);
    loop{if cancel.load(Ordering::Acquire){return Err("Native staging cancelled".into());}let mut ready=true;for p in &mut players{if p.eof&&p.metadata.is_none(){p.audio=None;continue;}ready&=pump_staged(p)?;}decoded_bytes(&players)?;if ready{for player in &mut players{if player.view.track.finished(){player.worker=None;if let Some(mask)=&mut player.mask{mask.worker=None;mask.eof=true;}}}return Ok(players);}if std::time::Instant::now()>deadline{return Err("Native staging readiness timed out".into());}std::thread::sleep(std::time::Duration::from_millis(1));}
}
fn reserve_decoder(input:&InputReceipt,total:&mut usize)->Result<(),String>{
    // Actual native metadata, opened on the staging thread before worker spawn.
    // Reserve held/queued/in-flight RGBA and two-second PCM for every decoder;
    // opaque FFmpeg codec allocations remain outside this application budget.
    let decoder=rvn_media::decoder::Decoder::open(&input.owned.path)?;
    let meta=&decoder.metadata;
    let bytes=(meta.width as usize).checked_mul(meta.height as usize).and_then(|n|n.checked_mul(4*8)).and_then(|n|n.checked_add(CAPACITY*std::mem::size_of::<f32>()*4)).ok_or("Native decoder reservation overflow")?;
    *total=total.checked_add(bytes).filter(|n|*n<=DECODED_LIMIT).ok_or("Native decoder aggregate reservation exceeds256MiB")?;
    Ok(())
}
fn outgoing(world:&World,instance:u64,gate:&crate::checkpoint_pcm::CheckpointGate,retirement:Option<&crate::runtime_restart::RetirementSource>)->Result<Vec<VideoAudio>,String>{
    let mut sources=Vec::new();if let Some(assets)=world.get_resource::<Assets<VideoAudio>>(){for(_,source)in assets.iter(){let buffer=source.0.lock().map_err(|_|"Outgoing native buffer poisoned")?;if !buffer.verifies_owner(instance,gate)&&!retirement.is_some_and(|source|source.admits_native_buffer(&buffer)){return Err("Resident native buffer has foreign/unbound owner or producer gate".into());}drop(buffer);if !sources.iter().any(|old:&VideoAudio|Arc::ptr_eq(&old.0,&source.0)){sources.push(source.clone());}}}Ok(sources)
}
pub(crate) fn prepare_restore(world:&World,core:&rvn_core::engine::PreparedSave15)->Result<Option<PreparedNativeRestore>,String>{
    let game=&world.get_resource::<VnEngine>().ok_or("Native staging requires actual Engine")?.0;
    // A direct caller must already have warmed legitimate live projections.
    // If this fills a cache, the opaque Core source check below rejects it.
    let current_views=game.video_views().map_err(|e|e.to_string())?;
    if !game.accepts_prepared_save15(core){return Err("Native staging Core source changed".into());}let gate=game.renderer.audio_checkpoints.gate();let old=outgoing(world,game.instance_id(),&gate,None)?;let key=key(core)?;
    let gain=world.get_resource::<crate::systems::settings_menu::Settings>().map(|s|s.sfx_volume).unwrap_or(1.0);
    if !gain.is_finite()||!(0.0..=1.0).contains(&gain){return Err("Invalid native video user gain before staging".into());}
    let owner=world.get_resource::<Players>();let runtime=owner.map(|p|p.restore_authority.clone());
    let active=owner.is_some_and(|p|!p.players.is_empty()||!p.startup_errors.is_empty())||!core.render_batch().videos.is_empty()||!current_views.is_empty()||!old.is_empty();
    if !active{return Ok(Some(PreparedNativeRestore{source:NativeSource::Game(game.debug_state_source().map_err(|e|e.to_string())?),key,root:PathBuf::new(),players:Vec::new(),outgoing:old,instance:game.instance_id(),gate,runtime,retained:Vec::new(),retirement:None}));}
    let root=world.get_resource::<crate::project_paths::ProjectPaths>().ok_or("Native restore requires actual ProjectPaths")?.assets.canonicalize().map_err(|e|e.to_string())?;
    if world.get_resource::<Assets<Image>>().is_none()||world.get_resource::<Assets<VideoAudio>>().is_none(){return Err("Native restore requires actual Image/VideoAudio asset runtimes".into());}
    let owner=owner.ok_or("Native restore requires actual NativeVideo Players owner")?;
    let mut cache=owner.restore_staging.lock().map_err(|_|"Native staging cache poisoned")?;
    if cache.as_ref().is_some_and(|cached|cached.key!=key||cached.root!=root||!matches!(&cached.source,NativeSource::Game(source) if game.accepts_debug_state_source(source))){*cache=None;}
    if cache.is_none(){let mut views=Vec::new();for view in &core.render_batch().videos{views.push((view.clone(),crate::video_audio::source_for_prepared(world,core,view)?,core.native_media_checkpoint().and_then(|rows|rows.iter().find(|row|row.id==view.id)).cloned()));}let cancel=Arc::new(AtomicBool::new(false));let(child_tx,receiver)=mpsc::sync_channel(1);let job_root=root.clone();let job_cancel=cancel.clone();std::thread::Builder::new().name("rvn-native-restore-stage".into()).spawn(move||{let result=run(job_root,views,job_cancel,false);let _=child_tx.send(result);}).map_err(|e|e.to_string())?;*cache=Some(PendingRestore{source:NativeSource::Game(game.debug_state_source().map_err(|e|e.to_string())?),key,root:root.clone(),retained:Vec::new(),cancel,receiver});return Ok(None);}
    match cache.as_ref().unwrap().receiver.try_recv(){Ok(result)=>{let _current=cache.take().unwrap();let players=result?;Ok(Some(PreparedNativeRestore{source:NativeSource::Game(game.debug_state_source().map_err(|e|e.to_string())?),key,root,players,outgoing:old,instance:game.instance_id(),gate,runtime,retained:Vec::new(),retirement:None}))},Err(mpsc::TryRecvError::Empty)=>Ok(None),Err(mpsc::TryRecvError::Disconnected)=>{*cache=None;Err("Native staging worker disconnected".into())}}
}
pub(crate) fn verify_restore(world:&World,core:&rvn_core::engine::PreparedSave15,plan:&PreparedNativeRestore)->Result<(),String>{
    let game=&world.get_resource::<VnEngine>().ok_or("Actual Engine vanished")?.0;if !game.accepts_prepared_save15(core)||!matches!(&plan.source,NativeSource::Game(source) if game.accepts_debug_state_source(source))||key(core)?!=plan.key{return Err("Native restore source/candidate changed".into());}
    verify_runtime(world,plan)
}
fn verify_runtime(world:&World,plan:&PreparedNativeRestore)->Result<(),String>{
    let gain=world.get_resource::<crate::systems::settings_menu::Settings>().map(|s|s.sfx_volume).unwrap_or(1.0);if !gain.is_finite()||!(0.0..=1.0).contains(&gain){return Err("Invalid native video user gain before publication".into());}
    match (&plan.runtime,world.get_resource::<Players>()){
        (None,None)=>{},(Some(expected),Some(current))if Arc::ptr_eq(expected,&current.restore_authority)=>{},
        _=>return Err("NativeVideo owner identity changed after preparation".into()),
    }
    if !plan.root.as_os_str().is_empty(){let root=world.get_resource::<crate::project_paths::ProjectPaths>().ok_or("Actual ProjectPaths vanished")?.assets.canonicalize().map_err(|e|e.to_string())?;if root!=plan.root||world.get_resource::<Assets<Image>>().is_none()||world.get_resource::<Assets<VideoAudio>>().is_none(){return Err("Native restore asset runtime changed".into());}}
    verify_retained(world,plan)?;
    if !crate::runtime_restart::accepts_retirement(world,plan.instance,&plan.gate,&plan.retirement)?{return Err("Actual replacement retirement source changed before publication".into());}
    let current=outgoing(world,plan.instance,&plan.gate,plan.retirement.as_ref())?;
    if current.len()!=plan.outgoing.len()||current.iter().any(|source|!plan.outgoing.iter().any(|old|Arc::ptr_eq(&source.0,&old.0))){return Err("Resident native sources changed after preparation".into());}
    for p in &plan.players{for input in &p.inputs{if safe_path(&plan.root,&input.locator)?!=input.original{return Err("Native media locator changed before commit".into());}let actual=std::fs::metadata(&input.original).map_err(|e|e.to_string())?;if actual.len()!=input.bytes{return Err("Native original media size changed before commit".into());}let mut file=std::fs::File::open(&input.original).map_err(|e|e.to_string())?;let mut hash=Sha256::new();let mut block=[0u8;64*1024];let mut bytes=0u64;loop{let n=file.read(&mut block).map_err(|e|e.to_string())?;if n==0{break;}bytes+=n as u64;if bytes>input.bytes{return Err("Native original media changed while verifying".into());}hash.update(&block[..n]);}let digest:[u8;32]=hash.finalize().into();if bytes!=input.bytes||digest!=input.digest{return Err("Native actual media bytes changed before commit".into());}}}Ok(())
}
impl PreparedNativeRestore{
    pub(crate) fn with_retirement<T>(&self,publish:impl FnOnce(&mut dyn FnMut())->Result<T,String>)->Result<T,String>{
        self.with_retirement_policy(matches!(&self.source,NativeSource::Startup(_)),publish)
    }
    /// The actual World startup installer can retain readers under its fence
    /// without reopening a clock that its owner has not yet admitted. Closing
    /// needs no temporal permit. Only the ordinary authenticated tick resumes.
    pub(crate) fn with_suspended_retirement<T>(&self,publish:impl FnOnce(&mut dyn FnMut())->Result<T,String>)->Result<T,String>{
        self.with_retirement_policy(true,publish)
    }
    fn with_retirement_policy<T>(&self,suspend:bool,publish:impl FnOnce(&mut dyn FnMut())->Result<T,String>)->Result<T,String>{
        let mut guards=Vec::new();for audio in &self.outgoing{let guard=audio.0.lock().map_err(|_|"Outgoing native buffer poisoned before commit")?;if !guard.verifies_owner(self.instance,&self.gate)&&!self.retirement.as_ref().is_some_and(|source|source.admits_native_buffer(&guard)){return Err("Outgoing native authority changed before commit".into());}
            if let Some(retained)=self.retained.iter().find(|row|row.audio.as_ref().is_some_and(|kept|Arc::ptr_eq(&kept.0,&audio.0))){if guard.revoked||guard.playout_abandoned()||!guard.verifies_view(self.instance,&retained.before){return Err("Retained native capability changed before the actual fence".into());}}
            guards.push(guard);}
        // Media->broker->gate lock order. Source next uses gate->TRY media,
        // never waits on these mutexes, so there is no reverse blocking edge.
        let mut retire=||{for (audio,buffer) in self.outgoing.iter().zip(&mut guards){
            if let Some(retained)=self.retained.iter().find(|row|row.audio.as_ref().is_some_and(|kept|Arc::ptr_eq(&kept.0,&audio.0))){
                buffer.install_prepared_authority(retained.authority.clone().expect("Retained real PCM owns its prepared capability"),!suspend&&retained.view.track.playback==Playback::Playing);
            }else{buffer.running=false;buffer.ended=true;buffer.revoked=true;}
        }};publish(&mut retire)
    }
}
pub(crate) fn install_restore(world:&mut World,plan:PreparedNativeRestore){
    if plan.runtime.is_none(){return;}
    let mut live=world.remove_resource::<Players>().expect("Actual native owner was preflighted under exclusive World");let mut previous=std::mem::take(&mut live.players);
    for retained in plan.retained{let mut player=previous.remove(&retained.view.id).expect("Retained player was preflighted under exclusive World");player.view=retained.view;
        let gain=player.view.track.clip.volume*world.get_resource::<crate::systems::settings_menu::Settings>().map(|settings|settings.sfx_volume).unwrap_or(1.0);
        if let Some(entity)=player.audio_entity{if let Some(mut settings)=world.get_mut::<PlaybackSettings>(entity){settings.volume=Volume::new(gain);settings.paused=player.view.track.playback!=Playback::Playing;}if let Some(sink)=world.get::<bevy::audio::AudioSink>(entity){use bevy::audio::AudioSinkPlayback;sink.set_volume(gain);if player.view.track.playback==Playback::Playing{sink.play();}else{sink.pause();}}}
        live.players.insert(player.view.id.clone(),player);
    }
    for(_,player)in previous{remove(world,player);}live.startup_errors.clear();live.scene_views=None;
    for staged in plan.players{
        let view=staged.view;let position=view.track.position;let frame=staged.frames.front();let mut pixels=frame.map(|f|f.rgba.clone());if let(Some(pixels),Some(mask))=(&mut pixels,staged.mask.as_ref().and_then(|m|m.frames.front())){apply_mask(pixels,&mask.rgba);}
        let mut image=if let Some(frame)=frame{Image::new_fill(Extent3d{width:frame.width,height:frame.height,depth_or_array_layers:1},TextureDimension::D2,pixels.as_ref().unwrap(),TextureFormat::Rgba8UnormSrgb,RenderAssetUsages::default())}else{Image::new_fill(Extent3d{width:1,height:1,depth_or_array_layers:1},TextureDimension::D2,&[0,0,0,255],TextureFormat::Rgba8UnormSrgb,RenderAssetUsages::default())};image.sampler=bevy::render::texture::ImageSampler::linear();let image=world.resource_mut::<Assets<Image>>().add(image);
        let(audio_handle,audio_entity)=if let Some(audio)=&staged.audio{let handle=world.resource_mut::<Assets<VideoAudio>>().add(audio.clone());let gain=view.track.clip.volume*world.get_resource::<crate::systems::settings_menu::Settings>().map(|s|s.sfx_volume).unwrap_or(1.0);let entity=world.spawn(AudioSourceBundle{source:handle.clone(),settings:PlaybackSettings::ONCE.with_volume(Volume::new(gain)),..default()}).id();(Some(handle),Some(entity))}else{(None,None)};
        live.players.insert(view.id.clone(),Player{instance:plan.instance,view,worker:staged.worker,metadata:staged.metadata,decoded_source:staged.decoded_source,pending:None,frames:staged.frames,eof:staged.eof,image,entity:None,caption:None,audio:staged.audio,audio_entity,audio_handle,position,last_report:position,age:0.0,drawn:false,mask:staged.mask,last_frame:None,terminal:default()});
        // Worker owns each snapshot until its decoder closes. Inputs retain
        // original receipts only until this infallible ownership transfer.
    }
    world.insert_resource(live);
}

/// Ordinary real-origin starts use immutable input and actual decoder facts.
/// Legacy nonzero presentation seeks retain the historical position contract.
pub(super) fn ordinary_worker(path:PathBuf,position:f64)->Result<Worker,String>{
    let root=path.parent().ok_or("Native source has no parent")?;let locator=path.file_name().ok_or("Native source has no name")?.to_str().ok_or("Native source is not UTF8")?;
    let mut total=0;let input=snapshot(root,locator,&AtomicBool::new(false),&mut total)?;
    if position==0.0{Worker::spawn_proven(input.owned.path.clone(),Some(input.owned))}else{Worker::spawn_with_keepalive(input.owned.path.clone(),position,Some(input.owned))}
}

/// All source locks precede broker/gate. Actual samplers use gate -> try_lock,
/// so this capture shares one fence without a reverse blocking edge.
pub(crate) fn capture_save15(world:&World,program:&rvn_parser::ParsedProgram,session:Option<&rvn_core::engine::DebugSession>,slot:u32,label:String)->Result<rvn_core::save::consumer_checkpoint::Save15,String>{
    let game=&world.get_resource::<VnEngine>().ok_or("Exact capture requires actual Engine")?.0;
    let views=game.video_views().map_err(|e|e.to_string())?;
    let owner=world.get_resource::<Players>().ok_or("Exact capture requires actual NativeVideo owner")?;
    let mut players=Vec::new();for view in &views{if view.track.finished()&&!(view.track.playback==Playback::Ended&&view.track.clip.keep_last_frame){continue;}let player=owner.players.get(&view.id).filter(|p|same_capability(&p.view,view)&&p.view.track.clip==view.track.clip).ok_or("Native reader differs from actual Core video")?;players.push(player);}
    let mut buffers=Vec::new();for player in &players{buffers.push(player.audio.as_ref().map(|audio|audio.0.lock().map_err(|_|"Native capture buffer poisoned")).transpose()?);}
    let build=|snapshot|{
        let mut native=Vec::new();for(player,buffer)in players.iter().zip(&buffers){
            let (asset_sha256,profile)=player.decoded_source.clone().ok_or("Native source lacks canonical origin provenance")?;
            let picture=player.last_frame.as_ref().and_then(|f|f.proof.clone()).ok_or("Native selected picture has no actual ordinal/PTS")?;
            let audio=if player.view.track.finished(){None}else if let Some(buffer)=buffer{if !buffer.verifies_owner(game.instance_id(),&game.renderer.audio_checkpoints.gate()){return Err("Native capture has a foreign source/gate".into());}let(asset,actual,cursor)=buffer.exact_cursor()?;if asset!=asset_sha256||actual!=profile{return Err("Native PCM/picture provenance differs".into());}Some(cursor)}else{if profile.audio_stream.is_some(){return Err("Native sound reader is not resident".into());}None};
            let mask=player.mask.as_ref().map(|mask|{
                let(asset_sha256,profile)=mask.decoded_source.clone().ok_or("Native mask lacks canonical provenance")?;
                let picture=mask.current.as_ref().and_then(|f|f.proof.clone()).ok_or("Native mask has no selected frame ordinal/PTS")?;
                Ok::<_,String>(rvn_media::checkpoint::MaskCheckpoint{source_locator:player.view.track.clip.mask.clone().ok_or("Actual Core mask locator absent")?,asset_sha256,profile,picture})
            }).transpose()?;
            native.push(rvn_media::checkpoint::NativeCheckpoint{id:player.view.id.clone(),source_locator:player.view.track.clip.source.clone(),asset_sha256,profile,picture,audio,mask});
        }
        game.capture_save15_with_fenced_media(program,session,slot,label,snapshot,native).map_err(|e|e.to_string())
    };
    if game.save15_session_requires_frame_audio(session){game.renderer.audio_checkpoints.capture_frame_with(&game.state,game.instance_id(),game.runtime_epoch(),build)}else{game.renderer.audio_checkpoints.capture_with(&game.state,game.instance_id(),game.runtime_epoch(),build)}
}

/// Runtime-only, issued exclusively by an owned real staging plan. No public
/// constructor/serde and no desired DTO can manufacture this host permission.
pub(crate) struct NativeAdmission{key:[u8;32]}
impl PreparedNativeRestore{pub(crate) fn admission(&self)->NativeAdmission{NativeAdmission{key:self.key}}}
pub(crate) fn admits_exact(receipt:Option<&NativeAdmission>,core:&rvn_core::engine::PreparedSave15)->Result<bool,String>{Ok(receipt.is_some_and(|r|key(core).is_ok_and(|k|r.key==k)))}

#[cfg(test)]
pub(crate) fn key_for_test(core:&rvn_core::engine::PreparedSave15)->Result<[u8;32],String>{key(core)}

#[cfg(test)]
pub(crate) fn key_contrasts_for_test(core:&rvn_core::engine::PreparedSave15)->Result<([u8;32],[u8;32],[u8;32]),String>{
    // Only hash a clone of the actual batch; this creates no authority/permit.
    let mut left=core.render_batch().clone();let mut pairs:Vec<_>=left.state.vars.iter().map(|(k,v)|(k.clone(),v.clone())).collect();pairs.sort_by(|a,b|a.0.cmp(&b.0));
    left.state.vars.clear();for(k,v)in pairs.iter().cloned(){left.state.vars.insert(k,v);}
    left.state.vars.insert("array_order".into(),rvn_parser::Value::List(vec![rvn_parser::Value::Int(1),rvn_parser::Value::Int(2)]));
    let mut right=left.clone();right.state.vars.clear();for(k,v)in pairs.into_iter().rev(){right.state.vars.insert(k,v);}right.state.vars.insert("array_order".into(),rvn_parser::Value::List(vec![rvn_parser::Value::Int(1),rvn_parser::Value::Int(2)]));
    assert_eq!(left.state.vars,right.state.vars);let first=batch_key(&left,core.native_media_checkpoint())?;let second=batch_key(&right,core.native_media_checkpoint())?;
    right.state.vars.insert("array_order".into(),rvn_parser::Value::List(vec![rvn_parser::Value::Int(2),rvn_parser::Value::Int(1)]));let third=batch_key(&right,core.native_media_checkpoint())?;Ok((first,second,third))
}


/// The real pending Init owner admits this batch. No Game Engine or Save15 is
/// constructed to stage a live step, and no client batch can issue this source.
pub(crate) fn prepare_startup(world:&World,core:&rvn_core::engine::PreparedStartupStep)->Result<Option<PreparedNativeRestore>,String>{
    if world.contains_resource::<VnEngine>(){return Err("Native startup cannot share a gameplay owner".into());}
    let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>().ok_or("Native startup requires its actual owner")?;
    if startup.publication_pending||startup.failed.is_some(){return Err("Actual startup publication is unsettled".into());}
    let source=startup.owner.prepared_publication_source(core).map_err(|e|e.to_string())?;
    let batch=core.render_batch().ok_or("Computation-only startup has no native publication")?;
    if batch.runtime_epoch!=source.runtime_epoch(){return Err("Native startup batch has a foreign generation".into());}
    let gate=startup.broker.gate();let instance=source.instance_id();let retirement=crate::runtime_restart::retirement_source(world,instance,&gate)?;let old=outgoing(world,instance,&gate,retirement.as_ref())?;let key=startup_key(batch,core.video_commands())?;
    let gain=world.get_resource::<crate::systems::settings_menu::Settings>().map(|s|s.sfx_volume).unwrap_or(1.0);
    if !gain.is_finite()||!(0.0..=1.0).contains(&gain){return Err("Invalid native startup user gain before staging".into());}
    let owner=world.get_resource::<Players>();let runtime=owner.map(|p|p.restore_authority.clone());
    let active=owner.is_some_and(|p|!p.players.is_empty()||!p.startup_errors.is_empty())||!batch.videos.is_empty()||!old.is_empty();
    if !active{return Ok(Some(PreparedNativeRestore{source:NativeSource::Startup(source),key,root:PathBuf::new(),players:Vec::new(),outgoing:old,instance,gate,runtime,retained:Vec::new(),retirement}));}
    let root=world.get_resource::<crate::project_paths::ProjectPaths>().ok_or("Native startup requires actual ProjectPaths")?.assets.canonicalize().map_err(|e|e.to_string())?;
    if world.get_resource::<Assets<Image>>().is_none()||world.get_resource::<Assets<VideoAudio>>().is_none(){return Err("Native startup requires actual Image/VideoAudio asset runtimes".into());}
    let owner=owner.ok_or("Native startup requires actual NativeVideo Players owner")?;
    let retained=match retain_startup(world,core,batch,instance,&gate,&root){Ok(rows)=>rows,Err(error)=>{*owner.restore_staging.lock().map_err(|_|"Native staging cache poisoned during refusal")?=None;return Err(error);}};
    let mut cache=owner.restore_staging.lock().map_err(|_|"Native startup staging cache poisoned")?;
    if cache.as_ref().is_some_and(|cached|cached.key!=key||cached.root!=root||!same_retained(&cached.retained,&retained)||!matches!(&cached.source,NativeSource::Startup(actual) if actual==&source)){*cache=None;}
    if retained.len()==batch.videos.len(){*cache=None;return Ok(Some(PreparedNativeRestore{source:NativeSource::Startup(source),key,root,players:Vec::new(),outgoing:old,instance,gate,runtime,retained,retirement}));}
    if cache.is_none(){
        let mut views=Vec::new();for view in &batch.videos{if !retained.iter().any(|row|row.view.id==view.id){views.push((view.clone(),crate::video_audio::source_for_startup(world,core,view)?,None));}}
        let cancel=Arc::new(AtomicBool::new(false));let(child_tx,receiver)=mpsc::sync_channel(1);let job_root=root.clone();let job_cancel=cancel.clone();
        std::thread::Builder::new().name("rvn-native-startup-stage".into()).spawn(move||{let result=run(job_root,views,job_cancel,true);let _=child_tx.send(result);}).map_err(|e|e.to_string())?;
        *cache=Some(PendingRestore{source:NativeSource::Startup(source),key,root,retained,cancel,receiver});return Ok(None);
    }
    match cache.as_ref().unwrap().receiver.try_recv(){
        Ok(result)=>{let _current=cache.take().unwrap();Ok(Some(PreparedNativeRestore{source:NativeSource::Startup(source),key,root,players:result?,outgoing:old,instance,gate,runtime,retained,retirement}))},
        Err(mpsc::TryRecvError::Empty)=>Ok(None),Err(mpsc::TryRecvError::Disconnected)=>{*cache=None;Err("Native startup staging worker disconnected".into())}
    }
}
pub(crate) fn verify_startup(world:&World,core:&rvn_core::engine::PreparedStartupStep,plan:&PreparedNativeRestore)->Result<(),String>{
    if world.contains_resource::<VnEngine>(){return Err("Native startup cannot share a gameplay owner".into());}
    let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>().ok_or("Actual startup owner vanished")?;
    if startup.publication_pending||startup.failed.is_some(){return Err("Actual startup publication is unsettled".into());}
    let source=startup.owner.prepared_publication_source(core).map_err(|e|e.to_string())?;
    let batch=core.render_batch().ok_or("Computation-only startup has no native publication")?;
    if !matches!(&plan.source,NativeSource::Startup(actual) if actual==&source)||startup_key(batch,core.video_commands())?!=plan.key||!startup.broker.gate().same_gate(&plan.gate){return Err("Native startup source/candidate/broker changed".into());}
    verify_runtime(world,plan)
}
impl PreparedNativeRestore{
    /// Called with the exclusive true owner guard before entering the broker.
    /// The retirement callback itself must run inside broker.publish's gate.
    pub(crate) fn verify_startup_guard(&self,guard:&rvn_core::engine::StartupHostStepGuard<'_,crate::bevy_renderer::BevyRenderer>,broker:&crate::checkpoint_broker::AudioCheckpointBroker)->Result<(),String>{
        let batch=guard.render_batch().ok_or("Computation-only startup has no native publication")?;
        if !matches!(&self.source,NativeSource::Startup(source) if source==guard.publication_source())||startup_key(batch,guard.video_commands())?!=self.key{return Err("Native startup guard differs from its actual prepared source/batch".into());}
        if !broker.gate().same_gate(&self.gate)||guard.host_publication().is_none_or(|publication|!publication.accepts_broker(broker)){return Err("Native startup guard requires its actual renderer broker gate".into());}
        Ok(())
    }
}

fn same_audio(a:Option<&VideoAudio>,b:Option<&VideoAudio>)->bool{match(a,b){(None,None)=>true,(Some(a),Some(b))=>Arc::ptr_eq(&a.0,&b.0),_=>false}}
fn same_retained(a:&[RetainedPlayer],b:&[RetainedPlayer])->bool{a.len()==b.len()&&a.iter().all(|left|b.iter().any(|right|left.before==right.before&&left.authority==right.authority&&left.view==right.view&&left.image==right.image&&same_audio(left.audio.as_ref(),right.audio.as_ref())))}
fn live_input(root:&Path,locator:&str,known:Option<[u8;32]>,total:&mut u64)->Result<LiveInputFact,String>{
    let original=safe_path(root,locator)?;let metadata=std::fs::metadata(&original).map_err(|e|e.to_string())?;
    if !metadata.is_file(){return Err("Retained native input is not a regular file".into());}*total=total.checked_add(metadata.len()).filter(|n|*n<=INPUT_LIMIT).ok_or("Retained native input aggregate exceeds1GiB")?;
    let mut input=std::fs::File::open(&original).map_err(|e|e.to_string())?;let mut hash=Sha256::new();let mut bytes=0u64;let mut block=[0u8;64*1024];loop{let n=input.read(&mut block).map_err(|e|e.to_string())?;if n==0{break;}bytes=bytes.checked_add(n as u64).filter(|n|*n<=metadata.len()).ok_or("Retained input grew while reading")?;hash.update(&block[..n]);}let digest:[u8;32]=hash.finalize().into();if bytes!=metadata.len()||known.is_some_and(|expected|expected!=digest){return Err("Retained actual native input differs from its source".into());}
    Ok(LiveInputFact{locator:locator.into(),original,digest,bytes})
}
fn retain_startup(world:&World,core:&rvn_core::engine::PreparedStartupStep,batch:&rvn_core::renderer::SceneRenderBatch,instance:u64,gate:&crate::checkpoint_pcm::CheckpointGate,root:&Path)->Result<Vec<RetainedPlayer>,String>{
    let startup=world.get_resource::<crate::debugger_startup::DebugBootstrap>().ok_or("Retained actual Startup owner absent")?;
    startup.owner.validate_prepared_step(core).map_err(|e|e.to_string())?;
    let current=startup.owner.video_views().map_err(|e|e.to_string())?;
    let owner=world.get_resource::<Players>().ok_or("Retained NativeVideo owner absent")?;let images=world.get_resource::<Assets<Image>>().ok_or("Retained Image assets absent")?;let mut rows=Vec::new();let mut total=0;
    for view in &batch.videos{let Some(player)=owner.players.get(&view.id).filter(|player|player.instance==instance&&current.iter().any(|actual|actual==&player.view)&&retains_actual_commands(&player.view,view,core.video_commands()))else{continue;};
        // A legacy position-only reader has no immutable source receipt to
        // compare to today's file. Preserve its existing re-decoding path;
        // current file bytes alone cannot authenticate an older live worker.
        if player.decoded_source.is_none()||view.track.clip.mask.is_some()&&player.mask.as_ref().is_none_or(|mask|mask.decoded_source.is_none()){continue;}
        if images.get(&player.image).is_none(){return Err("Retained native image asset vanished".into());}
        if let Some(audio)=&player.audio{let buffer=audio.0.lock().map_err(|_|"Retained PCM poisoned")?;if !buffer.verifies_owner(instance,gate)||!buffer.verifies_view(instance,&player.view)||buffer.revoked||buffer.playout_abandoned(){return Err("Retained native reader has a foreign/revoked source".into());}}
        let mut inputs=Vec::new();inputs.push(live_input(root,&view.track.clip.source,player.decoded_source.as_ref().map(|(sha,_)|*sha),&mut total)?);
        if let Some(locator)=&view.track.clip.mask{inputs.push(live_input(root,locator,player.mask.as_ref().and_then(|mask|mask.decoded_source.as_ref().map(|(sha,_)|*sha)),&mut total)?);}
        let authority=player.audio.as_ref().map(|_|crate::video_audio::authority_for_startup(world,core,view)).transpose()?;
        rows.push(RetainedPlayer{before:player.view.clone(),authority,view:view.clone(),image:player.image.clone(),audio:player.audio.clone(),inputs});
    }Ok(rows)
}
fn verify_retained(world:&World,plan:&PreparedNativeRestore)->Result<(),String>{
    let mut total=0;for retained in &plan.retained{let player=world.get_resource::<Players>().and_then(|owner|owner.players.get(&retained.view.id)).ok_or("Retained native player vanished")?;
        if player.instance!=plan.instance||player.view!=retained.before||player.image!=retained.image||!same_audio(player.audio.as_ref(),retained.audio.as_ref())||world.get_resource::<Assets<Image>>().is_none_or(|images|images.get(&retained.image).is_none()){return Err("Retained native resource identity changed before publication".into());}
        for input in &retained.inputs{let actual=live_input(&plan.root,&input.locator,Some(input.digest),&mut total)?;if actual.original!=input.original||actual.bytes!=input.bytes{return Err("Retained native input changed before publication".into());}}
    }Ok(())
}

#[cfg(test)]
pub(super) fn retained_counts_for_test(plan:&PreparedNativeRestore)->(usize,usize){(plan.players.len(),plan.retained.len())}

/// Only the real prepared command trace distinguishes Seek(0) from no change.
/// Seconds are consistency data; no presentation position derives a PCM cursor.
fn retains_actual_commands(before:&VideoView,next:&VideoView,commands:&[rvn_core::engine::StartupVideoCommand])->bool{
 use rvn_core::engine::StartupVideoCommandKind as Kind;
 if before.id!=next.id||before.ui_projection!=next.ui_projection{return false;}
 match(&before.resource_token,&next.resource_token){
  (Some(a),Some(b))if a.task.runtime_epoch==b.task.runtime_epoch&&a.task.key==b.task.key&&a.target==b.target=>{},
  (None,None)=>{},_=>return false,
 }
 let mut expected=before.track.clone();expected.clip.volume=next.track.clip.volume;
 for command in commands.iter().filter(|command|command.name()==before.id){match command.kind(){
  Kind::Play|Kind::Seek|Kind::Stop|Kind::Skip=>return false,
  Kind::Pause=>{if expected.playback==Playback::Playing{expected.playback=Playback::Paused;}},
  Kind::Resume=>{if matches!(expected.playback,Playback::Paused|Playback::Blocked){expected.playback=Playback::Playing;expected.message=None;}else if expected.finished(){return false;}},
  Kind::Volume=>{},
 }}
 expected==next.track
}

/// Batch equality alone cannot distinguish a real same-position Seek. Bind the
/// exact accepted operation order as well as the full live Source at all gates.
fn startup_key(batch:&rvn_core::renderer::SceneRenderBatch,commands:&[rvn_core::engine::StartupVideoCommand])->Result<[u8;32],String>{
 use rvn_core::engine::StartupVideoCommandKind as Kind;
 if commands.len()>128{return Err("Native startup command receipt exceeds128 rows".into());}
 let mut hash=Sha256::new();hash.update(b"RVN.NativeStartup.BatchTrace.1\0");hash.update(batch_key(batch,None)?);hash.update((commands.len()as u64).to_le_bytes());let mut bytes=0usize;
 for command in commands{bytes=bytes.checked_add(command.name().len()).filter(|n|*n<=64*1024).ok_or("Native startup command receipt exceeds64KiB")?;let tag:u8=match command.kind(){Kind::Play=>1,Kind::Pause=>2,Kind::Resume=>3,Kind::Stop=>4,Kind::Skip=>5,Kind::Seek=>6,Kind::Volume=>7};hash.update([tag]);hash.update((command.name().len()as u64).to_le_bytes());hash.update(command.name().as_bytes());}
 Ok(hash.finalize().into())
}
