//! Lua-based type definitions for note validation.
//!
//! This module provides a system for defining custom note types with:
//! - Field schemas (required fields, types, constraints)
//! - Custom validation functions
//! - Lifecycle hooks (on_create, on_update)
//!
//! Type definitions are loaded from Lua files in `~/.config/mdvault/types/`.
//!
//! # Example Type Definition
//!
//! ```lua
//! -- ~/.config/mdvault/types/meeting.lua
//! return {
//!     name = "meeting",
//!     description = "Meeting notes with attendees",
//!
//!     schema = {
//!         attendees = { type = "list", required = true },
//!         status = { type = "string", enum = { "scheduled", "completed" } },
//!     },
//!
//!     validate = function(note)
//!         if note.frontmatter.status == "completed" and not note.frontmatter.summary then
//!             return false, "Completed meetings must have a summary"
//!         end
//!         return true
//!     end,
//! }
//! ```

pub mod definition;
pub mod discovery;
pub mod errors;
pub mod registry;
pub mod schema;
pub mod validation;

// Re-export commonly used types
pub use definition::{TypeDefinition, TypedefInfo};
pub use discovery::TypedefRepository;
pub use errors::{TypedefError, ValidationError, ValidationResult};
pub use registry::TypeRegistry;
pub use schema::{FieldSchema, FieldType};
pub use validation::validate_note;
