//! Interactive prompts for collecting variable values.
//!
//! This module provides functionality to:
//! - Prompt users for missing variable values
//! - Show prompts and descriptions from VarSpec
//! - Handle defaults and required/optional status
//! - Support batch mode (non-interactive) for CI/scripting

use dialoguer::{theme::ColorfulTheme, Input};
use markadd_core::templates::engine::RenderContext;
use markadd_core::vars::{
    collect_all_variables, try_evaluate_date_expr, VarSpec, VarsMap,
};
use std::collections::HashMap;
use std::io::{self, IsTerminal};

/// Options for prompting behavior.
#[derive(Debug, Clone, Default)]
pub struct PromptOptions {
    /// If true, fail on missing variables instead of prompting.
    pub batch_mode: bool,
}

/// Result of variable collection.
#[derive(Debug)]
pub struct CollectedVars {
    /// All collected variable values.
    pub values: HashMap<String, String>,
    /// Variables that were prompted for.
    #[allow(dead_code)]
    pub prompted: Vec<String>,
    /// Variables that used defaults.
    #[allow(dead_code)]
    pub defaulted: Vec<String>,
}

/// Error type for variable collection.
#[derive(Debug)]
pub enum PromptError {
    /// Missing required variable in batch mode.
    MissingRequired(String),
    /// IO error during prompting.
    Io(io::Error),
    /// User cancelled input.
    Cancelled,
}

impl std::fmt::Display for PromptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptError::MissingRequired(name) => {
                write!(f, "missing required variable: {name}\n  Hint: use --var {name}=\"...\" or remove --batch")
            }
            PromptError::Io(e) => write!(f, "IO error: {e}"),
            PromptError::Cancelled => write!(f, "input cancelled by user"),
        }
    }
}

impl std::error::Error for PromptError {}

impl From<io::Error> for PromptError {
    fn from(e: io::Error) -> Self {
        PromptError::Io(e)
    }
}

/// Collect all required variables by prompting the user for missing values.
///
/// # Arguments
/// * `vars_map` - Variable specifications from template/capture frontmatter
/// * `content` - Template/capture content to extract additional variables from
/// * `provided` - Variables already provided via --var flags
/// * `context` - Render context with built-in variables
/// * `options` - Prompting options (batch mode, etc.)
///
/// # Returns
/// Collected variables including provided, prompted, and defaulted values.
pub fn collect_variables(
    vars_map: Option<&VarsMap>,
    content: &str,
    provided: &HashMap<String, String>,
    context: &RenderContext,
    options: &PromptOptions,
) -> Result<CollectedVars, PromptError> {
    let mut values = provided.clone();
    let mut prompted = Vec::new();
    let mut defaulted = Vec::new();

    // Check if stdin is a terminal (interactive)
    let is_interactive = io::stdin().is_terminal() && !options.batch_mode;

    // Get all variables needed
    let all_vars = collect_all_variables(vars_map, content);

    for (name, spec) in all_vars {
        // Skip if already provided
        if values.contains_key(&name) {
            continue;
        }

        // Skip if it's in the context (built-in variable)
        if context.contains_key(&name) {
            continue;
        }

        // Try to get default value
        let default_value = spec
            .as_ref()
            .and_then(|s| s.default())
            .and_then(|d| resolve_default(d, &values, context));

        let is_required = spec.as_ref().is_none_or(|s| s.is_required());

        if let Some(default) = default_value {
            if is_interactive {
                // Prompt with default pre-filled
                let value = prompt_with_default(&name, spec.as_ref(), &default)?;
                if value != default {
                    prompted.push(name.clone());
                } else {
                    defaulted.push(name.clone());
                }
                values.insert(name, value);
            } else {
                // Use default in batch mode
                defaulted.push(name.clone());
                values.insert(name, default);
            }
        } else if is_required {
            if is_interactive {
                // Prompt for required variable
                let value = prompt_required(&name, spec.as_ref())?;
                prompted.push(name.clone());
                values.insert(name, value);
            } else {
                // Fail in batch mode
                return Err(PromptError::MissingRequired(name));
            }
        }
        // Optional variables without defaults are skipped
    }

    Ok(CollectedVars { values, prompted, defaulted })
}

