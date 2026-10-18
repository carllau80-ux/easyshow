//! Extended format decoders (JPEG XL / SVG / PSD), compiled with the `formats`
//! feature. HDR and EXR decode through the `image` crate and share the
//! tone-mapping path in [`crate::codec`].

use std::path::Path;
use std::sync::{Arc, OnceLock};

use crate::error::{Error, Result};
use crate::image_buf::ImageBuf;

/// Upper bound for decoded pixel counts of heavy formats (protects against
/// decompression bombs; ~100–200 MP).
const MAX_PIXELS: u64 = 200_000_000;

// ----------------------------------------------------------------------
// JPEG XL
// ----------------------------------------------------------------------

pub fn decode_jxl(path: &Path) -> Result<ImageBuf> {
    use image::ImageDecoder;

    let file = std::fs::File::open(path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    let decoder = jxl_oxide::integration::JxlDecoder::new(file)
        .map_err(|e| Error::Decode(format!("jxl: {e}")))?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return Err(Error::Decode("jxl: empty image".into()));
    }
    if w as u64 * h as u64 > MAX_PIXELS {
        return Err(Error::Decode(format!("jxl: image too large ({w}×{h})")));
    }
    let img = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| Error::Decode(format!("jxl: {e}")))?;
    crate::codec::dynamic_to_image_buf(img)
}

pub fn jxl_dimensions(path: &Path) -> Result<(u32, u32)> {
    use image::ImageDecoder;

    let file = std::fs::File::open(path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    let decoder = jxl_oxide::integration::JxlDecoder::new(file)
        .map_err(|e| Error::Decode(format!("jxl: {e}")))?;
    Ok(decoder.dimensions())
}

// ----------------------------------------------------------------------
// PSD (flat composite)
// ----------------------------------------------------------------------

pub fn decode_psd(path: &Path) -> Result<ImageBuf> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    let psd = psd::Psd::from_bytes(&bytes).map_err(|e| Error::Decode(format!("psd: {e}")))?;
    let (w, h) = (psd.width(), psd.height());
    if w == 0 || h == 0 {
        return Err(Error::Decode("psd: empty image".into()));
    }
    if w as u64 * h as u64 > MAX_PIXELS {
        return Err(Error::Decode(format!("psd: image too large ({w}×{h})")));
    }
    let rgba = psd.rgba();
    if rgba.len() != w as usize * h as usize * 4 {
        return Err(Error::Decode("psd: pixel buffer size mismatch".into()));
    }
    Ok(ImageBuf::new(w, h, rgba))
}

pub fn psd_dimensions(path: &Path) -> Result<(u32, u32)> {
    // 文件头:8BPS + version(2) + reserved(6) + channels(2) + height(4) + width(4)
    let bytes = std::fs::read(path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    if bytes.len() < 26 || &bytes[0..4] != b"8BPS" {
        return Err(Error::Decode("psd: invalid header".into()));
    }
    let h = u32::from_be_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]);
    let w = u32::from_be_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
    Ok((w, h))
}

// ----------------------------------------------------------------------
// SVG
// ----------------------------------------------------------------------

fn fontdb() -> Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        // 通用字体族(sans-serif 等)需要显式映射到本机存在的具体字体,
        // 否则 SVG 里的文字会被 usvg 判定为无匹配字体而不渲染。
        let available: Vec<String> = db
            .faces()
            .flat_map(|face| face.families.iter().map(|(name, _)| name.to_lowercase()))
            .collect();
        let pick = |candidates: &[&str]| -> Option<String> {
            candidates
                .iter()
                .find(|c| available.iter().any(|a| a == &c.to_lowercase()))
                .map(|s| s.to_string())
        };
        if let Some(family) = pick(&["Noto Sans", "DejaVu Sans", "Segoe UI", "Helvetica", "Arial", "Liberation Sans"]) {
            db.set_sans_serif_family(family);
        }
        if let Some(family) = pick(&["Noto Serif", "DejaVu Serif", "Times New Roman", "Liberation Serif"]) {
            db.set_serif_family(family);
        }
        if let Some(family) = pick(&["Noto Sans Mono", "DejaVu Sans Mono", "Consolas", "Courier New"]) {
            db.set_monospace_family(family);
        }
        Arc::new(db)
    })
    .clone()
}

