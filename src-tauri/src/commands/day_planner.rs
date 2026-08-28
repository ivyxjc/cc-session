//! Generate Day Planner-style time blocks (à la Obsidian's day-planner plugin)
//! by scanning each session's JSONL for messages timestamped on the requested day.

use crate::activity::{collect_timestamps_on_day, split_into_blocks, GAP_SPLIT_MINUTES};
use crate::db::Database;
use crate::llm::daily::{generate_daily, DailyReport};
use crate::llm::summary::RefLink;
use chrono::DateTime;
use rusqlite::params;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayPlannerBlock {
    pub session_db_id: i64,
    pub session_id: String,
    pub project_name: String,
    /// Session-level summary (whole arc). Fallback for older days that haven't
    /// been run through the daily map-reduce yet.
    pub title: String,
    /// Session-level AI tags.
    pub ai_tags: Vec<String>,
    /// Day-specific summary (from `daily_session_summaries`). When present, the
    /// frontend prefers this over `title`.
    pub daily_summary: Option<String>,
    /// Day-specific tags.
    pub daily_tags: Vec<String>,
    /// Day-specific Jira/PR references (from the daily map step).
    pub daily_refs: Vec<RefLink>,
    /// Absolute timestamps (epoch ms, UTC). Frontend renders these via JS Date
    /// so the displayed HH:MM follows the OS timezone reliably.
    pub start_ms: i64,
    pub end_ms: i64,
}

#[tauri::command]
pub fn get_day_planner(
    db: State<'_, Arc<Database>>,
    start_ms: i64,
    end_ms: i64,
    date: Option<String>,
) -> Result<Vec<DayPlannerBlock>, String> {
    if end_ms <= start_ms {
        return Err(format!("invalid range: end_ms ({}) must be > start_ms ({})", end_ms, start_ms));
    }

    // JOIN key for daily_session_summaries cache. Frontend should pass the
    // user-facing YYYY-MM-DD it picked — that's the same string used when
    // writing the cache via generate_daily_summary. Falling back to deriving
    // from start_ms via chrono::Local works on a healthy system but breaks if
    // chrono::Local resolves to UTC (e.g. missing TZ env in the Tauri process).
    let day_key = date.unwrap_or_else(|| {
        let dt = DateTime::from_timestamp_millis(start_ms).unwrap_or_default();
        dt.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string()
    });

    let conn = db.conn();
    // First-pass filter: sessions whose [started_at, last_active] window
    // intersects the day. This avoids walking JSONLs that can't possibly have
    // messages on the date. LEFT JOIN `daily_session_summaries` so we can
    // surface cached per-day LLM summaries when they exist.
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.session_id, s.jsonl_path,
                    p.display_name, s.summary, s.slug, s.ai_tags,
                    d.summary, d.tags, d.noise, d.refs, d.generated_at
             FROM sessions s
             JOIN projects p ON s.project_id = p.id
             LEFT JOIN daily_session_summaries d
               ON d.session_db_id = s.id AND d.date = ?3
             WHERE COALESCE(s.last_active, s.started_at, 0) >= ?1
               AND COALESCE(s.started_at, s.last_active, 0) <= ?2",
        )
        .map_err(|e| format!("DB error: {}", e))?;

    type SessionRow = (
        i64, String, String, String,
        Option<String>, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<i64>, Option<String>,
        Option<i64>,
    );
    let rows = stmt
        .query_map(params![start_ms, end_ms, day_key], |row| -> rusqlite::Result<SessionRow> {
            Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
                row.get(4)?, row.get(5)?, row.get(6)?,
                row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
            ))
        })
        .map_err(|e| format!("DB error: {}", e))?;

    let mut blocks: Vec<DayPlannerBlock> = Vec::new();

    for (sid, session_id, jsonl_path, project_name, summary, slug, ai_tags_json, daily_summary, daily_tags_json, noise, daily_refs_json, daily_generated_at) in rows.flatten() {
        let timestamps = collect_timestamps_on_day(&jsonl_path, start_ms, end_ms);
        if timestamps.is_empty() {
            continue;
        }
        // The daily map-step LLM judged this slice as no-actual-work
        // (/clear, bare greeting…) — hide it from the timeline. Only trust the
        // verdict while it is still current: work done after the summary was
        // generated was never judged, so the session stays visible.
        let noise_is_current = daily_generated_at
            .map(|gen_ms| timestamps.last().copied().unwrap_or(0) <= gen_ms)
            .unwrap_or(false);
        if noise.unwrap_or(0) != 0 && noise_is_current {
            continue;
        }
        let ranges = split_into_blocks(&timestamps, GAP_SPLIT_MINUTES * 60_000);
        let title = summary
            .filter(|s| !s.trim().is_empty())
            .or_else(|| slug.filter(|s| !s.trim().is_empty()))
            .unwrap_or_else(|| "[Untitled]".to_string());
        let ai_tags: Vec<String> = ai_tags_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let daily_tags: Vec<String> = daily_tags_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let daily_refs: Vec<RefLink> = daily_refs_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        for (s_ms, e_ms) in ranges {
            blocks.push(DayPlannerBlock {
                session_db_id: sid,
                session_id: session_id.clone(),
                project_name: project_name.clone(),
                title: title.clone(),
                ai_tags: ai_tags.clone(),
                daily_summary: daily_summary.clone(),
                daily_tags: daily_tags.clone(),
                daily_refs: daily_refs.clone(),
                start_ms: s_ms,
                end_ms: e_ms,
            });
        }
    }

    blocks.sort_by_key(|b| b.start_ms);
    Ok(blocks)
}

/// Run the two-step map-reduce daily report:
///   - per-session-per-day summaries (cached in `daily_session_summaries`)
///   - final narrative (cached in `daily_summaries`)
/// `date` should be the frontend's YYYY-MM-DD local-day label.
#[tauri::command]
pub async fn generate_daily_summary(
    db: State<'_, Arc<Database>>,
    date: String,
    start_ms: i64,
    end_ms: i64,
    force: Option<bool>,
) -> Result<DailyReport, String> {
    let db_arc = (*db).clone();
    let force = force.unwrap_or(false);
    generate_daily(db_arc, date, start_ms, end_ms, force)
        .await
        .map_err(|e| e.to_string())
}
