//! Domain module for note type behaviors.
//!
//! This module provides trait-based polymorphic dispatch for first-class note types,
//! replacing scattered if/else checks with clean trait implementations.
//!
//! # Architecture
//!
//! - **Traits** (`traits.rs`): Define the behavior interfaces (NoteIdentity, NoteLifecycle, NotePrompts)
//! - **Context** (`context.rs`): Define the context types passed through the creation lifecycle
//! - **Behaviors** (`behaviors/`): Implementations for each first-class type
//!
//! # Usage
//!
//! ```ignore
//! use mdvault_core::domain::{NoteType, CreationContext};
//!
//! let note_type = NoteType::from_name("task", &registry)?;
//! let behavior = note_type.behavior();
//!
//! // Use polymorphic dispatch
//! behavior.before_create(&mut ctx)?;
//! let path = behavior.output_path(&ctx)?;
//! ```

pub mod behaviors;
pub mod context;
pub mod creator;
pub mod services;
pub mod traits;

pub use behaviors::{
    CustomBehavior, DailyBehavior, MeetingBehavior, ProjectBehavior, TaskBehavior,
    WeeklyBehavior, ZettelBehavior, find_project_file, task_belongs_to_project,
};
pub use context::{
    CoreMetadata, CreationContext, FieldPrompt, PromptContext, PromptType,
};
pub use creator::{CreationResult, NoteCreator};
pub use services::DailyLogService;
pub use traits::{
    DomainError, DomainResult, NoteBehavior, NoteIdentity, NoteLifecycle, NotePrompts,
};

use crate::types::TypeRegistry;

/// Enumeration of all note types with their behaviors.
pub enum NoteType {
    Task(TaskBehavior),
    Project(ProjectBehavior),
    Daily(DailyBehavior),
    Weekly(WeeklyBehavior),
    Meeting(MeetingBehavior),
    Zettel(ZettelBehavior),
    Custom(CustomBehavior),
}

impl NoteType {
    /// Create a NoteType from a type name, using the registry to find typedef overrides.
    pub fn from_name(name: &str, registry: &TypeRegistry) -> DomainResult<Self> {
        let typedef = registry.get(name);

        match name.to_lowercase().as_str() {
            "task" => Ok(NoteType::Task(TaskBehavior::new(typedef))),
            "project" => Ok(NoteType::Project(ProjectBehavior::new(typedef))),
            "daily" => Ok(NoteType::Daily(DailyBehavior::new(typedef))),
            "weekly" => Ok(NoteType::Weekly(WeeklyBehavior::new(typedef))),
            "meeting" => Ok(NoteType::Meeting(MeetingBehavior::new(typedef))),
            "zettel" | "knowledge" => Ok(NoteType::Zettel(ZettelBehavior::new(typedef))),
            _ => {
                // Custom type - must have a typedef
                let td = typedef.ok_or_else(|| {
                    DomainError::Other(format!(
                        "Unknown type: {}. No typedef found.",
                        name
                    ))
                })?;
                Ok(NoteType::Custom(CustomBehavior::new(td)))
            }
        }
    }

    /// Get a reference to the behavior trait object.
    pub fn behavior(&self) -> &dyn NoteBehavior {
        match self {
            NoteType::Task(b) => b,
            NoteType::Project(b) => b,
            NoteType::Daily(b) => b,
            NoteType::Weekly(b) => b,
            NoteType::Meeting(b) => b,
            NoteType::Zettel(b) => b,
            NoteType::Custom(b) => b,
        }
    }

    /// Get a mutable reference to the behavior for lifecycle methods.
    pub fn behavior_mut(&mut self) -> &mut dyn NoteBehavior {
        match self {
            NoteType::Task(b) => b,
            NoteType::Project(b) => b,
            NoteType::Daily(b) => b,
            NoteType::Weekly(b) => b,
            NoteType::Meeting(b) => b,
            NoteType::Zettel(b) => b,
            NoteType::Custom(b) => b,
        }
    }

    /// Get the type name.
    pub fn type_name(&self) -> &str {
        match self {
            NoteType::Task(_) => "task",
            NoteType::Project(_) => "project",
            NoteType::Daily(_) => "daily",
            NoteType::Weekly(_) => "weekly",
            NoteType::Meeting(_) => "meeting",
            NoteType::Zettel(_) => "zettel",
            NoteType::Custom(b) => &b.typedef().name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_type_from_name_builtins() {
        let registry = TypeRegistry::new();

        assert!(matches!(
            NoteType::from_name("task", &registry).unwrap(),
            NoteType::Task(_)
        ));
        assert!(matches!(
            NoteType::from_name("project", &registry).unwrap(),
            NoteType::Project(_)
        ));
        assert!(matches!(
            NoteType::from_name("daily", &registry).unwrap(),
            NoteType::Daily(_)
        ));
        assert!(matches!(
            NoteType::from_name("weekly", &registry).unwrap(),
            NoteType::Weekly(_)
        ));
        assert!(matches!(
            NoteType::from_name("meeting", &registry).unwrap(),
            NoteType::Meeting(_)
        ));
        assert!(matches!(
            NoteType::from_name("zettel", &registry).unwrap(),
            NoteType::Zettel(_)
        ));
        assert!(matches!(
            NoteType::from_name("knowledge", &registry).unwrap(),
            NoteType::Zettel(_)
        ));
    }

    #[test]
    fn test_note_type_unknown_fails() {
        let registry = TypeRegistry::new();
        assert!(NoteType::from_name("unknown_type", &registry).is_err());
    }
}
