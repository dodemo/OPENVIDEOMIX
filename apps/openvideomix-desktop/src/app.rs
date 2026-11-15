#[cfg(feature = "gstreamer-capture")]
use crate::capture::{CameraEvent, CameraSession};
#[cfg(feature = "gstreamer-capture")]
use crate::capture_devices::{discover_video_devices, CameraDeviceInfo};
#[cfg(feature = "midi-input")]
use crate::midi_input::MidiInputService;
#[cfg(feature = "local-control-api")]
use crate::remote_control::RemoteControl;
use chrono::Local;
use eframe::egui::{
    self, Color32, Key, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, Vec2,
};
#[cfg(feature = "gstreamer-recording")]
use gstreamer as gst;
use openvideomix_core::commands::{CommandDispatcher, ProductionCommand};
#[cfg(feature = "midi-input")]
use openvideomix_core::midi::{
    MidiBinding, MidiEvent, MidiMapper, MidiProfile, MidiSource, MidiTarget,
};
use openvideomix_core::{
    InputId, InputKind, MixerState, ProjectFile, TitleData, VideoInput, DEMO_CAMERA_PATH,
};
#[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
use openvideomix_desktop::desktop_production::{DesktopProduction, RecordingState};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
/// The console displays at most 640x360 pixels; production buffers keep their
/// full requested dimensions. This avoids repeated full-HD GUI color conversion.
#[cfg(any(feature = "gstreamer-capture", test))]
fn monitor_image(width: usize, height: usize, rgba: &[u8]) -> egui::ColorImage {
    let step = width.div_ceil(640).max(height.div_ceil(360)).max(1);
    let size = [width.div_ceil(step), height.div_ceil(step)];
    let mut pixels = Vec::with_capacity(size[0] * size[1]);
    for y in (0..height).step_by(step) {
        for x in (0..width).step_by(step) {
            let index = (y * width + x) * 4;
            pixels.push(Color32::from_rgba_unmultiplied(
                rgba[index],
                rgba[index + 1],
                rgba[index + 2],
                rgba[index + 3],
            ));
        }
    }
    egui::ColorImage { size, pixels }
}

/// egui may replay one input through multiple layout passes. GUI commands must
/// run once per input timestamp; remote/MIDI dispatch remain independently ordered.
#[derive(Default)]
struct GuiCommandGate {
    time: Option<f64>,
    commands: Vec<ProductionCommand>,
}
impl GuiCommandGate {
    fn allow(&mut self, time: f64, command: &ProductionCommand) -> bool {
        if self.time != Some(time) {
            self.time = Some(time);
            self.commands.clear();
        }
        if self.commands.contains(command) {
            return false;
        }
        self.commands.push(command.clone());
        true
    }
}

fn camera_placeholder_text(device_path: &str) -> String {
    if device_path == DEMO_CAMERA_PATH {
        "CÁMARA DE DEMOSTRACIÓN\nCaptura non iniciada; dispositivo non seleccionado".into()
    } else {
        format!("WEBCAM: {device_path}\nSen fotogramas reais (captura non iniciada)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    pub fn is_dark(&self) -> bool {
        matches!(self, Self::Dark)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Dark => "🌙 Modo Escuro (Broadcast Dark Studio)",
            Self::Light => "☀️ Modo Claro (Clean Light Studio)",
        }
    }

    pub fn visuals(&self) -> egui::Visuals {
        match self {
            Self::Dark => {
                let mut visuals = egui::Visuals::dark();
                visuals.panel_fill = Color32::from_rgb(18, 20, 24);
                visuals.window_fill = Color32::from_rgb(24, 27, 32);
                visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(28, 32, 38);
                visuals.widgets.inactive.bg_fill = Color32::from_rgb(36, 40, 48);
                visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 54, 64);
                visuals.widgets.active.bg_fill = Color32::from_rgb(58, 66, 78);
                visuals
            }
            Self::Light => {
                let mut visuals = egui::Visuals::light();
                visuals.panel_fill = Color32::from_rgb(240, 243, 246);
                visuals.window_fill = Color32::from_rgb(248, 250, 252);
                visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(230, 235, 240);
                visuals.widgets.inactive.bg_fill = Color32::from_rgb(218, 224, 232);
                visuals.widgets.hovered.bg_fill = Color32::from_rgb(200, 210, 222);
                visuals.widgets.active.bg_fill = Color32::from_rgb(185, 196, 210);
                visuals
            }
        }
    }
}

pub struct OpenVideoMixApp {
    pub mixer: MixerState,
    pub dispatcher: CommandDispatcher,
    gui_time: f64,
    gui_commands: GuiCommandGate,
    pub theme_mode: ThemeMode,
    pub ui_scale: f32,
    pub unicode_test_text: String,
    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
    production: Option<DesktopProduction>,
    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
    program_texture: Option<TextureHandle>,
    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
    program_frame_index: Option<u64>,
    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
    recording_intent_seen: u64,
    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
    show_recording_setup: bool,
    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
    recording_path: String,
    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
    live_audio_factory: String,
    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
    live_audio_device: String,
    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
    live_audio_selected: bool,

    #[cfg(feature = "local-control-api")]
    remote: Option<RemoteControl>,
    pub status_notification: Option<String>,
    pub show_settings: bool,
    pub show_diagnostics: bool,
    pub show_title_designer: bool,
    pub selected_title_input: Option<InputId>,
    pub editing_title: TitleData,
    pub current_project_path: Option<PathBuf>,
    pub recording_dir: PathBuf,
    pub recording_filename_prefix: String,
    pub recording_start_instant: Option<Instant>,
    pub recording_elapsed_secs: u64,
    pub show_recording_panel: bool,
    pub show_stop_recording_confirm: bool,
    pub show_close_warning_modal: bool,
    pub force_close_confirmed: bool,
    pub settings_tab: usize, // 0: Video, 1: Audio, 2: Dispositivos
    pub show_safe_areas: bool,
    pub input_search_filter: String,
    pub input_category_filter: usize,
    pub audio_mixer_collapsed: bool,
    pub transition_duration_ms: u32,
    pub last_frame_time: Instant,
    pub animated_tick: f32,
    pub connected_v4l2_devices: Vec<String>,
    pub loaded_textures: HashMap<InputId, TextureHandle>,
    #[cfg(feature = "gstreamer-capture")]
    capture_sessions: HashMap<InputId, CameraSession>,
    #[cfg(feature = "gstreamer-capture")]
    discovered_cameras: Vec<CameraDeviceInfo>,
    #[cfg(feature = "gstreamer-capture")]
    camera_discovery_rx: Option<std::sync::mpsc::Receiver<Result<Vec<CameraDeviceInfo>, String>>>,
    #[cfg(feature = "midi-input")]
    midi_ports: Vec<String>,
    #[cfg(feature = "midi-input")]
    midi_port_selected: usize,
    #[cfg(feature = "midi-input")]
    midi_session: Option<MidiInputService>,
    #[cfg(feature = "midi-input")]
    midi_mapper: MidiMapper,
    #[cfg(feature = "midi-input")]
    midi_learn: Option<MidiTarget>,
}

