use chrono::Local;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_FILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();
static CONSOLE: OnceLock<bool> = OnceLock::new();

pub fn init(path: &Path, console: bool) {
    let _ = LOG_PATH.set(path.to_path_buf());
    let _ = CONSOLE.set(console);

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok();

    let _ = LOG_FILE.set(Mutex::new(file));
}

pub fn path() -> Option<&'static Path> {
    LOG_PATH.get().map(|p| p.as_path())
}

pub fn log(target: &str, message: impl AsRef<str>) {
    write_line("INFO", target, message.as_ref());
}

pub fn warn(target: &str, message: impl AsRef<str>) {
    write_line("WARN", target, message.as_ref());
}

pub fn error(target: &str, message: impl std::fmt::Display) {
    write_line("ERROR", target, &message.to_string());
}

fn write_line(level: &str, target: &str, message: &str) {
    let line = format!(
        "{} [{}] [{}] {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        level,
        target,
        message
    );

    if CONSOLE.get().copied().unwrap_or(true) {
        if level == "ERROR" {
            eprint!("{line}");
        } else {
            print!("{line}");
        }
    }

    if let Some(mutex) = LOG_FILE.get() {
        if let Ok(mut guard) = mutex.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.write_all(line.as_bytes());
                let _ = file.flush();
            }
        }
    }
}
