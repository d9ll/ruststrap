use eframe::egui::{self, RichText};

use crate::fastflags::{self, MSAA_MODES, TEXTURE_QUALITY_LEVELS};

use super::{
    card, muted, open_folder, open_url, page_header, secondary_button, theme, RuststrapApp,
    Screen, SettingsTab,
};

impl RuststrapApp {
    pub fn draw_settings(&mut self, ui: &mut egui::Ui, tab: &mut SettingsTab) {
        let mut go_back = false;

        ui.horizontal(|ui| {
            if ui.button("← Back").clicked() {
                go_back = true;
            }
            ui.label(RichText::new("Settings").size(24.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(muted(format!(
                    "Ruststrap v{}",
                    crate::paths::PROJECT_VERSION
                )));
            });
        });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for (value, label) in SettingsTab::ALL {
                if ui
                    .selectable_label(*tab == *value, RichText::new(*label).size(14.0))
                    .clicked()
                {
                    *tab = *value;
                }
            }
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                match *tab {
                    SettingsTab::General => general_page(self, ui),
                    SettingsTab::FastFlags => fast_flags_page(self, ui),
                    SettingsTab::Mods => mods_page(self, ui),
                    SettingsTab::Integrations => integrations_page(self, ui),
                    SettingsTab::Advanced => advanced_page(self, ui),
                    SettingsTab::About => about_page(self, ui),
                }
                ui.add_space(20.0);
            });

        if go_back {
            self.next_screen = Some(Screen::Menu);
        }
    }
}

fn general_page(app: &mut RuststrapApp, ui: &mut egui::Ui) {
    let Some(env) = app.env.as_mut() else {
        return;
    };
    let mut changed = false;

    page_header(ui, "General", "Installation, updates and application behaviour.");

    card(ui, |ui| {
        ui.label(RichText::new("Installation").size(17.0).strong());
        ui.add_space(6.0);
        ui.label(muted(env.paths.base.to_string_lossy().to_string()));
        ui.add_space(8.0);
        if ui.button("Open install folder").clicked() {
            open_folder(&env.paths.base);
        }
    });

    ui.add_space(10.0);

    card(ui, |ui| {
        ui.label(RichText::new("Updates").size(17.0).strong());
        ui.add_space(6.0);
        changed |= ui
            .checkbox(&mut env.settings.check_for_updates, "Check for updates")
            .changed();
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Roblox channel:");
            changed |= ui
                .add(
                    egui::TextEdit::singleline(&mut env.settings.channel)
                        .desired_width(180.0)
                        .hint_text("production"),
                )
                .changed();
        });
        ui.label(muted(
            "Leave this as \"production\" unless you know the channel you need.",
        ));
    });

    ui.add_space(10.0);

    card(ui, |ui| {
        ui.label(RichText::new("Behaviour").size(17.0).strong());
        ui.add_space(6.0);
        changed |= ui
            .checkbox(
                &mut env.settings.confirm_launches,
                "Ask before closing Roblox to install an update",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut env.settings.use_fast_flag_manager,
                "Use the fast flag manager (ClientAppSettings.json)",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut env.settings.debug_disable_version_package_cleanup,
                "Keep downloaded packages (debug)",
            )
            .changed();
    });

    ui.add_space(10.0);

    card(ui, |ui| {
        ui.label(RichText::new("Appearance").size(17.0).strong());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Theme:");
            let previous = env.settings.theme.clone();
            egui::ComboBox::from_id_source("theme_combo")
                .selected_text(env.settings.theme.clone())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut env.settings.theme, "Dark".into(), "Dark");
                    ui.selectable_value(&mut env.settings.theme, "Light".into(), "Light");
                });
            if env.settings.theme != previous {
                theme::apply(ui.ctx(), &env.settings.theme);
                changed = true;
            }
        });
    });

    if changed {
        env.save_settings();
    }
}

