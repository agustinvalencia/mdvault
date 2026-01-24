//! Context query types for day/week aggregation.

use std::path::PathBuf;

use serde::Serialize;
use thiserror::Error;

/// Error type for context queries.
#[derive(Debug, Error)]
pub enum ContextError {
    #[error("Failed to read activity log: {0}")]
    ActivityError(String),

    #[error("Failed to query index: {0}")]
    IndexError(String),

    #[error("Invalid date: {0}")]
    InvalidDate(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Context for a specific day.
#[derive(Debug, Clone, Serialize)]
pub struct DayContext {
    /// Date in YYYY-MM-DD format.
    pub date: String,

    /// Day of week (e.g., "Thursday").
    pub day_of_week: String,

    /// Summary statistics.
    pub summary: DaySummary,

    /// Daily note information (if exists).
    pub daily_note: Option<DailyNoteInfo>,

    /// Task activity for the day.
    pub tasks: TaskActivity,

    /// All activity entries for the day.
    pub activity: Vec<ActivityItem>,

    /// Notes modified on this day.
    pub modified_notes: Vec<ModifiedNote>,

    /// Project activity summary.
    pub projects: Vec<ProjectActivity>,
}

/// Summary statistics for a day.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DaySummary {
    /// Number of tasks completed.
    pub tasks_completed: u32,

    /// Number of tasks created.
    pub tasks_created: u32,

    /// Number of notes modified.
    pub notes_modified: u32,

    /// Active focus project (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
}

/// Information about a daily note.
#[derive(Debug, Clone, Serialize)]
pub struct DailyNoteInfo {
    /// Path to the daily note.
    pub path: PathBuf,

    /// Whether the daily note exists.
    pub exists: bool,

    /// Section headings in the daily note.
    pub sections: Vec<String>,

    /// Number of log entries (lines starting with `- `).
    pub log_count: u32,
}

/// Task activity aggregation.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TaskActivity {
    /// Tasks completed on this day.
    pub completed: Vec<TaskInfo>,

    /// Tasks created on this day.
    pub created: Vec<TaskInfo>,

    /// Tasks currently in progress.
    pub in_progress: Vec<TaskInfo>,
}

/// Information about a task.
#[derive(Debug, Clone, Serialize)]
pub struct TaskInfo {
    /// Task ID (e.g., "TST-001").
    pub id: String,

    /// Task title.
    pub title: String,

    /// Associated project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,

    /// Path to the task note.
    pub path: PathBuf,
}

/// A single activity item (logged or detected).
#[derive(Debug, Clone, Serialize)]
pub struct ActivityItem {
    /// Timestamp (ISO 8601).
    pub ts: String,

    /// Source: "logged" (from activity log) or "detected" (from file mtime).
    pub source: String,

    /// Operation type.
    pub op: String,

    /// Note type (task, project, daily, etc.).
    pub note_type: String,

    /// Note ID (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Path to the note.
    pub path: PathBuf,

    /// Summary or description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// A modified note entry.
#[derive(Debug, Clone, Serialize)]
pub struct ModifiedNote {
    /// Path to the note.
    pub path: PathBuf,

    /// Note type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_type: Option<String>,

    /// Source: "logged" or "detected".
    pub source: String,

    /// Change summary (e.g., "new", "+2 logs").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_summary: Option<String>,
}

/// Project activity summary.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectActivity {
    /// Project name/ID.
    pub name: String,

    /// Tasks completed.
    pub tasks_done: u32,

    /// Tasks currently active.
    pub tasks_active: u32,

    /// Log entries added.
    pub logs_added: u32,
}

/// Context for a specific week.
#[derive(Debug, Clone, Serialize)]
pub struct WeekContext {
    /// ISO week identifier (e.g., "2026-W04").
    pub week: String,

    /// Start date (Monday) in YYYY-MM-DD format.
    pub start_date: String,

    /// End date (Sunday) in YYYY-MM-DD format.
    pub end_date: String,

