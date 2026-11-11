use anyhow::{bail, Context, Result};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::ConfigStore;
use crate::discord::{unix_millis, DiscordRpc};
use crate::paths::Paths;

#[derive(Clone, Debug, Deserialize)]
pub struct WatcherData {
    pub process_id: u32,
    #[serde(default)]
    pub log_file: Option<String>,
    #[serde(default)]
    pub autoclose_pids: Vec<u32>,
}

pub fn run(paths: Paths, encoded: Option<String>) -> Result<()> {
    let Some(encoded) = encoded else {
        bail!("Watcher data was not provided");
    };

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("decoding watcher data")?;
    let data: WatcherData =
        serde_json::from_slice(&decoded).context("parsing watcher data")?;

    let store = ConfigStore::new(paths.base.clone());
    let settings = store.load_settings();

    crate::logger::log(
        "Watcher",
        format!(
            "Watching PID {} with log {:?}",
            data.process_id, data.log_file
        ),
    );

    let stop = Arc::new(AtomicBool::new(false));
    let rpc: Arc<Mutex<Option<DiscordRpc>>> = Arc::new(Mutex::new(None));

    if settings.use_discord_rich_presence {
        let mut guard = rpc.lock().unwrap();
        *guard = Some(DiscordRpc::new(data.process_id));
    }

    if let Some(log_file) = data.log_file.as_ref() {
        let stop_flag = stop.clone();
        let log_path = PathBuf::from(log_file);
        let settings = settings.clone();
        let rpc = rpc.clone();
        std::thread::spawn(move || {
            tail_log(&log_path, &settings, &rpc, &stop_flag);
        });
    }

    wait_for_process(data.process_id);

    stop.store(true, Ordering::SeqCst);

    if let Some(rpc) = rpc.lock().unwrap().as_mut() {
        rpc.clear_activity();
    }

    for pid in data.autoclose_pids {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }

    crate::logger::log("Watcher", "Roblox exited");
    Ok(())
}

fn wait_for_process(pid: u32) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE,
        };

        unsafe {
            let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if handle.is_null() {
                return;
            }
            WaitForSingleObject(handle, INFINITE);
            CloseHandle(handle);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
    }
}

fn tail_log(
    path: &Path,
    settings: &crate::config::Settings,
    rpc: &Mutex<Option<DiscordRpc>>,
    stop: &AtomicBool,
) {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => {
            crate::logger::error("Watcher", format!("opening log file: {error}"));
            return;
        }
    };

    crate::logger::log("Watcher", format!("Tailing {}", path.display()));

    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut activity = Activity::default();
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout_read(Duration::from_secs(15))
        .user_agent(&format!(
            "{}/{}",
            crate::paths::PROJECT_NAME,
            crate::paths::PROJECT_VERSION
        ))
        .build();

    while !stop.load(Ordering::SeqCst) {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => std::thread::sleep(Duration::from_millis(500)),
            Ok(_) => activity.process_line(&line, settings, rpc, &agent),
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}

#[derive(Default)]
struct Activity {
    in_game: bool,
    place_id: u64,
    universe_id: u64,
    user_id: u64,
    job_id: String,
    started: i64,
    game_name: String,
    creator: String,
    icon_url: Option<String>,
    avatar_url: Option<String>,
    username: Option<String>,
    rpc_details: Option<String>,
    rpc_state: Option<String>,
    last_rpc: Option<Instant>,
}

impl Activity {
    fn process_line(
        &mut self,
        line: &str,
        settings: &crate::config::Settings,
        rpc: &Mutex<Option<DiscordRpc>>,
        agent: &ureq::Agent,
    ) {
        if line.contains("! Joining game '") {
            self.reset();
            self.parse_join(line);
            crate::logger::log(
                "Activity",
                format!("Joining place {} ({})", self.place_id, self.job_id),
            );
            return;
        }

        if line.contains("universeid:") && line.contains("userid:") {
            if let Some(universe) = digits_after(line, "universeid:") {
                self.universe_id = universe;
            }
            if let Some(user) = digits_after(line, "userid:") {
                self.user_id = user;
            }

            if self.universe_id != 0 {
                self.fetch_game(agent, settings);
            }
            return;
        }

        if line.contains("Replicator created:") {
            if !self.in_game {
                self.in_game = true;
                self.started = unix_millis();
                self.update_presence(settings, rpc);
            }
            return;
        }

        if line.contains("Time to disconnect replication data:")
            || line.contains("leaveUGCGameInternal")
        {
            if self.in_game || self.place_id != 0 {
                self.reset();
                if let Some(rpc) = rpc.lock().unwrap().as_mut() {
                    rpc.clear_activity();
                }
            }
            return;
        }

        if let Some(index) = line.find("[BloxstrapRPC] ") {
            self.process_rpc_message(&line[index + "[BloxstrapRPC] ".len()..], settings, rpc);
        }
    }

    fn parse_join(&mut self, line: &str) {
        let Some(start) = line.find("! Joining game '") else {
            return;
        };
        let rest = &line[start + "! Joining game '".len()..];
        if let Some(end) = rest.find('\'') {
            self.job_id = rest[..end].to_string();
        }
        self.place_id = digits_after(rest, "place ").unwrap_or(0);
    }

