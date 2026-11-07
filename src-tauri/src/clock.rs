//! Wall-clock times in the user's own Windows short-time format ("5:00 PM" or "17:00"), so they match the taskbar clock.
use chrono::{DateTime, Datelike, Local, Timelike};

/// `t` as hours and minutes, the way the taskbar clock writes them.
pub fn clock(t: DateTime<Local>) -> String {
    #[cfg(windows)]
    if let Some(s) = system(t) {
        return s;
    }
    t.format("%H:%M").to_string()
}

/// GetTimeFormatEx with the user's locale; `None` only if Windows rejects the time.
#[cfg(windows)]
fn system(t: DateTime<Local>) -> Option<String> {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::Globalization::{GetTimeFormatEx, TIME_NOSECONDS};
    let st = SYSTEMTIME {
        wYear: t.year() as u16,
        wMonth: t.month() as u16,
        wDayOfWeek: 0,
        wDay: t.day() as u16,
        wHour: t.hour() as u16,
        wMinute: t.minute() as u16,
        wSecond: 0,
        wMilliseconds: 0,
    };
    let mut buf = [0u16; 64];
    // SAFETY: null locale = the user's default; writes at most 64 UTF-16 units into `buf`.
    let n = unsafe { GetTimeFormatEx(std::ptr::null(), TIME_NOSECONDS, &st, std::ptr::null(), buf.as_mut_ptr(), 64) };
    (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
}
