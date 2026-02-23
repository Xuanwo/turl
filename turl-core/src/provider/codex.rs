use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use walkdir::WalkDir;

use crate::error::{Result, TurlError};
use crate::model::{ProviderKind, ResolutionMeta, ResolvedThread};
use crate::provider::Provider;

#[derive(Debug, Clone)]
pub struct CodexProvider {
    root: PathBuf,
}

#[derive(Debug, Clone)]
struct SqliteThreadRecord {
    rollout_path: PathBuf,
    archived: bool,
}

impl CodexProvider {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn sessions_root(&self) -> PathBuf {
        self.root.join("sessions")
    }

    fn archived_root(&self) -> PathBuf {
        self.root.join("archived_sessions")
    }

    fn state_db_paths(&self) -> Vec<PathBuf> {
        let mut paths = if let Ok(entries) = fs::read_dir(&self.root) {
            entries
                .filter_map(std::result::Result::ok)
                .filter_map(|entry| {
                    let path = entry.path();
                    let name = path.file_name()?.to_str()?;
                    let is_state_db = name == "state.sqlite"
                        || (name.starts_with("state_") && name.ends_with(".sqlite"));
                    if is_state_db && path.is_file() {
                        Some(path)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        paths.sort_by_key(|path| {
            let version = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| {
                    name.strip_prefix("state_")
                        .and_then(|name| name.strip_suffix(".sqlite"))
                })
                .and_then(|raw| raw.parse::<u32>().ok())
                .unwrap_or(0);
            let modified = fs::metadata(path)
                .and_then(|meta| meta.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (Reverse(version), Reverse(modified))
        });

        paths
    }

    fn query_thread_record(
        db_path: &Path,
        session_id: &str,
    ) -> std::result::Result<Option<SqliteThreadRecord>, rusqlite::Error> {
        let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut stmt =
            conn.prepare("SELECT rollout_path, archived FROM threads WHERE id = ?1 LIMIT 1")?;
        let row = stmt
            .query_row([session_id], |row| {
                Ok(SqliteThreadRecord {
                    rollout_path: PathBuf::from(row.get::<_, String>(0)?),
                    archived: row.get::<_, i64>(1)? != 0,
                })
            })
            .optional()?;
        Ok(row)
    }

    fn lookup_thread_from_state_db(
        state_dbs: &[PathBuf],
        session_id: &str,
        warnings: &mut Vec<String>,
    ) -> Option<SqliteThreadRecord> {
        for db_path in state_dbs {
            match Self::query_thread_record(db_path, session_id) {
                Ok(Some(record)) => return Some(record),
                Ok(None) => continue,
                Err(err) => warnings.push(format!(
                    "failed reading sqlite thread index {}: {err}",
                    db_path.display()
                )),
            }
        }

        None
    }

    fn find_candidates(root: &Path, session_id: &str) -> Vec<PathBuf> {
        let needle = format!("{session_id}.jsonl");
        if !root.exists() {
            return Vec::new();
        }

        WalkDir::new(root)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.into_path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(&needle))
            })
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

impl Provider for CodexProvider {
    fn resolve(&self, session_id: &str) -> Result<ResolvedThread> {
        let sessions = self.sessions_root();
        let archived = self.archived_root();
        let state_dbs = self.state_db_paths();
        let mut warnings = Vec::new();
        let sqlite_record =
            Self::lookup_thread_from_state_db(&state_dbs, session_id, &mut warnings);

        if let Some(record) = sqlite_record.as_ref().filter(|record| !record.archived) {
            if record.rollout_path.exists() {
                return Ok(ResolvedThread {
                    provider: ProviderKind::Codex,
                    session_id: session_id.to_string(),
                    path: record.rollout_path.clone(),
                    metadata: ResolutionMeta {
                        source: "codex:sqlite:sessions".to_string(),
                        candidate_count: 1,
                        warnings,
                    },
                });
            }

            warnings.push(format!(
                "sqlite thread index points to a missing rollout for session_id={session_id}: {}",
                record.rollout_path.display()
            ));
        }

        let active_candidates = Self::find_candidates(&sessions, session_id);
        if let Some((selected, count)) = Self::choose_latest(active_candidates) {
            if count > 1 {
                warnings.push(format!(
                    "multiple matches found ({count}) for session_id={session_id}; selected latest: {}",
                    selected.display()
                ));
            }

            let meta = ResolutionMeta {
                source: "codex:sessions".to_string(),
                candidate_count: count,
                warnings,
            };

            return Ok(ResolvedThread {
                provider: ProviderKind::Codex,
                session_id: session_id.to_string(),
                path: selected,
                metadata: meta,
            });
        }

        if let Some(record) = sqlite_record.as_ref().filter(|record| record.archived) {
            if record.rollout_path.exists() {
                return Ok(ResolvedThread {
                    provider: ProviderKind::Codex,
                    session_id: session_id.to_string(),
                    path: record.rollout_path.clone(),
                    metadata: ResolutionMeta {
                        source: "codex:sqlite:archived_sessions".to_string(),
                        candidate_count: 1,
                        warnings,
                    },
                });
            }

            warnings.push(format!(
                "sqlite thread index points to a missing archived rollout for session_id={session_id}: {}",
                record.rollout_path.display()
            ));
        }

        let archived_candidates = Self::find_candidates(&archived, session_id);
        if let Some((selected, count)) = Self::choose_latest(archived_candidates) {
            if count > 1 {
                warnings.push(format!(
                    "multiple archived matches found ({count}) for session_id={session_id}; selected latest: {}",
                    selected.display()
                ));
            }

            let meta = ResolutionMeta {
                source: "codex:archived_sessions".to_string(),
                candidate_count: count,
                warnings,
            };

            return Ok(ResolvedThread {
                provider: ProviderKind::Codex,
                session_id: session_id.to_string(),
                path: selected,
                metadata: meta,
            });
        }

        Err(TurlError::ThreadNotFound {
            provider: ProviderKind::Codex.to_string(),
            session_id: session_id.to_string(),
            searched_roots: vec![sessions, archived]
                .into_iter()
                .chain(state_dbs)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use rusqlite::Connection;
    use tempfile::tempdir;

    use crate::provider::Provider;
    use crate::provider::codex::CodexProvider;

    fn prepare_state_db(path: &Path) -> Connection {
        let conn = Connection::open(path).expect("open sqlite");
        conn.execute_batch(
            "
            CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                archived INTEGER NOT NULL DEFAULT 0
            );
            ",
        )
        .expect("create schema");
        conn
    }

    #[test]
    fn resolves_from_sessions() {
        let temp = tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("sessions/2026/02/23/rollout-2026-02-23T04-48-50-019c871c-b1f9-7f60-9c4f-87ed09f13592.jsonl");
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(&path, "{}\n").expect("write");

        let provider = CodexProvider::new(temp.path());
        let resolved = provider
            .resolve("019c871c-b1f9-7f60-9c4f-87ed09f13592")
            .expect("resolve should succeed");
        assert_eq!(resolved.path, path);
    }

    #[test]
    fn resolves_from_archived_when_not_in_sessions() {
        let temp = tempdir().expect("tempdir");
        let path = temp
            .path()
            .join("archived_sessions/rollout-2026-02-22T01-05-36-019c8129-f668-7951-8d56-cc5513541c26.jsonl");
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(&path, "{}\n").expect("write");

        let provider = CodexProvider::new(temp.path());
        let resolved = provider
            .resolve("019c8129-f668-7951-8d56-cc5513541c26")
            .expect("resolve should succeed");
        assert_eq!(resolved.path, path);
        assert_eq!(resolved.metadata.source, "codex:archived_sessions");
    }

    #[test]
    fn returns_not_found_when_missing() {
        let temp = tempdir().expect("tempdir");
        let provider = CodexProvider::new(temp.path());
        let err = provider
            .resolve("019c8129-f668-7951-8d56-cc5513541c26")
            .expect_err("should fail");
        assert!(format!("{err}").contains("thread not found"));
    }

    #[test]
    fn resolves_from_sqlite_state_index() {
        let temp = tempdir().expect("tempdir");
        let state_db = temp.path().join("state_5.sqlite");
        let conn = prepare_state_db(&state_db);

        let session_id = "019c871c-b1f9-7f60-9c4f-87ed09f13592";
        let rollout = temp.path().join("sessions/custom/path/thread.jsonl");
        fs::create_dir_all(rollout.parent().expect("parent")).expect("mkdir");
        fs::write(&rollout, "{}\n").expect("write");

        conn.execute(
            "INSERT INTO threads (id, rollout_path, archived) VALUES (?1, ?2, 0)",
            (&session_id, rollout.display().to_string()),
        )
        .expect("insert thread");

        let provider = CodexProvider::new(temp.path());
        let resolved = provider
            .resolve(session_id)
            .expect("resolve should succeed");
        assert_eq!(resolved.path, rollout);
        assert_eq!(resolved.metadata.source, "codex:sqlite:sessions");
    }

    #[test]
    fn resolves_archived_from_sqlite_state_index() {
        let temp = tempdir().expect("tempdir");
        let state_db = temp.path().join("state.sqlite");
        let conn = prepare_state_db(&state_db);

        let session_id = "019c8129-f668-7951-8d56-cc5513541c26";
        let rollout = temp
            .path()
            .join("archived_sessions/custom/path/thread.jsonl");
        fs::create_dir_all(rollout.parent().expect("parent")).expect("mkdir");
        fs::write(&rollout, "{}\n").expect("write");

        conn.execute(
            "INSERT INTO threads (id, rollout_path, archived) VALUES (?1, ?2, 1)",
            (&session_id, rollout.display().to_string()),
        )
        .expect("insert thread");

        let provider = CodexProvider::new(temp.path());
        let resolved = provider
            .resolve(session_id)
            .expect("resolve should succeed");
        assert_eq!(resolved.path, rollout);
        assert_eq!(resolved.metadata.source, "codex:sqlite:archived_sessions");
    }

    #[test]
    fn falls_back_to_filesystem_when_sqlite_rollout_missing() {
        let temp = tempdir().expect("tempdir");
        let state_db = temp.path().join("state_5.sqlite");
        let conn = prepare_state_db(&state_db);

        let session_id = "019c871c-b1f9-7f60-9c4f-87ed09f13592";
        let stale_rollout = temp.path().join("sessions/stale/path/thread.jsonl");
        conn.execute(
            "INSERT INTO threads (id, rollout_path, archived) VALUES (?1, ?2, 0)",
            (&session_id, stale_rollout.display().to_string()),
        )
        .expect("insert thread");

        let fs_rollout = temp.path().join(
            "sessions/2026/02/23/rollout-2026-02-23T04-48-50-019c871c-b1f9-7f60-9c4f-87ed09f13592.jsonl",
        );
        fs::create_dir_all(fs_rollout.parent().expect("parent")).expect("mkdir");
        fs::write(&fs_rollout, "{}\n").expect("write");

        let provider = CodexProvider::new(temp.path());
        let resolved = provider
            .resolve(session_id)
            .expect("resolve should succeed");
        assert_eq!(resolved.path, fs_rollout);
        assert_eq!(resolved.metadata.source, "codex:sessions");
        assert_eq!(resolved.metadata.warnings.len(), 1);
        assert!(resolved.metadata.warnings[0].contains("missing rollout"));
    }
}
