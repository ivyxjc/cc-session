//! Project paths the user has chosen not to see.
//!
//! Ignoring is a view filter, not a scan filter: the sessions stay indexed, so
//! un-ignoring a path restores it immediately instead of requiring a re-scan,
//! and the favorites and tags attached to those sessions survive.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreConfig {
    #[serde(default)]
    pub prefixes: Vec<String>,
}

pub fn read_config(conn: &Connection) -> IgnoreConfig {
    conn.query_row(
        "SELECT value FROM app_config WHERE key = 'ignore_config'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|j| serde_json::from_str(&j).ok())
    .unwrap_or_default()
}

pub fn write_config(conn: &Connection, config: &IgnoreConfig) -> Result<(), String> {
    let json = serde_json::to_string(config).map_err(|e| format!("Serialize error: {}", e))?;
    conn.execute(
        "INSERT INTO app_config (key, value) VALUES ('ignore_config', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![json],
    )
    .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

/// Blank entries would match every path, so they are dropped rather than
/// silently hiding the whole index.
pub fn prefixes(conn: &Connection) -> Vec<String> {
    read_config(conn)
        .prefixes
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

pub fn is_ignored(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|p| path.starts_with(p.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_on_prefix_only() {
        let ps = vec!["/tmp/skip".to_string()];
        assert!(is_ignored("/tmp/skip", &ps));
        assert!(is_ignored("/tmp/skip/deeper", &ps));
        assert!(!is_ignored("/tmp/keep", &ps));
        assert!(!is_ignored("/var/tmp/skip", &ps), "a prefix is not a substring");
    }

    #[test]
    fn blank_entries_are_dropped_rather_than_matching_everything() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE app_config (key TEXT PRIMARY KEY, value TEXT);")
            .unwrap();
        write_config(
            &conn,
            &IgnoreConfig {
                prefixes: vec!["  ".to_string(), " /tmp/skip ".to_string(), "".to_string()],
            },
        )
        .unwrap();
        assert_eq!(prefixes(&conn), vec!["/tmp/skip".to_string()]);
    }

    #[test]
    fn an_absent_config_ignores_nothing() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE app_config (key TEXT PRIMARY KEY, value TEXT);")
            .unwrap();
        assert!(prefixes(&conn).is_empty());
    }
}
