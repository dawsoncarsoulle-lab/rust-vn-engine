//! Font-aware measurement feeds the same layout algorithm used by the editor.
use super::*;
pub(super) fn tree(
    doc: &Document,
    page: usize,
    size: [f32; 2],
    assets: &AssetServer,
    ctx: &Context,
    list_page: usize,
    data: &[(&str, &str)],
) -> Vec<Element> {
    doc.layout_tree_measured(page, size, &mut |e, width, scale| {
        measure(e, width, scale, assets, ctx, list_page, data)
    })
}
pub(super) fn item(
    doc: &Document,
    template: &str,
    size: [f32; 2],
    data: &std::collections::BTreeMap<String, String>,
    assets: &AssetServer,
    ctx: &Context,
) -> Vec<Element> {
    doc.layout_item_measured(template, size, data, &mut |e, width, scale| {
        measure(e, width, scale, assets, ctx, 0, &[])
    })
}
fn measure(
    e: &Element,
    width: f32,
    scale: f32,
    assets: &AssetServer,
    ctx: &Context,
    list_page: usize,
    data: &[(&str, &str)],
) -> Option<f32> {
    // Rich dialogue has its own measured, scrolling typewriter presentation.
    if e.binding.as_deref() == Some("dialogue.text") {
        return None;
    }
    let font = e
        .font
        .as_ref()
        .or(ctx.theme.text.name.font_path.as_ref())
        .map(|f| assets.load(f.clone()))
        .unwrap_or_else(|| ctx.font.0.clone());
    let value = match e.binding.as_deref() {
        Some("save.page") => Some((list_page + 1).to_string()),
        Some("music_volume") => Some(format!("{:.0} %", ctx.values.music_volume * 100.0)),
        Some("sfx_volume") => Some(format!("{:.0} %", ctx.values.sfx_volume * 100.0)),
        Some("text_speed") => Some(format!("{:.1}", ctx.values.text_speed)),
        Some("auto_speed") => Some(format!("{:.1}", ctx.values.auto_speed)),
        Some("typewriter") => Some(
            if ctx.values.typewriter {
                "Activé"
            } else {
                "Désactivé"
            }
            .into(),
        ),
        Some("fullscreen") => Some(if ctx.values.fullscreen { "Oui" } else { "Non" }.into()),
        Some("language") => Some(ctx.values.language.clone()),
        _ => None,
    };
    let label = data
        .iter()
        .find(|(key, _)| e.binding.as_deref() == Some(*key))
        .map(|(_, value)| translated(ctx, value))
        .unwrap_or_else(|| {
            value
                .filter(|_| e.kind != Kind::CheckBox)
                .map(|v| format!("{} : {}", translated(ctx, &e.text), translated(ctx, &v)))
                .unwrap_or_else(|| translated(ctx, &e.text))
        });
    let font_size = e.font_size * scale;
    let padding = if e.kind == Kind::Text {
        0.0
    } else {
        (font_size * 0.15).min(6.0)
    };
    let border = e.appearance.border_width * scale;
    let mut text = Text::from_section(
        label,
        TextStyle {
            font,
            font_size,
            color: Color::WHITE,
        },
    );
    if !e.layout_options.text_wrap {
        text = text.with_no_wrap();
    }
    let measure = bevy::text::TextMeasureInfo::from_text(&text, &ctx.fonts, 1.0).ok()?;
    Some(
        measure
            .compute_size(Vec2::new(
                (width - 2.0 * (padding + border)).max(1.0),
                f32::INFINITY,
            ))
            .y
            .ceil()
            + 2.0 * (padding + border)
            + 2.0 * scale,
    )
}
