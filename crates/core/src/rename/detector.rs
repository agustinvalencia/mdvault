//! Reference detection for rename operations.
//!
//! Finds all references to a note in other files, with exact byte positions
//! for format-preserving updates.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::frontmatter;
use crate::rename::types::{Reference, ReferenceType, RenameError};

// Regex patterns for reference detection
static WIKILINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Matches [[target]] or [[target|alias]] or [[target#section]] or [[target#section|alias]]
    // Captures:
    // 1: target (note name/path, may include #section)
    // 2: alias (if present)
    Regex::new(r"\[\[([^\]|#]+(?:#[^\]|]+)?)(?:\|([^\]]+))?\]\]").unwrap()
});

static MARKDOWN_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Matches [text](url)
    // Captures:
    // 1: text
    // 2: url
    Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap()
});

/// Find all references to a target note in a file's content.
///
/// Returns references with exact byte positions for replacement.
pub fn find_references_in_content(
    content: &str,
    source_path: &Path,
    target_path: &Path,
    vault_root: &Path,
) -> Vec<Reference> {
    let mut references = Vec::new();

    // Get the target's basename and relative path for matching
    let target_basename = target_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let target_rel = target_path
        .strip_prefix(vault_root)
        .unwrap_or(target_path);

    // Find references in body content
    let body_refs = find_body_references(content, source_path, target_basename, target_rel);
    references.extend(body_refs);

    // Find references in frontmatter
    let fm_refs = find_frontmatter_references(content, source_path, target_basename);
    references.extend(fm_refs);

    references
}

fn find_body_references(
    content: &str,
    source_path: &Path,
    target_basename: &str,
    target_rel: &Path,
) -> Vec<Reference> {
    let mut references = Vec::new();

    // Track byte offset for each line
    let mut line_start_offset = 0;

    for (line_idx, line) in content.lines().enumerate() {
        let line_number = (line_idx + 1) as u32;

        // Find wikilinks in this line
        for cap in WIKILINK_RE.captures_iter(line) {
            let full_match = cap.get(0).unwrap();
            let target_text = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let alias = cap.get(2).map(|m| m.as_str().to_string());

            // Parse target (may include #section)
            let (note_ref, section) = parse_wikilink_target(target_text);

            if matches_target(note_ref, target_basename, target_rel) {
                let start = line_start_offset + full_match.start();
                let end = line_start_offset + full_match.end();
                let column = (full_match.start() + 1) as u32;

                let ref_type = match (&section, &alias) {
                    (Some(_), Some(_)) => ReferenceType::WikilinkWithSectionAndAlias,
                    (Some(_), None) => ReferenceType::WikilinkWithSection,
                    (None, Some(_)) => ReferenceType::WikilinkWithAlias,
                    (None, None) => ReferenceType::Wikilink,
                };

                references.push(Reference {
                    source_path: source_path.to_path_buf(),
                    line_number,
                    column,
                    start,
                    end,
                    original: full_match.as_str().to_string(),
                    ref_type,
                    alias,
                    section,
                    target_as_written: note_ref.to_string(),
                });
            }
        }

        // Find markdown links in this line
        for cap in MARKDOWN_LINK_RE.captures_iter(line) {
            let full_match = cap.get(0).unwrap();
            let link_text = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let url = cap.get(2).map(|m| m.as_str()).unwrap_or("");

            // Skip external URLs
            if url.starts_with("http://") || url.starts_with("https://") {
                continue;
            }

            if matches_markdown_target(url, target_basename, target_rel) {
                let start = line_start_offset + full_match.start();
                let end = line_start_offset + full_match.end();
                let column = (full_match.start() + 1) as u32;

                references.push(Reference {
                    source_path: source_path.to_path_buf(),
                    line_number,
                    column,
                    start,
                    end,
                    original: full_match.as_str().to_string(),
                    ref_type: ReferenceType::MarkdownLink,
                    alias: Some(link_text.to_string()),
                    section: None,
                    target_as_written: url.to_string(),
                });
            }
        }

        // Move to next line (add line length + newline character)
        line_start_offset += line.len() + 1; // +1 for \n
    }

    references
}

