//! Reads the logs incrementally: tracks each recent file's read offset and keeps the last 24h of entries.
use crate::agg;
use crate::model::{Entry, Snapshot};
use crate::parse::{parse_line, Parsed};
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Entries older than this are dropped: enough for today plus a 5h window that started yesterday.
const HORIZON: i64 = 24 * 3_600_000;

/// Deduplicated entries and the counters that come with them.
#[derive(Default)]
struct Ledger {
    entries: Vec<Entry>,
    /// Dedupe key -> entry time, so keys expire with their entries.
    keys: HashMap<u64, i64>,
    unreadable: u64,
    sorted: bool,
}

impl Ledger {
    /// Adds one log line if it is a new usage entry inside the horizon.
    fn ingest(&mut self, line: &[u8], oldest: i64) {
        match parse_line(line) {
            Parsed::Skip => {}
            Parsed::Bad => self.unreadable += 1,
            Parsed::Usage { entry, .. } if entry.ts < oldest => {}
            Parsed::Usage { key, entry } => {
                if key.is_some_and(|k| self.keys.insert(k, entry.ts).is_some()) {
                    return;
                }
                self.entries.push(entry);
                self.sorted = false;
            }
        }
    }

    fn prune(&mut self, oldest: i64) {
        self.entries.retain(|e| e.ts >= oldest);
        self.keys.retain(|_, ts| *ts >= oldest);
    }
}

/// Log reader rooted at the Claude Code projects folder.
pub struct Store {
    root: PathBuf,
    /// Tracked `.jsonl` files and how many bytes of complete lines were read from each.
    files: HashMap<PathBuf, u64>,
    ledger: Ledger,
}

impl Store {
    pub fn new(root: PathBuf) -> Self {
        Store { root, files: HashMap::new(), ledger: Ledger::default() }
    }

    /// Full rescan: tracks every `.jsonl` written within the horizon, forgets older ones, reads new bytes.
    pub fn walk(&mut self, now: DateTime<Local>) {
        let oldest = now.timestamp_millis() - HORIZON;
        let since = SystemTime::UNIX_EPOCH + Duration::from_millis(oldest.max(0) as u64);
        let mut fresh = Vec::new();
        collect(&self.root, since, &mut fresh);
        let mut files = HashMap::with_capacity(fresh.len());
        for p in fresh {
            let offset = self.files.get(&p).copied().unwrap_or(0);
            files.insert(p, offset);
        }
        self.files = files;
        self.ledger.prune(oldest);
        self.read(oldest);
    }

    /// After file events: reads new bytes of the changed `.jsonl` files only, tracking new ones.
    pub fn touch(&mut self, paths: Vec<PathBuf>, now: DateTime<Local>) {
        let oldest = now.timestamp_millis() - HORIZON;
        for p in paths.into_iter().filter(|p| is_log(p)) {
            let mut offset = self.files.get(&p).copied().unwrap_or(0);
            let ledger = &mut self.ledger;
            if keep(tail(&p, &mut offset, |line| ledger.ingest(line, oldest)), &p) {
                self.files.insert(p, offset);
            } else {
                self.files.remove(&p);
            }
        }
    }

    /// Reads new bytes of every tracked file (a stat each): someone is looking, so repair events the OS dropped.
    pub fn refresh(&mut self, now: DateTime<Local>) {
        self.read(now.timestamp_millis() - HORIZON);
    }

    pub fn root_found(&self) -> bool {
        self.root.is_dir()
    }

    /// Current snapshot and the next time it changes on its own (ms).
    pub fn snapshot(&mut self, now: DateTime<Local>) -> (Snapshot, i64) {
        let l = &mut self.ledger;
        if !l.sorted {
            l.entries.sort_unstable_by_key(|e| e.ts);
            l.sorted = true;
        }
        let t = agg::totals(&l.entries, now);
        let snap = Snapshot {
            window: t.window,
            window_end: t.window_end,
            today: t.today,
            unreadable: l.unreadable,
            root: self.root.display().to_string(),
            root_found: self.root.is_dir(),
        };
        (snap, t.next_wake)
    }

    /// Reads new complete lines of every tracked file; drops files that no longer exist.
    fn read(&mut self, oldest: i64) {
        let ledger = &mut self.ledger;
        self.files.retain(|path, offset| keep(tail(path, offset, |line| ledger.ingest(line, oldest)), path));
    }
}

