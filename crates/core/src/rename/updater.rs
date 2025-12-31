//! Reference update logic for rename operations.
//!
//! Updates references in files while preserving their original format.

use std::path::Path;

use crate::rename::types::{Reference, ReferenceType};

/// Generate the updated text for a reference.
///
/// Preserves the original format (wikilink style, alias, section, etc.)
/// while updating the target path.
pub fn update_reference(reference: &Reference, new_basename: &str) -> String {
    match &reference.ref_type {
        ReferenceType::Wikilink => {
            if reference.uses_full_path() {
                // Preserve path structure: [[path/old]] -> [[path/new]]
                let new_path = update_path_in_reference(&reference.target_as_written, new_basename);
                format!("[[{}]]", new_path)
            } else {
                format!("[[{}]]", new_basename)
            }
        }

        ReferenceType::WikilinkWithAlias => {
            let alias = reference.alias.as_deref().unwrap_or("");
            if reference.uses_full_path() {
                let new_path = update_path_in_reference(&reference.target_as_written, new_basename);
                format!("[[{}|{}]]", new_path, alias)
            } else {
                format!("[[{}|{}]]", new_basename, alias)
            }
        }

        ReferenceType::WikilinkWithSection => {
            let section = reference.section.as_deref().unwrap_or("");
            if reference.uses_full_path() {
                let new_path = update_path_in_reference(&reference.target_as_written, new_basename);
                format!("[[{}#{}]]", new_path, section)
            } else {
                format!("[[{}#{}]]", new_basename, section)
            }
        }

        ReferenceType::WikilinkWithSectionAndAlias => {
            let section = reference.section.as_deref().unwrap_or("");
            let alias = reference.alias.as_deref().unwrap_or("");
            if reference.uses_full_path() {
                let new_path = update_path_in_reference(&reference.target_as_written, new_basename);
                format!("[[{}#{}|{}]]", new_path, section, alias)
            } else {
                format!("[[{}#{}|{}]]", new_basename, section, alias)
            }
        }

        ReferenceType::MarkdownLink => {
            let link_text = reference.alias.as_deref().unwrap_or("");
            let new_url = update_markdown_url(&reference.target_as_written, new_basename);
            format!("[{}]({})", link_text, new_url)
        }

        ReferenceType::FrontmatterField { .. } | ReferenceType::FrontmatterList { .. } => {
            // Frontmatter references are just the basename
            new_basename.to_string()
        }
    }
}

/// Update a path-style reference, preserving the directory structure.
fn update_path_in_reference(original: &str, new_basename: &str) -> String {
    if let Some(slash_pos) = original.rfind('/') {
        // Preserve directory path
        let dir = &original[..=slash_pos];
        format!("{}{}", dir, new_basename)
    } else {
        new_basename.to_string()
    }
}

/// Update a markdown URL, preserving relative path structure and .md extension.
fn update_markdown_url(original: &str, new_basename: &str) -> String {
    // Preserve leading ./ or ../
    let prefix = if original.starts_with("./") {
        "./"
    } else if original.starts_with("../") {
        // Count how many ../ there are
        let mut prefix = String::new();
        let mut remaining = original;
        while remaining.starts_with("../") {
            prefix.push_str("../");
            remaining = &remaining[3..];
        }
        // Return the prefix portion for use
        return format!("{}{}", prefix, update_path_portion(remaining, new_basename));
    } else {
        ""
    };

    let without_prefix = original.strip_prefix(prefix).unwrap_or(original);
    let new_path = update_path_portion(without_prefix, new_basename);

    format!("{}{}", prefix, new_path)
}

/// Update the path portion of a URL (after any leading ./ or ../).
fn update_path_portion(path: &str, new_basename: &str) -> String {
    if let Some(slash_pos) = path.rfind('/') {
        // Preserve directory structure
        let dir = &path[..=slash_pos];
        format!("{}{}.md", dir, new_basename)
    } else {
        // Just the filename
        format!("{}.md", new_basename)
    }
}

