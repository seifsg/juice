//! What the taskbar badge says: a value and a caption, like the weather button's temperature and sky.
use crate::agg::spent;
use crate::clock::clock;
use crate::model::Snapshot;
use chrono::{Local, TimeZone};

/// The badge's two lines; `active` (a 5h window is running) picks the accent dot over grey.
#[derive(Clone, Debug, PartialEq)]
pub struct Lines {
    pub value: String,
    pub caption: String,
    pub active: bool,
}

/// Running window: its cost and reset time. Idle: today's cost. No logs folder: says so.
pub fn lines(s: &Snapshot) -> Lines {
    let (value, caption, active) = match s.window_end.and_then(|e| Local.timestamp_millis_opt(e).single()) {
        _ if !s.root_found => ("Juice".into(), "no logs found".into(), false),
        Some(end) => (spent(&s.window), format!("5h · resets {}", clock(end)), true),
        None => (spent(&s.today), "today".into(), false),
    };
    Lines { value, caption, active }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Period;

    #[test]
    fn badge_lines_per_state() {
        let end_at = Local.with_ymd_and_hms(2026, 11, 7, 16, 0, 0).unwrap();
        let end = end_at.timestamp_millis();
        let mut s = Snapshot {
            window: Period { cost: 11.05, ..Period::default() },
            window_end: Some(end),
            today: Period { cost: 18.88, unpriced: true, ..Period::default() },
            unreadable: 0,
            root: "C:\\x".into(),
            root_found: true,
        };
        let l = |v: &str, c: &str, a: bool| Lines { value: v.into(), caption: c.into(), active: a };
        assert_eq!(lines(&s), l("$11.05", &format!("5h · resets {}", clock(end_at)), true));
        s.window_end = None;
        assert_eq!(lines(&s), l("$18.88+", "today", false));
        s.root_found = false;
        assert_eq!(lines(&s), l("Juice", "no logs found", false));
    }
}
