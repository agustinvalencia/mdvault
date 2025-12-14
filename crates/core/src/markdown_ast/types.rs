use thiserror::Error;

/// Position within a section where content should be inserted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InsertPosition {
    /// Insert immediately after the section heading
    #[default]
    Begin,
    /// Insert at the end of the section, before the next heading
    End,
}

/// Configuration for section matching
#[derive(Debug, Clone)]
pub struct SectionMatch {
    /// The section title to find (compared after trimming)
    pub title: String,
    /// Use case-sensitive matching (default: false)
    pub case_sensitive: bool,
}

impl SectionMatch {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), case_sensitive: false }
    }

    pub fn case_sensitive(mut self, value: bool) -> Self {
        self.case_sensitive = value;
        self
    }
}

/// Information about a heading found in the document
#[derive(Debug, Clone)]
pub struct HeadingInfo {
    /// The heading text content
    pub title: String,
    /// The heading level (1-6)
    pub level: u8,
}

/// Result of an insertion operation
#[derive(Debug, Clone)]
pub struct InsertResult {
    /// The modified markdown content
    pub content: String,
    /// Information about the matched section
    pub matched_heading: HeadingInfo,
}

#[derive(Debug, Error)]
pub enum MarkdownAstError {
    #[error("section not found: {0}")]
    SectionNotFound(String),

    #[error("document is empty or contains no content")]
    EmptyDocument,

    #[error("failed to render markdown: {0}")]
    RenderError(String),
}
