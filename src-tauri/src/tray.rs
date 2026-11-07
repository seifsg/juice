//! Tray icon and flyout window: shows each snapshot, routes clicks to the flyout, sizes and places it.
use crate::model::Snapshot;
use crate::place::{place, Area};
use crate::worker::Msg;
use crate::{agg, badge, icon};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};
use tauri::image::Image;
use tauri::tray::{MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::utils::config::WindowEffectsConfig;
use tauri::window::Effect;
use tauri::{
    AppHandle, Emitter, Manager, State, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

pub const TRAY: &str = "juice";
const FLYOUT: &str = "flyout";
/// Flyout width in CSS pixels; height comes from the page via `fit`.
const WIDTH: f64 = 340.0;
/// Gap between flyout and taskbar, CSS pixels.
const GAP: f64 = 12.0;
/// A click right after a blur-hide is the same click that caused it: don't reopen.
const REOPEN_GUARD: Duration = Duration::from_millis(300);
/// The shell's overflow panel takes focus back as it closes; a blur this soon after showing is not the user leaving.
const SHOW_GUARD: Duration = Duration::from_millis(600);
/// Hidden this long, the flyout's webview is closed to hand back its memory (about 165 MB across WebView2's
/// processes); the next open rebuilds it in about 0.4 s.
const RELEASE_AFTER: Duration = Duration::from_secs(60);

/// When the flyout was last shown and last hidden on blur.
#[derive(Default)]
pub struct Moments(Mutex<(Option<Instant>, Option<Instant>)>);

/// Last published snapshot: the flyout's first paint, and identical ones skip the redraw.
#[derive(Default)]
pub struct Latest(Mutex<Option<Snapshot>>);

/// True when `t` happened less than `guard` ago.
fn within(t: Option<Instant>, guard: Duration) -> bool {
    t.is_some_and(|t| t.elapsed() < guard)
}

/// Creates the tray icon; any click toggles the flyout, hovering refreshes the numbers.
pub fn create<R: Runtime>(app: &AppHandle<R>, icon: Image<'_>) -> tauri::Result<TrayIcon<R>> {
    app.manage(Moments::default());
    app.manage(Latest::default());
    TrayIconBuilder::with_id(TRAY)
        .icon(icon)
        .tooltip("Juice")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click { button_state: MouseButtonState::Up, .. } => toggle(tray.app_handle()),
            TrayIconEvent::Enter { .. } => tick(tray.app_handle()),
            _ => {}
        })
        .build(app)
}

/// Asks the worker to recompute: a hover or open after sleep never shows a stale window.
fn tick<R: Runtime>(app: &AppHandle<R>) {
    if let Some(tx) = app.try_state::<Sender<Msg>>() {
        let _ = tx.send(Msg::Tick);
    }
}

/// Shows a new snapshot on the tray icon, its tooltip and the flyout. Called from the worker thread.
pub fn publish<R: Runtime>(app: &AppHandle<R>, snap: Snapshot) {
    {
        // Released before touching the tray: tray calls wait on the main thread, which may want this lock.
        let latest = app.state::<Latest>();
        let mut last = latest.0.lock().unwrap_or_else(PoisonError::into_inner);
        if last.as_ref() == Some(&snap) {
            return;
        }
        *last = Some(snap.clone());
    }
    if let Some(tray) = app.tray_by_id(TRAY) {
        let shown = tray
            .set_icon(Some(icon::tile(snap.window.cost, snap.window_end.is_some())))
            .and_then(|()| tray.set_tooltip(Some(agg::tooltip(&snap))));
        if let Err(e) = shown {
            eprintln!("juice: tray update failed: {e}");
        }
    }
    let lines = badge::lines(&snap);
    if let Err(e) = app.run_on_main_thread(move || badge::update(lines)) {
        eprintln!("juice: badge update failed: {e}");
    }
    if let Err(e) = app.emit_to(FLYOUT, "snapshot", &snap) {
        eprintln!("juice: flyout update failed: {e}");
    }
}

/// The flyout's first paint; later ones arrive as `snapshot` events.
#[tauri::command]
pub fn snapshot(latest: State<'_, Latest>) -> Option<Snapshot> {
    latest.0.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

/// Shows the flyout, or hides it when it is open.
pub fn toggle<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(FLYOUT) {
        if w.is_visible().unwrap_or(false) {
            hide(app, &w);
            return;
        }
    }
    let just_hidden = app.state::<Moments>().0.lock().is_ok_and(|m| within(m.1, REOPEN_GUARD));
    if !just_hidden {
        show(app);
    }
}

