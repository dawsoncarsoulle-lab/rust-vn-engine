//! WidgetV1 authored font sizes are points; Bevy TextStyle consumes PxScale height.
//! This boundary is independent of target DPI/UiScale and accessibility scaling.
use ab_glyph::Font as _;
use bevy::text::Font;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WidgetFontUnitError {
    MissingFont,
    InvalidPoints,
    MissingUnitsPerEm,
    InvalidMetrics,
    InvalidPixelScale,
}

impl std::fmt::Display for WidgetFontUnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WidgetV1 font unit conversion refused: {self:?}")
    }
}

/// Resolve the unscaled authored baseline. Accessibility multiplies this by
/// text_scale once; Bevy then applies the actual camera DPI times UiScale.
/// A missing/unsupported font is an error, never a request to use another font.
/// This establishes units, not FreeType hinting or raster parity.
pub(super) fn point_size_to_logical_px(
    font: Option<&Font>,
    points: f32,
) -> Result<f32, WidgetFontUnitError> {
    if !points.is_finite() || points <= 0.0 {
        return Err(WidgetFontUnitError::InvalidPoints);
    }
    let font = font.ok_or(WidgetFontUnitError::MissingFont)?;
    let units = font.font.units_per_em().ok_or(WidgetFontUnitError::MissingUnitsPerEm)?;
    let height = font.font.height_unscaled();
    if !units.is_finite() || units <= 0.0 || !height.is_finite() || height <= 0.0 {
        return Err(WidgetFontUnitError::InvalidMetrics);
    }
    let scale = font.font.pt_to_px_scale(points).ok_or(WidgetFontUnitError::MissingUnitsPerEm)?;
    if !scale.x.is_finite() || !scale.y.is_finite() || scale.x <= 0.0 || scale.y <= 0.0 || scale.x != scale.y {
        return Err(WidgetFontUnitError::InvalidPixelScale);
    }
    Ok(scale.y)
}

#[cfg(test)]
#[path = "widget_font_units_tests_0956.rs"]
mod tests;
