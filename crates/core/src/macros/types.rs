//! Macro specification types for multi-step workflows.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::vars::VarsMap;

/// A macro specification loaded from a YAML file.
///
/// Macros are multi-step workflows that can execute templates, captures,
/// and (with trust) shell commands.
#[derive(Debug, Clone, Deserialize)]
pub struct MacroSpec {
    /// Logical name of the macro.
    pub name: String,

    /// Human-readable description.
    #[serde(default)]
    pub description: String,

    /// Variable specifications with prompts and defaults.
    #[serde(default)]
    pub vars: Option<VarsMap>,

    /// Steps to execute in order.
    pub steps: Vec<MacroStep>,

    /// Error handling policy.
    #[serde(default)]
    pub on_error: ErrorPolicy,
}

/// A single step in a macro workflow.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum MacroStep {
    /// Execute a template to create a new file.
    Template(TemplateStep),
    /// Execute a capture to insert content into an existing file.
    Capture(CaptureStep),
    /// Execute a shell command (requires --trust).
    Shell(ShellStep),
}

/// Template step: create a new file from a template.
#[derive(Debug, Clone, Deserialize)]
pub struct TemplateStep {
    /// Logical template name.
    pub template: String,

    /// Output path (optional, can use template frontmatter).
    #[serde(default)]
    pub output: Option<String>,

    /// Variable overrides for this step.
    #[serde(default, rename = "with")]
    pub vars_with: HashMap<String, String>,
}

/// Capture step: insert content into an existing file.
#[derive(Debug, Clone, Deserialize)]
pub struct CaptureStep {
    /// Logical capture name.
    pub capture: String,

    /// Variable overrides for this step.
    #[serde(default, rename = "with")]
    pub vars_with: HashMap<String, String>,
}

/// Shell step: execute a shell command.
#[derive(Debug, Clone, Deserialize)]
pub struct ShellStep {
    /// Shell command to execute (supports {{var}} substitution).
    pub shell: String,

    /// Human-readable description of what this command does.
    #[serde(default)]
    pub description: String,
}

/// Error handling policy for macro execution.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ErrorPolicy {
    /// Stop execution on first error (default).
    #[default]
    Abort,
    /// Continue with remaining steps after an error.
    Continue,
}

/// Information about a discovered macro file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroInfo {
    /// Logical name (filename without .yaml extension).
    pub logical_name: String,
    /// Full path to the YAML file.
    pub path: PathBuf,
}

/// A fully loaded macro ready for execution.
#[derive(Debug, Clone)]
pub struct LoadedMacro {
    pub logical_name: String,
    pub path: PathBuf,
    pub spec: MacroSpec,
}

/// Result of executing a single macro step.
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Zero-based step index.
    pub step_index: usize,
    /// Whether the step succeeded.
    pub success: bool,
    /// Human-readable description of what happened.
    pub message: String,
    /// Output path if a file was created.
    pub output_path: Option<PathBuf>,
}

/// Result of executing an entire macro.
#[derive(Debug, Clone)]
pub struct MacroResult {
    /// Name of the macro that was executed.
    pub macro_name: String,
    /// Results for each step.
    pub step_results: Vec<StepResult>,
    /// Whether all steps succeeded.
    pub success: bool,
    /// Summary message.
    pub message: String,
}

impl MacroStep {
    /// Get a human-readable description of this step.
    pub fn description(&self) -> String {
        match self {
            MacroStep::Template(t) => format!("template: {}", t.template),
            MacroStep::Capture(c) => format!("capture: {}", c.capture),
            MacroStep::Shell(s) => {
                if s.description.is_empty() {
                    format!("shell: {}", s.shell)
                } else {
                    s.description.clone()
                }
            }
        }
    }

    /// Check if this step requires trust (shell execution).
    pub fn requires_trust(&self) -> bool {
        matches!(self, MacroStep::Shell(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_macro_spec() {
        let yaml = r#"
name: weekly-review
description: Set up weekly review documents
vars:
  week_topic:
    prompt: "What's the focus this week?"
steps:
  - template: weekly-summary
    with:
      topic: "{{week_topic}}"
  - capture: archive-tasks
"#;
        let spec: MacroSpec = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(spec.name, "weekly-review");
        assert_eq!(spec.steps.len(), 2);
        assert!(spec.vars.is_some());
    }

    #[test]
    fn test_parse_template_step() {
        let yaml = r#"
template: meeting-note
output: "meetings/{{date}}.md"
with:
  title: "Weekly sync"
"#;
        let step: TemplateStep = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(step.template, "meeting-note");
        assert_eq!(step.output, Some("meetings/{{date}}.md".to_string()));
        assert_eq!(step.vars_with.get("title"), Some(&"Weekly sync".to_string()));
    }

    #[test]
    fn test_parse_capture_step() {
        let yaml = r#"
capture: inbox
with:
  text: "Review PR #42"
"#;
        let step: CaptureStep = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(step.capture, "inbox");
        assert_eq!(step.vars_with.get("text"), Some(&"Review PR #42".to_string()));
    }

    #[test]
    fn test_parse_shell_step() {
        let yaml = r#"
shell: "git add {{file}}"
description: Stage file in git
"#;
        let step: ShellStep = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(step.shell, "git add {{file}}");
        assert_eq!(step.description, "Stage file in git");
    }

    #[test]
    fn test_error_policy_default() {
        let spec: MacroSpec = serde_yaml::from_str(
            r#"
name: test
steps: []
"#,
        )
        .unwrap();
        assert_eq!(spec.on_error, ErrorPolicy::Abort);
    }

    #[test]
    fn test_error_policy_continue() {
        let spec: MacroSpec = serde_yaml::from_str(
            r#"
name: test
on_error: continue
steps: []
"#,
        )
        .unwrap();
        assert_eq!(spec.on_error, ErrorPolicy::Continue);
    }

    #[test]
    fn test_step_requires_trust() {
        let template_step = MacroStep::Template(TemplateStep {
            template: "test".to_string(),
            output: None,
            vars_with: HashMap::new(),
        });
        let shell_step = MacroStep::Shell(ShellStep {
            shell: "echo hello".to_string(),
            description: String::new(),
        });

        assert!(!template_step.requires_trust());
        assert!(shell_step.requires_trust());
    }
}
