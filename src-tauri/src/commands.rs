//! Flyout footer commands: the "Start with Windows" and "Show on taskbar" switches, and Quit.
use crate::{badge, settings};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_autostart::ManagerExt;

/// Whether Juice starts with Windows (HKCU Run entry `Juice`).
#[tauri::command]
pub fn autostart<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

/// Adds or removes the Run entry; returns the state Windows now reports.
#[tauri::command]
pub fn set_autostart<R: Runtime>(app: AppHandle<R>, on: bool) -> Result<bool, String> {
    let a = app.autolaunch();
    if on { a.enable() } else { a.disable() }.and_then(|()| a.is_enabled()).map_err(|e| e.to_string())
}

fn config_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path().app_config_dir().map_err(|e| e.to_string())
}

/// Whether the taskbar badge shows.
#[tauri::command]
pub fn badge<R: Runtime>(app: AppHandle<R>) -> Result<bool, String> {
    Ok(settings::badge_on(&config_dir(&app)?))
}

/// Shows or hides the taskbar badge and remembers the choice.
#[tauri::command]
pub fn set_badge<R: Runtime>(app: AppHandle<R>, on: bool) -> Result<bool, String> {
    settings::set_badge(&config_dir(&app)?, on).map_err(|e| e.to_string())?;
    app.run_on_main_thread(move || badge::enable(on)).map_err(|e| e.to_string())?;
    Ok(on)
}

#[tauri::command]
pub fn quit<R: Runtime>(app: AppHandle<R>) {
    app.exit(0);
}
