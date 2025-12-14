use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options, format_commonmark, parse_document};

use crate::markdown_ast::types::*;

/// Parse markdown and insert fragment into the specified section
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

    let arena = Arena::new();
    let options = default_options();

    let root = parse_document(&arena, input, &options);

    // Find the target heading node
    let (heading_node, heading_info) = find_heading_node(root, section)?;

    // Parse the fragment into the same arena
    let fragment_root = parse_document(&arena, fragment, &options);

    // Determine insertion point based on position
    match position {
        InsertPosition::Begin => {
            insert_after_heading(heading_node, fragment_root);
        }
        InsertPosition::End => {
            insert_before_section_end(heading_node, fragment_root);
        }
    }

    // Render back to markdown
    let content = render_to_string(root, &options)?;

    Ok(InsertResult { content, matched_heading: heading_info })
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

fn collect_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut text = String::new();
    for child in node.descendants() {
        if let NodeValue::Text(ref t) = child.data.borrow().value {
            text.push_str(t);
        }
    }
    text
}

fn find_heading_node<'a>(
    root: &'a AstNode<'a>,
    section: &SectionMatch,
) -> Result<(&'a AstNode<'a>, HeadingInfo), MarkdownAstError> {
    for node in root.descendants() {
        if let NodeValue::Heading(ref heading) = node.data.borrow().value {
            let title = collect_text(node);
            if matches_title(&title, &section.title, section.case_sensitive) {
                let info = HeadingInfo { title, level: heading.level };
                return Ok((node, info));
            }
        }
    }
    Err(MarkdownAstError::SectionNotFound(section.title.clone()))
}

/// Find the last node in a section (before the next heading of same or higher level)
fn find_section_end<'a>(heading_node: &'a AstNode<'a>) -> Option<&'a AstNode<'a>> {
    let heading_level = match &heading_node.data.borrow().value {
        NodeValue::Heading(h) => h.level,
        _ => return None,
    };

    // Walk siblings after this heading
    let mut current = heading_node.next_sibling();
    let mut last_content_node: Option<&'a AstNode<'a>> = None;

    while let Some(node) = current {
        if let NodeValue::Heading(h) = &node.data.borrow().value {
            // Found a heading of same or higher level - section ends here
            if h.level <= heading_level {
                return last_content_node;
            }
        }
        last_content_node = Some(node);
        current = node.next_sibling();
    }

    // Section extends to end of document
    last_content_node
}

fn insert_after_heading<'a>(
    heading_node: &'a AstNode<'a>,
    fragment_root: &'a AstNode<'a>,
) {
    // Insert fragment children after the heading
    // We need to insert in reverse order because insert_after puts the new node
    // immediately after, so the last child should be inserted first
    let children: Vec<_> = fragment_root.children().collect();
    for child in children.into_iter().rev() {
        child.detach();
        heading_node.insert_after(child);
    }
}

fn insert_before_section_end<'a>(
    heading_node: &'a AstNode<'a>,
    fragment_root: &'a AstNode<'a>,
) {
    if let Some(last_node) = find_section_end(heading_node) {
        // Insert after the last node in the section
        let children: Vec<_> = fragment_root.children().collect();
        for child in children.into_iter().rev() {
            child.detach();
            last_node.insert_after(child);
        }
    } else {
        // Section is empty (only heading), insert after heading
        insert_after_heading(heading_node, fragment_root);
    }
}

fn render_to_string<'a>(
    root: &'a AstNode<'a>,
    options: &Options,
) -> Result<String, MarkdownAstError> {
    let mut output = Vec::new();
    format_commonmark(root, options, &mut output)
        .map_err(|e| MarkdownAstError::RenderError(e.to_string()))?;

    String::from_utf8(output).map_err(|e| MarkdownAstError::RenderError(e.to_string()))
}
