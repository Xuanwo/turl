use std::cmp::Reverse;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::Value;
use walkdir::WalkDir;

use crate::error::{Result, XurlError};
use crate::jsonl;
use crate::model::{ProviderKind, ResolutionMeta, ResolvedThread, WriteRequest, WriteResult};
use crate::provider::{Provider, WriteEventSink, append_passthrough_args};

#[derive(Debug, Deserialize)]
struct SessionsIndex {
    #[serde(default)]
    entries: Vec<SessionIndexEntry>,
}

#[derive(Debug, Deserialize)]
struct SessionIndexEntry {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "fullPath")]
    full_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ClaudeProvider {
    root: PathBuf,
}

impl ClaudeProvider {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn projects_root(&self) -> PathBuf {
        self.root.join("projects")
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

    fn find_from_sessions_index(projects_root: &Path, session_id: &str) -> Vec<PathBuf> {
        if !projects_root.exists() {
            return Vec::new();
        }

        WalkDir::new(projects_root)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| entry.file_name() == "sessions-index.json")
            .filter_map(|entry| fs::read_to_string(entry.path()).ok())
            .filter_map(|content| serde_json::from_str::<SessionsIndex>(&content).ok())
            .flat_map(|index| {
                index.entries.into_iter().filter_map(|entry| {
                    if entry.session_id == session_id {
                        entry.full_path
                    } else {
                        None
                    }
                })
            })
            .filter(|path| path.exists())
            .collect()
    }

    fn find_by_filename(projects_root: &Path, session_id: &str) -> Vec<PathBuf> {
        if !projects_root.exists() {
            return Vec::new();
        }

        let needle = format!("{session_id}.jsonl");
        WalkDir::new(projects_root)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.into_path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == needle)
            })
            .collect()
    }

    fn file_contains_session_id(path: &Path, session_id: &str) -> bool {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(_) => return false,
        };
        let reader = BufReader::new(file);

        for line in reader.lines().take(30).flatten() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<Value>(&line)
                && value
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id == session_id)
            {
                return true;
            }
        }

        false
    }

    fn find_by_header_scan(projects_root: &Path, session_id: &str) -> Vec<PathBuf> {
        if !projects_root.exists() {
            return Vec::new();
        }

        WalkDir::new(projects_root)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.into_path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext == "jsonl")
            })
            .filter(|path| Self::file_contains_session_id(path, session_id))
            .collect()
    }

    fn make_resolved(
        session_id: &str,
        selected: PathBuf,
        count: usize,
        source: &str,
    ) -> ResolvedThread {
        let mut metadata = ResolutionMeta {
            source: source.to_string(),
            candidate_count: count,
            warnings: Vec::new(),
        };

        if count > 1 {
            metadata.warnings.push(format!(
                "multiple matches found ({count}) for session_id={session_id}; selected latest: {}",
                selected.display()
            ));
        }

        ResolvedThread {
            provider: ProviderKind::Claude,
            session_id: session_id.to_string(),
            path: selected,
            metadata,
        }
    }

    fn claude_bin() -> String {
        std::env::var("XURL_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string())
    }

    fn spawn_claude_command(
        args: &[String],
        workdir: Option<&Path>,
    ) -> Result<std::process::Child> {
        let bin = Self::claude_bin();
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

    fn extract_assistant_text(value: &Value) -> Option<String> {
        let message = value.get("message")?;
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
        let mut child = Self::spawn_claude_command(args, req.options.workdir.as_deref())?;
        let stdout = child.stdout.take().ok_or_else(|| {
            XurlError::WriteProtocol("claude stdout pipe is unavailable".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            XurlError::WriteProtocol("claude stderr pipe is unavailable".to_string())
        })?;
        let stderr_handle = std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut content = String::new();
            let _ = reader.read_to_string(&mut content);
            content
        });

        let mut session_id = req.session_id.clone();
        let mut final_text = None::<String>;
        let stream_path = Path::new("<claude:stdout>");
        let reader = BufReader::new(stdout);
        jsonl::parse_jsonl_reader(stream_path, reader, |_, value| {
            let Some(event_type) = value.get("type").and_then(Value::as_str) else {
                return Ok(());
            };

            match event_type {
                "system" => {
                    if value.get("subtype").and_then(Value::as_str) == Some("init")
                        && let Some(current_session_id) =
                            value.get("session_id").and_then(Value::as_str)
                    {
                        sink.on_session_ready(ProviderKind::Claude, current_session_id)?;
                        session_id = Some(current_session_id.to_string());
                    }
                }
                "assistant" => {
                    if let Some(text) = Self::extract_assistant_text(&value) {
                        sink.on_text_delta(&text)?;
                        final_text = Some(text);
                    }
                    if let Some(current_session_id) =
                        value.get("session_id").and_then(Value::as_str)
                    {
                        session_id = Some(current_session_id.to_string());
                    }
                }
                "result" => {
                    if let Some(current_session_id) =
                        value.get("session_id").and_then(Value::as_str)
                    {
                        session_id = Some(current_session_id.to_string());
                    }
                    if final_text.is_none()
                        && let Some(text) = value.get("result").and_then(Value::as_str)
                        && !text.is_empty()
                    {
                        sink.on_text_delta(text)?;
                        final_text = Some(text.to_string());
                    }
                }
                _ => {}
            }
            Ok(())
        })?;

        let status = child.wait().map_err(|source| XurlError::Io {
            path: PathBuf::from(Self::claude_bin()),
            source,
        })?;
        let stderr_content = stderr_handle.join().unwrap_or_default();

        if !status.success() {
            return Err(XurlError::CommandFailed {
                command: format!("{} {}", Self::claude_bin(), args.join(" ")),
                code: status.code(),
                stderr: stderr_content.trim().to_string(),
            });
        }

        let session_id = if let Some(session_id) = session_id {
            session_id
        } else {
            return Err(XurlError::WriteProtocol(
                "missing session id in claude event stream".to_string(),
            ));
        };

        Ok(WriteResult {
            provider: ProviderKind::Claude,
            session_id,
            final_text,
            warnings,
        })
    }
}

