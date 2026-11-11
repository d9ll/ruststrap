use eframe::egui::{self, RichText};

use crate::cli::LaunchMode;

use super::{card, muted, open_folder, primary_button, secondary_button, theme, LaunchState, RuststrapApp, Screen};

impl RuststrapApp {
    pub fn draw_launch(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        state: &mut LaunchState,
    ) {
        if state.result.is_none() {
            match state.receiver.try_recv() {
                Ok(result) => {
                    state.result = Some(result);
                    if let Some(env) = self.env.as_mut() {
                        env.reload();
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    state.result = Some(Err("Bootstrapper process stopped".into()));
                }
            }
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        let snapshot = state.progress.snapshot();
        let product = match self.launch_request.launch_mode {
            LaunchMode::Studio | LaunchMode::StudioAuth => "Roblox Studio",
            _ => "Roblox",
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(50.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(format!("Launching {product}"))
                        .size(34.0)
                        .strong()
                        .color(theme::TEXT),
                );
                ui.label(muted("Ruststrap is making sure your client is ready."));
                ui.add_space(26.0);
            });

            ui.horizontal(|ui| {
                ui.add_space(110.0);
                ui.vertical(|ui| {
                    card(ui, |ui| {
                        ui.label(
                            RichText::new(if snapshot.status.is_empty() {
                                "Preparing..."
                            } else {
                                &snapshot.status
                            })
                            .size(17.0)
                            .strong(),
                        );

                        if !snapshot.detail.is_empty() {
                            ui.label(muted(&snapshot.detail));
                        }

                        ui.add_space(10.0);

                        if let Some(Err(error)) = state.result.as_ref() {
                            ui.label(RichText::new(error).color(theme::DANGER));
                            ui.add_space(8.0);
                            if secondary_button(ui, "Back to menu").clicked() {
                                self.next_screen = Some(Screen::Menu);
                            }
                            if ui.button("Open logs").clicked() {
                                if let Some(env) = &self.env {
                                    open_folder(&env.paths.logs);
                                }
                            }
                        } else if let Some(Ok(())) = state.result.as_ref() {
                            ui.label(
                                RichText::new(format!("{product} is starting."))
                                    .color(theme::SUCCESS)
                                    .strong(),
                            );
                            ui.add_space(8.0);
                            if primary_button(ui, "Close").clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                            if ui.button("Open logs").clicked() {
                                if let Some(env) = &self.env {
                                    open_folder(&env.paths.logs);
                                }
                            }
                        } else {
                            if snapshot.total > 0 {
                                ui.add(
                                    egui::ProgressBar::new(state.progress.fraction())
                                        .show_percentage()
                                        .desired_height(16.0),
                                );
                                ui.add_space(4.0);
                                ui.label(muted(format!(
                                    "{} / {}",
                                    crate::util::format_bytes(snapshot.downloaded),
                                    crate::util::format_bytes(snapshot.total)
                                )));
                            } else {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(muted("Working..."));
                                });
                            }

                            ui.add_space(8.0);
                            ui.label(muted(format!(
                                "Elapsed: {}s",
                                state.started.elapsed().as_secs()
                            )));

                            ui.add_space(10.0);
                            if ui.button("Cancel").clicked() {
                                state.progress.cancel();
                                state.progress.set_status("Cancelling...");
                            }
                        }
                    });
                });
                ui.add_space(110.0);
            });
        });
    }
}
