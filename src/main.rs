#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod appdata;
mod bootstrapper;
mod cli;
mod config;
mod deployment;
mod discord;
mod fastflags;
mod installer;
mod logger;
mod packages;
mod paths;
mod progress;
mod ui;
mod util;
mod watcher;

use std::path::PathBuf;

use bootstrapper::LaunchRequest;
use cli::{LaunchMode, LaunchSettings};
use paths::{Paths, PROJECT_NAME, PROJECT_VERSION};
use progress::Progress;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let launch = LaunchSettings::parse(args);

    let installed = installed_base();
    let log_base = installed
        .clone()
        .map(|base| base.join("Logs"))
        .unwrap_or_else(|| Paths::temp().join("Logs"));

    logger::init(&log_base.join("Ruststrap.log"), cfg!(debug_assertions));
    logger::log(
        "App",
        format!(
            "Starting {PROJECT_NAME} v{PROJECT_VERSION} ({}), args: {:?}",
            std::env::consts::ARCH,
            launch.raw
        ),
    );

    if launch.watcher {
        match installed {
            Some(base) => {
                if let Err(error) = watcher::run(Paths::new(base), launch.watcher_data.clone()) {
                    logger::error("App", format!("{error:#}"));
                }
            }
            None => logger::warn("App", "Watcher started but Ruststrap is not installed"),
        }
        return;
    }

    if launch.background_updater {
        if let Some(base) = installed {
            run_quiet(Paths::new(base), &launch);
        }
        return;
    }

    if launch.uninstall && launch.quiet {
        if let Err(error) = installer::do_uninstall(true) {
            logger::error("App", format!("{error:#}"));
            util::message_box(PROJECT_NAME, &format!("{error:#}"), true);
        }
        return;
    }

    let Some(base) = installed else {
        if launch.quiet {
            let location = paths::default_install_location();
            if let Err(error) = installer::do_install(&location, true, false) {
                logger::error("App", format!("{error:#}"));
                util::message_box(PROJECT_NAME, &format!("{error:#}"), true);
                return;
            }
            if launch.launch_mode != LaunchMode::None {
                run_quiet(Paths::new(location), &launch);
            }
            return;
        }

        run_gui(launch);
        return;
    };

    if launch.quiet {
        if launch.uninstall {
            if let Err(error) = installer::do_uninstall(true) {
                logger::error("App", format!("{error:#}"));
                util::message_box(PROJECT_NAME, &format!("{error:#}"), true);
            }
        } else if launch.launch_mode != LaunchMode::None {
            run_quiet(Paths::new(base), &launch);
        }
        return;
    }

    run_gui(launch);
}

fn installed_base() -> Option<PathBuf> {
    installer::get_install_location().filter(|base| {
        base.join(format!("{PROJECT_NAME}.exe")).is_file()
    })
}

fn run_quiet(paths: Paths, launch: &LaunchSettings) {
    let request = LaunchRequest {
        launch_mode: launch.launch_mode,
        launch_args: launch.roblox_launch_args.clone(),
        channel: launch.channel.clone(),
        version_guid: launch.version.clone(),
        force: launch.force,
        no_launch: launch.no_launch,
    };

    let progress = Progress::new();
    if let Err(error) = bootstrapper::run(paths, request, progress) {
        logger::error("App", format!("{error:#}"));
        util::message_box(PROJECT_NAME, &format!("{error:#}"), true);
    }
}

fn run_gui(launch: LaunchSettings) {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([980.0, 640.0])
            .with_min_inner_size([760.0, 500.0])
            .with_title(PROJECT_NAME),
        ..Default::default()
    };

    let result = eframe::run_native(
        PROJECT_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(ui::RuststrapApp::new(cc, launch)))),
    );

    if let Err(error) = result {
        logger::error("App", format!("GUI error: {error}"));
        util::message_box(PROJECT_NAME, &format!("Failed to start the interface: {error}"), true);
    }
}
