//! Integration tests for variable metadata (prompts, defaults, descriptions).

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
fn template_with_vars_metadata_uses_default() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/templates/meeting.md",
        r#"---
output: "meetings/{{title}}.md"
vars:
  title:
    prompt: "Meeting title"
    default: "Untitled Meeting"
  attendees:
    prompt: "Who's attending?"
    default: "TBD"
---
# {{title}}

## Attendees

{{attendees}}
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    // Run in batch mode - should use defaults
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("new")
        .arg("--template")
        .arg("meeting")
        .arg("--batch");

    cmd.assert().success();

    let output_file = vault.join("meetings/Untitled Meeting.md");
    assert!(output_file.exists(), "File with default title should exist");

    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("# Untitled Meeting"));
    assert!(content.contains("TBD"));
}

#[test]
fn template_vars_can_be_overridden() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/templates/note.md",
        r#"---
output: "notes/{{title}}.md"
vars:
  title:
    prompt: "Note title"
    default: "Untitled"
  author:
    prompt: "Author name"
    default: "Anonymous"
---
# {{title}}

By: {{author}}
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("new")
        .arg("--template")
        .arg("note")
        .arg("--var")
        .arg("title=Custom Title")
        .arg("--batch"); // Use default for author

    cmd.assert().success();

    let output_file = vault.join("notes/Custom Title.md");
    assert!(output_file.exists());

    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("# Custom Title"));
    assert!(content.contains("By: Anonymous")); // Default for author
}

#[test]
fn capture_with_vars_metadata_uses_default() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/notes.md",
        r#"# Notes

## Inbox

"#,
    );

    write(
        root,
        "vault/captures/inbox.yaml",
        r#"
name: inbox
description: Add to inbox with priority

vars:
  text:
    prompt: "What to add?"
    required: true
  priority:
    prompt: "Priority level"
    default: "normal"
    description: "Can be low, normal, or high"

target:
  file: "notes.md"
  section: Inbox
  position: end

content: "- [{{priority}}] {{text}}"
"#,
    );

    fs::create_dir_all(vault.join("templates")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("inbox")
        .arg("--var")
        .arg("text=Review PR")
        .arg("--batch"); // Uses default priority

    cmd.assert().success();

    let content = fs::read_to_string(vault.join("notes.md")).unwrap();
    assert!(content.contains("- [normal] Review PR"));
}

#[test]
fn batch_mode_fails_on_missing_required_var() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/templates/required.md",
        r#"---
output: "notes/{{title}}.md"
vars:
  title:
    prompt: "Title is required"
    required: true
---
# {{title}}
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("new")
        .arg("--template")
        .arg("required")
        .arg("--batch"); // No --var provided

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("missing required variable"))
        .stderr(predicate::str::contains("title"));
}

#[test]
fn macro_with_vars_metadata() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/templates/project.md",
        r#"---
output: "projects/{{name}}.md"
---
# Project: {{name}}

Status: {{status}}
"#,
    );

    write(
        root,
        "vault/macros/new-project.yaml",
        r#"
name: new-project
description: Create a new project
vars:
  name:
    prompt: "Project name"
    required: true
  status:
    prompt: "Initial status"
    default: "planning"
    description: "planning, active, or completed"
steps:
  - template: project
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("macro")
        .arg("new-project")
        .arg("--var")
        .arg("name=Website Redesign")
        .arg("--batch"); // Uses default status

    cmd.assert().success();

    let output_file = vault.join("projects/Website Redesign.md");
    assert!(output_file.exists());

    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("# Project: Website Redesign"));
    assert!(content.contains("Status: planning"));
}

#[test]
fn simple_var_spec_as_prompt() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    // Simple form: just the prompt string
    write(
        root,
        "vault/captures/quick.yaml",
        r#"
name: quick
vars:
  text: "Quick note text"
target:
  file: "notes.md"
  section: Quick
  position: end
content: "- {{text}}"
"#,
    );

    write(
        root,
        "vault/notes.md",
        r#"# Notes

## Quick

"#,
    );

    fs::create_dir_all(vault.join("templates")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("quick")
        .arg("--var")
        .arg("text=Simple note");

    cmd.assert().success();

    let content = fs::read_to_string(vault.join("notes.md")).unwrap();
    assert!(content.contains("- Simple note"));
}

#[test]
fn vars_with_date_math_default() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/templates/deadline.md",
        r#"---
output: "tasks/{{task}}.md"
vars:
  task:
    prompt: "Task name"
  due:
    prompt: "Due date"
    default: "{{today + 7d}}"
---
# {{task}}

Due: {{due}}
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("new")
        .arg("--template")
        .arg("deadline")
        .arg("--var")
        .arg("task=Review Code")
        .arg("--batch");

    cmd.assert().success();

    let output_file = vault.join("tasks/Review Code.md");
    assert!(output_file.exists());

    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("# Review Code"));
    // The default should have been evaluated as date math
    assert!(content.contains("Due: 20")); // Should start with year
    assert!(!content.contains("{{today + 7d}}")); // Should not contain raw expression
}
