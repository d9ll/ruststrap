use std::path::PathBuf;

use eframe::egui::{self, RichText};

use crate::installer;

use super::{
    card, muted, primary_button, theme, RuststrapApp, Screen,
};

impl RuststrapApp {
    pub fn draw_installer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.installer.installing {
            let result = self
                .installer
                .receiver
                .as_ref()
                .and_then(|receiver| match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Installer stopped unexpectedly".to_string()))
                    }
                });

            if let Some(result) = result {
                self.installer.installing = false;
                self.installer.receiver = None;
                match result {
                    Ok(()) => {
                        let location = PathBuf::from(&self.installer.location);
                        self.env = Some(super::Env::load(location));
                        if let Some(mode) = self.pending_launch.take() {
                            self.start_launch(mode);
                        } else {
                            self.next_screen = Some(Screen::Menu);
                        }
                        self.show_toast("Ruststrap installed successfully");
                    }
                    Err(error) => {
                        self.installer.error = Some(error);
                    }
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Welcome to RUSTSTRAP")
                        .size(36.0)
                        .strong()
                        .color(theme::ACCENT),
                );
                ui.label(
                    RichText::new(
                        "A fast, native Roblox bootstrapper. Choose where to install it.",
                    )
                    .color(theme::MUTED),
                );
                ui.add_space(22.0);
            });

            ui.horizontal(|ui| {
                ui.add_space(80.0);
                ui.vertical(|ui| {
                    card(ui, |ui| {
                        ui.label(RichText::new("Install location").size(17.0).strong());
                        ui.add_space(6.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.installer.location)
                                .desired_width(f32::INFINITY),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("Use default").clicked() {
                                self.installer.location = crate::paths::default_install_location()
                                    .to_string_lossy()
                                    .to_string();
                            }
                            if ui.button("Browse...").clicked() {
                                if let Some(path) = crate::util::pick_folder(
                                    &PathBuf::from(&self.installer.location),
                                ) {
                                    self.installer.location =
                                        path.to_string_lossy().to_string();
                                }
                            }
                        });
                    });

                    ui.add_space(10.0);

                    card(ui, |ui| {
                        ui.label(RichText::new("Options").size(17.0).strong());
                        ui.add_space(6.0);
                        ui.checkbox(
                            &mut self.installer.create_shortcuts,
                            "Create desktop and Start menu shortcuts",
                        );
                        ui.checkbox(
                            &mut self.installer.enable_analytics,
                            "Enable anonymous usage statistics",
                        );
                        ui.add_space(4.0);
                        ui.label(muted(
                            "Ruststrap does not collect any telemetry in this build. \
                             The option is kept for Bloxstrap compatibility.",
                        ));
                    });

                    ui.add_space(14.0);

                    if self.installer.installing {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Installing Ruststrap...");
                        });
                    } else if primary_button(ui, "Install Ruststrap").clicked() {
                        self.begin_install();
                    }

                    if let Some(error) = self.installer.error.as_ref() {
                        ui.add_space(8.0);
                        ui.label(RichText::new(error).color(theme::DANGER));
                    }

                    ui.add_space(10.0);
                    ui.label(muted(format!(
                        "Ruststrap v{} will be installed for the current user only.",
                        crate::paths::PROJECT_VERSION
                    )));
                });
                ui.add_space(80.0);
            });
        });
    }

    fn begin_install(&mut self) {
        let location = PathBuf::from(self.installer.location.trim());
        if let Err(error) = installer::validate_location(&location) {
            self.installer.error = Some(error.to_string());
            return;
        }

        self.installer.error = None;
        self.installer.installing = true;

        let shortcuts = self.installer.create_shortcuts;
        let analytics = self.installer.enable_analytics;
        let (sender, receiver) = std::sync::mpsc::channel();
        self.installer.receiver = Some(receiver);

        std::thread::spawn(move || {
            let result = installer::do_install(&location, shortcuts, analytics)
                .map_err(|error| format!("{error:#}"));
            let _ = sender.send(result);
        });
    }
}
