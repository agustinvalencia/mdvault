use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn write(dir: &std::path::Path, rel: &str, content: impl AsRef<str>) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content.as_ref()).unwrap();
}

fn make_config(vault_root: &str) -> String {
    format!(
        r#"
version = 1
profile = "test"

[profiles.test]
vault_root = "{vault_root}"
templates_dir = "{{{{vault_root}}}}/templates"
captures_dir = "{{{{vault_root}}}}/captures"
macros_dir = "{{{{vault_root}}}}/macros"
"#
    )
}

#[test]
fn capture_inserts_at_section_begin() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/inbox.yaml",
        r#"
name: inbox
description: Add to inbox

target:
  file: "notes.md"
  section: "Inbox"
  position: begin

content: "- {{text}}"
"#,
    );

    write(
        root,
        "vault/notes.md",
        r#"# My Notes

## Inbox

- Existing item

## Done

- Completed task
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("inbox")
        .arg("--var")
        .arg("text=New captured item");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OK   markadd capture"))
        .stdout(predicate::str::contains("capture: inbox"))
        .stdout(predicate::str::contains("section: Inbox"));

    let content = fs::read_to_string(root.join("vault/notes.md")).unwrap();
    assert!(content.contains("- New captured item"));
    assert!(content.contains("- Existing item"));

    let new_pos = content.find("New captured item").unwrap();
    let existing_pos = content.find("Existing item").unwrap();
    assert!(new_pos < existing_pos, "New item should appear before existing item");
}

#[test]
fn capture_inserts_at_section_end() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/todo.yaml",
        r#"
name: todo
description: Add to TODO

target:
  file: "tasks.md"
  section: "TODO"
  position: end

content: "- [ ] {{task}}"
"#,
    );

    write(
        root,
        "vault/tasks.md",
        r#"# Tasks

## TODO

- [ ] First task

## Done

- [x] Completed
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("todo")
        .arg("--var")
        .arg("task=New task");

    cmd.assert().success();

    let content = fs::read_to_string(root.join("vault/tasks.md")).unwrap();
    assert!(content.contains("- [ ] New task"));

    let first_pos = content.find("First task").unwrap();
    let new_pos = content.find("New task").unwrap();
    assert!(new_pos > first_pos, "New task should appear after first task");

    let done_pos = content.find("## Done").unwrap();
    assert!(new_pos < done_pos, "New task should appear before Done section");
}

#[test]
fn capture_fails_on_missing_section() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/inbox.yaml",
        r#"
name: inbox
target:
  file: "notes.md"
  section: "NonExistent"
  position: begin
content: "- {{text}}"
"#,
    );

    write(
        root,
        "vault/notes.md",
        r#"# Notes

## Existing Section

Content here
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("inbox")
        .arg("--var")
        .arg("text=Test");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Section not found"))
        .stderr(predicate::str::contains("Existing Section"));
}

#[test]
fn capture_fails_on_missing_file() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/inbox.yaml",
        r#"
name: inbox
target:
  file: "missing.md"
  section: "Inbox"
  position: begin
content: "- {{text}}"
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("inbox")
        .arg("--var")
        .arg("text=Test");

    cmd.assert().failure().stderr(predicate::str::contains("Failed to read target file"));
}

#[test]
fn capture_not_found_shows_available() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/inbox.yaml",
        r#"
name: inbox
target:
  file: "notes.md"
  section: "Inbox"
  position: begin
content: "test"
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config").arg(root.join("config.toml")).arg("capture").arg("nonexistent");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Capture not found: nonexistent"))
        .stderr(predicate::str::contains("inbox"));
}

#[test]
fn capture_list_shows_variables() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/inbox.yaml",
        r#"
name: inbox
target:
  file: "notes.md"
  section: "Inbox"
  position: begin
content: "- {{text}}"
"#,
    );

    write(
        root,
        "vault/captures/todo.yaml",
        r#"
name: todo
target:
  file: "tasks.md"
  section: "TODO"
  position: end
content: "- [ ] {{task}} ({{priority}})"
"#,
    );

    write(
        root,
        "vault/captures/simple.yaml",
        r#"
name: simple
target:
  file: "log.md"
  section: "Log"
  position: end
content: "Entry at {{date}}"
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config").arg(root.join("config.toml")).arg("capture").arg("--list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("inbox  [text]"))
        .stdout(predicate::str::contains("todo  [priority, task]"))
        .stdout(predicate::str::contains("simple\n")) // no variables
        .stdout(predicate::str::contains("-- 3 captures --"));
}
