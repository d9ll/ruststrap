use eframe::egui::{self, RichText};

use crate::installer;

use super::{
    card, danger_button, muted, secondary_button, theme, RuststrapApp, Screen,
};

impl RuststrapApp {
    pub fn draw_uninstall(&mut self, ui: &mut egui::Ui) {
        let mut back = false;
        let mut confirm = false;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(50.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Uninstall Ruststrap")
                        .size(34.0)
                        .strong()
                        .color(theme::DANGER),
                );
                ui.label(muted(
                    "This removes Ruststrap and its Roblox versions. Roblox itself is not removed.",
                ));
                ui.add_space(24.0);
            });

            ui.horizontal(|ui| {
                ui.add_space(110.0);
                ui.vertical(|ui| {
                    card(ui, |ui| {
                        ui.label(
                            RichText::new(
                                "Your settings, fast flags and modifications can be kept \
                                 so a future installation keeps your configuration.",
                            )
                            .color(theme::TEXT),
                        );
                        ui.add_space(8.0);
                        ui.checkbox(&mut self.keep_data, "Keep settings and modifications");
                        ui.add_space(12.0);
                        if danger_button(ui, "Uninstall Ruststrap").clicked() {
                            confirm = true;
                        }
                        ui.add_space(6.0);
                        if secondary_button(ui, "Cancel").clicked() {
                            back = true;
                        }

                        if let Some(error) = self.uninstall_error.as_ref() {
                            ui.add_space(8.0);
                            ui.label(RichText::new(error).color(theme::DANGER));
                        }
                    });
                });
                ui.add_space(110.0);
            });
        });

        if back {
            self.next_screen = Some(if self.env.is_some() {
                Screen::Menu
            } else {
                Screen::Installer
            });
        }

        if confirm {
            match installer::do_uninstall(self.keep_data) {
                Ok(()) => {
                    self.env = None;
                    self.uninstall_error = None;
                    self.next_screen = Some(Screen::Uninstalled);
                }
                Err(error) => {
                    self.uninstall_error = Some(format!("{error:#}"));
                }
            }
        }
    }

    pub fn draw_uninstalled(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(120.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("Ruststrap has been uninstalled")
                    .size(30.0)
                    .strong()
                    .color(theme::SUCCESS),
            );
            ui.add_space(6.0);
            ui.label(muted(
                "You can close this window. Your Roblox installation was left untouched.",
            ));
            ui.add_space(20.0);
            if ui.button("Close").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}
