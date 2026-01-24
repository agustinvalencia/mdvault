//! Context query service for day/week aggregation.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Utc};

use crate::activity::{ActivityEntry, ActivityLogService, Operation};
use crate::config::types::ResolvedConfig;
use crate::context::ContextManager;
use crate::frontmatter::parse as parse_frontmatter;
use crate::index::IndexDb;
use crate::markdown_ast::MarkdownEditor;

use super::query_types::*;

/// Service for querying day and week context.
pub struct ContextQueryService {
    /// Vault root path.
    vault_root: PathBuf,

    /// Activity log service.
    activity_service: Option<ActivityLogService>,

    /// Index database (optional).
    index_db: Option<IndexDb>,

    /// Daily note path pattern (relative to vault root).
    daily_note_pattern: String,
}

impl ContextQueryService {
    /// Create a new ContextQueryService.
    pub fn new(config: &ResolvedConfig) -> Self {
        let activity_service = ActivityLogService::try_from_config(config);

        let index_path = config.vault_root.join(".mdvault/index.db");
        let index_db = IndexDb::open(&index_path).ok();

        Self {
            vault_root: config.vault_root.clone(),
            activity_service,
            index_db,
            // TODO: Make configurable
            daily_note_pattern: "Journal/Daily/{date}.md".to_string(),
        }
    }

    /// Get context for a specific day.
    pub fn day_context(&self, date: NaiveDate) -> Result<DayContext, ContextError> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let day_of_week = date.format("%A").to_string();

        let mut context = DayContext::new(&date_str, &day_of_week);

        // Get logged activity for the day
        let activity_entries = self.get_logged_activity(date);

        // Convert to ActivityItems
        for entry in &activity_entries {
            context.activity.push(ActivityItem {
                ts: entry.ts.to_rfc3339(),
                source: "logged".to_string(),
                op: entry.op.to_string(),
                note_type: entry.note_type.clone(),
                id: if entry.id.is_empty() { None } else { Some(entry.id.clone()) },
                path: entry.path.clone(),
                summary: entry.meta.get("title").and_then(|v| v.as_str()).map(String::from),
            });
        }

        // Detect unlogged changes
        let detected = self.detect_unlogged_changes(date, &activity_entries);
        for note in detected {
            context.activity.push(ActivityItem {
                ts: format!("{}T00:00:00Z", date_str),
                source: "detected".to_string(),
                op: "update".to_string(),
                note_type: note.note_type.clone().unwrap_or_default(),
                id: None,
                path: note.path.clone(),
                summary: note.change_summary.clone(),
            });
            context.modified_notes.push(note);
        }

        // Add logged notes to modified_notes
        let mut logged_paths: HashSet<PathBuf> = HashSet::new();
        for entry in &activity_entries {
            if !logged_paths.contains(&entry.path) {
                logged_paths.insert(entry.path.clone());
                context.modified_notes.push(ModifiedNote {
                    path: entry.path.clone(),
                    note_type: Some(entry.note_type.clone()),
                    source: "logged".to_string(),
                    change_summary: Some(entry.op.to_string()),
                });
            }
        }

        // Parse daily note
        context.daily_note = self.parse_daily_note(date);

        // Aggregate tasks
        context.tasks = self.aggregate_tasks(&activity_entries);

        // Get focus context
        context.summary.focus = self.get_focus_for_day(date);

        // Calculate summary
        context.summary.tasks_completed = context.tasks.completed.len() as u32;
        context.summary.tasks_created = context.tasks.created.len() as u32;
        context.summary.notes_modified = context.modified_notes.len() as u32;

        // Aggregate project activity
        context.projects = self.aggregate_projects(&activity_entries);

