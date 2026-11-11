use std::collections::BTreeMap;

use crate::cli::LaunchMode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppKind {
    Player,
    Studio,
}

#[derive(Clone, Debug)]
pub struct AppData {
    pub kind: AppKind,
    pub product_name: &'static str,
    pub binary_type: &'static str,
    pub registry_name: &'static str,
    pub process_name: &'static str,
    pub executable_name: &'static str,
    pub package_map: BTreeMap<&'static str, &'static str>,
}

const COMMON_MAP: &[(&str, &str)] = &[
    ("Libraries.zip", ""),
    ("redist.zip", ""),
    ("shaders.zip", "shaders\\"),
    ("ssl.zip", "ssl\\"),
    ("WebView2.zip", ""),
    ("WebView2RuntimeInstaller.zip", "WebView2RuntimeInstaller\\"),
    ("content-avatar.zip", "content\\avatar\\"),
    ("content-configs.zip", "content\\configs\\"),
    ("content-fonts.zip", "content\\fonts\\"),
    ("content-sky.zip", "content\\sky\\"),
    ("content-sounds.zip", "content\\sounds\\"),
    ("content-textures2.zip", "content\\textures\\"),
    ("content-models.zip", "content\\models\\"),
    ("content-textures3.zip", "PlatformContent\\pc\\textures\\"),
    ("content-terrain.zip", "PlatformContent\\pc\\terrain\\"),
    ("content-platform-fonts.zip", "PlatformContent\\pc\\fonts\\"),
    (
        "content-platform-dictionaries.zip",
        "PlatformContent\\pc\\shared_compression_dictionaries\\",
    ),
    ("extracontent-luapackages.zip", "ExtraContent\\LuaPackages\\"),
    (
        "extracontent-translations.zip",
        "ExtraContent\\translations\\",
    ),
    ("extracontent-models.zip", "ExtraContent\\models\\"),
    ("extracontent-textures.zip", "ExtraContent\\textures\\"),
    ("extracontent-places.zip", "ExtraContent\\places\\"),
];

const PLAYER_MAP: &[(&str, &str)] =
    &[("RobloxApp.zip", "")];

const STUDIO_MAP: &[(&str, &str)] = &[
    ("RobloxStudio.zip", ""),
    ("LibrariesQt5.zip", ""),
    ("content-studio_svg_textures.zip", "content\\studio_svg_textures\\"),
    ("content-qt_translations.zip", "content\\qt_translations\\"),
    ("content-api-docs.zip", "content\\api_docs\\"),
    ("extracontent-scripts.zip", "ExtraContent\\scripts\\"),
    ("studiocontent-models.zip", "StudioContent\\models\\"),
    ("studiocontent-textures.zip", "StudioContent\\textures\\"),
    ("BuiltInPlugins.zip", "BuiltInPlugins\\"),
    ("BuiltInStandalonePlugins.zip", "BuiltInStandalonePlugins\\"),
    ("ApplicationConfig.zip", "ApplicationConfig\\"),
    ("Plugins.zip", "Plugins\\"),
    ("Qml.zip", "Qml\\"),
    ("StudioFonts.zip", "StudioFonts\\"),
    ("RibbonConfig.zip", "RibbonConfig\\"),
];

impl AppData {
    pub fn player() -> Self {
        Self::new(AppKind::Player)
    }

    pub fn studio() -> Self {
        Self::new(AppKind::Studio)
    }

    pub fn for_mode(mode: LaunchMode) -> Self {
        match mode {
            LaunchMode::Player => Self::player(),
            _ => Self::studio(),
        }
    }

    fn new(kind: AppKind) -> Self {
        let mut package_map = BTreeMap::new();
        for (name, dir) in COMMON_MAP {
            package_map.insert(*name, *dir);
        }
        let specific = match kind {
            AppKind::Player => PLAYER_MAP,
            AppKind::Studio => STUDIO_MAP,
        };
        for (name, dir) in specific {
            package_map.insert(*name, *dir);
        }

        let (product_name, binary_type, registry_name, process_name, executable_name) = match kind {
            AppKind::Player => (
                "Roblox",
                "WindowsPlayer",
                "RobloxPlayer",
                "RobloxPlayerBeta",
                "RobloxPlayerBeta.exe",
            ),
            AppKind::Studio => (
                "Roblox Studio",
                "WindowsStudio64",
                "RobloxStudio",
                "RobloxStudioBeta",
                "RobloxStudioBeta.exe",
            ),
        };

        Self {
            kind,
            product_name,
            binary_type,
            registry_name,
            process_name,
            executable_name,
            package_map,
        }
    }

    pub fn directory(&self, versions: &std::path::Path, version_guid: &str) -> std::path::PathBuf {
        versions.join(version_guid)
    }

    pub fn executable_path(
        &self,
        versions: &std::path::Path,
        version_guid: &str,
    ) -> std::path::PathBuf {
        self.directory(versions, version_guid).join(self.executable_name)
    }

    pub fn package_directory(&self, package_name: &str) -> Option<&'static str> {
        self.package_map.get(package_name).copied()
    }
}
