use std::io::{BufReader, Read};
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::error::{Result, XurlError};
use crate::jsonl;
use crate::model::{ProviderKind, ResolutionMeta, ResolvedThread, WriteRequest, WriteResult};
use crate::provider::{Provider, WriteEventSink, append_passthrough_args};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AmpProvider {
    root: PathBuf,
}

impl AmpProvider {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn threads_root(&self) -> PathBuf {
        self.root.join("threads")
    }

    fn amp_bin() -> String {
        std::env::var("XURL_AMP_BIN").unwrap_or_else(|_| "amp".to_string())
    }

    fn spawn_amp_command(args: &[String]) -> Result<std::process::Child> {
        let bin = Self::amp_bin();
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
        let mut child = Self::spawn_amp_command(args)?;
        let stdout = child.stdout.take().ok_or_else(|| {
            XurlError::WriteProtocol("amp stdout pipe is unavailable".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            XurlError::WriteProtocol("amp stderr pipe is unavailable".to_string())
        })?;
        let stderr_handle = std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut content = String::new();
            let _ = reader.read_to_string(&mut content);
            content
        });

        let mut session_id = req.session_id.clone();
        let mut final_text = None::<String>;
        let mut stream_error = None::<String>;
        let stream_path = Path::new("<amp:stdout>");
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
                        sink.on_session_ready(ProviderKind::Amp, current_session_id)?;
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
                    let is_error = value.get("is_error").and_then(Value::as_bool) == Some(true)
                        || value.get("subtype").and_then(Value::as_str) == Some("error");
                    if is_error {
                        stream_error = value
                            .get("result")
                            .and_then(Value::as_str)
                            .or_else(|| {
                                value
                                    .get("error")
                                    .and_then(Value::as_object)
                                    .and_then(|error| error.get("message"))
                                    .and_then(Value::as_str)
                            })
                            .map(ToString::to_string);
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
            path: PathBuf::from(Self::amp_bin()),
            source,
        })?;
        let stderr_content = stderr_handle.join().unwrap_or_default();
        if !status.success() {
            return Err(XurlError::CommandFailed {
                command: format!("{} {}", Self::amp_bin(), args.join(" ")),
                code: status.code(),
                stderr: stderr_content.trim().to_string(),
            });
        }

        if let Some(stream_error) = stream_error {
            return Err(XurlError::WriteProtocol(format!(
                "amp stream returned an error: {stream_error}"
            )));
        }

        let session_id = if let Some(session_id) = session_id {
            session_id
        } else {
            return Err(XurlError::WriteProtocol(
                "missing session id in amp event stream".to_string(),
            ));
        };

        Ok(WriteResult {
            provider: ProviderKind::Amp,
            session_id,
            final_text,
            warnings,
        })
    }
}

impl Provider for AmpProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Amp
    }

    fn resolve(&self, session_id: &str) -> Result<ResolvedThread> {
        let threads_root = self.threads_root();
        let path = threads_root.join(format!("{session_id}.json"));

        if !path.exists() {
            return Err(XurlError::ThreadNotFound {
                provider: ProviderKind::Amp.to_string(),
                session_id: session_id.to_string(),
                searched_roots: vec![threads_root],
            });
        }

        Ok(ResolvedThread {
            provider: ProviderKind::Amp,
            session_id: session_id.to_string(),
            path,
            metadata: ResolutionMeta {
                source: "amp:threads".to_string(),
                candidate_count: 1,
                warnings: Vec::new(),
            },
        })
    }

    fn write(&self, req: &WriteRequest, sink: &mut dyn WriteEventSink) -> Result<WriteResult> {
        let warnings = Vec::new();
        let mut args = Vec::new();
        if let Some(session_id) = req.session_id.as_deref() {
            args.push("threads".to_string());
            args.push("continue".to_string());
            args.push(session_id.to_string());
            args.push("-x".to_string());
            args.push(req.prompt.clone());
            args.push("--stream-json".to_string());
        } else {
            args.push("-x".to_string());
            args.push(req.prompt.clone());
            args.push("--stream-json".to_string());
        }
        append_passthrough_args(&mut args, &req.options.params);
        self.run_write(&args, req, sink, warnings)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::provider::Provider;
    use crate::provider::amp::AmpProvider;

    #[test]
    fn resolves_from_threads_directory() {
        let temp = tempdir().expect("tempdir");
        let threads = temp.path().join("threads");
        fs::create_dir_all(&threads).expect("mkdir");
        let path = threads.join("T-019c0797-c402-7389-bd80-d785c98df295.json");
        fs::write(&path, "{\"messages\":[]}").expect("write");

        let provider = AmpProvider::new(temp.path());
        let resolved = provider
            .resolve("T-019c0797-c402-7389-bd80-d785c98df295")
            .expect("resolve should succeed");
        assert_eq!(resolved.path, path);
        assert_eq!(resolved.metadata.source, "amp:threads");
    }

    #[test]
    fn missing_thread_returns_not_found() {
        let temp = tempdir().expect("tempdir");
        let provider = AmpProvider::new(temp.path());
        let err = provider
            .resolve("T-019c0797-c402-7389-bd80-d785c98df295")
            .expect_err("must fail");
        assert!(format!("{err}").contains("thread not found"));
    }
}