impl Provider for ClaudeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Claude
    }

    fn resolve(&self, session_id: &str) -> Result<ResolvedThread> {
        let projects = self.projects_root();

        let index_hits = Self::find_from_sessions_index(&projects, session_id);
        if let Some((selected, count)) = Self::choose_latest(index_hits) {
            return Ok(Self::make_resolved(
                session_id,
                selected,
                count,
                "claude:sessions-index",
            ));
        }

        let filename_hits = Self::find_by_filename(&projects, session_id);
        if let Some((selected, count)) = Self::choose_latest(filename_hits) {
            return Ok(Self::make_resolved(
                session_id,
                selected,
                count,
                "claude:filename",
            ));
        }

        let scanned_hits = Self::find_by_header_scan(&projects, session_id);
        if let Some((selected, count)) = Self::choose_latest(scanned_hits) {
            return Ok(Self::make_resolved(
                session_id,
                selected,
                count,
                "claude:header-scan",
            ));
        }

        Err(XurlError::ThreadNotFound {
            provider: ProviderKind::Claude.to_string(),
            session_id: session_id.to_string(),
            searched_roots: vec![projects],
        })
    }

    fn write(&self, req: &WriteRequest, sink: &mut dyn WriteEventSink) -> Result<WriteResult> {
        let mut warnings = Vec::new();
        let mut args = vec![
            "-p".to_string(),
            "--verbose".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
        ];
        for dir in &req.options.add_dirs {
            args.push("--add-dir".to_string());
            args.push(dir.display().to_string());
        }
        append_passthrough_args(
            &mut args,
            &req.options.passthrough,
            &[
                "workdir",
                "add_dir",
                "output-format",
                "print",
                "p",
                "resume",
                "continue",
                "session-id",
                "add-dir",
            ],
            &mut warnings,
        );
        if let Some(session_id) = req.session_id.as_deref() {
            args.push("--resume".to_string());
            args.push(session_id.to_string());
            args.push(req.prompt.clone());
            self.run_write(&args, req, sink, warnings)
        } else {
            args.push(req.prompt.clone());
            self.run_write(&args, req, sink, warnings)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::provider::Provider;
    use crate::provider::claude::ClaudeProvider;

    #[test]
    fn resolves_from_sessions_index() {
        let temp = tempdir().expect("tempdir");
        let projects = temp.path().join("projects/project-a");
        fs::create_dir_all(&projects).expect("mkdir");
        let thread_file = projects.join("2823d1df-720a-4c31-ac55-ae8ba726721f.jsonl");
        fs::write(&thread_file, "{}\n").expect("write thread");

        let index = projects.join("sessions-index.json");
        fs::write(
            &index,
            format!(
                "{{\"entries\":[{{\"sessionId\":\"2823d1df-720a-4c31-ac55-ae8ba726721f\",\"fullPath\":\"{}\"}}]}}",
                thread_file.display()
            ),
        )
        .expect("write index");

        let provider = ClaudeProvider::new(temp.path());
        let resolved = provider
            .resolve("2823d1df-720a-4c31-ac55-ae8ba726721f")
            .expect("resolve should succeed");
        assert_eq!(resolved.path, thread_file);
        assert_eq!(resolved.metadata.source, "claude:sessions-index");
    }

    #[test]
    fn resolves_from_filename_when_index_misses() {
        let temp = tempdir().expect("tempdir");
        let projects = temp.path().join("projects/project-b");
        fs::create_dir_all(&projects).expect("mkdir");

        let thread_file = projects.join("8c06e0f0-2978-48ac-bb42-90d13e3b0470.jsonl");
        fs::write(&thread_file, "{}\n").expect("write thread");

        let provider = ClaudeProvider::new(temp.path());
        let resolved = provider
            .resolve("8c06e0f0-2978-48ac-bb42-90d13e3b0470")
            .expect("resolve should succeed");
        assert_eq!(resolved.path, thread_file);
        assert_eq!(resolved.metadata.source, "claude:filename");
    }

    #[test]
    fn resolves_from_header_scan() {
        let temp = tempdir().expect("tempdir");
        let projects = temp.path().join("projects/project-c");
        fs::create_dir_all(&projects).expect("mkdir");

        let thread_file = projects.join("renamed.jsonl");
        fs::write(
            &thread_file,
            "{\"type\":\"user\",\"sessionId\":\"1bd3c108-41b8-4291-93e8-8a472ab09de8\"}\n",
        )
        .expect("write thread");

        let provider = ClaudeProvider::new(temp.path());
        let resolved = provider
            .resolve("1bd3c108-41b8-4291-93e8-8a472ab09de8")
            .expect("resolve should succeed");
        assert_eq!(resolved.path, thread_file);
        assert_eq!(resolved.metadata.source, "claude:header-scan");
    }
}
