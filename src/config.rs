use std::fmt;
use std::path::PathBuf;

use crate::error::MiniClawError;

#[derive(Debug, Clone)]
pub struct Config {
    pub telegram_token: String,
    pub workspace: PathBuf,
    pub session_dir: PathBuf,
    pub thinking_level: ThinkingLevel,
    pub allowed_users: Vec<i64>,
    pub rate_limit_cooldown_ms: u64,
    pub pi_timeout_ms: u64,
    pub shell_timeout_ms: u64,
    pub session_title_timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThinkingLevel {
    Low,
    Medium,
    High,
}

impl fmt::Display for ThinkingLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThinkingLevel::Low => write!(f, "low"),
            ThinkingLevel::Medium => write!(f, "medium"),
            ThinkingLevel::High => write!(f, "high"),
        }
    }
}

impl ThinkingLevel {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "medium" => ThinkingLevel::Medium,
            "high" => ThinkingLevel::High,
            _ => ThinkingLevel::Low,
        }
    }
}

pub fn load_config() -> Result<Config, MiniClawError> {
    dotenvy::dotenv().ok();

    let telegram_token = std::env::var("TELEGRAM_BOT_TOKEN")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if telegram_token.is_empty() {
        return Err(MiniClawError::Config(
            "TELEGRAM_BOT_TOKEN is required. Set it in .env file.".into(),
        ));
    }

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));

    let workspace = std::env::var("MINI_CLAW_WORKSPACE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| {
            if let Some(stripped) = s.strip_prefix('~') {
                home.join(stripped.trim_start_matches('/'))
            } else {
                PathBuf::from(s)
            }
        })
        .unwrap_or_else(|| home.join("mini-claw-workspace"));

    let session_dir = std::env::var("MINI_CLAW_SESSION_DIR")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| {
            if let Some(stripped) = s.strip_prefix('~') {
                home.join(stripped.trim_start_matches('/'))
            } else {
                PathBuf::from(s)
            }
        })
        .unwrap_or_else(|| home.join(".mini-claw").join("sessions"));

    let thinking_level = std::env::var("PI_THINKING_LEVEL")
        .ok()
        .map(|s| ThinkingLevel::from_str(s.trim()))
        .unwrap_or(ThinkingLevel::Low);

    let allowed_users = std::env::var("ALLOWED_USERS")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.split(',')
                .filter_map(|id| id.trim().parse::<i64>().ok())
                .collect()
        })
        .unwrap_or_default();

    let rate_limit_cooldown_ms = std::env::var("RATE_LIMIT_COOLDOWN_MS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(5000);

    let pi_timeout_ms = std::env::var("PI_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(5 * 60 * 1000);

    let shell_timeout_ms = std::env::var("SHELL_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(60_000);

    let session_title_timeout_ms = std::env::var("SESSION_TITLE_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(10_000);

    Ok(Config {
        telegram_token,
        workspace,
        session_dir,
        thinking_level,
        allowed_users,
        rate_limit_cooldown_ms,
        pi_timeout_ms,
        shell_timeout_ms,
        session_title_timeout_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_level_display() {
        assert_eq!(ThinkingLevel::Low.to_string(), "low");
        assert_eq!(ThinkingLevel::Medium.to_string(), "medium");
        assert_eq!(ThinkingLevel::High.to_string(), "high");
    }

    #[test]
    fn test_thinking_level_from_str() {
        assert_eq!(ThinkingLevel::from_str("low"), ThinkingLevel::Low);
        assert_eq!(ThinkingLevel::from_str("medium"), ThinkingLevel::Medium);
        assert_eq!(ThinkingLevel::from_str("high"), ThinkingLevel::High);
        assert_eq!(ThinkingLevel::from_str("HIGH"), ThinkingLevel::High);
        assert_eq!(ThinkingLevel::from_str("unknown"), ThinkingLevel::Low);
        assert_eq!(ThinkingLevel::from_str(""), ThinkingLevel::Low);
    }

    #[test]
    fn test_load_config_missing_token() {
        // Clear the token to test missing token error
        std::env::remove_var("TELEGRAM_BOT_TOKEN");
        let result = load_config();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("TELEGRAM_BOT_TOKEN"));
    }
}
