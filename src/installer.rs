use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
use winreg::RegKey;

use crate::paths::{Paths, PROJECT_NAME, PROJECT_VERSION};
use crate::util;

pub const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Ruststrap";

pub fn get_install_location() -> Option<PathBuf> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey_with_flags(UNINSTALL_KEY, KEY_READ).ok()?;
    let location: String = key.get_value("InstallLocation").ok()?;

    if location.is_empty() {
        return None;
    }

    let path = PathBuf::from(location);
    if path.is_dir() {
        Some(path)
    } else {
        None
    }
}

pub fn is_installed() -> bool {
    get_install_location()
        .map(|base| base.join(format!("{PROJECT_NAME}.exe")).is_file())
        .unwrap_or(false)
}

pub fn validate_location(location: &Path) -> Result<()> {
    let text = location.to_string_lossy();
    let length = text.trim_end_matches('\\').len();

    if length <= 3 {
        bail!("Cannot install to the root of a drive.");
    }
    if text.starts_with(r"\\") {
        bail!("Network locations are not supported.");
    }
    if text.to_lowercase().contains("\\temp\\") || text.to_lowercase().contains("\\windows\\") {
        bail!("This location cannot be used.");
    }
    if text.to_lowercase().contains("onedrive") {
        bail!("Installing inside OneDrive is not supported.");
    }
    if text.to_lowercase().contains("program files") {
        bail!("Installing into Program Files is not supported.");
    }
    if text.eq_ignore_ascii_case(&util::user_profile().to_string_lossy()) {
        bail!("Choose a subfolder instead of your user profile root.");
    }

    Ok(())
}

fn write_test(location: &Path) -> Result<()> {
    std::fs::create_dir_all(location)
        .with_context(|| format!("creating {}", location.display()))?;
    let test = location.join("RuststrapWriteTest.txt");
    std::fs::write(&test, b"test")
        .with_context(|| format!("writing to {}", location.display()))?;
    let _ = std::fs::remove_file(test);
    Ok(())
}

pub fn do_install(
    location: &Path,
    create_shortcuts: bool,
    _enable_analytics: bool,
) -> Result<()> {
    validate_location(location)?;
    write_test(location)?;

    let paths = Paths::new(location.to_path_buf());
    std::fs::create_dir_all(&paths.base)?;

    let process = util::current_exe();
    if process != paths.application {
        if paths.application.exists() {
            util::clear_readonly(&paths.application);
        }
        std::fs::copy(&process, &paths.application)
            .with_context(|| format!("copying executable to {}", paths.application.display()))?;
    }

    write_uninstall_registry(&paths)?;
    register_player(&paths.application)?;

    let studio_installed = stock_install_location("roblox-studio").is_some()
        || store_studio_installed(&paths);
    if studio_installed {
        let _ = register_studio(&paths.application);
    }

    if create_shortcuts {
        let _ = create_shortcut(
            &paths.application,
            &util::desktop_dir().join(format!("{PROJECT_NAME}.lnk")),
        );
        let _ = create_shortcut(
            &paths.application,
            &util::start_menu_dir().join(format!("{PROJECT_NAME}.lnk")),
        );
    }

    let store = crate::config::ConfigStore::new(&paths.base);
    let mut settings = store.load_settings();
    if settings.install_location.is_empty() {
        settings.install_location = paths.base.to_string_lossy().to_string();
    }
    store.save_settings(&settings);
    if !store.state_path().exists() {
        store.save_state(&crate::config::State::default());
    }

    crate::logger::log(
        "Installer",
        format!("Installed to {}", paths.base.display()),
    );
    Ok(())
}

fn write_uninstall_registry(paths: &Paths) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(UNINSTALL_KEY)?;

    key.set_value("DisplayIcon", &format!("{},0", paths.application.display()))?;
    key.set_value("DisplayName", &PROJECT_NAME)?;
    key.set_value("DisplayVersion", &PROJECT_VERSION)?;
    key.set_value(
        "InstallDate",
        &chrono::Local::now().format("%Y%m%d").to_string(),
    )?;
    key.set_value("InstallLocation", &paths.base.to_string_lossy().to_string())?;
    key.set_value("NoRepair", &1u32)?;
    key.set_value("Publisher", &"Ruststrap contributors")?;
    key.set_value(
        "ModifyPath",
        &format!("\"{}\" -settings", paths.application.display()),
    )?;
    key.set_value(
        "QuietUninstallString",
        &format!("\"{}\" -uninstall -quiet", paths.application.display()),
    )?;
    key.set_value(
        "UninstallString",
        &format!("\"{}\" -uninstall", paths.application.display()),
    )?;
    key.set_value("URLInfoAbout", &"https://github.com/")?;

    Ok(())
}

