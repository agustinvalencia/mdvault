//! Integration tests for variable metadata (prompts, defaults, descriptions).
//!
//! Note: Template `vars:` DSL was removed in v0.2.0 in favor of Lua-based schemas.
//! Templates now use `lua:` frontmatter to reference Lua scripts that define schemas.
//! These tests cover captures and macros which still support `vars:` in Lua format.

use assert_cmd::prelude::*;
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
        "vault/captures/inbox.lua",
        r#"
return {
    name = "inbox",
    description = "Add to inbox with priority",
    vars = {
        text = {
            prompt = "What to add?",
            required = true,
        },
        priority = {
            prompt = "Priority level",
            default = "normal",
            description = "Can be low, normal, or high",
        },
    },
    target = {
        file = "notes.md",
        section = "Inbox",
        position = "end",
    },
    content = "- [{{priority}}] {{text}}",
}
"#,
    );

    fs::create_dir_all(vault.join("templates")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
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
        "vault/macros/new-project.lua",
        r#"
return {
    name = "new-project",
    description = "Create a new project",
    vars = {
        name = {
            prompt = "Project name",
            required = true,
        },
        status = {
            prompt = "Initial status",
            default = "planning",
            description = "planning, active, or completed",
        },
    },
    steps = {
        { template = "project" },
    },
}
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
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
        "vault/captures/quick.lua",
        r#"
return {
    name = "quick",
    vars = {
        text = "Quick note text",
    },
    target = {
        file = "notes.md",
        section = "Quick",
        position = "end",
    },
    content = "- {{text}}",
}
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

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
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
