//! The badge window: a layered child of the taskbar (`Shell_TrayWnd`) placed left of the notification area
//! (`TrayNotifyWnd`). Child windows move, hide and auto-hide with the taskbar for free. Main thread only.
use super::paint::{frame, Style};
use super::Lines;
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use windows_sys::core::w;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowExW, FindWindowW, GetWindowRect,
    GetWindowThreadProcessId, LoadCursorW, RegisterClassW, RegisterWindowMessageW, SetWindowPos, UpdateLayeredWindow,
    EVENT_OBJECT_LOCATIONCHANGE, HWND_TOP, IDC_ARROW, OBJID_WINDOW, SWP_NOACTIVATE, ULW_ALPHA, WINEVENT_OUTOFCONTEXT,
    WM_DESTROY, WM_LBUTTONUP, WM_MOUSEMOVE, WM_SETTINGCHANGE, WNDCLASSW, WS_CHILD, WS_EX_LAYERED,
    WS_POPUP, WS_VISIBLE,
};

const CLASS: *const u16 = w!("JuiceBadge");
/// Sent after TrackMouseEvent(TME_LEAVE); defined here to skip the large `Win32_UI_Controls` feature.
const WM_MOUSELEAVE: u32 = 0x02A3;
/// Gap between the badge and the notification area, at 100% scale.
const GAP: f32 = 8.0;

/// Explorer broadcasts this to top-level windows when the taskbar is (re)created.
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

#[derive(Default)]
struct State {
    on_click: Option<Rc<dyn Fn()>>,
    lines: Option<Lines>,
    enabled: bool,
    hover: bool,
    /// Hidden top-level window: hears TaskbarCreated and theme changes.
    host: HWND,
    taskbar: HWND,
    notify: HWND,
    badge: HWND,
    hook: HWINEVENTHOOK,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Reads or edits the state. Never call Win32 inside `f`: window messages re-enter `proc` synchronously.
fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Registers the window class and the hidden host window; shows the badge when `enabled`.
pub fn init(on_click: impl Fn() + 'static, enabled: bool) {
    // SAFETY: class and window creation with static, NUL-terminated names on the thread that pumps messages.
    let host = unsafe {
        TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::Relaxed);
        let hinstance = GetModuleHandleW(null());
        let class = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: hinstance,
            lpszClassName: CLASS,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..std::mem::zeroed()
        };
        RegisterClassW(&class);
        CreateWindowExW(0, CLASS, w!("Juice"), WS_POPUP, 0, 0, 0, 0, null_mut(), null_mut(), hinstance, null())
    };
    with(|s| (s.on_click, s.host) = (Some(Rc::new(on_click) as Rc<dyn Fn()>), host));
    enable(enabled);
}

/// Shows or removes the badge.
pub fn enable(on: bool) {
    with(|s| s.enabled = on);
    detach();
    if on {
        attach();
    }
}

/// New text; redrawn at once.
pub fn update(lines: Lines) {
    with(|s| s.lines = Some(lines));
    paint();
}

/// Creates the badge inside the current taskbar and follows the notification area's moves.
fn attach() {
    // SAFETY: Win32 calls with checked handles; the hook callback is a plain fn.
    let (taskbar, notify, badge, hook) = unsafe {
        let taskbar = FindWindowW(w!("Shell_TrayWnd"), null());
        let notify = FindWindowExW(taskbar, null_mut(), w!("TrayNotifyWnd"), null());
        if taskbar.is_null() || notify.is_null() {
            return; // No taskbar yet: TaskbarCreated retries.
        }
        let badge = CreateWindowExW(WS_EX_LAYERED, CLASS, w!("Juice"), WS_CHILD | WS_VISIBLE, 0, 0, 1, 1, taskbar,
            null_mut(), GetModuleHandleW(null()), null());
        let mut pid = 0;
        let thread = GetWindowThreadProcessId(taskbar, &mut pid);
        let hook = SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE, null_mut(), Some(moved),
            pid, thread, WINEVENT_OUTOFCONTEXT);
        (taskbar, notify, badge, hook)
    };
    if badge.is_null() {
        crate::log::warn("taskbar badge window failed");
    }
    with(|s| (s.taskbar, s.notify, s.badge, s.hook) = (taskbar, notify, badge, hook));
    paint();
}

