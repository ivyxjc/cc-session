//! Session activity timeline helpers: scan a session JSONL for message
//! timestamps and group them into contiguous time blocks.
//!
//! Lives outside `commands/` and `llm/` because both need it — the Day Planner
//! IPC command and the daily map-reduce.

use chrono::DateTime;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Sessions are split into a new block whenever the gap between consecutive
/// messages exceeds this many minutes — keeps "morning + afternoon" visible
/// instead of one bar from 09:00 to 18:00.
pub const GAP_SPLIT_MINUTES: i64 = 30;

/// Walk a JSONL file, collect timestamps (ms) of user/assistant messages within
/// [start_ms, end_ms]. Sorted ascending.
pub fn collect_timestamps_on_day(jsonl_path: &str, start_ms: i64, end_ms: i64) -> Vec<i64> {
    let file = match File::open(Path::new(jsonl_path)) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = BufReader::new(file);

    let mut out: Vec<i64> = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if msg_type != "user" && msg_type != "assistant" {
            continue;
        }
        let ts_str = match val.get("timestamp").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let ts = match DateTime::parse_from_rfc3339(ts_str) {
            Ok(dt) => dt.timestamp_millis(),
            Err(_) => continue,
        };
        if ts >= start_ms && ts <= end_ms {
            out.push(ts);
        }
    }

    out.sort_unstable();
    out
}

/// Group sorted timestamps into contiguous blocks separated by gaps > `gap_ms`.
/// Single-message blocks get extended to 1 minute so the visual has width.
pub fn split_into_blocks(timestamps: &[i64], gap_ms: i64) -> Vec<(i64, i64)> {
    if timestamps.is_empty() {
        return Vec::new();
    }
    let mut result: Vec<(i64, i64)> = Vec::new();
    let mut block_start = timestamps[0];
    let mut block_end = timestamps[0];
    for &ts in &timestamps[1..] {
        if ts - block_end > gap_ms {
            result.push((block_start, block_end));
            block_start = ts;
        }
        block_end = ts;
    }
    result.push((block_start, block_end));

    result
        .into_iter()
        .map(|(s, e)| if e - s < 60_000 { (s, s + 60_000) } else { (s, e) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_single_timestamp_gets_minute_padding() {
        let ts = vec![1_700_000_000_000];
        let result = split_into_blocks(&ts, 30 * 60_000);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1 - result[0].0, 60_000);
    }

    #[test]
    fn split_contiguous_no_gap_one_block() {
        // 5 messages 5 minutes apart
        let base = 1_700_000_000_000;
        let ts: Vec<i64> = (0..5).map(|i| base + i * 5 * 60_000).collect();
        let result = split_into_blocks(&ts, 30 * 60_000);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, ts[0]);
        assert_eq!(result[0].1, ts[4]);
    }

    #[test]
    fn split_large_gap_two_blocks() {
        let base = 1_700_000_000_000;
        // 3 messages, then 2-hour gap, then 2 messages
        let ts = vec![
            base,
            base + 5 * 60_000,
            base + 10 * 60_000,
            base + 2 * 60 * 60_000,
            base + 2 * 60 * 60_000 + 60_000,
        ];
        let result = split_into_blocks(&ts, 30 * 60_000);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, ts[0]);
        assert_eq!(result[0].1, ts[2]);
        assert_eq!(result[1].0, ts[3]);
        assert_eq!(result[1].1, ts[4]);
    }
}