impl OpenVideoMixApp {
    pub fn configure_fonts(ctx: &egui::Context) {
        let fonts = egui::FontDefinitions::default();
        ctx.set_fonts(fonts);
    }

    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::configure_fonts(&_cc.egui_ctx);
        let mut app = Self {
            mixer: MixerState::default(),
            dispatcher: CommandDispatcher::default(),
            gui_time:0.0,
            gui_commands:GuiCommandGate::default(),
            theme_mode: ThemeMode::Dark,
            ui_scale: 1.0,
            unicode_test_text: "Ánxel e Iñaki están en directo desde a Coruña: vídeo público, ¿todo preparado? ¡Listo! · 1080p50 «OPENVIDEOMIX»".into(),
            #[cfg(all(feature="gstreamer-capture",feature="gstreamer-recording"))]
            production: None,
            #[cfg(all(feature="gstreamer-capture",feature="gstreamer-recording"))]
            program_texture: None,
            #[cfg(all(feature="gstreamer-capture",feature="gstreamer-recording"))]
            program_frame_index: None,
            #[cfg(all(feature="gstreamer-capture",feature="gstreamer-recording"))]
            recording_intent_seen: 0,
            #[cfg(all(feature="gstreamer-capture",feature="gstreamer-recording"))]
            show_recording_setup: false,
            #[cfg(all(feature="gstreamer-capture",feature="gstreamer-recording"))]
            recording_path: std::env::current_dir().unwrap_or_else(|_|std::env::temp_dir()).join("recordings").join(format!("openvideomix-{}.webm",Local::now().format("%Y%m%d-%H%M%S"))).to_string_lossy().into_owned(),
            #[cfg(all(feature="gstreamer-capture",feature="gstreamer-recording"))]
            live_audio_factory: "audiotestsrc".into(),
            #[cfg(all(feature="gstreamer-capture",feature="gstreamer-recording"))]
            live_audio_device: String::new(),
            #[cfg(all(feature="gstreamer-capture",feature="gstreamer-recording"))]
            live_audio_selected: false,


            #[cfg(feature = "local-control-api")]
            remote: None,
            status_notification: Some("Modo demostración: 50 FPS é só o perfil solicitado. A captura e as saídas aínda non están operativas.".into()),
            show_settings: false,
            show_diagnostics: false,
            show_title_designer: false,
            selected_title_input: Some(6),
            editing_title: TitleData::default(),
            current_project_path: None,
            recording_dir: std::env::current_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("recordings"),
            recording_filename_prefix: "openvideomix_rec".to_string(),
            recording_start_instant: None,
            recording_elapsed_secs: 0,
            show_recording_panel: false,
            show_stop_recording_confirm: false,
            show_close_warning_modal: false,
            force_close_confirmed: false,
            settings_tab: 0,
            show_safe_areas: false,
            input_search_filter: String::new(),
            input_category_filter: 0,
            audio_mixer_collapsed: false,
            transition_duration_ms: 1000,
            last_frame_time: Instant::now(),
            animated_tick: 0.0,
            connected_v4l2_devices: Vec::new(),
            loaded_textures: HashMap::new(),
            #[cfg(feature = "gstreamer-capture")]
            capture_sessions: HashMap::new(),
            #[cfg(feature = "gstreamer-capture")]
            discovered_cameras: Vec::new(),
            #[cfg(feature = "gstreamer-capture")]
            camera_discovery_rx: None,
            #[cfg(feature = "midi-input")]
            midi_ports: Vec::new(),
            #[cfg(feature = "midi-input")]
            midi_port_selected: 0,
            #[cfg(feature = "midi-input")]
            midi_session: None,
            #[cfg(feature = "midi-input")]
            midi_mapper: MidiMapper::default(),
            #[cfg(feature = "midi-input")]
            midi_learn: None,
        };
        app.refresh_video_devices();
        #[cfg(feature = "gstreamer-capture")]
        app.begin_camera_discovery();
        #[cfg(feature = "midi-input")]
        app.refresh_midi_ports();
        #[cfg(feature = "local-control-api")]
        match RemoteControl::start_if_configured(&app.mixer) {
            Ok(Some(server)) => {
                app.remote = Some(server);
                app.status_notification =
                    Some("API local autenticada activa en 127.0.0.1:21873".into());
            }
            Ok(None) => {}
            Err(error) => app.status_notification = Some(format!("API local desactivada: {error}")),
        }
        app
    }

    #[cfg(test)]
    pub fn new_test() -> Self {
        Self {
            mixer: MixerState::default(),
            dispatcher: CommandDispatcher::default(),
            gui_time: 0.0,
            gui_commands: GuiCommandGate::default(),
            theme_mode: ThemeMode::Dark,
            ui_scale: 1.0,
            unicode_test_text: "Ánxel e Iñaki están en directo desde a Coruña: vídeo público, ¿todo preparado? ¡Listo! · 1080p50 «OPENVIDEOMIX»".into(),
            #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
            production: None,
            #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
            program_texture: None,
            #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
            program_frame_index: None,
            #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
            recording_intent_seen: 0,
            #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
            show_recording_setup: false,
            #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
            recording_path: String::new(),
            #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
            live_audio_factory: "audiotestsrc".into(),
            #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
            live_audio_device: String::new(),
            #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
            live_audio_selected: false,
            #[cfg(feature = "local-control-api")]
            remote: None,
            status_notification: None,
            show_settings: false,
            show_diagnostics: false,
            show_title_designer: false,
            selected_title_input: Some(6),
            editing_title: TitleData::default(),
            current_project_path: None,
            recording_dir: std::env::current_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("recordings"),
            recording_filename_prefix: "openvideomix_rec".to_string(),
            recording_start_instant: None,
            recording_elapsed_secs: 0,
            show_recording_panel: false,
            show_stop_recording_confirm: false,
            show_close_warning_modal: false,
            force_close_confirmed: false,
            settings_tab: 0,
            show_safe_areas: false,
            input_search_filter: String::new(),
            input_category_filter: 0,
            audio_mixer_collapsed: false,
            transition_duration_ms: 1000,
            last_frame_time: Instant::now(),
            animated_tick: 0.0,
            connected_v4l2_devices: Vec::new(),
            loaded_textures: HashMap::new(),
            #[cfg(feature = "gstreamer-capture")]
            capture_sessions: HashMap::new(),
            #[cfg(feature = "gstreamer-capture")]
            discovered_cameras: Vec::new(),
            #[cfg(feature = "gstreamer-capture")]
            camera_discovery_rx: None,
            #[cfg(feature = "midi-input")]
            midi_ports: Vec::new(),
            #[cfg(feature = "midi-input")]
            midi_port_selected: 0,
            #[cfg(feature = "midi-input")]
            midi_session: None,
            #[cfg(feature = "midi-input")]
            midi_mapper: MidiMapper::default(),
            #[cfg(feature = "midi-input")]
            midi_learn: None,
        }
    }

    /// One validated command entry point for the GUI and external surfaces.
    fn apply_command(&mut self, command: ProductionCommand) {
        if !self.gui_commands.allow(self.gui_time, &command) {
            return;
        }
        if let Err(error) = self.dispatcher.dispatch(&mut self.mixer, command) {
            self.status_notification = Some(format!("Orde rexeitada: {error}"));
        }
    }

    /// Save mixer and production configuration to .ovmix project document.
    pub fn save_project_to_path(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), String> {
        #[cfg(feature = "midi-input")]
        let midi_profile = Some(self.midi_mapper.profile.clone());
        #[cfg(not(feature = "midi-input"))]
        let midi_profile = None;

        let project = ProjectFile::from_mixer_state(&self.mixer, midi_profile);
        project
            .save_atomic(&path)
            .map_err(|e| format!("Erro ao gardar proxecto: {e}"))?;
        self.current_project_path = Some(path.as_ref().to_path_buf());
        self.status_notification = Some(format!(
            "Proxecto gardado con éxito en: {}",
            path.as_ref().display()
        ));
        Ok(())
    }

    /// Load and restore mixer configuration from .ovmix project document.
    pub fn load_project_from_path(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), String> {
        let project =
            ProjectFile::load(&path).map_err(|e| format!("Erro ao cargar proxecto: {e}"))?;
        let (restored, report) = project.restore_mixer_state();
        self.mixer = restored;
        self.current_project_path = Some(path.as_ref().to_path_buf());
        if !report.missing_inputs.is_empty() {
            self.status_notification = Some(format!(
                "Proxecto cargado con {} entrada(s) sen arquivo local (marcadas como offline)",
                report.missing_inputs.len()
            ));
        } else {
            self.status_notification = Some(format!(
                "Proxecto cargado con éxito desde: {}",
                path.as_ref().display()
            ));
        }
        Ok(())
    }

    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
    fn start_test_studio(&mut self) {
        if self.production.is_some() {
            return;
        }
        let Some(first) = self
            .mixer
            .inputs
            .iter()
            .map(|input| input.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
        else {
            return;
        };
        let Some(end) = first.checked_add(4) else {
            return;
        };
        let ids: Vec<_> = (first..end).collect();
        match DesktopProduction::start(&ids) {
            Ok(production) => {
                for (pattern, id) in ids.into_iter().enumerate() {
                    let mut input = VideoInput::new(
                        id,
                        format!("TEST {} · 1080p50", pattern + 1),
                        InputKind::Camera {
                            device_path: format!("production-test:{pattern}"),
                        },
                    );
                    input.is_online = false;
                    self.mixer.inputs.push(input);
                    self.capture_sessions.insert(
                        id,
                        CameraSession::start_production_test(
                            id,
                            pattern as u32,
                            production.ingress(),
                        ),
                    );
                }
                self.production = Some(production);
                self.apply_command(ProductionCommand::SelectPreview { input: first });
                self.status_notification=Some("4 sinais de proba 1080p50. Preme CUT para envialos a Program. A gravación usa ton de proba, non micrófono.".into());
            }
            Err(error) => self.status_notification = Some(error),
        }
    }

    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
    fn poll_production(&mut self, ctx: &egui::Context) {
        let Some(production) = &self.production else {
            return;
        };
        production.route(
            self.mixer.program_input,
            self.mixer.active_overlay_1,
            self.mixer.master_bus.volume,
            self.mixer.master_bus.muted,
        );
        if let Some(intent) = &self.mixer.recording_intent {
            if intent.revision != self.recording_intent_seen {
                self.recording_intent_seen = intent.revision;
                let result = match &intent.path {
                    Some(path) => match &intent.live_audio {
                        Some((factory, device)) => production.start_live_recording(
                            PathBuf::from(path),
                            openvideomix_desktop::live_audio::AudioSource {
                                factory: factory.clone(),
                                device: device.clone(),
                            },
                        ),
                        None => production.start_test_recording(PathBuf::from(path)),
                    },
                    None => production.stop_recording(),
                };
                if let Err(error) = result {
                    self.status_notification = Some(error);
                }
            }
        }
        let view = production.view();
        self.mixer.is_recording = matches!(view.recording, RecordingState::Recording { .. });
        if let Some((index, frame)) = view.latest {
            if self.program_frame_index != Some(index) {
                let image =
                    monitor_image(frame.width as usize, frame.height as usize, &frame.pixels);
                match self.program_texture.as_mut() {
                    Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                    None => {
                        self.program_texture = Some(ctx.load_texture(
                            "actual-program",
                            image,
                            egui::TextureOptions::LINEAR,
                        ))
                    }
                }
                self.program_frame_index = Some(index);
            }
        } else {
            self.program_texture = None;
            self.program_frame_index = None;
        }
    }

    #[cfg(feature = "midi-input")]
    fn refresh_midi_ports(&mut self) {
        match MidiInputService::ports() {
            Ok(ports) => {
                self.midi_ports = ports;
                if self.midi_port_selected >= self.midi_ports.len() {
                    self.midi_port_selected = 0;
                }
            }
            Err(message) => {
                self.midi_ports.clear();
                self.status_notification = Some(message);
            }
        }
    }

    #[cfg(feature = "midi-input")]
    fn save_midi_profile(&mut self) {
        let profile =
            MidiProfile::new("OPENVIDEOMIX controller", self.midi_mapper.bindings.clone());
        if let Err(message) = profile.validate() {
            self.status_notification = Some(format!("Perfil MIDI inválido: {message}"));
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("OPENVIDEOMIX MIDI profile", &["json"])
            .set_file_name("openvideomix-midi.json")
            .save_file()
        else {
            return;
        };
        let result = serde_json::to_vec_pretty(&profile)
            .map_err(|error| error.to_string())
            .and_then(|bytes| std::fs::write(&path, bytes).map_err(|error| error.to_string()));
        self.status_notification = Some(match result {
            Ok(()) => format!("Perfil MIDI gardado: {}", path.display()),
            Err(message) => format!("Erro ao gardar perfil MIDI: {message}"),
        });
    }

    #[cfg(feature = "midi-input")]
    fn load_midi_profile(&mut self) {
        const MAX_PROFILE_BYTES: u64 = 128 * 1024;
        let Some(path) = rfd::FileDialog::new()
            .add_filter("OPENVIDEOMIX MIDI profile", &["json"])
            .pick_file()
        else {
            return;
        };
        let load = (|| -> Result<MidiProfile, String> {
            let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
            if metadata.len() > MAX_PROFILE_BYTES {
                return Err("O perfil MIDI supera o tamaño permitido".into());
            }
            let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
            if bytes.len() as u64 > MAX_PROFILE_BYTES {
                return Err("O perfil MIDI supera o tamaño permitido".into());
            }
            let profile: MidiProfile =
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            profile.validate()?;
            Ok(profile)
        })();
        match load {
            Ok(profile) => {
                let count = profile.bindings.len();
                self.midi_mapper.replace_bindings(profile.bindings);
                self.midi_learn = None;
                self.status_notification =
                    Some(format!("Perfil MIDI cargado: {count} asignacións"));
            }
            Err(message) => {
                self.status_notification = Some(format!("Erro ao cargar perfil MIDI: {message}"));
            }
        }
    }

    #[cfg(feature = "midi-input")]
    fn poll_midi(&mut self) {
        let events = self.midi_session.as_ref().map(MidiInputService::drain);
        let Some(events) = events else {
            return;
        };
        for event in events {
            if matches!(event, MidiEvent::NoteOff { .. }) {
                // Release messages are still useful to mapper state.
                if self.midi_learn.is_some() {
                    continue;
                }
            }
            if let Some(target) = self.midi_learn.take() {
                let source = MidiSource::from_event(event);
                let mut bindings = self.midi_mapper.bindings.clone();
                // Exactly one action per physical control for this first UI.
                bindings.retain(|binding| binding.source != source);
                bindings.push(MidiBinding { source, target });
                self.midi_mapper.replace_bindings(bindings);
                self.status_notification = Some(
                    "Control MIDI asignado. Podes gardar o perfil desde Axustes > MIDI.".into(),
                );
                continue;
            }
            let commands = self.midi_mapper.process(event, &self.mixer);
            for command in commands {
                self.apply_command(command);
            }
        }
    }

    #[cfg(feature = "gstreamer-capture")]
    fn begin_camera_discovery(&mut self) {
        if self.camera_discovery_rx.is_some() {
            return;
        }
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        self.camera_discovery_rx = Some(receiver);
        std::thread::spawn(move || {
            let result = discover_video_devices();
            let _ = sender.send(result);
        });
    }

    #[cfg(feature = "gstreamer-capture")]
    fn poll_camera_discovery(&mut self) {
        let outcome =
            self.camera_discovery_rx
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(
                        "A exploración de cámaras rematou inesperadamente".into(),
                    )),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                });
        if let Some(result) = outcome {
            self.camera_discovery_rx = None;
            match result {
                Ok(cameras) => {
                    self.discovered_cameras = cameras;
                    self.status_notification = Some(format!(
                        "{} fontes de vídeo GStreamer detectadas",
                        self.discovered_cameras.len()
                    ));
                }
                Err(message) => {
                    self.discovered_cameras.clear();
                    self.status_notification = Some(message);
                }
            }
        }
    }

    /// Scan Linux video nodes; keep the demo state device-free on other systems.
    pub fn refresh_video_devices(&mut self) {
        self.connected_v4l2_devices.clear();
        #[cfg(target_os = "linux")]
        {
            for i in 0..16 {
                let path = format!("/dev/video{}", i);
                if std::path::Path::new(&path).exists() {
                    self.connected_v4l2_devices.push(path);
                }
            }
        }
    }

    /// Start an independent pipeline, associated with its own stable input ID.
    fn add_default_usb_camera(&mut self, _synthetic: bool) {
        #[cfg(feature = "gstreamer-capture")]
        {
            let next_id = self.mixer.inputs.iter().map(|i| i.id).max().unwrap_or(0) + 1;
            let mut input = VideoInput::new(
                next_id,
                if _synthetic {
                    "Sinal sintético GStreamer - a conectar"
                } else {
                    "Webcam USB (fonte predeterminada) - a conectar"
                },
                InputKind::Camera {
                    device_path: if _synthetic {
                        "synthetic:videotestsrc".into()
                    } else {
                        "auto:default".into()
                    },
                },
            );
            input.is_online = false;
            self.mixer.inputs.push(input);
            self.capture_sessions.insert(
                next_id,
                if _synthetic {
                    CameraSession::start_synthetic()
                } else {
                    CameraSession::start()
                },
            );
            self.status_notification = Some(if _synthetic {
                "Iniciando sinal de proba de GStreamer...".into()
            } else {
                "Abrindo a webcam predeterminada mediante GStreamer...".into()
            });
        }
        #[cfg(not(feature = "gstreamer-capture"))]
        {
            self.status_notification = Some(
                "Captura USB desactivada nesta compilación. Compila con --features gstreamer-capture.".into(),
            );
        }
    }

    #[cfg(feature = "gstreamer-capture")]
    fn poll_camera(&mut self, ctx: &egui::Context) {
        // Drain at most one latest frame per input. Never hold a borrow of
        // the sessions while uploading textures or mutating the mixer.
        let mut updates = Vec::new();
        let mut failures = Vec::new();
        for (&id, session) in &self.capture_sessions {
            let mut latest = None;
            while let Ok(event) = session.receiver.try_recv() {
                match event {
                    CameraEvent::Frame(frame) => latest = Some(frame),
                    CameraEvent::Error(message) => {
                        failures.push((id, message));
                        latest = None;
                        break;
                    }
                }
            }
            if let Some(frame) = latest {
                updates.push((id, frame));
            }
        }

        for (id, frame) in updates {
            let image = monitor_image(frame.width, frame.height, &frame.rgba);
            if let Some(texture) = self.loaded_textures.get_mut(&id) {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.loaded_textures.insert(
                    id,
                    ctx.load_texture(format!("camera_{id}"), image, egui::TextureOptions::LINEAR),
                );
                let pts = frame
                    .pts_ns
                    .map(|value| format!("{value} ns"))
                    .unwrap_or_else(|| "non dispoñible".into());
                let shared_pts = frame
                    .capture_pts_ns
                    .map(|value| format!("{value} ns"))
                    .unwrap_or_else(|| "non dispoñible".into());
                let duration = frame
                    .duration_ns
                    .map(|value| format!("{value} ns"))
                    .unwrap_or_else(|| "non dispoñible".into());
                self.status_notification = Some(format!(
                    "Fonte {id}: GStreamer activo; PTS local {pts}, PTS común {shared_pts}, duración {duration}."
                ));
            }
            if let Some(input) = self.mixer.inputs.iter_mut().find(|input| input.id == id) {
                input.width = frame.width as u32;
                input.height = frame.height as u32;
                let production_test = matches!(&input.kind,InputKind::Camera {device_path} if device_path.starts_with("production-test:"));
                input.fps = if production_test { 50 } else { 30 }; // Negotiated, never measured FPS.
                input.is_online = true;
                if production_test {
                    continue;
                }
                input.name = if matches!(
                    &input.kind,
                    InputKind::Camera { device_path } if device_path.starts_with("synthetic:")
                ) {
                    format!("Sinal sintético #{id} (proba)")
                } else {
                    format!("Webcam USB #{id} - vídeo capturado")
                };
            }
        }
        for (id, message) in failures {
            self.capture_sessions.remove(&id);
            self.loaded_textures.remove(&id);
            if let Some(input) = self.mixer.inputs.iter_mut().find(|input| input.id == id) {
                input.is_online = false;
                input.name = format!("Fonte #{id} - erro de captura");
            }
            self.status_notification = Some(format!("Fonte #{id}: {message}"));
        }
    }

    /// Load real PNG / Image file with alpha transparency from disk
    pub fn load_image_input(&mut self, ctx: &egui::Context, path: PathBuf) {
        let path_str = path.to_string_lossy().to_string();
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        match image::open(&path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    &rgba.into_raw(),
                );

                let next_id = (self.mixer.inputs.iter().map(|i| i.id).max().unwrap_or(0)) + 1;
                let texture_handle = ctx.load_texture(
                    format!("tex_{}", next_id),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );

                self.loaded_textures.insert(next_id, texture_handle);

                let mut input = VideoInput::new(
                    next_id,
                    format!("PNG: {}", file_name),
                    InputKind::ImageFile { path: path_str },
                );
                input.width = w;
                input.height = h;

                // Set as active overlay 1 automatically so the user can see it right away
                self.mixer.active_overlay_1 = Some(next_id);
                input.is_overlay_active = true;

                self.mixer.inputs.push(input);
                self.mixer.update_tally_flags();
                self.status_notification = Some(format!(
                    "Imaxe cargada con éxito: {} ({}x{})",
                    file_name, w, h
                ));
            }
            Err(err) => {
                self.status_notification = Some(format!("Erro ao abrir o ficheiro PNG: {}", err));
            }
        }
    }

    /// Update frame time, calculations and audio simulation
    fn update_simulation(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;
        if dt > 0.0001 {
            let instant_fps = 1.0 / dt;
            self.mixer.fps = (self.mixer.fps * 0.94) + (instant_fps * 0.06);
        }

        self.animated_tick += 0.05;

        // Advance media clips playback clock
        let elapsed_ms = (dt * 1000.0) as u64;
        if elapsed_ms > 0 {
            for player in self.mixer.media_players.values_mut() {
                if player.is_playing {
                    player.advance(elapsed_ms);
                }
            }
        }

        // Track live recording elapsed duration
        if self.is_recording_or_streaming() {
            if self.recording_start_instant.is_none() {
                self.recording_start_instant = Some(now);
            }
            if let Some(start) = self.recording_start_instant {
                self.recording_elapsed_secs = start.elapsed().as_secs();
            }
        } else {
            self.recording_start_instant = None;
            self.recording_elapsed_secs = 0;
        }

        self.mixer.master_bus.peak_l = 0.0;
        self.mixer.master_bus.peak_r = 0.0;
        for input in &mut self.mixer.inputs {
            input.audio_meter_peak = 0.0; // Source audio has not been captured.
        }
    }

    /// Render dynamic lower-third title graphic over a monitor canvas
    fn render_title_graphics(painter: &egui::Painter, rect: Rect, template: &TitleData) {
        let base_scale = (rect.width() / 1920.0).max(0.18);
        let scale = base_scale * template.scale;

        let x_pos = rect.min.x + (rect.width() * (template.x_percent.clamp(0.0, 95.0) / 100.0));
        let y_pos = rect.min.y + (rect.height() * (template.y_percent.clamp(0.0, 95.0) / 100.0));

        let headline_font =
            egui::FontId::proportional((template.font_size_headline * scale).max(9.0));
        let subtitle_font =
            egui::FontId::proportional((template.font_size_subtitle * scale).max(7.0));

        let text_color = Color32::from_rgba_unmultiplied(
            template.text_color[0],
            template.text_color[1],
            template.text_color[2],
            template.text_color[3],
        );
        let subtitle_color = Color32::from_rgba_unmultiplied(
            template.subtitle_color[0],
            template.subtitle_color[1],
            template.subtitle_color[2],
            template.subtitle_color[3],
        );

        let headline_galley =
            painter.layout_no_wrap(template.headline.clone(), headline_font, text_color);
        let subtitle_galley =
            painter.layout_no_wrap(template.subtitle.clone(), subtitle_font, subtitle_color);

        let text_w = headline_galley.size().x.max(subtitle_galley.size().x);
        let gap = 4.0 * scale;
        let text_h = headline_galley.size().y + subtitle_galley.size().y + gap;

        let pad_h = template.bg_padding * scale;
        let pad_v = (template.bg_padding * 0.75) * scale;
        let accent_w = 6.0 * scale;

        let box_w = text_w + (pad_h * 2.0) + accent_w + (6.0 * scale);
        let box_h = text_h + (pad_v * 2.0);

        let box_rect = Rect::from_min_size(Pos2::new(x_pos, y_pos), Vec2::new(box_w, box_h));

        // 1. Drop shadow
        if template.has_drop_shadow {
            let shadow_offset = Vec2::new(
                template.shadow_offset[0] * scale,
                template.shadow_offset[1] * scale,
            );
            let shadow_rect = box_rect.translate(shadow_offset);
            let shadow_col = Color32::from_rgba_unmultiplied(
                template.shadow_color[0],
                template.shadow_color[1],
                template.shadow_color[2],
                template.shadow_color[3],
            );
            painter.rect_filled(shadow_rect, template.corner_radius * scale, shadow_col);
        }

        // 2. Background box
        let bg_col = Color32::from_rgba_unmultiplied(
            template.bg_color[0],
            template.bg_color[1],
            template.bg_color[2],
            template.bg_color[3],
        );
        painter.rect_filled(box_rect, template.corner_radius * scale, bg_col);

        // 3. Left accent bar
        let accent_rect = Rect::from_min_size(box_rect.min, Vec2::new(accent_w, box_h));
        let accent_col = Color32::from_rgba_unmultiplied(
            template.accent_bar_color[0],
            template.accent_bar_color[1],
            template.accent_bar_color[2],
            template.accent_bar_color[3],
        );
        painter.rect_filled(accent_rect, template.corner_radius * scale, accent_col);

        // 4. Headline and Subtitle text galleys
        let text_x = box_rect.min.x + accent_w + pad_h + (4.0 * scale);
        let headline_y = box_rect.min.y + pad_v;
        let subtitle_y = headline_y + headline_galley.size().y + gap;

        painter.galley(Pos2::new(text_x, headline_y), headline_galley, text_color);
        painter.galley(
            Pos2::new(text_x, subtitle_y),
            subtitle_galley,
            subtitle_color,
        );
    }

    /// Render video or graphic content inside a monitor rect
    fn render_monitor_content(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        input_id: Option<InputId>,
        is_program: bool,
    ) {
        let painter = ui.painter().with_clip_rect(rect);

        if is_program && self.mixer.is_fade_to_black {
            painter.rect_filled(rect, 0.0, Color32::BLACK);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "⚫ FADE TO BLACK (FTB ACTIVO)\nProgram e son en negro",
                egui::FontId::proportional(16.0),
                Color32::from_rgb(255, 180, 0),
            );
            return;
        }
        #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
        if is_program && self.production.is_some() {
            painter.rect_filled(rect, 0.0, Color32::BLACK);
            if let Some(texture) = &self.program_texture {
                painter.image(
                    texture.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Program sen fotogramas válidos",
                    egui::FontId::proportional(16.0),
                    Color32::YELLOW,
                );
            }
            return;
        }
        let input = input_id.and_then(|id| self.mixer.inputs.iter().find(|i| i.id == id));

        match input {
            Some(inp) => match &inp.kind {
                InputKind::ImageFile { path: _ } => {
                    // Check if we have loaded texture
                    if let Some(texture) = self.loaded_textures.get(&inp.id) {
                        // Render image preserving aspect ratio or fitting
                        painter.rect_filled(rect, 0.0, Color32::from_rgb(10, 12, 16));
                        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                        painter.image(texture.id(), rect, uv, Color32::WHITE);
                    } else {
                        painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 25));
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("🖼️ PNG: {}", inp.name),
                            egui::FontId::proportional(15.0),
                            Color32::WHITE,
                        );
                    }
                }
                InputKind::ColorBars => {
                    // 7-Bar SMPTE PAL / Broadcast standard
                    let colors = [
                        Color32::from_rgb(235, 235, 235), // White
                        Color32::from_rgb(220, 220, 20),  // Yellow
                        Color32::from_rgb(20, 220, 220),  // Cyan
                        Color32::from_rgb(30, 215, 30),   // Green
                        Color32::from_rgb(220, 20, 220),  // Magenta
                        Color32::from_rgb(225, 30, 30),   // Red
                        Color32::from_rgb(20, 20, 225),   // Blue
                    ];
                    let bar_w = rect.width() / 7.0;
                    for (idx, color) in colors.iter().enumerate() {
                        let bar_rect = Rect::from_min_size(
                            Pos2::new(rect.min.x + (idx as f32 * bar_w), rect.min.y),
                            Vec2::new(bar_w + 1.0, rect.height() * 0.75),
                        );
                        painter.rect_filled(bar_rect, 0.0, *color);
                    }
                    // Bottom calibration area
                    let bottom_rect = Rect::from_min_size(
                        Pos2::new(rect.min.x, rect.min.y + rect.height() * 0.75),
                        Vec2::new(rect.width(), rect.height() * 0.25),
                    );
                    painter.rect_filled(bottom_rect, 0.0, Color32::from_rgb(18, 18, 18));

                    // Continuous moving indicator verifying real 50Hz frame rate
                    let scan_x = rect.min.x + ((self.animated_tick * 28.0) % rect.width());
                    let scan_rect = Rect::from_min_size(
                        Pos2::new(scan_x, bottom_rect.min.y + 2.0),
                        Vec2::new(14.0, bottom_rect.height() - 4.0),
                    );
                    painter.rect_filled(scan_rect, 2.0, Color32::from_rgb(0, 255, 180));

                    // Timecode display in 50 FPS PAL standard
                    let now_str = Local::now().format("%H:%M:%S").to_string();
                    let frames_50 = ((self.animated_tick * 25.0) as u32) % 50;
                    let tc = format!("{}:{:02} (animación UI)", now_str, frames_50);
                    painter.text(
                        Pos2::new(rect.center().x, bottom_rect.center().y),
                        egui::Align2::CENTER_CENTER,
                        tc,
                        egui::FontId::monospace(14.0),
                        Color32::WHITE,
                    );
                }
                InputKind::Camera { device_path } => {
                    if let Some(texture) = self.loaded_textures.get(&inp.id) {
                        painter.rect_filled(rect, 0.0, Color32::BLACK);
                        painter.image(
                            texture.id(),
                            rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        painter.rect_filled(rect, 0.0, Color32::from_rgb(26, 18, 18));
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            camera_placeholder_text(device_path),
                            egui::FontId::proportional(15.0),
                            Color32::from_rgb(240, 160, 60),
                        );
                    }
                }
                InputKind::ScreenCapture { display_id } => {
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(24, 28, 38));
                    let taskbar = Rect::from_min_size(
                        Pos2::new(rect.min.x, rect.max.y - 18.0),
                        Vec2::new(rect.width(), 18.0),
                    );
                    painter.rect_filled(taskbar, 0.0, Color32::from_rgb(12, 14, 18));
                    painter.text(
                        Pos2::new(taskbar.max.x - 8.0, taskbar.center().y),
                        egui::Align2::RIGHT_CENTER,
                        Local::now().format("%H:%M").to_string(),
                        egui::FontId::monospace(11.0),
                        Color32::LIGHT_GRAY,
                    );
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!(
                            "ESCRITORIO #{}\nVista de demostración: sen captura real",
                            display_id
                        ),
                        egui::FontId::proportional(16.0),
                        Color32::WHITE,
                    );
                }
                InputKind::MediaFile { path } => {
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 24, 30));
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!(
                            "VÍDEO: {}\nMarcador de posición: sen decodificación real",
                            path
                        ),
                        egui::FontId::proportional(15.0),
                        Color32::from_rgb(150, 200, 255),
                    );
                }
                InputKind::Title { template } => {
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(14, 16, 22));
                    Self::render_title_graphics(&painter, rect, template);
                }
                _ => {
                    painter.rect_filled(rect, 0.0, Color32::BLACK);
                }
            },
            None => {
                painter.rect_filled(rect, 0.0, Color32::from_rgb(12, 12, 14));
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "NINGUNHA ENTRADA SELECCIONADA",
                    egui::FontId::proportional(15.0),
                    Color32::GRAY,
                );
            }
        }

        // RENDER ACTIVE OVERLAYS 1-4 WITH TRUE ALPHA TRANSPARENCY OVER PROGRAM
        if is_program {
            for (idx, ov_id_opt) in self.mixer.active_overlays.iter().enumerate() {
                if let Some(ov_id) = ov_id_opt {
                    if let Some(inp) = self.mixer.inputs.iter().find(|i| i.id == *ov_id) {
                        if let InputKind::Title { template } = &inp.kind {
                            Self::render_title_graphics(&painter, rect, template);
                        }
                    }
                    if let Some(texture) = self.loaded_textures.get(ov_id) {
                        let banner_w = rect.width() * 0.85;
                        let banner_h =
                            banner_w * (texture.size_vec2().y / texture.size_vec2().x.max(1.0));
                        let max_h = rect.height() * 0.40;
                        let final_h = banner_h.min(max_h);
                        let final_w = if banner_h > max_h {
                            banner_w * (max_h / banner_h)
                        } else {
                            banner_w
                        };

                        // Slight vertical offset for distinct overlay layers
                        let y_offset = 12.0 + (idx as f32 * 6.0);
                        let ov_rect = Rect::from_min_size(
                            Pos2::new(
                                rect.center().x - (final_w / 2.0),
                                rect.max.y - final_h - y_offset,
                            ),
                            Vec2::new(final_w, final_h),
                        );

                        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                        painter.image(texture.id(), ov_rect, uv, Color32::WHITE);
                    }
                }
            }
        }

        // SAFE AREAS (90% Action Safe / 80% Title Safe + Center Crosshair)
        if self.show_safe_areas {
            let action_margin = Vec2::new(rect.width() * 0.05, rect.height() * 0.05);
            let action_rect = rect.shrink2(action_margin);
            painter.rect_stroke(
                action_rect,
                0.0,
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0, 220, 255, 140)),
                StrokeKind::Middle,
            );

            let title_margin = Vec2::new(rect.width() * 0.10, rect.height() * 0.10);
            let title_rect = rect.shrink2(title_margin);
            painter.rect_stroke(
                title_rect,
                0.0,
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 200, 0, 140)),
                StrokeKind::Middle,
            );

            let center = rect.center();
            let cross_size = 8.0;
            painter.line_segment(
                [
                    Pos2::new(center.x - cross_size, center.y),
                    Pos2::new(center.x + cross_size, center.y),
                ],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 130)),
            );
            painter.line_segment(
                [
                    Pos2::new(center.x, center.y - cross_size),
                    Pos2::new(center.x, center.y + cross_size),
                ],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 130)),
            );
        }
    }

    pub fn generate_diagnostics(&self) -> openvideomix_core::SystemDiagnostics {
        #[allow(unused_mut)]
        let mut plugins = Vec::new();
        let gst_available;
        let gst_version;

        #[cfg(feature = "gstreamer-recording")]
        {
            if gst::init().is_ok() {
                gst_available = true;
                gst_version = Some(gst::version_string().to_string());
                for name in &[
                    "videotestsrc",
                    "audiotestsrc",
                    "videoconvert",
                    "vp8enc",
                    "vorbisenc",
                    "webmmux",
                    "appsrc",
                ] {
                    if gst::ElementFactory::find(name).is_some() {
                        plugins.push((*name).to_string());
                    }
                }
            } else {
                gst_available = false;
                gst_version = None;
            }
        }
        #[cfg(not(feature = "gstreamer-recording"))]
        {
            gst_available = false;
            gst_version = None;
        }

        let inputs = self
            .mixer
            .inputs
            .iter()
            .map(|input| openvideomix_core::InputDiagnosticInfo {
                id: input.id,
                name: input.name.clone(),
                kind_summary: format!("{:?}", input.kind),
                is_active: Some(input.id) == self.mixer.preview_input
                    || Some(input.id) == self.mixer.program_input,
                is_online: input.is_online,
            })
            .collect();

        #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
        let output_metrics = self.production.as_ref().and_then(|p| {
            let view = p.view();
            if view.recording.active() {
                let duration_seconds = match &view.recording {
                    RecordingState::Recording { submitted, .. } => *submitted / 50,
                    _ => 0,
                };
                Some(openvideomix_core::OutputMetrics {
                    status: openvideomix_core::OutputStatus::Live {
                        destination: self.recording_path.clone(),
                        started_at: chrono::Utc::now(),
                    },
                    duration_seconds,
                    encoded_frames: view.stats.rendered_frames,
                    dropped_frames: view.stats.missed_deadlines,
                    average_fps: 50.0,
                    target_fps: 50.0,
                    bit_rate_kbps: 4500,
                })
            } else {
                None
            }
        });

        #[cfg(not(all(feature = "gstreamer-capture", feature = "gstreamer-recording")))]
        let output_metrics = None;

        openvideomix_core::SystemDiagnostics::new(
            "0.1.0-alpha",
            gst_available,
            gst_version,
            plugins,
            inputs,
            output_metrics,
        )
    }

    /// Returns true if recording, streaming, or recording start intent is currently active.
    pub fn is_recording_or_streaming(&self) -> bool {
        let mixer_active = self.mixer.is_recording
            || self.mixer.is_streaming
            || self
                .mixer
                .recording_intent
                .as_ref()
                .is_some_and(|intent| intent.path.is_some());
        #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
        let prod_active = self
            .production
            .as_ref()
            .map(DesktopProduction::view)
            .is_some_and(|view| view.recording.active());
        #[cfg(not(all(feature = "gstreamer-capture", feature = "gstreamer-recording")))]
        let prod_active = false;

        mixer_active || prod_active
    }
}

