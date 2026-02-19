//! Project management commands.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::{IndexDb, IndexedNote, NoteQuery, NoteType};
use serde::Serialize;
use std::path::Path;
use tabled::{settings::Style, Table, Tabled};

/// Row for project list table.
#[derive(Tabled)]
struct ProjectRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Open")]
    open: usize,
    #[tabled(rename = "Done")]
    done: usize,
    #[tabled(rename = "Total")]
    total: usize,
}

/// Row for task list in status view.
#[derive(Tabled)]
struct TaskRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Status")]
    status: String,
}

/// List all projects with task counts.
pub fn list(config: Option<&Path>, profile: Option<&str>, status_filter: Option<&str>) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        }
    };

    let index_path = cfg.vault_root.join(".mdvault/index.db");
    let db = match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open index: {e}");
            eprintln!("Run 'mdv reindex' first.");
            std::process::exit(1);
        }
    };

    // Query all projects
    let project_query =
        NoteQuery { note_type: Some(NoteType::Project), ..Default::default() };

    let projects = match db.query_notes(&project_query) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to query projects: {e}");
            std::process::exit(1);
        }
    };

    if projects.is_empty() {
        println!("No projects found.");
        println!("Create one with: mdv new project");
        return;
    }

    // Query all tasks to count per project
    let task_query = NoteQuery { note_type: Some(NoteType::Task), ..Default::default() };
    let tasks = db.query_notes(&task_query).unwrap_or_default();

    // Build table rows
    let mut rows: Vec<ProjectRow> = Vec::new();

    for project in &projects {
        // Get project ID and status from frontmatter
        let (project_id, project_status) = extract_project_info(project);

        // Filter by status if specified
        if let Some(filter) = status_filter {
            if project_status != filter {
                continue;
            }
        }

        let title = if project.title.is_empty() {
            project
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        } else {
            project.title.clone()
        };

        // Count tasks for this project
        let project_folder =
            project.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        let project_tasks: Vec<_> = tasks
            .iter()
            .filter(|t| {
                let path_str = t.path.to_string_lossy();
                path_str.contains(&format!("Projects/{}/", project_folder))
            })
            .collect();

        let total = project_tasks.len();
        let done = project_tasks
            .iter()
            .filter(|t| {
                get_task_status(t)
                    .map(|s| s == "done" || s == "completed")
                    .unwrap_or(false)
            })
            .count();
        let cancelled = project_tasks
            .iter()
            .filter(|t| {
                get_task_status(t)
                    .map(|s| s == "cancelled" || s == "canceled")
                    .unwrap_or(false)
            })
            .count();
        let open = total - done - cancelled;

        rows.push(ProjectRow {
            id: project_id,
            title,
            status: project_status,
            open,
            done,
            total,
        });
    }

    if rows.is_empty() {
        println!("No projects match the filter.");
        return;
    }

    let table = Table::new(&rows).with(Style::rounded()).to_string();

    println!("{}", table);
    println!("\nTotal: {} projects", rows.len());
}

