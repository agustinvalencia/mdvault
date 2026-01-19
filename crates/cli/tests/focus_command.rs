//! Integration tests for the `mdv focus` command.

use std::fs;
use std::io::Write;
use std::process::Command;
use tempfile::tempdir;

fn mdv_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mdv"))
}

fn create_test_config(vault_path: &std::path::Path, config_path: &std::path::Path) {
    let config_content = format!(
        r#"
version = 1
profile = "test"

[profiles.test]
vault_root = "{}"
templates_dir = "{}/templates"
captures_dir = "{}/captures"
macros_dir = "{}/macros"
"#,
        vault_path.display(),
        vault_path.display(),
        vault_path.display(),
        vault_path.display()
    );

    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    let mut file = fs::File::create(config_path).unwrap();
    file.write_all(config_content.as_bytes()).unwrap();
}

#[test]
fn test_focus_shows_no_active_focus() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    let output = mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No active focus"));
}

#[test]
fn test_focus_set_project() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    // Set focus
    let output = mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus", "MDV"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Focus set to: MDV"));

    // Verify focus is set
    let output = mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Active focus: MDV"));
}

#[test]
fn test_focus_set_with_note() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    // Set focus with note
    let output = mdv_cmd()
        .args([
            "--config",
            config.to_str().unwrap(),
            "focus",
            "PROJ",
            "--note",
            "Working on OAuth",
        ])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Focus set to: PROJ"));
    assert!(stdout.contains("Note: Working on OAuth"));

    // Verify
    let output = mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Active focus: PROJ"));
    assert!(stdout.contains("Note: Working on OAuth"));
}

#[test]
fn test_focus_clear() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    // Set focus first
    mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus", "TEST"])
        .output()
        .expect("Failed to execute command");

    // Clear focus
    let output = mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus", "--clear"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Focus cleared"));

    // Verify cleared
    let output = mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No active focus"));
}

#[test]
fn test_focus_json_output() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    // Set focus
    mdv_cmd()
        .args([
            "--config",
            config.to_str().unwrap(),
            "focus",
            "JSON",
            "--note",
            "Test",
        ])
        .output()
        .expect("Failed to execute command");

    // Get JSON output
    let output = mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus", "--json"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse as JSON
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON output");

    assert_eq!(json["focus"]["project"], "JSON");
    assert_eq!(json["focus"]["note"], "Test");
}

#[test]
fn test_focus_json_empty() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    // Get JSON output with no focus
    let output = mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus", "--json"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse as JSON
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Invalid JSON output");

    assert!(json["focus"].is_null());
}

#[test]
fn test_focus_state_file_created() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    // Set focus
    mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus", "FILE"])
        .output()
        .expect("Failed to execute command");

    // Verify state file exists
    let state_file = vault.join(".mdvault/state/context.toml");
    assert!(state_file.exists());

    let content = fs::read_to_string(&state_file).unwrap();
    assert!(content.contains("project = \"FILE\""));
}

#[test]
fn test_focus_replace_existing() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    let config = tmp.path().join("config.toml");

    fs::create_dir_all(&vault).unwrap();
    create_test_config(&vault, &config);

    // Set first focus
    mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus", "FIRST"])
        .output()
        .expect("Failed to execute command");

    // Set second focus (replaces first)
    mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus", "SECOND"])
        .output()
        .expect("Failed to execute command");

    // Verify only SECOND is active
    let output = mdv_cmd()
        .args(["--config", config.to_str().unwrap(), "focus"])
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Active focus: SECOND"));
    assert!(!stdout.contains("FIRST"));
}