fn find_frontmatter_references(
    content: &str,
    source_path: &Path,
    target_basename: &str,
) -> Vec<Reference> {
    let mut references = Vec::new();

    // Parse frontmatter
    let parsed = match frontmatter::parse(content) {
        Ok(p) => p,
        Err(_) => return references,
    };

    let fm = match parsed.frontmatter {
        Some(fm) => fm,
        None => return references,
    };

    // Known reference fields
    let ref_fields = ["project", "parent", "related", "blocks", "blocked_by"];

    // Find frontmatter section bounds
    let fm_bounds = find_frontmatter_bounds(content);

    for field in &ref_fields {
        if let Some(value) = fm.fields.get(*field) {
            // Handle single string value
            if let Some(s) = value.as_str()
                && matches_frontmatter_ref(s, target_basename)
                    && let Some((start, end)) =
                        find_frontmatter_field_value(content, field, s, &fm_bounds)
                    {
                        references.push(Reference {
                            source_path: source_path.to_path_buf(),
                            line_number: 0, // Frontmatter
                            column: 0,
                            start,
                            end,
                            original: s.to_string(),
                            ref_type: ReferenceType::FrontmatterField {
                                field: field.to_string(),
                            },
                            alias: None,
                            section: None,
                            target_as_written: s.to_string(),
                        });
                    }

            // Handle array of strings
            if let Some(arr) = value.as_sequence() {
                for (idx, item) in arr.iter().enumerate() {
                    if let Some(s) = item.as_str()
                        && matches_frontmatter_ref(s, target_basename)
                            && let Some((start, end)) =
                                find_frontmatter_list_item(content, field, s, idx, &fm_bounds)
                            {
                                references.push(Reference {
                                    source_path: source_path.to_path_buf(),
                                    line_number: 0,
                                    column: 0,
                                    start,
                                    end,
                                    original: s.to_string(),
                                    ref_type: ReferenceType::FrontmatterList {
                                        field: field.to_string(),
                                        index: idx,
                                    },
                                    alias: None,
                                    section: None,
                                    target_as_written: s.to_string(),
                                });
                            }
                }
            }
        }
    }

    references
}

/// Parse a wikilink target, separating the note reference from the section anchor.
fn parse_wikilink_target(target: &str) -> (&str, Option<String>) {
    if let Some(hash_pos) = target.find('#') {
        let note = &target[..hash_pos];
        let section = &target[hash_pos + 1..];
        (note, Some(section.to_string()))
    } else {
        (target, None)
    }
}

/// Check if a wikilink reference matches the target note.
fn matches_target(reference: &str, target_basename: &str, target_rel: &Path) -> bool {
    let ref_lower = reference.to_lowercase();
    let basename_lower = target_basename.to_lowercase();

    // Direct basename match (most common case)
    if ref_lower == basename_lower {
        return true;
    }

    // Match with .md extension
    if ref_lower == format!("{}.md", basename_lower) {
        return true;
    }

    // Match full relative path
    let target_rel_str = target_rel.to_string_lossy().to_lowercase();
    if ref_lower == target_rel_str {
        return true;
    }

    // Match relative path without .md
    let target_no_ext = target_rel_str.strip_suffix(".md").unwrap_or(&target_rel_str);
    if ref_lower == target_no_ext {
        return true;
    }

    false
}

/// Check if a markdown link URL matches the target note.
fn matches_markdown_target(url: &str, target_basename: &str, target_rel: &Path) -> bool {
    // Normalize the URL path
    let url_normalized = url
        .trim_start_matches("./")
        .trim_start_matches("../");

    let url_lower = url_normalized.to_lowercase();
    let basename_lower = target_basename.to_lowercase();
    let target_rel_str = target_rel.to_string_lossy().to_lowercase();

    // Match basename with .md
    if url_lower == format!("{}.md", basename_lower) {
        return true;
    }

    // Match relative path
    if url_lower == target_rel_str {
        return true;
    }

    // Check if URL ends with target path (for relative paths)
    if url_lower.ends_with(&target_rel_str) {
        return true;
    }

    // Check basename match in URL
    if let Some(filename) = Path::new(url_normalized).file_name() {
        let filename_str = filename.to_string_lossy().to_lowercase();
        if filename_str == format!("{}.md", basename_lower) {
            return true;
        }
    }

    false
}

