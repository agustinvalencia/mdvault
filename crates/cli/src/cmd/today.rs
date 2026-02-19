//! Daily planning and review dashboard commands.

use chrono::{Local, NaiveDate, Timelike};
use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::{IndexDb, IndexedNote, NoteQuery};
use serde::Serialize;
use std::path::Path;
use tabled::{settings::Style, Table, Tabled};
use tracing::error;

use crate::TodayArgs;

/// Dashboard data for JSON output.
#[derive(Serialize)]
struct DashboardData {
    date: String,
    mode: String, // "plan" or "review"
    daily_note_exists: bool,
    daily_note_path: Option<String>,
    pending_tasks: Vec<TaskInfo>,
    in_progress_tasks: Vec<TaskInfo>,
    completed_today: Vec<TaskInfo>,
    overdue_tasks: Vec<TaskInfo>,
    suggestions: Vec<String>,
}

#[derive(Serialize, Clone)]
struct TaskInfo {
    id: String,
    title: String,
    project: String,
    status: String,
    priority: Option<String>,
    due_date: Option<String>,
}

/// Row for pending tasks table.
#[derive(Tabled)]
struct TaskRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Project")]
    project: String,
    #[tabled(rename = "Priority")]
    priority: String,
}

/// Run the today command.
pub fn run(config: Option<&Path>, profile: Option<&str>, args: TodayArgs) {
    // Handle the 'open' subcommand
    if args.command.is_some() {
        open_daily_note(config, profile);
        return;
    }

    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            error!("Failed to load config: {e}");
            std::process::exit(1);
        }
    };

    let index_path = cfg.vault_root.join(".mdvault/index.db");
    let db = match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(e) => {
            error!("Failed to open index: {e}");
            error!("Run 'mdv reindex' first.");
            std::process::exit(1);
        }
    };

    // Determine mode based on flags or time of day
    let mode = if args.plan {
        "plan"
    } else if args.review {
        "review"
    } else {
        // Auto-select based on time: before noon = plan, after noon = review
        if Local::now().hour() < 12 {
            "plan"
        } else {
            "review"
        }
    };

    let today = Local::now().date_naive();
    let dashboard = gather_dashboard_data(&db, &cfg.vault_root, today, mode);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&dashboard).unwrap());
    } else {
        print_dashboard(&dashboard);
    }
}

/// Open today's daily note in the default editor.
fn open_daily_note(config: Option<&Path>, profile: Option<&str>) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            error!("Failed to load config: {e}");
            std::process::exit(1);
        }
    };

    let today = Local::now().date_naive();
    let daily_path =
        cfg.vault_root.join(format!("Journal/Daily/{}.md", today.format("%Y-%m-%d")));

    if !daily_path.exists() {
        error!(
            "Today's daily note doesn't exist yet. Create it with: mdv new daily \"{}\"",
            today.format("%Y-%m-%d")
        );
        std::process::exit(1);
    }

    // Get editor from environment
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vim".to_string());

    // Open the file in the editor
    let status = std::process::Command::new(&editor).arg(&daily_path).status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            error!("Editor exited with status: {}", s);
            std::process::exit(1);
        }
        Err(e) => {
            error!("Failed to open editor '{}': {}", editor, e);
            std::process::exit(1);
        }
    }
}

/// Gather all data for the dashboard.
fn gather_dashboard_data(
    db: &IndexDb,
    vault_root: &Path,
    today: NaiveDate,
    mode: &str,
) -> DashboardData {
    let all_notes = db.query_notes(&NoteQuery::default()).unwrap_or_default();

    // Check if today's daily note exists
    let daily_path =
        vault_root.join(format!("Journal/Daily/{}.md", today.format("%Y-%m-%d")));
    let daily_note_exists = daily_path.exists();
    let daily_note_path = if daily_note_exists {
        Some(daily_path.to_string_lossy().to_string())
    } else {
        None
    };

    // Filter tasks
    let tasks: Vec<&IndexedNote> = all_notes
        .iter()
        .filter(|n| get_note_type(n) == Some("task".to_string()))
        .collect();

    // Categorize tasks
    let mut pending_tasks: Vec<TaskInfo> = Vec::new();
    let mut in_progress_tasks: Vec<TaskInfo> = Vec::new();
    let mut completed_today: Vec<TaskInfo> = Vec::new();
    let mut overdue_tasks: Vec<TaskInfo> = Vec::new();

    for task in &tasks {
        let info = extract_task_info(task);

        // Check if completed today
        if let Some(completed_at) = get_completed_at(task) {
            if completed_at == today {
                completed_today.push(info);
                continue;
            }
        }

        // Check status
        match info.status.as_str() {
            "done" | "cancelled" | "canceled" => {
                // Skip completed/cancelled tasks (not actionable)
            }
            "in-progress" => {
                // Check if overdue
                if let Some(ref due) = info.due_date {
                    if let Ok(due_date) = NaiveDate::parse_from_str(due, "%Y-%m-%d") {
                        if due_date < today {
                            overdue_tasks.push(info.clone());
                        }
                    }
                }
                in_progress_tasks.push(info);
            }
            _ => {
                // Check if overdue for todo/pending/other non-done statuses
                if let Some(ref due) = info.due_date {
                    if let Ok(due_date) = NaiveDate::parse_from_str(due, "%Y-%m-%d") {
                        if due_date < today {
                            overdue_tasks.push(info.clone());
                        }
                    }
                }
                if info.status != "blocked" {
                    pending_tasks.push(info);
                }
            }
        }
    }

    // Sort tasks by priority
    pending_tasks.sort_by_key(priority_order);
    in_progress_tasks.sort_by_key(priority_order);

    // Generate suggestions based on mode
    let suggestions = generate_suggestions(
        mode,
        daily_note_exists,
        &pending_tasks,
        &in_progress_tasks,
        &completed_today,
        &overdue_tasks,
    );

    DashboardData {
        date: today.format("%Y-%m-%d").to_string(),
        mode: mode.to_string(),
        daily_note_exists,
        daily_note_path,
        pending_tasks,
        in_progress_tasks,
        completed_today,
        overdue_tasks,
        suggestions,
    }
}

