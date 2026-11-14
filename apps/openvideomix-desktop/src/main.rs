mod app;
#[cfg(feature = "local-control-api")]
mod remote_control;
#[cfg(feature = "midi-input")]
mod midi_input;

use app::OpenVideoMixApp;

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 840.0])
            .with_min_inner_size([1024.0, 640.0])
            .with_title("OPENVIDEOMIX — Live Video Production Studio"),
        ..Default::default()
    };

    eframe::run_native(
        "OPENVIDEOMIX",
        native_options,
        Box::new(|cc| Ok(Box::new(OpenVideoMixApp::new(cc)))),
    )
}
