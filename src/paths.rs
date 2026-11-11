use std::path::PathBuf;

pub const PROJECT_NAME: &str = "Ruststrap";
pub const PROJECT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug)]
pub struct Paths {
    pub base: PathBuf,
    pub downloads: PathBuf,
    pub logs: PathBuf,
    pub integrations: PathBuf,
    pub versions: PathBuf,
    pub modifications: PathBuf,
    pub custom_themes: PathBuf,
    pub application: PathBuf,
}

impl Paths {
    pub fn new(base: PathBuf) -> Self {
        Self {
            downloads: base.join("Downloads"),
            logs: base.join("Logs"),
            integrations: base.join("Integrations"),
            versions: base.join("Versions"),
            modifications: base.join("Modifications"),
            custom_themes: base.join("CustomThemes"),
            application: base.join(format!("{PROJECT_NAME}.exe")),
            base,
        }
    }

    pub fn install_location() -> PathBuf {
        crate::installer::get_install_location()
            .unwrap_or_else(default_install_location)
    }

    pub fn temp() -> PathBuf {
        std::env::temp_dir().join(PROJECT_NAME)
    }

    pub fn temp_logs() -> PathBuf {
        Self::temp().join("Logs")
    }

    pub fn log_file(&self) -> PathBuf {
        self.logs.join("Ruststrap.log")
    }

    pub fn custom_font(&self) -> PathBuf {
        self.modifications.join("content\\fonts\\CustomFont.ttf")
    }

    pub fn client_settings(&self) -> PathBuf {
        self.modifications
            .join("ClientSettings\\ClientAppSettings.json")
    }
}

pub fn default_install_location() -> PathBuf {
    crate::util::local_app_data().join(PROJECT_NAME)
}