    /// Summary statistics for the week.
    pub summary: WeekSummary,

    /// Per-day summaries.
    pub days: Vec<DaySummaryWithDate>,

    /// Task activity for the week.
    pub tasks: TaskActivity,

    /// Project activity for the week.
    pub projects: Vec<ProjectActivity>,
}

/// Summary statistics for a week.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WeekSummary {
    /// Total tasks completed.
    pub tasks_completed: u32,

    /// Total tasks created.
    pub tasks_created: u32,

    /// Total notes modified.
    pub notes_modified: u32,

    /// Days with activity.
    pub active_days: u32,
}

/// Day summary with date for week context.
#[derive(Debug, Clone, Serialize)]
pub struct DaySummaryWithDate {
    /// Date in YYYY-MM-DD format.
    pub date: String,

    /// Day of week.
    pub day_of_week: String,

    /// Summary for this day.
    #[serde(flatten)]
    pub summary: DaySummary,
}

impl DayContext {
    /// Create a new empty DayContext for a given date.
    pub fn new(date: &str, day_of_week: &str) -> Self {
        Self {
            date: date.to_string(),
            day_of_week: day_of_week.to_string(),
            summary: DaySummary::default(),
            daily_note: None,
            tasks: TaskActivity::default(),
            activity: Vec::new(),
            modified_notes: Vec::new(),
            projects: Vec::new(),
        }
    }

    /// Format as markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();

        // Header
        out.push_str(&format!("# Context: {} ({})\n\n", self.date, self.day_of_week));

        // Summary
        out.push_str("## Summary\n");
        out.push_str(&format!("- {} tasks completed\n", self.summary.tasks_completed));
        out.push_str(&format!("- {} tasks created\n", self.summary.tasks_created));
        out.push_str(&format!("- {} notes modified\n", self.summary.notes_modified));
        if let Some(ref focus) = self.summary.focus {
            out.push_str(&format!("- Focus: {}\n", focus));
        }
        out.push('\n');

        // Daily note
        if let Some(ref daily) = self.daily_note {
            out.push_str("## Daily Note\n");
            out.push_str(&format!("- Path: {}\n", daily.path.display()));
            if daily.exists {
                if !daily.sections.is_empty() {
                    out.push_str(&format!("- Sections: {}\n", daily.sections.join(", ")));
                }
                out.push_str(&format!("- Log entries: {}\n", daily.log_count));
            } else {
                out.push_str("- (does not exist)\n");
            }
            out.push('\n');
        }

        // Task activity
        if !self.tasks.completed.is_empty()
            || !self.tasks.created.is_empty()
            || !self.tasks.in_progress.is_empty()
        {
            out.push_str("## Task Activity\n\n");

            if !self.tasks.completed.is_empty() {
                out.push_str(&format!(
                    "### Completed ({})\n",
                    self.tasks.completed.len()
                ));
                out.push_str("| Task | Title | Project |\n");
                out.push_str("|------|-------|--------|\n");
                for task in &self.tasks.completed {
                    let project = task.project.as_deref().unwrap_or("-");
                    out.push_str(&format!(
                        "| {} | {} | {} |\n",
                        task.id, task.title, project
                    ));
                }
                out.push('\n');
            }

            if !self.tasks.created.is_empty() {
                out.push_str(&format!("### Created ({})\n", self.tasks.created.len()));
                out.push_str("| Task | Title | Project |\n");
                out.push_str("|------|-------|--------|\n");
                for task in &self.tasks.created {
                    let project = task.project.as_deref().unwrap_or("-");
                    out.push_str(&format!(
                        "| {} | {} | {} |\n",
                        task.id, task.title, project
                    ));
                }
                out.push('\n');
            }

            if !self.tasks.in_progress.is_empty() {
                out.push_str(&format!(
                    "### In Progress ({})\n",
                    self.tasks.in_progress.len()
                ));
                out.push_str("| Task | Title | Project |\n");
                out.push_str("|------|-------|--------|\n");
                for task in &self.tasks.in_progress {
                    let project = task.project.as_deref().unwrap_or("-");
                    out.push_str(&format!(
                        "| {} | {} | {} |\n",
                        task.id, task.title, project
                    ));
                }
                out.push('\n');
            }
        }

