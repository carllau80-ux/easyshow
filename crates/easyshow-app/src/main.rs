#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod decode;
mod fonts;
mod geo;
mod geocode;
mod geo_scan;
mod i18n;
mod icons;
mod map;
mod shell_icons;
mod sort;
mod theme;
mod thumbs;
mod tiles;
mod tree;
mod ui;
mod viewer;

use clap::Parser;

/// 变体标识:轻量版(编译时关闭 `heic` feature,无 HEIC/AVIF)在标题栏与设置菜单中
/// 标注为 "EasyShow Lite",与完整版一眼区分。
pub const APP_NAME: &str = if cfg!(feature = "heic") {
    "EasyShow"
} else {
    "EasyShow Lite"
};

/// EasyShow — a lightweight, fast image viewer.
#[derive(Parser, Debug)]
#[command(name = "easyshow", version, about = "EasyShow — a lightweight, fast image viewer")]
struct Args {
    /// Image file or directory to open
    path: Option<std::path::PathBuf>,
}

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| {
            fonts::install(&cc.egui_ctx);
            theme::install(&cc.egui_ctx);
            Ok(Box::new(app::EasyShowApp::new(cc, args.path)))
        }),
    )
}
