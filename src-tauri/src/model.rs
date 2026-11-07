//! Shared data: token counts, one usage entry, and the snapshot the tray and flyout show.
use serde::Serialize;
use std::ops::AddAssign;
use std::sync::Arc;

/// Token counts of one response, or a sum of responses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
}

impl Tokens {
    /// Every token the requests processed, cache traffic included.
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_write + self.cache_read
    }
}

impl AddAssign for Tokens {
    fn add_assign(&mut self, o: Self) {
        self.input += o.input;
        self.output += o.output;
        self.cache_write += o.cache_write;
        self.cache_read += o.cache_read;
    }
}

/// One deduplicated assistant response from the logs.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Milliseconds since the Unix epoch.
    pub ts: i64,
    pub model: Arc<str>,
    pub tok: Tokens,
    /// Estimated USD at API list prices; `None` when the model or mode has no known price.
    pub cost: Option<f64>,
}

/// One model's totals inside a period.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRow {
    pub model: String,
    pub tokens: Tokens,
    /// USD of the priced responses.
    pub cost: f64,
    /// Some responses had no known price, so `cost` is a lower bound.
    pub unpriced: bool,
}

/// Totals for one period (the 5h window or today), models sorted by cost then tokens.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Period {
    pub tokens: u64,
    pub cost: f64,
    pub unpriced: bool,
    pub models: Vec<ModelRow>,
}

/// Everything the tray icon, tooltip and flyout show.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Current 5h window; empty when idle.
    pub window: Period,
    /// When the current window resets (ms since epoch); `None` when no window is active.
    pub window_end: Option<i64>,
    pub today: Period,
    /// Usage lines that could not be read (format changed or corrupt).
    pub unreadable: u64,
    /// Folder Juice reads.
    pub root: String,
    pub root_found: bool,
}