impl eframe::App for OpenVideoMixApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.gui_time = ctx.input(|input| input.time);

        // Broadcast Safety Close Guard: intercept OS close ([X], Alt+F4) if live/recording
        if ctx.input(|i| i.viewport().close_requested())
            && !self.force_close_confirmed
            && self.is_recording_or_streaming()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.show_close_warning_modal = true;
        }

        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::Q)) {
            if !self.force_close_confirmed && self.is_recording_or_streaming() {
                self.show_close_warning_modal = true;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        // Quick Save shortcut: Ctrl+S
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::S)) {
            if let Some(ref path) = self.current_project_path.clone() {
                let _ = self.save_project_to_path(path);
            } else if let Some(path) = rfd::FileDialog::new()
                .add_filter("Proxecto OPENVIDEOMIX (.ovmix)", &["ovmix", "json"])
                .set_file_name("estudo_directo.ovmix")
                .save_file()
            {
                let _ = self.save_project_to_path(path);
            }
        }
        #[cfg(feature = "local-control-api")]
        if let Some(remote) = &mut self.remote {
            remote.poll(&mut self.dispatcher, &mut self.mixer);
        }
        #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
        self.poll_production(ctx);
        self.update_simulation();
        #[cfg(feature = "gstreamer-capture")]
        self.poll_camera(ctx);
        #[cfg(feature = "gstreamer-capture")]
        self.poll_camera_discovery();
        #[cfg(feature = "midi-input")]
        self.poll_midi();

        // Consume one CUT key once; text entry must never trigger an on-air CUT.
        if !ctx.wants_keyboard_input()
            && ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, Key::Space)
                    || input.consume_key(egui::Modifiers::NONE, Key::Enter)
            })
        {
            self.apply_command(ProductionCommand::Cut);
        }
        if !ctx.wants_keyboard_input()
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::M))
        {
            self.apply_command(ProductionCommand::SetTransition {
                transition: openvideomix_core::TransitionType::Fade,
                duration_ms: self.transition_duration_ms,
            });
            self.apply_command(ProductionCommand::Cut);
        }
        if !ctx.wants_keyboard_input()
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::W))
        {
            self.apply_command(ProductionCommand::SetTransition {
                transition: openvideomix_core::TransitionType::Wipe,
                duration_ms: self.transition_duration_ms,
            });
            self.apply_command(ProductionCommand::Cut);
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::F12)) {
            self.apply_command(ProductionCommand::ToggleFadeToBlack);
        }
        if ctx.input(|i| i.key_pressed(Key::F1)) {
            self.show_settings = !self.show_settings;
        }
        if ctx.input(|i| i.key_pressed(Key::F2)) {
            self.show_safe_areas = !self.show_safe_areas;
        }
        if ctx.input(|i| i.key_pressed(Key::F3)) {
            self.audio_mixer_collapsed = !self.audio_mixer_collapsed;
        }
        for (idx, key) in [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ]
        .iter()
        .enumerate()
        {
            if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(*key)) {
                if let Some(inp) = self.mixer.inputs.get(idx) {
                    let id = inp.id;
                    self.apply_command(ProductionCommand::SelectPreview { input: id });
                }
            }
        }

        // Visual theme styling & UI scaling
        ctx.set_visuals(self.theme_mode.visuals());
        if (ctx.zoom_factor() - self.ui_scale).abs() > 0.001 {
            ctx.set_zoom_factor(self.ui_scale);
        }

        #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
        if self.show_recording_setup {
            let mut start = false;
            let mut cancel = false;
            egui::Window::new("Gravar Program · proba audiovisual").default_width(650.0).default_pos(Pos2::new(300.0,150.0)).collapsible(false).resizable(true).show(ctx,|ui| {
                ui.label("Sinal: Program composto. Audio estéreo 48 kHz.");
                ui.checkbox(&mut self.live_audio_selected, "Captura de audio en directo GStreamer (selección explícita)");
                if self.live_audio_selected {
                    ui.label("Fonte: audiotestsrc = sintética en directo; pulsesrc / wasapisrc / osxaudiosrc segundo plataforma. Sen conversión de taxa/canles.");
                    ui.horizontal(|ui| {ui.label("Elemento fonte:");ui.text_edit_singleline(&mut self.live_audio_factory);});
                    ui.horizontal(|ui| {ui.label("Dispositivo (baleiro = predeterminado):");ui.text_edit_singleline(&mut self.live_audio_device);});
                } else { ui.label("Ton Rust xerado de 1 kHz; sen micrófono."); }
                ui.label("Destino WebM (non se sobrescriben ficheiros):");
                ui.add(egui::TextEdit::singleline(&mut self.recording_path).desired_width(580.0));
                ui.horizontal(|ui| {start=ui.button("Iniciar gravación de proba").clicked();cancel=ui.button("Cancelar").clicked();});
            });
            if cancel {
                self.show_recording_setup = false;
            }
            if start {
                let path = PathBuf::from(&self.recording_path);
                let result = (|| {
                    let directory = path.parent().ok_or("Destino sen cartafol")?;
                    std::fs::create_dir_all(directory)
                        .map_err(|error| format!("Cartafol: {error}"))?;
                    self.production
                        .as_ref()
                        .ok_or("Estudo non iniciado")?
                        .approve_recording_directory(directory)?;
                    Ok::<(), String>(())
                })();
                match result {
                    Ok(()) => {
                        let command = if self.live_audio_selected {
                            ProductionCommand::StartLiveRecording {
                                path: self.recording_path.clone(),
                                factory: self.live_audio_factory.clone(),
                                device: if self.live_audio_device.trim().is_empty() {
                                    None
                                } else {
                                    Some(self.live_audio_device.clone())
                                },
                            }
                        } else {
                            ProductionCommand::StartTestRecording {
                                path: self.recording_path.clone(),
                            }
                        };
                        self.apply_command(command);
                        self.show_recording_setup = false;
                    }
                    Err(error) => self.status_notification = Some(error),
                }
            }
        }

        // TOP HEADER
        egui::TopBottomPanel::top("top_header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new("📹 OPENVIDEOMIX")
                        .color(Color32::from_rgb(0, 190, 255))
                        .strong(),
                );
                ui.label(RichText::new("v0.1.0-alpha").weak());
                ui.separator();

                ui.label(RichText::new("PERFÍL:").strong());
                ui.label(&self.mixer.settings.output_resolution);
                ui.label(
                    RichText::new(format!("@ {}", self.mixer.settings.frame_rate))
                        .color(Color32::from_rgb(0, 255, 180))
                        .strong(),
                );

                if self.is_recording_or_streaming() {
                    ui.separator();
                    let h = self.recording_elapsed_secs / 3600;
                    let m = (self.recording_elapsed_secs % 3600) / 60;
                    let s = self.recording_elapsed_secs % 60;
                    ui.label(
                        RichText::new(format!("🔴 REC {h:02}:{m:02}:{s:02}"))
                            .color(Color32::from_rgb(255, 60, 60))
                            .strong()
                            .monospace(),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled(false, egui::Button::new("EMITIR (pendente)"))
                        .on_disabled_hover_text(
                            "RTMP/SRT non están implementados. Este botón non inicia unha emisión.",
                        );
                    #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
                    {
                        let view = self.production.as_ref().map(DesktopProduction::view);
                        let active = view.as_ref().is_some_and(|view| view.recording.active())
                            || self.mixer.is_recording;
                        let can_interact =
                            (view.as_ref().is_some_and(|view| view.latest.is_some()) || active)
                                && (!active || !self.mixer.is_safety_locked);
                        let btn_text = if active {
                            "PARAR GRAVACIÓN"
                        } else {
                            "GRAVAR TESTE"
                        };
                        let stop_hover = if active && self.mixer.is_safety_locked {
                            "🔒 Estudo bloqueado por seguridade. Desbloquea para deter a gravación."
                        } else if active {
                            "Deter a gravación actual de Program (solicitará confirmación)"
                        } else {
                            "Grava Program real con ton de proba (1 kHz / 48 kHz). Sen mic."
                        };

                        if ui
                            .add_enabled(can_interact, egui::Button::new(btn_text))
                            .on_hover_text(stop_hover)
                            .clicked()
                        {
                            if active {
                                self.show_stop_recording_confirm = true;
                            } else {
                                self.show_recording_setup = true;
                            }
                        }
                    }
                    #[cfg(not(all(
                        feature = "gstreamer-capture",
                        feature = "gstreamer-recording"
                    )))]
                    {
                        let active = self.mixer.is_recording;
                        if active {
                            let can_interact = !self.mixer.is_safety_locked;
                            if ui
                                .add_enabled(can_interact, egui::Button::new("PARAR GRAVACIÓN"))
                                .on_hover_text(if can_interact {
                                    "Deter gravación (solicitará confirmación)"
                                } else {
                                    "🔒 Estudo bloqueado por seguridade. Desbloquea para deter."
                                })
                                .clicked()
                            {
                                self.show_stop_recording_confirm = true;
                            }
                        } else {
                            ui.add_enabled(false, egui::Button::new("GRAVAR (pendente)"));
                        }
                    }

                    // Broadcast Padlock (Cadeado de Seguridade)
                    let lock_text = if self.mixer.is_safety_locked {
                        "🔒 BLOQUEADO"
                    } else {
                        "🔓 DESBLOQUEADO"
                    };
                    let lock_fill = if self.mixer.is_safety_locked {
                        Color32::from_rgb(180, 40, 40)
                    } else if self.theme_mode.is_dark() {
                        Color32::from_rgb(45, 50, 60)
                    } else {
                        Color32::from_rgb(215, 222, 230)
                    };
                    let lock_text_color =
                        if self.mixer.is_safety_locked || self.theme_mode.is_dark() {
                            Color32::WHITE
                        } else {
                            Color32::from_rgb(30, 35, 45)
                        };
                    let lock_tooltip = if self.mixer.is_safety_locked {
                        "Cadeado de Seguridade ACTIVADO. Preme para desbloquear."
                    } else {
                        "Cadeado de Seguridade DESACTIVADO. Preme para bloquear o estudio."
                    };
                    let lock_btn =
                        egui::Button::new(RichText::new(lock_text).color(lock_text_color).strong())
                            .fill(lock_fill);
                    if ui.add(lock_btn).on_hover_text(lock_tooltip).clicked() {
                        self.apply_command(ProductionCommand::ToggleSafetyLock);
                    }

                    let btn_fill = if self.show_settings {
                        Color32::from_rgb(0, 120, 200)
                    } else if self.theme_mode.is_dark() {
                        Color32::from_rgb(45, 50, 60)
                    } else {
                        Color32::from_rgb(215, 222, 230)
                    };
                    let btn_text_color = if self.show_settings || self.theme_mode.is_dark() {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(30, 35, 45)
                    };
                    let settings_btn =
                        egui::Button::new(RichText::new("⚙ AXUSTES (F1)").color(btn_text_color))
                            .fill(btn_fill);
                    if ui.add(settings_btn).clicked() {
                        self.show_settings = !self.show_settings;
                    }

                    let diag_fill = if self.show_diagnostics {
                        Color32::from_rgb(0, 120, 200)
                    } else if self.theme_mode.is_dark() {
                        Color32::from_rgb(45, 50, 60)
                    } else {
                        Color32::from_rgb(215, 222, 230)
                    };
                    let diag_text_color = if self.show_diagnostics || self.theme_mode.is_dark() {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(30, 35, 45)
                    };
                    let diag_btn =
                        egui::Button::new(RichText::new("📋 DIAGNÓSTICO").color(diag_text_color))
                            .fill(diag_fill);
                    if ui
                        .add(diag_btn)
                        .on_hover_text(
                            "Exportar informe de diagnóstico redactado e sen datos persoais",
                        )
                        .clicked()
                    {
                        self.show_diagnostics = !self.show_diagnostics;
                    }

                    let title_fill = if self.show_title_designer {
                        Color32::from_rgb(0, 120, 200)
                    } else if self.theme_mode.is_dark() {
                        Color32::from_rgb(45, 50, 60)
                    } else {
                        Color32::from_rgb(215, 222, 230)
                    };
                    let title_text_color = if self.show_title_designer || self.theme_mode.is_dark()
                    {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(30, 35, 45)
                    };
                    let title_btn =
                        egui::Button::new(RichText::new("🎨 TITULADOR").color(title_text_color))
                            .fill(title_fill);
                    if ui
                        .add(title_btn)
                        .on_hover_text("Deseñador de títulos e rótulos en directo (Lower-Thirds)")
                        .clicked()
                    {
                        self.show_title_designer = !self.show_title_designer;
                    }

                    let rec_panel_fill = if self.show_recording_panel {
                        Color32::from_rgb(180, 40, 40)
                    } else if self.theme_mode.is_dark() {
                        Color32::from_rgb(45, 50, 60)
                    } else {
                        Color32::from_rgb(215, 222, 230)
                    };
                    let rec_panel_btn = egui::Button::new(RichText::new("🔴 GRAVACIÓN").color(
                        if self.show_recording_panel || self.theme_mode.is_dark() {
                            Color32::WHITE
                        } else {
                            Color32::from_rgb(30, 35, 45)
                        },
                    ))
                    .fill(rec_panel_fill);
                    if ui
                        .add(rec_panel_btn)
                        .on_hover_text("Panel de gravación, disco e cartafol de destino")
                        .clicked()
                    {
                        self.show_recording_panel = !self.show_recording_panel;
                    }

                    if ui
                        .button(RichText::new("💾 GARDAR").small())
                        .on_hover_text("Gardar proxecto (.ovmix) [Atallo: Ctrl+S]")
                        .clicked()
                    {
                        if let Some(ref path) = self.current_project_path.clone() {
                            let _ = self.save_project_to_path(path);
                        } else if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Proxecto OPENVIDEOMIX (.ovmix)", &["ovmix", "json"])
                            .set_file_name("estudo_directo.ovmix")
                            .save_file()
                        {
                            let _ = self.save_project_to_path(path);
                        }
                    }

                    if ui
                        .button(RichText::new("📂 ABRIR").small())
                        .on_hover_text("Abrir proxecto de estudio (.ovmix)")
                        .clicked()
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Proxecto OPENVIDEOMIX (.ovmix)", &["ovmix", "json"])
                            .pick_file()
                        {
                            let _ = self.load_project_from_path(path);
                        }
                    }
                });
            });
            ui.add_space(4.0);
        });

        // BOTTOM STATUS BAR
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Refresco UI (demo): {:.1}", self.mixer.fps))
                        .strong()
                        .color(Color32::from_rgb(0, 230, 160)),
                );
                ui.separator();
                ui.label(format!("CPU (demo): {:.1}%", self.mixer.cpu_usage_pct));
                #[cfg(target_os = "linux")]
                {
                    ui.separator();
                    ui.label(format!(
                        "Nós V4L2 (Linux): {}",
                        self.connected_v4l2_devices.len()
                    ));
                }
                ui.separator();

                if let Some(msg) = &self.status_notification {
                    let notif_color = if self.theme_mode.is_dark() {
                        Color32::from_rgb(240, 200, 100)
                    } else {
                        Color32::from_rgb(180, 100, 10)
                    };
                    ui.label(RichText::new(msg).color(notif_color));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(
                            "Atallos: Espazo/Enter (CUT) | M (MIX) | W (WIPE) | F12 (FTB) | 1-9 (PVW) | F1 (Axustes) | F2 (Safe) | F3 (Audio)",
                        )
                        .weak(),
                    );
                });
            });
        });

        // CENTRAL STUDIO AREA
        egui::CentralPanel::default().show(ctx, |ui| {
            // MONITORS ROW
            ui.horizontal(|ui| {
                let available_width = ui.available_width();
                let monitor_width = (available_width - 150.0) / 2.0;
                let monitor_height = monitor_width * (9.0 / 16.0);

                // PREVIEW (PVW)
                ui.vertical(|ui| {
                    let preview_name = self
                        .mixer
                        .preview_input
                        .and_then(|id| self.mixer.inputs.iter().find(|i| i.id == id))
                        .map(|i| i.name.as_str())
                        .unwrap_or("NINGÚN");

                    let pvw_label_color = if self.theme_mode.is_dark() {
                        Color32::from_rgb(40, 220, 80)
                    } else {
                        Color32::from_rgb(0, 150, 50)
                    };
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("PREVIEW")
                                .color(pvw_label_color)
                                .strong(),
                        );
                        ui.label(format!("({})", preview_name));
                    });

                    let (rect, _response) = ui.allocate_exact_size(
                        Vec2::new(monitor_width, monitor_height),
                        Sense::hover(),
                    );
                    ui.painter()
                        .rect_filled(rect, 4.0, Color32::from_rgb(15, 18, 20));
                    self.render_monitor_content(ui, rect, self.mixer.preview_input, false);
                    ui.painter().rect_stroke(
                        rect,
                        4.0,
                        Stroke::new(2.5_f32, Color32::from_rgb(40, 200, 70)),
                        StrokeKind::Inside,
                    );
                    // Color-independent PVW badge
                    let badge_rect = Rect::from_min_size(rect.min + Vec2::new(8.0, 8.0), Vec2::new(48.0, 20.0));
                    ui.painter().rect_filled(badge_rect, 3.0, Color32::from_rgb(30, 160, 60));
                    ui.painter().text(
                        badge_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "PVW",
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                });

                // Primary transition controls (vMix style without T-bar)
                ui.vertical(|ui| {
                    ui.set_width(140.0);
                    ui.add_space(4.0);

                    let (cut_fill, cut_text) = if self.theme_mode.is_dark() {
                        (Color32::from_rgb(45, 50, 60), Color32::WHITE)
                    } else {
                        (Color32::from_rgb(215, 222, 230), Color32::from_rgb(20, 25, 35))
                    };
                    let cut_response = ui.add_sized(
                        [130.0, 34.0],
                        egui::Button::new(
                            RichText::new("CUT")
                                .strong()
                                .size(15.0)
                                .color(cut_text),
                        ).fill(cut_fill),
                    ).on_hover_text("Conmutación inmediata entre Preview e Program (Atallo: Espazo ou Enter)");
                    if cut_response.clicked() {
                        self.apply_command(ProductionCommand::Cut);
                    }

                    ui.add_space(3.0);

                    let (mix_fill, mix_text) = if self.theme_mode.is_dark() {
                        (Color32::from_rgb(40, 46, 56), Color32::from_rgb(180, 220, 255))
                    } else {
                        (Color32::from_rgb(215, 222, 230), Color32::from_rgb(20, 80, 160))
                    };
                    let mix_response = ui.add_sized(
                        [130.0, 28.0],
                        egui::Button::new(
                            RichText::new("MIX")
                                .strong()
                                .size(13.0)
                                .color(mix_text),
                        ).fill(mix_fill),
                    ).on_hover_text("Transición Fade interpolada (Atallo: M)");
                    if mix_response.clicked() {
                        self.apply_command(ProductionCommand::SetTransition {
                            transition: openvideomix_core::TransitionType::Fade,
                            duration_ms: self.transition_duration_ms,
                        });
                        self.apply_command(ProductionCommand::Cut);
                    }

                    ui.add_space(2.0);

                    let (wipe_fill, wipe_text) = if self.theme_mode.is_dark() {
                        (Color32::from_rgb(40, 46, 56), Color32::from_rgb(200, 200, 255))
                    } else {
                        (Color32::from_rgb(215, 222, 230), Color32::from_rgb(60, 40, 150))
                    };
                    let wipe_response = ui.add_sized(
                        [130.0, 28.0],
                        egui::Button::new(
                            RichText::new("WIPE")
                                .strong()
                                .size(13.0)
                                .color(wipe_text),
                        ).fill(wipe_fill),
                    ).on_hover_text("Transición Wipe direccional (Atallo: W)");
                    if wipe_response.clicked() {
                        self.apply_command(ProductionCommand::SetTransition {
                            transition: openvideomix_core::TransitionType::Wipe,
                            duration_ms: self.transition_duration_ms,
                        });
                        self.apply_command(ProductionCommand::Cut);
                    }

                    ui.add_space(3.0);

                    let ftb_text = if self.mixer.is_fade_to_black {
                        "⚫ FTB ACTIVO"
                    } else {
                        "FTB"
                    };
                    let ftb_color = if self.mixer.is_fade_to_black {
                        Color32::from_rgb(240, 140, 0)
                    } else {
                        Color32::from_rgb(80, 30, 30)
                    };
                    let ftb_response = ui.add_sized(
                        [130.0, 26.0],
                        egui::Button::new(
                            RichText::new(ftb_text)
                                .strong()
                                .size(12.0)
                                .color(Color32::WHITE),
                        ).fill(ftb_color),
                    ).on_hover_text("Fade To Black: fundido a negro e silencio master (Atallo: F12)");
                    if ftb_response.clicked() {
                        self.apply_command(ProductionCommand::ToggleFadeToBlack);
                    }

                    ui.add_space(4.0);
                    ui.separator();

                    ui.label(RichText::new("Duración:").small().weak());
                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.transition_duration_ms == 500, "0.5s").clicked() {
                            self.transition_duration_ms = 500;
                        }
                        if ui.selectable_label(self.transition_duration_ms == 1000, "1.0s").clicked() {
                            self.transition_duration_ms = 1000;
                        }
                        if ui.selectable_label(self.transition_duration_ms == 2000, "2.0s").clicked() {
                            self.transition_duration_ms = 2000;
                        }
                    });

                    ui.add_space(2.0);
                    let safe_label = if self.show_safe_areas {
                        "🛡️ Ocultar Safe"
                    } else {
                        "🛡️ Amosar Safe"
                    };
                    if ui.button(RichText::new(safe_label).small()).on_hover_text("Áreas seguras: 90% Acción, 80% Título e cruz central (Atallo: F2)").clicked() {
                        self.show_safe_areas = !self.show_safe_areas;
                    }
                });

                // PROGRAM (PGM)
                ui.vertical(|ui| {
                    let program_name = self
                        .mixer
                        .program_input
                        .and_then(|id| self.mixer.inputs.iter().find(|i| i.id == id))
                        .map(|i| i.name.as_str())
                        .unwrap_or("NINGÚN");

                    let pgm_label_color = if self.theme_mode.is_dark() {
                        Color32::from_rgb(240, 50, 50)
                    } else {
                        Color32::from_rgb(200, 25, 25)
                    };
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("PROGRAM")
                                .color(pgm_label_color)
                                .strong(),
                        );
                        ui.label(format!("({})", program_name));
                    });

                    let (rect, _response) = ui.allocate_exact_size(
                        Vec2::new(monitor_width, monitor_height),
                        Sense::hover(),
                    );
                    ui.painter()
                        .rect_filled(rect, 4.0, Color32::from_rgb(18, 12, 12));
                    self.render_monitor_content(ui, rect, self.mixer.program_input, true);
                    ui.painter().rect_stroke(
                        rect,
                        4.0,
                        Stroke::new(2.5_f32, Color32::from_rgb(230, 40, 40)),
                        StrokeKind::Inside,
                    );
                    // Color-independent PGM / FTB badge
                    let badge_rect = Rect::from_min_size(rect.min + Vec2::new(8.0, 8.0), Vec2::new(60.0, 20.0));
                    let (badge_color, badge_text) = if self.mixer.is_fade_to_black {
                        (Color32::from_rgb(220, 120, 0), "FTB")
                    } else {
                        (Color32::from_rgb(220, 30, 30), "PGM")
                    };
                    ui.painter().rect_filled(badge_rect, 3.0, badge_color);
                    ui.painter().text(
                        badge_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        badge_text,
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            #[cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        self.production.is_none(),
                        egui::Button::new("Iniciar estudo de proba · 4 fontes 1080p50"),
                    )
                    .clicked()
                {
                    self.start_test_studio();
                }
                if self.production.is_some() {
                    let sources:Vec<_>=self.mixer.inputs.iter().filter(|input|matches!(&input.kind,InputKind::Camera {device_path} if device_path.starts_with("production-test:"))).map(|input|(input.id,input.name.clone(),input.is_online)).collect();
                    for (number,(id,name,online)) in sources.into_iter().enumerate() {
                        if ui.add_enabled(online,egui::Button::new(format!("PVW T{}",number+1))).on_hover_text(name).clicked() {self.apply_command(ProductionCommand::SelectPreview {input:id});}
                    }
                }
                if let Some(production) = &self.production {
                    let view = production.view();
                    ui.label(format!(
                        "Program: {} compostos · {} prazos perdidos · {} paquetes descartados",
                        view.stats.rendered_frames,
                        view.stats.missed_deadlines,
                        view.stats.output_queue_drops
                    ));
                    ui.label(&view.audio_label);
                    if let Some(audio) = &view.audio_status {
                        ui.label(format!("Audio {:?} · {} · peak {:?} / RMS {:?} dBFS · {} perdas de cola · {} mostras de silencio · {} solapadas",
                            audio.state, if view.audio_muted {"MUTE"} else {"ganancia Master"},
                            view.audio_alignment.peak_dbfs, view.audio_alignment.rms_dbfs,
                            audio.queue_drops, view.audio_alignment.silence_samples, view.audio_alignment.overlap_samples));
                    }
                    match view.recording {
                        RecordingState::Idle => {
                            ui.label("Son para gravación: ton de proba (sen micrófono)");
                        }
                        RecordingState::Starting(path) => {
                            ui.label(format!("Abrindo {}", path.display()));
                        }
                        RecordingState::Recording {
                            submitted, gaps, ..
                        } => {
                            ui.label(format!("GRAVANDO PROGRAM: {submitted} enviados, {gaps} ocos · {} descartados na gravación", view.recording_queue_drops));
                        }
                        RecordingState::Saved(stats) => {
                            ui.label(format!(
                                "Gardado: {} · {} codificados · {} ocos",
                                stats.path.display(),
                                stats.encoded_video_frames,
                                stats.missing_video_frames
                            ));
                        }
                        RecordingState::Error(error) => {
                            ui.colored_label(Color32::YELLOW, error);
                        }
                    }
                    if let Some(error) = view.program_error {
                        ui.label(format!("Fonte non dispoñible: {error}"));
                    }
                }
            });

            // LOWER SECTION: INPUTS RACK + AUDIO MIXER
            ui.horizontal(|ui| {
                let total_available = ui.available_width();
                let inputs_width = if self.audio_mixer_collapsed {
                    total_available - 36.0
                } else {
                    (total_available - 16.0) * 0.70
                };

                // INPUTS SWITCHER RACK
                ui.vertical(|ui| {
                    ui.set_width(inputs_width);

                    // Inputs Toolbar
                    ui.horizontal(|ui| {
                        ui.heading("Entradas de Vídeo");
                        ui.label(format!("({} activas)", self.mixer.inputs.len()));

                        ui.separator();
                        ui.add(
                            egui::TextEdit::singleline(&mut self.input_search_filter)
                                .desired_width(110.0)
                                .hint_text("🔍 Filtrar..."),
                        );

                        ui.selectable_value(&mut self.input_category_filter, 0, "Todas");
                        ui.selectable_value(&mut self.input_category_filter, 1, "Cámaras");
                        ui.selectable_value(&mut self.input_category_filter, 2, "Imaxes");
                        ui.selectable_value(&mut self.input_category_filter, 3, "Sintéticos");

                        ui.separator();
                        let label_btn_color = if self.theme_mode.is_dark() {
                            Color32::from_rgb(0, 210, 255)
                        } else {
                            Color32::from_rgb(0, 110, 180)
                        };
                        if ui
                            .button(
                                RichText::new("🖼️ Engadir Rótulo")
                                    .strong()
                                    .color(label_btn_color),
                            )
                            .on_hover_text("Cargar imaxe con transparencia PNG/WebP/JPG")
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter(
                                    "Imaxes con transparencia (*.png, *.webp, *.jpg)",
                                    &["png", "webp", "jpg", "jpeg"],
                                )
                                .set_title("Seleccionar Rótulo ou Imaxe PNG")
                                .pick_file()
                            {
                                self.load_image_input(ctx, path);
                            }
                        }

                        if ui.button("📷 Cámara USB").clicked() {
                            self.add_default_usb_camera(false);
                        }
                        if ui.button("⚙ Sintético").clicked() {
                            self.add_default_usb_camera(true);
                        }
                    });

                    // Inputs Cards Scroll Area
                    let filter_text = self.input_search_filter.trim().to_lowercase();
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let mut action_cut_to = None;
                            let mut action_pvw_to = None;
                            let mut action_remove = None;
                            let mut action_reorder = None;
                            let mut action_toggle_ov = None;
                            let mut action_open_title_designer = None;
                            let mut action_media_toggle_play = None;
                            let mut action_media_toggle_loop = None;
                            let mut action_media_seek = None;

                            for (real_idx, input) in self.mixer.inputs.iter().enumerate() {
                                // Category filter
                                let cat_match = match self.input_category_filter {
                                    1 => matches!(&input.kind, InputKind::Camera { .. }),
                                    2 => matches!(
                                        &input.kind,
                                        InputKind::ImageFile { .. }
                                            | InputKind::MediaFile { .. }
                                            | InputKind::Title { .. }
                                    ),
                                    3 => matches!(
                                        &input.kind,
                                        InputKind::ColorBars | InputKind::ScreenCapture { .. }
                                    ),
                                    _ => true,
                                };
                                if !cat_match {
                                    continue;
                                }
                                if !filter_text.is_empty()
                                    && !input.name.to_lowercase().contains(&filter_text)
                                {
                                    continue;
                                }

                                let border_color = if input.is_tally_program {
                                    Color32::from_rgb(230, 40, 40)
                                } else if input.is_tally_preview {
                                    Color32::from_rgb(40, 200, 70)
                                } else if self.theme_mode.is_dark() {
                                    Color32::from_rgb(50, 55, 62)
                                } else {
                                    Color32::from_rgb(195, 205, 215)
                                };

                                ui.group(|ui| {
                                    ui.set_width(184.0);
                                    ui.vertical(|ui| {
                                        // Header with Reorder [◀][▶], Title and Close [✕]
                                        ui.horizontal(|ui| {
                                            if ui
                                                .button(RichText::new("◀").small())
                                                .on_hover_text("Mover entrada á esquerda no rack")
                                                .clicked()
                                            {
                                                action_reorder =
                                                    Some((input.id, real_idx.saturating_sub(1)));
                                            }
                                            if ui
                                                .button(RichText::new("▶").small())
                                                .on_hover_text("Mover entrada á dereita no rack")
                                                .clicked()
                                            {
                                                action_reorder = Some((input.id, real_idx + 1));
                                            }
                                            let card_title_color = if self.theme_mode.is_dark() {
                                                Color32::WHITE
                                            } else {
                                                Color32::from_rgb(20, 25, 35)
                                            };
                                            ui.label(
                                                RichText::new(format!(
                                                    "#{}: {}",
                                                    input.id, input.name
                                                ))
                                                .strong()
                                                .color(card_title_color),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    let remove_enabled =
                                                        !self.mixer.is_safety_locked;
                                                    let remove_btn = egui::Button::new(
                                                        RichText::new("✕")
                                                            .color(if remove_enabled {
                                                                Color32::from_rgb(240, 80, 80)
                                                            } else {
                                                                Color32::from_rgb(120, 120, 120)
                                                            })
                                                            .small(),
                                                    );
                                                    if ui
                                                        .add_enabled(remove_enabled, remove_btn)
                                                        .on_hover_text("Pechar esta entrada")
                                                        .on_disabled_hover_text(
                                                            "🔒 Estudo bloqueado. Desbloquea \
                                                             para pechar entradas.",
                                                        )
                                                        .clicked()
                                                    {
                                                        action_remove = Some(input.id);
                                                    }
                                                },
                                            );
                                        });

                                        // Thumbnail Box
                                        let (rect, response) = ui.allocate_exact_size(
                                            Vec2::new(176.0, 99.0),
                                            Sense::click(),
                                        );
                                        ui.painter().rect_filled(
                                            rect,
                                            3.0,
                                            Color32::from_rgb(25, 28, 34),
                                        );
                                        self.render_monitor_content(
                                            ui,
                                            rect,
                                            Some(input.id),
                                            false,
                                        );
                                        ui.painter().rect_stroke(
                                            rect,
                                            3.0,
                                            Stroke::new(2.0_f32, border_color),
                                            StrokeKind::Inside,
                                        );

                                        // Color-independent badges on thumbnail
                                        if input.is_tally_program {
                                            let badge_rect = Rect::from_min_size(
                                                rect.min + Vec2::new(4.0, 4.0),
                                                Vec2::new(42.0, 18.0),
                                            );
                                            ui.painter().rect_filled(
                                                badge_rect,
                                                2.0,
                                                Color32::from_rgb(220, 30, 30),
                                            );
                                            ui.painter().text(
                                                badge_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                "PGM",
                                                egui::FontId::proportional(11.0),
                                                Color32::WHITE,
                                            );
                                        } else if input.is_tally_preview {
                                            let badge_rect = Rect::from_min_size(
                                                rect.min + Vec2::new(4.0, 4.0),
                                                Vec2::new(42.0, 18.0),
                                            );
                                            ui.painter().rect_filled(
                                                badge_rect,
                                                2.0,
                                                Color32::from_rgb(30, 160, 60),
                                            );
                                            ui.painter().text(
                                                badge_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                "PVW",
                                                egui::FontId::proportional(11.0),
                                                Color32::WHITE,
                                            );
                                        }

                                        // Overlay badges on thumbnail (top-right)
                                        let mut ov_x_offset = rect.max.x - 4.0;
                                        for ch in (1..=4).rev() {
                                            if self.mixer.active_overlays[ch - 1] == Some(input.id) {
                                                ov_x_offset -= 34.0;
                                                let badge_rect = Rect::from_min_size(
                                                    Pos2::new(ov_x_offset, rect.min.y + 4.0),
                                                    Vec2::new(32.0, 18.0),
                                                );
                                                ui.painter().rect_filled(
                                                    badge_rect,
                                                    2.0,
                                                    Color32::from_rgb(0, 140, 200),
                                                );
                                                ui.painter().text(
                                                    badge_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    format!("OV{ch}"),
                                                    egui::FontId::proportional(10.0),
                                                    Color32::WHITE,
                                                );
                                                ov_x_offset -= 2.0;
                                            }
                                        }

                                        if !input.is_online {
                                            let off_rect = Rect::from_min_size(
                                                Pos2::new(rect.min.x + 4.0, rect.max.y - 22.0),
                                                Vec2::new(56.0, 16.0),
                                            );
                                            ui.painter().rect_filled(
                                                off_rect,
                                                2.0,
                                                Color32::from_rgb(80, 70, 20),
                                            );
                                            ui.painter().text(
                                                off_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                "OFFLINE",
                                                egui::FontId::proportional(10.0),
                                                Color32::from_rgb(255, 200, 50),
                                            );
                                        }

                                        if response.clicked() {
                                            action_pvw_to = Some(input.id);
                                        }

                                        // Audio Peak Bar Underneath
                                        let bar_rect = Rect::from_min_size(
                                            Pos2::new(rect.min.x + 4.0, rect.max.y - 6.0),
                                            Vec2::new(
                                                (rect.width() - 8.0) * input.audio_meter_peak,
                                                3.0,
                                            ),
                                        );
                                        let meter_color = if input.audio_meter_peak > 0.85 {
                                            Color32::from_rgb(235, 60, 60)
                                        } else if input.audio_meter_peak > 0.65 {
                                            Color32::from_rgb(235, 200, 40)
                                        } else {
                                            Color32::from_rgb(50, 210, 90)
                                        };
                                        ui.painter().rect_filled(bar_rect, 1.0, meter_color);

                                        // Controls: PVW, PGM, and Overlays 1-4
                                        ui.horizontal(|ui| {
                                            if ui.button(RichText::new("PVW").small()).clicked() {
                                                action_pvw_to = Some(input.id);
                                            }
                                            if ui
                                                .button(
                                                    RichText::new("PGM")
                                                        .small()
                                                        .color(Color32::from_rgb(255, 100, 100)),
                                                )
                                                .clicked()
                                            {
                                                action_cut_to = Some(input.id);
                                            }

                                            // 4 Overlay toggle buttons
                                            for channel in 1..=4 {
                                                let is_active = self.mixer.active_overlays
                                                    [channel - 1]
                                                    == Some(input.id);
                                                let ov_color = if is_active {
                                                    Color32::WHITE
                                                } else if self.theme_mode.is_dark() {
                                                    Color32::from_rgb(140, 170, 200)
                                                } else {
                                                    Color32::from_rgb(60, 80, 110)
                                                };
                                                let ov_fill = if is_active {
                                                    Color32::from_rgb(0, 120, 190)
                                                } else if self.theme_mode.is_dark() {
                                                    Color32::from_rgb(32, 36, 44)
                                                } else {
                                                    Color32::from_rgb(215, 222, 230)
                                                };
                                                let ov_btn = egui::Button::new(
                                                    RichText::new(format!("{channel}"))
                                                        .small()
                                                        .strong()
                                                        .color(ov_color),
                                                )
                                                .fill(ov_fill);
                                                if ui
                                                    .add(ov_btn)
                                                    .on_hover_text(format!(
                                                        "Conmutar Overlay {channel} para esta entrada"
                                                    ))
                                                    .clicked()
                                                {
                                                    action_toggle_ov =
                                                        Some((channel as u8, input.id));
                                                }
                                            }

                                            if matches!(input.kind, InputKind::Title { .. })
                                                && ui
                                                    .button(RichText::new("✏️").small())
                                                    .on_hover_text(
                                                        "Editar este título no Deseñador",
                                                    )
                                                    .clicked()
                                            {
                                                if let InputKind::Title { template } = &input.kind {
                                                    action_open_title_designer =
                                                        Some((input.id, template.clone()));
                                                }
                                            }
                                        });

                                        // Media Clip Playback Transport
                                        if matches!(input.kind, InputKind::MediaFile { .. }) {
                                            if let Some(player) =
                                                self.mixer.media_players.get(&input.id)
                                            {
                                                let is_playing = player.is_playing;
                                                let is_looping = player.is_looping;
                                                let pos_s = player.position_ms / 1000;
                                                let dur_s = player.duration_ms / 1000;
                                                let pos_str = format!(
                                                    "{:02}:{:02}/{:02}:{:02}",
                                                    pos_s / 60,
                                                    pos_s % 60,
                                                    dur_s / 60,
                                                    dur_s % 60
                                                );

                                                ui.horizontal(|ui| {
                                                    let play_text =
                                                        if is_playing { "⏸" } else { "▶" };
                                                    if ui
                                                        .button(RichText::new(play_text).small())
                                                        .on_hover_text("Reproducir / Pausar clip")
                                                        .clicked()
                                                    {
                                                        action_media_toggle_play = Some(input.id);
                                                    }
                                                    let loop_col = if is_looping {
                                                        Color32::from_rgb(0, 200, 180)
                                                    } else {
                                                        Color32::GRAY
                                                    };
                                                    if ui
                                                        .button(
                                                            RichText::new("🔁")
                                                                .small()
                                                                .color(loop_col),
                                                        )
                                                        .on_hover_text("Conmutar bucle de vídeo")
                                                        .clicked()
                                                    {
                                                        action_media_toggle_loop = Some(input.id);
                                                    }
                                                    ui.label(RichText::new(pos_str).small().weak());
                                                });

                                                let mut progress = if player.duration_ms > 0 {
                                                    player.position_ms as f32
                                                        / player.duration_ms as f32
                                                } else {
                                                    0.0
                                                };
                                                if ui
                                                    .add(
                                                        egui::Slider::new(&mut progress, 0.0..=1.0)
                                                            .show_value(false),
                                                    )
                                                    .changed()
                                                {
                                                    let target_ms =
                                                        (progress * player.duration_ms as f32)
                                                            as u64;
                                                    action_media_seek =
                                                        Some((input.id, target_ms));
                                                }
                                            }
                                        }
                                    });
                                });
                                ui.add_space(4.0);
                            }

                            if let Some(id) = action_cut_to {
                                self.apply_command(ProductionCommand::SelectProgram { input: id });
                            }
                            if let Some(id) = action_pvw_to {
                                self.apply_command(ProductionCommand::SelectPreview { input: id });
                            }
                            if let Some(id) = action_remove {
                                if !self.mixer.is_safety_locked {
                                    #[cfg(feature = "gstreamer-capture")]
                                    self.capture_sessions.remove(&id);
                                    self.loaded_textures.remove(&id);
                                    self.mixer.remove_input(id);
                                }
                            }
                            if let Some((id, target)) = action_reorder {
                                self.apply_command(ProductionCommand::ReorderInput {
                                    input: id,
                                    target_index: target,
                                });
                            }
                            if let Some((ch, id)) = action_toggle_ov {
                                self.apply_command(ProductionCommand::ToggleOverlay {
                                    channel: ch,
                                    input: id,
                                });
                            }
                            if let Some((id, template)) = action_open_title_designer {
                                self.selected_title_input = Some(id);
                                self.editing_title = template;
                                self.show_title_designer = true;
                            }
                            if let Some(id) = action_media_toggle_play {
                                self.apply_command(ProductionCommand::MediaTogglePlay {
                                    input: id,
                                });
                            }
                            if let Some(id) = action_media_toggle_loop {
                                self.apply_command(ProductionCommand::MediaToggleLoop {
                                    input: id,
                                });
                            }
                            if let Some((id, pos)) = action_media_seek {
                                self.apply_command(ProductionCommand::MediaSeek {
                                    input: id,
                                    position_ms: pos,
                                });
                            }
                        });
                    });
                });

                ui.separator();

                // DOCKABLE AUDIO MIXER (Right section)
                if self.audio_mixer_collapsed {
                    ui.vertical(|ui| {
                        if ui
                            .button(RichText::new("🔊").size(18.0))
                            .on_hover_text("Despregar mesturador de son (Atallo: F3)")
                            .clicked()
                        {
                            self.audio_mixer_collapsed = false;
                        }
                    });
                } else {
                    ui.vertical(|ui| {
                        let mut action_set_vol = None;
                        let mut action_set_mute = None;
                        let mut action_set_afv = None;
                        let mut action_set_delay = None;

                        ui.horizontal(|ui| {
                            ui.heading("Mesturador de Son");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .button(RichText::new("✕").small())
                                        .on_hover_text("Repregar panel de audio (Atallo: F3)")
                                        .clicked()
                                    {
                                        self.audio_mixer_collapsed = true;
                                    }
                                },
                            );
                        });

                        egui::ScrollArea::horizontal().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Channel strips for inputs
                                for input in &self.mixer.inputs {
                                    ui.group(|ui| {
                                        ui.set_width(54.0);
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(format!("#{}", input.id))
                                                    .strong()
                                                    .small(),
                                            );
                                            let mut vol = input.volume;
                                            if ui
                                                .add(
                                                    egui::Slider::new(&mut vol, 0.0..=1.0)
                                                        .vertical()
                                                        .show_value(false),
                                                )
                                                .on_hover_text(format!(
                                                    "Volume: {:.0}%",
                                                    vol * 100.0
                                                ))
                                                .changed()
                                            {
                                                action_set_vol = Some((input.id, vol));
                                            }
                                            let mute_label =
                                                if input.is_muted { "MUTE" } else { "M" };
                                            let mute_color = if input.is_muted {
                                                Color32::from_rgb(255, 80, 80)
                                            } else {
                                                Color32::GRAY
                                            };
                                            if ui
                                                .button(
                                                    RichText::new(mute_label)
                                                        .small()
                                                        .color(mute_color),
                                                )
                                                .clicked()
                                            {
                                                action_set_mute =
                                                    Some((input.id, !input.is_muted));
                                            }
                                            let afv_color = if input.afv {
                                                Color32::from_rgb(0, 220, 180)
                                            } else {
                                                Color32::GRAY
                                            };
                                            if ui
                                                .button(
                                                    RichText::new("AFV").small().color(afv_color),
                                                )
                                                .on_hover_text("Audio-Follows-Video")
                                                .clicked()
                                            {
                                                action_set_afv = Some((input.id, !input.afv));
                                            }

                                            let mut delay = input.audio_delay_ms;
                                            if ui
                                                .add(
                                                    egui::DragValue::new(&mut delay)
                                                        .range(0..=1000)
                                                        .suffix("ms")
                                                        .speed(5.0),
                                                )
                                                .on_hover_text("Retardo de son (Lip Sync en ms)")
                                                .changed()
                                            {
                                                action_set_delay = Some((Some(input.id), delay));
                                            }
                                        });
                                    });
                                }

                                ui.separator();

                                // Master Bus
                                ui.group(|ui| {
                                    ui.set_width(62.0);
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new("MASTER").strong().small());
                                        let mut vol = self.mixer.master_bus.volume;
                                        if ui
                                            .add(
                                                egui::Slider::new(&mut vol, 0.0..=1.0)
                                                    .vertical()
                                                    .show_value(false),
                                            )
                                            .on_hover_text(format!(
                                                "Master: {:.0}%",
                                                vol * 100.0
                                            ))
                                            .changed()
                                        {
                                            self.apply_command(
                                                ProductionCommand::SetMasterVolume { volume: vol },
                                            );
                                        }
                                        let mute_label =
                                            if self.mixer.master_bus.muted {
                                                "MUTE"
                                            } else {
                                                "M"
                                            };
                                        let mute_color = if self.mixer.master_bus.muted {
                                            Color32::from_rgb(255, 80, 80)
                                        } else {
                                            Color32::GRAY
                                        };
                                        if ui
                                            .button(
                                                RichText::new(mute_label).small().color(mute_color),
                                            )
                                            .clicked()
                                        {
                                            self.apply_command(ProductionCommand::SetMasterMute {
                                                muted: !self.mixer.master_bus.muted,
                                            });
                                        }

                                        let mut master_delay = self.mixer.master_bus.delay_ms;
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut master_delay)
                                                    .range(0..=1000)
                                                    .suffix("ms")
                                                    .speed(5.0),
                                            )
                                            .on_hover_text(
                                                "Retardo Master de saída (Lip Sync en ms)",
                                            )
                                            .changed()
                                        {
                                            action_set_delay = Some((None, master_delay));
                                        }
                                    });
                                });

                                // Bus A
                                ui.group(|ui| {
                                    ui.set_width(54.0);
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new("BUS A").small());
                                        let mut vol = self.mixer.bus_a.volume;
                                        if ui
                                            .add(
                                                egui::Slider::new(&mut vol, 0.0..=1.0)
                                                    .vertical()
                                                    .show_value(false),
                                            )
                                            .changed()
                                        {
                                            self.mixer.bus_a.volume = vol;
                                        }
                                    });
                                });

                                // Headphones
                                ui.group(|ui| {
                                    ui.set_width(54.0);
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new("AURIC.").small());
                                        let mut vol = self.mixer.headphones_bus.volume;
                                        if ui
                                            .add(
                                                egui::Slider::new(&mut vol, 0.0..=1.0)
                                                    .vertical()
                                                    .show_value(false),
                                            )
                                            .changed()
                                        {
                                            self.mixer.headphones_bus.volume = vol;
                                        }
                                    });
                                });
                            });
                        });

                        if let Some((id, vol)) = action_set_vol {
                            self.apply_command(ProductionCommand::SetInputVolume {
                                input: id,
                                volume: vol,
                            });
                        }
                        if let Some((id, muted)) = action_set_mute {
                            self.apply_command(ProductionCommand::SetInputMute {
                                input: id,
                                muted,
                            });
                        }
                        if let Some((id, enabled)) = action_set_afv {
                            self.apply_command(ProductionCommand::SetInputAfv {
                                input: id,
                                enabled,
                            });
                        }
                        if let Some((target, delay)) = action_set_delay {
                            self.apply_command(ProductionCommand::SetAudioDelay {
                                input: target,
                                delay_ms: delay,
                            });
                        }
                    });
                }
            });
        });

        // SETTINGS MODAL WINDOW (WHEN CLICKING AXUSTES OR F1)
        if self.show_settings {
            egui::Window::new("⚙ Axustes de OPENVIDEOMIX")
                .collapsible(false)
                .resizable(true)
                .default_size([580.0, 420.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.settings_tab, 0, "🎥 Vídeo");
                        ui.selectable_value(&mut self.settings_tab, 1, "🔊 Son");
                        ui.selectable_value(&mut self.settings_tab, 2, "🔌 Captura");
                        #[cfg(feature = "midi-input")]
                        ui.selectable_value(&mut self.settings_tab, 3, "MIDI");
                        ui.selectable_value(&mut self.settings_tab, 4, "🎨 Aspecto e Tema");
                    });
                    ui.separator();

                    match self.settings_tab {
                        0 => {
                            ui.heading("Configuración de Vídeo Profesional");
                            ui.horizontal(|ui| {
                                ui.label("Resolución de Emisión:");
                                egui::ComboBox::from_id_salt("res_combo")
                                    .selected_text(&self.mixer.settings.output_resolution)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.mixer.settings.output_resolution, "1920x1080 (Full HD)".into(), "1920x1080 (Full HD)");
                                        ui.selectable_value(&mut self.mixer.settings.output_resolution, "1280x720 (HD)".into(), "1280x720 (HD)");
                                        ui.selectable_value(&mut self.mixer.settings.output_resolution, "3840x2160 (4K UHD)".into(), "3840x2160 (4K UHD)");
                                    });
                            });
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Perfil de cadencia previsto:").strong());
                                egui::ComboBox::from_id_salt("fps_combo")
                                    .selected_text(&self.mixer.settings.frame_rate)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.mixer.settings.frame_rate, "50.00 fps (PAL Broadcast 50Hz)".into(), "50.00 fps (PAL Broadcast 50Hz)");
                                        ui.selectable_value(&mut self.mixer.settings.frame_rate, "59.94 fps (NTSC Broadcast)".into(), "59.94 fps (NTSC Broadcast)");
                                        ui.selectable_value(&mut self.mixer.settings.frame_rate, "60.00 fps".into(), "60.00 fps");
                                        ui.selectable_value(&mut self.mixer.settings.frame_rate, "25.00 fps (PAL Cinema)".into(), "25.00 fps (PAL Cinema)");
                                        ui.selectable_value(&mut self.mixer.settings.frame_rate, "30.00 fps".into(), "30.00 fps");
                                    });
                            });
                            ui.add_space(4.0);
                            ui.label(RichText::new("ℹ️ O perfil 50p fixa o obxectivo de saída. A GUI actual só axusta a frecuencia de repintado; aínda non existe un motor de saída 1080p50 validado.").weak());
                        }
                        1 => {
                            ui.heading("Configuración de Son");
                            ui.label(format!("Dispositivo de entrada: {}", self.mixer.settings.audio_device));
                            ui.label(format!("Frecuencia de mostraxe: {} Hz", self.mixer.settings.audio_sample_rate));
                            let mut muted = self.mixer.master_bus.muted;
                            if ui.checkbox(&mut muted, "Silenciar Master (Mute)").changed() {
                                self.apply_command(ProductionCommand::SetMasterMute { muted });
                            }
                        }
                        2 => {
                            ui.heading("Dispositivos de Captura Detectados");
                            #[cfg(target_os = "linux")]
                            {
                                if ui.button("🔄 Refrescar Dispositivos").clicked() {
                                    self.refresh_video_devices();
                                }
                                if self.connected_v4l2_devices.is_empty() {
                                    ui.label("Non se detectaron nodos V4L2 en /dev/video*");
                                } else {
                                    for dev in &self.connected_v4l2_devices {
                                        ui.label(format!(
                                            "Nodo V4L2 dispoñible: {dev} (captura non iniciada)"
                                        ));
                                    }
                                }
                            }
                            #[cfg(not(target_os = "linux"))]
                            ui.label(
                                "As cámaras iniciais son demostracións, non dispositivos físicos detectados.",
                            );
                            #[cfg(not(feature = "gstreamer-capture"))]
                            ui.label(
                                "Detección real de cámaras non activada nesta compilación.",
                            );
                            #[cfg(feature = "gstreamer-capture")]
                            {
                                ui.separator();
                                ui.label("Cámaras GStreamer (Windows, Linux e macOS)");
                                if ui.button("Explorar cámaras GStreamer").clicked() {
                                    self.begin_camera_discovery();
                                }
                                if self.camera_discovery_rx.is_some() {
                                    ui.spinner();
                                    ui.label("Explorando dispositivos en segundo fío...");
                                } else if self.discovered_cameras.is_empty() {
                                    ui.label("Non hai cámaras listadas ou o provedor de GStreamer non está dispoñible.");
                                } else {
                                    for camera in &self.discovered_cameras {
                                        ui.collapsing(
                                            format!(
                                                "{} ({})",
                                                camera.display_name, camera.device_class
                                            ),
                                            |ui| {
                                                ui.label("Capacidades anunciadas:");
                                                ui.monospace(camera.capabilities.as_str());
                                            },
                                        );
                                    }
                                }
                                ui.label(
                                    "Esta lista só identifica dispositivos. A selección e a captura por ID físico están pendentes.",
                                );
                            }
                        }
                        #[cfg(feature = "midi-input")]
                        3 => {
                            ui.heading("Superficie de control MIDI");
                            ui.label("MIDI Learn: escolle unha función e move un botón ou fader.");
                            if ui.button("Actualizar portos MIDI").clicked() {
                                self.refresh_midi_ports();
                            }
                            egui::ComboBox::from_id_salt("midi_port")
                                .selected_text(
                                    self.midi_ports
                                        .get(self.midi_port_selected)
                                        .map(String::as_str)
                                        .unwrap_or("Ningún porto MIDI"),
                                )
                                .show_ui(ui, |ui| {
                                    for (index, port) in self.midi_ports.iter().enumerate() {
                                        ui.selectable_value(
                                            &mut self.midi_port_selected,
                                            index,
                                            port,
                                        );
                                    }
                                });
                            ui.horizontal(|ui| {
                                if ui.button("Conectar").clicked() {
                                    match MidiInputService::connect(self.midi_port_selected) {
                                        Ok(session) => {
                                            let name = session.port_name.clone();
                                            self.midi_session = Some(session);
                                            self.status_notification =
                                                Some(format!("MIDI conectado: {name}"));
                                        }
                                        Err(message) => self.status_notification = Some(message),
                                    }
                                }
                                if ui.button("Desconectar").clicked() {
                                    self.midi_session = None;
                                    self.status_notification =
                                        Some("MIDI desconectado".into());
                                }
                            });
                            if let Some(session) = &self.midi_session {
                                ui.label(format!("Porto activo: {}", session.port_name));
                            }
                            ui.separator();
                            ui.horizontal(|ui| {
                                if ui.button("Aprender CUT").clicked() {
                                    self.midi_learn = Some(MidiTarget::Cut);
                                }
                                if ui.button("Aprender volume Master").clicked() {
                                    self.midi_learn = Some(MidiTarget::MasterVolume);
                                }
                            });
                            ui.label("Asignar botóns de cámara a Preview:");
                            egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
                                for input in &self.mixer.inputs {
                                    if ui.button(format!("PVW #{} {}", input.id, input.name)).clicked() {
                                        self.midi_learn = Some(MidiTarget::SelectPreview(input.id));
                                    }
                                }
                            });
                            ui.label(format!("Asignacións MIDI nesta sesión: {}", self.midi_mapper.bindings.len()));
                            ui.horizontal(|ui| {
                                if ui.button("Gardar perfil MIDI...").clicked() {
                                    self.save_midi_profile();
                                }
                                if ui.button("Cargar perfil MIDI...").clicked() {
                                    self.load_midi_profile();
                                }
                            });
                            if self.midi_learn.is_some() {
                                ui.label(RichText::new("Aprendizaxe activa: move un control MIDI").strong());
                                if ui.button("Cancelar MIDI Learn").clicked() {
                                    self.midi_learn = None;
                                }
                            }
                        }
                        4 => {
                            ui.heading("🎨 Aspecto, Tema e Tipografía");
                            ui.add_space(8.0);

                            ui.group(|ui| {
                                ui.label(RichText::new("Tema Visual do Estudo").strong());
                                ui.horizontal(|ui| {
                                    ui.selectable_value(
                                        &mut self.theme_mode,
                                        ThemeMode::Dark,
                                        ThemeMode::Dark.label(),
                                    );
                                    ui.selectable_value(
                                        &mut self.theme_mode,
                                        ThemeMode::Light,
                                        ThemeMode::Light.label(),
                                    );
                                });
                                ui.add_space(4.0);
                                let theme_desc = match self.theme_mode {
                                    ThemeMode::Dark => {
                                        "Modo escuro broadcast: fondo escuro neutro (#121418), deseñado para estudios de realización, baixa iluminación e diminución da fatiga ocular."
                                    }
                                    ThemeMode::Light => {
                                        "Modo claro broadcast: fondo suave e contrastado (#F0F3F6), deseñado para contornas con iluminación natural, exteriores e oficinas."
                                    }
                                };
                                ui.label(RichText::new(theme_desc).weak());
                            });

                            ui.add_space(8.0);

                            ui.group(|ui| {
                                ui.label(RichText::new("Escala da Interface e Tipografía (DPI / Zoom)").strong());
                                ui.horizontal(|ui| {
                                    ui.label("Escala global:");
                                    egui::ComboBox::from_id_salt("ui_scale_selector")
                                        .selected_text(format!("{}%", (self.ui_scale * 100.0).round() as i32))
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut self.ui_scale, 0.9, "90% (Compacto)");
                                            ui.selectable_value(&mut self.ui_scale, 1.0, "100% (Estándar)");
                                            ui.selectable_value(&mut self.ui_scale, 1.1, "110% (Medio)");
                                            ui.selectable_value(&mut self.ui_scale, 1.25, "125% (Grande - 2K/4K)");
                                            ui.selectable_value(&mut self.ui_scale, 1.5, "150% (Moi grande)");
                                        });
                                    if ui.button("Restablecer 100%").clicked() {
                                        self.ui_scale = 1.0;
                                    }
                                });
                                ui.label(
                                    RichText::new("Axusta a escala visual en monitores 4K ou pantallas de alta densidade.").weak(),
                                );
                            });

                            ui.add_space(8.0);

                            ui.group(|ui| {
                                ui.label(RichText::new("Soporte Unicode e Caracteres Especiais").strong());
                                ui.label(
                                    RichText::new(
                                        "OPENVIDEOMIX soporta nativamente a codificación UTF-8 para nomes de entrada, rótulos, subtítulos e grafismos. Non hai caracteres 'tofu' nin caixas baleiras con tiles ou eñes:",
                                    )
                                    .weak(),
                                );
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label("Cadro de proba interactiva:");
                                    ui.add(egui::TextEdit::singleline(&mut self.unicode_test_text).desired_width(320.0));
                                });
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label("Caracteres galegos e especiais:");
                                    ui.monospace("ñ Ñ á é í ó ú à è ì ò ù ç Ç ü Ü ¿ ¡ · « »");
                                });
                                ui.add_space(4.0);
                                let status_color = if self.theme_mode.is_dark() {
                                    Color32::from_rgb(0, 220, 140)
                                } else {
                                    Color32::from_rgb(0, 150, 80)
                                };
                                ui.label(
                                    RichText::new("✅ Motor de tipografía activo: compatibilidade total con tiles, eñes e diacríticos.")
                                        .color(status_color)
                                        .strong(),
                                );
                            });
                        }
                        _ => {}
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Pechar").clicked() {
                            self.show_settings = false;
                        }
                    });
                });
        }

        if self.show_diagnostics {
            egui::Window::new("📋 Diagnóstico do Sistema (Sanitizado)")
                .collapsible(false)
                .resizable(true)
                .default_size([640.0, 480.0])
                .show(ctx, |ui| {
                    ui.label(
                        "Este informe está redactado e NON contén datos persoais, \
                         contrasinais nin rutas de usuario.",
                    );
                    let diag = self.generate_diagnostics();
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .max_height(340.0)
                        .show(ui, |ui| {
                            ui.monospace(diag.to_markdown_report());
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui
                            .button("💾 Gardar Markdown...")
                            .on_hover_text("Gardar informe como ficheiro .md")
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name("openvideomix-diagnostics.md")
                                .add_filter("Markdown", &["md"])
                                .save_file()
                            {
                                let _ = std::fs::write(path, diag.to_markdown_report());
                            }
                        }
                        if ui
                            .button("💾 Gardar JSON...")
                            .on_hover_text("Gardar datos en bruto como ficheiro .json")
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name("openvideomix-diagnostics.json")
                                .add_filter("JSON", &["json"])
                                .save_file()
                            {
                                if let Ok(json) = diag.to_json() {
                                    let _ = std::fs::write(path, json);
                                }
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Pechar").clicked() {
                                self.show_diagnostics = false;
                            }
                        });
                    });
                });
        }

        // Live Title Designer & Lower-Third Editor
        if self.show_title_designer {
            egui::Window::new("🎨 Deseñador de Títulos e Grafismos (Lower-Thirds)")
                .collapsible(false)
                .resizable(true)
                .default_size([620.0, 540.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Lower-Thirds e Rótulos");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✕ Pechar").clicked() {
                                self.show_title_designer = false;
                            }
                        });
                    });
                    ui.separator();

                    // Input selector
                    let title_inputs: Vec<(InputId, String)> = self
                        .mixer
                        .inputs
                        .iter()
                        .filter_map(|i| {
                            if matches!(i.kind, InputKind::Title { .. }) {
                                Some((i.id, i.name.clone()))
                            } else {
                                None
                            }
                        })
                        .collect();

                    if title_inputs.is_empty() {
                        ui.label("Non hai entradas de tipo Título configuradas no estudo.");
                        if ui.button("➕ Crear Nova Entrada de Título").clicked() {
                            let next_id =
                                self.mixer.inputs.iter().map(|i| i.id).max().unwrap_or(0) + 1;
                            let title_input = VideoInput::new(
                                next_id,
                                format!("GRAFISMO #{} - Rótulo", next_id),
                                InputKind::Title {
                                    template: TitleData::default(),
                                },
                            );
                            self.mixer.inputs.push(title_input);
                            self.selected_title_input = Some(next_id);
                        }
                        return;
                    }

                    let current_selected = self.selected_title_input.unwrap_or(title_inputs[0].0);
                    let mut selected_id = current_selected;

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Entrada de Título a editar:").strong());
                        egui::ComboBox::from_id_salt("title_input_select")
                            .selected_text(
                                title_inputs
                                    .iter()
                                    .find(|(id, _)| *id == selected_id)
                                    .map(|(_, name)| name.as_str())
                                    .unwrap_or("Seleccionar entrada"),
                            )
                            .show_ui(ui, |ui| {
                                for (id, name) in &title_inputs {
                                    ui.selectable_value(&mut selected_id, *id, name);
                                }
                            });

                        if ui
                            .button("➕ Novo Título")
                            .on_hover_text("Engadir nova capa de grafismo ao rack")
                            .clicked()
                        {
                            let next_id =
                                self.mixer.inputs.iter().map(|i| i.id).max().unwrap_or(0) + 1;
                            let title_input = VideoInput::new(
                                next_id,
                                format!("GRAFISMO #{} - Rótulo", next_id),
                                InputKind::Title {
                                    template: TitleData::default(),
                                },
                            );
                            self.mixer.inputs.push(title_input);
                            selected_id = next_id;
                        }
                    });

                    // Sync current selected title data into editing_title if changed
                    if self.selected_title_input != Some(selected_id) {
                        self.selected_title_input = Some(selected_id);
                        if let Some(inp) = self.mixer.inputs.iter().find(|i| i.id == selected_id) {
                            if let InputKind::Title { template } = &inp.kind {
                                self.editing_title = template.clone();
                            }
                        }
                    }

                    ui.add_space(6.0);

                    // Quick Model Presets
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Modelos rápidos:").strong());
                        if ui.button("📘 Portavoz / Convidado").clicked() {
                            let cur_h = self.editing_title.headline.clone();
                            let cur_s = self.editing_title.subtitle.clone();
                            self.editing_title = TitleData::preset_speaker(cur_h, cur_s);
                            self.apply_command(ProductionCommand::UpdateTitle {
                                input: selected_id,
                                template: self.editing_title.clone(),
                            });
                        }
                        if ui.button("🔴 Última Hora").clicked() {
                            let cur_h = self.editing_title.headline.clone();
                            let cur_s = self.editing_title.subtitle.clone();
                            self.editing_title = TitleData::preset_breaking_news(cur_h, cur_s);
                            self.apply_command(ProductionCommand::UpdateTitle {
                                input: selected_id,
                                template: self.editing_title.clone(),
                            });
                        }
                        if ui.button("✨ Mínimo").clicked() {
                            let cur_h = self.editing_title.headline.clone();
                            let cur_s = self.editing_title.subtitle.clone();
                            self.editing_title = TitleData::preset_minimal(cur_h, cur_s);
                            self.apply_command(ProductionCommand::UpdateTitle {
                                input: selected_id,
                                template: self.editing_title.clone(),
                            });
                        }
                    });

                    ui.add_space(8.0);
                    ui.separator();

                    let mut changed = false;

                    // Text Content Section
                    ui.group(|ui| {
                        ui.heading("📝 Contido do Rótulo");
                        ui.horizontal(|ui| {
                            ui.label("Titular principal:");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.editing_title.headline)
                                        .desired_width(340.0),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Subtítulo / Cargo:");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.editing_title.subtitle)
                                        .desired_width(340.0),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                        });
                    });

                    ui.add_space(6.0);

                    // Spatial Positioning & Geometry
                    ui.group(|ui| {
                        ui.heading("📐 Posicionamento e Escala");
                        ui.horizontal(|ui| {
                            ui.label("Posición X:");
                            if ui
                                .add(
                                    egui::Slider::new(
                                        &mut self.editing_title.x_percent,
                                        0.0..=90.0,
                                    )
                                    .suffix("%"),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                            ui.label("Posición Y:");
                            if ui
                                .add(
                                    egui::Slider::new(
                                        &mut self.editing_title.y_percent,
                                        0.0..=90.0,
                                    )
                                    .suffix("%"),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Escala:");
                            if ui
                                .add(
                                    egui::Slider::new(&mut self.editing_title.scale, 0.5..=2.5)
                                        .step_by(0.05),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                            ui.label("Recheo:");
                            if ui
                                .add(egui::Slider::new(
                                    &mut self.editing_title.bg_padding,
                                    4.0..=30.0,
                                ))
                                .changed()
                            {
                                changed = true;
                            }
                            ui.label("Redondeo:");
                            if ui
                                .add(egui::Slider::new(
                                    &mut self.editing_title.corner_radius,
                                    0.0..=20.0,
                                ))
                                .changed()
                            {
                                changed = true;
                            }
                        });
                    });

                    ui.add_space(6.0);

                    // Typography & Styling
                    ui.group(|ui| {
                        ui.heading("🎨 Tipografía e Estilo");
                        ui.horizontal(|ui| {
                            ui.label("Tamaño Titular:");
                            if ui
                                .add(egui::Slider::new(
                                    &mut self.editing_title.font_size_headline,
                                    12.0..=48.0,
                                ))
                                .changed()
                            {
                                changed = true;
                            }
                            ui.label("Tamaño Subtítulo:");
                            if ui
                                .add(egui::Slider::new(
                                    &mut self.editing_title.font_size_subtitle,
                                    10.0..=32.0,
                                ))
                                .changed()
                            {
                                changed = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            if ui
                                .checkbox(
                                    &mut self.editing_title.has_drop_shadow,
                                    "Sombra proxectada (Drop Shadow)",
                                )
                                .changed()
                            {
                                changed = true;
                            }
                            if self.editing_title.has_drop_shadow {
                                ui.label("Desprazamento X:");
                                if ui
                                    .add(egui::Slider::new(
                                        &mut self.editing_title.shadow_offset[0],
                                        -10.0..=10.0,
                                    ))
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.label("Y:");
                                if ui
                                    .add(egui::Slider::new(
                                        &mut self.editing_title.shadow_offset[1],
                                        -10.0..=10.0,
                                    ))
                                    .changed()
                                {
                                    changed = true;
                                }
                            }
                        });
                    });

                    // Live apply
                    if changed {
                        self.editing_title.sanitize();
                        self.apply_command(ProductionCommand::UpdateTitle {
                            input: selected_id,
                            template: self.editing_title.clone(),
                        });
                    }

                    ui.add_space(8.0);
                    ui.separator();

                    // Overlay 1-4 Quick Control & Save/Load
                    ui.horizontal(|ui| {
                        let is_ov1 = self.mixer.active_overlays[0] == Some(selected_id);
                        let ov1_text = if is_ov1 {
                            "🔴 Quitar de Overlay 1"
                        } else {
                            "🟢 Activar en Overlay 1"
                        };
                        if ui.button(RichText::new(ov1_text).strong()).clicked() {
                            self.apply_command(ProductionCommand::ToggleOverlay {
                                channel: 1,
                                input: selected_id,
                            });
                        }

                        ui.separator();

                        if ui.button("💾 Gardar Modelo JSON...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name("modelo-rotulo.ovtitle.json")
                                .add_filter("OPENVIDEOMIX Title", &["ovtitle.json", "json"])
                                .save_file()
                            {
                                if let Ok(json) = serde_json::to_string_pretty(&self.editing_title)
                                {
                                    let _ = std::fs::write(path, json);
                                }
                            }
                        }

                        if ui.button("📂 Cargar Modelo JSON...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("OPENVIDEOMIX Title", &["ovtitle.json", "json"])
                                .pick_file()
                            {
                                if let Ok(data) = std::fs::read_to_string(path) {
                                    if let Ok(loaded) = serde_json::from_str::<TitleData>(&data) {
                                        self.editing_title = loaded;
                                        self.apply_command(ProductionCommand::UpdateTitle {
                                            input: selected_id,
                                            template: self.editing_title.clone(),
                                        });
                                    }
                                }
                            }
                        }
                    });
                });
        }

        // Broadcast Safety Modal: Confirm Stop Recording
        if self.show_stop_recording_confirm {
            egui::Window::new("⚠️ Confirmar Parada de Gravación")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .default_width(440.0)
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("⚠️")
                                .size(32.0)
                                .color(Color32::from_rgb(255, 170, 0)),
                        );
                        ui.vertical(|ui| {
                            ui.heading("Seguro que queres deter a gravación de Program?");
                            ui.label("A gravación actual finalizará e o ficheiro quedará gardado.");
                        });
                    });
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                RichText::new("⏹ Confirmar e Deter")
                                    .color(Color32::from_rgb(255, 90, 90))
                                    .strong(),
                            )
                            .clicked()
                        {
                            self.apply_command(ProductionCommand::StopRecording);
                            self.show_stop_recording_confirm = false;
                        }
                        if ui.button("Cancelar (Continuar Gravando)").clicked() {
                            self.show_stop_recording_confirm = false;
                        }
                    });
                    ui.add_space(4.0);
                });
        }

        // Broadcast Safety Modal: Alert on Window Close during Active Recording/Streaming
        if self.show_close_warning_modal {
            egui::Window::new("🛑 Gravación ou Emisión en Curso")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .default_width(480.0)
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("🛑")
                                .size(36.0)
                                .color(Color32::from_rgb(255, 60, 60)),
                        );
                        ui.vertical(|ui| {
                            ui.heading("Hai unha gravación ou emisión activa!");
                            ui.label(
                                "Debes deter a gravación/emisión antes de saír de \
                                 OPENVIDEOMIX para evitar a perda ou corrupción dos ficheiros.",
                            );
                        });
                    });
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                RichText::new("⏹ Deter Gravación e Pechar")
                                    .color(Color32::from_rgb(255, 90, 90))
                                    .strong(),
                            )
                            .clicked()
                        {
                            self.apply_command(ProductionCommand::StopRecording);
                            self.force_close_confirmed = true;
                            self.show_close_warning_modal = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("Cancelar (Seguir Emitindo)").clicked() {
                            self.show_close_warning_modal = false;
                        }
                    });
                    ui.add_space(4.0);
                });
        }

        // ADVANCED RECORDING & DISK PANEL WINDOW
        if self.show_recording_panel {
            egui::Window::new("🔴 Panel de Gravación e Disco")
                .collapsible(false)
                .resizable(true)
                .default_size([540.0, 420.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Configuración de Gravación de Program");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✕ Pechar").clicked() {
                                self.show_recording_panel = false;
                            }
                        });
                    });
                    ui.separator();
                    ui.label(
                        "Grava a saída de Program a 1080p50 continuo sen perda de fotogramas.",
                    );
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Cartafol de destino:").strong());
                        ui.label(self.recording_dir.to_string_lossy().to_string());
                        if ui.button("📁 Examinar...").clicked() {
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                self.recording_dir = folder;
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Prefixo do ficheiro:").strong());
                        ui.text_edit_singleline(&mut self.recording_filename_prefix);
                    });

                    let target_filename = format!(
                        "{}_{}.webm",
                        self.recording_filename_prefix,
                        chrono::Local::now().format("%Y%m%d_%H%M%S")
                    );
                    ui.label(
                        RichText::new(format!("Ficheiro seguinte: {target_filename}"))
                            .weak()
                            .small(),
                    );

                    ui.add_space(8.0);
                    ui.group(|ui| {
                        ui.heading("📊 Estatísticas e Espazo en Disco");
                        let is_rec = self.is_recording_or_streaming();
                        let status_text = if is_rec {
                            let h = self.recording_elapsed_secs / 3600;
                            let m = (self.recording_elapsed_secs % 3600) / 60;
                            let s = self.recording_elapsed_secs % 60;
                            format!("🔴 GRAVANDO ({h:02}:{m:02}:{s:02})")
                        } else {
                            "⚪ DETIDO (Listo para gravar)".to_string()
                        };
                        let status_color = if is_rec {
                            Color32::from_rgb(255, 60, 60)
                        } else {
                            Color32::from_rgb(100, 220, 120)
                        };
                        ui.label(RichText::new(status_text).color(status_color).strong());

                        ui.label("Taxa de bits obxectivo: ~12 Mbps (1080p50 VP9/WebM)");
                        ui.label("Espazo estimado por hora: ~5.4 GB / hora");
                    });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let is_rec = self.is_recording_or_streaming();
                        if is_rec {
                            let can_stop = !self.mixer.is_safety_locked;
                            let stop_btn = egui::Button::new(
                                RichText::new("⏹ DETER GRAVACIÓN")
                                    .color(Color32::from_rgb(255, 100, 100))
                                    .strong(),
                            );
                            if ui
                                .add_enabled(can_stop, stop_btn)
                                .on_hover_text("Deter gravación de Program")
                                .on_disabled_hover_text("🔒 Desbloquea o estudo para deter")
                                .clicked()
                            {
                                self.show_stop_recording_confirm = true;
                            }
                        } else {
                            let start_btn = egui::Button::new(
                                RichText::new("🔴 INICIAR GRAVACIÓN")
                                    .color(Color32::from_rgb(0, 220, 180))
                                    .strong(),
                            );
                            if ui.add(start_btn).clicked() {
                                let path = self.recording_dir.join(&target_filename);
                                let path_str = path.to_string_lossy().to_string();
                                self.apply_command(ProductionCommand::StartTestRecording {
                                    path: path_str,
                                });
                            }
                        }
                    });
                });
        }

        #[cfg(feature = "local-control-api")]
        if let Some(remote) = &mut self.remote {
            remote.refresh_snapshot(&self.dispatcher, &self.mixer);
        }
        // UI-only repaint scheduling. Not a guaranteed media/output frame cadence.
        let target_fps = self.mixer.settings.target_fps_value();
        let frame_interval_millis = (1000.0 / target_fps) as u64;
        ctx.request_repaint_after(Duration::from_millis(frame_interval_millis));
    }
}

