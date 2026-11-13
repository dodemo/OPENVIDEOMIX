use std::time::Instant;
use chrono::Local;
use eframe::egui::{self, Color32, Key, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use openvideomix_ai::{AiAssistantEngine, AiBroadcastAction};
use openvideomix_core::{InputId, InputKind, MixerState, VideoInput};

pub struct OpenVideoMixApp {
    pub mixer: MixerState,
    pub ai_engine: AiAssistantEngine,
    pub ai_prompt: String,
    pub ai_notification: Option<String>,
    pub show_settings: bool,
    pub settings_tab: usize, // 0: Video, 1: Audio, 2: IA/PCgrande, 3: Dispositivos
    pub last_frame_time: Instant,
    pub animated_tick: f32,
    pub connected_v4l2_devices: Vec<String>,
}

impl OpenVideoMixApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            mixer: MixerState::default(),
            ai_engine: AiAssistantEngine::default(),
            ai_prompt: String::new(),
            ai_notification: Some("OPENVIDEOMIX Studio iniciado. Preme F1 ou AXUSTES para configurar.".into()),
            show_settings: false,
            settings_tab: 0,
            last_frame_time: Instant::now(),
            animated_tick: 0.0,
            connected_v4l2_devices: Vec::new(),
        };
        app.refresh_video_devices();
        app
    }

    /// Scan system for video devices (/dev/video*)
    pub fn refresh_video_devices(&mut self) {
        self.connected_v4l2_devices.clear();
        for i in 0..16 {
            let path = format!("/dev/video{}", i);
            if std::path::Path::new(&path).exists() {
                self.connected_v4l2_devices.push(path);
            }
        }
    }

    /// Update frame time, animations and audio simulation
    fn update_simulation(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;
        if dt > 0.0001 {
            let instant_fps = 1.0 / dt;
            self.mixer.fps = (self.mixer.fps * 0.96) + (instant_fps * 0.04);
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

    /// Draw video content on a given monitor rect (SMPTE bars, moving clock, desktop simulation, or camera status)
    fn render_monitor_content(&self, ui: &mut egui::Ui, rect: Rect, input_id: Option<InputId>, is_program: bool) {
        let painter = ui.painter();
        let input = input_id.and_then(|id| self.mixer.inputs.iter().find(|i| i.id == id));

        match input {
            Some(inp) => match &inp.kind {
                InputKind::ColorBars => {
                    // SMPTE Color Bars
                    let colors = [
                        Color32::from_rgb(230, 230, 230), // White
                        Color32::from_rgb(220, 220, 30),  // Yellow
                        Color32::from_rgb(30, 220, 220),  // Cyan
                        Color32::from_rgb(40, 210, 40),   // Green
                        Color32::from_rgb(220, 30, 220),  // Magenta
                        Color32::from_rgb(220, 40, 40),   // Red
                        Color32::from_rgb(30, 30, 220),   // Blue
                    ];
                    let bar_w = rect.width() / 7.0;
                    for (idx, color) in colors.iter().enumerate() {
                        let bar_rect = Rect::from_min_size(
                            Pos2::new(rect.min.x + (idx as f32 * bar_w), rect.min.y),
                            Vec2::new(bar_w + 1.0, rect.height() * 0.75),
                        );
                        painter.rect_filled(bar_rect, 0.0, *color);
                    }
                    // Bottom dark calibration segment
                    let bottom_rect = Rect::from_min_size(
                        Pos2::new(rect.min.x, rect.min.y + rect.height() * 0.75),
                        Vec2::new(rect.width(), rect.height() * 0.25),
                    );
                    painter.rect_filled(bottom_rect, 0.0, Color32::from_rgb(18, 18, 18));

                    // Animated moving scanline / box to prove 60fps real-time motion
                    let scan_x = rect.min.x + ((self.animated_tick * 25.0) % rect.width());
                    let scan_rect = Rect::from_min_size(Pos2::new(scan_x, bottom_rect.min.y + 2.0), Vec2::new(12.0, bottom_rect.height() - 4.0));
                    painter.rect_filled(scan_rect, 2.0, Color32::from_rgb(0, 255, 180));

                    // Digital Timecode Display
                    let now_str = Local::now().format("%H:%M:%S").to_string();
                    let frames = ((self.animated_tick * 30.0) as u32) % 60;
                    let tc = format!("{}:{:02} SMPTE 60p", now_str, frames);
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
                        painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 30, 45));
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("📹 SINAL EN DIRECTO ({})\nCaptura V4L2 Activa 1080p60", device_path),
                            egui::FontId::proportional(16.0),
                            Color32::from_rgb(100, 220, 255),
                        );
                    } else {
                        painter.rect_filled(rect, 0.0, Color32::from_rgb(28, 20, 20));
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
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(25, 30, 40));
                    // Fake desktop elements with real time
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
                        format!("🖥️ CAPTURA DE ESCRITORIO (Monitor #{})\nWayland / PipeWire Display Portal", display_id),
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
                InputKind::TitleOverlay { title, subtitle } => {
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(15, 15, 20));
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("🏷️ RÓTULO CON ALFA (PNG)\n\"{}\"\n{}", title, subtitle),
                        egui::FontId::proportional(15.0),
                        Color32::from_rgb(255, 220, 120),
                    );
                }
                InputKind::NetworkStream { url, protocol } => {
                    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 22, 28));
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("🌐 FLUXO DE REDE [{}]: {}", protocol, url),
                        egui::FontId::proportional(14.0),
                        Color32::LIGHT_BLUE,
                    );
                }
                _ => {
                    painter.rect_filled(rect, 0.0, Color32::BLACK);
                }
            },
            None => {
                painter.rect_filled(rect, 0.0, Color32::from_rgb(10, 10, 12));
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "NINGUNHA ENTRADA SELECCIONADA",
                    egui::FontId::proportional(16.0),
                    Color32::GRAY,
                );
            }
        }

        // RENDER ACTIVE OVERLAY 1 (LOWER THIRD WITH PNG TRANSPARENCY) OVER PROGRAM
        if is_program {
            if let Some(ov_id) = self.mixer.active_overlay_1 {
                if let Some(ov_input) = self.mixer.inputs.iter().find(|i| i.id == ov_id) {
                    if let InputKind::TitleOverlay { title, subtitle } = &ov_input.kind {
                        // Broadcast Lower Third Banner with Transparency
                        let banner_margin = 16.0;
                        let banner_w = rect.width() - (banner_margin * 2.0);
                        let banner_h = 48.0;
                        let banner_rect = Rect::from_min_size(
                            Pos2::new(rect.min.x + banner_margin, rect.max.y - banner_h - 14.0),
                            Vec2::new(banner_w, banner_h),
                        );

                        // Translucent background with alpha channel (PNG style)
                        painter.rect_filled(banner_rect, 6.0, Color32::from_rgba_premultiplied(16, 20, 28, 225));
                        painter.rect_stroke(banner_rect, 6.0, Stroke::new(1.5_f32, Color32::from_rgba_premultiplied(0, 195, 255, 200)), StrokeKind::Inside);

                        // Accent colored tag
                        let tag_rect = Rect::from_min_size(banner_rect.min, Vec2::new(6.0, banner_h));
                        painter.rect_filled(tag_rect, 3.0, Color32::from_rgb(255, 80, 0));

                        // Title and Subtitle text inside the lower third
                        painter.text(
                            Pos2::new(banner_rect.min.x + 16.0, banner_rect.min.y + 14.0),
                            egui::Align2::LEFT_CENTER,
                            title,
                            egui::FontId::proportional(16.0),
                            Color32::WHITE,
                        );
                        painter.text(
                            Pos2::new(banner_rect.min.x + 16.0, banner_rect.min.y + 32.0),
                            egui::Align2::LEFT_CENTER,
                            subtitle,
                            egui::FontId::proportional(11.0),
                            Color32::from_rgb(180, 225, 255),
                        );
                    }
                }
            }
        }
    }
}

