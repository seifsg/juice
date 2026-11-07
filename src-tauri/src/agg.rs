//! Totals for the current 5-hour window and today, plus the tooltip text.
use crate::clock::clock;
use crate::model::{Entry, ModelRow, Period, Snapshot, Tokens};
use chrono::{DateTime, Local, NaiveDate, TimeZone};
use std::collections::HashMap;

const HOUR: i64 = 3_600_000;
/// Usage windows last 5 hours.
pub const BLOCK: i64 = 5 * HOUR;
/// Windows chosen by `totals`: the 5h window and today's local calendar day.
pub struct Totals {
    pub window: Period,
    pub window_end: Option<i64>,
    pub today: Period,
    /// Next moment the totals change without new log lines: window end or local midnight (ms).
    pub next_wake: i64,
}

/// Start of the block holding the last entry. A block starts at its first entry floored to the hour
/// and lasts 5h; the first entry at or after its end opens the next block. `entries` sorted by `ts`.
fn block_start(entries: &[Entry]) -> Option<i64> {
    entries.iter().fold(None, |start, e| match start {
        Some(s) if e.ts < s + BLOCK => Some(s),
        _ => Some(e.ts - e.ts.rem_euclid(HOUR)),
    })
}

/// Sums entries per model; models with the biggest cost (then tokens) first.
fn period(entries: &[Entry]) -> Period {
    let mut by_model: HashMap<&str, ModelRow> = HashMap::new();
    for e in entries {
        let row = by_model.entry(&e.model).or_insert_with(|| ModelRow {
            model: e.model.to_string(),
            tokens: Tokens::default(),
            cost: 0.0,
            unpriced: false,
        });
        row.tokens += e.tok;
        match e.cost {
            Some(c) => row.cost += c,
            None => row.unpriced = true,
        }
    }
    let mut models: Vec<ModelRow> = by_model.into_values().collect();
    models.sort_by(|a, b| b.cost.total_cmp(&a.cost).then(b.tokens.total().cmp(&a.tokens.total())));
    Period {
        tokens: models.iter().map(|m| m.tokens.total()).sum(),
        // fold from +0.0: an empty f64 `sum()` is -0.0 and would print "$-0.00".
        cost: models.iter().fold(0.0, |sum, m| sum + m.cost),
        unpriced: models.iter().any(|m| m.unpriced),
        models,
    }
}

/// First instant of a local calendar day (skips a DST gap at midnight).
fn day_start(d: NaiveDate) -> i64 {
    (0..3)
        .find_map(|h| Local.from_local_datetime(&d.and_hms_opt(h, 0, 0)?).earliest())
        .map(|t| t.timestamp_millis())
        .expect("every local day has a start within its first 3 hours")
}

/// Totals at `now` from entries sorted by `ts`; entries after `now` are not counted yet.
pub fn totals(entries: &[Entry], now: DateTime<Local>) -> Totals {
    let now_ms = now.timestamp_millis();
    let seen = &entries[..entries.partition_point(|e| e.ts <= now_ms)];
    let window_end = block_start(seen).map(|s| s + BLOCK).filter(|&end| end > now_ms);
    let window = window_end.map_or_else(Period::default, |end| period(&seen[seen.partition_point(|e| e.ts < end - BLOCK)..]));
    let midnight = day_start(now.date_naive());
    let next_midnight = now.date_naive().succ_opt().map_or(i64::MAX, day_start);
    Totals {
        window,
        window_end,
        today: period(&seen[seen.partition_point(|e| e.ts < midnight)..]),
        next_wake: window_end.map_or(next_midnight, |end| end.min(next_midnight)),
    }
}

/// "$3.42", "$142": cents only while they matter (cut where rounding would print "$100.00").
pub fn money(usd: f64) -> String {
    if usd < 99.995 { format!("${usd:.2}") } else { format!("${usd:.0}") }
}

/// A period's cost; "+" marks a lower bound (some usage had no price).
pub fn spent(p: &Period) -> String {
    format!("{}{}", money(p.cost), if p.unpriced { "+" } else { "" })
}

