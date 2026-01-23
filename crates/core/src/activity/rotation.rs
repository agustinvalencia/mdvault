//! Log rotation for activity logs.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use chrono::{Datelike, Duration, Utc};

use super::service::ActivityError;
use super::types::ActivityEntry;

type Result<T> = std::result::Result<T, ActivityError>;

/// Rotate the activity log if entries exceed retention period.
///
/// Strategy:
/// 1. Read all entries from current log
/// 2. Partition into "keep" (within retention) and "archive" (older)
/// 3. Archive older entries by month (e.g., activity_2024-12.jsonl)
/// 4. Rewrite current log with only recent entries
pub fn rotate_log(
    log_path: &Path,
    archive_dir: &Path,
    retention_days: u32,
) -> Result<()> {
    if !log_path.exists() {
        return Ok(());
    }

    let cutoff = Utc::now() - Duration::days(retention_days as i64);

    // Read all entries
    let file = File::open(log_path)?;
    let reader = BufReader::new(file);

    let mut keep: Vec<String> = Vec::new();
    let mut archive: HashMap<String, Vec<String>> = HashMap::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        // Parse to check timestamp
        if let Ok(entry) = serde_json::from_str::<ActivityEntry>(&line) {
            if entry.ts >= cutoff {
                keep.push(line);
            } else {
                // Group by month for archival
                let month_key = format!("{}-{:02}", entry.ts.year(), entry.ts.month());
                archive.entry(month_key).or_default().push(line);
            }
        } else {
            // Keep unparseable lines (shouldn't happen, but safe)
            keep.push(line);
        }
    }

    // If nothing to archive, we're done
    if archive.is_empty() {
        return Ok(());
    }

    // Create archive directory
    fs::create_dir_all(archive_dir)?;

    // Write archived entries by month
    for (month_key, entries) in archive {
        let archive_path = archive_dir.join(format!("activity_{}.jsonl", month_key));
        let mut file =
            OpenOptions::new().create(true).append(true).open(&archive_path)?;

        for entry in entries {
            writeln!(file, "{}", entry)?;
        }
    }

    // Rewrite current log with only recent entries
    let mut file = File::create(log_path)?;
    for entry in keep {
        writeln!(file, "{}", entry)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::types::Operation;
    use tempfile::tempdir;

    #[test]
    fn test_rotate_log_no_file() {
        let tmp = tempdir().unwrap();
        let log_path = tmp.path().join("nonexistent.jsonl");
        let archive_dir = tmp.path().join("archive");

        // Should not error on non-existent file
        rotate_log(&log_path, &archive_dir, 90).unwrap();
    }

    #[test]
    fn test_rotate_log_keeps_recent() {
        let tmp = tempdir().unwrap();
        let log_path = tmp.path().join("activity.jsonl");
        let archive_dir = tmp.path().join("archive");

        // Create a log file with recent entries
        let recent_entry = ActivityEntry::new(Operation::New, "task", "tasks/TST-001.md");
        let json = serde_json::to_string(&recent_entry).unwrap();
        fs::write(&log_path, format!("{}\n", json)).unwrap();

        rotate_log(&log_path, &archive_dir, 90).unwrap();

        // Recent entry should still be there
        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("TST-001"));

        // No archive should be created
        assert!(!archive_dir.exists());
    }

    #[test]
    fn test_rotate_log_archives_old() {
        let tmp = tempdir().unwrap();
        let log_path = tmp.path().join("activity.jsonl");
        let archive_dir = tmp.path().join("archive");

        // Create an old entry (100 days ago)
        let old_ts = Utc::now() - Duration::days(100);
        let old_entry = ActivityEntry {
            ts: old_ts,
            op: Operation::New,
            note_type: "task".into(),
            id: "OLD-001".into(),
            path: "tasks/OLD-001.md".into(),
            meta: HashMap::new(),
        };

        // Create a recent entry
        let recent_entry = ActivityEntry::new(Operation::New, "task", "tasks/NEW-001.md")
            .with_id("NEW-001");

        let mut content = String::new();
        content.push_str(&serde_json::to_string(&old_entry).unwrap());
        content.push('\n');
        content.push_str(&serde_json::to_string(&recent_entry).unwrap());
        content.push('\n');

        fs::write(&log_path, &content).unwrap();

        rotate_log(&log_path, &archive_dir, 90).unwrap();

        // Recent entry should still be in main log
        let main_content = fs::read_to_string(&log_path).unwrap();
        assert!(main_content.contains("NEW-001"));
        assert!(!main_content.contains("OLD-001"));

        // Old entry should be archived
        assert!(archive_dir.exists());
        let archive_files: Vec<_> = fs::read_dir(&archive_dir).unwrap().collect();
        assert_eq!(archive_files.len(), 1);

        let archive_content =
            fs::read_to_string(archive_files[0].as_ref().unwrap().path()).unwrap();
        assert!(archive_content.contains("OLD-001"));
    }
}
