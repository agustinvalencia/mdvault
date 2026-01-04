//! Task management commands.

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::{IndexDb, NoteQuery, NoteType};
use std::path::Path;

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

    // Filter and group tasks
    let mut filtered_tasks: Vec<_> = tasks
        .into_iter()
        .filter(|task| {
            // Filter by project if specified
            if let Some(proj) = project_filter {
                let path_str = task.path.to_string_lossy();
                if !path_str.contains(proj) {
                    return false;
                }
            }

            // Filter by status if specified
            if let Some(status) = status_filter {
                if let Some(ref fm_json) = task.frontmatter_json {
                    if let Ok(fm) = serde_json::from_str::<serde_json::Value>(fm_json) {
                        if let Some(task_status) =
                            fm.get("status").and_then(|s| s.as_str())
                        {
                            if task_status != status {
                                return false;
                            }
                        }
                    }
                }
            }

            true
        })
        .collect();

    // Sort by project path
    filtered_tasks.sort_by(|a, b| a.path.cmp(&b.path));

    // Group by project
    let mut current_project = String::new();
    let mut task_count = 0;

    for task in &filtered_tasks {
        // Extract project from path (e.g., Projects/foo/Tasks/bar.md -> foo)
        let path_str = task.path.to_string_lossy();
        let project = extract_project_from_path(&path_str);

        if project != current_project {
            if !current_project.is_empty() {
                println!();
            }
            println!("## {}", if project.is_empty() { "No Project" } else { &project });
            current_project = project;
        }

        // Get status from frontmatter
        let status = task
            .frontmatter_json
            .as_ref()
            .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok())
            .and_then(|fm| fm.get("status").and_then(|s| s.as_str()).map(String::from))
            .unwrap_or_else(|| "unknown".to_string());

        let status_icon = match status.as_str() {
            "todo" => "[ ]",
            "in-progress" | "in_progress" | "doing" => "[~]",
            "done" | "completed" => "[x]",
            "blocked" | "waiting" => "[!]",
            _ => "[ ]",
        };

        let title = if task.title.is_empty() {
            task.path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled")
        } else {
            &task.title
        };

        println!("  {} {} ({})", status_icon, title, task.path.display());
        task_count += 1;
    }

    println!();
    println!("Total: {} tasks", task_count);
}

/// Extract project name from a task path.
fn extract_project_from_path(path: &str) -> String {
    // Expected format: Projects/<project>/Tasks/<task>.md
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 && parts[0] == "Projects" {
        return parts[1].to_string();
    }
    String::new()
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

    let task_name = full_path.file_stem().and_then(|s| s.to_str()).unwrap_or("task");

    println!("OK   mdv task done");
    println!("task: {}", task_name);
    println!("status: done");
    if summary.is_some() {
        println!("summary: logged to task");
    }
}
