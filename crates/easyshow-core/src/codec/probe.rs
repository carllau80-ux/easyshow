//! Image format detection: magic bytes first, file extension as fallback.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Jpeg,
    Png,
    Gif,
    WebP,
    Bmp,
    Tiff,
    Ico,
    Qoi,
    Pnm,
    Tga,
    /// HEIC/HEIF 容器(需 `heic` feature)
    #[cfg(feature = "heic")]
    Heif,
    /// AVIF 容器(需 `heic` feature)
    #[cfg(feature = "heic")]
    Avif,
    // --- 扩展格式(需 `formats` feature) ---
    #[cfg(feature = "formats")]
    Hdr,
    #[cfg(feature = "formats")]
    Exr,
    #[cfg(feature = "formats")]
    Jxl,
    #[cfg(feature = "formats")]
    Psd,
    #[cfg(feature = "formats")]
    Svg,
}

impl Format {
    pub fn name(&self) -> &'static str {
        match self {
            Format::Jpeg => "JPEG",
            Format::Png => "PNG",
            Format::Gif => "GIF",
            Format::WebP => "WebP",
            Format::Bmp => "BMP",
            Format::Tiff => "TIFF",
            Format::Ico => "ICO",
            Format::Qoi => "QOI",
            Format::Pnm => "PNM",
            Format::Tga => "TGA",
            #[cfg(feature = "heic")]
            Format::Heif => "HEIC",
            #[cfg(feature = "heic")]
            Format::Avif => "AVIF",
            #[cfg(feature = "formats")]
            Format::Hdr => "HDR",
            #[cfg(feature = "formats")]
            Format::Exr => "EXR",
            #[cfg(feature = "formats")]
            Format::Jxl => "JPEG XL",
            #[cfg(feature = "formats")]
            Format::Psd => "PSD",
            #[cfg(feature = "formats")]
            Format::Svg => "SVG",
        }
    }
}

/// Detect the format from the first bytes of a file.
pub fn probe(head: &[u8]) -> Option<Format> {
    if head.len() < 4 {
        return None;
    }
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(Format::Jpeg);
    }
    if head.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some(Format::Png);
    }
    if head.starts_with(b"GIF8") {
        return Some(Format::Gif);
    }
    if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Some(Format::WebP);
    }
    if head.starts_with(b"BM") {
        return Some(Format::Bmp);
    }
    if head.starts_with(b"II*\0") || head.starts_with(b"MM\0*") {
        return Some(Format::Tiff);
    }
    if head.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some(Format::Ico);
    }
    if head.starts_with(b"qoif") {
        return Some(Format::Qoi);
    }
    if head[0] == b'P' && (b'1'..=b'6').contains(&head[1]) {
        return Some(Format::Pnm);
    }
    // ISO base media file format (HEIF family): ....ftyp<brand>
    #[cfg(feature = "heic")]
    if head.len() >= 12 && &head[4..8] == b"ftyp" {
        return match &head[8..12] {
            b"avif" | b"avis" => Some(Format::Avif),
            b"heic" | b"heix" | b"hevc" | b"hevx" | b"mif1" | b"msf1" => Some(Format::Heif),
            _ => None,
        };
    }
    #[cfg(feature = "formats")]
    {
        // Radiance HDR
        if head.starts_with(b"#?RADIANCE") || head.starts_with(b"#?RGBE") {
            return Some(Format::Hdr);
        }
        // OpenEXR
        if head.starts_with(&[0x76, 0x2F, 0x31, 0x01]) {
            return Some(Format::Exr);
        }
        // JPEG XL: 容器签名或裸码流
        if (head.len() >= 12 && head.starts_with(&[0x00, 0x00, 0x00, 0x0C]) && &head[4..8] == b"JXL ")
            || head.starts_with(&[0xFF, 0x0A])
        {
            return Some(Format::Jxl);
        }
        // Photoshop
        if head.starts_with(b"8BPS") {
            return Some(Format::Psd);
        }
        // SVG: XML 文档(在前 2 KiB 内出现 <svg)
        if looks_like_svg(head) {
            return Some(Format::Svg);
        }
    }
    None
}

/// 粗略判定:去掉 BOM/空白后以 `<` 开头,且前 2 KiB 内出现 `<svg`。
#[cfg(feature = "formats")]
fn looks_like_svg(head: &[u8]) -> bool {
    let window = &head[..head.len().min(2048)];
    let mut i = 0;
    if window.starts_with(&[0xEF, 0xBB, 0xBF]) {
        i = 3;
    }
    while i < window.len() && window[i].is_ascii_whitespace() {
        i += 1;
    }
    if window.get(i) != Some(&b'<') {
        return false;
    }
    let lower: Vec<u8> = window.iter().map(|b| b.to_ascii_lowercase()).collect();
    lower.windows(4).any(|w| w == b"<svg")
}

