//! Note content extraction: links, title, type, frontmatter.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::frontmatter::{self, Frontmatter};
use crate::index::types::{LinkType, NoteType};

/// Extracted information from a note file.
#[derive(Debug, Clone)]
pub struct ExtractedNote {
    /// Note title (from frontmatter, first heading, or filename).
    pub title: String,
    /// Note type from frontmatter `type:` field.
    pub note_type: NoteType,
    /// Frontmatter as JSON string (if present).
    pub frontmatter_json: Option<String>,
    /// All links found in the document.
    pub links: Vec<ExtractedLink>,
}

/// A link extracted from a note.
#[derive(Debug, Clone)]
pub struct ExtractedLink {
    /// Target path/name (raw, as written in the link).
    pub target: String,
    /// Display text (alias for wikilinks, text for markdown links).
    pub text: Option<String>,
    /// Type of link.
    pub link_type: LinkType,
    /// Line number where link appears (1-based).
    pub line_number: u32,
    /// Context text (surrounding content).
    pub context: Option<String>,
}

// Regex patterns for link extraction
static WIKILINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Matches [[target]] or [[target|alias]]
    // Also handles [[target#section]] and [[target#section|alias]]
    Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").unwrap()
});

static MARKDOWN_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Matches [text](url) - captures .md files and relative paths
    // Excludes http:// and https:// URLs
    Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap()
});

/// Extract note information from file content.
pub fn extract_note(content: &str, file_path: &Path) -> ExtractedNote {
    // Parse frontmatter
    let parsed = frontmatter::parse(content).unwrap_or_else(|_| {
        crate::frontmatter::ParsedDocument {
            frontmatter: None,
            body: content.to_string(),
        }
    });

    // Extract note type from frontmatter
    let note_type = parsed
        .frontmatter
        .as_ref()
        .and_then(|fm| fm.fields.get("type"))
        .and_then(|v| v.as_str())
        .map(|s| s.parse().unwrap_or_default())
        .unwrap_or_default();

    // Extract title: frontmatter > first heading > filename
    let title = extract_title(&parsed.frontmatter, &parsed.body, file_path);

    // Serialize frontmatter to JSON
    let frontmatter_json = parsed
        .frontmatter
        .as_ref()
        .map(|fm| serde_json::to_string(&fm.fields).unwrap_or_default());

    // Extract links from body
    let mut links = extract_links(&parsed.body);

    // Extract frontmatter references (project:, parent:, etc.)
    let fm_links = extract_frontmatter_links(&parsed.frontmatter);
    links.extend(fm_links);

    ExtractedNote { title, note_type, frontmatter_json, links }
}

fn extract_title(fm: &Option<Frontmatter>, body: &str, file_path: &Path) -> String {
    // Try frontmatter title
    if let Some(fm) = fm
        && let Some(title) = fm.fields.get("title").and_then(|v| v.as_str())
    {
        return title.to_string();
    }

    // Try first heading
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix('#') {
            let heading = heading.trim_start_matches('#').trim();
            if !heading.is_empty() {
                return heading.to_string();
            }
        }
    }

    // Fall back to filename without extension
    file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").to_string()
}

fn extract_links(body: &str) -> Vec<ExtractedLink> {
    let mut links = Vec::new();

    for (line_num, line) in body.lines().enumerate() {
        let line_number = (line_num + 1) as u32;

        // Extract wikilinks
        for cap in WIKILINK_RE.captures_iter(line) {
            let target = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let alias = cap.get(2).map(|m| m.as_str().to_string());

            links.push(ExtractedLink {
                target: target.to_string(),
                text: alias,
                link_type: LinkType::Wikilink,
                line_number,
                context: Some(truncate_context(line, 100)),
            });
        }

        // Extract markdown links to local files
        for cap in MARKDOWN_LINK_RE.captures_iter(line) {
            let text = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let url = cap.get(2).map(|m| m.as_str()).unwrap_or("");

            // Skip external URLs
            if url.starts_with("http://") || url.starts_with("https://") {
                continue;
            }

            // Skip non-markdown links (images, etc.) unless they're relative paths
            if !url.ends_with(".md") && !is_likely_note_reference(url) {
                continue;
            }

            links.push(ExtractedLink {
                target: url.to_string(),
                text: Some(text.to_string()),
                link_type: LinkType::Markdown,
                line_number,
                context: Some(truncate_context(line, 100)),
            });
        }
    }

    links
}

fn is_likely_note_reference(url: &str) -> bool {
    // Consider it a note reference if it:
    // - Doesn't have a file extension (might be a note name)
    // - Or ends with .md
    // - And doesn't look like an image or other asset
    let lower = url.to_lowercase();

    // Skip obvious non-notes
    if lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".svg")
        || lower.ends_with(".pdf")
    {
        return false;
    }

    // If no extension, it might be a note reference
    !url.contains('.')
}

