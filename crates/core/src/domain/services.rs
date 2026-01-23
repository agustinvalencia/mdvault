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

        // Build daily note path (default pattern: Journal/Daily/YYYY-MM-DD.md)
        let daily_path = config.vault_root.join(format!("Journal/Daily/{}.md", today));

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
        let daily_path = tmp.path().join(format!("Journal/Daily/{}.md", today));
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
        let daily_path = tmp.path().join(format!("Journal/Daily/{}.md", today));
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
}
