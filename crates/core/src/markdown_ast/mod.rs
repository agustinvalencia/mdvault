pub mod comrak;
pub mod editor;
pub mod types;

// Re-export primary API
pub use editor::MarkdownEditor;
pub use types::{
    HeadingInfo, InsertPosition, InsertResult, MarkdownAstError, SectionMatch,
};
