//! Flyout footer commands: the "Start with Windows" switch and Quit.
use tauri::{AppHandle, Runtime};
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

#[tauri::command]
pub fn quit<R: Runtime>(app: AppHandle<R>) {
    app.exit(0);
}
