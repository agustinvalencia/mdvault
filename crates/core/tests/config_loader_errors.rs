use markadd_core::config::loader::{ConfigError, ConfigLoader};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn write_file(path: &PathBuf, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn missing_file_fails() {
    let tmp = tempdir().unwrap();
    let cfg_path = tmp.path().join("nope/config.toml");
    let err = ConfigLoader::load(Some(&cfg_path), None).unwrap_err();
    match err {
        ConfigError::NotFound(_) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn bad_version_fails() {
    let tmp = tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");
    write_file(&cfg_path, "version = 2\nprofiles = {}\n");

    let err = ConfigLoader::load(Some(&cfg_path), None).unwrap_err();
    match err {
        ConfigError::BadVersion(2) => {}
        other => panic!("expected BadVersion(2), got {other:?}"),
    }
}

#[test]
fn no_profiles_fails() {
    let tmp = tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");
    write_file(&cfg_path, "version = 1\nprofiles = {}\n");

    let err = ConfigLoader::load(Some(&cfg_path), None).unwrap_err();
    match err {
        ConfigError::NoProfiles => {}
        other => panic!("expected NoProfiles, got {other:?}"),
    }
}

#[test]
fn profile_not_found_fails() {
    let tmp = tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");
    let toml = r#"
version = 1
profile = "default"
[profiles.default]
vault_root = "/tmp/vault"
templates_dir = "{{vault_root}}/tpl"
captures_dir  = "{{vault_root}}/cap"
macros_dir    = "{{vault_root}}/mac"
"#;
    write_file(&cfg_path, toml);

    let err = ConfigLoader::load(Some(&cfg_path), Some("missing")).unwrap_err();
    match err {
        ConfigError::ProfileNotFound(p) if p == "missing" => {}
        other => panic!("expected ProfileNotFound(\"missing\"), got {other:?}"),
    }
}
