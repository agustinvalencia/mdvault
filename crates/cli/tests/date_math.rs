//! Integration tests for date math expressions in templates.

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
fn template_with_date_math_today() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/templates/dated.md",
        r#"# Note for {{today}}

Tomorrow: {{today + 1d}}
Yesterday: {{today - 1d}}
Next week: {{today + 1w}}
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    let output = vault.join("output.md");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("new")
        .arg("--template")
        .arg("dated")
        .arg("--output")
        .arg(&output);

    cmd.assert().success();

    let content = fs::read_to_string(&output).unwrap();

    // Check that date math expressions are replaced with actual dates
    // (we can't check exact values since they depend on current date)
    assert!(!content.contains("{{today}}"), "today should be replaced");
    assert!(!content.contains("{{today + 1d}}"), "today + 1d should be replaced");
    assert!(!content.contains("{{today - 1d}}"), "today - 1d should be replaced");
    assert!(!content.contains("{{today + 1w}}"), "today + 1w should be replaced");

    // Check format is YYYY-MM-DD
    let lines: Vec<&str> = content.lines().collect();
    assert!(lines[0].starts_with("# Note for 20"), "Date should be in YYYY format");
}

#[test]
fn template_with_date_format() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/templates/formatted.md",
        r#"Day name: {{today | %A}}
Month: {{today | %B}}
Year: {{today | %Y}}
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    let output = vault.join("output.md");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("new")
        .arg("--template")
        .arg("formatted")
        .arg("--output")
        .arg(&output);

    cmd.assert().success();

    let content = fs::read_to_string(&output).unwrap();

    // Format expressions should be replaced
    assert!(!content.contains("{{today | %A}}"), "format should be replaced");
    assert!(!content.contains("{{today | %B}}"), "format should be replaced");

    // Should contain day/month names (not numeric)
    assert!(content.contains("Day name: "));
    assert!(content.contains("Month: "));
    assert!(content.contains("Year: 20"));
}

#[test]
fn capture_with_date_math() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/notes.md",
        r#"# Notes

## Log

"#,
    );

    write(
        root,
        "vault/captures/log.yaml",
        r#"
name: log
target:
  file: "notes.md"
  section: Log
  position: begin
content: "- [{{today}}] {{text}} (due: {{today + 7d}})"
"#,
    );

    fs::create_dir_all(vault.join("templates")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("capture")
        .arg("log")
        .arg("--var")
        .arg("text=Test entry");

    cmd.assert().success();

    let content = fs::read_to_string(vault.join("notes.md")).unwrap();

    // Date expressions should be replaced
    assert!(!content.contains("{{today}}"), "today should be replaced");
    assert!(!content.contains("{{today + 7d}}"), "date math should be replaced");
    assert!(content.contains("Test entry"));
    assert!(content.contains("due:"));
}

#[test]
fn template_with_weekday_math() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    write(
        root,
        "vault/templates/weekly.md",
        r#"# Weekly Planning

Next Monday: {{today + monday}}
Last Friday: {{today - friday}}
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    let output = vault.join("output.md");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("new")
        .arg("--template")
        .arg("weekly")
        .arg("--output")
        .arg(&output);

    cmd.assert().success();

    let content = fs::read_to_string(&output).unwrap();

    // Weekday expressions should be replaced
    assert!(!content.contains("{{today + monday}}"), "weekday should be replaced");
    assert!(!content.contains("{{today - friday}}"), "weekday should be replaced");
}

#[test]
fn template_output_path_with_date_math() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let vault = root.join("vault");

    write(root, "config.toml", make_config(&vault.to_string_lossy()));

    // Template with date math in output path
    write(
        root,
        "vault/templates/daily.md",
        r#"---
output: "daily/{{today}}.md"
---
# Daily Note

Date: {{today}}
"#,
    );

    fs::create_dir_all(vault.join("captures")).unwrap();
    fs::create_dir_all(vault.join("macros")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(root.join("config.toml"))
        .arg("new")
        .arg("--template")
        .arg("daily");

    cmd.assert().success();

    // Find the created file in daily/ directory
    let daily_dir = vault.join("daily");
    assert!(daily_dir.exists(), "daily directory should be created");

    let entries: Vec<_> = fs::read_dir(&daily_dir).unwrap().collect();
    assert_eq!(entries.len(), 1, "Should have one daily note");

    let file_path = entries[0].as_ref().unwrap().path();
    let filename = file_path.file_name().unwrap().to_string_lossy();
    assert!(filename.ends_with(".md"));
    assert!(filename.starts_with("20"), "Filename should start with year");
}
