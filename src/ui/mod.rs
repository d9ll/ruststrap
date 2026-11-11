mod installer_ui;
mod launch;
mod menu;
mod settings;
mod theme;
mod uninstall;

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use eframe::egui::{self, Color32, RichText};
use eframe::CreationContext;

use crate::bootstrapper::LaunchRequest;
use crate::cli::{LaunchMode, LaunchSettings};
use crate::config::{ConfigStore, DistributionState, Settings, State};
use crate::fastflags::FastFlags;
use crate::installer;
use crate::paths::Paths;
use crate::progress::Progress;

pub struct Env {
    pub paths: Paths,
    pub store: ConfigStore,
    pub settings: Settings,
    pub state: State,
    pub player_state: DistributionState,
    pub studio_state: DistributionState,
    pub flags: FastFlags,
}

impl Env {
    pub fn load(base: PathBuf) -> Self {
        let store = ConfigStore::new(base.clone());
        let paths = Paths::new(base);
        let settings = store.load_settings();
        let state = store.load_state();
        let player_state = store.load_player_state();
        let studio_state = store.load_studio_state();
        let flags = FastFlags::load(paths.client_settings());
        Self {
            paths,
            store,
            settings,
            state,
            player_state,
            studio_state,
            flags,
        }
    }

    pub fn reload(&mut self) {
        self.settings = self.store.load_settings();
        self.state = self.store.load_state();
        self.player_state = self.store.load_player_state();
        self.studio_state = self.store.load_studio_state();
        self.flags = FastFlags::load(self.paths.client_settings());
    }

    pub fn save_settings(&self) {
        self.store.save_settings(&self.settings);
    }

    pub fn save_state(&self) {
        self.store.save_state(&self.state);
    }

    pub fn save_flags(&self) {
        if let Err(error) = self.flags.save() {
            crate::logger::error("UI", format!("saving fast flags: {error}"));
        }
    }

    pub fn player_version(&self) -> &str {
        &self.player_state.version_guid
    }

    pub fn studio_version(&self) -> &str {
        &self.studio_state.version_guid
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    FastFlags,
    Mods,
    Integrations,
    Advanced,
    About,
}

impl SettingsTab {
    pub const ALL: &'static [(SettingsTab, &'static str)] = &[
        (SettingsTab::General, "General"),
        (SettingsTab::FastFlags, "Fast flags"),
        (SettingsTab::Mods, "Mods"),
        (SettingsTab::Integrations, "Integrations"),
        (SettingsTab::Advanced, "Advanced"),
        (SettingsTab::About, "About"),
    ];
}

pub enum Screen {
    Menu,
    Installer,
    Settings(SettingsTab),
    Launch(LaunchState),
    Uninstall,
    Uninstalled,
}

pub struct LaunchState {
    pub progress: Progress,
    pub receiver: Receiver<Result<(), String>>,
    pub result: Option<Result<(), String>>,
    pub started: Instant,
}

pub struct InstallerState {
    pub location: String,
    pub create_shortcuts: bool,
    pub enable_analytics: bool,
    pub installing: bool,
    pub error: Option<String>,
    pub receiver: Option<Receiver<Result<(), String>>>,
}

impl Default for InstallerState {
    fn default() -> Self {
        Self {
            location: crate::paths::default_install_location()
                .to_string_lossy()
                .to_string(),
            create_shortcuts: true,
            enable_analytics: false,
            installing: false,
            error: None,
            receiver: None,
        }
    }
}

pub struct RuststrapApp {
    pub env: Option<Env>,
    pub screen: Screen,
    pub next_screen: Option<Screen>,
    pub launch_request: LaunchSettings,
    pub pending_launch: Option<LaunchMode>,
    pub installer: InstallerState,
    pub new_flag_key: String,
    pub new_flag_value: String,
    pub flag_filter: String,
    pub toast: Option<(String, Instant)>,
    pub keep_data: bool,
    pub uninstall_error: Option<String>,
}

impl RuststrapApp {
    pub fn new(cc: &CreationContext<'_>, launch_request: LaunchSettings) -> Self {
        theme::apply(&cc.egui_ctx, "Dark");

        let mut app = Self {
            env: None,
            screen: Screen::Menu,
            next_screen: None,
            launch_request,
            pending_launch: None,
            installer: InstallerState::default(),
            new_flag_key: String::new(),
            new_flag_value: String::new(),
            flag_filter: String::new(),
            toast: None,
            keep_data: true,
            uninstall_error: None,
        };

        let installed = installer::get_install_location()
            .filter(|base| base.join(format!("{}.exe", crate::paths::PROJECT_NAME)).is_file());

        if let Some(base) = installed {
            let env = Env::load(base);
            theme::apply(&cc.egui_ctx, &env.settings.theme);
            app.env = Some(env);
            app.route_launch();
        } else {
            app.screen = Screen::Installer;
            if app.launch_request.launch_mode != LaunchMode::None {
                app.pending_launch = Some(app.launch_request.launch_mode);
            }
        }

        app
    }

