use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn resolve_kiro_data_path() -> PathBuf {
    if let Ok(override_dir) = std::env::var("KIRO_CLI_DATA_DIR") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("data.sqlite3");
        }
    }

    if cfg!(target_os = "macos") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("kiro-cli")
                .join("data.sqlite3");
        }
    }

    if cfg!(target_os = "windows") {
        if let Ok(app_data) = std::env::var("APPDATA") {
            return PathBuf::from(app_data)
                .join("kiro-cli")
                .join("data.sqlite3");
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("kiro-cli")
            .join("data.sqlite3");
    }

    PathBuf::from(".local/share/kiro-cli/data.sqlite3")
}

pub fn parse_session_id_from_stdout(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(id) = parse_uuid_after_marker(line, "Session ID:") {
            return Some(id);
        }
        if let Some(id) = parse_uuid_after_marker(line, "--resume-id") {
            return Some(id);
        }
    }
    None
}

pub fn list_conversation_ids_for_cwd(cwd: &Path, data_path: &Path) -> HashSet<String> {
    let keys = kiro_cwd_keys(cwd);
    if keys.is_empty() || !data_path.is_file() {
        return HashSet::new();
    }

    let Ok(conn) = rusqlite::Connection::open_with_flags(
        data_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) else {
        return HashSet::new();
    };

    let placeholders = keys.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("SELECT conversation_id FROM conversations_v2 WHERE key IN ({placeholders})");

    let mut stmt = match conn.prepare(&sql) {
        Ok(stmt) => stmt,
        Err(_) => return HashSet::new(),
    };

    let rows = match stmt.query_map(rusqlite::params_from_iter(keys.iter()), |row| {
        row.get::<_, String>(0)
    }) {
        Ok(rows) => rows,
        Err(_) => return HashSet::new(),
    };

    let mut ids = HashSet::new();
    for row in rows.flatten() {
        let trimmed = row.trim();
        if !trimmed.is_empty() {
            ids.insert(trimmed.to_string());
        }
    }
    ids
}

pub fn resolve_session_id_after_spawn(
    cwd: &Path,
    before_ids: &HashSet<String>,
    updated_after_ms: u64,
    data_path: &Path,
) -> Option<String> {
    let after_ids = list_conversation_ids_for_cwd(cwd, data_path);
    let novel: Vec<String> = after_ids
        .iter()
        .filter(|id| !before_ids.contains(*id))
        .cloned()
        .collect();

    match novel.len() {
        0 => newest_conversation_id(cwd, updated_after_ms, data_path),
        1 => novel.first().cloned(),
        _ => pick_newest_from_candidates(cwd, &novel, updated_after_ms, data_path)
            .or_else(|| novel.first().cloned()),
    }
}

pub fn emit_session_footer(session_id: &str) {
    eprintln!("[ai-e] session: {session_id}");
    eprintln!("[ai-e] resume: ai-e kiro --resume {session_id} \"your next prompt\"");
}

fn newest_conversation_id(cwd: &Path, updated_after_ms: u64, data_path: &Path) -> Option<String> {
    query_newest_conversation(cwd, None, updated_after_ms, data_path)
}

fn pick_newest_from_candidates(
    cwd: &Path,
    candidates: &[String],
    updated_after_ms: u64,
    data_path: &Path,
) -> Option<String> {
    query_newest_conversation(cwd, Some(candidates), updated_after_ms, data_path)
}

