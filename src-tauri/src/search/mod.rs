use crate::sources::{self, Provider};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Bumped whenever the indexer's coverage changes. `content_indexed_at` is
/// compared against the file's mtime, which does not move when our own rules
/// do — so without this, already-indexed sessions would keep their old, thinner
/// rows forever. A mismatch forces a full rebuild on the next scan.
pub const FTS_SCHEMA_VERSION: i64 = 2;

/// Index a session's searchable content into messages_fts.
/// Replaces any existing rows for this session_db_id.
pub fn index_session_content(
    conn: &Connection,
    session_db_id: i64,
    provider: Provider,
    jsonl_path: &Path,
) -> Result<(), String> {
    // Drop stale rows for this session first
    conn.execute(
        "DELETE FROM messages_fts WHERE session_db_id = ?1",
        params![session_db_id],
    )
    .map_err(|e| format!("FTS delete error: {}", e))?;

    if !jsonl_path.exists() {
        return Ok(()); // Missing file isn't fatal — silently skip
    }

    let mut stmt = conn
        .prepare(
            "INSERT INTO messages_fts (content, session_db_id, message_uuid, role, timestamp_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .map_err(|e| format!("FTS prepare error: {}", e))?;

    match provider {
        // Claude files can be hundreds of MB — stream them line by line.
        Provider::Claude => {
            let file = File::open(jsonl_path).map_err(|e| format!("FTS open error: {}", e))?;
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&line) {
                    index_raw_message(&mut stmt, session_db_id, &raw)?;
                }
            }
        }
        // Other providers are projected into the Claude line shape.
        _ => {
            for raw in sources::raw_messages(provider, jsonl_path)? {
                index_raw_message(&mut stmt, session_db_id, &raw)?;
            }
        }
    }

    Ok(())
}

fn index_raw_message(
    stmt: &mut rusqlite::Statement<'_>,
    session_db_id: i64,
    raw: &serde_json::Value,
) -> Result<(), String> {
    {
        let msg_type = raw.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if msg_type != "user" && msg_type != "assistant" {
            return Ok(());
        }

        let uuid = raw.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp_ms = raw
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0);

        let content_arr = raw
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array());

        // String-form content (legacy user messages where content is a plain string)
        if content_arr.is_none() {
            if let Some(s) = raw
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
            {
                if !s.trim().is_empty() {
                    stmt.execute(params![s, session_db_id, uuid, msg_type, timestamp_ms])
                        .map_err(|e| format!("FTS insert error: {}", e))?;
                }
            }
            return Ok(());
        }

        for block in content_arr.unwrap() {
            let bt = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let (text, role): (Option<String>, &str) = match bt {
                "text" => (
                    block.get("text").and_then(|v| v.as_str()).map(str::to_owned),
                    msg_type,
                ),
                "thinking" => (
                    block.get("thinking").and_then(|v| v.as_str()).map(str::to_owned),
                    "thinking",
                ),
                // What was actually done: tool name plus the string values of
                // its input — file paths, shell commands, patterns, replacement
                // text. Most of what one wants to find again lives here.
                "tool_use" => (tool_use_text(block), "tool"),
                // And what came back: command output, errors, file contents.
                "tool_result" => (tool_result_text(block), "tool_result"),
                _ => (None, ""),
            };
            if let Some(t) = text {
                if !t.trim().is_empty() {
                    stmt.execute(params![t, session_db_id, uuid, role, timestamp_ms])
                        .map_err(|e| format!("FTS insert error: {}", e))?;
                }
            }
        }
    }

    Ok(())
}

/// Cap per indexed tool block. Session files reach hundreds of MB, largely tool
/// traffic; indexing it whole would multiply the index size for text nobody
/// searches. The head of a command or a diff is what identifies it.
const TOOL_INPUT_MAX: usize = 2_000;
const TOOL_OUTPUT_MAX: usize = 4_000;

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

