//! Map-reduce daily activity reports.
//!
//! Step 1 (map): for each session active on a day, build an LLM input from just
//! that day's slice (anchor + tail constrained to the [start, end] window) and
//! ask the model "what was done in this slice". Cached in `daily_session_summaries`.
//!
//! Step 2 (reduce): collect all per-session daily summaries, feed them back to the
//! model, ask for a unified narrative. Cached in `daily_summaries`.

use crate::activity::{collect_timestamps_on_day, split_into_blocks, GAP_SPLIT_MINUTES};
use crate::db::Database;
use crate::llm::client::{parse_json_payload, LlmClient};
use crate::llm::input_builder;
use crate::llm::summary::{load_config, GenError, RefLink, SummaryAndTags};
use crate::sources::Provider;
use rusqlite::params;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;

const SCHEMA_VERSION: &str = "v6";

const PER_SESSION_SYSTEM_PROMPT: &str = "You are summarizing a single day's slice of a Claude Code session. The session may span multiple days; you only see the messages that fell within the target date. Each turn is labeled with its local time (e.g. [user 14:32]). Describe what was *done that day specifically* — not the session's whole arc.

Return ONLY JSON, no markdown fences, no commentary:
{\"summary\": \"<= 80 chars\", \"tags\": [\"...\", \"...\"], \"noise\": false, \"refs\": [{\"label\": \"SCEM-12059\", \"url\": null}, {\"label\": \"flex#14521\", \"url\": \"https://github.com/...\"}]}

Rules:
- refs: Jira issue keys (e.g. SCEM-12059) and pull requests that the day's work was actually about — not every passing mention. Take PR urls from the PR LINKS section or from urls visible in the conversation; for Jira keys without a visible url, set url to null. NEVER fabricate a url. Empty array when none.
- EVIDENCE: summarize only what ACTUALLY happened, evidenced by assistant turns and tool usage. A request alone is not work. If the slice contains only user requests with no assistant response, or ends with \"[Request interrupted by user]\" before anything was done, or TOOLS USED is (none) with no assistant output — then NOTHING happened: set noise=true and state it plainly (e.g. \"提交请求发出后被中断,未执行\").
- noise: true ONLY when the slice contains no actual work at all — e.g. just a /clear or /model command, a bare greeting, an interrupted request, or a \"continue\" that produced nothing. Any real outcome, however small (a question answered, a commit completed, a config change), means noise: false. Noise slices are hidden from the daily report, so still fill in summary/tags as a fallback.
- COVERAGE: weight the summary by where the day's effort actually went, using the turn times. A commit/wrap-up at the end of the slice is the conclusion, NOT the headline — never let the last event eclipse hours of preceding work.
- summary: specific action verbs + object (\"Tweaked PR11152 description & cleaned Cursor Bugbot notes\", not \"Worked on PR\").
- Some sessions are automated runs whose INITIAL REQUEST is a canned template (e.g. \"You are committing staged git changes...\" or a review-bot prompt). NEVER echo the template — describe only what the conversation shows was actually done, not what the template asked for.
- Brief slices (a greeting, one slash command, a single quick question) still get a concrete summary of what actually happened — never generic filler like \"brief interaction\".
- LANGUAGE (hard rule): if the human user's messages are mostly Chinese, the summary MUST be written in Chinese. Templates and system text do not count as user messages.
- tags: 2-4 short lowercase English topical labels, hyphen-joined.";

const NARRATIVE_SYSTEM_PROMPT: &str = "Generate a concise daily activity report from per-session summaries of one day.

Input you receive:
- The date
- For each session: project name, time range (local time), duration, summary, tags, and refs (Jira tickets / PRs, some with urls)
- Total active time

About the input:
- Sessions often run in PARALLEL — overlapping time ranges are normal, not an error. Organize by theme/project, not strict chronology.
- Many slices are tiny (a minute or two: an automated commit, a one-off question). Fold them into their theme — or one short \"misc\" mention — instead of giving each fragment its own bullet.

Output Markdown, ≤ 400 chars total:
- 2-3 sentence overview of the day at the top
- Grouped bullets by theme (MERGE related work across sessions, do NOT just list each session)
- Mention the Jira tickets / PRs the work was about: render refs with a url as markdown links ([flex#14521](https://...)), and bare Jira keys as plain text. Never invent urls.
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
    /// Sum of gap-split block durations — actual engaged time, not span.
    pub active_ms: i64,
    /// Jira/PR references the model extracted for this day's work.
    pub refs: Vec<RefLink>,
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
/// Returns `(summary, tags, noise, refs)`.
pub async fn generate_for_session_day(
    db: Arc<Database>,
    session_db_id: i64,
    date: &str,
    start_ms: i64,
    end_ms: i64,
    force: bool,
) -> Result<(String, Vec<String>, bool, Vec<RefLink>), GenError> {
    // Look up session metadata.
    let (provider, jsonl_path, prev_hash, prev_summary, prev_tags_json, prev_noise, prev_refs_json): (
        Provider,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
    ) = {
        let conn = db.conn();
        conn.query_row(
            "SELECT s.provider, s.jsonl_path, d.input_hash, d.summary, d.tags, d.noise, d.refs
             FROM sessions s
             LEFT JOIN daily_session_summaries d
               ON d.session_db_id = s.id AND d.date = ?2
             WHERE s.id = ?1",
            params![session_db_id, date],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => GenError::SessionNotFound,
            other => GenError::Db(other.to_string()),
        })?
    };

    // Build LLM input for this window.
    let path = PathBuf::from(&jsonl_path);
    let input = input_builder::build_for_window(provider, &path, Some((start_ms, end_ms)))
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
                let refs: Vec<RefLink> = prev_refs_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                return Ok((summary.to_string(), tags, prev_noise.unwrap_or(0) != 0, refs));
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
    let refs = dedupe_refs(parsed.refs);
    let refs_json = serde_json::to_string(&refs).unwrap_or_else(|_| "[]".to_string());

    let now_ms = chrono::Utc::now().timestamp_millis();
    {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO daily_session_summaries
              (session_db_id, date, summary, tags, input_hash, generated_at, noise, refs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(session_db_id, date) DO UPDATE SET
               summary = excluded.summary,
               tags = excluded.tags,
               input_hash = excluded.input_hash,
               generated_at = excluded.generated_at,
               noise = excluded.noise,
               refs = excluded.refs",
            params![session_db_id, date, summary, tags_json, new_hash, now_ms, parsed.noise, refs_json],
        )
        .map_err(|e| GenError::Db(e.to_string()))?;
    }

    Ok((summary, tags, parsed.noise, refs))
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
        // One pass over the JSONL gives presence, span AND active time.
        let timestamps = collect_timestamps_on_day(&jsonl_path, start_ms, end_ms);
        if timestamps.is_empty() {
            continue;
        }
        let (summary, tags, noise, refs) = match generate_for_session_day(
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
        // The model judged this slice as no-actual-work (/clear, bare greeting…)
        // — keep it cached but leave it out of the narrative.
        if noise {
            continue;
        }
        let active_ms: i64 = split_into_blocks(&timestamps, GAP_SPLIT_MINUTES * 60_000)
            .iter()
            .map(|(s, e)| e - s)
            .sum();
        per_session.push(DailySessionSummary {
            session_db_id: sid,
            session_id,
            project_name,
            summary,
            tags,
            start_ms: timestamps[0],
            end_ms: *timestamps.last().unwrap(),
            active_ms,
            refs,
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

/// Render the input we feed to the narrative LLM call.
fn render_narrative_input(date: &str, per_session: &[DailySessionSummary]) -> String {
    use chrono::DateTime;
    let mut out = String::with_capacity(2048);
    out.push_str("DATE: ");
    out.push_str(date);
    out.push_str("\n\nSESSIONS (chronological):\n");

    let mut total_minutes: i64 = 0;
    for s in per_session {
        // Active minutes, not first→last span — parallel/fragmented sessions
        // would otherwise massively inflate the totals fed to the model.
        let mins = s.active_ms.max(0) / 60_000;
        total_minutes += mins;
        // Local time — must match the day window the user picked, not UTC.
        let start_label = DateTime::from_timestamp_millis(s.start_ms)
            .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
            .unwrap_or_default();
        let end_label = DateTime::from_timestamp_millis(s.end_ms)
            .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
            .unwrap_or_default();
        let tag_str = if s.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", s.tags.join(", "))
        };
        let refs_str = if s.refs.is_empty() {
            String::new()
        } else {
            let parts: Vec<String> = s
                .refs
                .iter()
                .map(|r| match r.url.as_deref() {
                    Some(u) => format!("{} ({})", r.label, u),
                    None => r.label.clone(),
                })
                .collect();
            format!(" | refs: {}", parts.join(", "))
        };
        out.push_str(&format!(
            "- [{}] {}–{} (active {}m): {}{}{}\n",
            s.project_name, start_label, end_label, mins, s.summary, tag_str, refs_str
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

/// Collapse refs that repeat the same label, preferring the one that carries a url.
fn dedupe_refs(refs: Vec<RefLink>) -> Vec<RefLink> {
    let mut out: Vec<RefLink> = Vec::new();
    for r in refs {
        match out.iter_mut().find(|e| e.label == r.label) {
            Some(existing) => {
                if existing.url.is_none() {
                    existing.url = r.url;
                }
            }
            None => out.push(r),
        }
    }
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
