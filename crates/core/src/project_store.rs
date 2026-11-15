//! Versioned project store (.ovmix) and output state machines.
//!
//! Provides atomic project saving, schema migrations, graceful missing-input
//! restoration, and truthful operational status state machines for recording
//! and RTMP streaming.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::midi::MidiProfile;
use crate::{
    AudioBus, DEMO_CAMERA_PATH, InputId, InputKind, MixerState, ProjectSettings, TransitionType,
    VideoInput,
};

pub const CURRENT_PROJECT_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ProjectStoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("unsupported project format version: {0} (latest is {CURRENT_PROJECT_VERSION})")]
    UnsupportedVersion(u32),
    #[error("atomic write failed: could not commit temporary file")]
    AtomicWriteFailed,
}

/// A serialized input entry in the project file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectInputEntry {
    pub id: InputId,
    pub name: String,
    pub kind: InputKind,
    pub volume: f32,
    pub is_muted: bool,
    pub afv: bool,
}

/// Versioned, portable project document (.ovmix).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub format_version: u32,
    pub created_at: DateTime<Utc>,
    pub app_version: String,
    pub settings: ProjectSettings,
    pub inputs: Vec<ProjectInputEntry>,
    pub program_input: Option<InputId>,
    pub preview_input: Option<InputId>,
    #[serde(default)]
    pub active_overlays: [Option<InputId>; 4],
    pub active_transition: TransitionType,
    pub transition_duration_ms: u32,
    pub master_bus: AudioBus,
    pub bus_a: AudioBus,
    pub bus_b: AudioBus,
    pub headphones_bus: AudioBus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi_profile: Option<MidiProfile>,
}

/// Summary report returned when opening a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLoadReport {
    pub missing_inputs: Vec<InputId>,
    pub migrated_from_version: Option<u32>,
}

impl ProjectFile {
    /// Create a project document from current mixer state.
    pub fn from_mixer_state(state: &MixerState, midi_profile: Option<MidiProfile>) -> Self {
        let inputs = state
            .inputs
            .iter()
            .map(|i| ProjectInputEntry {
                id: i.id,
                name: i.name.clone(),
                kind: i.kind.clone(),
                volume: i.volume,
                is_muted: i.is_muted,
                afv: i.afv,
            })
            .collect();

        Self {
            format_version: CURRENT_PROJECT_VERSION,
            created_at: Utc::now(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            settings: state.settings.clone(),
            inputs,
            program_input: state.program_input,
            preview_input: state.preview_input,
            active_overlays: state.active_overlays,
            active_transition: state.active_transition,
            transition_duration_ms: state.transition_duration_ms,
            master_bus: state.master_bus.clone(),
            bus_a: state.bus_a.clone(),
            bus_b: state.bus_b.clone(),
            headphones_bus: state.headphones_bus.clone(),
            midi_profile,
        }
    }

    /// Restore MixerState from this project. If referenced files or cameras are missing,
    /// marks them as offline without failing or discarding them from the rack slots.
    pub fn restore_mixer_state(&self) -> (MixerState, ProjectLoadReport) {
        let mut missing_inputs = Vec::new();
        let mut restored_inputs = Vec::with_capacity(self.inputs.len());

        for entry in &self.inputs {
            let mut is_online = true;

            // Check if media files exist locally
            match &entry.kind {
                InputKind::MediaFile { path } | InputKind::ImageFile { path } => {
                    if !Path::new(path).exists() {
                        is_online = false;
                        missing_inputs.push(entry.id);
                    }
                }
                InputKind::Camera { device_path } if device_path == DEMO_CAMERA_PATH => {
                    // Demo cameras stay online as synthetic generators
                    is_online = true;
                }
                _ => {}
            }

            let mut input = VideoInput::new(entry.id, &entry.name, entry.kind.clone());
            input.volume = entry.volume;
            input.is_muted = entry.is_muted;
            input.afv = entry.afv;
            input.is_online = is_online;
            restored_inputs.push(input);
        }

        let mut mixer = MixerState {
            program_input: self.program_input,
            preview_input: self.preview_input,
            active_overlay_1: self.active_overlays[0],
            active_overlays: self.active_overlays,
            overlay_opacity: 1.0,
            active_transition: self.active_transition,
            transition_duration_ms: self.transition_duration_ms,
            tbar_position: 0.0,
            inputs: restored_inputs,
            master_bus: self.master_bus.clone(),
            bus_a: self.bus_a.clone(),
            bus_b: self.bus_b.clone(),
            headphones_bus: self.headphones_bus.clone(),
            is_streaming: false,
            is_recording: false,
            recording_intent: None,
            is_external_out: false,
            fps: 50.0,
            cpu_usage_pct: 0.0,
            dropped_frames: 0,
            settings: self.settings.clone(),
            started_at: Utc::now(),
        };

        mixer.update_tally_flags();

        let report = ProjectLoadReport {
            missing_inputs,
            migrated_from_version: if self.format_version < CURRENT_PROJECT_VERSION {
                Some(self.format_version)
            } else {
                None
            },
        };

        (mixer, report)
    }

    /// Atomically write project to path using a temporary file in the destination directory.
    pub fn save_atomic(&self, path: impl AsRef<Path>) -> Result<(), ProjectStoreError> {
        let dest_path = path.as_ref();
        let parent = dest_path.parent().unwrap_or_else(|| Path::new("."));

        fs::create_dir_all(parent)?;

        let temp_path = parent.join(format!(
            ".{}.tmp.{}",
            dest_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("project"),
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        let json = serde_json::to_string_pretty(self)?;
        {
            let mut file = File::create(&temp_path)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
        }

        if let Err(e) = fs::rename(&temp_path, dest_path) {
            let _ = fs::remove_file(&temp_path);
            return Err(ProjectStoreError::Io(e));
        }

        Ok(())
    }

    /// Load and validate a project file from path.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProjectStoreError> {
        let data = fs::read_to_string(path)?;
        let project: Self = serde_json::from_str(&data)?;

        if project.format_version > CURRENT_PROJECT_VERSION {
            return Err(ProjectStoreError::UnsupportedVersion(project.format_version));
        }

        Ok(project)
    }
}

/// Operational output status for recording and live streaming.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputStatus {
    Idle,
    Starting { destination: String },
    Live {
        destination: String,
        started_at: DateTime<Utc>,
    },
    Stopping { destination: String },
    Error { message: String },
}

/// Continuous operational metrics for an output channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputMetrics {
    pub status: OutputStatus,
    pub duration_seconds: u64,
    pub encoded_frames: u64,
    pub dropped_frames: u64,
    pub average_fps: f32,
    pub target_fps: f32,
    pub bit_rate_kbps: u32,
}