#[cfg(test)]
mod gui_command_tests {
    use super::*;

    #[test]
    fn demo_camera_ui_never_claims_linux_hardware() {
        let label = camera_placeholder_text(DEMO_CAMERA_PATH);
        assert!(label.contains("DEMOSTRACIÓN"));
        assert!(label.contains("Captura non iniciada"));
        assert!(label.contains('\n'));
        assert!(!label.contains("V4L2"));
        assert!(!label.contains("/dev/video"));
    }
    #[test]
    fn monitoring_is_bounded_without_mutating_the_production_pixels() {
        let rgba = vec![128; 1920 * 1080 * 4];
        let image = monitor_image(1920, 1080, &rgba);
        assert_eq!(image.size, [640, 360]);
        assert_eq!(rgba.len(), 1920 * 1080 * 4);
        assert_eq!(
            image.pixels[0],
            Color32::from_rgba_unmultiplied(128, 128, 128, 128)
        );
    }

    #[test]
    fn replayed_layout_input_does_not_cut_back_to_the_old_program() {
        let mut gate = GuiCommandGate::default();
        let mut mixer = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();
        let destination = mixer.preview_input;
        for _ in 0..2 {
            if gate.allow(1.0, &ProductionCommand::Cut) {
                dispatcher
                    .dispatch(&mut mixer, ProductionCommand::Cut)
                    .unwrap();
            }
        }
        assert_eq!(mixer.program_input, destination);
        assert_eq!(dispatcher.revision(), 1);
        if gate.allow(2.0, &ProductionCommand::Cut) {
            dispatcher
                .dispatch(&mut mixer, ProductionCommand::Cut)
                .unwrap();
        }
        assert_eq!(dispatcher.revision(), 2);
        assert_eq!(mixer.preview_input, destination);
    }

