//! Real World/AssetServer staging and publication. No native/GPU parity claim.
use super::*;
use bevy::prelude::*;
use std::{path::PathBuf,time::{Duration,Instant}};
const SCRIPT:&str=r#"init{flow.consume_v2("world_map","map"){capture "draw"=random(1,9) imagemap{background:"map.png" hover:"hover.png" hotspot{name:"choose" area:(0,0,100,100)}=>{set chosen=1}}} set done=1}
scene label start
"Boundary"
"#;
pub(super) fn fixture(source:&str)->(PathBuf,App){
 let(root,mut app)=super::tests::fixture_source(source);
 let image=PathBuf::from(std::env::var("RVN_STARTUP_WORLD_FIXTURES").unwrap()).parent().unwrap().join("shadow/examples/animations/assets/card-one.png");
 for name in ["map.png","hover.png"]{std::fs::copy(&image,root.join("assets").join(name)).unwrap();}
 app.init_asset::<Image>().init_asset_loader::<bevy::render::texture::ImageLoader>()
  .init_resource::<crate::resources::ImagemapState>().init_resource::<crate::resources::VnRenderState>()
  .add_systems(Update,(crate::systems::imagemap_system,crate::systems::imagemap_dimensions_system).chain());
 (root,app)
}
fn state(app:&App)->serde_json::Value{serde_json::to_value(app.world().resource::<DebugBootstrap>().owner.state()).unwrap()}
pub(super) fn ready(app:&mut App,prepared:&mut PreparedBootstrapStep){
 let end=Instant::now()+Duration::from_secs(15);
 while !refresh_world_step(app.world(),prepared).unwrap(){app.update();assert!(Instant::now()<end,"Actual Init Imagemap AssetServer timeout");std::thread::sleep(Duration::from_millis(2));}
}
#[test]
fn startup_imagemap0998_world_staging_keeps_real_decoded_images_and_one_unpublished_gather_until_install(){
 let(_,mut app)=fixture(SCRIPT);let before=state(&app);let observed=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
 let mut prepared=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();assert!(!prepared.ready());
 let candidate=serde_json::to_value(&prepared.core().render_batch().unwrap().state).unwrap();assert!(matches!(prepared.core().imagemap_interaction(),Some(rvn_core::Interaction::Imagemap{hotspots,..})if hotspots.len()==1));
 for _ in 0..3{assert!(!refresh_world_step(app.world(),&mut prepared).unwrap());assert_eq!(state(&app),before);assert_eq!(serde_json::to_value(&prepared.core().render_batch().unwrap().state).unwrap(),candidate);}
 ready(&mut app,&mut prepared);assert_eq!(state(&app),before);assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&observed));
 for name in ["map.png","hover.png"]{let h=app.world().resource::<AssetServer>().get_handle::<Image>(name).unwrap();let image=app.world().resource::<Assets<Image>>().get(&h).unwrap();assert!(image.width()>0&&image.height()>0&&!image.data.is_empty());}
 let committed=commit_world_step(app.world_mut(),prepared).unwrap();assert!(app.world().resource::<DebugBootstrap>().publication_pending);assert!(!app.world().resource::<crate::resources::ImagemapState>().active);
 install_committed_world_step(app.world_mut(),committed);assert!(!app.world().resource::<DebugBootstrap>().publication_pending);assert!(app.world().resource::<DebugBootstrap>().owner.waiting_for_imagemap());assert!(!app.world().contains_resource::<crate::resources::VnEngine>());assert_eq!(state(&app),candidate);validate_installed_imagemap_images(app.world()).unwrap();app.update();
 let actual=app.world().resource::<crate::resources::ImagemapState>();assert!(actual.active);assert!(actual.source_w>0.0&&actual.source_h>0.0);assert_eq!(actual.hotspots.len(),1);
 let entity=app.world_mut().query_filtered::<Entity,With<crate::components::ImagemapBackground>>().single(app.world());let handle=app.world().resource::<AssetServer>().get_handle::<Image>("map.png").unwrap();assert_eq!(app.world().get::<Handle<Image>>(entity).unwrap().id(),handle.id());
}
#[test]
fn startup_imagemap0998_world_drop_and_changed_decoded_pixels_refuse_before_owner_and_publication_effects(){
 let(_,mut app)=fixture(SCRIPT);let before=state(&app);let observed=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();let mut prepared=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();ready(&mut app,&mut prepared);drop(prepared);assert_eq!(state(&app),before);assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&observed));assert!(!app.world().resource::<crate::resources::ImagemapState>().active);
 let mut prepared=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();ready(&mut app,&mut prepared);let h=app.world().resource::<AssetServer>().get_handle::<Image>("hover.png").unwrap();app.world_mut().resource_mut::<Assets<Image>>().get_mut(&h).unwrap().data[0]^=1;
 assert!(commit_world_step(app.world_mut(),prepared).err().unwrap().contains("pixels"));assert_eq!(state(&app),before);assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&observed));assert!(!app.world().resource::<DebugBootstrap>().publication_pending);assert!(!app.world().resource::<crate::resources::ImagemapState>().active);
}
#[test]
fn startup_imagemap0998_world_selection_installs_real_branch_and_retires_map_without_executing_child(){
 let(_,mut app)=fixture(SCRIPT);let mut header=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();ready(&mut app,&mut header);let committed=commit_world_step(app.world_mut(),header).unwrap();install_committed_world_step(app.world_mut(),committed);app.update();
 let owner=&app.world().resource::<DebugBootstrap>().owner;let source=owner.imagemap_input_source(std::sync::Arc::new(owner.observation_source().unwrap())).unwrap();let random=owner.state().random;
 let selection=prepare_world_imagemap_selection(app.world(),&source,0).unwrap();assert!(selection.ready());assert!(app.world().resource::<crate::resources::ImagemapState>().active);assert!(!app.world().resource::<DebugBootstrap>().owner.state().vars.contains_key("chosen"));
 let committed=commit_world_step_boxed(app.world_mut(),selection).unwrap();install_committed_world_step_boxed(app.world_mut(),committed);assert!(!app.world().resource::<crate::resources::ImagemapState>().active);assert!(!app.world().resource::<DebugBootstrap>().owner.state().vars.contains_key("chosen"));assert_eq!(app.world().resource::<DebugBootstrap>().owner.state().random,random);assert!(!app.world().resource::<DebugBootstrap>().owner.accepts_imagemap_input_source(&source));assert!(!app.world().contains_resource::<crate::resources::VnEngine>());
}
#[test]
fn startup_imagemap0998_world_missing_image_refuses_without_publishing_the_completed_header_or_rng(){
 let(_,mut app)=fixture(&SCRIPT.replace("hover.png","missing.png"));let before=state(&app);let source=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();let mut prepared=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();
 let end=Instant::now()+Duration::from_secs(15);let error=loop{match refresh_world_step(app.world(),&mut prepared){Err(error)=>break error,Ok(false)=>{},Ok(true)=>panic!("An actual missing Init image cannot become ready")};app.update();assert!(Instant::now()<end,"Actual missing Image loader must settle");std::thread::sleep(Duration::from_millis(2));};
 assert!(error.contains("could not be loaded"));assert_eq!(state(&app),before);assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source));assert!(!app.world().resource::<DebugBootstrap>().publication_pending);assert!(!app.world().resource::<crate::resources::ImagemapState>().active);assert!(!app.world().contains_resource::<crate::resources::VnEngine>());
}
#[test]
fn startup_imagemap0998_world_replacing_actual_asset_server_cannot_reuse_old_decoded_handles_at_commit(){
 let(_,mut app)=fixture(SCRIPT);let before=state(&app);let source=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();let mut prepared=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();ready(&mut app,&mut prepared);
 let(_,mut other)=fixture(SCRIPT);let replacement=other.world_mut().remove_resource::<AssetServer>().unwrap();app.insert_resource(replacement);
 // Replacing the real server invalidates both image and text providers.
 // Prove the image guard itself, then preserve the earlier shared text guard
 // used by the complete transaction instead of requiring its later message.
 let image_error=imagemap_images::verify(app.world(),prepared.core().render_batch().unwrap().display.as_ref(),prepared.imagemap_images.as_ref().unwrap()).err().expect("Foreign actual image provider must refuse");
 assert_eq!(image_error,"Init input AssetServer changed before publication");
 let commit_error=commit_world_step(app.world_mut(),prepared).err().expect("Replaced actual AssetServer must refuse before publication");
 assert_eq!(commit_error,"Text AssetServer/source reader authority changed","Actual transaction refusal: {commit_error}");assert_eq!(state(&app),before);assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source));assert!(!app.world().resource::<DebugBootstrap>().publication_pending);assert!(!app.world().resource::<crate::resources::ImagemapState>().active);
}
#[test]
fn startup_imagemap0998_world_loaded_servers_with_equal_ids_and_paths_keep_distinct_strong_image_providers(){
 let(_,mut app)=fixture(SCRIPT);let mut prepared=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();ready(&mut app,&mut prepared);let before=state(&app);let source=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();
 let(_,mut other)=fixture(SCRIPT);let mut other_prepared=prepare_world_step(other.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();ready(&mut other,&mut other_prepared);
 for name in ["map.png","hover.png"]{
  let original=app.world().resource::<AssetServer>().get_handle::<Image>(name).unwrap();let replacement=other.world().resource::<AssetServer>().get_handle::<Image>(name).unwrap();assert_eq!(original.id(),replacement.id(),"Independent actual Image allocators repeat the same ordered IDs");
  let(Handle::Strong(original_strong),Handle::Strong(replacement_strong))=(&original,&replacement)else{panic!("Actual loaded AssetServer images must keep their strong producer handles")};assert!(!std::sync::Arc::ptr_eq(original_strong,replacement_strong));
  assert!(matches!(app.world().resource::<AssetServer>().get_load_state(original.id()),Some(bevy::asset::LoadState::Loaded)));assert!(matches!(other.world().resource::<AssetServer>().get_load_state(replacement.id()),Some(bevy::asset::LoadState::Loaded)));
  assert_eq!(app.world().resource::<Assets<Image>>().get(&original).unwrap().data,other.world().resource::<Assets<Image>>().get(&replacement).unwrap().data,"Identical decoded pixels cannot substitute for the original provider");
 }
 let replacement=other.world_mut().remove_resource::<AssetServer>().unwrap();app.insert_resource(replacement);
 // Equal IDs/paths/pixels do not authenticate the original strong provider.
 let image_error=imagemap_images::verify(app.world(),prepared.core().render_batch().unwrap().display.as_ref(),prepared.imagemap_images.as_ref().unwrap()).err().expect("Different loaded provider must refuse despite equal IDs");
 assert_eq!(image_error,"Init input AssetServer changed before publication");
 let image_refresh_error=imagemap_images::refresh(app.world(),prepared.core.render_batch().unwrap().display.as_ref(),prepared.imagemap_images.as_mut().unwrap()).err().expect("Staging must reject the same foreign strong provider");
 assert_eq!(image_refresh_error,"Init input AssetServer no longer owns the staged image path/handle");
 let refresh_error=refresh_world_step(app.world(),&mut prepared).err().expect("Full staging must retain its real text provider guard");
 assert_eq!(refresh_error,"Text AssetServer/source reader authority changed","Actual staging refusal: {refresh_error}");
 let commit_error=commit_world_step(app.world_mut(),prepared).err().expect("Full commit must retain its real text provider guard");
 assert_eq!(commit_error,"Text AssetServer/source reader authority changed","Actual transaction refusal: {commit_error}");assert_eq!(state(&app),before);assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source));assert!(!app.world().resource::<DebugBootstrap>().publication_pending);assert!(!app.world().resource::<crate::resources::ImagemapState>().active);
}
fn changed_render_metadata(view:bool){
 let(_,mut app)=fixture(SCRIPT);let mut warm=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();ready(&mut app,&mut warm);
 // The staging receipt owns the actual loaded Strong handles. AssetServer's
 // path lookup keeps only Weak handles; retain those same producers before
 // dropping the warm candidate so only rendering metadata changes below.
 let retained_images=["map.png","hover.png"].map(|path|{
  let handle=app.world().resource::<AssetServer>().get_handle::<Image>(path).expect("Warm actual receipt must own its loaded image producer");
  assert!(matches!(&handle,Handle::Strong(_)));assert!(matches!(app.world().resource::<AssetServer>().get_load_state(handle.id()),Some(bevy::asset::LoadState::Loaded)));
  assert!(app.world().resource::<Assets<Image>>().get(&handle).is_some());handle
 });
 let image=retained_images[0].clone();drop(warm);
app.world_mut().resource_mut::<Assets<Image>>().get_mut(&image).unwrap().sampler=bevy::render::texture::ImageSampler::nearest();
 let before=state(&app);let source=app.world().resource::<DebugBootstrap>().owner.observation_source().unwrap();let mut prepared=prepare_world_step(app.world(),rvn_core::editor_debugger::StepKind::Continue).unwrap();ready(&mut app,&mut prepared);
 let pixels=app.world().resource::<Assets<Image>>().get(&image).unwrap().data.clone();
 {
  let mut images=app.world_mut().resource_mut::<Assets<Image>>();let image=images.get_mut(&image).unwrap();
  if view{image.texture_view_descriptor=Some(bevy::render::render_resource::TextureViewDescriptor{dimension:Some(bevy::render::render_resource::TextureViewDimension::D2Array),..default()});}
  else{image.sampler=bevy::render::texture::ImageSampler::linear();}
 }
 assert_eq!(app.world().resource::<Assets<Image>>().get(&image).unwrap().data,pixels);assert_eq!(state(&app),before);
 assert!(refresh_world_step(app.world(),&mut prepared).err().unwrap().contains("rendering metadata"));assert!(commit_world_step(app.world_mut(),prepared).err().unwrap().contains("rendering metadata"));assert_eq!(state(&app),before);assert!(app.world().resource::<DebugBootstrap>().owner.accepts_observation_source(&source));assert!(!app.world().resource::<DebugBootstrap>().publication_pending);assert!(!app.world().resource::<crate::resources::ImagemapState>().active);drop(retained_images);
}
#[test]
fn startup_imagemap0998_world_sampler_only_mutation_refuses_before_owner_rng_and_publication_effects(){changed_render_metadata(false);}
#[test]
fn startup_imagemap0998_world_texture_view_only_mutation_refuses_before_owner_rng_and_publication_effects(){changed_render_metadata(true);}
