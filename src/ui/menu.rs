use eframe::egui::{self, RichText};

use crate::cli::LaunchMode;

use super::{
    card, muted, open_folder, primary_button, secondary_button, theme, RuststrapApp, Screen,
    SettingsTab,
};

impl RuststrapApp {
    pub fn draw_menu(&mut self, ui: &mut egui::Ui) {
        let mut play = false;
        let mut studio = false;
        let mut settings = false;
        let mut about = false;
        let mut uninstall = false;
        let mut open_install = false;
        let mut open_logs = false;

        let (player_version, studio_version, base, channel) = match self.env.as_ref() {
            Some(env) => (
                env.player_version().to_string(),
                env.studio_version().to_string(),
                env.paths.base.to_string_lossy().to_string(),
                env.settings.channel.clone(),
            ),
            None => (String::new(), String::new(), String::new(), String::new()),
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(28.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("RUSTSTRAP")
                        .size(46.0)
                        .strong()
                        .color(theme::ACCENT),
                );
                ui.label(
                    RichText::new("A fast, native Roblox bootstrapper written in Rust")
                        .color(theme::MUTED),
                );
                ui.add_space(26.0);
            });

            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.columns(2, |columns| {
                    columns[0].vertical(|ui| {
                        card(ui, |ui| {
                            ui.label(RichText::new("Launch").size(18.0).strong());
                            ui.add_space(4.0);
                            ui.label(muted("Start the Roblox player or Studio."));
                            ui.add_space(12.0);

                            if primary_button(ui, "Play Roblox").clicked() {
                                play = true;
                            }
                            ui.add_space(6.0);
                            if secondary_button(ui, "Open Roblox Studio").clicked() {
                                studio = true;
                            }
                        });

                        ui.add_space(12.0);

                        card(ui, |ui| {
                            ui.label(RichText::new("Installation").size(18.0).strong());
                            ui.add_space(8.0);
                            ui.label(muted(format!("Channel: {channel}")));
                            ui.label(muted(format!(
                                "Player: {}",
                                if player_version.is_empty() {
                                    "not installed".to_string()
                                } else {
                                    player_version
                                }
                            )));
                            ui.label(muted(format!(
                                "Studio: {}",
                                if studio_version.is_empty() {
                                    "not installed".to_string()
                                } else {
                                    studio_version
                                }
                            )));
                            ui.add_space(8.0);
                            ui.label(muted(base));
                        });
                    });

                    columns[1].vertical(|ui| {
                        card(ui, |ui| {
                            ui.label(RichText::new("Tools").size(18.0).strong());
                            ui.add_space(10.0);
                            if secondary_button(ui, "Settings").clicked() {
                                settings = true;
                            }
                            ui.add_space(6.0);
                            if secondary_button(ui, "About Ruststrap").clicked() {
                                about = true;
                            }
                            ui.add_space(6.0);
                            if secondary_button(ui, "Open install folder").clicked() {
                                open_install = true;
                            }
                            ui.add_space(6.0);
                            if secondary_button(ui, "Open logs").clicked() {
                                open_logs = true;
                            }
                            ui.add_space(12.0);
                            ui.separator();
                            ui.add_space(8.0);
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Uninstall Ruststrap")
                                            .color(theme::DANGER),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                uninstall = true;
                            }
                        });

                        ui.add_space(12.0);

                        card(ui, |ui| {
                            ui.label(RichText::new("Tip").size(18.0).strong());
                            ui.add_space(6.0);
                            ui.label(muted(
                                "Ruststrap registers the roblox:// and roblox-player:// \
                                 protocols. Launching a game from the Roblox website will \
                                 use Ruststrap automatically.",
                            ));
                        });
                    });
                });
                ui.add_space(24.0);
            });

            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(muted(format!(
                    "Ruststrap v{} · MIT License",
                    crate::paths::PROJECT_VERSION
                )));
            });
            ui.add_space(20.0);
        });

        if play {
            self.start_launch(LaunchMode::Player);
        } else if studio {
            self.start_launch(LaunchMode::Studio);
        } else if settings {
            self.next_screen = Some(Screen::Settings(SettingsTab::General));
        } else if about {
            self.next_screen = Some(Screen::Settings(SettingsTab::About));
        } else if uninstall {
            self.next_screen = Some(Screen::Uninstall);
        } else if open_install {
            if let Some(env) = &self.env {
                open_folder(&env.paths.base);
            }
        } else if open_logs {
            if let Some(env) = &self.env {
                open_folder(&env.paths.logs);
            }
        }
    }
}
