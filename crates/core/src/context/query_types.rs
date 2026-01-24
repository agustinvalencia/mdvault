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
                out.push_str(&format!("### Completed ({})\n", self.tasks.completed.len()));
                out.push_str("| Task | Title | Project |\n");
                out.push_str("|------|-------|--------|\n");
                for task in &self.tasks.completed {
                    let project = task.project.as_deref().unwrap_or("-");
                    out.push_str(&format!("| {} | {} | {} |\n", task.id, task.title, project));
                }
                out.push('\n');
            }

            if !self.tasks.created.is_empty() {
                out.push_str(&format!("### Created ({})\n", self.tasks.created.len()));
                out.push_str("| Task | Title | Project |\n");
                out.push_str("|------|-------|--------|\n");
                for task in &self.tasks.created {
                    let project = task.project.as_deref().unwrap_or("-");
                    out.push_str(&format!("| {} | {} | {} |\n", task.id, task.title, project));
                }
                out.push('\n');
            }

            if !self.tasks.in_progress.is_empty() {
                out.push_str(&format!("### In Progress ({})\n", self.tasks.in_progress.len()));
                out.push_str("| Task | Title | Project |\n");
                out.push_str("|------|-------|--------|\n");
                for task in &self.tasks.in_progress {
                    let project = task.project.as_deref().unwrap_or("-");
                    out.push_str(&format!("| {} | {} | {} |\n", task.id, task.title, project));
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