pub fn register_player(exe: &Path) -> Result<()> {
    let handler = exe.to_string_lossy().to_string();
    register_protocol("roblox", "Roblox", &handler, "-player \"%1\"")?;
    register_protocol("roblox-player", "Roblox", &handler, "-player \"%1\"")?;
    Ok(())
}

pub fn register_studio(exe: &Path) -> Result<()> {
    let handler = exe.to_string_lossy().to_string();
    register_protocol("roblox-studio", "Roblox", &handler, "-studio \"%1\"")?;
    register_protocol("roblox-studio-auth", "Roblox", &handler, "-studio \"%1\"")?;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (place_key, _) = hkcu.create_subkey(r"Software\Classes\Roblox.Place")?;
    place_key.set_value("", &"Roblox Place")?;
    let (icon_key, _) = place_key.create_subkey("DefaultIcon")?;
    icon_key.set_value("", &format!("{},0", exe.display()))?;
    let (open_key, _) = place_key.create_subkey(r"shell\Open")?;
    open_key.set_value("", &"Open")?;
    let (command_key, _) = open_key.create_subkey("command")?;
    command_key.set_value("", &format!("\"{}\" -studio \"%1\"", exe.display()))?;

    for extension in [".rbxl", ".rbxlx"] {
        let (ext_key, _) = hkcu.create_subkey(format!(r"Software\Classes\{extension}"))?;
        ext_key.set_value("", &"Roblox.Place")?;
        let _ = ext_key.create_subkey(r"Roblox.Place\ShellNew");
    }

    Ok(())
}

fn register_protocol(key_name: &str, name: &str, handler: &str, args: &str) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(format!(r"Software\Classes\{key_name}"))?;
    let _ = key.set_value("", &format!("URL: {name} Protocol"));
    let _ = key.set_value("URL Protocol", &"");
    let (icon, _) = key.create_subkey("DefaultIcon")?;
    let _ = icon.set_value("", &handler);
    let (command, _) = key.create_subkey(r"shell\open\command")?;
    command.set_value("", &format!("\"{handler}\" {args}"))?;
    Ok(())
}

pub fn unregister_protocols() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for key in [
        "roblox",
        "roblox-player",
        "roblox-studio",
        "roblox-studio-auth",
        "Roblox.Place",
        ".rbxl",
        ".rbxlx",
    ] {
        let _ = hkcu.delete_subkey_all(format!(r"Software\Classes\{key}"));
    }
}

/// Restores the protocol handlers of the stock Roblox installation, if one is
/// present. Called during uninstall so that removing Ruststrap does not leave
/// the user without working roblox:// links.
pub fn restore_stock_protocols() {
    if let Some(location) = stock_install_location("roblox-player") {
        let handler = location.join("RobloxPlayerBeta.exe");
        let handler = handler.to_string_lossy();
        let _ = register_protocol("roblox", "Roblox", &handler, "\"%1\"");
        let _ = register_protocol("roblox-player", "Roblox", &handler, "\"%1\"");
    }

    if let Some(location) = stock_install_location("roblox-studio") {
        let handler = location.join("RobloxStudioBeta.exe");
        let handler = handler.to_string_lossy();
        let _ = register_protocol("roblox-studio", "Roblox", &handler, "\"%1\"");
        let _ = register_protocol("roblox-studio-auth", "Roblox", &handler, "\"%1\"");

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok((place_key, _)) = hkcu.create_subkey(r"Software\Classes\Roblox.Place") {
            let _ = place_key.set_value("", &"Roblox Place");
            if let Ok((icon_key, _)) = place_key.create_subkey("DefaultIcon") {
                let _ = icon_key.set_value("", &format!("{handler},0"));
            }
            if let Ok((open_key, _)) = place_key.create_subkey(r"shell\Open") {
                let _ = open_key.set_value("", &"Open");
                if let Ok((command_key, _)) = open_key.create_subkey("command") {
                    let _ =
                        command_key.set_value("", &format!("\"{handler}\" -ide \"%1\""));
                }
            }
        }

        for extension in [".rbxl", ".rbxlx"] {
            if let Ok((ext_key, _)) =
                hkcu.create_subkey(format!(r"Software\Classes\{extension}"))
            {
                let _ = ext_key.set_value("", &"Roblox.Place");
                let _ = ext_key.create_subkey(r"Roblox.Place\ShellNew");
            }
        }
    }
}