fn svg_options() -> resvg::usvg::Options<'static> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fontdb();
    opt.font_family = "sans-serif".to_string();
    opt
}

/// Parse an SVG and render it to RGBA at (approximately) `target` dimensions.
/// Without a target the intrinsic size is used, capped at 2048px.
pub fn decode_svg(path: &Path, target: Option<(u32, u32)>) -> Result<ImageBuf> {
    let data = std::fs::read(path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    let tree = resvg::usvg::Tree::from_data(&data, &svg_options())
        .map_err(|e| Error::Decode(format!("svg: {e}")))?;
    let size = tree.size();
    let (sw, sh) = (size.width(), size.height());
    if !(sw > 0.0 && sh > 0.0) {
        return Err(Error::Decode("svg: invalid size".into()));
    }
    let (max_w, max_h) = match target {
        Some((w, h)) => (w.max(1) as f32, h.max(1) as f32),
        None => (2048.0, 2048.0),
    };
    let scale = (max_w / sw).min(max_h / sh).clamp(0.01, 8.0);
    let pw = ((sw * scale).ceil() as u32).max(1);
    let ph = ((sh * scale).ceil() as u32).max(1);
    if pw as u64 * ph as u64 > MAX_PIXELS / 4 {
        return Err(Error::Decode(format!("svg: render size too large ({pw}×{ph})")));
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pw, ph)
        .ok_or_else(|| Error::Decode("svg: cannot allocate pixmap".into()))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia outputs premultiplied RGBA; egui expects straight alpha.
    let src = pixmap.data();
    let mut rgba = Vec::with_capacity((pw as usize) * (ph as usize) * 4);
    for px in src.chunks_exact(4) {
        let a = px[3];
        if a == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else if a == 255 {
            rgba.extend_from_slice(px);
        } else {
            let af = a as u32;
            let un = |c: u8| (((c as u32 * 255) + af / 2) / af).min(255) as u8;
            rgba.extend_from_slice(&[un(px[0]), un(px[1]), un(px[2]), a]);
        }
    }
    Ok(ImageBuf::new(pw, ph, rgba))
}

pub fn svg_dimensions(path: &Path) -> Result<(u32, u32)> {
    let data = std::fs::read(path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    let tree = resvg::usvg::Tree::from_data(&data, &svg_options())
        .map_err(|e| Error::Decode(format!("svg: {e}")))?;
    let size = tree.size();
    Ok(((size.width().round() as u32).max(1), (size.height().round() as u32).max(1)))
}

/// Container-only dimensions for formats that cannot go through `image`.
pub fn dimensions(path: &Path, format: crate::codec::Format) -> Result<(u32, u32)> {
    use crate::codec::Format as F;
    match format {
        F::Jxl => jxl_dimensions(path),
        F::Psd => psd_dimensions(path),
        F::Svg => svg_dimensions(path),
        _ => image::image_dimensions(path)
            .map(|(w, h)| (w, h))
            .map_err(|e| Error::Decode(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_sample(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("easyshow-core-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn renders_svg_at_target_size() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50">
            <rect width="100" height="50" fill="#ff0000"/>
            <circle cx="50" cy="25" r="10" fill="#00ff00"/>
        </svg>"##;
        let path = write_sample("sample.svg", svg);
        let img = decode_svg(&path, Some((160, 160))).unwrap();
        assert_eq!((img.width, img.height), (160, 80));
        // 左上角是红色;中心是绿色圆
        let corner = img.pixel(2, 2);
        assert!(corner[0] > 200 && corner[1] < 60, "corner {corner:?}");
        let center = img.pixel(80, 40);
        assert!(center[1] > 150 && center[0] < 80, "center {center:?}");
    }

    #[test]
    fn svg_dimensions_match_intrinsic_size() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="320" height="200"></svg>"##;
        let path = write_sample("dims.svg", svg);
        assert_eq!(svg_dimensions(&path).unwrap(), (320, 200));
    }
}
