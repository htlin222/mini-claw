use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tokio::process::Command;
use tokio::time::Duration;

use crate::config::Config;
use crate::error::MiniClawError;

pub struct SessionManager {
    active_sessions: HashMap<String, String>, // chatId -> session filename
    active_sessions_file: PathBuf,
    loaded: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionInfo {
    pub filename: String,
    pub chat_id: String,
    pub path: PathBuf,
    pub modified_at: SystemTime,
    pub size_bytes: u64,
    pub title: Option<String>,
}

impl SessionManager {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        Self {
            active_sessions: HashMap::new(),
            active_sessions_file: home.join(".mini-claw").join("active-sessions.json"),
            loaded: false,
        }
    }

    async fn load(&mut self) {
        if self.loaded {
            return;
        }
        if let Ok(data) = tokio::fs::read_to_string(&self.active_sessions_file).await {
            if let Ok(parsed) = serde_json::from_str(&data) {
                self.active_sessions = parsed;
            }
        }
        self.loaded = true;
    }

    async fn save(&self) -> Result<(), MiniClawError> {
        if let Some(dir) = self.active_sessions_file.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let json = serde_json::to_string_pretty(&self.active_sessions)?;
        tokio::fs::write(&self.active_sessions_file, json).await?;
        Ok(())
    }

    pub fn default_session_filename(chat_id: i64) -> String {
        format!("telegram-{chat_id}.jsonl")
    }

    pub async fn get_active_session_filename(&mut self, chat_id: i64) -> String {
        self.load().await;
        let key = chat_id.to_string();
        self.active_sessions
            .get(&key)
            .cloned()
            .unwrap_or_else(|| Self::default_session_filename(chat_id))
    }

    pub async fn switch_session(
        &mut self,
        config: &Config,
        chat_id: i64,
        target_filename: &str,
    ) -> Result<(), MiniClawError> {
        self.load().await;

        let current_filename = self.get_active_session_filename(chat_id).await;
        let default_filename = Self::default_session_filename(chat_id);

        if current_filename == target_filename {
            return Ok(());
        }

        let target_path = config.session_dir.join(target_filename);
        let default_path = config.session_dir.join(&default_filename);

        // Verify target exists
        if tokio::fs::metadata(&target_path).await.is_err() {
            return Err(MiniClawError::Session(format!(
                "Session not found: {target_filename}"
            )));
        }

        // Archive current session if it's the default
        let current_path = config.session_dir.join(&current_filename);
        if current_filename == default_filename
            && tokio::fs::metadata(&current_path).await.is_ok() {
                let timestamp = chrono_like_timestamp();
                let archive_name = format!("telegram-{chat_id}-{timestamp}.jsonl");
                let archive_path = config.session_dir.join(&archive_name);
                tokio::fs::rename(&current_path, &archive_path).await?;
            }

        // Copy target to default path (Pi always uses default path)
        tokio::fs::copy(&target_path, &default_path).await?;

        self.active_sessions
            .insert(chat_id.to_string(), target_filename.to_string());
        self.save().await?;
        Ok(())
    }

    pub async fn clear_active_session(&mut self, chat_id: i64) {
        self.load().await;
        self.active_sessions.remove(&chat_id.to_string());
        let _ = self.save().await;
    }
}

