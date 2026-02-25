use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};

use crate::error::{Result, XurlError};
use crate::model::{ProviderKind, ResolutionMeta, ResolvedThread, WriteRequest, WriteResult};
use crate::provider::{Provider, WriteEventSink, append_passthrough_args};

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

    fn materialized_path(&self, session_id: &str) -> PathBuf {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.root.hash(&mut hasher);
        let root_key = format!("{:016x}", hasher.finish());

        std::env::temp_dir()
            .join("xurl-opencode")
            .join(root_key)
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

    fn opencode_bin() -> String {
        std::env::var("XURL_OPENCODE_BIN").unwrap_or_else(|_| "opencode".to_string())
    }

    fn spawn_opencode_command(
        args: &[String],
        workdir: Option<&std::path::Path>,
    ) -> Result<std::process::Child> {
        let bin = Self::opencode_bin();
        let mut command = Command::new(&bin);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(workdir) = workdir {
            command.current_dir(workdir);
        }
        command.spawn().map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                XurlError::CommandNotFound { command: bin }
            } else {
                XurlError::Io {
                    path: PathBuf::from(bin),
                    source,
                }
            }
        })
    }

    fn collect_text(value: Option<&Value>) -> String {
        match value {
            Some(Value::String(text)) => text.to_string(),
            Some(Value::Array(items)) => items
                .iter()
                .map(|item| Self::collect_text(Some(item)))
                .collect::<Vec<_>>()
                .join(""),
            Some(Value::Object(map)) => {
                if map.get("type").and_then(Value::as_str) == Some("text")
                    && let Some(text) = map.get("text").and_then(Value::as_str)
                {
                    return text.to_string();
                }

                if let Some(text) = map.get("text").and_then(Value::as_str) {
                    return text.to_string();
                }

                if let Some(content) = map.get("content") {
                    return Self::collect_text(Some(content));
                }

                String::new()
            }
            _ => String::new(),
        }
    }

    fn extract_session_id(value: &Value) -> Option<&str> {
        value
            .get("sessionID")
            .and_then(Value::as_str)
            .or_else(|| value.get("sessionId").and_then(Value::as_str))
    }

    fn extract_delta_text(value: &Value) -> Option<String> {
        value
            .get("delta")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(ToString::to_string)
            .or_else(|| {
                value
                    .get("textDelta")
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty())
                    .map(ToString::to_string)
            })
            .or_else(|| {
                value
                    .get("message")
                    .and_then(Value::as_object)
                    .and_then(|message| message.get("delta"))
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty())
                    .map(ToString::to_string)
            })
    }

    fn extract_assistant_text(value: &Value) -> Option<String> {
        if value.get("role").and_then(Value::as_str) == Some("assistant") {
            let text = Self::collect_text(value.get("content"));
            if !text.is_empty() {
                return Some(text);
            }
        }

        if let Some(message) = value.get("message")
            && message.get("role").and_then(Value::as_str) == Some("assistant")
        {
            let text = Self::collect_text(message.get("content"));
            if !text.is_empty() {
                return Some(text);
            }
        }

        value
            .get("response")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(ToString::to_string)
    }

    fn run_write(
        &self,
        args: &[String],
        req: &WriteRequest,
        sink: &mut dyn WriteEventSink,
        warnings: Vec<String>,
    ) -> Result<WriteResult> {
        let mut child = Self::spawn_opencode_command(args, req.options.workdir.as_deref())?;
        let stdout = child.stdout.take().ok_or_else(|| {
            XurlError::WriteProtocol("opencode stdout pipe is unavailable".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            XurlError::WriteProtocol("opencode stderr pipe is unavailable".to_string())
        })?;
        let stderr_handle = std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut content = String::new();
            let _ = reader.read_to_string(&mut content);
            content
        });

        let stream_path = PathBuf::from("<opencode:stdout>");
        let mut session_id = req.session_id.clone();
        let mut final_text = None::<String>;
        let mut streamed_text = String::new();
        let mut streamed_delta = false;
        let mut stream_error = None::<String>;
        let mut saw_json_event = false;
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = line.map_err(|source| XurlError::Io {
                path: stream_path.clone(),
                source,
            })?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
                continue;
            };
            saw_json_event = true;

            if let Some(current_session_id) = Self::extract_session_id(&value)
                && session_id.as_deref() != Some(current_session_id)
            {
                sink.on_session_ready(ProviderKind::Opencode, current_session_id)?;
                session_id = Some(current_session_id.to_string());
            }

            if value.get("type").and_then(Value::as_str) == Some("error") {
                stream_error = value
                    .get("error")
                    .and_then(Value::as_object)
                    .and_then(|error| {
                        error
                            .get("data")
                            .and_then(Value::as_object)
                            .and_then(|data| data.get("message"))
                            .and_then(Value::as_str)
                            .or_else(|| error.get("message").and_then(Value::as_str))
                    })
                    .or_else(|| value.get("message").and_then(Value::as_str))
                    .map(ToString::to_string);
                continue;
            }

            if let Some(delta) = Self::extract_delta_text(&value) {
                sink.on_text_delta(&delta)?;
                streamed_text.push_str(&delta);
                final_text = Some(streamed_text.clone());
                streamed_delta = true;
                continue;
            }

            if !streamed_delta && let Some(text) = Self::extract_assistant_text(&value) {
                sink.on_text_delta(&text)?;
                final_text = Some(text);
            }
        }

        let status = child.wait().map_err(|source| XurlError::Io {
            path: PathBuf::from(Self::opencode_bin()),
            source,
        })?;
        let stderr_content = stderr_handle.join().unwrap_or_default();
        if !status.success() {
            return Err(XurlError::CommandFailed {
                command: format!("{} {}", Self::opencode_bin(), args.join(" ")),
                code: status.code(),
                stderr: stderr_content.trim().to_string(),
            });
        }

        if !saw_json_event {
            return Err(XurlError::WriteProtocol(
                "opencode output does not contain JSON events".to_string(),
            ));
        }

        if let Some(stream_error) = stream_error {
            return Err(XurlError::WriteProtocol(format!(
                "opencode stream returned an error: {stream_error}"
            )));
        }

        let session_id = if let Some(session_id) = session_id {
            session_id
        } else {
            return Err(XurlError::WriteProtocol(
                "missing session id in opencode event stream".to_string(),
            ));
        };

        Ok(WriteResult {
            provider: ProviderKind::Opencode,
            session_id,
            final_text,
            warnings,
        })
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
        let path = self.materialized_path(session_id);

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

    fn write(&self, req: &WriteRequest, sink: &mut dyn WriteEventSink) -> Result<WriteResult> {
        let mut warnings = Vec::new();
        let mut args = vec!["run".to_string(), req.prompt.clone()];
        if let Some(workdir) = req.options.workdir.as_ref() {
            args.push("--dir".to_string());
            args.push(workdir.display().to_string());
        }
        if !req.options.add_dirs.is_empty() {
            warnings.push(
                "ignored query parameter `add_dir`: OpenCode CLI has no compatible option"
                    .to_string(),
            );
        }
        if let Some(session_id) = req.session_id.as_deref() {
            args.push("--session".to_string());
            args.push(session_id.to_string());
        } else {
            // keep create flow without session binding
        }
        args.push("--format".to_string());
        args.push("json".to_string());
        append_passthrough_args(
            &mut args,
            &req.options.passthrough,
            &[
                "workdir", "add_dir", "format", "session", "continue", "resume", "dir",
            ],
            &mut warnings,
        );
        self.run_write(&args, req, sink, warnings)
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

    #[test]
    fn materialized_paths_are_isolated_by_root() {
        let first_root = tempdir().expect("first tempdir");
        let second_root = tempdir().expect("second tempdir");
        let first = OpencodeProvider::new(first_root.path());
        let second = OpencodeProvider::new(second_root.path());
        let session_id = "ses_43a90e3adffejRgrTdlJa48CtE";

        let first_path = first.materialized_path(session_id);
        let second_path = second.materialized_path(session_id);

        assert_ne!(first_path, second_path);
    }
}
