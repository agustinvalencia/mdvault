//! Macro discovery and repository.

use std::path::{Path, PathBuf};

use thiserror::Error;
use walkdir::WalkDir;

use super::lua_loader::load_macro_from_lua;
use super::types::{LoadedMacro, MacroFormat, MacroInfo};

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

    #[error("failed to parse macro Lua {path}: {source}")]
    LuaParse {
        path: PathBuf,
        #[source]
        source: crate::scripting::ScriptingError,
    },

    #[error("invalid macro definition in {path}: {message}")]
    LuaInvalid { path: PathBuf, message: String },
}

/// Discover Lua macro files in a directory.
pub fn discover_macros(root: &Path) -> Result<Vec<MacroInfo>, MacroDiscoveryError> {
    let root = root
        .canonicalize()
        .map_err(|_| MacroDiscoveryError::MissingDir(root.display().to_string()))?;

    if !root.exists() {
        return Err(MacroDiscoveryError::MissingDir(root.display().to_string()));
    }

    let mut macros: Vec<MacroInfo> = Vec::new();

    for entry in WalkDir::new(&root) {
        let entry = entry
            .map_err(|e| MacroDiscoveryError::WalkError(root.display().to_string(), e))?;

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Only process .lua files
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) if n.ends_with(".lua") => n,
            _ => continue,
        };

        let logical = name.strip_suffix(".lua").unwrap().to_string();

        let rel = path.strip_prefix(&root).unwrap_or(path);
        let full_logical =
            if rel.parent().map(|p| p.as_os_str().is_empty()).unwrap_or(true) {
                logical
            } else {
                let parent = rel.parent().unwrap().to_string_lossy();
                format!("{}/{}", parent, logical)
            };

        macros.push(MacroInfo {
            logical_name: full_logical,
            path: path.to_path_buf(),
            format: MacroFormat::Lua,
        });
    }

    macros.sort_by(|a, b| a.logical_name.cmp(&b.logical_name));
    Ok(macros)
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

        let spec = load_macro_from_lua(&info.path)?;

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
    fn test_discover_macros_lua() {
        let temp = TempDir::new().unwrap();
        let macros_dir = temp.path().join("macros");
        fs::create_dir_all(&macros_dir).unwrap();

        fs::write(macros_dir.join("weekly-review.lua"), "return {}").unwrap();
        fs::write(macros_dir.join("daily-note.lua"), "return {}").unwrap();
        // Non-lua files should be ignored
        fs::write(macros_dir.join("notes.md"), "# Notes").unwrap();
        fs::write(macros_dir.join("old.yaml"), "name: old\nsteps: []").unwrap(); // YAML no longer supported

        let macros = discover_macros(&macros_dir).unwrap();

        assert_eq!(macros.len(), 2);
        assert!(
            macros
                .iter()
                .any(|m| m.logical_name == "daily-note" && m.format == MacroFormat::Lua)
        );
        assert!(
            macros.iter().any(
                |m| m.logical_name == "weekly-review" && m.format == MacroFormat::Lua
            )
        );
    }

    #[test]
    fn test_discover_macros_nested() {
        let temp = TempDir::new().unwrap();
        let macros_dir = temp.path().join("macros");
        fs::create_dir_all(&macros_dir).unwrap();

        // Create a subdirectory with macros
        let sub_dir = macros_dir.join("project");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("setup.lua"), "return {}").unwrap();
        fs::write(sub_dir.join("teardown.lua"), "return {}").unwrap();

        let macros = discover_macros(&macros_dir).unwrap();

        assert_eq!(macros.len(), 2);
        assert!(macros.iter().any(|m| m.logical_name == "project/setup"));
        assert!(macros.iter().any(|m| m.logical_name == "project/teardown"));
    }

    #[test]
    fn test_macro_repository() {
        let temp = TempDir::new().unwrap();
        let macros_dir = temp.path().join("macros");
        fs::create_dir_all(&macros_dir).unwrap();

        fs::write(
            macros_dir.join("test.lua"),
            r#"
return {
    name = "test",
    description = "A test macro",
    steps = {
        { template = "meeting-note", ["with"] = { title = "Test" } }
    }
}
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
