#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod chart;
mod drivetrain;
mod legal;
mod theme;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1220.0, 960.0])
            .with_min_inner_size([820.0, 600.0])
            .with_app_id("cogcalc")
            .with_icon(theme::window_icon()),
        centered: true,
        ..Default::default()
    };
    #[cfg(feature = "screenshot")]
    let options = preview_options(options);
    eframe::run_native(
        app::APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(app::GearApp::new(cc)))),
    )
}

// Isolate automated previews from the user's saved setup and window placement.
#[cfg(feature = "screenshot")]
fn preview_options(mut options: eframe::NativeOptions) -> eframe::NativeOptions {
    if std::env::var_os("EFRAME_SCREENSHOT_TO").is_some() {
        options.persistence_path =
            Some(std::env::temp_dir().join(format!("cogcalc-preview-{}.ron", std::process::id())));
        options.persist_window = false;
        if std::env::var_os("COGCALC_PREVIEW_COMPACT").is_some() {
            options.viewport = options.viewport.with_inner_size([820.0, 600.0]);
        }
    }
    options
}
