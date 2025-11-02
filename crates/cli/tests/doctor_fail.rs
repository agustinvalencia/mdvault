use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn doctor_fails_when_config_missing() {
    let tmp = tempdir().unwrap();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.env("XDG_CONFIG_HOME", tmp.path()); // empty dir → no config
    cmd.arg("doctor");
    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("FAIL markadd doctor"))
        .stdout(predicate::str::contains("looked for:"));
}
