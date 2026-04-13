use crate::db::Database;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    pub date: String,
    pub session_count: i64,
    pub user_msg_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_creation_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub total_tokens: i64,
}

#[tauri::command]
pub fn get_daily_usage(
    db: State<'_, Arc<Database>>,
    days: Option<i64>,
) -> Result<Vec<DailyUsage>, String> {
    let conn = db.conn();
    let limit = days.unwrap_or(30);

    let query = "
        SELECT
            date,
            COUNT(DISTINCT session_id) as session_count,
            SUM(user_msg_count) as user_msgs,
            SUM(input_tokens) as input_tokens,
            SUM(output_tokens) as output_tokens,
            SUM(cache_creation_tokens) as cache_creation,
            SUM(cache_read_tokens) as cache_read
        FROM daily_token_usage
        WHERE date != 'unknown'
        GROUP BY date
        ORDER BY date DESC
        LIMIT ?1
    ";

    let mut stmt = conn.prepare(query).map_err(|e| format!("DB error: {}", e))?;
    let rows = stmt
        .query_map([limit], |row| {
            let input: i64 = row.get(3)?;
            let output: i64 = row.get(4)?;
            let cache_creation: i64 = row.get(5)?;
            let cache_read: i64 = row.get(6)?;
            Ok(DailyUsage {
                date: row.get(0)?,
                session_count: row.get(1)?,
                user_msg_count: row.get(2)?,
                total_input_tokens: input,
                total_output_tokens: output,
                total_cache_creation_tokens: cache_creation,
                total_cache_read_tokens: cache_read,
                total_tokens: input + output + cache_creation + cache_read,
            })
        })
        .map_err(|e| format!("DB error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}
