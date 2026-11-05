//! Real Font assets and Bevy's own text measure; no graphics/native claim.
use super::*;
fn fonts()->(Assets<Font>,Handle<Font>){let mut fonts=Assets::default();let handle=fonts.add(Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap());(fonts,handle)}
fn text(value:&str)->UiComponent{UiComponent::parse(serde_json::json!({"id":"text","kind":"text","text":value,"font_size":24,"layout_contract":"widget_v1","spacing":0})).unwrap()}
fn view()->ScreenView{ScreenView{name:"Widget0912".into(),modal:false,layer:0,order:0,focus:None,root:UiComponent::parse(serde_json::json!({"id":"root","kind":"column","layout_contract":"widget_v1","spacing":0,"children":[{"id":"button","kind":"button","spacing":0,"parent_slot":{"kind":"vertical_box","padding":{"left":8,"right":8,"top":8,"bottom":8}},"children":[{"id":"label","kind":"text","spacing":0,"text":"AVATAR","font_size":24,"parent_slot":{"kind":"button","padding":{"left":6,"right":6,"top":6,"bottom":6},"horizontal":"center","vertical":"center"}}]}]})).unwrap()}}
fn assets()->App{
    // Actual existing fixture resource root, independent of the harness cwd.
    // A CPU-only layout test does not need a filesystem change watcher.
    let root=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");assert!(root.is_dir());assert!(root.join("DejaVuSans.ttf").is_file());
    let mut app=App::new();app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin{file_path:root.to_str().expect("fixture resource path must be Unicode").to_owned(),watch_for_changes_override:Some(false),..default()})).init_asset::<Font>().register_asset_loader(bevy::text::FontLoader::default());app
}
fn engine()->rvn_core::Engine<crate::bevy_renderer::BevyRenderer>{
    let source=r#"
screen Widget0912() { return {"id":"root","kind":"column","layout_contract":"widget_v1","spacing":0,"children":[{"id":"button","kind":"button","spacing":0,"parent_slot":{"kind":"vertical_box","padding":{"left":8,"right":8,"top":8,"bottom":8}},"children":[{"id":"label","kind":"text","spacing":0,"text":"AVATAR","font_size":24,"parent_slot":{"kind":"button","padding":{"left":6,"right":6,"top":6,"bottom":6},"horizontal":"center","vertical":"center"}}]}]} }
screen Neighbour0912() { return {"id":"root","kind":"column","layout_contract":"widget_v1","spacing":0,"children":[{"id":"button","kind":"button","spacing":0,"parent_slot":{"kind":"vertical_box"},"children":[{"id":"label","kind":"text","spacing":0,"text":"Neighbour","parent_slot":{"kind":"button","padding":{"left":4,"right":4,"top":2,"bottom":2},"horizontal":"center","vertical":"center"}}]}]} }
label start
 ui.open("Widget0912",[],false,0)
 ui.open("Neighbour0912",[],false,0)
 "Waiting"
"#;
    let mut engine=rvn_core::Engine::new(rvn_parser::parse(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();engine.step_until_interaction().unwrap();engine.renderer.scene_batch=None;engine
}
fn setup_camera(app:&mut App){
    use bevy::ecs::system::RunSystemOnce;
    app.world_mut().spawn((Window{resolution:bevy::window::WindowResolution::new(840.0,300.0).with_scale_factor_override(1.5),..default()},bevy::window::PrimaryWindow));
    app.world_mut().spawn((Camera::default(),OrthographicProjection::default()));
    app.world_mut().run_system_once(bevy::render::camera::camera_system::<OrthographicProjection>);
}
fn layout_app()->(App,Handle<Font>){
    let mut app=assets();app.init_asset::<Image>().init_resource::<bevy::render::camera::ManualTextureViews>()
        .add_event::<bevy::window::WindowResized>().add_event::<bevy::window::WindowCreated>().add_event::<bevy::window::WindowScaleFactorChanged>()
        .init_resource::<Screens>().init_resource::<WidgetLayouts>().init_resource::<crate::accessibility::Accessibility>().insert_resource(bevy::ui::UiScale(2.0)).insert_resource(State::new(VnState::Waiting));
    let font=app.world_mut().resource_mut::<Assets<Font>>().add(Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap());app.insert_resource(crate::menu_documents::MenuFont(font.clone()));
    let engine=engine();app.world_mut().resource_mut::<Screens>().views=engine.interface_views().unwrap();app.insert_resource(VnEngine(engine));(app,font)
}
fn actual_sources(app:&App)->Vec<UiInputSource>{app.world().resource::<Screens>().views.iter().map(|view|app.world().resource::<VnEngine>().0.capture_ui_input_source(&view.name).unwrap()).collect()}
fn button_press(entity:Entity,source:&UiInputSource)->PendingPress{PendingPress{entity,control:Control{screen:source.screen.clone(),element:"button".into(),option:None},source:source.clone(),component:[0;32],image:None,pointer:None,action:UiInput{screen:source.screen.clone(),element:"button".into(),kind:EventKind::Click,value:None,key:None}}}
fn near(actual:f32,expected:f32){assert!((actual-expected).abs()<0.001,"{actual} != {expected}");}
#[test]
fn widget0912_measure_is_the_actual_bevy_font_kerning_and_multiline_engine(){
    let(fonts,font)=fonts();for value in["AVATAR","AV AV","éàç Ω","line one\nline two","","\n","\n\n"," ","\t"]{let node=text(value);let measured=widget_text_size(&node,&font,&fonts,1.0,1.25).unwrap_or_else(|error|panic!("actual content {value:?}: {error:?}"));let actual=Text::from_section(value,TextStyle{font:font.clone(),font_size:widget_font_base_size(&node,&font,&fonts).unwrap(),color:Color::WHITE}).with_no_wrap();let raw=bevy::text::TextMeasureInfo::from_text(&actual,&fonts,1.25).unwrap().compute_size(Vec2::INFINITY)/1.25;
        // Run the real runtime CPU text pipeline for every case, including
        // newline-only/control-only and a space with real advance/no outline.
        let mut pipeline=bevy::text::TextPipeline::default();let mut atlas=bevy::text::FontAtlasSets::default();let mut layouts=Assets::<bevy::sprite::TextureAtlasLayout>::default();let mut images=Assets::<Image>::default();
        let layout=pipeline.queue_text(&fonts,&actual.sections,1.25,actual.justify,actual.linebreak_behavior,Vec2::INFINITY,&mut atlas,&mut layouts,&mut images,&bevy::text::TextSettings::default(),bevy::text::YAxisOrientation::TopToBottom).unwrap();let expected=layout.logical_size/1.25;
        eprintln!("widget0916 actual text={value:?} TextMeasureInfo={raw:?} queue_text={expected:?} positioned_glyphs={}",layout.glyphs.len());
        if raw.is_finite()&&raw.min_element()>=0.0{near(measured[0],raw.x);}else{assert_eq!(expected,Vec2::ZERO,"No glyph output is distinct from a present empty line box for {value:?}");}
        use ab_glyph::{Font as _,ScaleFont as _};
        let font_height=widget_font_base_size(&node,&font,&fonts).unwrap()*1.25;
        let scaled=fonts.get(&font).unwrap().font.as_scaled(font_height);
        let line_height=(scaled.height()+scaled.line_gap())/1.25;
        let lines=value.chars().filter(|character|*character=='\n').count()+1;
        near(measured[1],line_height*lines as f32);assert!(measured[1]>0.0);
        if value==" "{assert!(expected.x>0.0,"A space keeps its real advance despite no outlined glyph");}
        near(measured[0],expected.x);}
    assert!(widget_text_size(&text("WWW"),&font,&fonts,1.0,1.0).unwrap()[0]>widget_text_size(&text("iii"),&font,&fonts,1.0,1.0).unwrap()[0],"actual proportional font cannot be replaced by character count");
}
#[test]
fn widget0912_missing_font_waits_and_missing_glyph_or_invalid_scale_refuses_without_fallback(){
    let(mut fonts,font)=fonts();let node=text("AV");let removed=fonts.remove(font.id()).unwrap();for value in["","\n","\n\n"," ","\t"]{assert!(matches!(widget_text_size(&text(value),&font,&fonts,1.0,1.0),Err(WidgetLayoutProblem::Pending(_))));}assert!(matches!(widget_text_size(&node,&font,&fonts,1.0,1.0),Err(WidgetLayoutProblem::Pending(_))));fonts.insert(font.id(),removed);assert!(widget_text_size(&node,&font,&fonts,1.0,1.0).is_ok());
    assert!(matches!(widget_text_size(&text("\u{10FFFF}"),&font,&fonts,1.0,1.0),Err(WidgetLayoutProblem::Invalid(message))if message.contains("U+10FFFF")));
    for(scale,dpi)in[(0.0,1.0),(f32::NAN,1.0),(1.0,0.0),(1.0,f32::INFINITY)]{for value in["AV","","\n","\n\n"," ","\t"]{assert!(matches!(widget_text_size(&text(value),&font,&fonts,scale,dpi),Err(WidgetLayoutProblem::Invalid(_))));}}
}
#[test]
fn widget0912_shared_slot_layout_uses_real_font_and_parent_local_rectangles_without_mutating_definition(){
    let mut app=assets();let font=app.world_mut().resource_mut::<Assets<Font>>().add(Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap());let assets=app.world().resource::<AssetServer>();let fonts=app.world().resource::<Assets<Font>>();let view=view();let before=serde_json::to_vec(&view.root).unwrap();let bounds=[0.0,0.0,280.0,100.0];let actual=widget_layout_view(&view,bounds,1.0,1.0,assets,&fonts,Some(&font)).unwrap();let size=widget_text_size(view.root.find("label").unwrap(),&font,&fonts,1.0,1.0).unwrap();let expected=rvn_ui::programmable::layout_widget_rects(&view.root,bounds,&mut|node|widget_text_size(node,&font,&fonts,1.0,1.0).map_err(|error|error.message().into())).unwrap();
    for rect in expected{let component=actual.root.find(&rect.id).unwrap();if rect.id=="root"{assert_eq!(component.rect,Some(rect.rect));}}
    let button=actual.root.find("button").unwrap().rect.unwrap();let label=actual.root.find("label").unwrap().rect.unwrap();near(button[0],8.0);near(button[1],8.0);near(button[2],264.0);near(button[3],size[1]+12.0);near(label[0],(264.0-size[0])*0.5);near(label[1],6.0);near(label[2],size[0]);near(label[3],size[1]);assert_eq!(serde_json::to_vec(&view.root).unwrap(),before);assert_eq!(view.root.find("button").unwrap().children.len(),1);
}
#[test]
fn widget0912_font_size_and_dpi_are_measured_once_in_real_logical_widget_units(){
    let mut app=assets();let font=app.world_mut().resource_mut::<Assets<Font>>().add(Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap());let assets=app.world().resource::<AssetServer>();let fonts=app.world().resource::<Assets<Font>>();let view=view();for dpi in[1.0,1.25,1.5,2.0]{let normal=widget_layout_view(&view,[0.0,0.0,280.0,100.0],1.0,dpi,assets,&fonts,Some(&font)).unwrap();let enlarged=widget_layout_view(&view,[0.0,0.0,280.0,100.0],2.0,dpi,assets,&fonts,Some(&font)).unwrap();let a=normal.root.find("label").unwrap().rect.unwrap();let b=enlarged.root.find("label").unwrap().rect.unwrap();near(b[2],a[2]*2.0);near(b[3],a[3]*2.0);assert_eq!(enlarged.root.find("label").unwrap().font_size,24.0);assert_eq!(enlarged.root.rect,Some([0.0,0.0,280.0,100.0]));}
}
#[test]
fn widget0912_inherited_custom_font_is_required_and_does_not_fall_back_to_menu_font(){
    let mut app=assets();let font=app.world_mut().resource_mut::<Assets<Font>>().add(Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap());let assets=app.world().resource::<AssetServer>();let fonts=app.world().resource::<Assets<Font>>();let mut view=view();view.root.font=Some("missing/widget0912.ttf".into());let before=view.clone();let result=widget_layout_view(&view,[0.0,0.0,280.0,100.0],1.0,1.0,assets,&fonts,Some(&font));let authored=assets.get_handle::<Font>("missing/widget0912.ttf").expect("the real requested authored-font handle");assert_ne!(authored.id(),font.id(),"The actual AssetServer and Assets store share one handle provider");assert!(fonts.get(authored.id()).is_none());assert!(matches!(result,Err(WidgetLayoutProblem::Pending(_))|Err(WidgetLayoutProblem::Invalid(_))));assert_eq!(view,before);
}


#[test]
fn widget0912_actual_camera_preflight_waits_then_uses_dpi_uiscale_and_rejects_stale_definition(){
    use bevy::ecs::system::RunSystemOnce;
    let(mut app,font)=layout_app();
    app.world_mut().run_system_once(prepare_widget_layouts);assert!(!widget_geometry_ready(app.world()));assert!(widget_layout_problem(app.world()).unwrap().contains("camera"));assert!(app.world().resource::<WidgetLayouts>().plans.is_empty());
    setup_camera(&mut app);app.world_mut().run_system_once(prepare_widget_layouts);assert!(widget_geometry_ready(app.world()));let layout=app.world().resource::<WidgetLayouts>();let metric=layout.metrics.as_ref().unwrap();assert_eq!(metric.bounds,[0.0,0.0,280.0,100.0]);assert_eq!(metric.physical_scale,3.0);let actual=actual_sources(&app);assert!(layout.plans.contains_key(&actual[0].to_wire().unwrap()));assert!(layout.plans.values().all(|plan|actual.contains(&plan.source)));
    app.world_mut().resource_mut::<Screens>().views[0].root.find_mut("label").unwrap().text="New actual definition".into();assert!(!widget_geometry_ready(app.world()),"A Ready for the old source cannot publish a newly changed definition");app.world_mut().run_system_once(prepare_widget_layouts);assert!(widget_geometry_ready(app.world()));
    let font=app.world().resource::<WidgetLayouts>().default_font.clone().unwrap();
    let removed=app.world_mut().resource_mut::<Assets<Font>>().remove(font.id()).unwrap();app.world_mut().run_system_once(prepare_widget_layouts);assert!(!widget_geometry_ready(app.world()));assert!(widget_layout_problem(app.world()).unwrap().contains("Font asset"));app.world_mut().resource_mut::<Assets<Font>>().insert(font.id(),removed);app.world_mut().run_system_once(prepare_widget_layouts);assert!(widget_geometry_ready(app.world()));
}

#[test]
fn widget0912_real_spawn_has_one_content_text_exact_first_frame_and_no_legacy_minimums(){
    for font_size in [24.0,0.5]{
    let mut app=assets();let assets=app.world().resource::<AssetServer>().clone();let font=app.world_mut().resource_mut::<Assets<Font>>().add(Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap());let mut view=view();view.root.find_mut("label").unwrap().font_size=font_size;let resolved=widget_layout_view(&view,[0.0,0.0,280.0,100.0],2.0,1.0,&assets,app.world().resource::<Assets<Font>>(),Some(&font)).unwrap();let resolved=widget_render_text_view(resolved,&assets,app.world().resource::<Assets<Font>>(),Some(&font)).unwrap();let root=app.world_mut().spawn(NodeBundle::default()).id();let mut queue=bevy::ecs::world::CommandQueue::default();let settings=rvn_ui::accessibility::AccessibilitySettings{text_scale:2.0,..default()};
    {let mut commands=Commands::new(&mut queue,app.world());spawn(&mut commands,root,&resolved,&resolved.root,true,&assets,Some(&font),Vec2::new(1400.0,500.0),&Editing::default(),&HashMap::new(),1.0,0.0,0,None,None,Some(&settings));}queue.apply(app.world_mut());
    let texts=app.world_mut().query::<&Text>().iter(app.world()).collect::<Vec<_>>();assert_eq!(texts.len(),1,"Widget Button must not create an empty or duplicate legacy Text entity");assert_eq!(texts[0].sections[0].value,"AVATAR");assert_eq!(texts[0].sections[0].style.font_size,widget_font_units_0956::point_size_to_logical_px(app.world().resource::<Assets<Font>>().get(&font),font_size).unwrap()*2.0);assert_eq!(texts[0].sections[0].style.font,font);assert_eq!(texts[0].linebreak_behavior,bevy::text::BreakLineOn::NoWrap);
    let button=app.world_mut().query::<(&crate::composed_motion::InterfaceTarget,&Style)>().iter(app.world()).find(|(target,_)|target.element=="button").unwrap().1;assert_eq!(button.width,Val::Px(264.0));assert_eq!(button.min_width,Val::Auto);assert_eq!(button.min_height,Val::Auto);near(if let Val::Px(value)=button.height{value}else{panic!("actual rect")},resolved.root.find("button").unwrap().rect.unwrap()[3]);
    assert!(std::panic::catch_unwind(||layout_view_with_text_scale(&view,1.0)).is_err(),"The reference-pixel legacy helper must not be an implicit Widget fallback");
    }
}

#[test]
fn widget0912_legacy_leaf_button_keeps_existing_spawn_and_layout_behavior(){
    let mut app=assets();let assets=app.world().resource::<AssetServer>().clone();let font=app.world_mut().resource_mut::<Assets<Font>>().add(Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap());let view=ScreenView{root:UiComponent::parse(serde_json::json!({"id":"legacy","kind":"button","text":"Legacy"})).unwrap(),..view()};let resolved=layout_view_with_text_scale(&view,1.0);assert_eq!(resolved,layout_view(&view));let root=app.world_mut().spawn(NodeBundle::default()).id();let mut queue=bevy::ecs::world::CommandQueue::default();
    {let mut commands=Commands::new(&mut queue,app.world());spawn(&mut commands,root,&resolved,&resolved.root,true,&assets,Some(&font),Vec2::new(1920.0,1080.0),&Editing::default(),&HashMap::new(),1.0,0.0,0,None,None,None);}queue.apply(app.world_mut());let texts=app.world_mut().query::<&Text>().iter(app.world()).collect::<Vec<_>>();assert_eq!(texts.len(),1);assert_eq!(texts[0].sections[0].value,"Legacy");assert_eq!(texts[0].sections[0].style.font_size,24.0);
}

#[test]
fn widget0912_source_identity_rejects_ambiguous_mount_and_waits_for_real_core_port(){
    use bevy::ecs::system::RunSystemOnce;
    let(mut app,_)=layout_app();setup_camera(&mut app);let duplicate=app.world().resource::<Screens>().views[0].clone();app.world_mut().resource_mut::<Screens>().views.push(duplicate);app.world_mut().run_system_once(prepare_widget_layouts);assert!(!widget_geometry_ready(app.world()));assert!(widget_layout_problem(app.world()).unwrap().contains("per-mount identity"));
    app.world_mut().resource_mut::<Screens>().views.pop();app.world_mut().remove_resource::<VnEngine>();app.world_mut().run_system_once(prepare_widget_layouts);assert!(!widget_geometry_ready(app.world()));assert!(widget_layout_problem(app.world()).unwrap().contains("Core input-source"));assert!(app.world().resource::<WidgetLayouts>().plans.is_empty());
}
#[test]
fn widget0912_revocation_uses_real_source_not_shared_local_id_and_preserves_neighbour_captures(){
    let(mut app,_)=layout_app();let sources=actual_sources(&app);assert_ne!(sources[0],sources[1]);let old=app.world_mut().spawn_empty().id();let mut screens=app.world_mut().resource_mut::<Screens>();
    for source in &sources{screens.rendered_sources.insert(source.screen.clone(),source.clone());screens.frame_sources.insert(source.screen.clone(),source.clone());}
    screens.pending_press=Some(button_press(old,&sources[0]));screens.dragging=Some(Dragging{screen:sources[0].screen.clone(),element:"button".into()});screens.dragging_source=Some(sources[0].clone());screens.keyboard_focus=Some((sources[1].screen.clone(),"button".into()));screens.input_focus=screens.keyboard_focus.clone();
    for(index,source)in sources.iter().enumerate(){let producer=std::sync::Arc::new(NativeKeyProducer{focus:Some((source.screen.clone(),"button".into())),source:Some(source.clone()),sources:sources.iter().map(|source|(source.screen.clone(),source.clone())).collect(),key_edges:[KeyCode::Enter].into_iter().collect(),component_kind:Some(Kind::Button),canvas_token:None,preview_focus_order:Some(sources.iter().map(|source|(source.screen.clone(),"button".into())).collect()),preview_focus_kinds:None,preview_canvas_tokens:None});screens.key_producers.insert(index,producer.clone());screens.ime_producers.insert(index,producer);}
    revoke_widget_sources(&mut screens,&sources[..1]);assert!(screens.pending_press.is_none());assert!(screens.dragging.is_none());assert!(screens.dragging_source.is_none());assert_eq!(screens.keyboard_focus,Some((sources[1].screen.clone(),"button".into())));assert_eq!(screens.input_focus,screens.keyboard_focus);assert!(!screens.frame_sources.contains_key(&sources[0].screen));assert_eq!(screens.frame_sources.get(&sources[1].screen),Some(&sources[1]));
    for producers in[&screens.key_producers,&screens.ime_producers]{assert!(producers[&0].source.is_none());assert!(producers[&0].key_edges.is_empty());assert_eq!(producers[&1].source,Some(sources[1].clone()));assert!(producers[&1].key_edges.contains(&KeyCode::Enter));assert_eq!(producers[&1].preview_focus_order,Some(vec![(sources[1].screen.clone(),"button".into())]));}
    screens.pending_press=Some(button_press(old,&sources[1]));revoke_widget_sources(&mut screens,&sources[..1]);assert_eq!(screens.pending_press.as_ref().unwrap().source,sources[1],"Revoking an unrelated owner cannot cancel the neighbouring press");
}
#[test]
fn widget0912_definition_rebuild_removes_old_press_entity_but_preserves_real_neighbour_root(){
    use bevy::ecs::system::RunSystemOnce;
    let(mut app,_)=layout_app();setup_camera(&mut app);app.world_mut().run_system_once(prepare_widget_layouts);app.world_mut().run_system_once(render);let sources=actual_sources(&app);
    let roots=app.world_mut().query::<(Entity,&WidgetRootSource)>().iter(app.world()).map(|(entity,source)|(source.0.to_wire().unwrap(),entity)).collect::<HashMap<_,_>>();assert_eq!(roots.len(),2);
    let control=app.world_mut().query::<(Entity,&ControlSource,&Control)>().iter(app.world()).find(|(_,source,control)|source.0==sources[0]&&control.element=="button").unwrap().0;
    {let mut screens=app.world_mut().resource_mut::<Screens>();screens.pending_press=Some(button_press(control,&sources[0]));screens.keyboard_focus=Some((sources[1].screen.clone(),"button".into()));screens.views[0].root.find_mut("label").unwrap().text="New geometry".into();}app.insert_resource(bevy::a11y::Focus(Some(control)));
    app.world_mut().run_system_once(prepare_widget_layouts);assert!(app.world().get_entity(control).is_none());assert!(app.world().resource::<bevy::a11y::Focus>().0.is_none());assert!(app.world().resource::<Screens>().pending_press.is_none(),"A delayed release/continuation cannot reuse the same local button after geometry changes");app.world_mut().run_system_once(render);
    let next=app.world_mut().query::<(Entity,&WidgetRootSource)>().iter(app.world()).map(|(entity,source)|(source.0.to_wire().unwrap(),entity)).collect::<HashMap<_,_>>();assert_eq!(next.len(),2);assert_ne!(next[&sources[0].to_wire().unwrap()],roots[&sources[0].to_wire().unwrap()]);assert_eq!(next[&sources[1].to_wire().unwrap()],roots[&sources[1].to_wire().unwrap()]);assert_eq!(app.world().resource::<Screens>().keyboard_focus,Some((sources[1].screen.clone(),"button".into())));
}
#[test]
fn widget0912_actual_metrics_changes_revoke_geometry_and_font_reload_requires_real_rebind(){
    use bevy::ecs::system::RunSystemOnce;
    let(mut app,font)=layout_app();setup_camera(&mut app);app.world_mut().run_system_once(prepare_widget_layouts);app.world_mut().run_system_once(render);let sources=actual_sources(&app);
    app.world_mut().resource_mut::<Screens>().pending_press=Some(button_press(Entity::PLACEHOLDER,&sources[0]));app.world_mut().resource_mut::<bevy::ui::UiScale>().0=1.0;assert!(!widget_geometry_ready(app.world()));app.world_mut().run_system_once(prepare_widget_layouts);assert!(app.world().resource::<Screens>().pending_press.is_none());assert!(widget_geometry_ready(app.world()));app.world_mut().run_system_once(render);
    app.world_mut().resource_mut::<Screens>().pending_press=Some(button_press(Entity::PLACEHOLDER,&sources[0]));app.world_mut().resource_mut::<crate::accessibility::Accessibility>().settings.text_scale=1.5;assert!(!widget_geometry_ready(app.world()));app.world_mut().run_system_once(prepare_widget_layouts);assert!(app.world().resource::<Screens>().pending_press.is_none());app.world_mut().run_system_once(render);
    let font=app.world().resource::<WidgetLayouts>().default_font.clone().unwrap();
    let alternate=Font::try_from_bytes(rvn_ui::widget_fonts::face(rvn_ui::widget_fonts::WidgetBuiltinFace::DefaultText).bytes.to_vec()).unwrap();let original=app.world_mut().resource_mut::<Assets<Font>>().remove(font.id()).unwrap();app.world_mut().run_system_once(prepare_widget_layouts);assert!(!widget_geometry_ready(app.world()));assert!(app.world().resource::<WidgetLayouts>().plans.is_empty());app.world_mut().resource_mut::<Assets<Font>>().insert(font.id(),alternate);app.world_mut().run_system_once(prepare_widget_layouts);assert!(widget_geometry_ready(app.world()),"An identical-byte restored real Font may become ready again");app.world_mut().run_system_once(render);drop(original);
    // Valid same-glyph TrueType file with one trailing padding byte: a real
    // changed binding must not use Bevy's old FontId/atlas cache silently.
    let mut changed=rvn_ui::widget_fonts::face(rvn_ui::widget_fonts::WidgetBuiltinFace::DefaultText).bytes.to_vec();changed.push(0);let changed=Font::try_from_bytes(changed).unwrap();app.world_mut().resource_mut::<Assets<Font>>().insert(font.id(),changed);assert!(!widget_geometry_ready(app.world()));app.world_mut().run_system_once(prepare_widget_layouts);assert!(widget_layout_problem(app.world()).unwrap().contains("fresh renderer font binding"));assert!(app.world().resource::<WidgetLayouts>().plans.is_empty());app.world_mut().run_system_once(render);app.world_mut().run_system_once(prepare_widget_layouts);assert!(widget_layout_problem(app.world()).unwrap().contains("fresh renderer font binding"),"Removing Widget entities does not reset Bevy's FontId cache receipt");

}
#[test]
fn widget0912_same_alias_order_after_engine_replacement_cannot_reuse_old_layout_source(){
    use bevy::ecs::system::RunSystemOnce;
    let(mut app,_)=layout_app();setup_camera(&mut app);app.world_mut().run_system_once(prepare_widget_layouts);app.world_mut().run_system_once(render);let previous=actual_sources(&app);app.world_mut().resource_mut::<Screens>().pending_press=Some(button_press(Entity::PLACEHOLDER,&previous[0]));let replacement=engine();let current=replacement.capture_ui_input_source(&previous[0].screen).unwrap();assert_eq!(previous[0].screen,current.screen);assert_eq!(previous[0].order,current.order);assert_ne!(previous[0].lineage,current.lineage);app.insert_resource(VnEngine(replacement));assert!(!widget_geometry_ready(app.world()));app.world_mut().run_system_once(prepare_widget_layouts);assert!(widget_geometry_ready(app.world()));assert!(app.world().resource::<Screens>().pending_press.is_none());assert!(!app.world().resource::<WidgetLayouts>().plans.contains_key(&previous[0].to_wire().unwrap()));assert!(app.world().resource::<WidgetLayouts>().plans.contains_key(&current.to_wire().unwrap()));
}

#[test]
fn widget0912_raw_target_dpi_change_waits_for_actual_camera_and_revokes_old_geometry(){
    use bevy::ecs::system::RunSystemOnce;
    let(mut app,_)=layout_app();setup_camera(&mut app);app.world_mut().run_system_once(prepare_widget_layouts);app.world_mut().run_system_once(render);let source=actual_sources(&app)[0].clone();app.world_mut().resource_mut::<Screens>().pending_press=Some(button_press(Entity::PLACEHOLDER,&source));
    let window=app.world_mut().query_filtered::<Entity,With<Window>>().single(app.world());app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_scale_factor_override(Some(2.0));assert!(!widget_geometry_ready(app.world()),"The last computed camera cannot attest a raw Window DPI change");app.world_mut().run_system_once(prepare_widget_layouts);assert!(app.world().resource::<Screens>().pending_press.is_none());assert!(widget_layout_problem(app.world()).unwrap().contains("actual render target"));app.world_mut().resource_mut::<Events<bevy::window::WindowScaleFactorChanged>>().send(bevy::window::WindowScaleFactorChanged{window,scale_factor:2.0});app.world_mut().run_system_once(bevy::render::camera::camera_system::<OrthographicProjection>);app.world_mut().run_system_once(prepare_widget_layouts);assert!(widget_geometry_ready(app.world()));assert_eq!(app.world().resource::<WidgetLayouts>().metrics.as_ref().unwrap().physical_scale,4.0);
}

#[test]
fn widget0912_one_missing_inherited_font_removes_only_its_real_owner_and_keeps_neighbour_entity(){
    use bevy::ecs::system::RunSystemOnce;
    let(mut app,_)=layout_app();setup_camera(&mut app);app.world_mut().run_system_once(prepare_widget_layouts);app.world_mut().run_system_once(render);let sources=actual_sources(&app);let neighbour=app.world_mut().query::<(Entity,&WidgetRootSource)>().iter(app.world()).find(|(_,source)|source.0==sources[1]).unwrap().0;
    app.world_mut().resource_mut::<Screens>().views[0].root.font=Some("not-present/widget0912.ttf".into());app.world_mut().run_system_once(prepare_widget_layouts);assert!(!widget_geometry_ready(app.world()));assert!(!app.world().resource::<WidgetLayouts>().plans.contains_key(&sources[0].to_wire().unwrap()));assert!(app.world().resource::<WidgetLayouts>().plans.contains_key(&sources[1].to_wire().unwrap()));app.world_mut().run_system_once(render);let roots=app.world_mut().query::<(Entity,&WidgetRootSource)>().iter(app.world()).map(|(entity,source)|(entity,source.0.clone())).collect::<Vec<_>>();assert_eq!(roots,vec![(neighbour,sources[1].clone())]);
}
#[test]
fn widget0912_real_modal_context_change_cannot_reuse_old_interactive_geometry(){
    use bevy::ecs::system::RunSystemOnce;
    let(mut app,_)=layout_app();setup_camera(&mut app);app.world_mut().run_system_once(prepare_widget_layouts);app.world_mut().run_system_once(render);let sources=actual_sources(&app);assert!(app.world().resource::<WidgetLayouts>().plans[&sources[0].to_wire().unwrap()].interactive);
    // Actual Core presentation flags, not a second renderer modal model.
    app.world_mut().resource_mut::<VnEngine>().0.state.ui.screens.iter_mut().find(|screen|screen.name==sources[1].screen).unwrap().modal=true;let views=app.world().resource::<VnEngine>().0.interface_views().unwrap();app.world_mut().resource_mut::<Screens>().views=views;assert!(!widget_geometry_ready(app.world()));app.world_mut().run_system_once(prepare_widget_layouts);assert!(!app.world().resource::<WidgetLayouts>().plans[&sources[0].to_wire().unwrap()].interactive);assert!(app.world().resource::<WidgetLayouts>().plans[&sources[1].to_wire().unwrap()].interactive);app.world_mut().run_system_once(render);assert_eq!(app.world_mut().query::<&WidgetRootSource>().iter(app.world()).count(),2);
}

#[path = "programmable_horizontal_box_tests_0954.rs"]
mod horizontal_box_0954;

#[test]
fn widget0957_default_font_removal_keeps_real_custom_owner_and_entity_stable(){
    use bevy::ecs::system::RunSystemOnce;
    let(mut app,_)=layout_app();
    let source=r#"
screen Default0957(){return {"id":"default","kind":"text","text":"Default","layout_contract":"widget_v1"}}
screen Custom0957(){return {"id":"custom","kind":"column","font":"DejaVuSans.ttf","layout_contract":"widget_v1","children":[{"id":"label","kind":"text","parent_slot":{"kind":"vertical_box"},"text":"Custom"}]}}
label start
 ui.open("Default0957",[],false,0)
 ui.open("Custom0957",[],false,0)
 "Waiting"
"#;
    let mut engine=rvn_core::Engine::new(rvn_parser::parse(source).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();engine.step_until_interaction().unwrap();engine.renderer.scene_batch=None;
    app.world_mut().resource_mut::<Screens>().views=engine.interface_views().unwrap();app.insert_resource(VnEngine(engine));
    let custom=app.world().resource::<AssetServer>().load::<Font>("DejaVuSans.ttf");
    app.world_mut().resource_mut::<Assets<Font>>().insert(custom.id(),Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap());
    setup_camera(&mut app);app.world_mut().run_system_once(prepare_widget_layouts);app.world_mut().run_system_once(render);
    let sources=actual_sources(&app);let default=sources.iter().find(|source|source.screen=="Default0957").unwrap();let custom_source=sources.iter().find(|source|source.screen=="Custom0957").unwrap();
    let key=custom_source.to_wire().unwrap();let plan=app.world().resource::<WidgetLayouts>().plans[&key].clone();assert_eq!(plan.fonts.len(),1);assert!(plan.fonts.contains_key(&custom.id()));
    let entity=app.world_mut().query::<(Entity,&WidgetRootSource)>().iter(app.world()).find(|(_,source)|source.0==*custom_source).unwrap().0;
    let canonical=app.world().resource::<WidgetLayouts>().default_font.clone().unwrap();app.world_mut().resource_mut::<Assets<Font>>().remove(canonical.id()).unwrap();
    app.world_mut().run_system_once(prepare_widget_layouts);
    assert!(!app.world().resource::<WidgetLayouts>().plans.contains_key(&default.to_wire().unwrap()));assert_eq!(app.world().resource::<WidgetLayouts>().plans[&key],plan);
    app.world_mut().run_system_once(render);let actual=app.world_mut().query::<(Entity,&WidgetRootSource)>().iter(app.world()).find(|(_,source)|source.0==*custom_source).unwrap().0;assert_eq!(actual,entity);
    assert!(!app.world().resource::<WidgetLayouts>().plans[&key].fonts.contains_key(&canonical.id()));
}

#[path="programmable_widget_line_integration_tests_0960.rs"]
mod lineboxes0960;

#[path = "programmable_widget_retained_weight_tests1031.rs"]
mod retained_weight1031;
