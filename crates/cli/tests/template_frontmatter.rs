use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn write(path: &PathBuf, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn make_config(vault_root: &str, templates_dir: &str) -> String {
    format!(
        r#"
version = 1
profile = "test"

[profiles.test]
vault_root = "{vault_root}"
templates_dir = "{templates_dir}"
captures_dir = "{{{{vault_root}}}}/captures"
macros_dir = "{{{{vault_root}}}}/macros"
"#
    )
}

#[test]
fn template_with_frontmatter_output_creates_file_without_output_flag() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    let vault = root.join("vault");
    let templates = vault.join("templates");
    let config_path = root.join("config.toml");

    write(
        &config_path,
        &make_config(&vault.to_string_lossy(), &templates.to_string_lossy()),
    );

    // Template with frontmatter output path
    write(
        &templates.join("daily.md"),
        r#"---
output: daily/{{date}}.md
---
# Daily Note for {{date}}

## Inbox

## Tasks
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config").arg(&config_path).arg("new").arg("--template").arg("daily");
    // Note: no --output flag

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OK   mdv new"))
        .stdout(predicate::str::contains("template: daily"))
        .stdout(predicate::str::contains("daily/"));

    // Verify the file was created in the expected location
    let daily_dir = vault.join("daily");
    assert!(daily_dir.exists(), "daily directory should be created");

    let files: Vec<_> = fs::read_dir(&daily_dir).unwrap().collect();
    assert_eq!(files.len(), 1, "should have one file");

    let file_path = files[0].as_ref().unwrap().path();
    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("# Daily Note for"));
    assert!(content.contains("## Inbox"));
    assert!(content.contains("## Tasks"));
    // Frontmatter should be stripped
    assert!(!content.contains("output:"));
}

#[test]
fn template_without_frontmatter_requires_output_flag() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    let vault = root.join("vault");
    let templates = vault.join("templates");
    let config_path = root.join("config.toml");

    write(
        &config_path,
        &make_config(&vault.to_string_lossy(), &templates.to_string_lossy()),
    );

    // Template without frontmatter output path
    write(
        &templates.join("simple.md"),
        r#"# Simple Note

Content here.
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config").arg(&config_path).arg("new").arg("--template").arg("simple");
    // Note: no --output flag

    cmd.assert().failure().stderr(predicate::str::contains("--output is required"));
}

#[test]
fn template_extra_frontmatter_is_rendered_in_output() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    let vault = root.join("vault");
    let templates = vault.join("templates");
    let config_path = root.join("config.toml");

    write(
        &config_path,
        &make_config(&vault.to_string_lossy(), &templates.to_string_lossy()),
    );

    // Template with output path AND extra frontmatter fields
    write(
        &templates.join("daily.md"),
        r#"---
output: daily/{{date}}.md
created: "{{date}}"
tags:
  - daily
  - journal
---
# Daily Note for {{date}}

## Inbox
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config").arg(&config_path).arg("new").arg("--template").arg("daily");

    cmd.assert().success();

    // Read the created file
    let daily_dir = vault.join("daily");
    let files: Vec<_> = fs::read_dir(&daily_dir).unwrap().collect();
    let file_path = files[0].as_ref().unwrap().path();
    let content = fs::read_to_string(&file_path).unwrap();

    // Extra frontmatter fields should be in output
    assert!(content.contains("---"), "should have frontmatter delimiters");
    assert!(content.contains("created:"), "should have created field");
    assert!(content.contains("tags:"), "should have tags field");
    assert!(content.contains("- daily"), "should have daily tag");
    assert!(content.contains("- journal"), "should have journal tag");

    // The created field should have the date substituted (not {{date}})
    assert!(!content.contains("{{date}}"), "date placeholder should be substituted");

    // The output field should NOT be in the rendered output
    assert!(
        !content.contains("output:"),
        "output field should not be in rendered output"
    );
}

#[test]
fn template_with_user_vars_in_frontmatter() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    let vault = root.join("vault");
    let templates = vault.join("templates");
    let config_path = root.join("config.toml");

    write(
        &config_path,
        &make_config(&vault.to_string_lossy(), &templates.to_string_lossy()),
    );

    // Template with user-defined variable in frontmatter
    write(
        &templates.join("meeting.md"),
        r#"---
output: meetings/{{date}}.md
title: "{{meeting_title}}"
attendees: "{{attendees}}"
vars:
  meeting_title: "Meeting title"
  attendees: "Attendees"
---
# {{meeting_title}}

Attendees: {{attendees}}
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(&config_path)
        .arg("new")
        .arg("--template")
        .arg("meeting")
        .arg("--var")
        .arg("meeting_title=Quarterly Review")
        .arg("--var")
        .arg("attendees=Alice, Bob");

    cmd.assert().success();

    // Read the created file
    let meetings_dir = vault.join("meetings");
    let files: Vec<_> = fs::read_dir(&meetings_dir).unwrap().collect();
    let file_path = files[0].as_ref().unwrap().path();
    let content = fs::read_to_string(&file_path).unwrap();

    // User variables should be substituted in frontmatter
    assert!(
        content.contains("title: Quarterly Review"),
        "title should have user value, got: {}",
        content
    );
    assert!(
        content.contains("attendees: Alice, Bob"),
        "attendees should have user value"
    );

    // User variables should also be substituted in body
    assert!(
        content.contains("# Quarterly Review"),
        "title in body should be substituted"
    );
    assert!(
        content.contains("Attendees: Alice, Bob"),
        "attendees in body should be substituted"
    );

    // The vars field should NOT be in the output (it's metadata)
    assert!(!content.contains("vars:"), "vars metadata should not be in output");
}

#[test]
fn output_flag_overrides_frontmatter_output() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    let vault = root.join("vault");
    let templates = vault.join("templates");
    let config_path = root.join("config.toml");

    write(
        &config_path,
        &make_config(&vault.to_string_lossy(), &templates.to_string_lossy()),
    );

    // Template with frontmatter output path
    write(
        &templates.join("daily.md"),
        r#"---
output: daily/{{date}}.md
---
# Daily Note

Content
"#,
    );

    let custom_output = vault.join("custom").join("my-note.md");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config")
        .arg(&config_path)
        .arg("new")
        .arg("--template")
        .arg("daily")
        .arg("--output")
        .arg(&custom_output);

    cmd.assert().success().stdout(predicate::str::contains("my-note.md"));

    assert!(custom_output.exists(), "custom output should be created");

    // The frontmatter output path should not be used
    assert!(
        !vault.join("daily").exists(),
        "frontmatter output dir should not be created"
    );
}
