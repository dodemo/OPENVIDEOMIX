use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use chrono::Local;
use eframe::egui::{self, Color32, Key, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, Vec2};
use openvideomix_core::{InputId, InputKind, MixerState, VideoInput};
use openvideomix_core::commands::{CommandDispatcher, ProductionCommand};
#[cfg(feature = "midi-input")]
use openvideomix_core::midi::{MidiBinding, MidiEvent, MidiMapper, MidiProfile, MidiSource, MidiTarget};
#[cfg(feature = "midi-input")]
use crate::midi_input::MidiInputService;
#[cfg(feature = "local-control-api")]
use crate::remote_control::RemoteControl;

pub struct OpenVideoMixApp {
    pub mixer: MixerState,
    pub dispatcher: CommandDispatcher,
    #[cfg(feature = "local-control-api")]
    remote: Option<RemoteControl>,
    pub status_notification: Option<String>,
    pub show_settings: bool,
    pub settings_tab: usize, // 0: Video, 1: Audio, 2: Dispositivos
    pub last_frame_time: Instant,
    pub animated_tick: f32,
    pub connected_v4l2_devices: Vec<String>,
    pub loaded_textures: HashMap<InputId, TextureHandle>,
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
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            mixer: MixerState::default(),
            dispatcher: CommandDispatcher::default(),
            #[cfg(feature = "local-control-api")]
            remote: None,
            status_notification: Some("Modo demostración: 50 FPS é só o perfil solicitado. A captura e as saídas aínda non están operativas.".into()),
            show_settings: false,
            settings_tab: 0,
            last_frame_time: Instant::now(),
            animated_tick: 0.0,
            connected_v4l2_devices: Vec::new(),
            loaded_textures: HashMap::new(),
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
        #[cfg(feature = "midi-input")]
        app.refresh_midi_ports();
        #[cfg(feature = "local-control-api")]
        match RemoteControl::start_if_configured(&app.mixer) {
            Ok(Some(server)) => {
                app.remote = Some(server);
                app.status_notification = Some("API local autenticada activa en 127.0.0.1:21873".into());
            }
            Ok(None) => {}
            Err(error) => app.status_notification = Some(format!("API local desactivada: {error}")),
        }
        app
    }

    /// One validated command entry point for the GUI and external surfaces.
    fn apply_command(&mut self, command: ProductionCommand) {
        if let Err(error) = self.dispatcher.dispatch(&mut self.mixer, command) {
            self.status_notification = Some(format!("Orde rexeitada: {error}"));
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
        let profile = MidiProfile::new("OPENVIDEOMIX controller", self.midi_mapper.bindings.clone());
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
                self.status_notification = Some(format!("Perfil MIDI cargado: {count} asignacións"));
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

    /// Scan system for real video devices (/dev/video*)
    pub fn refresh_video_devices(&mut self) {
        self.connected_v4l2_devices.clear();
        for i in 0..16 {
            let path = format!("/dev/video{}", i);
            if std::path::Path::new(&path).exists() {
                self.connected_v4l2_devices.push(path);
            }
        }
    }

    /// Load real PNG / Image file with alpha transparency from disk
    pub fn load_image_input(&mut self, ctx: &egui::Context, path: PathBuf) {
        let path_str = path.to_string_lossy().to_string();
        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

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
                self.status_notification = Some(format!("Imaxe cargada con éxito: {} ({}x{})", file_name, w, h));
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

        // Dynamic audio meters update
        let sin_val = (self.animated_tick.sin().abs() * 0.65) + 0.15;
        self.mixer.master_bus.peak_l = sin_val;
        self.mixer.master_bus.peak_r = (sin_val * 0.92).clamp(0.0, 1.0);

        for (i, input) in self.mixer.inputs.iter_mut().enumerate() {
            let offset = (i as f32) * 1.3;
            let p = ((self.animated_tick + offset).sin().abs() * 0.75).clamp(0.0, 0.98);
            input.audio_meter_peak = if input.is_muted { 0.0 } else { p };
        }
    }

    /// Render video or graphic content inside a monitor rect
    fn render_monitor_content(&self, ui: &mut egui::Ui, rect: Rect, input_id: Option<InputId>, is_program: bool) {
        let painter = ui.painter();
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
                    let scan_rect = Rect::from_min_size(Pos2::new(scan_x, bottom_rect.min.y + 2.0), Vec2::new(14.0, bottom_rect.height() - 4.0));
                    painter.rect_filled(scan_rect, 2.0, Color32::from_rgb(0, 255, 180));

                    // Timecode display in 50 FPS PAL standard
                    let now_str = Local::now().format("%H:%M:%S").to_string();
                    let frames_50 = ((self.animated_tick * 25.0) as u32) % 50;
                    let tc = format!("{}:{:02} (50.00 FPS PAL)", now_str, frames_50);
                    painter.text(
                        Pos2::new(rect.center().x, bottom_rect.center().y),
                        egui::Align2::CENTER_CENTER,
                        tc,
                        egui::FontId::monospace(14.0),
                        Color32::WHITE,
                    );
                }
                InputKind::Camera { device_path } => {
                    let has_dev = self.connected_v4l2_devices.contains(device_path);
                    if has_dev {
                        painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 32, 48));
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("Dispositivo USB detectado: {}\nCAPTURA NON IMPLEMENTADA", device_path),
                            egui::FontId::proportional(16.0),
                            Color32::from_rgb(90, 220, 255),
                        );
                    } else {
                        painter.rect_filled(rect, 0.0, Color32::from_rgb(26, 18, 18));
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("⚠️ SEN SINAL: {}\nConecta a túa webcam USB ao ordenador", device_path),
                            egui::FontId::proportional(15.0),
                            Color32::from_rgb(240, 160, 60),
                        );
                    }
                }
                InputKind::ScreenCapture { display_id } => {
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(24, 28, 38));
                    let taskbar = Rect::from_min_size(Pos2::new(rect.min.x, rect.max.y - 18.0), Vec2::new(rect.width(), 18.0));
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
                        format!("ESCRITORIO #{}\nVista de demostración: sen captura real", display_id),
                        egui::FontId::proportional(16.0),
                        Color32::WHITE,
                    );
                }
                InputKind::MediaFile { path } => {
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 24, 30));
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("VÍDEO: {}\nMarcador de posición: sen decodificación real", path),
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

        // RENDER ACTIVE OVERLAY 1 WITH TRUE ALPHA TRANSPARENCY OVER PROGRAM
        if is_program {
            if let Some(ov_id) = self.mixer.active_overlay_1 {
                if let Some(texture) = self.loaded_textures.get(&ov_id) {
                    // Draw user-loaded transparent PNG over Program!
                    // In vMix, lower thirds typically align at the bottom-center or left
                    let banner_w = rect.width() * 0.85;
                    let banner_h = banner_w * (texture.size_vec2().y / texture.size_vec2().x.max(1.0));
                    let max_h = rect.height() * 0.40;
                    let final_h = banner_h.min(max_h);
                    let final_w = if banner_h > max_h { banner_w * (max_h / banner_h) } else { banner_w };

                    let ov_rect = Rect::from_min_size(
                        Pos2::new(rect.center().x - (final_w / 2.0), rect.max.y - final_h - 12.0),
                        Vec2::new(final_w, final_h),
                    );

                    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                    // egui respects RGBA alpha transparency automatically
                    painter.image(texture.id(), ov_rect, uv, Color32::WHITE);
                }
            }
        }
    }
}

