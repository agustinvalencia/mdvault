//! Context state types.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Root context state structure.
///
/// Serialized to `.mdvault/state/context.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextState {
    /// Current focus context (if any).
    #[serde(default)]
    pub focus: Option<FocusContext>,
}

/// Active focus context.
///
/// Represents what the user is currently working on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusContext {
    /// Active project ID (e.g., "MCP", "VAULT").
    pub project: String,

    /// When the focus was set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Local>>,

    /// Optional description of the current work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl FocusContext {
    /// Create a new focus context.
    pub fn new(project: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            started_at: Some(Local::now()),
            note: None,
        }
    }

    /// Create a focus context with a note.
    pub fn with_note(project: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            started_at: Some(Local::now()),
            note: Some(note.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_context_new() {
        let focus = FocusContext::new("MCP");
        assert_eq!(focus.project, "MCP");
        assert!(focus.started_at.is_some());
        assert!(focus.note.is_none());
    }

    #[test]
    fn test_focus_context_with_note() {
        let focus = FocusContext::with_note("MCP", "Working on context");
        assert_eq!(focus.project, "MCP");
        assert_eq!(focus.note.as_deref(), Some("Working on context"));
    }

    #[test]
    fn test_context_state_serialization() {
        let state = ContextState {
            focus: Some(FocusContext::new("TEST")),
        };

        let toml = toml::to_string_pretty(&state).unwrap();
        assert!(toml.contains("project = \"TEST\""));

        let parsed: ContextState = toml::from_str(&toml).unwrap();
        assert_eq!(parsed.focus.unwrap().project, "TEST");
    }

    #[test]
    fn test_empty_context_state() {
        let state = ContextState::default();
        assert!(state.focus.is_none());

        let toml = toml::to_string_pretty(&state).unwrap();
        let parsed: ContextState = toml::from_str(&toml).unwrap();
        assert!(parsed.focus.is_none());
    }
}
