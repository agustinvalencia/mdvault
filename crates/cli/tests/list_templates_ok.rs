use assert_cmd::prelude::*;
use predicates::prelude::*; // needed for `.not()`
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

    // XDG-style config location
    let xdg = tmp.path().join("xdg");
    let cfg_dir = xdg.join("mdvault");
    let cfg_path = cfg_dir.join("config.toml");
    fs::create_dir_all(&cfg_dir).unwrap();

    // Templates tree
    let tpl_root = tmp.path().join("vault").join(".mdvault").join("templates");
    let a = tpl_root.join("daily.md");
    let b = tpl_root.join("blog").join("post.md");
    let ignored = tpl_root.join("ignore.tpl.md"); // should be ignored under MD-only rule

    write(&a, "# daily");
    write(&b, "# blog");
    write(&ignored, "nope");

    // Config pointing to our temporary vault/templates
    let toml = format!(
        r#"
version = 1
profile = "default"

[profiles.default]
vault_root = "{vault}"
templates_dir = "{tpl}"
captures_dir  = "{{{{vault_root}}}}/.mdvault/captures"
macros_dir    = "{{{{vault_root}}}}/.mdvault/macros"
"#,
        vault = tmp.path().join("vault").display(),
        tpl = tpl_root.display(),
    );
    fs::write(&cfg_path, toml).unwrap();

    // With clap: global flags can be before the subcommand
    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin!("mdv"));
    cmd.env("XDG_CONFIG_HOME", &xdg);
    cmd.env("NO_COLOR", "1"); // keep output deterministic
    cmd.args([
        "--config",
        cfg_path.to_str().unwrap(),
        "--profile",
        "default",
        "list-templates",
    ]);

    cmd.assert()
        .success()
        .stdout(predicates::str::contains("daily"))
        .stdout(predicates::str::contains("blog/post"))
        .stdout(predicates::str::contains("-- 2 templates --"))
        // `not_contains` doesn't exist; use `contains(...).not()` from PredicateBooleanExt
        .stdout(predicates::str::contains("ignore").not());
}