/// "950", "1.2k", "340k", "1.2M", "34M", "1.2B". Units switch where rounding would print "1000k" or "10.0k".
pub fn tokens(n: u64) -> String {
    let f = n as f64;
    let (v, unit) = match n {
        0..=999 => return n.to_string(),
        1_000..=999_499 => (f / 1e3, "k"),
        999_500..=999_499_999 => (f / 1e6, "M"),
        _ => (f / 1e9, "B"),
    };
    if v < 9.95 { format!("{v:.1}{unit}") } else { format!("{v:.0}{unit}") }
}

/// Tray tooltip, at most 127 characters (the shell's limit). No app name: the shell already shows it.
pub fn tooltip(s: &Snapshot) -> String {
    let text = if !s.root_found {
        format!("No logs at {}", s.root)
    } else {
        let window = match s.window_end.and_then(|e| Local.timestamp_millis_opt(e).single()) {
            Some(end) => format!(
                "5h {} · {} tok · resets {}",
                spent(&s.window),
                tokens(s.window.tokens),
                clock(end)
            ),
            None => "No active 5h window".to_string(),
        };
        format!("{window}\nToday {} · {} tok", spent(&s.today), tokens(s.today.tokens))
    };
    text.chars().take(127).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn entry(ts: i64, model: &str, cost: Option<f64>) -> Entry {
        Entry { ts, model: model.into(), tok: Tokens { input: 1, output: 9, ..Tokens::default() }, cost }
    }

    #[test]
    fn window_and_today_totals() {
        let now = Local.with_ymd_and_hms(2026, 11, 7, 14, 30, 0).unwrap();
        let at = |h: i64, m: i64| (now - Duration::hours(h) - Duration::minutes(m)).timestamp_millis();
        // 08:10 opens 08:00-13:00; 13:20 opens 13:00-18:00 (current); yesterday 23:50 is not today.
        let entries = vec![
            entry(at(14, 40), "claude-opus-5-5", Some(1.0)),
            entry(at(6, 20), "claude-opus-5-5", Some(2.0)),
            entry(at(1, 10), "claude-sonnet-5-5", Some(0.5)),
            entry(at(0, 5), "claude-opus-5-5", Some(3.0)),
            entry(at(0, 1), "mystery-model", None),
            entry(at(-1, 0), "claude-opus-5-5", Some(100.0)),
        ];
        let t = totals(&entries, now);
        let opened = at(1, 10);
        assert_eq!(t.window_end, Some(opened - opened.rem_euclid(HOUR) + BLOCK));
        assert_eq!(t.window.cost, 3.5);
        assert!(t.window.unpriced);
        assert_eq!(t.window.models[0].model, "claude-opus-5-5");
        assert_eq!(t.window.tokens, 30);
        assert_eq!(t.today.cost, 5.5);
        assert_eq!(t.next_wake, t.window_end.unwrap());
        let idle = totals(&entries[..2], now);
        assert_eq!((idle.window_end, idle.window.tokens), (None, 0));
        assert_eq!(money(period(&[]).cost), "$0.00");
        assert_eq!(idle.next_wake, Local.with_ymd_and_hms(2026, 11, 8, 0, 0, 0).unwrap().timestamp_millis());
    }

    #[test]
    fn tooltip_fits_shell_limit() {
        let snap = |root: String, found: bool| Snapshot {
            window: Period { tokens: 1_234_567, cost: 3.42, unpriced: true, models: vec![] },
            window_end: Some(0),
            today: Period { tokens: 999_999_999_999, cost: 123_456.0, ..Period::default() },
            unreadable: 0,
            root,
            root_found: found,
        };
        let t = tooltip(&snap("C:\\x".into(), true));
        assert!(t.starts_with("5h $3.42+ · 1.2M tok · resets "), "{t}");
        assert!(t.ends_with("\nToday $123456 · 1000B tok"), "{t}");
        assert_eq!(tooltip(&snap("p".repeat(300), false)).chars().count(), 127);
        let t: Vec<_> = [950, 9_949, 9_960, 999_499, 999_500].map(tokens).into();
        assert_eq!(t, ["950", "9.9k", "10k", "999k", "1.0M"]);
        assert_eq!((money(99.994), money(99.996)), ("$99.99".into(), "$100".into()));
    }
}
