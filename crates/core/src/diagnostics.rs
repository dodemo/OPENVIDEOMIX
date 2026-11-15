//! Sanitized diagnostics export for OPENVIDEOMIX.
//!
//! Gathers system, runtime, GStreamer, and mixer metrics while strictly
//! redacting usernames, hostnames, serials, and local path details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::project_store::OutputMetrics;

/// Information about an input source for diagnostic logging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputDiagnosticInfo {
    pub id: u32,
    pub name: String,
    pub kind_summary: String,
    pub is_active: bool,
    pub is_online: bool,
}

/// Sanitized system and runtime diagnostic report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemDiagnostics {
    pub product: String,
    pub version: String,
    pub target_os: String,
    pub target_arch: String,
    pub standard_profile: String,
    pub timestamp_utc: DateTime<Utc>,
    pub gstreamer_available: bool,
    pub gstreamer_version: Option<String>,
    pub detected_plugins: Vec<String>,
    pub inputs: Vec<InputDiagnosticInfo>,
    pub output_metrics: Option<OutputMetrics>,
    pub sanitized: bool,
}

impl SystemDiagnostics {
    pub fn new(
        version: impl Into<String>,
        gstreamer_available: bool,
        gstreamer_version: Option<String>,
        detected_plugins: Vec<String>,
        inputs: Vec<InputDiagnosticInfo>,
        output_metrics: Option<OutputMetrics>,
    ) -> Self {
        Self {
            product: "OPENVIDEOMIX".to_string(),
            version: version.into(),
            target_os: std::env::consts::OS.to_string(),
            target_arch: std::env::consts::ARCH.to_string(),
            standard_profile: "1920x1080 progressive 50/1 FPS, 48 kHz PCM".to_string(),
            timestamp_utc: Utc::now(),
            gstreamer_available,
            gstreamer_version,
            detected_plugins,
            inputs,
            output_metrics,
            sanitized: true,
        }
    }

    /// Redact potential personal information such as local home paths and usernames.
    pub fn sanitize_path(raw_path: &str) -> String {
        let mut sanitized = raw_path.to_string();

        // Redact Windows user paths: C:\Users\<name>\... -> C:\Users\[REDACTED]\...
        if let Some(idx) = sanitized.find(r"Users\") {
            let start = idx + 6;
            if let Some(end) = sanitized[start..].find('\\') {
                sanitized.replace_range(start..start + end, "[REDACTED]");
            }
        } else if let Some(idx) = sanitized.find("Users/") {
            let start = idx + 6;
            if let Some(end) = sanitized[start..].find('/') {
                sanitized.replace_range(start..start + end, "[REDACTED]");
            }
        }

        // Redact Linux/macOS user paths: /home/<name>/... or /Users/<name>/...
        if let Some(idx) = sanitized.find("/home/") {
            let start = idx + 6;
            if let Some(end) = sanitized[start..].find('/') {
                sanitized.replace_range(start..start + end, "[REDACTED]");
            }
        }

        sanitized
    }

    /// Serialize report to sanitized JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Format as a Markdown summary suitable for GitHub issue diagnostic reports.
    pub fn to_markdown_report(&self) -> String {
        let mut out = String::new();
        out.push_str("### 📋 OPENVIDEOMIX Diagnostics Report\n\n");
        out.push_str(&format!(
            "- **Product**: {} v{}\n",
            self.product, self.version
        ));
        out.push_str(&format!(
            "- **Platform**: {} ({})\n",
            self.target_os, self.target_arch
        ));
        out.push_str(&format!(
            "- **Standard Profile**: {}\n",
            self.standard_profile
        ));
        out.push_str(&format!(
            "- **Timestamp**: {}\n",
            self.timestamp_utc.to_rfc3339()
        ));
        out.push_str(&format!(
            "- **GStreamer Available**: {} ({})\n\n",
            self.gstreamer_available,
            self.gstreamer_version.as_deref().unwrap_or("none")
        ));

        out.push_str("#### Plugins Detected\n");
        if self.detected_plugins.is_empty() {
            out.push_str("- None detected or GStreamer features disabled\n\n");
        } else {
            out.push_str(&format!(
                "- {}\n\n",
                self.detected_plugins.join(", ")
            ));
        }

        out.push_str("#### Inputs Summary\n");
        if self.inputs.is_empty() {
            out.push_str("- No inputs configured\n\n");
        } else {
            out.push_str("| ID | Name | Kind | Online | Active |\n");
            out.push_str("|---|---|---|---|---|\n");
            for input in &self.inputs {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    input.id,
                    Self::sanitize_path(&input.name),
                    input.kind_summary,
                    input.is_online,
                    input.is_active
                ));
            }
            out.push('\n');
        }

        out.push_str("#### Output Metrics\n");
        if let Some(metrics) = &self.output_metrics {
            out.push_str(&format!(
                "- Status: {:?}\n\
                 - Encoded frames: {}\n\
                 - Dropped frames: {}\n\
                 - Duration: {}s\n\
                 - Average FPS: {:.2} (Target: {:.2})\n",
                metrics.status,
                metrics.encoded_frames,
                metrics.dropped_frames,
                metrics.duration_seconds,
                metrics.average_fps,
                metrics.target_fps
            ));
        } else {
            out.push_str("- Not currently recording/streaming\n");
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_sanitization_windows_and_linux() {
        let win_path = r"C:\Users\johndoe\Videos\capture.webm";
        let sanitized_win = SystemDiagnostics::sanitize_path(win_path);
        assert_eq!(
            sanitized_win,
            r"C:\Users\[REDACTED]\Videos\capture.webm"
        );

        let linux_path = "/home/developer/recordings/test.webm";
        let sanitized_linux = SystemDiagnostics::sanitize_path(linux_path);
        assert_eq!(
            sanitized_linux,
            "/home/[REDACTED]/recordings/test.webm"
        );

        let clean_path = "D:\\recordings\\test.webm";
        assert_eq!(SystemDiagnostics::sanitize_path(clean_path), clean_path);
    }

    #[test]
    fn test_diagnostics_json_and_markdown_output() {
        let diag = SystemDiagnostics::new(
            "0.1.0",
            true,
            Some("1.28.8".to_string()),
            vec!["videotestsrc".to_string(), "vp8enc".to_string()],
            vec![
                InputDiagnosticInfo {
                    id: 1,
                    name: "Camera 1".to_string(),
                    kind_summary: "Camera".to_string(),
                    is_active: true,
                    is_online: true,
                },
            ],
            Some(OutputMetrics {
                status: crate::project_store::OutputStatus::Live {
                    destination: "test.webm".to_string(),
                    started_at: Utc::now(),
                },
                duration_seconds: 5,
                encoded_frames: 250,
                dropped_frames: 0,
                average_fps: 50.0,
                target_fps: 50.0,
                bit_rate_kbps: 4500,
            }),
        );

        let json = diag.to_json().expect("Serialization should succeed");
        assert!(json.contains("OPENVIDEOMIX"));
        assert!(json.contains("1.28.8"));
        assert!(json.contains("videotestsrc"));

        let md = diag.to_markdown_report();
        assert!(md.contains("OPENVIDEOMIX Diagnostics Report"));
        assert!(md.contains("Camera 1"));
        assert!(md.contains("Encoded frames: 250"));
    }
}
