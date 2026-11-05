//! Real source publication and Camera CPU metrics; no GPU/native observation.
use super::*;
pub(crate) fn install_actual_empty_owner_geometry(world:&mut World){
 let window=world.iter_entities().find(|e|e.contains::<bevy::window::PrimaryWindow>()).unwrap();let window_id=window.id();let window_value=window.get::<Window>().unwrap();
 let camera=world.iter_entities().find(|e|e.contains::<Camera2d>()).unwrap();let camera_id=camera.id();let camera_value=camera.get::<Camera>().unwrap();let target=camera_value.target.normalize(Some(window_id)).unwrap();
 let info=target.get_render_target_info(std::iter::once((window_id,window_value)),world.resource::<Assets<Image>>(),world.resource::<bevy::render::camera::ManualTextureViews>()).unwrap();
 let metric=actual_widget_camera_metrics(camera_id,camera_value,world.resource::<bevy::ui::UiScale>().0,&rvn_ui::accessibility::AccessibilitySettings::default(),target,info).unwrap();
 let projection=world.resource::<OwnerSceneProjection>().projection.clone();assert!(projection.descriptions().is_empty());let plan=prepare_geometry(world,projection,metric,&HashMap::new()).unwrap();let batch=world.resource::<crate::scene_publication::ScenePresentation>().batch.clone().unwrap();install_preflighted(world,&batch,plan).unwrap();
}
