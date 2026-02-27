use std::cmp::Reverse;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use serde_json::Value;
use walkdir::WalkDir;

use crate::error::{Result, XurlError};
use crate::jsonl;
use crate::model::{ProviderKind, ResolutionMeta, ResolvedThread, WriteRequest, WriteResult};
use crate::provider::{Provider, WriteEventSink, append_passthrough_args};

#[derive(Debug, Clone)]
pub struct PiProvider {
    root: PathBuf,
}

impl PiProvider {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn sessions_root(&self) -> PathBuf {
        self.root.join("sessions")
    }

    fn has_session_id(path: &Path, session_id: &str) -> bool {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(_) => return false,
        };
        let reader = BufReader::new(file);

        let Some(first_non_empty) = reader
            .lines()
            .take(20)
            .filter_map(std::result::Result::ok)
            .find(|line| !line.trim().is_empty())
        else {
            return false;
        };

        let Ok(header) = serde_json::from_str::<Value>(&first_non_empty) else {
            return false;
        };

        header.get("type").and_then(Value::as_str) == Some("session")
            && header
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id.eq_ignore_ascii_case(session_id))
    }

    fn find_candidates(sessions_root: &Path, session_id: &str) -> Vec<PathBuf> {
        if !sessions_root.exists() {
            return Vec::new();
        }

        WalkDir::new(sessions_root)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.into_path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext == "jsonl")
            })
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

    fn pi_bin() -> String {
        std::env::var("XURL_PI_BIN").unwrap_or_else(|_| "pi".to_string())
    }

    fn spawn_pi_command(args: &[String]) -> Result<std::process::Child> {
        let bin = Self::pi_bin();
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

    fn extract_assistant_text(message: &Value) -> Option<String> {
        if message.get("role").and_then(Value::as_str) != Some("assistant") {
            return None;
        }

        if let Some(content) = message.get("content").and_then(Value::as_str) {
            if content.is_empty() {
                return None;
            }
            return Some(content.to_string());
        }

        let content = message.get("content")?.as_array()?;
        let text = content
            .iter()
            .filter_map(|item| {
                if item.get("type").and_then(Value::as_str) == Some("text") {
                    item.get("text").and_then(Value::as_str)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");
        if text.is_empty() { None } else { Some(text) }
    }

    fn run_write(
        &self,
        args: &[String],
        req: &WriteRequest,
        sink: &mut dyn WriteEventSink,
        warnings: Vec<String>,
    ) -> Result<WriteResult> {
        let mut child = Self::spawn_pi_command(args)?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| XurlError::WriteProtocol("pi stdout pipe is unavailable".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| XurlError::WriteProtocol("pi stderr pipe is unavailable".to_string()))?;
        let stderr_handle = std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut content = String::new();
            let _ = reader.read_to_string(&mut content);
            content
        });

        let mut session_id = req.session_id.clone();
        let mut final_text = None::<String>;
        let mut streamed_text = String::new();
        let mut streamed_delta = false;
        let stream_path = Path::new("<pi:stdout>");
        let reader = BufReader::new(stdout);
        jsonl::parse_jsonl_reader(stream_path, reader, |_, value| {
            let Some(event_type) = value.get("type").and_then(Value::as_str) else {
                return Ok(());
            };

            match event_type {
                "session" => {
                    if let Some(current_session_id) = value.get("id").and_then(Value::as_str)
                        && session_id.as_deref() != Some(current_session_id)
                    {
                        sink.on_session_ready(ProviderKind::Pi, current_session_id)?;
                        session_id = Some(current_session_id.to_string());
                    }
                }
                "message_update" => {
                    if value
                        .get("assistantMessageEvent")
                        .and_then(Value::as_object)
                        .and_then(|event| event.get("type"))
                        .and_then(Value::as_str)
                        == Some("text_delta")
                        && let Some(delta) = value
                            .get("assistantMessageEvent")
                            .and_then(Value::as_object)
                            .and_then(|event| event.get("delta"))
                            .and_then(Value::as_str)
                        && !delta.is_empty()
                    {
                        sink.on_text_delta(delta)?;
                        streamed_text.push_str(delta);
                        final_text = Some(streamed_text.clone());
                        streamed_delta = true;
                    }
                }
                "message_end" | "turn_end" => {
                    if streamed_delta {
                        return Ok(());
                    }
                    if let Some(text) = value
                        .get("message")
                        .and_then(Self::extract_assistant_text)
                        .filter(|text| !text.is_empty())
                    {
                        sink.on_text_delta(&text)?;
                        final_text = Some(text);
                    }
                }
                _ => {}
            }
            Ok(())
        })?;

        let status = child.wait().map_err(|source| XurlError::Io {
            path: PathBuf::from(Self::pi_bin()),
            source,
        })?;
        let stderr_content = stderr_handle.join().unwrap_or_default();
        if !status.success() {
            return Err(XurlError::CommandFailed {
                command: format!("{} {}", Self::pi_bin(), args.join(" ")),
                code: status.code(),
                stderr: stderr_content.trim().to_string(),
            });
        }

        let session_id = if let Some(session_id) = session_id {
            session_id
        } else {
            return Err(XurlError::WriteProtocol(
                "missing session id in pi event stream".to_string(),
            ));
        };

        Ok(WriteResult {
            provider: ProviderKind::Pi,
            session_id,
            final_text,
            warnings,
        })
    }
}

impl Provider for PiProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Pi
    }

    fn resolve(&self, session_id: &str) -> Result<ResolvedThread> {
        let sessions_root = self.sessions_root();
        let candidates = Self::find_candidates(&sessions_root, session_id);

        if let Some((selected, count)) = Self::choose_latest(candidates) {
            let mut metadata = ResolutionMeta {
                source: "pi:sessions".to_string(),
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
                provider: ProviderKind::Pi,
                session_id: session_id.to_string(),
                path: selected,
                metadata,
            });
        }

        Err(XurlError::ThreadNotFound {
            provider: ProviderKind::Pi.to_string(),
            session_id: session_id.to_string(),
            searched_roots: vec![sessions_root],
        })
    }

    fn write(&self, req: &WriteRequest, sink: &mut dyn WriteEventSink) -> Result<WriteResult> {
        if let Some(role) = req.options.role.as_deref() {
            return Err(XurlError::InvalidMode(format!(
                "provider `{}` does not support role-based write URI (`{role}`)",
                ProviderKind::Pi
            )));
        }
        let warnings = Vec::new();
        let mut args = Vec::new();
        if let Some(session_id) = req.session_id.as_deref() {
            let resolved = self.resolve(session_id)?;
            let session_path = resolved.path.to_string_lossy().to_string();
            args.push("--session".to_string());
            args.push(session_path);
            args.push("-p".to_string());
            args.push(req.prompt.clone());
            args.push("--mode".to_string());
            args.push("json".to_string());
        } else {
            args.push("-p".to_string());
            args.push(req.prompt.clone());
            args.push("--mode".to_string());
            args.push("json".to_string());
        }
        append_passthrough_args(&mut args, &req.options.params);
        self.run_write(&args, req, sink, warnings)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::thread;
    use std::time::Duration;

    use tempfile::tempdir;

    use crate::provider::Provider;
    use crate::provider::pi::PiProvider;

    fn write_session(root: &Path, session_dir: &str, file_name: &str, session_id: &str) -> PathBuf {
        let path = root.join("sessions").join(session_dir).join(file_name);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(
            &path,
            format!(
                "{{\"type\":\"session\",\"version\":3,\"id\":\"{session_id}\",\"timestamp\":\"2026-02-23T13:00:12.780Z\",\"cwd\":\"/tmp/project\"}}\n{{\"type\":\"message\",\"id\":\"a1b2c3d4\",\"parentId\":null,\"timestamp\":\"2026-02-23T13:00:13.000Z\",\"message\":{{\"role\":\"user\",\"content\":[{{\"type\":\"text\",\"text\":\"hello\"}}],\"timestamp\":1771851717843}}}}\n"
            ),
        )
        .expect("write");
        path
    }

    #[test]
    fn resolves_from_sessions_directory() {
        let temp = tempdir().expect("tempdir");
        let session_id = "12cb4c19-2774-4de4-a0d0-9fa32fbae29f";
        let path = write_session(
            temp.path(),
            "--Users-xuanwo-Code-xurl--",
            "2026-02-23T13-00-12-780Z_12cb4c19-2774-4de4-a0d0-9fa32fbae29f.jsonl",
            session_id,
        );

        let provider = PiProvider::new(temp.path());
        let resolved = provider
            .resolve(session_id)
            .expect("resolve should succeed");

        assert_eq!(resolved.path, path);
        assert_eq!(resolved.metadata.source, "pi:sessions");
    }

    #[test]
    fn selects_latest_when_multiple_matches_exist() {
        let temp = tempdir().expect("tempdir");
        let session_id = "12cb4c19-2774-4de4-a0d0-9fa32fbae29f";

        let first = write_session(
            temp.path(),
            "--Users-xuanwo-Code-project-a--",
            "2026-02-23T13-00-12-780Z_12cb4c19-2774-4de4-a0d0-9fa32fbae29f.jsonl",
            session_id,
        );
        thread::sleep(Duration::from_millis(15));
        let second = write_session(
            temp.path(),
            "--Users-xuanwo-Code-project-b--",
            "2026-02-23T13-10-12-780Z_12cb4c19-2774-4de4-a0d0-9fa32fbae29f.jsonl",
            session_id,
        );

        let provider = PiProvider::new(temp.path());
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
        let provider = PiProvider::new(temp.path());
        let err = provider
            .resolve("12cb4c19-2774-4de4-a0d0-9fa32fbae29f")
            .expect_err("must fail");
        assert!(format!("{err}").contains("thread not found"));
    }
}
