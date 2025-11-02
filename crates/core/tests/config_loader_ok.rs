use markadd_core::config::loader::ConfigLoader;
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
fn load_default_profile_ok() {
    let tmp = tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");
    let toml = r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "/tmp/vault"
templates_dir = "{{vault_root}}/.markadd/templates"
captures_dir  = "{{vault_root}}/.markadd/captures"
macros_dir    = "{{vault_root}}/.markadd/macros"

[security]
allow_shell = false
allow_http  = true
"#;

    write_file(&cfg_path, toml);

    let rc = ConfigLoader::load(Some(&cfg_path), None).expect("should load");
    assert_eq!(rc.active_profile, "default");
    assert_eq!(rc.vault_root.display().to_string(), "/tmp/vault");
    assert!(rc.templates_dir.ends_with(".markadd/templates"));
    assert!(rc.captures_dir.ends_with(".markadd/captures"));
    assert!(rc.macros_dir.ends_with(".markadd/macros"));
    assert!(!rc.security.allow_shell);
    assert!(rc.security.allow_http);
}

#[test]
fn load_with_profile_override_ok() {
    let tmp = tempdir().unwrap();
    let cfg_path = tmp.path().join("markadd/config.toml");
    let toml = r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "/tmp/def"
templates_dir = "{{vault_root}}/tpl"
captures_dir  = "{{vault_root}}/cap"
macros_dir    = "{{vault_root}}/mac"

[profiles.work]
vault_root = "/tmp/work"
templates_dir = "{{vault_root}}/tpl"
captures_dir  = "{{vault_root}}/cap"
macros_dir    = "{{vault_root}}/mac"
"#;
    write_file(&cfg_path, toml);

    let rc = ConfigLoader::load(Some(&cfg_path), Some("work")).expect("should load");
    assert_eq!(rc.active_profile, "work");
    assert_eq!(rc.vault_root.display().to_string(), "/tmp/work");
}
