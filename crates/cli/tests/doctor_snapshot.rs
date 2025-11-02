use assert_cmd::prelude::*;
use predicates::prelude::*;
use regex::Regex;
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

fn normalize_paths(s: &str) -> String {
    let re = Regex::new(r#"(?m)^path: .*$"#).unwrap();
    re.replace(s, "path: <CFG>").to_string()
}

#[test]
fn doctor_snapshot_default_profile() {
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

    let assert = Command::new(assert_cmd::cargo::cargo_bin!("markadd"))
        .args(["doctor", "--config", cfg.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("OK   markadd doctor"));

    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let norm = normalize_paths(&out);

    insta::assert_snapshot!("doctor_default_profile", norm);
}
