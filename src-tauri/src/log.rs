//! Problems found while running go to `juice.log` in the app's config folder: release builds have no console.
use chrono::Local;
use std::fmt::Display;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Past this size the log starts over, so it never grows without bound.
const MAX_BYTES: u64 = 256 * 1024;
static FILE: OnceLock<PathBuf> = OnceLock::new();

/// Sets the log folder; before this (and in tests) messages go to stderr.
pub fn init(dir: PathBuf) {
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("juice: no log folder {}: {e}", dir.display());
    }
    let _ = FILE.set(dir.join("juice.log"));
}

/// Appends one timestamped line.
pub fn warn(msg: impl Display) {
    let line = format!("{} {msg}\n", Local::now().format("%Y-%m-%d %H:%M:%S"));
    let Some(path) = FILE.get() else { return eprint!("juice: {line}") };
    let full = fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES);
    let written = OpenOptions::new()
        .create(true)
        .write(true)
        .append(!full)
        .truncate(full)
        .open(path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
    if let Err(e) = written {
        eprintln!("juice: cannot write {}: {e}", path.display());
    }
}
