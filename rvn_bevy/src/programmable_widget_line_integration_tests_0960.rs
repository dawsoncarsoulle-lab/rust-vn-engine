use super::*;

#[test]
fn widget0960_real_core_source_crlf_stays_authored_while_paint_uses_same_line_break_projection(){
    use bevy::ecs::system::RunSystemOnce;
    let(mut app,_)=layout_app();
    let authored="\r\nA\r\n";
    let value=serde_json::to_string(authored).unwrap();
    let script=format!(r#"
screen Lines0960() {{return {{"id":"root","kind":"column","layout_contract":"widget_v1","children":[{{"id":"line","kind":"text","font_size":12,"text":{value},"parent_slot":{{"kind":"vertical_box"}}}}]}}}}
label start
 ui.open("Lines0960",[],false,0)
 "Waiting"
"#);
    let mut engine=rvn_core::Engine::new(rvn_parser::parse(&script).unwrap(),crate::bevy_renderer::BevyRenderer::new(),32).unwrap();engine.step_until_interaction().unwrap();engine.renderer.scene_batch=None;
    let before=engine.interface_views().unwrap();assert_eq!(before[0].root.find("line").unwrap().text,authored);
    app.world_mut().resource_mut::<Screens>().views=before.clone();app.insert_resource(VnEngine(engine));setup_camera(&mut app);
    app.world_mut().run_system_once(prepare_widget_layouts);assert!(widget_geometry_ready(app.world()));app.world_mut().run_system_once(render);
    assert_eq!(app.world().resource::<Screens>().views,before);assert_eq!(app.world().resource::<VnEngine>().0.interface_views().unwrap(),before);
    let text_entity=app.world_mut().query::<(Entity,&Text)>().iter(app.world()).find(|(_,text)|text.sections[0].value=="\nA\n").unwrap().0;
    let text=app.world().get::<Text>(text_entity).unwrap();let style=app.world().get::<Style>(text_entity).unwrap();
    assert_eq!(style.height,Val::Percent(100.0));assert_eq!(text.linebreak_behavior,bevy::text::BreakLineOn::NoWrap);
    let source=app.world().resource::<VnEngine>().0.capture_ui_input_source("Lines0960").unwrap();let layouts=app.world().resource::<WidgetLayouts>();let plan=&layouts.plans[&source.to_wire().unwrap()];let measured=plan.rects.iter().find(|rect|rect.id=="line").unwrap().rect;
    let font=&text.sections[0].style.font;let fonts=app.world().resource::<Assets<Font>>();let target=layouts.metrics.as_ref().unwrap().physical_scale;
    let actual=widget_text_lines_0960::layout(authored,font,fonts,text.sections[0].style.font_size,target).unwrap();assert_eq!(actual.lines.len(),3);near(measured[3],actual.size[1]);assert_eq!(actual.normalized,text.sections[0].value);
    let mut brush=bevy::text::GlyphBrush::default();brush.add_font(font.id(),fonts.get(font).unwrap().font.clone());let info=bevy::text::TextMeasureInfo::from_text(text,fonts,target).unwrap();let glyphs=brush.compute_glyphs(info.sections.as_ref(),Vec2::INFINITY,text.justify,text.linebreak_behavior).unwrap();assert_eq!(glyphs.len(),1);near(glyphs[0].glyph.position.y/target,actual.lines[1].baseline);
}

#[test]
fn widget0960_present_empty_text_keeps_button_content_height_and_legacy_text_is_unchanged(){
    let mut app=assets();let assets=app.world().resource::<AssetServer>().clone();let font=app.world_mut().resource_mut::<Assets<Font>>().add(Font::try_from_bytes(rvn_ui::widget_fonts::face(rvn_ui::widget_fonts::WidgetBuiltinFace::DefaultText).bytes.to_vec()).unwrap());
    let mut widget=view();widget.root.find_mut("label").unwrap().text.clear();
    let desired=widget_text_size(widget.root.find("label").unwrap(),&font,app.world().resource::<Assets<Font>>(),1.0,1.0).unwrap();assert_eq!(desired[0],0.0);assert!(desired[1]>0.0);
    let resolved=widget_layout_view(&widget,[0.0,0.0,280.0,160.0],1.0,1.0,&assets,app.world().resource::<Assets<Font>>(),Some(&font)).unwrap();let rect=resolved.root.find("label").unwrap().rect.unwrap();near(rect[3],desired[1]);assert!(resolved.root.find("button").unwrap().rect.unwrap()[3]>rect[3]);
    let mut legacy=widget.clone();legacy.root.layout_contract=None;legacy.root.find_mut("label").unwrap().text="A\r\nB".into();let before=legacy.clone();assert_eq!(widget_render_text_view(legacy,&assets,app.world().resource::<Assets<Font>>(),Some(&font)).unwrap(),before);
}
