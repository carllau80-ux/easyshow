//! Thumbnail generation and JPEG (de)serialization for the on-disk cache.

use std::path::Path;

use crate::error::{Error, Result};
use crate::image_buf::ImageBuf;

/// Decode → apply EXIF orientation → downscale so the long edge ≤ `max_edge`.
/// Vector formats (SVG) are rendered directly at thumbnail size; HEIC/AVIF
/// prefer their embedded thumbnail (much faster than decoding the full image).
pub fn make_thumbnail(path: &Path, max_edge: u32) -> Result<ImageBuf> {
    let mut image = decode_for_thumbnail(path, max_edge)?;
    if let Some(orientation) = crate::exif::orientation_from_file(path) {
        image = crate::exif::apply_orientation(image, orientation);
    }
    let scale = crate::resize::fit_scale(image.width, image.height, max_edge, max_edge);
    if scale < 0.98 {
        let new_w = ((image.width as f64 * scale).round() as u32).max(1);
        let new_h = ((image.height as f64 * scale).round() as u32).max(1);
        image = crate::resize::resize_rgba(&image, new_w, new_h)?;
    }
    Ok(image)
}

/// HEIC/AVIF 优先用容器内嵌缩略图(iPhone 照片通常 320–512px),
/// 解不出来时回退到正常解码路径。
fn decode_for_thumbnail(path: &Path, max_edge: u32) -> Result<ImageBuf> {
    #[cfg(feature = "heic")]
    {
        if matches!(
            crate::codec::detect_format(path),
            Some(crate::codec::Format::Heif | crate::codec::Format::Avif)
        ) {
            match crate::codec::heif::decode_thumbnail(path) {
                Ok(image) => return Ok(image),
                Err(e) => log::debug!("heic embedded thumbnail unavailable ({}): {e}", path.display()),
            }
        }
    }
    crate::codec::decode_file_scaled(path, Some((max_edge, max_edge)))
}

/// Encode an RGBA buffer as JPEG (used for the thumbnail disk cache).
pub fn encode_jpeg(buf: &ImageBuf, quality: u8) -> Result<Vec<u8>> {
    let rgb: Vec<u8> = buf.rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let image = image::RgbImage::from_raw(buf.width, buf.height, rgb)
        .ok_or_else(|| Error::Decode("thumbnail: buffer size mismatch".into()))?;
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(&image)
        .map_err(|e| Error::Decode(format!("thumbnail encode: {e}")))?;
    Ok(out)
}

/// Decode a JPEG byte buffer into RGBA.
pub fn decode_jpeg_bytes(bytes: &[u8]) -> Result<ImageBuf> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)
        .map_err(|e| Error::Decode(format!("thumbnail decode: {e}")))?;
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(ImageBuf::new(w, h, rgba.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_shrink_to_edge() {
        let dir = std::env::temp_dir().join("easyshow-core-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("thumb-src.png");
        let img = image::RgbaImage::from_fn(400, 300, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 60, 255])
        });
        img.save(&path).unwrap();

        let thumb = make_thumbnail(&path, 160).unwrap();
        assert_eq!((thumb.width, thumb.height), (160, 120));

        let jpeg = encode_jpeg(&thumb, 80).unwrap();
        let back = decode_jpeg_bytes(&jpeg).unwrap();
        assert_eq!((back.width, back.height), (160, 120));
        assert!(jpeg.len() < 40_000, "jpeg too large: {}", jpeg.len());
    }

    #[test]
    fn small_images_are_not_upscaled() {
        let dir = std::env::temp_dir().join("easyshow-core-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("thumb-small.png");
        image::RgbaImage::from_fn(64, 32, |_, _| image::Rgba([200, 100, 50, 255]))
            .save(&path)
            .unwrap();
        let thumb = make_thumbnail(&path, 160).unwrap();
        assert_eq!((thumb.width, thumb.height), (64, 32));
    }
}
