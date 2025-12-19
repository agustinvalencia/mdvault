//! Macro runner for executing multi-step workflows.

use std::collections::HashMap;

use thiserror::Error;

use super::types::{
    CaptureStep, ErrorPolicy, LoadedMacro, MacroResult, MacroSpec, MacroStep, ShellStep,
    StepResult, TemplateStep,
};
use crate::templates::engine::render_string;

/// Error type for macro execution.
#[derive(Debug, Error)]
pub enum MacroRunError {
    #[error("step {step} failed: {message}")]
    StepFailed { step: usize, message: String },

    #[error("shell execution requires --trust flag")]
    TrustRequired,

    #[error("shell execution is disabled in config")]
    ShellDisabled,

    #[error("template error: {0}")]
    TemplateError(String),

    #[error("capture error: {0}")]
    CaptureError(String),

    #[error("shell error: {0}")]
    ShellError(String),

    #[error("variable error: {0}")]
    VariableError(String),
}

/// Options for macro execution.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Whether the --trust flag was provided.
    pub trust: bool,

    /// Whether shell execution is allowed by config.
    pub allow_shell: bool,

    /// Whether to run in dry-run mode (no actual changes).
    pub dry_run: bool,
}

/// Context passed to step executors.
#[derive(Debug, Clone)]
pub struct RunContext {
    /// Current variable values (macro vars + step overrides).
    pub vars: HashMap<String, String>,

    /// Execution options.
    pub options: RunOptions,

    /// Results from previous steps (for chaining).
    pub previous_results: Vec<StepResult>,
}

impl RunContext {
    /// Create a new run context with initial variables.
    pub fn new(vars: HashMap<String, String>, options: RunOptions) -> Self {
        Self { vars, options, previous_results: Vec::new() }
    }

    /// Merge step-level variable overrides into context.
    pub fn with_step_vars(
        &self,
        step_vars: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut merged = self.vars.clone();

        // Render step vars (they may reference macro vars)
        for (key, value) in step_vars {
            let rendered =
                render_string(value, &merged).unwrap_or_else(|_| value.clone());
            merged.insert(key.clone(), rendered);
        }

        merged
    }

    /// Add a step result to the context.
    pub fn add_result(&mut self, result: StepResult) {
        // If the step created a file, add it as a variable for subsequent steps
        if let Some(ref path) = result.output_path {
            let var_name = format!("step_{}_output", result.step_index);
            self.vars.insert(var_name, path.to_string_lossy().to_string());
        }
        self.previous_results.push(result);
    }
}

/// Trait for executing individual macro steps.
///
/// This allows the CLI/TUI to provide their own implementations
/// that integrate with their error handling and UI.
pub trait StepExecutor {
    /// Execute a template step.
    fn execute_template(
        &self,
        step: &TemplateStep,
        ctx: &RunContext,
    ) -> Result<StepResult, MacroRunError>;

    /// Execute a capture step.
    fn execute_capture(
        &self,
        step: &CaptureStep,
        ctx: &RunContext,
    ) -> Result<StepResult, MacroRunError>;

    /// Execute a shell step.
    fn execute_shell(
        &self,
        step: &ShellStep,
        ctx: &RunContext,
    ) -> Result<StepResult, MacroRunError>;
}

/// Run a macro with the given executor and context.
pub fn run_macro<E: StepExecutor>(
    loaded: &LoadedMacro,
    executor: &E,
    mut ctx: RunContext,
) -> MacroResult {
    let spec = &loaded.spec;
    let mut all_success = true;
    let mut step_results = Vec::new();

    for (index, step) in spec.steps.iter().enumerate() {
        let result = execute_step(executor, step, index, &ctx);

        match result {
            Ok(step_result) => {
                ctx.add_result(step_result.clone());
                step_results.push(step_result);
            }
            Err(e) => {
                all_success = false;
                let error_result = StepResult {
                    step_index: index,
                    success: false,
                    message: e.to_string(),
                    output_path: None,
                };
                step_results.push(error_result);

                // Check error policy
                if spec.on_error == ErrorPolicy::Abort {
                    break;
                }
            }
        }
    }

    let message = if all_success {
        format!("Completed {} steps successfully", step_results.len())
    } else {
        let failed_count = step_results.iter().filter(|r| !r.success).count();
        format!(
            "Completed with {} failures out of {} steps",
            failed_count,
            step_results.len()
        )
    };

    MacroResult {
        macro_name: loaded.logical_name.clone(),
        step_results,
        success: all_success,
        message,
    }
}

