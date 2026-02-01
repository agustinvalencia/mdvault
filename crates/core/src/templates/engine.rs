use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Local;
use thiserror::Error;
use tracing::debug;

use crate::config::types::ResolvedConfig;
use crate::vars::datemath::{evaluate_date_expr, is_date_expr, parse_date_expr};

use super::discovery::TemplateInfo;
use super::repository::LoadedTemplate;

#[derive(Debug, Error)]
pub enum TemplateRenderError {
    #[error("invalid regex for template placeholder: {0}")]
    Regex(String),
}

pub type RenderContext = HashMap<String, String>;

/// Build a minimal render context with date/time and config variables.
///
/// This is useful for resolving template output paths from frontmatter
/// before the actual output path is known.
pub fn build_minimal_context(
    cfg: &ResolvedConfig,
    template: &TemplateInfo,
) -> RenderContext {
    let mut ctx = RenderContext::new();

    // Date/time (basic versions - date math expressions are handled separately)
    let now = Local::now();
    ctx.insert("date".into(), now.format("%Y-%m-%d").to_string());
    ctx.insert("time".into(), now.format("%H:%M").to_string());
    ctx.insert("datetime".into(), now.to_rfc3339());
    // Add today/now as aliases
    ctx.insert("today".into(), now.format("%Y-%m-%d").to_string());
    ctx.insert("now".into(), now.to_rfc3339());

    // From config
    ctx.insert("vault_root".into(), cfg.vault_root.to_string_lossy().to_string());
    ctx.insert("templates_dir".into(), cfg.templates_dir.to_string_lossy().to_string());
    ctx.insert("captures_dir".into(), cfg.captures_dir.to_string_lossy().to_string());
    ctx.insert("macros_dir".into(), cfg.macros_dir.to_string_lossy().to_string());

    // Template info
    ctx.insert("template_name".into(), template.logical_name.clone());
    ctx.insert("template_path".into(), template.path.to_string_lossy().to_string());

    ctx
}

pub fn build_render_context(
    cfg: &ResolvedConfig,
    template: &TemplateInfo,
    output_path: &Path,
) -> RenderContext {
    let mut ctx = build_minimal_context(cfg, template);

    // Output info
    let output_abs = absolutize(output_path);
    ctx.insert("output_path".into(), output_abs.to_string_lossy().to_string());
    if let Some(name) = output_abs.file_name().and_then(|s| s.to_str()) {
        ctx.insert("output_filename".into(), name.to_string());
    }
    if let Some(parent) = output_abs.parent() {
        ctx.insert("output_dir".into(), parent.to_string_lossy().to_string());
    }

    ctx
}

fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
    }
}

