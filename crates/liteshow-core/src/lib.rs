//! Liteshow core: format probing, decoding, EXIF orientation and resizing.
//!
//! This crate has no UI dependencies so it can be unit-tested and reused by
//! benches, CLIs or a future thumbnail service.

pub mod codec;
pub mod error;
pub mod exif;
pub mod image_buf;
pub mod resize;
pub mod thumb;

use std::path::Path;

pub use codec::{decode_file, from_extension, probe, Format};
pub use error::{Error, Result};
pub use exif::ExifSummary;
pub use image_buf::ImageBuf;

/// Image dimensions without decoding pixels (JPEG/PNG/... read headers;
/// HEIC/AVIF query the container).
pub fn image_dimensions(path: &Path) -> Result<(u32, u32)> {
    match codec::detect_format(path) {
        Some(Format::Heif | Format::Avif) => codec::heif::dimensions(path),
        _ => image::image_dimensions(path)
            .map(|(w, h)| (w, h))
            .map_err(|e| Error::Decode(e.to_string())),
    }
}