fn extract_frontmatter_links(fm: &Option<Frontmatter>) -> Vec<ExtractedLink> {
    let mut links = Vec::new();

    let fm = match fm {
        Some(fm) => fm,
        None => return links,
    };

    // Known reference fields
    let ref_fields = ["project", "parent", "related", "blocks", "blocked_by"];

    for field in &ref_fields {
        if let Some(value) = fm.fields.get(*field) {
            // Handle single string value
            if let Some(s) = value.as_str() {
                links.push(ExtractedLink {
                    target: s.to_string(),
                    text: Some(format!("{}: {}", field, s)),
                    link_type: LinkType::Frontmatter,
                    line_number: 0, // Frontmatter doesn't have meaningful line numbers
                    context: None,
                });
            }
            // Handle array of strings
            if let Some(arr) = value.as_sequence() {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        links.push(ExtractedLink {
                            target: s.to_string(),
                            text: Some(format!("{}: {}", field, s)),
                            link_type: LinkType::Frontmatter,
                            line_number: 0,
                            context: None,
                        });
                    }
                }
            }
        }
    }

    links
}

fn truncate_context(line: &str, max_len: usize) -> String {
    if line.len() <= max_len {
        line.to_string()
    } else {
        format!("{}...", &line[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_wikilinks() {
        let content = r#"---
title: Test Note
---
# Heading

This links to [[other-note]] and [[another|with alias]].
Also [[path/to/note]] works.
"#;
        let note = extract_note(content, Path::new("test.md"));

        assert_eq!(note.links.len(), 3);
        assert_eq!(note.links[0].target, "other-note");
        assert_eq!(note.links[0].text, None);
        assert_eq!(note.links[0].link_type, LinkType::Wikilink);

        assert_eq!(note.links[1].target, "another");
        assert_eq!(note.links[1].text, Some("with alias".to_string()));

        assert_eq!(note.links[2].target, "path/to/note");
    }

    #[test]
    fn test_extract_markdown_links() {
        let content = r#"# Note

See [this note](./other.md) for details.
Also [external](https://example.com) should be skipped.
And [image](./pic.png) should be skipped too.
"#;
        let note = extract_note(content, Path::new("test.md"));

        assert_eq!(note.links.len(), 1);
        assert_eq!(note.links[0].target, "./other.md");
        assert_eq!(note.links[0].text, Some("this note".to_string()));
        assert_eq!(note.links[0].link_type, LinkType::Markdown);
    }

    #[test]
    fn test_extract_frontmatter_links() {
        let content = r#"---
title: Task
type: task
project: my-project
related:
  - note-a
  - note-b
---
# Task content
"#;
        let note = extract_note(content, Path::new("task.md"));

        let fm_links: Vec<_> =
            note.links.iter().filter(|l| l.link_type == LinkType::Frontmatter).collect();

        assert_eq!(fm_links.len(), 3);
        assert!(fm_links.iter().any(|l| l.target == "my-project"));
        assert!(fm_links.iter().any(|l| l.target == "note-a"));
        assert!(fm_links.iter().any(|l| l.target == "note-b"));
    }

    #[test]
    fn test_extract_title_from_frontmatter() {
        let content = r#"---
title: My Title
---
# Heading
"#;
        let note = extract_note(content, Path::new("file.md"));
        assert_eq!(note.title, "My Title");
    }

    #[test]
    fn test_extract_title_from_heading() {
        let content = "# First Heading\n\nContent here.";
        let note = extract_note(content, Path::new("file.md"));
        assert_eq!(note.title, "First Heading");
    }

    #[test]
    fn test_extract_title_from_filename() {
        let content = "No frontmatter, no heading.";
        let note = extract_note(content, Path::new("my-note.md"));
        assert_eq!(note.title, "my-note");
    }

    #[test]
    fn test_extract_note_type() {
        let content = r#"---
type: task
---
# Task
"#;
        let note = extract_note(content, Path::new("task.md"));
        assert_eq!(note.note_type, NoteType::Task);
    }

    #[test]
    fn test_extract_note_type_default() {
        let content = "# Just a note";
        let note = extract_note(content, Path::new("note.md"));
        assert_eq!(note.note_type, NoteType::None);
    }

    #[test]
    fn test_line_numbers() {
        let content = r#"Line 1
Line 2 with [[link1]]
Line 3
Line 4 with [[link2]]
"#;
        let note = extract_note(content, Path::new("test.md"));

        assert_eq!(note.links.len(), 2);
        assert_eq!(note.links[0].line_number, 2);
        assert_eq!(note.links[1].line_number, 4);
    }

    #[test]
    fn test_wikilink_with_section() {
        let content = "Link to [[note#section]] here.";
        let note = extract_note(content, Path::new("test.md"));

        assert_eq!(note.links.len(), 1);
        assert_eq!(note.links[0].target, "note#section");
    }
}