    #[test]
    fn test_input_reorder_and_tally_integrity() {
        let mut mixer = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();
        let initial_pvw = mixer.preview_input;
        let initial_pgm = mixer.program_input;

        // Reorder input 5 (BARRAS) to index 0
        assert_eq!(mixer.inputs[4].id, 5);
        dispatcher
            .dispatch(
                &mut mixer,
                ProductionCommand::ReorderInput {
                    input: 5,
                    target_index: 0,
                },
            )
            .unwrap();

        assert_eq!(mixer.inputs[0].id, 5);
        // Preview and Program inputs remain logically intact
        assert_eq!(mixer.preview_input, initial_pvw);
        assert_eq!(mixer.program_input, initial_pgm);
    }

    #[test]
    fn test_fade_to_black_and_overlay_commands() {
        let mut mixer = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();

        assert!(!mixer.is_fade_to_black);
        dispatcher
            .dispatch(&mut mixer, ProductionCommand::ToggleFadeToBlack)
            .unwrap();
        assert!(mixer.is_fade_to_black);

        // Overlay 2 toggle
        assert_eq!(mixer.active_overlays[1], None);
        dispatcher
            .dispatch(
                &mut mixer,
                ProductionCommand::ToggleOverlay {
                    channel: 2,
                    input: 3,
                },
            )
            .unwrap();
        assert_eq!(mixer.active_overlays[1], Some(3));
    }

