#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LaunchMode {
    #[default]
    None,
    Player,
    Studio,
    StudioAuth,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct LaunchSettings {
    pub raw: Vec<String>,
    pub menu: bool,
    pub watcher: bool,
    pub watcher_data: Option<String>,
    pub background_updater: bool,
    pub quiet: bool,
    pub uninstall: bool,
    pub no_launch: bool,
    pub test_mode: bool,
    pub no_gpu: bool,
    pub upgrade: bool,
    pub force: bool,
    pub version: Option<String>,
    pub channel: Option<String>,
    pub launch_mode: LaunchMode,
    pub roblox_launch_args: String,
}

impl LaunchSettings {
    pub fn parse(args: Vec<String>) -> Self {
        let mut settings = Self {
            raw: args.clone(),
            menu: false,
            watcher: false,
            watcher_data: None,
            background_updater: false,
            quiet: false,
            uninstall: false,
            no_launch: false,
            test_mode: false,
            no_gpu: false,
            upgrade: false,
            force: false,
            version: None,
            channel: None,
            launch_mode: LaunchMode::None,
            roblox_launch_args: String::new(),
        };

        let mut start_index = 0;

        if let Some(first) = args.first() {
            let lower = first.to_lowercase();
            if lower.starts_with("roblox:") || lower.starts_with("roblox-player:") {
                settings.launch_mode = LaunchMode::Player;
                settings.roblox_launch_args = first.clone();
                start_index = 1;
            } else if lower.starts_with("version-") {
                settings.version = Some(first.clone());
                settings.launch_mode = LaunchMode::Unknown;
                start_index = 1;
            }
        }

        let mut index = start_index;
        while index < args.len() {
            let arg = &args[index];
            if !arg.starts_with('-') {
                index += 1;
                continue;
            }

            let identifier = arg.trim_start_matches('-').to_lowercase();
            let data = if index + 1 < args.len() && !args[index + 1].starts_with('-') {
                Some(args[index + 1].clone())
            } else {
                None
            };

            let consumed_data = data.is_some();

            match identifier.as_str() {
                "preferences" | "menu" | "settings" => settings.menu = true,
                "watcher" => {
                    settings.watcher = true;
                    settings.watcher_data = data.clone();
                }
                "backgroundupdater" => {
                    settings.background_updater = true;
                    settings.quiet = true;
                    settings.no_launch = true;
                    if let Some(data) = data.as_deref() {
                        settings.launch_mode = match data.to_lowercase().as_str() {
                            "player" => LaunchMode::Player,
                            "studio" => LaunchMode::Studio,
                            _ => LaunchMode::Unknown,
                        };
                    }
                }
                "quiet" => settings.quiet = true,
                "uninstall" => settings.uninstall = true,
                "nolaunch" => settings.no_launch = true,
                "testmode" => settings.test_mode = true,
                "nogpu" => settings.no_gpu = true,
                "upgrade" => settings.upgrade = true,
                "force" => settings.force = true,
                "version" => {
                    settings.version = data.clone();
                    settings.launch_mode = LaunchMode::Unknown;
                }
                "channel" => settings.channel = data.clone(),
                "player" => {
                    settings.launch_mode = LaunchMode::Player;
                    if let Some(data) = &data {
                        settings.roblox_launch_args = data.clone();
                    }
                }
                "studio" => {
                    settings.launch_mode = LaunchMode::Studio;
                    if let Some(data) = &data {
                        if data.starts_with("roblox-studio-auth:") {
                            settings.launch_mode = LaunchMode::StudioAuth;
                            settings.roblox_launch_args = data.clone();
                        } else if data.starts_with("roblox-studio:") {
                            settings.roblox_launch_args = data.clone();
                        } else {
                            settings.roblox_launch_args =
                                format!("-task EditFile -localPlaceFile \"{data}\"");
                        }
                    }
                }
                _ => {
                    // Unknown arguments for the Roblox client are passed through
                    // when they were supplied after a Roblox launch URI.
                    if settings.launch_mode != LaunchMode::None && !settings.roblox_launch_args.is_empty()
                    {
                        settings.roblox_launch_args.push(' ');
                        settings.roblox_launch_args.push_str(arg);
                    }
                }
            }

            index += if consumed_data { 2 } else { 1 };
        }

        settings
    }

    pub fn wants_gui(&self) -> bool {
        !self.watcher && !self.background_updater && !self.quiet
    }

    pub fn channel_or_default(&self) -> String {
        self.channel
            .clone()
            .filter(|c| !c.is_empty())
            .or_else(|| {
                channel_from_uri(&self.roblox_launch_args)
                    .map(|c| c.to_lowercase())
            })
            .unwrap_or_else(|| "production".into())
    }
}

pub fn channel_from_uri(uri: &str) -> Option<String> {
    let decoded = uri.replace("%3a", ":").replace("%3A", ":");
    let lower = decoded.to_lowercase();
    let marker = "+channel:";
    let start = lower.find(marker)? + marker.len();
    let rest = &decoded[start..];
    let end = rest
        .find('+')
        .or_else(|| rest.find('&'))
        .unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_player_uri() {
        let args = vec![
            "-player".to_string(),
            "roblox-player:1+launchmode:play+gameinfo:123+channel:zlive".to_string(),
        ];
        let settings = LaunchSettings::parse(args);
        assert_eq!(settings.launch_mode, LaunchMode::Player);
        assert!(settings.roblox_launch_args.contains("roblox-player:"));
        assert_eq!(settings.channel_or_default(), "zlive");
    }
}