fn stock_install_location(uninstall_key: &str) -> Option<PathBuf> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(format!(
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{uninstall_key}"
        ))
        .ok()?;
    let location: String = key.get_value("InstallLocation").ok()?;
    let path = PathBuf::from(location);
    path.is_dir().then_some(path)
}

fn store_studio_installed(paths: &Paths) -> bool {
    crate::util::read_json::<crate::config::DistributionState>(
        &paths.base.join("StudioState.json"),
    )
    .map(|state| !state.version_guid.is_empty())
    .unwrap_or(false)
}

pub fn create_shortcut(target: &Path, shortcut: &Path) -> Result<()> {
    if let Some(parent) = shortcut.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let target = target.to_string_lossy().replace('\'', "''");
    let shortcut_path = shortcut.to_string_lossy().replace('\'', "''");
    let working_dir = Path::new(&target)
        .parent()
        .map(|p| p.to_string_lossy().replace('\'', "''"))
        .unwrap_or_default();

    let script = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{shortcut_path}');\
         $s.TargetPath='{target}';\
         $s.WorkingDirectory='{working_dir}';\
         $s.IconLocation='{target},0';\
         $s.Save()"
    );

    let status = util::run_powershell(&script)?;
    if !status.success() {
        bail!("PowerShell failed to create shortcut");
    }
    Ok(())
}

pub fn do_uninstall(keep_data: bool) -> Result<()> {
    let Some(base) = get_install_location() else {
        bail!("Ruststrap is not installed");
    };
    let paths = Paths::new(base.clone());

    util::kill_process_by_name("RobloxPlayerBeta");
    util::kill_process_by_name("RobloxStudioBeta");

    let _ = std::fs::remove_file(util::desktop_dir().join(format!("{PROJECT_NAME}.lnk")));
    let _ = std::fs::remove_file(util::start_menu_dir().join(format!("{PROJECT_NAME}.lnk")));

    unregister_protocols();
    restore_stock_protocols();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(UNINSTALL_KEY);

    if keep_data {
        for dir in [&paths.versions, &paths.downloads] {
            let _ = util::remove_dir_all_force(dir);
        }
        for file in [
            paths.base.join("State.json"),
            paths.base.join("PlayerState.json"),
            paths.base.join("StudioState.json"),
        ] {
            let _ = std::fs::remove_file(file);
        }
        // The running executable cannot be deleted, so schedule cleanup.
        schedule_self_delete(&base, false);
    } else {
        schedule_self_delete(&base, true);
    }

    crate::logger::log("Installer", "Uninstalled Ruststrap");
    Ok(())
}

/// Deletes the running executable (or the whole installation folder) after this
/// process exits. Windows locks the image of a running executable, so cleanup
/// has to be delegated to a short-lived cmd.exe process.
fn schedule_self_delete(base: &Path, delete_all: bool) {
    let base_text = base.to_string_lossy().replace('"', "");
    let exe = format!("{base_text}\\{PROJECT_NAME}.exe").replace('"', "");
    // Retry for up to two minutes so the cleanup still works when the GUI stays
    // open on the "uninstalled" screen instead of exiting immediately.
    let command = if delete_all {
        format!(
            "for /l %i in (1,1,120) do (del /f /q \"{exe}\" >nul 2>&1 & \
             if not exist \"{exe}\" (rmdir /s /q \"{base_text}\" >nul 2>&1 & exit /b 0) & \
             timeout 1 >nul)"
        )
    } else {
        format!(
            "for /l %i in (1,1,120) do (del /f /q \"{exe}\" >nul 2>&1 & \
             if not exist \"{exe}\" exit /b 0 & timeout 1 >nul)"
        )
    };

    let mut cmd = std::process::Command::new("cmd.exe");
    cmd.args(["/C", &command])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let _ = cmd.spawn();
}
