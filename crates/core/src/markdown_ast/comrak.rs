use comrak::nodes::{NodeValue, Sourcepos};
use comrak::{Arena, Options, parse_document};

use crate::markdown_ast::types::*;

/// Information about a section's position in the document
#[derive(Debug)]
struct SectionBounds {
    /// The heading info
    heading: HeadingInfo,
    /// Byte offset where the heading line ends (after newline)
    content_start: usize,
    /// Byte offset where the section content ends (before next heading or EOF)
    content_end: usize,
}

/// Parse markdown and insert fragment into the specified section.
/// Uses string-based insertion to preserve original formatting (including wikilinks).
pub fn insert_into_section(
    input: &str,
    section: &SectionMatch,
    fragment: &str,
    position: InsertPosition,
) -> Result<InsertResult, MarkdownAstError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(MarkdownAstError::EmptyDocument);
    }

    // Handle empty fragment (no-op)
    if fragment.trim().is_empty() {
        let headings = find_headings(input);
        let matched = headings
            .into_iter()
            .find(|h| matches_title(&h.title, &section.title, section.case_sensitive))
            .ok_or_else(|| MarkdownAstError::SectionNotFound(section.title.clone()))?;

        return Ok(InsertResult { content: input.to_string(), matched_heading: matched });
    }

    // Find section bounds using comrak for parsing
    let bounds = find_section_bounds(input, section)?;

    // Perform string-based insertion
    let content = match position {
        InsertPosition::Begin => {
            // Insert right after the heading line
            let mut result = String::with_capacity(input.len() + fragment.len() + 2);
            result.push_str(&input[..bounds.content_start]);

            // Add newline before fragment if needed
            if !result.ends_with('\n') {
                result.push('\n');
            }

            // Add the fragment
            result.push_str(fragment);

            // Ensure fragment ends with newline
            if !fragment.ends_with('\n') {
                result.push('\n');
            }

            // Add rest of document
            result.push_str(&input[bounds.content_start..]);
            result
        }
        InsertPosition::End => {
            // Insert at the end of the section content, but before trailing blank lines
            // This preserves section separator formatting
            let section_content = &input[bounds.content_start..bounds.content_end];
            let trimmed_end = find_content_end_before_blanks(section_content);
            let insert_point = bounds.content_start + trimmed_end;

            let mut result = String::with_capacity(input.len() + fragment.len() + 2);
            result.push_str(&input[..insert_point]);

            // Ensure there's a newline before the fragment
            if insert_point > 0 && !input[..insert_point].ends_with('\n') {
                result.push('\n');
            }

            // Add the fragment
            result.push_str(fragment);

            // Ensure fragment ends with newline
            if !fragment.ends_with('\n') {
                result.push('\n');
            }

            // Add the trailing blank lines and rest of document
            // If there were trailing blanks, add one blank line for section separation
            let had_trailing_blanks = trimmed_end < section_content.len();
            if had_trailing_blanks && bounds.content_end < input.len() {
                // There's a next section, add a blank line separator
                result.push('\n');
            }

            // Add rest of document (starting from the next section)
            result.push_str(&input[bounds.content_end..]);
            result
        }
    };

    Ok(InsertResult { content, matched_heading: bounds.heading })
}

/// Find the bounds of a section in the document
fn find_section_bounds(
    input: &str,
    section: &SectionMatch,
) -> Result<SectionBounds, MarkdownAstError> {
    let arena = Arena::new();
    let options = default_options();
    let root = parse_document(&arena, input, &options);

    let mut target_heading: Option<(HeadingInfo, Sourcepos)> = None;
    let mut headings_with_pos: Vec<(HeadingInfo, Sourcepos)> = Vec::new();

    // Collect all headings with their source positions
    for node in root.descendants() {
        if let NodeValue::Heading(ref heading) = node.data.borrow().value {
            let title = collect_text(node);
            let sourcepos = node.data.borrow().sourcepos;
            let info = HeadingInfo { title: title.clone(), level: heading.level };

            if target_heading.is_none()
                && matches_title(&title, &section.title, section.case_sensitive)
            {
                target_heading = Some((info.clone(), sourcepos));
            }

            headings_with_pos.push((info, sourcepos));
        }
    }

    let (heading, heading_pos) = target_heading
        .ok_or_else(|| MarkdownAstError::SectionNotFound(section.title.clone()))?;

    // Calculate content_start: byte offset after the heading line
    let content_start = line_end_offset(input, heading_pos.end.line);

    // Calculate content_end: before the next heading of same or higher level, or EOF
    let content_end =
        find_section_end_offset(input, &heading, &headings_with_pos, heading_pos);

    Ok(SectionBounds { heading, content_start, content_end })
}

