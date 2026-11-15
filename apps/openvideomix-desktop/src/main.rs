mod app;
#[cfg(feature = "gstreamer-capture")]
use openvideomix_desktop::capture;
#[cfg(feature = "gstreamer-capture")]
mod capture_devices;
#[cfg(feature = "midi-input")]
mod midi_input;
#[cfg(feature = "local-control-api")]
mod remote_control;

use app::OpenVideoMixApp;

#[cfg(all(
    target_os = "windows",
    any(feature = "gstreamer-capture", feature = "gstreamer-recording")
))]
fn setup_bundled_gstreamer_windows() {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate_plugin_dir = exe_dir.join("lib").join("gstreamer-1.0");
            let local_plugin_dir = exe_dir.join("gstreamer-1.0");
            let plugin_dir = if candidate_plugin_dir.exists() {
                Some(candidate_plugin_dir)
            } else if local_plugin_dir.exists() {
                Some(local_plugin_dir)
            } else {
                None
            };

            if let Some(plugins) = plugin_dir {
                if std::env::var_os("GST_PLUGIN_SYSTEM_PATH_1_0").is_none() {
                    std::env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", &plugins);
                }
                if std::env::var_os("GST_PLUGIN_PATH_1_0").is_none() {
                    std::env::set_var("GST_PLUGIN_PATH_1_0", &plugins);
                }
            }

            let candidate_bin_dir = exe_dir.join("bin");
            let dll_dir = if candidate_bin_dir.exists() {
                candidate_bin_dir
            } else {
                exe_dir.to_path_buf()
            };

            if let Ok(current_path) = std::env::var("PATH") {
                let mut new_path = dll_dir.to_string_lossy().to_string();
                new_path.push(';');
                new_path.push_str(&current_path);
                std::env::set_var("PATH", new_path);
            }
        }
    }
}

#[cfg(not(all(
    target_os = "windows",
    any(feature = "gstreamer-capture", feature = "gstreamer-recording")
)))]
fn setup_bundled_gstreamer_windows() {}

fn main() -> eframe::Result {
    setup_bundled_gstreamer_windows();
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
