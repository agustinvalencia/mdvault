//! Vault index for fast queries on notes and links.
//!
//! This module provides SQLite-based indexing for:
//! - Note metadata (path, type, title, frontmatter)
//! - Links between notes (wikilinks, markdown links, frontmatter refs)
//! - Temporal activity (when notes are referenced in dailies)
//!
//! # Example
//!
//! ```no_run
//! use mdvault_core::index::{IndexDb, IndexedNote, NoteType, NoteQuery};
//! use std::path::Path;
//!
//! let db = IndexDb::open(Path::new(".mdvault/index.db")).unwrap();
//!
//! // Query all tasks
//! let query = NoteQuery {
//!     note_type: Some(NoteType::Task),
//!     ..Default::default()
//! };
//! let tasks = db.query_notes(&query).unwrap();
//! ```

pub mod db;
pub mod schema;
pub mod types;

pub use db::{IndexDb, IndexError};
pub use schema::{SchemaError, SCHEMA_VERSION};
pub use types::{
    ActivitySummary, IndexedLink, IndexedNote, LinkType, NoteQuery, NoteType, ProjectStatus,
    TaskStatus, TemporalActivity,
};
