//! Domain traits for note type behaviors.
//!
//! These traits define the polymorphic interface for first-class note types,
//! allowing type-specific behavior without scattered if/else checks.

use std::path::PathBuf;

use super::context::{CreationContext, FieldPrompt, PromptContext};

/// Errors that can occur during domain operations.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("ID generation failed: {0}")]
    IdGeneration(String),

    #[error("path resolution failed: {0}")]
    PathResolution(String),

    #[error("lifecycle hook failed: {0}")]
    LifecycleHook(String),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("{0}")]
    Other(String),
}

pub type DomainResult<T> = Result<T, DomainError>;

/// How does this note type generate identifiers and paths?
pub trait NoteIdentity {
    /// Generate a unique ID for this note type.
    /// Returns None if this type doesn't use IDs (e.g., daily notes use dates).
    fn generate_id(&self, ctx: &CreationContext) -> DomainResult<Option<String>>;

    /// Determine the output path for this note.
    /// This is called after prompts are collected and before_create runs.
    fn output_path(&self, ctx: &CreationContext) -> DomainResult<PathBuf>;

    /// Get the fields that this type manages (core metadata).
    /// These fields will be preserved through template/hook modifications.
    fn core_fields(&self) -> Vec<&'static str>;
}

/// What happens during the note lifecycle?
pub trait NoteLifecycle {
    /// Called before the note is written to disk.
    /// Can modify the context (e.g., set computed vars, update counters).
    fn before_create(&self, ctx: &mut CreationContext) -> DomainResult<()>;

    /// Called after the note is successfully written to disk.
    /// Used for side effects (logging to daily, reindexing, etc.).
    fn after_create(&self, ctx: &CreationContext, content: &str) -> DomainResult<()>;
}

/// What interactive prompts does this type need?
pub trait NotePrompts {
    /// Return type-specific prompts (e.g., project selector for tasks).
    /// These run BEFORE schema-based prompts.
    fn type_prompts(&self, ctx: &PromptContext) -> Vec<FieldPrompt>;

    /// Whether this type should prompt for schema fields.
    /// Default: true. Override to false for types that compute all fields.
    fn should_prompt_schema(&self) -> bool {
        true
    }
}

/// Combined trait for a note type behavior.
/// All first-class types implement this.
pub trait NoteBehavior: NoteIdentity + NoteLifecycle + NotePrompts + Send + Sync {
    /// Get the type name for this behavior.
    fn type_name(&self) -> &'static str;
}
