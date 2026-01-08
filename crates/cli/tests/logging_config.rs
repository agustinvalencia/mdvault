use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_logging_to_file() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let log_file = root.join("mdvault.log");

    // Create config with file logging
    let config_path = root.join("config.toml");
    let config_content = format!(
        r#"
version = 1
[profiles.default]
vault_root = "{}"
templates_dir = "templates"
captures_dir = "captures"
macros_dir = "macros"

[logging]
level = "debug"
file = "{}"
"#,
        root.display(),
        log_file.display()
    );
    fs::write(&config_path, &config_content).unwrap();

    // Create required directories
    fs::create_dir(root.join("templates")).unwrap();
    fs::create_dir(root.join("captures")).unwrap();
    fs::create_dir(root.join("macros")).unwrap();

    // Run a command that triggers logging (e.g. doctor)
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config").arg(&config_path).arg("doctor").assert().success();

    // Verify log file exists
    assert!(log_file.exists(), "Log file should be created");
}

#[test]
fn test_logging_level_parsing() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let config_path = root.join("config.toml");
    let config_content = format!(
        r#"
version = 1
[profiles.default]
vault_root = "{}"
templates_dir = "templates"
captures_dir = "captures"
macros_dir = "macros"

[logging]
level = "trace"
"#,
        root.display()
    );
    fs::write(&config_path, &config_content).unwrap();

    fs::create_dir(root.join("templates")).unwrap();
    fs::create_dir(root.join("captures")).unwrap();
    fs::create_dir(root.join("macros")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config").arg(&config_path).arg("doctor").assert().success();

    // If it didn't crash, the level parsing worked.
}

#[test]
fn test_logging_split_levels() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let log_file = root.join("split.log");

    let config_path = root.join("config.toml");
    let config_content = format!(
        r#"
version = 1
[profiles.default]
vault_root = "{}"
templates_dir = "templates"
captures_dir = "captures"
macros_dir = "macros"

[logging]
level = "info"
file_level = "debug"
file = "{}"
"#,
        root.display(),
        log_file.display()
    );
    fs::write(&config_path, &config_content).unwrap();

    fs::create_dir(root.join("templates")).unwrap();
    fs::create_dir(root.join("captures")).unwrap();
    fs::create_dir(root.join("macros")).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.arg("--config").arg(&config_path).arg("doctor").assert().success();

    assert!(log_file.exists());
}
