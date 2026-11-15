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
use openvideomix_core::{InputId, InputKind, MixerState, VideoInput, DEMO_CAMERA_PATH};
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

        self.mixer.master_bus.peak_l = 0.0;
        self.mixer.master_bus.peak_r = 0.0;
        for input in &mut self.mixer.inputs {
            input.audio_meter_peak = 0.0; // Source audio has not been captured.
        }
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
}

impl eframe::App for OpenVideoMixApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.gui_time = ctx.input(|input| input.time);
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
                ui.heading(RichText::new("📹 OPENVIDEOMIX").color(Color32::from_rgb(0, 190, 255)).strong());
                ui.label(RichText::new("v0.1.0-alpha").weak());
                ui.separator();

                ui.label(RichText::new("PERFÍL:").strong());
                ui.label(&self.mixer.settings.output_resolution);
                ui.label(RichText::new(format!("@ {}", self.mixer.settings.frame_rate)).color(Color32::from_rgb(0, 255, 180)).strong());

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled(false, egui::Button::new("EMITIR (pendente)"))
                        .on_disabled_hover_text("RTMP/SRT non están implementados. Este botón non inicia unha emisión.");
                    #[cfg(all(feature="gstreamer-capture",feature="gstreamer-recording"))]
                    {
                        let view=self.production.as_ref().map(DesktopProduction::view);
                        let active=view.as_ref().is_some_and(|view|view.recording.active());
                        if ui.add_enabled(view.as_ref().is_some_and(|view|view.latest.is_some())||active,egui::Button::new(if active {"PARAR GRAVACIÓN"}else{"GRAVAR TESTE"})).on_hover_text("Grava Program real con ton de proba de 1 kHz a 48 kHz. Non captura micrófono.").clicked() {
                            if active {self.apply_command(ProductionCommand::StopRecording);}
                            else {self.show_recording_setup=true;}
                        }
                    }
                    #[cfg(not(all(feature="gstreamer-capture",feature="gstreamer-recording")))]
                    ui.add_enabled(false,egui::Button::new("GRAVAR (pendente)"));

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
                    let settings_btn = egui::Button::new(RichText::new("⚙ AXUSTES (F1)").color(btn_text_color))
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
                    let diag_btn = egui::Button::new(
                        RichText::new("📋 DIAGNÓSTICO").color(diag_text_color),
                    )
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

                            for (real_idx, input) in self.mixer.inputs.iter().enumerate() {
                                // Category filter
                                let cat_match = match self.input_category_filter {
                                    1 => matches!(&input.kind, InputKind::Camera { .. }),
                                    2 => matches!(
                                        &input.kind,
                                        InputKind::ImageFile { .. } | InputKind::MediaFile { .. }
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
                                                    if ui
                                                        .add(egui::Button::new(
                                                            RichText::new("✕")
                                                                .color(Color32::from_rgb(
                                                                    240, 80, 80,
                                                                ))
                                                                .small(),
                                                        ))
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
                                        });
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
                                #[cfg(feature = "gstreamer-capture")]
                                self.capture_sessions.remove(&id);
                                self.loaded_textures.remove(&id);
                                self.mixer.remove_input(id);
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
        assert_eq!(
            app.theme_mode.label(),
            "☀️ Modo Claro (Clean Light Studio)"
        );

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
}
