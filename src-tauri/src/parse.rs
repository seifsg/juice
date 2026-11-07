//! One JSONL log line into a priced usage entry, or the reason it is not one.
use crate::model::{Entry, Tokens};
use crate::price;
use serde::Deserialize;
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Deserialize)]
struct Line {
    timestamp: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
}

/// The `message.usage` object of an assistant response.
#[derive(Debug, Default, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    pub cache_creation: Option<CacheCreation>,
    pub speed: Option<String>,
    pub inference_geo: Option<String>,
}

/// Split of cache writes by lifetime (1h writes cost more).
#[derive(Debug, Default, Deserialize)]
pub struct CacheCreation {
    #[serde(default)]
    pub ephemeral_1h_input_tokens: u64,
}

/// What a log line holds.
#[derive(Debug)]
pub enum Parsed {
    /// Not a usage line (user turn, tool output, summary, synthetic reply).
    Skip,
    /// Looked like usage but could not be read: counted and shown, never guessed.
    Bad,
    /// A response; `key` dedupes the many lines one response writes.
    Usage { key: Option<u64>, entry: Entry },
}

/// Parses one line. Only lines containing `"usage"` are decoded, so the bulk of the log costs a byte scan.
pub fn parse_line(line: &[u8]) -> Parsed {
    if memchr::memmem::find(line, b"\"usage\"").is_none() {
        return Parsed::Skip;
    }
    let Ok(l) = serde_json::from_slice::<Line>(line) else { return Parsed::Bad };
    let Some(Message { id, model, usage: Some(u) }) = l.message else { return Parsed::Skip };
    let ts = l.timestamp.as_deref().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
    let (Some(model), Some(ts)) = (model, ts) else { return Parsed::Bad };
    if model == "<synthetic>" {
        return Parsed::Skip;
    }
    let key = id.zip(l.request_id).map(|k| {
        let mut h = DefaultHasher::new();
        k.hash(&mut h);
        h.finish()
    });
    let tok = Tokens {
        input: u.input_tokens,
        output: u.output_tokens,
        cache_write: u.cache_creation_input_tokens,
        cache_read: u.cache_read_input_tokens,
    };
    let cost = price::cost(&model, &u);
    Parsed::Usage { key, entry: Entry { ts: ts.timestamp_millis(), model: model.into(), tok, cost } }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = r#"{"type":"assistant","timestamp":"2026-11-07T09:36:30.579Z","requestId":"req_1","message":{"id":"msg_1","model":"claude-opus-5-5","content":[{"type":"text","text":"hi"}],"usage":{"input_tokens":10,"output_tokens":200,"cache_creation_input_tokens":1000,"cache_read_input_tokens":5000,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":1000},"speed":"standard","inference_geo":"global"}}}"#;

    #[test]
    fn parse_usage_line_skip_and_bad() {
        let Parsed::Usage { key, entry } = parse_line(LINE.as_bytes()) else { panic!("usage line not parsed") };
        assert!(key.is_some());
        assert_eq!(entry.ts, 1_794_044_190_579);
        assert_eq!(&*entry.model, "claude-opus-5-5");
        assert_eq!(entry.tok.total(), 6210);
        let Parsed::Usage { key: k2, .. } = parse_line(LINE.replace("\"text\":\"hi\"", "\"text\":\"again\"").as_bytes()) else {
            panic!("second content block not parsed")
        };
        assert_eq!(key, k2, "same message.id + requestId must dedupe");
        assert!(matches!(parse_line(br#"{"type":"user","message":{"role":"user","content":"no usage here"}}"#), Parsed::Skip));
        assert!(matches!(parse_line(LINE.replace("claude-opus-5-5", "<synthetic>").as_bytes()), Parsed::Skip));
        assert!(matches!(parse_line(br#"{"message":{"usage":{"input_tokens":"many"}}}"#), Parsed::Bad));
        assert!(matches!(parse_line(br#"{"message":{"usage":{"#), Parsed::Bad));
    }
}
