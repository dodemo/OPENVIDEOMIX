use std::time::Instant;
use eframe::egui::{self, Color32, RichText, Sense, Stroke, StrokeKind, Vec2};
use openvideomix_ai::{AiAssistantEngine, AiBroadcastAction};
use openvideomix_core::MixerState;

pub struct OpenVideoMixApp {
    pub mixer: MixerState,
    pub ai_engine: AiAssistantEngine,
    pub ai_prompt: String,
    pub ai_notification: Option<String>,
    pub last_frame_time: Instant,
    pub simulated_audio_tick: f32,
}

impl OpenVideoMixApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            mixer: MixerState::default(),
            ai_engine: AiAssistantEngine::default(),
            ai_prompt: String::new(),
            ai_notification: Some("OPENVIDEOMIX listo. Conectado ao motor central.".into()),
            last_frame_time: Instant::now(),
            simulated_audio_tick: 0.0,
        }
    }

    /// Simulate audio peak meters dynamic movement and compute FPS
    fn update_simulation(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;
        if dt > 0.0001 {
            let instant_fps = 1.0 / dt;
            self.mixer.fps = (self.mixer.fps * 0.95) + (instant_fps * 0.05);
        }

        self.simulated_audio_tick += 0.05;
        let sin_val = (self.simulated_audio_tick.sin().abs() * 0.7) + 0.15;
        self.mixer.master_bus.peak_l = sin_val;
        self.mixer.master_bus.peak_r = (sin_val * 0.9).clamp(0.0, 1.0);

        for (i, input) in self.mixer.inputs.iter_mut().enumerate() {
            let offset = (i as f32) * 1.2;
            let p = ((self.simulated_audio_tick + offset).sin().abs() * 0.65).clamp(0.0, 0.95);
            input.audio_meter_peak = p;
        }
    }
}

