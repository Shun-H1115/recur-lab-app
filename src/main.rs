#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod math;
mod simulation;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("RECUR — 漸化式ラボ")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 700.0]),
        multisampling: 4,
        ..Default::default()
    };
    eframe::run_native(
        "recur-lab",
        options,
        Box::new(|cc| Ok(Box::new(app::RecurApp::new(cc)))),
    )
}
