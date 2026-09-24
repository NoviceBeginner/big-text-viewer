#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod file_handler;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([700.0, 450.0])
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Big Text Viewer V4",
        options,
        Box::new(|cc| Ok(Box::new(app::BigTextViewer::new(cc)))),
    )
}
