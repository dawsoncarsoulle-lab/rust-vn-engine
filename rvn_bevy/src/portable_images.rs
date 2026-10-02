//! Import ordinary artwork into a GPU format available on desktop and WebGL.
//! Bevy's PNG decoder otherwise preserves 16-bit channels, whose GPU formats
//! are optional and can panic only after a game starts loading its artwork.
use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    prelude::*,
    render::{
        render_resource::TextureFormat,
        texture::{ImageLoader, ImageLoaderSettings},
    },
};

pub(crate) struct PortableImagesPlugin;
impl Plugin for PortableImagesPlugin {
    fn build(&self, _app: &mut App) {}
    fn finish(&self, app: &mut App) {
        let decoder = ImageLoader::from_world(app.world_mut());
        app.register_asset_loader(PortableImageLoader(decoder));
    }
}
struct PortableImageLoader(ImageLoader);
impl AssetLoader for PortableImageLoader {
    type Asset = Image;
    type Settings = ImageLoaderSettings;
    type Error = std::io::Error;
    async fn load<'a>(
        &'a self,
        reader: &'a mut Reader<'_>,
        settings: &'a Self::Settings,
        context: &'a mut LoadContext<'_>,
    ) -> Result<Image, Self::Error> {
        let mut image = self
            .0
            .load(reader, settings, context)
            .await
            .map_err(std::io::Error::other)?;
        normalize(&mut image, settings.is_srgb)?;
        Ok(image)
    }
    fn extensions(&self) -> &[&str] {
        &["png", "jpg", "jpeg", "webp"]
    }
}
fn normalize(image: &mut Image, is_srgb: bool) -> Result<(), std::io::Error> {
    let channels = match image.texture_descriptor.format {
        TextureFormat::R16Uint | TextureFormat::R16Unorm => 1,
        TextureFormat::Rg16Uint | TextureFormat::Rg16Unorm => 2,
        TextureFormat::Rgba16Unorm => 4,
        _ => return Ok(()),
    };
    let pixels = image.texture_descriptor.size.width as usize
        * image.texture_descriptor.size.height as usize
        * image.texture_descriptor.size.depth_or_array_layers as usize;
    if image.texture_descriptor.mip_level_count != 1
        || pixels.checked_mul(channels * 2) != Some(image.data.len())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid 16-bit artwork dimensions",
        ));
    }
    let mut data = Vec::with_capacity(pixels * 4);
    for pixel in image.data.chunks_exact(channels * 2) {
        let channel = |index: usize| -> u8 {
            // The decoder stores native-endian u16 channels; rounded conversion
            // preserves black, white, transparency and grayscale on every host.
            let value = u16::from_ne_bytes([pixel[index * 2], pixel[index * 2 + 1]]) as u32;
            ((value + 128) / 257) as u8
        };
        data.extend_from_slice(&match channels {
            1 => [channel(0), channel(0), channel(0), 255],
            2 => [channel(0), channel(0), channel(0), channel(1)],
            _ => [channel(0), channel(1), channel(2), channel(3)],
        });
    }
    image.data = data;
    image.texture_descriptor.format = if is_srgb {
        TextureFormat::Rgba8UnormSrgb
    } else {
        TextureFormat::Rgba8Unorm
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension},
        texture::{CompressedImageFormats, ImageSampler, ImageType},
    };
    #[test]
    fn sixteen_bit_rgba_and_grayscale_artwork_are_portable_without_optional_gpu_features() {
        let mut image = Image::from_buffer(
            include_bytes!("../../examples/animations/assets/card-one.png"),
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::Default,
            RenderAssetUsages::default(),
        )
        .unwrap();
        assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba16Unorm);
        let size = image.texture_descriptor.size;
        normalize(&mut image, true).unwrap();
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!(image.texture_descriptor.size, size);
        assert_eq!(image.data.len(), 420 * 260 * 4);
        for format in [TextureFormat::R16Uint, TextureFormat::Rg16Uint] {
            let channels = if format == TextureFormat::R16Uint {
                1
            } else {
                2
            };
            let samples: Vec<u8> = [0u16, 65535, 32768, 0]
                .into_iter()
                .take(channels * 2)
                .flat_map(u16::to_ne_bytes)
                .collect();
            let mut image = Image::new(
                Extent3d {
                    width: 2,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                samples,
                format,
                RenderAssetUsages::default(),
            );
            normalize(&mut image, false).unwrap();
            assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm);
            assert_eq!(
                image.data,
                if channels == 1 {
                    vec![0, 0, 0, 255, 255, 255, 255, 255]
                } else {
                    vec![0, 0, 0, 255, 128, 128, 128, 0]
                }
            );
        }
    }
}
