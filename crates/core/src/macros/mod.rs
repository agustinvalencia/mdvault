//! Macro system for multi-step workflows.
//!
//! Macros allow users to define sequences of template and capture operations
//! that execute as a single workflow.

pub mod discovery;
pub mod runner;
pub mod types;

pub use discovery::{
    MacroDiscoveryError, MacroRepoError, MacroRepository, discover_macros,
};
pub use runner::{
    MacroRunError, RunContext, RunOptions, StepExecutor, get_shell_commands,
    requires_trust, run_macro,
};
pub use types::{
    CaptureStep, ErrorPolicy, LoadedMacro, MacroInfo, MacroResult, MacroSpec, MacroStep,
    ShellStep, StepResult, TemplateStep,
};