fn chrono_like_timestamp() -> String {
    // Format similar to ISO but with dashes instead of colons/dots
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();

    // Simple UTC timestamp formatting
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    let millis = now.subsec_millis();

    // Calculate year/month/day from days since epoch (simplified)
    let (year, month, day) = days_to_ymd(days);

    format!("{year:04}-{month:02}-{day:02}T{hours:02}-{minutes:02}-{seconds:02}-{millis:03}Z")
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Simplified calendar calculation
    let mut y = 1970;
    let mut remaining = days;

    loop {
        let days_in_year = if is_leap_year(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let months = if is_leap_year(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 0;
    for (i, &days_in_month) in months.iter().enumerate() {
        if remaining < days_in_month {
            m = i as u64 + 1;
            break;
        }
        remaining -= days_in_month;
    }

    (y, m, remaining + 1)
}

fn is_leap_year(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

pub async fn list_sessions(config: &Config) -> Vec<SessionInfo> {
    static CHAT_ID_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^telegram-(-?\d+)").unwrap());

    let mut sessions = Vec::new();

    let Ok(mut entries) = tokio::fs::read_dir(&config.session_dir).await else {
        return sessions;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let filename = entry.file_name().to_string_lossy().to_string();
        if !filename.ends_with(".jsonl") {
            continue;
        }

        let Ok(meta) = entry.metadata().await else {
            continue;
        };

        // Extract chat ID from filename: telegram-<chatId>.jsonl or telegram-<chatId>-<timestamp>.jsonl
        let chat_id = CHAT_ID_RE
            .captures(&filename)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        sessions.push(SessionInfo {
            filename,
            chat_id,
            path: entry.path(),
            modified_at: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            size_bytes: meta.len(),
            title: None,
        });
    }

    // Sort by modified date, newest first
    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    sessions
}

async fn get_first_user_message(session_path: &Path) -> Option<String> {
    let content = tokio::fs::read_to_string(session_path).await.ok()?;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let entry: serde_json::Value = serde_json::from_str(line).ok()?;

        if entry.get("role").and_then(|r| r.as_str()) == Some("user") {
            if let Some(content) = entry.get("content") {
                let text = if let Some(s) = content.as_str() {
                    s.to_string()
                } else if let Some(arr) = content.as_array() {
                    arr.first()
                        .and_then(|item| item.get("text"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string()
                } else {
                    continue;
                };

                let text = text.trim().to_string();
                if !text.is_empty() {
                    return Some(text[..text.len().min(500)].to_string());
                }
            }
        }
    }

    None
}

pub async fn generate_session_title(session_path: &Path, timeout_ms: u64) -> String {
    let first_message = match get_first_user_message(session_path).await {
        Some(msg) => msg,
        None => return "Empty session".to_string(),
    };

    let prompt = format!(
        "Generate a very short title (max 5 words) for a conversation that started with: \"{}\". Reply with ONLY the title, no quotes, no explanation.",
        &first_message[..first_message.len().min(200)]
    );

    let result = tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        async {
            let output = Command::new("pi")
                .args(["--print", "--no-session", &prompt])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output()
                .await;

            match output {
                Ok(out) => {
                    let title = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if title.is_empty() {
                        None
                    } else {
                        Some(title[..title.len().min(50)].to_string())
                    }
                }
                Err(_) => None,
            }
        },
    )
    .await;

    match result {
        Ok(Some(title)) => title,
        _ => {
            // Fallback: use first few words
            let words: Vec<&str> = first_message.split_whitespace().take(5).collect();
            let fallback = words.join(" ");
            if fallback.len() > 30 {
                format!("{}...", &fallback[..30])
            } else {
                fallback
            }
        }
    }
}

pub async fn archive_session(config: &Config, chat_id: i64) -> Option<String> {
    let current_path = config
        .session_dir
        .join(format!("telegram-{chat_id}.jsonl"));

    if tokio::fs::metadata(&current_path).await.is_err() {
        return None;
    }

    let timestamp = chrono_like_timestamp();
    let archive_name = format!("telegram-{chat_id}-{timestamp}.jsonl");
    let archive_path = config.session_dir.join(&archive_name);

    tokio::fs::rename(&current_path, &archive_path).await.ok()?;
    Some(archive_name)
}

pub async fn delete_session(session_path: &Path) -> Result<(), MiniClawError> {
    tokio::fs::remove_file(session_path).await?;
    Ok(())
}

pub async fn cleanup_old_sessions(config: &Config, keep_count: usize) -> usize {
    let sessions = list_sessions(config).await;

    // Group by chat ID
    let mut by_chat_id: HashMap<String, Vec<SessionInfo>> = HashMap::new();
    for session in sessions {
        by_chat_id
            .entry(session.chat_id.clone())
            .or_default()
            .push(session);
    }

    let mut deleted_count = 0;

    for (_, chat_sessions) in by_chat_id {
        // Already sorted newest first
        for session in chat_sessions.iter().skip(keep_count) {
            if delete_session(&session.path).await.is_ok() {
                deleted_count += 1;
            }
        }
    }

    deleted_count
}

pub fn format_session_age(time: SystemTime) -> String {
    let diff = SystemTime::now()
        .duration_since(time)
        .unwrap_or_default();

    let secs = diff.as_secs();
    let mins = secs / 60;
    let hours = secs / 3600;
    let days = secs / 86400;

    if mins < 1 {
        "just now".to_string()
    } else if mins < 60 {
        format!("{mins}m ago")
    } else if hours < 24 {
        format!("{hours}h ago")
    } else if days < 7 {
        format!("{days}d ago")
    } else {
        // Simple date format
        let (year, month, day) = days_to_ymd(
            time.duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                / 86400,
        );
        format!("{month}/{day}/{year}")
    }
}

pub fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_session_filename() {
        assert_eq!(
            SessionManager::default_session_filename(123),
            "telegram-123.jsonl"
        );
        assert_eq!(
            SessionManager::default_session_filename(-456),
            "telegram--456.jsonl"
        );
    }

    #[test]
    fn test_format_file_size_bytes() {
        assert_eq!(format_file_size(500), "500B");
    }

    #[test]
    fn test_format_file_size_kb() {
        assert_eq!(format_file_size(2048), "2.0KB");
    }

    #[test]
    fn test_format_file_size_mb() {
        assert_eq!(format_file_size(1048576), "1.0MB");
    }

    #[test]
    fn test_format_session_age_just_now() {
        let now = SystemTime::now();
        assert_eq!(format_session_age(now), "just now");
    }

    #[test]
    fn test_format_session_age_minutes() {
        let time =
            SystemTime::now() - std::time::Duration::from_secs(300);
        assert_eq!(format_session_age(time), "5m ago");
    }

    #[test]
    fn test_format_session_age_hours() {
        let time =
            SystemTime::now() - std::time::Duration::from_secs(7200);
        assert_eq!(format_session_age(time), "2h ago");
    }

    #[test]
    fn test_format_session_age_days() {
        let time =
            SystemTime::now() - std::time::Duration::from_secs(172800);
        assert_eq!(format_session_age(time), "2d ago");
    }

    #[test]
    fn test_chrono_like_timestamp_format() {
        let ts = chrono_like_timestamp();
        // Should match pattern: YYYY-MM-DDTHH-MM-SS-mmmZ
        assert!(ts.ends_with('Z'));
        assert!(ts.contains('T'));
        assert_eq!(ts.len(), 24);
    }

    #[test]
    fn test_days_to_ymd() {
        // 2025-01-01 is day 20089 since epoch
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_is_leap_year() {
        assert!(is_leap_year(2000));
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2023));
    }
}
