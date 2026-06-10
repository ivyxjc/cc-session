//! Map-reduce daily activity reports.
//!
//! Step 1 (map): for each session active on a day, build an LLM input from just
//! that day's slice (anchor + tail constrained to the [start, end] window) and
//! ask the model "what was done in this slice". Cached in `daily_session_summaries`.
//!
//! Step 2 (reduce): collect all per-session daily summaries, feed them back to the
//! model, ask for a unified narrative. Cached in `daily_summaries`.

use crate::db::Database;
use crate::llm::client::{parse_json_payload, LlmClient};
use crate::llm::input_builder;
use crate::llm::summary::{load_config, GenError, SummaryAndTags};
use rusqlite::params;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;

const SCHEMA_VERSION: &str = "v1";

const PER_SESSION_SYSTEM_PROMPT: &str = "You are summarizing a single day's slice of a Claude Code session. The session may span multiple days; you only see the messages that fell within the target date. Describe what was *done that day specifically* — not the session's whole arc.

Return ONLY JSON, no markdown fences, no commentary:
{\"summary\": \"<= 80 chars\", \"tags\": [\"...\", \"...\"]}

Rules:
- summary: use the SAME language as the input. Specific action verbs + object (\"Tweaked PR11152 description & cleaned Cursor Bugbot notes\", not \"Worked on PR\").
- tags: 2-4 short lowercase English topical labels, hyphen-joined (e.g. \"pull-request\", \"refactor\", \"db-schema\").
- Even if the slice is brief, produce a best-guess.";

const NARRATIVE_SYSTEM_PROMPT: &str = "Generate a concise daily activity report from per-session summaries of one day.

Input you receive:
- The date
- For each session: project name, time range, summary, tags, top tools used
- Total active time

Output Markdown, ≤ 400 chars total:
- 2-3 sentence overview of the day at the top
- Grouped bullets by theme (MERGE related work across sessions, do NOT just list each session)
- Brief time-distribution note at the end (which project ate the most time)

Use the same language as the input summaries. Be specific. Avoid generic filler like \"worked on several things\".";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailySessionSummary {
    pub session_db_id: i64,
    pub session_id: String,
    pub project_name: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailySessionError {
    pub session_db_id: i64,
    pub session_id: String,
    pub project_name: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReport {
    pub date: String,
    pub narrative: String,
    pub per_session: Vec<DailySessionSummary>,
    /// Sessions that failed during the map step. Frontend surfaces these instead
    /// of silently producing an empty report when everything fails.
    pub errors: Vec<DailySessionError>,
}

/// Generate (or fetch from cache) the per-session-per-day summary.
/// Returns `(summary, tags)`.
pub async fn generate_for_session_day(
    db: Arc<Database>,
    session_db_id: i64,
    date: &str,
    start_ms: i64,
    end_ms: i64,
    force: bool,
) -> Result<(String, Vec<String>), GenError> {
    // Look up session metadata.
    let (jsonl_path, prev_hash, prev_summary, prev_tags_json): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = {
        let conn = db.conn();
        conn.query_row(
            "SELECT s.jsonl_path, d.input_hash, d.summary, d.tags
             FROM sessions s
             LEFT JOIN daily_session_summaries d
               ON d.session_db_id = s.id AND d.date = ?2
             WHERE s.id = ?1",
            params![session_db_id, date],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => GenError::SessionNotFound,
            other => GenError::Db(other.to_string()),
        })?
    };

    // Build LLM input for this window.
    let path = PathBuf::from(&jsonl_path);
    let input = input_builder::build_for_window(&path, Some((start_ms, end_ms)))
        .map_err(GenError::Build)?;
    let rendered = input.render();
    let new_hash = format!("{}:{:x}", SCHEMA_VERSION, Sha256::digest(rendered.as_bytes()));

    // Cache hit?
    if !force {
        if let (Some(prev), Some(summary)) = (prev_hash.as_deref(), prev_summary.as_deref()) {
            if prev == new_hash {
                let tags: Vec<String> = prev_tags_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                return Ok((summary.to_string(), tags));
            }
        }
    }

    // Call LLM.
    let cfg = load_config(&db).map_err(GenError::Db)?.ok_or(GenError::NotConfigured)?;
    let client = LlmClient::new(cfg);
    let raw = client
        .complete(PER_SESSION_SYSTEM_PROMPT, &rendered, 400)
        .await
        .map_err(GenError::Llm)?;
    let parsed: SummaryAndTags = parse_json_payload(&raw).map_err(GenError::Llm)?;
    let summary = clamp_text(&parsed.summary, 80);
    let tags = normalize_tags(parsed.tags);
    let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());

    let now_ms = chrono::Utc::now().timestamp_millis();
    {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO daily_session_summaries
              (session_db_id, date, summary, tags, input_hash, generated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(session_db_id, date) DO UPDATE SET
               summary = excluded.summary,
               tags = excluded.tags,
               input_hash = excluded.input_hash,
               generated_at = excluded.generated_at",
            params![session_db_id, date, summary, tags_json, new_hash, now_ms],
        )
        .map_err(|e| GenError::Db(e.to_string()))?;
    }

    Ok((summary, tags))
}

/// Look up sessions that were active in [start_ms, end_ms].
fn list_active_sessions(
    db: &Database,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<(i64, String, String, String, Option<String>, Option<String>)>, String> {
    let conn = db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.session_id, s.jsonl_path, p.display_name, s.summary, s.slug
             FROM sessions s
             JOIN projects p ON s.project_id = p.id
             WHERE COALESCE(s.last_active, s.started_at, 0) >= ?1
               AND COALESCE(s.started_at, s.last_active, 0) <= ?2",
        )
        .map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt
        .query_map(
            params![start_ms, end_ms],
            |row| -> rusqlite::Result<(i64, String, String, String, Option<String>, Option<String>)> {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
            },
        )
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(rows.flatten().collect())
}

/// Determine which sessions actually had messages within the window (cheap path
/// is impossible — we have to peek at the JSONL).
fn session_has_messages_in_window(jsonl_path: &str, start_ms: i64, end_ms: i64) -> bool {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    let file = match File::open(jsonl_path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let mt = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if mt != "user" && mt != "assistant" {
            continue;
        }
        if let Some(ts_str) = val.get("timestamp").and_then(|v| v.as_str()) {
            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                let ms = ts.timestamp_millis();
                if ms >= start_ms && ms <= end_ms {
                    return true;
                }
            }
        }
    }
    false
}

/// Orchestrate the full daily-report pipeline.
pub async fn generate_daily(
    db: Arc<Database>,
    date: String,
    start_ms: i64,
    end_ms: i64,
    force: bool,
) -> Result<DailyReport, GenError> {
    let _cfg = load_config(&db).map_err(GenError::Db)?.ok_or(GenError::NotConfigured)?;

    let candidates = list_active_sessions(&db, start_ms, end_ms).map_err(GenError::Db)?;

    // Step 1: per-session daily summaries (sequential — concurrency add later if needed).
    let mut per_session: Vec<DailySessionSummary> = Vec::new();
    let mut errors: Vec<DailySessionError> = Vec::new();
    for (sid, session_id, jsonl_path, project_name, _, _) in candidates {
        if !session_has_messages_in_window(&jsonl_path, start_ms, end_ms) {
            continue;
        }
        let (summary, tags) = match generate_for_session_day(
            db.clone(),
            sid,
            &date,
            start_ms,
            end_ms,
            force,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                let msg = e.to_string();
                eprintln!("[daily] session {} ({}) failed: {}", sid, project_name, msg);
                errors.push(DailySessionError {
                    session_db_id: sid,
                    session_id,
                    project_name,
                    error: msg,
                });
                continue;
            }
        };
        // Determine the actual window covered (min/max message timestamps).
        let (s_ms, e_ms) = window_for_session(&jsonl_path, start_ms, end_ms)
            .unwrap_or((start_ms, end_ms));
        per_session.push(DailySessionSummary {
            session_db_id: sid,
            session_id,
            project_name,
            summary,
            tags,
            start_ms: s_ms,
            end_ms: e_ms,
        });
    }
    per_session.sort_by_key(|s| s.start_ms);

    // Step 2: narrative.
    let narrative_input = render_narrative_input(&date, &per_session);
    let source_hash = format!("{}:{:x}", SCHEMA_VERSION, Sha256::digest(narrative_input.as_bytes()));

    if !force {
        let conn = db.conn();
        let cached: Option<(String, String)> = conn
            .query_row(
                "SELECT narrative, source_hash FROM daily_summaries WHERE date = ?1",
                params![date],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        if let Some((narrative, prev_hash)) = cached {
            if prev_hash == source_hash {
                return Ok(DailyReport { date, narrative, per_session, errors });
            }
        }
    }

    // Empty day → skip LLM, return a placeholder narrative.
    let narrative = if per_session.is_empty() {
        if !errors.is_empty() {
            format!("_(All {} session(s) failed to summarize. See error list below.)_", errors.len())
        } else {
            "_(no Claude Code activity recorded for this day)_".to_string()
        }
    } else {
        let cfg = load_config(&db).map_err(GenError::Db)?.ok_or(GenError::NotConfigured)?;
        let client = LlmClient::new(cfg);
        let raw = client
            .complete(NARRATIVE_SYSTEM_PROMPT, &narrative_input, 600)
            .await
            .map_err(GenError::Llm)?;
        raw.trim().to_string()
    };

    let now_ms = chrono::Utc::now().timestamp_millis();
    {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO daily_summaries (date, narrative, source_hash, generated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(date) DO UPDATE SET
               narrative = excluded.narrative,
               source_hash = excluded.source_hash,
               generated_at = excluded.generated_at",
            params![date, narrative, source_hash, now_ms],
        )
        .map_err(|e| GenError::Db(e.to_string()))?;
    }

    Ok(DailyReport { date, narrative, per_session, errors })
}

/// Look up the (min_ts, max_ts) of user/assistant messages for this session
/// within the day window. Returns None when no messages match.
fn window_for_session(jsonl_path: &str, start_ms: i64, end_ms: i64) -> Option<(i64, i64)> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    let file = File::open(jsonl_path).ok()?;
    let reader = BufReader::new(file);
    let mut min: Option<i64> = None;
    let mut max: Option<i64> = None;
    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let mt = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if mt != "user" && mt != "assistant" {
            continue;
        }
        let ts_str = val.get("timestamp").and_then(|v| v.as_str())?;
        let ts = match chrono::DateTime::parse_from_rfc3339(ts_str) {
            Ok(d) => d.timestamp_millis(),
            Err(_) => continue,
        };
        if ts < start_ms || ts > end_ms {
            continue;
        }
        min = Some(min.map_or(ts, |m| m.min(ts)));
        max = Some(max.map_or(ts, |m| m.max(ts)));
    }
    match (min, max) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    }
}