        // Modified notes
        if !self.modified_notes.is_empty() {
            out.push_str(&format!("## Modified Notes ({})\n", self.modified_notes.len()));
            out.push_str("| Note | Type | Source |\n");
            out.push_str("|------|------|--------|\n");
            for note in &self.modified_notes {
                let note_type = note.note_type.as_deref().unwrap_or("-");
                let summary = note.change_summary.as_deref().unwrap_or(&note.source);
                out.push_str(&format!(
                    "| {} | {} | {} |\n",
                    note.path.display(),
                    note_type,
                    summary
                ));
            }
            out.push('\n');
        }

        // Projects
        if !self.projects.is_empty() {
            out.push_str("## Projects with Activity\n");
            out.push_str("| Project | Tasks Done | Tasks Active | Logs Added |\n");
            out.push_str("|---------|------------|--------------|------------|\n");
            for proj in &self.projects {
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    proj.name, proj.tasks_done, proj.tasks_active, proj.logs_added
                ));
            }
        }

        out
    }

    /// Format as one-line summary.
    pub fn to_summary(&self) -> String {
        format!(
            "{}: {} done, {} new, {} notes modified",
            self.date,
            self.summary.tasks_completed,
            self.summary.tasks_created,
            self.summary.notes_modified
        )
    }
}

impl WeekContext {
    /// Format as markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();

        // Header
        out.push_str(&format!(
            "# Context: Week {} ({} to {})\n\n",
            self.week, self.start_date, self.end_date
        ));

        // Summary
        out.push_str("## Summary\n");
        out.push_str(&format!("- {} tasks completed\n", self.summary.tasks_completed));
        out.push_str(&format!("- {} tasks created\n", self.summary.tasks_created));
        out.push_str(&format!("- {} notes modified\n", self.summary.notes_modified));
        out.push_str(&format!("- {} active days\n", self.summary.active_days));
        out.push('\n');

        // Daily breakdown
        out.push_str("## Daily Breakdown\n");
        out.push_str("| Date | Day | Completed | Created | Modified |\n");
        out.push_str("|------|-----|-----------|---------|----------|\n");
        for day in &self.days {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                day.date,
                day.day_of_week,
                day.summary.tasks_completed,
                day.summary.tasks_created,
                day.summary.notes_modified
            ));
        }
        out.push('\n');

        // Projects
        if !self.projects.is_empty() {
            out.push_str("## Projects\n");
            out.push_str("| Project | Tasks Done | Tasks Active | Logs Added |\n");
            out.push_str("|---------|------------|--------------|------------|\n");
            for proj in &self.projects {
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    proj.name, proj.tasks_done, proj.tasks_active, proj.logs_added
                ));
            }
        }

        out
    }

    /// Format as one-line summary.
    pub fn to_summary(&self) -> String {
        format!(
            "{}: {} done, {} new, {} notes modified over {} days",
            self.week,
            self.summary.tasks_completed,
            self.summary.tasks_created,
            self.summary.notes_modified,
            self.summary.active_days
        )
    }
}

// ============================================================================
// Note Context Types
// ============================================================================

/// Context for a specific note.
#[derive(Debug, Clone, Serialize)]
pub struct NoteContext {
    /// Note type (project, task, daily, etc.).
    pub note_type: String,

    /// Path to the note.
    pub path: PathBuf,

    /// Note title.
    pub title: String,

    /// Frontmatter metadata.
    pub metadata: serde_json::Value,

    /// Section headings in the note.
    pub sections: Vec<String>,

    /// Task counts (for projects).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<TaskCounts>,

