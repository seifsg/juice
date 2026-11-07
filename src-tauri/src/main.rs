//! Juice: tray meter for local Claude Code token usage and estimated cost (unofficial).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod icon;
mod model;
mod parse;
mod place;
mod price;
mod tray;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![tray::fit])
        .setup(|app| {
            tray::create(app.handle(), icon::tile(0.0, false))?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("juice: tauri runtime failed to start");
}
