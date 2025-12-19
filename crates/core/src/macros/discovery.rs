//! Macro discovery and repository.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;
use walkdir::WalkDir;

use super::types::{LoadedMacro, MacroInfo, MacroSpec};

/// Error type for macro discovery.
#[derive(Debug, Error)]
pub enum MacroDiscoveryError {
    #[error("macros directory does not exist: {0}")]
    MissingDir(String),

    #[error("failed to read macros directory {0}: {1}")]
    WalkError(String, #[source] walkdir::Error),
}

/// Error type for macro repository operations.
#[derive(Debug, Error)]
pub enum MacroRepoError {
    #[error(transparent)]
    Discovery(#[from] MacroDiscoveryError),

    #[error("macro not found: {0}")]
    NotFound(String),

    #[error("failed to read macro file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse macro YAML {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
}

/// Discover macro files in a directory.
///
/// Finds all `.yaml` files in the given directory and its subdirectories.
pub fn discover_macros(root: &Path) -> Result<Vec<MacroInfo>, MacroDiscoveryError> {
    let root = root
        .canonicalize()
        .map_err(|_| MacroDiscoveryError::MissingDir(root.display().to_string()))?;

    if !root.exists() {
        return Err(MacroDiscoveryError::MissingDir(root.display().to_string()));
    }

    let mut out = Vec::new();

    for entry in WalkDir::new(&root) {
        let entry = entry
            .map_err(|e| MacroDiscoveryError::WalkError(root.display().to_string(), e))?;

        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !is_yaml_file(path) {
            continue;
        }

        let rel = path.strip_prefix(&root).unwrap_or(path);
        let logical = logical_name_from_relative(rel);

        out.push(MacroInfo { logical_name: logical, path: path.to_path_buf() });
    }

    out.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));
    Ok(out)
}

fn is_yaml_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e == "yaml" || e == "yml")
}

fn logical_name_from_relative(rel: &Path) -> String {
    let s = rel.to_string_lossy();
    // Remove .yaml or .yml extension
    if let Some(stripped) = s.strip_suffix(".yaml") {
        return stripped.to_string();
    }
    if let Some(stripped) = s.strip_suffix(".yml") {
        return stripped.to_string();
    }
    s.to_string()
}

/// Repository for discovering and loading macros.
pub struct MacroRepository {
    pub root: PathBuf,
    pub macros: Vec<MacroInfo>,
}

impl MacroRepository {
    /// Create a new macro repository from a directory.
    pub fn new(root: &Path) -> Result<Self, MacroDiscoveryError> {
        let macros = discover_macros(root)?;
        Ok(Self { root: root.to_path_buf(), macros })
    }

    /// List all discovered macros.
    pub fn list_all(&self) -> &[MacroInfo] {
        &self.macros
    }

    /// Get a macro by its logical name.
    pub fn get_by_name(&self, name: &str) -> Result<LoadedMacro, MacroRepoError> {
        let info = self
            .macros
            .iter()
            .find(|m| m.logical_name == name)
            .ok_or_else(|| MacroRepoError::NotFound(name.to_string()))?;

        let content = fs::read_to_string(&info.path)
            .map_err(|e| MacroRepoError::Io { path: info.path.clone(), source: e })?;

        let spec: MacroSpec = serde_yaml::from_str(&content)
            .map_err(|e| MacroRepoError::Parse { path: info.path.clone(), source: e })?;

        Ok(LoadedMacro {
            logical_name: info.logical_name.clone(),
            path: info.path.clone(),
            spec,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_discover_macros() {
        let temp = TempDir::new().unwrap();
        let macros_dir = temp.path().join("macros");
        fs::create_dir_all(&macros_dir).unwrap();

        // Create some macro files
        fs::write(
            macros_dir.join("weekly-review.yaml"),
            "name: weekly-review\nsteps: []",
        )
        .unwrap();
        fs::write(macros_dir.join("daily-note.yml"), "name: daily-note\nsteps: []")
            .unwrap();

        // Create a subdirectory with another macro
        let sub_dir = macros_dir.join("project");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("setup.yaml"), "name: project-setup\nsteps: []").unwrap();

        // Create a non-yaml file (should be ignored)
        fs::write(macros_dir.join("notes.md"), "# Notes").unwrap();

        let macros = discover_macros(&macros_dir).unwrap();

        assert_eq!(macros.len(), 3);
        assert!(macros.iter().any(|m| m.logical_name == "daily-note"));
        assert!(macros.iter().any(|m| m.logical_name == "weekly-review"));
        assert!(macros.iter().any(|m| m.logical_name == "project/setup"));
    }

    #[test]
    fn test_macro_repository() {
        let temp = TempDir::new().unwrap();
        let macros_dir = temp.path().join("macros");
        fs::create_dir_all(&macros_dir).unwrap();

        fs::write(
            macros_dir.join("test.yaml"),
            r#"
name: test
description: A test macro
steps:
  - template: meeting-note
    with:
      title: "Test"
"#,
        )
        .unwrap();

        let repo = MacroRepository::new(&macros_dir).unwrap();
        assert_eq!(repo.list_all().len(), 1);

        let loaded = repo.get_by_name("test").unwrap();
        assert_eq!(loaded.spec.name, "test");
        assert_eq!(loaded.spec.description, "A test macro");
        assert_eq!(loaded.spec.steps.len(), 1);
    }

    #[test]
    fn test_macro_not_found() {
        let temp = TempDir::new().unwrap();
        let macros_dir = temp.path().join("macros");
        fs::create_dir_all(&macros_dir).unwrap();

        let repo = MacroRepository::new(&macros_dir).unwrap();
        let result = repo.get_by_name("nonexistent");

        assert!(matches!(result, Err(MacroRepoError::NotFound(_))));
    }
}
