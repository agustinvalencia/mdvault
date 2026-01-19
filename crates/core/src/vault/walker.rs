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
    /// Folders to exclude from walking (relative paths from vault root).
    excluded_folders: Vec<PathBuf>,
}

impl VaultWalker {
    /// Create a new walker for the given vault root.
    pub fn new(root: &Path) -> Result<Self, VaultWalkerError> {
        Self::with_exclusions(root, Vec::new())
    }

    /// Create a new walker with folder exclusions.
    ///
    /// Excluded folders can be specified as:
    /// - Relative paths from vault root (e.g., "automations/templates")
    /// - Absolute paths (will be converted to relative)
    pub fn with_exclusions(
        root: &Path,
        excluded_folders: Vec<PathBuf>,
    ) -> Result<Self, VaultWalkerError> {
        let root = root
            .canonicalize()
            .map_err(|_| VaultWalkerError::MissingRoot(root.display().to_string()))?;

        if !root.exists() {
            return Err(VaultWalkerError::MissingRoot(root.display().to_string()));
        }

        // Normalize exclusions to be relative to root
        let excluded_folders = excluded_folders
            .into_iter()
            .map(|p| {
                if p.is_absolute() {
                    p.strip_prefix(&root).unwrap_or(&p).to_path_buf()
                } else {
                    p
                }
            })
            .collect();

        Ok(Self { root, excluded_folders })
    }

    /// Walk the vault and return all markdown files.
    /// Excludes hidden directories, common non-vault directories, and configured exclusions.
    pub fn walk(&self) -> Result<Vec<WalkedFile>, VaultWalkerError> {
        let mut files = Vec::new();

        for entry in WalkDir::new(&self.root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !self.is_excluded(e))
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

    /// Check if an entry should be excluded from walking.
    fn is_excluded(&self, entry: &walkdir::DirEntry) -> bool {
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
        if matches!(name.as_ref(), "node_modules" | "target" | "__pycache__" | "venv") {
            return true;
        }

        // Check against configured exclusions
        if !self.excluded_folders.is_empty()
            && let Ok(relative) = entry.path().strip_prefix(&self.root)
        {
            for excluded in &self.excluded_folders {
                // Check if the entry's path starts with the excluded folder
                if relative.starts_with(excluded) {
                    return true;
                }
            }
        }

        false
    }

    /// Get the vault root path.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e == "md")
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

    #[test]
    fn test_walk_with_exclusions() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create some markdown files in various directories
        fs::write(root.join("note1.md"), "# Note 1").unwrap();

        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("templates/task.md"), "# Task Template").unwrap();

        fs::create_dir_all(root.join("automations/templates")).unwrap();
        fs::write(root.join("automations/templates/meeting.md"), "# Meeting").unwrap();

        fs::create_dir_all(root.join("projects")).unwrap();
        fs::write(root.join("projects/proj.md"), "# Project").unwrap();

        // Walk without exclusions - should find all 4 files
        let walker = VaultWalker::new(root).unwrap();
        let files = walker.walk().unwrap();
        assert_eq!(files.len(), 4);

        // Walk with exclusions - should skip templates and automations
        let excluded = vec![PathBuf::from("templates"), PathBuf::from("automations")];
        let walker = VaultWalker::with_exclusions(root, excluded).unwrap();
        let files = walker.walk().unwrap();

        assert_eq!(files.len(), 2);

        let paths: Vec<_> =
            files.iter().map(|f| f.relative_path.to_string_lossy().to_string()).collect();

        assert!(paths.contains(&"note1.md".to_string()));
        assert!(paths.contains(&"projects/proj.md".to_string()));
        assert!(!paths.iter().any(|p| p.contains("templates")));
        assert!(!paths.iter().any(|p| p.contains("automations")));
    }

    #[test]
    fn test_walk_with_nested_exclusion() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create nested structure
        fs::create_dir_all(root.join("docs/internal")).unwrap();
        fs::write(root.join("docs/readme.md"), "# Docs").unwrap();
        fs::write(root.join("docs/internal/secret.md"), "# Secret").unwrap();

        fs::write(root.join("note.md"), "# Note").unwrap();

        // Exclude only docs/internal, not all of docs
        let excluded = vec![PathBuf::from("docs/internal")];
        let walker = VaultWalker::with_exclusions(root, excluded).unwrap();
        let files = walker.walk().unwrap();

        assert_eq!(files.len(), 2);

        let paths: Vec<_> =
            files.iter().map(|f| f.relative_path.to_string_lossy().to_string()).collect();

        assert!(paths.contains(&"note.md".to_string()));
        assert!(paths.contains(&"docs/readme.md".to_string()));
        assert!(!paths.iter().any(|p| p.contains("internal")));
    }
}
