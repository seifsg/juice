//! API list prices and the estimated cost of one response.
//! Source: Anthropic API pricing as published 2026-10-06 (Claude API docs, model table and per-model notes).
use crate::parse::Usage;

/// Model id, then $ per million tokens: input, output, cache read; fast-mode multiplier where one is published.
/// Cache writes cost 1.25x input (5-minute) or 2x input (1-hour) on every model.
const PRICES: &[(&str, f64, f64, f64, Option<f64>)] = &[
    ("claude-fable-5-1", 10.0, 50.0, 0.25, None),
    ("claude-mythos-5-1", 10.0, 50.0, 0.25, None),
    ("claude-fable-5", 10.0, 50.0, 1.0, None),
    ("claude-mythos-5", 10.0, 50.0, 1.0, None),
    ("claude-opus-5-5", 4.0, 20.0, 0.20, Some(2.0)),
    ("claude-opus-5", 5.0, 25.0, 0.50, Some(2.0)),
    ("claude-opus-4-8", 5.0, 25.0, 0.50, None),
    ("claude-opus-4-7", 5.0, 25.0, 0.50, None),
    ("claude-opus-4-6", 5.0, 25.0, 0.50, None),
    ("claude-sonnet-5-5", 2.0, 10.0, 0.20, None),
    ("claude-sonnet-5", 2.0, 10.0, 0.20, None),
    ("claude-sonnet-4-6", 3.0, 15.0, 0.30, None),
    ("claude-haiku-5-5", 0.10, 0.50, 0.01, None),
    ("claude-haiku-4-5", 1.0, 5.0, 0.10, None),
];

/// Haiku 5.5 bills every rate 5x when the prompt is over 100K tokens.
const LONG_PROMPT: (&str, u64, f64) = ("claude-haiku-5-5", 100_000, 5.0);
/// US-only inference (`inference_geo: "us"`) costs 1.1x.
const US_GEO: f64 = 1.1;

/// Estimated USD of one response, or `None` when its model, speed or region has no published price.
pub fn cost(model: &str, u: &Usage) -> Option<f64> {
    let id = strip_date(model);
    let &(_, input, output, read, fast) = PRICES.iter().find(|p| p.0 == id)?;
    let speed = match u.speed.as_deref() {
        None | Some("standard") => 1.0,
        Some("fast") => fast?,
        Some(_) => return None,
    };
    let geo = match u.inference_geo.as_deref() {
        None | Some("global") => 1.0,
        Some("us") => US_GEO,
        Some(_) => return None,
    };
    let prompt = u.input_tokens + u.cache_creation_input_tokens + u.cache_read_input_tokens;
    let long = if id == LONG_PROMPT.0 && prompt > LONG_PROMPT.1 { LONG_PROMPT.2 } else { 1.0 };
    let write_1h = u.cache_creation.as_ref().map_or(0, |c| c.ephemeral_1h_input_tokens).min(u.cache_creation_input_tokens);
    let write_5m = u.cache_creation_input_tokens - write_1h;
    let per_million = u.input_tokens as f64 * input
        + u.output_tokens as f64 * output
        + write_5m as f64 * input * 1.25
        + write_1h as f64 * input * 2.0
        + u.cache_read_input_tokens as f64 * read;
    Some(per_million / 1e6 * speed * geo * long)
}

/// `claude-sonnet-4-6-20260101` -> `claude-sonnet-4-6`: dated snapshots share their model's price.
fn strip_date(model: &str) -> &str {
    match model.rsplit_once('-') {
        Some((base, date)) if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) => base,
        _ => model,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::CacheCreation;

    fn usage(input: u64, output: u64, write: u64, write_1h: u64, read: u64) -> Usage {
        Usage {
            input_tokens: input,
            output_tokens: output,
            cache_creation_input_tokens: write,
            cache_read_input_tokens: read,
            cache_creation: Some(CacheCreation { ephemeral_1h_input_tokens: write_1h }),
            ..Usage::default()
        }
    }

    fn close(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    #[test]
    fn cost_rules() {
        // 1M of each kind on Opus 5.5: 4 + 20 + (0.5M*5 + 0.5M*8)/1M + 0.20.
        let u = usage(1_000_000, 1_000_000, 1_000_000, 500_000, 1_000_000);
        assert!(close(cost("claude-opus-5-5", &u), 4.0 + 20.0 + 2.5 + 4.0 + 0.2));
        assert!(close(cost("claude-sonnet-4-6-20260101", &usage(1_000_000, 0, 0, 0, 0)), 3.0));
        assert!(close(cost("claude-opus-5-5", &Usage { speed: Some("fast".into()), ..usage(1_000_000, 0, 0, 0, 0) }), 8.0));
        assert!(close(cost("claude-sonnet-5-5", &Usage { inference_geo: Some("us".into()), ..usage(1_000_000, 0, 0, 0, 0) }), 2.2));
        assert!(close(cost("claude-haiku-5-5", &usage(200_000, 0, 0, 0, 0)), 0.1));
        assert!(close(cost("claude-haiku-5-5", &usage(100_000, 0, 0, 0, 0)), 0.01));
        assert_eq!(cost("claude-sonnet-5-5", &Usage { speed: Some("fast".into()), ..Usage::default() }), None);
        assert_eq!(cost("claude-opus-5-5", &Usage { inference_geo: Some("mars".into()), ..Usage::default() }), None);
        assert_eq!(cost("some-new-model", &Usage::default()), None);
    }
}
