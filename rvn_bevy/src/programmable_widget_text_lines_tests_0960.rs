use super::*;
use ab_glyph::{Font as _,ScaleFont as _};

fn font()->(Assets<Font>,Handle<Font>,f32){
    let mut fonts:Assets<Font>=Assets::default();let handle=fonts.add(Font::try_from_bytes(rvn_ui::widget_fonts::face(rvn_ui::widget_fonts::WidgetBuiltinFace::DefaultText).bytes.to_vec()).unwrap());
    let pixels=fonts.get(&handle).unwrap().font.pt_to_px_scale(24.0).unwrap().y;(fonts,handle,pixels)
}
#[test]
fn widget0960_line_ranges_preserve_empty_final_crlf_and_source_byte_boundaries(){
    for(authored,expected,contents)in[
        ("","",vec![""]),("\n","\n",vec!["",""]),("A\n\n","A\n\n",vec!["A","",""]),
        ("\r\nA\r\n\r\nB\r\n","\nA\n\nB\n",vec!["","A","","B",""]),
        ("é\u{000b}\u{000c}\u{0085}\u{2028}\u{2029}Ω","é\n\n\n\n\nΩ",vec!["é","","","","","Ω"]),
    ]{
        let before=authored.to_owned();let(normalized,ranges)=line_ranges(authored).unwrap();
        assert_eq!(normalized,expected);assert_eq!(ranges.len(),contents.len());
        for(range,expected)in ranges.iter().zip(contents){assert_eq!(&authored[range.source.clone()],expected);assert_eq!(&normalized[range.normalized.clone()],expected);assert!(authored.is_char_boundary(range.separator.start)&&authored.is_char_boundary(range.separator.end));}
        assert_eq!(authored,before);assert_eq!(normalized.chars().filter(|c|*c=='\n').count()+1,ranges.len());
    }
}
#[test]
fn widget0960_empty_space_and_control_lines_keep_real_font_line_metrics_without_fake_glyph(){
    let(fonts,font,pixels)=font();
    for text in["","\n","\n\n","A\n\n"," ","\t"]{
        let actual=layout(text,&font,&fonts,pixels,1.25).unwrap();
        let scaled=fonts.get(&font).unwrap().font.as_scaled(pixels*1.25);
        let height=(scaled.height()+scaled.line_gap())/1.25;
        assert!(height>0.0);assert!((actual.size[1]-height*actual.lines.len()as f32).abs()<0.001);
        for(index,line)in actual.lines.iter().enumerate(){assert!((line.top-index as f32*height).abs()<0.001);assert_eq!(line.height,height);}
        assert_eq!(actual.normalized,normalize(text).unwrap());
        if text==""||text=="\t"{assert_eq!(actual.size[0],0.0);}
        if text==" "{assert!(actual.size[0]>0.0,"actual space advance survives absence of ink");}
    }
}
#[test]
fn widget0960_actual_paint_glyph_baselines_follow_leading_intermediate_empty_lines_for_all_alignments(){
    let(fonts,font,pixels)=font();
    for physical in[1.0,1.25,2.0]{for text_scale in[1.0,1.5]{
        let authored="\r\nA\r\n\r\nB\r\n";let actual=layout(authored,&font,&fonts,pixels*text_scale,physical).unwrap();assert_eq!(actual.lines.len(),5);
        for justify in[JustifyText::Left,JustifyText::Center,JustifyText::Right]{
            let text=Text::from_section(&actual.normalized,TextStyle{font:font.clone(),font_size:pixels*text_scale,color:Color::WHITE}).with_no_wrap().with_justify(justify);
            let info=bevy::text::TextMeasureInfo::from_text(&text,&fonts,physical).unwrap();
            let mut brush=bevy::text::GlyphBrush::default();brush.add_font(font.id(),fonts.get(&font).unwrap().font.clone());
            let glyphs=brush.compute_glyphs(info.sections.as_ref(),Vec2::INFINITY,justify,text.linebreak_behavior).unwrap();assert_eq!(glyphs.len(),2);
            for(glyph,line_index)in glyphs.iter().zip([1usize,3]){let line=&actual.lines[line_index];assert_eq!(glyph.byte_index,line.normalized.start);assert!((glyph.glyph.position.y/physical-line.baseline).abs()<0.001);let offset=match justify{JustifyText::Left=>0.0,JustifyText::Center=>-line.width*0.5,JustifyText::Right=>-line.width};assert!((glyph.glyph.position.x/physical-offset).abs()<0.001);}
            assert!(actual.size[1]>(glyphs[1].glyph.position.y+(-fonts.get(&font).unwrap().font.as_scaled(pixels*text_scale*physical).descent()))/physical,"trailing empty line exceeds the last painted glyph line");
            // Exercise the actual atlas/raster submission stage without a GPU.
            let mut pipeline=bevy::text::TextPipeline::default();let mut atlas=bevy::text::FontAtlasSets::default();let mut images=Assets::<Image>::default();let mut textures=Assets::<bevy::sprite::TextureAtlasLayout>::default();
            let painted=pipeline.queue_text(&fonts,&text.sections,physical,justify,text.linebreak_behavior,Vec2::INFINITY,&mut atlas,&mut textures,&mut images,&bevy::text::TextSettings::default(),bevy::text::YAxisOrientation::TopToBottom).unwrap();
            assert_eq!(painted.glyphs.len(),2);assert!(painted.glyphs[1].position.y>painted.glyphs[0].position.y);assert!(!images.is_empty());
        }
    }}
}
#[test]
fn widget0960_missing_font_invalid_scale_and_existing_text_extent_limits_refuse(){
    let(mut fonts,font,pixels)=font();
    for logical in[0.0,f32::NAN,f32::INFINITY]{assert!(layout("",&font,&fonts,logical,1.0).is_err());}
    assert!(layout("\n".repeat(65_537).as_str(),&font,&fonts,pixels,1.0).is_err());
    assert!(layout("\n".repeat(2_000).as_str(),&font,&fonts,pixels,1.0).is_err());
    fonts.remove(font.id());assert!(layout("",&font,&fonts,pixels,1.0).is_err());
}
