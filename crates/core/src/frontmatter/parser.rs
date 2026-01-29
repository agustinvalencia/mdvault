//! Frontmatter parsing from markdown documents.

use super::types::{Frontmatter, ParsedDocument, TemplateFrontmatter};
use thiserror::Error;

/// Errors that can occur during frontmatter parsing.
#[derive(Debug, Error)]
pub enum FrontmatterParseError {
    #[error("invalid YAML frontmatter: {0}")]
    InvalidYaml(#[from] serde_yaml::Error),
}

/// Parse frontmatter from markdown content.
///
/// Frontmatter is delimited by `---` at the start of the document:
/// ```markdown
/// ---
/// key: value
/// ---
/// # Document content
/// ```
pub fn parse(content: &str) -> Result<ParsedDocument, FrontmatterParseError> {
    let trimmed = content.trim_start();

    // Check if document starts with frontmatter delimiter
    if !trimmed.starts_with("---") {
        return Ok(ParsedDocument { frontmatter: None, body: content.to_string() });
    }

    // Find the closing ---
    let after_first = &trimmed[3..];

    // Skip the newline after opening ---
    let after_newline = after_first
        .strip_prefix('\n')
        .or_else(|| after_first.strip_prefix("\r\n"))
        .unwrap_or(after_first);

    // Find closing delimiter
    if let Some(end_pos) = find_closing_delimiter(after_newline) {
        let yaml_content = &after_newline[..end_pos];

        // Calculate body start (skip closing --- and following newline)
        let after_closing = &after_newline[end_pos + 3..];
        let body = after_closing
            .strip_prefix('\n')
            .or_else(|| after_closing.strip_prefix("\r\n"))
            .unwrap_or(after_closing)
            .to_string();

        // Parse YAML
        let frontmatter: Frontmatter = if yaml_content.trim().is_empty() {
            Frontmatter::default()
        } else {
            serde_yaml::from_str(yaml_content.trim())?
        };

        Ok(ParsedDocument { frontmatter: Some(frontmatter), body })
    } else {
        // No closing ---, treat as no frontmatter
        Ok(ParsedDocument { frontmatter: None, body: content.to_string() })
    }
}

/// Find the position of closing `---` delimiter.
fn find_closing_delimiter(content: &str) -> Option<usize> {
    // Look for --- at the start of a line
    for (i, line) in content.lines().enumerate() {
        if line.trim() == "---" {
            // Calculate byte position
            let pos: usize = content
                .lines()
                .take(i)
                .map(|l| l.len() + 1) // +1 for newline
                .sum();
            return Some(pos);
        }
    }
    None
}

/// Parse template-specific frontmatter.
///
/// Returns the parsed template frontmatter (if present), raw frontmatter text, and the body content.
///
/// Unlike regular frontmatter parsing, this function is lenient about YAML parsing errors
/// because template frontmatter may contain unrendered variables like `{{var}}` that are
/// not valid YAML until after template rendering.
///
/// The raw frontmatter text is returned separately so it can be used for rendering
/// after variable substitution.
pub fn parse_template_frontmatter(
    content: &str,
) -> Result<(Option<TemplateFrontmatter>, Option<String>, String), FrontmatterParseError>
{
    let trimmed = content.trim_start();

    // Check if document starts with frontmatter delimiter
    if !trimmed.starts_with("---") {
        return Ok((None, None, content.to_string()));
    }

    // Find the closing ---
    let after_first = &trimmed[3..];

    // Skip the newline after opening ---
    let after_newline = after_first
        .strip_prefix('\n')
        .or_else(|| after_first.strip_prefix("\r\n"))
        .unwrap_or(after_first);

    // Find closing delimiter
    if let Some(end_pos) = find_closing_delimiter(after_newline) {
        let yaml_content = &after_newline[..end_pos];

        // Calculate body start (skip closing --- and following newline)
        let after_closing = &after_newline[end_pos + 3..];
        let body = after_closing
            .strip_prefix('\n')
            .or_else(|| after_closing.strip_prefix("\r\n"))
            .unwrap_or(after_closing)
            .to_string();

        // Store raw frontmatter text for rendering
        let raw_fm = yaml_content.to_string();

        // Try to parse template-specific fields (output, lua)
        // Be lenient - if parsing fails due to template variables, that's OK
        // We'll still have the raw text for rendering
        let template_fm = if yaml_content.trim().is_empty() {
            Some(TemplateFrontmatter::default())
        } else {
            // Try to parse, but ignore errors (template vars may make it invalid YAML)
            match parse_lenient_template_frontmatter(yaml_content) {
                Ok(fm) => Some(fm),
                Err(_) => {
                    // Parsing failed (likely due to template variables)
                    // Create a minimal TemplateFrontmatter with just the raw content
                    Some(TemplateFrontmatter::default())
                }
            }
        };

        Ok((template_fm, Some(raw_fm), body))
    } else {
        // No closing ---, treat as no frontmatter
        Ok((None, None, content.to_string()))
    }
}

/// Parse template frontmatter leniently, extracting only template-specific fields.
///
/// This attempts to extract `output:` and `lua:` fields via simple line parsing,
/// which works even if other fields contain template variables that make the YAML invalid.
///
/// We intentionally don't try to parse other fields since they may contain template
/// variables. The raw frontmatter text is stored separately for rendering.
fn parse_lenient_template_frontmatter(
    yaml_content: &str,
) -> Result<TemplateFrontmatter, FrontmatterParseError> {
    let mut output: Option<String> = None;
    let mut lua: Option<String> = None;
    let extra = std::collections::HashMap::new();

    // Try simple line-by-line parsing for top-level string fields
    for line in yaml_content.lines() {
        let trimmed = line.trim();

        // Extract output field
        if let Some(rest) = trimmed.strip_prefix("output:") {
            let value = rest.trim();
            // Remove quotes if present
            let value = value
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(value);
            output = Some(value.to_string());
        }
        // Extract lua field
        else if let Some(rest) = trimmed.strip_prefix("lua:") {
            let value = rest.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(value);
            lua = Some(value.to_string());
        }
    }

    // We intentionally don't parse other fields into `extra` because they may contain
    // template variables. The raw frontmatter text will be used for rendering instead.
    Ok(TemplateFrontmatter { lua, output, extra })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_no_frontmatter() {
        let content = "# Hello\n\nSome content";
        let result = parse(content).unwrap();
        assert!(result.frontmatter.is_none());
        assert_eq!(result.body, content);
    }

    #[test]
    fn parse_simple_frontmatter() {
        let content = "---\ntitle: Hello\n---\n# Content";
        let result = parse(content).unwrap();
        assert!(result.frontmatter.is_some());
        let fm = result.frontmatter.unwrap();
        assert_eq!(fm.fields.get("title").and_then(|v| v.as_str()), Some("Hello"));
        assert_eq!(result.body, "# Content");
    }

    #[test]
    fn parse_frontmatter_with_multiple_fields() {
        let content =
            "---\ntitle: Test\ndate: 2024-01-15\ntags:\n  - rust\n  - cli\n---\n\nBody";
        let result = parse(content).unwrap();
        assert!(result.frontmatter.is_some());
        let fm = result.frontmatter.unwrap();
        assert_eq!(fm.fields.get("title").and_then(|v| v.as_str()), Some("Test"));
        assert!(fm.fields.contains_key("tags"));
        assert_eq!(result.body, "\nBody");
    }

    #[test]
    fn parse_empty_frontmatter() {
        let content = "---\n---\n# Content";
        let result = parse(content).unwrap();
        assert!(result.frontmatter.is_some());
        assert!(result.frontmatter.unwrap().fields.is_empty());
        assert_eq!(result.body, "# Content");
    }

    #[test]
    fn parse_template_frontmatter_with_output() {
        let content = "---\noutput: daily/{{date}}.md\ntags: [daily]\n---\n# Daily";
        let (fm, raw_fm, body) = parse_template_frontmatter(content).unwrap();
        assert!(fm.is_some());
        let fm = fm.unwrap();
        assert_eq!(fm.output, Some("daily/{{date}}.md".to_string()));
        // Note: extra fields are NOT parsed (they may contain template vars)
        // The raw_frontmatter contains all fields for rendering
        assert!(raw_fm.is_some());
        let raw = raw_fm.unwrap();
        assert!(raw.contains("tags: [daily]"), "raw frontmatter should contain tags");
        assert_eq!(body, "# Daily");
    }
}