/// Collect the string leaves of a tool_use input, skipping keys. Values are what
/// gets searched for ("src/main.rs", "cargo test"); key names are noise.
fn collect_strings(value: &serde_json::Value, out: &mut Vec<String>, budget: &mut usize) {
    if *budget == 0 {
        return;
    }
    match value {
        serde_json::Value::String(s) => {
            let s = s.trim();
            if !s.is_empty() {
                let take = truncate_chars(s, *budget);
                *budget -= take.chars().count();
                out.push(take);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_strings(item, out, budget);
            }
        }
        serde_json::Value::Object(map) => {
            for (_, v) in map {
                collect_strings(v, out, budget);
            }
        }
        _ => {}
    }
}

fn tool_use_text(block: &serde_json::Value) -> Option<String> {
    let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let mut parts: Vec<String> = Vec::new();
    if !name.is_empty() {
        parts.push(name.to_string());
    }
    let mut budget = TOOL_INPUT_MAX;
    if let Some(input) = block.get("input") {
        collect_strings(input, &mut parts, &mut budget);
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// tool_result content is either a plain string or blocks. Only text blocks are
/// taken — an image block carries base64 that would bloat the index for nothing.
fn tool_result_text(block: &serde_json::Value) -> Option<String> {
    let content = block.get("content")?;
    if let Some(s) = content.as_str() {
        return Some(truncate_chars(s, TOOL_OUTPUT_MAX));
    }
    let arr = content.as_array()?;
    let mut budget = TOOL_OUTPUT_MAX;
    let mut parts: Vec<String> = Vec::new();
    for item in arr {
        if budget == 0 {
            break;
        }
        if item.get("type").and_then(|v| v.as_str()) == Some("text") {
            if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                let take = truncate_chars(t.trim(), budget);
                budget -= take.chars().count();
                if !take.is_empty() {
                    parts.push(take);
                }
            }
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSearchResult {
    pub session_db_id: i64,
    pub session_id: String,
    pub slug: Option<String>,
    pub project_name: String,
    pub project_path: String,
    pub message_uuid: String,
    pub role: String,
    pub timestamp_ms: i64,
    pub snippet: String,
}

/// Characters of context returned around a hit. The trigram tokenizer makes
/// FTS5's own `snippet()` useless here — its token count is in 3-character
/// trigrams and caps at 64, so it yields ~30 characters, far too little to
/// tell whether a hit is the one you wanted. A substr window around the first
/// occurrence gives real context; the frontend highlights the terms.
const SNIPPET_CHARS: i64 = 300;
const SNIPPET_LEAD: i64 = 80;

/// Upper bound on the recency penalty, in bm25 units. bm25 spans roughly 9
/// units between a strong and a weak match, so an uncapped linear decay (which
/// reaches +11 over a year) buries an exact match from last year beneath a weak
/// one from today. Capped, recency only breaks ties between comparable matches.
const AGE_PENALTY_CAP: f64 = 3.0;

/// Rank bonus for containing the query as written, so that when the terms do
/// appear together that row still outranks one where they are merely scattered.
const PHRASE_BONUS: f64 = -2.0;

/// The trigram index cannot match anything shorter than 3 characters, so terms
/// below that would silently reduce an AND query to zero rows.
fn indexable_terms(query: &str) -> Vec<&str> {
    query
        .split_whitespace()
        .filter(|t| t.chars().count() >= 3)
        .collect()
}

/// Search message content using FTS5 + BM25, recency-weighted.
///
/// Terms are ANDed rather than run as one literal phrase: a query like
/// "terminal pane" should find sessions discussing both, not only the ones
/// where the words happen to be adjacent. `provider` of `None` searches every
/// provider.
pub fn search_messages(
    conn: &Connection,
    query: &str,
    provider: Option<Provider>,
    limit: usize,
) -> Result<Vec<ContentSearchResult>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    // Nothing the trigram index can match (e.g. a two-character CJK word).
    let terms = indexable_terms(q);
    let Some(first_term) = terms.first().copied() else {
        return search_messages_like(conn, q, provider, limit);
    };

    // Quote every term so FTS5 reads none of them as an operator (AND/OR/NOT).
    let match_expr = terms
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ");

    let now_ms = chrono::Utc::now().timestamp_millis();
    // Linear decay: every 30 days adds +1 to bm25 (worse rank, since smaller = better)
    let decay_per_ms: f64 = 1.0 / (30.0 * 86_400_000.0);

    let sql = format!("
        SELECT
            f.session_db_id,
            s.session_id,
            s.slug,
            p.display_name,
            p.original_path,
            f.message_uuid,
            f.role,
            f.timestamp_ms,
            substr(f.content, max(1, instr(lower(f.content), lower(?5)) - {lead}), {chars}) AS snip,
            bm25(messages_fts)
              + min((?2 - f.timestamp_ms) * ?3, {cap})
              + (CASE WHEN instr(lower(f.content), lower(?6)) > 0 THEN {bonus} ELSE 0 END) AS weighted
        FROM messages_fts f
        JOIN sessions s ON s.id = f.session_db_id
        JOIN projects p ON p.id = s.project_id
        WHERE messages_fts MATCH ?1
          AND (?7 IS NULL OR s.provider = ?7)
        ORDER BY weighted ASC
        LIMIT ?4
    ", lead = SNIPPET_LEAD, chars = SNIPPET_CHARS, cap = AGE_PENALTY_CAP, bonus = PHRASE_BONUS);

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("FTS query prepare error: {}", e))?;

    let rows = stmt
        .query_map(
            params![match_expr, now_ms, decay_per_ms, limit as i64, first_term, q, provider],
            |row| {
                Ok(ContentSearchResult {
                    session_db_id: row.get(0)?,
                    session_id: row.get(1)?,
                    slug: row.get(2)?,
                    project_name: row.get(3)?,
                    project_path: row.get(4)?,
                    message_uuid: row.get(5)?,
                    role: row.get(6)?,
                    timestamp_ms: row.get(7)?,
                    snippet: row.get(8)?,
                })
            },
        )
        .map_err(|e| format!("FTS query error: {}", e))?;

    Ok(rows.flatten().collect())
}

/// Fallback for short queries (<3 chars) where trigram tokenizer can't index.
/// Linear scan via LIKE — slower but correct.
fn search_messages_like(
    conn: &Connection,
    q: &str,
    provider: Option<Provider>,
    limit: usize,
) -> Result<Vec<ContentSearchResult>, String> {
    let pattern = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));
    let sql = format!("
        SELECT
            f.session_db_id,
            s.session_id,
            s.slug,
            p.display_name,
            p.original_path,
            f.message_uuid,
            f.role,
            f.timestamp_ms,
            substr(f.content, max(1, instr(lower(f.content), lower(?1)) - {lead}), {chars}) AS snip
        FROM messages_fts f
        JOIN sessions s ON s.id = f.session_db_id
        JOIN projects p ON p.id = s.project_id
        WHERE f.content LIKE ?2 ESCAPE '\\'
          AND (?4 IS NULL OR s.provider = ?4)
        ORDER BY f.timestamp_ms DESC
        LIMIT ?3
    ", lead = SNIPPET_LEAD, chars = SNIPPET_CHARS);
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("LIKE query prepare error: {}", e))?;
    let rows = stmt
        .query_map(params![q, pattern, limit as i64, provider], |row| {
            Ok(ContentSearchResult {
                session_db_id: row.get(0)?,
                session_id: row.get(1)?,
                slug: row.get(2)?,
                project_name: row.get(3)?,
                project_path: row.get(4)?,
                message_uuid: row.get(5)?,
                role: row.get(6)?,
                timestamp_ms: row.get(7)?,
                snippet: row.get(8)?,
            })
        })
        .map_err(|e| format!("LIKE query error: {}", e))?;

    Ok(rows.flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE projects (id INTEGER PRIMARY KEY, provider TEXT, display_name TEXT, original_path TEXT);
             CREATE TABLE sessions (id INTEGER PRIMARY KEY, provider TEXT, session_id TEXT, slug TEXT, project_id INTEGER);
             CREATE VIRTUAL TABLE messages_fts USING fts5(
                content, session_db_id UNINDEXED, message_uuid UNINDEXED,
                role UNINDEXED, timestamp_ms UNINDEXED, tokenize = 'trigram');
             INSERT INTO projects VALUES (1, 'claude', 'proj', '/tmp/proj');
             INSERT INTO sessions VALUES (1, 'claude', 's1', 'slug', 1);",
        )
        .unwrap();
        conn
    }

    fn add(conn: &Connection, uuid: &str, content: &str, ts: i64) {
        conn.execute(
            "INSERT INTO messages_fts (content, session_db_id, message_uuid, role, timestamp_ms)
             VALUES (?1, 1, ?2, 'user', ?3)",
            params![content, uuid, ts],
        )
        .unwrap();
    }

    fn now() -> i64 {
        chrono::Utc::now().timestamp_millis()
    }

    fn uuids(results: &[ContentSearchResult]) -> Vec<&str> {
        results.iter().map(|r| r.message_uuid.as_str()).collect()
    }

    #[test]
    fn multi_word_query_matches_terms_that_are_not_adjacent() {
        let conn = test_db();
        add(&conn, "scattered", "the terminal froze while resizing the pane", now());
        add(&conn, "adjacent", "the terminal pane is broken", now());
        add(&conn, "neither", "completely unrelated text here", now());

        let hits = search_messages(&conn, "terminal pane", None, 10).unwrap();
        // A literal-phrase query would only have found "adjacent".
        assert_eq!(hits.len(), 2);
        assert!(uuids(&hits).contains(&"scattered"));
    }

    #[test]
    fn adjacent_phrase_outranks_scattered_terms() {
        let conn = test_db();
        add(&conn, "scattered", "the terminal froze while resizing the pane", now());
        add(&conn, "adjacent", "the terminal pane is broken", now());

        let hits = search_messages(&conn, "terminal pane", None, 10).unwrap();
        assert_eq!(hits[0].message_uuid, "adjacent");
    }

    #[test]
    fn an_old_exact_match_still_beats_a_recent_weak_one() {
        let conn = test_db();
        let year_ago = now() - 365 * 86_400_000;
        add(&conn, "old-strong", "terminal pane", year_ago);
        add(&conn, "new-weak", &format!("terminal {}", "filler word ".repeat(200)), now());

        let hits = search_messages(&conn, "terminal pane", None, 10).unwrap();
        // Uncapped linear decay reached +12 over a year and buried this.
        assert_eq!(hits[0].message_uuid, "old-strong");
    }

    #[test]
    fn queries_below_the_trigram_minimum_fall_back_to_like() {
        let conn = test_db();
        add(&conn, "cjk", "重构了终端面板的渲染逻辑", now());

        // "终端" is two characters — the trigram index cannot match it.
        let hits = search_messages(&conn, "终端", None, 10).unwrap();
        assert_eq!(uuids(&hits), vec!["cjk"]);
    }

    #[test]
    fn snippet_carries_real_context_around_the_hit() {
        let conn = test_db();
        let filler = "x".repeat(400);
        add(&conn, "long", &format!("{} terminal pane {}", filler, filler), now());

        let hits = search_messages(&conn, "terminal", None, 10).unwrap();
        // FTS5's own snippet() yields ~30 chars under the trigram tokenizer.
        assert!(hits[0].snippet.chars().count() > 200, "got {:?}", hits[0].snippet);
        assert!(hits[0].snippet.contains("terminal"));
    }

    #[test]
    fn tool_use_indexes_the_name_and_its_string_arguments() {
        let block = serde_json::json!({
            "type": "tool_use",
            "name": "Bash",
            "input": {"command": "cargo test --workspace", "timeout": 120}
        });
        let text = tool_use_text(&block).unwrap();
        assert!(text.contains("Bash"));
        assert!(text.contains("cargo test --workspace"));
        // Numbers and key names are not content anyone searches for.
        assert!(!text.contains("timeout"));
    }

    #[test]
    fn tool_result_skips_image_blocks_and_caps_length() {
        let block = serde_json::json!({
            "type": "tool_result",
            "content": [
                {"type": "text", "text": "error: cannot find value `foo`"},
                {"type": "image", "source": {"data": "AAAABBBBCCCC"}}
            ]
        });
        let text = tool_result_text(&block).unwrap();
        assert!(text.contains("cannot find value"));
        assert!(!text.contains("AAAABBBB"));

        let huge = serde_json::json!({"type": "tool_result", "content": "y".repeat(50_000)});
        assert_eq!(tool_result_text(&huge).unwrap().chars().count(), TOOL_OUTPUT_MAX);
    }
}