fn fast_flags_page(app: &mut RuststrapApp, ui: &mut egui::Ui) {
    page_header(
        ui,
        "Fast flags",
        "Advanced Roblox client configuration. Changes apply on the next launch.",
    );

    let mut filter = app.flag_filter.clone();
    let mut new_key = app.new_flag_key.clone();
    let mut new_value = app.new_flag_value.clone();
    let mut add_clicked = false;
    let mut reset_flags = false;
    let mut reset_done = false;

    if let Some(env) = app.env.as_mut() {
        if !env.settings.use_fast_flag_manager {
            ui.label(
                RichText::new(
                    "The fast flag manager is disabled in General settings, so these flags will not be applied.",
                )
                .color(theme::DANGER),
            );
            ui.add_space(6.0);
        }

        card(ui, |ui| {
            ui.label(RichText::new("Presets").size(17.0).strong());
            ui.add_space(6.0);

            let mut manual_fullscreen =
                env.flags.get("FFlagHandleAltEnterFullscreenManually") == Some("True");
            if ui
                .checkbox(&mut manual_fullscreen, "Manual fullscreen (Alt+Enter)")
                .changed()
            {
                env.flags.set(
                    "FFlagHandleAltEnterFullscreenManually",
                    Some(if manual_fullscreen { "True" } else { "False" }.into()),
                );
                env.save_flags();
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("MSAA:");
                let current = fastflags::current_msaa(&env.flags).to_string();
                let mut selected = current.clone();
                egui::ComboBox::from_id_source("msaa_combo")
                    .selected_text(current.clone())
                    .show_ui(ui, |ui| {
                        for (name, _) in MSAA_MODES {
                            ui.selectable_value(&mut selected, (*name).to_string(), *name);
                        }
                    });
                if selected != current {
                    let value = MSAA_MODES
                        .iter()
                        .find(|(name, _)| *name == selected)
                        .and_then(|(_, value)| *value);
                    env.flags.set_preset("Rendering.MSAA", value);
                    env.save_flags();
                }
            });

            ui.horizontal(|ui| {
                ui.label("Texture quality:");
                let current = fastflags::current_texture_quality(&env.flags).to_string();
                let mut selected = current.clone();
                egui::ComboBox::from_id_source("texture_quality_combo")
                    .selected_text(current.clone())
                    .show_ui(ui, |ui| {
                        for (name, _) in TEXTURE_QUALITY_LEVELS {
                            ui.selectable_value(&mut selected, (*name).to_string(), *name);
                        }
                    });
                if selected != current {
                    let value = TEXTURE_QUALITY_LEVELS
                        .iter()
                        .find(|(name, _)| *name == selected)
                        .and_then(|(_, value)| *value);
                    if let Some(value) = value {
                        env.flags
                            .set_preset("Rendering.TextureQuality.Level", Some(value));
                        env.flags.set_preset(
                            "Rendering.TextureQuality.OverrideEnabled",
                            Some("True"),
                        );
                    } else {
                        env.flags.set_preset("Rendering.TextureQuality", None);
                    }
                    env.save_flags();
                }
            });
        });

        ui.add_space(10.0);

        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Flags").size(17.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Reset to defaults").clicked() {
                        reset_flags = true;
                    }
                });
            });

            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut filter)
                    .hint_text("Search flags...")
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(6.0);
            let filter_lower = filter.to_lowercase();
            let keys: Vec<String> = env
                .flags
                .values
                .keys()
                .filter(|key| filter_lower.is_empty() || key.to_lowercase().contains(&filter_lower))
                .cloned()
                .collect();

            let mut changed = false;
            let mut remove: Option<String> = None;

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if keys.is_empty() {
                        ui.label(muted("No flags match this filter."));
                    }
                    for key in keys {
                        let mut value = env.flags.values.get(&key).cloned().unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [430.0, 24.0],
                                egui::Label::new(
                                    RichText::new(&key).monospace().size(12.5),
                                )
                                .truncate(),
                            );
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut value)
                                        .desired_width(180.0)
                                        .font(egui::TextStyle::Monospace),
                                )
                                .changed()
                            {
                                env.flags.values.insert(key.clone(), value.clone());
                                changed = true;
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("✕").color(theme::DANGER),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                remove = Some(key.clone());
                            }
                        });
                    }
                });

            if let Some(key) = remove {
                env.flags.values.remove(&key);
                changed = true;
            }
            if changed {
                env.save_flags();
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut new_key)
                        .hint_text("FFlagName")
                        .desired_width(300.0)
                        .font(egui::TextStyle::Monospace),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut new_value)
                        .hint_text("value")
                        .desired_width(150.0)
                        .font(egui::TextStyle::Monospace),
                );
                if ui.button("Add flag").clicked() {
                    add_clicked = true;
                }
            });
        });

        if reset_flags {
            env.flags.values.clear();
            env.flags.set("FFlagHandleAltEnterFullscreenManually", Some("False".into()));
            env.save_flags();
            reset_done = true;
        }
    }

    if reset_done {
        app.show_toast("Fast flags reset to defaults");
    }

    app.flag_filter = filter;
    app.new_flag_key = new_key;
    app.new_flag_value = new_value;

    if add_clicked {
        let key = app.new_flag_key.trim().to_string();
        let value = app.new_flag_value.trim().to_string();
        if !key.is_empty() {
            if let Some(env) = app.env.as_mut() {
                env.flags.set(&key, Some(value));
                env.save_flags();
            }
            app.new_flag_key.clear();
            app.new_flag_value.clear();
            app.show_toast("Fast flag added");
        }
    }
}

