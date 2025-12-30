//! Scripting types and error definitions.

use thiserror::Error;

/// Errors that can occur during Lua script execution.
#[derive(Debug, Error)]
pub enum ScriptingError {
    /// Error from the Lua runtime.
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    /// Error evaluating a date math expression.
    #[error("date math error: {0}")]
    DateMath(String),

    /// Error rendering a template.
    #[error("template render error: {0}")]
    TemplateRender(String),

    /// Sandbox security violation.
    #[error("sandbox violation: {0}")]
    SandboxViolation(String),
}

/// Configuration for the Lua sandbox.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Maximum memory the Lua VM can allocate (in bytes). 0 = unlimited.
    pub memory_limit: usize,

    /// Maximum instructions before timeout. 0 = unlimited.
    pub instruction_limit: u32,

    /// Whether to allow `require` for loading modules.
    pub allow_require: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self::restricted()
    }
}

impl SandboxConfig {
    /// A restrictive sandbox suitable for user scripts.
    pub fn restricted() -> Self {
        Self {
            memory_limit: 10 * 1024 * 1024, // 10 MB
            instruction_limit: 100_000,
            allow_require: false,
        }
    }

    /// An unrestricted configuration (use with caution).
    pub fn unrestricted() -> Self {
        Self { memory_limit: 0, instruction_limit: 0, allow_require: true }
    }
}
