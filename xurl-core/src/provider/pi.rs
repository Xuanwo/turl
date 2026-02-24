use std::cmp::Reverse;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use walkdir::WalkDir;

use crate::error::{Result, XurlError};
use crate::model::{ProviderKind, ResolutionMeta, ResolvedThread};
use crate::provider::Provider;

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
