//! Domain services for note lifecycle operations.
//!
//! These services handle cross-cutting concerns like daily logging
//! that can be used by multiple behaviors.

use std::fs;
use std::path::Path;

use chrono::Local;

use crate::config::types::ResolvedConfig;

/// Service for logging note creation events to daily notes.
pub struct DailyLogService;

impl DailyLogService {
    /// Log a creation event to today's daily note.
    ///
    /// Creates the daily note if it doesn't exist. The log entry includes
    /// a wikilink to the created note.
    ///
    /// # Arguments
    /// * `config` - Resolved vault configuration
    /// * `note_type` - Type of note created (e.g., "task", "project")
    /// * `title` - Title of the created note
    /// * `note_id` - ID of the note (e.g., "TST-001"), can be empty
    /// * `output_path` - Path where the note was written
    pub fn log_creation(
        config: &ResolvedConfig,
        note_type: &str,
        title: &str,
        note_id: &str,
        output_path: &Path,
    ) -> Result<(), String> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let time = Local::now().format("%H:%M").to_string();
        let year = &today[..4];

        // Build daily note path (default pattern: Journal/{year}/Daily/YYYY-MM-DD.md)
        let daily_path =
            config.vault_root.join(format!("Journal/{}/Daily/{}.md", year, today));

        // Ensure parent directory exists
        if let Some(parent) = daily_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Could not create daily directory: {e}"))?;
        }

        // Read or create daily note
        let mut content = match fs::read_to_string(&daily_path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Create minimal daily note
                let content = format!(
                    "---\ntype: daily\ndate: {}\n---\n\n# {}\n\n## Log\n",
                    today, today
                );
                fs::write(&daily_path, &content)
                    .map_err(|e| format!("Could not create daily note: {e}"))?;
                content
            }
            Err(e) => return Err(format!("Could not read daily note: {e}")),
        };

        // Build the log entry with link to the note
        let rel_path =
            output_path.strip_prefix(&config.vault_root).unwrap_or(output_path);
        let link = rel_path.file_stem().and_then(|s| s.to_str()).unwrap_or("note");

        // Format: "- **HH:MM**: Created task TST-001: [[TST-001|Title]]"
        let id_display =
            if note_id.is_empty() { String::new() } else { format!(" {}", note_id) };

        let log_entry = format!(
            "- **{}**: Created {}{}: [[{}|{}]]\n",
            time, note_type, id_display, link, title
        );

        // Find the Log section and append, or append at end
        if let Some(log_pos) = content.find("## Log") {
            // Find the end of the Log section (next ## or end of file)
            let after_log = &content[log_pos + 6..]; // Skip "## Log"
            let insert_pos = if let Some(next_section) = after_log.find("\n## ") {
                log_pos + 6 + next_section
            } else {
                content.len()
            };

            // Insert the log entry after a newline
            content.insert_str(insert_pos, &format!("\n{}", log_entry));
        } else {
            // No Log section, add one
            content.push_str(&format!("\n## Log\n{}", log_entry));
        }

        // Write back
        fs::write(&daily_path, &content)
            .map_err(|e| format!("Could not write daily note: {e}"))?;

        Ok(())
    }
}

impl DailyLogService {
    /// Log a generic event to today's daily note.
    ///
    /// Used for task completion, cancellation, and other lifecycle events
    /// that should appear in the daily journal.
    ///
    /// # Arguments
    /// * `config` - Resolved vault configuration
    /// * `action` - Action verb (e.g., "Completed", "Cancelled")
    /// * `note_type` - Type of note (e.g., "task")
    /// * `title` - Title of the note
    /// * `note_id` - ID of the note (e.g., "TST-001"), can be empty
    /// * `output_path` - Path to the note file
    pub fn log_event(
        config: &ResolvedConfig,
        action: &str,
        note_type: &str,
        title: &str,
        note_id: &str,
        output_path: &Path,
    ) -> Result<(), String> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let time = Local::now().format("%H:%M").to_string();
        let year = &today[..4];

        let daily_path =
            config.vault_root.join(format!("Journal/{}/Daily/{}.md", year, today));