    #[test]
    fn test_generate_diagnostics_sanitized() {
        let app = OpenVideoMixApp::new_test();
        let diag = app.generate_diagnostics();
        assert_eq!(diag.product, "OPENVIDEOMIX");
        assert!(diag.sanitized);
        assert!(!diag.inputs.is_empty());
        let md = diag.to_markdown_report();
        assert!(md.contains("OPENVIDEOMIX Diagnostics Report"));
    }

    #[test]
    fn test_theme_mode_switching_and_visuals() {
        let mut app = OpenVideoMixApp::new_test();
        assert_eq!(app.theme_mode, ThemeMode::Dark);
        assert!(app.theme_mode.is_dark());
        assert_eq!(
            app.theme_mode.label(),
            "🌙 Modo Escuro (Broadcast Dark Studio)"
        );
        assert_eq!(app.ui_scale, 1.0);

        let dark_visuals = app.theme_mode.visuals();
        assert_eq!(dark_visuals.panel_fill, Color32::from_rgb(18, 20, 24));
        assert_eq!(dark_visuals.window_fill, Color32::from_rgb(24, 27, 32));

        // Switch to Light theme
        app.theme_mode = ThemeMode::Light;
        assert_eq!(app.theme_mode, ThemeMode::Light);
        assert!(!app.theme_mode.is_dark());
        assert_eq!(app.theme_mode.label(), "☀️ Modo Claro (Clean Light Studio)");

        let light_visuals = app.theme_mode.visuals();
        assert_eq!(light_visuals.panel_fill, Color32::from_rgb(240, 243, 246));
        assert_eq!(light_visuals.window_fill, Color32::from_rgb(248, 250, 252));

        // UI scale bounds and update
        app.ui_scale = 1.25;
        assert!((app.ui_scale - 1.25).abs() < 0.001);
    }