/// Render the input we feed to the narrative LLM call.
fn render_narrative_input(date: &str, per_session: &[DailySessionSummary]) -> String {
    use chrono::DateTime;
    let mut out = String::with_capacity(2048);
    out.push_str("DATE: ");
    out.push_str(date);
    out.push_str("\n\nSESSIONS (chronological):\n");

    let mut total_minutes: i64 = 0;
    for s in per_session {
        let mins = (s.end_ms - s.start_ms).max(0) / 60_000;
        total_minutes += mins;
        let start_label = DateTime::from_timestamp_millis(s.start_ms)
            .map(|d| d.format("%H:%M").to_string())
            .unwrap_or_default();
        let end_label = DateTime::from_timestamp_millis(s.end_ms)
            .map(|d| d.format("%H:%M").to_string())
            .unwrap_or_default();
        let tag_str = if s.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", s.tags.join(", "))
        };
        out.push_str(&format!(
            "- [{}] {}–{} ({}m): {}{}\n",
            s.project_name, start_label, end_label, mins, s.summary, tag_str
        ));
    }
    out.push_str(&format!(
        "\nTotal active: {}h {}m\n",
        total_minutes / 60,
        total_minutes % 60
    ));
    out
}

fn clamp_text(s: &str, max_chars: usize) -> String {
    let cleaned: String = s.replace('\n', " ").trim().to_string();
    let chars: Vec<char> = cleaned.chars().collect();
    if chars.len() <= max_chars {
        return cleaned;
    }
    let mut out: String = chars[..max_chars].iter().collect();
    out.push('…');
    out
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    tags.into_iter()
        .map(|t| {
            t.trim()
                .to_lowercase()
                .replace([' ', '_'], "-")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect::<String>()
        })
        .filter(|t| !t.is_empty() && t.len() <= 30)
        .take(4)
        .collect()
}
