//! EXIF: orientation handling plus a compact summary for the info panel.

use std::io::BufReader;
use std::path::Path;

#[cfg(feature = "heic")]
use crate::codec::probe::Format;
use crate::image_buf::ImageBuf;

/// A few human-relevant EXIF fields, preformatted for display.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExifSummary {
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    /// e.g. `f/2.8`
    pub f_number: Option<String>,
    /// e.g. `1/250 s`
    pub exposure: Option<String>,
    /// e.g. `50 mm`
    pub focal_length: Option<String>,
    /// e.g. `2026-05-18 19:42:05`
    pub date_time: Option<String>,
    /// GPS 坐标(纬度, 经度;十进制度)。
    pub gps: Option<(f64, f64)>,
}

impl ExifSummary {
    pub fn is_empty(&self) -> bool {
        self.make.is_none()
            && self.model.is_none()
            && self.lens.is_none()
            && self.iso.is_none()
            && self.f_number.is_none()
            && self.exposure.is_none()
            && self.focal_length.is_none()
            && self.date_time.is_none()
            && self.gps.is_none()
    }
}

/// Read an EXIF summary. Works for JPEG/TIFF/PNG/WebP (kamadak-exif) and for
/// HEIC/AVIF (via the libheif metadata item, when available).
pub fn read_summary(path: &Path) -> Option<ExifSummary> {
    let exif = read_exif(path)?;
    let mut s = ExifSummary::default();

    let text = |tag: exif::Tag| -> Option<String> {
        exif.get_field(tag, exif::In::PRIMARY)
            .map(|f| trim_quotes(&f.display_value().to_string()))
            .filter(|v| !v.is_empty())
    };
    let rational = |tag: exif::Tag| -> Option<f64> {
        exif.get_field(tag, exif::In::PRIMARY).and_then(|f| match &f.value {
            exif::Value::Rational(v) => v.first().map(|r| r.to_f64()),
            exif::Value::SRational(v) => v.first().map(|r| r.to_f64()),
            _ => None,
        })
    };

    s.make = text(exif::Tag::Make);
    s.model = text(exif::Tag::Model);
    s.iso = exif
        .get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0));
    s.f_number = rational(exif::Tag::FNumber).map(|v| format!("f/{}", fmt_num(v)));
    s.exposure = rational(exif::Tag::ExposureTime).map(format_exposure);
    s.focal_length = rational(exif::Tag::FocalLength).map(|v| format!("{} mm", fmt_num(v)));
    s.date_time = text(exif::Tag::DateTimeOriginal);

    // LensModel (0xA434)没有具名常量,按 tag 编号匹配。
    s.lens = exif
        .fields()
        .find(|f| f.tag.number() == 0xA434)
        .map(|f| trim_quotes(&f.display_value().to_string()))
        .filter(|v| !v.is_empty());

    s.gps = gps_coordinates(&exif);

    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 只读取 GPS 坐标(纬度, 经度)。用于地图模式的批量扫描——比 `read_summary`
/// 少做全部字段格式化,适合成千上万张照片的后台遍历。
pub fn read_gps(path: &Path) -> Option<(f64, f64)> {
    let exif = read_exif(path)?;
    gps_coordinates(&exif)
}

/// GPS 纬度/经度 → 十进制度(南纬/西经取负)。
fn gps_coordinates(exif: &exif::Exif) -> Option<(f64, f64)> {
    fn dms(exif: &exif::Exif, tag: exif::Tag) -> Option<f64> {
        let field = exif.get_field(tag, exif::In::PRIMARY)?;
        if let exif::Value::Rational(v) = &field.value {
            if v.len() >= 3 {
                return Some(v[0].to_f64() + v[1].to_f64() / 60.0 + v[2].to_f64() / 3600.0);
            }
        }
        None
    }
    fn reference(exif: &exif::Exif, tag: exif::Tag) -> String {
        exif.get_field(tag, exif::In::PRIMARY)
            .map(|f| trim_quotes(&f.display_value().to_string()))
            .unwrap_or_default()
    }

    let lat = dms(exif, exif::Tag::GPSLatitude)?;
    let lon = dms(exif, exif::Tag::GPSLongitude)?;
    let lat = if reference(exif, exif::Tag::GPSLatitudeRef).to_ascii_uppercase().starts_with('S') {
        -lat
    } else {
        lat
    };
    let lon = if reference(exif, exif::Tag::GPSLongitudeRef).to_ascii_uppercase().starts_with('W') {
        -lon
    } else {
        lon
    };
    Some((lat, lon))
}

/// Read the EXIF orientation (1..=8) of an image file, if present.
pub fn orientation_from_file(path: &Path) -> Option<u16> {
    let exif = read_exif(path)?;
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .map(|v| v as u16)
        .filter(|v| (1..=8).contains(v))
}

fn read_exif(path: &Path) -> Option<exif::Exif> {
    match crate::codec::detect_format(path) {
        #[cfg(feature = "heic")]
        Some(Format::Heif | Format::Avif) => {
            let bytes = crate::codec::heif::exif_bytes(path).ok()?;
            let tiff = locate_tiff(&bytes)?;
            exif::Reader::new().read_raw(tiff.to_vec()).ok()
        }
        _ => {
            let file = std::fs::File::open(path).ok()?;
            let mut reader = BufReader::new(file);
            exif::Reader::new().read_from_container(&mut reader).ok()
        }
    }
}

