//! The one stored preference: whether the taskbar badge shows. A marker file, so there is no format to go stale.
use std::io;
use std::path::Path;

const BADGE_OFF: &str = "badge-off";

/// On unless the user turned it off.
pub fn badge_on(dir: &Path) -> bool {
    !dir.join(BADGE_OFF).exists()
}

/// Remembers the choice: on removes the marker, off writes it.
pub fn set_badge(dir: &Path, on: bool) -> io::Result<()> {
    let marker = dir.join(BADGE_OFF);
    if on {
        match std::fs::remove_file(marker) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    } else {
        std::fs::create_dir_all(dir)?;
        std::fs::write(marker, b"")
    }
}
