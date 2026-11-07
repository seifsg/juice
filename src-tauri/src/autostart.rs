//! "Start with Windows": the `HKCU\...\Run` value `Juice` holding the quoted exe path, plus Task Manager's
//! StartupApproved flag, which can disable a Run entry on its own.
use std::io;
use std::ptr::null_mut;
use windows_sys::core::{w, PCWSTR};
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_BINARY, REG_SZ, RRF_RT_REG_BINARY,
    RRF_RT_REG_SZ,
};

const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const APPROVED: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run");
const NAME: PCWSTR = w!("Juice");
/// StartupApproved value meaning "enabled" (an odd first byte means Task Manager disabled it).
const APPROVED_ON: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

fn check(code: WIN32_ERROR) -> io::Result<()> {
    if code == 0 { Ok(()) } else { Err(io::Error::from_raw_os_error(code as i32)) }
}

/// Whether Windows will start Juice at sign-in.
pub fn enabled() -> bool {
    let (mut approved, mut size) = ([0u8; 12], 12u32);
    // SAFETY: the first call only probes existence; the second writes at most `size` bytes into `approved`.
    unsafe {
        let run = RegGetValueW(HKEY_CURRENT_USER, RUN, NAME, RRF_RT_REG_SZ, null_mut(), null_mut(), null_mut()) == 0;
        let flag = RegGetValueW(HKEY_CURRENT_USER, APPROVED, NAME, RRF_RT_REG_BINARY, null_mut(),
            approved.as_mut_ptr().cast(), &mut size);
        run && (flag != 0 || approved[0] & 1 == 0)
    }
}

/// Adds the Run value with this exe's quoted path (spaces in the path stay one argument), or removes it.
pub fn set(on: bool) -> io::Result<()> {
    if !on {
        // SAFETY: deletes one named value; a missing value is already the wanted state.
        let code = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, NAME) };
        return if code == ERROR_FILE_NOT_FOUND { Ok(()) } else { check(code) };
    }
    let value: Vec<u16> = format!("\"{}\"", std::env::current_exe()?.display()).encode_utf16().chain([0]).collect();
    // SAFETY: both buffers live through the calls and their byte lengths are passed exactly.
    unsafe {
        check(RegSetKeyValueW(HKEY_CURRENT_USER, RUN, NAME, REG_SZ, value.as_ptr().cast(), (value.len() * 2) as u32))?;
        check(RegSetKeyValueW(HKEY_CURRENT_USER, APPROVED, NAME, REG_BINARY, APPROVED_ON.as_ptr().cast(), 12))
    }
}
