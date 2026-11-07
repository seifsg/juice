fn main() {
    // Tauri's default manifest plus a Windows 10/11 compatibility entry (see app.manifest).
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("tauri build script failed");
}
