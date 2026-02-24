use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};

use crate::error::{Result, XurlError};
use crate::model::{ProviderKind, ResolutionMeta, ResolvedThread};
use crate::provider::Provider;

#[derive(Debug, Clone)]
pub struct OpencodeProvider {
    root: PathBuf,
}

impl OpencodeProvider {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn db_path(&self) -> PathBuf {
        self.root.join("opencode.db")
    }

    fn materialized_path(session_id: &str) -> PathBuf {
        std::env::temp_dir()
            .join("xurl-opencode")
            .join(format!("{session_id}.jsonl"))
    }

    fn session_exists(
        conn: &Connection,
        session_id: &str,
    ) -> std::result::Result<bool, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT 1 FROM session WHERE id = ?1 LIMIT 1")?;
        let mut rows = stmt.query([session_id])?;
        Ok(rows.next()?.is_some())
    }

    fn fetch_messages(
        conn: &Connection,
        session_id: &str,
        warnings: &mut Vec<String>,
    ) -> std::result::Result<Vec<(String, Value)>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, data
             FROM message
             WHERE session_id = ?1
             ORDER BY time_created ASC, id ASC",
        )?;

        let rows = stmt.query_map([session_id], |row| {
            let id = row.get::<_, String>(0)?;
            let data = row.get::<_, String>(1)?;
            Ok((id, data))
        })?;

        let mut result = Vec::new();
        for row in rows {
            let (id, data) = row?;
            match serde_json::from_str::<Value>(&data) {
                Ok(value) => result.push((id, value)),
                Err(err) => warnings.push(format!(
                    "skipped message id={id}: invalid json payload ({err})"
                )),
            }
        }

        Ok(result)
    }

    fn fetch_parts(
        conn: &Connection,
        session_id: &str,
        warnings: &mut Vec<String>,
    ) -> std::result::Result<HashMap<String, Vec<Value>>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT message_id, data
             FROM part
             WHERE session_id = ?1
             ORDER BY time_created ASC, id ASC",
        )?;

        let rows = stmt.query_map([session_id], |row| {
            let message_id = row.get::<_, String>(0)?;
            let data = row.get::<_, String>(1)?;
            Ok((message_id, data))
        })?;

        let mut result = HashMap::new();
        for row in rows {
            let (message_id, data) = row?;
            match serde_json::from_str::<Value>(&data) {
                Ok(value) => {
                    result
                        .entry(message_id)
                        .or_insert_with(Vec::new)
                        .push(value);
                }
                Err(err) => warnings.push(format!(
                    "skipped part for message_id={message_id}: invalid json payload ({err})"
                )),
            }
        }

        Ok(result)
    }

    fn render_jsonl(
        session_id: &str,
        messages: Vec<(String, Value)>,
        mut parts: HashMap<String, Vec<Value>>,
    ) -> String {
        let mut lines = Vec::with_capacity(messages.len() + 1);
        lines.push(json!({
            "type": "session",
            "sessionId": session_id,
        }));

        for (id, message) in messages {
            lines.push(json!({
                "type": "message",
                "id": id,
                "sessionId": session_id,
                "message": message,
                "parts": parts.remove(&id).unwrap_or_default(),
            }));
        }

        let mut output = String::new();
        for line in lines {
            let encoded = serde_json::to_string(&line).expect("json serialization should succeed");
            output.push_str(&encoded);
            output.push('\n');
        }
        output
    }
}

