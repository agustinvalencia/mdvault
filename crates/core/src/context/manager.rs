//! Context manager for persistent focus state.

use std::fs;
use std::path::{Path, PathBuf};

use crate::context::types::{ContextState, FocusContext};

/// Error type for context operations.
#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("Failed to read context state: {0}")]
    Read(#[from] std::io::Error),

    #[error("Failed to parse context state: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("Failed to serialize context state: {0}")]
    Serialize(#[from] toml::ser::Error),
}

type Result<T> = std::result::Result<T, ContextError>;

/// Manages persistent focus context state.
///
/// State is stored in `.mdvault/state/context.toml` within the vault.
#[derive(Debug)]
pub struct ContextManager {
    /// Path to the context state file.
    state_path: PathBuf,

    /// Current context state.
    state: ContextState,
}

impl ContextManager {
    /// State file location relative to vault root.
    const STATE_DIR: &'static str = ".mdvault/state";
    const STATE_FILE: &'static str = "context.toml";

    /// Load context manager for a vault.
    ///
    /// Creates the state file if it doesn't exist.
    pub fn load(vault_root: &Path) -> Result<Self> {
        let state_dir = vault_root.join(Self::STATE_DIR);
        let state_path = state_dir.join(Self::STATE_FILE);

        let state = if state_path.exists() {
            let content = fs::read_to_string(&state_path)?;
            toml::from_str(&content)?
        } else {
            ContextState::default()
        };

        Ok(Self { state_path, state })
    }

    /// Save current state to disk.
    pub fn save(&self) -> Result<()> {
        // Ensure state directory exists
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(&self.state)?;
        fs::write(&self.state_path, content)?;
        Ok(())
    }

    /// Set focus to a project.
    ///
    /// Replaces any existing focus.
    pub fn set_focus(&mut self, project: &str) -> Result<()> {
        self.state.focus = Some(FocusContext::new(project));
        self.save()
    }

    /// Set focus with an optional note.
    pub fn set_focus_with_note(&mut self, project: &str, note: &str) -> Result<()> {
        self.state.focus = Some(FocusContext::with_note(project, note));
        self.save()
    }

    /// Clear the current focus.
    pub fn clear_focus(&mut self) -> Result<()> {
        self.state.focus = None;
        self.save()
    }

    /// Get the active project ID, if any.
    pub fn active_project(&self) -> Option<&str> {
        self.state.focus.as_ref().map(|f| f.project.as_str())
    }

    /// Get the full focus context, if any.
    pub fn focus(&self) -> Option<&FocusContext> {
        self.state.focus.as_ref()
    }

    /// Get the current state (for serialization to MCP).
    pub fn state(&self) -> &ContextState {
        &self.state
    }

    /// Check if there is an active focus.
    pub fn has_focus(&self) -> bool {
        self.state.focus.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_creates_default_state() {
        let tmp = tempdir().unwrap();
        let manager = ContextManager::load(tmp.path()).unwrap();

        assert!(!manager.has_focus());
        assert!(manager.active_project().is_none());
    }

    #[test]
    fn test_set_and_clear_focus() {
        let tmp = tempdir().unwrap();
        let mut manager = ContextManager::load(tmp.path()).unwrap();

        // Set focus
        manager.set_focus("MCP").unwrap();
        assert!(manager.has_focus());
        assert_eq!(manager.active_project(), Some("MCP"));

        // Verify persisted
        let manager2 = ContextManager::load(tmp.path()).unwrap();
        assert_eq!(manager2.active_project(), Some("MCP"));

        // Clear focus
        manager.clear_focus().unwrap();
        assert!(!manager.has_focus());

        // Verify cleared
        let manager3 = ContextManager::load(tmp.path()).unwrap();
        assert!(!manager3.has_focus());
    }

    #[test]
    fn test_set_focus_with_note() {
        let tmp = tempdir().unwrap();
        let mut manager = ContextManager::load(tmp.path()).unwrap();

        manager
            .set_focus_with_note("VAULT", "Implementing context state")
            .unwrap();

        let focus = manager.focus().unwrap();
        assert_eq!(focus.project, "VAULT");
        assert_eq!(focus.note.as_deref(), Some("Implementing context state"));
        assert!(focus.started_at.is_some());
    }

    #[test]
    fn test_state_file_location() {
        let tmp = tempdir().unwrap();
        let mut manager = ContextManager::load(tmp.path()).unwrap();

        manager.set_focus("TEST").unwrap();

        let expected_path = tmp.path().join(".mdvault/state/context.toml");
        assert!(expected_path.exists());

        let content = fs::read_to_string(&expected_path).unwrap();
        assert!(content.contains("project = \"TEST\""));
    }
}
