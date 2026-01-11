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

pub fn render(
    template: &LoadedTemplate,
    ctx: &RenderContext,
) -> Result<String, TemplateRenderError> {
    debug!("Rendering template '{}' with vars: {:?}", template.logical_name, ctx.keys());
    let rendered_body = render_string(&template.body, ctx)?;

    // Check if template has frontmatter to include in output.
    // We render from the RAW frontmatter text to avoid YAML parsing issues
    // with template variables like {{title}} being interpreted as YAML mappings.
    if template.frontmatter.is_some()
        && let Some(raw_fm) = extract_raw_frontmatter(&template.content)
    {
        // Filter out template-specific fields (output, lua, vars)
        let filtered_fm = filter_template_fields(&raw_fm);
        if !filtered_fm.trim().is_empty() {
            // Render variables in the filtered frontmatter text
            let rendered_fm = render_string(&filtered_fm, ctx)?;
            return Ok(format!("---\n{}---\n\n{}", rendered_fm, rendered_body));
        }
    }

    Ok(rendered_body)
}

/// Extract raw frontmatter text from content (without the --- delimiters).
fn extract_raw_frontmatter(content: &str) -> Option<String> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_first = &trimmed[3..];
    let after_newline = after_first
        .strip_prefix('\n')
        .or_else(|| after_first.strip_prefix("\r\n"))
        .unwrap_or(after_first);

    // Find closing ---
    for (i, line) in after_newline.lines().enumerate() {
        if line.trim() == "---" {
            let pos: usize = after_newline.lines().take(i).map(|l| l.len() + 1).sum();
            return Some(after_newline[..pos].to_string());
        }
    }
    None
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
        let starts_field = template_fields.iter().any(|f| trimmed.starts_with(f));

        if starts_field {
            // Start skipping this field and any continuation lines
            skip_until_next_field = true;
            continue;
        }

        // Check if this is a continuation line (indented) or a new field
        if skip_until_next_field {
            // If line is indented (starts with whitespace) or empty, it's a continuation
            if line.starts_with(' ') || line.starts_with('\t') || line.trim().is_empty() {
                continue;
            }
            // New top-level field, stop skipping
            skip_until_next_field = false;
        }

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
}
