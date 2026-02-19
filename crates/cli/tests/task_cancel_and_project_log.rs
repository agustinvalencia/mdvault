//! Integration tests for task cancel and project logging on done/cancel.

use std::fs;
use std::io::Write;
use std::process::Command;
use tempfile::tempdir;

fn mdv_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mdv"))
}

fn create_test_config(vault_path: &std::path::Path, config_path: &std::path::Path) {
    let config_content = format!(
        r#"
version = 1
profile = "test"

[profiles.test]
vault_root = "{}"
templates_dir = "{}/templates"
captures_dir = "{}/captures"
macros_dir = "{}/macros"
"#,
        vault_path.display(),
        vault_path.display(),
        vault_path.display(),
        vault_path.display()
    );

    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    let mut file = fs::File::create(config_path).unwrap();
    file.write_all(config_content.as_bytes()).unwrap();
}

/// Create a minimal project and task structure for testing.
fn scaffold_project_and_task(
    vault: &std::path::Path,
    project_slug: &str,
    project_id: &str,
    task_id: &str,
    task_status: &str,
    due_date: Option<&str>,
) -> (std::path::PathBuf, std::path::PathBuf) {
    // Create project file
    let project_dir = vault.join(format!("Projects/{}", project_slug));
    fs::create_dir_all(&project_dir).unwrap();
    let project_file = project_dir.join(format!("{}.md", project_slug));
    fs::write(
        &project_file,
        format!(
            "---\ntype: project\ntitle: {slug}\nproject-id: {pid}\ntask_counter: 1\nstatus: active\n---\n\n## Logs\n",
            slug = project_slug,
            pid = project_id,
        ),
    )
    .unwrap();

    // Create task file
    let tasks_dir = project_dir.join("Tasks");
    fs::create_dir_all(&tasks_dir).unwrap();
    let task_file = tasks_dir.join(format!("{}.md", task_id));
    let mut fm = format!(
        "---\ntype: task\ntitle: Test task\ntask-id: {tid}\nproject: {slug}\nstatus: {status}\n",
        tid = task_id,
        slug = project_slug,
        status = task_status,
    );
    if let Some(due) = due_date {
        fm.push_str(&format!("due_date: {}\n", due));
    }
    fm.push_str("---\n\n## Notes\n");
    fs::write(&task_file, &fm).unwrap();

    (project_file, task_file)
}

#[test]
fn task_done_logs_to_project_note() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    let (project_file, _task_file) =
        scaffold_project_and_task(&vault, "test-proj", "TST", "TST-001", "todo", None);

    // Run mdv task done
    let task_rel = "Projects/test-proj/Tasks/TST-001.md";
    let output = mdv_cmd()
        .args([
            "--config",
            config.to_str().unwrap(),
            "task",
            "done",
            task_rel,
            "--summary",
            "All done",
        ])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("OK   mdv task done"),
        "Expected success message, got: {}",
        stdout
    );

    // Check project note has completion log entry
    let project_content = fs::read_to_string(&project_file).unwrap();
    assert!(
        project_content.contains("Completed task [[TST-001]]"),
        "Project note should contain completion log entry. Content:\n{}",
        project_content
    );

    // Check daily note has completion log entry
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let daily_path = vault.join(format!("Journal/Daily/{}.md", today));
    assert!(daily_path.exists(), "Daily note should be created");
    let daily_content = fs::read_to_string(&daily_path).unwrap();
    assert!(
        daily_content.contains("Completed task TST-001"),
        "Daily note should contain completion entry. Content:\n{}",
        daily_content
    );
}

#[test]
fn task_cancel_sets_status_and_logs() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    let (project_file, task_file) =
        scaffold_project_and_task(&vault, "test-proj", "TST", "TST-002", "todo", None);

    // Run mdv task cancel
    let task_rel = "Projects/test-proj/Tasks/TST-002.md";
    let output = mdv_cmd()
        .args([
            "--config",
            config.to_str().unwrap(),
            "task",
            "cancel",
            task_rel,
            "--reason",
            "No longer needed",
        ])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("OK   mdv task cancel"),
        "Expected success message, got: {}",
        stdout
    );

    // Check task frontmatter has status: cancelled
    let task_content = fs::read_to_string(&task_file).unwrap();
    assert!(
        task_content.contains("cancelled"),
        "Task should have status cancelled. Content:\n{}",
        task_content
    );
    assert!(
        task_content.contains("cancelled_at"),
        "Task should have cancelled_at timestamp. Content:\n{}",
        task_content
    );

    // Check project note has cancellation log entry
    let project_content = fs::read_to_string(&project_file).unwrap();
    assert!(
        project_content.contains("Cancelled task [[TST-002]]"),
        "Project note should contain cancellation log entry. Content:\n{}",
        project_content
    );

    // Check daily note has cancellation log entry
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let daily_path = vault.join(format!("Journal/Daily/{}.md", today));
    assert!(daily_path.exists(), "Daily note should be created");
    let daily_content = fs::read_to_string(&daily_path).unwrap();
    assert!(
        daily_content.contains("Cancelled task TST-002"),
        "Daily note should contain cancellation entry. Content:\n{}",
        daily_content
    );
}

#[test]
fn cancelled_task_not_shown_as_overdue() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    // Create a task with a past due date
    let (_project_file, _task_file) = scaffold_project_and_task(
        &vault,
        "test-proj",
        "TST",
        "TST-003",
        "todo",
        Some("2020-01-01"),
    );

    // Build index first
    mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "reindex"])
        .output()
        .expect("Failed to reindex");

    // Cancel the overdue task
    let task_rel = "Projects/test-proj/Tasks/TST-003.md";
    mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "task", "cancel", task_rel])
        .output()
        .expect("Failed to cancel task");

    // Rebuild index after cancel
    mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "reindex"])
        .output()
        .expect("Failed to reindex");

    // Run today command in JSON mode
    let output = mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "today", "--json"])
        .output()
        .expect("Failed to run today");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Expected valid JSON from today --json");

    // The cancelled task should NOT appear in overdue_tasks
    let empty = vec![];
    let overdue = json["overdue_tasks"].as_array().unwrap_or(&empty);
    let has_cancelled_task = overdue.iter().any(|t| t["id"].as_str() == Some("TST-003"));
    assert!(
        !has_cancelled_task,
        "Cancelled task should not appear as overdue. Overdue tasks: {:?}",
        overdue
    );

    // The cancelled task should NOT appear in pending_tasks either
    let pending = json["pending_tasks"].as_array().unwrap_or(&empty);
    let has_cancelled_in_pending =
        pending.iter().any(|t| t["id"].as_str() == Some("TST-003"));
    assert!(
        !has_cancelled_in_pending,
        "Cancelled task should not appear as pending. Pending tasks: {:?}",
        pending
    );
}
