//! Decode dispatch: magic-byte probing, a fast zune-jpeg path for JPEG, the
//! `image` crate for everything else, and runtime-loaded libheif for HEIC.

pub mod probe;
pub mod heif;

use std::io::Read;
use std::path::Path;

use crate::error::{Error, Result};
use crate::image_buf::ImageBuf;
pub use probe::{from_extension, probe, Format};

/// Detect a file's format: magic bytes first, extension as fallback.
pub fn detect_format(path: &Path) -> Option<Format> {
    let mut head = [0u8; 32];
    let n = std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut head))
        .unwrap_or(0);
    probe(&head[..n]).or_else(|| from_extension(path))
}

/// Decode any supported image file into a tightly packed RGBA buffer.
pub fn decode_file(path: &Path) -> Result<ImageBuf> {
    let format = detect_format(path).ok_or_else(|| Error::UnsupportedFormat(path.display().to_string()))?;

    match format {
        Format::Jpeg => decode_jpeg(path),
        Format::Heif | Format::Avif => heif::decode(path),
        _ => decode_via_image(path),
    }
}

/// JPEG: fast path with zune-jpeg, falling back to the image crate.
fn decode_jpeg(path: &Path) -> Result<ImageBuf> {
    match decode_jpeg_fast(path) {
        Ok(buf) => Ok(buf),
        Err(err) => {
            log::warn!("zune-jpeg failed for {} ({err}); falling back to image crate", path.display());
            decode_via_image(path)
        }
    }
}

fn decode_jpeg_fast(path: &Path) -> Result<ImageBuf> {
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let data = std::fs::read(path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(&data), options);
    let pixels = decoder.decode().map_err(|e| Error::Decode(format!("jpeg: {e:?}")))?;
    let info = decoder.info().ok_or_else(|| Error::Decode("jpeg: missing image info".into()))?;
    let (width, height) = (info.width as u32, info.height as u32);
    let expected = width as usize * height as usize * 4;
    if pixels.len() != expected {
        return Err(Error::Decode(format!(
            "jpeg: buffer size {} does not match {width}x{height} RGBA ({expected})",
            pixels.len()
        )));
    }
    Ok(ImageBuf::new(width, height, pixels))
}

fn decode_via_image(path: &Path) -> Result<ImageBuf> {
    let img = image::open(path).map_err(|e| Error::Decode(e.to_string()))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(ImageBuf::new(width, height, rgba.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("liteshow-core-tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn testdata(name: &str) -> std::path::PathBuf {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(name)
    }

    #[test]
    fn round_trips_png_exactly() {
        let img = image::RgbaImage::from_fn(40, 24, |x, y| {
            image::Rgba([(x * 6) as u8, (y * 10) as u8, 128, 255])
        });
        let path = tmp_path("roundtrip.png");
        img.save(&path).unwrap();

        let out = decode_file(&path).unwrap();
        assert_eq!((out.width, out.height), (40, 24));
        assert_eq!(out.pixel(4, 2), [24, 20, 128, 255]);
    }

    #[test]
    fn round_trips_jpeg_lossy() {
        let img = image::RgbImage::from_fn(64, 48, |x, y| {
            image::Rgb([(x * 3) as u8, (y * 4) as u8, 90])
        });
        let mut encoded = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 92)
            .encode_image(&img)
            .unwrap();
        let path = tmp_path("roundtrip.jpg");
        std::fs::write(&path, &encoded).unwrap();

        let out = decode_file(&path).unwrap();
        assert_eq!((out.width, out.height), (64, 48));
        let px = out.pixel(10, 10);
        assert!((60..=130).contains(&px[2]), "blue channel out of range: {px:?}");
    }

    #[test]
    fn decodes_hevc_heic_sample() {
        let path = testdata("sample.heic");
        if !path.exists() {
            eprintln!("skip: {} missing", path.display());
            return;
        }
        match decode_file(&path) {
            Ok(img) => {
                assert_eq!((img.width, img.height), (1600, 1000));
                assert_eq!(img.rgba.len(), 1600 * 1000 * 4);
            }
            Err(Error::HeifUnavailable(msg)) => eprintln!("skip: libheif unavailable ({msg})"),
            Err(e) => panic!("HEIC (HEVC) decode failed: {e}"),
        }
    }

    #[test]
    fn decodes_av1_heif_sample() {
        let path = testdata("sample-av1.heic");
        if !path.exists() {
            eprintln!("skip: {} missing", path.display());
            return;
        }
        match decode_file(&path) {
            Ok(img) => assert_eq!((img.width, img.height), (1600, 1000)),
            Err(Error::HeifUnavailable(msg)) => eprintln!("skip: libheif unavailable ({msg})"),
            Err(e) => panic!("HEIF (AV1) decode failed: {e}"),
        }
    }
}
