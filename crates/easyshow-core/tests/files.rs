//! Smoke tests against the real files in `testdata/` (skipped when absent).

use std::path::PathBuf;

fn testdata(name: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata")).join(name)
}

#[test]
fn decodes_and_downscales_24mp_jpeg() {
    let path = testdata("big-24mp.jpg");
    if !path.exists() {
        eprintln!("skip: {} missing", path.display());
        return;
    }
    let t0 = std::time::Instant::now();
    let image = easyshow_core::decode_file(&path).expect("decode 24MP jpeg");
    let decode_ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert_eq!((image.width, image.height), (6000, 4000));

    let t1 = std::time::Instant::now();
    let scale = easyshow_core::resize::fit_scale(image.width, image.height, 2400, 1400);
    let new_w = (image.width as f64 * scale).round() as u32;
    let new_h = (image.height as f64 * scale).round() as u32;
    let small = easyshow_core::resize::resize_rgba(&image, new_w, new_h).expect("downscale");
    let resize_ms = t1.elapsed().as_secs_f64() * 1000.0;
    assert_eq!((small.width, small.height), (new_w, new_h));
    eprintln!("timing: 24MP jpeg decode {decode_ms:.1} ms, downscale {resize_ms:.1} ms");
}

#[test]
fn decodes_portrait_png() {
    let path = testdata("portrait.png");
    if !path.exists() {
        eprintln!("skip: {} missing", path.display());
        return;
    }
    let image = easyshow_core::decode_file(&path).expect("decode png");
    assert_eq!((image.width, image.height), (900, 1400));
}

#[test]
fn reads_dimensions_without_decoding() {
    let path = testdata("big-24mp.jpg");
    if !path.exists() {
        return;
    }
    assert_eq!(easyshow_core::image_dimensions(&path).unwrap(), (6000, 4000));

    #[cfg(feature = "heic")]
    {
        let heic = testdata("sample.heic");
        if heic.exists() && easyshow_core::codec::heif::is_available() {
            assert_eq!(easyshow_core::image_dimensions(&heic).unwrap(), (1600, 1000));
        }
    }
}

#[test]
fn reads_exif_summary_from_jpeg() {
    let path = testdata("exif-sample.jpg");
    if !path.exists() {
        eprintln!("skip: {} missing", path.display());
        return;
    }
    let s = easyshow_core::exif::read_summary(&path).expect("exif summary");
    assert_eq!(s.make.as_deref(), Some("EasyShowCam"));
    assert_eq!(s.model.as_deref(), Some("Model X"));
    assert_eq!(s.lens.as_deref(), Some("FE 24-70mm F2.8"));
    assert_eq!(s.iso, Some(400));
    assert_eq!(s.f_number.as_deref(), Some("f/2.8"));
    assert_eq!(s.exposure.as_deref(), Some("1/250 s"));
    assert_eq!(s.focal_length.as_deref(), Some("35 mm"));
    assert!(s.date_time.as_deref().unwrap_or("").starts_with("2026-"));
    let (lat, lon) = s.gps.expect("gps coordinates");
    assert!((lat - 31.230416).abs() < 1e-4, "lat={lat}");
    assert!((lon - 121.473701).abs() < 1e-4, "lon={lon}");
}

#[cfg(feature = "heic")]
#[test]
fn reads_exif_summary_from_heic() {
    let path = testdata("sample-exif.heic");
    if !path.exists() {
        eprintln!("skip: {} missing", path.display());
        return;
    }
    if !easyshow_core::codec::heif::is_available() {
        eprintln!("skip: libheif unavailable");
        return;
    }
    let s = easyshow_core::exif::read_summary(&path).expect("heic exif summary");
    assert_eq!(s.make.as_deref(), Some("EasyShowCam"));
    assert_eq!(s.iso, Some(400));
    assert_eq!(s.f_number.as_deref(), Some("f/2.8"));
    let (lat, lon) = s.gps.expect("heic gps coordinates");
    assert!((lat - 31.230416).abs() < 1e-4, "lat={lat}");
    assert!((lon - 121.473701).abs() < 1e-4, "lon={lon}");
}

#[cfg(feature = "formats")]
#[test]
fn decodes_extended_format_samples() {
    for name in [
        "extended/sample.svg",
        "extended/sample.psd",
        "extended/sample.hdr",
        "extended/sample.exr",
        "extended/splines.jxl",
        "extended/pq-gradient.jxl",
    ] {
        let path = testdata(name);
        if !path.exists() {
            eprintln!("skip: {} missing", path.display());
            continue;
        }
        let img = easyshow_core::decode_file(&path)
            .unwrap_or_else(|e| panic!("{name} decode failed: {e}"));
        assert!(img.width > 0 && img.height > 0, "{name}: empty");
        assert_eq!(
            img.rgba.len(),
            img.width as usize * img.height as usize * 4,
            "{name}: buffer size mismatch"
        );
        eprintln!("{name}: {}x{}", img.width, img.height);
    }
}

#[cfg(feature = "formats")]
#[test]
fn reads_extended_dimensions() {
    for (name, expected) in [
        ("extended/sample.svg", (640, 400)),
        ("extended/sample.psd", (360, 240)),
        ("extended/sample.hdr", (360, 240)),
        ("extended/sample.exr", (360, 240)),
    ] {
        let path = testdata(name);
        if !path.exists() {
            continue;
        }
        assert_eq!(
            easyshow_core::image_dimensions(&path).unwrap(),
            expected,
            "{name}"
        );
    }
}