/// Clean up YAML content by removing problematic lines and quoting special values.
///
/// This handles two cases:
/// 1. Unreplaced template variables (e.g., `field: {{var}}`) - removes the line
/// 2. YAML-problematic values (e.g., `field: -`) - quotes the value
///
/// Examples:
/// - `status: {{status}}` where status wasn't provided -> line removed
/// - `status: todo` where status was provided -> line kept
/// - `phone: -` -> becomes `phone: "-"` (quoted to avoid YAML list marker interpretation)
fn remove_unreplaced_vars(content: &str) -> String {
    content
        .lines()
        .filter_map(|line| {
            // Check if line has a key-value pair
            if let Some((key, value)) = line.split_once(':') {
                let value = value.trim();

                // Case 1: Unreplaced template variable - remove line
                if value.starts_with("{{")
                    && value.ends_with("}}")
                    && !value.contains(' ')
                {
                    return None;
                }

                // Case 2: YAML-problematic values - quote them
                // Check if value needs quoting (single dash, or starts with special YAML chars)
                if needs_yaml_quoting(value) {
                    return Some(format!("{}: \"{}\"", key, value));
                }
            }
            Some(line.to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n" // Add trailing newline
}

/// Check if a YAML value needs quoting to avoid parsing errors.
///
/// Returns true for values that would be misinterpreted or cause parsing errors:
/// - List markers: `-` (single dash starts a list item)
/// - Empty string: becomes null without quotes
///
/// Note: YAML booleans (true/false/yes/no/on/off) and null values (null/~)
/// are NOT quoted because they are valid YAML values. Templates with these
/// values will have them parsed correctly as their respective types.
fn needs_yaml_quoting(value: &str) -> bool {
    // Already quoted - no need to quote again
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        return false;
    }

    // Single dash is a list marker in YAML - must be quoted
    if value == "-" {
        return true;
    }

    // Empty string becomes null in YAML - quote to preserve as empty string
    if value.is_empty() {
        return true;
    }

    false
}

pub fn render(
    template: &LoadedTemplate,
    ctx: &RenderContext,
) -> Result<String, TemplateRenderError> {
    debug!("Rendering template '{}' with vars: {:?}", template.logical_name, ctx.keys());
    let rendered_body = render_string(&template.body, ctx)?;

    // Check if template has frontmatter to include in output.
    // We render from the RAW frontmatter text to avoid YAML parsing issues
    // with template variables like {{title}} being interpreted as YAML mappings.
    if let Some(ref raw_fm) = template.raw_frontmatter {
        // Filter out template-specific fields (output, lua, vars)
        let filtered_fm = filter_template_fields(raw_fm);
        if !filtered_fm.trim().is_empty() {
            // Render variables in the filtered frontmatter text
            let rendered_fm = render_string(&filtered_fm, ctx)?;
            // Remove lines with unreplaced template variables (optional fields)
            let cleaned_fm = remove_unreplaced_vars(&rendered_fm);
            return Ok(format!("---\n{}---\n\n{}", cleaned_fm, rendered_body));
        }
    }

    Ok(rendered_body)
}

/// Filter out template-specific fields (output, lua, vars) from raw frontmatter.
/// These fields are used by the template system and should not appear in output.
fn filter_template_fields(raw_fm: &str) -> String {
    let template_fields = ["output:", "lua:", "vars:"];
    let mut result = Vec::new();
    let mut skip_until_next_field = false;

    for line in raw_fm.lines() {
        // Check if this line starts a template-specific field
        let trimmed = line.trim_start();
        let starts_template_field =
            template_fields.iter().any(|f| trimmed.starts_with(f));

        if starts_template_field {
            // Start skipping this field and any continuation lines
            skip_until_next_field = true;
            continue;
        }

        // If we're skipping continuation lines from a template field
        if skip_until_next_field {
            // Check if this is a new top-level field (not indented)
            // A new field starts at column 0 and contains a colon
            let is_new_field = !line.starts_with(' ')
                && !line.starts_with('\t')
                && !line.trim().is_empty()
                && line.contains(':');

            if is_new_field {
                // This is a new field, stop skipping and include this line
                skip_until_next_field = false;
                result.push(line);
            }
            // Continue to next line (either included the new field or skipped continuation)
            continue;
        }

        // Not skipping, include this line
        result.push(line);
    }

    let mut filtered = result.join("\n");
    // Ensure trailing newline if original had one
    if raw_fm.ends_with('\n') && !filtered.ends_with('\n') {
        filtered.push('\n');
    }
    filtered
}

/// Render a string template with variable substitution.
///
/// Supports:
/// - Simple variables: `{{var_name}}`
/// - Date math expressions: `{{today + 1d}}`, `{{now - 2h}}`, `{{today | %Y-%m-%d}}`
/// - Filters: `{{var_name | filter}}` (currently supports: slugify)
pub fn render_string(
    template: &str,
    ctx: &RenderContext,
) -> Result<String, TemplateRenderError> {
    // Match both simple vars and date math expressions
    // Captures everything between {{ and }} that looks like a valid expression
    let re = Regex::new(r"\{\{([^{}]+)\}\}")
        .map_err(|e| TemplateRenderError::Regex(e.to_string()))?;

    let result = re.replace_all(template, |caps: &regex::Captures<'_>| {
        let expr = caps[1].trim();

        // First, check if it's a date math expression
        if is_date_expr(expr)
            && let Ok(parsed) = parse_date_expr(expr)
        {
            return evaluate_date_expr(&parsed);
        }

        // Check for filter syntax: "var_name | filter"
        if let Some((var_name, filter)) = parse_filter_expr(expr) {
            if let Some(value) = ctx.get(var_name) {
                return apply_filter(value, filter);
            }
            debug!("Template variable not found for filter: {}", var_name);
            // Variable not found, return original
            return caps[0].to_string();
        }

        // Otherwise, try simple variable lookup
        if let Some(val) = ctx.get(expr) {
            val.clone()
        } else {
            debug!("Template variable not found: {}", expr);
            caps[0].to_string()
        }
    });

    Ok(result.into_owned())
}

/// Parse a filter expression like "var_name | filter_name".
/// Returns (var_name, filter_name) if valid, None otherwise.
fn parse_filter_expr(expr: &str) -> Option<(&str, &str)> {
    // Don't parse date expressions with format as filters (e.g., "today | %Y-%m-%d")
    if is_date_expr(expr) {
        return None;
    }

    let parts: Vec<&str> = expr.splitn(2, '|').collect();
    if parts.len() == 2 {
        let var_name = parts[0].trim();
        let filter = parts[1].trim();
        if !var_name.is_empty() && !filter.is_empty() {
            return Some((var_name, filter));
        }
    }
    None
}

/// Apply a filter to a value.
fn apply_filter(value: &str, filter: &str) -> String {
    match filter {
        "slugify" => slugify(value),
        "lowercase" | "lower" => value.to_lowercase(),
        "uppercase" | "upper" => value.to_uppercase(),
        "trim" => value.trim().to_string(),
        _ => value.to_string(), // Unknown filter, return unchanged
    }
}

/// Convert a string to a URL-friendly slug.
///
/// - Converts to lowercase
/// - Replaces spaces and underscores with hyphens
/// - Removes non-alphanumeric characters (except hyphens)
/// - Collapses multiple hyphens into one
/// - Trims leading/trailing hyphens
fn slugify(s: &str) -> String {
    let mut result = String::with_capacity(s.len());

    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            result.push(c.to_ascii_lowercase());
        } else if c == ' ' || c == '_' || c == '-' {
            // Only add hyphen if last char wasn't already a hyphen
            if !result.ends_with('-') {
                result.push('-');
            }
        }
        // Other characters are skipped
    }

    // Trim leading/trailing hyphens
    result.trim_matches('-').to_string()
}

