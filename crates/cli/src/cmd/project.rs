//! Project management commands.

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::{IndexDb, NoteQuery, NoteType};
use std::path::Path;

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

    // Filter projects by status if specified
    let filtered_projects: Vec<_> = projects
        .into_iter()
        .filter(|project| {
            if let Some(status) = status_filter {
                if let Some(ref fm_json) = project.frontmatter_json {
                    if let Ok(fm) = serde_json::from_str::<serde_json::Value>(fm_json) {
                        if let Some(proj_status) =
                            fm.get("status").and_then(|s| s.as_str())
                        {
                            return proj_status == status;
                        }
                    }
                }
                return false;
            }
            true
        })
        .collect();

    println!("## Projects\n");

    for project in &filtered_projects {
        let title = if project.title.is_empty() {
            project.path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled")
        } else {
            &project.title
        };

        // Get project status
        let status = project
            .frontmatter_json
            .as_ref()
            .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok())
            .and_then(|fm| fm.get("status").and_then(|s| s.as_str()).map(String::from))
            .unwrap_or_else(|| "unknown".to_string());

        // Count tasks for this project
        let project_name =
            project.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        let project_tasks: Vec<_> = tasks
            .iter()
            .filter(|t| {
                let path_str = t.path.to_string_lossy();
                path_str.contains(&format!("Projects/{}/", project_name))
            })
            .collect();

        let total_tasks = project_tasks.len();
        let done_tasks = project_tasks
            .iter()
            .filter(|t| {
                t.frontmatter_json
                    .as_ref()
                    .and_then(|fm| serde_json::from_str::<serde_json::Value>(fm).ok())
                    .and_then(|fm| {
                        fm.get("status").and_then(|s| s.as_str()).map(String::from)
                    })
                    .map(|s| s == "done" || s == "completed")
                    .unwrap_or(false)
            })
            .count();

        let open_tasks = total_tasks - done_tasks;

        let status_icon = match status.as_str() {
            "active" => "[*]",
            "completed" | "done" => "[x]",
            "on-hold" | "paused" => "[~]",
            "archived" => "[-]",
            _ => "[ ]",
        };

        println!(
            "{} {} ({}) - {} open, {} done",
            status_icon, title, status, open_tasks, done_tasks
        );
    }

    println!();
    println!("Total: {} projects", filtered_projects.len());
}
