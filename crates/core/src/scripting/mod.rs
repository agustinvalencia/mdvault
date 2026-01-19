//! Lua scripting support for mdvault.
//!
//! This module provides a sandboxed Lua environment with access to
//! mdvault's date math and template rendering engines.
//!
//! # Overview
//!
//! The scripting layer allows users to define custom validation logic,
//! type definitions, and automation rules in Lua while having access
//! to mdvault's core functionality.
//!
//! # Example
//!
//! ```rust
//! use mdvault_core::scripting::LuaEngine;
//!
//! let engine = LuaEngine::sandboxed().unwrap();
//!
//! // Use date math
//! let date = engine.eval_string(r#"mdv.date("today + 7d")"#).unwrap();
//! println!("One week from now: {}", date);
//!
//! // Render templates
//! let greeting = engine.eval_string(
//!     r#"mdv.render("Hello {{name}}!", { name = "World" })"#
//! ).unwrap();
//! println!("{}", greeting);
//! ```
//!
//! # Available Lua Functions
//!
//! The `mdv` global table provides:
//!
//! - `mdv.date(expr, format?)` - Evaluate date math expressions
//! - `mdv.render(template, context)` - Render templates with variables
//! - `mdv.is_date_expr(str)` - Check if a string is a date expression
//!
//! With vault context (via `LuaEngine::with_vault_context`):
//! - `mdv.template(name, vars?)` - Render a template by name
//! - `mdv.capture(name, vars?)` - Execute a capture workflow
//! - `mdv.macro(name, vars?)` - Execute a macro workflow
//! - `mdv.read_note(path)` - Read a note's content and frontmatter
//! - `mdv.current_note()` - Get the current note being processed
//! - `mdv.backlinks(path)` - Get notes linking to a path
//! - `mdv.outlinks(path)` - Get notes a path links to
//! - `mdv.query(opts)` - Query the vault index
//! - `mdv.selector(opts)` - Show interactive fuzzy selector for notes of a type
//!
//! # Security
//!
//! By default, the Lua environment is sandboxed to prevent:
//! - File system access (`io` library removed)
//! - Shell command execution (`os` library removed)
//! - Loading external modules (`require` removed)
//! - Arbitrary code loading (`load`, `loadfile`, `dofile` removed)
//! - Debug library access (`debug` removed)

pub mod bindings;
pub mod engine;
pub mod hook_runner;
pub mod hooks;
pub mod index_bindings;
pub mod selector;
pub mod types;
pub mod vault_bindings;
pub mod vault_context;

pub use engine::LuaEngine;
pub use hook_runner::{
    HookResult, UpdateHookResult, run_on_create_hook, run_on_update_hook,
};
pub use hooks::{HookError, NoteContext};
pub use selector::{SelectorCallback, SelectorItem, SelectorOptions};
pub use types::{SandboxConfig, ScriptingError};
pub use vault_context::{CurrentNote, VaultContext};
