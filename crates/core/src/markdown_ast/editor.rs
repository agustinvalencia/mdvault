use crate::markdown_ast::comrak;
use crate::markdown_ast::types::*;

/// High-level API for Markdown AST operations
pub struct MarkdownEditor;

impl MarkdownEditor {
    /// Insert a fragment into a named section
    ///
    /// # Arguments
    /// * `input` - The source Markdown document
    /// * `section` - Section matching configuration
    /// * `fragment` - Markdown content to insert
    /// * `position` - Where in the section to insert (Begin or End)
    ///
    /// # Returns
    /// The modified document and metadata about the matched section
    ///
    /// # Errors
    /// * `SectionNotFound` - No heading matches the section specification
    /// * `EmptyDocument` - Input is empty or whitespace-only
    pub fn insert_into_section(
        input: &str,
        section: &SectionMatch,
        fragment: &str,
        position: InsertPosition,
    ) -> Result<InsertResult, MarkdownAstError> {
        comrak::insert_into_section(input, section, fragment, position)
    }

    /// Find all headings in a document
    ///
    /// Useful for validation, debugging, and building section selectors
    pub fn find_headings(input: &str) -> Vec<HeadingInfo> {
        comrak::find_headings(input)
    }

    /// Check if a section exists in the document
    pub fn section_exists(input: &str, section: &SectionMatch) -> bool {
        comrak::find_section(input, section).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_begin() {
        let input = "# Title\n\n## Section A\nExisting content.\n\n## Section B\n";
        let section =
            SectionMatch { title: "Section A".to_string(), case_sensitive: false };
        let fragment = "New content.";

        let result = MarkdownEditor::insert_into_section(
            input,
            &section,
            fragment,
            InsertPosition::Begin,
        )
        .unwrap();

        assert!(result.content.contains("## Section A\nNew content.\nExisting content."));
    }

    #[test]
    fn test_insert_end() {
        let input = "# Title\n\n## Section A\nExisting content.\n\n## Section B\n";
        let section =
            SectionMatch { title: "Section A".to_string(), case_sensitive: false };
        let fragment = "New content.";

        let result = MarkdownEditor::insert_into_section(
            input,
            &section,
            fragment,
            InsertPosition::End,
        )
        .unwrap();

        assert!(
            result.content.contains("Existing content.\nNew content.\n\n## Section B")
        );
    }

    #[test]
    fn test_section_not_found() {
        let input = "# Title\n";
        let section =
            SectionMatch { title: "Missing".to_string(), case_sensitive: false };
        let result = MarkdownEditor::insert_into_section(
            input,
            &section,
            "content",
            InsertPosition::End,
        );

        assert!(matches!(result, Err(MarkdownAstError::SectionNotFound(_))));
    }

    #[test]
    fn test_case_sensitivity() {
        let input = "## SECTION A\nContent";

        let match_insensitive =
            SectionMatch { title: "section a".to_string(), case_sensitive: false };
        assert!(
            MarkdownEditor::insert_into_section(
                input,
                &match_insensitive,
                "x",
                InsertPosition::End
            )
            .is_ok()
        );

        let match_sensitive =
            SectionMatch { title: "section a".to_string(), case_sensitive: true };
        assert!(
            MarkdownEditor::insert_into_section(
                input,
                &match_sensitive,
                "x",
                InsertPosition::End
            )
            .is_err()
        );
    }

    #[test]
    fn test_nested_headers() {
        let input = "# Root\n## Parent\n### Child\n## Uncle";
        let section = SectionMatch { title: "Parent".to_string(), case_sensitive: false };
        let fragment = "New info";

        let result = MarkdownEditor::insert_into_section(
            input,
            &section,
            fragment,
            InsertPosition::End,
        )
        .unwrap();

        // Should insert before "Uncle" but after "Child" because Child is inside Parent
        // The implementation preserves existing formatting. Since the input didn't have
        // a blank line before "## Uncle", the output won't enforce one either.

        // Let's verify exact output or substring
        assert!(result.content.contains("### Child\nNew info\n## Uncle"));
    }
}
