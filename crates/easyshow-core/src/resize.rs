//! Image resampling built on `fast_image_resize` (SIMD-accelerated).

use fast_image_resize as fir;

use crate::error::{Error, Result};
use crate::image_buf::ImageBuf;

/// Largest scale factor ≤ 1.0 that makes `w×h` fit inside `max_w×max_h`.
/// Returns 1.0 when the image already fits (we never upscale here).
pub fn fit_scale(w: u32, h: u32, max_w: u32, max_h: u32) -> f64 {
    if w == 0 || h == 0 || max_w == 0 || max_h == 0 {
        return 1.0;
    }
    (max_w as f64 / w as f64)
        .min(max_h as f64 / h as f64)
        .min(1.0)
}

/// Resize an RGBA buffer to `new_w × new_h`.
pub fn resize_rgba(src: &ImageBuf, new_w: u32, new_h: u32) -> Result<ImageBuf> {
    let src_view = fir::images::ImageRef::new(src.width, src.height, &src.rgba, fir::PixelType::U8x4)
        .map_err(|e| Error::Decode(format!("resize source: {e}")))?;
    let mut dst = fir::images::Image::new(new_w, new_h, fir::PixelType::U8x4);
    let mut resizer = fir::Resizer::new();
    let options = fir::ResizeOptions::new().resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Bilinear));
    resizer
        .resize(&src_view, &mut dst, &options)
        .map_err(|e| Error::Decode(format!("resize: {e}")))?;
    Ok(ImageBuf::new(new_w, new_h, dst.into_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_scale_never_upscales() {
        assert_eq!(fit_scale(100, 100, 200, 200), 1.0);
        assert!((fit_scale(2000, 1000, 1000, 1000) - 0.5).abs() < 1e-9);
        assert!((fit_scale(1000, 2000, 1000, 1000) - 0.5).abs() < 1e-9);
        assert!((fit_scale(4000, 3000, 1200, 800) - 0.2666).abs() < 1e-3);
    }

    #[test]
    fn resize_produces_requested_dimensions() {
        let mut rgba = Vec::with_capacity(64 * 48 * 4);
        for y in 0..48u32 {
            for x in 0..64u32 {
                rgba.extend_from_slice(&[(x * 4) as u8, (y * 5) as u8, 128, 255]);
            }
        }
        let src = ImageBuf::new(64, 48, rgba);
        let out = resize_rgba(&src, 16, 12).unwrap();
        assert_eq!((out.width, out.height), (16, 12));
        assert_eq!(out.rgba.len(), 16 * 12 * 4);
    }
}
