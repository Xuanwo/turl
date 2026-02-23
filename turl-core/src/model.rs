use std::fmt;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ProviderKind {
    Amp,
    Codex,
    Claude,
    Gemini,
    Opencode,
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Amp => write!(f, "amp"),
            Self::Codex => write!(f, "codex"),
            Self::Claude => write!(f, "claude"),
            Self::Gemini => write!(f, "gemini"),
            Self::Opencode => write!(f, "opencode"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolutionMeta {
    pub source: String,
    pub candidate_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedThread {
    pub provider: ProviderKind,
    pub session_id: String,
    pub path: PathBuf,
    pub metadata: ResolutionMeta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MessageRole {
    User,
    Assistant,
}

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMessage {
    pub role: MessageRole,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubagentQuery {
    pub provider: String,
    pub main_thread_id: String,
    pub agent_id: Option<String>,
    pub list: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SubagentRelation {
    pub validated: bool,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubagentLifecycleEvent {
    pub timestamp: Option<String>,
    pub event: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubagentExcerptMessage {
    pub role: MessageRole,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubagentThreadRef {
    pub thread_id: String,
    pub path: Option<String>,
    pub last_updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubagentDetailView {
    pub query: SubagentQuery,
    pub relation: SubagentRelation,
    pub lifecycle: Vec<SubagentLifecycleEvent>,
    pub status: String,
    pub status_source: String,
    pub child_thread: Option<SubagentThreadRef>,
    pub excerpt: Vec<SubagentExcerptMessage>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubagentListItem {
    pub agent_id: String,
    pub status: String,
    pub status_source: String,
    pub last_update: Option<String>,
    pub relation: SubagentRelation,
    pub child_thread: Option<SubagentThreadRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubagentListView {
    pub query: SubagentQuery,
    pub agents: Vec<SubagentListItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubagentView {
    List(SubagentListView),
    Detail(SubagentDetailView),
}