    fn process_rpc_message(
        &mut self,
        payload: &str,
        settings: &crate::config::Settings,
        rpc: &Mutex<Option<DiscordRpc>>,
    ) {
        if let Some(last) = self.last_rpc {
            if last.elapsed() < Duration::from_secs(1) {
                return;
            }
        }

        let Ok(message) = serde_json::from_str::<Value>(payload.trim()) else {
            return;
        };
        let command = message
            .get("command")
            .and_then(|value| value.as_str())
            .unwrap_or_default();

        if command != "SetRichPresence" {
            return;
        }

        self.last_rpc = Some(Instant::now());
        let Some(data) = message.get("data") else {
            return;
        };

        if let Some(details) = data.get("details").and_then(|value| value.as_str()) {
            self.rpc_details = if details == "<reset>" {
                None
            } else {
                Some(details.chars().take(128).collect())
            };
        }
        if let Some(state) = data.get("state").and_then(|value| value.as_str()) {
            self.rpc_state = if state == "<reset>" {
                None
            } else {
                Some(state.chars().take(128).collect())
            };
        }

        self.update_presence(settings, rpc);
    }

    fn reset(&mut self) {
        self.in_game = false;
        self.place_id = 0;
        self.universe_id = 0;
        self.user_id = 0;
        self.job_id.clear();
        self.started = 0;
        self.game_name.clear();
        self.creator.clear();
        self.icon_url = None;
        self.avatar_url = None;
        self.username = None;
        self.rpc_details = None;
        self.rpc_state = None;
    }

    fn fetch_game(&mut self, agent: &ureq::Agent, settings: &crate::config::Settings) {
        if self.universe_id == 0 {
            return;
        }

        let games_url = format!(
            "https://games.roblox.com/v1/games?universeIds={}",
            self.universe_id
        );
        if let Ok(body) = get_text(agent, &games_url) {
            if let Ok(value) = serde_json::from_str::<Value>(&body) {
                if let Some(game) = value.get("data").and_then(|data| data.get(0)) {
                    self.game_name = game
                        .get("name")
                        .and_then(|value| value.as_str())
                        .unwrap_or("Roblox")
                        .to_string();
                    self.creator = game
                        .get("creator")
                        .and_then(|creator| creator.get("name"))
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_string();
                }
            }
        }

        let icons_url = format!(
            "https://thumbnails.roblox.com/v1/games/icons?universeIds={}&returnPolicy=PlaceHolder&size=512x512&format=Png&isCircular=false",
            self.universe_id
        );
        if let Ok(body) = get_text(agent, &icons_url) {
            if let Ok(value) = serde_json::from_str::<Value>(&body) {
                self.icon_url = value
                    .get("data")
                    .and_then(|data| data.get(0))
                    .and_then(|icon| icon.get("imageUrl"))
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string());
            }
        }

        if settings.show_account_on_rich_presence && self.user_id != 0 {
            let avatar_url = format!(
                "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={}&size=150x150&format=Png&isCircular=true",
                self.user_id
            );
            if let Ok(body) = get_text(agent, &avatar_url) {
                if let Ok(value) = serde_json::from_str::<Value>(&body) {
                    self.avatar_url = value
                        .get("data")
                        .and_then(|data| data.get(0))
                        .and_then(|icon| icon.get("imageUrl"))
                        .and_then(|value| value.as_str())
                        .map(|value| value.to_string());
                }
            }

            let user_url = format!("https://users.roblox.com/v1/users/{}", self.user_id);
            if let Ok(body) = get_text(agent, &user_url) {
                if let Ok(value) = serde_json::from_str::<Value>(&body) {
                    self.username = value
                        .get("name")
                        .and_then(|value| value.as_str())
                        .map(|value| value.to_string());
                }
            }
        }
    }

    fn update_presence(
        &self,
        settings: &crate::config::Settings,
        rpc: &Mutex<Option<DiscordRpc>>,
    ) {
        if !self.in_game || self.place_id == 0 {
            return;
        }

        let mut activity = json!({
            "instance": true,
            "details": self.rpc_details.clone().unwrap_or_else(|| {
                if self.game_name.is_empty() {
                    "Playing Roblox".to_string()
                } else {
                    self.game_name.chars().take(128).collect()
                }
            }),
            "state": self.rpc_state.clone().unwrap_or_else(|| {
                if self.creator.is_empty() {
                    "In game".to_string()
                } else {
                    format!("by {}", self.creator)
                }
            }),
        });

        if self.started > 0 {
            activity["timestamps"] = json!({ "start": self.started });
        }

        if let Some(icon) = &self.icon_url {
            activity["assets"] = json!({
                "large_image": icon,
                "large_text": self.game_name,
            });
        }

        if settings.show_account_on_rich_presence {
            if let Some(avatar) = &self.avatar_url {
                if activity.get("assets").is_none() {
                    activity["assets"] = json!({});
                }
                activity["assets"]["small_image"] = json!(avatar);
                if let Some(username) = &self.username {
                    activity["assets"]["small_text"] = json!(username);
                }
            }
        }

        if !settings.hide_rpc_buttons {
            activity["buttons"] = json!([{
                "label": "View on Roblox",
                "url": format!("https://www.roblox.com/games/{}", self.place_id)
            }]);
        }

        if let Some(rpc) = rpc.lock().unwrap().as_mut() {
            rpc.set_activity(activity);
        }
    }
}

fn get_text(agent: &ureq::Agent, url: &str) -> Result<String> {
    let response = agent.get(url).call().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(response.into_string()?)
}

fn digits_after(text: &str, marker: &str) -> Option<u64> {
    let index = text.find(marker)? + marker.len();
    let digits: String = text[index..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}
