//! Canonical, licensed WidgetV1 faces shared by editor and runtime adapters.
//! This is not a global menu/dialogue font or a claim about raster parity.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetBuiltinFace {
    /// Source UTextBlock constructor: Roboto composite, typeface Bold.
    DefaultText,
    /// Explicit regular face for authored overrides and real-font fixtures.
    Regular,
}

#[derive(Clone, Copy, Debug)]
pub struct WidgetFontFace {
    pub family: &'static str,
    pub typeface: &'static str,
    pub version: &'static str,
    pub bytes: &'static [u8],
    pub sha256: [u8; 32],
    pub license: &'static str,
    pub notice: &'static str,
}

pub const DEFAULT_TEXT_POINTS: f32 = 24.0;

pub fn face(face: WidgetBuiltinFace) -> WidgetFontFace {
    match face {
        WidgetBuiltinFace::DefaultText => WidgetFontFace {
            family: "Roboto", typeface: "Bold", version: "1.00000; 2011",
            bytes: include_bytes!("../resources/fonts/Roboto-2011-Bold.ttf"),
            sha256: [0x24,0x57,0x15,0x03,0x14,0x07,0x60,0x24,0x09,0x24,0xdc,0xd1,0x23,0x8f,0x77,0xe7,0xcd,0x04,0x54,0xc0,0xd8,0xb0,0x79,0x39,0x90,0xce,0xfa,0x2f,0xad,0x71,0x47,0x1f],
            license: include_str!("../resources/fonts/Roboto-2011-Bold-LICENSE.txt"),
            notice: include_str!("../resources/fonts/Roboto-2011-Bold-NOTICE.txt"),
        },
        WidgetBuiltinFace::Regular => WidgetFontFace {
            family: "Roboto", typeface: "Regular", version: "1.00000; 2011",
            bytes: include_bytes!("../resources/fonts/Roboto-2011-Regular.ttf"),
            sha256: [0x16,0x46,0x6e,0xf6,0x50,0x64,0xe6,0xf3,0x88,0x5a,0x6d,0x28,0x06,0xb8,0x94,0x9a,0xc1,0xac,0x38,0xb5,0x24,0xdd,0x0c,0xf8,0xfc,0x96,0x56,0x5e,0xb4,0xcc,0x28,0xe8],
            license: include_str!("../resources/fonts/Roboto-2011-LICENSE.txt"),
            notice: include_str!("../resources/fonts/Roboto-2011-NOTICE.txt"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn widget0957_default_face_is_explicit_bold_and_size_default_is_24_points() {
        assert_eq!(face(WidgetBuiltinFace::DefaultText).typeface,"Bold");
        assert_eq!(DEFAULT_TEXT_POINTS,24.0);
        let component=crate::programmable::Component::parse(serde_json::json!({"id":"text","kind":"text","layout_contract":"widget_v1"})).unwrap();
        assert_eq!(component.font_size,DEFAULT_TEXT_POINTS);
        assert!(component.font.is_none());
    }
    #[test]
    fn widget0957_both_canonical_faces_carry_full_license_and_attribution() {
        for selected in [WidgetBuiltinFace::DefaultText,WidgetBuiltinFace::Regular] {
            let font=face(selected);
            assert!(font.license.contains("TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION"));
            assert!(font.license.contains("END OF TERMS AND CONDITIONS"));
            assert!(font.notice.contains("Google 2011"));assert!(font.notice.contains("Google 2012"));
            assert!(font.notice.contains("redistributed unchanged"));
            assert!(!font.bytes.is_empty());
        }
        assert_ne!(face(WidgetBuiltinFace::DefaultText).sha256,face(WidgetBuiltinFace::Regular).sha256);
    }
}