/// Generate context-aware suggestions.
fn generate_suggestions(
    mode: &str,
    daily_note_exists: bool,
    pending_tasks: &[TaskInfo],
    in_progress_tasks: &[TaskInfo],
    completed_today: &[TaskInfo],
    overdue_tasks: &[TaskInfo],
) -> Vec<String> {
    let mut suggestions = Vec::new();

    // Always suggest creating daily note if missing
    if !daily_note_exists {
        suggestions.push("Create today's daily note: mdv new daily".to_string());
    }

    if mode == "plan" {
        // Morning planning suggestions
        if !overdue_tasks.is_empty() {
            suggestions.push(format!(
                "Address {} overdue task(s) - reschedule or complete",
                overdue_tasks.len()
            ));
        }

        if in_progress_tasks.len() > 3 {
            suggestions.push(format!(
                "You have {} tasks in progress - consider finishing some before starting new ones",
                in_progress_tasks.len()
            ));
        }

        if pending_tasks.is_empty() && in_progress_tasks.is_empty() {
            suggestions
                .push("No active tasks! Check projects for work to pull in".to_string());
        } else {
            // Suggest high priority tasks
            let high_priority: Vec<&TaskInfo> = pending_tasks
                .iter()
                .filter(|t| {
                    t.priority.as_deref() == Some("high")
                        || t.priority.as_deref() == Some("urgent")
                })
                .collect();

            if !high_priority.is_empty() {
                suggestions.push(format!(
                    "Focus on {} high-priority task(s) today",
                    high_priority.len()
                ));
            }
        }
    } else {
        // Evening review suggestions
        if completed_today.is_empty() {
            suggestions.push(
                "No tasks completed today - consider what blocked progress".to_string(),
            );
        } else {
            suggestions.push(format!(
                "Great job! Completed {} task(s) today",
                completed_today.len()
            ));
        }

        if !overdue_tasks.is_empty() {
            suggestions.push(format!(
                "Reschedule or address {} overdue task(s) for tomorrow",
                overdue_tasks.len()
            ));
        }

        // Check for stale in-progress tasks
        if in_progress_tasks.len() > 2 {
            suggestions
                .push("Review in-progress tasks - any that can be closed?".to_string());
        }
    }

    suggestions
}

/// Print dashboard to terminal.
fn print_dashboard(data: &DashboardData) {
    let mode_title =
        if data.mode == "plan" { "Morning Planning" } else { "Evening Review" };

    println!();
    println!("{}", "=".repeat(65));
    println!("{:^65}", format!("{} - {}", mode_title, data.date));
    println!("{}", "=".repeat(65));
    println!();

    // Daily note status
    if data.daily_note_exists {
        println!("Daily note: [x] exists");
    } else {
        println!("Daily note: [ ] not created yet");
    }
    println!();

    // Show different sections based on mode
    if data.mode == "plan" {
        print_plan_mode(data);
    } else {
        print_review_mode(data);
    }

    // Suggestions
    if !data.suggestions.is_empty() {
        println!("SUGGESTIONS");
        for suggestion in &data.suggestions {
            println!("  - {}", suggestion);
        }
        println!();
    }
}

/// Print plan mode specific sections.
fn print_plan_mode(data: &DashboardData) {
    // Overdue tasks (if any)
    if !data.overdue_tasks.is_empty() {
        println!(
            "OVERDUE ({} task{})",
            data.overdue_tasks.len(),
            if data.overdue_tasks.len() == 1 { "" } else { "s" }
        );
        let rows: Vec<TaskRow> =
            data.overdue_tasks.iter().take(5).map(task_to_row).collect();
        let table = Table::new(&rows).with(Style::rounded()).to_string();
        println!("{}", table);
        println!();
    }

    // In-progress tasks
    if !data.in_progress_tasks.is_empty() {
        println!(
            "IN PROGRESS ({} task{})",
            data.in_progress_tasks.len(),
            if data.in_progress_tasks.len() == 1 { "" } else { "s" }
        );
        let rows: Vec<TaskRow> =
            data.in_progress_tasks.iter().take(5).map(task_to_row).collect();
        let table = Table::new(&rows).with(Style::rounded()).to_string();
        println!("{}", table);
        println!();
    }

    // Pending tasks (top priority ones)
    if !data.pending_tasks.is_empty() {
        println!(
            "PENDING ({} task{}) - Top priority shown",
            data.pending_tasks.len(),
            if data.pending_tasks.len() == 1 { "" } else { "s" }
        );
        let rows: Vec<TaskRow> =
            data.pending_tasks.iter().take(8).map(task_to_row).collect();
        let table = Table::new(&rows).with(Style::rounded()).to_string();
        println!("{}", table);
        println!();
    }
}

