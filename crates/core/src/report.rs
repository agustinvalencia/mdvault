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
    pub overdue: Vec<FlaggedTask>,
    pub high_priority: Vec<FlaggedTask>,
    pub upcoming_deadlines: Vec<FlaggedTask>,
    pub zombie: Vec<FlaggedTask>,
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

    // Determine scope
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

    // Build per-project reports
    let project_reports: Vec<ProjectReport> =
        target_projects.iter().map(|p| build_project_report(p, &tasks)).collect();

    // Resolve project filter to folder name for consistent matching
    let resolved_project_folder: Option<String> = if options.project.is_some() {
        target_projects
            .first()
            .and_then(|p| p.path.file_stem().and_then(|s| s.to_str()).map(String::from))
    } else {
        None
    };

    // Vault summary
    let summary =
        build_vault_summary(&all_notes, &tasks, &projects, &resolved_project_folder);

    // Activity report
    let activity = build_activity_report(db, &all_notes, &tasks, options)?;

    // Build actionable task lists
    let today = chrono::Local::now().date_naive();
    let (overdue, high_priority, upcoming_deadlines, zombie) =
        build_flagged_tasks(&tasks, &target_projects, today, options.zombie_days);

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
        *notes_by_type.entry(note.note_type.as_str().to_string()).or_default() += 1;
    }

    // If scoped to a project, filter tasks to that project
    let scoped_tasks: Vec<&&IndexedNote> = if let Some(pf) = project_filter {
        tasks.iter().filter(|t| task_matches_project(t, pf)).collect()
    } else {
        tasks.iter().collect()
    };

    let mut tasks_by_status: HashMap<String, usize> = HashMap::new();
    for task in &scoped_tasks {
        let status =
            get_frontmatter_str(task, "status").unwrap_or_else(|| "todo".to_string());
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

fn build_project_report(
    project: &IndexedNote,
    all_tasks: &[&IndexedNote],
) -> ProjectReport {
    let (id, status, kind) = extract_project_info(project);
    let project_folder = project.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let title = if project.title.is_empty() {
        project_folder.to_string()
    } else {
        project.title.clone()
    };

    let project_tasks: Vec<&&IndexedNote> =
        all_tasks.iter().filter(|t| task_matches_project(t, project_folder)).collect();

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

        let modified =
            all_notes.iter().filter(|n| n.modified.date_naive() == current).count();

        daily_activity.push(DayActivity {
            date: date_str,
            weekday: format!("{:?}", current.weekday()),
            tasks_completed: completed,
            tasks_created: created,
            notes_modified: modified,
        });

        current += Duration::days(1);
    }

    // Stale notes — only tasks and projects are actionable, so exclude
    // timeless note types like zettels, contacts, and daily/weekly notes.
    let stale_notes = db
        .get_stale_notes(options.stale_threshold, None, Some(options.stale_limit))
        .map_err(|e| format!("Failed to query stale notes: {e}"))?
        .into_iter()
        .filter(|(note, _)| matches!(note.note_type, NoteType::Task | NoteType::Project))
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

    Ok(ActivityReport { period_days: options.activity_days, daily_activity, stale_notes })
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

fn build_flagged_tasks(
    tasks: &[&IndexedNote],
    target_projects: &[&IndexedNote],
    today: NaiveDate,
    zombie_days: u32,
) -> (Vec<FlaggedTask>, Vec<FlaggedTask>, Vec<FlaggedTask>, Vec<FlaggedTask>) {
    // Build folder-name → project-id map
    let folder_to_id: HashMap<String, String> = target_projects
        .iter()
        .filter_map(|p| {
            let folder = p.path.file_stem().and_then(|s| s.to_str())?.to_string();
            let (id, _, _) = extract_project_info(p);
            Some((folder, id))
        })
        .collect();

    let resolve_project = |task: &IndexedNote| -> String {
        let raw = get_frontmatter_str(task, "project").unwrap_or_default();
        folder_to_id.get(&raw).cloned().unwrap_or(raw)
    };

    let is_open =
        |status: &str| !matches!(normalise_status(status).as_str(), "done" | "cancelled");

    // Overdue
    let mut overdue: Vec<FlaggedTask> = tasks
        .iter()
        .filter_map(|t| {
            let status = get_frontmatter_str(t, "status").unwrap_or_default();
            if !is_open(&status) {
                return None;
            }
            let due = get_frontmatter_date(t, "due_date")?;
            if due >= today {
                return None;
            }
            Some(FlaggedTask {
                id: get_frontmatter_str(t, "task-id").unwrap_or_default(),
                title: t.title.clone(),
                project: resolve_project(t),
                due_date: Some(due.format("%Y-%m-%d").to_string()),
                priority: get_frontmatter_str(t, "priority"),
                status: normalise_status(&status),
                days_overdue: Some((today - due).num_days()),
            })
        })
        .collect();
    overdue.sort_by(|a, b| b.days_overdue.cmp(&a.days_overdue));

    // High priority
    let mut high_priority: Vec<FlaggedTask> = tasks
        .iter()
        .filter_map(|t| {
            let status = get_frontmatter_str(t, "status").unwrap_or_default();
            if !is_open(&status) {
                return None;
            }
            let priority = get_frontmatter_str(t, "priority")?;
            if priority != "high" {
                return None;
            }
            Some(FlaggedTask {
                id: get_frontmatter_str(t, "task-id").unwrap_or_default(),
                title: t.title.clone(),
                project: resolve_project(t),
                due_date: get_frontmatter_date(t, "due_date")
                    .map(|d| d.format("%Y-%m-%d").to_string()),
                priority: Some(priority),
                status: normalise_status(&status),
                days_overdue: None,
            })
        })
        .collect();
    high_priority.truncate(10);

    // Upcoming deadlines (next 14 days)
    let deadline_horizon = today + Duration::days(14);
    let mut upcoming_deadlines: Vec<FlaggedTask> = tasks
        .iter()
        .filter_map(|t| {
            let status = get_frontmatter_str(t, "status").unwrap_or_default();
            if !is_open(&status) {
                return None;
            }
            let due = get_frontmatter_date(t, "due_date")?;
            if due < today || due > deadline_horizon {
                return None;
            }
            Some(FlaggedTask {
                id: get_frontmatter_str(t, "task-id").unwrap_or_default(),
                title: t.title.clone(),
                project: resolve_project(t),
                due_date: Some(due.format("%Y-%m-%d").to_string()),
                priority: get_frontmatter_str(t, "priority"),
                status: normalise_status(&status),
                days_overdue: None,
            })
        })
        .collect();
    upcoming_deadlines.sort_by(|a, b| a.due_date.cmp(&b.due_date));

    // Zombie tasks — stuck in "todo" for zombie_days+ days
    let zombie_horizon = today - Duration::days(i64::from(zombie_days));
    let mut zombie: Vec<FlaggedTask> = tasks
        .iter()
        .filter_map(|t| {
            let status = get_frontmatter_str(t, "status").unwrap_or_default();
            if normalise_status(&status) != "todo" {
                return None;
            }
            let created = get_frontmatter_date(t, "created_at")?;
            if created > zombie_horizon {
                return None;
            }
            Some(FlaggedTask {
                id: get_frontmatter_str(t, "task-id").unwrap_or_default(),
                title: t.title.clone(),
                project: resolve_project(t),
                due_date: get_frontmatter_date(t, "due_date")
                    .map(|d| d.format("%Y-%m-%d").to_string()),
                priority: get_frontmatter_str(t, "priority"),
                status: normalise_status(&status),
                days_overdue: Some((today - created).num_days()),
            })
        })
        .collect();
    zombie.sort_by(|a, b| b.days_overdue.cmp(&a.days_overdue));

    (overdue, high_priority, upcoming_deadlines, zombie)
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
            chrono::DateTime::parse_from_rfc3339(&date_str).ok().map(|dt| dt.date_naive())
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
            project.path.file_stem().and_then(|s| s.to_str()).unwrap_or("???").to_string()
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

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};
    use std::path::PathBuf;

    /// Helper to create a minimal IndexedNote for testing.
    fn make_note(
        path: &str,
        note_type: NoteType,
        title: &str,
        frontmatter: Option<&str>,
    ) -> IndexedNote {
        IndexedNote {
            id: None,
            path: PathBuf::from(path),
            note_type,
            title: title.to_string(),
            created: Some(Utc::now()),
            modified: Utc::now(),
            frontmatter_json: frontmatter.map(String::from),
            content_hash: "test".to_string(),
        }
    }

    fn make_project(folder: &str, id: &str, title: &str, status: &str) -> IndexedNote {
        let fm = serde_json::json!({
            "project-id": id,
            "status": status,
            "kind": "project",
        });
        make_note(
            &format!("Projects/{folder}/{folder}.md"),
            NoteType::Project,
            title,
            Some(&fm.to_string()),
        )
    }

    fn make_task(
        project_folder: &str,
        task_id: &str,
        title: &str,
        status: &str,
        created_at: Option<NaiveDate>,
        completed_at: Option<NaiveDate>,
    ) -> IndexedNote {
        let mut fm = serde_json::json!({
            "task-id": task_id,
            "status": status,
            "project": project_folder,
        });
        if let Some(d) = created_at {
            fm["created_at"] =
                serde_json::Value::String(d.format("%Y-%m-%d").to_string());
        }
        if let Some(d) = completed_at {
            fm["completed_at"] =
                serde_json::Value::String(d.format("%Y-%m-%d").to_string());
        }
        let modified = completed_at
            .or(created_at)
            .map(|d| Utc.from_utc_datetime(&d.and_hms_opt(12, 0, 0).unwrap()))
            .unwrap_or_else(Utc::now);
        IndexedNote {
            modified,
            ..make_note(
                &format!("Projects/{project_folder}/Tasks/{task_id}.md"),
                NoteType::Task,
                title,
                Some(&fm.to_string()),
            )
        }
    }

    // ── normalise_status ─────────────────────────────────────────────────

    #[test]
    fn normalise_status_canonical_values() {
        assert_eq!(normalise_status("todo"), "todo");
        assert_eq!(normalise_status("in_progress"), "in_progress");
        assert_eq!(normalise_status("blocked"), "blocked");
        assert_eq!(normalise_status("done"), "done");
        assert_eq!(normalise_status("cancelled"), "cancelled");
    }

    #[test]
    fn normalise_status_aliases() {
        assert_eq!(normalise_status("open"), "todo");
        assert_eq!(normalise_status("in-progress"), "in_progress");
        assert_eq!(normalise_status("doing"), "in_progress");
        assert_eq!(normalise_status("waiting"), "blocked");
        assert_eq!(normalise_status("completed"), "done");
        assert_eq!(normalise_status("canceled"), "cancelled");
    }

    #[test]
    fn normalise_status_unknown_passes_through() {
        assert_eq!(normalise_status("review"), "review");
    }

    // ── frontmatter helpers ──────────────────────────────────────────────

    #[test]
    fn get_frontmatter_str_returns_value() {
        let note = make_note(
            "test.md",
            NoteType::Task,
            "Test",
            Some(r#"{"status": "done", "project": "my-project"}"#),
        );
        assert_eq!(get_frontmatter_str(&note, "status"), Some("done".into()));
        assert_eq!(get_frontmatter_str(&note, "project"), Some("my-project".into()));
    }

    #[test]
    fn get_frontmatter_str_missing_key_returns_none() {
        let note =
            make_note("test.md", NoteType::Task, "Test", Some(r#"{"status": "done"}"#));
        assert_eq!(get_frontmatter_str(&note, "nonexistent"), None);
    }

    #[test]
    fn get_frontmatter_str_no_frontmatter_returns_none() {
        let note = make_note("test.md", NoteType::Task, "Test", None);
        assert_eq!(get_frontmatter_str(&note, "status"), None);
    }

    #[test]
    fn get_frontmatter_date_parses_ymd() {
        let note = make_note(
            "test.md",
            NoteType::Task,
            "Test",
            Some(r#"{"completed_at": "2025-06-15"}"#),
        );
        let expected = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        assert_eq!(get_frontmatter_date(&note, "completed_at"), Some(expected));
    }

    #[test]
    fn get_frontmatter_date_parses_rfc3339() {
        let note = make_note(
            "test.md",
            NoteType::Task,
            "Test",
            Some(r#"{"completed_at": "2025-06-15T10:30:00+00:00"}"#),
        );
        let expected = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        assert_eq!(get_frontmatter_date(&note, "completed_at"), Some(expected));
    }

    #[test]
    fn get_frontmatter_date_parses_datetime_no_tz() {
        let note = make_note(
            "test.md",
            NoteType::Task,
            "Test",
            Some(r#"{"completed_at": "2025-06-15T10:30:00"}"#),
        );
        let expected = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        assert_eq!(get_frontmatter_date(&note, "completed_at"), Some(expected));
    }

    // ── extract_project_info ─────────────────────────────────────────────

    #[test]
    fn extract_project_info_reads_frontmatter() {
        let project = make_project("my-proj", "MP", "My Project", "open");
        let (id, status, kind) = extract_project_info(&project);
        assert_eq!(id, "MP");
        assert_eq!(status, "open");
        assert_eq!(kind, "project");
    }

    #[test]
    fn extract_project_info_defaults_without_frontmatter() {
        let note = make_note(
            "Projects/fallback-proj/fallback-proj.md",
            NoteType::Project,
            "Fallback",
            None,
        );
        let (id, status, kind) = extract_project_info(&note);
        assert_eq!(id, "fallback-proj");
        assert_eq!(status, "unknown");
        assert_eq!(kind, "project");
    }

    // ── task_matches_project ─────────────────────────────────────────────

    #[test]
    fn task_matches_project_via_frontmatter() {
        let task = make_task("my-project", "T-1", "Task", "todo", None, None);
        assert!(task_matches_project(&task, "my-project"));
    }

    #[test]
    fn task_matches_project_via_path() {
        let task = make_note(
            "Projects/my-project/Tasks/T-1.md",
            NoteType::Task,
            "Task",
            Some(r#"{"status": "todo"}"#),
        );
        assert!(task_matches_project(&task, "my-project"));
    }

    #[test]
    fn task_does_not_match_wrong_project() {
        let task = make_task("other-project", "T-1", "Task", "todo", None, None);
        assert!(!task_matches_project(&task, "my-project"));
    }

    // ── calculate_velocity ───────────────────────────────────────────────

    #[test]
    fn velocity_with_no_tasks() {
        let tasks: Vec<&IndexedNote> = vec![];
        let refs: Vec<&&IndexedNote> = tasks.iter().collect();
        let v = calculate_velocity(&refs);
        assert_eq!(v.tasks_per_week_4w, 0.0);
        assert_eq!(v.tasks_per_week_2w, 0.0);
        assert_eq!(v.completed_last_7d, 0);
        assert_eq!(v.created_last_7d, 0);
    }

    #[test]
    fn velocity_counts_recent_completions() {
        let today = chrono::Local::now().date_naive();
        let yesterday = today - Duration::days(1);
        let two_days_ago = today - Duration::days(2);

        let t1 =
            make_task("proj", "T-1", "A", "done", Some(two_days_ago), Some(yesterday));
        let t2 = make_task("proj", "T-2", "B", "done", Some(two_days_ago), Some(today));

        let tasks = [&t1, &t2];
        let refs: Vec<&&IndexedNote> = tasks.iter().collect();
        let v = calculate_velocity(&refs);

        assert_eq!(v.completed_last_7d, 2);
        assert_eq!(v.created_last_7d, 2);
        assert!(v.tasks_per_week_2w > 0.0);
        assert!(v.tasks_per_week_4w > 0.0);
    }

    #[test]
    fn velocity_excludes_old_completions() {
        let old_date = chrono::Local::now().date_naive() - Duration::days(60);
        let t1 = make_task("proj", "T-1", "Old", "done", Some(old_date), Some(old_date));

        let tasks = [&t1];
        let refs: Vec<&&IndexedNote> = tasks.iter().collect();
        let v = calculate_velocity(&refs);

        assert_eq!(v.completed_last_7d, 0);
        assert_eq!(v.tasks_per_week_4w, 0.0);
    }

    // ── recent_completions ───────────────────────────────────────────────

    #[test]
    fn recent_completions_filters_and_sorts() {
        let today = chrono::Local::now().date_naive();
        let yesterday = today - Duration::days(1);
        let old = today - Duration::days(30);

        let t1 = make_task("proj", "T-1", "Recent", "done", None, Some(yesterday));
        let t2 = make_task("proj", "T-2", "Today", "done", None, Some(today));
        let t3 = make_task("proj", "T-3", "Old", "done", None, Some(old));

        let tasks = [&t1, &t2, &t3];
        let refs: Vec<&&IndexedNote> = tasks.iter().collect();
        let result = recent_completions(&refs, "PROJ", 7, 10);

        assert_eq!(result.len(), 2); // excludes old
        assert_eq!(result[0].id, "T-2"); // today first (sorted desc)
        assert_eq!(result[1].id, "T-1");
    }

    #[test]
    fn recent_completions_respects_limit() {
        let today = chrono::Local::now().date_naive();

        let t1 = make_task("proj", "T-1", "A", "done", None, Some(today));
        let t2 = make_task("proj", "T-2", "B", "done", None, Some(today));
        let t3 = make_task("proj", "T-3", "C", "done", None, Some(today));

        let tasks = [&t1, &t2, &t3];
        let refs: Vec<&&IndexedNote> = tasks.iter().collect();
        let result = recent_completions(&refs, "PROJ", 7, 2);

        assert_eq!(result.len(), 2);
    }

    // ── build_project_report ─────────────────────────────────────────────

    #[test]
    fn project_report_calculates_progress() {
        let project = make_project("my-proj", "MP", "My Project", "open");
        let t1 = make_task("my-proj", "MP-1", "Done task", "done", None, None);
        let t2 = make_task("my-proj", "MP-2", "Todo task", "todo", None, None);
        let t3 = make_task("my-proj", "MP-3", "Cancelled", "cancelled", None, None);

        let all_tasks: Vec<&IndexedNote> = vec![&t1, &t2, &t3];
        let report = build_project_report(&project, &all_tasks);

        assert_eq!(report.id, "MP");
        assert_eq!(report.title, "My Project");
        assert_eq!(report.tasks.total, 3);
        assert_eq!(report.tasks.done, 1);
        assert_eq!(report.tasks.todo, 1);
        assert_eq!(report.tasks.cancelled, 1);
        // Progress excludes cancelled: 1 done / 2 active = 50%
        assert!((report.progress_percent - 50.0).abs() < 0.01);
    }

    #[test]
    fn project_report_zero_tasks() {
        let project = make_project("empty", "EMP", "Empty", "open");
        let all_tasks: Vec<&IndexedNote> = vec![];
        let report = build_project_report(&project, &all_tasks);

        assert_eq!(report.tasks.total, 0);
        assert_eq!(report.progress_percent, 0.0);
    }

    #[test]
    fn project_report_all_done() {
        let project = make_project("done-proj", "DP", "Done Project", "done");
        let t1 = make_task("done-proj", "DP-1", "A", "done", None, None);
        let t2 = make_task("done-proj", "DP-2", "B", "done", None, None);

        let all_tasks: Vec<&IndexedNote> = vec![&t1, &t2];
        let report = build_project_report(&project, &all_tasks);

        assert!((report.progress_percent - 100.0).abs() < 0.01);
    }

    // ── build_vault_summary ──────────────────────────────────────────────

    #[test]
    fn vault_summary_counts_correctly() {
        let project = make_project("proj", "P", "Proj", "open");
        let t1 = make_task("proj", "T-1", "A", "done", None, None);
        let t2 = make_task("proj", "T-2", "B", "todo", None, None);
        let daily = make_note("Journal/2025-01-01.md", NoteType::Daily, "Jan 1", None);

        let all_notes = vec![project.clone(), t1.clone(), t2.clone(), daily];
        let tasks: Vec<&IndexedNote> = vec![&t1, &t2];
        let projects: Vec<&IndexedNote> = vec![&project];

        let summary = build_vault_summary(&all_notes, &tasks, &projects, &None);

        assert_eq!(summary.total_notes, 4);
        assert_eq!(summary.total_tasks, 2);
        assert_eq!(summary.total_projects, 1);
        assert_eq!(summary.active_projects, 1);
        assert_eq!(summary.tasks_by_status.get("done"), Some(&1));
        assert_eq!(summary.tasks_by_status.get("todo"), Some(&1));
    }

    #[test]
    fn vault_summary_filters_by_project() {
        let p1 = make_project("proj-a", "PA", "A", "open");
        let p2 = make_project("proj-b", "PB", "B", "open");
        let t1 = make_task("proj-a", "T-1", "A task", "done", None, None);
        let t2 = make_task("proj-b", "T-2", "B task", "todo", None, None);

        let all_notes = vec![p1.clone(), p2.clone(), t1.clone(), t2.clone()];
        let tasks: Vec<&IndexedNote> = vec![&t1, &t2];
        let projects: Vec<&IndexedNote> = vec![&p1, &p2];

        let summary = build_vault_summary(
            &all_notes,
            &tasks,
            &projects,
            &Some("proj-a".to_string()),
        );

        // Only proj-a tasks counted
        assert_eq!(summary.total_tasks, 1);
        assert_eq!(summary.tasks_by_status.get("done"), Some(&1));
        assert_eq!(summary.tasks_by_status.get("todo"), None);
    }

    #[test]
    fn vault_summary_excludes_archived_from_active() {
        let active = make_project("active", "A", "Active", "open");
        let archived = make_project("archived", "B", "Archived", "archived");
        let done = make_project("done", "C", "Done", "done");

        let all_notes = vec![active.clone(), archived.clone(), done.clone()];
        let tasks: Vec<&IndexedNote> = vec![];
        let projects: Vec<&IndexedNote> = vec![&active, &archived, &done];

        let summary = build_vault_summary(&all_notes, &tasks, &projects, &None);

        assert_eq!(summary.total_projects, 3);
        assert_eq!(summary.active_projects, 1);
    }

    // ── build_dashboard (integration via IndexDb) ────────────────────────

    #[test]
    fn build_dashboard_vault_wide() {
        let db = IndexDb::open_in_memory().unwrap();

        let project = make_project("test-proj", "TP", "Test Project", "open");
        db.insert_note(&project).unwrap();

        let t1 = make_task("test-proj", "TP-1", "Done task", "done", None, None);
        let t2 = make_task("test-proj", "TP-2", "Todo task", "todo", None, None);
        db.insert_note(&t1).unwrap();
        db.insert_note(&t2).unwrap();

        let options = DashboardOptions::default();
        let report = build_dashboard(&db, &options).unwrap();

        assert!(matches!(report.scope, ReportScope::Vault));
        assert_eq!(report.projects.len(), 1);
        assert_eq!(report.projects[0].id, "TP");
        assert_eq!(report.summary.total_tasks, 2);
    }

    #[test]
    fn build_dashboard_scoped_to_project() {
        let db = IndexDb::open_in_memory().unwrap();

        let p1 = make_project("proj-a", "PA", "Project A", "open");
        let p2 = make_project("proj-b", "PB", "Project B", "open");
        db.insert_note(&p1).unwrap();
        db.insert_note(&p2).unwrap();

        let t1 = make_task("proj-a", "PA-1", "A task", "done", None, None);
        let t2 = make_task("proj-b", "PB-1", "B task", "todo", None, None);
        db.insert_note(&t1).unwrap();
        db.insert_note(&t2).unwrap();

        let options =
            DashboardOptions { project: Some("PA".to_string()), ..Default::default() };
        let report = build_dashboard(&db, &options).unwrap();

        assert!(matches!(report.scope, ReportScope::Project { .. }));
        assert_eq!(report.projects.len(), 1);
        assert_eq!(report.projects[0].id, "PA");
    }

    #[test]
    fn build_dashboard_project_not_found() {
        let db = IndexDb::open_in_memory().unwrap();

        let options = DashboardOptions {
            project: Some("NONEXISTENT".to_string()),
            ..Default::default()
        };
        let result = build_dashboard(&db, &options);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Project not found"));
    }

    #[test]
    fn build_dashboard_empty_vault() {
        let db = IndexDb::open_in_memory().unwrap();
        let options = DashboardOptions::default();
        let report = build_dashboard(&db, &options).unwrap();

        assert!(matches!(report.scope, ReportScope::Vault));
        assert_eq!(report.projects.len(), 0);
        assert_eq!(report.summary.total_notes, 0);
        assert_eq!(report.summary.total_tasks, 0);
    }

    #[test]
    fn build_dashboard_activity_days_respected() {
        let db = IndexDb::open_in_memory().unwrap();
        let options = DashboardOptions { activity_days: 7, ..Default::default() };
        let report = build_dashboard(&db, &options).unwrap();

        assert_eq!(report.activity.period_days, 7);
        assert_eq!(report.activity.daily_activity.len(), 8); // 7 days + today
    }

    #[test]
    fn stale_notes_excludes_non_actionable_types() {
        let db = IndexDb::open_in_memory().unwrap();

        // Insert notes of various types — all will have default staleness of 1.0
        // (no activity_summary row → COALESCE to 1.0)
        let task = make_task("proj", "T-1", "Stale task", "todo", None, None);
        let project = make_project("proj", "P", "Stale project", "open");
        let zettel = make_note("Zettelkasten/z1.md", NoteType::Zettel, "A zettel", None);
        let daily = make_note("Journal/2025-01-01.md", NoteType::Daily, "Jan 1", None);

        db.insert_note(&task).unwrap();
        db.insert_note(&project).unwrap();
        db.insert_note(&zettel).unwrap();
        db.insert_note(&daily).unwrap();

        let options = DashboardOptions {
            stale_threshold: 0.5,
            stale_limit: 50,
            ..Default::default()
        };
        let report = build_dashboard(&db, &options).unwrap();

        let stale_types: Vec<&str> =
            report.activity.stale_notes.iter().map(|s| s.note_type.as_str()).collect();

        assert!(stale_types.iter().all(|t| *t == "task" || *t == "project"));
        assert!(!stale_types.contains(&"zettel"));
        assert!(!stale_types.contains(&"daily"));
    }

    #[test]
    fn zombie_tasks_flagged_correctly() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 22).unwrap();
        let old_date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(); // 80 days ago
        let recent_date = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap(); // 12 days ago

        let project = make_project("proj", "P", "Test Project", "open");

        // Zombie: todo for 80 days
        let zombie_task = make_task("proj", "T-1", "Old task", "todo", Some(old_date), None);
        // Not zombie: todo but only 12 days
        let fresh_task = make_task("proj", "T-2", "Fresh task", "todo", Some(recent_date), None);
        // Not zombie: done (even if old)
        let done_task = make_task("proj", "T-3", "Done task", "done", Some(old_date), Some(today));
        // Not zombie: in-progress (even if old)
        let ip_task = make_task("proj", "T-4", "Active task", "in-progress", Some(old_date), None);

        let tasks: Vec<&IndexedNote> = vec![&zombie_task, &fresh_task, &done_task, &ip_task];
        let projects: Vec<&IndexedNote> = vec![&project];

        let (_, _, _, zombie) = build_flagged_tasks(&tasks, &projects, today, 30);

        assert_eq!(zombie.len(), 1);
        assert_eq!(zombie[0].id, "T-1");
        assert_eq!(zombie[0].days_overdue, Some(80));
    }
}
