use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

use crate::config::Config;

#[derive(Debug)]
pub struct RunResult {
    pub output: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActivityType {
    Thinking,
    Reading,
    Writing,
    Running,
    Searching,
    Working,
}

#[derive(Debug, Clone)]
pub struct ActivityUpdate {
    pub activity_type: ActivityType,
    pub detail: String,
    pub elapsed: u64,
}

#[derive(Default)]
pub struct RunPiOptions {
    pub image_paths: Vec<PathBuf>,
}


// Per-chat locking to prevent concurrent Pi executions
pub struct ChatLocks {
    locks: Mutex<HashMap<i64, Arc<Mutex<()>>>>,
}

impl ChatLocks {
    pub fn new() -> Self {
        Self {
            locks: Mutex::new(HashMap::new()),
        }
    }

    pub async fn acquire(&self, chat_id: i64) -> tokio::sync::OwnedMutexGuard<()> {
        let mutex = {
            let mut locks = self.locks.lock().await;
            locks
                .entry(chat_id)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        mutex.lock_owned().await
    }
}

fn detect_activity(line: &str) -> Option<(ActivityType, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    static READING: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)^(?:Reading|Read)\s+(.+)").unwrap());
    static WRITING: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)^(?:Writing|Wrote|Creating|Created)\s+(.+)").unwrap()
    });
    static RUNNING: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)^(?:Running|Executing|>\s*\$)\s*(.+)").unwrap()
    });
    static SEARCHING: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)^(?:Searching|Search|Looking|Finding)").unwrap()
    });
    static THINKING: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)^(?:Thinking|Analyzing|Processing)").unwrap()
    });

    if let Some(caps) = READING.captures(trimmed) {
        return Some((
            ActivityType::Reading,
            caps.get(1).map_or("file", |m| m.as_str()).to_string(),
        ));
    }
    if let Some(caps) = WRITING.captures(trimmed) {
        return Some((
            ActivityType::Writing,
            caps.get(1).map_or("file", |m| m.as_str()).to_string(),
        ));
    }
    if let Some(caps) = RUNNING.captures(trimmed) {
        let detail = caps
            .get(1)
            .map_or("command", |m| &m.as_str()[..m.as_str().len().min(50)]);
        return Some((ActivityType::Running, detail.to_string()));
    }
    if SEARCHING.is_match(trimmed) {
        return Some((ActivityType::Searching, "codebase".to_string()));
    }
    if THINKING.is_match(trimmed) {
        return Some((ActivityType::Thinking, String::new()));
    }

    None
}

fn get_session_path(config: &Config, chat_id: i64) -> PathBuf {
    config.session_dir.join(format!("telegram-{chat_id}.jsonl"))
}

pub async fn check_pi_auth() -> bool {
    match Command::new("pi")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

pub async fn run_pi_with_streaming<F>(
    config: &Config,
    chat_id: i64,
    prompt: &str,
    workspace: &str,
    on_activity: F,
    options: Option<RunPiOptions>,
) -> RunResult
where
    F: Fn(ActivityUpdate) + Send + Sync + 'static,
{
    let start = Instant::now();

    // Ensure session directory exists
    if let Err(e) = tokio::fs::create_dir_all(&config.session_dir).await {
        return RunResult {
            output: String::new(),
            error: Some(format!("Failed to create session dir: {e}")),
        };
    }

    let session_path = get_session_path(config, chat_id);

    let mut args = vec![
        "--session".to_string(),
        session_path.to_string_lossy().to_string(),
        "--print".to_string(),
        "--thinking".to_string(),
        config.thinking_level.to_string(),
    ];

    // Add image paths with @ prefix
    if let Some(opts) = &options {
        for image_path in &opts.image_paths {
            args.push(format!("@{}", image_path.display()));
        }
    }

    args.push(prompt.to_string());

    let home = dirs::home_dir().unwrap_or_default();
    let pi_agent_dir = home.join(".pi").join("agent");

    let mut child = match Command::new("pi")
        .args(&args)
        .current_dir(workspace)
        .env("PI_AGENT_DIR", &pi_agent_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return RunResult {
                output: String::new(),
                error: Some(format!("Failed to start Pi: {e}")),
            };
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let on_activity = Arc::new(on_activity);
    let on_activity_clone = on_activity.clone();

    // Read stdout line-by-line
    let stdout_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        let mut output = String::new();
        let mut last_activity_elapsed: u64 = 0;
        let start = Instant::now();

        while let Ok(Some(line)) = reader.next_line().await {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&line);

            if let Some((activity_type, detail)) = detect_activity(&line) {
                let elapsed = start.elapsed().as_secs();
                last_activity_elapsed = elapsed;
                on_activity_clone(ActivityUpdate {
                    activity_type,
                    detail,
                    elapsed,
                });
            }

            let _ = last_activity_elapsed; // suppress unused warning
        }

        output
    });

    // Read stderr
    let stderr_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut error_output = String::new();

        while let Ok(Some(line)) = reader.next_line().await {
            if !error_output.is_empty() {
                error_output.push('\n');
            }
            error_output.push_str(&line);
        }

        error_output
    });

    // Periodic "working" updates
    let on_activity_periodic = on_activity.clone();
    let timeout_ms = config.pi_timeout_ms;
    let periodic_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.tick().await; // skip first immediate tick
        loop {
            interval.tick().await;
            let elapsed = start.elapsed().as_secs();
            on_activity_periodic(ActivityUpdate {
                activity_type: ActivityType::Working,
                detail: String::new(),
                elapsed,
            });
        }
    });

    // Wait for process with timeout
    let result = tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait()).await;

    periodic_handle.abort();

    let (stdout_output, stderr_output) = tokio::join!(stdout_handle, stderr_handle);
    let stdout_output = stdout_output.unwrap_or_default();
    let stderr_output = stderr_handle_result(stderr_output);

    match result {
        Ok(Ok(status)) => {
            if !status.success() && !stderr_output.is_empty() {
                RunResult {
                    output: if stdout_output.is_empty() {
                        "Error occurred".to_string()
                    } else {
                        stdout_output
                    },
                    error: Some(stderr_output),
                }
            } else {
                RunResult {
                    output: if stdout_output.is_empty() {
                        "(no output)".to_string()
                    } else {
                        stdout_output
                    },
                    error: None,
                }
            }
        }
        Ok(Err(e)) => RunResult {
            output: stdout_output,
            error: Some(format!("Pi process error: {e}")),
        },
        Err(_) => {
            // Timeout - kill the process
            let _ = child.kill().await;
            RunResult {
                output: stdout_output,
                error: Some("Timeout: Pi took too long".to_string()),
            }
        }
    }
}