fn execute_step<E: StepExecutor>(
    executor: &E,
    step: &MacroStep,
    _index: usize,
    ctx: &RunContext,
) -> Result<StepResult, MacroRunError> {
    // Check trust requirements for shell steps
    if step.requires_trust() {
        if !ctx.options.trust {
            return Err(MacroRunError::TrustRequired);
        }
        if !ctx.options.allow_shell {
            return Err(MacroRunError::ShellDisabled);
        }
    }

    match step {
        MacroStep::Template(t) => executor.execute_template(t, ctx),
        MacroStep::Capture(c) => executor.execute_capture(c, ctx),
        MacroStep::Shell(s) => executor.execute_shell(s, ctx),
    }
}

/// Check if a macro contains any steps that require trust.
pub fn requires_trust(spec: &MacroSpec) -> bool {
    spec.steps.iter().any(|s| s.requires_trust())
}

/// Get descriptions of all shell commands in a macro.
pub fn get_shell_commands(spec: &MacroSpec) -> Vec<String> {
    spec.steps
        .iter()
        .filter_map(|s| match s {
            MacroStep::Shell(shell) => Some(shell.shell.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct MockExecutor;

    impl StepExecutor for MockExecutor {
        fn execute_template(
            &self,
            step: &TemplateStep,
            _ctx: &RunContext,
        ) -> Result<StepResult, MacroRunError> {
            Ok(StepResult {
                step_index: 0,
                success: true,
                message: format!("Created template: {}", step.template),
                output_path: Some(PathBuf::from("test.md")),
            })
        }

        fn execute_capture(
            &self,
            step: &CaptureStep,
            _ctx: &RunContext,
        ) -> Result<StepResult, MacroRunError> {
            Ok(StepResult {
                step_index: 0,
                success: true,
                message: format!("Executed capture: {}", step.capture),
                output_path: None,
            })
        }

        fn execute_shell(
            &self,
            step: &ShellStep,
            _ctx: &RunContext,
        ) -> Result<StepResult, MacroRunError> {
            Ok(StepResult {
                step_index: 0,
                success: true,
                message: format!("Executed: {}", step.shell),
                output_path: None,
            })
        }
    }

    #[test]
    fn test_run_macro_simple() {
        let spec = MacroSpec {
            name: "test".to_string(),
            description: String::new(),
            vars: None,
            steps: vec![MacroStep::Template(TemplateStep {
                template: "meeting".to_string(),
                output: None,
                vars_with: HashMap::new(),
            })],
            on_error: ErrorPolicy::Abort,
        };

        let loaded = LoadedMacro {
            logical_name: "test".to_string(),
            path: PathBuf::from("test.yaml"),
            spec,
        };

        let ctx = RunContext::new(HashMap::new(), RunOptions::default());
        let result = run_macro(&loaded, &MockExecutor, ctx);

        assert!(result.success);
        assert_eq!(result.step_results.len(), 1);
    }

    #[test]
    fn test_shell_requires_trust() {
        let spec = MacroSpec {
            name: "test".to_string(),
            description: String::new(),
            vars: None,
            steps: vec![MacroStep::Shell(ShellStep {
                shell: "echo hello".to_string(),
                description: String::new(),
            })],
            on_error: ErrorPolicy::Abort,
        };

        let loaded = LoadedMacro {
            logical_name: "test".to_string(),
            path: PathBuf::from("test.yaml"),
            spec,
        };

        // Without trust
        let ctx = RunContext::new(HashMap::new(), RunOptions::default());
        let result = run_macro(&loaded, &MockExecutor, ctx);
        assert!(!result.success);

        // With trust but shell disabled
        let ctx = RunContext::new(
            HashMap::new(),
            RunOptions { trust: true, allow_shell: false, dry_run: false },
        );
        let result = run_macro(&loaded, &MockExecutor, ctx);
        assert!(!result.success);

        // With trust and shell enabled
        let ctx = RunContext::new(
            HashMap::new(),
            RunOptions { trust: true, allow_shell: true, dry_run: false },
        );
        let result = run_macro(&loaded, &MockExecutor, ctx);
        assert!(result.success);
    }

    #[test]
    fn test_requires_trust_check() {
        let spec_with_shell = MacroSpec {
            name: "test".to_string(),
            description: String::new(),
            vars: None,
            steps: vec![
                MacroStep::Template(TemplateStep {
                    template: "meeting".to_string(),
                    output: None,
                    vars_with: HashMap::new(),
                }),
                MacroStep::Shell(ShellStep {
                    shell: "git add .".to_string(),
                    description: String::new(),
                }),
            ],
            on_error: ErrorPolicy::Abort,
        };

        let spec_without_shell = MacroSpec {
            name: "test".to_string(),
            description: String::new(),
            vars: None,
            steps: vec![MacroStep::Template(TemplateStep {
                template: "meeting".to_string(),
                output: None,
                vars_with: HashMap::new(),
            })],
            on_error: ErrorPolicy::Abort,
        };

        assert!(requires_trust(&spec_with_shell));
        assert!(!requires_trust(&spec_without_shell));
    }
}