fn mods_page(app: &mut RuststrapApp, ui: &mut egui::Ui) {
    let Some(env) = app.env.as_mut() else {
        return;
    };
    let mut open_mods = false;
    let mut open_client_settings = false;
    let mut open_font_folder = false;

    page_header(
        ui,
        "Mods",
        "Drop replacement files into the Modifications folder. They are copied over the Roblox installation on every launch.",
    );

    card(ui, |ui| {
        ui.label(RichText::new("Modifications folder").size(17.0).strong());
        ui.add_space(6.0);
        ui.label(muted(env.paths.modifications.to_string_lossy().to_string()));
        ui.add_space(8.0);
        if ui.button("Open Modifications").clicked() {
            open_mods = true;
        }
    });

    ui.add_space(10.0);

    card(ui, |ui| {
        ui.label(RichText::new("Custom font").size(17.0).strong());
        ui.add_space(6.0);
        ui.label(muted(
            "Place a font at Modifications\\content\\fonts\\CustomFont.ttf to replace the Roblox UI font.",
        ));
        ui.add_space(6.0);
        if env.paths.custom_font().is_file() {
            ui.label(
                RichText::new("Custom font detected")
                    .color(theme::SUCCESS)
                    .strong(),
            );
        } else {
            ui.label(muted("No custom font installed."));
        }
        ui.horizontal(|ui| {
            if ui.button("Open font folder").clicked() {
                open_font_folder = true;
            }
            if ui.button("Open ClientSettings").clicked() {
                open_client_settings = true;
            }
        });
    });

    ui.add_space(10.0);

    card(ui, |ui| {
        ui.label(RichText::new("How it works").size(17.0).strong());
        ui.add_space(6.0);
        ui.label(muted(
            "Files keep their relative paths. For example, Modifications\\content\\sounds\\ \
             replaces content\\sounds\\ in the Roblox version folder. Delete a file from \
             Modifications and the original is restored on the next launch.",
        ));
    });

    if open_mods {
        open_folder(&env.paths.modifications);
    }
    if open_client_settings {
        open_folder(&env.paths.modifications.join("ClientSettings"));
    }
    if open_font_folder {
        open_folder(&env.paths.modifications.join("content\\fonts"));
    }
}

fn integrations_page(app: &mut RuststrapApp, ui: &mut egui::Ui) {
    let Some(env) = app.env.as_mut() else {
        return;
    };
    let mut changed = false;

    page_header(
        ui,
        "Integrations",
        "Activity tracking and Discord Rich Presence.",
    );

    card(ui, |ui| {
        ui.label(RichText::new("Activity tracking").size(17.0).strong());
        ui.add_space(6.0);
        changed |= ui
            .checkbox(
                &mut env.settings.enable_activity_tracking,
                "Track the current game from Roblox logs",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut env.settings.use_discord_rich_presence,
                "Show game activity in Discord Rich Presence",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut env.settings.hide_rpc_buttons,
                "Hide \"View on Roblox\" button in Rich Presence",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut env.settings.show_account_on_rich_presence,
                "Show your Roblox account in Rich Presence",
            )
            .changed();
    });

    if changed {
        env.save_settings();
    }
}