impl OutputMetrics {
    pub fn new_idle(target_fps: f32) -> Self {
        Self {
            status: OutputStatus::Idle,
            duration_seconds: 0,
            encoded_frames: 0,
            dropped_frames: 0,
            average_fps: 0.0,
            target_fps,
            bit_rate_kbps: 0,
        }
    }

    pub fn start(&mut self, destination: impl Into<String>) {
        self.status = OutputStatus::Starting {
            destination: destination.into(),
        };
        self.duration_seconds = 0;
        self.encoded_frames = 0;
        self.dropped_frames = 0;
        self.average_fps = 0.0;
        self.bit_rate_kbps = 0;
    }

    pub fn mark_live(&mut self, destination: impl Into<String>) {
        self.status = OutputStatus::Live {
            destination: destination.into(),
            started_at: Utc::now(),
        };
    }

    pub fn stop(&mut self) {
        if let OutputStatus::Live { ref destination, .. } = self.status {
            self.status = OutputStatus::Stopping {
                destination: destination.clone(),
            };
        } else {
            self.status = OutputStatus::Idle;
        }
    }

    pub fn mark_idle(&mut self) {
        self.status = OutputStatus::Idle;
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.status = OutputStatus::Error {
            message: message.into(),
        };
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status, OutputStatus::Live { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_roundtrip_serialization_and_atomic_save() {
        let temp_dir = std::env::temp_dir().join(format!("ovmix_test_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        let project_path = temp_dir.join("test_session.ovmix");

        let state = MixerState::default();
        let project = ProjectFile::from_mixer_state(&state, None);

        // Atomic write
        project.save_atomic(&project_path).unwrap();
        assert!(project_path.exists());

        // Load and verify equality
        let loaded = ProjectFile::load(&project_path).unwrap();
        assert_eq!(loaded.format_version, CURRENT_PROJECT_VERSION);
        assert_eq!(loaded.program_input, Some(1));
        assert_eq!(loaded.preview_input, Some(5));
        assert_eq!(loaded.inputs.len(), state.inputs.len());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn restore_mixer_state_handles_missing_files_gracefully() {
        let mut state = MixerState::default();
        // Add a media file pointing to a non-existent path
        state.inputs.push(VideoInput::new(
            99,
            "Missing Video",
            InputKind::MediaFile {
                path: "/non/existent/path/to/video.mp4".into(),
            },
        ));

        let project = ProjectFile::from_mixer_state(&state, None);
        let (restored, report) = project.restore_mixer_state();

        assert!(report.missing_inputs.contains(&99));
        assert!(report.missing_inputs.contains(&4));
        let missing_entry = restored.inputs.iter().find(|i| i.id == 99).unwrap();
        assert!(!missing_entry.is_online);
        // The remaining inputs stay intact and online
        assert_eq!(restored.inputs.len(), state.inputs.len());
    }

    #[test]
    fn output_state_machine_transitions() {
        let mut metrics = OutputMetrics::new_idle(50.0);
        assert_eq!(metrics.status, OutputStatus::Idle);
        assert!(!metrics.is_active());

        metrics.start("rtmp://localhost/live/stream");
        assert_eq!(
            metrics.status,
            OutputStatus::Starting {
                destination: "rtmp://localhost/live/stream".into()
            }
        );
        assert!(!metrics.is_active());

        metrics.mark_live("rtmp://localhost/live/stream");
        assert!(metrics.is_active());

        metrics.stop();
        assert_eq!(
            metrics.status,
            OutputStatus::Stopping {
                destination: "rtmp://localhost/live/stream".into()
            }
        );
        assert!(!metrics.is_active());

        metrics.mark_idle();
        assert_eq!(metrics.status, OutputStatus::Idle);

        metrics.set_error("Connection refused");
        assert_eq!(
            metrics.status,
            OutputStatus::Error {
                message: "Connection refused".into()
            }
        );
    }

    #[test]
    fn rejects_unsupported_future_version() {
        let temp_dir = std::env::temp_dir().join(format!("ovmix_ver_test_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        let project_path = temp_dir.join("future.ovmix");
        let _ = fs::create_dir_all(&temp_dir);

        let state = MixerState::default();
        let mut project = ProjectFile::from_mixer_state(&state, None);
        project.format_version = 999; // future unsupported format

        let json = serde_json::to_string_pretty(&project).unwrap();
        fs::write(&project_path, json).unwrap();

        let result = ProjectFile::load(&project_path);
        assert!(matches!(result, Err(ProjectStoreError::UnsupportedVersion(999))));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
