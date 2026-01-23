//! Activity log service implementation.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::config::types::{ActivityConfig, ResolvedConfig};

use super::types::{ActivityEntry, Operation};

/// Error type for activity logging.
#[derive(Debug, Error)]
pub enum ActivityError {
    #[error("Failed to write activity log: {0}")]
    WriteError(#[from] std::io::Error),

    #[error("Failed to serialize entry: {0}")]
    SerializeError(#[from] serde_json::Error),

    #[error("Activity logging is disabled")]
    Disabled,
}

type Result<T> = std::result::Result<T, ActivityError>;

/// Service for logging vault activities to JSONL file.
pub struct ActivityLogService {
    /// Path to the activity log file
    log_path: PathBuf,

    /// Configuration
    config: ActivityConfig,

    /// Vault root for path relativization
    vault_root: PathBuf,
}

impl ActivityLogService {
    const LOG_FILE: &'static str = ".mdvault/activity.jsonl";
    const ARCHIVE_DIR: &'static str = ".mdvault/activity_archive";

    /// Create a new ActivityLogService for the given vault.
    pub fn new(vault_root: &Path, config: ActivityConfig) -> Self {
        let log_path = vault_root.join(Self::LOG_FILE);
        Self { log_path, config, vault_root: vault_root.to_path_buf() }
    }

    /// Create from ResolvedConfig.
    /// Returns None if activity logging is disabled.
    pub fn try_from_config(config: &ResolvedConfig) -> Option<Self> {
        if config.activity.enabled {
            Some(Self::new(&config.vault_root, config.activity.clone()))
        } else {
            None
        }
    }

    /// Check if logging is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if a specific operation should be logged.
    pub fn should_log(&self, op: Operation) -> bool {
        if !self.config.enabled {
            return false;
        }
        // Empty log_operations means log all operations
        if self.config.log_operations.is_empty() {
            return true;
        }
        self.config.log_operations.contains(&op.to_string())
    }

    /// Log an activity entry.
    pub fn log(&self, entry: ActivityEntry) -> Result<()> {
        if !self.should_log(entry.op) {
            return Ok(()); // Silently skip disabled operations
        }

        // Ensure parent directory exists
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Serialize to JSON line
        let json = serde_json::to_string(&entry)?;

        // Append to file
        let mut file =
            OpenOptions::new().create(true).append(true).open(&self.log_path)?;

        writeln!(file, "{}", json)?;
        Ok(())
    }

    /// Log a "new" operation (note creation).
    pub fn log_new(
        &self,
        note_type: &str,
        id: &str,
        path: &Path,
        title: Option<&str>,
    ) -> Result<()> {
        let rel_path = self.relativize(path);
        let mut entry =
            ActivityEntry::new(Operation::New, note_type, rel_path).with_id(id);

        if let Some(t) = title {
            entry = entry.with_meta("title", t);
        }

        self.log(entry)
    }

    /// Log a "complete" operation (task completed).
    pub fn log_complete(
        &self,
        note_type: &str,
        id: &str,
        path: &Path,
        summary: Option<&str>,
    ) -> Result<()> {
        let rel_path = self.relativize(path);
        let mut entry =
            ActivityEntry::new(Operation::Complete, note_type, rel_path).with_id(id);

        if let Some(s) = summary {
            entry = entry.with_meta("summary", s);
        }

        self.log(entry)
    }

    /// Log a "capture" operation.
    pub fn log_capture(
        &self,
        capture_name: &str,
        target_path: &Path,
        section: Option<&str>,
    ) -> Result<()> {
        let rel_path = self.relativize(target_path);
        let mut entry = ActivityEntry::new(Operation::Capture, "capture", rel_path)
            .with_meta("capture_name", capture_name);

        if let Some(s) = section {
            entry = entry.with_meta("section", s);
        }

        self.log(entry)
    }

    /// Log a "rename" operation.
    pub fn log_rename(
        &self,
        note_type: &str,
        old_path: &Path,
        new_path: &Path,
        references_updated: usize,
    ) -> Result<()> {
        let rel_new = self.relativize(new_path);
        let rel_old = self.relativize(old_path);

        let entry = ActivityEntry::new(Operation::Rename, note_type, rel_new)
            .with_meta("old_path", rel_old.to_string_lossy())
            .with_meta("references_updated", references_updated);

        self.log(entry)
    }

    /// Log a "focus" operation.
    pub fn log_focus(
        &self,
        project: &str,
        note: Option<&str>,
        action: &str,
    ) -> Result<()> {
        let mut entry = ActivityEntry::new(
            Operation::Focus,
            "focus",
            PathBuf::new(), // Focus has no path
        )
        .with_meta("project", project)
        .with_meta("action", action);

        if let Some(n) = note {
            entry = entry.with_meta("note", n);
        }

        self.log(entry)
    }

    /// Relativize a path to the vault root.
    fn relativize(&self, path: &Path) -> PathBuf {
        path.strip_prefix(&self.vault_root).unwrap_or(path).to_path_buf()
    }

    /// Perform log rotation if needed.
    /// Should be called at startup or periodically.
    pub fn rotate_if_needed(&self) -> Result<()> {
        super::rotation::rotate_log(
            &self.log_path,
            &self.vault_root.join(Self::ARCHIVE_DIR),
            self.config.retention_days,
        )
    }

    /// Read entries within a date range (for querying).
    pub fn read_entries(
        &self,
        since: Option<DateTime<Utc>>,
        until: Option<DateTime<Utc>>,
    ) -> Result<Vec<ActivityEntry>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(entry) = serde_json::from_str::<ActivityEntry>(&line) {
                // Filter by date range
                if let Some(s) = since
                    && entry.ts < s
                {
                    continue;
                }
                if let Some(u) = until
                    && entry.ts > u
                {
                    continue;
                }
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    /// Get the path to the log file.
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_test_config(enabled: bool) -> ActivityConfig {
        ActivityConfig { enabled, retention_days: 90, log_operations: vec![] }
    }

    #[test]
    fn test_log_new_creates_entry() {
        let tmp = tempdir().unwrap();
        let service = ActivityLogService::new(tmp.path(), make_test_config(true));

        service
            .log_new(
                "task",
                "TST-001",
                &tmp.path().join("tasks/TST-001.md"),
                Some("Test"),
            )
            .unwrap();

        let content = fs::read_to_string(service.log_path()).unwrap();
        assert!(content.contains(r#""op":"new""#));
        assert!(content.contains(r#""type":"task""#));
        assert!(content.contains(r#""id":"TST-001""#));
    }

    #[test]
    fn test_log_disabled_does_nothing() {
        let tmp = tempdir().unwrap();
        let service = ActivityLogService::new(tmp.path(), make_test_config(false));

        // This should not create any file
        service
            .log_new("task", "TST-001", &tmp.path().join("tasks/TST-001.md"), None)
            .unwrap();

        assert!(!service.log_path().exists());
    }

    #[test]
    fn test_should_log_respects_operations_filter() {
        let config = ActivityConfig {
            enabled: true,
            retention_days: 90,
            log_operations: vec!["new".into()],
        };
        let tmp = tempdir().unwrap();
        let service = ActivityLogService::new(tmp.path(), config);

        assert!(service.should_log(Operation::New));
        assert!(!service.should_log(Operation::Complete));
        assert!(!service.should_log(Operation::Focus));
    }

    #[test]
    fn test_read_entries() {
        let tmp = tempdir().unwrap();
        let service = ActivityLogService::new(tmp.path(), make_test_config(true));

        // Log several entries
        service
            .log_new("task", "TST-001", &tmp.path().join("tasks/TST-001.md"), None)
            .unwrap();
        service
            .log_complete("task", "TST-001", &tmp.path().join("tasks/TST-001.md"), None)
            .unwrap();

        let entries = service.read_entries(None, None).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].op, Operation::New);
        assert_eq!(entries[1].op, Operation::Complete);
    }

    #[test]
    fn test_relativize_path() {
        let tmp = tempdir().unwrap();
        let service = ActivityLogService::new(tmp.path(), make_test_config(true));

        let abs_path = tmp.path().join("tasks/TST-001.md");
        let rel_path = service.relativize(&abs_path);
        assert_eq!(rel_path, PathBuf::from("tasks/TST-001.md"));
    }
}