/// Opens the flyout above the tray icon, creating its webview on first use.
pub fn show<R: Runtime>(app: &AppHandle<R>) {
    match flyout(app) {
        Ok(w) => {
            if let Ok(mut m) = app.state::<Moments>().0.lock() {
                m.0 = Some(Instant::now());
            }
            tick(app);
            if let Err(e) = w.show().and_then(|()| w.set_focus()) {
                eprintln!("juice: flyout show failed: {e}");
            }
        }
        Err(e) => eprintln!("juice: flyout failed: {e}"),
    }
}

/// Hides the flyout, then frees its webview unless it is shown again within `RELEASE_AFTER`.
fn hide<R: Runtime>(app: &AppHandle<R>, w: &WebviewWindow<R>) {
    let _ = w.hide();
    let (app, w) = (app.clone(), w.clone());
    std::thread::spawn(move || {
        std::thread::sleep(RELEASE_AFTER);
        let shown_since = app.state::<Moments>().0.lock().is_ok_and(|m| within(m.0, RELEASE_AFTER));
        if !shown_since && !w.is_visible().unwrap_or(true) {
            let _ = w.destroy();
        }
    });
}

/// The flyout window; built on open so users who never open it pay no webview memory.
fn flyout<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    if let Some(w) = app.get_webview_window(FLYOUT) {
        return Ok(w);
    }
    let w = WebviewWindowBuilder::new(app, FLYOUT, WebviewUrl::App("index.html".into()))
        .title("Juice")
        .inner_size(WIDTH, 360.0)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .transparent(true)
        .shadow(true)
        .effects(WindowEffectsConfig { effects: vec![Effect::Mica], ..Default::default() })
        .build()?;
    let handle = app.clone();
    let win = w.clone();
    w.on_window_event(move |e| {
        if let WindowEvent::Focused(false) = e {
            // Decide under the lock, act after it: focus and hide calls can raise window events again.
            let reclaim = handle.state::<Moments>().0.lock().is_ok_and(|mut m| {
                let reclaim = within(m.0, SHOW_GUARD);
                if !reclaim {
                    m.1 = Some(Instant::now());
                }
                reclaim
            });
            if reclaim {
                let _ = win.set_focus();
            } else {
                hide(&handle, &win);
            }
        }
    });
    fit_to(app, &w, 360.0)?;
    Ok(w)
}

/// Resizes the flyout to `height` CSS pixels and places it over the tray icon.
pub fn fit_to<R: Runtime>(app: &AppHandle<R>, w: &WebviewWindow<R>, height: f64) -> tauri::Result<()> {
    let scale = w.scale_factor()?;
    let (fw, fh) = ((WIDTH * scale).round() as i32, (height * scale).round() as i32);
    let icon = app.tray_by_id(TRAY).and_then(|t| t.rect().ok().flatten()).map(|r| {
        let p = r.position.to_physical::<i32>(scale);
        let s = r.size.to_physical::<i32>(scale);
        Area { x: p.x, y: p.y, w: s.width, h: s.height }
    });
    let monitor = match icon {
        Some(i) => app.monitor_from_point(f64::from(i.x), f64::from(i.y))?,
        None => None,
    }
    .or(app.primary_monitor()?);
    let Some(m) = monitor else { return w.set_size(PhysicalSize::new(fw, fh)) };
    let wa = m.work_area();
    let work = Area { x: wa.position.x, y: wa.position.y, w: wa.size.width as i32, h: wa.size.height as i32 };
    // No tray rect (icon hidden in overflow): anchor at the work area's bottom-right corner.
    let icon = icon.unwrap_or(Area { x: work.x + work.w, y: work.y + work.h, w: 0, h: 0 });
    let (x, y) = place(icon, fw, fh, work, (GAP * scale).round() as i32);
    w.set_size(PhysicalSize::new(fw, fh))?;
    w.set_position(PhysicalPosition::new(x, y))
}

/// Page reports its content height; resize and re-place the flyout.
#[tauri::command]
pub fn fit<R: Runtime>(app: AppHandle<R>, window: WebviewWindow<R>, height: f64) -> Result<(), String> {
    if !(height.is_finite() && height > 0.0) {
        return Err(format!("bad height {height}"));
    }
    fit_to(&app, &window, height.min(2000.0)).map_err(|e| e.to_string())
}
