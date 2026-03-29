//! Dashboard report data types and builder.
//!
//! Provides a unified JSON-serialisable schema consumed by:
//! - CLI (`mdv report --json`)
//! - TUI dashboard (`mdv dashboard`)
//! - MCP tools (via the MCP server)
//! - PNG chart generation

mod aggregation;
mod helpers;
#[cfg(test)]
mod tests;

use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;

use crate::index::{IndexDb, IndexedNote, NoteQuery, NoteType};

use aggregation::{
    build_activity_report, build_flagged_tasks, build_project_report, build_review_due,
    build_vault_summary,
};
use helpers::extract_project_info;

// ─────────────────────────────────────────────────────────────────────────────
// Schema types
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level dashboard report. Can be vault-wide or scoped to a single project.
#[derive(Debug, Serialize)]
pub struct DashboardReport {
    pub generated_at: String,
    pub scope: ReportScope,
    pub summary: VaultSummary,
    pub projects: Vec<ProjectReport>,
    pub activity: ActivityReport,
    pub overdue: Vec<FlaggedTask>,
    pub high_priority: Vec<FlaggedTask>,
    pub upcoming_deadlines: Vec<FlaggedTask>,
    pub zombie: Vec<FlaggedTask>,
    pub review_due: Vec<ReviewDueProject>,
}

/// A project or area that hasn't been reviewed within its review_interval.
#[derive(Debug, Serialize)]
pub struct ReviewDueProject {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub review_interval: String,
    pub last_reviewed: Option<String>,
    pub days_since_review: i64,
    pub days_overdue: i64,
}

/// Whether this report covers the whole vault or a single project.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ReportScope {
    #[serde(rename = "vault")]
    Vault,
    #[serde(rename = "project")]
    Project { id: String, title: String },
}

/// High-level vault/scope summary.
#[derive(Debug, Serialize)]
pub struct VaultSummary {
    pub total_notes: usize,
    pub notes_by_type: HashMap<String, usize>,
    pub total_tasks: usize,
    pub tasks_by_status: HashMap<String, usize>,
    pub total_projects: usize,
    pub active_projects: usize,
}

/// Per-project breakdown.
#[derive(Debug, Serialize)]
pub struct ProjectReport {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub tasks: TaskCounts,
    pub progress_percent: f64,
    pub velocity: Velocity,
    pub recent_completions: Vec<CompletedTask>,
}

/// Task counts grouped by status.
#[derive(Debug, Default, Serialize)]
pub struct TaskCounts {
    pub total: usize,
    pub todo: usize,
    pub in_progress: usize,
    pub blocked: usize,
    pub done: usize,
    pub cancelled: usize,
}

/// Velocity metrics over sliding windows.
#[derive(Debug, Serialize)]
pub struct Velocity {
    pub tasks_per_week_4w: f64,
    pub tasks_per_week_2w: f64,
    pub created_last_7d: usize,
    pub completed_last_7d: usize,
}

/// A completed task reference.
#[derive(Debug, Serialize)]
pub struct CompletedTask {
    pub id: String,
    pub title: String,
    pub completed_at: String,
    pub project: String,
}

/// Activity over time — burndown, heatmap, staleness.
#[derive(Debug, Serialize)]
pub struct ActivityReport {
    pub period_days: u32,
    pub daily_activity: Vec<DayActivity>,
    pub stale_notes: Vec<StaleNote>,
}

/// Activity for a single day (tasks completed + created).
#[derive(Debug, Serialize)]
pub struct DayActivity {
    pub date: String,
    pub weekday: String,
    pub tasks_completed: usize,
    pub tasks_created: usize,
    pub notes_modified: usize,
}

/// A note flagged as stale.
#[derive(Debug, Serialize)]
pub struct StaleNote {
    pub title: String,
    pub path: String,
    pub note_type: String,
    pub staleness_score: f64,
    pub last_seen: Option<String>,
}

/// A task flagged for attention (overdue, high priority, or upcoming deadline).
#[derive(Debug, Serialize)]
pub struct FlaggedTask {
    pub id: String,
    pub title: String,
    pub project: String,
    pub due_date: Option<String>,
    pub priority: Option<String>,
    pub status: String,
    pub days_overdue: Option<i64>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Builder
// ─────────────────────────────────────────────────────────────────────────────

/// Options for building a dashboard report.
pub struct DashboardOptions {
    /// Scope to a single project (by ID or folder name). None = vault-wide.
    pub project: Option<String>,
    /// Number of days of activity history to include (default: 30).
    pub activity_days: u32,
    /// Maximum stale notes to include (default: 10).
    pub stale_limit: u32,
    /// Minimum staleness score to flag (default: 0.5).
    pub stale_threshold: f64,
    /// Minimum days in "todo" status before a task is flagged as zombie (default: 30).
    pub zombie_days: u32,
}

impl Default for DashboardOptions {
    fn default() -> Self {
        Self {
            project: None,
            activity_days: 30,
            stale_limit: 10,
            stale_threshold: 0.5,
            zombie_days: 30,
        }
    }
}

/// Build a dashboard report from the index.
pub fn build_dashboard(
    db: &IndexDb,
    options: &DashboardOptions,
) -> Result<DashboardReport, String> {
    let all_notes = db
        .query_notes(&NoteQuery::default())
        .map_err(|e| format!("Failed to query notes: {e}"))?;

    let tasks: Vec<&IndexedNote> =
        all_notes.iter().filter(|n| n.note_type == NoteType::Task).collect();

    let projects: Vec<&IndexedNote> =
        all_notes.iter().filter(|n| n.note_type == NoteType::Project).collect();

    let (scope, target_projects) = if let Some(ref project_filter) = options.project {
        let matched = projects
            .iter()
            .find(|p| {
                let (id, _, _) = extract_project_info(p);
                let folder = p.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                id.eq_ignore_ascii_case(project_filter)
                    || folder.eq_ignore_ascii_case(project_filter)
            })
            .ok_or_else(|| format!("Project not found: {project_filter}"))?;

        let (id, _, _) = extract_project_info(matched);
        let title =
            if matched.title.is_empty() { id.clone() } else { matched.title.clone() };

        (ReportScope::Project { id: id.clone(), title }, vec![*matched])
    } else {
        (ReportScope::Vault, projects.to_vec())
    };

    let project_reports: Vec<ProjectReport> =
        target_projects.iter().map(|p| build_project_report(p, &tasks)).collect();

    let resolved_project_folder: Option<String> = if options.project.is_some() {
        target_projects
            .first()
            .and_then(|p| p.path.file_stem().and_then(|s| s.to_str()).map(String::from))
    } else {
        None
    };

    let summary =
        build_vault_summary(&all_notes, &tasks, &projects, &resolved_project_folder);
    let activity = build_activity_report(db, &all_notes, &tasks, options)?;

    let today = chrono::Local::now().date_naive();
    let (overdue, high_priority, upcoming_deadlines, zombie) =
        build_flagged_tasks(&tasks, &target_projects, today, options.zombie_days);

    let review_due = build_review_due(&target_projects, today);

    Ok(DashboardReport {
        generated_at: Utc::now().to_rfc3339(),
        scope,
        summary,
        projects: project_reports,
        activity,
        overdue,
        high_priority,
        upcoming_deadlines,
        zombie,
        review_due,
    })
}
