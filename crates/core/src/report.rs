//! Dashboard report data types and builder.
//!
//! Provides a unified JSON-serialisable schema consumed by:
//! - CLI (`mdv report --json`)
//! - TUI dashboard (`mdv dashboard`)
//! - MCP tools (via the MCP server)
//! - PNG chart generation

use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde::Serialize;
use std::collections::HashMap;

use crate::index::{IndexDb, IndexedNote, NoteQuery, NoteType};

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
}

impl Default for DashboardOptions {
    fn default() -> Self {
        Self {
            project: None,
            activity_days: 30,
            stale_limit: 10,
            stale_threshold: 0.5,
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

    let tasks: Vec<&IndexedNote> = all_notes
        .iter()
        .filter(|n| n.note_type == NoteType::Task)
        .collect();

    let projects: Vec<&IndexedNote> = all_notes
        .iter()
        .filter(|n| n.note_type == NoteType::Project)
        .collect();

    // Determine scope
    let (scope, target_projects) = if let Some(ref project_filter) = options.project {
        let matched = projects
            .iter()
            .find(|p| {
                let (id, _, _) = extract_project_info(p);
                let folder = p
                    .path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                id.eq_ignore_ascii_case(project_filter)
                    || folder.eq_ignore_ascii_case(project_filter)
            })
            .ok_or_else(|| format!("Project not found: {project_filter}"))?;

        let (id, _, _) = extract_project_info(matched);
        let title = if matched.title.is_empty() {
            id.clone()
        } else {
            matched.title.clone()
        };

        (
            ReportScope::Project {
                id: id.clone(),
                title,
            },
            vec![*matched],
        )
    } else {
        (ReportScope::Vault, projects.to_vec())
    };

    // Build per-project reports
    let project_reports: Vec<ProjectReport> = target_projects
        .iter()
        .map(|p| build_project_report(p, &tasks))
        .collect();

    // Resolve project filter to folder name for consistent matching
    let resolved_project_folder: Option<String> = if options.project.is_some() {
        target_projects.first().and_then(|p| {
            p.path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(String::from)
        })
    } else {
        None
    };

    // Vault summary
    let summary =
        build_vault_summary(&all_notes, &tasks, &projects, &resolved_project_folder);

    // Activity report
    let activity =
        build_activity_report(db, &all_notes, &tasks, options)?;

    Ok(DashboardReport {
        generated_at: Utc::now().to_rfc3339(),
        scope,
        summary,
        projects: project_reports,
        activity,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal builders
// ─────────────────────────────────────────────────────────────────────────────

fn build_vault_summary(
    all_notes: &[IndexedNote],
    tasks: &[&IndexedNote],
    projects: &[&IndexedNote],
    project_filter: &Option<String>,
) -> VaultSummary {
    let mut notes_by_type: HashMap<String, usize> = HashMap::new();
    for note in all_notes {
        *notes_by_type
            .entry(note.note_type.as_str().to_string())
            .or_default() += 1;
    }

    // If scoped to a project, filter tasks to that project
    let scoped_tasks: Vec<&&IndexedNote> = if let Some(pf) = project_filter {
        tasks
            .iter()
            .filter(|t| task_matches_project(t, pf))
            .collect()
    } else {
        tasks.iter().collect()
    };

    let mut tasks_by_status: HashMap<String, usize> = HashMap::new();
    for task in &scoped_tasks {
        let status = get_frontmatter_str(task, "status").unwrap_or_else(|| "todo".to_string());
        let normalised = normalise_status(&status);
        *tasks_by_status.entry(normalised).or_default() += 1;
    }

    let active_projects = projects
        .iter()
        .filter(|p| {
            let (_, status, _) = extract_project_info(p);
            !matches!(status.as_str(), "archived" | "done" | "completed")
        })
        .count();

    VaultSummary {
        total_notes: all_notes.len(),
        notes_by_type,
        total_tasks: scoped_tasks.len(),
        tasks_by_status,
        total_projects: projects.len(),
        active_projects,
    }
}

fn build_project_report(project: &IndexedNote, all_tasks: &[&IndexedNote]) -> ProjectReport {
    let (id, status, kind) = extract_project_info(project);
    let project_folder = project
        .path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let title = if project.title.is_empty() {
        project_folder.to_string()
    } else {
        project.title.clone()
    };

    let project_tasks: Vec<&&IndexedNote> = all_tasks
        .iter()
        .filter(|t| task_matches_project(t, project_folder))
        .collect();

    let mut counts = TaskCounts::default();
    for task in &project_tasks {
        let s = get_frontmatter_str(task, "status").unwrap_or_else(|| "todo".to_string());
        match normalise_status(&s).as_str() {
            "todo" => counts.todo += 1,
            "in_progress" => counts.in_progress += 1,
            "blocked" => counts.blocked += 1,
            "done" => counts.done += 1,
            "cancelled" => counts.cancelled += 1,
            _ => counts.todo += 1,
        }
    }
    counts.total = project_tasks.len();

    let active_total = counts.total - counts.cancelled;
    let progress_percent = if active_total > 0 {
        (counts.done as f64 / active_total as f64) * 100.0
    } else {
        0.0
    };

    let velocity = calculate_velocity(&project_tasks);
    let recent_completions = recent_completions(&project_tasks, &id, 7, 10);

    ProjectReport {
        id,
        title,
        kind,
        status,
        tasks: counts,
        progress_percent,
        velocity,
        recent_completions,
    }
}

fn build_activity_report(
    db: &IndexDb,
    all_notes: &[IndexedNote],
    tasks: &[&IndexedNote],
    options: &DashboardOptions,
) -> Result<ActivityReport, String> {
    let today = chrono::Local::now().date_naive();
    let start = today - Duration::days(options.activity_days as i64);

    // Build daily activity
    let mut daily_activity: Vec<DayActivity> = Vec::new();
    let mut current = start;
    while current <= today {
        let date_str = current.format("%Y-%m-%d").to_string();

        let completed = tasks
            .iter()
            .filter(|t| get_frontmatter_date(t, "completed_at") == Some(current))
            .count();

        let created = tasks
            .iter()
            .filter(|t| get_frontmatter_date(t, "created_at") == Some(current))
            .count();

        let modified = all_notes
            .iter()
            .filter(|n| n.modified.date_naive() == current)
            .count();

        daily_activity.push(DayActivity {
            date: date_str,
            weekday: format!("{:?}", current.weekday()),
            tasks_completed: completed,
            tasks_created: created,
            notes_modified: modified,
        });

        current += Duration::days(1);
    }

    // Stale notes
    let stale_notes = db
        .get_stale_notes(options.stale_threshold, None, Some(options.stale_limit))
        .map_err(|e| format!("Failed to query stale notes: {e}"))?
        .into_iter()
        .map(|(note, score)| {
            let last_seen = db
                .get_activity_summary(note.id.unwrap_or(0))
                .ok()
                .flatten()
                .and_then(|s| s.last_seen.map(|d| d.format("%Y-%m-%d").to_string()));

            StaleNote {
                title: note.title.clone(),
                path: note.path.to_string_lossy().to_string(),
                note_type: note.note_type.as_str().to_string(),
                staleness_score: score,
                last_seen,
            }
        })
        .collect();

    Ok(ActivityReport {
        period_days: options.activity_days,
        daily_activity,
        stale_notes,
    })
}

fn calculate_velocity(tasks: &[&&IndexedNote]) -> Velocity {
    let now = Utc::now();
    let two_weeks_ago = (now - Duration::weeks(2)).date_naive();
    let four_weeks_ago = (now - Duration::weeks(4)).date_naive();
    let seven_days_ago = (now - Duration::days(7)).date_naive();

    let completed_4w = tasks
        .iter()
        .filter(|t| {
            get_frontmatter_date(t, "completed_at")
                .map(|d| d >= four_weeks_ago)
                .unwrap_or(false)
        })
        .count();

    let completed_2w = tasks
        .iter()
        .filter(|t| {
            get_frontmatter_date(t, "completed_at")
                .map(|d| d >= two_weeks_ago)
                .unwrap_or(false)
        })
        .count();

    let completed_7d = tasks
        .iter()
        .filter(|t| {
            get_frontmatter_date(t, "completed_at")
                .map(|d| d >= seven_days_ago)
                .unwrap_or(false)
        })
        .count();

    let created_7d = tasks
        .iter()
        .filter(|t| {
            get_frontmatter_date(t, "created_at")
                .map(|d| d >= seven_days_ago)
                .unwrap_or(false)
        })
        .count();

    Velocity {
        tasks_per_week_4w: completed_4w as f64 / 4.0,
        tasks_per_week_2w: completed_2w as f64 / 2.0,
        created_last_7d: created_7d,
        completed_last_7d: completed_7d,
    }
}

fn recent_completions(
    tasks: &[&&IndexedNote],
    project_id: &str,
    days: i64,
    limit: usize,
) -> Vec<CompletedTask> {
    let cutoff = (Utc::now() - Duration::days(days)).date_naive();

    let mut completions: Vec<CompletedTask> = tasks
        .iter()
        .filter_map(|t| {
            let completed_at = get_frontmatter_date(t, "completed_at")?;
            if completed_at < cutoff {
                return None;
            }
            Some(CompletedTask {
                id: get_frontmatter_str(t, "task-id").unwrap_or_default(),
                title: t.title.clone(),
                completed_at: completed_at.format("%Y-%m-%d").to_string(),
                project: project_id.to_string(),
            })
        })
        .collect();

    completions.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));
    completions.truncate(limit);
    completions
}

// ─────────────────────────────────────────────────────────────────────────────
// Frontmatter helpers
// ─────────────────────────────────────────────────────────────────────────────

fn get_frontmatter_str(note: &IndexedNote, key: &str) -> Option<String> {
    note.frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok())
        .and_then(|fm| fm.get(key).and_then(|v| v.as_str()).map(String::from))
}

fn get_frontmatter_date(note: &IndexedNote, key: &str) -> Option<NaiveDate> {
    let date_str = get_frontmatter_str(note, key)?;
    NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
        .ok()
        .or_else(|| {
            chrono::DateTime::parse_from_rfc3339(&date_str)
                .ok()
                .map(|dt| dt.date_naive())
        })
        .or_else(|| {
            // Handle "YYYY-MM-DDThh:mm:ss" without timezone
            chrono::NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|dt| dt.date())
        })
}

fn extract_project_info(project: &IndexedNote) -> (String, String, String) {
    let fm = project
        .frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok());

    let id = fm
        .as_ref()
        .and_then(|fm| fm.get("project-id").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| {
            project
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("???")
                .to_string()
        });

    let status = fm
        .as_ref()
        .and_then(|fm| fm.get("status").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| "unknown".to_string());

    let kind = fm
        .as_ref()
        .and_then(|fm| fm.get("kind").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| "project".to_string());

    (id, status, kind)
}

fn task_matches_project(task: &IndexedNote, project_folder: &str) -> bool {
    // Check frontmatter project field
    if let Some(project) = get_frontmatter_str(task, "project")
        && project.eq_ignore_ascii_case(project_folder)
    {
        return true;
    }

    // Check path
    let path_str = task.path.to_string_lossy();
    crate::domain::task_belongs_to_project(&path_str, project_folder)
}

fn normalise_status(status: &str) -> String {
    match status {
        "todo" | "open" => "todo".to_string(),
        "in-progress" | "in_progress" | "doing" => "in_progress".to_string(),
        "blocked" | "waiting" => "blocked".to_string(),
        "done" | "completed" => "done".to_string(),
        "cancelled" | "canceled" => "cancelled".to_string(),
        other => other.to_string(),
    }
}
