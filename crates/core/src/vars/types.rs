//! Variable specification types.

use serde::Deserialize;
use std::collections::HashMap;

/// A map of variable names to their specifications.
pub type VarsMap = HashMap<String, VarSpec>;

/// Specification for a single variable.
///
/// Variables can be specified in two forms in YAML:
///
/// Simple form (just the prompt string):
/// ```yaml
/// vars:
///   title: "Meeting title"
/// ```
///
/// Full form (with metadata):
/// ```yaml
/// vars:
///   title:
///     prompt: "Meeting title"
///     required: true
///   date:
///     prompt: "Meeting date"
///     default: "{{today}}"
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum VarSpec {
    /// Simple form: just the prompt string
    Simple(String),
    /// Full form: detailed metadata
    Full(VarMetadata),
}

impl VarSpec {
    /// Get the prompt text for this variable.
    #[must_use]
    pub fn prompt(&self) -> &str {
        match self {
            VarSpec::Simple(s) => s,
            VarSpec::Full(m) => m.prompt.as_deref().unwrap_or(""),
        }
    }

    /// Get the default value, if any.
    #[must_use]
    pub fn default(&self) -> Option<&str> {
        match self {
            VarSpec::Simple(_) => None,
            VarSpec::Full(m) => m.default.as_deref(),
        }
    }

    /// Check if this variable is required.
    ///
    /// A variable is required if:
    /// - It uses the simple form (no default possible)
    /// - It uses the full form with `required: true` or no default
    #[must_use]
    pub fn is_required(&self) -> bool {
        match self {
            VarSpec::Simple(_) => true,
            VarSpec::Full(m) => m.required.unwrap_or_else(|| m.default.is_none()),
        }
    }

    /// Get the description, if any.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        match self {
            VarSpec::Simple(_) => None,
            VarSpec::Full(m) => m.description.as_deref(),
        }
    }
}

/// Full metadata for a variable specification.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VarMetadata {
    /// Human-readable prompt shown when collecting input.
    pub prompt: Option<String>,

    /// Longer description for help text.
    pub description: Option<String>,

    /// Whether this variable is required.
    /// Default: true if no default is provided.
    pub required: Option<bool>,

    /// Default value (static string or computed expression like "{{today}}").
    pub default: Option<String>,
    // Future extensions:
    // pub options: Option<Vec<String>>,  // Selection/dropdown prompt
    // pub validate: Option<String>,       // Regex validation pattern
    // pub var_type: Option<VarType>,      // Type hints (string, date, number)
}

/// Extract variable names from a template string.
///
/// Finds all `{{var_name}}` patterns and returns the unique variable names.
/// Does not include built-in variables like `date`, `time`, `today`, etc.
pub fn extract_variable_names(template: &str) -> Vec<String> {
    use regex::Regex;

    // Built-in variables that shouldn't be prompted for
    const BUILTINS: &[&str] = &[
        "date",
        "time",
        "datetime",
        "today",
        "now",
        "vault_root",
        "templates_dir",
        "captures_dir",
        "macros_dir",
        "template_name",
        "template_path",
        "output_path",
        "output_filename",
        "output_dir",
    ];

    let re = Regex::new(r"\{\{([a-zA-Z_][a-zA-Z0-9_]*)\}\}").expect("valid regex");
    let mut seen = std::collections::HashSet::new();
    let mut vars = Vec::new();

    for cap in re.captures_iter(template) {
        let name = &cap[1];
        if !BUILTINS.contains(&name) && seen.insert(name.to_string()) {
            vars.push(name.to_string());
        }
    }

    vars
}

/// Collect all variable names needed by a template/capture.
///
/// Combines:
/// - Variables declared in `vars` metadata
/// - Variables found in template content via `{{var}}` patterns
///
/// Returns variable names in order: declared vars first, then extracted vars.
pub fn collect_all_variables(
    vars_map: Option<&VarsMap>,
    content: &str,
) -> Vec<(String, Option<VarSpec>)> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // First add declared variables (in iteration order)
    if let Some(vars) = vars_map {
        for (name, spec) in vars {
            seen.insert(name.clone());
            result.push((name.clone(), Some(spec.clone())));
        }
    }

    // Then add any variables found in content that weren't declared
    for name in extract_variable_names(content) {
        if !seen.contains(&name) {
            seen.insert(name.clone());
            result.push((name, None));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_variable_names() {
        let template = "# {{title}}\nDate: {{date}}\nBy: {{author}}\n{{title}} again";
        let vars = extract_variable_names(template);
        // date is a builtin, title appears twice but should be unique
        assert_eq!(vars, vec!["title", "author"]);
    }

    #[test]
    fn test_extract_ignores_builtins() {
        let template = "{{today}} {{now}} {{time}} {{custom_var}}";
        let vars = extract_variable_names(template);
        assert_eq!(vars, vec!["custom_var"]);
    }

    #[test]
    fn test_varspec_simple() {
        let spec = VarSpec::Simple("Enter title".to_string());
        assert_eq!(spec.prompt(), "Enter title");
        assert!(spec.default().is_none());
        assert!(spec.is_required());
    }

    #[test]
    fn test_varspec_full_required() {
        let spec = VarSpec::Full(VarMetadata {
            prompt: Some("Enter title".to_string()),
            required: Some(true),
            ..Default::default()
        });
        assert_eq!(spec.prompt(), "Enter title");
        assert!(spec.is_required());
    }

    #[test]
    fn test_varspec_full_with_default() {
        let spec = VarSpec::Full(VarMetadata {
            prompt: Some("Enter date".to_string()),
            default: Some("{{today}}".to_string()),
            ..Default::default()
        });
        assert_eq!(spec.prompt(), "Enter date");
        assert_eq!(spec.default(), Some("{{today}}"));
        assert!(!spec.is_required()); // has default, so not required
    }

    #[test]
    fn test_varspec_deserialize_simple() {
        let yaml = r#""Enter your name""#;
        let spec: VarSpec = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(spec, VarSpec::Simple(_)));
        assert_eq!(spec.prompt(), "Enter your name");
    }

    #[test]
    fn test_varspec_deserialize_full() {
        let yaml = r#"
prompt: "Enter date"
default: "{{today}}"
description: "The meeting date"
"#;
        let spec: VarSpec = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(spec, VarSpec::Full(_)));
        assert_eq!(spec.prompt(), "Enter date");
        assert_eq!(spec.default(), Some("{{today}}"));
        assert_eq!(spec.description(), Some("The meeting date"));
    }

    #[test]
    fn test_collect_all_variables() {
        let mut vars_map = VarsMap::new();
        vars_map.insert("title".to_string(), VarSpec::Simple("Enter title".to_string()));

        let content = "# {{title}}\nBy: {{author}}";
        let all = collect_all_variables(Some(&vars_map), content);

        assert_eq!(all.len(), 2);
        assert_eq!(all[0].0, "title");
        assert!(all[0].1.is_some());
        assert_eq!(all[1].0, "author");
        assert!(all[1].1.is_none()); // not declared, just extracted
    }
}
