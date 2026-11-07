//! Flyout footer commands: the "Start with Windows" and "Show on taskbar" switches, and Quit.
use crate::{badge, settings};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};

#[cfg(windows)]
use crate::autostart;
/// Starting at sign-in is Windows-only for now.
#[cfg(not(windows))]
mod autostart {
    pub fn enabled() -> bool {
        false
    }
    pub fn set(_on: bool) -> std::io::Result<()> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

/// Whether Juice starts with Windows.
#[tauri::command]
pub fn autostart() -> bool {
    autostart::enabled()
}

/// Adds or removes the Run entry; returns the state Windows now reports.
#[tauri::command]
pub fn set_autostart(on: bool) -> Result<bool, String> {
    autostart::set(on).map_err(|e| e.to_string())?;
    Ok(autostart::enabled())
}

/// Juice's settings and log folder.
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

/// The flyout reports its own errors here, so they land in juice.log with the rest.
#[tauri::command]
pub fn log(message: String) {
    crate::log::warn(format_args!("flyout: {message}"));
}

/// Quits Juice: tray icon, badge and flyout.
#[tauri::command]
pub fn quit<R: Runtime>(app: AppHandle<R>) {
    app.exit(0);
}