/// Detect the format from the file extension (used for formats without a
/// reliable magic number, e.g. TGA).
pub fn from_extension(path: &Path) -> Option<Format> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" | "jpe" => Format::Jpeg,
        "png" => Format::Png,
        "gif" => Format::Gif,
        "webp" => Format::WebP,
        "bmp" => Format::Bmp,
        "tif" | "tiff" => Format::Tiff,
        "ico" => Format::Ico,
        "qoi" => Format::Qoi,
        "ppm" | "pgm" | "pbm" | "pnm" | "pam" => Format::Pnm,
        "tga" => Format::Tga,
        #[cfg(feature = "heic")]
        "heic" | "heif" | "hif" => Format::Heif,
        #[cfg(feature = "heic")]
        "avif" => Format::Avif,
        #[cfg(feature = "formats")]
        "hdr" => Format::Hdr,
        #[cfg(feature = "formats")]
        "exr" => Format::Exr,
        #[cfg(feature = "formats")]
        "jxl" => Format::Jxl,
        #[cfg(feature = "formats")]
        "psd" => Format::Psd,
        #[cfg(feature = "formats")]
        "svg" => Format::Svg,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probes_common_magic_headers() {
        assert_eq!(probe(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(Format::Jpeg));
        assert_eq!(probe(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A]), Some(Format::Png));
        assert_eq!(probe(b"GIF89a"), Some(Format::Gif));
        assert_eq!(probe(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Some(Format::WebP));
        assert_eq!(probe(b"BM\x36\x00\x00\x00"), Some(Format::Bmp));
        assert_eq!(probe(b"II*\x00\x10\x00\x00\x00"), Some(Format::Tiff));
        assert_eq!(probe(b"MM\x00*\x00\x00\x00\x10"), Some(Format::Tiff));
        assert_eq!(probe(&[0, 0, 1, 0, 1, 0]), Some(Format::Ico));
        assert_eq!(probe(b"qoif\x00\x00\x00\x10"), Some(Format::Qoi));
        assert_eq!(probe(b"P6\n1024 768\n255\n"), Some(Format::Pnm));
        #[cfg(feature = "heic")]
        {
            assert_eq!(probe(&[0, 0, 0, 24, b'f', b't', b'y', b'p', b'h', b'e', b'i', b'c']), Some(Format::Heif));
            assert_eq!(probe(&[0, 0, 0, 24, b'f', b't', b'y', b'p', b'm', b'i', b'f', b'1']), Some(Format::Heif));
            assert_eq!(probe(&[0, 0, 0, 24, b'f', b't', b'y', b'p', b'a', b'v', b'i', b'f']), Some(Format::Avif));
        }
        assert_eq!(probe(b"not an image at all"), None);
    }

    #[test]
    fn extension_fallback_covers_tga_and_heif() {
        assert_eq!(from_extension(Path::new("x.tga")), Some(Format::Tga));
        #[cfg(feature = "heic")]
        {
            assert_eq!(from_extension(Path::new("IMG_1234.HEIC")), Some(Format::Heif));
            assert_eq!(from_extension(Path::new("photo.avif")), Some(Format::Avif));
        }
        assert_eq!(from_extension(Path::new("readme.txt")), None);
    }

    #[cfg(feature = "formats")]
    #[test]
    fn probes_extended_formats() {
        assert_eq!(probe(b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n"), Some(Format::Hdr));
        assert_eq!(probe(&[0x76, 0x2F, 0x31, 0x01, 0x02]), Some(Format::Exr));
        assert_eq!(
            probe(&[0x00, 0x00, 0x00, 0x0C, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A]),
            Some(Format::Jxl)
        );
        assert_eq!(probe(&[0xFF, 0x0A, 0x00, 0x00]), Some(Format::Jxl));
        assert_eq!(probe(b"8BPS\x00\x01"), Some(Format::Psd));
        assert_eq!(
            probe(b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"),
            Some(Format::Svg)
        );
        assert_eq!(probe(b"<html><body>no svg here</body></html>"), None);

        for (ext, fmt) in [
            ("a.hdr", Format::Hdr),
            ("a.exr", Format::Exr),
            ("a.jxl", Format::Jxl),
            ("a.psd", Format::Psd),
            ("a.svg", Format::Svg),
        ] {
            assert_eq!(from_extension(Path::new(ext)), Some(fmt), "{ext}");
        }
    }
}
