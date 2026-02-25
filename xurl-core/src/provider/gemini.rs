use std::cmp::Reverse;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use serde_json::Value;
use walkdir::WalkDir;

use crate::error::{Result, XurlError};
use crate::model::{ProviderKind, ResolutionMeta, ResolvedThread, WriteRequest, WriteResult};
use crate::provider::{Provider, WriteEventSink, append_passthrough_args};

#[derive(Debug, Clone)]
pub struct GeminiProvider {
    root: PathBuf,
}

impl GeminiProvider {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn tmp_root(&self) -> PathBuf {
        self.root.join("tmp")
    }

    fn is_session_file(path: &Path) -> bool {
        let is_session_file = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("session-") && name.ends_with(".json"));

        let is_chats_entry = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "chats");

        is_session_file && is_chats_entry
    }

    fn has_session_id(path: &Path, session_id: &str) -> bool {
        let Ok(raw) = fs::read_to_string(path) else {
            return false;
        };

        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            return false;
        };

        value
            .get("sessionId")
            .and_then(Value::as_str)
            .is_some_and(|id| id.eq_ignore_ascii_case(session_id))
    }

    fn find_candidates(tmp_root: &Path, session_id: &str) -> Vec<PathBuf> {
        if !tmp_root.exists() {
            return Vec::new();
        }

        WalkDir::new(tmp_root)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.into_path())
            .filter(|path| Self::is_session_file(path))
            .filter(|path| Self::has_session_id(path, session_id))
            .collect()
    }

    fn choose_latest(paths: Vec<PathBuf>) -> Option<(PathBuf, usize)> {
        if paths.is_empty() {
            return None;
        }

        let mut scored = paths
            .into_iter()
            .map(|path| {
                let modified = fs::metadata(&path)
                    .and_then(|meta| meta.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                (path, modified)
            })
            .collect::<Vec<_>>();

        scored.sort_by_key(|(_, modified)| Reverse(*modified));
        let count = scored.len();
        scored.into_iter().next().map(|(path, _)| (path, count))
    }

    fn gemini_bin() -> String {
        std::env::var("XURL_GEMINI_BIN").unwrap_or_else(|_| "gemini".to_string())
    }

    fn spawn_gemini_command(args: &[String]) -> Result<std::process::Child> {
        let bin = Self::gemini_bin();
        let mut command = Command::new(&bin);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
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

    fn run_write(
        &self,
        args: &[String],
        req: &WriteRequest,
        sink: &mut dyn WriteEventSink,
        warnings: Vec<String>,
    ) -> Result<WriteResult> {
        let mut child = Self::spawn_gemini_command(args)?;
        let stdout = child.stdout.take().ok_or_else(|| {
            XurlError::WriteProtocol("gemini stdout pipe is unavailable".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            XurlError::WriteProtocol("gemini stderr pipe is unavailable".to_string())
        })?;
        let stderr_handle = std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut content = String::new();
            let _ = reader.read_to_string(&mut content);
            content
        });

        let stream_path = Path::new("<gemini:stdout>");
        let mut session_id = req.session_id.clone();
        let mut final_text = None::<String>;
        let mut streamed_text = String::new();
        let mut stream_error = None::<String>;
        let mut saw_json_event = false;
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = line.map_err(|source| XurlError::Io {
                path: stream_path.to_path_buf(),
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

            if let Some(current_session_id) = value.get("session_id").and_then(Value::as_str)
                && session_id.as_deref() != Some(current_session_id)
            {
                sink.on_session_ready(ProviderKind::Gemini, current_session_id)?;
                session_id = Some(current_session_id.to_string());
            }

            if value.get("type").and_then(Value::as_str) == Some("message")
                && value.get("role").and_then(Value::as_str) == Some("assistant")
                && let Some(text) = value.get("content").and_then(Value::as_str)
                && !text.is_empty()
            {
                sink.on_text_delta(text)?;
                if value.get("delta").and_then(Value::as_bool) == Some(true) {
                    streamed_text.push_str(text);
                    final_text = Some(streamed_text.clone());
                } else {
                    final_text = Some(text.to_string());
                }
            }

            if value.get("type").and_then(Value::as_str) == Some("result")
                && value.get("status").and_then(Value::as_str) != Some("success")
            {
                stream_error = value
                    .get("error")
                    .and_then(Value::as_str)
                    .or_else(|| value.get("message").and_then(Value::as_str))
                    .or_else(|| value.get("status").and_then(Value::as_str))
                    .map(ToString::to_string);
            }

            if final_text.is_none()
                && let Some(text) = value.get("response").and_then(Value::as_str)
                && !text.is_empty()
            {
                sink.on_text_delta(text)?;
                final_text = Some(text.to_string());
            }
        }

        let status = child.wait().map_err(|source| XurlError::Io {
            path: PathBuf::from(Self::gemini_bin()),
            source,
        })?;
        let stderr_content = stderr_handle.join().unwrap_or_default();
        if !status.success() {
            return Err(XurlError::CommandFailed {
                command: format!("{} {}", Self::gemini_bin(), args.join(" ")),
                code: status.code(),
                stderr: stderr_content.trim().to_string(),
            });
        }

        if !saw_json_event {
            return Err(XurlError::WriteProtocol(
                "gemini output does not contain JSON events".to_string(),
            ));
        }

        if let Some(stream_error) = stream_error {
            return Err(XurlError::WriteProtocol(format!(
                "gemini stream returned an error: {stream_error}"
            )));
        }

        let session_id = if let Some(session_id) = session_id {
            session_id
        } else {
            return Err(XurlError::WriteProtocol(
                "missing session id in gemini event stream".to_string(),
            ));
        };

        Ok(WriteResult {
            provider: ProviderKind::Gemini,
            session_id,
            final_text,
            warnings,
        })
    }
}

impl Provider for GeminiProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Gemini
    }

    fn resolve(&self, session_id: &str) -> Result<ResolvedThread> {
        let tmp_root = self.tmp_root();
        let candidates = Self::find_candidates(&tmp_root, session_id);

        if let Some((selected, count)) = Self::choose_latest(candidates) {
            let mut metadata = ResolutionMeta {
                source: "gemini:chats".to_string(),
                candidate_count: count,
                warnings: Vec::new(),
            };

            if count > 1 {
                metadata.warnings.push(format!(
                    "multiple matches found ({count}) for session_id={session_id}; selected latest: {}",
                    selected.display()
                ));
            }

            return Ok(ResolvedThread {
                provider: ProviderKind::Gemini,
                session_id: session_id.to_string(),
                path: selected,
                metadata,
            });
        }

        Err(XurlError::ThreadNotFound {
            provider: ProviderKind::Gemini.to_string(),
            session_id: session_id.to_string(),
            searched_roots: vec![tmp_root],
        })
    }

    fn write(&self, req: &WriteRequest, sink: &mut dyn WriteEventSink) -> Result<WriteResult> {
        let warnings = Vec::new();
        let mut args = vec![
            "-p".to_string(),
            req.prompt.clone(),
            "--output-format".to_string(),
            "stream-json".to_string(),
        ];
        append_passthrough_args(&mut args, &req.options.params);
        if let Some(session_id) = req.session_id.as_deref() {
            args.push("--resume".to_string());
            args.push(session_id.to_string());
            self.run_write(&args, req, sink, warnings)
        } else {
            self.run_write(&args, req, sink, warnings)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::thread;
    use std::time::Duration;

    use tempfile::tempdir;

    use crate::provider::Provider;
    use crate::provider::gemini::GeminiProvider;

    fn write_session(
        root: &Path,
        project_hash: &str,
        file_name: &str,
        session_id: &str,
        user_text: &str,
    ) -> PathBuf {
        let path = root
            .join("tmp")
            .join(project_hash)
            .join("chats")
            .join(file_name);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");

        let content = format!(
            r#"{{
  "sessionId": "{session_id}",
  "projectHash": "{project_hash}",
  "startTime": "2026-01-08T11:55:12.379Z",
  "lastUpdated": "2026-01-08T12:31:14.881Z",
  "messages": [
    {{ "type": "user", "content": "{user_text}" }},
    {{ "type": "gemini", "content": "done" }}
  ]
}}"#,
        );
        fs::write(&path, content).expect("write");
        path
    }

    use std::path::{Path, PathBuf};

    #[test]
    fn resolves_from_gemini_tmp_chats() {
        let temp = tempdir().expect("tempdir");
        let path = write_session(
            temp.path(),
            "0c0d7b04c22749f3687ea60b66949fd32bcea2551d4349bf72346a9ccc9a9ba4",
            "session-2026-01-08T11-55-29-29d207db.json",
            "29d207db-ca7e-40ba-87f7-e14c9de60613",
            "hello",
        );

        let provider = GeminiProvider::new(temp.path());
        let resolved = provider
            .resolve("29d207db-ca7e-40ba-87f7-e14c9de60613")
            .expect("resolve should succeed");
        assert_eq!(resolved.path, path);
        assert_eq!(resolved.metadata.source, "gemini:chats");
    }

    #[test]
    fn selects_latest_when_multiple_matches_exist() {
        let temp = tempdir().expect("tempdir");
        let session_id = "29d207db-ca7e-40ba-87f7-e14c9de60613";

        let first = write_session(
            temp.path(),
            "hash-a",
            "session-2026-01-08T11-55-29-29d207db.json",
            session_id,
            "first",
        );

        thread::sleep(Duration::from_millis(15));

        let second = write_session(
            temp.path(),
            "hash-b",
            "session-2026-01-08T12-00-00-29d207db.json",
            session_id,
            "second",
        );

        let provider = GeminiProvider::new(temp.path());
        let resolved = provider
            .resolve(session_id)
            .expect("resolve should succeed");
        assert_eq!(resolved.path, second);
        assert_eq!(resolved.metadata.candidate_count, 2);
        assert_eq!(resolved.metadata.warnings.len(), 1);
        assert!(resolved.metadata.warnings[0].contains("multiple matches"));

        assert!(first.exists());
    }

    #[test]
    fn missing_thread_returns_not_found() {
        let temp = tempdir().expect("tempdir");
        let provider = GeminiProvider::new(temp.path());
        let err = provider
            .resolve("29d207db-ca7e-40ba-87f7-e14c9de60613")
            .expect_err("must fail");
        assert!(format!("{err}").contains("thread not found"));
    }
}
