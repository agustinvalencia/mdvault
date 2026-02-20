//! Integration tests for project archive command.

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

/// Create a minimal project with given status and tasks.
fn scaffold_project(
    vault: &std::path::Path,
    project_slug: &str,
    project_id: &str,
    project_status: &str,
    tasks: &[(&str, &str)], // (task_id, status)
) -> std::path::PathBuf {
    let project_dir = vault.join(format!("Projects/{}", project_slug));
    fs::create_dir_all(&project_dir).unwrap();
    let project_file = project_dir.join(format!("{}.md", project_slug));
    fs::write(
        &project_file,
        format!(
            "---\ntype: project\ntitle: {slug}\nproject-id: {pid}\ntask_counter: {tc}\nstatus: {status}\n---\n\n## Logs\n",
            slug = project_slug,
            pid = project_id,
            tc = tasks.len(),
            status = project_status,
        ),
    )
    .unwrap();

    // Create tasks
    let tasks_dir = project_dir.join("Tasks");
    fs::create_dir_all(&tasks_dir).unwrap();
    for (task_id, task_status) in tasks {
        let task_file = tasks_dir.join(format!("{}.md", task_id));
        fs::write(
            &task_file,
            format!(
                "---\ntype: task\ntitle: Task {tid}\ntask-id: {tid}\nproject: {slug}\nstatus: {status}\n---\n\n## Notes\n",
                tid = task_id,
                slug = project_slug,
                status = task_status,
            ),
        )
        .unwrap();
    }

    project_file
}

#[test]
fn archive_moves_files_and_cancels_tasks() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    // Create a done project with mix of tasks
    scaffold_project(
        &vault,
        "test-proj",
        "TST",
        "done",
        &[("TST-001", "done"), ("TST-002", "todo"), ("TST-003", "doing")],
    );

    // Build index
    mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "reindex"])
        .output()
        .expect("Failed to reindex");

    // Run archive
    let output = mdv_cmd()
        .args([
            "--config",
            config.to_str().unwrap(),
            "project",
            "archive",
            "test-proj",
            "--yes",
        ])
        .output()
        .expect("Failed to execute archive");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("OK   mdv project archive"),
        "Expected success message, got stdout: {}\nstderr: {}",
        stdout,
        stderr
    );

    // Assert: project file moved to archive
    let archived_project = vault.join("Projects/_archive/test-proj/test-proj.md");
    assert!(archived_project.exists(), "Project file should be in archive");

    // Assert: original project dir is gone
    let original_dir = vault.join("Projects/test-proj");
    assert!(!original_dir.exists(), "Original project directory should be removed");

    // Assert: task files moved to archive
    let archived_task1 = vault.join("Projects/_archive/test-proj/Tasks/TST-001.md");
    let archived_task2 = vault.join("Projects/_archive/test-proj/Tasks/TST-002.md");
    let archived_task3 = vault.join("Projects/_archive/test-proj/Tasks/TST-003.md");
    assert!(archived_task1.exists(), "Task TST-001 should be in archive");
    assert!(archived_task2.exists(), "Task TST-002 should be in archive");
    assert!(archived_task3.exists(), "Task TST-003 should be in archive");

    // Assert: project has status: archived and archived_at
    let project_content = fs::read_to_string(&archived_project).unwrap();
    assert!(
        project_content.contains("archived"),
        "Project should have archived status. Content:\n{}",
        project_content
    );
    assert!(
        project_content.contains("archived_at"),
        "Project should have archived_at timestamp. Content:\n{}",
        project_content
    );

    // Assert: open tasks have status: cancelled
    let task2_content = fs::read_to_string(&archived_task2).unwrap();
    assert!(
        task2_content.contains("cancelled"),
        "Open task TST-002 should be cancelled. Content:\n{}",
        task2_content
    );

    let task3_content = fs::read_to_string(&archived_task3).unwrap();
    assert!(
        task3_content.contains("cancelled"),
        "Open task TST-003 should be cancelled. Content:\n{}",
        task3_content
    );

    // Assert: done task is unchanged (still done)
    let task1_content = fs::read_to_string(&archived_task1).unwrap();
    assert!(
        task1_content.contains("status: done") || task1_content.contains("\"done\""),
        "Done task TST-001 should still be done. Content:\n{}",
        task1_content
    );

    // Assert: daily note has archive log entry
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let year = &today[..4];
    let daily_path = vault.join(format!("Journal/{}/Daily/{}.md", year, today));
    assert!(daily_path.exists(), "Daily note should be created");
    let daily_content = fs::read_to_string(&daily_path).unwrap();
    assert!(
        daily_content.contains("Archived project"),
        "Daily note should contain archive entry. Content:\n{}",
        daily_content
    );
}

#[test]
fn archive_rejects_non_done_project() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    // Create an active (not done) project
    scaffold_project(&vault, "active-proj", "ACT", "open", &[]);

    // Build index
    mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "reindex"])
        .output()
        .expect("Failed to reindex");

    // Attempt archive - should fail
    let output = mdv_cmd()
        .args([
            "--config",
            config.to_str().unwrap(),
            "project",
            "archive",
            "active-proj",
            "--yes",
        ])
        .output()
        .expect("Failed to execute archive");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cannot archive") || stderr.contains("must be 'done'"),
        "Should reject non-done project. stderr: {}",
        stderr
    );

    // Project should still be in original location
    assert!(
        vault.join("Projects/active-proj/active-proj.md").exists(),
        "Project should not have been moved"
    );
}
