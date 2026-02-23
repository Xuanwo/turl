use std::env;
use std::path::PathBuf;

use dirs::home_dir;

use crate::error::{Result, TurlError};
use crate::model::ResolvedThread;

pub mod amp;
pub mod claude;
pub mod codex;
pub mod gemini;
pub mod opencode;

pub trait Provider {
    fn resolve(&self, session_id: &str) -> Result<ResolvedThread>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRoots {
    pub amp_root: PathBuf,
    pub codex_root: PathBuf,
    pub claude_root: PathBuf,
    pub gemini_root: PathBuf,
    pub opencode_root: PathBuf,
}

impl ProviderRoots {
    pub fn from_env_or_home() -> Result<Self> {
        let home = home_dir().ok_or(TurlError::HomeDirectoryNotFound)?;

        // Precedence:
        // 1) XDG_DATA_HOME/amp
        // 2) ~/.local/share/amp
        let amp_root = env::var_os("XDG_DATA_HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .map(|path| path.join("amp"))
            .unwrap_or_else(|| home.join(".local/share/amp"));

        // Precedence:
        // 1) CODEX_HOME (official Codex home env)
        // 2) ~/.codex (Codex default)
        let codex_root = env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"));

        // Precedence:
        // 1) CLAUDE_CONFIG_DIR (official Claude Code config/data root env)
        // 2) ~/.claude (Claude default)
        let claude_root = env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".claude"));

        // Precedence:
        // 1) GEMINI_CLI_HOME/.gemini (official Gemini CLI home env)
        // 2) ~/.gemini (Gemini default)
        let gemini_root = env::var_os("GEMINI_CLI_HOME")
            .map(PathBuf::from)
            .map(|path| path.join(".gemini"))
            .unwrap_or_else(|| home.join(".gemini"));

        // Precedence:
        // 1) XDG_DATA_HOME/opencode
        // 2) ~/.local/share/opencode
        let opencode_root = env::var_os("XDG_DATA_HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .map(|path| path.join("opencode"))
            .unwrap_or_else(|| home.join(".local/share/opencode"));

        Ok(Self {
            amp_root,
            codex_root,
            claude_root,
            gemini_root,
            opencode_root,
        })
    }
}
