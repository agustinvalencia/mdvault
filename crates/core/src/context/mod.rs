//! Context state management for focus mode.
//!
//! The `ContextManager` maintains persistent focus state across CLI/TUI/MCP layers.
//! State is stored per-vault in `.mdvault/state/context.toml`.

mod manager;
mod types;

pub use manager::ContextManager;
pub use types::{ContextState, FocusContext};
