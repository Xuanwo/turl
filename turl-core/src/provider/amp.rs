use std::path::PathBuf;

use crate::error::{Result, TurlError};
use crate::model::{ProviderKind, ResolutionMeta, ResolvedThread};
use crate::provider::Provider;

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
}

impl Provider for AmpProvider {
    fn resolve(&self, session_id: &str) -> Result<ResolvedThread> {
        let threads_root = self.threads_root();
        let path = threads_root.join(format!("{session_id}.json"));

        if !path.exists() {
            return Err(TurlError::ThreadNotFound {
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
