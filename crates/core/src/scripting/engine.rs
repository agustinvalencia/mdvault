//! Lua scripting engine with sandboxing.
//!
//! This module provides a sandboxed Lua execution environment for
//! running user-defined scripts safely.

use mlua::{Lua, Result as LuaResult, StdLib, Value};

use super::bindings::register_mdv_table;
use super::types::{SandboxConfig, ScriptingError};
use super::vault_bindings::register_vault_bindings;
use super::vault_context::VaultContext;

/// A sandboxed Lua execution environment.
///
/// The engine provides access to mdvault functionality through the `mdv`
/// global table while restricting dangerous operations like file I/O
/// and shell execution.
///
/// # Example
///
/// ```rust
/// use mdvault_core::scripting::LuaEngine;
///
/// let engine = LuaEngine::sandboxed().unwrap();
/// let result = engine.eval_string(r#"mdv.date("today + 7d")"#).unwrap();
/// println!("One week from now: {}", result);
/// ```
pub struct LuaEngine {
    lua: Lua,
    #[allow(dead_code)]
    config: SandboxConfig,
}

impl LuaEngine {
    /// Create a new Lua engine with the given sandbox configuration.
    pub fn new(config: SandboxConfig) -> Result<Self, ScriptingError> {
        // Create Lua with restricted standard library
        // Note: base functions (print, type, tostring, etc.) are always available
        // We add: table, string, utf8, math
        let libs = StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH;

        let lua = Lua::new_with(libs, mlua::LuaOptions::default())?;

        // Apply memory limit if configured
        if config.memory_limit > 0 {
            lua.set_memory_limit(config.memory_limit)?;
        }

        // Remove dangerous globals
        Self::apply_sandbox(&lua)?;

        // Register mdv bindings
        register_mdv_table(&lua)?;

        Ok(Self { lua, config })
    }

    /// Create a new engine with default restrictive sandbox.
    pub fn sandboxed() -> Result<Self, ScriptingError> {
        Self::new(SandboxConfig::restricted())
    }

    /// Create a new Lua engine with vault context for hook execution.
    ///
    /// This provides access to `mdv.template()`, `mdv.capture()`, `mdv.macro()`
    /// in addition to the standard sandboxed bindings.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use mdvault_core::scripting::{LuaEngine, VaultContext, SandboxConfig};
    ///
    /// let vault_ctx = VaultContext::new(config, templates, captures, macros, types);
    /// let engine = LuaEngine::with_vault_context(SandboxConfig::restricted(), vault_ctx)?;
    ///
    /// // Now Lua scripts can use vault operations
    /// engine.eval_string(r#"
    ///     local ok, err = mdv.capture("log-to-daily", { text = "Hello" })
    /// "#)?;
    /// ```
    pub fn with_vault_context(
        config: SandboxConfig,
        vault_ctx: VaultContext,
    ) -> Result<Self, ScriptingError> {
        // Create Lua with restricted standard library
        let libs = StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH;
        let lua = Lua::new_with(libs, mlua::LuaOptions::default())?;

        // Apply memory limit if configured
        if config.memory_limit > 0 {
            lua.set_memory_limit(config.memory_limit)?;
        }

        // Remove dangerous globals
        Self::apply_sandbox(&lua)?;

        // Register standard mdv bindings
        register_mdv_table(&lua)?;

        // Register vault operation bindings
        register_vault_bindings(&lua, vault_ctx)?;

        Ok(Self { lua, config })
    }

    /// Execute a Lua script and return the result.
    ///
    /// Returns `None` if the script returns nil or no value.
    pub fn eval(&self, script: &str) -> Result<Option<String>, ScriptingError> {
        let value: Value = self.lua.load(script).eval()?;

        match value {
            Value::Nil => Ok(None),
            Value::String(s) => Ok(Some(s.to_str()?.to_string())),
            Value::Integer(i) => Ok(Some(i.to_string())),
            Value::Number(n) => Ok(Some(n.to_string())),
            Value::Boolean(b) => Ok(Some(b.to_string())),
            _ => Ok(Some(format!("{:?}", value))),
        }
    }

    /// Execute a Lua script that must return a string value.
    ///
    /// Returns an error if the script returns nil.
    pub fn eval_string(&self, script: &str) -> Result<String, ScriptingError> {
        self.eval(script)?.ok_or_else(|| {
            ScriptingError::Lua(mlua::Error::runtime("script returned nil"))
        })
    }

    /// Execute a Lua script that returns a boolean.
    pub fn eval_bool(&self, script: &str) -> Result<bool, ScriptingError> {
        let value: Value = self.lua.load(script).eval()?;
        match value {
            Value::Boolean(b) => Ok(b),
            Value::Nil => Ok(false),
            _ => {
                Err(ScriptingError::Lua(mlua::Error::runtime("expected boolean result")))
            }
        }
    }