        if let Some(parent) = daily_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Could not create daily directory: {e}"))?;
        }

        let mut content = match fs::read_to_string(&daily_path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let content = format!(
                    "---\ntype: daily\ndate: {}\n---\n\n# {}\n\n## Log\n",
                    today, today
                );
                fs::write(&daily_path, &content)
                    .map_err(|e| format!("Could not create daily note: {e}"))?;
                content
            }
            Err(e) => return Err(format!("Could not read daily note: {e}")),
        };

        let rel_path =
            output_path.strip_prefix(&config.vault_root).unwrap_or(output_path);
        let link = rel_path.file_stem().and_then(|s| s.to_str()).unwrap_or("note");

        let id_display =
            if note_id.is_empty() { String::new() } else { format!(" {}", note_id) };

        let log_entry = format!(
            "- **{}**: {} {}{}: [[{}|{}]]\n",
            time, action, note_type, id_display, link, title
        );

        if let Some(log_pos) = content.find("## Log") {
            let after_log = &content[log_pos + 6..];
            let insert_pos = if let Some(next_section) = after_log.find("\n## ") {
                log_pos + 6 + next_section
            } else {
                content.len()
            };
            content.insert_str(insert_pos, &format!("\n{}", log_entry));
        } else {
            content.push_str(&format!("\n## Log\n{}", log_entry));
        }

        fs::write(&daily_path, &content)
            .map_err(|e| format!("Could not write daily note: {e}"))?;

        Ok(())
    }
}

/// Service for logging events to project notes.
pub struct ProjectLogService;

impl ProjectLogService {
    /// Append a log entry to a project note's "## Logs" section.
    pub fn log_entry(project_file: &Path, message: &str) -> Result<(), String> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let time = Local::now().format("%H:%M").to_string();

        let content = fs::read_to_string(project_file)
            .map_err(|e| format!("Could not read project note: {e}"))?;

        let log_entry = format!("- [[{}]] - {}: {}\n", today, time, message);

        let new_content = if let Some(log_pos) = content.find("## Logs") {
            let after_log = &content[log_pos + 7..]; // Skip "## Logs"
            let insert_pos = if let Some(next_section) = after_log.find("\n## ") {
                log_pos + 7 + next_section
            } else {
                content.len()
            };
            let mut c = content.clone();
            c.insert_str(insert_pos, &format!("\n{}", log_entry));
            c
        } else {
            format!("{}\n## Logs\n{}", content, log_entry)
        };

