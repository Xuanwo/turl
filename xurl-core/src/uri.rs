use std::str::FromStr;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::{Result, XurlError};
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
static PI_SHORT_ENTRY_ID_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)^[0-9a-f]{8}$").expect("valid regex"));

pub fn is_uuid_session_id(input: &str) -> bool {
    SESSION_ID_RE.is_match(input)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadUri {
    pub provider: ProviderKind,
    pub session_id: String,
    pub agent_id: Option<String>,
}

impl ThreadUri {
    pub fn parse(input: &str) -> Result<Self> {
        input.parse()
    }

    pub fn as_agents_string(&self) -> String {
        match &self.agent_id {
            Some(agent_id) => format!(
                "agents://{}/{}/{}",
                self.provider, self.session_id, agent_id
            ),
            None => format!("agents://{}/{}", self.provider, self.session_id),
        }
    }

    pub fn as_string(&self) -> String {
        match &self.agent_id {
            Some(agent_id) => format!("{}://{}/{}", self.provider, self.session_id, agent_id),
            None => format!("{}://{}", self.provider, self.session_id),
        }
    }
}

impl FromStr for ThreadUri {
    type Err = XurlError;

    fn from_str(input: &str) -> Result<Self> {
        let (scheme, target) = input
            .split_once("://")
            .ok_or_else(|| XurlError::InvalidUri(input.to_string()))?;

        let (provider, provider_target) = if scheme == "agents" {
            let (provider_scheme, provider_target) = target
                .split_once('/')
                .ok_or_else(|| XurlError::InvalidUri(input.to_string()))?;
            if provider_target.is_empty() {
                return Err(XurlError::InvalidUri(input.to_string()));
            }
            (parse_provider(provider_scheme)?, provider_target)
        } else {
            (parse_provider(scheme)?, target)
        };

        let normalized_target = match provider {
            ProviderKind::Amp => provider_target,
            ProviderKind::Codex => provider_target
                .strip_prefix("threads/")
                .unwrap_or(provider_target),
            ProviderKind::Claude
            | ProviderKind::Gemini
            | ProviderKind::Pi
            | ProviderKind::Opencode => provider_target,
        };

        let (id, agent_id) = match provider {
            ProviderKind::Amp
            | ProviderKind::Codex
            | ProviderKind::Claude
            | ProviderKind::Gemini
            | ProviderKind::Pi
            | ProviderKind::Opencode => {
                let mut segments = normalized_target.split('/');
                let main_id = segments.next().unwrap_or_default();
                let agent_id = segments.next().map(str::to_string);

                if segments.next().is_some() {
                    return Err(XurlError::InvalidUri(input.to_string()));
                }

                if agent_id.as_deref().is_some_and(str::is_empty) {
                    return Err(XurlError::InvalidUri(input.to_string()));
                }

                (main_id, agent_id)
            }
        };

        match provider {
            ProviderKind::Amp if !AMP_SESSION_ID_RE.is_match(id) => {
                return Err(XurlError::InvalidSessionId(id.to_string()));
            }
            ProviderKind::Codex
            | ProviderKind::Claude
            | ProviderKind::Gemini
            | ProviderKind::Pi
                if !is_uuid_session_id(id) =>
            {
                return Err(XurlError::InvalidSessionId(id.to_string()));
            }
            ProviderKind::Opencode if !OPENCODE_SESSION_ID_RE.is_match(id) => {
                return Err(XurlError::InvalidSessionId(id.to_string()));
            }
            _ => {}
        }

        if provider == ProviderKind::Amp
            && let Some(agent_id) = agent_id.as_deref()
            && !AMP_SESSION_ID_RE.is_match(agent_id)
        {
            return Err(XurlError::InvalidSessionId(agent_id.to_string()));
        }

        let session_id = match provider {
            ProviderKind::Amp => format!("T-{}", id[2..].to_ascii_lowercase()),
            ProviderKind::Codex
            | ProviderKind::Claude
            | ProviderKind::Gemini
            | ProviderKind::Pi => id.to_ascii_lowercase(),
            ProviderKind::Opencode => id.to_string(),
        };

        let agent_id = agent_id.map(|agent_id| {
            if provider == ProviderKind::Amp && AMP_SESSION_ID_RE.is_match(&agent_id) {
                format!("T-{}", agent_id[2..].to_ascii_lowercase())
            } else if ((provider == ProviderKind::Codex || provider == ProviderKind::Gemini)
                && SESSION_ID_RE.is_match(&agent_id))
                || (provider == ProviderKind::Pi
                    && (is_uuid_session_id(&agent_id) || PI_SHORT_ENTRY_ID_RE.is_match(&agent_id)))
            {
                agent_id.to_ascii_lowercase()
            } else {
                agent_id
            }
        });

        if provider == ProviderKind::Opencode
            && let Some(child_id) = agent_id.as_deref()
            && !OPENCODE_SESSION_ID_RE.is_match(child_id)
        {
            return Err(XurlError::InvalidSessionId(child_id.to_string()));
        }

        Ok(Self {
            provider,
            session_id,
            agent_id,
        })
    }
}

fn parse_provider(scheme: &str) -> Result<ProviderKind> {
    match scheme {
        "amp" => Ok(ProviderKind::Amp),
        "codex" => Ok(ProviderKind::Codex),
        "claude" => Ok(ProviderKind::Claude),
        "gemini" => Ok(ProviderKind::Gemini),
        "pi" => Ok(ProviderKind::Pi),
        "opencode" => Ok(ProviderKind::Opencode),
        _ => Err(XurlError::UnsupportedScheme(scheme.to_string())),
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
        assert_eq!(uri.agent_id, None);
    }

    #[test]
    fn parse_valid_amp_uri() {
        let uri = ThreadUri::parse("amp://T-019C0797-C402-7389-BD80-D785C98DF295")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Amp);
        assert_eq!(uri.session_id, "T-019c0797-c402-7389-bd80-d785c98df295");
        assert_eq!(uri.agent_id, None);
    }

    #[test]
    fn parse_codex_deeplink_uri() {
        let uri = ThreadUri::parse("codex://threads/019c871c-b1f9-7f60-9c4f-87ed09f13592")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Codex);
        assert_eq!(uri.session_id, "019c871c-b1f9-7f60-9c4f-87ed09f13592");
        assert_eq!(uri.agent_id, None);
    }

    #[test]
    fn parse_agents_uri() {
        let uri = ThreadUri::parse("agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Codex);
        assert_eq!(uri.session_id, "019c871c-b1f9-7f60-9c4f-87ed09f13592");
        assert_eq!(uri.agent_id, None);
    }

    #[test]
    fn parse_agents_codex_deeplink_uri() {
        let uri = ThreadUri::parse("agents://codex/threads/019c871c-b1f9-7f60-9c4f-87ed09f13592")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Codex);
        assert_eq!(uri.session_id, "019c871c-b1f9-7f60-9c4f-87ed09f13592");
        assert_eq!(uri.agent_id, None);
    }

    #[test]
    fn parse_codex_subagent_uri() {
        let uri = ThreadUri::parse(
            "codex://019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495",
        )
        .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Codex);
        assert_eq!(uri.session_id, "019c871c-b1f9-7f60-9c4f-87ed09f13592");
        assert_eq!(
            uri.agent_id,
            Some("019c87fb-38b9-7843-92b1-832f02598495".to_string())
        );
    }

    #[test]
    fn parse_agents_codex_subagent_uri() {
        let uri = ThreadUri::parse(
            "agents://codex/019c871c-b1f9-7f60-9c4f-87ed09f13592/019c87fb-38b9-7843-92b1-832f02598495",
        )
        .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Codex);
        assert_eq!(uri.session_id, "019c871c-b1f9-7f60-9c4f-87ed09f13592");
        assert_eq!(
            uri.agent_id,
            Some("019c87fb-38b9-7843-92b1-832f02598495".to_string())
        );
    }

    #[test]
    fn parse_amp_subagent_uri() {
        let uri = ThreadUri::parse(
            "amp://T-019C0797-C402-7389-BD80-D785C98DF295/T-1ABC0797-C402-7389-BD80-D785C98DF295",
        )
        .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Amp);
        assert_eq!(uri.session_id, "T-019c0797-c402-7389-bd80-d785c98df295");
        assert_eq!(
            uri.agent_id,
            Some("T-1abc0797-c402-7389-bd80-d785c98df295".to_string())
        );
    }

    #[test]
    fn parse_agents_amp_subagent_uri() {
        let uri = ThreadUri::parse(
            "agents://amp/T-019C0797-C402-7389-BD80-D785C98DF295/T-1ABC0797-C402-7389-BD80-D785C98DF295",
        )
        .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Amp);
        assert_eq!(uri.session_id, "T-019c0797-c402-7389-bd80-d785c98df295");
        assert_eq!(
            uri.agent_id,
            Some("T-1abc0797-c402-7389-bd80-d785c98df295".to_string())
        );
    }

    #[test]
    fn parse_claude_subagent_uri() {
        let uri = ThreadUri::parse("claude://2823d1df-720a-4c31-ac55-ae8ba726721f/acompact-69d537")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Claude);
        assert_eq!(uri.session_id, "2823d1df-720a-4c31-ac55-ae8ba726721f");
        assert_eq!(uri.agent_id, Some("acompact-69d537".to_string()));
    }

    #[test]
    fn parse_rejects_extra_path_segments() {
        let err = ThreadUri::parse("codex://019c871c-b1f9-7f60-9c4f-87ed09f13592/a/b")
            .expect_err("must reject nested path");
        assert!(format!("{err}").contains("invalid uri"));
    }

    #[test]
    fn parse_rejects_invalid_child_id_for_amp() {
        let err = ThreadUri::parse("amp://T-019c0797-c402-7389-bd80-d785c98df295/child")
            .expect_err("must reject amp path segment");
        assert!(format!("{err}").contains("invalid session id"));
    }

    #[test]
    fn parse_rejects_extra_path_segments_for_amp() {
        let err = ThreadUri::parse(
            "amp://T-019c0797-c402-7389-bd80-d785c98df295/T-1abc0797-c402-7389-bd80-d785c98df295/extra",
        )
        .expect_err("must reject nested path");
        assert!(format!("{err}").contains("invalid uri"));
    }

    #[test]
    fn parse_rejects_invalid_scheme() {
        let err = ThreadUri::parse("cursor://019c871c-b1f9-7f60-9c4f-87ed09f13592")
            .expect_err("must reject unsupported scheme");
        assert!(format!("{err}").contains("unsupported scheme"));
    }

    #[test]
    fn parse_rejects_invalid_agents_provider() {
        let err = ThreadUri::parse("agents://cursor/019c871c-b1f9-7f60-9c4f-87ed09f13592")
            .expect_err("must reject unsupported provider");
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
        assert_eq!(uri.agent_id, None);
    }

    #[test]
    fn parse_valid_gemini_uri() {
        let uri = ThreadUri::parse("gemini://29D207DB-CA7E-40BA-87F7-E14C9DE60613")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Gemini);
        assert_eq!(uri.session_id, "29d207db-ca7e-40ba-87f7-e14c9de60613");
        assert_eq!(uri.agent_id, None);
    }

    #[test]
    fn parse_gemini_subagent_uri() {
        let uri = ThreadUri::parse(
            "gemini://29d207db-ca7e-40ba-87f7-e14c9de60613/2B112C8A-D80A-4CFF-9C8A-6F3E6FBAF7FB",
        )
        .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Gemini);
        assert_eq!(uri.session_id, "29d207db-ca7e-40ba-87f7-e14c9de60613");
        assert_eq!(
            uri.agent_id,
            Some("2b112c8a-d80a-4cff-9c8a-6f3e6fbaf7fb".to_string())
        );
    }

    #[test]
    fn parse_agents_gemini_subagent_uri() {
        let uri = ThreadUri::parse(
            "agents://gemini/29d207db-ca7e-40ba-87f7-e14c9de60613/2b112c8a-d80a-4cff-9c8a-6f3e6fbaf7fb",
        )
        .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Gemini);
        assert_eq!(uri.session_id, "29d207db-ca7e-40ba-87f7-e14c9de60613");
        assert_eq!(
            uri.agent_id,
            Some("2b112c8a-d80a-4cff-9c8a-6f3e6fbaf7fb".to_string())
        );
    }

    #[test]
    fn parse_valid_pi_uri() {
        let uri = ThreadUri::parse("pi://12CB4C19-2774-4DE4-A0D0-9FA32FBAE29F")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Pi);
        assert_eq!(uri.session_id, "12cb4c19-2774-4de4-a0d0-9fa32fbae29f");
        assert_eq!(uri.agent_id, None);
    }

    #[test]
    fn parse_valid_pi_entry_uri() {
        let uri = ThreadUri::parse("pi://12cb4c19-2774-4de4-a0d0-9fa32fbae29f/1C130174")
            .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Pi);
        assert_eq!(uri.session_id, "12cb4c19-2774-4de4-a0d0-9fa32fbae29f");
        assert_eq!(uri.agent_id, Some("1c130174".to_string()));
    }

    #[test]
    fn parse_valid_pi_child_session_uri() {
        let uri = ThreadUri::parse(
            "pi://12cb4c19-2774-4de4-a0d0-9fa32fbae29f/72B3A4A8-4F08-40AF-8D7F-8B2C77584E89",
        )
        .expect("parse should succeed");
        assert_eq!(uri.provider, ProviderKind::Pi);
        assert_eq!(uri.session_id, "12cb4c19-2774-4de4-a0d0-9fa32fbae29f");
        assert_eq!(
            uri.agent_id,
            Some("72b3a4a8-4f08-40af-8d7f-8b2c77584e89".to_string())
        );
    }

    #[test]
    fn parse_rejects_extra_path_segments_for_pi() {
        let err = ThreadUri::parse("pi://12cb4c19-2774-4de4-a0d0-9fa32fbae29f/a/b")
            .expect_err("must reject nested path");
        assert!(format!("{err}").contains("invalid uri"));
    }
}
