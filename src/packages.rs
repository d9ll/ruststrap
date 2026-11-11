use anyhow::{anyhow, bail, Context, Result};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::deployment::{Deployment, Package};
use crate::progress::Progress;
use crate::util;

const MAX_DOWNLOAD_TRIES: u32 = 5;

pub fn download_package(
    deployment: &Deployment,
    version_guid: &str,
    package: &Package,
    downloads: &Path,
    progress: &Progress,
) -> Result<PathBuf> {
    if progress.is_cancelled() {
        bail!("Cancelled");
    }

    fs::create_dir_all(downloads)?;
    let destination = downloads.join(&package.signature);
    let roblox_copy = util::local_app_data()
        .join("Roblox\\Downloads")
        .join(&package.signature);

    if destination.is_file() {
        match util::md5_file(&destination) {
            Ok(hash) if hash == package.signature => {
                crate::logger::log(
                    "Packages",
                    format!("{} already downloaded", package.name),
                );
                progress.add_bytes(package.packed_size);
                return Ok(destination);
            }
            _ => {
                crate::logger::warn(
                    "Packages",
                    format!("{} is corrupted, re-downloading", package.name),
                );
                util::clear_readonly(&destination);
                let _ = fs::remove_file(&destination);
            }
        }
    } else if roblox_copy.is_file() {
        crate::logger::log(
            "Packages",
            format!("Copying cached Roblox package {}", package.signature),
        );
        util::copy_force(&roblox_copy, &destination)?;
        progress.add_bytes(package.packed_size);
        return Ok(destination);
    }

    let url = deployment.location(&format!("/{version_guid}-{}", package.name))?;
    let mut current_url = url;
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 1..=MAX_DOWNLOAD_TRIES {
        if progress.is_cancelled() {
            bail!("Cancelled");
        }

        crate::logger::log(
            "Packages",
            format!(
                "Downloading {} (attempt {attempt}/{MAX_DOWNLOAD_TRIES})",
                package.name
            ),
        );

        match download_once(deployment, &current_url, &destination, package, progress) {
            Ok(()) => {
                if let Err(error) = verify(&destination, &package.signature) {
                    let _ = fs::remove_file(&destination);
                    progress.sub_bytes(package.packed_size);
                    last_error = Some(error);
                } else {
                    crate::logger::log(
                        "Packages",
                        format!("Downloaded {}", package.name),
                    );
                    return Ok(destination);
                }
            }
            Err(error) => {
                let _ = fs::remove_file(&destination);
                progress.sub_bytes(package.packed_size);
                last_error = Some(error);
            }
        }

        if current_url.starts_with("https://") {
            current_url = current_url.replacen("https://", "http://", 1);
        }

        std::thread::sleep(std::time::Duration::from_millis(300 * attempt as u64));
    }

    Err(last_error.unwrap_or_else(|| anyhow!("Failed to download {}", package.name)))
}

fn download_once(
    deployment: &Deployment,
    url: &str,
    destination: &Path,
    package: &Package,
    progress: &Progress,
) -> Result<()> {
    let mut reader = deployment
        .download_reader(url)
        .with_context(|| format!("GET {url}"))?;

    let mut file = File::create(destination)
        .with_context(|| format!("creating {}", destination.display()))?;
    let mut buffer = [0u8; 64 * 1024];

    loop {
        if progress.is_cancelled() {
            let _ = file.flush();
            return Err(anyhow!("Cancelled"));
        }

        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])?;
        progress.add_bytes(read as u64);
    }

    file.flush()?;
    let _ = package;
    Ok(())
}

fn verify(path: &Path, expected: &str) -> Result<()> {
    let actual = util::md5_file(path)?;
    if actual != expected {
        bail!("Checksum mismatch for {} (expected {expected}, got {actual})", path.display());
    }
    Ok(())
}

pub fn extract_package(
    zip_path: &Path,
    destination: &Path,
    only_files: Option<&[String]>,
) -> Result<()> {
    let file = File::open(zip_path)
        .with_context(|| format!("opening {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("reading zip {}", zip_path.display()))?;

    let mut wanted: Option<Vec<String>> = only_files.map(|files| {
        files
            .iter()
            .map(|file| file.replace('\\', "/").to_lowercase())
            .collect()
    });

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().replace('\\', "/");

        if let Some(wanted) = wanted.as_mut() {
            let matches = wanted.iter().any(|wanted_name| {
                name.to_lowercase() == *wanted_name
                    || name.to_lowercase().ends_with(&format!("/{wanted_name}"))
            });
            if !matches {
                continue;
            }
            wanted.retain(|wanted_name| {
                name.to_lowercase() != *wanted_name
                    && !name.to_lowercase().ends_with(&format!("/{wanted_name}"))
            });
        }

        let Some(relative) = entry.enclosed_name() else {
            crate::logger::warn(
                "Packages",
                format!("Skipping unsafe zip entry {}", entry.name()),
            );
            continue;
        };

        let output = destination.join(&relative);

        if entry.is_dir() {
            fs::create_dir_all(&output)?;
            continue;
        }

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }

        util::clear_readonly(&output);
        let mut out = File::create(&output)
            .with_context(|| format!("creating {}", output.display()))?;
        io::copy(&mut entry, &mut out)?;
        out.flush()?;

        if wanted.as_ref().is_some_and(|wanted| wanted.is_empty()) {
            break;
        }
    }

    Ok(())
}

pub fn clean_package_cache(
    downloads: &Path,
    keep_signatures: &[String],
    enabled: bool,
) {
    if !enabled || !downloads.is_dir() {
        return;
    }

    let Ok(entries) = fs::read_dir(downloads) else {
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !keep_signatures.iter().any(|keep| keep == &name) {
            crate::logger::log("Packages", format!("Deleting unused package {name}"));
            let path = entry.path();
            util::clear_readonly(&path);
            let _ = fs::remove_file(path);
        }
    }
}

pub fn free_disk_space(path: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide.push(0);
        let mut free: u64 = 0;
        let result = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if result != 0 {
            Some(free)
        } else {
            None
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}