fn query_newest_conversation(
    cwd: &Path,
    candidates: Option<&[String]>,
    updated_after_ms: u64,
    data_path: &Path,
) -> Option<String> {
    let keys = kiro_cwd_keys(cwd);
    if keys.is_empty() || !data_path.is_file() {
        return None;
    }

    let Ok(conn) = rusqlite::Connection::open_with_flags(
        data_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) else {
        return None;
    };

    let key_ph = keys.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = if let Some(candidates) = candidates {
        if candidates.is_empty() {
            return None;
        }
        let id_ph = candidates.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        format!(
            "SELECT conversation_id FROM conversations_v2
             WHERE key IN ({key_ph}) AND conversation_id IN ({id_ph}) AND updated_at >= ?
             ORDER BY updated_at DESC LIMIT 1"
        )
    } else {
        format!(
            "SELECT conversation_id FROM conversations_v2
             WHERE key IN ({key_ph}) AND updated_at >= ?
             ORDER BY updated_at DESC LIMIT 1"
        )
    };

    let mut stmt = conn.prepare(&sql).ok()?;
    let mut idx = 1;
    for key in &keys {
        stmt.raw_bind_parameter(idx, key.as_str()).ok()?;
        idx += 1;
    }
    if let Some(candidates) = candidates {
        for candidate in candidates {
            stmt.raw_bind_parameter(idx, candidate.as_str()).ok()?;
            idx += 1;
        }
    }
    stmt.raw_bind_parameter(idx, i64::try_from(updated_after_ms).unwrap_or(0))
        .ok()?;

    let mut rows = stmt.raw_query();
    let row = rows.next().ok()??;
    let id = row.get::<_, String>(0).ok()?;
    let trimmed = id.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn kiro_cwd_keys(cwd: &Path) -> Vec<String> {
    let mut keys = HashSet::new();
    let raw = cwd.to_string_lossy().trim().to_string();
    if !raw.is_empty() {
        keys.insert(raw.clone());
        if let Ok(canonical) = std::fs::canonicalize(cwd) {
            keys.insert(canonical.to_string_lossy().to_string());
        }
    }
    keys.into_iter().collect()
}

fn parse_uuid_after_marker(line: &str, marker: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let marker_lower = marker.to_ascii_lowercase();
    let idx = lower.find(&marker_lower)?;
    let tail = line[idx + marker.len()..].trim();
    let token = tail
        .split_whitespace()
        .next()?
        .trim_matches(|c| c == '.' || c == ',');
    is_uuid(token).then(|| token.to_string())
}

fn is_uuid(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let lengths = [8, 4, 4, 4, 12];
    parts
        .iter()
        .zip(lengths)
        .all(|(part, len)| part.len() == len && part.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn parse_session_id_from_tui_stdout() {
        let raw = "Session ended.\nResume with: kiro-cli --resume-id 79eee8a5-7c00-4cd9-8385-c534a2f8b814\n";
        assert_eq!(
            parse_session_id_from_stdout(raw),
            Some("79eee8a5-7c00-4cd9-8385-c534a2f8b814".to_string())
        );
    }

    #[test]
    fn parse_session_id_from_session_id_line() {
        let raw = "● Session ID: 24b53e9c-e117-479e-8d9e-191688be7dd5\n";
        assert_eq!(
            parse_session_id_from_stdout(raw),
            Some("24b53e9c-e117-479e-8d9e-191688be7dd5".to_string())
        );
    }

    #[test]
    fn resolve_session_id_after_spawn_prefers_set_diff() {
        let dir = tempdir().unwrap();
        let cwd = dir.path().join("workspace");
        fs::create_dir_all(&cwd).unwrap();
        let data_path = dir.path().join("data.sqlite3");

        let conn = rusqlite::Connection::open(&data_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE conversations_v2 (
                key TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                value TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (key, conversation_id)
            );",
        )
        .unwrap();

        let cwd_key = cwd.canonicalize().unwrap().to_string_lossy().to_string();
        conn.execute(
            "INSERT INTO conversations_v2 (key, conversation_id, value, created_at, updated_at)
             VALUES (?1, ?2, '{}', 0, ?3)",
            rusqlite::params![cwd_key, "stale-latest", 1_700_000_000_000_i64],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversations_v2 (key, conversation_id, value, created_at, updated_at)
             VALUES (?1, ?2, '{}', 0, ?3)",
            rusqlite::params![cwd_key, "older-existing", 1_699_000_000_000_i64],
        )
        .unwrap();

        let before = list_conversation_ids_for_cwd(&cwd, &data_path);
        assert_eq!(before.len(), 2);

        conn.execute(
            "INSERT INTO conversations_v2 (key, conversation_id, value, created_at, updated_at)
             VALUES (?1, ?2, '{}', 0, ?3)",
            rusqlite::params![cwd_key, "brand-new", 1_700_500_000_000_i64],
        )
        .unwrap();

        let resolved = resolve_session_id_after_spawn(&cwd, &before, 0, &data_path);
        assert_eq!(resolved, Some("brand-new".to_string()));
    }
}
