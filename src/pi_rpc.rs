use std::collections::HashMap;
use std::path::Path;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::config::ThinkingLevel;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PiEvent {
    AgentStart,
    TextDelta(String),
    ThinkingDelta(String),
    ToolStart { name: String },
    ToolUpdate(String),
    ToolEnd,
    AgentEnd,
    Error(String),
}

pub struct PiRpcProcess {
    child: Child,
    stdin: BufWriter<tokio::process::ChildStdin>,
    event_rx: mpsc::UnboundedReceiver<PiEvent>,
    request_counter: u64,
    _reader_handle: tokio::task::JoinHandle<()>,
}

impl PiRpcProcess {
    async fn spawn(
        session_path: &Path,
        workspace: &Path,
        thinking_level: ThinkingLevel,
    ) -> anyhow::Result<Self> {
        let home = dirs::home_dir().unwrap_or_default();
        let pi_agent_dir = home.join(".pi").join("agent");

        let mut child = Command::new("pi")
            .arg("--mode")
            .arg("rpc")
            .arg("--session")
            .arg(session_path)
            .arg("--thinking")
            .arg(thinking_level.to_string())
            .current_dir(workspace)
            .env("PI_AGENT_DIR", &pi_agent_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        let stdout = child.stdout.take().ok_or_else(|| {
            anyhow::anyhow!("Failed to capture Pi stdout")
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            anyhow::anyhow!("Failed to capture Pi stdin")
        })?;

        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Spawn stdout reader task
        let reader_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let event = parse_rpc_event(&line);
                if event_tx.send(event).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            event_rx,
            request_counter: 0,
            _reader_handle: reader_handle,
        })
    }

    pub async fn send_prompt(&mut self, message: &str) -> anyhow::Result<()> {
        self.request_counter += 1;
        let cmd = serde_json::json!({
            "id": format!("req-{}", self.request_counter),
            "type": "prompt",
            "message": message,
        });
        let mut line = serde_json::to_string(&cmd)?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn send_steer(&mut self, message: &str) -> anyhow::Result<()> {
        self.request_counter += 1;
        let cmd = serde_json::json!({
            "id": format!("req-{}", self.request_counter),
            "type": "steer",
            "message": message,
        });
        let mut line = serde_json::to_string(&cmd)?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn send_follow_up(&mut self, message: &str) -> anyhow::Result<()> {
        self.request_counter += 1;
        let cmd = serde_json::json!({
            "id": format!("req-{}", self.request_counter),
            "type": "follow_up",
            "message": message,
        });
        let mut line = serde_json::to_string(&cmd)?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    pub async fn recv_event(&mut self) -> Option<PiEvent> {
        self.event_rx.recv().await
    }

    pub fn is_alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }

    pub async fn kill(&mut self) {
        let _ = self.child.kill().await;
    }
}

impl Drop for PiRpcProcess {
    fn drop(&mut self) {
        self._reader_handle.abort();
    }
}

fn parse_rpc_event(line: &str) -> PiEvent {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return PiEvent::Error(format!("Invalid JSON: {line}"));
    };

    let event_type = value
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    match event_type {
        "agent_start" => PiEvent::AgentStart,
        "agent_end" => PiEvent::AgentEnd,
        "message_update" => {
            if let Some(evt) = value.get("assistantMessageEvent") {
                let delta_type = evt.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let delta = evt
                    .get("delta")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();

                match delta_type {
                    "text_delta" => PiEvent::TextDelta(delta),
                    "thinking_delta" => PiEvent::ThinkingDelta(delta),
                    "toolcall_start" => {
                        let name = evt
                            .get("partial")
                            .and_then(|p| p.get("name"))
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        PiEvent::ToolStart { name }
                    }
                    "toolcall_delta" => PiEvent::ToolUpdate(delta),
                    "done" => PiEvent::AgentEnd,
                    "error" => {
                        let reason = evt
                            .get("reason")
                            .and_then(|r| r.as_str())
                            .unwrap_or("unknown error")
                            .to_string();
                        PiEvent::Error(reason)
                    }
                    _ => PiEvent::TextDelta(String::new()),
                }
            } else {
                PiEvent::TextDelta(String::new())
            }
        }
        "tool_execution_start" => {
            let name = value
                .get("tool")
                .and_then(|t| t.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("tool")
                .to_string();
            PiEvent::ToolStart { name }
        }
        "tool_execution_update" => {
            let output = value
                .get("output")
                .and_then(|o| o.as_str())
                .unwrap_or("")
                .to_string();
            PiEvent::ToolUpdate(output)
        }
        "tool_execution_end" => PiEvent::ToolEnd,
        "error" => {
            let msg = value
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error")
                .to_string();
            PiEvent::Error(msg)
        }
        _ => PiEvent::TextDelta(String::new()), // ignore unknown events
    }
}

/// Manages persistent live Pi RPC sessions per chat
pub struct LiveSessionManager {
    sessions: HashMap<i64, PiRpcProcess>,
}

impl LiveSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub async fn start_session(
        &mut self,
        chat_id: i64,
        session_path: &Path,
        workspace: &Path,
        thinking_level: ThinkingLevel,
    ) -> anyhow::Result<()> {
        // Kill existing session if any
        self.stop_session(chat_id).await;

        let process = PiRpcProcess::spawn(session_path, workspace, thinking_level).await?;
        self.sessions.insert(chat_id, process);
        Ok(())
    }

    pub async fn send_prompt(&mut self, chat_id: i64, message: &str) -> anyhow::Result<()> {
        let process = self
            .sessions
            .get_mut(&chat_id)
            .ok_or_else(|| anyhow::anyhow!("No live session for chat {chat_id}"))?;
        process.send_prompt(message).await
    }

    #[allow(dead_code)]
    pub async fn send_steer(&mut self, chat_id: i64, message: &str) -> anyhow::Result<()> {
        let process = self
            .sessions
            .get_mut(&chat_id)
            .ok_or_else(|| anyhow::anyhow!("No live session for chat {chat_id}"))?;
        process.send_steer(message).await
    }

    pub async fn recv_event(&mut self, chat_id: i64) -> Option<PiEvent> {
        let process = self.sessions.get_mut(&chat_id)?;
        process.recv_event().await
    }

    pub fn is_active(&mut self, chat_id: i64) -> bool {
        self.sessions
            .get_mut(&chat_id)
            .map(|p| p.is_alive())
            .unwrap_or(false)
    }

    pub async fn stop_session(&mut self, chat_id: i64) {
        if let Some(mut process) = self.sessions.remove(&chat_id) {
            process.kill().await;
        }
    }

    #[allow(dead_code)]
    pub async fn stop_all(&mut self) {
        let ids: Vec<i64> = self.sessions.keys().copied().collect();
        for id in ids {
            self.stop_session(id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_start() {
        let event = parse_rpc_event(r#"{"type":"agent_start"}"#);
        assert!(matches!(event, PiEvent::AgentStart));
    }

    #[test]
    fn test_parse_agent_end() {
        let event = parse_rpc_event(r#"{"type":"agent_end","messages":[]}"#);
        assert!(matches!(event, PiEvent::AgentEnd));
    }

    #[test]
    fn test_parse_text_delta() {
        let event = parse_rpc_event(
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Hello"}}"#,
        );
        match event {
            PiEvent::TextDelta(text) => assert_eq!(text, "Hello"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_parse_tool_start() {
        let event = parse_rpc_event(r#"{"type":"tool_execution_start","tool":{"name":"bash"}}"#);
        match event {
            PiEvent::ToolStart { name } => assert_eq!(name, "bash"),
            _ => panic!("Expected ToolStart"),
        }
    }

    #[test]
    fn test_parse_error() {
        let event = parse_rpc_event(r#"{"type":"error","error":"something failed"}"#);
        match event {
            PiEvent::Error(msg) => assert_eq!(msg, "something failed"),
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_parse_invalid_json() {
        let event = parse_rpc_event("not json");
        assert!(matches!(event, PiEvent::Error(_)));
    }

    #[test]
    fn test_live_session_manager_new() {
        let mgr = LiveSessionManager::new();
        assert!(mgr.sessions.is_empty());
    }
}
