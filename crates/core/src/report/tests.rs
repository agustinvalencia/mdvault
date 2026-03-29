use super::aggregation::{
    build_flagged_tasks, build_project_report, build_review_due, build_vault_summary,
    calculate_velocity, recent_completions,
};
use super::helpers::{
    extract_project_info, get_frontmatter_date, get_frontmatter_str, normalise_status,
    parse_review_interval, task_matches_project,
};
use super::*;
use chrono::{Duration, NaiveDate, TimeZone, Utc};
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
        fm["created_at"] = serde_json::Value::String(d.format("%Y-%m-%d").to_string());
    }
    if let Some(d) = completed_at {
        fm["completed_at"] = serde_json::Value::String(d.format("%Y-%m-%d").to_string());
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

    let t1 = make_task("proj", "T-1", "A", "done", Some(two_days_ago), Some(yesterday));
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

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, "T-2");
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

    let summary =
        build_vault_summary(&all_notes, &tasks, &projects, &Some("proj-a".to_string()));

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
    assert_eq!(report.activity.daily_activity.len(), 8);
}

#[test]
fn stale_notes_excludes_non_actionable_types() {
    let db = IndexDb::open_in_memory().unwrap();

    let task = make_task("proj", "T-1", "Stale task", "todo", None, None);
    let project = make_project("proj", "P", "Stale project", "open");
    let zettel = make_note("Zettelkasten/z1.md", NoteType::Zettel, "A zettel", None);
    let daily = make_note("Journal/2025-01-01.md", NoteType::Daily, "Jan 1", None);

    db.insert_note(&task).unwrap();
    db.insert_note(&project).unwrap();
    db.insert_note(&zettel).unwrap();
    db.insert_note(&daily).unwrap();

    let options =
        DashboardOptions { stale_threshold: 0.5, stale_limit: 50, ..Default::default() };
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
    let old_date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let recent_date = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();

    let project = make_project("proj", "P", "Test Project", "open");

    let zombie_task = make_task("proj", "T-1", "Old task", "todo", Some(old_date), None);
    let fresh_task =
        make_task("proj", "T-2", "Fresh task", "todo", Some(recent_date), None);
    let done_task =
        make_task("proj", "T-3", "Done task", "done", Some(old_date), Some(today));
    let ip_task =
        make_task("proj", "T-4", "Active task", "in-progress", Some(old_date), None);

    let tasks: Vec<&IndexedNote> = vec![&zombie_task, &fresh_task, &done_task, &ip_task];
    let projects: Vec<&IndexedNote> = vec![&project];

    let (_, _, _, zombie) = build_flagged_tasks(&tasks, &projects, today, 30);

    assert_eq!(zombie.len(), 1);
    assert_eq!(zombie[0].id, "T-1");
    assert_eq!(zombie[0].days_overdue, Some(80));
}

#[test]
fn parse_review_interval_parses_correctly() {
    assert_eq!(parse_review_interval("1w"), Some(7));
    assert_eq!(parse_review_interval("2w"), Some(14));
    assert_eq!(parse_review_interval("30d"), Some(30));
    assert_eq!(parse_review_interval("1m"), Some(30));
    assert_eq!(parse_review_interval(""), None);
    assert_eq!(parse_review_interval("abc"), None);
}

#[test]
fn review_due_flags_stale_projects() {
    let today = NaiveDate::from_ymd_opt(2026, 3, 22).unwrap();
    let old_update = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    let recent_update = NaiveDate::from_ymd_opt(2026, 3, 20).unwrap();

    let stale_proj = make_note(
        "Projects/stale/stale.md",
        NoteType::Project,
        "Stale Project",
        Some(
            &serde_json::json!({
                "project-id": "SP",
                "status": "open",
                "kind": "project",
                "review_interval": "1w",
                "updated_at": old_update.format("%Y-%m-%d").to_string(),
            })
            .to_string(),
        ),
    );

    let fresh_proj = make_note(
        "Projects/fresh/fresh.md",
        NoteType::Project,
        "Fresh Project",
        Some(
            &serde_json::json!({
                "project-id": "FP",
                "status": "open",
                "kind": "project",
                "review_interval": "1w",
                "updated_at": recent_update.format("%Y-%m-%d").to_string(),
            })
            .to_string(),
        ),
    );

    let done_proj = make_note(
        "Projects/done/done.md",
        NoteType::Project,
        "Done Project",
        Some(
            &serde_json::json!({
                "project-id": "DP",
                "status": "done",
                "kind": "project",
                "review_interval": "1w",
                "updated_at": old_update.format("%Y-%m-%d").to_string(),
            })
            .to_string(),
        ),
    );

    let reviewed_proj = make_note(
        "Projects/reviewed/reviewed.md",
        NoteType::Project,
        "Reviewed Project",
        Some(
            &serde_json::json!({
                "project-id": "RP",
                "status": "open",
                "kind": "project",
                "review_interval": "2w",
                "updated_at": old_update.format("%Y-%m-%d").to_string(),
                "last_reviewed": recent_update.format("%Y-%m-%d").to_string(),
            })
            .to_string(),
        ),
    );

    let projects: Vec<&IndexedNote> =
        vec![&stale_proj, &fresh_proj, &done_proj, &reviewed_proj];
    let due = build_review_due(&projects, today);

    assert_eq!(due.len(), 1);
    assert_eq!(due[0].id, "SP");
    assert_eq!(due[0].days_overdue, 14);
}
