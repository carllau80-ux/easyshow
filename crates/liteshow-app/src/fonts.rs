//! Font loading: bundled Phosphor icon subset + a system CJK font.
//! A bundled CJK subset replaces the system font lookup in M3.

use std::sync::Arc;

use eframe::egui;

/// 内置图标字体(Phosphor 子集,仅含用到的 25 个图标,~7 KB)。
const PHOSPHOR: &[u8] = include_bytes!("../../../assets/fonts/phosphor-subset.ttf");

const CJK_CANDIDATES: &[&str] = &[
    // Linux
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    // macOS
    "/System/Library/Fonts/PingFang.ttc",
    // Windows
    "C:\\Windows\\Fonts\\msyh.ttc",
    "C:\\Windows\\Fonts\\msyh.ttf",
];

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // 图标字体放在最前:私有区码点优先命中图标,CJK 不会抢占。
    fonts
        .font_data
        .insert("phosphor".to_owned(), Arc::new(egui::FontData::from_static(PHOSPHOR)));
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().insert(0, "phosphor".to_owned());
    }

    for path in CJK_CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else { continue };
        fonts
            .font_data
            .insert("cjk".to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push("cjk".to_owned());
        }
        log::info!("CJK font loaded: {path}");
        break;
    }

    ctx.set_fonts(fonts);
}
