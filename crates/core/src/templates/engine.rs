use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Local;
use serde_yaml::Value;
use thiserror::Error;

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

pub fn render(
    template: &LoadedTemplate,
    ctx: &RenderContext,
) -> Result<String, TemplateRenderError> {
    let rendered_body = render_string(&template.body, ctx)?;

    // Check if template has extra frontmatter fields to include in output
    // Note: can't use let chains (Rust 2024) so we nest the if statements
    #[allow(clippy::collapsible_if)]
    if let Some(ref fm) = template.frontmatter {
        if !fm.extra.is_empty() {
            // Render variable placeholders in frontmatter values
            let rendered_fm = render_frontmatter_values(&fm.extra, ctx)?;
            // Serialize as YAML frontmatter
            let yaml = serde_yaml::to_string(&rendered_fm).unwrap_or_default();
            return Ok(format!("---\n{}---\n\n{}", yaml, rendered_body));
        }
    }

    Ok(rendered_body)
}

/// Render variable placeholders in frontmatter values.
fn render_frontmatter_values(
    fields: &HashMap<String, Value>,
    ctx: &RenderContext,
) -> Result<HashMap<String, Value>, TemplateRenderError> {
    let mut rendered = HashMap::new();
    for (key, value) in fields {
        let rendered_value = render_yaml_value(value, ctx)?;
        rendered.insert(key.clone(), rendered_value);
    }
    Ok(rendered)
}

/// Recursively render variable placeholders in a YAML value.
fn render_yaml_value(
    value: &Value,
    ctx: &RenderContext,
) -> Result<Value, TemplateRenderError> {
    match value {
        Value::String(s) => {
            let rendered = render_string(s, ctx)?;
            Ok(Value::String(rendered))
        }
        Value::Sequence(seq) => {
            let rendered: Result<Vec<Value>, _> =
                seq.iter().map(|v| render_yaml_value(v, ctx)).collect();
            Ok(Value::Sequence(rendered?))
        }
        Value::Mapping(map) => {
            let mut rendered_map = serde_yaml::Mapping::new();
            for (k, v) in map {
                let rendered_v = render_yaml_value(v, ctx)?;
                rendered_map.insert(k.clone(), rendered_v);
            }
            Ok(Value::Mapping(rendered_map))
        }
        // Other types (numbers, bools, null) pass through unchanged
        _ => Ok(value.clone()),
    }
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
            // Variable not found, return original
            return caps[0].to_string();
        }

        // Otherwise, try simple variable lookup
        ctx.get(expr).cloned().unwrap_or_else(|| caps[0].to_string())
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
}