    fn route_launch(&mut self) {
        if self.launch_request.uninstall {
            self.screen = Screen::Uninstall;
            return;
        }
        if self.launch_request.menu {
            self.screen = Screen::Settings(SettingsTab::General);
            return;
        }
        if self.launch_request.launch_mode != LaunchMode::None {
            let mode = self.launch_request.launch_mode;
            self.pending_launch = Some(mode);
            self.start_launch(mode);
            return;
        }
        self.screen = Screen::Menu;
    }

    pub fn start_launch(&mut self, mode: LaunchMode) {
        let Some(env) = self.env.as_mut() else {
            self.pending_launch = Some(mode);
            self.next_screen = Some(Screen::Installer);
            return;
        };

        env.save_settings();

        let request = LaunchRequest {
            launch_mode: mode,
            launch_args: self.launch_request.roblox_launch_args.clone(),
            channel: self.launch_request.channel.clone(),
            version_guid: self.launch_request.version.clone(),
            force: self.launch_request.force,
            no_launch: self.launch_request.no_launch,
        };

        let paths = env.paths.clone();
        let progress = Progress::new();
        let (sender, receiver) = std::sync::mpsc::channel();
        let thread_progress = progress.clone();

        std::thread::spawn(move || {
            let result = crate::bootstrapper::run(paths, request, thread_progress);
            let _ = sender.send(result.map_err(|error| format!("{error:#}")));
        });

        self.pending_launch = None;
        self.next_screen = Some(Screen::Launch(LaunchState {
            progress,
            receiver,
            result: None,
            started: Instant::now(),
        }));
    }

    pub fn show_toast(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    fn draw_toast(&mut self, ctx: &egui::Context) {
        let Some((message, created)) = self.toast.as_ref() else {
            return;
        };
        if created.elapsed().as_secs_f32() > 4.0 {
            self.toast = None;
            return;
        }

        egui::Area::new(egui::Id::new("toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -18.0))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(theme::CARD_HOVER)
                    .rounding(egui::Rounding::same(10.0))
                    .inner_margin(egui::Margin::symmetric(16.0, 10.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new(message).color(theme::TEXT));
                    });
            });

        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

impl eframe::App for RuststrapApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(next) = self.next_screen.take() {
            self.screen = next;
        }

        let mut screen = std::mem::replace(&mut self.screen, Screen::Menu);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme::BG))
            .show(ctx, |ui| match &mut screen {
                Screen::Menu => self.draw_menu(ui),
                Screen::Installer => self.draw_installer(ui, ctx),
                Screen::Settings(tab) => self.draw_settings(ui, tab),
                Screen::Launch(state) => self.draw_launch(ui, ctx, state),
                Screen::Uninstall => self.draw_uninstall(ui),
                Screen::Uninstalled => self.draw_uninstalled(ui, ctx),
            });

        if let Some(next) = self.next_screen.take() {
            self.screen = next;
        } else {
            self.screen = screen;
        }

        self.draw_toast(ctx);
    }
}

pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(theme::CARD)
        .stroke(egui::Stroke::new(1.0f32, theme::BORDER))
        .rounding(egui::Rounding::same(12.0))
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let width = ui.available_width().min(320.0);
    ui.add_sized(
        [width, 42.0],
        egui::Button::new(RichText::new(text).size(15.5).color(Color32::WHITE))
            .fill(theme::ACCENT)
            .rounding(egui::Rounding::same(10.0)),
    )
}

pub fn secondary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let width = ui.available_width().min(320.0);
    ui.add_sized(
        [width, 38.0],
        egui::Button::new(RichText::new(text).size(14.5).color(theme::TEXT))
            .fill(theme::CARD_HOVER)
            .stroke(egui::Stroke::new(1.0f32, theme::BORDER))
            .rounding(egui::Rounding::same(10.0)),
    )
}

pub fn danger_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let width = ui.available_width().min(320.0);
    ui.add_sized(
        [width, 38.0],
        egui::Button::new(RichText::new(text).size(14.5).color(Color32::WHITE))
            .fill(theme::DANGER)
            .rounding(egui::Rounding::same(10.0)),
    )
}

pub fn page_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.add_space(6.0);
    ui.label(
        RichText::new(title)
            .size(30.0)
            .strong()
            .color(theme::TEXT),
    );
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).color(theme::MUTED));
    }
    ui.add_space(8.0);
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(theme::MUTED).size(13.0)
}

pub fn open_folder(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
    crate::util::open_path(path);
}

pub fn open_url(url: &str) {
    let _ = webbrowser::open(url);
}
