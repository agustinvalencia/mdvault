//! Context state management for focus mode.
//!
//! The `ContextManager` maintains persistent focus state across CLI/TUI/MCP layers.
//! State is stored per-vault in `.mdvault/state/context.toml`.
//!
//! This module also provides context query services for day/week aggregation.

mod manager;
mod query;
mod query_types;
mod types;

pub use manager::ContextManager;
pub use query::ContextQueryService;
pub use query_types::{
    ActivityItem, ContextError, DailyNoteInfo, DayContext, DaySummary,
    DaySummaryWithDate, FocusContextOutput, LinkInfo, ModifiedNote, NoteActivity,
    NoteContext, NoteReferences, ProjectActivity, RecentTasks, TaskActivity, TaskCounts,
    TaskInfo, WeekContext, WeekSummary,
};
pub use types::{ContextState, FocusContext};
