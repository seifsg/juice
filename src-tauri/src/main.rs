//! Juice: tray meter for local Claude Code token usage and estimated cost (unofficial).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod place;
mod tray;

use tauri::image::Image;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![tray::fit])
        .setup(|app| {
            let tile = vec![0xFF, 0x8A, 0x1F, 0xFF].repeat(32 * 32);
            tray::create(app.handle(), Image::new_owned(tile, 32, 32))?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("juice: tauri runtime failed to start");
}
