use chrono::{Datelike, Duration, NaiveDate, Utc};
use std::collections::HashMap;

use crate::index::{IndexDb, IndexedNote, NoteType};

use super::helpers::{
    extract_project_info, get_frontmatter_date, get_frontmatter_str, normalise_status,
    parse_review_interval, task_matches_project,
};
use super::{
    ActivityReport, CompletedTask, DashboardOptions, DayActivity, FlaggedTask,
    ProjectReport, ReviewDueProject, StaleNote, TaskCounts, VaultSummary, Velocity,
};

pub(super) fn build_vault_summary(
    all_notes: &[IndexedNote],
    tasks: &[&IndexedNote],
    projects: &[&IndexedNote],
    project_filter: &Option<String>,
) -> VaultSummary {
    let mut notes_by_type: HashMap<String, usize> = HashMap::new();
    for note in all_notes {
        *notes_by_type.entry(note.note_type.as_str().to_string()).or_default() += 1;
    }

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

pub(super) fn build_project_report(
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
    let completions = recent_completions(&project_tasks, &id, 7, 10);

    ProjectReport {
        id,
        title,
        kind,
        status,
        tasks: counts,
        progress_percent,
        velocity,
        recent_completions: completions,
    }
}

pub(super) fn build_activity_report(
    db: &IndexDb,
    all_notes: &[IndexedNote],
    tasks: &[&IndexedNote],
    options: &DashboardOptions,
) -> Result<ActivityReport, String> {
    let today = chrono::Local::now().date_naive();
    let start = today - Duration::days(options.activity_days as i64);

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

pub(super) fn calculate_velocity(tasks: &[&&IndexedNote]) -> Velocity {
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

pub(super) fn recent_completions(
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

pub(super) fn build_flagged_tasks(
    tasks: &[&IndexedNote],
    target_projects: &[&IndexedNote],
    today: NaiveDate,
    zombie_days: u32,
) -> (Vec<FlaggedTask>, Vec<FlaggedTask>, Vec<FlaggedTask>, Vec<FlaggedTask>) {
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

    // Zombie tasks
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

pub(super) fn build_review_due(
    projects: &[&IndexedNote],
    today: NaiveDate,
) -> Vec<ReviewDueProject> {
    let mut due: Vec<ReviewDueProject> = projects
        .iter()
        .filter_map(|p| {
            let (id, status, kind) = extract_project_info(p);
            if matches!(status.as_str(), "done" | "archived") {
                return None;
            }
            let interval_str = get_frontmatter_str(p, "review_interval")?;
            let interval_days = parse_review_interval(&interval_str)?;

            let last_reviewed = get_frontmatter_date(p, "last_reviewed")
                .or_else(|| get_frontmatter_date(p, "updated_at"))?;

            let days_since = (today - last_reviewed).num_days();
            let days_overdue = days_since - interval_days;

            if days_overdue <= 0 {
                return None;
            }

            Some(ReviewDueProject {
                id,
                title: p.title.clone(),
                kind,
                review_interval: interval_str,
                last_reviewed: Some(last_reviewed.format("%Y-%m-%d").to_string()),
                days_since_review: days_since,
                days_overdue,
            })
        })
        .collect();

    due.sort_by(|a, b| b.days_overdue.cmp(&a.days_overdue));
    due
}