    /// Get a reference to the underlying Lua state (for advanced usage).
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Apply sandbox restrictions by removing dangerous globals.
    fn apply_sandbox(lua: &Lua) -> LuaResult<()> {
        let globals = lua.globals();

        // Remove dangerous functions that could:
        // - Execute arbitrary code: load, loadfile, dofile
        // - Access the filesystem: io
        // - Execute system commands: os
        // - Load external modules: require, package
        // - Inspect/modify internals: debug
        // - Cause resource exhaustion: collectgarbage

        globals.set("dofile", Value::Nil)?;
        globals.set("loadfile", Value::Nil)?;
        globals.set("load", Value::Nil)?;
        globals.set("require", Value::Nil)?;
        globals.set("package", Value::Nil)?;
        globals.set("io", Value::Nil)?;
        globals.set("os", Value::Nil)?;
        globals.set("debug", Value::Nil)?;
        globals.set("collectgarbage", Value::Nil)?;

        Ok(())
    }
}

impl Default for LuaEngine {
    fn default() -> Self {
        Self::sandboxed().expect("failed to create default Lua engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_basic() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval_string(r#"mdv.date("today")"#).unwrap();
        // Should be in YYYY-MM-DD format
        assert_eq!(result.len(), 10);
        assert_eq!(result.chars().nth(4), Some('-'));
        assert_eq!(result.chars().nth(7), Some('-'));
    }

    #[test]
    fn test_date_with_offset() {
        let engine = LuaEngine::sandboxed().unwrap();
        // Just verify it doesn't error - exact value depends on current date
        let result = engine.eval_string(r#"mdv.date("today + 1d")"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_date_with_format() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval_string(r#"mdv.date("today", "%A")"#).unwrap();
        // Should be a weekday name
        let valid_days = [
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
        ];
        assert!(valid_days.contains(&result.as_str()));
    }

    #[test]
    fn test_date_week() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval_string(r#"mdv.date("week")"#).unwrap();
        // Should be a number between 1 and 53
        let week: u32 = result.parse().expect("week should be a number");
        assert!((1..=53).contains(&week));
    }

    #[test]
    fn test_date_year() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval_string(r#"mdv.date("year")"#).unwrap();
        // Should be a 4-digit year
        assert_eq!(result.len(), 4);
        let year: u32 = result.parse().expect("year should be a number");
        assert!(year >= 2020);
    }

    #[test]
    fn test_render_basic() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine
            .eval_string(r#"mdv.render("Hello {{name}}", { name = "World" })"#)
            .unwrap();
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_render_multiple_vars() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine
            .eval_string(r#"mdv.render("{{greeting}}, {{name}}!", { greeting = "Hi", name = "Lua" })"#)
            .unwrap();
        assert_eq!(result, "Hi, Lua!");
    }

    #[test]
    fn test_render_with_numbers() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result =
            engine.eval_string(r#"mdv.render("Count: {{n}}", { n = 42 })"#).unwrap();
        assert_eq!(result, "Count: 42");
    }

    #[test]
    fn test_render_with_date_expr() {
        let engine = LuaEngine::sandboxed().unwrap();
        // Template engine should handle date expressions in templates
        let result = engine.eval_string(r#"mdv.render("Date: {{today}}", {})"#).unwrap();
        // Should contain "Date: " followed by a date
        assert!(result.starts_with("Date: "));
        assert_eq!(result.len(), 16); // "Date: " + "YYYY-MM-DD"
    }

    #[test]
    fn test_is_date_expr_true() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval_bool(r#"mdv.is_date_expr("today + 1d")"#).unwrap();
        assert!(result);
    }

    #[test]
    fn test_is_date_expr_false() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval_bool(r#"mdv.is_date_expr("hello")"#).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_is_date_expr_week() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval_bool(r#"mdv.is_date_expr("week/start")"#).unwrap();
        assert!(result);
    }

    #[test]
    fn test_sandbox_no_io() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval(r#"io"#).unwrap();
        assert!(result.is_none(), "io should be nil in sandbox");
    }

    #[test]
    fn test_sandbox_no_os() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval(r#"os"#).unwrap();
        assert!(result.is_none(), "os should be nil in sandbox");
    }

    #[test]
    fn test_sandbox_no_require() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval(r#"require"#).unwrap();
        assert!(result.is_none(), "require should be nil in sandbox");
    }

    #[test]
    fn test_sandbox_no_load() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval(r#"load"#).unwrap();
        assert!(result.is_none(), "load should be nil in sandbox");
    }

    #[test]
    fn test_sandbox_no_debug() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval(r#"debug"#).unwrap();
        assert!(result.is_none(), "debug should be nil in sandbox");
    }

    #[test]
    fn test_date_error_handling() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval_string(r#"mdv.date("invalid_expr")"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_pure_lua_math() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval_string(r#"tostring(1 + 2)"#).unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn test_pure_lua_string() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval_string(r#"string.upper("hello")"#).unwrap();
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_pure_lua_table() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result =
            engine.eval_string(r#"local t = {1, 2, 3}; return tostring(#t)"#).unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn test_pure_lua_math_functions() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval_string(r#"tostring(math.floor(3.7))"#).unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn test_eval_returns_none_for_nil() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval(r#"nil"#).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_eval_returns_none_for_no_return() {
        let engine = LuaEngine::sandboxed().unwrap();
        let result = engine.eval(r#"local x = 1"#).unwrap();
        assert!(result.is_none());
    }
}