impl Provider for OpencodeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Opencode
    }

    fn resolve(&self, session_id: &str) -> Result<ResolvedThread> {
        let db_path = self.db_path();
        if !db_path.exists() {
            return Err(XurlError::ThreadNotFound {
                provider: ProviderKind::Opencode.to_string(),
                session_id: session_id.to_string(),
                searched_roots: vec![db_path],
            });
        }

        let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|source| XurlError::Sqlite {
                path: db_path.clone(),
                source,
            })?;

        if !Self::session_exists(&conn, session_id).map_err(|source| XurlError::Sqlite {
            path: db_path.clone(),
            source,
        })? {
            return Err(XurlError::ThreadNotFound {
                provider: ProviderKind::Opencode.to_string(),
                session_id: session_id.to_string(),
                searched_roots: vec![db_path],
            });
        }

        let mut warnings = Vec::new();
        let messages =
            Self::fetch_messages(&conn, session_id, &mut warnings).map_err(|source| {
                XurlError::Sqlite {
                    path: db_path.clone(),
                    source,
                }
            })?;
        let parts = Self::fetch_parts(&conn, session_id, &mut warnings).map_err(|source| {
            XurlError::Sqlite {
                path: db_path.clone(),
                source,
            }
        })?;

        let raw = Self::render_jsonl(session_id, messages, parts);
        let path = Self::materialized_path(session_id);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| XurlError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        fs::write(&path, raw).map_err(|source| XurlError::Io {
            path: path.clone(),
            source,
        })?;

        Ok(ResolvedThread {
            provider: ProviderKind::Opencode,
            session_id: session_id.to_string(),
            path,
            metadata: ResolutionMeta {
                source: "opencode:sqlite".to_string(),
                candidate_count: 1,
                warnings,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use rusqlite::{Connection, params};
    use tempfile::tempdir;

    use crate::provider::Provider;
    use crate::provider::opencode::OpencodeProvider;

    fn prepare_db(path: &Path) -> Connection {
        let conn = Connection::open(path).expect("open sqlite");
        conn.execute_batch(
            "
            CREATE TABLE session (
                id TEXT PRIMARY KEY
            );
            CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE part (
                id TEXT PRIMARY KEY,
                message_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                data TEXT NOT NULL
            );
            ",
        )
        .expect("create schema");
        conn
    }

    #[test]
    fn resolves_from_sqlite_db() {
        let temp = tempdir().expect("tempdir");
        let db = temp.path().join("opencode.db");
        let conn = prepare_db(&db);

        let session_id = "ses_43a90e3adffejRgrTdlJa48CtE";
        conn.execute("INSERT INTO session (id) VALUES (?1)", [session_id])
            .expect("insert session");

        conn.execute(
            "INSERT INTO message (id, session_id, time_created, data) VALUES (?1, ?2, ?3, ?4)",
            params![
                "msg_1",
                session_id,
                1_i64,
                r#"{"role":"user","time":{"created":1}}"#
            ],
        )
        .expect("insert user");
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, data) VALUES (?1, ?2, ?3, ?4)",
            params![
                "msg_2",
                session_id,
                2_i64,
                r#"{"role":"assistant","time":{"created":2,"completed":3}}"#
            ],
        )
        .expect("insert assistant");

        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "prt_1",
                "msg_1",
                session_id,
                1_i64,
                r#"{"type":"text","text":"hello"}"#
            ],
        )
        .expect("insert user part");
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "prt_2",
                "msg_2",
                session_id,
                2_i64,
                r#"{"type":"text","text":"world"}"#
            ],
        )
        .expect("insert assistant part");

        let provider = OpencodeProvider::new(temp.path());
        let resolved = provider
            .resolve(session_id)
            .expect("resolve should succeed");

        assert_eq!(resolved.metadata.source, "opencode:sqlite");
        assert!(resolved.path.exists());

        let raw = fs::read_to_string(&resolved.path).expect("read materialized");
        assert!(raw.contains(r#""type":"session""#));
        assert!(raw.contains(r#""type":"message""#));
        assert!(raw.contains(r#""text":"hello""#));
        assert!(raw.contains(r#""text":"world""#));
    }

    #[test]
    fn returns_not_found_when_db_missing() {
        let temp = tempdir().expect("tempdir");
        let provider = OpencodeProvider::new(temp.path());
        let err = provider
            .resolve("ses_43a90e3adffejRgrTdlJa48CtE")
            .expect_err("must fail");
        assert!(format!("{err}").contains("thread not found"));
    }
}
