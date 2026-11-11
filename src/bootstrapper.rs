use anyhow::{bail, Context, Result};
use base64::Engine;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

use crate::appdata::{AppData, AppKind};
use crate::cli::{channel_from_uri, LaunchMode};
use crate::config::{ConfigStore, DistributionState, Settings, State};
use crate::deployment::{Deployment, PackageManifest};
use crate::fastflags::FastFlags;
use crate::packages::{self, download_package, extract_package};
use crate::paths::Paths;
use crate::progress::Progress;
use crate::util;

const APP_SETTINGS_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Settings>\r\n\t<ContentFolder>content</ContentFolder>\r\n\t<BaseUrl>http://www.roblox.com</BaseUrl>\r\n</Settings>\r\n";

#[derive(Clone, Debug, Default)]
pub struct LaunchRequest {
    pub launch_mode: LaunchMode,
    pub launch_args: String,
    pub channel: Option<String>,
    pub version_guid: Option<String>,
    pub force: bool,
    pub no_launch: bool,
}

#[derive(Serialize)]
struct WatcherData {
    process_id: u32,
    log_file: Option<String>,
    autoclose_pids: Vec<u32>,
}

pub fn run(paths: Paths, request: LaunchRequest, progress: Progress) -> Result<()> {
    crate::logger::log("Bootstrapper", "Starting bootstrapper");

    let store = ConfigStore::new(paths.base.clone());
    let settings = store.load_settings();
    let mut state = store.load_state();
    let mut flags = FastFlags::load(paths.client_settings());
    let mut player_state = store.load_player_state();
    let mut studio_state = store.load_studio_state();

    let mut app = AppData::for_mode(request.launch_mode);
    let mut deployment = Deployment::new();
    deployment.binary_type = app.binary_type.into();

    progress.set_status("Connecting to Roblox...");
    deployment.initialize_connectivity()?;
    if progress.is_cancelled() {
        bail!("Cancelled");
    }

    let channel = resolve_channel(&request, &app, &settings);
    deployment.channel = channel.clone();
    crate::logger::log("Bootstrapper", format!("Using channel {channel}"));

    progress.set_status("Checking for updates...");
    let (version_guid, manifest) = load_version(&mut deployment, &request)?;
    update_channel_registry(&app, &deployment.channel);

    if request.launch_mode == LaunchMode::Unknown {
        let is_player = manifest.find("RobloxApp.zip").is_some();
        let detected = if is_player {
            LaunchMode::Player
        } else {
            LaunchMode::Studio
        };
        crate::logger::log(
            "Bootstrapper",
            format!("Detected launch mode {detected:?} from package manifest"),
        );
        app = AppData::for_mode(detected);
        deployment.binary_type = app.binary_type.into();
    }

    let version_dir = paths.versions.join(&version_guid);
    let executable = version_dir.join(app.executable_name);
    let dist_state = if app.kind == AppKind::Player {
        &mut player_state
    } else {
        &mut studio_state
    };

    let must_upgrade = request.force
        || state.force_reinstall
        || dist_state.version_guid != version_guid
        || !executable.is_file();

    if must_upgrade || request.force {
        upgrade(
            &paths,
            &app,
            &deployment,
            &version_guid,
            &manifest,
            dist_state,
            &mut state,
            &settings,
            &progress,
        )?;
    } else {
        progress.set_status("Roblox is up to date");
    }

    if progress.is_cancelled() {
        bail!("Cancelled");
    }

    apply_modifications(
        &paths,
        &app,
        &deployment,
        &version_guid,
        &manifest,
        dist_state,
        &settings,
        &mut flags,
        &progress,
    )?;

    dist_state.version_guid = version_guid.clone();
    if app.kind == AppKind::Player {
        store.save_player_state(&player_state);
    } else {
        store.save_studio_state(&studio_state);
    }
    store.save_state(&state);

    if app.kind == AppKind::Player {
        let _ = crate::installer::register_player(&paths.application);
    } else {
        let _ = crate::installer::register_studio(&paths.application);
    }

    cleanup_versions(&paths, &[&player_state, &studio_state]);
    let keep_hashes: Vec<String> = player_state
        .package_hashes
        .values()
        .chain(studio_state.package_hashes.values())
        .cloned()
        .collect();
    packages::clean_package_cache(
        &paths.downloads,
        &keep_hashes,
        !settings.debug_disable_version_package_cleanup,
    );

    if !request.no_launch {
        start_roblox(
            &paths,
            &app,
            &version_guid,
            &request,
            &settings,
            &progress,
        )?;
    }

    progress.finish();
    Ok(())
}

