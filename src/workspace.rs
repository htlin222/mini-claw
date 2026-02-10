use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::MiniClawError;

pub struct WorkspaceManager {
    state: HashMap<String, PathBuf>,
    state_file: PathBuf,
    loaded: bool,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        Self {
            state: HashMap::new(),
            state_file: home.join(".mini-claw").join("workspaces.json"),
            loaded: false,
        }
    }

    async fn load(&mut self) {
        if self.loaded {
            return;
        }
        if let Ok(data) = tokio::fs::read_to_string(&self.state_file).await {
            if let Ok(parsed) = serde_json::from_str::<HashMap<String, String>>(&data) {
                self.state = parsed
                    .into_iter()
                    .map(|(k, v)| (k, PathBuf::from(v)))
                    .collect();
            }
        }
        self.loaded = true;
    }

    async fn save(&self) -> Result<(), MiniClawError> {
        if let Some(dir) = self.state_file.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let map: HashMap<&str, &str> = self
            .state
            .iter()
            .filter_map(|(k, v)| v.to_str().map(|s| (k.as_str(), s)))
            .collect();
        let json = serde_json::to_string_pretty(&map)?;
        tokio::fs::write(&self.state_file, json).await?;
        Ok(())
    }

    pub async fn get_workspace(&mut self, chat_id: i64) -> PathBuf {
        self.load().await;
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let key = chat_id.to_string();

        if let Some(path) = self.state.get(&key) {
            if tokio::fs::metadata(path).await.map(|m| m.is_dir()).unwrap_or(false) {
                return path.clone();
            }
        }

        home
    }

    pub async fn set_workspace(
        &mut self,
        chat_id: i64,
        path: &str,
    ) -> Result<PathBuf, MiniClawError> {
        self.load().await;
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));

        let resolved = if path.starts_with('~') {
            home.join(path.trim_start_matches('~').trim_start_matches('/'))
        } else if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            // Relative path: resolve from current workspace
            let current = self.get_workspace(chat_id).await;
            current.join(path).canonicalize().map_err(|_| {
                MiniClawError::Workspace(format!("Directory not found: {path}"))
            })?
        };

        // Verify directory exists
        match tokio::fs::metadata(&resolved).await {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                return Err(MiniClawError::Workspace(format!(
                    "Not a directory: {}",
                    resolved.display()
                )));
            }
            Err(_) => {
                return Err(MiniClawError::Workspace(format!(
                    "Directory not found: {}",
                    resolved.display()
                )));
            }
        }

        self.state.insert(chat_id.to_string(), resolved.clone());
        self.save().await?;
        Ok(resolved)
    }

    pub fn format_path(path: &Path) -> String {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let home_str = home.to_string_lossy();
        let path_str = path.to_string_lossy();

        if path == home {
            "~".to_string()
        } else if path_str.starts_with(&*home_str) {
            format!("~{}", &path_str[home_str.len()..])
        } else {
            path_str.into_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_path_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(WorkspaceManager::format_path(&home), "~");
    }

    #[test]
    fn test_format_path_subdirectory() {
        let home = dirs::home_dir().unwrap();
        let path = home.join("projects");
        assert_eq!(WorkspaceManager::format_path(&path), "~/projects");
    }

    #[test]
    fn test_format_path_absolute() {
        let path = PathBuf::from("/etc/config");
        assert_eq!(WorkspaceManager::format_path(&path), "/etc/config");
    }

    #[tokio::test]
    async fn test_get_workspace_default_home() {
        let mut mgr = WorkspaceManager::new();
        mgr.loaded = true; // Skip file loading
        let ws = mgr.get_workspace(999).await;
        let home = dirs::home_dir().unwrap();
        assert_eq!(ws, home);
    }

    #[tokio::test]
    async fn test_set_workspace_tilde() {
        let mut mgr = WorkspaceManager::new();
        mgr.loaded = true;
        // Use a temp dir to test
        let dir = tempfile::tempdir().unwrap();
        // We can't test ~ expansion easily since it needs to be a real dir
        // but we can test absolute path
        let result = mgr.set_workspace(123, dir.path().to_str().unwrap()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dir.path());
    }

    #[tokio::test]
    async fn test_set_workspace_nonexistent() {
        let mut mgr = WorkspaceManager::new();
        mgr.loaded = true;
        let result = mgr.set_workspace(123, "/nonexistent/dir/xyz").await;
        assert!(result.is_err());
    }
}
