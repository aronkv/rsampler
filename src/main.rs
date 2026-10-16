use eframe::egui;

struct App;

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(format!("rsampler {}", env!("CARGO_PKG_VERSION")));
            ui.label("Work in progress");
        });
    }
}

fn main() -> eframe::Result {
    eframe::run_native("rsampler", eframe::NativeOptions::default(), Box::new(|_| Ok(Box::new(App))))
}
