//! Task management commands.

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::{IndexDb, IndexedNote, NoteQuery, NoteType};
use std::path::Path;
use tabled::{settings::Style, Table, Tabled};

/// Row for task list table.
#[derive(Tabled)]
struct TaskListRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Project")]
    project: String,
}

/// List tasks with optional filters.
pub fn list(
    config: Option<&Path>,
    profile: Option<&str>,
    project_filter: Option<&str>,
    status_filter: Option<&str>,
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

    // Query all tasks
    let query = NoteQuery { note_type: Some(NoteType::Task), ..Default::default() };

    let tasks = match db.query_notes(&query) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to query tasks: {e}");
            std::process::exit(1);
        }
    };

    if tasks.is_empty() {
        println!("No tasks found.");
        return;
    }

    // Build table rows
    let mut rows: Vec<TaskListRow> = Vec::new();

    for task in &tasks {
        let path_str = task.path.to_string_lossy();

        // Filter by project if specified
        if let Some(proj) = project_filter {
            if !path_str.contains(proj) {
                continue;
            }
        }

        // Get task info from frontmatter
        let (task_id, task_status, project) = extract_task_info(task);

        // Filter by status if specified
        if let Some(status) = status_filter {
            if task_status != status {
                continue;
            }
        }

        let title = if task.title.is_empty() {
            task.path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        } else {
            task.title.clone()
        };

        rows.push(TaskListRow { id: task_id, title, status: task_status, project });
    }

    if rows.is_empty() {
        println!("No tasks match the filter.");
        return;
    }

    // Sort by project then ID
    rows.sort_by(|a, b| a.project.cmp(&b.project).then_with(|| a.id.cmp(&b.id)));

    let table = Table::new(&rows).with(Style::rounded()).to_string();

    println!("{}", table);
    println!("\nTotal: {} tasks", rows.len());
}

/// Show detailed status for a specific task.
pub fn status(config: Option<&Path>, profile: Option<&str>, task_id: &str) {
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

    // Query all tasks and find the one with matching ID
    let query = NoteQuery { note_type: Some(NoteType::Task), ..Default::default() };
    let tasks = db.query_notes(&query).unwrap_or_default();

    let task = tasks.iter().find(|t| {
        let (id, _, _) = extract_task_info(t);
        id.eq_ignore_ascii_case(task_id)
    });

    let task = match task {
        Some(t) => t,
        None => {
            // Also try matching by filename
            let task_by_path = tasks.iter().find(|t| {
                let stem = t.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                stem.eq_ignore_ascii_case(task_id)
            });
            match task_by_path {
                Some(t) => t,
                None => {
                    eprintln!("Task not found: {}", task_id);
                    eprintln!("Run 'mdv task list' to see available tasks.");
                    std::process::exit(1);
                }
            }
        }
    };

    // Extract all task info
    let (id, status, project) = extract_task_info(task);
    let title = if task.title.is_empty() {
        task.path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").to_string()
    } else {
        task.title.clone()
    };

    // Get additional info from frontmatter
    let fm = task
        .frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok());

    let created = fm
        .as_ref()
        .and_then(|fm| fm.get("created").and_then(|v| v.as_str()))
        .unwrap_or("-");

    let completed_at = fm
        .as_ref()
        .and_then(|fm| fm.get("completed_at").and_then(|v| v.as_str()))
        .unwrap_or("-");

    // Print task details
    println!("Task: {} [{}]", title, id);
    println!();
    println!("  Status:       {}", status);
    println!("  Project:      {}", project);
    println!("  Created:      {}", created);
    if status == "done" || status == "completed" {
        println!("  Completed:    {}", completed_at);
    }
    println!("  Path:         {}", task.path.display());
}

/// Mark a task as done.
pub fn done(
    config: Option<&Path>,
    profile: Option<&str>,
    task_path: &Path,
    summary: Option<&str>,
) {
    let cfg = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        }
    };

    // Resolve task path relative to vault root
    let full_path = if task_path.is_absolute() {
        task_path.to_path_buf()
    } else {
        cfg.vault_root.join(task_path)
    };

    if !full_path.exists() {
        eprintln!("Task not found: {}", full_path.display());
        std::process::exit(1);
    }

    // Read the task file
    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read task: {e}");
            std::process::exit(1);
        }
    };

    // Parse and update frontmatter
    let parsed = match mdvault_core::frontmatter::parse(&content) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to parse task frontmatter: {e}");
            std::process::exit(1);
        }
    };

    let mut fm = match parsed.frontmatter {
        Some(fm) => fm,
        None => {
            eprintln!("Task has no frontmatter");
            std::process::exit(1);
        }
    };

    // Update status to done
    fm.fields.insert("status".to_string(), serde_yaml::Value::String("done".to_string()));

    // Update completed_at
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    fm.fields.insert("completed_at".to_string(), serde_yaml::Value::String(now.clone()));

    // Get task ID for output
    let task_id = fm
        .fields
        .get("task-id")
        .and_then(|v| match v {
            serde_yaml::Value::String(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_else(|| {
            full_path.file_stem().and_then(|s| s.to_str()).unwrap_or("task").to_string()
        });

    // Rebuild the document
    let mut mapping = serde_yaml::Mapping::new();
    for (k, v) in fm.fields {
        mapping.insert(serde_yaml::Value::String(k), v);
    }
    let yaml_str = match serde_yaml::to_string(&serde_yaml::Value::Mapping(mapping)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to serialize frontmatter: {e}");
            std::process::exit(1);
        }
    };

    // Append summary to body if provided
    let body = if let Some(sum) = summary {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let time = chrono::Local::now().format("%H:%M").to_string();
        format!(
            "{}\n- **[[{}]] {}** : Completed - {}\n",
            parsed.body.trim_end(),
            today,
            time,
            sum
        )
    } else {
        parsed.body
    };

    let final_content = format!("---\n{}---\n{}", yaml_str, body);

    // Write back
    if let Err(e) = std::fs::write(&full_path, final_content) {
        eprintln!("Failed to write task: {e}");
        std::process::exit(1);
    }

    println!("OK   mdv task done");
    println!("task:   {}", task_id);
    println!("status: done");
    if summary.is_some() {
        println!("summary: logged to task");
    }
}

/// Extract task ID, status, and project from frontmatter.
fn extract_task_info(task: &IndexedNote) -> (String, String, String) {
    let fm = task
        .frontmatter_json
        .as_ref()
        .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok());

    let id = fm
        .as_ref()
        .and_then(|fm| fm.get("task-id").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| "-".to_string());

    let status = fm
        .as_ref()
        .and_then(|fm| fm.get("status").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| "unknown".to_string());

    let project = fm
        .as_ref()
        .and_then(|fm| fm.get("project").and_then(|v| v.as_str()))
        .map(String::from)
        .unwrap_or_else(|| {
            // Try to extract from path
            extract_project_from_path(&task.path.to_string_lossy())
        });

    (id, status, project)
}

/// Extract project name from a task path.
fn extract_project_from_path(path: &str) -> String {
    // Expected format: Projects/<project>/Tasks/<task>.md
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 && parts[0] == "Projects" {
        return parts[1].to_string();
    }
    "inbox".to_string()
}
