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

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config").arg(&config_path).arg("new").arg("--template").arg("daily");
    // Note: no --output flag

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OK   markadd new"))
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

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.arg("--config").arg(&config_path).arg("new").arg("--template").arg("simple");
    // Note: no --output flag

    cmd.assert().failure().stderr(predicate::str::contains("--output is required"));
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

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
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
