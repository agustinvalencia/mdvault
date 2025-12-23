//! Index data types for vault notes and links.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Note type classification based on frontmatter `type:` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NoteType {
    /// Daily journal notes - temporal backbone of the vault.
    Daily,
    /// Weekly overview notes.
    Weekly,
    /// Individual actionable tasks.
    Task,
    /// Collections of related tasks.
    Project,
    /// Knowledge notes (Zettelkasten-style).
    Zettel,
    /// Uncategorised notes awaiting triage.
    #[default]
    None,
}

impl NoteType {
    /// Parse note type from string (case-insensitive).
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "daily" => Self::Daily,
            "weekly" => Self::Weekly,
            "task" => Self::Task,
            "project" => Self::Project,
            "zettel" | "knowledge" => Self::Zettel,
            _ => Self::None,
        }
    }

    /// Convert to database string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Task => "task",
            Self::Project => "project",
            Self::Zettel => "zettel",
            Self::None => "none",
        }
    }
}

/// Task status values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    #[default]
    Open,
    InProgress,
    Blocked,
    Done,
    Cancelled,
}

impl TaskStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().replace(['-', '_'], "").as_str() {
            "open" => Some(Self::Open),
            "inprogress" => Some(Self::InProgress),
            "blocked" => Some(Self::Blocked),
            "done" | "completed" => Some(Self::Done),
            "cancelled" | "canceled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in-progress",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Project status values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    #[default]
    Planning,
    Active,
    Paused,
    Completed,
    Archived,
}

impl ProjectStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "planning" => Some(Self::Planning),
            "active" => Some(Self::Active),
            "paused" => Some(Self::Paused),
            "completed" | "done" => Some(Self::Completed),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Archived => "archived",
        }
    }
}

/// Type of link between notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkType {
    /// Wikilink: [[note]] or [[note|alias]]
    Wikilink,
    /// Markdown link: [text](path.md)
    Markdown,
    /// Frontmatter reference: project: note-name
    Frontmatter,
}

impl LinkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wikilink => "wikilink",
            Self::Markdown => "markdown",
            Self::Frontmatter => "frontmatter",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "wikilink" => Some(Self::Wikilink),
            "markdown" => Some(Self::Markdown),
            "frontmatter" => Some(Self::Frontmatter),
            _ => None,
        }
    }
}

/// A note in the vault index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedNote {
    /// Database ID (None if not yet inserted).
    pub id: Option<i64>,
    /// Path relative to vault root.
    pub path: PathBuf,
    /// Note type from frontmatter.
    pub note_type: NoteType,
    /// Note title (from first heading or filename).
    pub title: String,
    /// File creation time.
    pub created: Option<DateTime<Utc>>,
    /// File modification time.
    pub modified: DateTime<Utc>,
    /// Frontmatter as JSON string.
    pub frontmatter_json: Option<String>,
    /// Content hash for change detection.
    pub content_hash: String,
}

/// A link between two notes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedLink {
    /// Database ID (None if not yet inserted).
    pub id: Option<i64>,
    /// Source note ID.
    pub source_id: i64,
    /// Target note ID (None if broken link).
    pub target_id: Option<i64>,
    /// Raw target path from the link.
    pub target_path: String,
    /// Link display text (content within [[brackets]] or [text]).
    pub link_text: Option<String>,
    /// Type of link.
    pub link_type: LinkType,
    /// Surrounding context text.
    pub context: Option<String>,
    /// Line number in source file.
    pub line_number: Option<u32>,
}

/// Temporal activity record - when a note was referenced in a daily.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalActivity {
    /// Database ID.
    pub id: Option<i64>,
    /// The note being referenced.
    pub note_id: i64,
    /// The daily note containing the reference.
    pub daily_id: i64,
    /// Date of the daily note.
    pub activity_date: NaiveDate,
    /// Context of the reference.
    pub context: Option<String>,
}

/// Activity summary for a note (derived/cached).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitySummary {
    /// Note ID.
    pub note_id: i64,
    /// Last time note was referenced in a daily.
    pub last_seen: Option<NaiveDate>,
    /// Reference count in last 30 days.
    pub access_count_30d: u32,
    /// Reference count in last 90 days.
    pub access_count_90d: u32,
    /// Computed staleness score (higher = more stale).
    pub staleness_score: f32,
}

/// Query filter for listing notes.
#[derive(Debug, Clone, Default)]
pub struct NoteQuery {
    /// Filter by note type.
    pub note_type: Option<NoteType>,
    /// Filter by path prefix.
    pub path_prefix: Option<PathBuf>,
    /// Modified after this date.
    pub modified_after: Option<DateTime<Utc>>,
    /// Modified before this date.
    pub modified_before: Option<DateTime<Utc>>,
    /// Maximum number of results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}
