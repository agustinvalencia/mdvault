//! Data structures for the rename and reference management system.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during rename operations
#[derive(Debug, Error)]
pub enum RenameError {
    #[error("source file not found: {0}")]
    SourceNotFound(PathBuf),

    #[error("target file already exists: {0}")]
    TargetExists(PathBuf),

    #[error("failed to read file {path}: {source}")]
    ReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write file {path}: {source}")]
    WriteError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to rename file: {0}")]
    RenameError(#[source] std::io::Error),

    #[error("index error: {0}")]
    IndexError(String),

    #[error("note not found in index: {0}")]
    NoteNotInIndex(PathBuf),
}

/// Type of reference found in a file
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceType {
    /// Basic wikilink: [[note]] or [[path/note]]
    Wikilink,
    /// Wikilink with display alias: [[note|Display Text]]
    WikilinkWithAlias,
    /// Wikilink with section anchor: [[note#section]]
    WikilinkWithSection,
    /// Wikilink with both alias and section: [[note#section|Display Text]]
    WikilinkWithSectionAndAlias,
    /// Standard markdown link: [text](path.md)
    MarkdownLink,
    /// Frontmatter scalar field: project: note-name
    FrontmatterField { field: String },
    /// Frontmatter list item: related: [note1, note2]
    FrontmatterList { field: String, index: usize },
}

/// A reference to a note found in a file
#[derive(Debug, Clone)]
pub struct Reference {
    /// File containing the reference
    pub source_path: PathBuf,
    /// Line number (1-based, 0 for frontmatter)
    pub line_number: u32,
    /// Column number (1-based)
    pub column: u32,
    /// Byte offset start in file
    pub start: usize,
    /// Byte offset end in file
    pub end: usize,
    /// Original text of the reference (e.g., "[[old-note|Alias]]")
    pub original: String,
    /// Reference type
    pub ref_type: ReferenceType,
    /// For wikilinks with aliases, the alias text
    pub alias: Option<String>,
    /// For wikilinks with sections, the section anchor
    pub section: Option<String>,
    /// The link target as written (may be basename or full path)
    pub target_as_written: String,
}

impl Reference {
    /// Returns true if this is a wikilink-style reference
    pub fn is_wikilink(&self) -> bool {
        matches!(
            self.ref_type,
            ReferenceType::Wikilink
                | ReferenceType::WikilinkWithAlias
                | ReferenceType::WikilinkWithSection
                | ReferenceType::WikilinkWithSectionAndAlias
        )
    }

    /// Returns true if this is a markdown link reference
    pub fn is_markdown_link(&self) -> bool {
        matches!(self.ref_type, ReferenceType::MarkdownLink)
    }

    /// Returns true if this is a frontmatter reference
    pub fn is_frontmatter(&self) -> bool {
        matches!(
            self.ref_type,
            ReferenceType::FrontmatterField { .. } | ReferenceType::FrontmatterList { .. }
        )
    }

    /// Returns true if the original reference used a full path (not just basename)
    pub fn uses_full_path(&self) -> bool {
        self.target_as_written.contains('/')
    }
}

/// A change to be made to a file
#[derive(Debug, Clone)]
pub struct FileChange {
    /// Path to the file
    pub path: PathBuf,
    /// Original content of the file
    pub original_content: String,
    /// New content after applying updates
    pub new_content: String,
    /// References in this file that will be updated
    pub references: Vec<Reference>,
}

/// Preview of what a rename operation would do
#[derive(Debug)]
pub struct RenamePreview {
    /// Original path of the note
    pub old_path: PathBuf,
    /// New path for the note
    pub new_path: PathBuf,
    /// All references found across the vault
    pub references: Vec<Reference>,
    /// Changes that would be made to each file
    pub changes: Vec<FileChange>,
    /// Warnings about potential issues
    pub warnings: Vec<String>,
}

impl RenamePreview {
    /// Total number of references that would be updated
    pub fn total_references(&self) -> usize {
        self.references.len()
    }

    /// Number of files that would be modified
    pub fn files_affected(&self) -> usize {
        self.changes.len()
    }
}

/// Result of a successful rename operation
#[derive(Debug)]
pub struct RenameResult {
    /// Original path of the note
    pub old_path: PathBuf,
    /// New path of the note
    pub new_path: PathBuf,
    /// Files that were modified
    pub files_modified: Vec<PathBuf>,
    /// Number of references updated
    pub references_updated: usize,
    /// Warnings about potential issues
    pub warnings: Vec<String>,
}