impl eframe::App for OpenVideoMixApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(feature = "local-control-api")]
        if let Some(remote) = &mut self.remote {
            remote.poll(&mut self.dispatcher, &mut self.mixer);
        }
        self.update_simulation();
        #[cfg(feature = "midi-input")]
        self.poll_midi();

        // Keyboard Shortcuts (CUT with Space/Enter; other transitions unavailable)
        if ctx.input(|i| i.key_pressed(Key::Space)) {
            self.apply_command(ProductionCommand::Cut);
        }
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            self.apply_command(ProductionCommand::Cut);
        }
        if ctx.input(|i| i.key_pressed(Key::F1)) {
            self.show_settings = !self.show_settings;
        }
        for (idx, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6].iter().enumerate() {
            if ctx.input(|i| i.key_pressed(*key)) {
                if let Some(inp) = self.mixer.inputs.get(idx) {
                    let id = inp.id;
                    self.apply_command(ProductionCommand::SelectPreview { input: id });
                }
            }
        }

        // Dark Studio Styling
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(18, 20, 24);
        visuals.window_fill = Color32::from_rgb(24, 27, 32);
        ctx.set_visuals(visuals);

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
                    ui.add_enabled(false, egui::Button::new("GRAVAR (pendente)"))
                        .on_disabled_hover_text("A gravación real aínda non está implementada. Este botón non crea un ficheiro.");

                    let settings_btn = egui::Button::new(RichText::new("⚙ AXUSTES (F1)").color(Color32::WHITE))
                        .fill(if self.show_settings { Color32::from_rgb(0, 120, 200) } else { Color32::from_rgb(45, 50, 60) });
                    if ui.add(settings_btn).clicked() {
                        self.show_settings = !self.show_settings;
                    }
                });
            });
            ui.add_space(4.0);
        });

        // BOTTOM STATUS BAR
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Refresco UI (demo): {:.1}", self.mixer.fps)).strong().color(Color32::from_rgb(0, 230, 160)));
                ui.separator();
                ui.label(format!("CPU (demo): {:.1}%", self.mixer.cpu_usage_pct));
                ui.separator();
                ui.label(format!("Nós V4L2 (Linux): {}", self.connected_v4l2_devices.len()));
                ui.separator();

                if let Some(msg) = &self.status_notification {
                    ui.label(RichText::new(msg).color(Color32::from_rgb(240, 200, 100)));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Atallos: Espazo (CUT) | Enter (FADE) | 1-6 (PVW) | F1 (Axustes)").weak());
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
                    let preview_name = self.mixer.preview_input
                        .and_then(|id| self.mixer.inputs.iter().find(|i| i.id == id))
                        .map(|i| i.name.as_str())
                        .unwrap_or("NINGÚN");

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("PREVIEW").color(Color32::from_rgb(40, 220, 80)).strong());
                        ui.label(format!("({})", preview_name));
                    });

                    let (rect, _response) = ui.allocate_exact_size(Vec2::new(monitor_width, monitor_height), Sense::hover());
                    ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(15, 18, 20));
                    self.render_monitor_content(ui, rect, self.mixer.preview_input, false);
                    ui.painter().rect_stroke(rect, 4.0, Stroke::new(2.5_f32, Color32::from_rgb(40, 200, 70)), StrokeKind::Inside);
                });

                // TRANSITION CONTROLS & T-BAR
                ui.vertical(|ui| {
                    ui.set_width(130.0);
                    ui.add_space(8.0);

                    if ui.add_sized([120.0, 38.0], egui::Button::new(RichText::new("CUT").strong().size(15.0).color(Color32::WHITE))).clicked() {
                        self.apply_command(ProductionCommand::Cut);
                    }

                    ui.add_space(4.0);

                    ui.add_enabled(false, egui::Button::new("FADE (pendente)"))
                        .on_disabled_hover_text("FADE require composición interpolada de vídeo; non é un CUT.");
                    ui.add_enabled(false, egui::Button::new("MERGE (pendente)"))
                        .on_disabled_hover_text("MERGE aínda non produce unha transición visual.");
                    ui.add_enabled(false, egui::Button::new("WIPE (pendente)"))
                        .on_disabled_hover_text("WIPE require máscaras no compositor GPU.");

                    ui.add_space(8.0);
                    ui.label(RichText::new("T-BAR (pendente)").weak());
                    let mut tbar = self.mixer.tbar_position;
                    ui.add_enabled(false, egui::Slider::new(&mut tbar, 0.0..=1.0).vertical().show_value(false))
                        .on_disabled_hover_text("O T-Bar aínda non renderiza fotogramas de transición.");
                });

                // PROGRAM (PGM)
                ui.vertical(|ui| {
                    let program_name = self.mixer.program_input
                        .and_then(|id| self.mixer.inputs.iter().find(|i| i.id == id))
                        .map(|i| i.name.as_str())
                        .unwrap_or("NINGÚN");

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("PROGRAM (demostración)").color(Color32::from_rgb(240, 50, 50)).strong());
                        ui.label(format!("({})", program_name));
                    });

                    let (rect, _response) = ui.allocate_exact_size(Vec2::new(monitor_width, monitor_height), Sense::hover());
                    ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(18, 12, 12));
                    self.render_monitor_content(ui, rect, self.mixer.program_input, true);
                    ui.painter().rect_stroke(rect, 4.0, Stroke::new(2.5_f32, Color32::from_rgb(230, 40, 40)), StrokeKind::Inside);
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            // LOWER SECTION: INPUTS RACK + AUDIO MIXER
            ui.horizontal(|ui| {
                // INPUTS SWITCHER (Left 75% width)
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width() * 0.75);
                    ui.horizontal(|ui| {
                        ui.heading("Entradas de Vídeo");
                        ui.label(format!("({} activas)", self.mixer.inputs.len()));

                        // Open native file dialog for real PNG/images
                        if ui.button(RichText::new("🖼️ ENGADIR Rótulo PNG (Explorador)").strong().color(Color32::from_rgb(0, 210, 255))).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Imaxes con transparencia (*.png, *.webp, *.jpg)", &["png", "webp", "jpg", "jpeg"])
                                .set_title("Seleccionar Rótulo ou Imaxe PNG")
                                .pick_file()
                            {
                                self.load_image_input(ctx, path);
                            }
                        }

                        if ui.button("➕ Engadir Cámara USB").clicked() {
                            let next_id = (self.mixer.inputs.iter().map(|i| i.id).max().unwrap_or(0)) + 1;
                            self.mixer.inputs.push(VideoInput::new(
                                next_id,
                                format!("Nova Cámara {}", next_id),
                                InputKind::Camera { device_path: format!("/dev/video{}", next_id - 1) },
                            ));
                        }
                    });

                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let mut action_cut_to = None;
                            let mut action_pvw_to = None;
                            let mut action_remove = None;
                            let mut action_toggle_ov1 = None;

                            for input in &self.mixer.inputs {
                                let border_color = if input.is_tally_program {
                                    Color32::from_rgb(230, 40, 40)
                                } else if input.is_tally_preview {
                                    Color32::from_rgb(40, 200, 70)
                                } else {
                                    Color32::from_rgb(50, 55, 62)
                                };

                                ui.group(|ui| {
                                    ui.set_width(176.0);
                                    ui.vertical(|ui| {
                                        // Header with Title and Close Button [✕]
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("#{}: {}", input.id, input.name)).strong().color(Color32::WHITE));
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                // Close button
                                                if ui.add(egui::Button::new(RichText::new("✕").color(Color32::from_rgb(240, 80, 80)).small())).clicked() {
                                                    action_remove = Some(input.id);
                                                }
                                            });
                                        });

                                        // Thumbnail / Preview Box
                                        let (rect, response) = ui.allocate_exact_size(Vec2::new(168.0, 95.0), Sense::click());
                                        ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(25, 28, 34));
                                        self.render_monitor_content(ui, rect, Some(input.id), false);
                                        ui.painter().rect_stroke(rect, 3.0, Stroke::new(2.0_f32, border_color), StrokeKind::Inside);

                                        if response.clicked() {
                                            action_pvw_to = Some(input.id);
                                        }

                                        // Audio Peak Bar Underneath
                                        let bar_rect = Rect::from_min_size(
                                            Pos2::new(rect.min.x + 4.0, rect.max.y - 7.0),
                                            Vec2::new((rect.width() - 8.0) * input.audio_meter_peak, 4.0),
                                        );
                                        let meter_color = if input.audio_meter_peak > 0.85 {
                                            Color32::from_rgb(235, 60, 60)
                                        } else if input.audio_meter_peak > 0.65 {
                                            Color32::from_rgb(235, 200, 40)
                                        } else {
                                            Color32::from_rgb(50, 210, 90)
                                        };
                                        ui.painter().rect_filled(bar_rect, 1.0, meter_color);

                                        // Controls: PVW, PGM, and Overlay 1 [OV1]
                                        ui.horizontal(|ui| {
                                            if ui.button(RichText::new("PVW").small()).clicked() {
                                                action_pvw_to = Some(input.id);
                                            }
                                            if ui.button(RichText::new("PGM").small().color(Color32::from_rgb(255, 100, 100))).clicked() {
                                                action_cut_to = Some(input.id);
                                            }

                                            // Overlay 1 button (vMix style)
                                            let ov1_color = if input.is_overlay_active { Color32::from_rgb(0, 190, 255) } else { Color32::GRAY };
                                            if ui.button(RichText::new("OV1").small().color(ov1_color)).clicked() {
                                                action_toggle_ov1 = Some(input.id);
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
                                self.mixer.remove_input(id);
                            }
                            if let Some(id) = action_toggle_ov1 {
                                self.apply_command(ProductionCommand::ToggleOverlay1 { input: id });
                            }
                        });
                    });
                });

                ui.separator();

                // AUDIO MIXER (Right 25% width)
                ui.vertical(|ui| {
                    ui.heading("Mesturador de Son");
                    ui.horizontal(|ui| {
                        // Master Bus with Peak Meter
                        ui.vertical(|ui| {
                            ui.label(RichText::new("MASTER").strong());
                            let mut vol = self.mixer.master_bus.volume;
                            if ui.add(egui::Slider::new(&mut vol, 0.0..=1.0).vertical().text("Vol")).changed() {
                                self.apply_command(ProductionCommand::SetMasterVolume { volume: vol });
                            }
                        });

                        ui.add_space(6.0);

                        // Bus A
                        ui.vertical(|ui| {
                            ui.label("BUS A");
                            let mut vol = self.mixer.bus_a.volume;
                            if ui.add(egui::Slider::new(&mut vol, 0.0..=1.0).vertical()).changed() {
                                self.mixer.bus_a.volume = vol;
                            }
                        });

                        ui.add_space(6.0);

                        // Headphones
                        ui.vertical(|ui| {
                            ui.label("AURIC.");
                            let mut vol = self.mixer.headphones_bus.volume;
                            if ui.add(egui::Slider::new(&mut vol, 0.0..=1.0).vertical()).changed() {
                                self.mixer.headphones_bus.volume = vol;
                            }
                        });
                    });
                });
            });
        });

        // SETTINGS MODAL WINDOW (WHEN CLICKING AXUSTES OR F1)
        if self.show_settings {
            egui::Window::new("⚙ Axustes de OPENVIDEOMIX")
                .collapsible(false)
                .resizable(true)
                .default_size([540.0, 380.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.settings_tab, 0, "🎥 Vídeo (Resolución & FPS)");
                        ui.selectable_value(&mut self.settings_tab, 1, "🔊 Son");
                        ui.selectable_value(&mut self.settings_tab, 2, "🔌 Dispositivos de Captura");
                        #[cfg(feature = "midi-input")]
                        ui.selectable_value(&mut self.settings_tab, 3, "MIDI");
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
                            if ui.button("🔄 Refrescar Dispositivos").clicked() {
                                self.refresh_video_devices();
                            }
                            if self.connected_v4l2_devices.is_empty() {
                                ui.label("Non se detectaron cámaras web USB en /dev/video*");
                                ui.label(RichText::new("👉 Podes conectar unha webcam USB e premer 'Refrescar Dispositivos'").color(Color32::KHAKI));
                            } else {
                                for dev in &self.connected_v4l2_devices {
                                    ui.label(format!("✅ Dispositivo dispoñible: {}", dev));
                                }
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
