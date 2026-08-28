//! Build the LLM input slice for a session JSONL.
//!
//! Strategy:
//! - Anchor: first real user message (cleaned, capped at 1000 chars).
//! - Tail: walk messages in reverse, taking text/thinking blocks (skipping tool_use
//!   and tool_result), per-message capped, until ~28K chars filled.
//! - Tool stats: count tool names across the whole session.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

// Budgets are in chars. Target ≈ 200K tokens: mixed code/English (~4 chars/token)
// and Chinese (~1 char/token) blends to roughly 3 chars/token → 600K chars.
const ANCHOR_MAX: usize = 4_000;
const PER_USER_MAX: usize = 8_000;
const PER_ASSISTANT_TEXT_MAX: usize = 12_000;
const PER_ASSISTANT_THINKING_MAX: usize = 4_000;
const TAIL_BUDGET: usize = 600_000;

pub struct LlmInput {
    pub anchor: Option<String>,
    /// `(role_label, body)` ordered chronologically (oldest first).
    pub tail_turns: Vec<(String, String)>,
    pub omitted_count: usize,
    /// e.g. "Read×12, Edit×8, Bash×3"
    pub tool_stats: String,
    /// PR URLs from `pr-link` events in the window, e.g.
    /// "flexcompute/flex#14521 — https://github.com/flexcompute/flex/pull/14521"
    pub pr_links: Vec<String>,
}

impl LlmInput {
    /// Render as the final prompt text fed to the LLM.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(8192);
        out.push_str("INITIAL REQUEST:\n");
        out.push_str(self.anchor.as_deref().unwrap_or("(none)"));
        out.push_str("\n\n");
        if self.omitted_count > 0 {
            out.push_str(&format!("[... {} earlier turns omitted ...]\n\n", self.omitted_count));
        }
        out.push_str("RECENT CONVERSATION:\n");
        for (role, body) in &self.tail_turns {
            out.push_str(&format!("[{}]\n{}\n\n", role, body));
        }
        out.push_str("TOOLS USED IN SESSION:\n");
        if self.tool_stats.is_empty() {
            out.push_str("(none)");
        } else {
            out.push_str(&self.tool_stats);
        }
        if !self.pr_links.is_empty() {
            out.push_str("\n\nPR LINKS SEEN IN SESSION:\n");
            for link in &self.pr_links {
                out.push_str(link);
                out.push('\n');
            }
        }
        out
    }
}

