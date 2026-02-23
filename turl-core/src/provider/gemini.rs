use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use walkdir::WalkDir;

use crate::error::{Result, TurlError};
use crate::model::{ProviderKind, ResolutionMeta, ResolvedThread};
use crate::provider::Provider;

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
}

impl Provider for GeminiProvider {
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

        Err(TurlError::ThreadNotFound {
            provider: ProviderKind::Gemini.to_string(),
            session_id: session_id.to_string(),
            searched_roots: vec![tmp_root],
        })
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
