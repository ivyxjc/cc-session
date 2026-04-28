use crate::db::Database;
use crate::llm::client::{LlmClient, LlmConfig};
use crate::llm::summary::{generate_for_session, load_config, GenerateOutcome};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[tauri::command]
pub fn get_ai_summary_config(db: State<'_, Arc<Database>>) -> Result<AiSummaryConfig, String> {
    let conn = db.conn();
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_config WHERE key = 'ai_summary_config'",
            [],
            |row| row.get(0),
        )
        .ok();
    if let Some(s) = raw {
        return serde_json::from_str(&s).map_err(|e| e.to_string());
    }
    Ok(AiSummaryConfig::default())
}

#[tauri::command]
pub fn set_ai_summary_config(
    db: State<'_, Arc<Database>>,
    config: AiSummaryConfig,
) -> Result<(), String> {
    let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    let conn = db.conn();
    conn.execute(
        "INSERT INTO app_config (key, value) VALUES ('ai_summary_config', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![json],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn test_ai_summary_connection(config: AiSummaryConfig) -> Result<String, String> {
    let cfg = LlmConfig {
        base_url: config.base_url,
        api_key: config.api_key,
        model: config.model,
    };
    if cfg.base_url.is_empty() || cfg.api_key.is_empty() || cfg.model.is_empty() {
        return Err("Base URL, API key, and model are all required".into());
    }
    let client = LlmClient::new(cfg);
    let resp = client.ping().await.map_err(|e| e.to_string())?;
    Ok(resp.trim().to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryResult {
    pub generated: bool,
    pub summary: Option<String>,
    pub tags: Option<Vec<String>>,
}

impl From<GenerateOutcome> for AiSummaryResult {
    fn from(o: GenerateOutcome) -> Self {
        Self {
            generated: o.generated,
            summary: o.summary,
            tags: o.tags,
        }
    }
}

#[tauri::command]
pub async fn generate_ai_summary(
    db: State<'_, Arc<Database>>,
    session_db_id: i64,
    force: Option<bool>,
) -> Result<AiSummaryResult, String> {
    let db = (*db).clone();
    let force = force.unwrap_or(true);
    let outcome = generate_for_session(db, session_db_id, force)
        .await
        .map_err(|e| e.to_string())?;
    Ok(outcome.into())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgressEvent {
    pub current: usize,
    pub total: usize,
    pub session_db_id: i64,
    pub status: String,        // "ok" | "skipped" | "error"
    pub error: Option<String>, // populated when status == "error"
    pub summary: Option<String>,
}

/// Kick off batch generation across (optionally filtered) sessions.
/// Returns immediately; progress is emitted via the `ai-summary-progress` event.
/// `force=false` lets the hash-based cache skip unchanged sessions.
#[tauri::command]
pub async fn generate_ai_summaries_batch(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    force: Option<bool>,
) -> Result<usize, String> {
    let db = (*db).clone();
    let force = force.unwrap_or(false);

    // Collect target session ids (only non-hidden ones).
    let ids: Vec<i64> = {
        let conn = db.conn();
        let mut stmt = conn
            .prepare("SELECT id FROM sessions WHERE is_hidden = 0 ORDER BY last_active DESC NULLS LAST")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let total = ids.len();
    if total == 0 {
        return Ok(0);
    }

    // Verify config exists up front so we don't loop with a permanent error.
    if load_config(&db).map_err(|e| e.to_string())?.is_none() {
        return Err("AI summary not configured (set base URL, API key and model in Settings)".into());
    }

    // Spawn the worker. Sequential for now (concurrency added once stable).
    let app_handle = app.clone();
    let db_for_task = db.clone();
    tauri::async_runtime::spawn(async move {
        for (idx, session_id) in ids.iter().enumerate() {
            let current = idx + 1;
            let outcome = generate_for_session(db_for_task.clone(), *session_id, force).await;
            let evt = match outcome {
                Ok(o) => BatchProgressEvent {
                    current,
                    total,
                    session_db_id: *session_id,
                    status: if o.generated { "ok".into() } else { "skipped".into() },
                    error: None,
                    summary: o.summary,
                },
                Err(e) => BatchProgressEvent {
                    current,
                    total,
                    session_db_id: *session_id,
                    status: "error".into(),
                    error: Some(e.to_string()),
                    summary: None,
                },
            };
            let _ = app_handle.emit("ai-summary-progress", evt);
        }
        let _ = app_handle.emit(
            "ai-summary-done",
            serde_json::json!({ "total": total }),
        );
    });

    Ok(total)
}
