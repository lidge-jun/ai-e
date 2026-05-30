use std::ffi::OsString;

pub mod claude_code;
pub mod kiro_session;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    ClaudeCode,
    Codex,
    Gemini,
    Grok,
    Copilot,
    Kiro,
}

impl ProviderKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "claude" | "claude-code" => Some(Self::ClaudeCode),
            "codex" => Some(Self::Codex),
            "gemini" => Some(Self::Gemini),
            "grok" => Some(Self::Grok),
            "copilot" | "github-copilot" => Some(Self::Copilot),
            "kiro" | "kiro-code" => Some(Self::Kiro),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Grok => "grok",
            Self::Copilot => "copilot",
            Self::Kiro => "kiro",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex CLI",
            Self::Gemini => "Gemini CLI",
            Self::Grok => "Grok CLI",
            Self::Copilot => "GitHub Copilot CLI",
            Self::Kiro => "Kiro CLI",
        }
    }

    pub fn default_binary(self) -> &'static str {
        match self {
            Self::ClaudeCode => claude_code::DEFAULT_BINARY,
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Grok => "grok",
            Self::Copilot => "copilot",
            Self::Kiro => "kiro-cli",
        }
    }

    pub fn binary_env_vars(self) -> &'static [&'static str] {
        match self {
            Self::ClaudeCode => claude_code::BINARY_ENV_VARS,
            Self::Codex => &["AI_E_CODEX_BIN", "CODEX_BIN"],
            Self::Gemini => &["AI_E_GEMINI_BIN", "GEMINI_BIN"],
            Self::Grok => &["AI_E_GROK_BIN", "GROK_BIN"],
            Self::Copilot => &["AI_E_COPILOT_BIN", "COPILOT_BIN"],
            Self::Kiro => &["AI_E_KIRO_BIN", "KIRO_BIN", "KIRO_CLI_BIN"],
        }
    }

    pub fn resolve_binary(self) -> String {
        self.binary_env_vars()
            .iter()
            .find_map(|name| {
                std::env::var(name)
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or_else(|| self.default_binary().to_string())
    }

    pub fn is_pty_provider(self) -> bool {
        matches!(
            self,
            Self::ClaudeCode | Self::Codex | Self::Gemini | Self::Grok | Self::Copilot
        )
    }

    pub fn is_headless_provider(self) -> bool {
        matches!(
            self,
            Self::Codex | Self::Gemini | Self::Grok | Self::Copilot | Self::Kiro
        )
    }

    pub fn supports_interactive(self) -> bool {
        matches!(
            self,
            Self::Codex | Self::Gemini | Self::Grok | Self::Copilot | Self::Kiro
        )
    }
}

pub fn split_provider_args(raw_args: Vec<OsString>) -> (ProviderKind, Vec<OsString>) {
    let Some(first) = raw_args.first().and_then(|arg| arg.to_str()) else {
        return (ProviderKind::ClaudeCode, raw_args);
    };

    let Some(provider) = ProviderKind::parse(first) else {
        return (ProviderKind::ClaudeCode, raw_args);
    };

    let rest = raw_args.into_iter().skip(1).collect();
    (provider, rest)
}

pub fn unsupported_provider_message(provider: ProviderKind) -> String {
    format!(
        "{} provider is not available in this runtime path.",
        provider.label()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn splits_explicit_provider() {
        let (provider, args) = split_provider_args(os_args(&["copilot", "--model", "gpt-5-mini"]));
        assert_eq!(provider, ProviderKind::Copilot);
        assert_eq!(args, os_args(&["--model", "gpt-5-mini"]));
    }

    #[test]
    fn defaults_to_claude_when_provider_is_omitted() {
        let (provider, args) = split_provider_args(os_args(&["--model", "opus", "hello"]));
        assert_eq!(provider, ProviderKind::ClaudeCode);
        assert_eq!(args, os_args(&["--model", "opus", "hello"]));
    }

    #[test]
    fn parses_kiro_alias() {
        assert_eq!(ProviderKind::parse("kiro-code"), Some(ProviderKind::Kiro));
    }

    #[test]
    fn pty_backed_providers_exclude_kiro() {
        for provider in [
            ProviderKind::ClaudeCode,
            ProviderKind::Codex,
            ProviderKind::Gemini,
            ProviderKind::Grok,
            ProviderKind::Copilot,
        ] {
            assert!(
                provider.is_pty_provider(),
                "{} should use PTY",
                provider.id()
            );
        }
        assert!(
            !ProviderKind::Kiro.is_pty_provider(),
            "kiro should not use PTY"
        );
    }

    #[test]
    fn headless_providers_include_kiro() {
        for provider in [
            ProviderKind::Codex,
            ProviderKind::Gemini,
            ProviderKind::Grok,
            ProviderKind::Copilot,
            ProviderKind::Kiro,
        ] {
            assert!(
                provider.is_headless_provider(),
                "{} should use headless adapter",
                provider.id()
            );
        }
    }
}
