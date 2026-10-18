//! Decode dispatch: magic-byte probing, a fast zune-jpeg path for JPEG, the
//! `image` crate for everything else, and (with the `heic` feature)
//! runtime-loaded libheif for HEIC/AVIF.
//!
//! With the `formats` feature, JPEG XL / SVG / PSD get dedicated pure-Rust
//! decoders (see [`formats`]); HDR/EXR go through the `image` crate features
//! and are tone-mapped into sRGB.

pub mod probe;
#[cfg(feature = "heic")]
pub mod heif;
#[cfg(feature = "formats")]
pub mod formats;

use std::io::Read;
use std::path::Path;

use crate::error::{Error, Result};
use crate::image_buf::ImageBuf;
pub use probe::{from_extension, probe, Format};

/// Detect a file's format: magic bytes first, extension as fallback.
///
/// Reads a 2 KiB head so that XML-based formats (SVG) can be recognized.
pub fn detect_format(path: &Path) -> Option<Format> {
    let mut head = [0u8; 2048];
    let n = std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut head))
        .unwrap_or(0);
    probe(&head[..n]).or_else(|| from_extension(path))
}

/// Decode any supported image file into a tightly packed RGBA buffer.
pub fn decode_file(path: &Path) -> Result<ImageBuf> {
    decode_file_scaled(path, None)
}

/// Like [`decode_file`], but tells vector formats (SVG) the target pixel size
/// so they can render directly at display/thumbnail resolution.
pub fn decode_file_scaled(path: &Path, target: Option<(u32, u32)>) -> Result<ImageBuf> {
    #[cfg(not(feature = "formats"))]
    let _ = target;
    let format =
        detect_format(path).ok_or_else(|| Error::UnsupportedFormat(path.display().to_string()))?;

    match format {
        Format::Jpeg => decode_jpeg(path),
        #[cfg(feature = "heic")]
        Format::Heif | Format::Avif => heif::decode(path),
        #[cfg(feature = "formats")]
        Format::Svg => formats::decode_svg(path, target),
        #[cfg(feature = "formats")]
        Format::Jxl => formats::decode_jxl(path),
        #[cfg(feature = "formats")]
        Format::Psd => formats::decode_psd(path),
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
    dynamic_to_image_buf(img)
}

/// Convert a `DynamicImage` to 8-bit RGBA. Floating-point images (HDR, EXR,
/// float JPEG XL) are tone-mapped through a highlight-compressing curve and
/// sRGB transfer instead of a hard clamp.
pub(crate) fn dynamic_to_image_buf(img: image::DynamicImage) -> Result<ImageBuf> {
    use image::DynamicImage as D;
    let buf = match img {
        D::ImageRgb32F(buf) => tone_map_f32(buf.width(), buf.height(), buf.as_raw(), 3),
        D::ImageRgba32F(buf) => tone_map_f32(buf.width(), buf.height(), buf.as_raw(), 4),
        other => {
            let rgba = other.to_rgba8();
            let (w, h) = rgba.dimensions();
            ImageBuf::new(w, h, rgba.into_raw())
        }
    };
    Ok(buf)
}

/// Linear f32 samples → sRGB u8. If the image is SDR (≤1.0) this is just the
/// sRGB transfer; HDR content gets a Reinhard highlight roll-off first.
fn tone_map_f32(width: u32, height: u32, samples: &[f32], channels: usize) -> ImageBuf {
    let max = samples
        .chunks_exact(channels)
        .map(|px| px[0].max(px[1]).max(px[2]))
        .fold(0.0f32, f32::max);
    let compress = max > 1.02;
    let map = |v: f32| -> u8 {
        let v = v.max(0.0);
        let v = if compress { v / (1.0 + v) } else { v.min(1.0) };
        srgb_encode(v)
    };
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for px in samples.chunks_exact(channels) {
        let a = if channels == 4 { (px[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8 } else { 255 };
        rgba.extend_from_slice(&[map(px[0]), map(px[1]), map(px[2]), a]);
    }
    ImageBuf::new(width, height, rgba)
}

fn srgb_encode(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("easyshow-core-tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
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
    fn tone_maps_hdr_highlights() {
        // 线性 SDR:0.5 → sRGB ≈ 188
        let sdr = tone_map_f32(1, 1, &[0.5, 0.5, 0.5], 3);
        assert!((180..=195).contains(&sdr.rgba[0]), "sdr 0.5 -> {}", sdr.rgba[0]);
        // HDR 高光被压缩而不是截断;1.0 与 4.0 都应可区分且 <= 255
        let hdr = tone_map_f32(2, 1, &[1.0, 1.0, 1.0, 4.0, 4.0, 4.0], 3);
        assert!(hdr.rgba[0] < 255);
        assert!(hdr.rgba[4] > hdr.rgba[0]);
    }

    #[cfg(feature = "heic")]
    #[test]
    fn decodes_hevc_heic_sample() {
        let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join("sample.heic");
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

    #[cfg(feature = "heic")]
    #[test]
    fn decodes_av1_heif_sample() {
        let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join("sample-av1.heic");
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