/// Read a JSONL session and produce the LLM input slice for a specific time
/// window. If `window` is None, the entire session is considered.
///
/// Window semantics: anchor is the first real user message **within the window**
/// (not the session's very first message); tail walks backward through window
/// messages; tool stats count only tools used in the window.
pub fn build_for_window(
    jsonl_path: &Path,
    window: Option<(i64, i64)>,
) -> Result<LlmInput, String> {
    let file = File::open(jsonl_path).map_err(|e| format!("open: {}", e))?;
    let reader = BufReader::new(file);

    let mut messages: Vec<Value> = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|e| format!("read: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            // Filter by window if provided.
            if let Some((start_ms, end_ms)) = window {
                let ts_ms = v
                    .get("timestamp")
                    .and_then(|t| t.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.timestamp_millis());
                match ts_ms {
                    Some(ts) if ts >= start_ms && ts <= end_ms => {}
                    _ => continue,
                }
            }
            messages.push(v);
        }
    }

    // 1. Anchor: first real user message (using parser's cleanup heuristic to mirror the heuristic summary)
    let mut anchor: Option<String> = None;
    for msg in &messages {
        if msg.get("type").and_then(|v| v.as_str()) != Some("user") {
            continue;
        }
        let content = match msg.get("message").and_then(|m| m.get("content")) {
            Some(c) => c,
            None => continue,
        };
        if !is_real_user(content) {
            continue;
        }
        if anchor.is_none() {
            if let Some(t) = first_user_text(content) {
                let cleaned = crate::parser::clean_summary_text(&t);
                // Local-command echoes clean to "" — keep scanning so the next
                // genuine user message becomes the anchor.
                if !cleaned.is_empty() {
                    anchor = Some(truncate_chars(&cleaned, ANCHOR_MAX));
                    break;
                }
            }
        }
    }

    // 2. Tail: reverse walk
    let mut tail_rev: Vec<(String, String)> = Vec::new();
    let mut budget: i64 = TAIL_BUDGET as i64;
    let mut tail_start_index = messages.len(); // index of first message included in tail (lowest)
    for (idx, msg) in messages.iter().enumerate().rev() {
        if budget <= 0 {
            break;
        }
        let msg_type = match msg.get("type").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => continue,
        };
        // Label turns with local HH:MM so the model can weight work by how the
        // day actually progressed instead of recency.
        let time_label = msg
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Local).format(" %H:%M").to_string())
            .unwrap_or_default();
        match msg_type {
            "user" => {
                let content = match msg.get("message").and_then(|m| m.get("content")) {
                    Some(c) => c,
                    None => continue,
                };
                if !is_real_user(content) {
                    continue;
                }
                let raw = match first_user_text(content) {
                    Some(t) => t,
                    None => continue,
                };
                let cleaned = crate::parser::clean_summary_text(&raw);
                if cleaned.is_empty() {
                    continue;
                }
                let body = truncate_chars(&cleaned, PER_USER_MAX);
                let cost = body.chars().count() as i64 + 16;
                if cost > budget {
                    let allow = (budget - 16).max(0) as usize;
                    if allow > 0 {
                        tail_rev.push((format!("user{}", time_label), truncate_chars(&body, allow)));
                        tail_start_index = idx;
                    }
                    break;
                }
                tail_rev.push((format!("user{}", time_label), body));
                tail_start_index = idx;
                budget -= cost;
            }
            "assistant" => {
                let content_arr = msg.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array());
                let arr = match content_arr {
                    Some(a) => a,
                    None => continue,
                };
                let mut text_parts: Vec<String> = Vec::new();
                let mut thinking_parts: Vec<String> = Vec::new();
                for block in arr {
                    let bt = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if bt == "text" {
                        if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                            text_parts.push(t.to_string());
                        }
                    } else if bt == "thinking" {
                        if let Some(t) = block.get("thinking").and_then(|v| v.as_str()) {
                            thinking_parts.push(t.to_string());
                        }
                    }
                    // skip tool_use and tool_result
                }
                let text_combined = text_parts.join("\n");
                let thinking_combined = thinking_parts.join("\n");
                let text_capped = truncate_chars(&text_combined, PER_ASSISTANT_TEXT_MAX);
                let thinking_capped = truncate_chars(&thinking_combined, PER_ASSISTANT_THINKING_MAX);

                if text_capped.is_empty() && thinking_capped.is_empty() {
                    continue; // pure tool_use turn — skip
                }
                let mut body = String::new();
                if !thinking_capped.is_empty() {
                    body.push_str("[thinking] ");
                    body.push_str(&thinking_capped);
                    body.push('\n');
                }
                if !text_capped.is_empty() {
                    body.push_str(&text_capped);
                }
                let body = body.trim().to_string();
                let cost = body.chars().count() as i64 + 16;
                if cost > budget {
                    let allow = (budget - 16).max(0) as usize;
                    if allow > 0 {
                        tail_rev.push((format!("assistant{}", time_label), truncate_chars(&body, allow)));
                        tail_start_index = idx;
                    }
                    break;
                }
                tail_rev.push((format!("assistant{}", time_label), body));
                tail_start_index = idx;
                budget -= cost;
            }
            _ => continue,
        }
    }
    tail_rev.reverse();

    // 3. Tool stats: count tool_use blocks across whole session
    let mut tool_counts: BTreeMap<String, u32> = BTreeMap::new();
    for msg in &messages {
        if msg.get("type").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }
        let arr = match msg.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
            Some(a) => a,
            None => continue,
        };
        for block in arr {
            if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                *tool_counts.entry(name.to_string()).or_insert(0) += 1;
            }
        }
    }
    let mut tool_pairs: Vec<(String, u32)> = tool_counts.into_iter().collect();
    tool_pairs.sort_by(|a, b| b.1.cmp(&a.1)); // descending by count
    let tool_stats = tool_pairs
        .iter()
        .take(15) // cap at top 15 tools
        .map(|(name, count)| format!("{}×{}", name, count))
        .collect::<Vec<_>>()
        .join(", ");

    // 4. Compute omitted count: messages before tail_start_index that were "real" turns
    let mut omitted = 0usize;
    if tail_start_index > 0 {
        for msg in &messages[..tail_start_index] {
            let mt = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if mt == "user" {
                let content = msg.get("message").and_then(|m| m.get("content"));
                if let Some(c) = content {
                    if is_real_user(c) {
                        omitted += 1;
                    }
                }
            } else if mt == "assistant" {
                omitted += 1;
            }
        }
    }

    // 5. PR links: dedupe pr-link events in the window (they carry full URLs)
    let mut pr_links: Vec<String> = Vec::new();
    let mut seen_urls: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for msg in &messages {
        if msg.get("type").and_then(|v| v.as_str()) != Some("pr-link") {
            continue;
        }
        let url = msg.get("prUrl").and_then(|v| v.as_str()).unwrap_or("");
        if url.is_empty() || !seen_urls.insert(url.to_string()) {
            continue;
        }
        let repo = msg.get("prRepository").and_then(|v| v.as_str()).unwrap_or("");
        let label = match (repo.is_empty(), msg.get("prNumber").and_then(|v| v.as_i64())) {
            (false, Some(n)) => format!("{}#{}", repo, n),
            _ => url.to_string(),
        };
        pr_links.push(format!("{} — {}", label, url));
    }

    Ok(LlmInput {
        anchor,
        tail_turns: tail_rev,
        omitted_count: omitted,
        tool_stats,
        pr_links,
    })
}

fn is_real_user(content: &Value) -> bool {
    if content.is_string() {
        return true;
    }
    if let Some(arr) = content.as_array() {
        return arr
            .iter()
            .any(|b| b.get("type").and_then(|t| t.as_str()) != Some("tool_result"));
    }
    false
}

fn first_user_text(content: &Value) -> Option<String> {
    if let Some(s) = content.as_str() {
        return Some(s.to_string());
    }
    let arr = content.as_array()?;
    for block in arr {
        if block.get("type").and_then(|v| v.as_str()) == Some("text") {
            if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        return s.to_string();
    }
    chars[..max_chars].iter().collect()
}