/// Check if a frontmatter reference matches the target note.
fn matches_frontmatter_ref(reference: &str, target_basename: &str) -> bool {
    let ref_lower = reference.to_lowercase();
    let basename_lower = target_basename.to_lowercase();

    ref_lower == basename_lower || ref_lower == format!("{}.md", basename_lower)
}

/// Find the bounds of the frontmatter section.
fn find_frontmatter_bounds(content: &str) -> Option<(usize, usize)> {
    if !content.starts_with("---") {
        return None;
    }

    let start = 4; // After "---\n"
    content[start..].find("\n---").map(|end_marker| (0, start + end_marker + 4))
}

/// Find the byte position of a frontmatter field value.
fn find_frontmatter_field_value(
    content: &str,
    field: &str,
    value: &str,
    bounds: &Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    let (fm_start, fm_end) = bounds.as_ref()?;
    let fm_content = &content[*fm_start..*fm_end];

    // Look for pattern: "field: value" or "field: 'value'" or 'field: "value"'
    let patterns = [
        format!("{}: {}", field, value),
        format!("{}: '{}'", field, value),
        format!("{}: \"{}\"", field, value),
    ];

    for pattern in &patterns {
        if let Some(pos) = fm_content.find(pattern.as_str()) {
            let value_start = pos + field.len() + 2; // +2 for ": "
            let start = *fm_start + value_start;

            // Adjust for quotes if present
            let actual_start = if fm_content[value_start..].starts_with('\'')
                || fm_content[value_start..].starts_with('"')
            {
                start + 1
            } else {
                start
            };

            return Some((actual_start, actual_start + value.len()));
        }
    }

    None
}

/// Find the byte position of a frontmatter list item.
fn find_frontmatter_list_item(
    content: &str,
    field: &str,
    value: &str,
    _index: usize,
    bounds: &Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    let (fm_start, fm_end) = bounds.as_ref()?;
    let fm_content = &content[*fm_start..*fm_end];

    // Find the field first
    let field_pattern = format!("{}:", field);
    let field_pos = fm_content.find(&field_pattern)?;

    // Search after the field for the value in list format
    let after_field = &fm_content[field_pos..];

    // Look for "- value" pattern (YAML list item)
    let list_patterns = [
        format!("- {}", value),
        format!("- '{}'", value),
        format!("- \"{}\"", value),
    ];

    for pattern in &list_patterns {
        if let Some(pos) = after_field.find(pattern.as_str()) {
            let value_start = pos + 2; // +2 for "- "
            let start = *fm_start + field_pos + value_start;

            // Adjust for quotes
            let actual_start = if after_field[value_start..].starts_with('\'')
                || after_field[value_start..].starts_with('"')
            {
                start + 1
            } else {
                start
            };

            return Some((actual_start, actual_start + value.len()));
        }
    }

    // Also check inline array format: [item1, item2]
    let inline_patterns = [
        format!("[{}", value),
        format!(", {}", value),
        format!("['{}']", value),
        format!(", '{}']", value),
        format!("[\"{}\"]", value),
        format!(", \"{}\"]", value),
    ];

    for pattern in &inline_patterns {
        if let Some(pos) = after_field.find(pattern.as_str()) {
            // Find where the value actually starts
            let pattern_value_offset = if pattern.starts_with('[') { 1 } else { 2 };
            let value_start = pos + pattern_value_offset;
            let start = *fm_start + field_pos + value_start;

            // Adjust for quotes
            let actual_start = if after_field[value_start..].starts_with('\'')
                || after_field[value_start..].starts_with('"')
            {
                start + 1
            } else {
                start
            };

            return Some((actual_start, actual_start + value.len()));
        }
    }

    None
}

