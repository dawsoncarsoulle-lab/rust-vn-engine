//! WidgetV1 default font binding. MenuFont and default Font handles are untouched.
use ab_glyph::Font as _;
use bevy::prelude::*;
use sha2::Digest;

#[derive(Debug,Clone,PartialEq,Eq)]
pub(super) enum Problem { MissingAsset, ChangedAsset, InvalidBundledFont(String) }
impl std::fmt::Display for Problem {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        match self {
            Self::MissingAsset=>write!(f,"Widget canonical Font asset is unavailable; no MenuFont substitution is permitted"),
            Self::ChangedAsset=>write!(f,"Widget canonical Font asset changed in-place: a fresh renderer font binding is required"),
            Self::InvalidBundledFont(error)=>write!(f,"Widget canonical font could not be loaded: {error}"),
        }
    }
}
fn canonical_font()->Result<Font,Problem>{
    let face=rvn_ui::widget_fonts::face(rvn_ui::widget_fonts::WidgetBuiltinFace::DefaultText);
    let digest:[u8;32]=sha2::Sha256::digest(face.bytes).into();
    if digest!=face.sha256{return Err(Problem::InvalidBundledFont("Bundled bytes disagree with canonical identity".into()));}
    Font::try_from_bytes(face.bytes.to_vec()).map_err(|error|Problem::InvalidBundledFont(format!("{error:?}")))
}
pub(super) fn ensure(fonts:&mut Assets<Font>,cached:&mut Option<Handle<Font>>)->Result<Handle<Font>,Problem>{
    if let Some(handle)=cached.as_ref(){
        let asset=fonts.get(handle).ok_or(Problem::MissingAsset)?;
        let digest:[u8;32]=sha2::Sha256::digest(asset.font.font_data()).into();
        if digest!=rvn_ui::widget_fonts::face(rvn_ui::widget_fonts::WidgetBuiltinFace::DefaultText).sha256{return Err(Problem::ChangedAsset);}
        return Ok(handle.clone());
    }
    let handle=fonts.add(canonical_font()?);*cached=Some(handle.clone());Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn widget0957_bevy_consumes_exact_canonical_bold_and_reuses_its_private_handle(){
        let mut fonts=Assets::default();let mut cache=None;
        let handle=ensure(&mut fonts,&mut cache).unwrap();let again=ensure(&mut fonts,&mut cache).unwrap();assert_eq!(handle,again);
        let face=rvn_ui::widget_fonts::face(rvn_ui::widget_fonts::WidgetBuiltinFace::DefaultText);
        assert_eq!(fonts.get(&handle).unwrap().font.font_data(),face.bytes);
        let regular=rvn_ui::widget_fonts::face(rvn_ui::widget_fonts::WidgetBuiltinFace::Regular);
        let actual_regular=Font::try_from_bytes(regular.bytes.to_vec()).unwrap();
        let digest:[u8;32]=sha2::Sha256::digest(actual_regular.font.font_data()).into();assert_eq!(digest,regular.sha256);
    }
    #[test]
    fn widget0957_missing_or_changed_canonical_handle_refuses_without_menu_substitution(){
        let mut fonts=Assets::default();let mut cache=None;let handle=ensure(&mut fonts,&mut cache).unwrap();
        let original=fonts.remove(handle.id()).unwrap();assert_eq!(ensure(&mut fonts,&mut cache),Err(Problem::MissingAsset));
        fonts.insert(handle.id(),Font::try_from_bytes(rvn_ui::widget_fonts::face(rvn_ui::widget_fonts::WidgetBuiltinFace::Regular).bytes.to_vec()).unwrap());
        assert_eq!(ensure(&mut fonts,&mut cache),Err(Problem::ChangedAsset));
        fonts.insert(handle.id(),original);assert_eq!(ensure(&mut fonts,&mut cache).unwrap(),handle);
    }
    #[test]
    fn widget0957_legacy_default_font_asset_remains_byte_exact(){
        let mut fonts:Assets<Font>=Assets::default();let legacy=Font::try_from_bytes(include_bytes!("../resources/DejaVuSans.ttf").to_vec()).unwrap();
        let old=fonts.add(legacy);let before=fonts.get(&old).unwrap().font.font_data().to_vec();
        let mut cache=None;let canonical=ensure(&mut fonts,&mut cache).unwrap();assert_ne!(canonical,old);
        assert_eq!(fonts.get(&old).unwrap().font.font_data(),before);assert!(fonts.get(Handle::<Font>::default().id()).is_none());
    }
}
