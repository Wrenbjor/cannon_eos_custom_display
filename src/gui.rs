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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 820.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Open EOS Studio",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            let mut style = (*cc.egui_ctx.style()).clone();
            style.spacing.item_spacing = egui::vec2(12.0, 10.0);
            cc.egui_ctx.set_style(style);
            Ok(Box::new(Studio::new()))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
struct Studio {
    controller: Controller,
    settings: Settings,
    microphone: Microphone,
    folder: String,
    texture: Option<egui::TextureHandle>,
    sequence: u64,
    closing: bool,
    notice: Option<String>,
}
impl Studio {
    fn new() -> Self {
        let settings = Settings::default();
        let folder = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("Recordings")))
            .unwrap_or_else(|| PathBuf::from("Recordings"));
        Self {
            controller: Controller::new(false, settings),
            settings,
            microphone: Microphone::Default,
            folder: folder.to_string_lossy().into(),
            texture: None,
            sequence: 0,
            closing: false,
            notice: None,
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
        if ctx.input(|i| i.viewport().close_requested()) && state.phase != Phase::Closed {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.closing = true;
            self.controller.shutdown();
        }
        if self.closing && state.phase == Phase::Closed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if state.sequence != self.sequence
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
        if state.frame.is_none() {
            self.texture = None;
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
                            Phase::Preview => "Live preview",
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
                        let old = (self.settings.rotation, self.settings.aspect);
                        egui::ComboBox::from_label("Rotation")
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
                        if old != (self.settings.rotation, self.settings.aspect) {
                            self.send(Command::Configure(self.settings));
                        }
                    });
                    if let Some(frame) = &state.frame {
                        ui.label(format!(
                            "{} × {}  •  {:.1} fps",
                            frame.width, frame.height, state.fps
                        ));
                    }
                    ui.small("The saved video uses this exact orientation and crop. No upscaling.");
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
                        ui.horizontal(|ui| {
                            if ui.button("Near").clicked() {
                                self.send(Command::Focus(false));
                            }
                            if ui.button("Far").clicked() {
                                self.send(Command::Focus(true));
                            }
                            if ui.button("Autofocus").clicked() {
                                self.send(Command::Autofocus);
                            }
                        });
                    });
                    ui.small("Focus controls require a compatible lens in AF mode.");
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
            if let Some(texture) = &self.texture {
                let available = ui.available_size();
                let size = texture.size_vec2();
                let scale = (available.x / size.x).min(available.y / size.y);
                ui.centered_and_justified(|ui| { ui.add(egui::Image::new(texture).fit_to_exact_size(size * scale)); });
            } else {
                ui.centered_and_justified(|ui| { ui.label("Switch on the T3i and connect USB.\nUse Reconnect camera if it has gone to sleep."); });
            }
        });
        ctx.request_repaint_after(Duration::from_millis(33));
    }
}
