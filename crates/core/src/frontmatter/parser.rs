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
/// Returns the parsed template frontmatter (if present) and the body content.
pub fn parse_template_frontmatter(
    content: &str,
) -> Result<(Option<TemplateFrontmatter>, String), FrontmatterParseError> {
    let parsed = parse(content)?;

    if let Some(fm) = parsed.frontmatter {
        // Convert Frontmatter to TemplateFrontmatter
        let yaml_value = serde_yaml::to_value(&fm.fields)?;
        let template_fm: TemplateFrontmatter = serde_yaml::from_value(yaml_value)?;
        Ok((Some(template_fm), parsed.body))
    } else {
        Ok((None, parsed.body))
    }
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
        let (fm, body) = parse_template_frontmatter(content).unwrap();
        assert!(fm.is_some());
        let fm = fm.unwrap();
        assert_eq!(fm.output, Some("daily/{{date}}.md".to_string()));
        assert!(fm.extra.contains_key("tags"));
        assert_eq!(body, "# Daily");
    }
}
