use md5::{Digest, Md5};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(value)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, data)?;
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(tmp, path)
}

pub fn ensure_dir(path: &Path) -> io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

pub fn local_app_data() -> PathBuf {
    PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| "C:\\".into()))
}

pub fn user_profile() -> PathBuf {
    PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_else(|| "C:\\".into()))
}

pub fn desktop_dir() -> PathBuf {
    user_profile().join("Desktop")
}

pub fn start_menu_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_else(|| "C:\\".into()))
        .join("Microsoft\\Windows\\Start Menu\\Programs")
}

pub fn windows_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
}

pub fn current_exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ruststrap.exe"))
}

pub fn md5_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Md5::new();
    let mut buffer = [0u8; 128 * 1024];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn clear_readonly(path: &Path) {
    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        if permissions.readonly() {
            permissions.set_readonly(false);
            let _ = fs::set_permissions(path, permissions);
        }
    }
}

pub fn remove_dir_all_force(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    fn clear(path: &Path) {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                clear_readonly(&p);
                if p.is_dir() {
                    clear(&p);
                }
            }
        }
    }

    clear(path);
    clear_readonly(path);
    fs::remove_dir_all(path)
}

pub fn copy_force(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    clear_readonly(to);
    fs::copy(from, to)?;
    Ok(())
}

pub fn open_path(path: &Path) {
    let _ = std::process::Command::new("explorer.exe").arg(path).spawn();
}

pub fn run_powershell(script: &str) -> io::Result<std::process::ExitStatus> {
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .status()
}

pub fn kill_process_by_name(name: &str) {
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", &format!("{name}.exe")])
        .status();
}

pub fn pick_folder(initial: &Path) -> Option<PathBuf> {
    let initial = initial.to_string_lossy().replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; \
         $d=New-Object System.Windows.Forms.FolderBrowserDialog; \
         $d.SelectedPath='{initial}'; \
         if($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK){{ Write-Output $d.SelectedPath }}"
    );

    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .ok()?;

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

pub fn message_box(title: &str, text: &str, error: bool) {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, MB_ICONERROR, MB_ICONINFORMATION, MB_OK,
        };

        let title: Vec<u16> = std::ffi::OsStr::new(title)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let text: Vec<u16> = std::ffi::OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let icon = if error { MB_ICONERROR } else { MB_ICONINFORMATION };

        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                MB_OK | icon,
            );
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (title, text, error);
    }
}

pub fn confirm(title: &str, text: &str) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, IDOK, MB_ICONWARNING, MB_OKCANCEL,
        };

        let title: Vec<u16> = std::ffi::OsStr::new(title)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let text: Vec<u16> = std::ffi::OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let result = unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                MB_OKCANCEL | MB_ICONWARNING,
            )
        };
        result == IDOK
    }
    #[cfg(not(windows))]
    {
        let _ = (title, text);
        true
    }
}

pub fn process_running(name: &str) -> bool {
    let filter = format!("IMAGENAME eq {name}.exe");
    let Ok(output) = std::process::Command::new("tasklist")
        .args(["/FI", &filter, "/NH"])
        .output()
    else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout)
        .to_lowercase()
        .contains(&format!("{}.exe", name.to_lowercase()))
}

pub fn process_exists(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                false
            } else {
                CloseHandle(handle);
                true
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        false
    }
}
