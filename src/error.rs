use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum MiniClawError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Pi execution error: {0}")]
    PiExecution(String),

    #[error("Pi not authenticated or not installed")]
    PiNotAuthenticated,

    #[error("Session error: {0}")]
    Session(String),

    #[error("Workspace error: {0}")]
    Workspace(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Timeout after {0} seconds")]
    Timeout(u64),
}
