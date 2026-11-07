//! Juice: tray meter for local Claude Code token usage and estimated cost (unofficial).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agg;
mod badge;
mod commands;
mod icon;
mod model;
mod parse;
mod place;
mod price;
mod settings;
mod store;
mod tray;
mod worker;

use std::path::PathBuf;
use tauri::Manager;

/// Claude Code's projects folder: `$CLAUDE_CONFIG_DIR/projects`, else `~/.claude/projects`.
fn logs_root(home: tauri::Result<PathBuf>) -> tauri::Result<PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        Some(dir) => Ok(PathBuf::from(dir).join("projects")),
        None => Ok(home?.join(".claude").join("projects")),
    }
}

fn main() {
    tauri::Builder::default()
        // First plugin, so a second launch exits before building anything: it only opens this copy's flyout.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| tray::show(app)))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            tray::fit,
            tray::snapshot,
            commands::autostart,
            commands::set_autostart,
            commands::badge,
            commands::set_badge,
            commands::quit
        ])
        .setup(|app| {
            tray::create(app.handle(), icon::loading())?;
            let handle = app.handle().clone();
            let on = settings::badge_on(&app.path().app_config_dir()?);
            badge::init(move || tray::toggle(&handle), on);
            let handle = app.handle().clone();
            let tx = worker::spawn(logs_root(app.path().home_dir())?, move |s| tray::publish(&handle, s));
            app.manage(tx);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("juice: tauri runtime failed to start");
}