/// Whether a file stays tracked after a read: a deleted file is dropped; other errors are logged and retried later.
fn keep(read: io::Result<()>, path: &Path) -> bool {
    match read {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => {
            eprintln!("juice: cannot read {}: {e}", path.display());
            true
        }
    }
}

fn is_log(p: &Path) -> bool {
    p.extension().is_some_and(|e| e == "jsonl")
}

/// Recursively lists `.jsonl` files modified at or after `since`. Unreadable folders are skipped.
fn collect(dir: &Path, since: SystemTime, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(kind) = e.file_type() else { continue };
        let path = e.path();
        if kind.is_dir() {
            collect(&path, since, out);
        } else if kind.is_file() && is_log(&path) && e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t >= since) {
            out.push(path);
        }
    }
}

/// Feeds each complete line after `offset` to `f` and advances `offset` past it.
/// A partial last line (still being written) stays unread; a file shorter than `offset` was replaced and is reread.
fn tail(path: &Path, offset: &mut u64, mut f: impl FnMut(&[u8])) -> io::Result<()> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if len < *offset {
        *offset = 0;
    }
    if len == *offset {
        return Ok(());
    }
    file.seek(SeekFrom::Start(*offset))?;
    let mut reader = BufReader::with_capacity(1 << 16, file.take(len - *offset));
    let mut line = Vec::new();
    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 || line.last() != Some(&b'\n') {
            return Ok(());
        }
        *offset += n as u64;
        f(&line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn line(id: &str, ts: &str) -> String {
        format!(
            r#"{{"timestamp":"{ts}","requestId":"r-{id}","message":{{"id":"{id}","model":"claude-sonnet-5-5","usage":{{"input_tokens":100,"output_tokens":10}}}}}}"#
        ) + "\n"
    }

    #[test]
    fn store_tails_dedupes_and_rereads() {
        let root = std::env::temp_dir().join(format!("juice-store-{}", std::process::id()));
        let sub = root.join("proj").join("session").join("subagents");
        fs::create_dir_all(&sub).unwrap();
        let now = Local::now();
        let ts = (now - chrono::Duration::minutes(5)).to_rfc3339();
        let a = root.join("proj").join("a.jsonl");
        // Complete line, duplicate of it, then a half-written line.
        fs::write(&a, line("m1", &ts) + &line("m1", &ts) + &line("m2", &ts)[..20]).unwrap();
        fs::write(sub.join("agent-1.jsonl"), line("m1", &ts)).unwrap();
        let mut s = Store::new(root.clone());
        s.walk(now);
        assert_eq!(s.snapshot(now).0.window.tokens, 110, "m1 counted once across files");
        // The writer finishes the partial line.
        let mut f = fs::OpenOptions::new().append(true).open(&a).unwrap();
        f.write_all(line("m2", &ts)[20..].as_bytes()).unwrap();
        s.touch(vec![a.clone()], now);
        assert_eq!(s.snapshot(now).0.window.tokens, 220);
        // Replaced by a shorter file: reread from the start, old keys still dedupe.
        fs::write(&a, line("m3", &ts)).unwrap();
        s.refresh(now);
        let (snap, _) = s.snapshot(now);
        assert_eq!((snap.window.tokens, snap.unreadable), (330, 0));
        fs::remove_dir_all(&root).unwrap();
    }

    /// Yesterday's totals from the real logs, for comparison with an independent scan:
    /// `cargo test closed_day -- --ignored --nocapture`. A walk at yesterday's last instant reads exactly that day.
    #[test]
    #[ignore = "reads the real ~/.claude/projects"]
    fn closed_day_totals() {
        use chrono::TimeZone;
        let root = std::env::home_dir().expect("home dir").join(".claude").join("projects");
        let today = Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        let end = Local.from_local_datetime(&today).earliest().unwrap() - chrono::Duration::milliseconds(1);
        let mut s = Store::new(root);
        s.walk(end);
        let (snap, _) = s.snapshot(end);
        for m in &snap.today.models {
            let t = m.tokens;
            println!("{} {} {} {} {} {:.6}", m.model, t.input, t.output, t.cache_write, t.cache_read, m.cost);
        }
        println!("unreadable {}", snap.unreadable);
    }
}
