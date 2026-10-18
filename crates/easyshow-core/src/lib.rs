//! EasyShow core: format probing, decoding, EXIF orientation and resizing.
//!
//! This crate has no UI dependencies so it can be unit-tested and reused by
//! benches, CLIs or a future thumbnail service.

pub mod codec;
pub mod error;
pub mod exif;
pub mod image_buf;
pub mod resize;
pub mod thumb;
pub mod timeutil;

use std::path::Path;

pub use codec::{decode_file, decode_file_scaled, from_extension, probe, Format};
pub use error::{Error, Result};
pub use exif::{read_gps, read_summary, read_taken_time, ExifSummary};
pub use image_buf::ImageBuf;
pub use timeutil::DateTimeParts;

/// Image dimensions without decoding pixels (JPEG/PNG/... read headers;
/// HEIC/AVIF query the container with the `heic` feature; JPEG XL / PSD / SVG
/// read their headers/containers with the `formats` feature).
pub fn image_dimensions(path: &Path) -> Result<(u32, u32)> {
    if let Some(format) = codec::detect_format(path) {
        let _ = format;
        #[cfg(feature = "heic")]
        if matches!(format, Format::Heif | Format::Avif) {
            return codec::heif::dimensions(path);
        }
        #[cfg(feature = "formats")]
        if matches!(format, Format::Svg | Format::Jxl | Format::Psd) {
            return codec::formats::dimensions(path, format);
        }
    }
    image::image_dimensions(path)
        .map(|(w, h)| (w, h))
        .map_err(|e| Error::Decode(e.to_string()))
}