/// Removes the badge and its hook.
fn detach() {
    let (badge, hook) = with(|s| {
        s.hover = false;
        (std::mem::replace(&mut s.badge, null_mut()), std::mem::replace(&mut s.hook, null_mut()))
    });
    // SAFETY: both were created by `attach`; null handles are skipped, a dead badge just fails.
    unsafe {
        if !hook.is_null() {
            UnhookWinEvent(hook);
        }
        if !badge.is_null() {
            DestroyWindow(badge);
        }
    }
}

/// Whether the taskbar uses the dark theme (Windows default when the value is missing).
fn dark_taskbar() -> bool {
    let (mut light, mut size) = (0u32, 4u32);
    // SAFETY: reads one DWORD into `light`, with its size in `size`.
    let found = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"), RRF_RT_REG_DWORD, null_mut(), (&raw mut light).cast(), &mut size)
    } == 0;
    !(found && light == 1)
}

/// Screen rectangle of a window.
fn rect(h: HWND) -> RECT {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: writes into `r`; a stale handle leaves it zeroed.
    unsafe { GetWindowRect(h, &mut r) };
    r
}

/// Draws the current text and places the badge just left of the notification area.
fn paint() {
    let (badge, taskbar, notify, hover, lines) = with(|s| (s.badge, s.taskbar, s.notify, s.hover, s.lines.clone()));
    let Some(lines) = lines.filter(|_| !badge.is_null()) else { return };
    let (bar, tray) = (rect(taskbar), rect(notify));
    if bar.right - bar.left < bar.bottom - bar.top {
        return; // Vertical taskbar (Windows 10): no room for text; the tray icon still shows the number.
    }
    // SAFETY: reads the taskbar's DPI; 0 for a stale handle falls back to 96.
    let scale = match unsafe { GetDpiForWindow(taskbar) } {
        0 => 1.0,
        dpi => dpi as f32 / 96.0,
    };
    let style = Style { scale, height: bar.bottom - bar.top, dark: dark_taskbar(), hover };
    let Some(f) = frame(&lines, &style) else { return };
    let x = tray.left - bar.left - f.size.cx - (GAP * scale).round() as i32;
    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
    // SAFETY: `badge` is our live child window; `f` owns a DC with a premultiplied 32-bit bitmap of `f.size`.
    unsafe {
        SetWindowPos(badge, HWND_TOP, x, 0, f.size.cx, f.size.cy, SWP_NOACTIVATE);
        UpdateLayeredWindow(badge, null_mut(), null(), &f.size, f.dc, &POINT { x: 0, y: 0 }, 0, &blend, ULW_ALPHA);
    }
}

/// The notification area grew, shrank or moved: follow it.
unsafe extern "system" fn moved(_: HWINEVENTHOOK, _: u32, hwnd: HWND, object: i32, _: i32, _: u32, _: u32) {
    if object == OBJID_WINDOW && with(|s| s.notify == hwnd) {
        paint();
    }
}

/// Window procedure of both windows: TaskbarCreated and theme changes (host), clicks and hover (badge).
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let is_badge = with(|s| s.badge == hwnd && !hwnd.is_null());
    match msg {
        // Explorer restarted: the old badge died with the old taskbar.
        m if m == TASKBAR_CREATED.load(Ordering::Relaxed) && m != 0 => enable(with(|s| s.enabled)),
        WM_SETTINGCHANGE => paint(),
        WM_LBUTTONUP if is_badge => {
            if let Some(f) = with(|s| s.on_click.clone()) {
                f();
            }
        }
        WM_MOUSEMOVE if is_badge && !with(|s| s.hover) => {
            with(|s| s.hover = true);
            let mut t = TRACKMOUSEEVENT { cbSize: size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
            // SAFETY: `t` describes our live window.
            unsafe { TrackMouseEvent(&mut t) };
            paint();
        }
        WM_MOUSELEAVE if is_badge => {
            with(|s| s.hover = false);
            paint();
        }
        WM_DESTROY if is_badge => with(|s| s.badge = null_mut()),
        // SAFETY: default handling for everything else.
        _ => return unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
    0
}