/// Apply reference updates to file content.
///
/// References must be sorted by start position (will be processed in reverse order).
pub fn apply_updates(
    content: &str,
    references: &[Reference],
    new_basename: &str,
) -> String {
    // Sort references by start position (descending) to apply from end to start
    let mut sorted_refs: Vec<_> = references.iter().collect();
    sorted_refs.sort_by(|a, b| b.start.cmp(&a.start));

    let mut result = content.to_string();

    for reference in sorted_refs {
        let replacement = update_reference(reference, new_basename);

        // Verify bounds are valid
        if reference.start <= result.len() && reference.end <= result.len() {
            result.replace_range(reference.start..reference.end, &replacement);
        }
    }

    result
}

/// Compute the new relative path for a markdown link when the target moves.
///
/// This handles the case where we need to recalculate relative paths.
#[allow(dead_code)]
pub fn compute_relative_path(
    source_path: &Path,
    _old_target: &Path,
    new_target: &Path,
    vault_root: &Path,
) -> String {
    // Get paths relative to vault root
    let source_rel = source_path.strip_prefix(vault_root).unwrap_or(source_path);
    let new_target_rel = new_target.strip_prefix(vault_root).unwrap_or(new_target);

    // Get parent directories
    let source_dir = source_rel.parent().unwrap_or(Path::new(""));
    let target_dir = new_target_rel.parent().unwrap_or(Path::new(""));

    // If same directory, just use filename
    if source_dir == target_dir {
        return format!(
            "./{}.md",
            new_target_rel.file_stem().unwrap_or_default().to_string_lossy()
        );
    }

    // Calculate relative path
    let mut prefix = String::new();
    let mut source_components: Vec<_> = source_dir.components().collect();
    let mut target_components: Vec<_> = target_dir.components().collect();

    // Find common prefix
    while !source_components.is_empty()
        && !target_components.is_empty()
        && source_components[0] == target_components[0]
    {
        source_components.remove(0);
        target_components.remove(0);
    }

    // Add ../ for each remaining source component
    for _ in &source_components {
        prefix.push_str("../");
    }

    // Add remaining target components
    for comp in &target_components {
        prefix.push_str(&comp.as_os_str().to_string_lossy());
        prefix.push('/');
    }

    format!(
        "{}{}.md",
        prefix,
        new_target_rel.file_stem().unwrap_or_default().to_string_lossy()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_reference(
        original: &str,
        ref_type: ReferenceType,
        start: usize,
        end: usize,
    ) -> Reference {
        Reference {
            source_path: PathBuf::from("source.md"),
            line_number: 1,
            column: 1,
            start,
            end,
            original: original.to_string(),
            ref_type,
            alias: None,
            section: None,
            target_as_written: extract_target(original),
        }
    }

    fn extract_target(original: &str) -> String {
        // Simple extraction for tests
        original
            .trim_start_matches("[[")
            .trim_end_matches("]]")
            .split('|')
            .next()
            .unwrap_or("")
            .split('#')
            .next()
            .unwrap_or("")
            .to_string()
    }

    #[test]
    fn test_update_basic_wikilink() {
        let reference = make_reference("[[old-note]]", ReferenceType::Wikilink, 0, 12);
        let result = update_reference(&reference, "new-note");
        assert_eq!(result, "[[new-note]]");
    }

    #[test]
    fn test_update_wikilink_with_alias() {
        let mut reference = make_reference("[[old-note|My Alias]]", ReferenceType::WikilinkWithAlias, 0, 21);
        reference.alias = Some("My Alias".to_string());

        let result = update_reference(&reference, "new-note");
        assert_eq!(result, "[[new-note|My Alias]]");
    }

    #[test]
    fn test_update_wikilink_with_section() {
        let mut reference = make_reference("[[old-note#section]]", ReferenceType::WikilinkWithSection, 0, 20);
        reference.section = Some("section".to_string());

        let result = update_reference(&reference, "new-note");
        assert_eq!(result, "[[new-note#section]]");
    }

    #[test]
    fn test_update_wikilink_preserves_path() {
        let mut reference = make_reference("[[tasks/old-note]]", ReferenceType::Wikilink, 0, 18);
        reference.target_as_written = "tasks/old-note".to_string();

        let result = update_reference(&reference, "new-note");
        assert_eq!(result, "[[tasks/new-note]]");
    }

    #[test]
    fn test_update_markdown_link() {
        let mut reference = make_reference("[link text](./old-note.md)", ReferenceType::MarkdownLink, 0, 26);
        reference.alias = Some("link text".to_string());
        reference.target_as_written = "./old-note.md".to_string();

        let result = update_reference(&reference, "new-note");
        assert_eq!(result, "[link text](./new-note.md)");
    }

    #[test]
    fn test_update_markdown_link_relative() {
        let mut reference = make_reference("[text](../tasks/old-note.md)", ReferenceType::MarkdownLink, 0, 28);
        reference.alias = Some("text".to_string());
        reference.target_as_written = "../tasks/old-note.md".to_string();

        let result = update_reference(&reference, "new-note");
        assert_eq!(result, "[text](../tasks/new-note.md)");
    }

    #[test]
    fn test_update_frontmatter_field() {
        let reference = Reference {
            source_path: PathBuf::from("source.md"),
            line_number: 0,
            column: 0,
            start: 20,
            end: 28,
            original: "old-note".to_string(),
            ref_type: ReferenceType::FrontmatterField {
                field: "project".to_string(),
            },
            alias: None,
            section: None,
            target_as_written: "old-note".to_string(),
        };

        let result = update_reference(&reference, "new-note");
        assert_eq!(result, "new-note");
    }

    #[test]
    fn test_apply_updates_single() {
        let content = "Link to [[old-note]] here.";
        let reference = make_reference("[[old-note]]", ReferenceType::Wikilink, 8, 20);

        let result = apply_updates(content, &[reference], "new-note");
        assert_eq!(result, "Link to [[new-note]] here.");
    }

    #[test]
    fn test_apply_updates_multiple() {
        let content = "First [[old-note]] and second [[old-note|alias]].";
        let ref1 = make_reference("[[old-note]]", ReferenceType::Wikilink, 6, 18);
        let mut ref2 = make_reference("[[old-note|alias]]", ReferenceType::WikilinkWithAlias, 30, 48);
        ref2.alias = Some("alias".to_string());

        let result = apply_updates(content, &[ref1, ref2], "new-note");
        assert_eq!(result, "First [[new-note]] and second [[new-note|alias]].");
    }

    #[test]
    fn test_compute_relative_path_same_dir() {
        let source = Path::new("/vault/notes/source.md");
        let old = Path::new("/vault/notes/old.md");
        let new = Path::new("/vault/notes/new.md");
        let vault = Path::new("/vault");

        let result = compute_relative_path(source, old, new, vault);
        assert_eq!(result, "./new.md");
    }

    #[test]
    fn test_compute_relative_path_parent_dir() {
        let source = Path::new("/vault/notes/subdir/source.md");
        let old = Path::new("/vault/notes/old.md");
        let new = Path::new("/vault/notes/new.md");
        let vault = Path::new("/vault");

        let result = compute_relative_path(source, old, new, vault);
        assert_eq!(result, "../new.md");
    }

    #[test]
    fn test_compute_relative_path_different_branch() {
        let source = Path::new("/vault/notes/source.md");
        let old = Path::new("/vault/tasks/old.md");
        let new = Path::new("/vault/tasks/new.md");
        let vault = Path::new("/vault");

        let result = compute_relative_path(source, old, new, vault);
        assert_eq!(result, "../tasks/new.md");
    }
}
