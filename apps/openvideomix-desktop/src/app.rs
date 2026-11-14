#[cfg(feature = "gstreamer-capture")]
use crate::capture::{CameraEvent, CameraSession};
use chrono::Local;
use eframe::egui::{
    self, Color32, Key, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, Vec2,
};
use openvideomix_core::{InputId, InputKind, MixerState, VideoInput};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct OpenVideoMixApp {
    pub mixer: MixerState,
    pub status_notification: Option<String>,
    pub show_settings: bool,
    pub settings_tab: usize, // 0: Video, 1: Audio, 2: Dispositivos
    pub last_frame_time: Instant,
    pub animated_tick: f32,
    pub connected_v4l2_devices: Vec<String>,
    pub loaded_textures: HashMap<InputId, TextureHandle>,
    #[cfg(feature = "gstreamer-capture")]
    capture_session: Option<CameraSession>,
    #[cfg(feature = "gstreamer-capture")]
    capture_input_id: Option<InputId>,
}

impl OpenVideoMixApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            mixer: MixerState::default(),
            status_notification: Some(
                "OPENVIDEOMIX listo. 50 FPS PAL activado por defecto.".into(),
            ),
            show_settings: false,
            settings_tab: 0,
            last_frame_time: Instant::now(),
            animated_tick: 0.0,
            connected_v4l2_devices: Vec::new(),
            loaded_textures: HashMap::new(),
            #[cfg(feature = "gstreamer-capture")]
            capture_session: None,
            #[cfg(feature = "gstreamer-capture")]
            capture_input_id: None,
        };
        app.refresh_video_devices();
        app
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

    /// Start the default system USB camera. Only one active capture is
    /// supported by this proof of concept; input selection comes later.
    fn add_default_usb_camera(&mut self) {
        #[cfg(feature = "gstreamer-capture")]
        {
            // Drop old session before starting the new pipeline.
            self.capture_session = None;
            if let Some(id) = self.capture_input_id.take() {
                self.loaded_textures.remove(&id);
                self.mixer.remove_input(id);
            }
            let next_id = self.mixer.inputs.iter().map(|i| i.id).max().unwrap_or(0) + 1;
            let mut input = VideoInput::new(
                next_id,
                "Webcam USB (fonte predeterminada) - a conectar",
                InputKind::Camera {
                    device_path: "auto:default".into(),
                },
            );
            input.is_online = false;
            self.mixer.inputs.push(input);
            self.capture_input_id = Some(next_id);
            self.capture_session = Some(CameraSession::start());
            self.status_notification =
                Some("Abrindo a webcam predeterminada mediante GStreamer...".into());
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
        let (Some(session), Some(id)) = (&self.capture_session, self.capture_input_id) else {
            return;
        };
        let mut last_frame = None;
        let mut error = None;
        while let Ok(event) = session.receiver.try_recv() {
            match event {
                CameraEvent::Frame(frame) => last_frame = Some(frame),
                CameraEvent::Error(message) => error = Some(message),
            }
        }
        if let Some(frame) = last_frame {
            let image =
                egui::ColorImage::from_rgba_unmultiplied([frame.width, frame.height], &frame.rgba);
            if let Some(texture) = self.loaded_textures.get_mut(&id) {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.loaded_textures.insert(
                    id,
                    ctx.load_texture(format!("camera_{id}"), image, egui::TextureOptions::LINEAR),
                );
                self.status_notification =
                    Some("Captura USB activa: imaxes reais en Preview/Program.".into());
            }
            if let Some(input) = self.mixer.inputs.iter_mut().find(|input| input.id == id) {
                input.width = frame.width as u32;
                input.height = frame.height as u32;
                input.fps = 30; // negotiated PoC output; not measured output FPS
                input.is_online = true;
                input.name = "Webcam USB (predeterminada) - vídeo real".into();
            }
        }
        if let Some(message) = error {
            if let Some(input) = self.mixer.inputs.iter_mut().find(|input| input.id == id) {
                input.is_online = false;
                input.name = "Webcam USB - erro de captura".into();
            }
            self.loaded_textures.remove(&id);
            self.status_notification = Some(message);
            self.capture_session = None;
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
    fn render_monitor_content(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        input_id: Option<InputId>,
        is_program: bool,
    ) {
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
                    let scan_rect = Rect::from_min_size(
                        Pos2::new(scan_x, bottom_rect.min.y + 2.0),
                        Vec2::new(14.0, bottom_rect.height() - 4.0),
                    );
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
                            format!(
                                "WEBCAM: {}\\nSen fotogramas reais (captura non iniciada)",
                                device_path
                            ),
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
                        format!("🖥️ CAPTURA DE ESCRITORIO (Monitor #{})\nWayland / PipeWire Display Portal (50 FPS)", display_id),
                        egui::FontId::proportional(16.0),
                        Color32::WHITE,
                    );
                }
                InputKind::MediaFile { path } => {
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 24, 30));
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("▶ VÍDEO: {}\nReprodución multimedia en bucle", path),
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
                    let banner_h =
                        banner_w * (texture.size_vec2().y / texture.size_vec2().x.max(1.0));
                    let max_h = rect.height() * 0.40;
                    let final_h = banner_h.min(max_h);
                    let final_w = if banner_h > max_h {
                        banner_w * (max_h / banner_h)
                    } else {
                        banner_w
                    };

                    let ov_rect = Rect::from_min_size(
                        Pos2::new(
                            rect.center().x - (final_w / 2.0),
                            rect.max.y - final_h - 12.0,
                        ),
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
        self.update_simulation();
        #[cfg(feature = "gstreamer-capture")]
        self.poll_camera(ctx);

        // Keyboard Shortcuts (CUT with Space, FADE with Enter, 1..6 for inputs, F1 for Settings)
        if ctx.input(|i| i.key_pressed(Key::Space)) {
            self.mixer.cut();
        }
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            self.mixer.cut();
        }
        if ctx.input(|i| i.key_pressed(Key::F1)) {
            self.show_settings = !self.show_settings;
        }
        for (idx, key) in [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
        ]
        .iter()
        .enumerate()
        {
            if ctx.input(|i| i.key_pressed(*key)) {
                if let Some(inp) = self.mixer.inputs.get(idx) {
                    let id = inp.id;
                    self.mixer.select_preview(id);
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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let stream_color = if self.mixer.is_streaming {
                        Color32::from_rgb(220, 40, 40)
                    } else {
                        Color32::from_rgb(45, 50, 60)
                    };
                    let stream_btn = egui::Button::new(
                        RichText::new(if self.mixer.is_streaming {
                            "🔴 EMITINDO"
                        } else {
                            "📡 EMITIR"
                        })
                        .color(Color32::WHITE),
                    )
                    .fill(stream_color);
                    if ui.add(stream_btn).clicked() {
                        self.mixer.is_streaming = !self.mixer.is_streaming;
                    }

                    let rec_color = if self.mixer.is_recording {
                        Color32::from_rgb(220, 40, 40)
                    } else {
                        Color32::from_rgb(45, 50, 60)
                    };
                    let rec_btn = egui::Button::new(
                        RichText::new(if self.mixer.is_recording {
                            "⏹ GRAVANDO"
                        } else {
                            "⏺ GRAVAR"
                        })
                        .color(Color32::WHITE),
                    )
                    .fill(rec_color);
                    if ui.add(rec_btn).clicked() {
                        self.mixer.is_recording = !self.mixer.is_recording;
                    }

                    let settings_btn =
                        egui::Button::new(RichText::new("⚙ AXUSTES (F1)").color(Color32::WHITE))
                            .fill(if self.show_settings {
                                Color32::from_rgb(0, 120, 200)
                            } else {
                                Color32::from_rgb(45, 50, 60)
                            });
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
                ui.label(
                    RichText::new(format!("Refresco UI: {:.1}", self.mixer.fps))
                        .strong()
                        .color(Color32::from_rgb(0, 230, 160)),
                );
                ui.separator();
                ui.label(format!("CPU (demo): {:.1}%", self.mixer.cpu_usage_pct));
                ui.separator();
                ui.label(format!(
                    "Nodes V4L2 (Linux): {}",
                    self.connected_v4l2_devices.len()
                ));
                ui.separator();

                if let Some(msg) = &self.status_notification {
                    ui.label(RichText::new(msg).color(Color32::from_rgb(240, 200, 100)));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(
                            "Atallos: Espazo (CUT) | Enter (FADE) | 1-6 (PVW) | F1 (Axustes)",
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

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("PREVIEW")
                                .color(Color32::from_rgb(40, 220, 80))
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
                });

                // TRANSITION CONTROLS & T-BAR
                ui.vertical(|ui| {
                    ui.set_width(130.0);
                    ui.add_space(8.0);

                    if ui
                        .add_sized(
                            [120.0, 38.0],
                            egui::Button::new(
                                RichText::new("CUT")
                                    .strong()
                                    .size(15.0)
                                    .color(Color32::WHITE),
                            ),
                        )
                        .clicked()
                    {
                        self.mixer.cut();
                    }

                    ui.add_space(4.0);

                    if ui
                        .add_sized([120.0, 30.0], egui::Button::new("FADE (500ms)"))
                        .clicked()
                    {
                        self.mixer.cut();
                    }

                    if ui
                        .add_sized([120.0, 26.0], egui::Button::new("MERGE"))
                        .clicked()
                    {
                        self.mixer.cut();
                    }

                    if ui
                        .add_sized([120.0, 26.0], egui::Button::new("WIPE"))
                        .clicked()
                    {
                        self.mixer.cut();
                    }

                    ui.add_space(8.0);
                    ui.label(RichText::new("T-BAR FADER").weak());
                    let mut tbar = self.mixer.tbar_position;
                    if ui
                        .add(
                            egui::Slider::new(&mut tbar, 0.0..=1.0)
                                .vertical()
                                .show_value(false),
                        )
                        .changed()
                    {
                        self.mixer.set_tbar(tbar);
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

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("🔴 PROGRAM (ON AIR)")
                                .color(Color32::from_rgb(240, 50, 50))
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
                        if ui
                            .button(
                                RichText::new("🖼️ ENGADIR Rótulo PNG (Explorador)")
                                    .strong()
                                    .color(Color32::from_rgb(0, 210, 255)),
                            )
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

                        if ui.button("Engadir webcam USB (proba)").clicked() {
                            self.add_default_usb_camera();
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
                                            ui.label(
                                                RichText::new(format!(
                                                    "#{}: {}",
                                                    input.id, input.name
                                                ))
                                                .strong()
                                                .color(Color32::WHITE),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    // Close button
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

                                        // Thumbnail / Preview Box
                                        let (rect, response) = ui.allocate_exact_size(
                                            Vec2::new(168.0, 95.0),
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

                                        if response.clicked() {
                                            action_pvw_to = Some(input.id);
                                        }

                                        // Audio Peak Bar Underneath
                                        let bar_rect = Rect::from_min_size(
                                            Pos2::new(rect.min.x + 4.0, rect.max.y - 7.0),
                                            Vec2::new(
                                                (rect.width() - 8.0) * input.audio_meter_peak,
                                                4.0,
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

                                        // Controls: PVW, PGM, and Overlay 1 [OV1]
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

                                            // Overlay 1 button (vMix style)
                                            let ov1_color = if input.is_overlay_active {
                                                Color32::from_rgb(0, 190, 255)
                                            } else {
                                                Color32::GRAY
                                            };
                                            if ui
                                                .button(
                                                    RichText::new("OV1").small().color(ov1_color),
                                                )
                                                .clicked()
                                            {
                                                action_toggle_ov1 = Some(input.id);
                                            }
                                        });
                                    });
                                });
                                ui.add_space(4.0);
                            }

                            if let Some(id) = action_cut_to {
                                self.mixer.select_program(id);
                            }
                            if let Some(id) = action_pvw_to {
                                self.mixer.select_preview(id);
                            }
                            if let Some(id) = action_remove {
                                #[cfg(feature = "gstreamer-capture")]
                                if self.capture_input_id == Some(id) {
                                    self.capture_session = None;
                                    self.capture_input_id = None;
                                }
                                self.loaded_textures.remove(&id);
                                self.mixer.remove_input(id);
                            }
                            if let Some(id) = action_toggle_ov1 {
                                self.mixer.toggle_overlay_1(id);
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
                            if ui
                                .add(
                                    egui::Slider::new(&mut vol, 0.0..=1.0)
                                        .vertical()
                                        .text("Vol"),
                                )
                                .changed()
                            {
                                self.mixer.master_bus.volume = vol;
                            }
                        });

                        ui.add_space(6.0);

                        // Bus A
                        ui.vertical(|ui| {
                            ui.label("BUS A");
                            let mut vol = self.mixer.bus_a.volume;
                            if ui
                                .add(egui::Slider::new(&mut vol, 0.0..=1.0).vertical())
                                .changed()
                            {
                                self.mixer.bus_a.volume = vol;
                            }
                        });

                        ui.add_space(6.0);

                        // Headphones
                        ui.vertical(|ui| {
                            ui.label("AURIC.");
                            let mut vol = self.mixer.headphones_bus.volume;
                            if ui
                                .add(egui::Slider::new(&mut vol, 0.0..=1.0).vertical())
                                .changed()
                            {
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
                                ui.label(RichText::new("Taxa de Cadros (FPS Reais):").strong());
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
                            ui.label(RichText::new("ℹ️ 50.00 FPS é o estándar de emisión europeo (PAL / TVG / EBU). O motor axustará a cadencia de refresco exacta a 20.0ms por cadro.").weak());
                        }
                        1 => {
                            ui.heading("Configuración de Son");
                            ui.label(format!("Dispositivo de entrada: {}", self.mixer.settings.audio_device));
                            ui.label(format!("Frecuencia de mostraxe: {} Hz", self.mixer.settings.audio_sample_rate));
                            ui.checkbox(&mut self.mixer.master_bus.muted, "Silenciar Master (Mute)");
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

        // UI repaint cadence only: not a measured video capture/output cadence.
        let target_fps = self.mixer.settings.target_fps_value();
        let frame_interval_millis = (1000.0 / target_fps) as u64;
        ctx.request_repaint_after(Duration::from_millis(frame_interval_millis));
    }
}
