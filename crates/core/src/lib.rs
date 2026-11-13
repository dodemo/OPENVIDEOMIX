//! OPENVIDEOMIX Core Engine Types and State Management
//!
//! Provides the primary broadcast models: Inputs, Mixer State,
//! Audio Buses, Overlays, Transitions, and Settings models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique identifier for an input source
pub type InputId = u32;

/// Supported broadcast input formats
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputKind {
    Camera { device_path: String },
    ScreenCapture { display_id: u32 },
    NetworkStream { url: String, protocol: String },
    MediaFile { path: String },
    TitleOverlay { title: String, subtitle: String },
    ColorBars,
    AudioOnly,
}

/// Represents an individual input item in the switcher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInput {
    pub id: InputId,
    pub name: String,
    pub kind: InputKind,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub is_online: bool,
    pub volume: f32, // 0.0 to 1.0
    pub is_muted: bool,
    pub afv: bool, // Audio Follows Video
    pub audio_meter_peak: f32, // 0.0 to 1.0 for UI display
    pub is_tally_program: bool,
    pub is_tally_preview: bool,
    pub is_overlay_active: bool,
}

impl VideoInput {
    pub fn new(id: InputId, name: impl Into<String>, kind: InputKind) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            width: 1920,
            height: 1080,
            fps: 60,
            is_online: true,
            volume: 0.85,
            is_muted: false,
            afv: true,
            audio_meter_peak: 0.0,
            is_tally_program: false,
            is_tally_preview: false,
            is_overlay_active: false,
        }
    }
}

/// Broadcast transition types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionType {
    Cut,
    Fade,
    Merge,
    Wipe,
}

/// Audio Bus channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioBus {
    pub name: String,
    pub volume: f32, // 0.0 to 1.0
    pub muted: bool,
    pub solo: bool,
    pub peak_l: f32, // 0.0 to 1.0
    pub peak_r: f32, // 0.0 to 1.0
}

impl AudioBus {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            volume: 0.80,
            muted: false,
            solo: false,
            peak_l: 0.0,
            peak_r: 0.0,
        }
    }
}

/// Studio and hardware configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub output_resolution: String, // e.g. "1920x1080"
    pub frame_rate: String,        // e.g. "59.94p"
    pub audio_device: String,
    pub audio_sample_rate: u32,
    pub pcgrande_endpoint: String,
    pub pcgrande_model: String,
    pub openai_api_key: String,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            output_resolution: "1920x1080 (Full HD)".into(),
            frame_rate: "59.94 fps".into(),
            audio_device: "SN6140 Analog / PipeWire".into(),
            audio_sample_rate: 48000,
            pcgrande_endpoint: "http://192.168.0.100:11434".into(),
            pcgrande_model: "qwen2.5-coder:32b".into(),
            openai_api_key: "".into(),
        }
    }
}

/// Global production state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerState {
    pub program_input: Option<InputId>,
    pub preview_input: Option<InputId>,
    pub active_overlay_1: Option<InputId>,
    pub overlay_opacity: f32, // 0.0 to 1.0
    pub active_transition: TransitionType,
    pub transition_duration_ms: u32,
    pub tbar_position: f32, // 0.0 (Preview) to 1.0 (Program)
    pub inputs: Vec<VideoInput>,
    pub master_bus: AudioBus,
    pub bus_a: AudioBus,
    pub bus_b: AudioBus,
    pub headphones_bus: AudioBus,
    pub is_streaming: bool,
    pub is_recording: bool,
    pub is_external_out: bool,
    pub fps: f32,
    pub cpu_usage_pct: f32,
    pub dropped_frames: u64,
    pub settings: ProjectSettings,
    pub started_at: DateTime<Utc>,
}

