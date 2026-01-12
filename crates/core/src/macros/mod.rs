//! Macro system for multi-step workflows.
//!
//! Macros allow users to define sequences of template and capture operations
//! that execute as a single workflow.

pub mod discovery;
pub mod lua_loader;
pub mod runner;
pub mod types;

pub use discovery::{
    MacroDiscoveryError, MacroRepoError, MacroRepository, discover_macros,
};
pub use lua_loader::load_macro_from_lua;
pub use runner::{
    MacroRunError, RunContext, RunOptions, StepExecutor, get_shell_commands,
    requires_trust, run_macro,
};
pub use types::{
    CaptureStep, ErrorPolicy, LoadedMacro, MacroFormat, MacroInfo, MacroResult,
    MacroSpec, MacroStep, ShellStep, StepResult, TemplateStep,
};
