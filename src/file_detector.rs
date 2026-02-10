use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::SystemTime;

static IMAGE_EXTENSIONS: LazyLock<HashSet<&str>> =
    LazyLock::new(|| [".png", ".jpg", ".jpeg", ".gif", ".webp"].into());

static DOCUMENT_EXTENSIONS: LazyLock<HashSet<&str>> = LazyLock::new(|| {
    [
        ".pdf", ".txt", ".md", ".json", ".csv", ".html", ".xml", ".yaml", ".yml",
    ]
    .into()
});

#[derive(Debug, Clone)]
pub struct DetectedFile {
    pub path: PathBuf,
    pub filename: String,
    pub file_type: DetectedFileType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DetectedFileType {
    Photo,
    Document,
}

pub fn parse_output_for_files(output: &str) -> Vec<PathBuf> {
    static PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
        vec![
            regex::Regex::new(
                r#"(?i)(?:Created|Saved to|Wrote|Output|File saved|Generated|Exported):\s*([^\s]+\.\w+)"#,
            )
            .unwrap(),
            regex::Regex::new(
                r#"(?i)(?:saved|wrote|created|generated|exported)\s+(?:to\s+)?["']?([^\s"']+\.\w+)["']?"#,
            )
            .unwrap(),
            regex::Regex::new(r#"(?i)(?:file|output):\s*["']?([^\s"']+\.\w+)["']?"#).unwrap(),
        ]
    });

    let mut files = HashSet::new();

    for pattern in PATTERNS.iter() {
        for caps in pattern.captures_iter(output) {
            if let Some(m) = caps.get(1) {
                let path = m.as_str();
                if path.starts_with('/') {
                    files.insert(PathBuf::from(path));
                }
            }
        }
    }

    files.into_iter().collect()
}

pub async fn snapshot_workspace(workspace: &Path) -> HashMap<PathBuf, u128> {
    let mut files = HashMap::new();
    let Ok(mut entries) = tokio::fs::read_dir(workspace).await else {
        return files;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(ft) = entry.file_type().await else {
            continue;
        };
        if ft.is_file() {
            let path = entry.path();
            if let Ok(meta) = tokio::fs::metadata(&path).await {
                if let Ok(mtime) = meta.modified() {
                    let ms = mtime
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    files.insert(path, ms);
                }
            }
        }
    }

    files
}

pub async fn detect_new_files(
    workspace: &Path,
    before: &HashMap<PathBuf, u128>,
) -> Vec<PathBuf> {
    let after = snapshot_workspace(workspace).await;
    let mut new_files = Vec::new();

    for (path, mtime) in &after {
        match before.get(path) {
            None => new_files.push(path.clone()),
            Some(before_mtime) if mtime > before_mtime => new_files.push(path.clone()),
            _ => {}
        }
    }

    new_files
}

pub fn categorize_files(file_paths: &[PathBuf]) -> Vec<DetectedFile> {
    let mut result = Vec::new();

    for path in file_paths {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_lowercase()))
            .unwrap_or_default();

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if IMAGE_EXTENSIONS.contains(ext.as_str()) {
            result.push(DetectedFile {
                path: path.clone(),
                filename,
                file_type: DetectedFileType::Photo,
            });
        } else if DOCUMENT_EXTENSIONS.contains(ext.as_str()) {
            result.push(DetectedFile {
                path: path.clone(),
                filename,
                file_type: DetectedFileType::Document,
            });
        }
    }

    result
}

pub async fn detect_files(
    output: &str,
    workspace: &Path,
    before_snapshot: &HashMap<PathBuf, u128>,
) -> Vec<DetectedFile> {
    let parsed_files = parse_output_for_files(output);
    let new_files = detect_new_files(workspace, before_snapshot).await;

    let mut all_files: HashSet<PathBuf> = HashSet::new();
    for f in parsed_files {
        all_files.insert(f);
    }
    for f in new_files {
        all_files.insert(f);
    }

    categorize_files(&all_files.into_iter().collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_output_created() {
        let output = "Created: /tmp/test.png\nDone.";
        let files = parse_output_for_files(output);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], PathBuf::from("/tmp/test.png"));
    }

    #[test]
    fn test_parse_output_saved_to() {
        let output = "Saved to: /home/user/file.pdf";
        let files = parse_output_for_files(output);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], PathBuf::from("/home/user/file.pdf"));
    }

    #[test]
    fn test_parse_output_no_absolute_paths() {
        let output = "Created: relative/file.txt";
        let files = parse_output_for_files(output);
        assert!(files.is_empty());
    }

    #[test]
    fn test_parse_output_multiple_files() {
        let output = "Created: /tmp/a.png\nWrote: /tmp/b.txt";
        let files = parse_output_for_files(output);
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_categorize_photo() {
        let files = categorize_files(&[PathBuf::from("/tmp/image.png")]);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_type, DetectedFileType::Photo);
        assert_eq!(files[0].filename, "image.png");
    }

    #[test]
    fn test_categorize_document() {
        let files = categorize_files(&[PathBuf::from("/tmp/doc.pdf")]);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_type, DetectedFileType::Document);
    }

    #[test]
    fn test_categorize_unsupported_skipped() {
        let files = categorize_files(&[PathBuf::from("/tmp/file.exe")]);
        assert!(files.is_empty());
    }

    #[test]
    fn test_categorize_case_insensitive() {
        let files = categorize_files(&[PathBuf::from("/tmp/photo.JPG")]);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_type, DetectedFileType::Photo);
    }

    #[tokio::test]
    async fn test_snapshot_nonexistent_dir() {
        let snap = snapshot_workspace(Path::new("/nonexistent/dir")).await;
        assert!(snap.is_empty());
    }

    #[tokio::test]
    async fn test_detect_new_files_empty() {
        let dir = tempfile::tempdir().unwrap();
        let before = snapshot_workspace(dir.path()).await;
        let new = detect_new_files(dir.path(), &before).await;
        assert!(new.is_empty());
    }

    #[tokio::test]
    async fn test_detect_new_files_created() {
        let dir = tempfile::tempdir().unwrap();
        let before = snapshot_workspace(dir.path()).await;

        // Create a new file
        tokio::fs::write(dir.path().join("new.txt"), "hello").await.unwrap();

        let new = detect_new_files(dir.path(), &before).await;
        assert_eq!(new.len(), 1);
        assert!(new[0].ends_with("new.txt"));
    }
}
