//! Hook types and errors.
//!
//! This module provides types for hook execution, including the note context
//! passed to hooks and error types for hook failures.

use std::path::PathBuf;

use thiserror::Error;

/// Note context passed to Lua hooks.
///
/// Contains all information about a note that hooks might need to access.
#[derive(Debug, Clone)]
pub struct NoteContext {
    /// Path to the note file (relative to vault root).
    pub path: PathBuf,
    /// Note type from frontmatter (e.g., "task", "meeting").
    pub note_type: String,
    /// Parsed frontmatter as YAML value.
    pub frontmatter: serde_yaml::Value,
    /// Full content of the note (including frontmatter).
    pub content: String,
    /// Template variables used to render the note (as a map).
    pub variables: serde_yaml::Value,
}

impl NoteContext {
    /// Create a new NoteContext.
    pub fn new(
        path: PathBuf,
        note_type: String,
        frontmatter: serde_yaml::Value,
        content: String,
        variables: serde_yaml::Value,
    ) -> Self {
        Self { path, note_type, frontmatter, content, variables }
    }
}

/// Errors that can occur during hook execution.
#[derive(Debug, Error)]
pub enum HookError {
    /// Template not found.
    #[error("template not found: {0}")]
    TemplateNotFound(String),

    /// Capture not found.
    #[error("capture not found: {0}")]
    CaptureNotFound(String),

    /// Macro not found.
    #[error("macro not found: {0}")]
    MacroNotFound(String),

    /// Hook execution failed.
    #[error("hook execution failed: {0}")]
    Execution(String),

    /// Lua runtime error.
    #[error("Lua error: {0}")]
    LuaError(String),

    /// Template rendering error.
    #[error("template render error: {0}")]
    TemplateRender(String),

    /// Capture execution error.
    #[error("capture execution error: {0}")]
    CaptureExecution(String),

    /// Macro execution error.
    #[error("macro execution error: {0}")]
    MacroExecution(String),

    /// IO error during hook execution.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