impl eframe::App for OpenVideoMixApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_simulation();

        // Dark broadcast theme
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(22, 24, 28);
        visuals.window_fill = Color32::from_rgb(28, 31, 36);
        ctx.set_visuals(visuals);

        // Top Header
        egui::TopBottomPanel::top("top_header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading(RichText::new("📹 OPENVIDEOMIX").color(Color32::from_rgb(0, 190, 255)).strong());
                ui.label(RichText::new("v0.1.0-alpha").weak());
                ui.separator();

                ui.label(RichText::new("PRODUCION:").strong());
                ui.label("Estudo Principal (1080p60)");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Action Buttons
                    let stream_color = if self.mixer.is_streaming { Color32::from_rgb(220, 40, 40) } else { Color32::from_rgb(50, 55, 65) };
                    let stream_btn = egui::Button::new(RichText::new(if self.mixer.is_streaming { "🔴 STREAMING ACTIVO" } else { "📡 EMITIR" }).color(Color32::WHITE)).fill(stream_color);
                    if ui.add(stream_btn).clicked() {
                        self.mixer.is_streaming = !self.mixer.is_streaming;
                    }

                    let rec_color = if self.mixer.is_recording { Color32::from_rgb(220, 40, 40) } else { Color32::from_rgb(50, 55, 65) };
                    let rec_btn = egui::Button::new(RichText::new(if self.mixer.is_recording { "⏹ GRAVANDO" } else { "⏺ GRAVAR" }).color(Color32::WHITE)).fill(rec_color);
                    if ui.add(rec_btn).clicked() {
                        self.mixer.is_recording = !self.mixer.is_recording;
                    }

                    if ui.button(RichText::new("⚙ AXUSTES").color(Color32::LIGHT_GRAY)).clicked() {
                        self.ai_notification = Some("Panel de axustes aberto.".into());
                    }
                });
            });
            ui.add_space(4.0);
        });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("FPS: {:.2}", self.mixer.fps));
                ui.separator();
                ui.label(format!("CPU: {:.1}%", self.mixer.cpu_usage_pct));
                ui.separator();
                ui.label(format!("Frames Perdidos: {}", self.mixer.dropped_frames));
                ui.separator();

                if let Some(msg) = &self.ai_notification {
                    ui.label(RichText::new(format!("🤖 IA: {}", msg)).color(Color32::from_rgb(100, 220, 180)));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Licenza MIT — Código Aberto").weak());
                });
            });
        });

        // Central Studio Area (Monitors + Switcher + Audio + AI)
        egui::CentralPanel::default().show(ctx, |ui| {
            // Dual Monitor Area
            ui.horizontal(|ui| {
                let available_width = ui.available_width();
                let monitor_width = (available_width - 150.0) / 2.0;
                let monitor_height = monitor_width * (9.0 / 16.0);

                // PREVIEW (PVW) MONITOR
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
                    ui.painter().rect_stroke(rect, 4.0, Stroke::new(2.0_f32, Color32::from_rgb(40, 200, 70)), StrokeKind::Inside);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("PREVIEW: {}", preview_name),
                        egui::FontId::proportional(18.0),
                        Color32::LIGHT_GRAY,
                    );
                });

                // TRANSITION CONTROLS & T-BAR
                ui.vertical(|ui| {
                    ui.set_width(130.0);
                    ui.add_space(10.0);

                    if ui.add_sized([120.0, 36.0], egui::Button::new(RichText::new("CUT").strong().color(Color32::WHITE))).clicked() {
                        self.mixer.cut();
                    }

                    ui.add_space(4.0);

                    if ui.add_sized([120.0, 32.0], egui::Button::new("FADE (500ms)")).clicked() {
                        self.mixer.cut();
                    }

                    if ui.add_sized([120.0, 28.0], egui::Button::new("MERGE")).clicked() {
                        self.mixer.cut();
                    }

                    if ui.add_sized([120.0, 28.0], egui::Button::new("WIPE")).clicked() {
                        self.mixer.cut();
                    }

                    ui.add_space(8.0);
                    ui.label(RichText::new("T-BAR FADER").weak());
                    let mut tbar = self.mixer.tbar_position;
                    if ui.add(egui::Slider::new(&mut tbar, 0.0..=1.0).vertical().show_value(false)).changed() {
                        self.mixer.set_tbar(tbar);
                    }
                });

                // PROGRAM (PGM) MONITOR
                ui.vertical(|ui| {
                    let program_name = self.mixer.program_input
                        .and_then(|id| self.mixer.inputs.iter().find(|i| i.id == id))
                        .map(|i| i.name.as_str())
                        .unwrap_or("NINGÚN");

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("PROGRAM (ON AIR)").color(Color32::from_rgb(240, 50, 50)).strong());
                        ui.label(format!("({})", program_name));
                    });

                    let (rect, _response) = ui.allocate_exact_size(Vec2::new(monitor_width, monitor_height), Sense::hover());
                    ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(20, 10, 10));
                    ui.painter().rect_stroke(rect, 4.0, Stroke::new(2.5_f32, Color32::from_rgb(230, 40, 40)), StrokeKind::Inside);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("🔴 EMITINDO: {}", program_name),
                        egui::FontId::proportional(20.0),
                        Color32::WHITE,
                    );
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            // Lower Section: Inputs Switcher Rack + Audio Mixer + AI Panel
            ui.horizontal(|ui| {
                // INPUTS SWITCHER GRID (Left 65% width)
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width() * 0.65);
                    ui.horizontal(|ui| {
                        ui.heading("Entradas de Vídeo");
                        if ui.button("➕ Engadir Entrada").clicked() {
                            let next_id = (self.mixer.inputs.len() + 1) as u32;
                            self.mixer.inputs.push(openvideomix_core::VideoInput::new(
                                next_id,
                                format!("Nova Entrada {}", next_id),
                                openvideomix_core::InputKind::ColorBars,
                            ));
                        }
                    });

                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let mut action_cut_to = None;
                            let mut action_pvw_to = None;

                            for input in &self.mixer.inputs {
                                let border_color = if input.is_tally_program {
                                    Color32::from_rgb(230, 40, 40)
                                } else if input.is_tally_preview {
                                    Color32::from_rgb(40, 200, 70)
                                } else {
                                    Color32::from_rgb(50, 55, 60)
                                };

                                ui.group(|ui| {
                                    ui.set_width(170.0);
                                    ui.vertical(|ui| {
                                        // Header
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("#{}: {}", input.id, input.name)).strong().color(Color32::WHITE));
                                        });

                                        // Thumbnail Mockup
                                        let (rect, response) = ui.allocate_exact_size(Vec2::new(160.0, 90.0), Sense::click());
                                        ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(30, 34, 40));
                                        ui.painter().rect_stroke(rect, 3.0, Stroke::new(2.0_f32, border_color), StrokeKind::Inside);

                                        if response.clicked() {
                                            action_pvw_to = Some(input.id);
                                        }

                                        // Audio meter underneath
                                        let bar_rect = egui::Rect::from_min_size(
                                            egui::pos2(rect.min.x + 4.0, rect.max.y - 8.0),
                                            Vec2::new((rect.width() - 8.0) * input.audio_meter_peak, 4.0),
                                        );
                                        ui.painter().rect_filled(bar_rect, 1.0, Color32::from_rgb(50, 220, 100));

                                        // Action buttons
                                        ui.horizontal(|ui| {
                                            if ui.button(RichText::new("PVW").small()).clicked() {
                                                action_pvw_to = Some(input.id);
                                            }
                                            if ui.button(RichText::new("PGM").small().color(Color32::from_rgb(255, 100, 100))).clicked() {
                                                action_cut_to = Some(input.id);
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
                        });
                    });
                });

                ui.separator();

                // AUDIO MIXER & AI ASSISTANT (Right 35% width)
                ui.vertical(|ui| {
                    ui.heading("Mesturador de Son");
                    ui.horizontal(|ui| {
                        // Master Bus
                        ui.vertical(|ui| {
                            ui.label(RichText::new("MASTER").strong());
                            let mut vol = self.mixer.master_bus.volume;
                            if ui.add(egui::Slider::new(&mut vol, 0.0..=1.0).vertical().text("Vol")).changed() {
                                self.mixer.master_bus.volume = vol;
                            }
                        });

                        ui.add_space(8.0);

                        // Bus A
                        ui.vertical(|ui| {
                            ui.label("BUS A");
                            let mut vol = self.mixer.bus_a.volume;
                            if ui.add(egui::Slider::new(&mut vol, 0.0..=1.0).vertical()).changed() {
                                self.mixer.bus_a.volume = vol;
                            }
                        });

                        ui.add_space(8.0);

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
                    ui.label("Control de realización por voz ou texto:");
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

        // Request animation frame for smooth meters and clock
        ctx.request_repaint_after(std::time::Duration::from_millis(33)); // ~30fps UI repaint
    }
}
