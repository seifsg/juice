//! Background reader: file events, hover ticks and one timed wake in; snapshots out. Never polls.
use crate::model::Snapshot;
use crate::store::Store;
use chrono::Local;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

/// What wakes the worker.
pub enum Msg {
    /// Files changed under the watched folder.
    Fs(Vec<PathBuf>),
    /// Watcher error or timed wake (window end, midnight): rescan everything.
    Rescan,
    /// Someone is looking (hover, flyout open): reread every tracked file and recompute.
    Tick,
}

/// One write raises a burst of events; collect it before reading.
const DEBOUNCE: Duration = Duration::from_millis(250);
/// Longest sleep between wakes: a daily rescan bounds any missed event.
const MAX_SLEEP_MS: i64 = 24 * 3_600_000;

/// Starts the worker thread. The returned sender takes `Msg::Tick`.
pub fn spawn(root: PathBuf, publish: impl Fn(Snapshot) + Send + 'static) -> Sender<Msg> {
    let (tx, rx) = mpsc::channel();
    let events = tx.clone();
    std::thread::Builder::new()
        .name("juice-worker".into())
        .spawn(move || run(&root, &rx, &events, &publish))
        .expect("juice: cannot start the worker thread");
    tx
}

/// Watches `root` recursively; while it is missing, watches its nearest existing ancestor to see it appear.
/// Returns the watcher and whether it covers `root` itself.
fn arm(root: &Path, tx: &Sender<Msg>) -> (Option<RecommendedWatcher>, bool) {
    let tx = tx.clone();
    let handler = move |r: notify::Result<Event>| {
        let _ = tx.send(r.map_or(Msg::Rescan, |e| Msg::Fs(e.paths)));
    };
    let found = root.is_dir();
    let (target, mode) = match root.ancestors().find(|a| a.is_dir()) {
        Some(a) if found => (a, RecursiveMode::Recursive),
        Some(a) => (a, RecursiveMode::NonRecursive),
        None => return (None, false),
    };
    match notify::recommended_watcher(handler).and_then(|mut w| w.watch(target, mode).map(|()| w)) {
        Ok(w) => (Some(w), found),
        Err(e) => {
            crate::log::warn(format_args!("cannot watch {}: {e}", target.display()));
            (None, found)
        }
    }
}

/// The worker loop: wait for an event or the next timed wake, read what changed, publish the new snapshot.
fn run(root: &Path, rx: &Receiver<Msg>, tx: &Sender<Msg>, publish: &dyn Fn(Snapshot)) {
    let mut store = Store::new(root.to_path_buf());
    let (mut watcher, mut covers_root) = arm(root, tx);
    store.walk(Local::now());
    let mut wake = emit(&mut store, publish);
    loop {
        let wait = (wake - Local::now().timestamp_millis()).clamp(0, MAX_SLEEP_MS);
        let msg = match rx.recv_timeout(Duration::from_millis(wait as u64)) {
            Ok(m) => m,
            Err(RecvTimeoutError::Timeout) => Msg::Rescan,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if let Msg::Fs(_) = msg {
            std::thread::sleep(DEBOUNCE);
        }
        let (mut paths, mut rescan, mut tick) = (Vec::new(), false, false);
        for m in std::iter::once(msg).chain(rx.try_iter()) {
            match m {
                Msg::Fs(p) => paths.extend(p),
                Msg::Rescan => rescan = true,
                Msg::Tick => tick = true,
            }
        }
        // The root appeared, vanished, or the watcher failed: watch again and rescan.
        if rescan || watcher.is_none() || covers_root != store.root_found() {
            (watcher, covers_root) = arm(root, tx);
            store.walk(Local::now());
        } else {
            store.touch(paths, Local::now());
            if tick {
                store.refresh(Local::now());
            }
        }
        wake = emit(&mut store, publish);
    }
}

/// Publishes the current snapshot; returns when it next changes without new lines.
fn emit(store: &mut Store, publish: &dyn Fn(Snapshot)) -> i64 {
    let (snap, wake) = store.snapshot(Local::now());
    publish(snap);
    wake
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::time::Instant;

    /// Waits up to `secs` for a published snapshot matching `ok`.
    fn wait_for(rx: &Receiver<Snapshot>, secs: u64, ok: impl Fn(&Snapshot) -> bool) -> bool {
        let end = Instant::now() + Duration::from_secs(secs);
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(s) if ok(&s) => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
        false
    }

    #[test]
    fn worker_follows_appends_and_root_recreation() {
        let base = std::env::temp_dir().join(format!("juice-worker-{}", std::process::id()));
        let root = base.join("projects");
        fs::create_dir_all(root.join("p")).unwrap();
        let log = root.join("p").join("s.jsonl");
        fs::write(&log, "").unwrap();
        let (snaps, rx) = mpsc::channel();
        let _tx = spawn(root.clone(), move |s| {
            let _ = snaps.send(s);
        });
        assert!(wait_for(&rx, 5, |s| s.root_found && s.window.tokens == 0));
        let ts = Local::now().to_rfc3339();
        let line = format!(
            r#"{{"timestamp":"{ts}","requestId":"r1","message":{{"id":"m1","model":"claude-opus-5-5","usage":{{"input_tokens":7,"output_tokens":3}}}}}}"#
        ) + "\n";
        fs::OpenOptions::new().append(true).open(&log).unwrap().write_all(line.as_bytes()).unwrap();
        assert!(wait_for(&rx, 2, |s| s.window.tokens == 10), "append not seen within 2 s");
        fs::remove_dir_all(&root).unwrap();
        assert!(wait_for(&rx, 5, |s| !s.root_found), "root removal not seen");
        fs::create_dir_all(root.join("q")).unwrap();
        fs::write(root.join("q").join("t.jsonl"), line.replace("m1", "m2")).unwrap();
        assert!(wait_for(&rx, 5, |s| s.root_found && s.window.tokens == 20), "recreated root not followed");
        fs::remove_dir_all(&base).unwrap();
    }
}
