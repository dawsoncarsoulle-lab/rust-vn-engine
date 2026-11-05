//! Actual Bevy font/glyph measurements plus explicit logical line boxes.
//! Empty lines contain no substituted glyph. This is not FreeType parity.
use ab_glyph::{Font as _, ScaleFont as _};
use bevy::prelude::*;
use std::ops::Range;

#[derive(Clone,Debug,PartialEq)]
pub(super) struct TextLine {
    pub source:Range<usize>,
    pub normalized:Range<usize>,
    pub separator:Range<usize>,
    pub top:f32,
    pub baseline:f32,
    pub width:f32,
    pub height:f32,
}
#[derive(Clone,Debug,PartialEq)]
pub(super) struct WidgetTextLayout {
    pub normalized:String,
    pub lines:Vec<TextLine>,
    pub size:[f32;2],
    pub physical_line_advance:f32,
    pub physical_ascent:f32,
}
#[derive(Clone,Debug,PartialEq,Eq)]
struct LineRange { source:Range<usize>, normalized:Range<usize>, separator:Range<usize> }

// Corresponds to the finite source-documented TCHAR line separator classes.
// CRLF is one delimiter; Rust UTF-8 offsets stay private, never masquerade as
// Unreal UTF-16 indices. VM/source strings are not mutated.
fn is_separator(character:char)->bool {
    matches!(character,'\n'|'\r'|'\u{000b}'|'\u{000c}'|'\u{0085}'|'\u{2028}'|'\u{2029}')
}
fn line_ranges(text:&str)->Result<(String,Vec<LineRange>),String>{
    if text.len()>65_536{return Err("Widget text exceeds its existing65536-byte contract".into());}
    let mut normalized=String::with_capacity(text.len());let mut ranges=Vec::new();
    let mut source_start=0;let mut chars=text.char_indices().peekable();
    while let Some((index,character))=chars.next(){
        if !is_separator(character){continue;}
        let mut separator_end=index+character.len_utf8();
        if character=='\r'&&chars.peek().is_some_and(|(_,next)|*next=='\n'){
            let(index,next)=chars.next().unwrap();separator_end=index+next.len_utf8();
        }
        let normalized_start=normalized.len();normalized.push_str(&text[source_start..index]);
        let normalized_end=normalized.len();
        ranges.push(LineRange{source:source_start..index,normalized:normalized_start..normalized_end,separator:index..separator_end});
        normalized.push('\n');source_start=separator_end;
    }
    let normalized_start=normalized.len();normalized.push_str(&text[source_start..]);
    ranges.push(LineRange{source:source_start..text.len(),normalized:normalized_start..normalized.len(),separator:text.len()..text.len()});
    Ok((normalized,ranges))
}
pub(super) fn normalize(text:&str)->Result<String,String>{line_ranges(text).map(|(text,_)|text)}

/// `logical_px_height` is the converted baseline times accessibility scale,
/// supplied by the existing shared point resolver; target DPI/UiScale is only
/// applied here in the same way as Bevy's actual text pipeline.
pub(super) fn layout(
    text:&str,font:&Handle<Font>,fonts:&Assets<Font>,logical_px_height:f32,physical_scale:f32,
)->Result<WidgetTextLayout,String>{
    if !logical_px_height.is_finite()||logical_px_height<=0.0||!physical_scale.is_finite()||physical_scale<=0.0{return Err("Invalid Widget text or target scale".into());}
    let asset=fonts.get(font).ok_or("Widget text waits for its actual Font asset")?;
    let physical_height=logical_px_height*physical_scale;
    if !physical_height.is_finite()||physical_height<=0.0{return Err("Invalid physical Widget font size".into());}
    let scaled=asset.font.as_scaled(physical_height);
    let ascent=scaled.ascent();let advance=scaled.height()+scaled.line_gap();
    if !ascent.is_finite()||!advance.is_finite()||advance<=0.0{return Err("Actual Widget font has invalid line metrics".into());}
    let(normalized,ranges)=line_ranges(text)?;
    for character in text.chars().filter(|character|!character.is_control()&&!is_separator(*character)){
        if asset.font.glyph_id(character).0==0{return Err(format!("Actual Widget font has no glyph U+{:04X}",character as u32));}
    }
    let mut brush=bevy::text::GlyphBrush::default();brush.add_font(font.id(),asset.font.clone());
    let mut lines=Vec::with_capacity(ranges.len());let mut width=0.0f32;
    for(index,range)in ranges.into_iter().enumerate(){
        let content=&normalized[range.normalized.clone()];
        let shaped=Text::from_section(content,TextStyle{font:font.clone(),font_size:logical_px_height,color:Color::WHITE}).with_no_wrap();
        let info=bevy::text::TextMeasureInfo::from_text(&shaped,fonts,physical_scale).map_err(|error|format!("Actual Widget line measurement failed: {error:?}"))?;
        let glyphs=brush.compute_glyphs(info.sections.as_ref(),Vec2::INFINITY,shaped.justify,shaped.linebreak_behavior).map_err(|error|format!("Actual Widget line shaping failed: {error:?}"))?;
        let line_width=if glyphs.is_empty(){0.0}else{info.compute_size(Vec2::INFINITY).x/physical_scale};
        let top=advance*index as f32/physical_scale;
        let height=advance/physical_scale;let baseline=(advance*index as f32+ascent)/physical_scale;
        if [line_width,top,height,baseline].iter().any(|value|!value.is_finite())||line_width<0.0||top+height>16384.0||line_width>16384.0{return Err("Widget line box exceeds its existing finite layout extent".into());}
        width=width.max(line_width);
        lines.push(TextLine{source:range.source,normalized:range.normalized,separator:range.separator,top,baseline,width:line_width,height});
    }
    let height=advance*lines.len()as f32/physical_scale;
    Ok(WidgetTextLayout{normalized,lines,size:[width,height],physical_line_advance:advance,physical_ascent:ascent})
}

#[cfg(test)]
#[path="programmable_widget_text_lines_tests_0960.rs"]
mod tests;
