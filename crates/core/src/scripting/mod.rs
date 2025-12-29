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
pub mod types;

pub use engine::LuaEngine;
pub use types::{SandboxConfig, ScriptingError};
