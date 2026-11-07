//! Juice: tray meter for local Claude Code token usage and estimated cost (unofficial).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agg;
#[cfg(windows)]
mod autostart;
mod badge;
mod clock;
mod commands;
mod icon;
mod log;
mod model;
mod parse;
mod place;
mod price;
mod settings;
mod store;
mod tray;
mod worker;

use std::path::PathBuf;
use tauri::{Manager, RunEvent};

/// Claude Code's projects folder: `$CLAUDE_CONFIG_DIR/projects`, else `~/.claude/projects`.
fn logs_root(home: tauri::Result<PathBuf>) -> tauri::Result<PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        Some(dir) => Ok(PathBuf::from(dir).join("projects")),
        None => Ok(home?.join(".claude").join("projects")),
    }
}

/// Tray icon and badge first, the log worker next; the flyout is built on first open.
fn main() {
    tauri::Builder::default()
        // First plugin, so a second launch exits before building anything: it only opens this copy's flyout.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| tray::show(app)))
        .invoke_handler(tauri::generate_handler![
            tray::fit,
            tray::snapshot,
            commands::autostart,
            commands::set_autostart,
            commands::badge,
            commands::set_badge,
            commands::log,
            commands::quit,
            tray::close
        ])
        .setup(|app| {
            log::init(app.path().app_config_dir()?);
            tray::create(app.handle(), icon::loading())?;
            let (clicked, woke) = (app.handle().clone(), app.handle().clone());
            let on = settings::badge_on(&app.path().app_config_dir()?);
            badge::init(move || tray::toggle(&clicked), move || tray::tick(&woke), on);
            let handle = app.handle().clone();
            let tx = worker::spawn(logs_root(app.path().home_dir())?, move |s| tray::publish(&handle, s));
            app.manage(tx);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("juice: tauri runtime failed to start")
        .run(|_, event| {
            // Closing the flyout (the only window) must not end the tray app; only Quit (an exit code) does.
            if let RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