/// Show project status with tasks in kanban-style columns.
pub fn status(config: Option<&Path>, profile: Option<&str>, project_name: &str) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        }
    };

    let index_path = cfg.vault_root.join(".mdvault/index.db");
    let db = match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open index: {e}");
            eprintln!("Run 'mdv reindex' first.");
            std::process::exit(1);
        }
    };

    // Find the project
    let project_query =
        NoteQuery { note_type: Some(NoteType::Project), ..Default::default() };
    let projects = db.query_notes(&project_query).unwrap_or_default();

    let project = projects.iter().find(|p| {
        let folder = p.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let (id, _) = extract_project_info(p);
        folder.eq_ignore_ascii_case(project_name) || id.eq_ignore_ascii_case(project_name)
    });

    let project = match project {
        Some(p) => p,
        None => {
            eprintln!("Project not found: {}", project_name);
            eprintln!("Run 'mdv project list' to see available projects.");
            std::process::exit(1);
        }
    };

    let project_folder = project.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let (project_id, project_status) = extract_project_info(project);
    let project_title = if project.title.is_empty() {
        project_folder.to_string()
    } else {
        project.title.clone()
    };

    // Print project header
    println!("Project: {} [{}]", project_title, project_id);
    println!("Status:  {}", project_status);
    println!();

    // Query all tasks
    let task_query = NoteQuery { note_type: Some(NoteType::Task), ..Default::default() };
    let all_tasks = db.query_notes(&task_query).unwrap_or_default();

    // Filter tasks for this project
    let project_tasks: Vec<_> = all_tasks
        .into_iter()
        .filter(|t| {
            let path_str = t.path.to_string_lossy();
            path_str.contains(&format!("Projects/{}/", project_folder))
                || path_str.contains(&format!("projects/{}/", project_folder))
        })
        .collect();

    if project_tasks.is_empty() {
        println!("No tasks found for this project.");
        println!("Create one with: mdv new task");
        return;
    }

    // Group tasks by status
    let mut todo: Vec<&IndexedNote> = vec![];
    let mut in_progress: Vec<&IndexedNote> = vec![];
    let mut blocked: Vec<&IndexedNote> = vec![];
    let mut done: Vec<&IndexedNote> = vec![];
    let mut cancelled: Vec<&IndexedNote> = vec![];

    for task in &project_tasks {
        let status = get_task_status(task).unwrap_or_else(|| "todo".to_string());

        match status.as_str() {
            "todo" | "open" => todo.push(task),
            "in-progress" | "in_progress" | "doing" => in_progress.push(task),
            "blocked" | "waiting" => blocked.push(task),
            "done" | "completed" => done.push(task),
            "cancelled" | "canceled" => cancelled.push(task),
            _ => todo.push(task),
        }
    }

    // Print summary
    println!("Task Summary:");
    println!("  TODO:        {}", todo.len());
    println!("  In Progress: {}", in_progress.len());
    println!("  Blocked:     {}", blocked.len());
    println!("  Done:        {}", done.len());
    if !cancelled.is_empty() {
        println!("  Cancelled:   {}", cancelled.len());
    }
    println!("  Total:       {}", project_tasks.len());
    println!();

    // Print task tables by status
    if !todo.is_empty() {
        println!("TODO:");
        print_task_table(&todo);
        println!();
    }

    if !in_progress.is_empty() {
        println!("IN PROGRESS:");
        print_task_table(&in_progress);
        println!();
    }

    if !blocked.is_empty() {
        println!("BLOCKED:");
        print_task_table(&blocked);
        println!();
    }

    if !done.is_empty() {
        println!("DONE:");
        print_task_table(&done);
        println!();
    }

    if !cancelled.is_empty() {
        println!("CANCELLED:");
        print_task_table(&cancelled);
        println!();
    }
}

/// Print a table of tasks.
fn print_task_table(tasks: &[&IndexedNote]) {
    let rows: Vec<TaskRow> = tasks
        .iter()
        .map(|task| {
            let task_id = get_task_id(task).unwrap_or_else(|| "-".to_string());
            let title = if task.title.is_empty() {
                task.path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled")
                    .to_string()
            } else {
                task.title.clone()
            };
            let status = get_task_status(task).unwrap_or_else(|| "unknown".to_string());

            TaskRow { id: task_id, title, status }
        })
        .collect();

    let table = Table::new(&rows).with(Style::rounded()).to_string();

    println!("{}", table);
}

/// Extract project ID and status from frontmatter.
fn extract_project_info(project: &IndexedNote) -> (String, String) {
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

    (id, status)
}

/// Get task status from frontmatter.
fn get_task_status(task: &IndexedNote) -> Option<String> {
    task.frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok())
        .and_then(|fm| fm.get("status").and_then(|v| v.as_str()).map(String::from))
}

/// Get task ID from frontmatter.
fn get_task_id(task: &IndexedNote) -> Option<String> {
    task.frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok())
        .and_then(|fm| fm.get("task-id").and_then(|v| v.as_str()).map(String::from))
}

/// Get completed_at timestamp from task frontmatter.
fn get_completed_at(task: &IndexedNote) -> Option<DateTime<Utc>> {
    let fm_json = task.frontmatter_json.as_ref()?;
    let fm: serde_json::Value = serde_json::from_str(fm_json).ok()?;
    let date_str = fm.get("completed_at")?.as_str()?;

    // Try parsing as RFC3339 first, then as date
    DateTime::parse_from_rfc3339(date_str).map(|dt| dt.with_timezone(&Utc)).ok().or_else(
        || {
            NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .ok()
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
        },
    )
}

