//! Integration tests for Lua-based captures.
//!
//! These tests verify that Lua captures work identically to YAML captures
//! and test Lua-specific features.

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
fn lua_capture_inserts_at_section_begin() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/inbox.lua",
        r#"
return {
    name = "inbox",
    description = "Add to inbox",

    vars = {
        text = "What to capture?",
    },

    target = {
        file = "notes.md",
        section = "Inbox",
        position = "begin",
    },

    content = "- {{text}}",
}
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

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("inbox")
        .arg("--var")
        .arg("text=New captured item");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OK   mdv capture"))
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
fn lua_capture_inserts_at_section_end() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/todo.lua",
        r#"
return {
    name = "todo",
    description = "Add to TODO",

    vars = {
        task = "Task description?",
    },

    target = {
        file = "tasks.md",
        section = "TODO",
        position = "end",
    },

    content = "- [ ] {{task}}",
}
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

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
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
fn lua_capture_with_full_vars_metadata() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/detailed.lua",
        r#"
return {
    name = "detailed",

    vars = {
        title = {
            prompt = "Entry title?",
            required = true,
        },
        priority = {
            prompt = "Priority level?",
            default = "medium",
        },
    },

    target = {
        file = "log.md",
        section = "Entries",
        position = "end",
    },

    content = "- [{{priority}}] {{title}}",
}
"#,
    );

    write(
        root,
        "vault/log.md",
        r#"# Log

## Entries

- Existing entry
"#,
    );

    // Test with explicit vars
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("detailed")
        .arg("--var")
        .arg("title=Important task")
        .arg("--var")
        .arg("priority=high");

    cmd.assert().success();

    let content = fs::read_to_string(root.join("vault/log.md")).unwrap();
    assert!(content.contains("- [high] Important task"));
}

#[test]
fn lua_capture_with_frontmatter_set() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/status.lua",
        r#"
return {
    name = "status",

    target = {
        file = "project.md",
    },

    frontmatter = {
        status = "active",
        updated = "{{date}}",
    },
}
"#,
    );

    write(
        root,
        "vault/project.md",
        r#"---
title: My Project
status: draft
---

# My Project

Content here
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("status");

    cmd.assert().success();

    let content = fs::read_to_string(root.join("vault/project.md")).unwrap();
    assert!(content.contains("status: active"));
}

#[test]
fn lua_capture_with_frontmatter_operations() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/increment.lua",
        r#"
return {
    name = "increment",

    target = {
        file = "counter.md",
    },

    frontmatter = {
        { field = "count", op = "increment" },
    },
}
"#,
    );

    write(
        root,
        "vault/counter.md",
        r#"---
title: Counter
count: 5
---

# Counter

Content
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("increment");

    cmd.assert().success();

    let content = fs::read_to_string(root.join("vault/counter.md")).unwrap();
    assert!(content.contains("count: 6"));
}

#[test]
fn lua_capture_precedence_over_yaml() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    // Create both YAML and Lua with same name - Lua should win
    write(
        root,
        "vault/captures/test.yaml",
        r#"
name: test
target:
  file: "notes.md"
  section: "YAML"
  position: begin
content: "YAML content"
"#,
    );

    write(
        root,
        "vault/captures/test.lua",
        r#"
return {
    name = "test",
    target = {
        file = "notes.md",
        section = "Lua",
        position = "begin",
    },
    content = "Lua content",
}
"#,
    );

    write(
        root,
        "vault/notes.md",
        r#"# Notes

## YAML

YAML section

## Lua

Lua section
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("test");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("section: Lua")); // Lua wins

    let content = fs::read_to_string(root.join("vault/notes.md")).unwrap();
    assert!(content.contains("Lua content"));
    assert!(!content.contains("YAML content"));
}

#[test]
fn lua_capture_list_shows_captures() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/inbox.lua",
        r#"
return {
    name = "inbox",
    vars = {
        text = "What?",
    },
    target = {
        file = "notes.md",
        section = "Inbox",
        position = "begin",
    },
    content = "- {{text}}",
}
"#,
    );

    write(
        root,
        "vault/captures/todo.lua",
        r#"
return {
    name = "todo",
    vars = {
        task = "Task?",
        priority = "Priority?",
    },
    target = {
        file = "tasks.md",
        section = "TODO",
        position = "end",
    },
    content = "- [ ] {{task}} ({{priority}})",
}
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config").arg(root.join("config.toml")).arg("capture").arg("--list");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("inbox"))
        .stdout(predicate::str::contains("todo"))
        .stdout(predicate::str::contains("-- 2 captures --"));
}

#[test]
fn lua_capture_with_create_if_missing() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/captures/daily.lua",
        r#"
return {
    name = "daily",

    vars = {
        note = "What to log?",
    },

    target = {
        file = "daily/{{date}}.md",
        section = "Log",
        position = "end",
        create_if_missing = true,
    },

    content = "- {{note}}",
}
"#,
    );

    // Note: daily file doesn't exist yet

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("daily")
        .arg("--var")
        .arg("note=First entry");

    cmd.assert().success();

    // Verify file was created with the section
    let daily_dir = vault.join("daily");
    assert!(daily_dir.exists(), "daily directory should be created");

    let files: Vec<_> = fs::read_dir(&daily_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(files.len(), 1, "should create one daily file");

    let content = fs::read_to_string(files[0].path()).unwrap();
    assert!(content.contains("## Log"));
    assert!(content.contains("- First entry"));
}