    /// Recent task activity (for projects).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_tasks: Option<RecentTasks>,

    /// Recent activity related to this note.
    pub activity: NoteActivity,

    /// References (backlinks and outgoing links).
    pub references: NoteReferences,
}

/// Task status counts for a project.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TaskCounts {
    /// Total number of tasks.
    pub total: u32,

    /// Tasks with status "todo".
    pub todo: u32,

    /// Tasks with status "doing" or "in-progress".
    pub doing: u32,

    /// Tasks with status "done" or "completed".
    pub done: u32,

    /// Tasks with status "blocked" or "waiting".
    pub blocked: u32,
}

/// Recent task activity for a project.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RecentTasks {
    /// Recently completed tasks.
    pub completed: Vec<TaskInfo>,

    /// Currently active tasks.
    pub active: Vec<TaskInfo>,
}

/// Activity entries related to a note.
#[derive(Debug, Clone, Serialize)]
pub struct NoteActivity {
    /// Number of days of activity included.
    pub period_days: u32,

    /// Activity entries.
    pub entries: Vec<ActivityItem>,
}

impl Default for NoteActivity {
    fn default() -> Self {
        Self { period_days: 7, entries: Vec::new() }
    }
}

/// References for a note (backlinks and outgoing links).
#[derive(Debug, Clone, Default, Serialize)]
pub struct NoteReferences {
    /// Notes that link to this note.
    pub backlinks: Vec<LinkInfo>,

    /// Total count of backlinks.
    pub backlink_count: u32,

    /// Notes that this note links to.
    pub outgoing: Vec<LinkInfo>,

    /// Total count of outgoing links.
    pub outgoing_count: u32,
}

/// Information about a link.
#[derive(Debug, Clone, Serialize)]
pub struct LinkInfo {
    /// Path to the linked note.
    pub path: PathBuf,

    /// Title of the linked note.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Link text (if different from title).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_text: Option<String>,
}

/// Focus context output.
#[derive(Debug, Clone, Serialize)]
pub struct FocusContextOutput {
    /// Focused project name.
    pub project: String,

    /// Path to the project note.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<PathBuf>,

    /// When focus was started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,

    /// Note about current work.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,

    /// Full project context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Box<NoteContext>>,
}

impl NoteContext {
    /// Format as markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();

        // Header
        out.push_str(&format!(
            "# Context: {} ({})\n\n",
            self.path.display(),
            self.note_type
        ));