    #[test]
    fn test_unicode_special_characters_integrity() {
        let app = OpenVideoMixApp::new_test();
        assert!(app.unicode_test_text.contains('ñ'));
        assert!(app.unicode_test_text.contains('Á'));
        assert!(app.unicode_test_text.contains('á'));
        assert!(app.unicode_test_text.contains('¿'));
        assert!(app.unicode_test_text.contains('¡'));
        assert!(app.unicode_test_text.contains('«'));
        assert!(app.unicode_test_text.contains('»'));

        // Verify full string UTF-8 validity
        assert!(std::str::from_utf8(app.unicode_test_text.as_bytes()).is_ok());

        // Verify font definitions default
        let fonts = egui::FontDefinitions::default();
        assert!(!fonts.font_data.is_empty());
    }

    #[test]
    fn test_settings_tab_theme_navigation() {
        let mut app = OpenVideoMixApp::new_test();
        assert_eq!(app.settings_tab, 0);
        app.settings_tab = 4; // Aspecto e Tema
        assert_eq!(app.settings_tab, 4);
    }

    #[test]
    fn test_broadcast_safety_lock_in_app() {
        let mut app = OpenVideoMixApp::new_test();
        assert!(!app.mixer.is_safety_locked);

        // Lock safety via ProductionCommand
        app.gui_time = 1.0;
        app.apply_command(ProductionCommand::ToggleSafetyLock);
        assert!(app.mixer.is_safety_locked);

        // While safety locked, input removal must be blocked
        let initial_count = app.mixer.inputs.len();
        let first_id = app.mixer.inputs[0].id;
        app.mixer.remove_input(first_id);
        assert_eq!(app.mixer.inputs.len(), initial_count);
        assert!(app.mixer.inputs.iter().any(|i| i.id == first_id));

        // Unlock safety on a new frame timestamp
        app.gui_time = 2.0;
        app.apply_command(ProductionCommand::ToggleSafetyLock);
        assert!(!app.mixer.is_safety_locked);

        // Input removal succeeds when unlocked
        app.mixer.remove_input(first_id);
        assert_eq!(app.mixer.inputs.len(), initial_count - 1);
        assert!(!app.mixer.inputs.iter().any(|i| i.id == first_id));
    }

