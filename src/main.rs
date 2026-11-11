#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod context_menu;
mod folder;
mod history;
mod presets;
mod recycle;
mod render;
mod updater;
mod video;

use std::path::PathBuf;

fn load_app_icon() -> Option<eframe::egui::IconData> {
    let bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(eframe::egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result {
    updater::cleanup_old_executable();

    let args: Vec<String> = std::env::args().collect();
    let initial_file = if args.len() > 1 {
        let path = PathBuf::from(&args[1]);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    } else {
        None
    };

    let config = app::AppConfig::load();
    let initial_width = config.window_width.unwrap_or(1440.0).max(920.0);
    let initial_height = config.window_height.unwrap_or(880.0).max(580.0);

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("ztrim - Lossy & Lossless Fast Video Trimmer")
        .with_inner_size([initial_width, initial_height])
        .with_min_inner_size([920.0, 580.0])
        .with_drag_and_drop(true);

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let opts = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "ztrim",
        opts,
        Box::new(|cc| {
            app::setup_custom_fonts(&cc.egui_ctx);
            Ok(Box::new(app::ZTrimApp::new(initial_file, Some(cc.egui_ctx.clone()))))
        }),
    )
}