        // Metadata
        out.push_str("## Metadata\n");
        if let Some(obj) = self.metadata.as_object() {
            for (key, value) in obj {
                let val_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => value.to_string(),
                };
                out.push_str(&format!("- **{}**: {}\n", key, val_str));
            }
        }
        out.push('\n');

        // Sections
        if !self.sections.is_empty() {
            out.push_str("## Sections\n");
            out.push_str(&self.sections.join(", "));
            out.push_str("\n\n");
        }

        // Tasks (for projects)
        if let Some(ref tasks) = self.tasks {
            out.push_str("## Tasks\n");
            out.push_str("| Status | Count |\n");
            out.push_str("|--------|-------|\n");
            out.push_str(&format!("| Done | {} |\n", tasks.done));
            out.push_str(&format!("| In Progress | {} |\n", tasks.doing));
            out.push_str(&format!("| Todo | {} |\n", tasks.todo));
            out.push_str(&format!("| Blocked | {} |\n", tasks.blocked));
            out.push('\n');
        }

        // Recent tasks
        if let Some(ref recent) = self.recent_tasks {
            if !recent.completed.is_empty() {
                out.push_str("### Recent Completed\n");
                for task in &recent.completed {
                    out.push_str(&format!("- {}: {}\n", task.id, task.title));
                }
                out.push('\n');
            }

            if !recent.active.is_empty() {
                out.push_str("### Active\n");
                for task in &recent.active {
                    out.push_str(&format!("- {}: {}\n", task.id, task.title));
                }
                out.push('\n');
            }
        }

        // Activity
        if !self.activity.entries.is_empty() {
            out.push_str(&format!("## Activity ({} days)\n", self.activity.period_days));
            out.push_str("| Date | Operation | Summary |\n");
            out.push_str("|------|-----------|--------|\n");
            for entry in &self.activity.entries {
                let date = entry.ts.split('T').next().unwrap_or(&entry.ts);
                let summary = entry.summary.as_deref().unwrap_or("-");
                out.push_str(&format!("| {} | {} | {} |\n", date, entry.op, summary));
            }
            out.push('\n');
        }

        // References
        out.push_str("## References\n");
        out.push_str(&format!("- **Backlinks ({})**: ", self.references.backlink_count));
        if self.references.backlinks.is_empty() {
            out.push_str("(none)");
        } else {
            let paths: Vec<String> = self
                .references
                .backlinks
                .iter()
                .take(5)
                .map(|l| l.path.display().to_string())
                .collect();
            out.push_str(&paths.join(", "));
            if self.references.backlink_count > 5 {
                out.push_str(", ...");
            }
        }
        out.push('\n');

        out.push_str(&format!("- **Outgoing ({})**: ", self.references.outgoing_count));
        if self.references.outgoing.is_empty() {
            out.push_str("(none)");
        } else {
            let paths: Vec<String> = self
                .references
                .outgoing
                .iter()
                .take(5)
                .map(|l| l.path.display().to_string())
                .collect();
            out.push_str(&paths.join(", "));
            if self.references.outgoing_count > 5 {
                out.push_str(", ...");
            }
        }
        out.push('\n');

        out
    }

    /// Format as one-line summary.
    pub fn to_summary(&self) -> String {
        let tasks_str = if let Some(ref tasks) = self.tasks {
            format!(", {} done/{} doing/{} todo", tasks.done, tasks.doing, tasks.todo)
        } else {
            String::new()
        };

        format!(
            "{} ({}){}, {} backlinks",
            self.path.display(),
            self.note_type,
            tasks_str,
            self.references.backlink_count
        )
    }
}

impl FocusContextOutput {
    /// Format as markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();

        out.push_str("# Focus Context\n\n");
        out.push_str(&format!("- **Project**: {}\n", self.project));

        if let Some(ref path) = self.project_path {
            out.push_str(&format!("- **Path**: {}\n", path.display()));
        }

        if let Some(ref started) = self.started_at {
            out.push_str(&format!("- **Started**: {}\n", started));
        }

        if let Some(ref note) = self.note {
            out.push_str(&format!("- **Note**: {}\n", note));
        }

        out.push('\n');

        if let Some(ref ctx) = self.context {
            out.push_str("## Project Summary\n\n");
            // Include task counts if available
            if let Some(ref tasks) = ctx.tasks {
                out.push_str("| Status | Count |\n");
                out.push_str("|--------|-------|\n");
                out.push_str(&format!("| Done | {} |\n", tasks.done));
                out.push_str(&format!("| In Progress | {} |\n", tasks.doing));
                out.push_str(&format!("| Todo | {} |\n", tasks.todo));
                out.push_str(&format!("| Blocked | {} |\n", tasks.blocked));
                out.push('\n');
            }

            // Include active tasks
            if let Some(ref recent) = ctx.recent_tasks
                && !recent.active.is_empty()
            {
                out.push_str("### Active Tasks\n");
                for task in &recent.active {
                    out.push_str(&format!("- {}: {}\n", task.id, task.title));
                }
                out.push('\n');
            }
        }

        out
    }

    /// Format as one-line summary.
    pub fn to_summary(&self) -> String {
        let tasks_str = if let Some(ref ctx) = self.context {
            if let Some(ref tasks) = ctx.tasks {
                format!(" ({} done, {} doing)", tasks.done, tasks.doing)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        format!("Focus: {}{}", self.project, tasks_str)
    }
}
