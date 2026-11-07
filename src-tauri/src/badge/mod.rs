//! Taskbar badge: the 5h cost and its reset time as text on the taskbar, left of the tray, like the weather button.
//! Windows only; elsewhere the tray icon alone shows the number.
mod text;
#[cfg(windows)]
mod paint;
#[cfg(windows)]
mod win;

pub use text::{lines, Lines};
#[cfg(windows)]
pub use win::{enable, init, update};

/// No taskbar badge outside Windows.
#[cfg(not(windows))]
pub fn init(_on_click: impl Fn() + 'static, _enabled: bool) {}
/// No taskbar badge outside Windows.
#[cfg(not(windows))]
pub fn enable(_on: bool) {}
/// No taskbar badge outside Windows.
#[cfg(not(windows))]
pub fn update(_lines: Lines) {}
