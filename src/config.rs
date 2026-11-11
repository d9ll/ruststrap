use crate::util;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Settings {
    pub bootstrapper_style: String,
    pub bootstrapper_title: String,
    pub theme: String,
    pub check_for_updates: bool,
    pub confirm_launches: bool,
    pub locale: String,
    pub use_fast_flag_manager: bool,
    pub enable_analytics: bool,
    pub background_updates_enabled: bool,
    pub debug_disable_version_package_cleanup: bool,
    pub selected_custom_theme: Option<String>,
    pub enable_activity_tracking: bool,
    pub use_discord_rich_presence: bool,
    pub hide_rpc_buttons: bool,
    pub show_account_on_rich_presence: bool,
    pub show_server_details: bool,
    pub use_disable_app_patch: bool,
    pub channel: String,
    pub install_location: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bootstrapper_style: "FluentDialog".into(),
            bootstrapper_title: crate::paths::PROJECT_NAME.into(),
            theme: "Dark".into(),
            check_for_updates: true,
            confirm_launches: false,
            locale: "en".into(),
            use_fast_flag_manager: true,
            enable_analytics: false,
            background_updates_enabled: false,
            debug_disable_version_package_cleanup: false,
            selected_custom_theme: None,
            enable_activity_tracking: true,
            use_discord_rich_presence: true,
            hide_rpc_buttons: true,
            show_account_on_rich_presence: false,
            show_server_details: true,
            use_disable_app_patch: false,
            channel: "production".into(),
            install_location: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct State {
    pub prompt_webview2_install: bool,
    pub force_reinstall: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            prompt_webview2_install: true,
            force_reinstall: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct DistributionState {
    pub version_guid: String,
    pub package_hashes: HashMap<String, String>,
    pub size: u64,
    pub mod_manifest: Vec<String>,
}

impl Default for DistributionState {
    fn default() -> Self {
        Self {
            version_guid: String::new(),
            package_hashes: HashMap::new(),
            size: 0,
            mod_manifest: Vec::new(),
        }
    }
}

pub struct ConfigStore {
    pub base: std::path::PathBuf,
}

impl ConfigStore {
    pub fn new(base: impl Into<std::path::PathBuf>) -> Self {
        Self { base: base.into() }
    }

    pub fn settings_path(&self) -> std::path::PathBuf {
        self.base.join("Settings.json")
    }

    pub fn state_path(&self) -> std::path::PathBuf {
        self.base.join("State.json")
    }

    pub fn player_state_path(&self) -> std::path::PathBuf {
        self.base.join("PlayerState.json")
    }

    pub fn studio_state_path(&self) -> std::path::PathBuf {
        self.base.join("StudioState.json")
    }

    pub fn load_settings(&self) -> Settings {
        util::read_json(&self.settings_path()).unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &Settings) {
        if let Err(e) = util::write_json(&self.settings_path(), settings) {
            crate::logger::error("Config::SaveSettings", e);
        }
    }

    pub fn load_state(&self) -> State {
        util::read_json(&self.state_path()).unwrap_or_default()
    }

    pub fn save_state(&self, state: &State) {
        if let Err(e) = util::write_json(&self.state_path(), state) {
            crate::logger::error("Config::SaveState", e);
        }
    }

    pub fn load_player_state(&self) -> DistributionState {
        util::read_json(&self.player_state_path()).unwrap_or_default()
    }

    pub fn save_player_state(&self, state: &DistributionState) {
        if let Err(e) = util::write_json(&self.player_state_path(), state) {
            crate::logger::error("Config::SavePlayerState", e);
        }
    }

    pub fn load_studio_state(&self) -> DistributionState {
        util::read_json(&self.studio_state_path()).unwrap_or_default()
    }

    pub fn save_studio_state(&self, state: &DistributionState) {
        if let Err(e) = util::write_json(&self.studio_state_path(), state) {
            crate::logger::error("Config::SaveStudioState", e);
        }
    }
}

pub fn file_exists(path: &Path) -> bool {
    path.is_file()
}
