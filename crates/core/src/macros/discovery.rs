//! Macro discovery and repository.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;
use walkdir::WalkDir;

use super::lua_loader::load_macro_from_lua;
use super::types::{LoadedMacro, MacroFormat, MacroInfo, MacroSpec};

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

    #[error("failed to parse macro Lua {path}: {source}")]
    LuaParse {
        path: PathBuf,
        #[source]
        source: crate::scripting::ScriptingError,
    },

    #[error("invalid macro definition in {path}: {message}")]
    LuaInvalid { path: PathBuf, message: String },
}

/// Discover macro files in a directory.
///
/// Finds all `.lua` and `.yaml` files in the given directory and its subdirectories.
/// Lua files take precedence over YAML files with the same name.
pub fn discover_macros(root: &Path) -> Result<Vec<MacroInfo>, MacroDiscoveryError> {
    let root = root
        .canonicalize()
        .map_err(|_| MacroDiscoveryError::MissingDir(root.display().to_string()))?;

    if !root.exists() {
        return Err(MacroDiscoveryError::MissingDir(root.display().to_string()));
    }

    // Use a map to handle Lua/YAML precedence (Lua wins)
    let mut macros: HashMap<String, MacroInfo> = HashMap::new();

    for entry in WalkDir::new(&root) {
        let entry = entry
            .map_err(|e| MacroDiscoveryError::WalkError(root.display().to_string(), e))?;

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let (logical, format) = match get_macro_format(path) {
            Some((l, f)) => (l, f),
            None => continue,
        };

        let rel = path.strip_prefix(&root).unwrap_or(path);
        let full_logical = if rel.parent().map(|p| p.as_os_str().is_empty()).unwrap_or(true) {
            logical.clone()
        } else {
            let parent = rel.parent().unwrap().to_string_lossy();
            format!("{}/{}", parent, logical)
        };

        // Lua takes precedence over YAML
        match macros.get(&full_logical) {
            Some(existing) if existing.format == MacroFormat::Lua => {
                // Keep existing Lua file
                continue;
            }
            _ => {
                macros.insert(
                    full_logical.clone(),
                    MacroInfo {
                        logical_name: full_logical,
                        path: path.to_path_buf(),
                        format,
                    },
                );
            }
        }
    }

    let mut out: Vec<MacroInfo> = macros.into_values().collect();
    out.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));
    Ok(out)
}

/// Get the macro format and logical name from a file path.
/// Returns None if the file is not a macro file.
fn get_macro_format(path: &Path) -> Option<(String, MacroFormat)> {
    let name = path.file_name().and_then(|s| s.to_str())?;

    if name.ends_with(".lua") {
        let logical = name.strip_suffix(".lua")?.to_string();
        Some((logical, MacroFormat::Lua))
    } else if name.ends_with(".yaml") {
        let logical = name.strip_suffix(".yaml")?.to_string();
        Some((logical, MacroFormat::Yaml))
    } else if name.ends_with(".yml") {
        let logical = name.strip_suffix(".yml")?.to_string();
        Some((logical, MacroFormat::Yaml))
    } else {
        None
    }
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

        let spec = match info.format {
            MacroFormat::Lua => load_macro_from_lua(&info.path)?,
            MacroFormat::Yaml => {
                // Emit deprecation warning for YAML macros
                eprintln!(
                    "warning: YAML macros are deprecated. Please migrate '{}' to Lua format.",
                    info.path.display()
                );

                let content = fs::read_to_string(&info.path)
                    .map_err(|e| MacroRepoError::Io { path: info.path.clone(), source: e })?;

                serde_yaml::from_str::<MacroSpec>(&content)
                    .map_err(|e| MacroRepoError::Parse { path: info.path.clone(), source: e })?
            }
        };

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
    fn test_discover_macros_yaml() {
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
        assert!(macros.iter().any(|m| m.logical_name == "daily-note" && m.format == MacroFormat::Yaml));
        assert!(macros.iter().any(|m| m.logical_name == "weekly-review" && m.format == MacroFormat::Yaml));
        assert!(macros.iter().any(|m| m.logical_name == "project/setup" && m.format == MacroFormat::Yaml));
    }

    #[test]
    fn test_discover_macros_lua() {
        let temp = TempDir::new().unwrap();
        let macros_dir = temp.path().join("macros");
        fs::create_dir_all(&macros_dir).unwrap();

        fs::write(macros_dir.join("weekly-review.lua"), "return {}").unwrap();
        fs::write(macros_dir.join("daily-note.lua"), "return {}").unwrap();

        let macros = discover_macros(&macros_dir).unwrap();

        assert_eq!(macros.len(), 2);
        assert!(macros.iter().any(|m| m.logical_name == "daily-note" && m.format == MacroFormat::Lua));
        assert!(macros.iter().any(|m| m.logical_name == "weekly-review" && m.format == MacroFormat::Lua));
    }

    #[test]
    fn test_discover_macros_lua_precedence() {
        let temp = TempDir::new().unwrap();
        let macros_dir = temp.path().join("macros");
        fs::create_dir_all(&macros_dir).unwrap();

        // Both Lua and YAML with same name - Lua should win
        fs::write(macros_dir.join("test.lua"), "return {}").unwrap();
        fs::write(macros_dir.join("test.yaml"), "name: test\nsteps: []").unwrap();
        fs::write(macros_dir.join("yaml-only.yaml"), "name: yaml-only\nsteps: []").unwrap();

        let macros = discover_macros(&macros_dir).unwrap();

        assert_eq!(macros.len(), 2);

        let test_macro = macros.iter().find(|m| m.logical_name == "test").unwrap();
        assert_eq!(test_macro.format, MacroFormat::Lua);

        let yaml_only = macros.iter().find(|m| m.logical_name == "yaml-only").unwrap();
        assert_eq!(yaml_only.format, MacroFormat::Yaml);
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