/// Resolve a default value, which may contain date math expressions.
fn resolve_default(
    default: &str,
    values: &HashMap<String, String>,
    context: &RenderContext,
) -> Option<String> {
    // Check if it's a date math expression like "{{today + 1d}}"
    let trimmed = default.trim();
    if trimmed.starts_with("{{") && trimmed.ends_with("}}") {
        let inner = &trimmed[2..trimmed.len() - 2].trim();
        if let Some(evaluated) = try_evaluate_date_expr(inner) {
            return Some(evaluated);
        }
        // Try variable lookup
        if let Some(val) = values.get(*inner).or_else(|| context.get(*inner)) {
            return Some(val.clone());
        }
    }

    // Return as-is if it's a static default
    Some(default.to_string())
}

/// Prompt for a required variable (no default).
fn prompt_required(name: &str, spec: Option<&VarSpec>) -> Result<String, PromptError> {
    let theme = ColorfulTheme::default();
    let prompt_text = spec.map(|s| s.prompt()).filter(|p| !p.is_empty()).unwrap_or(name);

    // Show description if available
    if let Some(desc) = spec.and_then(|s| s.description()) {
        eprintln!("  {desc}");
    }

    Input::<String>::with_theme(&theme)
        .with_prompt(prompt_text)
        .interact_text()
        .map_err(dialoguer_error_to_prompt_error)
}

/// Prompt for a variable with a default value.
fn prompt_with_default(
    name: &str,
    spec: Option<&VarSpec>,
    default: &str,
) -> Result<String, PromptError> {
    let theme = ColorfulTheme::default();
    let prompt_text = spec.map(|s| s.prompt()).filter(|p| !p.is_empty()).unwrap_or(name);

    // Show description if available
    if let Some(desc) = spec.and_then(|s| s.description()) {
        eprintln!("  {desc}");
    }

    Input::<String>::with_theme(&theme)
        .with_prompt(prompt_text)
        .default(default.to_string())
        .allow_empty(true)
        .interact_text()
        .map_err(dialoguer_error_to_prompt_error)
}

/// Convert dialoguer error to our PromptError.
fn dialoguer_error_to_prompt_error(e: dialoguer::Error) -> PromptError {
    match e {
        dialoguer::Error::IO(io_err) => {
            if io_err.kind() == io::ErrorKind::UnexpectedEof {
                PromptError::Cancelled
            } else {
                PromptError::Io(io_err)
            }
        }
    }
}

/// Parse --var arguments into a HashMap.
///
/// Expected format: `key=value`
#[allow(dead_code)]
pub fn parse_var_args(args: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for arg in args {
        if let Some((key, value)) = arg.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_var_args() {
        let args = vec![
            "title=Hello".to_string(),
            "author=World".to_string(),
            "empty=".to_string(),
        ];
        let map = parse_var_args(&args);
        assert_eq!(map.get("title"), Some(&"Hello".to_string()));
        assert_eq!(map.get("author"), Some(&"World".to_string()));
        assert_eq!(map.get("empty"), Some(&String::new()));
    }

    #[test]
    fn test_resolve_default_static() {
        let values = HashMap::new();
        let context = RenderContext::new();
        let result = resolve_default("hello", &values, &context);
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn test_resolve_default_date_expr() {
        let values = HashMap::new();
        let context = RenderContext::new();
        let result = resolve_default("{{today}}", &values, &context);
        // Should be a date string
        assert!(result.is_some());
        assert!(result.unwrap().contains("-")); // YYYY-MM-DD format
    }

    #[test]
    fn test_resolve_default_variable_lookup() {
        let mut context = RenderContext::new();
        context.insert("vault_root".to_string(), "/home/user/vault".to_string());
        let values = HashMap::new();
        let result = resolve_default("{{vault_root}}", &values, &context);
        assert_eq!(result, Some("/home/user/vault".to_string()));
    }
}
