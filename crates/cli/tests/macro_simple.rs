//! Integration tests for macro command.

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

[security]
allow_shell = false
"#
    )
}

#[test]
fn macro_list_shows_available_macros() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/macros/weekly-review.yaml",
        r#"
name: weekly-review
description: Set up weekly review documents
steps:
  - template: summary
"#,
    );

    write(
        root,
        "vault/macros/daily-setup.yaml",
        r#"
name: daily-setup
description: Create daily note
steps:
  - template: daily
  - capture: inbox-clear
"#,
    );

    // Create templates/captures directories
    fs::create_dir_all(vault.join("templates")).unwrap();
    fs::create_dir_all(vault.join("captures")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config").arg(root.join("config.toml")).arg("macro").arg("--list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("weekly-review"))
        .stdout(predicate::str::contains("Set up weekly review"))
        .stdout(predicate::str::contains("daily-setup"))
        .stdout(predicate::str::contains("2 steps"))
        .stdout(predicate::str::contains("-- 2 macros --"));
}

#[test]
fn macro_not_found_shows_available() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/macros/existing.yaml",
        r#"
name: existing
steps:
  - template: test
"#,
    );

    fs::create_dir_all(vault.join("templates")).unwrap();
    fs::create_dir_all(vault.join("captures")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config").arg(root.join("config.toml")).arg("macro").arg("nonexistent");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Macro not found: nonexistent"))
        .stderr(predicate::str::contains("existing"));
}

#[test]
fn macro_with_shell_requires_trust() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/macros/deploy.yaml",
        r#"
name: deploy
description: Deploy with git
steps:
  - shell: "git add ."
    description: Stage changes
  - shell: "git commit -m 'deploy'"
    description: Commit
"#,
    );

    fs::create_dir_all(vault.join("templates")).unwrap();
    fs::create_dir_all(vault.join("captures")).unwrap();

    // Without --trust, should fail
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config").arg(root.join("config.toml")).arg("macro").arg("deploy");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--trust"))
        .stderr(predicate::str::contains("git add"));
}

#[test]
fn macro_list_shows_trust_required() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/macros/safe.yaml",
        r#"
name: safe
steps:
  - template: note
"#,
    );

    write(
        root,
        "vault/macros/dangerous.yaml",
        r#"
name: dangerous
steps:
  - template: note
  - shell: "echo hello"
"#,
    );

    fs::create_dir_all(vault.join("templates")).unwrap();
    fs::create_dir_all(vault.join("captures")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config").arg(root.join("config.toml")).arg("macro").arg("--list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("safe  (1 steps)"))
        .stdout(predicate::str::contains("dangerous  (2 steps) [requires --trust]"));
}

#[test]
fn macro_runs_template_step() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/templates/note.md",
        r#"---
output: "notes/{{title}}.md"
---
# {{title}}

Created on {{date}}
"#,
    );

    write(
        root,
        "vault/macros/create-note.yaml",
        r#"
name: create-note
description: Create a note
vars:
  title: "Note title"
steps:
  - template: note
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("macro")
        .arg("create-note")
        .arg("--var")
        .arg("title=My Test Note")
        .arg("--batch");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OK   markadd macro"))
        .stdout(predicate::str::contains("1 completed"));

    // Verify file was created
    let output_file = vault.join("notes/My Test Note.md");
    assert!(output_file.exists(), "Output file should be created");

    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("# My Test Note"));
}

#[test]
fn macro_runs_capture_step() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/inbox.md",
        r#"# Inbox

## Items

- Existing item

## Archive
"#,
    );

    write(
        root,
        "vault/captures/add-item.yaml",
        r#"
name: add-item
target:
  file: "inbox.md"
  section: Items
  position: end
content: "- {{text}}"
"#,
    );

    write(
        root,
        "vault/macros/quick-add.yaml",
        r#"
name: quick-add
description: Add item to inbox
vars:
  text: "Item text"
steps:
  - capture: add-item
"#,
    );

    fs::create_dir_all(vault.join("templates")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("macro")
        .arg("quick-add")
        .arg("--var")
        .arg("text=New macro item")
        .arg("--batch");

    cmd.assert().success().stdout(predicate::str::contains("OK   markadd macro"));

    let content = fs::read_to_string(vault.join("inbox.md")).unwrap();
    assert!(content.contains("- New macro item"));
    assert!(content.contains("- Existing item"));
}

#[test]
fn macro_runs_multiple_steps() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/templates/meeting.md",
        r#"---
output: "meetings/{{topic}}.md"
---
# Meeting: {{topic}}

## Attendees

## Notes

## Actions
"#,
    );

    write(
        root,
        "vault/log.md",
        r#"# Meeting Log

## Recent

"#,
    );

    write(
        root,
        "vault/captures/log-meeting.yaml",
        r#"
name: log-meeting
target:
  file: "log.md"
  section: Recent
  position: begin
content: "- [[meetings/{{topic}}]] - {{date}}"
"#,
    );

    write(
        root,
        "vault/macros/new-meeting.yaml",
        r#"
name: new-meeting
description: Create meeting note and log it
vars:
  topic: "Meeting topic"
steps:
  - template: meeting
  - capture: log-meeting
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("macro")
        .arg("new-meeting")
        .arg("--var")
        .arg("topic=Weekly Sync")
        .arg("--batch");

    cmd.assert().success().stdout(predicate::str::contains("2 completed"));

    // Check meeting file was created
    let meeting_file = vault.join("meetings/Weekly Sync.md");
    assert!(meeting_file.exists());
    let meeting_content = fs::read_to_string(&meeting_file).unwrap();
    assert!(meeting_content.contains("# Meeting: Weekly Sync"));

    // Check log was updated
    let log_content = fs::read_to_string(vault.join("log.md")).unwrap();
    assert!(log_content.contains("[[meetings/Weekly Sync]]"));
}

#[test]
fn macro_with_step_vars_override() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/templates/note.md",
        r#"---
output: "notes/{{filename}}.md"
---
# {{title}}
"#,
    );

    write(
        root,
        "vault/macros/fixed-output.yaml",
        r#"
name: fixed-output
description: Create note with fixed filename
vars:
  title: "Note title"
steps:
  - template: note
    with:
      filename: "fixed-name"
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("macro")
        .arg("fixed-output")
        .arg("--var")
        .arg("title=Dynamic Title")
        .arg("--batch");

    cmd.assert().success();

    // Should use fixed filename from step vars
    let output_file = vault.join("notes/fixed-name.md");
    assert!(output_file.exists());

    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("# Dynamic Title"));
}