impl Default for MixerState {
    fn default() -> Self {
        let mut inputs = vec![
            VideoInput::new(1, "CAM 1 - Principal (USB/V4L2)", InputKind::Camera { device_path: "/dev/video0".into() }),
            VideoInput::new(2, "CAM 2 - Plano Xeral (USB/V4L2)", InputKind::Camera { device_path: "/dev/video2".into() }),
            VideoInput::new(3, "CAPTURA - Escritorio / Pantalla", InputKind::ScreenCapture { display_id: 0 }),
            VideoInput::new(4, "VÍDEO - Clip Promocional", InputKind::MediaFile { path: "assets/intro.mp4".into() }),
            VideoInput::new(5, "RÓTULO - Lower Third (PNG)", InputKind::TitleOverlay {
                title: "PROGRAMA EN DIRECTO".into(),
                subtitle: "OPENVIDEOMIX Studio • vMix Alternativa Aberta".into(),
            }),
            VideoInput::new(6, "BARRAS - SMPTE 60fps Test", InputKind::ColorBars),
        ];

        // Program initial: CAM 1, Preview initial: BARRAS
        inputs[0].is_tally_program = true;
        inputs[5].is_tally_preview = true;

        Self {
            program_input: Some(1),
            preview_input: Some(6),
            active_overlay_1: Some(5), // Rótulo dispoñible para Overlay 1
            overlay_opacity: 1.0,
            active_transition: TransitionType::Fade,
            transition_duration_ms: 500,
            tbar_position: 0.0,
            inputs,
            master_bus: AudioBus::new("Master"),
            bus_a: AudioBus::new("Bus A"),
            bus_b: AudioBus::new("Bus B"),
            headphones_bus: AudioBus::new("Auriculares"),
            is_streaming: false,
            is_recording: false,
            is_external_out: false,
            fps: 59.94,
            cpu_usage_pct: 3.2,
            dropped_frames: 0,
            settings: ProjectSettings::default(),
            started_at: Utc::now(),
        }
    }
}

impl MixerState {
    /// Execute an instant CUT transition between Preview and Program
    pub fn cut(&mut self) {
        let old_program = self.program_input;
        let old_preview = self.preview_input;

        self.program_input = old_preview;
        self.preview_input = old_program;
        self.tbar_position = 0.0;
        self.update_tally_flags();
    }

    /// Select an input as Preview
    pub fn select_preview(&mut self, id: InputId) {
        self.preview_input = Some(id);
        self.update_tally_flags();
    }

    /// Select an input directly to Program
    pub fn select_program(&mut self, id: InputId) {
        self.program_input = Some(id);
        self.update_tally_flags();
    }

    /// Toggle Overlay 1 for a specific input
    pub fn toggle_overlay_1(&mut self, id: InputId) {
        if self.active_overlay_1 == Some(id) {
            self.active_overlay_1 = None;
        } else {
            self.active_overlay_1 = Some(id);
        }
        for input in &mut self.inputs {
            input.is_overlay_active = self.active_overlay_1 == Some(input.id);
        }
    }

    /// Remove an input from the mixer cleanly and safely
    pub fn remove_input(&mut self, id: InputId) {
        self.inputs.retain(|i| i.id != id);

        if self.program_input == Some(id) {
            self.program_input = self.inputs.first().map(|i| i.id);
        }
        if self.preview_input == Some(id) {
            self.preview_input = self.inputs.first().map(|i| i.id);
        }
        if self.active_overlay_1 == Some(id) {
            self.active_overlay_1 = None;
        }
        self.update_tally_flags();
    }

    /// Update tally state for all inputs
    pub fn update_tally_flags(&mut self) {
        for input in &mut self.inputs {
            input.is_tally_program = self.program_input == Some(input.id);
            input.is_tally_preview = self.preview_input == Some(input.id);
            input.is_overlay_active = self.active_overlay_1 == Some(input.id);
        }
    }

    /// Set T-bar position (0.0 to 1.0). When reaching 1.0, complete transition
    pub fn set_tbar(&mut self, value: f32) {
        let clamped = value.clamp(0.0, 1.0);
        self.tbar_position = clamped;
        if clamped >= 0.999 {
            self.cut();
        }
    }
}
