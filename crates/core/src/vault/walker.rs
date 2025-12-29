//! Recursive vault directory walker.

use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Error)]
pub enum VaultWalkerError {
    #[error("vault root does not exist: {0}")]
    MissingRoot(String),

    #[error("failed to walk vault directory {0}: {1}")]
    WalkError(String, #[source] walkdir::Error),

    #[error("failed to read file metadata {0}: {1}")]
    MetadataError(String, #[source] std::io::Error),
}

/// Information about a discovered markdown file.
#[derive(Debug, Clone)]
pub struct WalkedFile {
    /// Absolute path to the file.
    pub absolute_path: PathBuf,
    /// Path relative to vault root.
    pub relative_path: PathBuf,
    /// File modification time.
    pub modified: SystemTime,
    /// File size in bytes.
    pub size: u64,
}

/// Walker for discovering markdown files in a vault.
#[derive(Debug)]
pub struct VaultWalker {
    root: PathBuf,
}

impl VaultWalker {
    /// Create a new walker for the given vault root.
    pub fn new(root: &Path) -> Result<Self, VaultWalkerError> {
        let root = root
            .canonicalize()
            .map_err(|_| VaultWalkerError::MissingRoot(root.display().to_string()))?;

        if !root.exists() {
            return Err(VaultWalkerError::MissingRoot(root.display().to_string()));
        }

        Ok(Self { root })
    }

    /// Walk the vault and return all markdown files.
    /// Excludes hidden directories and common non-vault directories.
    pub fn walk(&self) -> Result<Vec<WalkedFile>, VaultWalkerError> {
        let mut files = Vec::new();

        for entry in WalkDir::new(&self.root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_hidden_or_ignored(e))
        {
            let entry = entry.map_err(|e| {
                VaultWalkerError::WalkError(self.root.display().to_string(), e)
            })?;

            let path = entry.path();
            if !path.is_file() || !is_markdown_file(path) {
                continue;
            }

            let metadata = path.metadata().map_err(|e| {
                VaultWalkerError::MetadataError(path.display().to_string(), e)
            })?;

            let relative_path =
                path.strip_prefix(&self.root).unwrap_or(path).to_path_buf();

            files.push(WalkedFile {
                absolute_path: path.to_path_buf(),
                relative_path,
                modified: metadata.modified().unwrap_or(std::time::UNIX_EPOCH),
                size: metadata.len(),
            });
        }

        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(files)
    }

    /// Get the vault root path.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e == "md")
}

fn is_hidden_or_ignored(entry: &walkdir::DirEntry) -> bool {
    // Never filter the root directory (depth 0)
    if entry.depth() == 0 {
        return false;
    }

    let name = entry.file_name().to_string_lossy();

    // Skip hidden files and directories
    if name.starts_with('.') {
        return true;
    }

    // Skip common non-vault directories
    matches!(name.as_ref(), "node_modules" | "target" | "__pycache__" | "venv")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_vault() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create some markdown files
        fs::write(root.join("note1.md"), "# Note 1").unwrap();
        fs::write(root.join("note2.md"), "# Note 2").unwrap();

        // Create subdirectory with notes
        fs::create_dir(root.join("subdir")).unwrap();
        fs::write(root.join("subdir/note3.md"), "# Note 3").unwrap();

        // Create hidden directory (should be skipped)
        fs::create_dir(root.join(".hidden")).unwrap();
        fs::write(root.join(".hidden/secret.md"), "# Secret").unwrap();

        // Create non-markdown file (should be skipped)
        fs::write(root.join("readme.txt"), "Not markdown").unwrap();

        dir
    }

    #[test]
    fn test_walk_finds_markdown_files() {
        let vault = create_test_vault();
        let walker = VaultWalker::new(vault.path()).unwrap();
        let files = walker.walk().unwrap();

        assert_eq!(files.len(), 3);

        let paths: Vec<_> = files.iter().map(|f| f.relative_path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("note1.md")));
        assert!(paths.contains(&PathBuf::from("note2.md")));
        assert!(paths.contains(&PathBuf::from("subdir/note3.md")));
    }

    #[test]
    fn test_walk_skips_hidden_directories() {
        let vault = create_test_vault();
        let walker = VaultWalker::new(vault.path()).unwrap();
        let files = walker.walk().unwrap();

        let paths: Vec<_> =
            files.iter().map(|f| f.relative_path.to_string_lossy().to_string()).collect();

        assert!(!paths.iter().any(|p| p.contains(".hidden")));
    }

    #[test]
    fn test_walk_skips_non_markdown() {
        let vault = create_test_vault();
        let walker = VaultWalker::new(vault.path()).unwrap();
        let files = walker.walk().unwrap();

        let paths: Vec<_> =
            files.iter().map(|f| f.relative_path.to_string_lossy().to_string()).collect();

        assert!(!paths.iter().any(|p| p.contains("readme.txt")));
    }

    #[test]
    fn test_walk_results_sorted() {
        let vault = create_test_vault();
        let walker = VaultWalker::new(vault.path()).unwrap();
        let files = walker.walk().unwrap();

        let paths: Vec<_> = files.iter().map(|f| &f.relative_path).collect();
        let mut sorted = paths.clone();
        sorted.sort();

        assert_eq!(paths, sorted);
    }

    #[test]
    fn test_missing_root() {
        let result = VaultWalker::new(Path::new("/nonexistent/path"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VaultWalkerError::MissingRoot(_)));
    }
}
