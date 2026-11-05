use super::*;
use ab_glyph::Font as _;

fn actual_font() -> Font {
    Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap()
}

#[test]
fn widget0956_points_use_actual_font_em_and_height() {
    let font = actual_font();
    let units = font.font.units_per_em().unwrap();
    let height = font.font.height_unscaled();
    for points in [0.5, 12.0, 18.0, 24.0] {
        let actual = point_size_to_logical_px(Some(&font), points).unwrap();
        let expected = points * (96.0 / 72.0) * height / units;
        assert_eq!(actual.to_bits(), expected.to_bits());
        // The font-specific height/em ratio is essential, not an empirical fit.
        assert_ne!(actual.to_bits(), (points * (96.0 / 72.0)).to_bits());
    }
}

#[test]
fn widget0956_missing_font_is_refused_without_substitution() {
    assert_eq!(point_size_to_logical_px(None, 18.0), Err(WidgetFontUnitError::MissingFont));
}

#[test]
fn widget0956_invalid_points_are_refused_before_metrics() {
    for points in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(point_size_to_logical_px(None, points), Err(WidgetFontUnitError::InvalidPoints));
    }
}

#[test]
fn widget0956_overflow_from_actual_font_is_refused() {
    let font = actual_font();
    assert_eq!(point_size_to_logical_px(Some(&font), f32::MAX), Err(WidgetFontUnitError::InvalidPixelScale));
}