/// HEIF metadata items store EXIF either as `Exif\0\0` + TIFF, as a raw TIFF
/// blob, or as a 4-byte big-endian offset followed by the TIFF blob.
#[cfg(feature = "heic")]
fn locate_tiff(data: &[u8]) -> Option<&[u8]> {
    fn is_tiff(d: &[u8]) -> bool {
        d.len() >= 4 && (d.starts_with(b"II*\0") || d.starts_with(b"MM\0*"))
    }
    if data.starts_with(b"Exif\0\0") {
        return Some(&data[6..]);
    }
    if is_tiff(data) {
        return Some(data);
    }
    if data.len() >= 4 {
        let offset = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let start = 4usize.checked_add(offset)?;
        if start + 4 <= data.len() && is_tiff(&data[start..]) {
            return Some(&data[start..]);
        }
    }
    None
}

fn trim_quotes(s: &str) -> String {
    s.trim().trim_matches('"').trim().to_string()
}

/// "8" / "2.8" / "1.5" — drop the decimal when it is exact.
fn fmt_num(v: f64) -> String {
    if (v.fract()).abs() < 1e-9 {
        format!("{:.0}", v)
    } else {
        format!("{:.1}", v)
    }
}

fn format_exposure(seconds: f64) -> String {
    if seconds >= 1.0 {
        format!("{} s", fmt_num(seconds))
    } else if seconds > 0.0 {
        format!("1/{} s", (1.0 / seconds).round() as u64)
    } else {
        "0 s".to_string()
    }
}

/// Apply an EXIF orientation tag (1..=8) to the pixel buffer.
///
/// Mapping (dst coordinates of src(x, y), with source size w×h):
/// 1: (x, y)            2: (w-1-x, y)        3: (w-1-x, h-1-y)   4: (x, h-1-y)
/// 5: (y, x)            6: (h-1-y, x)        7: (h-1-y, w-1-x)   8: (y, w-1-x)
/// Orientations 5..=8 swap the width and height.
pub fn apply_orientation(src: ImageBuf, orientation: u16) -> ImageBuf {
    match orientation {
        2 => src.flip_horizontal(),
        3 => src.rotated(2),
        4 => src.rotated(2).flip_horizontal(),
        5 => src.rotated(1).flip_horizontal(),
        6 => src.rotated(1),
        7 => src.rotated(3).flip_horizontal(),
        8 => src.rotated(3),
        _ => src, // 1 or unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2×3 image whose red channel stores a unique id: id = y * 2 + x.
    fn labeled() -> ImageBuf {
        let mut rgba = Vec::new();
        for id in 0u8..6 {
            rgba.extend_from_slice(&[id, 0, 0, 255]);
        }
        ImageBuf::new(2, 3, rgba)
    }

    fn ids(buf: &ImageBuf) -> Vec<u8> {
        (0..buf.width * buf.height).map(|i| buf.rgba[i as usize * 4]).collect()
    }

    #[test]
    fn orientations_map_like_the_spec() {
        // Expected grids written as rows, derived from the mapping table above.
        let cases: &[(u16, (u32, u32), &[u8])] = &[
            (1, (2, 3), &[0, 1, 2, 3, 4, 5]),
            (2, (2, 3), &[1, 0, 3, 2, 5, 4]),
            (3, (2, 3), &[5, 4, 3, 2, 1, 0]),
            (4, (2, 3), &[4, 5, 2, 3, 0, 1]),
            (5, (3, 2), &[0, 2, 4, 1, 3, 5]),
            (6, (3, 2), &[4, 2, 0, 5, 3, 1]),
            (7, (3, 2), &[5, 3, 1, 4, 2, 0]),
            (8, (3, 2), &[1, 3, 5, 0, 2, 4]),
        ];
        for (orientation, dims, expected) in cases {
            let out = apply_orientation(labeled(), *orientation);
            assert_eq!((out.width, out.height), *dims, "dims for orientation {orientation}");
            assert_eq!(ids(&out), expected.to_vec(), "pixels for orientation {orientation}");
        }
    }

    #[test]
    fn unknown_orientation_is_identity() {
        assert_eq!(ids(&apply_orientation(labeled(), 99)), vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn formats_exposure_and_numbers() {
        assert_eq!(format_exposure(1.0 / 250.0), "1/250 s");
        assert_eq!(format_exposure(2.0), "2 s");
        assert_eq!(fmt_num(8.0), "8");
        assert_eq!(fmt_num(2.8_f64.round()), "3");
        assert_eq!(fmt_num(1.8), "1.8");
    }

    #[cfg(feature = "heic")]
    #[test]
    fn locates_tiff_variants() {
        let tiff = b"II*\0rest";
        let mut with_prefix = b"Exif\0\0".to_vec();
        with_prefix.extend_from_slice(tiff);
        assert_eq!(locate_tiff(&with_prefix), Some(&tiff[..]));

        let mut with_offset = vec![0, 0, 0, 0];
        with_offset.extend_from_slice(tiff);
        assert_eq!(locate_tiff(&with_offset), Some(&tiff[..]));

        assert_eq!(locate_tiff(tiff), Some(&tiff[..]));
        assert_eq!(locate_tiff(b"garbage"), None);
    }
}
