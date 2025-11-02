use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn write_file(path: &PathBuf, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn doctor_reads_provided_config_path() {
    let tmp = tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    let toml = r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "/tmp/v"
templates_dir = "{{vault_root}}/.markadd/templates"
captures_dir  = "{{vault_root}}/.markadd/captures"
macros_dir    = "{{vault_root}}/.markadd/macros"

[security]
allow_shell = false
allow_http  = false
"#;
    write_file(&cfg, toml);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.args(["doctor", "--config", cfg.to_str().unwrap()]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OK   markadd doctor"))
        .stdout(predicate::str::contains("profile: default"))
        .stdout(predicate::str::contains("vault_root: /tmp/v"));
}

#[test]
fn doctor_uses_xdg_default_when_present() {
    let tmp = tempdir().unwrap();
    let cfg_dir = tmp.path().join("markadd");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();
    write_file(
        &cfg_path,
        r#"
version = 1
profile = "default"
[profiles.default]
vault_root = "/tmp/v"
templates_dir = "{{vault_root}}/t"
captures_dir  = "{{vault_root}}/c"
macros_dir    = "{{vault_root}}/m"
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.env("XDG_CONFIG_HOME", tmp.path());
    cmd.arg("doctor");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("OK   markadd doctor"))
        .stdout(predicate::str::contains("vault_root: /tmp/v"));
}
