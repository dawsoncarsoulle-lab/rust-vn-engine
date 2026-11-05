use super::*;

fn setup() -> (App, Handle<Font>) {
    let mut app = App::new();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin {
        file_path: root.to_str().unwrap().to_owned(), watch_for_changes_override: Some(false), ..default()
    })).init_asset::<Font>().register_asset_loader(bevy::text::FontLoader::default());
    let font = app.world_mut().resource_mut::<Assets<Font>>().add(
        Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap()
    );
    (app, font)
}

fn view() -> ScreenView {
    ScreenView {name:"Font0956".into(), modal:false, layer:0, order:0, focus:None,
        root:UiComponent::parse(serde_json::json!({"id":"label","kind":"text","layout_contract":"widget_v1","text":"AVATAR","font_size":18})).unwrap()}
}

#[test]
fn widget0956_measure_and_spawn_share_real_font_units_and_one_accessibility_scale() {
    let (mut app, font) = setup();
    let assets = app.world().resource::<AssetServer>().clone();
    let authored = view();
    let before = authored.clone();
    let baseline = widget_font_units_0956::point_size_to_logical_px(app.world().resource::<Assets<Font>>().get(&font), 18.0).unwrap();
    for text_scale in [1.0, 1.5, 2.0] {
        let resolved = widget_render_text_view(authored.clone(), &assets, app.world().resource::<Assets<Font>>(), Some(&font)).unwrap();
        assert_eq!(resolved.root.font_size, baseline);
        for physical_scale in [1.0, 1.25, 2.0] {
            let fonts = app.world().resource::<Assets<Font>>();
            let measured = widget_text_size(&authored.root, &font, fonts, text_scale, physical_scale).unwrap();
            let text = Text::from_section("AVATAR", TextStyle {font:font.clone(),font_size:baseline*text_scale,color:Color::WHITE}).with_no_wrap();
            let expected = bevy::text::TextMeasureInfo::from_text(&text,fonts,physical_scale).unwrap().compute_size(Vec2::INFINITY)/physical_scale;
            assert!((Vec2::from_array(measured)-expected).length()<0.001);
        }
        let parent = app.world_mut().spawn(NodeBundle::default()).id();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let settings = rvn_ui::accessibility::AccessibilitySettings {text_scale,..default()};
        {let mut commands=Commands::new(&mut queue,app.world());spawn(&mut commands,parent,&resolved,&resolved.root,true,&assets,Some(&font),Vec2::new(800.0,600.0),&Editing::default(),&HashMap::new(),1.0,0.0,0,None,None,Some(&settings));}
        queue.apply(app.world_mut());
        let entity = app.world().entity(parent).get::<Children>().unwrap()[0];
        let text_entity = app.world().entity(entity).get::<Children>().unwrap()[0];
        let text = app.world().entity(text_entity).get::<Text>().unwrap();
        assert_eq!(text.sections[0].style.font_size,baseline*text_scale);
        assert_eq!(text.sections[0].style.font,font);
    }
    assert_eq!(authored,before,"The conversion belongs to the private projection, not the source");
}

#[test]
fn widget0956_missing_inherited_asset_refuses_whole_private_projection() {
    let (app,font)=setup();let mut authored=view();authored.root.font=Some("unavailable/font0956.ttf".into());
    let before=authored.clone();
    let result=widget_render_text_view(authored.clone(),app.world().resource::<AssetServer>(),app.world().resource::<Assets<Font>>(),Some(&font));
    assert!(matches!(result,Err(WidgetLayoutProblem::Pending(_))));assert_eq!(authored,before);
}

#[test]
fn widget0956_legacy_projection_keeps_existing_numeric_font_size() {
    let (app,font)=setup();let mut legacy=view();legacy.root.layout_contract=None;
    let before=legacy.clone();let resolved=widget_render_text_view(legacy,app.world().resource::<AssetServer>(),app.world().resource::<Assets<Font>>(),Some(&font)).unwrap();assert_eq!(resolved,before);
}