fn resolve_channel(request: &LaunchRequest, app: &AppData, settings: &Settings) -> String {
    if let Some(channel) = request.channel.as_ref().filter(|c| !c.is_empty()) {
        return channel.to_lowercase();
    }
    if let Some(channel) = channel_from_uri(&request.launch_args) {
        return channel.to_lowercase();
    }
    if let Some(channel) = registry_channel(app) {
        return channel;
    }
    if !settings.channel.is_empty() {
        return settings.channel.to_lowercase();
    }
    "production".into()
}

fn registry_channel(app: &AppData) -> Option<String> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(format!(
            r"SOFTWARE\ROBLOX Corporation\Environments\{}\Channel",
            app.registry_name
        ))
        .ok()?;
    let value: String = key.get_value("www.roblox.com").ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_lowercase())
    }
}

fn update_channel_registry(app: &AppData, channel: &str) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = format!(
        r"SOFTWARE\ROBLOX Corporation\Environments\{}\Channel",
        app.registry_name
    );
    if let Ok((key, _)) = hkcu.create_subkey(path) {
        let value = if channel.eq_ignore_ascii_case("production") {
            ""
        } else {
            channel
        };
        let _ = key.set_value("www.roblox.com", &value);
    }
}

fn load_version(
    deployment: &mut Deployment,
    request: &LaunchRequest,
) -> Result<(String, PackageManifest)> {
    if let Some(guid) = request.version_guid.as_ref().filter(|v| !v.is_empty()) {
        let manifest = deployment.get_manifest(guid)?;
        return Ok((guid.clone(), manifest));
    }

    let info = match deployment.get_info() {
        Ok(info) => info,
        Err(error) if !deployment.channel.eq_ignore_ascii_case("production") => {
            crate::logger::warn(
                "Bootstrapper",
                format!(
                    "Channel {} failed ({error}), falling back to production",
                    deployment.channel
                ),
            );
            deployment.channel = "production".into();
            deployment.get_info()?
        }
        Err(error) => return Err(error),
    };

    crate::logger::log(
        "Bootstrapper",
        format!("Latest version is {} ({})", info.version, info.version_guid),
    );

    let manifest = deployment.get_manifest(&info.version_guid)?;
    Ok((info.version_guid, manifest))
}

