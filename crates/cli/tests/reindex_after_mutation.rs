//! Integration tests for MDV-018: verify that vault mutations trigger reindex
//! so newly created/modified notes are immediately visible in index queries.

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

// --- Test Harness (same pattern as new_builtin_types.rs) ---

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn setup_vault() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let cfg_path = setup_config(&tmp, &vault);
    (tmp, vault, cfg_path)
}

fn setup_config(tmp: &tempfile::TempDir, vault: &Path) -> PathBuf {
    let xdg = tmp.path().join("xdg");
    let cfg_dir = xdg.join("mdvault");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();

    fs::create_dir_all(vault.join(".mdvault/typedefs")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/templates")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/captures")).unwrap();
    fs::create_dir_all(vault.join(".mdvault/macros")).unwrap();

    let mut toml = String::new();
    writeln!(&mut toml, "version = 1").unwrap();
    writeln!(&mut toml, "profile = \"default\"").unwrap();
    writeln!(&mut toml).unwrap();
    writeln!(&mut toml, "[profiles.default]").unwrap();
    writeln!(&mut toml, "vault_root = \"{}\"", vault.display()).unwrap();
    writeln!(&mut toml, "typedefs_dir = \"{}/.mdvault/typedefs\"", vault.display())
        .unwrap();
    writeln!(&mut toml, "templates_dir = \"{}/.mdvault/templates\"", vault.display())
        .unwrap();
    writeln!(&mut toml, "captures_dir = \"{}/.mdvault/captures\"", vault.display())
        .unwrap();
    writeln!(&mut toml, "macros_dir = \"{}/.mdvault/macros\"", vault.display()).unwrap();

    fs::write(&cfg_path, toml).unwrap();
    cfg_path
}

fn run_mdv(cfg_path: &Path, args: &[&str]) -> std::process::Output {
    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("NO_COLOR", "1");
    let vault_root =
        cfg_path.parent().unwrap().parent().unwrap().parent().unwrap().join("vault");
    cmd.current_dir(&vault_root);
    cmd.args(["--config", cfg_path.to_str().unwrap()]);
    cmd.args(args);
    cmd.output().expect("Failed to run mdv")
}

// --- Tests ---

/// MDV-018: Task created via scaffolding mode should appear in `mdv task list`
/// without a manual reindex.
#[test]
fn scaffolding_mode_task_visible_in_task_list() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Create a project
    let project_path = vault.join("Projects/TST/TST.md");
    write(
        &project_path,
        "---\ntype: project\ntitle: Test Project\nproject-id: TST\ntask_counter: 0\n---\n",
    );

    // Action: Create a task via scaffolding mode (the MCP code path)
    let create_output = run_mdv(
        &cfg_path,
        &["new", "task", "Reindex Test Task", "--var", "project=TST", "--batch"],
    );
    assert!(
        create_output.status.success(),
        "Task creation failed: {}",
        String::from_utf8_lossy(&create_output.stderr)
    );

    // Verify: Task should appear in `mdv task list` immediately (no manual reindex)
    let list_output = run_mdv(&cfg_path, &["task", "list"]);
    assert!(
        list_output.status.success(),
        "task list failed: {}",
        String::from_utf8_lossy(&list_output.stderr)
    );

    let stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(
        stdout.contains("TST-001"),
        "Newly created task TST-001 should appear in task list without manual reindex.\nGot: {stdout}"
    );
    assert!(
        stdout.contains("Reindex Test Task"),
        "Task title should appear in task list.\nGot: {stdout}"
    );
}

/// Verify that multiple tasks created in sequence all appear in task list.
#[test]
fn scaffolding_mode_multiple_tasks_all_indexed() {
    let (_tmp, vault, cfg_path) = setup_vault();

    let project_path = vault.join("Projects/TST/TST.md");
    write(
        &project_path,
        "---\ntype: project\ntitle: Test Project\nproject-id: TST\ntask_counter: 0\n---\n",
    );

    // Create two tasks in sequence
    let out1 = run_mdv(
        &cfg_path,
        &["new", "task", "First Task", "--var", "project=TST", "--batch"],
    );
    assert!(out1.status.success());

    let out2 = run_mdv(
        &cfg_path,
        &["new", "task", "Second Task", "--var", "project=TST", "--batch"],
    );
    assert!(out2.status.success());

    // Both should appear in task list
    let list_output = run_mdv(&cfg_path, &["task", "list"]);
    let stdout = String::from_utf8(list_output.stdout).unwrap();

    assert!(stdout.contains("TST-001"), "First task should be in list.\nGot: {stdout}");
    assert!(stdout.contains("TST-002"), "Second task should be in list.\nGot: {stdout}");
}

/// Macro that creates a typed note should make it visible in index queries.
#[test]
fn macro_created_note_visible_in_task_list() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Template that creates a task-typed note.
    // First frontmatter block is template metadata (output path).
    // Second frontmatter block becomes the note's frontmatter in the rendered file.
    write(
        &vault.join(".mdvault/templates/task-note.md"),
        "---\noutput: \"tasks/{{title | slugify}}.md\"\n---\n---\ntype: task\ntitle: {{title}}\nstatus: todo\n---\n# {{title}}\n",
    );

    // Setup: Macro that uses the template
    write(
        &vault.join(".mdvault/macros/create-task.lua"),
        r#"
return {
    name = "create-task",
    description = "Create a task note",
    vars = { title = "Task title" },
    steps = {
        { template = "task-note" },
    },
}
"#,
    );

    // Action: Run the macro
    let macro_output = run_mdv(
        &cfg_path,
        &["macro", "create-task", "--var", "title=Macro Task", "--batch"],
    );
    assert!(
        macro_output.status.success(),
        "Macro failed: {}",
        String::from_utf8_lossy(&macro_output.stderr)
    );

    // Verify: Task should appear in task list without manual reindex
    let list_output = run_mdv(&cfg_path, &["task", "list"]);
    assert!(list_output.status.success());

    let stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(
        stdout.contains("Macro Task"),
        "Macro-created task should appear in task list without manual reindex.\nGot: {stdout}"
    );
}

/// Capture that creates a new file via create_if_missing should be indexed.
#[test]
fn capture_created_file_is_indexed() {
    let (_tmp, vault, cfg_path) = setup_vault();

    // Setup: Capture that targets a file with create_if_missing
    write(
        &vault.join(".mdvault/captures/log.lua"),
        r#"
return {
    name = "log",
    target = {
        file = "log.md",
        section = "Entries",
        position = "end",
        create_if_missing = true,
    },
    content = "- {{text}}",
}
"#,
    );

    // Action: Run capture (file doesn't exist yet, will be created)
    let capture_output =
        run_mdv(&cfg_path, &["capture", "log", "--var", "text=First entry"]);
    assert!(
        capture_output.status.success(),
        "Capture failed: {}",
        String::from_utf8_lossy(&capture_output.stderr)
    );

    // Verify: The created file exists and the index DB was updated
    assert!(vault.join("log.md").exists(), "Capture should create the target file");

    // Verify index DB exists (reindex_file was called)
    let index_path = vault.join(".mdvault/index.db");
    assert!(index_path.exists(), "Index DB should exist after capture with reindex");
}