        Ok(context)
    }

    /// Get context for a specific week.
    pub fn week_context(&self, date: NaiveDate) -> Result<WeekContext, ContextError> {
        // Get Monday of the week containing the date
        let days_from_monday = date.weekday().num_days_from_monday();
        let monday = date - Duration::days(days_from_monday as i64);
        let sunday = monday + Duration::days(6);

        let week_str = monday.format("%G-W%V").to_string();
        let start_str = monday.format("%Y-%m-%d").to_string();
        let end_str = sunday.format("%Y-%m-%d").to_string();

        let mut context = WeekContext {
            week: week_str,
            start_date: start_str,
            end_date: end_str,
            summary: WeekSummary::default(),
            days: Vec::new(),
            tasks: TaskActivity::default(),
            projects: Vec::new(),
        };

        // Collect data for each day
        let mut all_entries: Vec<ActivityEntry> = Vec::new();
        let mut project_map: HashMap<String, ProjectActivity> = HashMap::new();

        for i in 0..7 {
            let day = monday + Duration::days(i);
            let day_context = self.day_context(day)?;

            // Add to days list
            context.days.push(DaySummaryWithDate {
                date: day.format("%Y-%m-%d").to_string(),
                day_of_week: day.format("%A").to_string(),
                summary: day_context.summary.clone(),
            });

            // Accumulate summary
            context.summary.tasks_completed += day_context.summary.tasks_completed;
            context.summary.tasks_created += day_context.summary.tasks_created;
            context.summary.notes_modified += day_context.summary.notes_modified;

            if day_context.summary.tasks_completed > 0
                || day_context.summary.tasks_created > 0
                || day_context.summary.notes_modified > 0
            {
                context.summary.active_days += 1;
            }

            // Accumulate tasks
            context.tasks.completed.extend(day_context.tasks.completed);
            context.tasks.created.extend(day_context.tasks.created);

            // Accumulate project activity
            for proj in day_context.projects {
                let entry = project_map.entry(proj.name.clone()).or_insert(ProjectActivity {
                    name: proj.name,
                    tasks_done: 0,
                    tasks_active: 0,
                    logs_added: 0,
                });
                entry.tasks_done += proj.tasks_done;
                entry.tasks_active = entry.tasks_active.max(proj.tasks_active);
                entry.logs_added += proj.logs_added;
            }

            // Get logged entries for in-progress calculation
            all_entries.extend(self.get_logged_activity(day));
        }

        // Set in-progress tasks (query current state, not historical)
        context.tasks.in_progress = self.get_in_progress_tasks();

        // Convert project map to vec
        context.projects = project_map.into_values().collect();
        context.projects.sort_by(|a, b| b.tasks_done.cmp(&a.tasks_done));

        Ok(context)
    }

    /// Get logged activity entries for a specific day.
    fn get_logged_activity(&self, date: NaiveDate) -> Vec<ActivityEntry> {
        let Some(ref activity) = self.activity_service else {
            return Vec::new();
        };

        // Create start and end times for the day (in UTC)
        let start = Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let end = Local
            .from_local_datetime(&date.succ_opt().unwrap().and_hms_opt(0, 0, 0).unwrap())
            .unwrap()
            .with_timezone(&Utc);

        activity.read_entries(Some(start), Some(end)).unwrap_or_default()
    }

    /// Detect files modified on the given date that weren't logged.
    fn detect_unlogged_changes(
        &self,
        date: NaiveDate,
        logged_entries: &[ActivityEntry],
    ) -> Vec<ModifiedNote> {
        let mut result = Vec::new();

        // Collect paths already in activity log
        let logged_paths: HashSet<PathBuf> =
            logged_entries.iter().map(|e| e.path.clone()).collect();

        // Walk vault and check mtimes
        let walker = walkdir::WalkDir::new(&self.vault_root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && !name.starts_with('_')
            });

        for entry in walker.filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            if path.extension().map(|e| e != "md").unwrap_or(true) {
                continue;
            }

            // Get relative path
            let rel_path = match path.strip_prefix(&self.vault_root) {
                Ok(p) => p.to_path_buf(),
                Err(_) => continue,
            };

            // Skip if already logged
            if logged_paths.contains(&rel_path) {
                continue;
            }

            // Check modification time
            let metadata = match fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let mtime = match metadata.modified() {
                Ok(t) => t,
                Err(_) => continue,
            };

            let mtime_date: chrono::DateTime<Local> = mtime.into();
            if mtime_date.date_naive() == date {
                // Try to get note type from frontmatter
                let note_type = fs::read_to_string(path)
                    .ok()
                    .and_then(|content| parse_frontmatter(&content).ok())
                    .and_then(|doc| doc.frontmatter)
                    .and_then(|fm| fm.fields.get("type").cloned())
                    .and_then(|v| match v {
                        serde_yaml::Value::String(s) => Some(s),
                        _ => None,
                    });

                result.push(ModifiedNote {
                    path: rel_path,
                    note_type,
                    source: "detected".to_string(),
                    change_summary: Some("modified".to_string()),
                });
            }
        }

        result
    }

    /// Parse daily note for sections and log count.
    fn parse_daily_note(&self, date: NaiveDate) -> Option<DailyNoteInfo> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let rel_path = self.daily_note_pattern.replace("{date}", &date_str);
        let path = self.vault_root.join(&rel_path);

        let exists = path.exists();

        if !exists {
            return Some(DailyNoteInfo {
                path: PathBuf::from(rel_path),
                exists: false,
                sections: Vec::new(),
                log_count: 0,
            });
        }

        let content = fs::read_to_string(&path).ok()?;

        // Extract headings using MarkdownEditor
        let headings = MarkdownEditor::find_headings(&content);
        let sections: Vec<String> = headings.iter().map(|h| h.title.clone()).collect();

        // Count log entries (lines starting with "- ")
        let log_count = content.lines().filter(|line| line.trim_start().starts_with("- ")).count();

        Some(DailyNoteInfo {
            path: PathBuf::from(rel_path),
            exists: true,
            sections,
            log_count: log_count as u32,
        })
    }

    /// Aggregate task activity from entries.
    fn aggregate_tasks(&self, entries: &[ActivityEntry]) -> TaskActivity {
        let mut activity = TaskActivity::default();

        for entry in entries {
            if entry.note_type != "task" {
                continue;
            }

            let title = entry
                .meta
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();

            let project = entry
                .meta
                .get("project")
                .and_then(|v| v.as_str())
                .map(String::from);

            let task_info = TaskInfo {
                id: entry.id.clone(),
                title,
                project,
                path: entry.path.clone(),
            };

            match entry.op {
                Operation::New => activity.created.push(task_info),
                Operation::Complete => activity.completed.push(task_info),
                _ => {}
            }
        }

        // Get in-progress tasks from index
        activity.in_progress = self.get_in_progress_tasks();

        activity
    }

    /// Get currently in-progress tasks from index.
    fn get_in_progress_tasks(&self) -> Vec<TaskInfo> {
        let Some(ref db) = self.index_db else {
            return Vec::new();
        };

        use crate::index::{NoteQuery, NoteType};

        let query = NoteQuery { note_type: Some(NoteType::Task), ..Default::default() };

        let tasks = match db.query_notes(&query) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

        tasks
            .into_iter()
            .filter_map(|note| {
                // Parse frontmatter to get status
                let fm: serde_json::Value =
                    note.frontmatter_json.as_ref().and_then(|s| serde_json::from_str(s).ok())?;

                let status = fm.get("status").and_then(|v| v.as_str()).unwrap_or("todo");

                if status == "doing" || status == "in_progress" || status == "in-progress" {
                    let id = fm
                        .get("task-id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let project = fm.get("project").and_then(|v| v.as_str()).map(String::from);

                    Some(TaskInfo { id, title: note.title, project, path: note.path })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get focus for a specific day.
    fn get_focus_for_day(&self, _date: NaiveDate) -> Option<String> {
        // For now, just return current focus
        // TODO: Could query activity log for focus changes on that day
        ContextManager::load(&self.vault_root)
            .ok()
            .and_then(|mgr| mgr.active_project().map(String::from))
    }

    /// Aggregate project activity from entries.
    fn aggregate_projects(&self, entries: &[ActivityEntry]) -> Vec<ProjectActivity> {
        let mut project_map: HashMap<String, ProjectActivity> = HashMap::new();

        for entry in entries {
            // Try to get project from meta or from path
            let project = entry
                .meta
                .get("project")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| self.extract_project_from_path(&entry.path));

            let Some(project_name) = project else {
                continue;
            };

            let proj =
                project_map.entry(project_name.clone()).or_insert(ProjectActivity {
                    name: project_name,
                    tasks_done: 0,
                    tasks_active: 0,
                    logs_added: 0,
                });

            match entry.op {
                Operation::Complete if entry.note_type == "task" => {
                    proj.tasks_done += 1;
                }
                Operation::New if entry.note_type == "task" => {
                    proj.tasks_active += 1;
                }
                Operation::Capture => {
                    proj.logs_added += 1;
                }
                _ => {}
            }
        }

        let mut result: Vec<ProjectActivity> = project_map.into_values().collect();
        result.sort_by(|a, b| b.tasks_done.cmp(&a.tasks_done));
        result
    }

    /// Extract project name from a path like "Projects/MyProject/Tasks/TST-001.md".
    fn extract_project_from_path(&self, path: &Path) -> Option<String> {
        let path_str = path.to_string_lossy();
        let parts: Vec<&str> = path_str.split('/').collect();

        if parts.len() >= 2 && parts[0] == "Projects" {
            Some(parts[1].to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_day_context_empty() {
        let tmp = tempdir().unwrap();
        let config = ResolvedConfig {
            vault_root: tmp.path().to_path_buf(),
            activity: Default::default(),
            ..make_test_config(tmp.path().to_path_buf())
        };

        let service = ContextQueryService::new(&config);
        let today = Local::now().date_naive();
        let context = service.day_context(today).unwrap();

        assert_eq!(context.summary.tasks_completed, 0);
        assert_eq!(context.summary.tasks_created, 0);
    }

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
}
