// crates/cli/tests/list_templates_ok.rs
use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn write(path: &PathBuf, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn list_templates_reports_markdown_files_only() {
    let tmp = tempdir().unwrap();
    let xdg = tmp.path().join("xdg");
    let cfg_dir = xdg.join("markadd");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();

    let tpl_root = tmp.path().join("vault").join(".markadd").join("templates");
    let a = tpl_root.join("daily.md");
    let b = tpl_root.join("blog").join("post.md");
    let ignored = tpl_root.join("ignore.tpl.md");

    write(&a, "# daily");
    write(&b, "# blog");
    write(&ignored, "nope");

    let toml = format!(
        r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "{vault}"
templates_dir = "{tpl}"
captures_dir  = "{{{{vault_root}}}}/.markadd/captures"
macros_dir    = "{{{{vault_root}}}}/.markadd/macros"
"#,
        vault = tmp.path().join("vault").display(),
        tpl = tpl_root.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("markadd"));
    cmd.env("XDG_CONFIG_HOME", &xdg);
    cmd.arg("list-templates");

    cmd.assert()
        .success()
        .stdout(predicates::str::contains("daily"))
        .stdout(predicates::str::contains("blog/post"))
        .stdout(predicates::str::contains("-- 2 templates --"))
        .stdout(predicates::str::not_contains("ignore")); // should not see ignore.tpl.md
}