    #[test]
    fn test_is_recording_or_streaming_detection() {
        let mut app = OpenVideoMixApp::new_test();
        assert!(!app.is_recording_or_streaming());

        // Simulation or production flags
        app.mixer.is_recording = true;
        assert!(app.is_recording_or_streaming());
        app.mixer.is_recording = false;

        app.mixer.is_streaming = true;
        assert!(app.is_recording_or_streaming());
        app.mixer.is_streaming = false;

        app.mixer.recording_intent = Some(openvideomix_core::RecordingIntent {
            revision: 42,
            path: Some("rec.webm".into()),
            live_audio: None,
        });
        assert!(app.is_recording_or_streaming());
    }

    #[test]
    fn test_close_guard_and_stop_confirm_flow() {
        let mut app = OpenVideoMixApp::new_test();
        assert!(!app.show_stop_recording_confirm);
        assert!(!app.show_close_warning_modal);
        assert!(!app.force_close_confirmed);

        // Stopping active recording prompts modal confirmation
        app.mixer.is_recording = true;
        assert!(app.is_recording_or_streaming());

        app.show_stop_recording_confirm = true;
        assert!(app.show_stop_recording_confirm);

        // Simulating confirming stop
        app.apply_command(ProductionCommand::StopRecording);
        app.show_stop_recording_confirm = false;
        assert!(!app.show_stop_recording_confirm);

        // Active recording close interception
        app.mixer.is_recording = true;
        if !app.force_close_confirmed && app.is_recording_or_streaming() {
            app.show_close_warning_modal = true;
        }
        assert!(app.show_close_warning_modal);

        // Operator confirms stop and close in modal
        app.apply_command(ProductionCommand::StopRecording);
        app.force_close_confirmed = true;
        app.show_close_warning_modal = false;
        assert!(!app.show_close_warning_modal);
        assert!(app.force_close_confirmed);
    }

    #[test]
    fn test_title_designer_and_lower_thirds() {
        let mut app = OpenVideoMixApp::new_test();
        assert!(!app.show_title_designer);
        assert_eq!(app.selected_title_input, Some(6));

        // Find the title input (default input 6)
        let title_input = app
            .mixer
            .inputs
            .iter()
            .find(|i| matches!(i.kind, InputKind::Title { .. }))
            .expect("default state must include a Title input");

        let title_id = title_input.id;
        assert_eq!(title_id, 6);

        // Open title designer
        app.selected_title_input = Some(title_id);
        app.show_title_designer = true;
        if let InputKind::Title { template } = &title_input.kind {
            app.editing_title = template.clone();
        }

        // Edit template with Galician / Spanish Unicode characters
        let mut updated = app.editing_title.clone();
        updated.headline = "ÚLTIMA HORA: Eleccións no Parlamento de Galicia".to_string();
        updated.subtitle =
            "Santiago de Compostela · Conexión en directo co 100% escrutado".to_string();
        updated.x_percent = 5.0;
        updated.y_percent = 82.0;
        updated.scale = 1.1;

        // Apply command
        app.apply_command(ProductionCommand::UpdateTitle {
            input: title_id,
            template: updated.clone(),
        });

        // Verify that mixer state has updated
        let modified = app.mixer.inputs.iter().find(|i| i.id == title_id).unwrap();
        if let InputKind::Title { template } = &modified.kind {
            assert_eq!(
                template.headline,
                "ÚLTIMA HORA: Eleccións no Parlamento de Galicia"
            );
            assert_eq!(
                template.subtitle,
                "Santiago de Compostela · Conexión en directo co 100% escrutado"
            );
            assert_eq!(template.x_percent, 5.0);
            assert_eq!(template.y_percent, 82.0);
        } else {
            panic!("expected InputKind::Title");
        }

        // Toggle title onto Overlay 1
        app.gui_time += 1.0;
        app.apply_command(ProductionCommand::ToggleOverlay {
            channel: 1,
            input: title_id,
        });
        assert_eq!(app.mixer.active_overlays[0], Some(title_id));
        assert!(
            app.mixer
                .inputs
                .iter()
                .find(|i| i.id == title_id)
                .unwrap()
                .is_overlay_active
        );

        // Turn overlay 1 off
        app.gui_time += 1.0;
        app.apply_command(ProductionCommand::ToggleOverlay {
            channel: 1,
            input: title_id,
        });
        assert_eq!(app.mixer.active_overlays[0], None);
        assert!(
            !app.mixer
                .inputs
                .iter()
                .find(|i| i.id == title_id)
                .unwrap()
                .is_overlay_active
        );
    }

    #[test]
    fn test_audio_delay_lip_sync_gui() {
        let mut app = OpenVideoMixApp::new_test();
        assert_eq!(app.mixer.inputs[0].audio_delay_ms, 0);
        assert_eq!(app.mixer.master_bus.delay_ms, 0);

        app.gui_time += 1.0;
        app.apply_command(ProductionCommand::SetAudioDelay {
            input: Some(1),
            delay_ms: 140,
        });
        assert_eq!(app.mixer.inputs[0].audio_delay_ms, 140);

        app.gui_time += 1.0;
        app.apply_command(ProductionCommand::SetAudioDelay {
            input: None,
            delay_ms: 60,
        });
        assert_eq!(app.mixer.master_bus.delay_ms, 60);
    }

    #[test]
    fn test_media_clip_player_gui() {
        let mut app = OpenVideoMixApp::new_test();
        assert!(app.mixer.media_players.contains_key(&4));
        let player = app.mixer.media_players.get(&4).unwrap();
        assert!(!player.is_playing);
        assert!(player.is_looping);

        app.gui_time += 1.0;
        app.apply_command(ProductionCommand::MediaPlay { input: 4 });
        assert!(app.mixer.media_players.get(&4).unwrap().is_playing);

        app.gui_time += 1.0;
        app.apply_command(ProductionCommand::MediaSeek {
            input: 4,
            position_ms: 5000,
        });
        assert_eq!(app.mixer.media_players.get(&4).unwrap().position_ms, 5000);

        app.gui_time += 1.0;
        app.apply_command(ProductionCommand::MediaToggleLoop { input: 4 });
        assert!(!app.mixer.media_players.get(&4).unwrap().is_looping);

        app.gui_time += 1.0;
        app.apply_command(ProductionCommand::MediaPause { input: 4 });
        assert!(!app.mixer.media_players.get(&4).unwrap().is_playing);
    }

    #[test]
    fn test_project_store_save_and_load() {
        let mut app = OpenVideoMixApp::new_test();
        app.mixer.inputs[0].volume = 0.42;
        app.mixer.inputs[0].audio_delay_ms = 85;
        app.mixer.master_bus.delay_ms = 35;
        app.mixer.active_overlays[0] = Some(6);

        let temp_dir = std::env::temp_dir();
        let project_path = temp_dir.join(format!(
            "test_project_{}.ovmix",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        let res = app.save_project_to_path(&project_path);
        assert!(res.is_ok());
        assert_eq!(app.current_project_path, Some(project_path.clone()));

        let mut app2 = OpenVideoMixApp::new_test();
        assert_ne!(app2.mixer.inputs[0].volume, 0.42);

        let load_res = app2.load_project_from_path(&project_path);
        assert!(load_res.is_ok());
        assert_eq!(app2.mixer.inputs[0].volume, 0.42);
        assert_eq!(app2.mixer.inputs[0].audio_delay_ms, 85);
        assert_eq!(app2.mixer.master_bus.delay_ms, 35);
        assert_eq!(app2.mixer.active_overlays[0], Some(6));

        let _ = std::fs::remove_file(project_path);
    }

    #[test]
    fn test_recording_panel_and_timer() {
        let mut app = OpenVideoMixApp::new_test();
        assert!(!app.show_recording_panel);
        app.show_recording_panel = true;
        assert!(app.show_recording_panel);

        app.gui_time += 1.0;
        app.apply_command(ProductionCommand::StartTestRecording {
            path: "test_rec.webm".into(),
        });
        assert!(app.is_recording_or_streaming());

        // Verify elapsed duration simulation
        app.recording_start_instant = Some(Instant::now() - Duration::from_secs(12));
        app.update_simulation();
        assert!(app.recording_elapsed_secs >= 12);

        app.gui_time += 1.0;
        app.apply_command(ProductionCommand::StopRecording);
        app.update_simulation();
        assert!(!app.is_recording_or_streaming());
        assert_eq!(app.recording_elapsed_secs, 0);
    }
}
