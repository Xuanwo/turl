use std::str::FromStr;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::{Result, TurlError};
use crate::model::ProviderKind;

static SESSION_ID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .expect("valid regex")
});
static AMP_SESSION_ID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^t-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .expect("valid regex")
});
static OPENCODE_SESSION_ID_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^ses_[0-9A-Za-z]+$").expect("valid regex"));

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
            "amp" => ProviderKind::Amp,
            "codex" => ProviderKind::Codex,
            "claude" => ProviderKind::Claude,
            "gemini" => ProviderKind::Gemini,
            "opencode" => ProviderKind::Opencode,
            _ => return Err(TurlError::UnsupportedScheme(scheme.to_string())),
        };

        let id = match provider {
            ProviderKind::Amp => target,
            ProviderKind::Codex => target.strip_prefix("threads/").unwrap_or(target),
            ProviderKind::Claude | ProviderKind::Gemini | ProviderKind::Opencode => target,
        };

        match provider {
            ProviderKind::Amp if !AMP_SESSION_ID_RE.is_match(id) => {
                return Err(TurlError::InvalidSessionId(id.to_string()));
            }
            ProviderKind::Codex | ProviderKind::Claude | ProviderKind::Gemini
                if !SESSION_ID_RE.is_match(id) =>
            {
                return Err(TurlError::InvalidSessionId(id.to_string()));
            }
            ProviderKind::Opencode if !OPENCODE_SESSION_ID_RE.is_match(id) => {
                return Err(TurlError::InvalidSessionId(id.to_string()));
            }
            _ => {}
        }

        let session_id = match provider {
            ProviderKind::Amp => format!("T-{}", id[2..].to_ascii_lowercase()),
            ProviderKind::Codex | ProviderKind::Claude | ProviderKind::Gemini => {
                id.to_ascii_lowercase()
            }
            ProviderKind::Opencode => id.to_string(),
        };

        Ok(Self {
            provider,
            session_id,
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
    fn parse_valid_amp_uri() {
        let uri = ThreadUri::parse("amp://T-019C0797-C402-7389-BD80-D785C98DF295")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Amp);
        assert_eq!(uri.session_id, "T-019c0797-c402-7389-bd80-d785c98df295");
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

    #[test]
    fn parse_valid_opencode_uri() {
        let uri = ThreadUri::parse("opencode://ses_43a90e3adffejRgrTdlJa48CtE")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Opencode);
        assert_eq!(uri.session_id, "ses_43a90e3adffejRgrTdlJa48CtE");
    }

    #[test]
    fn parse_valid_gemini_uri() {
        let uri = ThreadUri::parse("gemini://29D207DB-CA7E-40BA-87F7-E14C9DE60613")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Gemini);
        assert_eq!(uri.session_id, "29d207db-ca7e-40ba-87f7-e14c9de60613");
    }
}