/// Row for progress table.
#[derive(Tabled)]
struct ProgressRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Progress")]
    progress: String,
    #[tabled(rename = "Bar")]
    bar: String,
}

/// Progress data for JSON output.
#[derive(Serialize)]
struct ProjectProgress {
    id: String,
    title: String,
    status: String,
    tasks: TaskCounts,
    progress_percent: f64,
    recent_completions: Vec<RecentCompletion>,
    velocity: f64,
}

#[derive(Serialize)]
struct TaskCounts {
    total: usize,
    done: usize,
    in_progress: usize,
    todo: usize,
    blocked: usize,
    cancelled: usize,
}

#[derive(Serialize)]
struct RecentCompletion {
    id: String,
    title: String,
    completed_at: String,
    days_ago: i64,
}

/// Generate a progress bar string.
fn progress_bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Show project progress with completion metrics and velocity.
pub fn progress(
    config: Option<&Path>,
    profile: Option<&str>,
    project_name: Option<&str>,
    json_output: bool,
    include_archived: bool,
) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        }
    };

    let index_path = cfg.vault_root.join(".mdvault/index.db");
    let db = match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open index: {e}");
            eprintln!("Run 'mdv reindex' first.");
            std::process::exit(1);
        }
    };

    // Query all projects
    let project_query =
        NoteQuery { note_type: Some(NoteType::Project), ..Default::default() };
    let projects = match db.query_notes(&project_query) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to query projects: {e}");
            std::process::exit(1);
        }
    };

    if projects.is_empty() {
        println!("No projects found.");
        println!("Create one with: mdv new project");
        return;
    }

    // Query all tasks
    let task_query = NoteQuery { note_type: Some(NoteType::Task), ..Default::default() };
    let all_tasks = db.query_notes(&task_query).unwrap_or_default();

    // If specific project requested, show detailed view
    if let Some(name) = project_name {
        let project = projects.iter().find(|p| {
            let folder = p.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let (id, _) = extract_project_info(p);
            folder.eq_ignore_ascii_case(name) || id.eq_ignore_ascii_case(name)
        });

        let project = match project {
            Some(p) => p,
            None => {
                eprintln!("Project not found: {}", name);
                eprintln!("Run 'mdv project list' to see available projects.");
                std::process::exit(1);
            }
        };

        let progress_data = calculate_project_progress(project, &all_tasks);

        if json_output {
            println!("{}", serde_json::to_string_pretty(&progress_data).unwrap());
        } else {
            print_single_project_progress(&progress_data);
        }
    } else {
        // Show all projects in table format
        let mut progress_list: Vec<ProjectProgress> = Vec::new();

        for project in &projects {
            let (_, project_status) = extract_project_info(project);

            // Filter archived unless requested
            if !include_archived && project_status == "archived" {
                continue;
            }

            progress_list.push(calculate_project_progress(project, &all_tasks));
        }

        if progress_list.is_empty() {
            println!("No projects match the filter.");
            return;
        }

        if json_output {
            println!("{}", serde_json::to_string_pretty(&progress_list).unwrap());
        } else {
            print_all_projects_progress(&progress_list);
        }
    }
}