/// Get the byte offset at the end of a line (after newline if present)
fn line_end_offset(input: &str, line_num: usize) -> usize {
    let mut current_line = 1;
    let mut offset = 0;

    for (i, ch) in input.char_indices() {
        if current_line == line_num && ch == '\n' {
            return i + 1;
        }
        if ch == '\n' {
            current_line += 1;
        }
        offset = i + ch.len_utf8();
    }

    // If we reach EOF on the target line
    offset
}

/// Find the byte offset where a section ends
fn find_section_end_offset(
    input: &str,
    target_heading: &HeadingInfo,
    all_headings: &[(HeadingInfo, Sourcepos)],
    target_pos: Sourcepos,
) -> usize {
    // Find the next heading of same or higher level
    for (heading, pos) in all_headings {
        // Skip headings before or at the target
        if pos.start.line <= target_pos.start.line {
            continue;
        }

        // Found a heading of same or higher level - section ends here
        if heading.level <= target_heading.level {
            // Return the byte offset at the start of this heading's line
            return line_start_offset(input, pos.start.line);
        }
    }

    // Section extends to EOF
    input.len()
}

/// Get the byte offset at the start of a line
fn line_start_offset(input: &str, line_num: usize) -> usize {
    if line_num <= 1 {
        return 0;
    }

    let mut current_line = 1;

    for (i, ch) in input.char_indices() {
        if ch == '\n' {
            current_line += 1;
            if current_line == line_num {
                return i + 1;
            }
        }
    }

    input.len()
}

/// Find the end of actual content, before any trailing blank lines.
/// Returns a byte offset relative to the start of the input string.
fn find_content_end_before_blanks(content: &str) -> usize {
    // Work backwards from the end to find the last non-blank line
    let bytes = content.as_bytes();
    let mut end = bytes.len();

    // Skip trailing whitespace/newlines
    while end > 0
        && (bytes[end - 1] == b'\n' || bytes[end - 1] == b' ' || bytes[end - 1] == b'\t')
    {
        end -= 1;
    }

    // Now find the end of the last content line (include its newline)
    // We want to return the position after the newline of the last content line
    if end < bytes.len() {
        // Find the newline after the content
        if let Some(newline_offset) = bytes[end..].iter().position(|&b| b == b'\n') {
            return end + newline_offset + 1;
        }
    }

    // No trailing newline found, return the trimmed position
    end
}

/// Find all headings in the document
pub fn find_headings(input: &str) -> Vec<HeadingInfo> {
    let arena = Arena::new();
    let options = default_options();
    let root = parse_document(&arena, input, &options);

    let mut headings = Vec::new();

    for node in root.descendants() {
        if let NodeValue::Heading(ref heading) = node.data.borrow().value {
            let title = collect_text(node);

            headings.push(HeadingInfo { title, level: heading.level });
        }
    }

    headings
}

/// Find section by match criteria (returns first match)
pub fn find_section(input: &str, section: &SectionMatch) -> Option<HeadingInfo> {
    find_headings(input)
        .into_iter()
        .find(|h| matches_title(&h.title, &section.title, section.case_sensitive))
}

// --- Internal helpers ---

fn default_options() -> Options<'static> {
    let mut options = Options::default();
    // Enable GFM extensions for compatibility
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.description_lists = true;

    // Parse options
    options.parse.smart = false; // Don't convert quotes/dashes

    // Render options for CommonMark output
    options.render.hardbreaks = false;
    options.render.github_pre_lang = true;
    options.render.unsafe_ = true; // Allow raw HTML passthrough

    options
}

fn matches_title(heading_title: &str, search_title: &str, case_sensitive: bool) -> bool {
    let h = heading_title.trim();
    let s = search_title.trim();

    if case_sensitive { h == s } else { h.eq_ignore_ascii_case(s) }
}

fn collect_text<'a>(node: &'a comrak::nodes::AstNode<'a>) -> String {
    let mut text = String::new();
    for child in node.descendants() {
        if let NodeValue::Text(ref t) = child.data.borrow().value {
            text.push_str(t);
        }
    }
    text
}
