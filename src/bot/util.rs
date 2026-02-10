use std::path::Path;

use tokio::process::Command;
use tokio::time::Duration;

const MAX_MESSAGE_LENGTH: usize = 4096;

pub fn split_message(text: &str) -> Vec<String> {
    if text.len() <= MAX_MESSAGE_LENGTH {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= MAX_MESSAGE_LENGTH {
            chunks.push(remaining.to_string());
            break;
        }

        // Try to split at newline
        let search_range = &remaining[..MAX_MESSAGE_LENGTH];
        let mut split_index = search_range.rfind('\n').unwrap_or(0);

        if split_index == 0 || split_index < MAX_MESSAGE_LENGTH / 2 {
            // Fall back to space
            split_index = search_range.rfind(' ').unwrap_or(0);
        }

        if split_index == 0 || split_index < MAX_MESSAGE_LENGTH / 2 {
            // Hard split
            split_index = MAX_MESSAGE_LENGTH;
        }

        chunks.push(remaining[..split_index].to_string());
        remaining = remaining[split_index..].trim_start();
    }

    chunks
}

pub struct ShellResult {
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
}

pub async fn run_shell(cmd: &str, cwd: &Path, timeout_ms: u64) -> ShellResult {
    let result = tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        Command::new("bash")
            .arg("-c")
            .arg(cmd)
            .current_dir(cwd)
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => ShellResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            code: output.status.code(),
        },
        Ok(Err(e)) => ShellResult {
            stdout: String::new(),
            stderr: e.to_string(),
            code: Some(1),
        },
        Err(_) => ShellResult {
            stdout: String::new(),
            stderr: "(timeout)".to_string(),
            code: Some(124),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_short_message() {
        let chunks = split_message("hello");
        assert_eq!(chunks, vec!["hello"]);
    }

    #[test]
    fn test_split_at_newline() {
        let text = format!("{}\n{}", "a".repeat(3000), "b".repeat(3000));
        let chunks = split_message(&text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "a".repeat(3000));
        assert_eq!(chunks[1], "b".repeat(3000));
    }

    #[test]
    fn test_split_at_space() {
        let text = format!("{} {}", "a".repeat(3000), "b".repeat(3000));
        let chunks = split_message(&text);
        assert_eq!(chunks.len(), 2);
    }

    #[test]
    fn test_split_hard() {
        let text = "a".repeat(5000);
        let chunks = split_message(&text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), MAX_MESSAGE_LENGTH);
    }

    #[test]
    fn test_split_empty() {
        let chunks = split_message("");
        assert_eq!(chunks, vec![""]);
    }

    #[test]
    fn test_split_exactly_max() {
        let text = "a".repeat(MAX_MESSAGE_LENGTH);
        let chunks = split_message(&text);
        assert_eq!(chunks.len(), 1);
    }

    #[tokio::test]
    async fn test_run_shell_echo() {
        let dir = tempfile::tempdir().unwrap();
        let result = run_shell("echo hello", dir.path(), 5000).await;
        assert_eq!(result.stdout.trim(), "hello");
        assert_eq!(result.code, Some(0));
    }

    #[tokio::test]
    async fn test_run_shell_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = run_shell("false", dir.path(), 5000).await;
        assert_ne!(result.code, Some(0));
    }
}
