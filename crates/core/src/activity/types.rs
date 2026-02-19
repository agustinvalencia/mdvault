//! Activity logging types.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Operations that can be logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    New,
    Update,
    Complete,
    Cancel,
    Reopen,
    Capture,
    Rename,
    Delete,
    Focus,
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::New => write!(f, "new"),
            Operation::Update => write!(f, "update"),
            Operation::Complete => write!(f, "complete"),
            Operation::Cancel => write!(f, "cancel"),
            Operation::Reopen => write!(f, "reopen"),
            Operation::Capture => write!(f, "capture"),
            Operation::Rename => write!(f, "rename"),
            Operation::Delete => write!(f, "delete"),
            Operation::Focus => write!(f, "focus"),
        }
    }
}

/// A single activity log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// ISO8601 timestamp
    pub ts: DateTime<Utc>,

    /// Operation type
    pub op: Operation,

    /// Note type (task, project, daily, etc.) or "focus" for focus operations
    #[serde(rename = "type")]
    pub note_type: String,

    /// Note ID (e.g., "TST-001", "MCP") - empty string if not applicable
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,

    /// Relative path from vault root
    pub path: PathBuf,

    /// Additional metadata (varies by operation)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub meta: HashMap<String, serde_json::Value>,
}

impl ActivityEntry {
    /// Create a new activity entry with the current timestamp.
    pub fn new(
        op: Operation,
        note_type: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            ts: Utc::now(),
            op,
            note_type: note_type.into(),
            id: String::new(),
            path: path.into(),
            meta: HashMap::new(),
        }
    }

    /// Set the note ID.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Add metadata.
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.meta.insert(key.into(), v);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_display() {
        assert_eq!(Operation::New.to_string(), "new");
        assert_eq!(Operation::Complete.to_string(), "complete");
        assert_eq!(Operation::Focus.to_string(), "focus");
    }

    #[test]
    fn test_activity_entry_serialization() {
        let entry = ActivityEntry::new(Operation::New, "task", "tasks/TST-001.md")
            .with_id("TST-001")
            .with_meta("title", "Test task");

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains(r#""op":"new""#));
        assert!(json.contains(r#""type":"task""#));
        assert!(json.contains(r#""id":"TST-001""#));
        assert!(json.contains(r#""path":"tasks/TST-001.md""#));
        assert!(json.contains(r#""title":"Test task""#));
    }

    #[test]
    fn test_activity_entry_deserialization() {
        let json = r#"{"ts":"2026-01-23T10:00:00Z","op":"complete","type":"task","id":"TST-001","path":"tasks/TST-001.md"}"#;
        let entry: ActivityEntry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.op, Operation::Complete);
        assert_eq!(entry.note_type, "task");
        assert_eq!(entry.id, "TST-001");
    }

    #[test]
    fn test_empty_id_not_serialized() {
        let entry =
            ActivityEntry::new(Operation::Capture, "capture", "daily/2026-01-23.md");
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains(r#""id""#));
    }

    #[test]
    fn test_empty_meta_not_serialized() {
        let entry = ActivityEntry::new(Operation::New, "task", "tasks/TST-001.md");
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains(r#""meta""#));
    }
}
