//! Project management commands.

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::{IndexDb, IndexedNote, NoteQuery, NoteType};
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
        let open = total - done;

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

    for task in &project_tasks {
        let status = get_task_status(task).unwrap_or_else(|| "todo".to_string());

        match status.as_str() {
            "todo" | "open" => todo.push(task),
            "in-progress" | "in_progress" | "doing" => in_progress.push(task),
            "blocked" | "waiting" => blocked.push(task),
            "done" | "completed" => done.push(task),
            _ => todo.push(task),
        }
    }

    // Print summary
    println!("Task Summary:");
    println!("  TODO:        {}", todo.len());
    println!("  In Progress: {}", in_progress.len());
    println!("  Blocked:     {}", blocked.len());
    println!("  Done:        {}", done.len());
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