fn advanced_page(app: &mut RuststrapApp, ui: &mut egui::Ui) {
    let Some(env) = app.env.as_mut() else {
        return;
    };
    let mut changed = false;
    let mut open_logs = false;
    let mut open_downloads = false;
    let mut open_versions = false;
    let mut go_uninstall = false;

    page_header(
        ui,
        "Advanced",
        "Maintenance tools and diagnostics. Handle with care.",
    );

    card(ui, |ui| {
        ui.label(RichText::new("Reinstall").size(17.0).strong());
        ui.add_space(6.0);
        changed |= ui
            .checkbox(
                &mut env.state.force_reinstall,
                "Force a full Roblox reinstall on the next launch",
            )
            .changed();
        ui.label(muted(
            "The current version folder will be removed and all packages will be downloaded again.",
        ));
    });

    ui.add_space(10.0);

    card(ui, |ui| {
        ui.label(RichText::new("Folders").size(17.0).strong());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Logs").clicked() {
                open_logs = true;
            }
            if ui.button("Downloads").clicked() {
                open_downloads = true;
            }
            if ui.button("Versions").clicked() {
                open_versions = true;
            }
        });
        ui.add_space(6.0);
        ui.label(muted(
            crate::logger::path()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|| "Log file is not initialized".to_string()),
        ));
    });

    ui.add_space(10.0);

    card(ui, |ui| {
        ui.label(RichText::new("Danger zone").size(17.0).strong());
        ui.add_space(6.0);
        if ui
            .add(
                egui::Button::new(
                    RichText::new("Uninstall Ruststrap").color(theme::DANGER),
                )
                .frame(false),
            )
            .clicked()
        {
            go_uninstall = true;
        }
    });

    if changed {
        env.save_state();
    }
    if open_logs {
        open_folder(&env.paths.logs);
    }
    if open_downloads {
        open_folder(&env.paths.downloads);
    }
    if open_versions {
        open_folder(&env.paths.versions);
    }
    if go_uninstall {
        app.next_screen = Some(Screen::Uninstall);
    }
}

fn about_page(app: &mut RuststrapApp, ui: &mut egui::Ui) {
    page_header(ui, "About Ruststrap", "A native Rust reimplementation of Bloxstrap.");

    card(ui, |ui| {
        ui.label(
            RichText::new(format!("Ruststrap v{}", crate::paths::PROJECT_VERSION))
                .size(20.0)
                .strong()
                .color(theme::ACCENT),
        );
        ui.add_space(6.0);
        ui.label(muted(
            "Ruststrap is an open-source Roblox bootstrapper. It installs and updates \
             the Roblox client, manages fast flags and file modifications, and integrates \
             with Discord Rich Presence.",
        ));
        ui.add_space(10.0);
        ui.label(muted("Built with Rust, egui/eframe and the Roblox deployment API."));
        ui.label(muted("Licensed under the MIT License."));
    });

    ui.add_space(10.0);

    card(ui, |ui| {
        ui.label(RichText::new("Links").size(17.0).strong());
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Roblox website").clicked() {
                open_url("https://www.roblox.com");
            }
            if ui.button("Rust language").clicked() {
                open_url("https://www.rust-lang.org");
            }
            if ui.button("egui").clicked() {
                open_url("https://github.com/emilk/egui");
            }
        });
    });

    ui.add_space(10.0);

    card(ui, |ui| {
        ui.label(RichText::new("Credits").size(17.0).strong());
        ui.add_space(6.0);
        ui.label(muted(
            "The original Bloxstrap project by pizzaboxer and contributors inspired \
             this reimplementation. Roblox is a trademark of Roblox Corporation.",
        ));
        ui.add_space(8.0);
        if secondary_button(ui, "Uninstall Ruststrap").clicked() {
            app.next_screen = Some(Screen::Uninstall);
        }
    });
}