/// Calculate progress data for a single project.
fn calculate_project_progress(
    project: &IndexedNote,
    all_tasks: &[IndexedNote],
) -> ProjectProgress {
    let project_folder = project.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let (project_id, project_status) = extract_project_info(project);
    let project_title = if project.title.is_empty() {
        project_folder.to_string()
    } else {
        project.title.clone()
    };

    // Filter tasks for this project
    let project_tasks: Vec<&IndexedNote> = all_tasks
        .iter()
        .filter(|t| {
            let path_str = t.path.to_string_lossy();
            path_str.contains(&format!("Projects/{}/", project_folder))
                || path_str.contains(&format!("projects/{}/", project_folder))
        })
        .collect();

    // Count by status
    let mut todo = 0;
    let mut in_progress = 0;
    let mut blocked = 0;
    let mut done = 0;
    let mut cancelled = 0;

    for task in &project_tasks {
        let status = get_task_status(task).unwrap_or_else(|| "todo".to_string());
        match status.as_str() {
            "todo" | "open" => todo += 1,
            "in-progress" | "in_progress" | "doing" => in_progress += 1,
            "blocked" | "waiting" => blocked += 1,
            "done" | "completed" => done += 1,
            "cancelled" | "canceled" => cancelled += 1,
            _ => todo += 1,
        }
    }

    let total = project_tasks.len();
    // Exclude cancelled tasks from progress denominator
    let active_total = total - cancelled;
    let progress_percent =
        if active_total > 0 { (done as f64 / active_total as f64) * 100.0 } else { 0.0 };

    // Recent completions (last 7 days)
    let now = Utc::now();
    let seven_days_ago = now - Duration::days(7);
    let mut recent_completions: Vec<RecentCompletion> = Vec::new();

    for task in &project_tasks {
        if let Some(completed_at) = get_completed_at(task) {
            if completed_at >= seven_days_ago {
                let days_ago = (now - completed_at).num_days();
                let task_id = get_task_id(task).unwrap_or_else(|| "-".to_string());
                let title = if task.title.is_empty() {
                    task.path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Untitled")
                        .to_string()
                } else {
                    task.title.clone()
                };

                recent_completions.push(RecentCompletion {
                    id: task_id,
                    title,
                    completed_at: completed_at.format("%Y-%m-%d").to_string(),
                    days_ago,
                });
            }
        }
    }

    // Sort by most recent first
    recent_completions.sort_by(|a, b| a.days_ago.cmp(&b.days_ago));

    // Calculate velocity (tasks per week over last 4 weeks)
    let four_weeks_ago = now - Duration::weeks(4);
    let completed_in_4_weeks: usize = project_tasks
        .iter()
        .filter(|t| get_completed_at(t).map(|ca| ca >= four_weeks_ago).unwrap_or(false))
        .count();
    let velocity = completed_in_4_weeks as f64 / 4.0;

    ProjectProgress {
        id: project_id,
        title: project_title,
        status: project_status,
        tasks: TaskCounts { total, done, in_progress, todo, blocked, cancelled },
        progress_percent,
        recent_completions,
        velocity,
    }
}

/// Print detailed progress for a single project.
fn print_single_project_progress(data: &ProjectProgress) {
    println!("Project: {} [{}]", data.title, data.id);
    println!();

    // Progress bar
    let bar = progress_bar(data.progress_percent, 20);
    println!(
        "Progress: {} {:.0}% ({}/{} tasks done)",
        bar, data.progress_percent, data.tasks.done, data.tasks.total
    );
    println!();

    // By status
    println!("By Status:");
    println!("  ✓ Done:        {}", data.tasks.done);
    println!("  → In Progress: {}", data.tasks.in_progress);
    println!("  ○ Todo:        {}", data.tasks.todo);
    println!("  ⊘ Blocked:     {}", data.tasks.blocked);
    if data.tasks.cancelled > 0 {
        println!("  ✗ Cancelled:   {}", data.tasks.cancelled);
    }
    println!();

    // Recent activity
    if !data.recent_completions.is_empty() {
        println!("Recent Activity (7 days):");
        for completion in &data.recent_completions {
            let ago_text = if completion.days_ago == 0 {
                "today".to_string()
            } else if completion.days_ago == 1 {
                "yesterday".to_string()
            } else {
                format!("{} days ago", completion.days_ago)
            };
            println!("  - {} completed ({})", completion.id, ago_text);
        }
        println!();
    }

    // Velocity
    println!("Velocity: {:.1} tasks/week (last 4 weeks)", data.velocity);
}

/// Print progress table for all projects.
fn print_all_projects_progress(data: &[ProjectProgress]) {
    let rows: Vec<ProgressRow> = data
        .iter()
        .map(|p| {
            let bar = progress_bar(p.progress_percent, 20);
            ProgressRow {
                id: p.id.clone(),
                title: if p.title.len() > 25 {
                    format!("{}...", &p.title[..22])
                } else {
                    p.title.clone()
                },
                progress: format!("{:.0}%", p.progress_percent),
                bar,
            }
        })
        .collect();

    let table = Table::new(&rows).with(Style::rounded()).to_string();
    println!("{}", table);
    println!("\nTotal: {} projects", data.len());
}