/// Read a file and find all references to a target note.
#[allow(dead_code)]
pub fn find_references_in_file(
    source_path: &Path,
    target_path: &Path,
    vault_root: &Path,
) -> Result<Vec<Reference>, RenameError> {
    let content = std::fs::read_to_string(source_path).map_err(|e| RenameError::ReadError {
        path: source_path.to_path_buf(),
        source: e,
    })?;

    Ok(find_references_in_content(&content, source_path, target_path, vault_root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_wikilink_basic() {
        let content = "Here is a link to [[my-note]] in the text.";
        let refs = find_references_in_content(
            content,
            Path::new("source.md"),
            Path::new("/vault/my-note.md"),
            Path::new("/vault"),
        );

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].original, "[[my-note]]");
        assert_eq!(refs[0].ref_type, ReferenceType::Wikilink);
        // "Here is a link to " = 18 chars, so [[my-note]] starts at 18
        assert_eq!(refs[0].start, 18);
        assert_eq!(refs[0].end, 29);
    }

    #[test]
    fn test_find_wikilink_with_alias() {
        let content = "Link to [[my-note|My Note Title]] here.";
        let refs = find_references_in_content(
            content,
            Path::new("source.md"),
            Path::new("/vault/my-note.md"),
            Path::new("/vault"),
        );

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_type, ReferenceType::WikilinkWithAlias);
        assert_eq!(refs[0].alias, Some("My Note Title".to_string()));
    }

    #[test]
    fn test_find_wikilink_with_section() {
        let content = "See [[my-note#section]] for details.";
        let refs = find_references_in_content(
            content,
            Path::new("source.md"),
            Path::new("/vault/my-note.md"),
            Path::new("/vault"),
        );

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_type, ReferenceType::WikilinkWithSection);
        assert_eq!(refs[0].section, Some("section".to_string()));
    }

    #[test]
    fn test_find_markdown_link() {
        let content = "Check out [this note](./my-note.md) for more.";
        let refs = find_references_in_content(
            content,
            Path::new("source.md"),
            Path::new("/vault/my-note.md"),
            Path::new("/vault"),
        );

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_type, ReferenceType::MarkdownLink);
        assert_eq!(refs[0].alias, Some("this note".to_string()));
    }

    #[test]
    fn test_find_multiple_references() {
        let content = r#"# Notes

Link to [[my-note]] and also [[my-note|alias]].
And a [markdown link](my-note.md) too.
"#;
        let refs = find_references_in_content(
            content,
            Path::new("source.md"),
            Path::new("/vault/my-note.md"),
            Path::new("/vault"),
        );

        assert_eq!(refs.len(), 3);
    }

    #[test]
    fn test_case_insensitive_matching() {
        let content = "Link to [[My-Note]] here.";
        let refs = find_references_in_content(
            content,
            Path::new("source.md"),
            Path::new("/vault/my-note.md"),
            Path::new("/vault"),
        );

        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn test_no_match_different_note() {
        let content = "Link to [[other-note]] here.";
        let refs = find_references_in_content(
            content,
            Path::new("source.md"),
            Path::new("/vault/my-note.md"),
            Path::new("/vault"),
        );

        assert_eq!(refs.len(), 0);
    }

    #[test]
    fn test_skip_external_urls() {
        let content = "See [example](https://example.com) for details.";
        let refs = find_references_in_content(
            content,
            Path::new("source.md"),
            Path::new("/vault/example.md"),
            Path::new("/vault"),
        );

        assert_eq!(refs.len(), 0);
    }

    #[test]
    fn test_line_numbers() {
        let content = "Line 1\nLine 2 with [[my-note]]\nLine 3";
        let refs = find_references_in_content(
            content,
            Path::new("source.md"),
            Path::new("/vault/my-note.md"),
            Path::new("/vault"),
        );

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].line_number, 2);
    }

    #[test]
    fn test_frontmatter_field_reference() {
        let content = r#"---
title: Test
project: my-note
---
# Content
"#;
        let refs = find_references_in_content(
            content,
            Path::new("source.md"),
            Path::new("/vault/my-note.md"),
            Path::new("/vault"),
        );

        assert_eq!(refs.len(), 1);
        assert!(matches!(
            refs[0].ref_type,
            ReferenceType::FrontmatterField { .. }
        ));
    }

    #[test]
    fn test_frontmatter_list_reference() {
        let content = r#"---
title: Test
related:
  - other-note
  - my-note
  - another-note
---
# Content
"#;
        let refs = find_references_in_content(
            content,
            Path::new("source.md"),
            Path::new("/vault/my-note.md"),
            Path::new("/vault"),
        );

        assert_eq!(refs.len(), 1);
        assert!(matches!(
            refs[0].ref_type,
            ReferenceType::FrontmatterList { .. }
        ));
    }
}
