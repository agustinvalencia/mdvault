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
