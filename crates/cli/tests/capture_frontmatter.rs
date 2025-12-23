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
fn capture_sets_frontmatter_field() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    // Capture that sets a frontmatter field
    write(
        root,
        "vault/captures/mark-done.yaml",
        r#"
name: mark-done
description: Mark daily note as completed

target:
  file: "daily/today.md"

frontmatter:
  completed: true
  reviewed_at: "{{datetime}}"
"#,
    );

    // Target file with existing frontmatter
    write(
        root,
        "vault/daily/today.md",
        r#"---
title: Today's Note
completed: false
---

# Today

Some content here.
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config").arg(root.join("config.toml")).arg("capture").arg("mark-done");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OK   mdv capture"))
        .stdout(predicate::str::contains("frontmatter: modified"));

    let content = fs::read_to_string(root.join("vault/daily/today.md")).unwrap();
    assert!(content.contains("completed: true"), "completed should be true");
    assert!(content.contains("reviewed_at:"), "reviewed_at should be set");
    assert!(content.contains("title: Today's Note"), "title should be preserved");
}

#[test]
fn capture_toggles_frontmatter_boolean() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    // Capture that toggles a boolean field
    write(
        root,
        "vault/captures/toggle-flag.yaml",
        r#"
name: toggle-flag
description: Toggle a flag

target:
  file: "notes.md"

frontmatter:
  - field: active
    op: toggle
"#,
    );

    // Target file with a false flag
    write(
        root,
        "vault/notes.md",
        r#"---
title: Notes
active: false
---

# Notes

Content here.
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config").arg(root.join("config.toml")).arg("capture").arg("toggle-flag");

    cmd.assert().success();

    let content = fs::read_to_string(root.join("vault/notes.md")).unwrap();
    assert!(content.contains("active: true"), "active should be toggled to true");
}

#[test]
fn capture_increments_frontmatter_counter() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    // Capture that increments a counter
    write(
        root,
        "vault/captures/increment-views.yaml",
        r#"
name: increment-views
description: Increment view count

target:
  file: "article.md"

frontmatter:
  - field: views
    op: increment
"#,
    );

    // Target file with a counter
    write(
        root,
        "vault/article.md",
        r#"---
title: Article
views: 5
---

# Article

Content here.
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("increment-views");

    cmd.assert().success();

    let content = fs::read_to_string(root.join("vault/article.md")).unwrap();
    assert!(content.contains("views: 6"), "views should be incremented to 6");
}

#[test]
fn capture_appends_to_frontmatter_list() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    // Capture that appends to a list
    write(
        root,
        "vault/captures/add-tag.yaml",
        r#"
name: add-tag
description: Add a tag

target:
  file: "note.md"

frontmatter:
  - field: tags
    op: append
    value: "{{tag}}"
"#,
    );

    // Target file with existing tags
    write(
        root,
        "vault/note.md",
        r#"---
title: Note
tags:
  - existing
---

# Note

Content here.
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("add-tag")
        .arg("--var")
        .arg("tag=new-tag");

    cmd.assert().success();

    let content = fs::read_to_string(root.join("vault/note.md")).unwrap();
    assert!(content.contains("- existing"), "existing tag should be preserved");
    assert!(content.contains("- new-tag"), "new tag should be appended");
}

#[test]
fn capture_combines_content_and_frontmatter() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    // Capture that does both content insertion and frontmatter modification
    write(
        root,
        "vault/captures/add-task.yaml",
        r#"
name: add-task
description: Add a task and update frontmatter

target:
  file: "tasks.md"
  section: "TODO"
  position: end

content: "- [ ] {{task}}"

frontmatter:
  has_tasks: true
  last_updated: "{{date}}"
"#,
    );

    // Target file
    write(
        root,
        "vault/tasks.md",
        r#"---
title: Tasks
has_tasks: false
---

# Tasks

## TODO

- [ ] Existing task

## Done

- [x] Completed
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("add-task")
        .arg("--var")
        .arg("task=New important task");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("section: TODO"))
        .stdout(predicate::str::contains("frontmatter: modified"));

    let content = fs::read_to_string(root.join("vault/tasks.md")).unwrap();
    // Check frontmatter was modified
    assert!(content.contains("has_tasks: true"), "has_tasks should be true");
    assert!(content.contains("last_updated:"), "last_updated should be set");
    // Check content was inserted
    assert!(content.contains("- [ ] New important task"), "task should be added");
    assert!(content.contains("- [ ] Existing task"), "existing task should be preserved");
}

#[test]
fn capture_creates_frontmatter_if_missing() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    // Capture that sets frontmatter
    write(
        root,
        "vault/captures/add-metadata.yaml",
        r#"
name: add-metadata
description: Add metadata to file

target:
  file: "plain.md"

frontmatter:
  created: "{{date}}"
  status: draft
"#,
    );

    // Target file without frontmatter
    write(
        root,
        "vault/plain.md",
        r#"# Plain Document

This file has no frontmatter.
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config").arg(root.join("config.toml")).arg("capture").arg("add-metadata");

    cmd.assert().success();

    let content = fs::read_to_string(root.join("vault/plain.md")).unwrap();
    assert!(content.contains("---"), "frontmatter should be created");
    assert!(content.contains("created:"), "created field should exist");
    assert!(content.contains("status: draft"), "status should be draft");
    assert!(content.contains("# Plain Document"), "content should be preserved");
}
