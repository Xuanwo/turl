use std::fs;
use std::path::Path;

use crate::error::{Result, TurlError};
use crate::model::{ProviderKind, ResolvedThread};
use crate::provider::amp::AmpProvider;
use crate::provider::claude::ClaudeProvider;
use crate::provider::codex::CodexProvider;
use crate::provider::gemini::GeminiProvider;
use crate::provider::opencode::OpencodeProvider;
use crate::provider::{Provider, ProviderRoots};
use crate::render;
use crate::uri::ThreadUri;

pub fn resolve_thread(uri: &ThreadUri, roots: &ProviderRoots) -> Result<ResolvedThread> {
    match uri.provider {
        ProviderKind::Amp => AmpProvider::new(&roots.amp_root).resolve(&uri.session_id),
        ProviderKind::Codex => CodexProvider::new(&roots.codex_root).resolve(&uri.session_id),
        ProviderKind::Claude => ClaudeProvider::new(&roots.claude_root).resolve(&uri.session_id),
        ProviderKind::Gemini => GeminiProvider::new(&roots.gemini_root).resolve(&uri.session_id),
        ProviderKind::Opencode => {
            OpencodeProvider::new(&roots.opencode_root).resolve(&uri.session_id)
        }
    }
}

pub fn read_thread_raw(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| TurlError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    if bytes.is_empty() {
        return Err(TurlError::EmptyThreadFile {
            path: path.to_path_buf(),
        });
    }

    String::from_utf8(bytes).map_err(|_| TurlError::NonUtf8ThreadFile {
        path: path.to_path_buf(),
    })
}

pub fn render_thread_markdown(uri: &ThreadUri, resolved: &ResolvedThread) -> Result<String> {
    let raw = read_thread_raw(&resolved.path)?;
    render::render_markdown(uri, &resolved.path, &raw)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::service::read_thread_raw;

    #[test]
    fn empty_file_returns_error() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("thread.jsonl");
        fs::write(&path, "").expect("write");

        let err = read_thread_raw(&path).expect_err("must fail");
        assert!(format!("{err}").contains("thread file is empty"));
    }
}
