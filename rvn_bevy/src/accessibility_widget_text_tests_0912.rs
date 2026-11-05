use super::*;

fn app(scale: f32, contrast: bool) -> App {
    let mut app=App::new();
    let mut runtime=Accessibility::default();
    runtime.settings.text_scale=scale;runtime.settings.high_contrast=contrast;
    app.insert_resource(runtime).add_systems(Update,visuals);
    app
}
fn bundle(size: f32,color: Color)->TextBundle{
    TextBundle::from_section("Widget",TextStyle{font_size:size,color,..Default::default()})
}
#[test]
fn widget_text0912_first_frame_and_repeated_visuals_keep_one_authored_baseline(){
    let mut app=app(1.5,true);let color=Color::srgb(0.2,0.4,0.8);
    let id=app.world_mut().spawn(initialize_widget_text(bundle(20.0,color),&AccessibilitySettings{text_scale:1.5,high_contrast:true,..Default::default()})).id();
    assert_eq!(app.world().get::<Text>(id).unwrap().sections[0].style.font_size,30.0);
    assert_eq!(app.world().get::<Text>(id).unwrap().sections[0].style.color,Color::WHITE);
    assert_eq!(app.world().get::<OriginalText>(id).unwrap().0[0],(20.0,color,30.0,Color::WHITE));
    app.update();app.update();
    assert_eq!(app.world().get::<Text>(id).unwrap().sections[0].style.font_size,30.0);
    app.world_mut().resource_mut::<Accessibility>().settings.text_scale=2.0;
    app.world_mut().resource_mut::<Accessibility>().settings.high_contrast=false;app.update();
    assert_eq!(app.world().get::<Text>(id).unwrap().sections[0].style.font_size,40.0);
    assert_eq!(app.world().get::<Text>(id).unwrap().sections[0].style.color,color);
    reset_for_owner_replacement(app.world_mut());
    assert_eq!(app.world().get::<Text>(id).unwrap().sections[0].style.font_size,20.0);
    assert_eq!(app.world().get::<Text>(id).unwrap().sections[0].style.color,color);
}
#[test]
fn widget_text0912_authored_mutation_and_rich_section_update_keep_their_own_bases(){
    let mut app=app(1.5,true);let blue=Color::srgb(0.1,0.2,0.9);let red=Color::srgb(0.9,0.2,0.1);
    let settings=app.world().resource::<Accessibility>().settings.clone();
    let id=app.world_mut().spawn(initialize_widget_text(bundle(20.0,blue),&settings)).id();
    {
        let mut text=app.world_mut().get_mut::<Text>(id).unwrap();
        text.sections[0].style.font_size=24.0;text.sections[0].style.color=red;
        text.sections.push(bevy::text::TextSection::new(" child",TextStyle{font_size:12.0,color:blue,..Default::default()}));
    }
    app.update();app.update();
    let text=app.world().get::<Text>(id).unwrap();assert_eq!(text.sections[0].style.font_size,36.0);assert_eq!(text.sections[1].style.font_size,18.0);
    reset_for_owner_replacement(app.world_mut());
    let text=app.world().get::<Text>(id).unwrap();assert_eq!(text.sections[0].style.font_size,24.0);assert_eq!(text.sections[0].style.color,red);assert_eq!(text.sections[1].style.font_size,12.0);assert_eq!(text.sections[1].style.color,blue);
}
#[test]
fn widget_text0912_legacy_first_observation_stays_deferred(){
    let mut app=app(2.0,true);let color=Color::srgb(0.3,0.5,0.7);let id=app.world_mut().spawn(bundle(16.0,color)).id();
    app.update();assert_eq!(app.world().get::<Text>(id).unwrap().sections[0].style.font_size,16.0);assert_eq!(app.world().get::<Text>(id).unwrap().sections[0].style.color,color);
    app.update();assert_eq!(app.world().get::<Text>(id).unwrap().sections[0].style.font_size,32.0);assert_eq!(app.world().get::<Text>(id).unwrap().sections[0].style.color,Color::WHITE);
}
