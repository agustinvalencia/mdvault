//! Activity logging for vault operations.
//!
//! This module provides infrastructure for logging all `mdv` operations to a
//! structured JSONL file (`.mdvault/activity.jsonl`) for later aggregation
//! by the `context` command.

mod rotation;
mod service;
mod types;

pub use rotation::rotate_log;
pub use service::{ActivityError, ActivityLogService};
pub use types::{ActivityEntry, Operation};
