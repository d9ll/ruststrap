use crate::util;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct FastFlags {
    pub path: PathBuf,
    pub values: BTreeMap<String, String>,
}

impl FastFlags {
    pub fn load(path: PathBuf) -> Self {
        let values = util::read_json::<BTreeMap<String, FlagValue>>(&path)
            .map(|map| {
                map.into_iter()
                    .filter_map(|(k, v)| v.0.map(|value| (k, value)))
                    .collect()
            })
            .unwrap_or_default();

        let mut flags = Self { path, values };
        if flags.get("FFlagHandleAltEnterFullscreenManually").is_none() {
            flags.set("FFlagHandleAltEnterFullscreenManually", Some("False".into()));
        }
        flags
    }

    pub fn save(&self) -> std::io::Result<()> {
        let map: BTreeMap<String, FlagValue> = self
            .values
            .iter()
            .map(|(k, v)| (k.clone(), FlagValue(Some(v.clone()))))
            .collect();
        util::write_json(&self.path, &map)
    }

    pub fn has_changed(&self) -> bool {
        true
    }

    pub fn set(&mut self, key: &str, value: Option<String>) {
        match value {
            Some(value) => {
                self.values.insert(key.to_string(), value);
            }
            None => {
                self.values.remove(key);
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    pub fn remove(&mut self, key: &str) {
        self.values.remove(key);
    }

    pub fn set_preset(&mut self, prefix: &str, value: Option<&str>) {
        for (name, flag) in PRESET_FLAGS {
            if name.starts_with(prefix) {
                self.set(flag, value.map(|v| v.to_string()));
            }
        }
    }

    pub fn set_preset_enum(&mut self, prefix: &str, target: &str, value: Option<&str>) {
        for (name, flag) in PRESET_FLAGS {
            if name.starts_with(prefix) {
                if name.starts_with(&format!("{prefix}.{target}")) {
                    self.set(flag, value.map(|v| v.to_string()));
                } else {
                    self.set(flag, None);
                }
            }
        }
    }

    pub fn get_preset(&self, name: &str) -> Option<&str> {
        PRESET_FLAGS
            .iter()
            .find(|(preset, _)| *preset == name)
            .and_then(|(_, flag)| self.get(flag))
    }

    pub fn ensure_defaults(&mut self) {
        if self.get("FFlagHandleAltEnterFullscreenManually").is_none() {
            self.set("FFlagHandleAltEnterFullscreenManually", Some("False".into()));
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Accepts both strings and primitive JSON values, as Roblox clients are
/// lenient about the value type of a fast flag.
struct FlagValue(Option<String>);

impl<'de> Deserialize<'de> for FlagValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self(match value {
            serde_json::Value::Null => None,
            serde_json::Value::String(s) => Some(s),
            other => Some(other.to_string()),
        }))
    }
}

impl Serialize for FlagValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.as_deref().unwrap_or(""))
    }
}

pub const PRESET_FLAGS: &[(&str, &str)] = &[
    (
        "Rendering.ManualFullscreen",
        "FFlagHandleAltEnterFullscreenManually",
    ),
    ("Rendering.DisableScaling", "DFFlagDisableDPIScale"),
    ("Rendering.MSAA", "FIntDebugForceMSAASamples"),
    (
        "Rendering.TextureQuality.OverrideEnabled",
        "DFFlagTextureQualityOverrideEnabled",
    ),
    (
        "Rendering.TextureQuality.Level",
        "DFIntTextureQualityOverride",
    ),
];

pub const MSAA_MODES: &[(&str, Option<&str>)] = &[
    ("Default", None),
    ("1x", Some("1")),
    ("2x", Some("2")),
    ("4x", Some("4")),
];

pub const TEXTURE_QUALITY_LEVELS: &[(&str, Option<&str>)] = &[
    ("Default", None),
    ("Level 0", Some("0")),
    ("Level 1", Some("1")),
    ("Level 2", Some("2")),
    ("Level 3", Some("3")),
];

pub fn current_msaa(flags: &FastFlags) -> &'static str {
    let value = flags.get_preset("Rendering.MSAA");
    MSAA_MODES
        .iter()
        .find(|(_, v)| *v == value)
        .map(|(name, _)| *name)
        .unwrap_or("Default")
}

pub fn current_texture_quality(flags: &FastFlags) -> &'static str {
    let value = flags.get_preset("Rendering.TextureQuality.Level");
    TEXTURE_QUALITY_LEVELS
        .iter()
        .find(|(_, v)| *v == value)
        .map(|(name, _)| *name)
        .unwrap_or("Default")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_set_and_clear_flags() {
        let mut flags = FastFlags::default();
        flags.set_preset("Rendering.MSAA", Some("4"));
        assert_eq!(flags.get("FIntDebugForceMSAASamples"), Some("4"));
        assert_eq!(current_msaa(&flags), "4x");

        flags.set_preset("Rendering.MSAA", None);
        assert_eq!(flags.get("FIntDebugForceMSAASamples"), None);
        assert_eq!(current_msaa(&flags), "Default");
    }

    #[test]
    fn texture_quality_preset() {
        let mut flags = FastFlags::default();
        flags.set_preset("Rendering.TextureQuality.Level", Some("2"));
        flags.set_preset("Rendering.TextureQuality.OverrideEnabled", Some("True"));
        assert_eq!(current_texture_quality(&flags), "Level 2");
        assert_eq!(
            flags.get("DFFlagTextureQualityOverrideEnabled"),
            Some("True")
        );
    }
}