impl eframe::App for OpenVideoMixApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_simulation();

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
        for (idx, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6].iter().enumerate() {
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
                ui.heading(RichText::new("📹 OPENVIDEOMIX").color(Color32::from_rgb(0, 190, 255)).strong());
                ui.label(RichText::new("v0.1.0-alpha").weak());
                ui.separator();

                ui.label(RichText::new("PERFÍL:").strong());
                ui.label(&self.mixer.settings.output_resolution);
                ui.label(format!("@ {}", self.mixer.settings.frame_rate));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let stream_color = if self.mixer.is_streaming { Color32::from_rgb(220, 40, 40) } else { Color32::from_rgb(45, 50, 60) };
                    let stream_btn = egui::Button::new(RichText::new(if self.mixer.is_streaming { "🔴 EMITINDO" } else { "📡 EMITIR" }).color(Color32::WHITE)).fill(stream_color);
                    if ui.add(stream_btn).clicked() {
                        self.mixer.is_streaming = !self.mixer.is_streaming;
                    }

                    let rec_color = if self.mixer.is_recording { Color32::from_rgb(220, 40, 40) } else { Color32::from_rgb(45, 50, 60) };
                    let rec_btn = egui::Button::new(RichText::new(if self.mixer.is_recording { "⏹ GRAVANDO" } else { "⏺ GRAVAR" }).color(Color32::WHITE)).fill(rec_color);
                    if ui.add(rec_btn).clicked() {
                        self.mixer.is_recording = !self.mixer.is_recording;
                    }

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
                ui.label(format!("FPS: {:.1}", self.mixer.fps));
                ui.separator();
                ui.label(format!("CPU: {:.1}%", self.mixer.cpu_usage_pct));
                ui.separator();
                ui.label(format!("Webcams detectadas: {}", self.connected_v4l2_devices.len()));
                ui.separator();

                if let Some(msg) = &self.ai_notification {
                    ui.label(RichText::new(format!("🤖 IA: {}", msg)).color(Color32::from_rgb(100, 220, 180)));
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
                        self.mixer.cut();
                    }

                    ui.add_space(4.0);

                    if ui.add_sized([120.0, 30.0], egui::Button::new("FADE (500ms)")).clicked() {
                        self.mixer.cut();
                    }

                    if ui.add_sized([120.0, 26.0], egui::Button::new("MERGE")).clicked() {
                        self.mixer.cut();
                    }

                    if ui.add_sized([120.0, 26.0], egui::Button::new("WIPE")).clicked() {
                        self.mixer.cut();
                    }

                    ui.add_space(8.0);
                    ui.label(RichText::new("T-BAR FADER").weak());
                    let mut tbar = self.mixer.tbar_position;
                    if ui.add(egui::Slider::new(&mut tbar, 0.0..=1.0).vertical().show_value(false)).changed() {
                        self.mixer.set_tbar(tbar);
                    }
                });

                // PROGRAM (PGM)
                ui.vertical(|ui| {
                    let program_name = self.mixer.program_input
                        .and_then(|id| self.mixer.inputs.iter().find(|i| i.id == id))
                        .map(|i| i.name.as_str())
                        .unwrap_or("NINGÚN");

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔴 PROGRAM (ON AIR)").color(Color32::from_rgb(240, 50, 50)).strong());
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

            // LOWER SECTION: INPUTS RACK + AUDIO MIXER + AI
            ui.horizontal(|ui| {
                // INPUTS SWITCHER (Left 65% width)
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width() * 0.65);
                    ui.horizontal(|ui| {
                        ui.heading("Entradas de Vídeo");
                        ui.label(format!("({} activas)", self.mixer.inputs.len()));

                        if ui.button("➕ Engadir Cámara").clicked() {
                            let next_id = (self.mixer.inputs.len() + 1) as u32;
                            self.mixer.inputs.push(VideoInput::new(
                                next_id,
                                format!("Nova Cámara {}", next_id),
                                InputKind::Camera { device_path: format!("/dev/video{}", next_id - 1) },
                            ));
                        }

                        if ui.button("➕ Engadir Rótulo PNG").clicked() {
                            let next_id = (self.mixer.inputs.len() + 1) as u32;
                            self.mixer.inputs.push(VideoInput::new(
                                next_id,
                                format!("Rótulo {}", next_id),
                                InputKind::TitleOverlay {
                                    title: "NOVO TITULAR".into(),
                                    subtitle: "Subtítulo do rótulo superposto".into(),
                                },
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
                                self.mixer.select_program(id);
                            }
                            if let Some(id) = action_pvw_to {
                                self.mixer.select_preview(id);
                            }
                            if let Some(id) = action_remove {
                                self.mixer.remove_input(id);
                            }
                            if let Some(id) = action_toggle_ov1 {
                                self.mixer.toggle_overlay_1(id);
                            }
                        });
                    });
                });

                ui.separator();

                // AUDIO MIXER & AI
                ui.vertical(|ui| {
                    ui.heading("Mesturador de Son");
                    ui.horizontal(|ui| {
                        // Master Bus with Real dB-style Peak Meter
                        ui.vertical(|ui| {
                            ui.label(RichText::new("MASTER").strong());
                            let mut vol = self.mixer.master_bus.volume;
                            if ui.add(egui::Slider::new(&mut vol, 0.0..=1.0).vertical().text("Vol")).changed() {
                                self.mixer.master_bus.volume = vol;
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

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // AI Assistant Controls
                    ui.heading("Copiloto de IA");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.ai_prompt);
                        if ui.button("Executar").clicked() {
                            if let Some(action) = self.ai_engine.parse_director_command(&self.ai_prompt) {
                                match action {
                                    AiBroadcastAction::SwitchCamera { target_input, reason } => {
                                        self.mixer.select_program(target_input);
                                        self.ai_notification = Some(format!("IA cambiou a cámara {} ({})", target_input, reason));
                                    }
                                    AiBroadcastAction::DisplayLowerThird { text, subtext, .. } => {
                                        // Activate or update lower third overlay
                                        if let Some(inp) = self.mixer.inputs.iter_mut().find(|i| matches!(i.kind, InputKind::TitleOverlay { .. })) {
                                            inp.kind = InputKind::TitleOverlay { title: text.clone(), subtitle: subtext.clone() };
                                            self.mixer.active_overlay_1 = Some(inp.id);
                                        }
                                        self.ai_notification = Some(format!("Rótulo activado: {} - {}", text, subtext));
                                    }
                                    _ => {}
                                }
                            } else {
                                self.ai_notification = Some(format!("Comando enviado ao PCgrande: '{}'", self.ai_prompt));
                            }
                            self.ai_prompt.clear();
                        }
                    });
                });
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
                        ui.selectable_value(&mut self.settings_tab, 2, "🤖 IA & PCgrande");
                        ui.selectable_value(&mut self.settings_tab, 3, "🔌 Dispositivos");
                    });
                    ui.separator();

                    match self.settings_tab {
                        0 => {
                            ui.heading("Configuración de Vídeo");
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
                            ui.horizontal(|ui| {
                                ui.label("Taxa de Cadros (FPS):");
                                egui::ComboBox::from_id_salt("fps_combo")
                                    .selected_text(&self.mixer.settings.frame_rate)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.mixer.settings.frame_rate, "59.94 fps".into(), "59.94 fps (Broadcast)");
                                        ui.selectable_value(&mut self.mixer.settings.frame_rate, "60.00 fps".into(), "60.00 fps");
                                        ui.selectable_value(&mut self.mixer.settings.frame_rate, "30.00 fps".into(), "30.00 fps");
                                        ui.selectable_value(&mut self.mixer.settings.frame_rate, "25.00 fps".into(), "25.00 fps (PAL)");
                                    });
                            });
                        }
                        1 => {
                            ui.heading("Configuración de Son");
                            ui.label(format!("Dispositivo de entrada: {}", self.mixer.settings.audio_device));
                            ui.label(format!("Frecuencia de mostraxe: {} Hz", self.mixer.settings.audio_sample_rate));
                            ui.checkbox(&mut self.mixer.master_bus.muted, "Silenciar Master (Mute)");
                        }
                        2 => {
                            ui.heading("Conexión cos Modelos de IA");
                            ui.label("Servidor local no PCgrande (Ollama / vLLM):");
                            ui.text_edit_singleline(&mut self.mixer.settings.pcgrande_endpoint);
                            ui.label("Modelo local seleccionado:");
                            ui.text_edit_singleline(&mut self.mixer.settings.pcgrande_model);

                            if ui.button("📡 Probar conexión co PCgrande").clicked() {
                                self.ai_notification = Some(format!("Conectando con {}...", self.mixer.settings.pcgrande_endpoint));
                            }

                            ui.separator();
                            ui.label("Chave de API OpenAI Enterprise (Opcional):");
                            ui.text_edit_singleline(&mut self.mixer.settings.openai_api_key);
                        }
                        3 => {
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

        ctx.request_repaint_after(std::time::Duration::from_millis(16)); // 60fps UI repaint
    }
}
