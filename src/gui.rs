use crate::{
    audio::Microphone,
    frame::Aspect,
    studio::{Command, Controller, Phase, Settings},
};
use anyhow::Result;
use eframe::egui::{self, Color32, RichText};
use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn open_path(path: &Path) -> Result<()> {
    use windows::{
        Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        core::{HSTRING, w},
    };
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(path.as_os_str()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    anyhow::ensure!(
        result.0 as isize > 32,
        "Windows could not open {}",
        path.display()
    );
    Ok(())
}
pub fn run() -> Result<()> {
    run_with_virtual_camera(false)
}
pub fn run_with_virtual_camera(auto_camera: bool) -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 820.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Open EOS Studio",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            let mut style = (*cc.egui_ctx.style()).clone();
            style.spacing.item_spacing = egui::vec2(12.0, 10.0);
            cc.egui_ctx.set_style(style);
            let mut studio = Studio::new();
            studio.auto_camera = auto_camera;
            Ok(Box::new(studio))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
struct Studio {
    auto_camera: bool,
    controller: Controller,
    settings: Settings,
    microphone: Microphone,
    folder: String,
    texture: Option<egui::TextureHandle>,
    sequence: u64,
    closing: bool,
    notice: Option<String>,
    show_preview: bool,
    show_focus: bool,
    focus_step: u32,
}
impl Studio {
    fn new() -> Self {
        let settings = Settings::default();
        let folder = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("Recordings")))
            .unwrap_or_else(|| PathBuf::from("Recordings"));
        Self {
            auto_camera: false,
            controller: Controller::new(false, settings),
            settings,
            microphone: Microphone::Default,
            folder: folder.to_string_lossy().into(),
            texture: None,
            sequence: 0,
            closing: false,
            notice: None,
            show_preview: true,
            show_focus: true,
            focus_step: 3,
        }
    }
    fn send(&mut self, command: Command) {
        if let Err(e) = self.controller.send(command) {
            self.notice = Some(e.to_string());
        }
    }
}
impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state = self.controller.snapshot();
        if self.auto_camera && state.phase == Phase::Preview {
            self.auto_camera = false;
            self.send(Command::VirtualCamera(true));
        }
        if ctx.input(|i| i.viewport().close_requested()) && state.phase != Phase::Closed {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.closing = true;
            self.controller.shutdown();
        }
        if self.closing && state.phase == Phase::Closed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let preview_visible =
            self.show_preview && !ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if preview_visible
            && state.sequence != self.sequence
            && let Some(frame) = &state.frame
        {
            let image = egui::ColorImage::from_rgb(
                [frame.width as usize, frame.height as usize],
                &frame.rgb,
            );
            if let Some(texture) = &mut self.texture {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.texture =
                    Some(ctx.load_texture("live-camera", image, egui::TextureOptions::LINEAR));
            }
            self.sequence = state.sequence;
        }
        if state.frame.is_none() || !preview_visible {
            self.texture = None;
            self.sequence = 0;
        }
        let adjustable =
            matches!(state.phase, Phase::Preview | Phase::Disconnected) && !self.closing;
        egui::TopBottomPanel::top("heading").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading("Open EOS Studio");
                ui.label(RichText::new("Canon 600D / Rebel T3i").color(Color32::GRAY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if self.closing {
                        "Finishing and closing…"
                    } else {
                        match state.phase {
                            Phase::Disconnected => "Camera disconnected",
                            Phase::Connecting => "Connecting…",
                            Phase::Preview => "Camera connected",
                            Phase::Starting => "Starting recording…",
                            Phase::Recording => "● Recording",
                            Phase::Saving => "Saving MP4…",
                            Phase::Closed => "Closed",
                        }
                    };
                    ui.label(
                        RichText::new(label).color(if state.phase == Phase::Recording {
                            Color32::LIGHT_RED
                        } else {
                            Color32::LIGHT_GREEN
                        }),
                    );
                });
            });
            ui.add_space(8.0);
        });
        egui::SidePanel::right("controls")
            .exact_width(300.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(12.0);
                    ui.heading("Framing");
                    ui.add_enabled_ui(adjustable, |ui| {
                        let old = self.settings;
                        ui.horizontal(|ui| {
                            if ui.button("Landscape").clicked() {
                                self.settings.rotation = 0;
                                self.settings.aspect = Aspect::Landscape;
                            }
                            if ui.button("Portrait / mobile").clicked() {
                                self.settings.rotation = 90;
                                self.settings.aspect = Aspect::Portrait;
                            }
                        });
                        if ui.button("Rotate 90° clockwise").clicked() {
                            self.settings.rotation = (self.settings.rotation + 90) % 360;
                            if self.settings.aspect != Aspect::Native {
                                self.settings.aspect = if self.settings.rotation.is_multiple_of(180) { Aspect::Landscape } else { Aspect::Portrait };
                            }
                        }
                        ui.small(format!("Camera rotation: {}° clockwise", self.settings.rotation));
                        egui::ComboBox::from_label("Output quality")
                            .selected_text(if self.settings.full_hd { "1080 output (upscaled)" } else { "Native USB (less processing)" })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.settings.full_hd, true, "1080 output (upscaled)");
                                ui.selectable_value(&mut self.settings.full_hd, false, "Native USB (less processing)");
                            });
                        egui::ComboBox::from_label("Fine rotation")
                            .selected_text(format!("{}° clockwise", self.settings.rotation))
                            .show_ui(ui, |ui| {
                                for degrees in [0, 90, 180, 270] {
                                    ui.selectable_value(
                                        &mut self.settings.rotation,
                                        degrees,
                                        format!("{degrees}° clockwise"),
                                    );
                                }
                            });
                        egui::ComboBox::from_label("Crop")
                            .selected_text(match self.settings.aspect {
                                Aspect::Native => "Full camera image",
                                Aspect::Portrait => "Portrait 9:16",
                                Aspect::Landscape => "Landscape 16:9",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.settings.aspect,
                                    Aspect::Native,
                                    "Full camera image",
                                );
                                ui.selectable_value(
                                    &mut self.settings.aspect,
                                    Aspect::Portrait,
                                    "Portrait 9:16",
                                );
                                ui.selectable_value(
                                    &mut self.settings.aspect,
                                    Aspect::Landscape,
                                    "Landscape 16:9",
                                );
                            });
                        if old != self.settings {
                            self.send(Command::Configure(self.settings));
                        }
                    });
                    if let Some(frame) = &state.frame {
                        ui.label(format!(
                            "{} × {}  •  {:.1} fps",
                            frame.width, frame.height, state.fps
                        ));
                    }
                    if state.source_size.0 > 0 { ui.small(format!("Camera USB: {} × {}. 1080 output is upscaled.", state.source_size.0, state.source_size.1)); }
                    ui.small("Preview, recording and virtual camera share the same framing. Reset rotation in OBS/TikTok to 0°.");
                    ui.checkbox(&mut self.show_preview, "Show Studio preview");
                    ui.small("Hide preview to reduce display work while streaming. Keep Studio open.");
                    ui.separator();
                    ui.heading("Use in OBS / other apps");
                    if ui.add_enabled(state.frame.is_some() && !self.closing, egui::Button::new(if state.virtual_camera { "Stop virtual camera" } else { "Start virtual camera" })).clicked() {
                        self.send(Command::VirtualCamera(!state.virtual_camera));
                    }
                    if state.virtual_camera { ui.colored_label(Color32::LIGHT_GREEN, "Open EOS Camera is available"); }
                    ui.small("Keep Studio open. Select Open EOS Camera in the other app and choose your microphone there.");
                    ui.separator();
                    ui.heading("Microphone");
                    ui.add_enabled_ui(adjustable, |ui| {
                        let selected = match &self.microphone {
                            Microphone::Off => "No audio",
                            Microphone::Default => "Windows default microphone",
                            Microphone::Named(name) => name,
                        };
                        egui::ComboBox::from_id_salt("microphone")
                            .width(265.0)
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.microphone,
                                    Microphone::Off,
                                    "No audio",
                                );
                                ui.selectable_value(
                                    &mut self.microphone,
                                    Microphone::Default,
                                    "Windows default microphone",
                                );
                                for name in &state.microphones {
                                    ui.selectable_value(
                                        &mut self.microphone,
                                        Microphone::Named(name.clone()),
                                        name,
                                    );
                                }
                            });
                    });
                    ui.add(
                        egui::ProgressBar::new(state.mic_level.clamp(0.0, 1.0))
                            .text("Mic level while recording"),
                    );
                    ui.separator();
                    ui.heading("Recording");
                    ui.label("Save folder");
                    ui.add_enabled(
                        adjustable,
                        egui::TextEdit::singleline(&mut self.folder).desired_width(270.0),
                    );
                    if ui.button("Open recordings folder").clicked() {
                        let folder = PathBuf::from(&self.folder);
                        if let Err(e) = std::fs::create_dir_all(&folder)
                            .map_err(anyhow::Error::from)
                            .and_then(|_| open_path(&folder))
                        {
                            self.notice = Some(e.to_string());
                        }
                    }
                    ui.add_space(8.0);
                    if ui
                        .add_enabled(
                            state.phase == Phase::Preview && !self.closing,
                            egui::Button::new(RichText::new("●  Start recording").size(18.0))
                                .fill(Color32::from_rgb(130, 35, 45))
                                .min_size(egui::vec2(270.0, 44.0)),
                        )
                        .clicked()
                    {
                        let id = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis();
                        let path = PathBuf::from(&self.folder).join(format!("EOS-{id}.mp4"));
                        self.notice = None;
                        self.send(Command::Start(path, self.microphone.clone()));
                    }
                    if ui
                        .add_enabled(
                            state.phase == Phase::Recording && !self.closing,
                            egui::Button::new("■  Stop and save").min_size(egui::vec2(270.0, 38.0)),
                        )
                        .clicked()
                    {
                        self.send(Command::Stop);
                    }
                    ui.label(format!(
                        "{:02}:{:02}",
                        state.elapsed as u64 / 60,
                        state.elapsed as u64 % 60
                    ));
                    ui.small("MP4 · H.264 video · AAC microphone audio");
                    if let Some(path) = &state.last_file {
                        ui.add_space(8.0);
                        ui.label("Last saved recording");
                        ui.small(path.file_name().unwrap_or_default().to_string_lossy());
                        if ui.button("Play last recording").clicked()
                            && let Err(e) = open_path(path)
                        {
                            self.notice = Some(e.to_string());
                        }
                    }
                    ui.separator();
                    ui.add_enabled_ui(state.phase == Phase::Preview && !self.closing, |ui| {
                        ui.heading("Focus");
                        egui::ComboBox::from_label("Lens step")
                            .selected_text(match self.focus_step { 1 => "Small", 2 => "Medium", _ => "Large" })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.focus_step, 1, "Small");
                                ui.selectable_value(&mut self.focus_step, 2, "Medium");
                                ui.selectable_value(&mut self.focus_step, 3, "Large");
                            });
                        ui.horizontal(|ui| {
                            if ui.button("Near").clicked() {
                                self.send(Command::Focus { far: false, step: self.focus_step });
                            }
                            if ui.button("Far").clicked() {
                                self.send(Command::Focus { far: true, step: self.focus_step });
                            }
                            if ui.add_enabled(!state.autofocus_active, egui::Button::new("Autofocus")).clicked() {
                                self.send(Command::Autofocus);
                            }
                        });
                    });
                    ui.small("Focus controls require a compatible lens in AF mode.");
                    if let Some(method) = state.af_method {
                        ui.small(match method {
                            0 => "Camera AF method: Quick (may interrupt live view)",
                            1 => "Camera AF method: Live (single focus area)",
                            2 => "Camera AF method: Live face detection",
                            _ => "Camera AF method: camera-specific",
                        });
                    }
                    if state.focus_mode == Some(3) { ui.colored_label(Color32::YELLOW, "Lens is reporting MF. Switch the lens to AF."); }
                    if !state.focus_message.is_empty() { ui.small(&state.focus_message); }
                    ui.checkbox(&mut self.show_focus, "Focus markers (experimental)");
                    ui.small(if state.focus_points.is_empty() { "No current focus-point positions reported by the camera." } else { "Camera-reported points: yellow = selected, not confirmed focus lock. Live-view alignment is experimental. Preview only." });
                    if ui
                        .add_enabled(
                            state.phase == Phase::Disconnected && !self.closing,
                            egui::Button::new("Reconnect camera"),
                        )
                        .clicked()
                    {
                        self.send(Command::Connect);
                    }
                });
            });
        egui::TopBottomPanel::bottom("message").show(ctx, |ui| {
            if let Some(error) = self.notice.as_ref().or(state.error.as_ref()) {
                ui.colored_label(Color32::LIGHT_RED, error);
            } else {
                ui.small(
                    "Local recording preview • Select the separate microphone you want to record.",
                );
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.show_preview {
                ui.centered_and_justified(|ui| { ui.label("Studio preview hidden\nCamera output and recording continue."); });
            } else if let Some(texture) = &self.texture {
                let available = ui.available_size();
                let size = texture.size_vec2();
                let scale = (available.x / size.x).min(available.y / size.y);
                let image_rect = egui::Rect::from_center_size(ui.max_rect().center(), size * scale);
                ui.painter().image(texture.id(), image_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), Color32::WHITE);
                    if self.show_focus {
                        for point in &state.focus_points {
                            let [x0,y0,x1,y1] = point.rect;
                            let rect = egui::Rect::from_min_max(
                                image_rect.min + image_rect.size() * egui::vec2(x0,y0),
                                image_rect.min + image_rect.size() * egui::vec2(x1,y1));
                            ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(2.0, if point.selected { Color32::YELLOW } else { Color32::GRAY }), egui::StrokeKind::Inside);
                        }
                    }
            } else {
                ui.centered_and_justified(|ui| { ui.label("Switch on the T3i and connect USB.\nUse Reconnect camera if it has gone to sleep."); });
            }
        });
        ctx.request_repaint_after(Duration::from_millis(if preview_visible {
            50
        } else {
            250
        }));
    }
}
