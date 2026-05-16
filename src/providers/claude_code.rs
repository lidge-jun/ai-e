pub const DEFAULT_BINARY: &str = "claude";
pub const DEFAULT_TIMEOUT_MS: u64 = 600_000;
pub const DEFAULT_COLS: u16 = 120;
pub const DEFAULT_ROWS: u16 = 40;

pub const BINARY_ENV_VARS: &[&str] = &["AI_E_CLAUDE_BIN", "CLAUDE_BIN"];

pub fn resolve_binary() -> String {
    BINARY_ENV_VARS
        .iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| DEFAULT_BINARY.to_string())
}
