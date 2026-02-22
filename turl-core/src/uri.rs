use std::str::FromStr;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::{Result, TurlError};
use crate::model::ProviderKind;

static SESSION_ID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .expect("valid regex")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadUri {
    pub provider: ProviderKind,
    pub session_id: String,
}

impl ThreadUri {
    pub fn parse(input: &str) -> Result<Self> {
        input.parse()
    }

    pub fn as_string(&self) -> String {
        format!("{}://{}", self.provider, self.session_id)
    }
}

impl FromStr for ThreadUri {
    type Err = TurlError;

    fn from_str(input: &str) -> Result<Self> {
        let (scheme, target) = input
            .split_once("://")
            .ok_or_else(|| TurlError::InvalidUri(input.to_string()))?;

        let provider = match scheme {
            "codex" => ProviderKind::Codex,
            "claude" => ProviderKind::Claude,
            _ => return Err(TurlError::UnsupportedScheme(scheme.to_string())),
        };

        let id = match provider {
            ProviderKind::Codex => target.strip_prefix("threads/").unwrap_or(target),
            ProviderKind::Claude => target,
        };

        if !SESSION_ID_RE.is_match(id) {
            return Err(TurlError::InvalidSessionId(id.to_string()));
        }

        Ok(Self {
            provider,
            session_id: id.to_ascii_lowercase(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ThreadUri;
    use crate::model::ProviderKind;

    #[test]
    fn parse_valid_uri() {
        let uri = ThreadUri::parse("codex://019c871c-b1f9-7f60-9c4f-87ed09f13592")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Codex);
        assert_eq!(uri.session_id, "019c871c-b1f9-7f60-9c4f-87ed09f13592");
    }

    #[test]
    fn parse_codex_deeplink_uri() {
        let uri = ThreadUri::parse("codex://threads/019c871c-b1f9-7f60-9c4f-87ed09f13592")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Codex);
        assert_eq!(uri.session_id, "019c871c-b1f9-7f60-9c4f-87ed09f13592");
    }

    #[test]
    fn parse_rejects_invalid_scheme() {
        let err = ThreadUri::parse("cursor://019c871c-b1f9-7f60-9c4f-87ed09f13592")
            .expect_err("must reject unsupported scheme");
        assert!(format!("{err}").contains("unsupported scheme"));
    }

    #[test]
    fn parse_rejects_invalid_session_id() {
        let err = ThreadUri::parse("codex://agent-a1b2c3").expect_err("must reject non-session id");
        assert!(format!("{err}").contains("invalid session id"));
    }
}
