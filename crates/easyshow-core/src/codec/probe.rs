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
    None
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
}
