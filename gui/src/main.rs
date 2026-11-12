use eframe::egui;

use crate::app::TractUi;

mod app;
mod discovery;
mod link;
mod scope;
mod session;

fn main() {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "TractCtrl",
        options,
        Box::new(|cc| Ok(Box::new(TractUi::new(cc)))),
    );
}