#[allow(clippy::too_many_arguments)]
fn upgrade(
    paths: &Paths,
    app: &AppData,
    deployment: &Deployment,
    version_guid: &str,
    manifest: &PackageManifest,
    dist_state: &mut DistributionState,
    state: &mut State,
    settings: &Settings,
    progress: &Progress,
) -> Result<()> {
    crate::logger::log("Bootstrapper", "Upgrading Roblox installation");
    progress.set_status(format!("Closing {}", app.product_name));

    if settings.confirm_launches && util::process_running(app.process_name) {
        let confirmed = util::confirm(
            crate::paths::PROJECT_NAME,
            &format!(
                "{} is currently running. It will be closed so the update can be installed.\n\nContinue?",
                app.product_name
            ),
        );
        if !confirmed {
            bail!("Cancelled by user");
        }
    }

    util::kill_process_by_name(app.process_name);
    util::kill_process_by_name("RobloxCrashHandler");

    for dir in [&paths.base, &paths.downloads, &paths.versions] {
        fs::create_dir_all(dir)?;
    }

    let version_dir = paths.versions.join(version_guid);
    if version_dir.exists() {
        util::remove_dir_all_force(&version_dir)
            .with_context(|| format!("removing {}", version_dir.display()))?;
    }
    fs::create_dir_all(&version_dir)?;

    let required_space = manifest
        .packages
        .iter()
        .filter(|package| !paths.downloads.join(&package.signature).is_file())
        .map(|package| package.packed_size)
        .sum::<u64>()
        + manifest.packages.iter().map(|package| package.size).sum::<u64>();

    if let Some(free) = packages::free_disk_space(&paths.base) {
        if free < required_space {
            bail!(
                "Not enough disk space. Required {}, available {}.",
                util::format_bytes(required_space),
                util::format_bytes(free)
            );
        }
    }

    let total_packed = manifest
        .packages
        .iter()
        .map(|package| package.packed_size)
        .sum::<u64>();
    progress.set_total(total_packed);

    for package in &manifest.packages {
        if progress.is_cancelled() {
            let _ = util::remove_dir_all_force(&version_dir);
            bail!("Cancelled");
        }

        progress.set_status(format!("Downloading {}", package.name));
        progress.set_detail(&package.name);

        let archive = download_package(
            deployment,
            version_guid,
            package,
            &paths.downloads,
            progress,
        )?;

        if package.name == "WebView2RuntimeInstaller.zip" {
            continue;
        }

        let Some(package_dir) = app.package_directory(&package.name) else {
            crate::logger::warn(
                "Bootstrapper",
                format!("{} is not in the package directory map", package.name),
            );
            continue;
        };

        progress.set_status(format!("Extracting {}", package.name));
        progress.set_detail(&package.name);
        extract_package(&archive, &version_dir.join(package_dir), None)
            .with_context(|| format!("extracting {}", package.name))?;
    }

    if progress.is_cancelled() {
        let _ = util::remove_dir_all_force(&version_dir);
        bail!("Cancelled");
    }

    fs::write(version_dir.join("AppSettings.xml"), APP_SETTINGS_XML)?;

    dist_state.version_guid = version_guid.to_string();
    dist_state.package_hashes = manifest
        .packages
        .iter()
        .map(|package| (package.name.clone(), package.signature.clone()))
        .collect();
    dist_state.size = manifest
        .packages
        .iter()
        .map(|package| package.size + package.packed_size)
        .sum::<u64>()
        / 1024;

    state.force_reinstall = false;

    if !settings.debug_disable_version_package_cleanup {
        // Package cache cleanup happens after both distribution states are updated.
    }

    crate::logger::log(
        "Bootstrapper",
        format!("Installed version {version_guid}"),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_modifications(
    paths: &Paths,
    app: &AppData,
    deployment: &Deployment,
    version_guid: &str,
    manifest: &PackageManifest,
    dist_state: &mut DistributionState,
    settings: &Settings,
    flags: &mut FastFlags,
    progress: &Progress,
) -> Result<()> {
    progress.set_status("Applying modifications");
    let version_dir = paths.versions.join(version_guid);

    fs::create_dir_all(&paths.modifications)?;

    if settings.use_fast_flag_manager {
        flags.ensure_defaults();
        if let Err(error) = flags.save() {
            crate::logger::error("Bootstrapper", format!("saving fast flags: {error}"));
        }
    }

    apply_custom_font(paths, &version_dir)?;

    let files = walk_files(&paths.modifications)?;
    let mut mod_files = Vec::new();

    for file in files {
        let relative = match file.strip_prefix(&paths.modifications) {
            Ok(path) => path.to_string_lossy().replace('/', "\\"),
            Err(_) => continue,
        };

        if relative.eq_ignore_ascii_case("README.txt") {
            let _ = fs::remove_file(&file);
            continue;
        }
        if !settings.use_fast_flag_manager
            && relative.eq_ignore_ascii_case("ClientSettings\\ClientAppSettings.json")
        {
            continue;
        }
        if relative.ends_with(".lock") {
            continue;
        }

        let destination = version_dir.join(&relative);

        if destination.is_file() {
            let same = match (util::md5_file(&file), util::md5_file(&destination)) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            };
            if same {
                mod_files.push(relative);
                continue;
            }
            util::clear_readonly(&destination);
        }

        match util::copy_force(&file, &destination) {
            Ok(()) => {
                mod_files.push(relative);
            }
            Err(error) => {
                crate::logger::error(
                    "Bootstrapper",
                    format!("failed to apply mod {relative}: {error}"),
                );
            }
        }
    }

    restore_removed_mods(
        paths,
        app,
        deployment,
        version_guid,
        manifest,
        dist_state,
        &mod_files,
        progress,
    )?;

    dist_state.mod_manifest = mod_files;
    Ok(())
}

fn apply_custom_font(paths: &Paths, version_dir: &Path) -> Result<()> {
    let families_source = version_dir.join("content\\fonts\\families");
    let families_mod = paths.modifications.join("content\\fonts\\families");

    if paths.custom_font().is_file() {
        if families_source.is_dir() {
            fs::create_dir_all(&families_mod)?;
            for entry in fs::read_dir(&families_source)?.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Some(name) = path.file_name() else {
                    continue;
                };
                let destination = families_mod.join(name);
                if destination.exists() {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                if let Some(faces) = value
                    .get_mut("faces")
                    .and_then(|faces| faces.as_array_mut())
                {
                    for face in faces {
                        if let Some(object) = face.as_object_mut() {
                            object.insert(
                                "assetId".into(),
                                serde_json::Value::String(
                                    "rbxasset://fonts/CustomFont.ttf".into(),
                                ),
                            );
                        }
                    }
                }
                let _ = fs::write(
                    &destination,
                    serde_json::to_string_pretty(&value).unwrap_or_default(),
                );
            }
        }
    } else if families_mod.is_dir() {
        let _ = util::remove_dir_all_force(&families_mod);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn restore_removed_mods(
    paths: &Paths,
    app: &AppData,
    deployment: &Deployment,
    version_guid: &str,
    manifest: &PackageManifest,
    dist_state: &DistributionState,
    mod_files: &[String],
    progress: &Progress,
) -> Result<()> {
    let version_dir = paths.versions.join(version_guid);
    let mut restore: HashMap<String, Vec<String>> = HashMap::new();

    for old in &dist_state.mod_manifest {
        if old.eq_ignore_ascii_case("README.txt") || old.ends_with(".lock") {
            continue;
        }
        if mod_files.iter().any(|file| file.eq_ignore_ascii_case(old)) {
            continue;
        }

        let mut package_found = None;
        for (package_name, directory) in &app.package_map {
            if directory.is_empty() {
                continue;
            }
            let normalized = directory.trim_end_matches('\\');
            let prefix = format!("{}\\", normalized.to_lowercase());
            if old.to_lowercase().starts_with(&prefix) {
                let relative = old[normalized.len() + 1..].to_string();
                package_found = Some((package_name.to_string(), relative));
                break;
            }
        }

        match package_found {
            Some((package_name, relative)) => {
                restore.entry(package_name).or_default().push(relative);
            }
            None => {
                let destination = version_dir.join(old);
                if destination.is_file() {
                    util::clear_readonly(&destination);
                    let _ = fs::remove_file(destination);
                }
            }
        }
    }

    for (package_name, files) in restore {
        let Some(package) = manifest.find(&package_name) else {
            continue;
        };
        progress.set_status(format!("Restoring {package_name}"));
        let archive = download_package(
            deployment,
            version_guid,
            package,
            &paths.downloads,
            progress,
        )?;
        let directory = app.package_directory(&package_name).unwrap_or("");
        extract_package(&archive, &version_dir.join(directory), Some(&files))?;
    }

    Ok(())
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(directory) = stack.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn cleanup_versions(paths: &Paths, states: &[&DistributionState]) {
    if !paths.versions.is_dir() {
        return;
    }
    let keep: Vec<&str> = states
        .iter()
        .map(|state| state.version_guid.as_str())
        .filter(|guid| !guid.is_empty())
        .collect();

    let Ok(entries) = fs::read_dir(&paths.versions) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !keep.contains(&name.as_str()) {
            crate::logger::log("Bootstrapper", format!("Deleting old version {name}"));
            let _ = util::remove_dir_all_force(&path);
        }
    }
}

fn start_roblox(
    paths: &Paths,
    app: &AppData,
    version_guid: &str,
    request: &LaunchRequest,
    settings: &Settings,
    progress: &Progress,
) -> Result<()> {
    progress.set_status("Starting Roblox");
    let version_dir = paths.versions.join(version_guid);
    let executable = version_dir.join(app.executable_name);

    if !executable.is_file() {
        bail!("{} is missing", executable.display());
    }

    if request.launch_mode == LaunchMode::StudioAuth {
        spawn_child(&executable, &request.launch_args, &version_dir)?;
        return Ok(());
    }

    let pid = if should_run_as_admin(&executable) {
        shell_execute_runas(&executable, &request.launch_args, &version_dir)?;
        crate::logger::log("Bootstrapper", "Started Roblox elevated");
        return Ok(());
    } else {
        spawn_child(&executable, &request.launch_args, &version_dir)?
    };

    crate::logger::log(
        "Bootstrapper",
        format!("Started {} (PID {pid})", app.product_name),
    );

    if app.kind != AppKind::Player {
        return Ok(());
    }

    let log_file = wait_for_log(Duration::from_secs(15));
    if log_file.is_none() {
        crate::logger::warn("Bootstrapper", "Could not find the Roblox log file");
    }

    if settings.enable_activity_tracking
        || settings.use_discord_rich_presence
        || settings.show_server_details
    {
        spawn_watcher(pid, log_file);
    }

    std::thread::sleep(Duration::from_millis(1000));
    Ok(())
}

fn spawn_child(executable: &Path, arguments: &str, working_dir: &Path) -> Result<u32> {
    let mut command = Command::new(executable);
    command.current_dir(working_dir);
    if !arguments.is_empty() {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.raw_arg(arguments);
        }
        #[cfg(not(windows))]
        {
            command.arg(arguments);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let child = command.spawn()?;
    Ok(child.id())
}

fn wait_for_log(timeout: Duration) -> Option<PathBuf> {
    let directory = util::local_app_data().join("Roblox\\logs");
    let started = Instant::now();

    while started.elapsed() < timeout {
        if let Ok(entries) = fs::read_dir(&directory) {
            let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("log") {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if !name.contains("player") {
                    continue;
                }
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                let created = metadata
                    .created()
                    .or_else(|_| metadata.modified())
                    .ok();
                let Some(created) = created else {
                    continue;
                };
                if created.elapsed().unwrap_or_default() > Duration::from_secs(20) {
                    continue;
                }
                if best
                    .as_ref()
                    .map(|(_, best_time)| created > *best_time)
                    .unwrap_or(true)
                {
                    best = Some((path, created));
                }
            }
            if let Some((path, _)) = best {
                return Some(path);
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    None
}

fn spawn_watcher(process_id: u32, log_file: Option<PathBuf>) {
    let data = WatcherData {
        process_id,
        log_file: log_file.map(|path| path.to_string_lossy().to_string()),
        autoclose_pids: Vec::new(),
    };
    let Ok(json) = serde_json::to_string(&data) else {
        return;
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(json);

    let executable = util::current_exe();
    let mut command = Command::new(executable);
    command.args(["-watcher", &encoded]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    match command.spawn() {
        Ok(_) => crate::logger::log("Bootstrapper", "Started watcher process"),
        Err(error) => crate::logger::error("Bootstrapper", format!("watcher: {error}")),
    }
}

fn should_run_as_admin(executable: &Path) -> bool {
    let name = executable.to_string_lossy().to_string();
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let key = RegKey::predef(root).open_subkey(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers",
        );
        if let Ok(key) = key {
            if let Ok(flags) = key.get_value::<String, _>(&name) {
                if flags.to_uppercase().contains("RUNASADMIN") {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(windows)]
fn shell_execute_runas(executable: &Path, arguments: &str, working_dir: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    let operation = wide(std::ffi::OsStr::new("runas"));
    let file = wide(executable.as_os_str());
    let parameters = wide(std::ffi::OsStr::new(arguments));
    let directory = wide(working_dir.as_os_str());

    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            parameters.as_ptr(),
            directory.as_ptr(),
            SW_SHOWNORMAL,
        )
    };

    if result as isize <= 32 {
        bail!("Failed to start Roblox as administrator");
    }
    Ok(())
}

#[cfg(not(windows))]
fn shell_execute_runas(_executable: &Path, _arguments: &str, _working_dir: &Path) -> Result<()> {
    Err(anyhow::anyhow!("Elevation is only supported on Windows"))
}
