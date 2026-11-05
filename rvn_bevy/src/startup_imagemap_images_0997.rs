//! Actual AssetServer/Image readiness for one prepared Init Imagemap.
//! Handles/bytes are host resources, never a source or input authorization.
use bevy::prelude::*;
use sha2::{Digest,Sha256};
struct ImageRow{path:String,handle:Handle<Image>,digest:Option<[u8;32]>}
pub(super) struct PreparedImages{rows:Vec<ImageRow>}
#[derive(Resource)]
pub(super) struct InstalledImages(PreparedImages);
// AssetId is local to an Assets provider. Distinct servers can allocate the
// same ID/path; only the actual retained strong allocation authenticates this
// AssetServer's producer. Bevy get_handle upgrades that same StrongHandle Arc.
fn same_server_handle(server:&AssetServer,row:&ImageRow)->bool{
 let Some(current)=server.get_handle::<Image>(row.path.clone())else{return false;};
 current.id()==row.handle.id()&&matches!((&current,&row.handle),(Handle::Strong(actual),Handle::Strong(staged))if std::sync::Arc::ptr_eq(actual,staged))
}
struct MetadataDigest<'a>(&'a mut Sha256);
impl std::fmt::Write for MetadataDigest<'_>{
 fn write_str(&mut self,value:&str)->std::fmt::Result{self.0.update(value.as_bytes());Ok(())}
}
fn digest(image:&Image)->[u8;32]{
 let mut hash=Sha256::new();hash.update(b"rvn.init-imagemap-image.v2\0");
 // Stream only actual render metadata, never format/clone the pixel Vec.
 // A Default sampler marker is frozen here; the RenderWorld's global default
 // sampler and actual GPU preparation remain separate native qualification.
 std::fmt::write(&mut MetadataDigest(&mut hash),format_args!("{:?}|{:?}|{:?}|{:?}",image.texture_descriptor,image.sampler,image.texture_view_descriptor,image.asset_usage)).expect("SHA256 formatting sink cannot fail");
 hash.update(&image.data);hash.finalize().into()
}
fn paths(display:Option<&rvn_core::Interaction>)->Result<Vec<String>,String>{
 match display{
  None=>Ok(vec![]),
  Some(rvn_core::Interaction::Imagemap{background,hover_image,..})=>{
   let mut result=vec![crate::systems::ensure_extension(background)];
   if let Some(path)=hover_image{let path=crate::systems::ensure_extension(path);if !result.contains(&path){result.push(path);}}
   Ok(result)
  },
  _=>Err("Init asset preparation has no narrative display authority".into()),
 }
}
pub(super) fn begin(world:&World,display:Option<&rvn_core::Interaction>)->Result<PreparedImages,String>{
 let paths=paths(display)?;if paths.is_empty(){return Ok(PreparedImages{rows:vec![]});}
 let server=world.get_resource::<AssetServer>().ok_or("Init input has no actual image AssetServer")?;
 Ok(PreparedImages{rows:paths.into_iter().map(|path|ImageRow{handle:server.load(path.clone()),path,digest:None}).collect()})
}
pub(super) fn refresh(world:&World,display:Option<&rvn_core::Interaction>,prepared:&mut PreparedImages)->Result<bool,String>{
 let paths=paths(display)?;
 if paths.len()!=prepared.rows.len()||paths.iter().zip(&prepared.rows).any(|(a,b)|a!=&b.path){return Err("Staged Init input images differ from their actual frozen display".into());}
 if paths.is_empty(){return Ok(true);}
 let server=world.get_resource::<AssetServer>().ok_or("Init input has no actual image AssetServer")?;
 let loaded=world.get_resource::<Assets<Image>>().ok_or("Init input has no actual decoded Image runtime")?;
 let mut pending=false;
 for row in &mut prepared.rows{
  if !same_server_handle(server,row){return Err("Init input AssetServer no longer owns the staged image path/handle".into());}
  if let Some(bevy::asset::LoadState::Failed(error))=server.get_load_state(row.handle.id()){return Err(format!("Init Imagemap asset {:?} could not be loaded: {error}",row.path));}
  let Some(image)=loaded.get(&row.handle)else{
   if row.digest.is_some(){return Err("Already prepared Init input pixels disappeared during staging".into());}
   pending=true;continue;
  };
  if image.width()==0||image.height()==0||image.data.is_empty(){return Err(format!("Init Imagemap asset {:?} has no decoded pixels",row.path));}
  if !matches!(server.get_load_state(row.handle.id()),Some(bevy::asset::LoadState::Loaded)){pending=true;continue;}
  let actual=digest(image);
  if row.digest.is_some_and(|expected|expected!=actual){return Err("Already prepared Init input pixels or rendering metadata changed during staging".into());}
  row.digest=Some(actual);
 }
 Ok(!pending)
}
pub(super) fn verify(world:&World,display:Option<&rvn_core::Interaction>,prepared:&PreparedImages)->Result<(),String>{
 let paths=paths(display)?;
 if paths.len()!=prepared.rows.len()||paths.iter().zip(&prepared.rows).any(|(a,b)|a!=&b.path){return Err("Init input image set differs from its actual frozen display".into());}
 if paths.is_empty(){return Ok(());}
 let server=world.get_resource::<AssetServer>().ok_or("Init input AssetServer vanished")?;
 let loaded=world.get_resource::<Assets<Image>>().ok_or("Init input decoded Image runtime vanished")?;
 for row in &prepared.rows{
  if !same_server_handle(server,row){return Err("Init input AssetServer changed before publication".into());}
  if !matches!(server.get_load_state(row.handle.id()),Some(bevy::asset::LoadState::Loaded)){return Err("Prepared Init image has no completed load in the actual AssetServer".into());}
  if matches!(server.get_load_state(row.handle.id()),Some(bevy::asset::LoadState::Failed(_))){return Err("Prepared Init input image failed before publication".into());}
  let image=loaded.get(&row.handle).ok_or("Prepared Init input image disappeared before publication")?;
  if Some(digest(image))!=row.digest{return Err("Prepared Init input pixels or rendering metadata changed before publication".into());}
 }
 Ok(())
}
pub(super) fn install(world:&mut World,display:Option<&rvn_core::Interaction>,prepared:PreparedImages){
 if display.is_some(){world.insert_resource(InstalledImages(prepared));}else{world.remove_resource::<InstalledImages>();}
}
pub(super) fn verify_installed(world:&World)->Result<(),String>{
 let startup=world.get_resource::<super::DebugBootstrap>().ok_or("Actual input Startup owner vanished")?;
 let display=startup.owner.imagemap_interaction().map_err(|error|error.to_string())?.ok_or("Actual Startup has no reached Imagemap wait")?;
 let installed=world.get_resource::<InstalledImages>().ok_or("Actual Init input images were never installed")?;
 verify(world,Some(&display),&installed.0)
}