/// Print review mode specific sections.
fn print_review_mode(data: &DashboardData) {
    // Completed today
    if !data.completed_today.is_empty() {
        println!(
            "COMPLETED TODAY ({} task{})",
            data.completed_today.len(),
            if data.completed_today.len() == 1 { "" } else { "s" }
        );
        let rows: Vec<TaskRow> =
            data.completed_today.iter().take(10).map(task_to_row).collect();
        let table = Table::new(&rows).with(Style::rounded()).to_string();
        println!("{}", table);
        println!();
    } else {
        println!("COMPLETED TODAY: None");
        println!();
    }

    // Overdue tasks
    if !data.overdue_tasks.is_empty() {
        println!(
            "OVERDUE - Need attention ({} task{})",
            data.overdue_tasks.len(),
            if data.overdue_tasks.len() == 1 { "" } else { "s" }
        );
        let rows: Vec<TaskRow> =
            data.overdue_tasks.iter().take(5).map(task_to_row).collect();
        let table = Table::new(&rows).with(Style::rounded()).to_string();
        println!("{}", table);
        println!();
    }

    // Still in progress
    if !data.in_progress_tasks.is_empty() {
        println!(
            "STILL IN PROGRESS ({} task{})",
            data.in_progress_tasks.len(),
            if data.in_progress_tasks.len() == 1 { "" } else { "s" }
        );
        let rows: Vec<TaskRow> =
            data.in_progress_tasks.iter().take(5).map(task_to_row).collect();
        let table = Table::new(&rows).with(Style::rounded()).to_string();
        println!("{}", table);
        println!();
    }
}

/// Convert TaskInfo to table row.
fn task_to_row(task: &TaskInfo) -> TaskRow {
    TaskRow {
        id: task.id.clone(),
        title: if task.title.len() > 35 {
            format!("{}...", &task.title[..32])
        } else {
            task.title.clone()
        },
        project: task.project.clone(),
        priority: task.priority.clone().unwrap_or_else(|| "-".to_string()),
    }
}

/// Get priority order for sorting (lower = higher priority).
fn priority_order(t: &TaskInfo) -> u8 {
    match t.priority.as_deref() {
        Some("high") | Some("urgent") => 0,
        Some("medium") | Some("normal") => 1,
        Some("low") => 2,
        _ => 3,
    }
}

// --- Helper functions ---

/// Get note type from frontmatter.
fn get_note_type(note: &IndexedNote) -> Option<String> {
    note.frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok())
        .and_then(|fm| fm.get("type").and_then(|v| v.as_str()).map(String::from))
}

/// Get completed_at date from frontmatter.
fn get_completed_at(note: &IndexedNote) -> Option<NaiveDate> {
    let fm_json = note.frontmatter_json.as_ref()?;
    let fm: serde_json::Value = serde_json::from_str(fm_json).ok()?;
    let date_str = fm.get("completed_at")?.as_str()?;

    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok().or_else(|| {
        chrono::DateTime::parse_from_rfc3339(date_str).ok().map(|dt| dt.date_naive())
    })
}

/// Extract task info from indexed note.
fn extract_task_info(note: &IndexedNote) -> TaskInfo {
    let fm = note
        .frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok());

    let id = fm
        .as_ref()
        .and_then(|fm| fm.get("task-id").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| "-".to_string());

    let title = if note.title.is_empty() {
        note.path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").to_string()
    } else {
        note.title.clone()
    };

    let project = fm
        .as_ref()
        .and_then(|fm| fm.get("project").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| {
            // Try to extract from path
            let path_str = note.path.to_string_lossy();
            if path_str.contains("Projects/") {
                let parts: Vec<&str> = path_str.split("Projects/").collect();
                if parts.len() > 1 {
                    if let Some(project_name) = parts[1].split('/').next() {
                        return project_name.to_string();
                    }
                }
            }
            "INB".to_string()
        });

    let status = fm
        .as_ref()
        .and_then(|fm| fm.get("status").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| "todo".to_string());

    let priority = fm
        .as_ref()
        .and_then(|fm| fm.get("priority").and_then(|v| v.as_str()))
        .map(String::from);

    let due_date = fm
        .as_ref()
        .and_then(|fm| fm.get("due_date").and_then(|v| v.as_str()))
        .map(String::from);

    TaskInfo { id, title, project, status, priority, due_date }
}
