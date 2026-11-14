//! Video device discovery independent from Linux-specific /dev/video nodes.
//!
//! The snapshot is deliberately read-only and is run from a worker thread by
//! the GUI. GStreamer may expose multiple identical display names: they must
//! remain separate until device-specific stable identifiers are available.

use gst::prelude::*;
use gstreamer as gst;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraDeviceInfo {
    pub display_name: String,
    pub device_class: String,
    pub capabilities: String,
}

/// Query a snapshot of devices that GStreamer classifies as video sources.
/// This function can probe hardware and must NOT run on the UI thread.
///
/// It does not select or open devices. GstDeviceMonitor::devices() is
/// documented to probe providers even when the monitor has not been started.
pub fn discover_video_devices() -> Result<Vec<CameraDeviceInfo>, String> {
    gst::init().map_err(|error| format!("GStreamer initialization failed: {error}"))?;

    let monitor = gst::DeviceMonitor::new();
    if monitor.add_filter(Some("Video/Source"), None).is_none() {
        return Ok(Vec::new());
    }

    let mut result: Vec<CameraDeviceInfo> = monitor
        .devices()
        .iter()
        .map(|device| CameraDeviceInfo {
            display_name: device.display_name().to_string(),
            device_class: device.device_class().to_string(),
            capabilities: device
                .caps()
                .map(|caps| caps.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        })
        .collect();

    // Stable presentation order for the same scan, not a persistent device ID.
    // Do not deduplicate: physically distinct webcams can share model names.
    result.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
            .then_with(|| a.capabilities.cmp(&b.capabilities))
    });
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_discovery_is_safe_on_headless_ci_without_cameras() {
        let found = discover_video_devices().unwrap();
        for item in found {
            assert!(!item.device_class.is_empty());
            assert!(!item.display_name.is_empty());
        }
    }
}
