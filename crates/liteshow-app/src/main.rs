#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod decode;
mod fonts;
mod i18n;
mod icons;
mod shell_icons;
mod sort;
mod theme;
mod thumbs;
mod tree;
mod ui;
mod viewer;

use clap::Parser;

/// Liteshow — a lightweight, fast image viewer.
#[derive(Parser, Debug)]
#[command(name = "liteshow", version, about = "Liteshow — a lightweight, fast image viewer")]
struct Args {
    /// Image file or directory to open
    path: Option<std::path::PathBuf>,
}

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Liteshow")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Liteshow",
        options,
        Box::new(|cc| {
            fonts::install(&cc.egui_ctx);
            theme::install(&cc.egui_ctx);
            Ok(Box::new(app::LiteshowApp::new(cc, args.path)))
        }),
    )
}