fn stderr_handle_result(result: Result<String, tokio::task::JoinError>) -> String {
    result.unwrap_or_default()
}

pub async fn get_session_line_count(config: &Config, chat_id: i64) -> usize {
    let session_path = get_session_path(config, chat_id);
    match tokio::fs::read_to_string(&session_path).await {
        Ok(content) => content.trim().lines().count(),
        Err(_) => 0,
    }
}

#[derive(Debug)]
pub struct ExtractedImage {
    pub data: Vec<u8>,
    pub mime_type: String,
}

pub async fn extract_images_from_session(
    config: &Config,
    chat_id: i64,
    after_line: usize,
) -> Vec<ExtractedImage> {
    let session_path = get_session_path(config, chat_id);
    let mut images = Vec::new();

    let Ok(content) = tokio::fs::read_to_string(&session_path).await else {
        return images;
    };

    let lines: Vec<&str> = content.trim().lines().collect();
    let new_lines = &lines[after_line.min(lines.len())..];

    for line in new_lines {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        if entry.get("type").and_then(|t| t.as_str()) != Some("message") {
            continue;
        }

        let Some(message) = entry.get("message") else {
            continue;
        };

        if message.get("role").and_then(|r| r.as_str()) != Some("toolResult") {
            continue;
        }

        let Some(content_arr) = message.get("content").and_then(|c| c.as_array()) else {
            continue;
        };

        for item in content_arr {
            if item.get("type").and_then(|t| t.as_str()) != Some("image") {
                continue;
            }

            // Format 1: { type: "image", data: "<base64>", mimeType: "..." }
            if let (Some(data_str), Some(mime)) = (
                item.get("data").and_then(|d| d.as_str()),
                item.get("mimeType").and_then(|m| m.as_str()),
            ) {
                if let Ok(decoded) =
                    base64::engine::general_purpose::STANDARD.decode(data_str)
                {
                    images.push(ExtractedImage {
                        data: decoded,
                        mime_type: mime.to_string(),
                    });
                }
                continue;
            }

            // Format 2: { type: "image", source: { type: "base64", media_type: "...", data: "..." } }
            if let Some(source) = item.get("source") {
                if source.get("type").and_then(|t| t.as_str()) == Some("base64") {
                    if let Some(data_str) = source.get("data").and_then(|d| d.as_str()) {
                        let mime = source
                            .get("media_type")
                            .and_then(|m| m.as_str())
                            .unwrap_or("image/png");
                        if let Ok(decoded) =
                            base64::engine::general_purpose::STANDARD.decode(data_str)
                        {
                            images.push(ExtractedImage {
                                data: decoded,
                                mime_type: mime.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    images
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_activity_reading() {
        let result = detect_activity("Reading package.json");
        assert!(result.is_some());
        let (t, d) = result.unwrap();
        assert_eq!(t, ActivityType::Reading);
        assert_eq!(d, "package.json");
    }

    #[test]
    fn test_detect_activity_writing() {
        let result = detect_activity("Writing src/main.rs");
        assert!(result.is_some());
        let (t, _) = result.unwrap();
        assert_eq!(t, ActivityType::Writing);
    }

    #[test]
    fn test_detect_activity_running() {
        let result = detect_activity("Running npm test");
        assert!(result.is_some());
        let (t, d) = result.unwrap();
        assert_eq!(t, ActivityType::Running);
        assert_eq!(d, "npm test");
    }

    #[test]
    fn test_detect_activity_searching() {
        let result = detect_activity("Searching for references");
        assert!(result.is_some());
        let (t, _) = result.unwrap();
        assert_eq!(t, ActivityType::Searching);
    }

    #[test]
    fn test_detect_activity_thinking() {
        let result = detect_activity("Thinking about the problem");
        assert!(result.is_some());
        let (t, _) = result.unwrap();
        assert_eq!(t, ActivityType::Thinking);
    }

    #[test]
    fn test_detect_activity_none() {
        assert!(detect_activity("").is_none());
        assert!(detect_activity("  ").is_none());
        assert!(detect_activity("Hello world").is_none());
    }

    #[test]
    fn test_detect_activity_case_insensitive() {
        assert!(detect_activity("READING file.txt").is_some());
        assert!(detect_activity("read file.txt").is_some());
    }

    #[test]
    fn test_chat_locks_new() {
        let _locks = ChatLocks::new();
    }
}