/// Resolve the output path for a template.
///
/// If the template has frontmatter with an `output` field, render it with the context.
/// Otherwise, return None.
pub fn resolve_template_output_path(
    template: &LoadedTemplate,
    cfg: &ResolvedConfig,
    ctx: &RenderContext,
) -> Result<Option<PathBuf>, TemplateRenderError> {
    if let Some(ref fm) = template.frontmatter
        && let Some(ref output) = fm.output
    {
        let rendered = render_string(output, ctx)?;
        let path = cfg.vault_root.join(&rendered);
        return Ok(Some(path));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_basic() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("Test Task"), "test-task");
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("What's up?"), "whats-up");
        assert_eq!(slugify("foo@bar.com"), "foobarcom");
    }

    #[test]
    fn test_slugify_underscores() {
        assert_eq!(slugify("hello_world"), "hello-world");
        assert_eq!(slugify("foo_bar_baz"), "foo-bar-baz");
    }

    #[test]
    fn test_slugify_multiple_spaces() {
        assert_eq!(slugify("hello   world"), "hello-world");
        assert_eq!(slugify("  leading and trailing  "), "leading-and-trailing");
    }

    #[test]
    fn test_slugify_mixed() {
        assert_eq!(slugify("My Task: Do Something!"), "my-task-do-something");
        assert_eq!(slugify("2024-01-15 Meeting Notes"), "2024-01-15-meeting-notes");
    }

    #[test]
    fn test_render_string_with_slugify_filter() {
        let mut ctx = RenderContext::new();
        ctx.insert("title".into(), "Hello World".into());

        let result = render_string("{{title | slugify}}", &ctx).unwrap();
        assert_eq!(result, "hello-world");
    }

    #[test]
    fn test_render_string_with_lowercase_filter() {
        let mut ctx = RenderContext::new();
        ctx.insert("name".into(), "HELLO".into());

        let result = render_string("{{name | lowercase}}", &ctx).unwrap();
        assert_eq!(result, "hello");

        let result = render_string("{{name | lower}}", &ctx).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_render_string_with_uppercase_filter() {
        let mut ctx = RenderContext::new();
        ctx.insert("name".into(), "hello".into());

        let result = render_string("{{name | uppercase}}", &ctx).unwrap();
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_render_string_filter_in_path() {
        let mut ctx = RenderContext::new();
        ctx.insert("vault_root".into(), "/vault".into());
        ctx.insert("title".into(), "My New Task".into());

        let result =
            render_string("{{vault_root}}/tasks/{{title | slugify}}.md", &ctx).unwrap();
        assert_eq!(result, "/vault/tasks/my-new-task.md");
    }

    #[test]
    fn test_render_string_unknown_filter() {
        let mut ctx = RenderContext::new();
        ctx.insert("name".into(), "hello".into());

        // Unknown filter returns value unchanged
        let result = render_string("{{name | unknown}}", &ctx).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_render_string_missing_var_with_filter() {
        let ctx = RenderContext::new();

        // Missing variable with filter returns original placeholder
        let result = render_string("{{missing | slugify}}", &ctx).unwrap();
        assert_eq!(result, "{{missing | slugify}}");
    }

    #[test]
    fn test_date_format_not_parsed_as_filter() {
        let ctx = RenderContext::new();

        // Date expressions with format should still work
        let result = render_string("{{today | %Y-%m-%d}}", &ctx).unwrap();
        // Should be a date, not "today" with filter "%Y-%m-%d"
        assert!(result.contains('-'));
        assert!(!result.contains("today"));
    }

    #[test]
    fn test_remove_unreplaced_vars() {
        // Test removing unreplaced template variables
        let content = "status: todo\nphone: {{phone}}\nemail: test@example.com\n";
        let result = super::remove_unreplaced_vars(content);
        assert!(result.contains("status: todo"));
        assert!(!result.contains("phone:"), "unreplaced var line should be removed");
        assert!(result.contains("email: test@example.com"));
    }

    #[test]
    fn test_remove_unreplaced_vars_quotes_dash() {
        // Test quoting of YAML-problematic dash value
        let content = "name: John\nphone: -\nemail: test@example.com\n";
        let result = super::remove_unreplaced_vars(content);
        assert!(result.contains("name: John"));
        assert!(
            result.contains("phone: \"-\""),
            "dash should be quoted, got: {}",
            result
        );
        assert!(result.contains("email: test@example.com"));
    }

    #[test]
    fn test_needs_yaml_quoting() {
        use super::needs_yaml_quoting;

        // Should need quoting - only values that cause parsing errors
        assert!(needs_yaml_quoting("-")); // List marker
        assert!(needs_yaml_quoting("")); // Empty string becomes null

        // Should NOT need quoting - valid YAML values
        assert!(!needs_yaml_quoting("hello"));
        assert!(!needs_yaml_quoting("123"));
        assert!(!needs_yaml_quoting("\"already quoted\""));
        assert!(!needs_yaml_quoting("'already quoted'"));
        assert!(!needs_yaml_quoting("test@example.com"));

        // YAML booleans should NOT be quoted (they're valid YAML)
        assert!(!needs_yaml_quoting("true"));
        assert!(!needs_yaml_quoting("false"));
        assert!(!needs_yaml_quoting("yes"));
        assert!(!needs_yaml_quoting("no"));

        // YAML null values should NOT be quoted (they're valid YAML)
        assert!(!needs_yaml_quoting("null"));
        assert!(!needs_yaml_quoting("~"));
    }
}