        fs::write(project_file, &new_content)
            .map_err(|e| format!("Could not write project note: {e}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_test_config(vault_root: PathBuf) -> ResolvedConfig {
        ResolvedConfig {
            active_profile: "test".into(),
            vault_root: vault_root.clone(),
            templates_dir: vault_root.join(".mdvault/templates"),
            captures_dir: vault_root.join(".mdvault/captures"),
            macros_dir: vault_root.join(".mdvault/macros"),
            typedefs_dir: vault_root.join(".mdvault/typedefs"),
            excluded_folders: vec![],
            security: Default::default(),
            logging: Default::default(),
            activity: Default::default(),
        }
    }

    #[test]
    fn test_log_creation_creates_daily_note() {
        let tmp = tempdir().unwrap();
        let config = make_test_config(tmp.path().to_path_buf());
        let output_path = tmp.path().join("Projects/TST/Tasks/TST-001.md");

        // Create the task file so strip_prefix works
        fs::create_dir_all(output_path.parent().unwrap()).unwrap();
        fs::write(&output_path, "test").unwrap();

        let result = DailyLogService::log_creation(
            &config,
            "task",
            "Test Task",
            "TST-001",
            &output_path,
        );

        assert!(result.is_ok());

        // Check daily note was created
        let today = Local::now().format("%Y-%m-%d").to_string();
        let year = &today[..4];
        let daily_path = tmp.path().join(format!("Journal/{}/Daily/{}.md", year, today));
        assert!(daily_path.exists());

        let content = fs::read_to_string(&daily_path).unwrap();
        assert!(content.contains("type: daily"));
        assert!(content.contains("## Log"));
        assert!(content.contains("Created task TST-001"));
        assert!(content.contains("[[TST-001|Test Task]]"));
    }

    #[test]
    fn test_log_creation_appends_to_existing() {
        let tmp = tempdir().unwrap();
        let config = make_test_config(tmp.path().to_path_buf());

        // Create existing daily note
        let today = Local::now().format("%Y-%m-%d").to_string();
        let year = &today[..4];
        let daily_path = tmp.path().join(format!("Journal/{}/Daily/{}.md", year, today));
        fs::create_dir_all(daily_path.parent().unwrap()).unwrap();
        fs::write(
            &daily_path,
            "---\ntype: daily\n---\n\n# Today\n\n## Log\n- Existing entry\n",
        )
        .unwrap();

        let output_path = tmp.path().join("Projects/NEW/NEW-001.md");
        fs::create_dir_all(output_path.parent().unwrap()).unwrap();
        fs::write(&output_path, "test").unwrap();

        let result = DailyLogService::log_creation(
            &config,
            "project",
            "New Project",
            "NEW",
            &output_path,
        );

        assert!(result.is_ok());

        let content = fs::read_to_string(&daily_path).unwrap();
        assert!(content.contains("- Existing entry"));
        assert!(content.contains("Created project NEW"));
    }

    #[test]
    fn test_project_log_appends_to_existing_logs_section() {
        let tmp = tempdir().unwrap();
        let project_file = tmp.path().join("project.md");
        fs::write(&project_file, "---\ntitle: Test\n---\n\n## Logs\n- Existing log\n")
            .unwrap();

        let result = ProjectLogService::log_entry(
            &project_file,
            "Created task [[TST-001]]: Fix bug",
        );
        assert!(result.is_ok());

        let content = fs::read_to_string(&project_file).unwrap();
        assert!(content.contains("- Existing log"));
        assert!(content.contains("Created task [[TST-001]]: Fix bug"));
        // Should still have the Logs heading
        assert!(content.contains("## Logs"));
    }

    #[test]
    fn test_project_log_creates_logs_section_if_missing() {
        let tmp = tempdir().unwrap();
        let project_file = tmp.path().join("project.md");
        fs::write(&project_file, "---\ntitle: Test\n---\n\nSome content\n").unwrap();

        let result = ProjectLogService::log_entry(
            &project_file,
            "Created task [[TST-002]]: New feature",
        );
        assert!(result.is_ok());

        let content = fs::read_to_string(&project_file).unwrap();
        assert!(content.contains("## Logs"));
        assert!(content.contains("Created task [[TST-002]]: New feature"));
        assert!(content.contains("Some content"));
    }

    #[test]
    fn test_log_event_completed_task() {
        let tmp = tempdir().unwrap();
        let config = make_test_config(tmp.path().to_path_buf());
        let output_path = tmp.path().join("Projects/TST/Tasks/TST-001.md");

        fs::create_dir_all(output_path.parent().unwrap()).unwrap();
        fs::write(&output_path, "test").unwrap();

        let result = DailyLogService::log_event(
            &config,
            "Completed",
            "task",
            "Fix the bug",
            "TST-001",
            &output_path,
        );

        assert!(result.is_ok());

        let today = Local::now().format("%Y-%m-%d").to_string();
        let year = &today[..4];
        let daily_path = tmp.path().join(format!("Journal/{}/Daily/{}.md", year, today));
        assert!(daily_path.exists());

        let content = fs::read_to_string(&daily_path).unwrap();
        assert!(content.contains("Completed task TST-001"));
        assert!(content.contains("[[TST-001|Fix the bug]]"));
    }

    #[test]
    fn test_log_event_cancelled_task() {
        let tmp = tempdir().unwrap();
        let config = make_test_config(tmp.path().to_path_buf());
        let output_path = tmp.path().join("Projects/TST/Tasks/TST-002.md");

        fs::create_dir_all(output_path.parent().unwrap()).unwrap();
        fs::write(&output_path, "test").unwrap();

        let result = DailyLogService::log_event(
            &config,
            "Cancelled",
            "task",
            "Old feature",
            "TST-002",
            &output_path,
        );

        assert!(result.is_ok());

        let today = Local::now().format("%Y-%m-%d").to_string();
        let year = &today[..4];
        let daily_path = tmp.path().join(format!("Journal/{}/Daily/{}.md", year, today));
        let content = fs::read_to_string(&daily_path).unwrap();
        assert!(content.contains("Cancelled task TST-002"));
        assert!(content.contains("[[TST-002|Old feature]]"));
    }

    #[test]
    fn test_project_log_preserves_sections_after_logs() {
        let tmp = tempdir().unwrap();
        let project_file = tmp.path().join("project.md");
        fs::write(
            &project_file,
            "---\ntitle: Test\n---\n\n## Logs\n- Old entry\n\n## Notes\nSome notes\n",
        )
        .unwrap();

        let result = ProjectLogService::log_entry(
            &project_file,
            "Created task [[TST-003]]: Refactor",
        );
        assert!(result.is_ok());

        let content = fs::read_to_string(&project_file).unwrap();
        assert!(content.contains("- Old entry"));
        assert!(content.contains("Created task [[TST-003]]: Refactor"));
        assert!(content.contains("## Notes"));
        assert!(content.contains("Some notes"));
    }
}
